//! In-app Mica: a live, blurred crop of the user's wallpaper drawn as the
//! window backdrop under translucent UI layers.
//!
//! Real Mica samples the wallpaper *behind the window* every frame. This
//! module replicates that without DWM: a background tracker follows the
//! window rect (500ms ticks) and regenerates a blurred crop of exactly the
//! wallpaper region behind the window, so moving/resizing the window (or
//! changing the wallpaper/slideshow image) updates the backdrop.
//!
//! Pipeline per regen: wallpaper layout math (Fill/Fit/Stretch/Center/Span,
//! from the same registry values DWM uses) → crop window region (+bleed
//! margin) → downscale to ~512px → gaussian blur → versioned JPEG. Versioned
//! filenames bust GPUI's image cache and make staleness impossible: a new
//! wallpaper/position always yields a new path. Old files are pruned.

use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::{OnceLock, RwLock};

/// Current backdrop path for the renderer (cheap read, called per frame).
static BACKDROP_PATH: OnceLock<RwLock<Option<PathBuf>>> = OnceLock::new();

fn slot() -> &'static RwLock<Option<PathBuf>> {
    BACKDROP_PATH.get_or_init(|| RwLock::new(None))
}

/// Current backdrop path for the renderer (cheap read, called per frame).
pub fn current() -> Option<PathBuf> {
    slot().read().ok().and_then(|g| g.clone())
}

fn set(p: Option<PathBuf>) {
    if let Ok(mut g) = slot().write() {
        *g = p;
    }
}

fn cache_dir() -> Option<PathBuf> {
    dirs::data_local_dir().map(|d| d.join("files-rs"))
}

/// Newest cached crop (instant first paint, zero work).
fn newest_cache() -> Option<PathBuf> {
    let dir = cache_dir()?;
    std::fs::read_dir(&dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with("mica_") && n.ends_with(".jpg"))
                .unwrap_or(false)
        })
        .max_by_key(|p| {
            std::fs::metadata(p)
                .ok()
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        })
}

/// Publish last run's crop instantly (called on window open).
pub fn prime() {
    // Migrate the pre-tracker single file once, then rely on versioned crops.
    if let Some(dir) = cache_dir() {
        let legacy = dir.join("mica_wallpaper.jpg");
        if legacy.is_file() && newest_cache().is_none() {
            let _ = std::fs::rename(&legacy, dir.join("mica_legacy.jpg"));
        }
    }
    if let Some(c) = newest_cache() {
        set(Some(c));
    }
}

/// One-shot full-image blur for instant first paint (old path, versioned).
pub fn ensure_async() {
    let src = match current_wallpaper() {
        Some(s) => s,
        None => return,
    };
    if current().is_some() {
        return;
    }
    if let Some(p) = render_full(&src) {
        if current().is_none() {
            set(Some(p));
        }
    }
}

fn hash_tag(parts: &[u64]) -> String {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    parts.hash(&mut h);
    format!("{:016x}", h.finish())
}

// ── wallpaper source + layout ─────────────────────────────────────────────

struct Wallpaper {
    path: PathBuf,
    iw: u32,
    ih: u32,
    style: WallpaperStyle,
    mtime_ns: u64,
    len: u64,
}

#[derive(Clone, Copy)]
enum WallpaperStyle {
    Fill,
    Stretch,
    Fit,
    Center,
    Span,
    Tile,
}

fn current_wallpaper() -> Option<Wallpaper> {
    #[cfg(not(windows))]
    {
        return None;
    }
    #[cfg(windows)]
    {
        let hkcu = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER);
        let desktop = hkcu.open_subkey(r"Control Panel\Desktop").ok()?;
        let raw: String = desktop.get_value("Wallpaper").ok()?;
        let path = PathBuf::from(raw);
        if !path.is_file() {
            return None;
        }
        let (iw, ih) = image::image_dimensions(&path).ok()?;
        if iw == 0 || ih == 0 {
            return None;
        }
        let style_raw: String = desktop.get_value("WallpaperStyle").unwrap_or_default();
        let tile_raw: String = desktop.get_value("TileWallpaper").unwrap_or_default();
        let style = match (style_raw.as_str(), tile_raw.as_str()) {
            ("10", _) => WallpaperStyle::Fill,
            ("2", _) => WallpaperStyle::Stretch,
            ("6", _) => WallpaperStyle::Fit,
            ("22", _) => WallpaperStyle::Span,
            (_, "1") => WallpaperStyle::Tile,
            _ => WallpaperStyle::Center,
        };
        let meta = std::fs::metadata(&path).ok()?;
        let mtime_ns = meta
            .modified()
            .ok()?
            .duration_since(std::time::UNIX_EPOCH)
            .ok()?
            .as_nanos() as u64;
        Some(Wallpaper {
            path,
            iw,
            ih,
            style,
            mtime_ns,
            len: meta.len(),
        })
    }
}

#[cfg(windows)]
#[derive(Clone, Copy, Debug, PartialEq)]
struct ScreenRect {
    x: i32,
    y: i32,
    w: i32,
    h: i32,
}

#[cfg(windows)]
fn virtual_screen() -> ScreenRect {
    use windows::Win32::UI::WindowsAndMessaging::{
        GetSystemMetrics, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN,
        SM_YVIRTUALSCREEN,
    };
    unsafe {
        ScreenRect {
            x: GetSystemMetrics(SM_XVIRTUALSCREEN),
            y: GetSystemMetrics(SM_YVIRTUALSCREEN),
            w: GetSystemMetrics(SM_CXVIRTUALSCREEN),
            h: GetSystemMetrics(SM_CYVIRTUALSCREEN),
        }
    }
}

#[cfg(windows)]
fn window_rect(hwnd: isize) -> Option<ScreenRect> {
    use windows::Win32::Foundation::{HWND, RECT};
    use windows::Win32::UI::WindowsAndMessaging::GetWindowRect;
    unsafe {
        let mut r = RECT::default();
        GetWindowRect(HWND(hwnd as *mut core::ffi::c_void), &mut r).ok()?;
        let w = r.right - r.left;
        let h = r.bottom - r.top;
        if w <= 0 || h <= 0 || r.left <= -10000 {
            return None; // minimized / degenerate
        }
        Some(ScreenRect {
            x: r.left,
            y: r.top,
            w,
            h,
        })
    }
}

/// Map a virtual-screen rect to wallpaper-image pixels, mirroring DWM's
/// layout for the configured style. Returns None for Tile (full-image blur).
#[cfg(windows)]
fn layout_map(wp: &Wallpaper, v: &ScreenRect) -> Option<(f64, f64, f64, f64)> {
    let (iw, ih) = (wp.iw as f64, wp.ih as f64);
    let (vw, vh) = (v.w as f64, v.h as f64);
    if vw <= 0.0 || vh <= 0.0 {
        return None;
    }
    match wp.style {
        WallpaperStyle::Fill => {
            let s = (vw / iw).max(vh / ih);
            let dw = iw * s;
            let dh = ih * s;
            Some((s, s, v.x as f64 + (vw - dw) / 2.0, v.y as f64 + (vh - dh) / 2.0))
        }
        WallpaperStyle::Span => {
            // Span == cover over the whole virtual screen.
            let s = (vw / iw).max(vh / ih);
            let dw = iw * s;
            let dh = ih * s;
            Some((s, s, v.x as f64 + (vw - dw) / 2.0, v.y as f64 + (vh - dh) / 2.0))
        }
        WallpaperStyle::Stretch => Some((vw / iw, vh / ih, v.x as f64, v.y as f64)),
        WallpaperStyle::Fit => {
            let s = (vw / iw).min(vh / ih);
            let dw = iw * s;
            let dh = ih * s;
            Some((s, s, v.x as f64 + (vw - dw) / 2.0, v.y as f64 + (vh - dh) / 2.0))
        }
        WallpaperStyle::Center => Some((
            1.0,
            1.0,
            v.x as f64 + (vw - iw) / 2.0,
            v.y as f64 + (vh - ih) / 2.0,
        )),
        WallpaperStyle::Tile => None,
    }
}

// ── rendering ─────────────────────────────────────────────────────────────

/// Blur the whole wallpaper (first paint / Tile fallback).
fn render_full(src: &Wallpaper) -> Option<PathBuf> {
    let img = image::open(&src.path).ok()?;
    let small = img.resize_to_fill(512, 288, image::imageops::FilterType::Triangle);
    let blurred = small.blur(9.0);
    save_versioned(&blurred, src.mtime_ns, src.len, 0, 0, 0, 0)
}

/// Blur the wallpaper region behind `win` (+bleed margin for the blur).
#[cfg(windows)]
fn render_crop(src: &Wallpaper, win: &ScreenRect) -> Option<PathBuf> {
    const BLEED: i32 = 160; // screen px; blur must sample beyond the edge
    let v = virtual_screen();
    let (sx, sy, ox, oy) = layout_map(src, &v)?;
    // Window region (expanded) in virtual-screen coords → image coords.
    let x0 = (win.x - BLEED).max(v.x) as f64;
    let y0 = (win.y - BLEED).max(v.y) as f64;
    let x1 = (win.x + win.w + BLEED).min(v.x + v.w) as f64;
    let y1 = (win.y + win.h + BLEED).min(v.y + v.h) as f64;
    if x1 <= x0 || y1 <= y0 {
        return None;
    }
    let mut ix0 = ((x0 - ox) / sx) as i64;
    let mut iy0 = ((y0 - oy) / sy) as i64;
    let mut ix1 = ((x1 - ox) / sx).ceil() as i64;
    let mut iy1 = ((y1 - oy) / sy).ceil() as i64;
    ix0 = ix0.clamp(0, src.iw as i64);
    iy0 = iy0.clamp(0, src.ih as i64);
    ix1 = ix1.clamp(0, src.iw as i64);
    iy1 = iy1.clamp(0, src.ih as i64);
    if ix1 - ix0 < 8 || iy1 - iy0 < 8 {
        return None;
    }
    let full = image::open(&src.path).ok()?;
    let crop = full.crop_imm(ix0 as u32, iy0 as u32, (ix1 - ix0) as u32, (iy1 - iy0) as u32);
    // Downscale to ~512px wide, then blur (fast and smooth).
    let w = crop.width().min(512);
    let h = ((crop.height() as f64) * (w as f64 / crop.width() as f64)).round() as u32;
    let small = image::imageops::resize(&crop, w, h.max(1), image::imageops::FilterType::Triangle);
    let blurred = image::DynamicImage::ImageRgba8(small).blur(10.0);
    save_versioned(
        &blurred,
        src.mtime_ns,
        src.len,
        win.x as u64,
        win.y as u64,
        win.w as u64,
        win.h as u64,
    )
}

fn save_versioned(
    img: &image::DynamicImage,
    mtime_ns: u64,
    len: u64,
    x: u64,
    y: u64,
    w: u64,
    h: u64,
) -> Option<PathBuf> {
    let dir = cache_dir()?;
    let _ = std::fs::create_dir_all(&dir);
    // Quantize position (32px buckets): tiny drags reuse the same crop.
    let tag = hash_tag(&[mtime_ns, len, x / 32, y / 32, w / 32, h / 32]);
    let path = dir.join(format!("mica_{}.jpg", tag));
    if !path.is_file() {
        img.save_with_format(&path, image::ImageFormat::Jpeg).ok()?;
        prune(&dir);
    }
    Some(path)
}

/// Keep only the 3 newest crops; the rest is stale wallpaper/positions.
fn prune(dir: &PathBuf) {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()
        .map(|rd| {
            rd.flatten()
                .map(|e| e.path())
                .filter(|p| {
                    p.file_name()
                        .and_then(|n| n.to_str())
                        .map(|n| n.starts_with("mica_") && n.ends_with(".jpg"))
                        .unwrap_or(false)
                })
                .collect()
        })
        .unwrap_or_default();
    files.sort_by_key(|p| {
        std::fs::metadata(p)
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
    });
    while files.len() > 3 {
        if let Some(old) = files.first() {
            let _ = std::fs::remove_file(old);
        }
        files.remove(0);
    }
}

// ── tracker ───────────────────────────────────────────────────────────────

/// Background loop: follows the window and the wallpaper source, publishes
/// a fresh blurred crop whenever either changes. Never clears the current
/// backdrop on failure (a stale blur beats a flat window).
#[cfg(windows)]
pub fn track_window(hwnd: isize) {
    let mut last_rect: Option<ScreenRect> = None;
    let mut last_src = (0u64, 0u64);
    loop {
        std::thread::sleep(std::time::Duration::from_millis(500));
        let src = match current_wallpaper() {
            Some(s) => s,
            None => continue,
        };
        let src_id = (src.mtime_ns, src.len);
        let rect = window_rect(hwnd);
        let rect_changed = rect != last_rect;
        // Skip work while minimized; keep serving the last good crop.
        let r = match rect {
            Some(r) => r,
            None => continue,
        };
        if src_id != last_src || rect_changed {
            let fresh = render_crop(&src, &r).or_else(|| render_full(&src));
            if let Some(p) = fresh {
                set(Some(p));
                last_rect = Some(r);
                last_src = src_id;
            }
        }
    }
}

/// Non-Windows stub (keeps main.rs cfg-simple).
#[cfg(not(windows))]
pub fn track_window(_hwnd: isize) {}

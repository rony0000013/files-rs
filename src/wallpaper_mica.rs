//! In-app Mica: a live, blurred copy of the user's wallpaper drawn as the
//! window backdrop under translucent UI layers.
//!
//! Real Mica samples the wallpaper *behind the window* every frame. Regen
//! per move would cost a JPEG decode + blur each tick, so this module
//! splits the work: the wallpaper is blurred ONCE per source/screen change
//! (background `watch_source`, ~2s ticks), and every frame the renderer just
//! *slides* that full-screen blur under the window from the live window
//! bounds (`placement`). Moving/resizing costs zero regen — pure layout —
//! so tracking is instant, and wallpaper changes rebuild the blur.

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

/// Placement of the full-screen blur for one frame (all logical px).
#[derive(Clone, Debug)]
pub struct MicaGeo {
    pub path: PathBuf,
    pub left: f32,
    pub top: f32,
    pub w: f32,
    pub h: f32,
}

fn cache_dir() -> Option<PathBuf> {
    dirs::data_local_dir().map(|d| d.join("files-rs"))
}

fn hash_tag(parts: &[u64]) -> String {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    parts.hash(&mut h);
    format!("{:016x}", h.finish())
}

/// Newest scheme-2 blur: `mica2_{vw}x{vh}_{tag}.jpg` (+ its screen dims).
fn newest_full() -> Option<(PathBuf, i32, i32)> {
    let dir = cache_dir()?;
    let mut best: Option<(PathBuf, i32, i32, std::time::SystemTime)> = None;
    for e in std::fs::read_dir(&dir).ok()?.flatten() {
        let p = e.path();
        let name = p.file_name()?.to_str()?.to_string();
        if !name.starts_with("mica2_") || !name.ends_with(".jpg") {
            continue;
        }
        let dims = name.trim_start_matches("mica2_").trim_end_matches(".jpg");
        let (ws, hs) = dims.split_once('x')?;
        let (tagpos, _) = hs.split_once('_').unwrap_or((hs, ""));
        let _ = tagpos;
        let vw: i32 = ws.parse().ok()?;
        // name is mica2_{vw}x{vh}_{tag}: split tail for vh.
        let rest = dims.split_once('x')?.1;
        let (vh_s, _) = rest.split_once('_')?;
        let vh: i32 = vh_s.parse().ok()?;
        let mt = std::fs::metadata(&p).ok()?.modified().ok()?;
        if best.as_ref().map(|b| mt > b.3).unwrap_or(true) {
            best = Some((p, vw, vh, mt));
        }
    }
    best.map(|(p, vw, vh, _)| (p, vw, vh))
}

/// Publish last run's blur instantly (called on window open). Drops
/// pre-tracker caches (wrong framing for the slide scheme).
pub fn prime() {
    if let Some(dir) = cache_dir() {
        let entries = std::fs::read_dir(&dir).ok().map(|rd| {
            rd.flatten()
                .map(|e| e.path())
                .filter(|p| {
                    p.file_name()
                        .and_then(|n| n.to_str())
                        .map(|n| {
                            n == "mica_wallpaper.jpg"
                                || (n.starts_with("mica_") && !n.starts_with("mica2_"))
                        })
                        .unwrap_or(false)
                })
                .collect::<Vec<_>>()
        });
        for old in entries.into_iter().flatten() {
            let _ = std::fs::remove_file(old);
        }
    }
    if let Some((p, _, _)) = newest_full() {
        set(Some(p));
    }
}

/// One-shot full blur for instant first paint when no cache exists yet.
pub fn ensure_async() {
    if current().is_some() {
        return;
    }
    let src = match current_wallpaper() {
        Some(s) => s,
        None => return,
    };
    let v = virtual_screen();
    if let Some(p) = render_full_cover(&src, &v) {
        if current().is_none() {
            set(Some(p));
        }
    }
}

/// Per-frame placement: slide the full-screen blur under the live window
/// origin. Zero regen — pure layout, so tracking is instant.
pub fn placement(win_x: f32, win_y: f32, scale: f32) -> Option<MicaGeo> {
    let s = scale.max(0.5);
    let (path, vw, vh) = newest_full()?;
    let v = virtual_screen();
    Some(MicaGeo {
        path,
        left: v.x as f32 / s - win_x,
        top: v.y as f32 / s - win_y,
        w: vw as f32 / s,
        h: vh as f32 / s,
    })
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

#[cfg(not(windows))]
#[derive(Clone, Copy, Debug, PartialEq)]
struct ScreenRect {
    x: i32,
    y: i32,
    w: i32,
    h: i32,
}

#[cfg(not(windows))]
fn virtual_screen() -> ScreenRect {
    ScreenRect { x: 0, y: 0, w: 0, h: 0 }
}

/// Virtual-screen rect → wallpaper-image scale + origin, mirroring DWM's
/// layout for the configured style. None for Tile (whole-image blur).
fn layout_map(wp: &Wallpaper, v: &ScreenRect) -> Option<(f64, f64, f64, f64)> {
    let (iw, ih) = (wp.iw as f64, wp.ih as f64);
    let (vw, vh) = (v.w as f64, v.h as f64);
    if vw <= 0.0 || vh <= 0.0 {
        return None;
    }
    match wp.style {
        WallpaperStyle::Fill | WallpaperStyle::Span => {
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

/// Blur the whole virtual screen once (regen only on source/screen change).
fn render_full_cover(src: &Wallpaper, v: &ScreenRect) -> Option<PathBuf> {
    let full = image::open(&src.path).ok()?;
    let (fw, fh) = (full.width(), full.height());
    // Image region covering the virtual screen (clamped to the image).
    let (mut ix0, mut iy0, mut ix1, mut iy1) = (0i64, 0i64, fw as i64, fh as i64);
    if let Some((sx, sy, ox, oy)) = layout_map(src, v) {
        ix0 = ((v.x as f64 - ox) / sx).round() as i64;
        iy0 = ((v.y as f64 - oy) / sy).round() as i64;
        ix1 = ((v.x as f64 + v.w as f64 - ox) / sx).round() as i64;
        iy1 = ((v.y as f64 + v.h as f64 - oy) / sy).round() as i64;
    }
    ix0 = ix0.clamp(0, fw as i64);
    iy0 = iy0.clamp(0, fh as i64);
    ix1 = ix1.clamp(0, fw as i64);
    iy1 = iy1.clamp(0, fh as i64);
    if ix1 - ix0 < 8 || iy1 - iy0 < 8 {
        return None;
    }
    let crop = full.crop_imm(ix0 as u32, iy0 as u32, (ix1 - ix0) as u32, (iy1 - iy0) as u32);
    // Blur at ~768px wide, stretched to the screen aspect (smooth anyway).
    let tw = 768u32.min(crop.width().max(64));
    let th = ((v.h as f64) * (tw as f64 / v.w.max(1) as f64)).round() as u32;
    let small = image::imageops::resize(
        &crop,
        tw,
        th.max(1),
        image::imageops::FilterType::Triangle,
    );
    let blurred = image::DynamicImage::ImageRgba8(small).blur(12.0);
    let dir = cache_dir()?;
    let _ = std::fs::create_dir_all(&dir);
    let tag = hash_tag(&[src.mtime_ns, src.len, v.w as u64, v.h as u64]);
    let path = dir.join(format!("mica2_{}x{}_{}.jpg", v.w, v.h, tag));
    if !path.is_file() {
        blurred
            .save_with_format(&path, image::ImageFormat::Jpeg)
            .ok()?;
        prune(&dir);
    }
    Some(path)
}

/// Keep only the 2 newest scheme-2 blurs.
fn prune(dir: &PathBuf) {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()
        .map(|rd| {
            rd.flatten()
                .map(|e| e.path())
                .filter(|p| {
                    p.file_name()
                        .and_then(|n| n.to_str())
                        .map(|n| n.starts_with("mica"))
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
    while files.len() > 2 {
        if let Some(old) = files.first() {
            let _ = std::fs::remove_file(old);
        }
        files.remove(0);
    }
}

// ── watcher ───────────────────────────────────────────────────────────────

/// Background loop: rebuilds the full-screen blur only when the wallpaper
/// source or screen geometry changes (~2s ticks, stat-only when idle).
/// Position tracking needs no work here — the renderer slides the blur
/// per-frame from the live window bounds.
pub fn watch_source() {
    let mut last = (0u64, 0u64, 0i32, 0i32, 0i32, 0i32);
    loop {
        std::thread::sleep(std::time::Duration::from_millis(2000));
        let v = virtual_screen();
        let src = match current_wallpaper() {
            Some(s) => s,
            None => continue,
        };
        let id = (src.mtime_ns, src.len, v.x, v.y, v.w, v.h);
        if id != last {
            if let Some(p) = render_full_cover(&src, &v) {
                set(Some(p));
                last = id;
            }
        } else if current().is_none() {
            if let Some((p, _, _)) = newest_full() {
                set(Some(p));
            }
        }
    }
}

/// Non-Windows stub (keeps main.rs cfg-simple).
#[cfg(not(windows))]
pub fn track_window(_hwnd: isize) {}

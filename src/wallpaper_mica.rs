//! In-app Mica: a blurred copy of the user's current wallpaper, drawn as the
//! window backdrop under translucent UI layers.
//!
//! Why in-app instead of DWM Mica: GPUI's DirectComposition swapchain on
//! this machine presents fully opaque (verified: 40%-alpha chrome over a
//! bright wallpaper still renders flat), and `DWMWA_SYSTEMBACKDROP_TYPE`
//! never reaches the screen through it. Mica is literally blurred wallpaper
//! + dark tint, so rendering those exact pixels ourselves is visually
//! identical and works on every machine, Win10 included.
//!
//! Flow: `prime()` publishes last run's cache instantly (zero startup cost),
//! then a background thread regenerates when the wallpaper is new/stale.

use std::path::PathBuf;
use std::sync::{OnceLock, RwLock};

static BLUR_PATH: OnceLock<RwLock<Option<PathBuf>>> = OnceLock::new();

fn slot() -> &'static RwLock<Option<PathBuf>> {
    BLUR_PATH.get_or_init(|| RwLock::new(None))
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

fn cache_path() -> Option<PathBuf> {
    dirs::data_local_dir().map(|d| d.join("files-rs").join("mica_wallpaper.jpg"))
}

/// Publish last run's cache immediately (no work, just a path check).
pub fn prime() {
    if let Some(c) = cache_path() {
        if c.is_file() {
            set(Some(c));
        }
    }
}

/// Background thread entry: regenerate the blur iff missing or stale.
pub fn ensure_async() {
    let cache = match cache_path() {
        Some(c) => c,
        None => return,
    };
    let wp = current_wallpaper();
    let wp_mtime = wp
        .as_ref()
        .and_then(|p| std::fs::metadata(p).ok())
        .and_then(|m| m.modified().ok());
    let cache_mtime = std::fs::metadata(&cache)
        .ok()
        .and_then(|m| m.modified().ok());
    let stale = match (wp_mtime, cache_mtime) {
        (Some(w), Some(c)) => w > c,
        _ => true,
    };
    if cache.is_file() && !stale {
        set(Some(cache));
        return;
    }
    if render_blur(&cache) {
        set(Some(cache));
    } else if cache.is_file() {
        // Keep serving the old blur rather than going flat.
        set(Some(cache));
    }
}

fn current_wallpaper() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        let hkcu = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER);
        let desktop = hkcu.open_subkey(r"Control Panel\Desktop").ok()?;
        let raw: String = desktop.get_value("Wallpaper").ok()?;
        let p = PathBuf::from(raw);
        if p.is_file() {
            return Some(p);
        }
        None
    }
    #[cfg(not(windows))]
    {
        None
    }
}

/// Downscale → gaussian blur → JPEG. Cheap: blur runs on a ~512px image.
fn render_blur(cache: &PathBuf) -> bool {
    let wp = match current_wallpaper() {
        Some(p) => p,
        None => return false,
    };
    let img = match image::open(&wp) {
        Ok(i) => i,
        Err(e) => {
            tracing::warn!("wallpaper_mica: cannot open {:?}: {}", wp, e);
            return false;
        }
    };
    let small = img.resize_to_fill(
        512,
        288,
        image::imageops::FilterType::Triangle,
    );
    let blurred = small.blur(9.0);
    if let Some(parent) = cache.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match blurred.save_with_format(cache, image::ImageFormat::Jpeg) {
        Ok(()) => true,
        Err(e) => {
            tracing::warn!("wallpaper_mica: cannot save {:?}: {}", cache, e);
            false
        }
    }
}

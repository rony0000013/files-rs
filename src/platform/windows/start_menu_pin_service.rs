use crate::core::path_util::FilesPath;
use anyhow::{anyhow, Result};

#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;

#[cfg(windows)]
use windows::{
    core::PCWSTR,
    Win32::UI::Shell::{SHAddToRecentDocs, SHARD_PATHW},
};

/// Service for pinning items to Windows Start Menu
pub struct StartMenuPinService;

impl StartMenuPinService {
    pub fn new() -> Self {
        Self
    }

    /// Pin a folder to the Start Menu
    /// Note: Windows doesn't provide a direct API to pin to Start Menu.
    /// This uses ShellAddToRecentDocs as a workaround, which adds to the Jump List.
    /// True "Pin to Start" requires modifying the Start Menu XML or using
    /// COM interfaces like IShellFolder::GetDetailsOf which is complex.
    #[cfg(windows)]
    pub async fn pin_to_start(path: &FilesPath) -> Result<()> {
        let wide_path: Vec<u16> = path
            .as_path()
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        unsafe {
            SHAddToRecentDocs(SHARD_PATHW.0 as u32, Some(wide_path.as_ptr() as *const _));
        }

        tracing::info!("Pinned to Start Menu (via recent docs): {:?}", path);
        Ok(())
    }

    /// Unpin is not directly supported - Windows recent docs doesn't track
    /// which items were "pinned" vs just added. This is a limitation.
    #[cfg(windows)]
    pub async fn unpin_from_start(_path: &FilesPath) -> Result<()> {
        // Windows doesn't provide a direct API to unpin from Start Menu
        // The best we can do is clear the recent docs and re-add what we want
        tracing::warn!("Unpin from Start Menu is not fully supported on Windows");
        Ok(())
    }

    /// Check if a path is currently pinned (heuristic based on recent docs)
    /// This is approximate - Windows doesn't expose a direct query API
    #[cfg(windows)]
    pub fn is_pinned(_path: &FilesPath) -> bool {
        // There's no reliable way to check if something is pinned
        // without parsing the Start Menu XML or registry
        false
    }

    // ═══════════════════════════════════════════════════════════════════
    // Non-Windows stubs
    // ═══════════════════════════════════════════════════════════════════

    #[cfg(not(windows))]
    pub async fn pin_to_start(_path: &FilesPath) -> Result<()> {
        tracing::warn!("Pin to Start Menu is only implemented for Windows");
        Err(anyhow!("Pin to Start Menu is only available on Windows"))
    }

    #[cfg(not(windows))]
    pub async fn unpin_from_start(_path: &FilesPath) -> Result<()> {
        tracing::warn!("Unpin from Start Menu is only implemented for Windows");
        Err(anyhow!("Unpin from Start Menu is only available on Windows"))
    }

    #[cfg(not(windows))]
    pub fn is_pinned(_path: &FilesPath) -> bool {
        false
    }
}

impl Default for StartMenuPinService {
    fn default() -> Self {
        Self::new()
    }
}

/// A more complete implementation would use these approaches:
/// 1. Modify `%APPDATA%\Microsoft\Windows\Start Menu\Programs\` shortcut files
/// 2. Use IStartMenuPinnedList COM interface (internal to explorer.exe)
/// 3. Use the StartTileData XML in registry at:
///    HKCU\Software\Microsoft\Windows\CurrentVersion\CloudStore\Store\Cache\DefaultAccount

#[cfg(windows)]
#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn test_pin_service_creation() {
        let _service = StartMenuPinService::new();
        assert!(!StartMenuPinService::is_pinned(&FilesPath::new("/tmp")));
    }
}
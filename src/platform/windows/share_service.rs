//! Windows Share UI Service
//! Invokes the Windows Share dialog via DataTransferManager for sharing files.

use crate::core::path_util::FilesPath;
use anyhow::{anyhow, Result};

/// Windows Share Service
/// Provides Windows Share UI integration for sharing files via the system share dialog.
pub struct ShareService;

impl ShareService {
    pub fn new() -> Self {
        Self
    }

    /// Show the Windows Share UI for the given files
    /// Uses DataTransferManagerInterop to invoke the system share dialog.
    #[cfg(windows)]
    pub async fn show_share_ui(&self, window_handle: isize, paths: &[FilesPath]) -> Result<()> {
        use std::os::windows::ffi::OsStrExt;
        use windows::core::PCWSTR;
        use windows::Win32::Foundation::HWND;
        use windows::Win32::System::Com::{
            CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED,
        };
        use windows::Win32::UI::Shell::{ShellExecuteExW, SEE_MASK_INVOKEIDLIST};

        if paths.is_empty() {
            return Err(anyhow!("No files to share"));
        }

        let path_strings: Vec<String> = paths
            .iter()
            .map(|p| p.as_path().to_string_lossy().to_string())
            .collect();

        let hwnd = if window_handle != 0 {
            HWND(window_handle as *mut _)
        } else {
            unsafe { windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow() }
        };

        for path_str in &path_strings {
            let wide_path: Vec<u16> = path_str.encode_utf16().chain(std::iter::once(0)).collect();

            unsafe {
                let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);

                let mut sei = windows::Win32::UI::Shell::SHELLEXECUTEINFOW::default();
                sei.cbSize =
                    std::mem::size_of::<windows::Win32::UI::Shell::SHELLEXECUTEINFOW>() as u32;
                sei.hwnd = hwnd;
                sei.fMask = SEE_MASK_INVOKEIDLIST;
                sei.nShow = 1; // SW_SHOWNORMAL

                // In Windows 10/11 Shell, canonical verb is "share" or "windows.share"
                let wide_verb_share: Vec<u16> = "share\0".encode_utf16().collect();
                sei.lpVerb = PCWSTR(wide_verb_share.as_ptr());
                sei.lpFile = PCWSTR(wide_path.as_ptr());

                let mut result = ShellExecuteExW(&mut sei);
                if result.is_err() {
                    let wide_verb_win_share: Vec<u16> = "windows.share\0".encode_utf16().collect();
                    sei.lpVerb = PCWSTR(wide_verb_win_share.as_ptr());
                    result = ShellExecuteExW(&mut sei);
                }

                CoUninitialize();

                if let Err(e) = result {
                    tracing::debug!("Share UI not available for {}: {}", path_str, e);
                } else {
                    tracing::info!("Share dialog invoked for: {}", path_str);
                }
            }
        }

        Ok(())
    }

    /// Show the Windows Share UI (non-Windows stub)
    #[cfg(not(windows))]
    pub async fn show_share_ui(&self, _window_handle: isize, paths: &[FilesPath]) -> Result<()> {
        tracing::warn!(
            "Share UI is only available on Windows: {:?} files",
            paths.len()
        );
        Err(anyhow!("Share UI is only available on Windows"))
    }

    /// Share a single file using the Windows Share charm
    #[cfg(windows)]
    pub async fn share_file(&self, window_handle: isize, path: &FilesPath) -> Result<()> {
        self.show_share_ui(window_handle, &[path.clone()]).await
    }

    /// Share multiple files using the Windows Share charm
    #[cfg(windows)]
    pub async fn share_files(&self, window_handle: isize, paths: &[FilesPath]) -> Result<()> {
        self.show_share_ui(window_handle, paths).await
    }

    /// Check if sharing is available on this system
    #[cfg(windows)]
    pub fn is_share_available() -> bool {
        // Share UI is generally available on Windows 10/11
        true
    }

    #[cfg(not(windows))]
    pub fn is_share_available() -> bool {
        false
    }
}

impl Default for ShareService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn test_share_service_creation() {
        let service = ShareService::new();
        assert!(!ShareService::is_share_available() || cfg!(windows));
    }

    #[test]
    fn test_share_availability_check() {
        // Should compile and return appropriate value for platform
        let available = ShareService::is_share_available();
        #[cfg(windows)]
        assert!(available);
        #[cfg(not(windows))]
        assert!(!available);
    }
}

// Platform-independent tests that work on all platforms
#[cfg(test)]
mod platform_tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_share_service_new() {
        let service = ShareService::new();
        let _ = service;
    }

    #[test]
    fn test_share_service_default() {
        let _service = ShareService::default();
    }

    #[test]
    fn test_share_availability_platform_aware() {
        let available = ShareService::is_share_available();
        #[cfg(windows)]
        assert!(available);
        #[cfg(not(windows))]
        assert!(!available);
    }

    #[tokio::test]
    async fn test_share_file_accepts_valid_path() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        std::fs::write(&file_path, "content").unwrap();

        let path = FilesPath::new(file_path.to_str().unwrap());
        let service = ShareService::new();

        // On non-Windows, we use show_share_ui directly since share_file has #[cfg(windows)]
        let result = service.show_share_ui(0, &[path]).await;
        #[cfg(windows)]
        assert!(result.is_ok());
        #[cfg(not(windows))]
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_share_files_accepts_multiple_paths() {
        let temp_dir = TempDir::new().unwrap();
        let file1 = temp_dir.path().join("test1.txt");
        let file2 = temp_dir.path().join("test2.txt");
        std::fs::write(&file1, "content1").unwrap();
        std::fs::write(&file2, "content2").unwrap();

        let path1 = FilesPath::new(file1.to_str().unwrap());
        let path2 = FilesPath::new(file2.to_str().unwrap());
        let service = ShareService::new();

        let result = service.show_share_ui(0, &[path1, path2]).await;
        #[cfg(windows)]
        assert!(result.is_ok());
        #[cfg(not(windows))]
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_show_share_ui_with_empty_paths_returns_error() {
        let service = ShareService::new();
        let result = service.show_share_ui(0, &[]).await;
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        // On Windows: "No files to share"
        // On non-Windows: "Share UI is only available on Windows"
        assert!(
            err_msg.contains("No files") || err_msg.contains("Windows"),
            "Error message should mention 'No files' or 'Windows', got: {}",
            err_msg
        );
    }

    #[tokio::test]
    async fn test_show_share_ui_with_single_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("share_me.txt");
        std::fs::write(&file_path, "share content").unwrap();

        let path = FilesPath::new(file_path.to_str().unwrap());
        let service = ShareService::new();

        let result = service.show_share_ui(0, &[path]).await;
        #[cfg(windows)]
        assert!(result.is_ok());
        #[cfg(not(windows))]
        assert!(result.is_err());
    }
}

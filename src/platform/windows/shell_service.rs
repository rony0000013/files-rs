use crate::core::path_util::FilesPath;
use anyhow::{anyhow, Result};
#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;
#[cfg(windows)]
use windows::core::PCWSTR;
#[cfg(windows)]
use windows::Win32::UI::Shell::{ShellExecuteExW, SEE_MASK_INVOKEIDLIST, SHELLEXECUTEINFOW};

pub struct ShellService;

impl ShellService {
    /// Synchronous core of [`Self::open_properties`] (the body is sync
    /// ShellExecuteExW; the async wrapper exists for callers on async contexts).
    pub fn open_properties_sync(path: &FilesPath) -> Result<()> {
        #[cfg(windows)]
        {
            let wide_path: Vec<u16> = path
                .as_path()
                .as_os_str()
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();
            let wide_op: Vec<u16> = "properties"
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();

            let mut sei = SHELLEXECUTEINFOW::default();
            sei.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
            sei.fMask = SEE_MASK_INVOKEIDLIST;
            sei.lpVerb = PCWSTR(wide_op.as_ptr());
            sei.lpFile = PCWSTR(wide_path.as_ptr());
            sei.nShow = windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL.0;

            unsafe {
                ShellExecuteExW(&mut sei)
                    .map_err(|e| anyhow!("Failed to open properties: {}", e))?;
            }
        }
        #[cfg(not(windows))]
        {
            let _ = path;
            tracing::warn!("open_properties is only implemented for Windows");
        }
        Ok(())
    }

    pub async fn open_properties(path: &FilesPath) -> Result<()> {
        Self::open_properties_sync(path)
    }

    pub async fn format_drive(drive_letter: &str) -> Result<()> {
        #[cfg(windows)]
        {
            let wide_drive: Vec<u16> = format!("{}:", drive_letter)
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            let wide_op: Vec<u16> = "format".encode_utf16().chain(std::iter::once(0)).collect();

            let mut sei = SHELLEXECUTEINFOW::default();
            sei.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
            sei.fMask = SEE_MASK_INVOKEIDLIST;
            sei.lpVerb = PCWSTR(wide_op.as_ptr());
            sei.lpFile = PCWSTR(wide_drive.as_ptr());
            sei.nShow = windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL.0;

            unsafe {
                ShellExecuteExW(&mut sei)
                    .map_err(|e| anyhow!("Failed to open format dialog: {}", e))?;
            }
        }
        #[cfg(not(windows))]
        {
            tracing::warn!("format_drive is only implemented for Windows");
        }
        Ok(())
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[tokio::test]
    #[cfg(not(windows))]
    async fn test_open_properties_creates_no_error() {
        // On non-Windows, this just logs a warning and returns Ok
        let path = FilesPath::new("C:\\test\\nonexistent.txt");
        let result = ShellService::open_properties(&path).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    #[cfg(not(windows))]
    async fn test_format_drive_creates_no_error() {
        // This only logs on non-Windows, returns Ok
        // On Windows, this can actually format drives so we skip it
        let result = ShellService::format_drive("C").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    #[cfg(not(windows))]
    async fn test_open_properties_with_different_paths() {
        // On non-Windows, these just log warnings
        let paths = vec![
            "C:\\Windows\\System32",
            "D:\\Users\\Public",
            "C:\\Program Files",
        ];

        for path_str in paths {
            let path = FilesPath::new(path_str);
            let result = ShellService::open_properties(&path).await;
            assert!(result.is_ok(), "Failed for path: {}", path_str);
        }
    }

    #[tokio::test]
    #[cfg(not(windows))]
    async fn test_format_drive_various_letters() {
        // This only logs on non-Windows
        // On Windows, format can cause data loss so we skip it
        for letter in ['C', 'D', 'X', 'Z'] {
            let result = ShellService::format_drive(&letter.to_string()).await;
            assert!(result.is_ok(), "Failed for drive: {}", letter);
        }
    }
}

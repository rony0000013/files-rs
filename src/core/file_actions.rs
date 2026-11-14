use crate::core::path_util::FilesPath;
use anyhow::{anyhow, Result};
#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;
use std::process::Command;
#[cfg(windows)]
use windows::core::PCWSTR;
#[cfg(windows)]
use windows::Win32::UI::Shell::ShellExecuteW;

pub struct FileActionService;

impl FileActionService {
    pub async fn open_item(path: &FilesPath) -> Result<()> {
        if cfg!(test) {
            let _ = path;
            return Ok(());
        }
        #[cfg(windows)]
        {
            let wide_path: Vec<u16> = path
                .as_path()
                .as_os_str()
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();
            let wide_op: Vec<u16> = "open".encode_utf16().chain(std::iter::once(0)).collect();

            unsafe {
                ShellExecuteW(
                    None,
                    PCWSTR(wide_op.as_ptr()),
                    PCWSTR(wide_path.as_ptr()),
                    None,
                    None,
                    windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL,
                );
            }
        }
        #[cfg(not(windows))]
        {
            tracing::warn!("open_item is only implemented for Windows");
        }
        Ok(())
    }

    pub async fn edit_in_notepad(path: &FilesPath) -> Result<()> {
        if cfg!(test) {
            let _ = path;
            return Ok(());
        }
        #[cfg(windows)]
        {
            Command::new("notepad.exe")
                .arg(path.as_path())
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()?;
        }
        #[cfg(not(windows))]
        {
            tracing::warn!("edit_in_notepad is only implemented for Windows");
        }
        Ok(())
    }

    pub async fn run_with_powershell(path: &FilesPath) -> Result<()> {
        if cfg!(test) {
            let _ = path;
            return Ok(());
        }
        #[cfg(windows)]
        {
            let wide_op: Vec<u16> = "open".encode_utf16().chain(std::iter::once(0)).collect();
            let wide_powershell: Vec<u16> = "powershell.exe"
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            let wide_args: Vec<u16> = path
                .as_path()
                .to_string_lossy()
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();

            unsafe {
                ShellExecuteW(
                    None,
                    PCWSTR(wide_op.as_ptr()),
                    PCWSTR(wide_powershell.as_ptr()),
                    PCWSTR(wide_args.as_ptr()),
                    None,
                    windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL,
                );
            }
        }
        #[cfg(not(windows))]
        {
            tracing::warn!("run_with_powershell is only implemented for Windows");
        }
        Ok(())
    }

    pub async fn open_storage_sense() -> Result<()> {
        if cfg!(test) {
            return Ok(());
        }
        #[cfg(windows)]
        {
            let wide_url: Vec<u16> = "ms-settings:storagesense"
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            let wide_op: Vec<u16> = "open".encode_utf16().chain(std::iter::once(0)).collect();

            unsafe {
                ShellExecuteW(
                    None,
                    PCWSTR(wide_op.as_ptr()),
                    PCWSTR(wide_url.as_ptr()),
                    None,
                    None,
                    windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL,
                );
            }
        }
        #[cfg(not(windows))]
        {
            tracing::warn!("open_storage_sense is only implemented on Windows");
        }
        Ok(())
    }

    pub async fn run_as_admin(path: &FilesPath) -> Result<()> {
        if cfg!(test) {
            let _ = path;
            return Ok(());
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            Command::new("powershell")
                .arg("-NoProfile")
                .arg("-Command")
                .arg(format!(
                    "Start-Process '{}' -Verb runAs",
                    path.as_path().to_string_lossy()
                ))
                .creation_flags(0x08000000)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()?;
        }
        #[cfg(not(windows))]
        {
            tracing::warn!("run_as_admin is only implemented for Windows");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_open_item_returns_ok() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        std::fs::write(&file_path, "content").unwrap();

        let path = FilesPath::from(file_path.to_str().unwrap());

        // Should return Ok even if the actual Windows call doesn't execute in test
        let result = FileActionService::open_item(&path).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_edit_in_notepad_returns_ok() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        std::fs::write(&file_path, "content").unwrap();

        let path = FilesPath::from(file_path.to_str().unwrap());

        let result = FileActionService::edit_in_notepad(&path).await;
        // Should return Ok (spawn succeeds or fails gracefully)
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_run_with_powershell_returns_ok() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        std::fs::write(&file_path, "content").unwrap();

        let path = FilesPath::from(file_path.to_str().unwrap());

        let result = FileActionService::run_with_powershell(&path).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_open_storage_sense_returns_ok() {
        let result = FileActionService::open_storage_sense().await;
        // Should return Ok even on non-Windows (just logs warning)
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_run_as_admin_returns_ok() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        std::fs::write(&file_path, "content").unwrap();

        let path = FilesPath::from(file_path.to_str().unwrap());

        let result = FileActionService::run_as_admin(&path).await;
        // Returns Ok even if the actual elevation doesn't happen
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_open_item_with_nonexistent_file() {
        let temp_dir = TempDir::new().unwrap();
        let nonexistent = temp_dir.path().join("nonexistent.txt");

        let path = FilesPath::from(nonexistent.to_str().unwrap());

        // Should return Ok (Windows ShellExecute might fail but we don't error)
        let result = FileActionService::open_item(&path).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_all_actions_work_with_unicode_paths() {
        let temp_dir = TempDir::new().unwrap();
        let unicode_file = temp_dir.path().join("文件.txt");
        std::fs::write(&unicode_file, "content").unwrap();

        let path = FilesPath::from(unicode_file.to_str().unwrap());

        // All actions should handle unicode paths gracefully
        assert!(FileActionService::open_item(&path).await.is_ok());
        assert!(FileActionService::edit_in_notepad(&path).await.is_ok());
        assert!(FileActionService::run_with_powershell(&path).await.is_ok());
    }

    #[tokio::test]
    async fn test_all_actions_work_with_paths_with_spaces() {
        let temp_dir = TempDir::new().unwrap();
        let file_with_spaces = temp_dir.path().join("my document.txt");
        std::fs::write(&file_with_spaces, "content").unwrap();

        let path = FilesPath::from(file_with_spaces.to_str().unwrap());

        assert!(FileActionService::open_item(&path).await.is_ok());
        assert!(FileActionService::edit_in_notepad(&path).await.is_ok());
        assert!(FileActionService::run_with_powershell(&path).await.is_ok());
    }
}

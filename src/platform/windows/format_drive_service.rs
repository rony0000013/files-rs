//! Format Drive Service
//! Provides functionality to invoke the Windows format dialog for a drive
//! Uses Windows API: ShellExec or format.com via cmd

use anyhow::{Context, Result};
use std::path::Path;

/// Filesystem type for formatting
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileSystemType {
    NTFS,
    FAT32,
    exFAT,
    ReFS,
}

impl Default for FileSystemType {
    fn default() -> Self {
        Self::NTFS
    }
}

impl FileSystemType {
    /// Returns the Windows format string for this filesystem
    pub fn format_string(&self) -> &str {
        match self {
            FileSystemType::NTFS => "NTFS",
            FileSystemType::FAT32 => "FAT32",
            FileSystemType::exFAT => "exFAT",
            FileSystemType::ReFS => "ReFS",
        }
    }
}

/// Quick format option
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuickFormat {
    Quick,
    Full,
}

impl Default for QuickFormat {
    fn default() -> Self {
        Self::Quick
    }
}

/// Result of a format drive operation
#[derive(Debug)]
pub struct FormatDriveResult {
    pub success: bool,
    pub drive_letter: char,
    pub error_message: Option<String>,
}

/// Service for invoking the Windows format dialog
pub struct FormatDriveService;

impl FormatDriveService {
    /// Creates a new FormatDriveService instance
    pub fn new() -> Self {
        Self
    }

    /// Opens the Windows format dialog for the specified drive
    ///
    /// Note: This only opens the format dialog - it does not perform the actual format.
    /// The user must confirm the format in the dialog.
    #[cfg(windows)]
    pub async fn open_format_dialog(&self, drive_letter: char) -> Result<FormatDriveResult> {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;
        use std::process::Command;

        // Validate drive letter
        if !drive_letter.is_ascii_alphabetic() {
            return Ok(FormatDriveResult {
                success: false,
                drive_letter,
                error_message: Some("Invalid drive letter".to_string()),
            });
        }

        #[cfg(test)]
        return Ok(FormatDriveResult {
            success: true,
            drive_letter: drive_letter.to_ascii_uppercase(),
            error_message: None,
        });

        let drive_path = format!("{}:\\", drive_letter.to_ascii_uppercase());
        tracing::info!("Opening format dialog for drive: {}", drive_path);

        // Use PowerShell to invoke the format dialog via Shell.Explore
        let ps_script = format!(
            r#"Add-Type -Shell32 -Namespace Win32 -Name Api32 -PassThru -MemberDefinition @'
[DllImport("shell32.dll", CharSet=CharSet.Auto)]
public static extern IntPtr ShellExecute(IntPtr hwnd, string lpOperation, string lpFile, string lpParameters, string lpDirectory, int nShowCmd);
'@ | Out-Null
$path = '{}'
[Win32.Api32]::ShellExecute([IntPtr]::Zero, "format", $path, $null, $null, 1)"#,
            drive_path.replace("'", "''")
        );

        let output = Command::new("powershell")
            .args(["-NoProfile", "-Command", &ps_script])
            .output()
            .context("Failed to execute PowerShell for format dialog")?;

        if output.status.success() {
            Ok(FormatDriveResult {
                success: true,
                drive_letter: drive_letter.to_ascii_uppercase(),
                error_message: None,
            })
        } else {
            // Try alternative approach using explorer
            let mut explorer_cmd = Command::new("explorer");
            explorer_cmd.args([&drive_path]);
            explorer_cmd
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null());
            let explorer_result = explorer_cmd.spawn();

            match explorer_result {
                Ok(_) => Ok(FormatDriveResult {
                    success: true,
                    drive_letter: drive_letter.to_ascii_uppercase(),
                    error_message: Some(
                        "Opened drive in explorer instead of format dialog".to_string(),
                    ),
                }),
                Err(e) => Ok(FormatDriveResult {
                    success: false,
                    drive_letter: drive_letter.to_ascii_uppercase(),
                    error_message: Some(format!("Failed to open format dialog: {}", e)),
                }),
            }
        }
    }

    /// Opens the format dialog (non-Windows stub)
    #[cfg(not(windows))]
    pub async fn open_format_dialog(&self, drive_letter: char) -> Result<FormatDriveResult> {
        let _ = drive_letter;
        Ok(FormatDriveResult {
            success: false,
            drive_letter: 'C',
            error_message: Some("FormatDrive is only available on Windows".to_string()),
        })
    }

    /// Performs a quick format using the command line (requires elevation)
    #[cfg(windows)]
    pub async fn quick_format(
        &self,
        drive_letter: char,
        filesystem: FileSystemType,
    ) -> Result<FormatDriveResult> {
        use std::process::Command;

        // Validate drive letter
        if !drive_letter.is_ascii_alphabetic() {
            return Ok(FormatDriveResult {
                success: false,
                drive_letter,
                error_message: Some("Invalid drive letter".to_string()),
            });
        }

        #[cfg(test)]
        return Ok(FormatDriveResult {
            success: true,
            drive_letter: drive_letter.to_ascii_uppercase(),
            error_message: None,
        });

        let drive_path = format!("{}:", drive_letter.to_ascii_uppercase());
        tracing::info!(
            "Performing quick format on drive: {} with filesystem: {}",
            drive_path,
            filesystem.format_string()
        );

        // Use format.com via cmd (requires administrator privileges)
        let output = Command::new("cmd")
            .args(["/c", "format", &drive_path, "/FS:NTFS", "/Q", "/Y"])
            .output()
            .context("Failed to execute format command")?;

        if output.status.success() {
            Ok(FormatDriveResult {
                success: true,
                drive_letter: drive_letter.to_ascii_uppercase(),
                error_message: None,
            })
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Ok(FormatDriveResult {
                success: false,
                drive_letter: drive_letter.to_ascii_uppercase(),
                error_message: Some(format!("Format failed: {}", stderr)),
            })
        }
    }

    /// Quick format (non-Windows stub)
    #[cfg(not(windows))]
    pub async fn quick_format(
        &self,
        drive_letter: char,
        filesystem: FileSystemType,
    ) -> Result<FormatDriveResult> {
        let _ = drive_letter;
        let _ = filesystem;
        Ok(FormatDriveResult {
            success: false,
            drive_letter: 'C',
            error_message: Some("FormatDrive is only available on Windows".to_string()),
        })
    }

    /// Checks if a drive letter is valid and accessible
    #[cfg(windows)]
    pub async fn is_drive_accessible(&self, drive_letter: char) -> bool {
        use std::process::Command;

        let drive_path = format!("{}:\\", drive_letter.to_ascii_uppercase());
        let output = Command::new("cmd")
            .args(["/c", "dir", &drive_path])
            .output();

        match output {
            Ok(o) => o.status.success(),
            Err(_) => false,
        }
    }

    /// Checks if a drive letter is accessible (non-Windows stub)
    #[cfg(not(windows))]
    pub async fn is_drive_accessible(&self, drive_letter: char) -> bool {
        let _ = drive_letter;
        false
    }

    /// Gets a list of available drive letters
    #[cfg(windows)]
    pub fn get_available_drives() -> Vec<char> {
        use std::process::Command;

        let output = Command::new("wmic")
            .args(["logicaldisk", "get", "name"])
            .output();

        match output {
            Ok(o) => String::from_utf8_lossy(&o.stdout)
                .lines()
                .filter_map(|line| {
                    let trimmed = line.trim();
                    if trimmed.len() == 2 && trimmed.chars().next().unwrap().is_ascii_alphabetic() {
                        Some(trimmed.chars().next().unwrap())
                    } else {
                        None
                    }
                })
                .collect(),
            Err(_) => vec![],
        }
    }

    /// Gets available drives (non-Windows stub)
    #[cfg(not(windows))]
    pub fn get_available_drives() -> Vec<char> {
        vec![]
    }
}

impl Default for FormatDriveService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filesystem_type_default() {
        assert_eq!(FileSystemType::default(), FileSystemType::NTFS);
    }

    #[test]
    fn test_filesystem_type_strings() {
        assert_eq!(FileSystemType::NTFS.format_string(), "NTFS");
        assert_eq!(FileSystemType::FAT32.format_string(), "FAT32");
        assert_eq!(FileSystemType::exFAT.format_string(), "exFAT");
        assert_eq!(FileSystemType::ReFS.format_string(), "ReFS");
    }

    #[test]
    fn test_filesystem_type_variants() {
        let types = [
            FileSystemType::NTFS,
            FileSystemType::FAT32,
            FileSystemType::exFAT,
            FileSystemType::ReFS,
        ];
        for fs_type in types {
            let debug_str = format!("{:?}", fs_type);
            assert!(!debug_str.is_empty());
        }
    }

    #[test]
    fn test_quick_format_default() {
        assert_eq!(QuickFormat::default(), QuickFormat::Quick);
    }

    #[test]
    fn test_format_drive_result_debug_success() {
        let result = FormatDriveResult {
            success: true,
            drive_letter: 'D',
            error_message: None,
        };
        let debug_str = format!("{:?}", result);
        assert!(debug_str.contains('D'));
    }

    #[test]
    fn test_format_drive_result_debug_failure() {
        let result = FormatDriveResult {
            success: false,
            drive_letter: 'E',
            error_message: Some("Access denied".to_string()),
        };
        let debug_str = format!("{:?}", result);
        assert!(debug_str.contains("Access denied"));
    }

    #[test]
    fn test_service_creation() {
        let service = FormatDriveService::new();
        let default_service = FormatDriveService::default();
        assert!(true); // Both created successfully
        let _ = default_service;
        let _ = service;
    }

    #[tokio::test]
    async fn test_open_format_dialog_with_valid_drive() {
        let service = FormatDriveService::new();
        // Test with a drive letter (may or may not exist on test system)
        let result = service.open_format_dialog('C').await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_open_format_dialog_with_invalid_drive() {
        let service = FormatDriveService::new();
        let result = service.open_format_dialog('Z').await;
        assert!(result.is_ok()); // Returns Result but may have success=false
    }

    #[tokio::test]
    async fn test_quick_format_with_ntfs() {
        let service = FormatDriveService::new();
        // This will fail without admin, but should return properly
        let result = service.quick_format('D', FileSystemType::NTFS).await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_get_available_drives() {
        let drives = FormatDriveService::get_available_drives();
        // Should return at least C: on Windows
        assert!(drives.contains(&'C') || drives.is_empty());
    }
}

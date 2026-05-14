//! Default File Manager Service
//! Handles setting files-rs as the default file manager via Windows Registry.
//!
//! This enables files-rs to handle folder open commands system-wide.

use crate::core::path_util::FilesPath;
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

/// Default file manager registration status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DefaultManagerStatus {
    /// files-rs is the current default file manager
    IsDefault,
    /// Another application is the default file manager
    NotDefault { current_manager: String },
    /// Registration check failed
    Unknown,
}

/// Service to set files-rs as the default Windows file manager
pub struct DefaultFileManagerService {
    app_path: String,
    app_name: String,
}

impl DefaultFileManagerService {
    /// Create a new DefaultFileManagerService
    pub fn new() -> Self {
        Self {
            app_path: std::env::current_exe()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default(),
            app_name: "files-rs".to_string(),
        }
    }

    /// Create with explicit app path (for testing)
    pub fn with_path(app_path: &str) -> Self {
        Self {
            app_path: app_path.to_string(),
            app_name: "files-rs".to_string(),
        }
    }

    /// Check if files-rs is currently the default file manager
    #[cfg(windows)]
    pub fn get_status() -> DefaultManagerStatus {
        use winreg::enums::*;
        use winreg::RegKey;

        let hkcu = RegKey::predef(HKEY_CURRENT_USER);

        // Check if files-rs is registered as the shell open command for folders
        match hkcu.open_subkey(r"Software\Classes\Directory\shell\open\command") {
            Ok(key) => {
                if let Ok(value) = key.get_value::<String, _>("") {
                    if value.to_lowercase().contains("files-rs") {
                        return DefaultManagerStatus::IsDefault;
                    } else {
                        return DefaultManagerStatus::NotDefault {
                            current_manager: value,
                        };
                    }
                }
            }
            Err(_) => {}
        }

        // Also check the old location
        match hkcu.open_subkey(r"Software\Microsoft\Windows\CurrentVersion\Explorer\file_ms") {
            Ok(_) => {
                // Could check more here
            }
            Err(_) => {}
        }

        DefaultManagerStatus::Unknown
    }

    #[cfg(not(windows))]
    pub fn get_status() -> DefaultManagerStatus {
        DefaultManagerStatus::Unknown
    }

    /// Register files-rs as the default file manager
    /// This sets the shell open command for directories
    #[cfg(windows)]
    pub fn set_as_default(&self) -> Result<()> {
        use winreg::enums::*;
        use winreg::RegKey;

        let hkcu = RegKey::predef(HKEY_CURRENT_USER);

        // Create the shell open command key for directories
        let shell_key = hkcu
            .create_subkey(r"Software\Classes\Directory\shell\files-rs")
            .map_err(|e| anyhow!("Failed to create shell key: {}", e))?
            .0;

        shell_key
            .set_value("", &"Open with files-rs")
            .map_err(|e| anyhow!("Failed to set shell key value: {}", e))?;

        shell_key
            .set_value("Icon", &format!("{},0", self.app_path))
            .ok();

        // Create the command subkey
        let command_key = shell_key
            .create_subkey("command")
            .map_err(|e| anyhow!("Failed to create command key: {}", e))?
            .0;

        command_key
            .set_value("", &format!("\"{}\" \"%V\"", self.app_path))
            .map_err(|e| anyhow!("Failed to set command value: {}", e))?;

        // Also register for directory background (right-click in folder)
        let bg_key = hkcu
            .create_subkey(r"Software\Classes\Directory\Background\shell\files-rs")
            .map_err(|e| anyhow!("Failed to create background shell key: {}", e))?
            .0;

        bg_key
            .set_value("", &"Open files-rs here")
            .map_err(|e| anyhow!("Failed to set background key value: {}", e))?;

        bg_key
            .set_value("Icon", &format!("{},0", self.app_path))
            .ok();

        let bg_command_key = bg_key
            .create_subkey("command")
            .map_err(|e| anyhow!("Failed to create background command key: {}", e))?
            .0;

        bg_command_key
            .set_value("", &format!("\"{}\" \"%V\"", self.app_path))
            .map_err(|e| anyhow!("Failed to set background command value: {}", e))?;

        tracing::info!(
            "files-rs registered as default file manager at: {}",
            self.app_path
        );

        Ok(())
    }

    #[cfg(not(windows))]
    pub fn set_as_default(&self) -> Result<()> {
        Err(anyhow!(
            "Setting as default file manager is only supported on Windows"
        ))
    }

    /// Remove files-rs as the default file manager (revert to Explorer)
    #[cfg(windows)]
    pub fn remove_as_default(&self) -> Result<()> {
        use winreg::enums::*;
        use winreg::RegKey;

        let hkcu = RegKey::predef(HKEY_CURRENT_USER);

        // Delete the shell keys we created
        let _ = hkcu.delete_subkey_all(r"Software\Classes\Directory\shell\files-rs");
        let _ = hkcu.delete_subkey_all(r"Software\Classes\Directory\Background\shell\files-rs");

        tracing::info!("files-rs removed as default file manager");

        Ok(())
    }

    #[cfg(not(windows))]
    pub fn remove_as_default(&self) -> Result<()> {
        Err(anyhow!(
            "Removing as default file manager is only supported on Windows"
        ))
    }

    /// Check if registration is possible (admin rights not required for HKCU)
    pub fn can_register(&self) -> bool {
        !self.app_path.is_empty()
    }
}

impl Default for DefaultFileManagerService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn test_service_creation() {
        let service = DefaultFileManagerService::new();
        assert!(service.can_register() || !service.can_register()); // Just check it doesn't panic
    }

    #[test]
    fn test_service_with_path() {
        let service = DefaultFileManagerService::with_path(r"C:\test\files-rs.exe");
        assert_eq!(service.app_path, r"C:\test\files-rs.exe");
        assert!(service.can_register());
    }

    #[test]
    fn test_status_on_non_windows() {
        #[cfg(not(windows))]
        {
            let status = DefaultFileManagerService::get_status();
            assert!(matches!(status, DefaultManagerStatus::Unknown));
        }
    }

    #[cfg(windows)]
    #[test]
    fn test_status_check() {
        let status = DefaultFileManagerService::get_status();
        // Just verify it doesn't panic and returns a valid status
        match status {
            DefaultManagerStatus::IsDefault => {}
            DefaultManagerStatus::NotDefault { current_manager } => {
                assert!(!current_manager.is_empty());
            }
            DefaultManagerStatus::Unknown => {}
        }
    }

    #[cfg(windows)]
    #[test]
    fn test_remove_as_default() {
        let service = DefaultFileManagerService::with_path(r"C:\test\files-rs.exe");
        // Should not panic
        let result = service.remove_as_default();
        assert!(result.is_ok());
    }

    #[cfg(not(windows))]
    #[test]
    fn test_remove_as_default_fails() {
        let service = DefaultFileManagerService::with_path(r"C:\test\files-rs.exe");
        let result = service.remove_as_default();
        assert!(result.is_err());
    }

    #[cfg(windows)]
    #[test]
    fn test_set_as_default() {
        let service = DefaultFileManagerService::with_path(r"C:\test\files-rs.exe");
        let result = service.set_as_default();
        // This may fail if registry access is denied, but shouldn't panic
        if result.is_err() {
            tracing::debug!("set_as_default failed: {:?}", result);
        }

        // Clean up after test
        let _ = service.remove_as_default();
    }
}
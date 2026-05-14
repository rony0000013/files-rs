//! Startup Service
//!
//! Manages automatic startup of the application when Windows boots.
//! Uses two strategies:
//!   1. Startup folder shortcut (via ShortcutService creating a .lnk in the
//!      `shell:startup` folder at `%APPDATA%\Microsoft\Windows\Start Menu\Programs\Startup\`)
//!   2. Registry RUN key fallback at `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`
//!
//! On non-Windows platforms, all operations return an error explaining the limitation.

use crate::core::path_util::FilesPath;
use crate::core::shortcut_service::{ShortcutOptions, ShortcutService, WindowStyle};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Startup configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartupConfig {
    /// Whether the app should launch at startup
    pub enabled: bool,
    /// Method used for startup registration
    pub method: StartupMethod,
    /// Arguments to pass to the app at startup
    pub arguments: Option<String>,
}

impl Default for StartupConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            method: StartupMethod::StartupFolder,
            arguments: None,
        }
    }
}

/// Method used to register for startup
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StartupMethod {
    /// Place a shortcut in the Windows Startup folder
    StartupFolder,
    /// Use the HKCU Registry RUN key
    Registry,
}

/// Result of a startup registration operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartupResult {
    /// Whether the operation was successful
    pub success: bool,
    /// Path or registry key that was targeted
    pub target: String,
    /// Error message if failed
    pub error: Option<String>,
}

/// Service for managing Windows startup registration.
pub struct StartupService;

impl StartupService {
    /// Enable startup for this application.
    ///
    /// On Windows, this creates a .lnk shortcut in the Startup folder
    /// (default) or registers in HKCU\...\Run registry key (fallback).
    /// On non-Windows, returns an error.
    pub fn enable(config: &StartupConfig) -> StartupResult {
        #[cfg(windows)]
        {
            Self::enable_impl(config)
        }

        #[cfg(not(windows))]
        {
            let _ = config;
            StartupResult {
                success: false,
                target: String::new(),
                error: Some(
                    "Startup registration is only available on Windows".to_string(),
                ),
            }
        }
    }

    /// Disable startup for this application.
    ///
    /// Removes the startup shortcut or registry entry.
    pub fn disable() -> StartupResult {
        Self::remove_startup_entry()
    }

    /// Check whether startup is currently enabled.
    pub fn is_enabled() -> bool {
        Self::check_startup_entry()
    }

    /// Get the path to the system startup folder.
    #[cfg(windows)]
    fn startup_folder_path() -> Result<PathBuf, String> {
        use std::ffi::OsString;
        use std::os::windows::ffi::OsStringExt;

        // Use SHGetFolderPathW with CSIDL_STARTUP (0x0007)
        // or get it from environment variable APPDATA
        let appdata = std::env::var("APPDATA")
            .map_err(|_| "APPDATA environment variable not set".to_string())?;
        let startup = PathBuf::from(appdata)
            .join("Microsoft")
            .join("Windows")
            .join("Start Menu")
            .join("Programs")
            .join("Startup");
        Ok(startup)
    }

    /// Get the path to this application's executable.
    fn app_exe_path() -> Result<PathBuf, String> {
        std::env::current_exe()
            .map_err(|e| format!("Failed to get current executable path: {}", e))
    }

    /// Get the shortcut name for this application.
    fn shortcut_name() -> String {
        "Files-Rust.lnk".to_string()
    }

    /// Get the registry key name for this application.
    fn registry_key_name() -> String {
        "Files-Rust".to_string()
    }

    /// Windows implementation of enabling startup.
    #[cfg(windows)]
    fn enable_impl(config: &StartupConfig) -> StartupResult {
        match config.method {
            StartupMethod::StartupFolder => {
                let result = Self::enable_startup_folder(config);
                if !result.success {
                    // Fall back to registry method
                    tracing::warn!(
                        "Startup folder registration failed, falling back to registry: {:?}",
                        result.error
                    );
                    Self::enable_registry(config)
                } else {
                    result
                }
            }
            StartupMethod::Registry => Self::enable_registry(config),
        }
    }

    /// Enable startup via shortcut in the Startup folder.
    #[cfg(windows)]
    fn enable_startup_folder(config: &StartupConfig) -> StartupResult {
        let startup_folder = match Self::startup_folder_path() {
            Ok(p) => p,
            Err(e) => {
                return StartupResult {
                    success: false,
                    target: String::new(),
                    error: Some(e),
                }
            }
        };

        // Ensure the startup folder exists
        if let Err(e) = std::fs::create_dir_all(&startup_folder) {
            return StartupResult {
                success: false,
                target: startup_folder.to_string_lossy().to_string(),
                error: Some(format!("Failed to create startup folder: {}", e)),
            };
        }

        let exe_path = match Self::app_exe_path() {
            Ok(p) => p,
            Err(e) => {
                return StartupResult {
                    success: false,
                    target: startup_folder.to_string_lossy().to_string(),
                    error: Some(e),
                }
            }
        };

        let shortcut_path = startup_folder.join(Self::shortcut_name());

        let mut options = ShortcutOptions::new(
            FilesPath::from(exe_path.to_str().unwrap_or("")),
            FilesPath::from(shortcut_path.to_str().unwrap_or("")),
        )
        .with_description("Files-Rust File Manager - launches at startup")
        .with_window_style(WindowStyle::Normal);

        if let Some(ref args) = config.arguments {
            options = options.with_arguments(args);
        }

        let result = ShortcutService::create_shortcut(options);

        if result.success {
            tracing::info!(
                "Startup shortcut created at: {}",
                shortcut_path.display()
            );
            StartupResult {
                success: true,
                target: shortcut_path.to_string_lossy().to_string(),
                error: None,
            }
        } else {
            StartupResult {
                success: false,
                target: shortcut_path.to_string_lossy().to_string(),
                error: result.error,
            }
        }
    }

    /// Enable startup via Registry RUN key.
    #[cfg(windows)]
    fn enable_registry(config: &StartupConfig) -> StartupResult {
        use winreg::enums::HKEY_CURRENT_USER;
        use winreg::RegKey;

        let exe_path = match Self::app_exe_path() {
            Ok(p) => p,
            Err(e) => {
                return StartupResult {
                    success: false,
                    target: String::new(),
                    error: Some(e),
                }
            }
        };

        let mut command = exe_path.to_string_lossy().to_string();
        if let Some(ref args) = config.arguments {
            command.push(' ');
            command.push_str(args);
        }

        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let run_key = match hkcu.open_subkey_with_flags(
            r"Software\Microsoft\Windows\CurrentVersion\Run",
            winreg::enums::KEY_SET_VALUE,
        ) {
            Ok(key) => key,
            Err(e) => {
                return StartupResult {
                    success: false,
                    target: r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run".to_string(),
                    error: Some(format!("Failed to open Run registry key: {}", e)),
                }
            }
        };

        match run_key.set_value(Self::registry_key_name(), &command) {
            Ok(_) => {
                tracing::info!(
                    "Startup registry entry created for {}: {}",
                    Self::registry_key_name(),
                    command
                );
                StartupResult {
                    success: true,
                    target: format!(
                        r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run\{}",
                        Self::registry_key_name()
                    ),
                    error: None,
                }
            }
            Err(e) => StartupResult {
                success: false,
                target: format!(
                    r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run\{}",
                    Self::registry_key_name()
                ),
                error: Some(format!("Failed to set registry value: {}", e)),
            },
        }
    }

    /// Remove the startup entry (shortcut or registry).
    #[cfg(windows)]
    fn remove_startup_entry_impl() -> StartupResult {
        // Try removing the startup folder shortcut first
        let shortcut_result = Self::remove_startup_folder_shortcut();

        // Also try removing the registry entry
        let registry_result = Self::remove_registry_entry();

        if shortcut_result.success || registry_result.success {
            StartupResult {
                success: true,
                target: String::new(),
                error: None,
            }
        } else {
            // Return the first error if both failed
            shortcut_result
        }
    }

    /// Remove the startup folder shortcut.
    #[cfg(windows)]
    fn remove_startup_folder_shortcut() -> StartupResult {
        let startup_folder = match Self::startup_folder_path() {
            Ok(p) => p,
            Err(e) => {
                return StartupResult {
                    success: false,
                    target: String::new(),
                    error: Some(e),
                }
            }
        };

        let shortcut_path = startup_folder.join(Self::shortcut_name());
        if !shortcut_path.exists() {
            return StartupResult {
                success: true,
                target: shortcut_path.to_string_lossy().to_string(),
                error: Some("Startup shortcut does not exist".to_string()),
            };
        }

        match std::fs::remove_file(&shortcut_path) {
            Ok(_) => {
                tracing::info!("Removed startup shortcut: {}", shortcut_path.display());
                StartupResult {
                    success: true,
                    target: shortcut_path.to_string_lossy().to_string(),
                    error: None,
                }
            }
            Err(e) => StartupResult {
                success: false,
                target: shortcut_path.to_string_lossy().to_string(),
                error: Some(format!("Failed to remove startup shortcut: {}", e)),
            },
        }
    }

    /// Remove the registry RUN key entry.
    #[cfg(windows)]
    fn remove_registry_entry() -> StartupResult {
        use winreg::enums::HKEY_CURRENT_USER;
        use winreg::RegKey;

        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let run_key = match hkcu.open_subkey_with_flags(
            r"Software\Microsoft\Windows\CurrentVersion\Run",
            winreg::enums::KEY_SET_VALUE,
        ) {
            Ok(key) => key,
            Err(e) => {
                return StartupResult {
                    success: false,
                    target: r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run".to_string(),
                    error: Some(format!("Failed to open Run registry key: {}", e)),
                }
            }
        };

        match run_key.delete_value(Self::registry_key_name()) {
            Ok(_) => {
                tracing::info!(
                    "Removed startup registry entry: {}",
                    Self::registry_key_name()
                );
                StartupResult {
                    success: true,
                    target: format!(
                        r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run\{}",
                        Self::registry_key_name()
                    ),
                    error: None,
                }
            }
            Err(e) => StartupResult {
                success: false,
                target: format!(
                    r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run\{}",
                    Self::registry_key_name()
                ),
                error: Some(format!("Failed to delete registry value: {}", e)),
            },
        }
    }

    /// Non-Windows stub for disabling startup.
    #[cfg(not(windows))]
    fn remove_startup_entry() -> StartupResult {
        StartupResult {
            success: false,
            target: String::new(),
            error: Some("Startup registration is only available on Windows".to_string()),
        }
    }

    /// Non-Windows stub for removing startup entry.
    #[cfg(not(windows))]
    fn check_startup_entry() -> bool {
        false
    }

    /// Windows implementation of checking startup status.
    #[cfg(windows)]
    fn check_startup_entry() -> bool {
        // Check startup folder
        if Self::check_startup_folder() {
            return true;
        }
        // Check registry
        Self::check_registry_entry()
    }

    /// Check if startup folder shortcut exists.
    #[cfg(windows)]
    fn check_startup_folder() -> bool {
        match Self::startup_folder_path() {
            Ok(folder) => {
                let shortcut = folder.join(Self::shortcut_name());
                shortcut.exists()
            }
            Err(_) => false,
        }
    }

    /// Check if registry RUN key exists.
    #[cfg(windows)]
    fn check_registry_entry() -> bool {
        use winreg::enums::HKEY_CURRENT_USER;
        use winreg::RegKey;

        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        match hkcu.open_subkey(r"Software\Microsoft\Windows\CurrentVersion\Run") {
            Ok(key) => key.get_value::<String, _>(Self::registry_key_name()).is_ok(),
            Err(_) => false,
        }
    }

    /// Windows implementation of disabling startup.
    #[cfg(windows)]
    fn remove_startup_entry() -> StartupResult {
        Self::remove_startup_entry_impl()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_startup_config_defaults() {
        let config = StartupConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.method, StartupMethod::StartupFolder);
        assert!(config.arguments.is_none());
    }

    #[test]
    fn test_startup_method_equality() {
        assert_eq!(StartupMethod::StartupFolder, StartupMethod::StartupFolder);
        assert_eq!(StartupMethod::Registry, StartupMethod::Registry);
        assert_ne!(StartupMethod::StartupFolder, StartupMethod::Registry);
    }

    #[test]
    fn test_startup_result_debug() {
        let result = StartupResult {
            success: true,
            target: r"C:\Users\Test\AppData\Roaming\Microsoft\Windows\Start Menu\Programs\Startup\Files-Rust.lnk".to_string(),
            error: None,
        };
        let debug_str = format!("{:?}", result);
        assert!(debug_str.contains("StartupResult"));
        assert!(debug_str.contains("true"));
    }

    #[test]
    fn test_startup_result_error() {
        let result = StartupResult {
            success: false,
            target: String::new(),
            error: Some("Test error".to_string()),
        };
        assert!(!result.success);
        assert_eq!(result.error, Some("Test error".to_string()));
    }

    #[test]
    fn test_shortcut_name() {
        let name = StartupService::shortcut_name();
        assert!(name.contains("Files-Rust"));
        assert!(name.ends_with(".lnk"));
    }

    #[test]
    fn test_registry_key_name() {
        let name = StartupService::registry_key_name();
        assert_eq!(name, "Files-Rust");
    }

    #[test]
    fn test_is_enabled_non_windows() {
        #[cfg(not(windows))]
        {
            assert!(!StartupService::is_enabled());
        }
        #[cfg(windows)]
        {
            // On Windows, this may be true or false depending on current state
            // Just verify it returns a bool without panicking
            let _ = StartupService::is_enabled();
        }
    }

    #[test]
    fn test_disable_non_windows() {
        #[cfg(not(windows))]
        {
            let result = StartupService::disable();
            assert!(!result.success);
            assert!(result.error.unwrap().contains("only available on Windows"));
        }
        #[cfg(windows)]
        {
            // On Windows, disable should succeed even if not currently enabled
            let result = StartupService::disable();
            // May succeed or fail depending on cleanup state
            // Just verify it doesn't panic
            let _ = result;
        }
    }

    #[test]
    fn test_enable_with_config_builder() {
        let config = StartupConfig {
            enabled: true,
            method: StartupMethod::StartupFolder,
            arguments: Some("--minimized".to_string()),
        };
        assert!(config.enabled);
        assert_eq!(config.arguments, Some("--minimized".to_string()));
    }

    #[test]
    fn test_startup_config_serialize() {
        let config = StartupConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("enabled"));
        assert!(json.contains("StartupFolder"));

        // Round-trip
        let deserialized: StartupConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.enabled, config.enabled);
        assert_eq!(deserialized.method, config.method);
    }

    #[test]
    fn test_enable_non_windows() {
        #[cfg(not(windows))]
        {
            let config = StartupConfig::default();
            let result = StartupService::enable(&config);
            assert!(!result.success);
            assert!(result.error.unwrap().contains("only available on Windows"));
        }
        #[cfg(windows)]
        {
            // On Windows, enabling startup writes to the filesystem/registry.
            // Just verify the function runs without panicking.
            let config = StartupConfig::default();
            let result = StartupService::enable(&config);
            // May succeed or fail depending on environment
            let _ = result;
        }
    }
}

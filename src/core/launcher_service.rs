//! Launcher Service
//!
//! Handles the `files-rs://` protocol handler registration and URL parsing.
//! On Windows, registers a protocol handler in the registry so that clicking
//! `files-rs://` links opens the application. Also handles command-line
//! arguments like `--open` and `--new-window`.
//!
//! ## Protocol URL Format
//!
//! ```text
//! files-rs:///C:/Users/Example/Documents
//! files-rs://open?path=C%3A%5CUsers%5CExample
//! files-rs://new-window?path=C%3A%5CUsers
//! ```
//!
//! On non-Windows platforms, all registration operations return an error
//! explaining the platform limitation.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Actions that can be requested via command-line or protocol launch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LaunchAction {
    /// Open the specified path in the existing window
    Open,
    /// Open the specified path in a new window
    NewWindow,
    /// Just show the application (no specific action)
    Show,
}

impl Default for LaunchAction {
    fn default() -> Self {
        Self::Show
    }
}

/// Parsed result from a protocol URL or command-line arguments.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchRequest {
    /// What action to perform
    pub action: LaunchAction,
    /// Optional path to open
    pub path: Option<PathBuf>,
    /// Whether this came from a protocol URL (vs command line)
    pub from_protocol: bool,
}

/// Result of a protocol registration operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolRegistrationResult {
    /// Whether the operation was successful
    pub success: bool,
    /// The protocol that was registered
    pub protocol: String,
    /// Error message if failed
    pub error: Option<String>,
}

/// Service for handling protocol URLs and command-line arguments.
pub struct LauncherService;

impl LauncherService {
    /// Register the `files-rs://` protocol handler in the Windows registry.
    ///
    /// On Windows, this creates the following registry keys:
    /// - `HKEY_CLASSES_ROOT\files-rs` → "URL:Files-Rust Protocol"
    /// - `HKEY_CLASSES_ROOT\files-rs\shell\open\command` → `"<exe>" "%1"`
    ///
    /// On non-Windows, returns an error.
    pub fn register_protocol() -> ProtocolRegistrationResult {
        #[cfg(windows)]
        {
            Self::register_protocol_impl()
        }

        #[cfg(not(windows))]
        {
            ProtocolRegistrationResult {
                success: false,
                protocol: "files-rs".to_string(),
                error: Some("Protocol registration is only available on Windows".to_string()),
            }
        }
    }

    /// Unregister the `files-rs://` protocol handler.
    pub fn unregister_protocol() -> ProtocolRegistrationResult {
        #[cfg(windows)]
        {
            Self::unregister_protocol_impl()
        }

        #[cfg(not(windows))]
        {
            ProtocolRegistrationResult {
                success: false,
                protocol: "files-rs".to_string(),
                error: Some("Protocol registration is only available on Windows".to_string()),
            }
        }
    }

    /// Check whether the `files-rs://` protocol is registered.
    pub fn is_protocol_registered() -> bool {
        #[cfg(windows)]
        {
            Self::is_protocol_registered_impl()
        }

        #[cfg(not(windows))]
        {
            false
        }
    }

    /// Parse a `files-rs://` protocol URL into a `LaunchRequest`.
    ///
    /// Supported formats:
    /// - `files-rs:///C:/path` — Opens the path
    /// - `files-rs://open?path=C%3A%5Cpath` — Opens the path
    /// - `files-rs://new-window?path=C%3A%5Cpath` — Opens in new window
    /// - `files-rs://` — Just shows the app
    pub fn parse_protocol_url(url: &str) -> Result<LaunchRequest, String> {
        let url = url.trim();

        // Validate scheme
        let without_scheme = url
            .strip_prefix("files-rs://")
            .or_else(|| url.strip_prefix("files-rs:"))
            .ok_or_else(|| {
                format!(
                    "Invalid protocol URL: '{}' — must start with files-rs:// or files-rs:",
                    url
                )
            })?;

        let without_scheme = without_scheme.trim_start_matches('/');

        // Empty URL — just show the app
        if without_scheme.is_empty() {
            return Ok(LaunchRequest {
                action: LaunchAction::Show,
                path: None,
                from_protocol: true,
            });
        }

        // Parse action path
        if let Some(query) = without_scheme.strip_prefix("open?") {
            let path = Self::parse_query_path(query)?;
            return Ok(LaunchRequest {
                action: LaunchAction::Open,
                path,
                from_protocol: true,
            });
        }

        if let Some(query) = without_scheme.strip_prefix("new-window?") {
            let path = Self::parse_query_path(query)?;
            return Ok(LaunchRequest {
                action: LaunchAction::NewWindow,
                path,
                from_protocol: true,
            });
        }

        // Treat the rest as a literal path (files-rs:///C:/path)
        let decoded = Self::url_decode(without_scheme);
        let path = PathBuf::from(&decoded);

        Ok(LaunchRequest {
            action: LaunchAction::Open,
            path: Some(path),
            from_protocol: true,
        })
    }

    /// Parse command-line arguments into a `LaunchRequest`.
    ///
    /// Supported flags:
    /// - `--open <path>` or `-o <path>` — Open path in existing window
    /// - `--new-window <path>` or `-n <path>` — Open path in new window
    /// - A bare path (no flag) — Open in existing window
    /// - `files-rs://...` URL — Parse as protocol URL
    pub fn parse_command_line(args: &[String]) -> LaunchRequest {
        let mut iter = args.iter().peekable();

        // Check if first arg is a protocol URL
        if let Some(first) = iter.peek() {
            if first.starts_with("files-rs://") || first.starts_with("files-rs:") {
                return Self::parse_protocol_url(first).unwrap_or_else(|e| {
                    tracing::warn!("Failed to parse protocol URL from args: {}", e);
                    LaunchRequest {
                        action: LaunchAction::Show,
                        path: None,
                        from_protocol: false,
                    }
                });
            }
        }

        while let Some(arg) = iter.next() {
            match arg.as_str() {
                "--open" | "-o" => {
                    if let Some(path) = iter.next() {
                        return LaunchRequest {
                            action: LaunchAction::Open,
                            path: Some(PathBuf::from(Self::unescape_path(path))),
                            from_protocol: false,
                        };
                    }
                }
                "--new-window" | "-n" => {
                    if let Some(path) = iter.next() {
                        return LaunchRequest {
                            action: LaunchAction::NewWindow,
                            path: Some(PathBuf::from(Self::unescape_path(path))),
                            from_protocol: false,
                        };
                    }
                }
                "--help" | "-h" | "/?" => {
                    // Help requested — just show
                    return LaunchRequest {
                        action: LaunchAction::Show,
                        path: None,
                        from_protocol: false,
                    };
                }
                flag if flag.starts_with("--") || flag.starts_with('-') => {
                    // Unknown flag — warn and continue
                    tracing::warn!("Unknown command-line flag: {}", flag);
                }
                bare_path => {
                    // Bare path (no flag) — open it
                    return LaunchRequest {
                        action: LaunchAction::Open,
                        path: Some(PathBuf::from(Self::unescape_path(bare_path))),
                        from_protocol: false,
                    };
                }
            }
        }

        // No arguments, just show
        LaunchRequest {
            action: LaunchAction::Show,
            path: None,
            from_protocol: false,
        }
    }

    /// Get the current executable path for protocol registration.
    fn get_exe_path() -> Result<String, String> {
        std::env::current_exe()
            .map(|p| p.to_string_lossy().to_string())
            .map_err(|e| format!("Failed to get executable path: {}", e))
    }

    /// Windows implementation of protocol registration.
    #[cfg(windows)]
    fn register_protocol_impl() -> ProtocolRegistrationResult {
        use winreg::enums::{HKEY_CLASSES_ROOT, KEY_ALL_ACCESS};
        use winreg::RegKey;

        let exe_path = match Self::get_exe_path() {
            Ok(p) => p,
            Err(e) => {
                return ProtocolRegistrationResult {
                    success: false,
                    protocol: "files-rs".to_string(),
                    error: Some(e),
                }
            }
        };

        // Open/create HKEY_CLASSES_ROOT\files-rs
        let hkcr_base = match RegKey::predef(HKEY_CLASSES_ROOT).create_subkey("files-rs") {
            Ok((key, _)) => key,
            Err(e) => {
                return ProtocolRegistrationResult {
                    success: false,
                    protocol: "files-rs".to_string(),
                    error: Some(format!(
                        "Failed to create HKEY_CLASSES_ROOT\\files-rs: {}",
                        e
                    )),
                }
            }
        };

        // Set default value: "URL:Files-Rust Protocol"
        if let Err(e) = hkcr_base.set_value("", &"URL:Files-Rust Protocol") {
            tracing::warn!("Failed to set protocol description: {}", e);
        }
        // Set URL Protocol hint
        if let Err(e) = hkcr_base.set_value("URL Protocol", &"") {
            tracing::warn!("Failed to set URL Protocol hint: {}", e);
        }

        // Create DefaultIcon key
        if let Err(e) = hkcr_base.set_value("DefaultIcon", &format!("\"{}\",0", exe_path)) {
            tracing::warn!("Failed to set DefaultIcon: {}", e);
        }

        // Create shell\open\command key
        let shell_key = match hkcr_base.create_subkey("shell") {
            Ok((key, _)) => key,
            Err(e) => {
                return ProtocolRegistrationResult {
                    success: false,
                    protocol: "files-rs".to_string(),
                    error: Some(format!("Failed to create shell key: {}", e)),
                }
            }
        };

        let open_key = match shell_key.create_subkey("open") {
            Ok((key, _)) => key,
            Err(e) => {
                return ProtocolRegistrationResult {
                    success: false,
                    protocol: "files-rs".to_string(),
                    error: Some(format!("Failed to create open key: {}", e)),
                }
            }
        };

        let command_key = match open_key.create_subkey("command") {
            Ok((key, _)) => key,
            Err(e) => {
                return ProtocolRegistrationResult {
                    success: false,
                    protocol: "files-rs".to_string(),
                    error: Some(format!("Failed to create command key: {}", e)),
                }
            }
        };

        let command_value = format!("\"{}\" \"%1\"", exe_path);
        match command_key.set_value("", &command_value) {
            Ok(_) => {
                tracing::info!(
                    "Protocol 'files-rs' registered: command = {}",
                    command_value
                );
                ProtocolRegistrationResult {
                    success: true,
                    protocol: "files-rs".to_string(),
                    error: None,
                }
            }
            Err(e) => ProtocolRegistrationResult {
                success: false,
                protocol: "files-rs".to_string(),
                error: Some(format!("Failed to set command value: {}", e)),
            },
        }
    }

    /// Windows implementation of protocol unregistration.
    #[cfg(windows)]
    fn unregister_protocol_impl() -> ProtocolRegistrationResult {
        use winreg::enums::HKEY_CLASSES_ROOT;
        use winreg::RegKey;

        // Delete the entire files-rs key tree
        let hkcr = RegKey::predef(HKEY_CLASSES_ROOT);
        match hkcr.delete_subkey_all("files-rs") {
            Ok(_) => {
                tracing::info!("Protocol 'files-rs' unregistered");
                ProtocolRegistrationResult {
                    success: true,
                    protocol: "files-rs".to_string(),
                    error: None,
                }
            }
            Err(e) => ProtocolRegistrationResult {
                success: false,
                protocol: "files-rs".to_string(),
                error: Some(format!("Failed to unregister protocol: {}", e)),
            },
        }
    }

    /// Windows implementation of checking protocol registration.
    #[cfg(windows)]
    fn is_protocol_registered_impl() -> bool {
        use winreg::enums::HKEY_CLASSES_ROOT;
        use winreg::RegKey;

        let hkcr = RegKey::predef(HKEY_CLASSES_ROOT);
        match hkcr.open_subkey(r"files-rs\shell\open\command") {
            Ok(key) => key.get_value::<String, _>("").is_ok(),
            Err(_) => false,
        }
    }

    /// Parse a query string like `path=C%3A%5CUsers%5CExample`
    /// and return the decoded path.
    fn parse_query_path(query: &str) -> Result<Option<PathBuf>, String> {
        // Simple query parser — supports `path=<url-encoded-path>`
        if let Some(value) = query.strip_prefix("path=") {
            let decoded = Self::url_decode(value);
            Ok(Some(PathBuf::from(decoded)))
        } else if query.is_empty() {
            Ok(None)
        } else {
            // Unknown query parameter — treat as path
            let decoded = Self::url_decode(query);
            Ok(Some(PathBuf::from(decoded)))
        }
    }

    /// Simple percent-decoding for URLs.
    fn url_decode(input: &str) -> String {
        let mut result = String::with_capacity(input.len());
        let mut chars = input.chars();

        while let Some(c) = chars.next() {
            if c == '%' {
                let hex: String = chars.by_ref().take(2).collect();
                if hex.len() == 2 {
                    if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                        result.push(byte as char);
                        continue;
                    }
                }
                // Invalid percent encoding — keep as-is
                result.push('%');
                result.push_str(&hex);
            } else if c == '+' {
                result.push(' ');
            } else {
                result.push(c);
            }
        }

        result
    }

    /// Basic path unescaping (currently just returns as-is).
    fn unescape_path(path: &str) -> String {
        // Remove surrounding quotes if present
        let trimmed = path.trim();
        if (trimmed.starts_with('"') && trimmed.ends_with('"'))
            || (trimmed.starts_with('\'') && trimmed.ends_with('\''))
        {
            trimmed[1..trimmed.len() - 1].to_string()
        } else {
            trimmed.to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_protocol_url_bare_path() {
        // files-rs:///C:/Users/Test
        let request = LauncherService::parse_protocol_url("files-rs:///C:/Users/Test").unwrap();
        assert_eq!(request.action, LaunchAction::Open);
        assert_eq!(request.path, Some(PathBuf::from("C:/Users/Test")));
        assert!(request.from_protocol);
    }

    #[test]
    fn test_parse_protocol_url_with_scheme_and_path() {
        let request = LauncherService::parse_protocol_url("files-rs:///D:/Documents/Work").unwrap();
        assert_eq!(request.action, LaunchAction::Open);
        assert_eq!(request.path, Some(PathBuf::from("D:/Documents/Work")));
    }

    #[test]
    fn test_parse_protocol_url_open_query() {
        let request =
            LauncherService::parse_protocol_url("files-rs://open?path=C%3A%5CUsers").unwrap();
        assert_eq!(request.action, LaunchAction::Open);
        assert_eq!(request.path, Some(PathBuf::from("C:\\Users")));
    }

    #[test]
    fn test_parse_protocol_url_new_window_query() {
        let request =
            LauncherService::parse_protocol_url("files-rs://new-window?path=D%3A%5C").unwrap();
        assert_eq!(request.action, LaunchAction::NewWindow);
        assert_eq!(request.path, Some(PathBuf::from("D:\\")));
    }

    #[test]
    fn test_parse_protocol_url_empty() {
        let request = LauncherService::parse_protocol_url("files-rs://").unwrap();
        assert_eq!(request.action, LaunchAction::Show);
        assert!(request.path.is_none());
    }

    #[test]
    fn test_parse_protocol_url_just_scheme() {
        let request = LauncherService::parse_protocol_url("files-rs:").unwrap();
        assert_eq!(request.action, LaunchAction::Show);
        assert!(request.path.is_none());
    }

    #[test]
    fn test_parse_protocol_invalid_scheme() {
        let result = LauncherService::parse_protocol_url("https://example.com");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_protocol_url_encoded_path() {
        let request = LauncherService::parse_protocol_url(
            "files-rs://open?path=C%3A%5CUsers%5CTest%20Folder",
        )
        .unwrap();
        assert_eq!(request.action, LaunchAction::Open);
        assert_eq!(request.path, Some(PathBuf::from("C:\\Users\\Test Folder")));
    }

    #[test]
    fn test_parse_command_line_open_flag() {
        let args = vec![
            "program.exe".to_string(),
            "--open".to_string(),
            "C:\\Users\\Test".to_string(),
        ];
        let request = LauncherService::parse_command_line(&args);
        assert_eq!(request.action, LaunchAction::Open);
        assert_eq!(request.path, Some(PathBuf::from("C:\\Users\\Test")));
        assert!(!request.from_protocol);
    }

    #[test]
    fn test_parse_command_line_short_open() {
        let args = vec![
            "program.exe".to_string(),
            "-o".to_string(),
            "D:\\Docs".to_string(),
        ];
        let request = LauncherService::parse_command_line(&args);
        assert_eq!(request.action, LaunchAction::Open);
        assert_eq!(request.path, Some(PathBuf::from("D:\\Docs")));
    }

    #[test]
    fn test_parse_command_line_new_window() {
        let args = vec![
            "program.exe".to_string(),
            "--new-window".to_string(),
            "E:\\Data".to_string(),
        ];
        let request = LauncherService::parse_command_line(&args);
        assert_eq!(request.action, LaunchAction::NewWindow);
        assert_eq!(request.path, Some(PathBuf::from("E:\\Data")));
    }

    #[test]
    fn test_parse_command_line_short_new_window() {
        let args = vec![
            "program.exe".to_string(),
            "-n".to_string(),
            "F:\\Backup".to_string(),
        ];
        let request = LauncherService::parse_command_line(&args);
        assert_eq!(request.action, LaunchAction::NewWindow);
        assert_eq!(request.path, Some(PathBuf::from("F:\\Backup")));
    }

    #[test]
    fn test_parse_command_line_bare_path() {
        let args = vec!["program.exe".to_string(), "C:\\Users".to_string()];
        let request = LauncherService::parse_command_line(&args);
        assert_eq!(request.action, LaunchAction::Open);
        assert_eq!(request.path, Some(PathBuf::from("C:\\Users")));
    }

    #[test]
    fn test_parse_command_line_no_args() {
        let args = vec!["program.exe".to_string()];
        let request = LauncherService::parse_command_line(&args);
        assert_eq!(request.action, LaunchAction::Show);
        assert!(request.path.is_none());
    }

    #[test]
    fn test_parse_command_line_protocol_url() {
        let args = vec![
            "program.exe".to_string(),
            "files-rs:///C:/Users".to_string(),
        ];
        let request = LauncherService::parse_command_line(&args);
        assert_eq!(request.action, LaunchAction::Open);
        assert_eq!(request.path, Some(PathBuf::from("C:/Users")));
        assert!(request.from_protocol);
    }

    #[test]
    fn test_parse_command_line_help() {
        let args = vec!["program.exe".to_string(), "--help".to_string()];
        let request = LauncherService::parse_command_line(&args);
        assert_eq!(request.action, LaunchAction::Show);
        assert!(request.path.is_none());
    }

    #[test]
    fn test_parse_command_line_unknown_flag() {
        let args = vec![
            "program.exe".to_string(),
            "--unknown".to_string(),
            "value".to_string(),
        ];
        let request = LauncherService::parse_command_line(&args);
        assert_eq!(request.action, LaunchAction::Show);
        assert!(request.path.is_none());
    }

    #[test]
    fn test_url_decode_simple() {
        assert_eq!(LauncherService::url_decode("hello%20world"), "hello world");
        assert_eq!(LauncherService::url_decode("C%3A%5CUsers"), "C:\\Users");
    }

    #[test]
    fn test_url_decode_plus() {
        assert_eq!(LauncherService::url_decode("a+b"), "a b");
    }

    #[test]
    fn test_url_decode_no_encoding() {
        assert_eq!(
            LauncherService::url_decode("C:\\Users\\Test"),
            "C:\\Users\\Test"
        );
    }

    #[test]
    fn test_url_decode_invalid_hex() {
        // Invalid percent encoding should keep as-is
        assert_eq!(LauncherService::url_decode("%XX"), "%XX");
    }

    #[test]
    fn test_unescape_path_no_quotes() {
        assert_eq!(
            LauncherService::unescape_path("C:\\Users\\Test"),
            "C:\\Users\\Test"
        );
    }

    #[test]
    fn test_unescape_path_double_quotes() {
        assert_eq!(
            LauncherService::unescape_path("\"C:\\Users\\Test\""),
            "C:\\Users\\Test"
        );
    }

    #[test]
    fn test_unescape_path_single_quotes() {
        assert_eq!(
            LauncherService::unescape_path("'C:\\Users\\Test'"),
            "C:\\Users\\Test"
        );
    }

    #[test]
    fn test_unescape_path_trimmed() {
        assert_eq!(LauncherService::unescape_path("  C:\\Test  "), "C:\\Test");
    }

    #[test]
    fn test_launch_action_default() {
        assert_eq!(LaunchAction::default(), LaunchAction::Show);
    }

    #[test]
    fn test_launch_action_equality() {
        assert_eq!(LaunchAction::Open, LaunchAction::Open);
        assert_ne!(LaunchAction::Open, LaunchAction::NewWindow);
        assert_ne!(LaunchAction::Show, LaunchAction::Open);
    }

    #[test]
    fn test_protocol_registration_result() {
        let result = ProtocolRegistrationResult {
            success: true,
            protocol: "files-rs".to_string(),
            error: None,
        };
        assert!(result.success);
        assert_eq!(result.protocol, "files-rs");
    }

    #[test]
    fn test_register_protocol_non_windows() {
        #[cfg(not(windows))]
        {
            let result = LauncherService::register_protocol();
            assert!(!result.success);
            assert!(result.error.unwrap().contains("only available on Windows"));
        }
        #[cfg(windows)]
        {
            // On Windows, registration may fail or succeed depending on permissions
            let result = LauncherService::register_protocol();
            // Just verify it doesn't panic
            let _ = result;
        }
    }

    #[test]
    fn test_unregister_protocol_non_windows() {
        #[cfg(not(windows))]
        {
            let result = LauncherService::unregister_protocol();
            assert!(!result.success);
            assert!(result.error.unwrap().contains("only available on Windows"));
        }
        #[cfg(windows)]
        {
            let result = LauncherService::unregister_protocol();
            let _ = result;
        }
    }

    #[test]
    fn test_is_protocol_registered_non_windows() {
        #[cfg(not(windows))]
        {
            assert!(!LauncherService::is_protocol_registered());
        }
        #[cfg(windows)]
        {
            let _ = LauncherService::is_protocol_registered();
        }
    }

    #[test]
    fn test_parse_command_line_new_window_short_flag() {
        let args = vec![
            "files-rs".to_string(),
            "-n".to_string(),
            "Z:\\mnt".to_string(),
        ];
        let request = LauncherService::parse_command_line(&args);
        assert_eq!(request.action, LaunchAction::NewWindow);
        assert!(request.path.is_some());
    }

    #[test]
    fn test_launch_request_serialization() {
        let request = LaunchRequest {
            action: LaunchAction::Open,
            path: Some(PathBuf::from("C:\\Test")),
            from_protocol: true,
        };
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("Open"));
        assert!(json.contains("C:\\\\Test"));

        // Round-trip
        let deserialized: LaunchRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.action, LaunchAction::Open);
        assert_eq!(deserialized.path, Some(PathBuf::from("C:\\Test")));
        assert!(deserialized.from_protocol);
    }

    #[test]
    fn test_parse_query_path_with_path_param() {
        let result = LauncherService::parse_query_path("path=D%3A%5CData").unwrap();
        assert_eq!(result, Some(PathBuf::from("D:\\Data")));
    }

    #[test]
    fn test_parse_query_path_empty() {
        let result = LauncherService::parse_query_path("").unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn test_parse_protocol_url_with_short_scheme() {
        let request = LauncherService::parse_protocol_url("files-rs:C:/Users/Short").unwrap();
        assert_eq!(request.action, LaunchAction::Open);
        assert_eq!(request.path, Some(PathBuf::from("C:/Users/Short")));
    }
}

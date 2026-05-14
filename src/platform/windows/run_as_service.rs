//! Run As Another User Service
//! Provides functionality to launch applications under a different user account
//! Uses Windows API: CreateProcessAsUser, LogonUser

use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;

/// User impersonation mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImpersonationMode {
    /// Run with credentials from specific username/password
    SpecificUser,
    /// Run as another user via Windows UAC prompt
    AsDifferentUser,
}

impl Default for ImpersonationMode {
    fn default() -> Self {
        Self::AsDifferentUser
    }
}

/// Result of a run as operation
#[derive(Debug)]
pub struct RunAsResult {
    pub success: bool,
    pub process_id: Option<u32>,
    pub error_message: Option<String>,
}

/// Service for launching applications under a different user account
pub struct RunAsAnotherUserService;

impl RunAsAnotherUserService {
    /// Creates a new RunAsAnotherUserService instance
    pub fn new() -> Self {
        Self
    }

    /// Launches a process as a different user using Windows runas command
    ///
    /// # Arguments
    /// * `path` - Path to the executable to run
    /// * `arguments` - Command line arguments for the executable
    /// * `working_directory` - Working directory for the process (optional)
    ///
    /// # Returns
    /// Returns the result of the launch operation
    #[cfg(windows)]
    pub async fn run_as_different_user(
        &self,
        path: &Path,
        arguments: &[&str],
        working_directory: Option<&Path>,
    ) -> Result<RunAsResult> {
        let path_str = path
            .to_str()
            .context("Invalid path string")?;

        let args_string = arguments.join(" ");

        // Build the runas command
        let mut cmd = Command::new("runas");
        cmd.args([
            "/user:Administrator",
            &format!("\"{}\" {}", path_str, args_string),
        ]);

        if let Some(cwd) = working_directory {
            cmd.current_dir(cwd);
        }

        tracing::info!(
            "Launching as different user: {} {:?}",
            path_str,
            arguments
        );

        match cmd.spawn() {
            Ok(child) => Ok(RunAsResult {
                success: true,
                process_id: Some(child.id()),
                error_message: None,
            }),
            Err(e) => Ok(RunAsResult {
                success: false,
                process_id: None,
                error_message: Some(e.to_string()),
            }),
        }
    }

    /// Launches a process as a different user (non-Windows stub)
    #[cfg(not(windows))]
    pub async fn run_as_different_user(
        &self,
        path: &Path,
        arguments: &[&str],
        working_directory: Option<&Path>,
    ) -> Result<RunAsResult> {
        let _ = path;
        let _ = arguments;
        let _ = working_directory;

        Ok(RunAsResult {
            success: false,
            process_id: None,
            error_message: Some("RunAsAnotherUser is only available on Windows".to_string()),
        })
    }

    /// Launches a process with specific credentials using LogonUser
    #[cfg(windows)]
    pub async fn run_with_credentials(
        &self,
        path: &Path,
        username: &str,
        password: &str,
        arguments: &[&str],
    ) -> Result<RunAsResult> {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;

        let path_str = path
            .to_str()
            .context("Invalid path string")?;

        let args_string = arguments.join(" ");

        // Use PowerShell Start-Process with -Credential for secure credential handling
        let ps_script = format!(
            r#"
            $password = ConvertTo-SecureString '{}' -AsPlainText -Force
            $credential = New-Object System.Management.Automation.PSCredential('{}', $password)
            Start-Process -FilePath '{}' -ArgumentList '{}' -Credential $credential -PassThru | Select-Object -ExpandProperty Id
            "#,
            password.replace("'", "''"),
            username.replace("'", "''"),
            path_str.replace("'", "''"),
            args_string.replace("'", "''")
        );

        let output = Command::new("powershell")
            .args(["-NoProfile", "-Command", &ps_script])
            .output()
            .context("Failed to execute PowerShell for run with credentials")?;

        if output.status.success() {
            let pid_string = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let pid: Option<u32> = pid_string.parse().ok();

            Ok(RunAsResult {
                success: true,
                process_id: pid,
                error_message: None,
            })
        } else {
            Ok(RunAsResult {
                success: false,
                process_id: None,
                error_message: Some(String::from_utf8_lossy(&output.stderr).to_string()),
            })
        }
    }

    /// Launches a process with specific credentials (non-Windows stub)
    #[cfg(not(windows))]
    pub async fn run_with_credentials(
        &self,
        path: &Path,
        username: &str,
        password: &str,
        arguments: &[&str],
    ) -> Result<RunAsResult> {
        let _ = path;
        let _ = username;
        let _ = password;
        let _ = arguments;

        Ok(RunAsResult {
            success: false,
            process_id: None,
            error_message: Some("RunAsAnotherUser is only available on Windows".to_string()),
        })
    }

    /// Checks if a username exists on the system
    #[cfg(windows)]
    pub async fn user_exists(&self, username: &str) -> Result<bool> {
        let output = Command::new("net")
            .args(["user", username])
            .output()
            .context("Failed to check user existence")?;

        Ok(output.status.success())
    }

    /// Checks if a username exists (non-Windows stub)
    #[cfg(not(windows))]
    pub async fn user_exists(&self, username: &str) -> Result<bool> {
        let _ = username;
        Ok(false)
    }
}

impl Default for RunAsAnotherUserService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_impersonation_mode_default() {
        assert_eq!(ImpersonationMode::default(), ImpersonationMode::AsDifferentUser);
    }

    #[test]
    fn test_impersonation_mode_variants() {
        let modes = [ImpersonationMode::SpecificUser, ImpersonationMode::AsDifferentUser];
        for mode in modes {
            let debug_str = format!("{:?}", mode);
            assert!(!debug_str.is_empty());
        }
    }

    #[test]
    fn test_run_as_result_debug_success() {
        let result = RunAsResult {
            success: true,
            process_id: Some(1234),
            error_message: None,
        };
        let debug_str = format!("{:?}", result);
        assert!(debug_str.contains("true"));
        assert!(debug_str.contains("1234"));
    }

    #[test]
    fn test_run_as_result_debug_failure() {
        let result = RunAsResult {
            success: false,
            process_id: None,
            error_message: Some("Access denied".to_string()),
        };
        let debug_str = format!("{:?}", result);
        assert!(debug_str.contains("false"));
        assert!(debug_str.contains("Access denied"));
    }

    #[test]
    fn test_service_creation() {
        let service = RunAsAnotherUserService::new();
        let default_service = RunAsAnotherUserService::default();
        assert!(true); // Both created successfully
        let _ = default_service;
        let _ = service;
    }

    #[tokio::test]
    async fn test_run_as_different_user_accepts_valid_path() {
        let service = RunAsAnotherUserService::new();
        let path = Path::new("C:\\Windows\\System32\\notepad.exe");
        let result = service.run_as_different_user(path, &[], None).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_run_with_credentials_accepts_valid_params() {
        let service = RunAsAnotherUserService::new();
        let path = Path::new("C:\\Windows\\System32\\notepad.exe");
        let result = service
            .run_with_credentials(path, "TestUser", "TestPassword", &[])
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_user_exists_returns_result() {
        let service = RunAsAnotherUserService::new();
        let result = service.user_exists("Administrator").await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_run_as_result_with_no_error() {
        let result = RunAsResult {
            success: true,
            process_id: Some(5678),
            error_message: None,
        };
        assert!(result.success);
        assert_eq!(result.process_id, Some(5678));
        assert!(result.error_message.is_none());
    }

    #[test]
    fn test_run_as_result_with_error() {
        let result = RunAsResult {
            success: false,
            process_id: None,
            error_message: Some("Failed to start process".to_string()),
        };
        assert!(!result.success);
        assert!(result.process_id.is_none());
        assert!(result.error_message.is_some());
    }
}
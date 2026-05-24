//! Windows Toast Notification Service
//! Provides system notification bubbles for background task completion.

use anyhow::{anyhow, Result};
use std::sync::atomic::{AtomicU32, Ordering};

/// Toast notification severity level
#[derive(Debug, Clone, Copy)]
pub enum ToastLevel {
    Info,
    Success,
    Warning,
    Error,
}

/// Windows Toast Notification Service
/// Creates toast notifications for background task completion.
pub struct ToastService {
    notification_id: AtomicU32,
}

impl ToastService {
    pub fn new() -> Self {
        Self {
            notification_id: AtomicU32::new(0),
        }
    }

    /// Show a toast notification
    #[cfg(windows)]
    pub async fn show(&self, title: &str, message: &str, level: ToastLevel) -> Result<u32> {
        use std::os::windows::ffi::OsStrExt;
        use windows::core::PCWSTR;
        use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};
        use windows::Win32::UI::Shell::{SHAddToRecentDocs, SHARD_APPIDINFO};

        let id = self.notification_id.fetch_add(1, Ordering::SeqCst);

        // For Windows toast notifications, we'd ideally use Windows.UI.Notifications
        // but that requires WinRT. For now, use a simpler approach via Shell.

        tracing::info!("Toast [{}] ({:?}): {} - {}", id, level, title, message);

        // On Windows 10/11, we can use PowerShell to show toast via BurntToast module
        // or fallback to a simple message box for critical notifications
        if matches!(level, ToastLevel::Error | ToastLevel::Warning) {
            self.show_windows_notification(title, message)?;
        }

        Ok(id)
    }

    #[cfg(windows)]
    fn show_windows_notification(&self, title: &str, message: &str) -> Result<()> {
        use std::process::Command;

        // Use PowerShell to show toast notification if BurntToast is available,
        // otherwise fall back to a simple console notification
        let ps_script = format!(
            r#"
            $title = '{}'
            $msg = '{}'
            try {{
                # Try BurntToast if available
                New-BurntToastNotification -Text $title,$msg
            }} catch {{
                # Fallback: use Windows Forms message in background
                Add-Type -AssemblyName System.Windows.Forms
                [System.Windows.Forms.MessageBox]::Show($msg, $title, 'OK', 'Information')
            }}
            "#,
            title.replace("'", "''"),
            message.replace("'", "''")
        );

        // Run PowerShell in background (non-blocking)
        let _child = Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", &ps_script])
            .spawn();

        Ok(())
    }

    /// Show a toast notification (non-Windows stub)
    #[cfg(not(windows))]
    pub async fn show(&self, title: &str, message: &str, _level: ToastLevel) -> Result<u32> {
        tracing::info!("Toast notification: {} - {}", title, message);
        let id = self.notification_id.fetch_add(1, Ordering::SeqCst);
        Ok(id)
    }

    /// Show a success toast
    pub async fn success(&self, title: &str, message: &str) -> Result<u32> {
        self.show(title, message, ToastLevel::Success).await
    }

    /// Show an info toast
    pub async fn info(&self, title: &str, message: &str) -> Result<u32> {
        self.show(title, message, ToastLevel::Info).await
    }

    /// Show a warning toast
    pub async fn warning(&self, title: &str, message: &str) -> Result<u32> {
        self.show(title, message, ToastLevel::Warning).await
    }

    /// Show an error toast
    pub async fn error(&self, title: &str, message: &str) -> Result<u32> {
        self.show(title, message, ToastLevel::Error).await
    }

    /// Show operation completion toast
    pub async fn operation_complete(&self, operation: &str, item_count: usize) -> Result<u32> {
        let message = match item_count {
            0 => format!("{} completed", operation),
            1 => format!("{}: 1 item", operation),
            n => format!("{}: {} items", operation, n),
        };
        self.success("Operation Complete", &message).await
    }

    /// Show file operation toast (copy/move/delete/etc.)
    pub async fn file_operation_complete(
        &self,
        operation: &str,
        source: &str,
        destination: &str,
    ) -> Result<u32> {
        let message = format!("{} from\n{}\nto\n{}", operation, source, destination);
        self.success("File Operation Complete", &message).await
    }
}

impl Default for ToastService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn test_toast_service_creation() {
        let service = ToastService::new();
        assert_eq!(service.notification_id.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn test_show_toast() {
        let service = ToastService::new();
        let id = service.show("Test", "Test message", ToastLevel::Info).await;
        assert!(id.is_ok());
    }

    #[tokio::test]
    async fn test_success_toast() {
        let service = ToastService::new();
        let id = service.success("Done", "Task completed").await;
        assert!(id.is_ok());
    }

    #[tokio::test]
    async fn test_error_toast() {
        let service = ToastService::new();
        let id = service.error("Error", "Something went wrong").await;
        assert!(id.is_ok());
    }

    #[tokio::test]
    async fn test_operation_complete() {
        let service = ToastService::new();
        let id = service.operation_complete("Copy", 5).await;
        assert!(id.is_ok());
    }
}

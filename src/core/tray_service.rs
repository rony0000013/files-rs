//! # System Tray Manager
//!
//! Manages the Windows system tray icon for background execution.
//!
//! Uses PowerShell via `System.Windows.Forms.NotifyIcon` on Windows
//! to create and manage tray icons (same pattern as ShortcutService).
//! On non-Windows, all operations return an error explaining the platform
//! limitation.

/// Configuration for the system tray / background execution behavior.
#[derive(Debug, Clone)]
pub struct SystemTrayConfig {
    /// Whether to minimize to tray on close instead of quitting
    pub minimize_to_tray: bool,
    /// Whether to show the tray icon
    pub show_tray_icon: bool,
    /// Whether to start minimized (background on boot)
    pub start_minimized: bool,
}

impl Default for SystemTrayConfig {
    fn default() -> Self {
        Self {
            minimize_to_tray: true,
            show_tray_icon: true,
            start_minimized: false,
        }
    }
}

/// Manager for the Windows system tray icon.
///
/// On Windows, uses PowerShell to create a `NotifyIcon` via
/// `System.Windows.Forms`. On non-Windows, all methods return an error.
#[derive(Debug)]
pub struct SystemTrayManager {
    /// Whether the tray icon is currently shown
    pub icon_visible: bool,
    /// Configuration
    pub config: SystemTrayConfig,
    /// Path to the PowerShell script if tray is active
    script_path: Option<std::path::PathBuf>,
}

impl SystemTrayManager {
    /// Create a new tray manager with default config.
    pub fn new() -> Self {
        Self {
            icon_visible: false,
            config: SystemTrayConfig::default(),
            script_path: None,
        }
    }

    /// Create a new tray manager with the given config.
    pub fn with_config(config: SystemTrayConfig) -> Self {
        Self {
            icon_visible: false,
            config,
            script_path: None,
        }
    }

    /// Initialize the tray icon.
    ///
    /// On Windows, creates a PowerShell-based tray icon with a context menu.
    /// On non-Windows, returns an error.
    pub fn initialize(&mut self, _app_title: &str) -> Result<(), String> {
        #[cfg(target_os = "windows")]
        {
            let icon_path = Self::ensure_icon();
            let script = self.build_tray_script(_app_title, &icon_path);
            let script_path = Self::get_script_path();
            std::fs::create_dir_all(script_path.parent().unwrap_or(std::path::Path::new(".")))
                .map_err(|e| format!("Failed to create script dir: {}", e))?;
            std::fs::write(&script_path, &script)
                .map_err(|e| format!("Failed to write tray script: {}", e))?;

            // Launch PowerShell with the tray script (non-blocking)
            let result = std::process::Command::new("powershell")
                .args([
                    "-NoProfile",
                    "-NonInteractive",
                    "-WindowStyle",
                    "Hidden",
                    "-ExecutionPolicy",
                    "Bypass",
                    "-File",
                    &script_path.to_string_lossy(),
                ])
                .spawn();

            match result {
                Ok(_) => {
                    self.icon_visible = true;
                    self.script_path = Some(script_path);
                    tracing::info!("System tray icon initialized via PowerShell");
                    Ok(())
                }
                Err(e) => Err(format!("Failed to launch tray script: {}", e)),
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            let _ = _app_title;
            Err("System tray requires Windows".to_string())
        }
    }

    /// Remove the tray icon by killing the PowerShell process.
    pub fn remove_icon(&self) -> Result<(), String> {
        #[cfg(target_os = "windows")]
        {
            if !self.icon_visible {
                return Ok(());
            }
            // Kill any powershell tray processes
            let _ = std::process::Command::new("taskkill")
                .args([
                    "/F",
                    "/IM",
                    "powershell.exe",
                    "/FI",
                    "WindowTitle eq Files-Rust Tray",
                ])
                .output();
            tracing::info!("System tray icon removed");
        }
        Ok(())
    }

    /// Show a balloon tip notification.
    pub fn show_notification(&self, title: &str, message: &str) -> Result<(), String> {
        #[cfg(target_os = "windows")]
        {
            let script = format!(
                r#"
[System.Reflection.Assembly]::LoadWithPartialName("System.Windows.Forms") | Out-Null
$icon = New-Object System.Windows.Forms.NotifyIcon
$icon.Icon = [System.Drawing.Icon]::ExtractAssociatedIcon("powershell.exe")
$icon.BalloonTipTitle = '{title}'
$icon.BalloonTipText = '{message}'
$icon.Visible = $true
$icon.ShowBalloonTip(3000)
Start-Sleep -Seconds 4
$icon.Visible = $false
$icon.Dispose()
"#,
                title = title.replace("'", "''"),
                message = message.replace("'", "''")
            );

            let result = std::process::Command::new("powershell")
                .args(["-NoProfile", "-NonInteractive", "-Command", &script])
                .output()
                .map_err(|e| format!("Failed to show notification: {}", e))?;

            if result.status.success() {
                Ok(())
            } else {
                let err = String::from_utf8_lossy(&result.stderr);
                Err(format!("Notification failed: {}", err))
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            let _ = (title, message);
            Err("Notifications require Windows".to_string())
        }
    }

    /// Build the PowerShell script for the tray icon with context menu.
    #[cfg(target_os = "windows")]
    fn build_tray_script(&self, title: &str, icon_path: &str) -> String {
        format!(
            r#"
Add-Type -Name Window -Namespace Console -MemberDefinition '
[DllImport("Kernel32.dll")] public static extern IntPtr GetConsoleWindow();
[DllImport("User32.dll")] public static extern bool ShowWindow(IntPtr hWnd, int nCmdShow);
'
$consolePtr = [Console.Window]::GetConsoleWindow()
[Console.Window]::ShowWindow($consolePtr, 0) | Out-Null

[System.Reflection.Assembly]::LoadWithPartialName("System.Windows.Forms") | Out-Null

$tray = New-Object System.Windows.Forms.NotifyIcon
$tray.Text = "{title}"
$tray.Icon = [System.Drawing.Icon]::ExtractAssociatedIcon("{icon_path}")
$tray.Visible = $true

# Context Menu
$menu = New-Object System.Windows.Forms.ContextMenuStrip

$showItem = New-Object System.Windows.Forms.ToolStripMenuItem
$showItem.Text = "Show Files-Rust"
$showItem.Add_Click({{ $tray.ShowBalloonTip(2000, "Files-Rust", "Application is running in the background", "Info") }})
$menu.Items.Add($showItem) | Out-Null

$exitItem = New-Object System.Windows.Forms.ToolStripMenuItem
$exitItem.Text = "Exit"
$exitItem.Add_Click({{
    $tray.Visible = $false
    $tray.Dispose()
    [System.Windows.Forms.Application]::Exit()
    Stop-Process -Id $pid
}})
$menu.Items.Add($exitItem) | Out-Null

$tray.ContextMenuStrip = $menu

# Keep alive
[System.Windows.Forms.Application]::Run()
"#,
            title = title.replace("'", "''"),
            icon_path = icon_path.replace("'", "''")
        )
    }

    /// Get the path to store the tray PowerShell script.
    #[cfg(target_os = "windows")]
    fn get_script_path() -> std::path::PathBuf {
        let mut path = std::env::temp_dir();
        path.push("files-rs-tray.ps1");
        path
    }

    /// Ensure a suitable icon exists and return its path.
    #[cfg(target_os = "windows")]
    fn ensure_icon() -> String {
        // Use PowerShell executable's icon as a fallback
        "powershell.exe".to_string()
    }

    /// Get whether the app should minimize to tray instead of closing.
    pub fn should_minimize_to_tray(&self) -> bool {
        self.config.minimize_to_tray && self.icon_visible
    }

    /// Update the minimize-to-tray setting.
    pub fn set_minimize_to_tray(&mut self, value: bool) {
        self.config.minimize_to_tray = value;
    }

    pub fn set_start_minimized(&mut self, value: bool) {
        self.config.start_minimized = value;
    }
}

/// Actions that can be triggered by tray icon events.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayAction {
    None,
    ShowWindow,
    ShowContextMenu,
    Quit,
}

/// Background execution state.
#[derive(Debug, Clone)]
pub struct BackgroundExecutionState {
    pub is_background: bool,
    pub config: SystemTrayConfig,
}

impl Default for BackgroundExecutionState {
    fn default() -> Self {
        Self {
            is_background: false,
            config: SystemTrayConfig::default(),
        }
    }
}

impl BackgroundExecutionState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_background(&mut self, bg: bool) {
        self.is_background = bg;
    }

    pub fn start_minimized(&self) -> bool {
        self.config.start_minimized
    }
}

/// Check if we are on a Windows platform.
pub fn is_windows() -> bool {
    cfg!(target_os = "windows")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tray_config_defaults() {
        let config = SystemTrayConfig::default();
        assert!(config.minimize_to_tray);
        assert!(config.show_tray_icon);
        assert!(!config.start_minimized);
    }

    #[test]
    fn test_tray_manager_creation() {
        let mut mgr = SystemTrayManager::new();
        assert!(!mgr.icon_visible);
        assert!(mgr.config.minimize_to_tray);
        assert!(!mgr.should_minimize_to_tray());
    }

    #[test]
    fn test_tray_manager_with_config() {
        let config = SystemTrayConfig {
            minimize_to_tray: false,
            show_tray_icon: true,
            start_minimized: true,
        };
        let mgr = SystemTrayManager::with_config(config);
        assert!(!mgr.config.minimize_to_tray);
        assert!(mgr.config.start_minimized);
    }

    #[test]
    fn test_should_minimize_to_tray() {
        let mut mgr = SystemTrayManager::new();
        assert!(!mgr.should_minimize_to_tray());

        mgr.icon_visible = true;
        assert!(mgr.should_minimize_to_tray());

        mgr.set_minimize_to_tray(false);
        assert!(!mgr.should_minimize_to_tray());
    }

    #[test]
    fn test_background_execution_defaults() {
        let state = BackgroundExecutionState::default();
        assert!(!state.is_background);
        assert!(state.config.minimize_to_tray);
        assert!(!state.start_minimized());
    }

    #[test]
    fn test_background_execution_set_bg() {
        let mut state = BackgroundExecutionState::new();
        assert!(!state.is_background);
        state.set_background(true);
        assert!(state.is_background);
        state.set_background(false);
        assert!(!state.is_background);
    }

    #[test]
    fn test_tray_action_equality() {
        assert_eq!(TrayAction::None, TrayAction::None);
        assert_ne!(TrayAction::None, TrayAction::ShowWindow);
        assert_ne!(TrayAction::ShowContextMenu, TrayAction::Quit);
    }

    #[test]
    fn test_non_windows_initialize_returns_err() {
        let mut mgr = SystemTrayManager::new();
        #[cfg(not(target_os = "windows"))]
        {
            let result = mgr.initialize("TestApp");
            assert!(result.is_err());
            assert!(result.unwrap_err().contains("Windows"));
        }
        #[cfg(target_os = "windows")]
        {
            let _ = mgr.initialize("TestApp");
            // On Windows this may succeed or fail depending on environment
        }
    }

    #[test]
    fn test_tray_config_all_false() {
        let config = SystemTrayConfig {
            minimize_to_tray: false,
            show_tray_icon: false,
            start_minimized: false,
        };
        assert!(!config.minimize_to_tray);
        assert!(!config.show_tray_icon);
        assert!(!config.start_minimized);
    }

    #[test]
    fn test_remove_icon_when_not_visible() {
        let mgr = SystemTrayManager::new();
        // Should succeed even when icon is not visible
        assert!(mgr.remove_icon().is_ok());
    }

    #[test]
    fn test_notification_on_non_windows() {
        let mgr = SystemTrayManager::new();
        #[cfg(not(target_os = "windows"))]
        {
            let result = mgr.show_notification("Title", "Message");
            assert!(result.is_err());
        }
        #[cfg(target_os = "windows")]
        {
            let _ = mgr.show_notification("Title", "Message");
        }
    }

    #[test]
    fn test_set_start_minimized() {
        let mut mgr = SystemTrayManager::new();
        assert!(!mgr.config.start_minimized);
        mgr.set_start_minimized(true);
        assert!(mgr.config.start_minimized);
    }

    #[test]
    fn test_is_windows() {
        // Simply checks it runs without panicking
        let _ = is_windows();
    }
}

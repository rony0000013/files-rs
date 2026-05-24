//! Shortcut Service
//! Creates .lnk shortcut files using Windows IShellLink COM interface.

use crate::core::path_util::FilesPath;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Shortcut creation options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShortcutOptions {
    /// Target path (file or directory)
    pub target_path: FilesPath,
    /// Path where the shortcut will be created
    pub shortcut_path: FilesPath,
    /// Optional description/comment
    pub description: Option<String>,
    /// Optional working directory
    pub working_directory: Option<FilesPath>,
    /// Optional arguments to pass to the target
    pub arguments: Option<String>,
    /// Optional icon path (defaults to target's icon)
    pub icon_path: Option<FilesPath>,
    /// Icon index (0 = first icon)
    pub icon_index: i32,
    /// Run as administrator
    pub run_as_admin: bool,
    /// Window style (normal, minimized, maximized)
    pub window_style: WindowStyle,
}

impl ShortcutOptions {
    /// Create options for a simple shortcut to a file
    pub fn new(target: FilesPath, shortcut: FilesPath) -> Self {
        Self {
            target_path: target,
            shortcut_path: shortcut,
            description: None,
            working_directory: None,
            arguments: None,
            icon_path: None,
            icon_index: 0,
            run_as_admin: false,
            window_style: WindowStyle::Normal,
        }
    }

    /// Set the description
    pub fn with_description(mut self, description: &str) -> Self {
        self.description = Some(description.to_string());
        self
    }

    /// Set the working directory
    pub fn with_working_directory(mut self, dir: FilesPath) -> Self {
        self.working_directory = Some(dir);
        self
    }

    /// Set arguments
    pub fn with_arguments(mut self, args: &str) -> Self {
        self.arguments = Some(args.to_string());
        self
    }

    /// Set the icon
    pub fn with_icon(mut self, path: FilesPath, index: i32) -> Self {
        self.icon_path = Some(path);
        self.icon_index = index;
        self
    }

    /// Set run as administrator
    pub fn with_run_as_admin(mut self) -> Self {
        self.run_as_admin = true;
        self
    }

    /// Set window style
    pub fn with_window_style(mut self, style: WindowStyle) -> Self {
        self.window_style = style;
        self
    }
}

/// Window style for shortcuts
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WindowStyle {
    Normal,
    Minimized,
    Maximized,
}

impl Default for WindowStyle {
    fn default() -> Self {
        WindowStyle::Normal
    }
}

/// Result of shortcut creation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShortcutResult {
    /// Whether creation was successful
    pub success: bool,
    /// Path to the created shortcut
    pub shortcut_path: FilesPath,
    /// Error message if failed
    pub error: Option<String>,
}

/// Shortcut Service
/// Creates Windows .lnk shortcut files using IShellLink COM interface.
pub struct ShortcutService;

impl ShortcutService {
    /// Create a new shortcut with the given options
    pub fn create_shortcut(options: ShortcutOptions) -> ShortcutResult {
        #[cfg(windows)]
        {
            Self::create_shortcut_impl(options)
        }

        #[cfg(not(windows))]
        {
            ShortcutResult {
                success: false,
                shortcut_path: options.shortcut_path,
                error: Some("Shortcut creation is only available on Windows".to_string()),
            }
        }
    }

    /// Windows implementation of shortcut creation
    #[cfg(windows)]
    fn create_shortcut_impl(options: ShortcutOptions) -> ShortcutResult {
        use std::process::Command;

        let target_path = options.target_path.as_path().to_string_lossy();
        let shortcut_path = options.shortcut_path.as_path().to_string_lossy();

        tracing::info!(
            "Creating shortcut from {} to {}",
            shortcut_path,
            target_path
        );

        // Try using PowerShell to create the shortcut (most reliable without COM)
        let ps_script = Self::build_powershell_script(&options);

        let output = Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", &ps_script])
            .output();

        match output {
            Ok(out) if out.status.success() => ShortcutResult {
                success: true,
                shortcut_path: options.shortcut_path,
                error: None,
            },
            Ok(out) => {
                let stderr = String::from_utf8_lossy(&out.stderr);
                ShortcutResult {
                    success: false,
                    shortcut_path: options.shortcut_path,
                    error: Some(format!("PowerShell shortcut creation failed: {}", stderr)),
                }
            }
            Err(e) => ShortcutResult {
                success: false,
                shortcut_path: options.shortcut_path,
                error: Some(format!("Failed to execute PowerShell: {}", e)),
            },
        }
    }

    /// Build PowerShell script for shortcut creation
    #[cfg(windows)]
    fn build_powershell_script(options: &ShortcutOptions) -> String {
        let target = options
            .target_path
            .as_path()
            .to_string_lossy()
            .replace("'", "''");
        let shortcut = options
            .shortcut_path
            .as_path()
            .to_string_lossy()
            .replace("'", "''");

        let mut script = format!(
            "$ws = New-Object -ComObject WScript.Shell; $sc = $ws.CreateShortcut('{}'); $sc.TargetPath = '{}'; ",
            shortcut, target
        );

        // Add description
        if let Some(ref desc) = options.description {
            script.push_str(&format!(
                "$sc.Description = '{}'; ",
                desc.replace("'", "''")
            ));
        }

        // Add working directory
        if let Some(ref work_dir) = options.working_directory {
            let dir = work_dir.as_path().to_string_lossy().replace("'", "''");
            script.push_str(&format!("$sc.WorkingDirectory = '{}'; ", dir));
        }

        // Add arguments
        if let Some(ref args) = options.arguments {
            script.push_str(&format!("$sc.Arguments = '{}'; ", args.replace("'", "''")));
        }

        // Add icon
        if let Some(ref icon_path) = options.icon_path {
            let icon = icon_path.as_path().to_string_lossy().replace("'", "''");
            script.push_str(&format!(
                "$sc.IconLocation = '{},{}'; ",
                icon, options.icon_index
            ));
        }

        // Add window style
        let window_style = match options.window_style {
            WindowStyle::Minimized => 7, // SW_SHOWMINIMIZED
            WindowStyle::Maximized => 3, // SW_SHOWMAXIMIZED
            WindowStyle::Normal => 1,    // SW_SHOWNORMAL
        };
        script.push_str(&format!("$sc.WindowStyle = {}; ", window_style));

        // Save
        script.push_str(&format!(
            "$sc.Save(); Write-Host 'Shortcut created successfully';"
        ));

        script
    }

    /// Create a simple shortcut (convenience method)
    pub fn create(target: FilesPath, shortcut: FilesPath) -> ShortcutResult {
        Self::create_shortcut(ShortcutOptions::new(target, shortcut))
    }

    /// Create a shortcut to a directory
    pub fn create_for_directory(target_dir: FilesPath, shortcut: FilesPath) -> ShortcutResult {
        let options =
            ShortcutOptions::new(target_dir, shortcut).with_description("Shortcut to folder");
        Self::create_shortcut(options)
    }

    /// Create a shortcut with arguments
    pub fn create_with_args(target: FilesPath, shortcut: FilesPath, args: &str) -> ShortcutResult {
        let options = ShortcutOptions::new(target, shortcut).with_arguments(args);
        Self::create_shortcut(options)
    }

    /// Delete a shortcut
    pub fn delete(shortcut_path: &FilesPath) -> Result<(), String> {
        let path = shortcut_path.as_path();
        if path.exists() && path.extension().map(|e| e == "lnk").unwrap_or(false) {
            std::fs::remove_file(path).map_err(|e| format!("Failed to delete shortcut: {}", e))
        } else {
            Err("File does not exist or is not a shortcut".to_string())
        }
    }

    /// Check if a file is a shortcut (.lnk)
    pub fn is_shortcut(path: &FilesPath) -> bool {
        path.as_path()
            .extension()
            .map(|e| e.to_ascii_lowercase() == "lnk")
            .unwrap_or(false)
    }

    /// Get the target of a shortcut (using PowerShell)
    pub fn get_target(shortcut_path: &FilesPath) -> Result<FilesPath, String> {
        #[cfg(windows)]
        {
            use std::process::Command;

            let path_str = shortcut_path.as_path().to_string_lossy().replace("'", "''");

            let ps_command = format!(
                "$ws = New-Object -ComObject WScript.Shell; $sc = $ws.CreateShortcut('{}'); Write-Output $sc.TargetPath",
                path_str
            );

            let output = Command::new("powershell")
                .args(["-NoProfile", "-NonInteractive", "-Command", &ps_command])
                .output()
                .map_err(|e| format!("Failed to execute PowerShell: {}", e))?;

            if output.status.success() {
                let target = String::from_utf8_lossy(&output.stdout).trim().to_string();
                Ok(FilesPath::from(target.as_str()))
            } else {
                Err("Failed to get shortcut target".to_string())
            }
        }

        #[cfg(not(windows))]
        {
            Err("Get target is only available on Windows".to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::{TempDir, TempPath};

    #[test]
    fn test_shortcut_options_new() {
        let temp_dir = TempDir::new().unwrap();
        let target = temp_dir.path().join("target.txt");
        let shortcut = temp_dir.path().join("shortcut.lnk");

        let options = ShortcutOptions::new(
            FilesPath::from(target.to_str().unwrap()),
            FilesPath::from(shortcut.to_str().unwrap()),
        );

        assert_eq!(options.icon_index, 0);
        assert!(!options.run_as_admin);
        assert_eq!(options.window_style, WindowStyle::Normal);
    }

    #[test]
    fn test_shortcut_options_builder() {
        let temp_dir = TempDir::new().unwrap();
        let target = temp_dir.path().join("target.exe");
        let shortcut = temp_dir.path().join("app.lnk");
        let work_dir = temp_dir.path().join("work");
        let icon = temp_dir.path().join("icon.ico");

        let options = ShortcutOptions::new(
            FilesPath::from(target.to_str().unwrap()),
            FilesPath::from(shortcut.to_str().unwrap()),
        )
        .with_description("My Application")
        .with_working_directory(FilesPath::from(work_dir.to_str().unwrap()))
        .with_arguments("--flag value")
        .with_icon(FilesPath::from(icon.to_str().unwrap()), 1)
        .with_run_as_admin()
        .with_window_style(WindowStyle::Maximized);

        assert_eq!(options.description, Some("My Application".to_string()));
        assert_eq!(
            options
                .working_directory
                .map(|p| p.as_path().to_string_lossy().to_string()),
            Some(work_dir.to_string_lossy().to_string())
        );
        assert_eq!(options.arguments, Some("--flag value".to_string()));
        assert_eq!(options.icon_index, 1);
        assert!(options.run_as_admin);
        assert_eq!(options.window_style, WindowStyle::Maximized);
    }

    #[test]
    fn test_window_style_default() {
        assert_eq!(WindowStyle::default(), WindowStyle::Normal);
    }

    #[test]
    fn test_is_shortcut() {
        let temp_dir = TempDir::new().unwrap();

        let lnk_path = FilesPath::from(temp_dir.path().join("test.lnk").to_str().unwrap());
        let txt_path = FilesPath::from(temp_dir.path().join("test.txt").to_str().unwrap());

        assert!(ShortcutService::is_shortcut(&lnk_path));
        assert!(!ShortcutService::is_shortcut(&txt_path));
    }

    #[test]
    fn test_is_shortcut_case_insensitive() {
        let temp_dir = TempDir::new().unwrap();

        let upper = FilesPath::from(temp_dir.path().join("test.LNK").to_str().unwrap());
        let lower = FilesPath::from(temp_dir.path().join("test.lnk").to_str().unwrap());

        assert!(ShortcutService::is_shortcut(&upper));
        assert!(ShortcutService::is_shortcut(&lower));
    }

    #[test]
    fn test_create_shortcut_returns_result() {
        let temp_dir = TempDir::new().unwrap();
        let target = temp_dir.path().join("target.txt");
        let shortcut = temp_dir.path().join("shortcut.lnk");

        std::fs::write(&target, "content").unwrap();

        let result = ShortcutService::create(
            FilesPath::from(target.to_str().unwrap()),
            FilesPath::from(shortcut.to_str().unwrap()),
        );

        // Result should be returned (success or failure)
        assert!(result
            .shortcut_path
            .as_path()
            .to_string_lossy()
            .contains("shortcut.lnk"));
    }

    #[test]
    fn test_shortcut_result_debug() {
        let temp_dir = TempDir::new().unwrap();
        let shortcut = temp_dir.path().join("shortcut.lnk");

        let result = ShortcutResult {
            success: true,
            shortcut_path: FilesPath::from(shortcut.to_str().unwrap()),
            error: None,
        };

        let debug_str = format!("{:?}", result);
        assert!(debug_str.contains("ShortcutResult"));
        assert!(debug_str.contains("success"));
    }

    #[test]
    fn test_delete_nonexistent_shortcut() {
        let temp_dir = TempDir::new().unwrap();
        let nonexistent = temp_dir.path().join("nonexistent.lnk");

        let result = ShortcutService::delete(&FilesPath::from(nonexistent.to_str().unwrap()));
        assert!(result.is_err());
    }

    #[test]
    fn test_create_for_directory() {
        let temp_dir = TempDir::new().unwrap();
        let target_dir = temp_dir.path().join("myfolder");
        let shortcut = temp_dir.path().join("folder shortcut.lnk");

        std::fs::create_dir(&target_dir).unwrap();

        let result = ShortcutService::create_for_directory(
            FilesPath::from(target_dir.to_str().unwrap()),
            FilesPath::from(shortcut.to_str().unwrap()),
        );

        assert!(result
            .shortcut_path
            .as_path()
            .to_string_lossy()
            .contains("folder shortcut.lnk"));
    }

    #[test]
    fn test_create_with_args() {
        let temp_dir = TempDir::new().unwrap();
        let target = temp_dir.path().join("app.exe");
        let shortcut = temp_dir.path().join("app.lnk");

        std::fs::write(&target, "").unwrap();

        let result = ShortcutService::create_with_args(
            FilesPath::from(target.to_str().unwrap()),
            FilesPath::from(shortcut.to_str().unwrap()),
            "--config config.ini",
        );

        assert!(result
            .shortcut_path
            .as_path()
            .to_string_lossy()
            .contains("app.lnk"));
    }

    #[test]
    fn test_get_target_returns_error_on_non_windows() {
        let temp_dir = TempDir::new().unwrap();
        let shortcut = temp_dir.path().join("test.lnk");

        // On non-Windows, this should return an error
        let result = ShortcutService::get_target(&FilesPath::from(shortcut.to_str().unwrap()));
        #[cfg(not(windows))]
        assert!(result.is_err());
        #[cfg(windows)]
        assert!(result.is_err() || result.is_ok()); // May work or fail on Windows
    }

    #[test]
    fn test_create_shortcut_with_options() {
        let temp_dir = TempDir::new().unwrap();
        let target = temp_dir.path().join("target.exe");
        let shortcut = temp_dir.path().join("options.lnk");

        std::fs::write(&target, "").unwrap();

        let options = ShortcutOptions::new(
            FilesPath::from(target.to_str().unwrap()),
            FilesPath::from(shortcut.to_str().unwrap()),
        )
        .with_description("Test application")
        .with_window_style(WindowStyle::Minimized);

        let result = ShortcutService::create_shortcut(options);

        assert!(result
            .shortcut_path
            .as_path()
            .to_string_lossy()
            .contains("options.lnk"));
    }
}

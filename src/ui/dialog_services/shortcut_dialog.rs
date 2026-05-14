//! Create Shortcut Dialog Service
//! State machine for CreateShortcutDialog.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Window style for shortcuts
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShortcutWindowStyle {
    Normal,
    Minimized,
    Maximized,
}

impl Default for ShortcutWindowStyle {
    fn default() -> Self {
        Self::Normal
    }
}

impl ShortcutWindowStyle {
    /// Get the display name
    pub fn display_name(&self) -> &'static str {
        match self {
            ShortcutWindowStyle::Normal => "Normal window",
            ShortcutWindowStyle::Minimized => "Minimized",
            ShortcutWindowStyle::Maximized => "Maximized",
        }
    }
}

/// State for CreateShortcutDialog
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CreateShortcutState {
    /// Target path (file or directory the shortcut points to)
    pub target_path: PathBuf,
    /// Directory where the shortcut will be created
    pub shortcut_directory: PathBuf,
    /// Shortcut name (without .lnk extension)
    pub shortcut_name: String,
    /// Optional description/comment
    pub description: Option<String>,
    /// Working directory for the target
    pub working_directory: Option<PathBuf>,
    /// Arguments to pass to the target
    pub arguments: Option<String>,
    /// Icon path (defaults to target's icon)
    pub icon_path: Option<PathBuf>,
    /// Icon index within the file (0 = first)
    pub icon_index: i32,
    /// Run as administrator
    pub run_as_admin: bool,
    /// Window style
    pub window_style: ShortcutWindowStyle,
    /// Show advanced options
    pub show_advanced: bool,
    /// Error message
    pub error: Option<String>,
}

impl Default for CreateShortcutState {
    fn default() -> Self {
        Self {
            target_path: PathBuf::new(),
            shortcut_directory: PathBuf::new(),
            shortcut_name: String::new(),
            description: None,
            working_directory: None,
            arguments: None,
            icon_path: None,
            icon_index: 0,
            run_as_admin: false,
            window_style: ShortcutWindowStyle::default(),
            show_advanced: false,
            error: None,
        }
    }
}

impl CreateShortcutState {
    /// Create a new shortcut state with target path
    pub fn new(target: PathBuf, shortcut_dir: PathBuf, name: String) -> Self {
        Self {
            target_path: target,
            shortcut_directory: shortcut_dir,
            shortcut_name: name,
            ..Default::default()
        }
    }

    /// Get the full shortcut path (.lnk)
    pub fn shortcut_path(&self) -> PathBuf {
        let mut path = self.shortcut_directory.clone();
        // Ensure we have a name
        let name = if self.shortcut_name.is_empty() {
            "Shortcut"
        } else {
            &self.shortcut_name
        };
        // Add .lnk extension if not present
        if name.ends_with(".lnk") {
            path.push(name);
        } else {
            path.push(format!("{}.lnk", name));
        }
        path
    }

    /// Set the shortcut name
    pub fn set_name(&mut self, name: String) {
        self.shortcut_name = name;
    }

    /// Set description
    pub fn set_description(&mut self, desc: Option<String>) {
        self.description = desc;
    }

    /// Set working directory
    pub fn set_working_directory(&mut self, dir: Option<PathBuf>) {
        self.working_directory = dir;
    }

    /// Set arguments
    pub fn set_arguments(&mut self, args: Option<String>) {
        self.arguments = args;
    }

    /// Set icon path
    pub fn set_icon_path(&mut self, path: Option<PathBuf>) {
        self.icon_path = path;
    }

    /// Set icon index
    pub fn set_icon_index(&mut self, index: i32) {
        self.icon_index = index.max(0);
    }

    /// Toggle run as admin
    pub fn toggle_run_as_admin(&mut self) {
        self.run_as_admin = !self.run_as_admin;
    }

    /// Set window style
    pub fn set_window_style(&mut self, style: ShortcutWindowStyle) {
        self.window_style = style;
    }

    /// Toggle advanced options visibility
    pub fn toggle_advanced(&mut self) {
        self.show_advanced = !self.show_advanced;
    }

    /// Validate the shortcut settings
    pub fn validate(&self) -> Result<(), String> {
        // Name validation first (most user-facing error)
        if self.shortcut_name.is_empty() {
            return Err("Shortcut name is required".to_string());
        }
        // Check for invalid characters in name
        let invalid_chars = ['\\', '/', ':', '*', '?', '"', '<', '>', '|'];
        if self.shortcut_name.chars().any(|c| invalid_chars.contains(&c)) {
            return Err("Shortcut name contains invalid characters".to_string());
        }
        // Target path validation
        if self.target_path.as_os_str().is_empty() {
            return Err("Target path is required".to_string());
        }
        if !self.target_path.exists() {
            return Err(format!("Target does not exist: {}", self.target_path.display()));
        }
        // Directory validation
        if self.shortcut_directory.as_os_str().is_empty() {
            return Err("Shortcut location is required".to_string());
        }
        if !self.shortcut_directory.exists() {
            return Err(format!(
                "Shortcut location does not exist: {}",
                self.shortcut_directory.display()
            ));
        }
        Ok(())
    }

    /// Check if the shortcut name has valid extension (.lnk is added automatically)
    pub fn has_valid_name(&self) -> bool {
        !self.shortcut_name.is_empty()
            && !self.shortcut_name.ends_with('.')
            && self.shortcut_name.len() <= 255
    }

    /// Get the final shortcut path with validation
    pub fn final_path(&self) -> Result<PathBuf, String> {
        self.validate()?;
        Ok(self.shortcut_path())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shortcut_window_style_default() {
        assert_eq!(ShortcutWindowStyle::default(), ShortcutWindowStyle::Normal);
    }

    #[test]
    fn test_shortcut_window_style_display_name() {
        assert_eq!(ShortcutWindowStyle::Normal.display_name(), "Normal window");
        assert_eq!(ShortcutWindowStyle::Minimized.display_name(), "Minimized");
        assert_eq!(ShortcutWindowStyle::Maximized.display_name(), "Maximized");
    }

    #[test]
    fn test_create_shortcut_state_default() {
        let state = CreateShortcutState::default();
        assert!(state.target_path.as_os_str().is_empty());
        assert!(state.shortcut_directory.as_os_str().is_empty());
        assert!(state.shortcut_name.is_empty());
        assert!(state.description.is_none());
        assert!(state.working_directory.is_none());
        assert!(state.arguments.is_none());
        assert!(state.icon_path.is_none());
        assert_eq!(state.icon_index, 0);
        assert!(!state.run_as_admin);
        assert_eq!(state.window_style, ShortcutWindowStyle::Normal);
        assert!(!state.show_advanced);
        assert!(state.error.is_none());
    }

    #[test]
    fn test_create_shortcut_state_new() {
        let target = PathBuf::from("C:\\test\\app.exe");
        let dir = PathBuf::from("C:\\test\\shortcuts");
        let name = "My App".to_string();
        let state = CreateShortcutState::new(target.clone(), dir.clone(), name.clone());
        assert_eq!(state.target_path, target);
        assert_eq!(state.shortcut_directory, dir);
        assert_eq!(state.shortcut_name, name);
    }

    #[test]
    fn test_create_shortcut_state_shortcut_path() {
        let mut state = CreateShortcutState::default();
        state.shortcut_directory = PathBuf::from("C:/test");
        state.shortcut_name = "MyApp".to_string();
        
        let path = state.shortcut_path();
        let path_str = path.to_string_lossy().replace('\\', "/");
        assert_eq!(path_str, "C:/test/MyApp.lnk");
    }

    #[test]
    fn test_create_shortcut_state_shortcut_path_with_extension() {
        let mut state = CreateShortcutState::default();
        state.shortcut_directory = PathBuf::from("C:/test");
        state.shortcut_name = "MyApp.lnk".to_string();
        
        let path = state.shortcut_path();
        let path_str = path.to_string_lossy().replace('\\', "/");
        assert_eq!(path_str, "C:/test/MyApp.lnk");
    }

    #[test]
    fn test_create_shortcut_state_shortcut_path_empty_name() {
        let mut state = CreateShortcutState::default();
        state.shortcut_directory = PathBuf::from("C:/test");
        state.shortcut_name = "".to_string();
        
        let path = state.shortcut_path();
        let path_str = path.to_string_lossy().replace('\\', "/");
        assert_eq!(path_str, "C:/test/Shortcut.lnk");
    }

    #[test]
    fn test_create_shortcut_state_set_name() {
        let mut state = CreateShortcutState::default();
        state.set_name("New Name".to_string());
        assert_eq!(state.shortcut_name, "New Name");
    }

    #[test]
    fn test_create_shortcut_state_set_description() {
        let mut state = CreateShortcutState::default();
        state.set_description(Some("Test description".to_string()));
        assert_eq!(state.description, Some("Test description".to_string()));
        state.set_description(None);
        assert!(state.description.is_none());
    }

    #[test]
    fn test_create_shortcut_state_set_working_directory() {
        let mut state = CreateShortcutState::default();
        let dir = PathBuf::from("C:\\test");
        state.set_working_directory(Some(dir.clone()));
        assert_eq!(state.working_directory, Some(dir));
        state.set_working_directory(None);
        assert!(state.working_directory.is_none());
    }

    #[test]
    fn test_create_shortcut_state_set_arguments() {
        let mut state = CreateShortcutState::default();
        state.set_arguments(Some("--flag value".to_string()));
        assert_eq!(state.arguments, Some("--flag value".to_string()));
        state.set_arguments(None);
        assert!(state.arguments.is_none());
    }

    #[test]
    fn test_create_shortcut_state_set_icon_path() {
        let mut state = CreateShortcutState::default();
        let path = PathBuf::from("C:\\test\\icon.ico");
        state.set_icon_path(Some(path.clone()));
        assert_eq!(state.icon_path, Some(path));
        state.set_icon_path(None);
        assert!(state.icon_path.is_none());
    }

    #[test]
    fn test_create_shortcut_state_set_icon_index() {
        let mut state = CreateShortcutState::default();
        state.set_icon_index(5);
        assert_eq!(state.icon_index, 5);
        state.set_icon_index(-1);
        assert_eq!(state.icon_index, 0); // Clamped to 0
    }

    #[test]
    fn test_create_shortcut_state_toggle_run_as_admin() {
        let mut state = CreateShortcutState::default();
        assert!(!state.run_as_admin);
        state.toggle_run_as_admin();
        assert!(state.run_as_admin);
        state.toggle_run_as_admin();
        assert!(!state.run_as_admin);
    }

    #[test]
    fn test_create_shortcut_state_set_window_style() {
        let mut state = CreateShortcutState::default();
        assert_eq!(state.window_style, ShortcutWindowStyle::Normal);
        state.set_window_style(ShortcutWindowStyle::Minimized);
        assert_eq!(state.window_style, ShortcutWindowStyle::Minimized);
        state.set_window_style(ShortcutWindowStyle::Maximized);
        assert_eq!(state.window_style, ShortcutWindowStyle::Maximized);
    }

    #[test]
    fn test_create_shortcut_state_toggle_advanced() {
        let mut state = CreateShortcutState::default();
        assert!(!state.show_advanced);
        state.toggle_advanced();
        assert!(state.show_advanced);
        state.toggle_advanced();
        assert!(!state.show_advanced);
    }

    #[test]
    fn test_create_shortcut_state_validate_empty_target() {
        let mut state = CreateShortcutState::default();
        state.shortcut_name = "MyApp".to_string(); // Set name first to pass name validation
        state.target_path = PathBuf::new();
        let result = state.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Target path is required"));
    }

    #[test]
    fn test_create_shortcut_state_validate_empty_name() {
        let mut state = CreateShortcutState::default();
        state.target_path = PathBuf::from("C:\\test\\app.exe");
        state.shortcut_directory = PathBuf::from("C:\\test");
        state.shortcut_name = "".to_string();
        let result = state.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Shortcut name is required"));
    }

    #[test]
    fn test_create_shortcut_state_validate_invalid_chars() {
        let invalid_names = [
            "file:name",
            "file|name",
            "file*name",
            "file?name",
            "file<name",
            "file>name",
            "file\"name",
            "file/name",
            "file\\name",
            "file:name",
        ];
        for name in invalid_names {
            let mut state = CreateShortcutState::default();
            state.target_path = PathBuf::from("/tmp/test.exe"); // Non-empty target
            state.shortcut_directory = PathBuf::from("/tmp"); // Valid directory
            state.shortcut_name = name.to_string();
            let result = state.validate();
            assert!(result.is_err(), "Expected '{}' to be invalid", name);
            assert!(result.as_ref().unwrap_err().contains("invalid characters"), "Expected invalid chars error, got: {:?}", result.unwrap_err());
        }
    }

    #[test]
    fn test_create_shortcut_state_has_valid_name() {
        let mut state = CreateShortcutState::default();
        state.shortcut_name = "".to_string();
        assert!(!state.has_valid_name());
        
        state.shortcut_name = "valid_name.lnk".to_string();
        assert!(state.has_valid_name());
        
        state.shortcut_name = "valid".to_string();
        assert!(state.has_valid_name());
        
        state.shortcut_name = ".".to_string();
        assert!(!state.has_valid_name()); // Ends with dot
        
        state.shortcut_name = "a".repeat(256); // Too long
        assert!(!state.has_valid_name());
    }

    #[test]
    fn test_create_shortcut_state_final_path() {
        let mut state = CreateShortcutState::default();
        state.shortcut_directory = PathBuf::from("C:\\test");
        state.shortcut_name = "MyApp".to_string();
        
        let result = state.final_path();
        // Should fail because target doesn't exist
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Target path is required"));
    }
}
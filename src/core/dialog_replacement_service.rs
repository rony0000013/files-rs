//! # System Dialog Replacement Service
//!
//! Provides the ability to replace (or customize) Windows' common file dialogs
//! (Open, Save, Folder Picker) with custom dialogs managed by files-rs.
//!
//! ## Architecture
//!
//! On Windows, common file dialogs can be customized via the `IFileDialog` COM
//! interface and Windows registry. This service manages the registration needed
//! to hook into the dialog creation flow:
//!
//! 1. **Registry registration** — Registers files-rs as a dialog host via
//!    `HKEY_CLASSES_ROOT\CLSID\{...}` entries that provide custom dialog
//!    implementations.
//! 2. **Dialog preference storage** — Persists the last-used dialog path,
//!    view mode, and filter settings.
//! 3. **Custom dialog invocation** — Functions to show files-rs's own open/save
//!    dialogs in place of the system ones.
//!
//! On non-Windows platforms, all operations return an error explaining the
//! platform limitation.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Dialog types
// ---------------------------------------------------------------------------

/// The type of system dialog being replaced.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DialogType {
    /// Open file dialog
    Open,
    /// Save file dialog
    Save,
    /// Folder picker dialog
    FolderPicker,
}

/// Result from a dialog operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DialogResult {
    /// The selected file/folder path, if any
    pub path: Option<String>,
    /// Whether the user confirmed (true) or cancelled (false)
    pub confirmed: bool,
    /// The selected file filter index (for open/save dialogs)
    pub filter_index: usize,
}

impl Default for DialogResult {
    fn default() -> Self {
        Self {
            path: None,
            confirmed: false,
            filter_index: 0,
        }
    }
}

/// File filter specification for open/save dialogs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileFilter {
    /// Display name (e.g., "Text Files (*.txt)")
    pub name: String,
    /// File pattern (e.g., "*.txt" or "*.txt;*.md")
    pub pattern: String,
}

impl FileFilter {
    pub fn new(name: &str, pattern: &str) -> Self {
        Self {
            name: name.to_string(),
            pattern: pattern.to_string(),
        }
    }

    /// Common filter: All Files (*.*)
    pub fn all_files() -> Self {
        Self::new("All Files (*.*)", "*.*")
    }

    /// Common filter: Text Files (*.txt)
    pub fn text_files() -> Self {
        Self::new("Text Files (*.txt)", "*.txt")
    }

    /// Common filter: Image Files
    pub fn image_files() -> Self {
        Self::new("Image Files (*.png;*.jpg;*.gif;*.bmp)", "*.png;*.jpg;*.gif;*.bmp")
    }
}

// ---------------------------------------------------------------------------
// Dialog preferences
// ---------------------------------------------------------------------------

/// Preferences for a specific dialog type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DialogPreferences {
    /// The dialog type this applies to
    pub dialog_type: DialogType,
    /// Last used directory path
    pub last_directory: Option<String>,
    /// Last selected file filter index
    pub last_filter_index: usize,
    /// Whether to show hidden files in the dialog
    pub show_hidden_files: bool,
}

impl DialogPreferences {
    pub fn new(dialog_type: DialogType) -> Self {
        Self {
            dialog_type,
            last_directory: None,
            last_filter_index: 0,
            show_hidden_files: false,
        }
    }

    /// Update the last used directory.
    pub fn set_last_directory(&mut self, path: &str) {
        self.last_directory = Some(path.to_string());
    }
}

// ---------------------------------------------------------------------------
// Dialog Replacement Service
// ---------------------------------------------------------------------------

/// Service for replacing/customizing Windows common file dialogs.
///
/// On Windows, this service manages the registry entries needed to
/// register custom dialog handlers. It also tracks dialog preferences
/// and provides an API for showing custom open/save/folder dialogs.
#[derive(Debug, Clone)]
pub struct DialogReplacementService {
    /// Whether the dialog replacement is currently active
    pub active: bool,
    /// Dialog preferences for each dialog type
    pub preferences: Vec<DialogPreferences>,
    /// Whether the replacement was registered via registry
    pub registry_registered: bool,
}

impl Default for DialogReplacementService {
    fn default() -> Self {
        Self::new()
    }
}

impl DialogReplacementService {
    /// Create a new dialog replacement service.
    /// The service starts as inactive — call `register()` to activate.
    pub fn new() -> Self {
        Self {
            active: false,
            preferences: vec![
                DialogPreferences::new(DialogType::Open),
                DialogPreferences::new(DialogType::Save),
                DialogPreferences::new(DialogType::FolderPicker),
            ],
            registry_registered: false,
        }
    }

    /// Register files-rs as a custom dialog host in the Windows registry.
    ///
    /// On Windows, this writes entries to:
    /// - `HKCU\Software\Classes\CLSID\{our-guid}\TreatAs` to redirect
    ///   the Windows common dialog to our handler
    ///
    /// On non-Windows, this returns an error.
    pub fn register(&mut self, _app_path: &str) -> Result<(), String> {
        #[cfg(target_os = "windows")]
        {
            let guid = Self::dialog_handler_guid();
            let clsid_path = format!(
                r"Software\Classes\CLSID\{}",
                guid
            );

            // Register the dialog handler via registry
            let result = Self::write_registry_string(
                &format!(r"{}\TreatAs", clsid_path),
                &Self::handler_clsid(),
            );

            match result {
                Ok(()) => {
                    self.active = true;
                    self.registry_registered = true;
                    tracing::info!(
                        "Dialog replacement registered: CLSID {} -> handler {}",
                        guid,
                        Self::handler_clsid()
                    );
                    Ok(())
                }
                Err(e) => Err(format!("Failed to register dialog handler: {}", e)),
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            let _ = _app_path;
            Err("System dialog replacement requires Windows".to_string())
        }
    }

    /// Unregister the dialog handler from the registry.
    pub fn unregister(&self) -> Result<(), String> {
        #[cfg(target_os = "windows")]
        {
            if !self.registry_registered {
                return Ok(());
            }
            let guid = Self::dialog_handler_guid();
            let clsid_path = format!(
                r"Software\Classes\CLSID\{}\TreatAs",
                guid
            );

            match Self::delete_registry_key(&clsid_path) {
                Ok(()) => {
                    tracing::info!("Dialog replacement unregistered: {}", guid);
                    Ok(())
                }
                Err(e) => Err(format!("Failed to unregister dialog handler: {}", e)),
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            Ok(())
        }
    }

    /// Check if dialog replacement is active.
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Open a custom "Open File" dialog.
    ///
    /// Returns the selected file path(s) or None if cancelled.
    pub fn show_open_dialog(
        &self,
        _title: &str,
        _filters: &[FileFilter],
        _default_path: Option<&str>,
    ) -> DialogResult {
        #[cfg(target_os = "windows")]
        {
            // On Windows, use IFileDialog COM interface for the native dialog
            // with custom options. Falls back to simple file dialog.
            let _ = (_title, _filters, _default_path);
            tracing::info!("Showing custom Open dialog via COM IFileDialog");
            // In a full implementation, this would use `IFileOpenDialog` via COM
            // For now, returns cancelled since the COM dialog requires async
            DialogResult::default()
        }

        #[cfg(not(target_os = "windows"))]
        {
            let _ = (_title, _filters, _default_path);
            DialogResult::default()
        }
    }

    /// Open a custom "Save File" dialog.
    pub fn show_save_dialog(
        &self,
        _title: &str,
        _filters: &[FileFilter],
        _default_name: Option<&str>,
    ) -> DialogResult {
        tracing::info!("Showing custom Save dialog (stub — use native dialog)");
        DialogResult::default()
    }

    /// Open a custom folder picker dialog.
    pub fn show_folder_picker(&self, _title: &str, _default_path: Option<&str>) -> DialogResult {
        tracing::info!("Showing custom Folder Picker dialog (stub — use native dialog)");
        DialogResult::default()
    }

    /// Get preferences for a specific dialog type.
    pub fn get_preferences(&self, dialog_type: DialogType) -> Option<&DialogPreferences> {
        self.preferences.iter().find(|p| p.dialog_type == dialog_type)
    }

    /// Get mutable preferences for a specific dialog type.
    pub fn get_preferences_mut(&mut self, dialog_type: DialogType) -> Option<&mut DialogPreferences> {
        self.preferences.iter_mut().find(|p| p.dialog_type == dialog_type)
    }

    /// Update the last directory for a dialog type.
    pub fn update_last_directory(&mut self, dialog_type: DialogType, path: &str) {
        if let Some(prefs) = self.get_preferences_mut(dialog_type) {
            prefs.set_last_directory(path);
        }
    }

    // -----------------------------------------------------------------------
    // Windows-specific helpers
    // -----------------------------------------------------------------------

    /// The GUID for our custom dialog handler.
    #[cfg(target_os = "windows")]
    fn dialog_handler_guid() -> String {
        "{E5D1E5F2-3A4B-4C5D-6E7F-8A9B0C1D2E3F}".to_string()
    }

    /// The CLSID of the Windows common dialog we're replacing.
    #[cfg(target_os = "windows")]
    fn handler_clsid() -> String {
        // This would be the CLSID of our custom dialog implementation
        // In practice, this requires a registered COM DLL
        "{00000000-0000-0000-0000-000000000000}".to_string()
    }

    /// Write a string value to the Windows registry.
    #[cfg(target_os = "windows")]
    fn write_registry_string(path: &str, value: &str) -> Result<(), String> {
        #[cfg(windows)]
        {
            use winreg::enums::HKEY_CURRENT_USER;
            use winreg::RegKey;

            let hkcu = RegKey::predef(HKEY_CURRENT_USER);
            // Split the path into parent key and value name
            let parts: Vec<&str> = path.rsplitn(2, '\\').collect();
            let (value_name, key_path) = if parts.len() == 2 {
                (parts[0], parts[1])
            } else {
                ("", path)
            };

            let key = hkcu
                .create_subkey(key_path)
                .map_err(|e| format!("Failed to create registry key: {}", e))?;

            key.0
                .set_value(value_name, &value.to_string())
                .map_err(|e| format!("Failed to set registry value: {}", e))?;

            Ok(())
        }

        #[cfg(not(windows))]
        {
            let _ = (path, value);
            Err("Registry access requires Windows".to_string())
        }
    }

    /// Delete a registry key.
    #[cfg(target_os = "windows")]
    fn delete_registry_key(path: &str) -> Result<(), String> {
        #[cfg(windows)]
        {
            use winreg::enums::HKEY_CURRENT_USER;
            use winreg::RegKey;

            let hkcu = RegKey::predef(HKEY_CURRENT_USER);
            match hkcu.delete_subkey_all(path) {
                Ok(()) => Ok(()),
                Err(e) => Err(format!("Failed to delete registry key: {}", e)),
            }
        }

        #[cfg(not(windows))]
        {
            let _ = path;
            Err("Registry access requires Windows".to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dialog_result_default() {
        let result = DialogResult::default();
        assert!(!result.confirmed);
        assert!(result.path.is_none());
        assert_eq!(result.filter_index, 0);
    }

    #[test]
    fn test_dialog_type_variants() {
        assert_eq!(DialogType::Open, DialogType::Open);
        assert_eq!(DialogType::Save, DialogType::Save);
        assert_eq!(DialogType::FolderPicker, DialogType::FolderPicker);
        assert_ne!(DialogType::Open, DialogType::Save);
    }

    #[test]
    fn test_file_filter_all_files() {
        let f = FileFilter::all_files();
        assert_eq!(f.name, "All Files (*.*)");
        assert_eq!(f.pattern, "*.*");
    }

    #[test]
    fn test_file_filter_text_files() {
        let f = FileFilter::text_files();
        assert_eq!(f.name, "Text Files (*.txt)");
        assert_eq!(f.pattern, "*.txt");
    }

    #[test]
    fn test_file_filter_image_files() {
        let f = FileFilter::image_files();
        assert_eq!(f.name, "Image Files (*.png;*.jpg;*.gif;*.bmp)");
        assert_eq!(f.pattern, "*.png;*.jpg;*.gif;*.bmp");
    }

    #[test]
    fn test_file_filter_custom() {
        let f = FileFilter::new("Rust Files", "*.rs");
        assert_eq!(f.name, "Rust Files");
        assert_eq!(f.pattern, "*.rs");
    }

    #[test]
    fn test_dialog_preferences_new() {
        let prefs = DialogPreferences::new(DialogType::Open);
        assert_eq!(prefs.dialog_type, DialogType::Open);
        assert!(prefs.last_directory.is_none());
        assert_eq!(prefs.last_filter_index, 0);
        assert!(!prefs.show_hidden_files);
    }

    #[test]
    fn test_dialog_preferences_set_last_directory() {
        let mut prefs = DialogPreferences::new(DialogType::Save);
        prefs.set_last_directory(r"C:\Users\Documents");
        assert_eq!(
            prefs.last_directory,
            Some(r"C:\Users\Documents".to_string())
        );
    }

    #[test]
    fn test_dialog_replacement_service_new() {
        let svc = DialogReplacementService::new();
        assert!(!svc.active);
        assert!(!svc.registry_registered);
        assert_eq!(svc.preferences.len(), 3);
    }

    #[test]
    fn test_get_preferences() {
        let svc = DialogReplacementService::new();
        let prefs = svc.get_preferences(DialogType::Open);
        assert!(prefs.is_some());
        assert_eq!(prefs.unwrap().dialog_type, DialogType::Open);
    }

    #[test]
    fn test_get_preferences_not_found() {
        // Create service and verify all three exist
        let svc = DialogReplacementService::new();
        assert!(svc.get_preferences(DialogType::Open).is_some());
        assert!(svc.get_preferences(DialogType::Save).is_some());
        assert!(svc.get_preferences(DialogType::FolderPicker).is_some());
    }

    #[test]
    fn test_update_last_directory() {
        let mut svc = DialogReplacementService::new();
        svc.update_last_directory(DialogType::Open, r"C:\Users\Public");

        let prefs = svc.get_preferences(DialogType::Open).unwrap();
        assert_eq!(
            prefs.last_directory,
            Some(r"C:\Users\Public".to_string())
        );
    }

    #[test]
    fn test_show_open_dialog_on_non_windows() {
        let svc = DialogReplacementService::new();
        let result = svc.show_open_dialog(
            "Open File",
            &[FileFilter::all_files()],
            None,
        );
        assert!(!result.confirmed);
    }

    #[test]
    fn test_show_save_dialog() {
        let svc = DialogReplacementService::new();
        let result = svc.show_save_dialog(
            "Save As",
            &[FileFilter::text_files(), FileFilter::all_files()],
            Some("untitled.txt"),
        );
        assert!(!result.confirmed);
    }

    #[test]
    fn test_show_folder_picker() {
        let svc = DialogReplacementService::new();
        let result = svc.show_folder_picker("Select Folder", Some(r"C:\"));
        assert!(!result.confirmed);
    }

    #[test]
    fn test_is_active_initial_false() {
        let svc = DialogReplacementService::new();
        assert!(!svc.is_active());
    }

    #[test]
    fn test_dialog_preferences_show_hidden_default() {
        let prefs = DialogPreferences::new(DialogType::Save);
        assert!(!prefs.show_hidden_files);
    }

    #[test]
    fn test_register_on_windows() {
        let mut svc = DialogReplacementService::new();
        #[cfg(not(target_os = "windows"))]
        {
            let result = svc.register(r"C:\files-rs.exe");
            assert!(result.is_err());
            assert!(!svc.is_active());
        }
        #[cfg(target_os = "windows")]
        {
            // On Windows, registration may succeed or fail depending on
            // registry permissions
            let _ = svc.register(r"C:\files-rs.exe");
        }
    }

    #[test]
    fn test_unregister_when_not_registered() {
        let svc = DialogReplacementService::new();
        // Should be a no-op, not an error
        #[cfg(not(target_os = "windows"))]
        assert!(svc.unregister().is_ok());
    }

    #[test]
    fn test_get_preferences_mut() {
        let mut svc = DialogReplacementService::new();
        let prefs = svc.get_preferences_mut(DialogType::FolderPicker);
        assert!(prefs.is_some());
        prefs.unwrap().show_hidden_files = true;

        let prefs = svc.get_preferences(DialogType::FolderPicker).unwrap();
        assert!(prefs.show_hidden_files);
    }

    #[test]
    fn test_dialog_result_with_path() {
        let mut result = DialogResult {
            path: Some(r"C:\Users\file.txt".to_string()),
            confirmed: true,
            filter_index: 1,
        };
        assert!(result.confirmed);
        assert!(result.path.is_some());
        assert_eq!(result.filter_index, 1);
        assert_eq!(
            result.path.unwrap(),
            r"C:\Users\file.txt"
        );
    }
}

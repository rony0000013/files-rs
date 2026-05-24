//! Custom Properties Dialog Service
//! State machine for CustomPropertiesDialog with tabs for different property categories.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Tab pages in the custom properties dialog
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PropertiesTab {
    General,
    Security,
    Customization,
    Hashes,
    Compatibility,
    Signatures,
}

impl Default for PropertiesTab {
    fn default() -> Self {
        Self::General
    }
}

impl PropertiesTab {
    /// Get all available tabs
    pub fn all() -> &'static [PropertiesTab] {
        &[
            PropertiesTab::General,
            PropertiesTab::Security,
            PropertiesTab::Customization,
            PropertiesTab::Hashes,
            PropertiesTab::Compatibility,
            PropertiesTab::Signatures,
        ]
    }

    /// Get the display name for this tab
    pub fn display_name(&self) -> &'static str {
        match self {
            PropertiesTab::General => "General",
            PropertiesTab::Security => "Security",
            PropertiesTab::Customization => "Customization",
            PropertiesTab::Hashes => "Hashes",
            PropertiesTab::Compatibility => "Compatibility",
            PropertiesTab::Signatures => "Signatures",
        }
    }
}

/// File permission / security information
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SecurityInfo {
    pub owner: Option<String>,
    pub can_read: bool,
    pub can_write: bool,
    pub can_delete: bool,
    pub is_readonly: bool,
    pub is_system: bool,
    pub is_hidden: bool,
    pub permissions_summary: String,
}

impl Default for SecurityInfo {
    fn default() -> Self {
        Self {
            owner: None,
            can_read: true,
            can_write: true,
            can_delete: true,
            is_readonly: false,
            is_system: false,
            is_hidden: false,
            permissions_summary: "Full control".to_string(),
        }
    }
}

/// Customization info (desktop.ini settings)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CustomizationInfo {
    pub folder_icon_index: Option<i32>,
    pub folder_icon_path: Option<String>,
    pub folder_background_image: Option<String>,
    pub folder_background_opacity: u8,
    pub folder_type: Option<String>,
    pub apply_to_subfolders: bool,
}

impl Default for CustomizationInfo {
    fn default() -> Self {
        Self {
            folder_icon_index: None,
            folder_icon_path: None,
            folder_background_image: None,
            folder_background_opacity: 50,
            folder_type: None,
            apply_to_subfolders: false,
        }
    }
}

/// Hash information for a file
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HashInfo {
    pub md5: Option<String>,
    pub sha1: Option<String>,
    pub sha256: Option<String>,
    pub sha384: Option<String>,
    pub sha512: Option<String>,
    pub crc32: Option<String>,
}

impl Default for HashInfo {
    fn default() -> Self {
        Self {
            md5: None,
            sha1: None,
            sha256: None,
            sha384: None,
            sha512: None,
            crc32: None,
        }
    }
}

impl HashInfo {
    /// Check if any hash has been computed
    pub fn has_any_hash(&self) -> bool {
        self.md5.is_some()
            || self.sha1.is_some()
            || self.sha256.is_some()
            || self.sha384.is_some()
            || self.sha512.is_some()
            || self.crc32.is_some()
    }
}

/// Compatibility settings for a file
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompatibilityInfo {
    pub run_as_admin: bool,
    pub color_mode: Option<String>, // "16-bit", "256-color", "True Color"
    pub disable_theming: bool,
    pub disable_visual_features: bool,
    pub run_in_sandbox: bool,
    pub emulation_mode: Option<String>, // "Windows 95", "Windows 98", "Windows XP", etc.
    pub dpi_settings: Option<String>,   // "Application", "System", "System (Enhanced)"
}

impl Default for CompatibilityInfo {
    fn default() -> Self {
        Self {
            run_as_admin: false,
            color_mode: None,
            disable_theming: false,
            disable_visual_features: false,
            run_in_sandbox: false,
            emulation_mode: None,
            dpi_settings: None,
        }
    }
}

/// Authenticode / signature information
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SignatureInfo {
    pub is_signed: bool,
    pub is_valid: bool,
    pub signer_name: Option<String>,
    pub issuer_name: Option<String>,
    pub valid_from: Option<String>,
    pub valid_to: Option<String>,
    pub serial_number: Option<String>,
    pub thumbprint: Option<String>,
}

impl Default for SignatureInfo {
    fn default() -> Self {
        Self {
            is_signed: false,
            is_valid: false,
            signer_name: None,
            issuer_name: None,
            valid_from: None,
            valid_to: None,
            serial_number: None,
            thumbprint: None,
        }
    }
}

/// Main state for CustomPropertiesDialog
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CustomPropertiesState {
    /// Target file/folder path
    pub path: PathBuf,
    /// Currently active tab
    pub active_tab: PropertiesTab,
    /// File name
    pub name: String,
    /// File type description
    pub file_type: String,
    /// File size
    pub size: u64,
    /// Created date
    pub created: Option<String>,
    /// Modified date
    pub modified: Option<String>,
    /// Accessed date
    pub accessed: Option<String>,
    /// Attributes
    pub is_readonly: bool,
    pub is_hidden: bool,
    pub is_system: bool,
    pub is_archive: bool,
    pub is_compressed: bool,
    pub is_encrypted: bool,
    /// Tab-specific data
    pub security: SecurityInfo,
    pub customization: CustomizationInfo,
    pub hashes: HashInfo,
    pub compatibility: CompatibilityInfo,
    pub signature: SignatureInfo,
    /// Loading states for async operations
    pub loading_hashes: bool,
    pub loading_signature: bool,
    /// Error message
    pub error: Option<String>,
}

impl Default for CustomPropertiesState {
    fn default() -> Self {
        Self {
            path: PathBuf::new(),
            active_tab: PropertiesTab::default(),
            name: String::new(),
            file_type: String::new(),
            size: 0,
            created: None,
            modified: None,
            accessed: None,
            is_readonly: false,
            is_hidden: false,
            is_system: false,
            is_archive: false,
            is_compressed: false,
            is_encrypted: false,
            security: SecurityInfo::default(),
            customization: CustomizationInfo::default(),
            hashes: HashInfo::default(),
            compatibility: CompatibilityInfo::default(),
            signature: SignatureInfo::default(),
            loading_hashes: false,
            loading_signature: false,
            error: None,
        }
    }
}

impl CustomPropertiesState {
    /// Create a new properties state for a path
    pub fn new(path: PathBuf) -> Self {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        Self {
            path,
            name,
            ..Default::default()
        }
    }

    /// Set the active tab
    pub fn set_tab(&mut self, tab: PropertiesTab) {
        self.active_tab = tab;
    }

    /// Navigate to next tab
    pub fn next_tab(&mut self) {
        let tabs = PropertiesTab::all();
        if let Some(pos) = tabs.iter().position(|&t| t == self.active_tab) {
            if pos + 1 < tabs.len() {
                self.active_tab = tabs[pos + 1];
            }
        }
    }

    /// Navigate to previous tab
    pub fn prev_tab(&mut self) {
        let tabs = PropertiesTab::all();
        if let Some(pos) = tabs.iter().position(|&t| t == self.active_tab) {
            if pos > 0 {
                self.active_tab = tabs[pos - 1];
            }
        }
    }

    /// Set hash information
    pub fn set_hashes(&mut self, hashes: HashInfo) {
        self.hashes = hashes;
        self.loading_hashes = false;
    }

    /// Set signature information
    pub fn set_signature(&mut self, sig: SignatureInfo) {
        self.signature = sig;
        self.loading_signature = false;
    }

    /// Set security information
    pub fn set_security(&mut self, security: SecurityInfo) {
        self.security = security;
    }

    /// Set customization information
    pub fn set_customization(&mut self, customization: CustomizationInfo) {
        self.customization = customization;
    }

    /// Set compatibility information
    pub fn set_compatibility(&mut self, compatibility: CompatibilityInfo) {
        self.compatibility = compatibility;
    }

    /// Toggle loading state for hashes
    pub fn set_loading_hashes(&mut self, loading: bool) {
        self.loading_hashes = loading;
    }

    /// Toggle loading state for signature
    pub fn set_loading_signature(&mut self, loading: bool) {
        self.loading_signature = loading;
    }

    /// Get human-readable file size
    pub fn formatted_size(&self) -> String {
        bytesize::ByteSize(self.size).to_string()
    }

    /// Get attribute summary string
    pub fn attribute_summary(&self) -> String {
        let mut attrs = Vec::new();
        if self.is_readonly {
            attrs.push("Read-only");
        }
        if self.is_hidden {
            attrs.push("Hidden");
        }
        if self.is_system {
            attrs.push("System");
        }
        if self.is_archive {
            attrs.push("Archive");
        }
        if self.is_compressed {
            attrs.push("Compressed");
        }
        if self.is_encrypted {
            attrs.push("Encrypted");
        }
        if attrs.is_empty() {
            "Normal".to_string()
        } else {
            attrs.join(", ")
        }
    }

    /// Validate that a path is set
    pub fn validate(&self) -> Result<(), String> {
        if self.path.as_os_str().is_empty() {
            return Err("No file specified".to_string());
        }
        if !self.path.exists() {
            return Err(format!("File does not exist: {}", self.path.display()));
        }
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_properties_tab_default() {
        assert_eq!(PropertiesTab::default(), PropertiesTab::General);
    }

    #[test]
    fn test_properties_tab_all() {
        let tabs = PropertiesTab::all();
        assert_eq!(tabs.len(), 6);
    }

    #[test]
    fn test_properties_tab_display_name() {
        assert_eq!(PropertiesTab::General.display_name(), "General");
        assert_eq!(PropertiesTab::Security.display_name(), "Security");
        assert_eq!(PropertiesTab::Customization.display_name(), "Customization");
        assert_eq!(PropertiesTab::Hashes.display_name(), "Hashes");
        assert_eq!(PropertiesTab::Compatibility.display_name(), "Compatibility");
        assert_eq!(PropertiesTab::Signatures.display_name(), "Signatures");
    }

    #[test]
    fn test_security_info_default() {
        let info = SecurityInfo::default();
        assert!(info.owner.is_none());
        assert!(info.can_read);
        assert!(info.can_write);
        assert!(info.can_delete);
        assert!(!info.is_readonly);
        assert!(!info.is_system);
        assert!(!info.is_hidden);
        assert_eq!(info.permissions_summary, "Full control");
    }

    #[test]
    fn test_customization_info_default() {
        let info = CustomizationInfo::default();
        assert!(info.folder_icon_index.is_none());
        assert!(info.folder_icon_path.is_none());
        assert!(info.folder_background_image.is_none());
        assert_eq!(info.folder_background_opacity, 50);
        assert!(info.folder_type.is_none());
        assert!(!info.apply_to_subfolders);
    }

    #[test]
    fn test_hash_info_default() {
        let info = HashInfo::default();
        assert!(info.md5.is_none());
        assert!(info.sha1.is_none());
        assert!(info.sha256.is_none());
        assert!(info.sha384.is_none());
        assert!(info.sha512.is_none());
        assert!(info.crc32.is_none());
    }

    #[test]
    fn test_hash_info_has_any_hash() {
        let mut info = HashInfo::default();
        assert!(!info.has_any_hash());

        info.md5 = Some("abc123".to_string());
        assert!(info.has_any_hash());

        info.sha256 = Some("def456".to_string());
        assert!(info.has_any_hash());
    }

    #[test]
    fn test_compatibility_info_default() {
        let info = CompatibilityInfo::default();
        assert!(!info.run_as_admin);
        assert!(info.color_mode.is_none());
        assert!(!info.disable_theming);
        assert!(!info.disable_visual_features);
        assert!(!info.run_in_sandbox);
        assert!(info.emulation_mode.is_none());
        assert!(info.dpi_settings.is_none());
    }

    #[test]
    fn test_signature_info_default() {
        let info = SignatureInfo::default();
        assert!(!info.is_signed);
        assert!(!info.is_valid);
        assert!(info.signer_name.is_none());
        assert!(info.issuer_name.is_none());
    }

    #[test]
    fn test_custom_properties_state_default() {
        let state = CustomPropertiesState::default();
        assert!(state.path.as_os_str().is_empty());
        assert_eq!(state.active_tab, PropertiesTab::General);
        assert!(state.name.is_empty());
        assert_eq!(state.size, 0);
        assert!(!state.loading_hashes);
        assert!(!state.loading_signature);
        assert!(state.error.is_none());
    }

    #[test]
    fn test_custom_properties_state_new() {
        let path = PathBuf::from("/test/file.txt");
        let state = CustomPropertiesState::new(path.clone());
        assert_eq!(state.path, path);
        assert_eq!(state.name, "file.txt");
    }

    #[test]
    fn test_custom_properties_state_set_tab() {
        let mut state = CustomPropertiesState::default();
        state.set_tab(PropertiesTab::Hashes);
        assert_eq!(state.active_tab, PropertiesTab::Hashes);
    }

    #[test]
    fn test_custom_properties_state_next_tab() {
        let mut state = CustomPropertiesState::default();
        assert_eq!(state.active_tab, PropertiesTab::General);

        state.next_tab();
        assert_eq!(state.active_tab, PropertiesTab::Security);

        state.next_tab();
        assert_eq!(state.active_tab, PropertiesTab::Customization);

        // Go to last tab
        state.set_tab(PropertiesTab::Signatures);
        state.next_tab();
        assert_eq!(state.active_tab, PropertiesTab::Signatures); // No change at end
    }

    #[test]
    fn test_custom_properties_state_prev_tab() {
        let mut state = CustomPropertiesState::default();
        state.set_tab(PropertiesTab::Security);

        state.prev_tab();
        assert_eq!(state.active_tab, PropertiesTab::General);

        state.prev_tab();
        assert_eq!(state.active_tab, PropertiesTab::General); // No change at start
    }

    #[test]
    fn test_custom_properties_state_set_hashes() {
        let mut state = CustomPropertiesState::default();
        state.set_loading_hashes(true);
        assert!(state.loading_hashes);

        let hashes = HashInfo {
            md5: Some("abc123".to_string()),
            sha256: Some("def456".to_string()),
            ..Default::default()
        };
        state.set_hashes(hashes);
        assert!(!state.loading_hashes);
        assert_eq!(state.hashes.md5, Some("abc123".to_string()));
    }

    #[test]
    fn test_custom_properties_state_formatted_size() {
        let mut state = CustomPropertiesState::default();
        state.size = 1024;
        assert_eq!(state.formatted_size(), "1.0 KiB");

        state.size = 1024 * 1024;
        assert_eq!(state.formatted_size(), "1.0 MiB");

        state.size = 1024 * 1024 * 1024;
        assert_eq!(state.formatted_size(), "1.0 GiB");
    }

    #[test]
    fn test_custom_properties_state_attribute_summary() {
        let mut state = CustomPropertiesState::default();

        assert_eq!(state.attribute_summary(), "Normal");

        state.is_readonly = true;
        state.is_hidden = true;
        assert_eq!(state.attribute_summary(), "Read-only, Hidden");

        state.is_compressed = true;
        state.is_encrypted = true;
        assert!(state.attribute_summary().contains("Compressed"));
        assert!(state.attribute_summary().contains("Encrypted"));
    }

    #[test]
    fn test_custom_properties_state_validate() {
        let state = CustomPropertiesState::default();
        let result = state.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("No file specified"));
    }
}

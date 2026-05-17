use crate::core::path_util::FilesPath;
use crate::core::terminal_service::TerminalType;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

const CURRENT_CONFIG_VERSION: u32 = 1;

fn default_version() -> u32 {
    CURRENT_CONFIG_VERSION
}

/// Core application settings, persisted to disk.
/// Mirrors the functionality of Files.App settings storage.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct AppConfig {
    #[serde(default = "default_version")]
    pub version: u32,
    /// User preferences for the file manager
    pub preferences: UserPreferences,
    /// Sidebar configuration (pinned locations, etc.)
    pub sidebar: SidebarConfig,
    /// View preferences (default view mode, sorting, etc.)
    pub view: ViewConfig,
    /// Per-folder layout preferences
    pub folder_preferences: HashMap<String, FolderPreferences>,
    /// Advanced/Experimental features
    pub experimental: ExperimentalConfig,
    /// Current session state (tabs, active tab, etc.)
    #[serde(default)]
    pub session: SessionConfig,
    /// Window size as (width, height) in pixels
    #[serde(default)]
    pub window_size: Option<(f32, f32)>,
    /// Whether the window was maximized
    #[serde(default)]
    pub window_maximized: bool,
    /// Window position as (x, y) in pixels
    #[serde(default)]
    pub window_position: Option<(f32, f32)>,
    /// Appearance preferences (theme, etc.)
    #[serde(default)]
    pub appearance: AppearanceConfig,
    /// Default terminal application preference
    #[serde(default)]
    pub terminal_app: TerminalType,
    /// Keyboard shortcut override bindings
    #[serde(default)]
    pub action_bindings: Option<crate::core::action_registry::ActionRegistry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppearanceConfig {
    pub theme: String,
    pub mica_enabled: bool,
    #[serde(default = "default_font_family")]
    pub font_family: String,
    #[serde(default = "default_font_size")]
    pub font_size: f64,
    #[serde(default)]
    pub mica_alt: bool,
}

fn default_font_family() -> String {
    "Segoe UI Variable Text".to_string()
}

fn default_font_size() -> f64 {
    13.0
}

impl Default for AppearanceConfig {
    fn default() -> Self {
        Self {
            theme: "system".to_string(),
            mica_enabled: true,
            font_family: default_font_family(),
            font_size: default_font_size(),
            mica_alt: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPreferences {
    pub show_hidden_files: bool,
    pub show_dot_files: bool,
    pub show_file_extensions: bool,
    pub show_system_files: bool,
    pub confirm_delete: bool,
    pub use_trash: bool, // Send deleted files to recycle bin
    pub locale: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[allow(dead_code)]
pub struct SidebarConfig {
    pub pinned_locations: Vec<String>,
    /// Persisted sidebar width in pixels (None = use default)
    #[serde(default)]
    pub sidebar_width: Option<f32>,
    /// Persisted section display order (list of SectionId names, e.g. "Favorites", "Libraries", ...)
    /// None = use default order from build_all_sections()
    #[serde(default)]
    pub section_order: Option<Vec<String>>,
    /// List of SectionId names that are hidden
    #[serde(default)]
    pub hidden_sections: Option<Vec<String>>,
    /// Persisted collapsed state of sections (SectionId -> collapsed bool)
    #[serde(default)]
    pub section_collapsed: Option<std::collections::HashMap<String, bool>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct ViewConfig {
    pub default_view_mode: ViewMode,
    pub sort_by: SortBy,
    pub sort_ascending: bool,
    pub folder_on_top: bool,
    pub group_by: GroupColumn,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "lowercase")]
#[allow(dead_code)]
pub enum ViewMode {
    #[default]
    List,
    Grid,
    Details,
    Columns,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
#[allow(dead_code)]
pub enum SortBy {
    #[default]
    Name,
    DateModified,
    DateCreated,
    Size,
    Type,
    /// Sort by first tag alphabetically, untagged items sort last
    Tag,
}

/// Group column options for organizing files into collapsible groups
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
#[allow(dead_code)]
pub enum GroupColumn {
    /// No grouping (default)
    #[default]
    None,
    /// Group by first letter of name (A, B, C, ...)
    Name,
    /// Group by date modified (Today, Yesterday, This Week, Earlier...)
    DateModified,
    /// Group by date created
    DateCreated,
    /// Group by size ranges (Tiny, Small, Medium, Large, Huge)
    Size,
    /// Group by file type (Documents, Images, Videos, Music, etc.)
    FileType,
    /// Group by sync status
    SyncStatus,
    /// Group by first tag alphabetically, untagged items in "Untagged" group
    Tag,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FolderPreferences {
    pub view_mode: ViewMode,
    pub zoom_level: f32,
    pub sort_by: SortBy,
    pub sort_ascending: bool,
    pub folder_on_top: bool,
    pub group_by: GroupColumn,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[allow(dead_code)]
pub struct ExperimentalConfig {
    pub enable_preview_pane: bool,
    pub enable_tabs: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SessionConfig {
    pub open_tabs: Vec<TabSession>,
    pub active_tab_index: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TabSession {
    pub path: String,
    pub view_mode: ViewMode,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            version: default_version(),
            preferences: UserPreferences {
                show_hidden_files: false,
                show_dot_files: false,
                show_file_extensions: true,
                show_system_files: false,
                confirm_delete: true,
                use_trash: true,
                locale: "en".to_string(),
            },
            sidebar: SidebarConfig::default(),
            view: ViewConfig {
                default_view_mode: ViewMode::List,
                sort_by: SortBy::Name,
                sort_ascending: true,
                folder_on_top: true,
                group_by: GroupColumn::None,
            },
            folder_preferences: HashMap::new(),
            experimental: ExperimentalConfig::default(),
            session: SessionConfig::default(),
            appearance: AppearanceConfig::default(),
            terminal_app: TerminalType::default(),
            action_bindings: None,
            window_size: Some((1280.0, 800.0)),
            window_maximized: true,
            window_position: None,
        }
    }
}

#[allow(dead_code)]
impl AppConfig {
    /// Returns the path to the config file in the user's config directory.
    fn config_path() -> PathBuf {
        // Use the `directories` crate to get the config folder
        if let Some(proj_dirs) = directories::ProjectDirs::from("com", "files-rs", "Files-Rust") {
            let config_dir = proj_dirs.config_dir();
            config_dir.join("settings.json")
        } else {
            // Fallback to current directory if we can't determine home directory
            PathBuf::from("settings.json")
        }
    }

    /// Returns the layout preferences for a specific folder.
    pub fn get_folder_preferences(&self, path: &FilesPath) -> FolderPreferences {
        let key = path.to_string_lossy().into_owned();
        self.folder_preferences
            .get(&key)
            .cloned()
            .unwrap_or_else(|| FolderPreferences {
                view_mode: self.view.default_view_mode.clone(),
                zoom_level: 0.5,
                sort_by: self.view.sort_by.clone(),
                sort_ascending: self.view.sort_ascending,
                folder_on_top: self.view.folder_on_top,
                group_by: self.view.group_by,
            })
    }

    /// Save layout preferences for a specific folder.
    pub fn set_folder_preferences(&mut self, path: &FilesPath, prefs: FolderPreferences) {
        let key = path.to_string_lossy().into_owned();
        self.folder_preferences.insert(key, prefs);
    }

    /// Load settings from disk, or return default if not found.
    pub fn load() -> Self {
        let path = Self::config_path();
        if path.exists() {
            if let Ok(contents) = fs::read_to_string(&path) {
                if let Ok(mut value) = serde_json::from_str::<serde_json::Value>(&contents) {
                    let version = value.get("version").and_then(|v| v.as_u64()).unwrap_or(0) as u32;

                    if version < CURRENT_CONFIG_VERSION {
                        tracing::info!(
                            "Migrating config from version {} to {}",
                            version,
                            CURRENT_CONFIG_VERSION
                        );
                        Self::migrate(&mut value, version);
                    }

                    if let Ok(config) = serde_json::from_value(value) {
                        tracing::info!("Loaded settings from {:?}", path);
                        return config;
                    } else {
                        tracing::warn!("Failed to parse migrated settings. Using defaults.");
                    }
                } else {
                    tracing::warn!("Failed to read settings file. Using defaults.");
                }
            }
        }

        tracing::info!("Using default settings.");
        Self::default()
    }

    /// Perform migrations for different versions.
    fn migrate(value: &mut serde_json::Value, from_version: u32) {
        if from_version < 1 {
            // Migration from version 0 to 1:
            // Currently, adding folder_preferences is handled by serde(default),
            // but we'll explicitly set the version here.
            if let Some(obj) = value.as_object_mut() {
                obj.insert("version".to_string(), serde_json::json!(1));
            }
        }
        // Future migrations will go here
    }

    /// Save current settings to disk.
    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let path = Self::config_path();

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let json = serde_json::to_string_pretty(self)?;
        fs::write(&path, json)?;

        tracing::info!("Settings saved to {:?}", path);
        Ok(())
    }
}

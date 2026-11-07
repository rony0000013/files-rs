use crate::core::path_util::FilesPath;
use crate::core::terminal_service::TerminalType;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

const CURRENT_CONFIG_VERSION: u32 = 2;

fn default_version() -> u32 {
    CURRENT_CONFIG_VERSION
}

fn default_true() -> bool {
    true
}

fn default_middle_click_folder_action() -> String {
    "new_tab".to_string()
}

fn default_middle_click_tab_action() -> String {
    "close_tab".to_string()
}

fn default_ctrl_click_folder_action() -> String {
    "select".to_string()
}

fn default_double_click_empty_action() -> String {
    "navigate_up".to_string()
}

fn default_mouse4_action() -> String {
    "navigate_back".to_string()
}

fn default_mouse5_action() -> String {
    "navigate_forward".to_string()
}

fn default_ctrl_wheel_action() -> String {
    "zoom".to_string()
}

fn default_shift_wheel_action() -> String {
    "scroll_horizontal".to_string()
}

/// Action & Mouse combination behaviors for the file view, mirroring `Files-community` action & folder settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ActionsConfig {
    /// When true, double-clicking empty space in the file view navigates up one level.
    #[serde(default = "default_true")]
    pub double_click_empty_space_go_up: bool,
    /// Action when double-clicking empty space: "navigate_up", "select_all", "new_folder", "none"
    #[serde(default = "default_double_click_empty_action")]
    pub double_click_empty_action: String,
    /// Open files with a single click instead of double-click
    pub open_files_with_single_click: bool,
    /// Open folders with a single click instead of double-click
    pub open_folders_with_single_click: bool,
    /// Navigate folders in Columns view with a single click (default true in Miller columns)
    #[serde(default = "default_true")]
    pub open_folders_in_columns_with_single_click: bool,
    /// Always open folders in a new tab on double-click
    pub open_folders_in_new_tab: bool,
    /// Select files and folders when hovering over them
    pub select_files_on_hover: bool,
    /// Scroll to previous folder when navigating up
    #[serde(default = "default_true")]
    pub scroll_to_previous_folder_when_navigating_up: bool,
    /// Middle-click on folder action: "new_tab", "new_pane", "new_window", "none"
    #[serde(default = "default_middle_click_folder_action")]
    pub middle_click_folder_action: String,
    /// Middle-click on tab action: "close_tab", "duplicate_tab", "pin_tab", "none"
    #[serde(default = "default_middle_click_tab_action")]
    pub middle_click_tab_action: String,
    /// Ctrl + Left-Click on folder action: "select" (toggle selection) or "new_tab"
    #[serde(default = "default_ctrl_click_folder_action")]
    pub ctrl_click_folder_action: String,
    /// Mouse 4 (XButton1 / Back button) action: "navigate_back", "navigate_up", "previous_tab", "none"
    #[serde(default = "default_mouse4_action")]
    pub mouse4_action: String,
    /// Mouse 5 (XButton2 / Forward button) action: "navigate_forward", "next_tab", "none"
    #[serde(default = "default_mouse5_action")]
    pub mouse5_action: String,
    /// Ctrl + Mouse Wheel action: "zoom", "cycle_view", "none"
    #[serde(default = "default_ctrl_wheel_action")]
    pub ctrl_wheel_action: String,
    /// Shift + Mouse Wheel action: "scroll_horizontal", "switch_tabs", "none"
    #[serde(default = "default_shift_wheel_action")]
    pub shift_wheel_action: String,
}

impl Default for ActionsConfig {
    fn default() -> Self {
        Self {
            double_click_empty_space_go_up: true,
            double_click_empty_action: default_double_click_empty_action(),
            open_files_with_single_click: false,
            open_folders_with_single_click: false,
            open_folders_in_columns_with_single_click: true,
            open_folders_in_new_tab: false,
            select_files_on_hover: false,
            scroll_to_previous_folder_when_navigating_up: true,
            middle_click_folder_action: default_middle_click_folder_action(),
            middle_click_tab_action: default_middle_click_tab_action(),
            ctrl_click_folder_action: default_ctrl_click_folder_action(),
            mouse4_action: default_mouse4_action(),
            mouse5_action: default_mouse5_action(),
            ctrl_wheel_action: default_ctrl_wheel_action(),
            shift_wheel_action: default_shift_wheel_action(),
        }
    }
}

/// Home Page Widgets visibility & expansion settings (`Files-community` `HomePage.xaml` / `GeneralPage.xaml`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct HomeWidgetsConfig {
    #[serde(default = "default_true")]
    pub show_quick_access: bool,
    #[serde(default = "default_true")]
    pub show_drives: bool,
    #[serde(default = "default_true")]
    pub show_network_locations: bool,
    #[serde(default = "default_true")]
    pub show_file_tags: bool,
    #[serde(default = "default_true")]
    pub show_recent_files: bool,
    #[serde(default = "default_true")]
    pub quick_access_expanded: bool,
    #[serde(default = "default_true")]
    pub drives_expanded: bool,
    #[serde(default = "default_true")]
    pub network_locations_expanded: bool,
    #[serde(default = "default_true")]
    pub file_tags_expanded: bool,
    #[serde(default = "default_true")]
    pub recent_files_expanded: bool,
}

impl Default for HomeWidgetsConfig {
    fn default() -> Self {
        Self {
            show_quick_access: true,
            show_drives: true,
            show_network_locations: true,
            show_file_tags: true,
            show_recent_files: true,
            quick_access_expanded: true,
            drives_expanded: true,
            network_locations_expanded: true,
            file_tags_expanded: true,
            recent_files_expanded: true,
        }
    }
}

/// Custom user-defined file tag definition (`Files-community` `TagsPage.xaml`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TagDefinition {
    /// Stable identifier used by the tag manager (`tag_N`).
    #[serde(default)]
    pub id: String,
    pub name: String,
    pub color: String,
}

impl Default for TagDefinition {
    fn default() -> Self {
        Self {
            id: "tag_default".to_string(),
            name: "Important".to_string(),
            color: "#0078D4".to_string(),
        }
    }
}

pub fn default_tag_definitions() -> Vec<TagDefinition> {
    vec![
        TagDefinition {
            id: "tag_1".to_string(),
            name: "Work".to_string(),
            color: "#E81123".to_string(),
        },
        TagDefinition {
            id: "tag_2".to_string(),
            name: "Personal".to_string(),
            color: "#107C10".to_string(),
        },
        TagDefinition {
            id: "tag_3".to_string(),
            name: "Important".to_string(),
            color: "#0078D4".to_string(),
        },
        TagDefinition {
            id: "tag_4".to_string(),
            name: "Documents".to_string(),
            color: "#FFB900".to_string(),
        },
        TagDefinition {
            id: "tag_5".to_string(),
            name: "Media".to_string(),
            color: "#881798".to_string(),
        },
        TagDefinition {
            id: "tag_6".to_string(),
            name: "Archive".to_string(),
            color: "#00B7C3".to_string(),
        },
    ]
}

/// DevTools & IDE integration settings (`Files-community` `DevToolsPage.xaml`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DevToolsConfig {
    /// When to display the Open in IDE button in Status Bar: "always", "git_only", "never"
    pub open_in_ide_visibility: String,
    /// Configured IDE display name (e.g., "VS Code", "Zed", "Cursor", "RustRover")
    pub ide_name: String,
    /// Configured IDE executable path or CLI alias (e.g., "code", "zed", "cursor")
    pub ide_path: String,
    /// GitHub connected account name (if connected)
    pub github_account: Option<String>,
    /// Show live FPS & frame-time telemetry badge
    pub show_fps_telemetry: bool,
}

impl Default for DevToolsConfig {
    fn default() -> Self {
        Self {
            open_in_ide_visibility: "git_only".to_string(),
            ide_name: "Visual Studio Code".to_string(),
            ide_path: "code".to_string(),
            github_account: None,
            show_fps_telemetry: false,
        }
    }
}

/// Serialized custom key/mouse binding overrides, keyed by `Debug` action name.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct KeyBindings {
    /// `{"ActionKind::Copy": "ctrl+c", ...}` — empty map means all defaults.
    #[serde(default)]
    pub bindings: std::collections::HashMap<String, String>,
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
    /// Appearance preferences (theme, backdrop, fonts, chrome visibility)
    #[serde(default)]
    pub appearance: AppearanceConfig,
    /// Default terminal application preference
    #[serde(default)]
    pub terminal_app: TerminalType,
    /// Action & mouse combination behaviors for the file view
    #[serde(default)]
    pub actions: ActionsConfig,
    /// Keyboard & mouse shortcut override bindings
    #[serde(default)]
    pub action_bindings: Option<crate::core::action_registry::ActionRegistry>,
    /// Serialized custom key/mouse binding overrides, keyed by action name.
    #[serde(default)]
    pub keybindings: KeyBindings,
    /// Home page widgets visibility and expansion state
    #[serde(default)]
    pub home_widgets: HomeWidgetsConfig,
    /// User-defined file tags with colors
    #[serde(default = "default_tag_definitions")]
    pub tag_definitions: Vec<TagDefinition>,
    /// DevTools & IDE settings
    #[serde(default)]
    pub devtools: DevToolsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppearanceConfig {
    pub theme: String,
    pub mica_enabled: bool,
    #[serde(default = "default_font_family")]
    pub font_family: String,
    #[serde(default = "default_font_size")]
    pub font_size: f64,
    #[serde(default)]
    pub mica_alt: bool,
    /// Transparent swapchain (vs Opaque).
    #[serde(default)]
    pub transparent_window: bool,
    /// Backdrop material: "mica_alt", "mica", "acrylic", "solid"
    #[serde(default = "default_backdrop_material")]
    pub backdrop_material: String,
    /// App theme preset name ("Default", "Cool Blue", "Emerald", "Amethyst", "Sunset Gold", "Crimson", "Slate")
    #[serde(default = "default_theme_preset")]
    pub theme_preset: String,
    /// Custom background image path
    pub background_image_path: Option<String>,
    /// Background image opacity (0.1 ..= 1.0)
    #[serde(default = "default_bg_opacity")]
    pub background_image_opacity: f32,
    /// Background image stretch fit ("fill", "fit", "stretch", "center")
    #[serde(default = "default_bg_fit")]
    pub background_image_fit: String,
    /// Background image vertical alignment ("top", "center", "bottom")
    #[serde(default = "default_bg_align")]
    pub background_image_v_align: String,
    /// Background image horizontal alignment ("left", "center", "right")
    #[serde(default = "default_bg_align")]
    pub background_image_h_align: String,
    /// Show tab actions button in tab bar
    #[serde(default = "default_true")]
    pub show_tab_actions: bool,
    /// Show Shelf pane toggle button in address bar
    #[serde(default = "default_true")]
    pub show_shelf_button: bool,
    /// Status Center button visibility: "always", "active_only", "never"
    #[serde(default = "default_status_center_visibility")]
    pub status_center_visibility: String,
    /// Show inner command bar ribbon above file view
    #[serde(default = "default_true")]
    pub show_toolbar: bool,
    /// Show bottom status bar
    #[serde(default = "default_true")]
    pub show_status_bar: bool,
    /// Show command labels alongside icons in the top ribbon toolbar
    #[serde(default = "default_true")]
    pub show_toolbar_labels: bool,
}

fn default_font_family() -> String {
    "Segoe UI Variable Text".to_string()
}

fn default_backdrop_material() -> String {
    "mica_alt".to_string()
}

fn default_theme_preset() -> String {
    "Default".to_string()
}

fn default_bg_opacity() -> f32 {
    0.35
}

fn default_bg_fit() -> String {
    "fill".to_string()
}

fn default_bg_align() -> String {
    "center".to_string()
}

fn default_status_center_visibility() -> String {
    "always".to_string()
}

fn default_context_menu_style() -> ContextMenuStyle {
    ContextMenuStyle::Files
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
            mica_alt: true,
            transparent_window: false,
            backdrop_material: default_backdrop_material(),
            theme_preset: default_theme_preset(),
            background_image_path: None,
            background_image_opacity: default_bg_opacity(),
            background_image_fit: default_bg_fit(),
            background_image_v_align: default_bg_align(),
            background_image_h_align: default_bg_align(),
            show_tab_actions: true,
            show_shelf_button: true,
            status_center_visibility: default_status_center_visibility(),
            show_toolbar: true,
            show_status_bar: true,
            show_toolbar_labels: true,
        }
    }
}

/// Right-click menu presentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ContextMenuStyle {
    /// Files-like: compact rows with hover flyouts ("Show more options" ›).
    #[default]
    #[serde(rename = "files")]
    Files,
    /// All classic shell items inlined below the app rows (no hover needed).
    #[serde(rename = "expanded")]
    Expanded,
    /// The native Windows classic menu popup (full shell extensions as-is).
    #[serde(rename = "classic")]
    WindowsClassic,
}

impl ContextMenuStyle {
    pub fn from_str(s: &str) -> Self {
        match s {
            "expanded" => Self::Expanded,
            "classic" => Self::WindowsClassic,
            _ => Self::Files,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Files => "files",
            Self::Expanded => "expanded",
            Self::WindowsClassic => "classic",
        }
    }
}

fn default_date_format() -> String {
    "application".to_string()
}

fn default_delete_confirmation_policy() -> String {
    "always".to_string()
}

fn default_size_unit_format() -> String {
    "binary".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UserPreferences {
    pub show_hidden_files: bool,
    pub show_dot_files: bool,
    pub show_file_extensions: bool,
    pub show_system_files: bool,
    pub show_alternate_data_streams: bool,
    #[serde(default = "default_true")]
    pub show_thumbnails: bool,
    pub show_checkboxes_when_selecting: bool,
    pub confirm_delete: bool,
    /// Delete confirmation policy: "always", "permanent_only", "never"
    #[serde(default = "default_delete_confirmation_policy")]
    pub delete_confirmation_policy: String,
    #[serde(default = "default_true")]
    pub show_file_extension_warning: bool,
    pub use_trash: bool, // Send deleted files to recycle bin
    pub locale: String,
    /// Date format style: "application" (relative), "system" (YYYY-MM-DD HH:MM), "iso8601", "universal"
    #[serde(default = "default_date_format")]
    pub date_format: String,
    /// Size unit format: "binary" (KiB/MiB/GiB), "decimal" (KB/MB/GB), "bytes"
    #[serde(default = "default_size_unit_format")]
    pub size_unit_format: String,
    /// Right-click menu presentation ("files" | "expanded" | "classic").
    #[serde(default = "default_context_menu_style")]
    pub context_menu_style: ContextMenuStyle,
    /// Move shell extensions into an overflow submenu
    #[serde(default = "default_true")]
    pub move_shell_extensions_to_submenu: bool,
    /// Context menu item visibility toggles (Files-community GeneralPage.xaml parity)
    #[serde(default = "default_true")]
    pub ctx_show_open_in_new_tab: bool,
    #[serde(default = "default_true")]
    pub ctx_show_open_in_new_window: bool,
    #[serde(default = "default_true")]
    pub ctx_show_open_in_new_pane: bool,
    #[serde(default = "default_true")]
    pub ctx_show_copy_path: bool,
    #[serde(default = "default_true")]
    pub ctx_show_create_folder_with_selection: bool,
    #[serde(default = "default_true")]
    pub ctx_show_create_shortcut: bool,
    #[serde(default = "default_true")]
    pub ctx_show_pin_to_sidebar: bool,
    #[serde(default = "default_true")]
    pub ctx_show_compression_options: bool,
    #[serde(default = "default_true")]
    pub ctx_show_send_to: bool,
    #[serde(default = "default_true")]
    pub ctx_show_open_terminal: bool,
    #[serde(default = "default_true")]
    pub ctx_show_edit_tags: bool,
    /// Enable smooth spring scrolling in file views and sidebar
    #[serde(default = "default_true")]
    pub enable_smooth_scrolling: bool,
    /// Reverse mouse wheel direction when scrolling tabs
    pub reverse_tab_scroll_direction: bool,
    /// Show informative tooltips with shortcuts when hovering over buttons and commands
    #[serde(default = "default_true")]
    pub show_button_tooltips: bool,
}

impl Default for UserPreferences {
    fn default() -> Self {
        Self {
            show_hidden_files: false,
            show_dot_files: false,
            show_file_extensions: true,
            show_system_files: false,
            show_alternate_data_streams: false,
            show_thumbnails: true,
            show_checkboxes_when_selecting: false,
            confirm_delete: true,
            delete_confirmation_policy: default_delete_confirmation_policy(),
            show_file_extension_warning: true,
            use_trash: true,
            locale: "en".to_string(),
            date_format: default_date_format(),
            size_unit_format: default_size_unit_format(),
            context_menu_style: ContextMenuStyle::Files,
            move_shell_extensions_to_submenu: true,
            ctx_show_open_in_new_tab: true,
            ctx_show_open_in_new_window: true,
            ctx_show_open_in_new_pane: true,
            ctx_show_copy_path: true,
            ctx_show_create_folder_with_selection: true,
            ctx_show_create_shortcut: true,
            ctx_show_pin_to_sidebar: true,
            ctx_show_compression_options: true,
            ctx_show_send_to: true,
            ctx_show_open_terminal: true,
            ctx_show_edit_tags: true,
            enable_smooth_scrolling: true,
            reverse_tab_scroll_direction: false,
            show_button_tooltips: true,
        }
    }
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

fn default_sort_priority() -> String {
    "folders_first".to_string()
}

fn default_group_date_unit() -> String {
    "month".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
#[allow(dead_code)]
pub struct ViewConfig {
    pub default_view_mode: ViewMode,
    pub last_view_mode: ViewMode,
    pub last_zoom_level: f32,
    pub sort_by: SortBy,
    pub sort_ascending: bool,
    pub folder_on_top: bool,
    /// Sort priority: "folders_first", "files_first", "together"
    #[serde(default = "default_sort_priority")]
    pub sort_priority: String,
    pub group_by: GroupColumn,
    #[serde(default = "default_true")]
    pub group_ascending: bool,
    /// Group by date unit: "year", "month", "day"
    #[serde(default = "default_group_date_unit")]
    pub group_by_date_unit: String,
    /// Sync folder layout/sort preferences across all directories
    pub sync_folder_preferences: bool,
    /// Adaptive layout mode (automatically switches between Details and Grid based on content)
    pub adaptive_layout_enabled: bool,
    /// Auto-fit columns in Details layout
    #[serde(default = "default_true")]
    pub auto_size_columns: bool,
    /// Calculate folder sizes in background
    #[serde(default = "default_true")]
    pub calculate_folder_sizes: bool,
    /// Index folder sizes across all local drives once into a persistent hashed cache and update incrementally
    #[serde(default = "default_true")]
    pub index_all_drives_folder_sizes: bool,
    /// Default column visibility in Details view
    #[serde(default = "default_true")]
    pub show_date_column: bool,
    pub show_date_created_column: bool,
    #[serde(default = "default_true")]
    pub show_type_column: bool,
    #[serde(default = "default_true")]
    pub show_size_column: bool,
    #[serde(default = "default_true")]
    pub show_tag_column: bool,
    pub always_open_dual_pane_in_new_tab: bool,
}

impl Default for ViewConfig {
    fn default() -> Self {
        Self {
            default_view_mode: ViewMode::Details,
            last_view_mode: ViewMode::Details,
            last_zoom_level: 0.25,
            sort_by: SortBy::Name,
            sort_ascending: true,
            folder_on_top: true,
            sort_priority: default_sort_priority(),
            group_by: GroupColumn::None,
            group_ascending: true,
            group_by_date_unit: default_group_date_unit(),
            sync_folder_preferences: false,
            adaptive_layout_enabled: false,
            auto_size_columns: true,
            calculate_folder_sizes: true,
            index_all_drives_folder_sizes: true,
            show_date_column: true,
            show_date_created_column: false,
            show_type_column: true,
            show_size_column: true,
            show_tag_column: true,
            always_open_dual_pane_in_new_tab: false,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
#[allow(dead_code)]
pub enum ViewMode {
    #[default]
    List,
    Cards,
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

impl GroupColumn {
    #[allow(non_upper_case_globals)]
    pub const Type: Self = Self::FileType;

    pub fn display_name(&self) -> &'static str {
        match self {
            GroupColumn::None => "None",
            GroupColumn::Name => "Name",
            GroupColumn::DateModified => "Date modified",
            GroupColumn::DateCreated => "Date created",
            GroupColumn::Size => "Size",
            GroupColumn::FileType => "Type",
            GroupColumn::SyncStatus => "Sync status",
            GroupColumn::Tag => "File tag",
        }
    }
}

pub type GroupBy = GroupColumn;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FolderPreferences {
    pub view_mode: ViewMode,
    pub zoom_level: f32,
    pub sort_by: SortBy,
    pub sort_ascending: bool,
    pub folder_on_top: bool,
    pub group_by: GroupColumn,
}

fn default_cache_limit() -> u32 {
    2048
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
#[allow(dead_code)]
pub struct ExperimentalConfig {
    #[serde(default = "default_true")]
    pub enable_preview_pane: bool,
    #[serde(default = "default_true")]
    pub enable_tabs: bool,
    pub open_on_windows_startup: bool,
    pub leave_app_running_in_background: bool,
    pub show_system_tray_icon: bool,
    pub set_as_default_file_manager: bool,
    pub replace_open_file_dialog: bool,
    #[serde(default = "default_true")]
    pub enable_thumbnail_cache: bool,
    #[serde(default = "default_cache_limit")]
    pub thumbnail_cache_size_limit: u32,
    pub show_flatten_options: bool,
}

impl Default for ExperimentalConfig {
    fn default() -> Self {
        Self {
            enable_preview_pane: true,
            enable_tabs: true,
            open_on_windows_startup: false,
            leave_app_running_in_background: false,
            show_system_tray_icon: false,
            set_as_default_file_manager: false,
            replace_open_file_dialog: false,
            enable_thumbnail_cache: true,
            thumbnail_cache_size_limit: default_cache_limit(),
            show_flatten_options: false,
        }
    }
}

fn default_startup_mode() -> String {
    "continue_session".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SessionConfig {
    pub open_tabs: Vec<TabSession>,
    pub active_tab_index: usize,
    /// Pane layout: "single", "horizontal", or "vertical"
    #[serde(default = "default_pane_layout")]
    pub pane_layout: String,
    /// Active pane: "primary" or "secondary"
    #[serde(default = "default_active_pane")]
    pub active_pane: String,
    /// Cycle tabs in most-recently-used order (like Alt+Tab) instead of sequential
    #[serde(default)]
    pub mru_tab_cycle: bool,
    /// Startup behavior: "new_tab", "continue_session", "specific_pages"
    #[serde(default = "default_startup_mode")]
    pub startup_mode: String,
    /// Specific pages to open when startup_mode == "specific_pages"
    #[serde(default)]
    pub startup_pages: Vec<String>,
    /// Open new tabs in existing instance
    #[serde(default = "default_true")]
    pub open_tab_in_existing_instance: bool,
    /// Always switch to newly opened tab
    #[serde(default = "default_true")]
    pub always_switch_to_new_tab: bool,
    /// Recently accessed folder paths (Home > Recent).
    #[serde(default)]
    pub recent_paths: Vec<String>,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            open_tabs: Vec::new(),
            active_tab_index: 0,
            pane_layout: default_pane_layout(),
            active_pane: default_active_pane(),
            mru_tab_cycle: false,
            startup_mode: default_startup_mode(),
            startup_pages: vec!["Home".to_string()],
            open_tab_in_existing_instance: true,
            always_switch_to_new_tab: true,
            recent_paths: Vec::new(),
        }
    }
}

fn default_pane_layout() -> String {
    "single".to_string()
}

fn default_active_pane() -> String {
    "primary".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TabSession {
    pub path: String,
    pub view_mode: ViewMode,
    #[serde(default)]
    pub zoom_level: f32,
    #[serde(default)]
    pub secondary_pane_path: Option<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            version: default_version(),
            preferences: UserPreferences::default(),
            sidebar: SidebarConfig::default(),
            view: ViewConfig::default(),
            folder_preferences: HashMap::new(),
            experimental: ExperimentalConfig::default(),
            actions: ActionsConfig::default(),
            session: SessionConfig::default(),
            appearance: AppearanceConfig::default(),
            terminal_app: TerminalType::default(),
            action_bindings: None,
            keybindings: KeyBindings::default(),
            home_widgets: HomeWidgetsConfig::default(),
            tag_definitions: default_tag_definitions(),
            devtools: DevToolsConfig::default(),
            window_size: Some((1280.0, 800.0)),
            window_maximized: true,
            window_position: None,
        }
    }
}

#[allow(dead_code)]
impl AppConfig {
    /// Returns the path to the config file in the user's config directory.
    pub fn config_path() -> PathBuf {
        if let Some(proj_dirs) = directories::ProjectDirs::from("com", "files-rs", "Files-Rust") {
            let config_dir = proj_dirs.config_dir();
            config_dir.join("settings.json")
        } else {
            PathBuf::from("settings.json")
        }
    }

    /// Returns the layout preferences for a specific folder.
    pub fn get_folder_preferences(&self, path: &FilesPath) -> FolderPreferences {
        if self.view.sync_folder_preferences {
            return FolderPreferences {
                view_mode: self.view.default_view_mode.clone(),
                zoom_level: self.view.last_zoom_level,
                sort_by: self.view.sort_by.clone(),
                sort_ascending: self.view.sort_ascending,
                folder_on_top: self.view.folder_on_top,
                group_by: self.view.group_by,
            };
        }
        let key = path.to_string_lossy().into_owned();
        self.folder_preferences
            .get(&key)
            .cloned()
            .unwrap_or_else(|| FolderPreferences {
                view_mode: self.view.last_view_mode.clone(),
                zoom_level: self.view.last_zoom_level,
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

    /// Remove saved layout preferences for a specific folder (unlocking it).
    pub fn remove_folder_preferences(&mut self, path: &FilesPath) {
        let key = path.to_string_lossy().into_owned();
        self.folder_preferences.remove(&key);
    }

    /// Check if a specific folder has locked layout preferences.
    pub fn has_folder_preferences(&self, path: &FilesPath) -> bool {
        let key = path.to_string_lossy().into_owned();
        self.folder_preferences.contains_key(&key)
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

                    if let Ok(mut config) = serde_json::from_value::<AppConfig>(value) {
                        if let Some(ref mut reg) = config.action_bindings {
                            reg.rebuild_cache();
                        }
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
        if from_version < CURRENT_CONFIG_VERSION {
            if let Some(obj) = value.as_object_mut() {
                obj.insert(
                    "version".to_string(),
                    serde_json::json!(CURRENT_CONFIG_VERSION),
                );
            }
        }
    }

    /// Save current settings to disk.
    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let path = Self::config_path();

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let json = serde_json::to_string_pretty(self)?;

        let tmp_path = path.with_extension("tmp");
        fs::write(&tmp_path, &json)?;
        fs::rename(&tmp_path, &path)?;

        tracing::info!("Settings saved to {:?}", path);
        Ok(())
    }

    /// Export settings to a specified backup path.
    pub fn export_to(&self, dest: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self)?;
        fs::write(dest, json)?;
        Ok(())
    }
}

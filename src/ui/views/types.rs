use crate::config::config::{GroupColumn, SortBy, ViewMode};
use crate::core::path_util::FilesPath;
use crate::domain::extended_metadata::ExtendedMetadataExtractor;
use crate::domain::metadata::FileItem;
use crate::platform::windows::recycle_bin_service::RecycleBinItem;
use crate::storage::storage::FileType;
use crate::ui::components::dialogs::{DialogType, SettingsTab};
use crate::ui::components::icon_renderer::{is_thumbnail_image, IconRenderer, ThumbnailRenderer};
use crate::ui::constants::UiConstants;
use gpui::*;
use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

// ─────────────────────────────────────────────────────────────────────────────
// View target
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ViewTarget {
    Directory(FilesPath),
    Home,
    RecycleBin,
    ThisPc,
}

// ─────────────────────────────────────────────────────────────────────────────
// Sort state
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SortState {
    pub column: SortBy,
    pub ascending: bool,
    pub folders_first: bool,
}

impl Default for SortState {
    fn default() -> Self {
        Self {
            column: SortBy::Name,
            ascending: true,
            folders_first: true,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Pane types
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum PaneType {
    Single,
    Dual,
}

/// How a dual pane is split
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
pub enum DualPaneSplit {
    #[default]
    Horizontal, // side-by-side
    Vertical, // top-and-bottom
}

/// Selection mode
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
pub enum SelectionMode {
    #[default]
    Normal,
    Checkbox,    // checkboxes on each row
    HoverSelect, // hover triggers selection
}

/// Fit mode for folder background images
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
pub enum BackgroundFit {
    #[default]
    Fill,
    Fit,
    Stretch,
    Center,
}

/// Per-folder custom background (desktop.ini sourced)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FolderBackground {
    pub image_path: Option<FilesPath>,
    pub opacity: f32,
    pub fit: BackgroundFit,
}

impl Default for FolderBackground {
    fn default() -> Self {
        Self {
            image_path: None,
            opacity: 0.3,
            fit: BackgroundFit::Fill,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Preview / Info pane state
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
pub enum SidePaneMode {
    #[default]
    None,
    Preview,
    Info,
    Details,
}

/// Shelf pane item (staged for batch operations)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ShelfItem {
    pub path: FilesPath,
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
}

/// Lasso (rubber-band) selection state
#[derive(Debug, Clone, Default)]
pub struct LassoState {
    pub start: Point<f32>,
    pub current: Point<f32>,
}

/// Minimum delta in pixels to consider a mouse movement as a drag (not a click)
/// This prevents accidental lasso selections on simple clicks
pub const LASSO_MIN_DELTA: f32 = 5.0;

impl LassoState {
    /// Returns the normalized rectangle (min/max coordinates)
    pub fn rect(&self) -> (Point<f32>, Point<f32>) {
        let min_x = self.start.x.min(self.current.x);
        let max_x = self.start.x.max(self.current.x);
        let min_y = self.start.y.min(self.current.y);
        let max_y = self.start.y.max(self.current.y);
        (point(min_x, min_y), point(max_x, max_y))
    }

    /// Check if the drag has exceeded the minimum threshold to be considered a lasso drag
    pub fn has_exceeded_threshold(&self) -> bool {
        let dx = (self.current.x - self.start.x).abs();
        let dy = (self.current.y - self.start.y).abs();
        dx > LASSO_MIN_DELTA || dy > LASSO_MIN_DELTA
    }
}

/// Minimum delta in pixels
pub fn rects_intersect(a: (Point<f32>, Point<f32>), b: (Point<f32>, Point<f32>)) -> bool {
    let a_left = a.0.x.min(a.1.x);
    let a_right = a.0.x.max(a.1.x);
    let a_top = a.0.y.min(a.1.y);
    let a_bottom = a.0.y.max(a.1.y);

    let b_left = b.0.x.min(b.1.x);
    let b_right = b.0.x.max(b.1.x);
    let b_top = b.0.y.min(b.1.y);
    let b_bottom = b.0.y.max(b.1.y);

    a_left < b_right && a_right > b_left && a_top < b_bottom && a_bottom > b_top
}

/// Drag operation state
#[derive(Debug, Clone, Default)]
pub struct DragState {
    /// Indices of items being dragged
    pub indices: Vec<usize>,
    /// Starting mouse position
    pub start: Point<f32>,
    /// Current mouse position
    pub current: Point<f32>,
    /// Whether dragging will copy (true) or move (false)
    pub is_copy: bool,
    /// Whether a drag operation is in progress
    pub is_dragging: bool,
}

/// Drag state for reordering items within/between groups
#[derive(Debug, Clone, Default)]
pub struct GroupDragState {
    /// Index of the item being dragged
    pub dragged_index: Option<usize>,
    /// Key of the source group
    pub source_group_key: Option<String>,
    /// Current target group key (where the item would be dropped)
    pub target_group_key: Option<String>,
    /// Insert position within target group (index within the group's items)
    pub insert_position: Option<usize>,
    /// Current Y position during drag
    pub current_y: f32,
    /// Whether a drag operation is in progress
    pub is_dragging: bool,
}

/// A single item in a folder preview thumbnail grid
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FolderPreviewItem {
    /// File or folder name
    pub name: String,
    /// Whether this is a directory
    pub is_dir: bool,
}

/// Preview content for different file types
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub enum PreviewContent {
    /// No preview available
    #[default]
    None,
    /// Text preview content (with 10k char limit)
    Text(String),
    /// PDF preview info (page count, title, author)
    Pdf(PdfPreviewInfo),
    /// Audio preview info (duration, bitrate, codec)
    Audio(MediaPreviewInfo),
    /// Video preview info (duration, resolution, codec)
    Video(MediaPreviewInfo),
    /// Archive contents listing
    Archive(ArchivePreviewInfo),
    /// Third-party preview requested (Peek/QuickLook/SeerPro)
    ThirdParty(ThirdPartyPreviewInfo),
    /// Markdown preview
    Markdown,
    /// Code preview with language hint
    Code { language: String },
    /// HTML preview
    Html,
    /// Rich text (RTF) preview with stripped control words
    RichText,
    /// Folder contents thumbnail grid
    Folder(Vec<FolderPreviewItem>),
}

/// PDF metadata for preview
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct PdfPreviewInfo {
    /// Number of pages in the PDF
    pub page_count: u32,
    /// PDF title (if available)
    pub title: Option<String>,
    /// PDF author (if available)
    pub author: Option<String>,
    /// PDF subject (if available)
    pub subject: Option<String>,
    /// PDF creator application
    pub creator: Option<String>,
}

/// Audio/Video metadata for preview
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct MediaPreviewInfo {
    /// Duration in seconds
    pub duration_secs: Option<f64>,
    /// Bitrate in kbps (if available)
    pub bitrate_kbps: Option<u32>,
    /// Codec name (e.g., "h264", "aac")
    pub codec: Option<String>,
    /// For video: width in pixels
    pub width: Option<u32>,
    /// For video: height in pixels
    pub height: Option<u32>,
    /// For video: frame rate (if available)
    pub frame_rate: Option<f64>,
    /// Sample rate (for audio, in Hz)
    pub sample_rate: Option<u32>,
    /// Number of audio channels
    pub channels: Option<u8>,
    /// Media format (e.g., "MP4", "MP3", "FLAC")
    pub format: Option<String>,
}

/// Archive contents for preview
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ArchivePreviewInfo {
    /// Total number of entries in the archive
    pub entry_count: usize,
    /// List of entry names (truncated if too many)
    pub entries: Vec<String>,
    /// Total uncompressed size (if available)
    pub uncompressed_size: Option<u64>,
    /// Compression ratio (if available)
    pub compression_ratio: Option<f32>,
}

/// Third-party preview information
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ThirdPartyPreviewInfo {
    /// Which preview handler is available (Peek, QuickLook, SeerPro)
    pub handler: ThirdPartyHandler,
    /// Whether the handler is available on this system
    pub is_available: bool,
}

/// Third-party preview handlers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum ThirdPartyHandler {
    /// Microsoft PowerToys Peek
    Peek,
    /// macOS QuickLook (via wrapper)
    QuickLook,
    /// SeerPro preview tool
    SeerPro,
    /// No handler available
    #[default]
    None,
}

// ─────────────────────────────────────────────────────────────────────────────
// Main view state
// ─────────────────────────────────────────────────────────────────────────────

/// Column widths configuration for Details view
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct ColumnWidths {
    pub name: f32,
    pub date: f32,
    pub type_col: f32,
    pub size: f32,
}

impl Default for ColumnWidths {
    fn default() -> Self {
        Self {
            name: 280.0,
            date: 180.0,
            type_col: 120.0,
            size: 80.0,
        }
    }
}

/// Tracks a column resize operation in progress (drag to resize column width)
#[derive(Debug, Clone)]
pub struct ColumnResizeState {
    /// Which column is being resized: "date", "type", or "size"
    pub column: String,
    /// The X position where the drag started (in window coordinates)
    pub start_x: f32,
    /// The initial width of the column being resized
    pub initial_width: f32,
    /// The initial width of the date column (needed because column widths vary)
    pub original_widths: ColumnWidths,
}

/// Column visibility configuration for Details view (Name is always visible)
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct ColumnVisibility {
    pub date: bool,
    pub type_col: bool,
    pub size: bool,
}

impl Default for ColumnVisibility {
    fn default() -> Self {
        Self {
            date: true,
            type_col: true,
            size: true,
        }
    }
}

/// Cached info for the details pane (updated when selection changes)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct DetailsPaneCache {
    pub tags: Vec<String>,
    pub git_branch: Option<String>,
    pub git_status: Option<String>,
    pub git_dirty: bool,
    pub exif_camera: Option<String>,
    pub exif_date: Option<String>,
    pub exif_dimensions: Option<String>,
    pub audio_artist: Option<String>,
    pub audio_album: Option<String>,
    pub audio_duration: Option<String>,
    pub video_duration: Option<String>,
    pub video_resolution: Option<String>,
}

/// Toolbar customization: tracks which toolbar buttons are visible.
/// Uses a HashMap of button ID -> visible state. Missing entries default to visible.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ToolbarCustomizationState {
    pub visible_buttons: std::collections::HashMap<String, bool>,
}

impl Default for ToolbarCustomizationState {
    fn default() -> Self {
        let mut map = std::collections::HashMap::new();
        for id in &[
            "new_folder",
            "new_file",
            "cut",
            "copy",
            "paste",
            "delete",
            "rename",
            "details_view",
            "grid_view",
            "columns_view",
            "preview_pane",
            "info_pane",
            "compact_mode",
            "search",
        ] {
            map.insert(id.to_string(), true);
        }
        Self {
            visible_buttons: map,
        }
    }
}

impl ToolbarCustomizationState {
    pub fn is_visible(&self, id: &str) -> bool {
        self.visible_buttons.get(id).copied().unwrap_or(true)
    }

    pub fn toggle(&mut self, id: &str) {
        if let Some(v) = self.visible_buttons.get_mut(id) {
            *v = !*v;
        }
    }
}

/// Search state for global search functionality
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct SearchState {
    /// Whether search mode is active
    pub is_searching: bool,
    /// Current search query
    pub query: String,
    /// Search results (indices into MainViewState.items)
    pub results: Vec<usize>,
    /// Whether search is in progress
    pub loading: bool,
    /// Search mode: name, content, size, date, type, tag
    pub mode: SearchMode,
    /// Search results from recursive folder search
    pub deep_results: Vec<SearchResult>,
    /// Content search toggle
    pub search_content: bool,
    /// Minimum file size filter (bytes)
    pub size_min: u64,
    /// Maximum file size filter (bytes)
    pub size_max: u64,
    /// Date filter: only show files modified after this (Unix timestamp)
    pub date_after: i64,
    /// Date filter: only show files modified before this (Unix timestamp)
    pub date_before: i64,
    /// File extension filter (e.g., "txt", "rs", "md")
    pub extension_filter: String,
    /// Tag filter
    pub tag_filter: String,
    /// Whether search has been cancelled (runtime-only, not persisted)
    #[serde(skip)]
    pub cancelled: bool,
    /// Cancellation token for the current search (runtime-only)
    #[serde(skip)]
    pub cancel_token: Option<Arc<AtomicBool>>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum SearchMode {
    Name,
    Content,
    Size,
    Date,
    Type,
    Tag,
}

impl Default for SearchMode {
    fn default() -> Self {
        SearchMode::Name
    }
}

/// A search result item with path info
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SearchResult {
    pub path: FilesPath,
    pub name: String,
    pub parent_path: String,
    pub is_dir: bool,
    pub size: u64,
    pub modified: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MainViewState {
    pub pane_type: PaneType,
    pub dual_pane_split: DualPaneSplit,
    pub view_mode: ViewMode,
    pub target: ViewTarget,
    pub primary_path: FilesPath,
    pub secondary_path: Option<FilesPath>,
    pub items: Vec<FileItem>,
    /// Indices of selected items
    pub selected_indices: HashSet<usize>,
    pub sort: SortState,
    /// Filter text (shown in filter header)
    pub filter_text: String,
    /// Show dot-prefixed files
    pub show_dot_files: bool,
    /// Show hidden (system-hidden) files
    pub show_hidden_files: bool,
    /// Show file extensions
    pub show_extensions: bool,
    /// Show protected system files
    pub show_system_files: bool,
    /// Whether we are loading items (shows spinner placeholder)
    pub loading: bool,
    /// Side pane mode (Preview / Info / Details / None)
    pub side_pane: SidePaneMode,
    /// Width of the side pane
    pub side_pane_width: Option<f32>,
    /// Side pane resize start mouse position
    pub side_pane_resize_start_x: Option<f32>,
    /// Side pane resize start width
    pub side_pane_resize_start_width: Option<f32>,
    /// Whether rename is in progress for an item index
    pub renaming_index: Option<usize>,
    /// Current rename text (used during inline rename)
    pub rename_text: String,
    pub compact_mode: bool,
    /// Cached text for file preview
    pub preview_text: Option<String>,
    /// Column widths for Details view
    pub column_widths: ColumnWidths,
    /// Column visibility for Details view (Name is always visible)
    pub column_visibility: ColumnVisibility,
    /// Group column for organizing items into collapsible groups
    pub group_by: GroupColumn,
    /// Set of collapsed group keys (e.g., "A", "Today", "Large")
    pub collapsed_groups: HashSet<String>,
    /// Selection interaction mode
    pub selection_mode: SelectionMode,
    /// Shelf pane is visible
    pub shelf_visible: bool,
    /// Items staged in the shelf
    pub shelf_items: Vec<ShelfItem>,
    /// Whether view mode / zoom level lock is enabled for per-directory memory
    pub view_lock_enabled: bool,
    /// Zoom level (0.0–1.0) for the zoom slider / view mode switcher
    pub zoom_level: f32,
    /// Per-folder background image settings
    pub folder_background: FolderBackground,
    /// Which pane is currently focused in dual-pane mode
    pub active_pane: crate::core::navigation_manager::PaneFocus,
    /// Last item clicked (for keyboard range select anchor)
    pub last_clicked_index: Option<usize>,
    /// Lasso (rubber-band) selection state (runtime-only, not persisted)
    #[serde(skip)]
    pub lasso_state: Option<LassoState>,
    /// Cached icon file paths for Windows Shell icons (populated by background extraction)
    #[serde(skip)]
    pub icon_paths:
        std::collections::HashMap<crate::core::path_util::FilesPath, std::path::PathBuf>,
    /// Drag operation state (runtime-only, not persisted)
    #[serde(skip)]
    pub drag_state: DragState,
    /// Cached details pane info (tags, git, exif, etc.)
    pub details_cache: DetailsPaneCache,
    /// Global search state
    pub search_state: SearchState,
    /// Toolbar button visibility customization
    #[serde(default)]
    pub toolbar_customization: ToolbarCustomizationState,
    /// Preview content for different file types (PDF, Audio, Video, Archive)
    pub preview_content: PreviewContent,
    /// Drag state for reordering items within/between groups (runtime-only, not persisted)
    #[serde(skip)]
    pub group_drag_state: GroupDragState,
    /// Items in the Recycle Bin
    pub recycle_bin_items: Vec<RecycleBinItem>,
    /// Cache of tags per item path (path -> tags), used for sort/group by tag
    #[serde(skip)]
    pub tags_cache: std::collections::HashMap<String, Vec<String>>,
    /// Cache of all tags with counts for home view (tag_name, color, count)
    #[serde(skip)]
    pub home_tags_cache: Vec<(String, String, i64)>,
    /// Scroll handle for mouse wheel scrolling (runtime-only, not persisted)
    #[serde(skip)]
    pub scroll_handle: ScrollHandle,
    /// Scroll handle for uniform_list (runtime-only, not persisted)
    #[serde(skip)]
    pub uniform_list_scroll_handle: UniformListScrollHandle,
    /// Timestamp of last click on empty space (for double-click → navigate_back detection)
    #[serde(skip)]
    pub last_empty_click: Option<std::time::Instant>,
    /// Last check timestamp for viewport-based icon refresh (debounce guard)
    #[serde(skip)]
    pub icon_refresh_last_check: Option<std::time::Instant>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Grouped item types (used for grouped grid view)
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Clone, PartialEq, Eq)]
pub enum GroupedItemType {
    Header {
        key: String,
        label: String,
        count: usize,
    },
    Item(usize),
}

#[derive(Clone, PartialEq, Eq)]
pub enum GroupedGridItem {
    Header {
        key: String,
        label: String,
        count: usize,
    },
    Row(Vec<usize>),
}

// ─────────────────────────────────────────────────────────────────────────────
// File items model for uniform_list
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct FileItems {
    pub items: Vec<FileItem>,
    pub selected_indices: HashSet<usize>,
}

impl Default for FileItems {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            selected_indices: HashSet::new(),
        }
    }
}

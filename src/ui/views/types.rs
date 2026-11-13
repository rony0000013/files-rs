use crate::config::config::{GroupColumn, SortBy, ViewMode};
use crate::core::path_util::FilesPath;
use crate::domain::extended_metadata::ExtendedMetadataExtractor;
use crate::domain::metadata::FileItem;
use crate::platform::windows::media_player::VideoFrame;
use crate::platform::windows::office_preview_host::OfficePreviewHost;
use crate::platform::windows::recycle_bin_service::RecycleBinItem;
use crate::storage::storage::FileType;
use crate::ui::components::bouncy_scroll::BouncyScrollState;
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
    Settings,
}

impl Default for ViewTarget {
    fn default() -> Self {
        ViewTarget::Home
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Sort state
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SortState {
    pub column: SortBy,
    pub ascending: bool,
    pub folders_first: bool,
    #[serde(default)]
    pub files_first: bool,
}

impl Default for SortState {
    fn default() -> Self {
        Self {
            column: SortBy::Name,
            ascending: true,
            folders_first: true,
            files_first: false,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Pane types
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
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
    /// Start position in pane-local coordinates (relative to the list viewport top-left)
    pub start: Point<f32>,
    /// Current position in pane-local coordinates
    pub current: Point<f32>,
    /// Window-space origin of the list viewport used to convert window mouse events to local coords
    pub pane_origin: Point<f32>,
    /// Whether Ctrl or Shift was held when the lasso started (additive selection)
    pub additive: bool,
    /// Snapshot of selected_indices at the start of an additive lasso
    pub initial_selection: HashSet<usize>,
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

/// Drag operation state (supports internal multi-target drop + out-of-window OS native drag)
#[derive(Debug, Clone, Default)]
pub struct DragState {
    /// Indices of items being dragged in `source_pane`
    pub indices: Vec<usize>,
    /// Full paths of items being dragged
    pub paths: Vec<FilesPath>,
    /// Source pane where drag started
    pub source_pane: crate::core::navigation_manager::PaneFocus,
    /// Starting mouse position (window coordinates)
    pub start: Point<f32>,
    /// Current mouse position (window coordinates)
    pub current: Point<f32>,
    /// Whether dragging will copy (true) or move (false)
    pub is_copy: bool,
    /// Whether Alt is held to create a shortcut/symlink
    pub is_link: bool,
    /// Whether a drag operation is in progress
    pub is_dragging: bool,
    /// Pending mouse-down candidate before crossing the 6px drag threshold: `(pane, item_idx, start_pos)`
    pub pending_candidate: Option<(crate::core::navigation_manager::PaneFocus, usize, Point<f32>)>,
    /// Pending mouse-down candidate for path-based items (Miller Column 1 & Column 3): `(pane, path, start_pos)`
    pub pending_path_candidate:
        Option<(crate::core::navigation_manager::PaneFocus, FilesPath, Point<f32>)>,
    /// Hovered destination folder path (if hovering a folder row/tile, tab, breadcrumb, or sidebar item)
    pub hovered_target_folder: Option<FilesPath>,
    /// Hovered destination display name for the floating Files-community drag badge
    pub hovered_target_label: Option<String>,
    /// Whether native OS drag-and-drop (`SHDoDragDrop` / `CF_HDROP` / `text/uri-list`) was triggered for this gesture
    pub os_drag_dispatched: bool,
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

/// A single item in a folder preview contents list/grid
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct FolderPreviewItem {
    /// File or folder name
    pub name: String,
    /// Whether this is a directory
    pub is_dir: bool,
    /// Full path for direct navigation on click
    #[serde(default)]
    pub path: FilesPath,
    /// File size in bytes
    #[serde(default)]
    pub size: u64,
    /// Formatted modified date
    #[serde(default)]
    pub modified: Option<String>,
    /// Uppercase extension or "FOLDER"
    #[serde(default)]
    pub ext: String,
}

/// Preview content for wide range of file types and folders
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub enum PreviewContent {
    /// No preview available
    #[default]
    None,
    /// Text preview content
    Text(String),
    /// PDF preview info (page count, title, author, extracted text)
    Pdf(PdfPreviewInfo),
    /// Audio preview info (duration, bitrate, codec, ID3/Vorbis tags)
    Audio(MediaPreviewInfo),
    /// Video preview info (duration, resolution, fps, codec, poster frame)
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
    /// Folder contents preview (subfolders & files with metadata)
    Folder(Vec<FolderPreviewItem>),
    /// Tabular CSV / TSV / XLSX spreadsheet preview
    CsvTable {
        headers: Vec<String>,
        rows: Vec<Vec<String>>,
        total_rows: usize,
        delimiter: char,
        /// Table source label ("CSV", "TSV", "XLSX ... sheet")
        kind: String,
    },
    /// Office / E-Book document preview (.docx, .xlsx, .pptx, .odt, .epub)
    Document {
        doc_type: String,
        title: Option<String>,
        sections: Vec<String>,
        summary: String,
    },
    /// SQLite database schema & table row counts preview
    Database {
        tables: Vec<(String, usize)>,
        page_size: u32,
    },
    /// Typography Font specimen waterfall preview (.ttf, .otf, .woff, .woff2)
    Font {
        family_name: String,
        format: String,
        file_size: u64,
    },
    /// Office document rendered as real page/slide images by the pure-Rust
    /// BetterOffice engine (DOCX layout, XLSX grid, PPTX slides -> PNG).
    Office {
        /// Rendered page/slide PNG cache files, in document order.
        pages: Vec<std::path::PathBuf>,
        /// Human label, e.g. "Excel Worksheet" or "PowerPoint Slides".
        label: String,
    },
    /// Binary / Executable (PE, ELF, Mach-O, WASM) header & Hex + ASCII inspector
    BinaryHex {
        format_description: String,
        arch: Option<String>,
        hex_lines: Vec<(u32, String, String)>,
    },
    /// Rich Image preview (with bounded VRAM cache path, dimensions, megapixels, aspect ratio, EXIF)
    Image(ImagePreviewInfo),
    /// Vector SVG preview (rasterized via `resvg` + viewBox/element stats + XML source)
    Svg(SvgPreviewInfo),
    /// Visual Office / E-Book preview (`.docx`, `.xlsx`, `.pptx`, `.odt`, `.ods`, `.odp`, `.epub`)
    /// with rendered page/slide canvas, multi-sheet Excel grid, or paginated document view
    OfficeVisual(OfficePreviewInfo),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum OfficeDocKind {
    #[default]
    WordDocument,
    Spreadsheet,
    Presentation,
    EBook,
}

/// Visual Cross-Platform Office & E-Book Preview state (`.docx`, `.xlsx`, `.pptx`, `.odt`, `.ods`, `.odp`, `.epub`)
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct OfficePreviewInfo {
    pub doc_kind: OfficeDocKind,
    pub format_label: String,
    /// Optional rendered page/slide/cover PNG path (from embedded thumbnail, LibreOffice/OS shell, or slide canvas)
    pub rendered_page_path: Option<std::path::PathBuf>,
    /// Extracted embedded images (`word/media/*`, `ppt/media/*`, EPUB cover) cached to disk
    pub embedded_media_paths: Vec<std::path::PathBuf>,
    /// Page titles, Slide titles, or Worksheet tab names
    pub section_titles: Vec<String>,
    /// Paginated text blocks per page (Word/ODT/EPUB) or bullet lines per slide (PPTX/ODP)
    pub pages_blocks: Vec<Vec<String>>,
    /// Currently selected 0-based page, slide, or worksheet tab index
    pub active_index: usize,
    /// For Spreadsheets (`.xlsx`, `.ods`): column headers (`A`, `B`, `C`...)
    pub sheet_headers: Vec<String>,
    /// For Spreadsheets (`.xlsx`, `.ods`): bounded cell rows (max 50 rows × 12 cols) for instant 120 FPS rendering
    pub sheet_rows: Vec<Vec<String>>,
    /// Estimated total rows in active worksheet
    pub total_rows: usize,
}

/// Rich Image metadata & bounded-VRAM preview path
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ImagePreviewInfo {
    /// Path to display in GPUI `img(...)` (downscaled cached PNG if source > 1280px or TIFF/ICO/TGA)
    pub display_path: std::path::PathBuf,
    /// Original width in pixels
    pub width: u32,
    /// Original height in pixels
    pub height: u32,
    /// Format label (e.g., "PNG", "JPEG", "WEBP", "TIFF", "ICO", "TGA")
    pub format: String,
    /// Color / bit-depth description (e.g., "RGBA8 (32-bit)", "RGB8 (24-bit)")
    pub color_type: String,
    /// Aspect ratio string (e.g., "16:9", "4:3", "1:1")
    pub aspect_ratio: String,
    /// Megapixels (e.g., 2.07 for 1920x1080)
    pub megapixels: f32,
    /// Whether the displayed image is a <=1024px VRAM-optimized proxy
    #[serde(default)]
    pub is_downscaled_preview: bool,
    /// Optional EXIF camera & exposure summary
    pub exif_summary: Option<String>,
    #[serde(default)]
    pub camera_model: Option<String>,
    #[serde(default)]
    pub lens_model: Option<String>,
    #[serde(default)]
    pub iso: Option<String>,
    #[serde(default)]
    pub aperture: Option<String>,
    #[serde(default)]
    pub exposure_time: Option<String>,
    #[serde(default)]
    pub focal_length: Option<String>,
    #[serde(default)]
    pub exif_date: Option<String>,
}

/// Vector SVG metadata & rasterized preview path (`resvg`)
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct SvgPreviewInfo {
    /// Path to rasterized 512x512 PNG in `icons_cache_dir()`
    pub rendered_png_path: Option<std::path::PathBuf>,
    /// SVG viewBox string (e.g., "0 0 512 512")
    pub view_box: String,
    /// SVG viewBox width
    pub width: f32,
    /// SVG viewBox height
    pub height: f32,
    /// Number of vector path/shape elements
    pub element_count: usize,
    /// Bounded XML source snippet
    pub xml_snippet: String,
}

/// Visual Rendered PDF Page preview info (no raw text dump; renders actual PDF pages to PNG)
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct PdfPreviewInfo {
    /// Total number of pages in the PDF
    pub page_count: u32,
    /// Currently viewed 1-based page number (`1..=page_count`)
    #[serde(default)]
    pub current_page: u32,
    /// Path to the rasterized PNG image of `current_page` in `icons_cache_dir()`
    #[serde(default)]
    pub rendered_page_path: Option<std::path::PathBuf>,
    /// Page width in PDF points (e.g. 595.0 for A4, 612.0 for Letter)
    #[serde(default)]
    pub page_width_pt: f32,
    /// Page height in PDF points (e.g. 842.0 for A4, 792.0 for Letter)
    #[serde(default)]
    pub page_height_pt: f32,
    /// Name of the active PDF rendering backend
    #[serde(default)]
    pub renderer_backend: String,
    /// PDF title (if available)
    pub title: Option<String>,
    /// PDF author (if available)
    pub author: Option<String>,
    /// PDF subject (if available)
    pub subject: Option<String>,
    /// PDF creator application
    pub creator: Option<String>,
    /// Extracted text preview from PDF pages
    #[serde(default)]
    pub extracted_text: Option<String>,
    /// PDF specification version (e.g. "1.7")
    #[serde(default)]
    pub pdf_version: Option<String>,
    /// File size in bytes
    #[serde(default)]
    pub file_size: u64,
    /// Rasterized page images (PNG cache files, first N pages via Pdfium)
    #[serde(skip)]
    pub pages: Vec<std::path::PathBuf>,
}

/// Audio/Video metadata for preview
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct MediaPreviewInfo {
    /// Duration in seconds
    pub duration_secs: Option<f64>,
    /// Bitrate in kbps (if available)
    pub bitrate_kbps: Option<u32>,
    /// Codec name (e.g., "H.264 / AVC", "HEVC", "AAC", "MP3")
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
    /// Media format (e.g., "MP4", "MKV", "MP3", "FLAC")
    pub format: Option<String>,
    /// Artist name (for audio)
    pub artist: Option<String>,
    /// Track/Video title
    #[serde(default)]
    pub title: Option<String>,
    /// Album name
    #[serde(default)]
    pub album: Option<String>,
    /// Extracted video poster frame or album art path
    #[serde(default)]
    pub poster_path: Option<std::path::PathBuf>,
    /// Container & audio stream summary
    #[serde(default)]
    pub container_info: Option<String>,
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

fn default_miller_col1_width() -> f32 {
    220.0
}
fn default_miller_col2_width() -> f32 {
    340.0
}
fn default_miller_col3_width() -> f32 {
    380.0
}

/// Column widths configuration for Details view and Miller Columns view
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct ColumnWidths {
    pub name: f32,
    pub date: f32,
    pub type_col: f32,
    pub size: f32,
    #[serde(default = "default_miller_col1_width")]
    pub miller_col1: f32,
    #[serde(default = "default_miller_col2_width")]
    pub miller_col2: f32,
    #[serde(default = "default_miller_col3_width")]
    pub miller_col3: f32,
}

impl Default for ColumnWidths {
    fn default() -> Self {
        Self {
            name: 280.0,
            date: 180.0,
            type_col: 120.0,
            size: 80.0,
            miller_col1: 220.0,
            miller_col2: 340.0,
            miller_col3: 380.0,
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
    #[serde(default)]
    pub git_commit_author: Option<String>,
    #[serde(default)]
    pub git_commit_date: Option<String>,
    #[serde(default)]
    pub git_commit_message: Option<String>,
    pub exif_camera: Option<String>,
    pub exif_date: Option<String>,
    pub exif_dimensions: Option<String>,
    #[serde(default)]
    pub exif_iso: Option<String>,
    #[serde(default)]
    pub exif_aperture: Option<String>,
    #[serde(default)]
    pub exif_exposure: Option<String>,
    #[serde(default)]
    pub exif_focal_length: Option<String>,
    #[serde(default)]
    pub exif_lens: Option<String>,
    #[serde(default)]
    pub mime_type: Option<String>,
    pub audio_artist: Option<String>,
    pub audio_album: Option<String>,
    pub audio_duration: Option<String>,
    pub video_duration: Option<String>,
    pub video_resolution: Option<String>,
    #[serde(default)]
    pub folder_subfolders: usize,
    #[serde(default)]
    pub folder_files: usize,
    #[serde(default)]
    pub folder_total_size: u64,
}

/// Toolbar customization: tracks which toolbar buttons are visible.
/// Uses a HashMap of button ID -> visible state. Missing entries default to visible.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ToolbarCustomizationState {
    pub visible_buttons: std::collections::HashMap<String, bool>,
    #[serde(skip)]
    pub active_flyout: Option<String>,
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
            active_flyout: None,
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
    /// Search results (indices into MainViewState.items) — HashSet for O(1) lookups
    pub results: HashSet<usize>,
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
    /// Treat search query as a regular expression
    #[serde(default)]
    pub use_regex: bool,
    /// Case-sensitive search matching
    #[serde(default)]
    pub case_sensitive: bool,
    /// Full-text metadata search (EXIF, ID3, PDF, dimensions, tags, dates, attributes)
    #[serde(default)]
    pub search_metadata: bool,
    /// Search subfolders recursively
    #[serde(default = "default_true_bool")]
    pub recursive: bool,
    /// Number of items scanned during deep search
    #[serde(skip)]
    pub scanned_count: usize,
    /// Snapshot of current folder items before deep search replaced `items` (for 0ms restore)
    #[serde(skip)]
    pub pre_search_items: Option<Vec<FileItem>>,
}

fn default_true_bool() -> bool {
    true
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
    #[serde(default)]
    pub match_snippet: Option<String>,
    #[serde(default)]
    pub match_indices: Vec<usize>,
    #[serde(default)]
    pub score: u32,
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
    /// Unified text input state for inline file rename
    #[serde(skip)]
    pub rename_input: crate::ui::components::text_input::TextInputState,
    /// Paths of files currently cut to the clipboard (rendered with opacity + dashed border)
    #[serde(skip)]
    pub cut_paths: HashSet<std::path::PathBuf>,
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
    /// Split ratio for dual-pane mode (0.2..=0.8, default 0.5)
    #[serde(default = "default_dual_pane_ratio")]
    pub dual_pane_ratio: f32,
    /// Active drag state for dual-pane splitter: (start_mouse_coord, start_ratio)
    #[serde(skip)]
    pub dual_pane_resize_start: Option<(f32, f32)>,
    /// Last item clicked / focused by keyboard
    pub last_clicked_index: Option<usize>,
    /// Anchor item index for Shift+Arrow / Shift+Click range selection
    #[serde(skip)]
    pub selection_anchor_index: Option<usize>,
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
    pub icon_refresh_last_check: std::cell::Cell<Option<std::time::Instant>>,
    /// Miller Columns: scroll handle for Column 1 (parent siblings).
    #[serde(skip)]
    pub col1_scroll_handle: UniformListScrollHandle,
    /// Miller Columns: scroll handle for Column 3 (child preview).
    #[serde(skip)]
    pub col3_scroll_handle: UniformListScrollHandle,
    /// Cached Column 1 rows (name, path, is_current), keyed by `col1_siblings_for`.
    #[serde(skip)]
    pub col1_siblings: Vec<(String, FilesPath, bool)>,
    #[serde(skip)]
    pub col1_siblings_for: Option<FilesPath>,
    /// Cached Column 3 preview rows (name, path, is_dir, size), keyed by `col3_children_for`.
    #[serde(skip)]
    pub col3_children: Vec<(String, FilesPath, bool, u64)>,
    #[serde(skip)]
    pub col3_children_for: Option<FilesPath>,
    /// Bouncy scroll state for the main file view content area (home, grouped, recycle bin only)
    #[serde(skip)]
    pub bouncy_scroll: BouncyScrollState,
    /// Cached git status per file path (populated during directory load)
    #[serde(skip)]
    pub git_status_cache:
        std::collections::HashMap<FilesPath, crate::domain::git_service::GitFileStatus>,
    /// Monotonic revision counter bumped whenever `items` or metadata batches update
    #[serde(skip)]
    pub items_revision: u64,
    /// Interior-mutable per-generation cache of filtered+sorted item indices (eliminates per-frame sorting)
    #[serde(skip)]
    pub visible_cache: std::cell::RefCell<Option<(VisibleCacheKey, std::rc::Rc<Vec<usize>>)>>,
    /// Scroll handle for the right-hand Details / Preview SidePane
    #[serde(skip)]
    pub side_pane_scroll_handle: ScrollHandle,
    /// Horizontal scroll handle for wide spreadsheet tables in the preview pane.
    #[serde(skip)]
    pub preview_table_scroll: ScrollHandle,
    /// Smooth scroll state for the right-hand Details / Preview SidePane
    #[serde(skip)]
    pub side_pane_bouncy_scroll: BouncyScrollState,
    /// Scroll handle for the Spacebar Peek Preview Modal
    #[serde(skip)]
    pub peek_scroll_handle: ScrollHandle,
    /// Smooth scroll state for the Spacebar Peek Preview Modal
    #[serde(skip)]
    pub peek_bouncy_scroll: BouncyScrollState,
    /// Whether the Spacebar Peek Preview Modal is currently open
    #[serde(skip)]
    pub show_peek_modal: bool,
    /// Monotonic generation ID for cancelling stale background preview loads
    #[serde(skip)]
    pub preview_request_id: u64,
    /// Pre-tokenized syntax-highlighted lines for code preview (prevents re-tokenizing on every resize frame)
    #[serde(skip)]
    pub preview_highlighted_lines: std::sync::Arc<Vec<Vec<(String, Option<gpui::Rgba>)>>>,
    /// Cached drive list for empty-selection Details pane (prevents Win32 drive queries during resize)
    #[serde(skip)]
    pub cached_drives: Vec<crate::platform::windows::drive_service::DriveInfo>,
    /// Interactive video preview playback state
    #[serde(skip)]
    pub video_playing: bool,
    /// Interactive video preview scrub progress (0.0..=1.0)
    #[serde(skip)]
    pub video_progress: f32,
    /// Interactive video preview mute state
    #[serde(skip)]
    pub video_muted: bool,
    /// Latest decoded video frame (Media Foundation pump). Rendered via
    /// `ImageSource::Render` so no per-frame files or cache entries exist.
    #[serde(skip)]
    pub video_frame: Option<std::sync::Arc<VideoFrame>>,
    /// WYSIWYG Office preview host (registered IPreviewHandler in a child HWND).
    /// Falls back to Markdown when no handler is registered.
    #[serde(skip)]
    pub office_preview: OfficePreviewHost,
    /// Whether the inline filter bar is toggled open
    #[serde(default)]
    pub show_filter_bar: bool,
    /// Whether groups are ordered ascending (`true`) or descending (`false`)
    #[serde(default = "default_true_bool")]
    pub group_ascending: bool,
    /// Expanded folder paths in main-view Tree Mode
    #[serde(skip)]
    pub tree_expanded_paths: HashSet<std::path::PathBuf>,
    /// Cached sorted direct children for expanded folders in main-view Tree Mode
    #[serde(skip)]
    pub tree_children_cache: std::collections::HashMap<std::path::PathBuf, Vec<FileItem>>,
    /// Folder paths currently loading children in the background for Tree Mode
    #[serde(skip)]
    pub tree_loading_paths: HashSet<std::path::PathBuf>,
    /// Top-level items snapshot for lossless switch between flat views and Tree Mode
    #[serde(skip)]
    pub tree_top_items: Vec<FileItem>,
    /// Per-item nesting depth aligned with `items` when `view_mode == ViewMode::Tree`
    #[serde(skip)]
    pub tree_depths: Vec<u16>,
    /// Monotonic generation token invalidating stale async Tree Mode child loads
    #[serde(skip)]
    pub tree_load_generation: u64,
}

impl MainViewState {
    /// Clear all inline rename state.
    pub fn clear_rename_state(&mut self) {
        self.renaming_index = None;
        self.rename_input = crate::ui::components::text_input::TextInputState::default();
    }

    /// Invalidate the cached sorted view index mapping when sort settings change.
    /// (`visible_items_rc()` performs non-destructive natural sorting so `self.items`,
    /// `self.selected_indices`, and `FileItems` stay index-aligned at all times.)
    pub fn apply_sort(&mut self) {
        if matches!(self.view_mode, crate::config::config::ViewMode::Tree) {
            self.rebuild_tree_items();
        } else {
            self.bump_items_revision();
        }
    }
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

// FileItems is defined in main_view.rs (required by GPUI uniform_list pattern)
// Re-exported here for convenience
pub use super::main_view::FileItems;


/// Key used to validate whether the cached `visible_items()` index vector is still current.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisibleCacheKey {
    pub primary_path: FilesPath,
    pub items_len: usize,
    pub items_revision: u64,
    pub first_modified: Option<std::time::SystemTime>,
    pub last_modified: Option<std::time::SystemTime>,
    pub last_size: u64,
    pub sort_col_discriminant: usize,
    pub sort_ascending: bool,
    pub folders_first: bool,
    pub files_first: bool,
    pub group_col_discriminant: usize,
    pub group_ascending: bool,
    pub filter_text: String,
    pub show_dot_files: bool,
    pub show_hidden_files: bool,
    pub show_system_files: bool,
    pub is_searching: bool,
    pub search_query: String,
    pub search_results_len: usize,
    pub tags_cache_len: usize,
    pub is_tree_mode: bool,
}

fn default_dual_pane_ratio() -> f32 {
    0.5
}

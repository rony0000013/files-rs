use crate::config::config::{SortBy, ViewMode};
use crate::config::settings_model::SettingsModel;
use crate::core::navigation_manager::{NavigationState, PaneLayout};
use crate::core::path_util::FilesPath;
use crate::domain::git_service::GitService;
use crate::domain::metadata::FileItem;
use crate::ui::components::dialogs::{DialogType, SettingsTab};
use crate::ui::components::navigation_bar::NavigationBarState;
use crate::ui::components::sidebar_state::{SidebarItemId, SidebarState};
use crate::ui::components::splash_screen::SplashState;
use crate::ui::components::status_bar::StatusBarState;
use crate::ui::constants::UiConstants;
use crate::ui::views::main_view::{ColumnResizeState, ColumnWidths, FileItems, MainViewState};
use gpui::*;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::core::undo_redo::UndoRedoManager;
use crate::platform::windows::thumbnail_service::{ThumbnailService, ThumbnailSize};
use crate::ui::components::clipboard_manager::{ClipboardItem, ClipboardOperation, ClipboardState};

/// Directory for cached icon PNG files
pub fn icons_cache_dir() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join("files-rs-icons");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// State of a tag across selected items
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TagState {
    /// All selected items have this tag
    All,
    /// Some selected items have this tag
    Some,
    /// No selected items have this tag
    None,
}

pub struct AppState {
    pub settings: Entity<SettingsModel>,
    pub localization: Entity<crate::localization::localization::LocalizationManager>,
    pub navigation: Entity<crate::core::navigation_manager::NavigationState>,
    pub main_view: MainViewState,
    pub file_items: Entity<FileItems>,
    pub nav_bar: NavigationBarState,
    pub sidebar: SidebarState,
    pub sidebar_visible: bool,
    pub status_bar: StatusBarState,
    pub status_center: crate::ui::components::status_center::StatusCenterState,
    pub compact_overlay: crate::ui::components::compact_overlay::CompactOverlayState,
    pub progress_engine: Arc<crate::core::progress::ProgressEngine>,
    pub tag_service: Arc<crate::core::tag_service::TagService>,
    pub git_service: Arc<GitService>,
    pub clipboard: ClipboardState,
    pub active_dialog: Option<DialogType>,
    pub active_settings_tab: SettingsTab,
    pub is_fullscreen: bool,
    pub action_registry: crate::core::action_registry::ActionRegistry,
    pub show_shortcut_overlay: bool,
    pub session_service: crate::core::session_service::SessionService,
    pub splash_state: SplashState,
    pub thumbnail_service: Arc<ThumbnailService>,
    /// Cache of extracted icon temp file paths, keyed by FilesPath
    pub icon_cache: Arc<Mutex<HashMap<FilesPath, std::path::PathBuf>>>,
    /// Undo/redo manager for file operations
    pub undo_redo: UndoRedoManager,
}

impl AppState {
    pub fn new(
        root: FilesPath,
        file_items: Entity<FileItems>,
        settings: Entity<SettingsModel>,
        localization: Entity<crate::localization::localization::LocalizationManager>,
        navigation: Entity<crate::core::navigation_manager::NavigationState>,
        progress_engine: Arc<crate::core::progress::ProgressEngine>,
        db: Arc<crate::core::database::Database>,
        cx: &gpui::App,
    ) -> Self {
        let config = settings.read(cx);
        let default_view_mode = config.config().view.default_view_mode.clone();

        Self {
            settings,
            localization,
            navigation,
            main_view: MainViewState::new(root.clone(), default_view_mode),
            file_items,
            nav_bar: NavigationBarState::new(root),
            sidebar: SidebarState::new(),
            sidebar_visible: true,
            status_bar: StatusBarState::new(),
            status_center: crate::ui::components::status_center::StatusCenterState::new(),
            compact_overlay: crate::ui::components::compact_overlay::CompactOverlayState::new(),
            progress_engine,
            tag_service: Arc::new(crate::core::tag_service::TagService::new(db)),
            git_service: Arc::new(GitService),
            clipboard: ClipboardState::default(),
            active_dialog: None,
            active_settings_tab: SettingsTab::General,
            is_fullscreen: false,
            action_registry: config
                .config()
                .action_bindings
                .clone()
                .unwrap_or_else(crate::core::action_registry::ActionRegistry::new),
            show_shortcut_overlay: false,
            splash_state: SplashState::new(),
            session_service: {
                let session_path =
                    crate::core::session_service::SessionService::default_session_path();
                crate::core::session_service::SessionService::new(session_path)
            },
            thumbnail_service: Arc::new(ThumbnailService::new()),
            icon_cache: Arc::new(Mutex::new(HashMap::new())),
            undo_redo: UndoRedoManager::new(),
        }
    }
}

pub struct SidebarDragState {
    pub dragged_index: usize,
    pub dragged_item_id: SidebarItemId,
    pub current_y: f32,
    pub drop_target_index: Option<usize>,
}

/// Sidebar pinned item drag state for reordering
#[derive(Debug, Clone)]
pub struct SidebarPinnedDragState {
    pub source_index: usize,
    pub current_y: f32,
    pub drop_target_index: Option<usize>,
    pub is_dragging: bool,
}

/// Data for the hover file-preview tooltip shown on short hover delay
#[derive(Debug, Clone)]
pub struct HoverTooltipData {
    /// Screen position of the tooltip
    pub position: gpui::Point<gpui::Pixels>,
    pub item: crate::domain::metadata::FileItem,
}

pub struct AppWindow {
    pub state: AppState,
    /// Index of the item currently being hovered (for hover-to-open timer)
    pub hover_item_index: Option<usize>,
    /// Hover tooltip state for file preview popup
    pub hover_tooltip: Option<HoverTooltipData>,
    /// Active hover timer task — cancelled on mouse leave or re-hover
    pub hover_timer_task: Option<Task<()>>,
    /// Drag state for sidebar pinned item reordering
    pub sidebar_pinned_drag: Option<SidebarPinnedDragState>,
    /// Whether the zoom slider is currently being dragged.
    /// Stores (initial_click_y, initial_zoom_at_start_of_drag)
    pub zoom_drag_start: Option<(f32, f32)>,
    /// Column resize drag state (for Details view column headers)
    pub column_resize: Option<ColumnResizeState>,
}

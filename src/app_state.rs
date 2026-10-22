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
use std::num::NonZeroUsize;
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

/// Disk cache filename for a path's extracted thumbnail/icon PNG.
/// Keyed by a hash of the FULL path: sanitized bare filenames collide
/// across folders (C:\a\doc.pdf vs D:\b\doc.pdf shared one PNG) and go
/// stale silently. Old un-hashed files are simply orphaned, never read.
pub fn disk_cache_key(path: &crate::core::path_util::FilesPath) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let s = path.to_string_lossy();
    let stem: String = s
        .rsplit(['\\', '/'])
        .next()
        .unwrap_or("item")
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '.' || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .take(48)
        .collect();
    let mut h = DefaultHasher::new();
    s.hash(&mut h);
    format!("{}_{:016x}", stem, h.finish())
}

/// Maximum icon temp-path entries retained in RAM (LRU-evicted beyond this).
/// Bounds the per-batch full-cache clones in `load_icons` to a small constant
/// instead of one entry per file ever seen this session.
pub const MAX_ICON_CACHE_ENTRIES: usize = 4_096;

/// Collects only the cached icon paths belonging to `dir` (children whose
/// parent is `dir`), so `MainViewState::icon_paths` stays proportional to the
/// visible directory instead of the whole session history.
pub fn icon_paths_for_dir(
    cache: &lru::LruCache<FilesPath, std::path::PathBuf>,
    dir: &std::path::Path,
) -> HashMap<FilesPath, std::path::PathBuf> {
    cache
        .iter()
        .filter(|(p, _)| p.as_path().parent() == Some(dir))
        .map(|(p, v)| (p.clone(), v.clone()))
        .collect()
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
    /// Independent state for the secondary pane in Dual-Pane split view
    pub secondary_view: MainViewState,
    /// Independent `FileItems` entity for the secondary pane's `uniform_list` closures
    pub secondary_file_items: Entity<FileItems>,
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
    /// Cache of extracted icon temp file paths, keyed by FilesPath.
    /// LRU-bounded at MAX_ICON_CACHE_ENTRIES so session-long browsing
    /// cannot grow icon RAM without limit.
    pub icon_cache: Arc<Mutex<lru::LruCache<FilesPath, std::path::PathBuf>>>,
    /// Bounded GPU image cache entity for file-view thumbnails (created in
    /// `AppWindow::new`; the file viewport is wrapped in its
    /// `ImageCacheElement` so evicted textures actually free atlas memory).
    pub image_cache: Option<Entity<crate::ui::components::bounded_image_cache::BoundedImageCache>>,
    /// Cache of tab folder thumbnail paths, keyed by FilesPath
    pub tab_thumbnail_cache: Arc<Mutex<HashMap<FilesPath, std::path::PathBuf>>>,
    /// Undo/redo manager for file operations
    pub undo_redo: UndoRedoManager,
    /// Tab drag state for reordering and cross-window operations
    pub tab_drag: TabDragState,
    /// Focus handle for the main content area (for keyboard events)
    pub main_focus: Option<gpui::FocusHandle>,
    /// Whether the window is currently maximized
    pub is_maximized: bool,
    /// Ultra-fast thumbnail cache with hash-based invalidation
    pub thumbnail_cache: crate::domain::thumbnail_cache::ThumbnailCache,
    /// Cooperative cancellation guard for in-flight directory navigation tasks (primary pane)
    pub nav_cancel_guard: crate::core::perf::cancellable_load::CancelGuard,
    /// Cooperative cancellation guard for in-flight directory navigation tasks (secondary pane)
    pub nav_cancel_guard_secondary: crate::core::perf::cancellable_load::CancelGuard,
    /// Last known live window size (for responsive tab bar metrics and page navigation)
    pub last_window_size: gpui::Size<gpui::Pixels>,
    /// Buffer for incremental type-to-select file jumping
    pub type_to_select_buffer: String,
    /// Timestamp of the last type-to-select keystroke
    pub type_to_select_last_time: Option<std::time::Instant>,
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
            main_view: MainViewState::new(root.clone(), default_view_mode.clone()),
            file_items: file_items.clone(),
            secondary_view: MainViewState::new(root.clone(), default_view_mode),
            secondary_file_items: file_items,
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
            icon_cache: Arc::new(Mutex::new(lru::LruCache::new(
                NonZeroUsize::new(MAX_ICON_CACHE_ENTRIES).unwrap(),
            ))),
            tab_thumbnail_cache: Arc::new(Mutex::new(HashMap::new())),
            image_cache: None,
            undo_redo: UndoRedoManager::new(),
            tab_drag: TabDragState::default(),
            main_focus: None,
            is_maximized: false,
            thumbnail_cache: crate::domain::thumbnail_cache::ThumbnailCache::new(),
            nav_cancel_guard: crate::core::perf::cancellable_load::CancelGuard::new(),
            nav_cancel_guard_secondary: crate::core::perf::cancellable_load::CancelGuard::new(),
            last_window_size: gpui::size(gpui::px(1280.0), gpui::px(800.0)),
            type_to_select_buffer: String::new(),
            type_to_select_last_time: None,
        }
    }

    #[inline]
    pub fn active_pane_focus(&self) -> crate::core::navigation_manager::PaneFocus {
        if matches!(
            self.main_view.pane_type,
            crate::ui::views::main_view::PaneType::Dual
        ) {
            self.main_view.active_pane
        } else {
            crate::core::navigation_manager::PaneFocus::Primary
        }
    }

    #[inline]
    pub fn pane_view(&self, focus: crate::core::navigation_manager::PaneFocus) -> &MainViewState {
        match focus {
            crate::core::navigation_manager::PaneFocus::Primary => &self.main_view,
            crate::core::navigation_manager::PaneFocus::Secondary => &self.secondary_view,
        }
    }

    #[inline]
    pub fn pane_view_mut(
        &mut self,
        focus: crate::core::navigation_manager::PaneFocus,
    ) -> &mut MainViewState {
        match focus {
            crate::core::navigation_manager::PaneFocus::Primary => &mut self.main_view,
            crate::core::navigation_manager::PaneFocus::Secondary => &mut self.secondary_view,
        }
    }

    #[inline]
    pub fn active_view(&self) -> &MainViewState {
        self.pane_view(self.active_pane_focus())
    }

    #[inline]
    pub fn active_view_mut(&mut self) -> &mut MainViewState {
        self.pane_view_mut(self.active_pane_focus())
    }

    #[inline]
    pub fn pane_file_items(
        &self,
        focus: crate::core::navigation_manager::PaneFocus,
    ) -> Entity<FileItems> {
        match focus {
            crate::core::navigation_manager::PaneFocus::Primary => self.file_items.clone(),
            crate::core::navigation_manager::PaneFocus::Secondary => {
                self.secondary_file_items.clone()
            }
        }
    }

    #[inline]
    pub fn active_file_items(&self) -> Entity<FileItems> {
        self.pane_file_items(self.active_pane_focus())
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

/// Tab drag state for reordering and cross-window operations
#[derive(Debug, Clone)]
pub struct TabDragState {
    /// Index of the tab being dragged
    pub dragged_index: Option<usize>,
    /// Current mouse position during drag
    pub current_x: f32,
    pub current_y: f32,
    /// Whether we're dragging from this window (for cross-window detection)
    pub is_dragging: bool,
    /// Whether dragging has actually started (after threshold)
    pub drag_active: bool,
    /// Whether we're dragging out of the window (to create new window)
    pub is_dragging_out: bool,
    /// Horizontal scroll offset for the tab bar (in pixels)
    pub scroll_offset: f32,
    /// Index where the tab will be dropped (based on mouse position)
    pub drop_target_index: Option<usize>,
    /// Position where mouse-down occurred (for drag threshold)
    pub drag_start_x: f32,
    pub drag_start_y: f32,
    /// When the mouse-down occurred (for 500ms hold threshold)
    pub drag_start_time: Option<std::time::Instant>,
}

impl Default for TabDragState {
    fn default() -> Self {
        Self {
            dragged_index: None,
            current_x: 0.0,
            current_y: 0.0,
            is_dragging: false,
            drag_active: false,
            is_dragging_out: false,
            scroll_offset: 0.0,
            drop_target_index: None,
            drag_start_x: 0.0,
            drag_start_y: 0.0,
            drag_start_time: None,
        }
    }
}

pub struct AppWindow {
    pub state: AppState,
    /// Index of the item currently being hovered (for hover-to-open timer)
    pub hover_item_index: Option<usize>,
    /// Hover tooltip state for file preview popup
    pub hover_tooltip: Option<HoverTooltipData>,
    /// Lightweight hover label for context-menu toolbar buttons and classic
    /// rows: (text, cursor position). Set on row hover, cleared on leave and
    /// whenever the menu closes.
    pub menu_tip: Option<(String, Point<Pixels>)>,
    /// Active hover timer task — cancelled on mouse leave or re-hover
    pub hover_timer_task: Option<Task<()>>,
    /// In-flight context-menu background work (classic items load / invoke).
    pub menu_task: Option<Task<()>>,
    /// Pending submenu grace-close timer + its generation. Dropped/cancelled
    /// whenever the cursor re-enters a row or panel.
    pub submenu_grace: Option<Task<()>>,
    pub submenu_grace_gen: u64,
    /// Pending submenu *switch* timer + generation (hover intent). While a
    /// flyout is open, moving onto a sibling row waits ~200ms before
    /// switching, so diagonal travel to the open panel isn't killed by an
    /// instant takeover. Cancelled on panel (re)entry like grace.
    pub submenu_intent: Option<Task<()>>,
    pub submenu_intent_gen: u64,
    /// Drag state for sidebar pinned item reordering
    pub sidebar_pinned_drag: Option<SidebarPinnedDragState>,
    /// Whether the zoom slider is currently being dragged.
    /// Stores (initial_click_y, initial_zoom_at_start_of_drag)
    pub zoom_drag_start: Option<(f32, f32)>,
    /// Column resize drag state (for Details view column headers)
    pub column_resize: Option<ColumnResizeState>,
    /// FPS counter state
    pub fps_last_time: Option<std::time::Instant>,
    pub fps_frame_count: u32,
    pub fps_current: f64,
    /// Previous frame instant (worst-frame / stutter tracking)
    pub fps_prev_frame: Option<std::time::Instant>,
    /// Worst single frame interval in the current fps window, in ms
    pub fps_worst_ms: f64,
    /// Frame-time tracker for frame-rate-independent scroll physics
    pub last_render_time: Option<std::time::Instant>,
    /// File system watcher for auto-refreshing the current folder (primary pane)
    pub file_watcher: Option<notify::RecommendedWatcher>,
    /// Channel receiver for file system events (primary pane)
    pub file_event_rx:
        Option<tokio::sync::mpsc::UnboundedReceiver<crate::storage::storage::FileEvent>>,
    /// File system watcher for secondary pane (split view)
    pub file_watcher_secondary: Option<notify::RecommendedWatcher>,
    /// Channel receiver for file system events (secondary pane)
    pub file_event_rx_secondary:
        Option<tokio::sync::mpsc::UnboundedReceiver<crate::storage::storage::FileEvent>>,
    /// Focus-out subscription (omnibar auto-dismiss). Held, not dropped:
    /// dropping a GPUI `Subscription` unsubscribes immediately.
    pub focus_out_subscription: Option<Subscription>,
    /// Window-bounds subscription: every resize/maximize/unmaximize must
    /// produce a fresh frame with the final geometry. Without it the window
    /// can keep showing a mid-drag frame (narrow content, missing caption
    /// buttons) until an unrelated event repaints.
    pub bounds_subscription: Option<Subscription>,
    /// Settle-guard task: re-armed on every bounds change, fires ~500ms
    /// after the LAST one (dropping it cancels the timer) to re-render +
    /// force a real OS repaint with the final geometry.
    pub settle_task: Option<Task<()>>,
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_icon_cache_lru_cap_bounds_session_growth() {
        let cache = Arc::new(Mutex::new(lru::LruCache::new(
            NonZeroUsize::new(MAX_ICON_CACHE_ENTRIES).unwrap(),
        )));
        // Simulate browsing well past capacity: 5,000 distinct files.
        {
            let mut guard = cache.lock().unwrap();
            for i in 0..5_000 {
                guard.put(
                    FilesPath::new(format!("C:/dir/file_{:05}.txt", i)),
                    std::path::PathBuf::from(format!("C:/tmp/{}.png", i)),
                );
            }
            // LRU eviction keeps RAM bounded no matter how much was seen.
            assert!(guard.len() <= MAX_ICON_CACHE_ENTRIES);
        }
    }

    #[test]
    fn test_icon_paths_for_dir_stays_within_active_directory() {
        // 10 directories x 1,000 files each, cache large enough to hold all.
        let mut cache = lru::LruCache::new(NonZeroUsize::new(20_000).unwrap());
        for d in 0..10 {
            for i in 0..1_000 {
                cache.put(
                    FilesPath::new(format!("C:/dir_{}/file_{:04}.txt", d, i)),
                    std::path::PathBuf::from(format!("C:/tmp/{}_{}.png", d, i)),
                );
            }
        }
        // Navigating across all 10 directories: each view sees at most its own 1,000.
        for d in 0..10 {
            let dir = std::path::PathBuf::from(format!("C:/dir_{}", d));
            let view = icon_paths_for_dir(&cache, &dir);
            assert!(
                view.len() <= 1_000,
                "dir {} leaked {} entries",
                d,
                view.len()
            );
            assert!(view.keys().all(|p| p.as_path().parent() == Some(dir.as_path())));
        }
    }
}

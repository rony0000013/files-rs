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
use crate::platform::windows::media_player::MfPlayer;
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
    /// Unique identifier for this window in `WindowTabRegistry`
    pub window_id: u64,
    /// Native Win32 HWND for this window (0 on non-Windows or before init)
    pub window_hwnd: isize,
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
    pub settings_ui: crate::ui::components::dialogs::SettingsUiState,
    pub is_fullscreen: bool,
    pub action_registry: crate::core::action_registry::ActionRegistry,
    pub show_shortcut_overlay: bool,
    pub session_service: crate::core::session_service::SessionService,
    pub splash_state: SplashState,
    pub thumbnail_service: Arc<ThumbnailService>,
    /// Active native video playback session (Media Foundation pump).
    /// `None` = timeline simulation (audio files, ffmpeg-backend fallback).
    pub video_player: Option<MfPlayer>,
    /// Cache of extracted icon temp file paths, keyed by FilesPath.
    /// LRU-bounded at MAX_ICON_CACHE_ENTRIES so session-long browsing
    /// cannot grow icon RAM without limit.
    pub icon_cache: Arc<Mutex<lru::LruCache<FilesPath, std::path::PathBuf>>>,
    /// Bounded GPU image cache entity for file-view thumbnails
    pub image_cache: Option<Entity<crate::ui::components::bounded_image_cache::BoundedImageCache>>,
    /// Cache of tab folder thumbnail paths, keyed by FilesPath
    pub tab_thumbnail_cache: Arc<Mutex<HashMap<FilesPath, std::path::PathBuf>>>,
    /// Undo/redo manager for file operations
    pub undo_redo: UndoRedoManager,
    /// Tab drag, scroll animation, hover preview, and cross-window docking state
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
    /// Last known window origin in screen coordinates
    pub last_window_origin: gpui::Point<gpui::Pixels>,
    /// Last known display scale factor
    pub last_scale_factor: f32,
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
        let sort_prio = config.config().view.sort_priority.as_str();
        let folders_first = sort_prio == "folders_first";
        let files_first = sort_prio == "files_first";
        let mut main_view = MainViewState::new(root.clone(), default_view_mode.clone());
        main_view.sort.folders_first = folders_first;
        main_view.sort.files_first = files_first;
        let mut secondary_view = MainViewState::new(root.clone(), default_view_mode);
        secondary_view.sort.folders_first = folders_first;
        secondary_view.sort.files_first = files_first;

        Self {
            window_id: crate::ui::components::tab_interactions::next_window_id(),
            window_hwnd: 0,
            settings,
            localization,
            navigation,
            main_view,
            file_items: file_items.clone(),
            secondary_view,
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
            settings_ui: crate::ui::components::dialogs::SettingsUiState::default(),
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
            video_player: None,
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
            last_window_origin: gpui::point(gpui::px(0.0), gpui::px(0.0)),
            last_scale_factor: 1.0,
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

/// Data for button/command tooltips shown on hover (with keyboard shortcut)
#[derive(Debug, Clone)]
pub struct ButtonTooltipData {
    pub text: String,
    pub shortcut: Option<String>,
    pub position: gpui::Point<gpui::Pixels>,
}

/// Tab drag, smooth scroll physics, hover preview, and cross-window docking state.
#[derive(Clone)]
pub struct TabDragState {
    /// Index of the tab currently being dragged
    pub dragged_index: Option<usize>,
    /// Initial index of the dragged tab when the drag began (for Escape cancel restore)
    pub initial_index: Option<usize>,
    /// Horizontal offset from the tab's left edge where the user grabbed it
    pub grab_offset_x: f32,
    /// Current mouse position during drag (in window-local logical pixels)
    pub current_x: f32,
    pub current_y: f32,
    /// Current mouse position in global screen coordinates (logical pixels)
    pub screen_x: f32,
    pub screen_y: f32,
    /// Whether a mouse-down initiated a potential drag on a tab
    pub is_dragging: bool,
    /// Whether dragging has exceeded the movement threshold (5px)
    pub drag_active: bool,
    /// Whether the tab is currently dragged outside the tab bar strip
    pub is_dragging_out: bool,
    /// Current interpolated horizontal scroll offset of the tab strip (in pixels)
    pub scroll_offset: f32,
    /// Target horizontal scroll offset of the tab strip (in pixels) for smooth lerp animation
    pub scroll_target: f32,
    /// Index where the tab will be dropped within this window
    pub drop_target_index: Option<usize>,
    /// Position where mouse-down occurred (for drag threshold)
    pub drag_start_x: f32,
    pub drag_start_y: f32,
    /// Timestamp when mouse-down occurred
    pub drag_start_time: Option<std::time::Instant>,
    /// Per-tab horizontal visual offset (in pixels) that smoothly decays to 0.0 when tabs swap slots
    pub slot_shift_offsets: Vec<f32>,
    /// Newly added/docked tab entrance animation: `(tab_index, progress 0.0..1.0)`
    pub new_tab_anim: Option<(usize, f32)>,
    /// Target window when dragging a tab from this window over another open window's tab bar
    pub cross_window_target: Option<crate::ui::components::tab_interactions::CrossWindowTabTarget>,
    /// Incoming tab preview when ANOTHER window is dragging a tab over THIS window's tab bar:
    /// `(insert_index, tab_label, local_cursor_x)`
    pub external_drop_preview: Option<(usize, String, f32)>,
    /// Index of the tab currently hovered by the pointer (for hover preview card & file-drag auto-switch)
    pub hovered_tab_index: Option<usize>,
    /// Window X coordinate of the hovered tab's left edge (for anchoring the hover preview card)
    pub hovered_tab_left_x: f32,
    /// Timestamp when the pointer entered `hovered_tab_index`
    pub hovered_tab_since: Option<std::time::Instant>,
    /// Whether the rich tab hover preview card should be displayed
    pub show_tab_hover_card: bool,
    /// Tab index currently highlighted as a drop target for dragged files/folders
    pub file_drag_over_tab: Option<usize>,
    /// Whether the `+` (New Tab) button is currently highlighted as a drop target for dragged folders
    pub file_drag_over_new_tab_btn: bool,
    /// Throttler for mouse-wheel tab switching when tabs do not overflow
    pub wheel_switch_throttle: crate::ui::components::tab_interactions::TabScrollState,
}

impl std::fmt::Debug for TabDragState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TabDragState")
            .field("dragged_index", &self.dragged_index)
            .field("is_dragging", &self.is_dragging)
            .field("drag_active", &self.drag_active)
            .field("is_dragging_out", &self.is_dragging_out)
            .field("scroll_offset", &self.scroll_offset)
            .field("scroll_target", &self.scroll_target)
            .field("drop_target_index", &self.drop_target_index)
            .finish()
    }
}

impl Default for TabDragState {
    fn default() -> Self {
        Self {
            dragged_index: None,
            initial_index: None,
            grab_offset_x: 60.0,
            current_x: 0.0,
            current_y: 0.0,
            screen_x: 0.0,
            screen_y: 0.0,
            is_dragging: false,
            drag_active: false,
            is_dragging_out: false,
            scroll_offset: 0.0,
            scroll_target: 0.0,
            drop_target_index: None,
            drag_start_x: 0.0,
            drag_start_y: 0.0,
            drag_start_time: None,
            slot_shift_offsets: Vec::new(),
            new_tab_anim: None,
            cross_window_target: None,
            external_drop_preview: None,
            hovered_tab_index: None,
            hovered_tab_left_x: 40.0,
            hovered_tab_since: None,
            show_tab_hover_card: false,
            file_drag_over_tab: None,
            file_drag_over_new_tab_btn: false,
            wheel_switch_throttle: crate::ui::components::tab_interactions::TabScrollState::new(),
        }
    }
}

impl TabDragState {
    /// Reset active drag fields while preserving smooth scroll state and animations.
    pub fn clear_drag_only(&mut self) {
        self.dragged_index = None;
        self.initial_index = None;
        self.is_dragging = false;
        self.drag_active = false;
        self.is_dragging_out = false;
        self.drop_target_index = None;
        self.drag_start_time = None;
        self.cross_window_target = None;
    }

    /// Tick smooth scroll interpolation, tab slot shift animations, and new-tab entrance animation.
    /// Returns `true` if any animation is still in flight and requires another frame.
    pub fn tick_animations(&mut self, dt: f32) -> bool {
        let mut active = false;
        let lerp_fast = 1.0 - (-22.0 * dt).exp();
        let lerp_scroll = 1.0 - (-18.0 * dt).exp();

        // 1. Smooth scroll offset -> scroll_target
        let scroll_diff = self.scroll_target - self.scroll_offset;
        if scroll_diff.abs() > 0.25 {
            self.scroll_offset += scroll_diff * lerp_scroll;
            active = true;
        } else if scroll_diff.abs() > 0.0 {
            self.scroll_offset = self.scroll_target;
        }

        // 2. Smooth neighbor slot shift offsets -> 0.0
        for shift in self.slot_shift_offsets.iter_mut() {
            if shift.abs() > 0.3 {
                *shift *= 1.0 - lerp_fast;
                active = true;
            } else {
                *shift = 0.0;
            }
        }

        // 3. New tab entrance animation -> 1.0
        if let Some((idx, mut p)) = self.new_tab_anim {
            p += dt * 7.5; // ~133ms entrance
            if p >= 1.0 {
                self.new_tab_anim = None;
            } else {
                self.new_tab_anim = Some((idx, p));
                active = true;
            }
        }

        active
    }
}

pub struct AppWindow {
    pub state: AppState,
    /// Index of the item currently being hovered (for hover-to-open timer)
    pub hover_item_index: Option<usize>,
    /// Hover tooltip state for file preview popup
    pub hover_tooltip: Option<HoverTooltipData>,
    /// Hover tooltip state for buttons and commands
    pub button_tooltip: Option<ButtonTooltipData>,
    /// Active button tooltip timer task
    pub button_tooltip_task: Option<Task<()>>,
    /// Lightweight hover label for context-menu toolbar buttons and classic
    /// rows: (text, cursor position). Set on row hover, cleared on leave and
    /// whenever the menu closes.
    pub menu_tip: Option<(String, Point<Pixels>)>,
    /// Active hover timer task — cancelled on mouse leave or re-hover
    pub hover_timer_task: Option<Task<()>>,
    /// Active tab hover timer task (for showing tab hover preview card or auto-switching on file drag)
    pub tab_hover_timer_task: Option<Task<()>>,
    /// In-flight context-menu background work (classic items load / invoke).
    pub menu_task: Option<Task<()>>,
    /// Pending submenu grace-close timer + its generation. Dropped/cancelled
    /// whenever the cursor re-enters a row or panel.
    pub submenu_grace: Option<Task<()>>,
    pub submenu_grace_gen: u64,
    /// Hover tracking to prevent race conditions during submenu travel
    pub submenu_panel_hovered: bool,
    pub submenu_row_hovered: bool,
    /// Pending submenu *switch* timer + generation (hover intent).
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
    /// Focus-out subscription (omnibar auto-dismiss).
    pub focus_out_subscription: Option<Subscription>,
    /// Window-bounds subscription
    pub bounds_subscription: Option<Subscription>,
    /// Settle-guard task
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
        {
            let mut guard = cache.lock().unwrap();
            for i in 0..5_000 {
                guard.put(
                    FilesPath::new(format!("C:/dir/file_{:05}.txt", i)),
                    std::path::PathBuf::from(format!("C:/tmp/{}.png", i)),
                );
            }
            assert!(guard.len() <= MAX_ICON_CACHE_ENTRIES);
        }
    }

    #[test]
    fn test_icon_paths_for_dir_stays_within_active_directory() {
        let mut cache = lru::LruCache::new(NonZeroUsize::new(20_000).unwrap());
        for d in 0..10 {
            for i in 0..1_000 {
                cache.put(
                    FilesPath::new(format!("C:/dir_{}/file_{:04}.txt", d, i)),
                    std::path::PathBuf::from(format!("C:/tmp/{}_{}.png", d, i)),
                );
            }
        }
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

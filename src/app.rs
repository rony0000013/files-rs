use crate::config::config::{SortBy, ViewMode};
use crate::config::settings_model::SettingsModel;
use crate::core::navigation_manager::{NavigationState, PaneLayout};
use crate::core::navigation_service::NavigationService;
use crate::core::path_util::FilesPath;
use crate::domain::extended_metadata::ExtendedMetadataExtractor;
use crate::domain::git_service::GitService;
use crate::domain::metadata::FileItem;
use crate::storage::storage::{FileType, LocalFileSystem, StorageService};
use crate::ui::components::app_background::AppBackground;
use crate::ui::components::dialogs::{DialogType, DialogsView, SettingsTab};
use crate::ui::components::navigation_bar::{NavigationBarState, NavigationBarView};
#[cfg(windows)]
use crate::ui::components::sidebar::{SidebarSplitter, SidebarView};
use crate::ui::components::sidebar_state::SidebarItemId;
use crate::ui::components::sidebar_state::SidebarState;
use crate::ui::components::splash_screen::SplashState;
use crate::ui::components::status_bar::{StatusBarState, StatusBarView};
use crate::ui::components::tab_bar::TabBarView;
use crate::ui::components::toolbar::ToolbarView;
use crate::ui::constants::UiConstants;
use crate::ui::views::main_view::{
    ColumnResizeState, ColumnWidths, DetailsPaneCache, FileItems, GroupDragState, MainView,
    MainViewState,
};
use gpui::*;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use crate::platform::windows::recycle_bin_service::RecycleBinService;
use crate::platform::windows::thumbnail_service::{ThumbnailService, ThumbnailSize};
use crate::ui::components::clipboard_manager::{ClipboardItem, ClipboardOperation, ClipboardState};

/// Directory for cached icon PNG files
fn icons_cache_dir() -> std::path::PathBuf {
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
    hover_item_index: Option<usize>,
    /// Hover tooltip state for file preview popup
    hover_tooltip: Option<HoverTooltipData>,
    /// Active hover timer task — cancelled on mouse leave or re-hover
    hover_timer_task: Option<Task<()>>,
    /// Drag state for sidebar pinned item reordering
    sidebar_pinned_drag: Option<SidebarPinnedDragState>,
    /// Whether the zoom slider is currently being dragged.
    /// Stores (initial_click_y, initial_zoom_at_start_of_drag)
    pub zoom_drag_start: Option<(f32, f32)>,
    /// Column resize drag state (for Details view column headers)
    pub column_resize: Option<ColumnResizeState>,
}

impl AppWindow {
    pub fn new(
        state: AppState,
        mut rx: tokio::sync::mpsc::UnboundedReceiver<crate::core::progress::JobUpdate>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut window_view = Self {
            state,
            hover_item_index: None,
            hover_tooltip: None,
            hover_timer_task: None,
            sidebar_pinned_drag: None,
            zoom_drag_start: None,
            column_resize: None,
        };
        let initial_path = window_view.state.main_view.primary_path.clone();
        window_view.navigate(initial_path, cx);

        // Crash detection: silently restore without showing dialog
        {
            let session = &window_view.state.session_service;
            if session.detect_crash() {
                tracing::warn!("Previous session crashed. Silently restoring...");
                // Reset crash count and start fresh
                window_view.state.session_service.start_session();
            }
        }

        // Start a new session (marks clean_shutdown=false until end_session)
        window_view.state.session_service.start_session();

        // Splash auto-dismiss timer removed (splash disabled)

        // Progress listener removed (prevents entity lease conflicts)
        // let this_weak = cx.weak_entity();
        // let mut async_cx = cx.to_async();
        // cx.foreground_executor()
        //     .spawn(async move {
        //         while let Some(update) = rx.recv().await {
        //             this_weak
        //                 .update(&mut async_cx, |this, cx| {
        //                     this.handle_job_update(update, cx);
        //                 })
        //                 .ok();
        //         }
        //     })
        //     .detach();

        window_view
    }

    fn handle_job_update(
        &mut self,
        update: crate::core::progress::JobUpdate,
        cx: &mut Context<Self>,
    ) {
        use crate::core::progress::JobUpdate;
        match update {
            JobUpdate::UpdateStatus(id, status) => {
                self.state.status_center.update_job(id, status);
            }
            JobUpdate::Complete(id) => {
                self.state.status_center.complete_job(id);
            }
            JobUpdate::Fail(id, reason) => {
                self.state.status_center.fail_job(id, reason);
            }
            JobUpdate::NewJob(job) => {
                self.state.status_center.add_job(job);
            }
        }
        cx.notify();
    }

    pub fn navigate(&mut self, path: FilesPath, cx: &mut Context<Self>) {
        // Update navigation history
        self.state.navigation.update(cx, |nav, _cx| {
            nav.tabs.active_tab_mut().navigate(path.clone());
        });

        self.state.main_view.primary_path = path.clone();
        self.state.nav_bar.update_path(path.clone());
        self.state.main_view.items.clear();
        // Clear icon_paths to avoid stale entries from previous folder during
        // the brief window before items are loaded and icon_cache is synced.
        self.state.main_view.icon_paths.clear();

        // Sync sidebar active item and status bar
        #[cfg(windows)]
        self.state.sidebar.set_active(&path);
        let folder_count = self
            .state
            .main_view
            .items
            .iter()
            .filter(|i| matches!(i.file_type, FileType::Directory))
            .count();
        let file_count = self.state.main_view.items.len() - folder_count;
        self.state
            .status_bar
            .sync(self.state.main_view.items.len(), folder_count, file_count);

        // Update git info in status bar
        self.state.status_bar.show_git_actions = GitService::is_repo(&path);
        self.state.status_bar.git_branch = GitService::get_branch(&path);

        // Early notify removed - will be called when items finish loading
        // cx.notify();

        let active_tab = self.state.navigation.read(cx).tabs.active_tab().clone();
        let settings_model = self.state.settings.read(cx);

        let prefs = settings_model.config().get_folder_preferences(&path);

        // If view lock is enabled, restore saved per-directory view mode and zoom
        if self.state.main_view.view_lock_enabled {
            self.state.main_view.view_mode = prefs.view_mode.clone();
            self.state.main_view.zoom_level = prefs.zoom_level;
        } else {
            self.state.main_view.view_mode = active_tab.view_mode.clone();
            self.state.main_view.zoom_level = settings_model.config().view.last_zoom_level;
        }

        // Load folder background from desktop.ini
        if let Some(desktop_ini) = crate::storage::desktop_ini::DesktopIni::load(path.as_path()) {
            self.state.main_view.folder_background.image_path = desktop_ini
                .background_image
                .map(|s| crate::core::path_util::FilesPath::new(s));
            if let Some(opacity) = desktop_ini.background_opacity {
                self.state.main_view.folder_background.opacity = opacity;
            }
        } else {
            self.state.main_view.folder_background =
                crate::ui::views::main_view::FolderBackground::default();
        }

        // Sync preferences to main view
        let app_prefs = &settings_model.config().preferences;
        self.state.main_view.show_dot_files = app_prefs.show_dot_files;
        self.state.main_view.show_hidden_files = app_prefs.show_hidden_files;
        self.state.main_view.show_extensions = app_prefs.show_file_extensions;
        self.state.main_view.show_system_files = app_prefs.show_system_files;

        tracing::info!(
            "Navigating to {:?}, using view mode: {:?}",
            path,
            prefs.view_mode
        );

        let path_clone = path.clone();
        let mut async_cx = cx.to_async();
        let this_weak = cx.weak_entity();
        cx.foreground_executor()
            .spawn(async move {
                tracing::info!(
                    "Progressive loading started for: {:?}",
                    path_clone
                );
                // =================================================================
                // PHASE 1: Fast initial listing (uses entry.file_type() — no stat)
                // This uses d_type from readdir, avoiding a per-file stat syscall
                // which is slow on WSL/9P. Items appear instantly with basic info;
                // full metadata (size, modified, hidden) is enriched in Phase 2.
                // =================================================================
                let fast_items: Vec<crate::domain::metadata::FileItem> = async_cx
                    .background_executor()
                    .spawn({
                        let p = path_clone.clone();
                        async move {
                            let mut items = Vec::new();
                            if let Ok(rd) = std::fs::read_dir(p.as_path()) {
                                for entry in rd.filter_map(|e| e.ok()) {
                                    let path = FilesPath::new(entry.path());
                                    let name = entry
                                        .file_name()
                                        .to_string_lossy()
                                        .into_owned();
                                    // entry.file_type() uses d_type from readdir —
                                    // no stat call, very fast
                                    let file_type = entry
                                        .file_type()
                                        .map(|t| {
                                            if t.is_dir() {
                                                FileType::Directory
                                            } else {
                                                FileType::File
                                            }
                                        })
                                        .unwrap_or(FileType::File);
                                    items.push(crate::domain::metadata::FileItem {
                                        name,
                                        path,
                                        file_type,
                                        size: 0,
                                        is_hidden: false,
                                        is_system: false,
                                        modified: None,
                                    });
                                }
                            }
                            items
                        }
                    })
                    .await;

                let total_count = fast_items.len();
                let item_paths: Vec<FilesPath> =
                    fast_items.iter().map(|i| i.path.clone()).collect();

                tracing::info!(
                    "Phase 1: {} items listed instantly from {:?}",
                    total_count,
                    path_clone
                );

                // Show items immediately so the user sees content without delay
                {
                    let p1 = path_clone.clone();
                    this_weak
                        .update(&mut async_cx, |this, cx| {
                            this.state.main_view.items = fast_items;
                            // Sync file_items model for uniform_list closures
                            this.state.file_items.update(cx, |m, _| {
                                m.items = this.state.main_view.items.clone();
                                m.selected_indices =
                                    this.state.main_view.selected_indices.clone();
                            });
                            this.state.main_view.primary_path = p1.clone();
                            this.state.nav_bar.update_path(p1.clone());
                            this.state.sidebar.set_active(&p1);
                            let folder_count = this
                                .state
                                .main_view
                                .items
                                .iter()
                                .filter(|i| matches!(i.file_type, FileType::Directory))
                                .count();
                            let file_count =
                                this.state.main_view.items.len() - folder_count;
                            this.state.status_bar.sync(
                                this.state.main_view.items.len(),
                                folder_count,
                                file_count,
                            );
                            this.state.status_bar.show_git_actions =
                                GitService::is_repo(&p1);
                            this.state.status_bar.git_branch =
                                GitService::get_branch(&p1);

                            // Sync cached icon paths so revisited folders show
                            // thumbnails instantly (Phase 3 will trigger load_icons
                            // for uncached items).
                            this.state.main_view.icon_paths = this
                                .state
                                .icon_cache
                                .lock()
                                .map(|cache| cache.clone())
                                .unwrap_or_default();
                            cx.notify();
                        })
                        .ok();
                }

                // =================================================================
                // PHASE 2: Enrich items with full metadata in background batches
                // Each batch fetches size, modified, is_hidden, is_system via stat
                // and updates the UI progressively.
                // =================================================================
                if !item_paths.is_empty() {
                    let batch_size = 100;
                    for chunk in item_paths.chunks(batch_size) {
                        let chunk = chunk.to_vec();
                        let enriched = async_cx
                            .background_executor()
                            .spawn(async move {
                                let mut results: Vec<(
                                    FilesPath,
                                    u64,
                                    Option<std::time::SystemTime>,
                                    bool,
                                    bool,
                                )> = Vec::new();
                                for path in &chunk {
                                    if let Ok(meta) = std::fs::metadata(path.as_path()) {
                                        let size = meta.len();
                                        let modified = meta.modified().ok();
                                        let mut is_hidden = false;
                                        let mut is_system = false;
                                        #[cfg(windows)]
                                        {
                                            use std::os::windows::fs::MetadataExt;
                                            let attrs = meta.file_attributes();
                                            is_hidden = (attrs
                                                & windows::Win32::Storage::FileSystem::FILE_ATTRIBUTE_HIDDEN.0)
                                                != 0;
                                            is_system = (attrs
                                                & windows::Win32::Storage::FileSystem::FILE_ATTRIBUTE_SYSTEM.0)
                                                != 0;
                                        }
                                        results.push((
                                            path.clone(),
                                            size,
                                            modified,
                                            is_hidden,
                                            is_system,
                                        ));
                                    }
                                }
                                results
                            })
                            .await;

                        if !enriched.is_empty() {
                            this_weak
                                .update(&mut async_cx, |this, cx| {
                                    for (path, size, modified, is_hidden, is_system) in
                                        enriched
                                    {
                                        if let Some(item) = this
                                            .state
                                            .main_view
                                            .items
                                            .iter_mut()
                                            .find(|i| i.path == path)
                                        {
                                            item.size = size;
                                            item.modified = modified;
                                            item.is_hidden = is_hidden;
                                            item.is_system = is_system;
                                        }
                                    }
                                    this.state.file_items.update(cx, |m, _| {
                                        m.items = this.state.main_view.items.clone();
                                    });
                                    cx.notify();
                                })
                                .ok();
                        }
                    }
                }

                // =================================================================
                // PHASE 3: Start icon extraction for all items
                // =================================================================
                {
                    let p3 = path_clone.clone();
                    this_weak
                        .update(&mut async_cx, |this, cx| {
                            tracing::info!(
                                "Phase 3: Starting icon extraction for {} items in {:?}",
                                this.state.main_view.items.len(),
                                p3
                            );
                            this.load_icons(cx);
                            // Re-sync icon cache (may have been populated by load_icons)
                            this.state.main_view.icon_paths = this
                                .state
                                .icon_cache
                                .lock()
                                .map(|cache| cache.clone())
                                .unwrap_or_default();
                            cx.notify();
                        })
                        .ok();
                }
            })
            .detach();

        // Update Jump List (recent folders)
        let path_str = path.to_string_lossy().into_owned();
        if !path_str.is_empty() {
            tracing::info!("Adding {} to jump list", path_str);
            // In a real app, we'd call WindowsJumpListService::add_recent(path_str)
        }

        // Check if we should switch to Home or Recycle Bin view
        let path_str = path.to_string_lossy();
        if path_str.is_empty() || path_str == "Home" {
            self.state.main_view.target = crate::ui::views::main_view::ViewTarget::Home;
            // Load tags for Home page
            if let Ok(tags) = self.state.tag_service.get_all_tags() {
                // Convert (name, count) to (name, color, count) with default color
                self.state.main_view.home_tags_cache = tags
                    .into_iter()
                    .map(|(name, count)| (name, "#808080".to_string(), count))
                    .collect();
            } else {
                self.state.main_view.home_tags_cache = Vec::new();
            }
        } else if path_str == "shell:RecycleBinFolder" || path_str.contains("RecycleBin") {
            self.state.main_view.target = crate::ui::views::main_view::ViewTarget::RecycleBin;
            // Load recycle bin items synchronously
            let service = RecycleBinService::new();
            if let Ok(items) = service.list_contents_sync() {
                self.state.main_view.recycle_bin_items = items;
            } else {
                self.state.main_view.recycle_bin_items = Vec::new();
            }
        } else {
            self.state.main_view.target =
                crate::ui::views::main_view::ViewTarget::Directory(path.clone());
        }

        self.save_session(cx);

        // Save session path for crash recovery
        self.save_session_path(cx);
    }

    pub fn save_session(&self, cx: &mut Context<Self>) {
        let nav = self.state.navigation.read(cx);
        let settings = self.state.settings.clone();

        let open_tabs: Vec<crate::config::config::TabSession> = nav
            .tabs
            .tabs
            .iter()
            .map(|t| crate::config::config::TabSession {
                path: t.path.to_string_lossy().into_owned(),
                view_mode: t.view_mode.clone(),
            })
            .collect();

        let active_index = nav.tabs.active_tab_index;

        settings.update(cx, move |s, _cx| {
            s.config.session.open_tabs = open_tabs;
            s.config.session.active_tab_index = active_index;
            let _ = s.config.save();
        });
    }

    /// Save the current active path to the session service for crash recovery.
    pub fn save_session_path(&mut self, cx: &mut Context<Self>) {
        let path = self.state.navigation.read(cx).active_pane_path();
        self.state
            .session_service
            .set_last_path(path.to_string_lossy().as_ref());
    }

    /// Open the documentation URL in the default browser.
    pub fn open_help(&self, _cx: &mut Context<Self>) {
        let url = "https://hermes-agent.nousresearch.com/docs";
        tracing::info!("Opening help URL: {}", url);
        #[cfg(windows)]
        {
            let _ = std::process::Command::new("cmd.exe")
                .args(["/C", "start", "", url])
                .spawn();
        }
        #[cfg(not(windows))]
        {
            let _ = std::process::Command::new("xdg-open").arg(url).spawn();
        }
    }

    /// Restore the last session path from the session service.
    pub fn restore_last_session(&mut self, cx: &mut Context<Self>) {
        let last_path = self.state.session_service.last_path().to_string();
        if !last_path.is_empty() {
            tracing::info!("Restoring last session path: {}", last_path);
            let path = crate::core::path_util::FilesPath::new(&last_path);
            self.navigate(path, cx);
        } else {
            tracing::info!("No last session path to restore.");
        }
    }

    // ---- Tab Operations ----

    pub fn new_tab(&mut self, cx: &mut Context<Self>) {
        let current_path = self.state.main_view.primary_path.clone();
        let mode = self.state.main_view.view_mode.clone();
        self.state.navigation.update(cx, |nav, _cx| {
            nav.tabs.add_tab(current_path, mode);
        });
        self.save_session(cx);
        cx.notify();
    }

    pub fn close_tab(&mut self, index: usize, cx: &mut Context<Self>) {
        self.state.navigation.update(cx, |nav, _cx| {
            nav.tabs.close_tab(index);
        });
        self.save_session(cx);
        let new_path = self
            .state
            .navigation
            .read(cx)
            .tabs
            .active_tab()
            .path
            .clone();
        self.navigate(new_path, cx);
    }

    pub fn switch_tab(&mut self, index: usize, cx: &mut Context<Self>) {
        self.state.navigation.update(cx, |nav, _cx| {
            nav.tabs.switch_tab(index);
        });
        self.save_session(cx);
        let new_path = self
            .state
            .navigation
            .read(cx)
            .tabs
            .active_tab()
            .path
            .clone();
        self.navigate(new_path, cx);
    }

    pub fn toggle_dual_pane(&mut self, cx: &mut Context<Self>) {
        self.state.navigation.update(cx, |nav, _cx| {
            nav.toggle_dual_pane();
        });
        let layout = self.state.navigation.read(cx).pane_layout;
        self.state.main_view.pane_type = match layout {
            PaneLayout::Single => crate::ui::views::main_view::PaneType::Single,
            _ => crate::ui::views::main_view::PaneType::Dual,
        };
        cx.notify();
    }

    /// Show a context menu for a tab at the given screen position.
    pub fn show_tab_context_menu(
        &mut self,
        tab_index: usize,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let tab_count = self.state.navigation.read(cx).tabs.tabs.len();
        self.state.active_dialog = Some(DialogType::TabContextMenu {
            tab_index,
            position,
            tab_count,
        });
        cx.notify();
    }

    /// Close all tabs except the specified one.
    pub fn close_other_tabs(&mut self, keep_index: usize, cx: &mut Context<Self>) {
        let tab_count = self.state.navigation.read(cx).tabs.tabs.len();
        // Close tabs from end to start, skipping the keep_index
        for i in (0..tab_count).rev() {
            if i != keep_index {
                self.state.navigation.update(cx, |nav, _cx| {
                    nav.tabs.close_tab(i);
                });
            }
        }
        self.save_session(cx);
        cx.notify();
    }

    /// Close all tabs to the right of the specified one.
    pub fn close_tabs_to_right(&mut self, from_index: usize, cx: &mut Context<Self>) {
        let tab_count = self.state.navigation.read(cx).tabs.tabs.len();
        // Close tabs from end to start, only those after from_index
        for i in (from_index + 1..tab_count).rev() {
            self.state.navigation.update(cx, |nav, _cx| {
                nav.tabs.close_tab(i);
            });
        }
        self.save_session(cx);
        cx.notify();
    }

    /// Duplicate the specified tab.
    pub fn duplicate_tab(&mut self, index: usize, cx: &mut Context<Self>) {
        let tab = self.state.navigation.read(cx).tabs.tabs.get(index).cloned();
        if let Some(tab) = tab {
            self.state.navigation.update(cx, |nav, _cx| {
                nav.tabs.add_tab(tab.path.clone(), tab.view_mode.clone());
            });
            self.save_session(cx);
            cx.notify();
        }
    }

    /// Close the current tab and open a new empty tab.
    pub fn pin_tab(&mut self, _index: usize, cx: &mut Context<Self>) {
        // TODO: Implement tab pinning (keep tab open even after closing all folders)
        cx.notify();
    }

    // ---- Navigation Operations ----

    pub fn navigate_back(&mut self, cx: &mut Context<Self>) {
        let maybe_path = self
            .state
            .navigation
            .update(cx, |nav, _cx| nav.tabs.active_tab_mut().navigate_back());
        if let Some(path) = maybe_path {
            self.navigate(path, cx);
        }
    }

    pub fn navigate_forward(&mut self, cx: &mut Context<Self>) {
        let maybe_path = self
            .state
            .navigation
            .update(cx, |nav, _cx| nav.tabs.active_tab_mut().navigate_forward());
        if let Some(path) = maybe_path {
            self.navigate(path, cx);
        }
    }

    pub fn navigate_up(&mut self, cx: &mut Context<Self>) {
        let current = self.state.main_view.primary_path.clone();
        if let Ok(parent) = NavigationService::get_parent_path(&current) {
            self.navigate(parent, cx);
        }
    }

    /// Navigate to the Home view (quick access dashboard)
    pub fn navigate_home(&mut self, cx: &mut Context<Self>) {
        match crate::core::navigation_service::NavigationService::get_home_path() {
            Ok(home_path) => {
                // Update navigation history
                self.state.navigation.update(cx, |nav, _cx| {
                    nav.tabs.active_tab_mut().navigate(home_path.clone());
                });
                self.navigate(home_path, cx);
            }
            Err(_) => {
                // Fallback: try user's home directory
                if let Some(home) = dirs::home_dir() {
                    self.navigate(crate::core::path_util::FilesPath::from(home), cx);
                }
            }
        }
    }

    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        let path = self.state.main_view.primary_path.clone();
        self.navigate(path, cx);
    }

    /// Initialize a git repository in the current directory.
    pub fn git_init(&mut self, cx: &mut Context<Self>) -> anyhow::Result<()> {
        let path = self.state.main_view.primary_path.clone();
        git2::Repository::init(path.as_path())?;
        // Refresh git info in status bar
        self.state.status_bar.show_git_actions = GitService::is_repo(&path);
        self.state.status_bar.git_branch = GitService::get_branch(&path);
        cx.notify();
        Ok(())
    }

    /// Clone a git repository from the URL in the CloneRepo dialog
    pub fn git_clone(&mut self, cx: &mut Context<Self>) {
        // Check if there's an active CloneRepo dialog
        if let Some(DialogType::CloneRepo { ref mut state }) = self.state.active_dialog {
            // Validate inputs
            if let Err(e) = state.validate_url() {
                tracing::error!("Clone validation error: {}", e);
                return;
            }
            if let Err(e) = state.validate_destination() {
                tracing::error!("Clone validation error: {}", e);
                return;
            }

            let url = state.url.clone();
            let dest = state.destination_path.clone();
            let branch = state.branch.clone();
            let this_weak = cx.weak_entity();

            state.start_cloning();

            let mut async_cx = cx.to_async();
            cx.foreground_executor()
                .spawn(async move {
                    let result = if let Some(ref branch_name) = branch {
                        // Clone with specific branch using FetchOptions
                        let mut fo = git2::FetchOptions::new();
                        let mut builder = git2::build::RepoBuilder::new();
                        builder.fetch_options(fo);
                        builder.branch(branch_name);
                        builder.clone(&url, &dest)
                    } else {
                        git2::Repository::clone(&url, &dest)
                    };

                    this_weak
                        .update(&mut async_cx, |this, cx| {
                            match result {
                                Ok(_) => {
                                    tracing::info!("Successfully cloned {} to {:?}", url, dest);
                                    // Mark as complete and close dialog
                                    if let Some(DialogType::CloneRepo { ref mut state }) =
                                        this.state.active_dialog
                                    {
                                        state.complete_cloning();
                                    }
                                    this.close_dialog(cx);
                                    // Navigate to the cloned repo
                                    let path = crate::core::path_util::FilesPath::from(
                                        dest.to_str().unwrap_or(""),
                                    );
                                    this.navigate(path, cx);
                                }
                                Err(e) => {
                                    tracing::error!("Failed to clone {}: {}", url, e);
                                    // Update state with error
                                    if let Some(DialogType::CloneRepo { ref mut state }) =
                                        this.state.active_dialog
                                    {
                                        state.is_cloning = false;
                                    }
                                    cx.notify();
                                }
                            }
                        })
                        .ok();
                })
                .detach();
        }
        cx.notify();
    }

    /// Navigate to the root of the current drive/path
    pub fn navigate_to_root(&mut self, cx: &mut Context<Self>) {
        let path = self.state.main_view.primary_path.clone();
        // Get root by climbing up until we hit a drive letter or "/"
        let mut current = path.clone();
        while let Ok(parent) = NavigationService::get_parent_path(&current) {
            if parent.as_path().to_string_lossy() == current.as_path().to_string_lossy() {
                break; // Reached root
            }
            current = parent;
        }
        if current != path {
            self.navigate(current, cx);
        }
    }

    /// Open the selected item (file or folder)
    pub fn open_item(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(item) = self.state.main_view.items.get(index) {
            match item.file_type {
                FileType::Directory => {
                    // Navigate into folder
                    self.navigate(item.path.clone(), cx);
                }
                FileType::File => {
                    // Open file with default handler
                    #[cfg(windows)]
                    {
                        use std::process::Command;
                        let path_str = item.path.as_path().to_string_lossy();
                        let _ = Command::new("cmd.exe")
                            .args(["/C", "start", "", &path_str])
                            .spawn();
                    }
                }
                FileType::Symlink => {
                    // TODO: Resolve symlink and open target
                    tracing::debug!("Opening symlink: {}", item.name);
                }
            }
        }
    }

    /// Delete selected items (move to trash)
    pub fn delete_selected(&mut self, cx: &mut Context<Self>) {
        if self.state.main_view.selected_indices.is_empty() {
            return;
        }

        // Get selected items
        let items: Vec<_> = self
            .state
            .main_view
            .selected_indices
            .iter()
            .filter_map(|&i| self.state.main_view.items.get(i))
            .collect();

        if items.is_empty() {
            return;
        }

        // For now, just log the intent - recycle bin not yet implemented
        tracing::info!(
            "Delete {} items (recycle bin not yet implemented)",
            items.len()
        );

        // Clear selection after delete
        self.clear_selection(cx);
    }

    /// Permanently delete selected items (bypasses recycle bin)
    /// Uses std::fs::remove_file for files and std::fs::remove_dir_all for directories
    pub fn delete_permanently(&mut self, cx: &mut Context<Self>) {
        if self.state.main_view.selected_indices.is_empty() {
            return;
        }

        // Get selected item paths
        let paths: Vec<_> = self
            .state
            .main_view
            .selected_indices
            .iter()
            .filter_map(|&i| self.state.main_view.items.get(i))
            .map(|item| item.path.clone())
            .collect();

        if paths.is_empty() {
            return;
        }

        let mut success_count = 0;
        let mut error_count = 0;

        for path in &paths {
            let path_buf = path.as_path().to_path_buf();
            let result = if path_buf.is_dir() {
                std::fs::remove_dir_all(&path_buf)
            } else {
                std::fs::remove_file(&path_buf)
            };

            match result {
                Ok(_) => {
                    success_count += 1;
                    tracing::info!("Permanently deleted: {:?}", path_buf);
                }
                Err(e) => {
                    error_count += 1;
                    tracing::error!("Failed to delete {:?}: {}", path_buf, e);
                }
            }
        }

        tracing::info!(
            "Permanent delete complete: {} succeeded, {} failed",
            success_count,
            error_count
        );

        // Clear selection and refresh the view
        self.clear_selection(cx);
        self.refresh(cx);
        cx.notify();
    }

    pub fn start_edit_mode(&mut self, cx: &mut Context<Self>) {
        self.state.nav_bar.edit_mode = true;
        cx.notify();
    }

    /// Enter the navigation bar edit mode (alias for start_edit_mode)
    pub fn enter_edit_mode(&mut self, cx: &mut Context<Self>) {
        self.start_edit_mode(cx);
    }

    pub fn navigate_from_edit(&mut self, path: FilesPath, cx: &mut Context<Self>) {
        self.state.nav_bar.edit_mode = false;
        self.navigate(path, cx);
    }

    /// Open the sibling folders dropdown for a given parent path.
    pub fn open_sibling_dropdown(&mut self, parent_path: FilesPath, cx: &mut Context<Self>) {
        self.state.nav_bar.load_siblings(&parent_path);
        self.state.nav_bar.open_dropdown_parent = Some(parent_path.clone());
        self.state.active_dialog = Some(DialogType::SiblingDropdown { parent_path });
        cx.notify();
    }

    /// Navigate to a sibling folder from the dropdown.
    pub fn navigate_to_sibling(&mut self, sibling_path: FilesPath, cx: &mut Context<Self>) {
        self.state.active_dialog = None;
        self.navigate(sibling_path, cx);
    }

    pub fn toggle_sidebar(&mut self, cx: &mut Context<Self>) {
        self.state.sidebar_visible = !self.state.sidebar_visible;
        cx.notify();
    }

    /// Open a path in the preferred IDE (VS Code by default).
    /// Checks if a `.git` folder exists in the selected directory (or current dir if no selection).
    pub fn open_in_ide(&mut self, path: Option<FilesPath>, cx: &mut Context<Self>) {
        let dir_path = path
            .as_ref()
            .cloned()
            .unwrap_or_else(|| self.state.main_view.primary_path.clone());

        let project_path = dir_path.as_path().to_path_buf();

        // If the path is a file, use its parent directory
        let project_dir = if project_path.is_file() {
            project_path.parent().unwrap_or(&project_path).to_path_buf()
        } else {
            project_path.clone()
        };

        let has_git = project_dir.join(".git").exists();
        tracing::info!(
            "Open in IDE: {:?} (has_git: {}, is_dir: {})",
            project_dir,
            has_git,
            project_dir.is_dir()
        );

        #[cfg(windows)]
        {
            let service = crate::platform::windows::open_in_ide_service::OpenInIDEService::new();
            let path_for_ide = project_dir.clone();
            cx.background_executor()
                .spawn(async move {
                    let _result = service.open_in_vscode(&path_for_ide).await;
                    tracing::info!("Open in IDE result: {:?}", _result);
                })
                .detach();
        }

        #[cfg(not(windows))]
        {
            let _ = project_dir;
            tracing::info!("Open in IDE not implemented on this platform");
        }

        cx.notify();
    }

    pub fn toggle_group_collapse(&mut self, key: String, cx: &mut Context<Self>) {
        if self.state.main_view.collapsed_groups.contains(&key) {
            self.state.main_view.collapsed_groups.remove(&key);
        } else {
            self.state.main_view.collapsed_groups.insert(key);
        }
        cx.notify();
    }

    // === Tag operations ===

    /// Get tags for a file at the given path
    pub fn get_file_tags(&self, path: &FilesPath) -> Vec<String> {
        self.state
            .tag_service
            .get_tags(path.as_path().to_string_lossy().as_ref())
            .unwrap_or_default()
    }

    /// Get tags for currently selected items
    pub fn get_selected_items_tags(&self) -> Vec<(usize, Vec<String>)> {
        self.state
            .main_view
            .selected_indices
            .iter()
            .filter_map(|&idx| {
                self.state.main_view.items.get(idx).map(|item| {
                    let tags = self.get_file_tags(&item.path);
                    (idx, tags)
                })
            })
            .collect()
    }

    /// Get all tags with their state across selected items
    pub fn get_common_tags_for_selected(&self) -> Vec<(String, TagState)> {
        let selected_tags = self.get_selected_items_tags();
        let total_selected = selected_tags.len();
        if total_selected == 0 {
            return Vec::new();
        }

        // Collect all unique tags from tag service
        let all_tags = self.state.tag_service.get_all_tags().unwrap_or_default();
        let mut result: Vec<(String, TagState)> = Vec::new();

        for (tag_name, _count) in &all_tags {
            let mut tagged_count = 0;
            for (_idx, tags) in &selected_tags {
                if tags.contains(tag_name) {
                    tagged_count += 1;
                }
            }
            let state = if tagged_count == total_selected {
                TagState::All
            } else if tagged_count > 0 {
                TagState::Some
            } else {
                TagState::None
            };
            result.push((tag_name.clone(), state));
        }

        result
    }

    /// Add a tag to selected items
    pub fn add_tag_to_selected(&self, tag: String, cx: &mut Context<Self>) {
        for &idx in &self.state.main_view.selected_indices {
            if let Some(item) = self.state.main_view.items.get(idx) {
                if let Err(e) = self
                    .state
                    .tag_service
                    .add_tag(item.path.as_path().to_string_lossy().as_ref(), &tag)
                {
                    tracing::error!("Failed to add tag {} to {:?}: {}", tag, item.path, e);
                }
            }
        }
        cx.notify();
    }

    /// Remove a tag from selected items
    pub fn remove_tag_from_selected(&self, tag: &str, cx: &mut Context<Self>) {
        for &idx in &self.state.main_view.selected_indices {
            if let Some(item) = self.state.main_view.items.get(idx) {
                if let Err(e) = self
                    .state
                    .tag_service
                    .remove_tag(item.path.as_path().to_string_lossy().as_ref(), tag)
                {
                    tracing::error!("Failed to remove tag {} from {:?}: {}", tag, item.path, e);
                }
            }
        }
        cx.notify();
    }

    /// Add a tag to a specific file
    pub fn add_tag_to_file(&self, path: &FilesPath, tag: String, cx: &mut Context<Self>) {
        if let Err(e) = self
            .state
            .tag_service
            .add_tag(path.as_path().to_string_lossy().as_ref(), &tag)
        {
            tracing::error!("Failed to add tag {} to {:?}: {}", tag, path, e);
        }
        cx.notify();
    }

    /// Get all files with a specific tag
    pub fn get_files_with_tag(&self, tag: &str) -> Vec<String> {
        self.state
            .tag_service
            .get_files_with_tag(tag)
            .unwrap_or_default()
    }

    /// Open all files tagged with the given tag
    /// Each file is opened using the default system handler.
    pub fn open_all_tagged_items(&mut self, tag: String, cx: &mut Context<Self>) {
        let paths = self.get_files_with_tag(&tag);
        for path_str in paths {
            let path = FilesPath::new(&path_str);
            let meta = std::fs::metadata(path.as_path());
            if let Ok(m) = meta {
                if m.is_dir() {
                    // Navigate into folder
                    self.navigate(path.clone(), cx);
                } else {
                    // Open file with default handler
                    #[cfg(windows)]
                    {
                        use std::process::Command;
                        let path_s = path.as_path().to_string_lossy();
                        let _ = Command::new("cmd.exe")
                            .args(["/C", "start", "", &path_s])
                            .spawn();
                    }
                    #[cfg(not(windows))]
                    {
                        use std::process::Command;
                        let path_s = path.as_path().to_string_lossy();
                        let _ = Command::new("xdg-open").arg(path_s.as_ref()).spawn();
                    }
                }
            }
        }
    }

    pub fn cycle_group_by(&mut self, cx: &mut Context<Self>) {
        use crate::config::config::GroupColumn;
        let current = &self.state.main_view.group_by;
        let next = match current {
            GroupColumn::None => GroupColumn::Name,
            GroupColumn::Name => GroupColumn::FileType,
            GroupColumn::FileType => GroupColumn::Size,
            GroupColumn::Size => GroupColumn::SyncStatus,
            GroupColumn::SyncStatus => GroupColumn::DateModified,
            GroupColumn::DateModified => GroupColumn::Tag,
            GroupColumn::DateCreated => GroupColumn::None,
            GroupColumn::Tag => GroupColumn::None,
        };
        self.state.main_view.group_by = next;
        // Clear collapsed groups when changing grouping
        self.state.main_view.collapsed_groups.clear();
        cx.notify();
    }

    pub fn pin_folder(&mut self, path: FilesPath, cx: &mut Context<Self>) {
        let settings_entity = self.state.settings.clone();
        let entity_id = settings_entity.entity_id();
        settings_entity.update(cx, |settings, _cx| {
            settings.pin_folder(&path, _cx, entity_id);
        });
        self.state.sidebar.reload(&settings_entity.read(cx).config);
        cx.notify();
    }

    pub fn unpin_folder(&mut self, path: FilesPath, cx: &mut Context<Self>) {
        let settings_entity = self.state.settings.clone();
        let entity_id = settings_entity.entity_id();
        settings_entity.update(cx, |settings, _cx| {
            settings.unpin_folder(&path, _cx, entity_id);
        });
        self.state.sidebar.reload(&settings_entity.read(cx).config);
        cx.notify();
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Group item drag-and-drop reorder
    // ─────────────────────────────────────────────────────────────────────────

    /// Start dragging a grouped item
    pub fn group_drag_start(&mut self, source_index: usize, start_y: f32) {
        self.state.main_view.group_drag_state = GroupDragState {
            dragged_index: Some(source_index),
            source_group_key: None,
            target_group_key: None,
            insert_position: None,
            current_y: start_y,
            is_dragging: true,
        };
    }

    /// Update drag position and calculate drop target
    pub fn group_drag_update(&mut self, current_y: f32) {
        let drag = &mut self.state.main_view.group_drag_state;
        if drag.is_dragging {
            drag.current_y = current_y;
            // Calculate target position based on Y coordinate
            // This would need item heights and group header positions for proper calculation
            // For now, we estimate based on average row height
            let row_height = if self.state.main_view.compact_mode {
                28.0
            } else {
                36.0
            };
            let header_height = if self.state.main_view.compact_mode {
                24.0
            } else {
                32.0
            };
            let base_y = header_height; // Approximate first group header position
            let relative_y = current_y - base_y;

            if relative_y > 0.0 {
                let position = (relative_y / row_height).floor() as usize;
                drag.insert_position = Some(position);
            }
        }
    }

    /// Complete the drag operation and reorder items
    pub fn group_drag_end(&mut self, _cx: &mut Context<Self>) {
        let drag = &self.state.main_view.group_drag_state;
        if drag.is_dragging {
            if let (Some(source_idx), Some(target_pos)) = (drag.dragged_index, drag.insert_position)
            {
                if target_pos != source_idx {
                    // Reorder the item within its group
                    self.state
                        .main_view
                        .reorder_item_within_group(source_idx, target_pos);
                }
            }
        }
        self.state.main_view.group_drag_state.is_dragging = false;
        self.state.main_view.group_drag_state.dragged_index = None;
        self.state.main_view.group_drag_state.insert_position = None;
    }

    /// Cancel the drag operation
    pub fn group_drag_cancel(&mut self) {
        self.state.main_view.group_drag_state.is_dragging = false;
        self.state.main_view.group_drag_state.dragged_index = None;
        self.state.main_view.group_drag_state.insert_position = None;
    }

    /// Check if currently dragging a group item
    pub fn is_group_dragging(&self) -> bool {
        self.state.main_view.group_drag_state.is_dragging
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Sidebar pinned item drag-and-drop reorder
    // ─────────────────────────────────────────────────────────────────────────

    /// Start dragging a pinned sidebar item
    pub fn sidebar_drag_start(&mut self, source_index: usize, start_y: f32) {
        self.sidebar_pinned_drag = Some(SidebarPinnedDragState {
            source_index,
            current_y: start_y,
            drop_target_index: None,
            is_dragging: true,
        });
    }

    /// Update drag position and calculate drop target
    pub fn sidebar_drag_update(&mut self, current_y: f32) {
        if let Some(ref mut drag) = self.sidebar_pinned_drag {
            drag.current_y = current_y;
            // Calculate drop target based on Y position
            // This is a simplified calculation - actual implementation would use item heights
            let item_height = 36.0; // UiConstants::ITEM_HEIGHT
            let base_y = 200.0; // Approximate starting Y of pinned items section
            let relative_y = current_y - base_y;
            let target_index = (relative_y / item_height).floor() as usize;
            drag.drop_target_index = Some(target_index);
        }
    }

    /// Complete the drag operation and reorder pinned items
    pub fn sidebar_drag_end(&mut self, cx: &mut Context<Self>) {
        if let Some(drag) = self.sidebar_pinned_drag.take() {
            if drag.is_dragging {
                if let Some(target_index) = drag.drop_target_index {
                    if target_index != drag.source_index {
                        self.reorder_pinned_items(drag.source_index, target_index, cx);
                    }
                }
            }
        }
    }

    /// Cancel the drag operation
    pub fn sidebar_drag_cancel(&mut self) {
        self.sidebar_pinned_drag = None;
    }

    /// Check if currently dragging a pinned item
    pub fn is_sidebar_dragging(&self) -> bool {
        self.sidebar_pinned_drag
            .as_ref()
            .map_or(false, |d| d.is_dragging)
    }

    /// Get the current drop target index during drag
    pub fn get_sidebar_drop_target(&self) -> Option<usize> {
        self.sidebar_pinned_drag
            .as_ref()
            .and_then(|d| d.drop_target_index)
    }

    /// Reorder pinned items: move item at source_index to target_index
    fn reorder_pinned_items(
        &mut self,
        source_index: usize,
        target_index: usize,
        cx: &mut Context<Self>,
    ) {
        let settings_entity = self.state.settings.clone();
        let entity_id = settings_entity.entity_id();

        // Get current pinned locations
        let config = settings_entity.read(cx);
        let pinned_locations = config.config().sidebar.pinned_locations.clone();
        drop(config);

        if source_index >= pinned_locations.len() || target_index >= pinned_locations.len() {
            return;
        }

        // Reorder: remove from source, insert at target
        let mut new_order = pinned_locations.clone();
        let item = new_order.remove(source_index);
        let adjusted_target = if target_index > source_index {
            target_index - 1
        } else {
            target_index
        };
        new_order.insert(adjusted_target, item);

        // Save the new order using the update pattern
        settings_entity.update(cx, |settings, _cx| {
            settings.config.sidebar.pinned_locations = new_order.clone();
        });
        let _ = settings_entity.read(cx).config().save();

        // Reload sidebar to reflect changes
        self.state.sidebar.reload(&settings_entity.read(cx).config);
        cx.notify();
    }

    // ---- Toolbar Actions ----

    pub fn new_folder(&mut self, cx: &mut Context<Self>) {
        self.state.active_dialog = Some(DialogType::AddItem {
            is_folder: true,
            name: String::new(),
        });
        cx.notify();
    }

    pub fn new_file(&mut self, cx: &mut Context<Self>) {
        self.state.active_dialog = Some(DialogType::AddItem {
            is_folder: false,
            name: String::new(),
        });
        cx.notify();
    }

    pub fn rename_selected(&mut self, cx: &mut Context<Self>) {
        if let Some(&first_selected) = self.state.main_view.selected_indices.iter().next() {
            self.start_rename(first_selected, cx);
        }
    }

    pub fn share_selected(&mut self, cx: &mut Context<Self>) {
        if let Some(&idx) = self.state.main_view.selected_indices.iter().next() {
            if let Some(item) = self.state.main_view.items.get(idx) {
                let share_path = item.path.clone();
                #[cfg(windows)]
                {
                    cx.background_executor().spawn(async move {
                        let service = crate::platform::windows::share_service::ShareService::new();
                        let _ = service.share_file(0, &share_path).await;
                    }).detach();
                }
                #[cfg(not(windows))]
                {
                    let _ = share_path;
                    tracing::info!("Share not implemented on this platform");
                }
            }
        }
    }

    /// Execute bulk rename with find/replace pattern
    pub fn execute_bulk_rename(&mut self, find: &str, replace: &str, cx: &mut Context<Self>) {
        if find.is_empty() {
            return;
        }

        let indices: Vec<usize> = self
            .state
            .main_view
            .selected_indices
            .iter()
            .copied()
            .collect();
        let items: Vec<(usize, FilesPath, String)> = indices
            .iter()
            .filter_map(|&idx| {
                self.state
                    .main_view
                    .items
                    .get(idx)
                    .map(|item| (idx, item.path.clone(), item.name.clone()))
            })
            .collect();

        // Calculate renames
        let items_to_rename: Vec<(std::path::PathBuf, std::path::PathBuf)> = items
            .iter()
            .filter_map(|(_, path, name)| {
                if name.contains(find) {
                    let new_name = name.replace(find, replace);
                    let parent = path.as_path().parent()?;
                    let new_path = parent.join(&new_name);
                    Some((path.as_path().to_path_buf(), new_path))
                } else {
                    None
                }
            })
            .collect();

        if items_to_rename.is_empty() {
            tracing::info!("No items match the find pattern");
            return;
        }

        // Perform renames synchronously
        let progress_engine = self.state.progress_engine.clone();
        let total = items_to_rename.len();
        let this_weak = cx.weak_entity();
        let mut async_cx = cx.to_async();

        cx.foreground_executor()
            .spawn(async move {
                let job_id = progress_engine
                    .start_job(format!("Bulk Rename ({} files)", total))
                    .await;

                for (i, (old_path, new_path)) in items_to_rename.into_iter().enumerate() {
                    match std::fs::rename(&old_path, &new_path) {
                        Ok(_) => {
                            tracing::info!(
                                "Renamed: {} -> {}",
                                old_path.display(),
                                new_path.display()
                            );
                        }
                        Err(e) => {
                            tracing::error!("Failed to rename {}: {}", old_path.display(), e);
                        }
                    }

                    // Update progress
                    let progress = (i + 1) as f32 / total as f32;
                    progress_engine
                        .update_progress(job_id, progress, (i + 1) as u64, total as u64)
                        .await;
                }

                progress_engine.complete_job(job_id).await;

                // Refresh the view
                this_weak
                    .update(&mut async_cx, |this, cx| {
                        this.refresh(cx);
                    })
                    .ok();
            })
            .detach();

        self.close_dialog(cx);
    }

    pub fn cut_selected(&mut self, cx: &mut Context<Self>) {
        let indices: Vec<usize> = self
            .state
            .main_view
            .selected_indices
            .iter()
            .copied()
            .collect();
        let items: Vec<ClipboardItem> = indices
            .iter()
            .filter_map(|&idx| {
                self.state
                    .main_view
                    .items
                    .get(idx)
                    .map(|item| ClipboardItem::new(item.path.as_path().to_path_buf()))
            })
            .collect();

        if !items.is_empty() {
            self.state.clipboard = ClipboardState::Cut(items.clone());
            tracing::info!("Cut {} items to clipboard", items.len());
        }
        cx.notify();
    }

    pub fn copy_selected(&mut self, cx: &mut Context<Self>) {
        let indices: Vec<usize> = self
            .state
            .main_view
            .selected_indices
            .iter()
            .copied()
            .collect();
        let items: Vec<ClipboardItem> = indices
            .iter()
            .filter_map(|&idx| {
                self.state
                    .main_view
                    .items
                    .get(idx)
                    .map(|item| ClipboardItem::new(item.path.as_path().to_path_buf()))
            })
            .collect();

        if !items.is_empty() {
            self.state.clipboard = ClipboardState::Copy(items.clone());
            tracing::info!("Copied {} items to clipboard", items.len());
        }
        cx.notify();
    }

    pub fn paste(&mut self, cx: &mut Context<Self>) {
        let target_dir = self.state.main_view.primary_path.clone();
        let target_path = target_dir.as_path().to_path_buf();

        let clipboard_items = match &self.state.clipboard {
            ClipboardState::Cut(items) if !items.is_empty() => items.clone(),
            ClipboardState::Copy(items) if !items.is_empty() => items.clone(),
            _ => {
                tracing::info!("No items to paste");
                return;
            }
        };
        let op_type = match &self.state.clipboard {
            ClipboardState::Cut(_) => "Move",
            ClipboardState::Copy(_) => "Copy",
            _ => return,
        };

        let progress_engine = self.state.progress_engine.clone();
        let this_weak = cx.weak_entity();
        let mut async_cx = cx.to_async();
        let total = clipboard_items.len();
        let is_cut = matches!(&self.state.clipboard, ClipboardState::Cut(_));

        cx.foreground_executor()
            .spawn(async move {
                let job_id = progress_engine
                    .start_job(format!("{} ({} items)", op_type, total))
                    .await;

                for (i, item) in clipboard_items.iter().enumerate() {
                    let file_name = item
                        .path
                        .file_name()
                        .map(|n| std::path::Path::new(n).to_path_buf())
                        .unwrap_or_else(|| "unknown".into());
                    let dest_path = target_path.join(&file_name);

                    if is_cut {
                        match std::fs::rename(&item.path, &dest_path) {
                            Ok(_) => tracing::info!(
                                "Moved: {} -> {}",
                                item.path.display(),
                                dest_path.display()
                            ),
                            Err(e) => {
                                tracing::error!("Failed to move {}: {}", item.path.display(), e)
                            }
                        }
                    } else {
                        if let Err(e) = Self::copy_recursive(&item.path, &dest_path) {
                            tracing::error!("Failed to copy {}: {}", item.path.display(), e);
                        } else {
                            tracing::info!(
                                "Copied: {} -> {}",
                                item.path.display(),
                                dest_path.display()
                            );
                        }
                    }

                    let progress = (i + 1) as f32 / total as f32;
                    progress_engine
                        .update_progress(job_id, progress, (i + 1) as u64, total as u64)
                        .await;
                }

                progress_engine.complete_job(job_id).await;

                this_weak
                    .update(&mut async_cx, |this, cx| {
                        this.state.clipboard = ClipboardState::Empty;
                        this.refresh(cx);
                    })
                    .ok();
            })
            .detach();
    }

    /// Recursive copy utility
    fn copy_recursive(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
        if src.is_dir() {
            std::fs::create_dir_all(dst)?;
            for entry in std::fs::read_dir(src)? {
                let entry = entry?;
                let ty = entry.file_type()?;
                if ty.is_dir() {
                    Self::copy_recursive(&entry.path(), &dst.join(entry.file_name()))?;
                } else {
                    std::fs::copy(entry.path(), dst.join(entry.file_name()))?;
                }
            }
        } else {
            std::fs::copy(src, dst)?;
        }
        Ok(())
    }

    /// Sort the file list by the given column (toggles direction if same column).
    pub fn sort_by(&mut self, col: SortBy, cx: &mut Context<Self>) {
        self.state.main_view.toggle_sort(col);
        cx.notify();
    }

    /// Select a single item by its index in `main_view.items`.
    pub fn select_item(&mut self, index: usize, shift: bool, ctrl: bool, cx: &mut Context<Self>) {
        if ctrl {
            if self.state.main_view.selected_indices.contains(&index) {
                self.state.main_view.selected_indices.remove(&index);
            } else {
                self.state.main_view.selected_indices.insert(index);
            }
        } else if shift {
            if let Some(&first) = self.state.main_view.selected_indices.iter().next() {
                let start = first.min(index);
                let end = first.max(index);
                self.state.main_view.selected_indices.clear();
                for i in start..=end {
                    self.state.main_view.selected_indices.insert(i);
                }
            } else {
                self.state.main_view.selected_indices.insert(index);
            }
        } else {
            self.state.main_view.selected_indices.clear();
            self.state.main_view.selected_indices.insert(index);
        }

        // Trigger preview load if single item selected
        if self.state.main_view.selected_indices.len() == 1 {
            let idx = *self.state.main_view.selected_indices.iter().next().unwrap();
            if let Some(item) = self.state.main_view.items.get(idx) {
                // Clone data we need to avoid borrow conflict
                let item_path = item.path.clone();
                let is_file = matches!(item.file_type, FileType::File);

                if is_file {
                    self.load_preview_content(item_path.clone(), cx);
                } else {
                    self.state.main_view.preview_text =
                        Some("Loading folder contents...".to_string());
                    self.load_folder_preview(item_path.clone(), cx);
                }

                // Update details pane cache (must do after releasing item borrow)
                self.update_details_cache_from_path(&item_path, cx);
            }
        } else {
            self.state.main_view.preview_text = None;
            self.state.main_view.preview_content =
                crate::ui::views::main_view::PreviewContent::default();
            self.state.main_view.details_cache = DetailsPaneCache::default();
        }

        // Update status bar with selection info
        let selected_size: u64 = self
            .state
            .main_view
            .selected_indices
            .iter()
            .filter_map(|&i| self.state.main_view.items.get(i))
            .map(|item| item.size)
            .sum();
        self.state.status_bar.selected_count = self.state.main_view.selected_indices.len();
        self.state.status_bar.selected_size_bytes = selected_size;

        // Sync file_items model for uniform_list closures
        self.state.file_items.update(cx, |m, _| {
            m.items = self.state.main_view.items.clone();
            m.selected_indices = self.state.main_view.selected_indices.clone();
        });

        cx.notify();
    }

    /// Select all items in the current view.
    pub fn select_all(&mut self, cx: &mut Context<Self>) {
        for i in 0..self.state.main_view.items.len() {
            self.state.main_view.selected_indices.insert(i);
        }
        self.state.status_bar.selected_count = self.state.main_view.selected_indices.len();
        let selected_size: u64 = self
            .state
            .main_view
            .selected_indices
            .iter()
            .filter_map(|&i| self.state.main_view.items.get(i))
            .map(|item| item.size)
            .sum();
        self.state.status_bar.selected_size_bytes = selected_size;
        cx.notify();
    }

    /// Clear all selections.
    pub fn clear_selection(&mut self, cx: &mut Context<Self>) {
        self.state.main_view.selected_indices.clear();
        self.state.status_bar.selected_count = 0;
        self.state.status_bar.selected_size_bytes = 0;
        self.state.main_view.preview_text = None;

        // Sync file_items model
        self.state.file_items.update(cx, |m, _| {
            m.selected_indices.clear();
        });

        cx.notify();
    }

    /// Invert selection (toggle selected state for all items).
    pub fn invert_selection(&mut self, cx: &mut Context<Self>) {
        for i in 0..self.state.main_view.items.len() {
            if self.state.main_view.selected_indices.contains(&i) {
                self.state.main_view.selected_indices.remove(&i);
            } else {
                self.state.main_view.selected_indices.insert(i);
            }
        }
        self.state.status_bar.selected_count = self.state.main_view.selected_indices.len();
        let selected_size: u64 = self
            .state
            .main_view
            .selected_indices
            .iter()
            .filter_map(|&i| self.state.main_view.items.get(i))
            .map(|item| item.size)
            .sum();
        self.state.status_bar.selected_size_bytes = selected_size;

        // Sync file_items model
        self.state.file_items.update(cx, |m, _| {
            m.selected_indices = self.state.main_view.selected_indices.clone();
        });

        cx.notify();
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Column resize handling (Details view column header drag-to-resize)
    // ─────────────────────────────────────────────────────────────────────────

    /// Start a column resize operation. Called from resize handle on_mouse_down.
    pub fn start_column_resize(&mut self, column: String, start_x: f32, cx: &mut Context<Self>) {
        let widths = self.state.main_view.column_widths;
        let initial_width = match column.as_str() {
            "name" => widths.name,
            "date" => widths.date,
            "type" => widths.type_col,
            "size" => widths.size,
            _ => return,
        };
        self.column_resize = Some(ColumnResizeState {
            column,
            start_x,
            initial_width,
            original_widths: widths,
        });
    }

    /// Update column width during a resize drag. Called from on_mouse_move.
    pub fn update_column_resize(&mut self, current_x: f32) {
        if let Some(ref resize) = self.column_resize.clone() {
            let delta = current_x - resize.start_x;
            let new_width = (resize.initial_width + delta).max(40.0); // min 40px
            match resize.column.as_str() {
                "name" => self.state.main_view.column_widths.name = new_width,
                "date" => self.state.main_view.column_widths.date = new_width,
                "type" => self.state.main_view.column_widths.type_col = new_width,
                "size" => self.state.main_view.column_widths.size = new_width,
                _ => {}
            }
        }
    }

    /// End a column resize operation. Called from on_mouse_up.
    pub fn end_column_resize(&mut self) {
        self.column_resize = None;
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Drag and drop handling
    // ─────────────────────────────────────────────────────────────────────────

    /// Start dragging selected items from the given position.
    pub fn start_drag(&mut self, x: f32, y: f32, cx: &mut Context<Self>) {
        let indices: Vec<usize> = self
            .state
            .main_view
            .selected_indices
            .iter()
            .cloned()
            .collect();
        if indices.is_empty() {
            return;
        }

        use gpui::Point;
        self.state.main_view.drag_start(indices, Point::new(x, y));
        cx.notify();
    }

    /// Update the current drag position. Call this on mouse move while dragging.
    /// is_copy indicates whether Ctrl modifier is held (for copy vs move).
    pub fn update_drag(&mut self, x: f32, y: f32, is_copy: bool, cx: &mut Context<Self>) {
        use gpui::Point;
        self.state.main_view.drag_update(Point::new(x, y), is_copy);
        cx.notify();
    }

    /// End the drag operation. Call this on mouse up.
    /// If a valid drop target is under the cursor, performs the operation.
    pub fn end_drag(&mut self, x: f32, y: f32, cx: &mut Context<Self>) {
        use gpui::Point;
        let point = Point::new(x, y);
        let visible = self.state.main_view.visible_items();

        // Check if we're over a valid drop target (directory)
        if let Some(target_idx) = self.state.main_view.get_drop_target(point, &visible) {
            if let Some(target_item) = self.state.main_view.items.get(target_idx) {
                let indices = self.state.main_view.drag_state.indices.clone();
                let is_copy = self.state.main_view.drag_state.is_copy;

                // Perform the move/copy operation
                for &idx in &indices {
                    if let Some(source_item) = self.state.main_view.items.get(idx) {
                        let operation = if is_copy { "copy" } else { "move" };
                        tracing::info!(
                            "Would {} {:?} to {:?}",
                            operation,
                            source_item.path,
                            target_item.path
                        );
                        // TODO: Actually perform the file operation using storage module
                    }
                }
            }
        }

        self.state.main_view.drag_end();
        cx.notify();
    }

    /// Cancel the current drag operation.
    pub fn cancel_drag(&mut self, cx: &mut Context<Self>) {
        self.state.main_view.drag_end();
        cx.notify();
    }

    /// Check if a drag operation is currently in progress.
    pub fn is_dragging(&self) -> bool {
        self.state.main_view.is_dragging()
    }

    /// Get the index of the item under the cursor for drop target highlighting.
    pub fn get_drag_drop_target(&self) -> Option<usize> {
        if !self.is_dragging() {
            return None;
        }
        let point = self.state.main_view.drag_state.current;
        let visible = self.state.main_view.visible_items();
        self.state.main_view.get_drop_target(point, &visible)
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Selection mode methods
    // ─────────────────────────────────────────────────────────────────────────

    /// Set the selection mode (Normal, Checkbox, or HoverSelect)
    pub fn set_selection_mode(
        &mut self,
        mode: crate::ui::views::main_view::SelectionMode,
        cx: &mut Context<Self>,
    ) {
        self.state.main_view.selection_mode = mode;
        cx.notify();
    }

    /// Get the current selection mode
    pub fn get_selection_mode(&self) -> crate::ui::views::main_view::SelectionMode {
        self.state.main_view.selection_mode.clone()
    }

    /// Cycle through selection modes: Normal → Checkbox → HoverSelect → Normal
    pub fn cycle_selection_mode(&mut self, cx: &mut Context<Self>) {
        use crate::ui::views::main_view::SelectionMode;
        let next = match self.state.main_view.selection_mode {
            SelectionMode::Normal => SelectionMode::Checkbox,
            SelectionMode::Checkbox => SelectionMode::HoverSelect,
            SelectionMode::HoverSelect => SelectionMode::Normal,
        };
        self.state.main_view.selection_mode = next;
        cx.notify();
    }

    /// Handle item hover for hover selection mode
    pub fn handle_item_hover(&mut self, index: usize, cx: &mut Context<Self>) {
        if matches!(
            self.state.main_view.selection_mode,
            crate::ui::views::main_view::SelectionMode::HoverSelect
        ) {
            self.state.main_view.selected_indices.clear();
            self.state.main_view.selected_indices.insert(index);
            self.state.status_bar.selected_count = 1;
            if let Some(item) = self.state.main_view.items.get(index) {
                self.state.status_bar.selected_size_bytes = item.size;
            }
            cx.notify();
        }
    }

    /// Toggle checkbox selection for an item
    pub fn toggle_checkbox_selection(&mut self, index: usize, cx: &mut Context<Self>) {
        if matches!(
            self.state.main_view.selection_mode,
            crate::ui::views::main_view::SelectionMode::Checkbox
        ) {
            if self.state.main_view.selected_indices.contains(&index) {
                self.state.main_view.selected_indices.remove(&index);
            } else {
                self.state.main_view.selected_indices.insert(index);
            }
            self.state.status_bar.selected_count = self.state.main_view.selected_indices.len();
            let selected_size: u64 = self
                .state
                .main_view
                .selected_indices
                .iter()
                .filter_map(|&i| self.state.main_view.items.get(i))
                .map(|item| item.size)
                .sum();
            self.state.status_bar.selected_size_bytes = selected_size;
            cx.notify();
        }
    }

    /// Select item by index, respecting current selection mode
    pub fn select_item_mode_aware(
        &mut self,
        index: usize,
        shift: bool,
        ctrl: bool,
        cx: &mut Context<Self>,
    ) {
        // If in checkbox mode and ctrl not held, clear selection first
        if matches!(
            self.state.main_view.selection_mode,
            crate::ui::views::main_view::SelectionMode::Checkbox
        ) && !ctrl
        {
            self.state.main_view.selected_indices.clear();
        }
        self.select_item(index, shift, ctrl, cx);
    }

    /// Select an item by keyboard delta (Arrow keys + Shift, Home/End).
    /// Positive delta = forward (down), negative = backward (up).
    /// Large delta (isize::MIN/MAX) = first/last item.
    pub fn select_item_by_keyboard(&mut self, delta: isize, extend: bool, cx: &mut Context<Self>) {
        let item_count = self.state.main_view.items.len();
        if item_count == 0 {
            return;
        }

        // Determine new index based on current last_clicked_index
        let current = self
            .state
            .main_view
            .last_clicked_index
            .unwrap_or(0)
            .min(item_count.saturating_sub(1));

        let new_idx = if delta == isize::MIN {
            // Home: first item
            0
        } else if delta == isize::MAX {
            // End: last item
            item_count.saturating_sub(1)
        } else {
            (current as isize + delta).clamp(0, item_count.saturating_sub(1) as isize) as usize
        };

        self.select_item(new_idx, extend, false, cx);
        self.state.main_view.last_clicked_index = Some(new_idx);
    }

    /// Update the details pane cache with tags and extended properties from path
    fn update_details_cache_from_path(&mut self, path: &FilesPath, _cx: &mut Context<Self>) {
        let mut cache = DetailsPaneCache::default();

        // Get tags from TagService
        cache.tags = self.get_file_tags(path);

        // Get extended properties for media files
        if let Ok(ext_props) = ExtendedMetadataExtractor::extract(path) {
            // Dimensions (images/video)
            if let Some((w, h)) = ext_props.dimensions {
                cache.exif_dimensions = Some(format!("{}x{}", w, h));
            }

            // Duration (audio/video)
            if let Some(dur) = ext_props.duration {
                let total_secs = dur.as_secs();
                let mins = total_secs / 60;
                let secs = total_secs % 60;
                cache.audio_duration = Some(format!("{}:{:02}", mins, secs));
                cache.video_duration = Some(format!("{}:{:02}", mins, secs));
            }
        }

        self.state.main_view.details_cache = cache;
    }

    // ═══════════════════════════════════════════════════════════════
    // SEARCH METHODS
    // ═══════════════════════════════════════════════════════════════

    /// Start a global search with the given query
    pub fn start_search(&mut self, query: String, cx: &mut Context<Self>) {
        if query.is_empty() {
            self.clear_search(cx);
            return;
        }

        self.state.main_view.search_state.is_searching = true;
        self.state.main_view.search_state.query = query.clone();
        self.state.main_view.search_state.loading = true;

        // Perform immediate name search in current items
        let search_service = crate::domain::search::SearchService::new();
        let item_names: Vec<String> = self
            .state
            .main_view
            .items
            .iter()
            .map(|i| i.name.clone())
            .collect();
        let matching_indices = search_service.filter_items(&query, &item_names);

        self.state.main_view.search_state.results = matching_indices;
        self.state.main_view.search_state.loading = false;

        // If query contains special chars, might be a deep search
        if query.len() > 2
            && (query.contains('/') || query.contains("\\") || query.starts_with('*'))
        {
            // Trigger deep recursive search with cancellation support
            let cancel_flag = Arc::new(AtomicBool::new(false));
            self.state.main_view.search_state.cancel_token = Some(cancel_flag.clone());
            let path_clone = self.state.main_view.primary_path.clone();
            let query_clone = query.clone();
            let weak_entity = cx.weak_entity();
            let mut async_cx = cx.to_async();

            cx.foreground_executor().spawn(async move {
                let results = Self::do_deep_search(path_clone, query_clone, cancel_flag);
                weak_entity.update(&mut async_cx, |this, _cx| {
                    this.state.main_view.search_state.deep_results = results;
                    this.state.main_view.search_state.loading = false;
                });
            });
        }

        cx.notify();
    }

    /// Perform a deep recursive search in a folder (blocking, called from async context)
    fn do_deep_search(
        base_path: FilesPath,
        query: String,
        cancel_flag: Arc<AtomicBool>,
    ) -> Vec<crate::ui::views::main_view::SearchResult> {
        use crate::ui::views::main_view::SearchResult;

        let mut results = Vec::new();
        let query_lower = query.to_lowercase();

        fn is_match(name: &str, query: &str) -> bool {
            let q = query.trim_start_matches('*');
            if query.starts_with('*') && query.ends_with('*') {
                name.to_lowercase().contains(&q.to_lowercase())
            } else if query.starts_with('*') {
                name.to_lowercase().ends_with(&q.to_lowercase())
            } else if query.ends_with('*') {
                name.to_lowercase().starts_with(&q.to_lowercase())
            } else {
                name.to_lowercase().contains(&query.to_lowercase())
            }
        }

        fn scan_dir(
            dir: &std::path::Path,
            query: &str,
            results: &mut Vec<SearchResult>,
            depth: usize,
            cancel_flag: &AtomicBool,
        ) {
            if depth > 10 {
                return;
            } // Limit recursion depth
            if cancel_flag.load(Ordering::Relaxed) {
                return;
            } // Check cancellation

            let Ok(entries) = std::fs::read_dir(dir) else {
                return;
            };

            for entry in entries.flatten() {
                let Ok(metadata) = entry.metadata() else {
                    continue;
                };
                let name = entry.file_name().to_string_lossy().to_string();

                if is_match(&name, query) {
                    let path = entry.path();
                    let modified = metadata.modified().ok().and_then(|t| {
                        // Format: YYYY-MM-DD HH:MM
                        let duration = t.duration_since(std::time::UNIX_EPOCH).ok()?;
                        let secs = duration.as_secs();
                        let days_since_epoch = secs / 86400;
                        let seconds_in_day = secs % 86400;
                        let hours = seconds_in_day / 3600;
                        let minutes = (seconds_in_day % 3600) / 60;

                        // Simple year calculation
                        let mut year = 1970i64;
                        let mut remaining_days = days_since_epoch as i64;
                        while remaining_days > 365 {
                            let days_in_year =
                                if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) {
                                    366
                                } else {
                                    365
                                };
                            if remaining_days >= days_in_year {
                                remaining_days -= days_in_year;
                                year += 1;
                            } else {
                                break;
                            }
                        }

                        // Get month/day
                        let is_leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
                        let days_in_months: [u64; 12] = if is_leap {
                            [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
                        } else {
                            [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
                        };
                        let mut month = 1u64;
                        for days in days_in_months.iter() {
                            if remaining_days < *days as i64 {
                                break;
                            }
                            remaining_days -= *days as i64;
                            month += 1;
                        }
                        let day = remaining_days + 1;

                        Some(format!(
                            "{:04}-{:02}-{:02} {:02}:{:02}",
                            year, month, day, hours, minutes
                        ))
                    });
                    let is_dir = metadata.is_dir();
                    let size = if is_dir { 0 } else { metadata.len() };

                    results.push(SearchResult {
                        path: FilesPath::new(path.clone()),
                        name,
                        parent_path: dir.to_string_lossy().to_string(),
                        is_dir,
                        size,
                        modified,
                    });
                }

                // Recurse into directories
                if entry.metadata().map(|m| m.is_dir()).unwrap_or(false) {
                    if let Ok(child_dir) = entry.path().canonicalize() {
                        scan_dir(&child_dir, query, results, depth + 1, cancel_flag);
                    }
                }
            }
        }

        scan_dir(
            base_path.as_path(),
            &query_lower,
            &mut results,
            0,
            &cancel_flag,
        );
        results
    }

    /// Update the search query (called as user types)
    pub fn update_search(&mut self, query: String, cx: &mut Context<Self>) {
        self.start_search(query, cx);
    }

    /// Clear search and return to normal view
    pub fn clear_search(&mut self, cx: &mut Context<Self>) {
        // Cancel any in-flight deep search
        if let Some(token) = self.state.main_view.search_state.cancel_token.take() {
            token.store(true, Ordering::Relaxed);
        }
        self.state.main_view.search_state = crate::ui::views::main_view::SearchState::default();
        cx.notify();
    }

    /// Check if search is currently active
    pub fn is_searching(&self) -> bool {
        self.state.main_view.search_state.is_searching
    }

    // ═══════════════════════════════════════════════════════════════
    // TERMINAL METHODS
    // ═══════════════════════════════════════════════════════════════

    /// Open a terminal at the specified path
    #[cfg(windows)]
    pub fn open_terminal_here(&mut self, path: &FilesPath, cx: &mut Context<Self>) {
        use std::process::Command;

        // Read terminal preference from config, or detect if not explicitly set
        let terminal_type = {
            let settings = self.state.settings.read(cx);
            settings.config().terminal_app.clone()
        };

        let path_buf = path.as_path().to_path_buf();
        let (cmd, args) =
            crate::core::terminal_service::TerminalService::get_command(&terminal_type, &path_buf);

        let result = Command::new(&cmd).args(&args).spawn();

        match result {
            Ok(_) => {
                tracing::info!("Opened terminal ({}) at: {:?}", cmd, path);
            }
            Err(e) => {
                tracing::error!("Failed to open terminal: {}", e);
            }
        }
    }

    /// Open a terminal as administrator at the specified path
    #[cfg(windows)]
    pub fn open_terminal_as_admin(&mut self, path: &FilesPath, _cx: &mut Context<Self>) {
        use std::process::Command;

        let path_str = path.as_path().to_string_lossy();

        // Use PowerShell's Start-Process with -Verb RunAs to elevate
        let ps_command = format!(
            "Start-Process powershell.exe -ArgumentList '-NoExit','cd \\'{}\\'' -Verb RunAs",
            path_str.replace('\'', "''")
        );

        let result = Command::new("powershell")
            .args(["-Command", &ps_command])
            .spawn();

        match result {
            Ok(_) => {
                tracing::info!("Opened elevated terminal at: {}", path_str);
            }
            Err(e) => {
                tracing::error!("Failed to open elevated terminal: {}", e);
            }
        }
    }

    /// Open terminal at the current primary path
    #[cfg(windows)]
    pub fn open_terminal_at_current(&mut self, cx: &mut Context<Self>) {
        let path = self.state.main_view.primary_path.clone();
        self.open_terminal_here(&path, cx);
    }

    #[cfg(not(windows))]
    pub fn open_terminal_here(&mut self, path: &FilesPath, cx: &mut Context<Self>) {
        use std::process::Command;

        // Read terminal preference from config
        let terminal_type = {
            let settings = self.state.settings.read(cx);
            settings.config().terminal_app.clone()
        };

        let path_buf = path.as_path().to_path_buf();
        let (cmd, args) =
            crate::core::terminal_service::TerminalService::get_command(&terminal_type, &path_buf);

        let result = Command::new(&cmd).args(&args).spawn();

        match result {
            Ok(_) => {
                tracing::info!("Opened terminal ({}) at: {:?}", cmd, path);
            }
            Err(e) => {
                tracing::error!("Failed to open terminal: {}", e);
            }
        }
    }

    #[cfg(not(windows))]
    pub fn open_terminal_as_admin(&mut self, _path: &FilesPath, _cx: &mut Context<Self>) {
        tracing::warn!("Open elevated terminal not implemented for this platform");
    }

    /// Load preview content for a file based on its type.
    /// Handles text, PDF, audio/video, and archive previews.
    fn load_preview_content(&mut self, path: FilesPath, cx: &mut Context<Self>) {
        use crate::storage::archive_service::ArchiveService;
        use crate::storage::preview_service;
        use crate::ui::views::main_view::PreviewContent;

        // Detect file type from extension
        let ext = path
            .as_path()
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_lowercase())
            .unwrap_or_default();

        // Determine preview type and load appropriate content
        match ext.as_str() {
            // Markdown files
            "md" | "markdown" | "rst" => {
                self.state.main_view.preview_content = PreviewContent::Markdown;
                self.load_text_preview(path, cx);
            }

            // HTML files
            "html" | "htm" => {
                self.state.main_view.preview_content = PreviewContent::Html;
                self.load_text_preview(path, cx);
            }

            // RTF files — rich text with plain text extraction
            "rtf" => {
                self.state.main_view.preview_content = PreviewContent::RichText;
                self.load_text_preview(path, cx);
            }

            // Code files with language detection
            "rs" => {
                self.state.main_view.preview_content = PreviewContent::Code {
                    language: "rust".to_string(),
                };
                self.load_text_preview(path, cx);
            }
            "py" => {
                self.state.main_view.preview_content = PreviewContent::Code {
                    language: "python".to_string(),
                };
                self.load_text_preview(path, cx);
            }
            "js" | "jsx" => {
                self.state.main_view.preview_content = PreviewContent::Code {
                    language: "javascript".to_string(),
                };
                self.load_text_preview(path, cx);
            }
            "ts" | "tsx" => {
                self.state.main_view.preview_content = PreviewContent::Code {
                    language: "typescript".to_string(),
                };
                self.load_text_preview(path, cx);
            }
            "css" | "scss" | "sass" | "less" => {
                self.state.main_view.preview_content = PreviewContent::Code {
                    language: "css".to_string(),
                };
                self.load_text_preview(path, cx);
            }
            "json" => {
                self.state.main_view.preview_content = PreviewContent::Code {
                    language: "json".to_string(),
                };
                self.load_text_preview(path, cx);
            }
            "xml" => {
                self.state.main_view.preview_content = PreviewContent::Code {
                    language: "xml".to_string(),
                };
                self.load_text_preview(path, cx);
            }
            "yaml" | "yml" => {
                self.state.main_view.preview_content = PreviewContent::Code {
                    language: "yaml".to_string(),
                };
                self.load_text_preview(path, cx);
            }
            "toml" | "ini" | "cfg" | "conf" | "config" => {
                self.state.main_view.preview_content = PreviewContent::Code {
                    language: "toml".to_string(),
                };
                self.load_text_preview(path, cx);
            }

            // Other text-based files (code, config, etc.)
            "txt" | "log" | "java" | "c" | "cpp" | "h" | "hpp" | "cs" | "go" | "rb" | "php"
            | "swift" | "kt" | "scala" | "r" | "lua" | "perl" | "sh" | "bash" | "zsh" | "fish"
            | "ps1" | "bat" | "cmd" | "dockerfile" | "makefile" | "gitignore" | "env"
            | "properties" | "sql" | "csv" => {
                self.load_text_preview(path, cx);
                self.state.main_view.preview_content = PreviewContent::None;
            }

            // PDF files - extract metadata using lopdf
            "pdf" => {
                self.state.main_view.preview_text = Some("Loading PDF metadata...".to_string());
                self.state.main_view.preview_content =
                    PreviewContent::Pdf(crate::ui::views::main_view::PdfPreviewInfo::default());

                let mut async_cx = cx.to_async();
                let this_weak = cx.weak_entity();
                let path_clone = path.clone();
                cx.foreground_executor()
                    .spawn(async move {
                        match preview_service::extract_pdf_metadata(&path_clone) {
                            Ok(info) => {
                                let mut content_lines = vec![
                                    format!("PDF Document"),
                                    format!("Pages: {}", info.page_count),
                                ];
                                if let Some(ref title) = info.title {
                                    content_lines.push(format!("Title: {}", title));
                                }
                                if let Some(ref author) = info.author {
                                    content_lines.push(format!("Author: {}", author));
                                }
                                if let Some(ref subject) = info.subject {
                                    content_lines.push(format!("Subject: {}", subject));
                                }
                                if let Some(ref creator) = info.creator {
                                    content_lines.push(format!("Creator: {}", creator));
                                }

                                this_weak
                                    .update(&mut async_cx, |this, cx| {
                                        this.state.main_view.preview_text =
                                            Some(content_lines.join("\n"));
                                        this.state.main_view.preview_content =
                                            PreviewContent::Pdf(info);
                                        cx.notify();
                                    })
                                    .ok();
                            }
                            Err(e) => {
                                this_weak
                                    .update(&mut async_cx, |this, cx| {
                                        this.state.main_view.preview_text = Some(format!(
                                            "PDF Preview\n\nFailed to read PDF metadata: {}",
                                            e
                                        ));
                                        cx.notify();
                                    })
                                    .ok();
                            }
                        }
                    })
                    .detach();
            }

            // Image files - no text preview needed (images shown in Details pane)
            "jpg" | "jpeg" | "png" | "gif" | "bmp" | "webp" | "ico" | "tiff" | "tif" | "svg"
            | "raw" | "psd" | "ai" | "eps" | "heic" | "heif" => {
                self.state.main_view.preview_text = None;
                self.state.main_view.preview_content = PreviewContent::None;
                cx.notify();
            }

            // Audio files - extract metadata using id3
            "mp3" | "wav" | "flac" | "ogg" | "m4a" | "aac" | "wma" | "ape" | "alac" | "aiff" => {
                self.state.main_view.preview_text = Some("Loading audio metadata...".to_string());
                self.state.main_view.preview_content =
                    PreviewContent::Audio(crate::ui::views::main_view::MediaPreviewInfo::default());

                let mut async_cx = cx.to_async();
                let this_weak = cx.weak_entity();
                let path_clone = path.clone();
                let ext_clone = ext.clone();
                cx.foreground_executor()
                    .spawn(async move {
                        match preview_service::extract_audio_metadata(&path_clone, &ext_clone) {
                            Ok(info) => {
                                let mut content_lines = vec![format!("Audio File")];
                                if let Some(ref format) = info.format {
                                    content_lines.push(format!("Format: {}", format));
                                }
                                if let Some(duration) = info.duration_secs {
                                    let mins = (duration / 60.0) as u64;
                                    let secs = (duration % 60.0) as u64;
                                    content_lines.push(format!("Duration: {}:{:02}", mins, secs));
                                }
                                if let Some(bitrate) = info.bitrate_kbps {
                                    content_lines.push(format!("Bitrate: {} kbps", bitrate));
                                }
                                if let Some(ref codec) = info.codec {
                                    content_lines.push(format!("Artist: {}", codec));
                                }

                                this_weak
                                    .update(&mut async_cx, |this, cx| {
                                        this.state.main_view.preview_text =
                                            Some(content_lines.join("\n"));
                                        this.state.main_view.preview_content =
                                            PreviewContent::Audio(info);
                                        cx.notify();
                                    })
                                    .ok();
                            }
                            Err(e) => {
                                this_weak
                                    .update(&mut async_cx, |this, cx| {
                                        this.state.main_view.preview_text = Some(format!(
                                            "Audio Preview\n\nFailed to read metadata: {}",
                                            e
                                        ));
                                        cx.notify();
                                    })
                                    .ok();
                            }
                        }
                    })
                    .detach();
            }

            // Video files - extract metadata
            "mp4" | "mkv" | "avi" | "mov" | "wmv" | "webm" | "m4v" | "mpg" | "mpeg" | "3gp"
            | "flv" | "vob" | "ogv" => {
                self.state.main_view.preview_text = Some("Loading video metadata...".to_string());
                self.state.main_view.preview_content =
                    PreviewContent::Video(crate::ui::views::main_view::MediaPreviewInfo::default());

                let mut async_cx = cx.to_async();
                let this_weak = cx.weak_entity();
                let path_clone = path.clone();
                let ext_clone = ext.clone();
                cx.foreground_executor()
                    .spawn(async move {
                        match preview_service::extract_video_metadata(&path_clone, &ext_clone) {
                            Ok(info) => {
                                let mut content_lines = vec![format!("Video File")];
                                if let Some(ref format) = info.format {
                                    content_lines.push(format!("Format: {}", format));
                                }
                                if let Some(duration) = info.duration_secs {
                                    let mins = (duration / 60.0) as u64;
                                    let secs = (duration % 60.0) as u64;
                                    content_lines.push(format!("Duration: {}:{:02}", mins, secs));
                                }
                                if let Some(bitrate) = info.bitrate_kbps {
                                    content_lines
                                        .push(format!("Bitrate: {} kbps (estimated)", bitrate));
                                }
                                if let (Some(w), Some(h)) = (info.width, info.height) {
                                    content_lines.push(format!("Resolution: {}x{}", w, h));
                                }

                                this_weak
                                    .update(&mut async_cx, |this, cx| {
                                        this.state.main_view.preview_text =
                                            Some(content_lines.join("\n"));
                                        this.state.main_view.preview_content =
                                            PreviewContent::Video(info);
                                        cx.notify();
                                    })
                                    .ok();
                            }
                            Err(e) => {
                                this_weak
                                    .update(&mut async_cx, |this, cx| {
                                        this.state.main_view.preview_text = Some(format!(
                                            "Video Preview\n\nFailed to read metadata: {}",
                                            e
                                        ));
                                        cx.notify();
                                    })
                                    .ok();
                            }
                        }
                    })
                    .detach();
            }

            // Archive files - list contents
            "zip" | "rar" | "7z" | "tar" | "gz" | "bz2" | "xz" | "tgz" | "tbz2" => {
                self.load_archive_preview(path, cx);
            }

            // Shortcut files - show target info (basic stub)
            "lnk" => {
                self.state.main_view.preview_text = Some(
                "Shortcut Preview\n\nTarget: (requires shell integration)\nArguments: (requires shell integration)".to_string()
            );
                self.state.main_view.preview_content = PreviewContent::None;
                cx.notify();
            }

            // Unknown or unsupported - no preview
            _ => {
                self.state.main_view.preview_text = None;
                self.state.main_view.preview_content = PreviewContent::None;
                cx.notify();
            }
        }
    }

    /// Load text preview content for a file (existing behavior, limited to 10k chars)
    fn load_text_preview(&mut self, path: FilesPath, cx: &mut Context<Self>) {
        self.state.main_view.preview_text = Some("Loading preview...".to_string());

        let mut async_cx = cx.to_async();
        let this_weak = cx.weak_entity();
        let path_clone = path.clone();
        cx.foreground_executor()
            .spawn(async move {
                let result = std::fs::read_to_string(path_clone.as_path());
                this_weak
                    .update(&mut async_cx, |this, cx| {
                        match result {
                            Ok(text) => {
                                // Limit to 10k chars as per TODO
                                let preview = if text.len() > 10000 {
                                    format!(
                                        "{}\n\n[Preview truncated at 10,000 characters]",
                                        &text[..10000]
                                    )
                                } else {
                                    text
                                };
                                this.state.main_view.preview_text = Some(preview);
                            }
                            Err(_) => {
                                this.state.main_view.preview_text =
                                    Some("Unable to load preview.".to_string());
                            }
                        }
                        cx.notify();
                    })
                    .ok();
            })
            .detach();
    }

    /// Load archive preview - list contents without extracting
    fn load_archive_preview(&mut self, path: FilesPath, cx: &mut Context<Self>) {
        use crate::storage::archive_service::ArchiveService;
        use crate::ui::views::main_view::{ArchivePreviewInfo, PreviewContent};

        self.state.main_view.preview_content =
            PreviewContent::Archive(ArchivePreviewInfo::default());
        self.state.main_view.preview_text = Some("Loading archive contents...".to_string());

        let mut async_cx = cx.to_async();
        let this_weak = cx.weak_entity();
        let path_clone = path.clone();
        cx.foreground_executor()
            .spawn(async move {
                // Try to list archive contents
                match ArchiveService::list_contents(&path_clone) {
                    Ok(entries) => {
                        let entry_count = entries.len();
                        let display_entries: Vec<String> = entries.into_iter().take(100).collect();
                        let has_more = entry_count > 100;

                        this_weak
                            .update(&mut async_cx, |this, cx| {
                                let content = if has_more {
                                    format!(
                                        "Archive Contents ({})\n\n{}\n\n... and {} more entries",
                                        entry_count,
                                        display_entries.join("\n"),
                                        entry_count - 100
                                    )
                                } else {
                                    format!(
                                        "Archive Contents ({})\n\n{}",
                                        entry_count,
                                        display_entries.join("\n")
                                    )
                                };
                                this.state.main_view.preview_text = Some(content);
                                this.state.main_view.preview_content =
                                    PreviewContent::Archive(ArchivePreviewInfo {
                                        entry_count,
                                        entries: display_entries,
                                        uncompressed_size: None,
                                        compression_ratio: None,
                                    });
                                cx.notify();
                            })
                            .ok();
                    }
                    Err(e) => {
                        this_weak
                            .update(&mut async_cx, |this, cx| {
                                this.state.main_view.preview_text = Some(format!(
                                    "Archive Preview\n\nUnable to list archive contents:\n{}",
                                    e
                                ));
                                cx.notify();
                            })
                            .ok();
                    }
                }
            })
            .detach();
    }

    /// Check which third-party preview handler is available on this system.
    /// Returns: Peek (PowerToys), QuickLook, SeerPro, or None.
    pub fn check_third_party_preview_handler(
        &self,
    ) -> crate::ui::views::main_view::ThirdPartyHandler {
        use crate::ui::views::main_view::ThirdPartyHandler;
        use std::process::Command;

        // Check for PowerToys Peek (peek.exe)
        if Command::new("peek.exe").arg("--version").output().is_ok() {
            return ThirdPartyHandler::Peek;
        }

        // Check for QuickLook (quicklook.exe or qlmanage on macOS)
        if Command::new("quicklook.exe").arg("-v").output().is_ok() {
            return ThirdPartyHandler::QuickLook;
        }

        // Check for Seer (seer.exe)
        if Command::new("seer.exe").arg("--version").output().is_ok() {
            return ThirdPartyHandler::SeerPro;
        }

        ThirdPartyHandler::None
    }

    /// Invoke third-party preview for the currently selected file.
    /// Opens PowerToys Peek, QuickLook, or SeerPro to preview the file.
    pub fn invoke_third_party_preview(&mut self, cx: &mut Context<Self>) {
        use crate::ui::views::main_view::{
            PreviewContent, ThirdPartyHandler, ThirdPartyPreviewInfo,
        };
        use std::process::Command;

        // Get the selected file path
        let selected_path = {
            if let Some(&idx) = self.state.main_view.selected_indices.iter().next() {
                self.state
                    .main_view
                    .items
                    .get(idx)
                    .map(|item| item.path.clone())
            } else {
                None
            }
        };

        let path = match selected_path {
            Some(p) => p,
            None => {
                tracing::debug!("No file selected for third-party preview");
                return;
            }
        };

        let path_str = path.as_path().to_string_lossy().to_string();
        let handler = self.check_third_party_preview_handler();

        // Update preview content state
        self.state.main_view.preview_content = PreviewContent::ThirdParty(ThirdPartyPreviewInfo {
            handler,
            is_available: handler != ThirdPartyHandler::None,
        });

        match handler {
            ThirdPartyHandler::Peek => {
                // PowerToys Peek
                match Command::new("peek.exe").arg(&path_str).spawn() {
                    Ok(_) => {
                        tracing::info!("Invoked PowerToys Peek for: {}", path_str);
                    }
                    Err(e) => {
                        tracing::error!("Failed to invoke Peek: {}", e);
                        self.state.main_view.preview_text = Some(format!(
                            "PowerToys Peek is not available or failed to start.\n\nError: {}",
                            e
                        ));
                    }
                }
            }
            ThirdPartyHandler::QuickLook => {
                // QuickLook (Windows)
                match Command::new("quicklook.exe").arg(&path_str).spawn() {
                    Ok(_) => {
                        tracing::info!("Invoked QuickLook for: {}", path_str);
                    }
                    Err(e) => {
                        tracing::error!("Failed to invoke QuickLook: {}", e);
                        self.state.main_view.preview_text = Some(format!(
                            "QuickLook is not available or failed to start.\n\nError: {}",
                            e
                        ));
                    }
                }
            }
            ThirdPartyHandler::SeerPro => {
                // SeerPro
                match Command::new("seer.exe").arg(&path_str).spawn() {
                    Ok(_) => {
                        tracing::info!("Invoked SeerPro for: {}", path_str);
                    }
                    Err(e) => {
                        tracing::error!("Failed to invoke SeerPro: {}", e);
                        self.state.main_view.preview_text = Some(format!(
                            "SeerPro is not available or failed to start.\n\nError: {}",
                            e
                        ));
                    }
                }
            }
            ThirdPartyHandler::None => {
                // Fall back to built-in shell preview for text-based files
                let ext = path
                    .as_path()
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_lowercase();

                match ext.as_str() {
                    "txt" | "md" | "markdown" | "csv" | "json" | "xml" | "rtf" | "log" | "html"
                    | "htm" => {
                        // Load text preview using the shell preview service
                        self.state.main_view.preview_text =
                            Some("Loading shell preview...".to_string());
                        let mut async_cx = cx.to_async();
                        let this_weak = cx.weak_entity();
                        let path_clone = path.clone();
                        cx.foreground_executor()
                            .spawn(async move {
                                let service = crate::platform::windows::shell_preview_service::ShellPreviewService::new();
                                let result = service.get_preview_via_shell(&path_clone).await;
                                this_weak
                                    .update(&mut async_cx, |this, cx| {
                                        match result {
                                            Ok(text) => {
                                                let preview = if text.len() > 10000 {
                                                    format!("{}\n\n[Preview truncated at 10,000 characters]", &text[..10000])
                                                } else {
                                                    text
                                                };
                                                this.state.main_view.preview_text = Some(preview);
                                            }
                                            Err(e) => {
                                                this.state.main_view.preview_text = Some(
                                                    format!("Shell Preview Error\n\n{}", e)
                                                );
                                            }
                                        }
                                        cx.notify();
                                    })
                                    .ok();
                            })
                            .detach();
                    }
                    "pdf" | "doc" | "docx" | "xls" | "xlsx" | "ppt" | "pptx" => {
                        // Log that shell preview would be used
                        tracing::info!(
                            "Shell preview (IPreviewHandler) would be used for .{} file: {}",
                            ext,
                            path_str
                        );
                        self.state.main_view.preview_text = Some(
                            format!(
                                "Shell Preview (IPreviewHandler)\n\nFile type: .{}\n\nThe Windows IPreviewHandler COM interface would render this file, but the implementation is in progress.\n\nThis message will be replaced with the actual preview once the COM interop is complete.",
                                ext
                            )
                        );
                    }
                    _ => {
                        self.state.main_view.preview_text = Some(
                            "Third-Party Preview\n\nNo preview handler detected.\n\nInstall one of:\n  - PowerToys Peek (Microsoft PowerToys)\n  - QuickLook (for Windows)\n  - SeerPro\n\nto enable Spacebar preview.".to_string()
                        );
                    }
                }
            }
        }

        cx.notify();
    }

    /// Toggle the Preview pane.
    pub fn toggle_preview_pane(&mut self, cx: &mut Context<Self>) {
        use crate::ui::views::main_view::SidePaneMode;
        self.state.main_view.side_pane = match self.state.main_view.side_pane {
            SidePaneMode::Preview => SidePaneMode::None,
            _ => SidePaneMode::Preview,
        };
        cx.notify();
    }

    /// Toggle the Info/Details pane.
    pub fn toggle_info_pane(&mut self, cx: &mut Context<Self>) {
        use crate::ui::views::main_view::SidePaneMode;
        self.state.main_view.side_pane = match self.state.main_view.side_pane {
            SidePaneMode::Info => SidePaneMode::None,
            _ => SidePaneMode::Info,
        };
        cx.notify();
    }

    pub fn toggle_hidden_files(&mut self, cx: &mut Context<Self>) {
        let settings = self.state.settings.read(cx);
        let current = settings.show_hidden_files();
        drop(settings);
        self.state.settings.update(cx, move |s, _cx| {
            s.config.preferences.show_hidden_files = !current;
            if let Err(e) = s.config.save() {
                tracing::error!("Failed to save settings: {:?}", e);
            }
        });
        self.state.main_view.show_hidden_files = !current;
        cx.notify();
    }

    pub fn toggle_dot_files(&mut self, cx: &mut Context<Self>) {
        let settings = self.state.settings.read(cx);
        let current = settings.show_dot_files();
        drop(settings);
        self.state.settings.update(cx, move |s, _cx| {
            s.config.preferences.show_dot_files = !current;
            if let Err(e) = s.config.save() {
                tracing::error!("Failed to save settings: {:?}", e);
            }
        });
        self.state.main_view.show_dot_files = !current;
        cx.notify();
    }

    pub fn toggle_file_extensions(&mut self, cx: &mut Context<Self>) {
        let settings = self.state.settings.read(cx);
        let current = settings.show_file_extensions();
        drop(settings);
        self.state.settings.update(cx, move |s, _cx| {
            s.config.preferences.show_file_extensions = !current;
            if let Err(e) = s.config.save() {
                tracing::error!("Failed to save settings: {:?}", e);
            }
        });
        self.state.main_view.show_extensions = !current;
        cx.notify();
    }

    pub fn close_dialog(&mut self, cx: &mut Context<Self>) {
        self.state.active_dialog = None;
        cx.notify();
    }

    pub fn confirm_add_item(&mut self, cx: &mut Context<Self>) {
        if let Some(DialogType::AddItem { is_folder, name }) = self.state.active_dialog.clone() {
            let trimmed = name.trim();
            if !trimmed.is_empty() {
                let mut target_path = self.state.main_view.primary_path.as_path().to_path_buf();
                target_path.push(trimmed);
                if is_folder {
                    let _ = std::fs::create_dir_all(&target_path);
                } else {
                    let _ = std::fs::write(&target_path, b"");
                }
                // Refresh directory
                let current_path = self.state.main_view.primary_path.clone();
                self.navigate(current_path, cx);
            }
        }
        self.state.active_dialog = None;
        cx.notify();
    }

    pub fn open_zoom_slider(&mut self, position: gpui::Point<Pixels>, cx: &mut Context<Self>) {
        use crate::ui::components::dialogs::DialogType;
        // Reset any stale drag state from a previous incomplete drag
        self.zoom_drag_start = None;
        let current_zoom = self.state.main_view.zoom_level;
        let current_mode = self.state.main_view.view_mode.clone();
        self.state.active_dialog = Some(DialogType::ZoomSlider {
            position,
            current_zoom,
            current_mode,
        });
        cx.notify();
    }

    pub fn end_zoom_drag(&mut self, _cx: &mut Context<Self>) {
        self.zoom_drag_start = None;
    }

    pub fn set_zoom_level(&mut self, level: f32, cx: &mut Context<Self>) {
        let level = level.clamp(0.0, 1.0);
        self.state.main_view.zoom_level = level;

        // Keep dialog snapshots in sync so re-renders use live values
        if let Some(crate::ui::components::dialogs::DialogType::ZoomSlider {
            ref mut current_zoom,
            ..
        }) = self.state.active_dialog
        {
            *current_zoom = level;
        }

        // Save last used zoom (always) + per-directory preferences if lock is enabled
        let path = self.state.main_view.primary_path.clone();
        let view_mode = self.state.main_view.view_mode.clone();
        let view_lock = self.state.main_view.view_lock_enabled;
        let settings = self.state.settings.clone();
        settings.update(cx, move |s, _cx| {
            // Always persist as the global "last used" zoom level
            s.config.view.last_zoom_level = level;

            // Save to per-directory preferences if lock is enabled
            if view_lock {
                s.config.set_folder_preferences(
                    &path,
                    crate::config::config::FolderPreferences {
                        view_mode,
                        zoom_level: level,
                        sort_by: s.config.view.sort_by.clone(),
                        sort_ascending: s.config.view.sort_ascending,
                        folder_on_top: s.config.view.folder_on_top,
                        group_by: s.config.view.group_by,
                    },
                );
            }
            let _ = s.config.save();
        });

        cx.notify();
    }

    pub fn dismiss_splash(&mut self, cx: &mut Context<Self>) {
        self.state.splash_state.visible = false;
        cx.notify();
    }

    pub fn toggle_shelf_pane(&mut self, cx: &mut Context<Self>) {
        self.state.main_view.shelf_visible = !self.state.main_view.shelf_visible;
        cx.notify();
    }

    /// Load Windows Shell icons for all items in the current directory.
    /// Extracts icons via ThumbnailService, saves as PNG, caches paths.
    pub fn load_icons(&mut self, cx: &mut Context<Self>) {
        #[cfg(windows)]
        {
            use crate::platform::windows::thumbnail_service::ThumbnailSize;
            use std::path::PathBuf;

            // Determine which items are visible in the viewport so we prioritize them
            let visible_start = self
                .state
                .main_view
                .uniform_list_scroll_handle
                .0
                .borrow()
                .base_handle
                .top_item();
            let visible_end = self
                .state
                .main_view
                .uniform_list_scroll_handle
                .0
                .borrow()
                .base_handle
                .bottom_item();

            let icon_dir = icons_cache_dir();
            let items = self.state.main_view.items.clone();
            let thumbnail_service = self.state.thumbnail_service.clone();
            let icon_cache = self.state.icon_cache.clone();
            let weak_entity = cx.weak_entity();
            let mut async_cx = cx.to_async();

            // Build a priority-ordered item list: visible items first, then the rest.
            // Process ALL items so the persistent disk cache is fully populated —
            // subsequent navigations to the same folder load cached thumbnails instantly.
            let total = items.len();
            let mut priority_items: Vec<usize> = (0..total).collect();
            if visible_end > visible_start && visible_start < total {
                // Rotate so visible range comes first
                let end = visible_end.min(total);
                let start = visible_start.min(total);
                let visible_range: Vec<usize> = (start..end).collect();
                let others: Vec<usize> = (0..start)
                    .chain(end..total)
                    .filter(|i| *i < total)
                    .collect();
                priority_items = visible_range.into_iter().chain(others).collect();
            }

            let batch_size = 25;

            // Use foreground executor (no Send requirement) that delegates blocking
            // extraction work to the background executor batch by batch.
            cx.foreground_executor()
                .spawn(async move {
                    for chunk in priority_items.chunks(batch_size) {
                        let chunk_vec = chunk.to_vec();
                        // Clone only Send types for the background task
                        let items_clone = items.clone();
                        let thumb_clone = thumbnail_service.clone();
                        let cache_clone = icon_cache.clone();
                        let dir_clone = icon_dir.clone();
                        let size = ThumbnailSize::Medium;

                        // Blocking thumbnail extraction runs on background thread pool
                        let batch = async_cx
                            .background_executor()
                            .spawn(async move {
                                let _com_init = unsafe {
                                    windows::Win32::System::Com::CoInitializeEx(
                                        None,
                                        windows::Win32::System::Com::COINIT_APARTMENTTHREADED,
                                    )
                                };

                                let mut batch_extracted = false;
                                for &idx in &chunk_vec {
                                    if idx >= items_clone.len() {
                                        continue;
                                    }
                                    let item = &items_clone[idx];
                                    let path = &item.path;

                                    {
                                        if let Ok(cache) = cache_clone.lock() {
                                            if cache.contains_key(path) {
                                                continue;
                                            }
                                        }
                                        // Check persistent disk cache — avoids expensive COM
                                        // re-extraction for items already processed in a
                                        // previous session. Populates in-memory cache so
                                        // thumbnails appear instantly on fresh start.
                                        let sanitized = path.to_string_lossy().replace(
                                            |c: char| !c.is_alphanumeric()
                                                && c != '.'
                                                && c != '_',
                                            "_",
                                        );
                                        let cached_png =
                                            dir_clone.join(format!("{}.png", sanitized));
                                        if cached_png.exists() {
                                            if let Ok(mut cache) = cache_clone.lock() {
                                                cache.insert(path.clone(), cached_png);
                                                batch_extracted = true;
                                            }
                                            continue;
                                        }
                                    }

                                    if let Ok(thumb) =
                                        thumb_clone.extract_thumbnail(path, size)
                                    {
                                        use image::{ImageBuffer, Rgba};
                                        let w = thumb.width;
                                        let h = thumb.height;
                                        if w > 0
                                            && h > 0
                                            && thumb.data.len() as u32 >= w * h * 4
                                        {
                                            let mut rgba_data = thumb.data.clone();
                                            for ch in rgba_data.chunks_exact_mut(4) {
                                                ch.swap(0, 2);
                                            }
                                            if let Some(img) =
                                                ImageBuffer::<Rgba<u8>, _>::from_raw(
                                                    w, h, rgba_data,
                                                )
                                            {
                                                let sanitized =
                                                    path.to_string_lossy().replace(
                                                        |c: char| {
                                                            !c.is_alphanumeric()
                                                                && c != '.'
                                                                && c != '_'
                                                        },
                                                        "_",
                                                    );
                                                let cache_path =
                                                    dir_clone.join(format!(
                                                        "{}.png",
                                                        sanitized
                                                    ));
                                                if img.save(&cache_path).is_ok() {
                                                    if let Ok(mut cache) =
                                                        cache_clone.lock()
                                                    {
                                                        cache.insert(
                                                            path.clone(),
                                                            cache_path,
                                                        );
                                                        batch_extracted = true;
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }

                                unsafe {
                                    windows::Win32::System::Com::CoUninitialize();
                                }

                                batch_extracted
                            })
                            .await;

                        // Back on foreground — update UI with extracted batch
                        if batch {
                            let _ = weak_entity.update(&mut async_cx, |this, cx| {
                                this.state.main_view.icon_paths = this
                                    .state
                                    .icon_cache
                                    .lock()
                                    .map(|cache| cache.clone())
                                    .unwrap_or_default();
                                cx.notify();
                            });
                        }
                    }
                })
                .detach();
        }
        #[cfg(not(windows))]
        {
            let _ = cx;
        }
    }

    /// Check if visible viewport items lack icons, and if so, trigger
    /// icon loading for them. Safe to call repeatedly — already-cached
    /// icons are skipped.
    pub fn refresh_icons_for_viewport(&mut self, cx: &mut Context<Self>) {
        let (top, bottom) = {
            let scroll = &self.state.main_view.uniform_list_scroll_handle;
            let base = &scroll.0.borrow().base_handle;
            (base.top_item(), base.bottom_item())
        };
        let items = &self.state.main_view.items;
        let icon_paths = &self.state.main_view.icon_paths;
        let limit = bottom.min(items.len().saturating_sub(1));

        // Check if any visible items in the viewport still lack icons
        let needs_reload = (top..=limit).any(|i| {
            if i >= items.len() {
                return false;
            }
            !icon_paths.contains_key(&items[i].path)
        });

        if needs_reload {
            tracing::debug!(
                "Viewport icons stale (viewport items {}..{}, total items {}), reloading",
                top,
                limit,
                items.len()
            );
            self.load_icons(cx);
        }
    }

    pub fn toggle_details_pane(&mut self, cx: &mut Context<Self>) {
        use crate::ui::views::main_view::SidePaneMode;
        self.state.main_view.side_pane = match self.state.main_view.side_pane {
            SidePaneMode::Details => SidePaneMode::None,
            _ => SidePaneMode::Details,
        };
        cx.notify();
    }

    pub fn toggle_system_files(&mut self, cx: &mut Context<Self>) {
        let settings = self.state.settings.read(cx);
        let current = settings.show_system_files();
        drop(settings);
        self.state.settings.update(cx, move |s, _cx| {
            s.config.preferences.show_system_files = !current;
            if let Err(e) = s.config.save() {
                tracing::error!("Failed to save settings: {:?}", e);
            }
        });
        self.state.main_view.show_system_files = !current;
        cx.notify();
    }

    pub fn toggle_split_direction(&mut self, cx: &mut Context<Self>) {
        use crate::ui::views::main_view::DualPaneSplit;
        self.state.main_view.dual_pane_split = match self.state.main_view.dual_pane_split {
            DualPaneSplit::Horizontal => DualPaneSplit::Vertical,
            DualPaneSplit::Vertical => DualPaneSplit::Horizontal,
        };
        cx.notify();
    }

    pub fn add_selected_to_shelf(&mut self, cx: &mut Context<Self>) {
        let items = &self.state.main_view.items;
        let shelf_items = &mut self.state.main_view.shelf_items;
        for &idx in &self.state.main_view.selected_indices {
            if let Some(item) = items.get(idx) {
                // Avoid duplicates
                if !shelf_items.iter().any(|s| s.path == item.path) {
                    shelf_items.push(crate::ui::views::main_view::ShelfItem {
                        path: item.path.clone(),
                        name: item.name.clone(),
                        is_dir: matches!(
                            item.file_type,
                            crate::storage::storage::FileType::Directory
                        ),
                        size: item.size,
                    });
                }
            }
        }
        cx.notify();
    }

    pub fn clear_shelf(&mut self, cx: &mut Context<Self>) {
        self.state.main_view.shelf_items.clear();
        cx.notify();
    }

    pub fn remove_from_shelf(&mut self, path: FilesPath, cx: &mut Context<Self>) {
        self.state.main_view.shelf_items.retain(|i| i.path != path);
        cx.notify();
    }

    pub fn open_command_palette(&mut self, cx: &mut Context<Self>) {
        self.state.active_dialog = Some(DialogType::CommandPalette);
        cx.notify();
    }

    pub fn new_window(&mut self, cx: &mut Context<Self>) {
        let progress_engine = self.state.progress_engine.clone();
        // Create a new database for the new window (shared same file)
        let db_path = dirs::data_local_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("Files-Rust")
            .join("files-rs.db");
        let db = Arc::new(
            crate::core::database::Database::new(db_path).unwrap_or_else(|_| {
                crate::core::database::Database::new(std::path::PathBuf::from(":memory:"))
                    .expect("Failed to create in-memory database")
            }),
        );
        let fi_res = cx.reserve_entity();
        let file_items = cx.insert_entity(fi_res, |_| {
            crate::ui::views::main_view::FileItems::default()
        });
        let app_state = AppState::new(
            self.state.main_view.primary_path.clone(),
            file_items,
            self.state.settings.clone(),
            self.state.localization.clone(),
            self.state.navigation.clone(),
            progress_engine.clone(),
            db,
            cx,
        );

        let options = gpui::WindowOptions {
            window_bounds: Some(gpui::WindowBounds::Windowed(gpui::Bounds {
                origin: Default::default(),
                size: gpui::size(gpui::Pixels::from(1280.0), gpui::Pixels::from(800.0)),
            })),
            titlebar: None,
            window_background: gpui::WindowBackgroundAppearance::Opaque,
            ..Default::default()
        };

        cx.open_window(options, move |window, cx| {
            let rx = progress_engine.subscribe();
            cx.new(|cx| AppWindow::new(app_state, rx, window, cx))
        })
        .expect("Failed to open window");
    }

    pub fn toggle_fullscreen(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.state.is_fullscreen = !self.state.is_fullscreen;
        // TODO: Fix fullscreen toggle for GPUI 0.2.x
        // window.set_fullscreen(self.state.is_fullscreen);
        cx.notify();
    }

    pub fn open_in_new_tab(&mut self, path: FilesPath, cx: &mut Context<Self>) {
        let mode = self.state.main_view.view_mode.clone();
        self.state.navigation.update(cx, |nav, _cx| {
            nav.tabs.add_tab(path, mode);
        });
        cx.notify();
    }

    pub fn open_in_new_window(&mut self, path: FilesPath, cx: &mut Context<Self>) {
        // Use the existing new_window function but set the initial path
        let progress_engine = self.state.progress_engine.clone();
        // Create a new database for the new window (shared same file)
        let db_path = dirs::data_local_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("Files-Rust")
            .join("files-rs.db");
        let db = Arc::new(
            crate::core::database::Database::new(db_path).unwrap_or_else(|_| {
                crate::core::database::Database::new(std::path::PathBuf::from(":memory:"))
                    .expect("Failed to create in-memory database")
            }),
        );
        let fi_res = cx.reserve_entity();
        let file_items = cx.insert_entity(fi_res, |_| {
            crate::ui::views::main_view::FileItems::default()
        });
        let app_state = AppState::new(
            path.clone(), // Use the provided path as initial path
            file_items,
            self.state.settings.clone(),
            self.state.localization.clone(),
            self.state.navigation.clone(),
            progress_engine.clone(),
            db,
            cx,
        );

        let options = gpui::WindowOptions {
            window_bounds: Some(gpui::WindowBounds::Windowed(gpui::Bounds {
                origin: Default::default(),
                size: gpui::size(gpui::Pixels::from(1280.0), gpui::Pixels::from(800.0)),
            })),
            ..gpui::WindowOptions::default()
        };

        cx.open_window(options, move |window, cx| {
            let rx = progress_engine.subscribe();
            cx.new(|cx| AppWindow::new(app_state, rx, window, cx))
        })
        .expect("Failed to open window");
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Hover-to-preview tooltip (delayed appearance for files)
    // ─────────────────────────────────────────────────────────────────────────

    const HOVER_TOOLTIP_DELAY_MS: u64 = 350;

    /// Start the hover timer for a file item. After HOVER_TOOLTIP_DELAY_MS,
    /// the tooltip appears. Calling again with a different index cancels the
    /// previous timer and restarts.
    pub fn start_hover_timer(
        &mut self,
        index: usize,
        position: gpui::Point<gpui::Pixels>,
        cx: &mut Context<Self>,
    ) {
        // Don't start if in hover selection mode
        if matches!(
            self.state.main_view.selection_mode,
            crate::ui::views::main_view::SelectionMode::HoverSelect
        ) {
            return;
        }

        // Validate item exists and is a file (not directory)
        {
            let Some(item) = self.state.main_view.items.get(index) else {
                return;
            };
            if matches!(item.file_type, FileType::Directory) {
                return;
            }
            // Already tracking this exact item - skip
            if self.hover_item_index == Some(index) {
                return;
            }
        }

        // Cancel any previous hover timer — assignment drops the old task
        self.hover_timer_task = None;

        // Store tracking state immediately
        self.hover_item_index = Some(index);

        let target_index = index;
        let tooltip_pos = position;
        let mut async_app = cx.to_async();

        let task = cx.spawn(move |this_weak: WeakEntity<Self>, _: &mut AsyncApp| async move {
            // Wait for hover delay on background
            async_app
                .background_executor()
                .timer(std::time::Duration::from_millis(Self::HOVER_TOOLTIP_DELAY_MS))
                .await;

            // Check if we're still hovering the same item
            let still_hovering = this_weak
                .update(&mut async_app, |this, _cx| {
                    this.hover_item_index == Some(target_index)
                })
                .unwrap_or(false);

            if still_hovering {
                // Show file preview tooltip
                this_weak
                    .update(&mut async_app, |this, cx| {
                        if let Some(item) = this.state.main_view.items.get(target_index) {
                            this.hover_tooltip = Some(HoverTooltipData {
                                position: tooltip_pos,
                                item: item.clone(),
                            });
                            cx.notify();
                        }
                    })
                    .ok();
            }
        });

        self.hover_timer_task = Some(task);
    }

    /// Stop the hover timer and clear the tooltip.
    pub fn stop_hover_timer(&mut self) {
        // Drop the old task to cancel it
        self.hover_timer_task = None;
        self.hover_item_index = None;
        self.hover_tooltip = None;
    }

    pub fn bulk_rename(&mut self, cx: &mut Context<Self>) {
        if self.state.main_view.selected_indices.is_empty() {
            return;
        }
        let indices = self
            .state
            .main_view
            .selected_indices
            .iter()
            .cloned()
            .collect();
        self.state.active_dialog = Some(DialogType::BulkRename {
            item_indices: indices,
        });
        cx.notify();
    }

    pub fn set_filter_text(&mut self, text: String, cx: &mut Context<Self>) {
        self.state.main_view.filter_text = text;
        cx.notify();
    }

    pub fn start_rename(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(item) = self.state.main_view.items.get(index) {
            self.state.main_view.renaming_index = Some(index);
            self.state.main_view.rename_text = item.name.clone();
            cx.notify();
        }
    }

    pub fn commit_rename(&mut self, cx: &mut Context<Self>) {
        if let Some(idx) = self.state.main_view.renaming_index {
            let new_name = self.state.main_view.rename_text.clone();
            tracing::info!("Renaming item {} to {}", idx, new_name);
            // In a real app, we'd call FileOperationService here
            if let Some(item) = self.state.main_view.items.get_mut(idx) {
                item.name = new_name;
            }
            self.state.main_view.renaming_index = None;
            cx.notify();
        }
    }

    pub fn cancel_rename(&mut self, cx: &mut Context<Self>) {
        self.state.main_view.renaming_index = None;
        cx.notify();
    }

    pub fn show_context_menu(
        &mut self,
        item_index: Option<usize>,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        // Pre-read selection data to avoid view.read(cx) crash during dialog render
        let selected_indices = &self.state.main_view.selected_indices;
        let has_selection = !selected_indices.is_empty();
        let selected_path = selected_indices
            .iter()
            .next()
            .and_then(|&idx| self.state.main_view.items.get(idx))
            .map(|item| item.path.clone());

        self.state.active_dialog = Some(crate::ui::components::dialogs::DialogType::ContextMenu {
            item_index,
            position,
            has_selection,
            selected_path,
        });
        cx.notify();
    }

    /// Show the column visibility toggle menu (right-click on column header)
    pub fn show_column_visibility_menu(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        self.state.active_dialog =
            Some(crate::ui::components::dialogs::DialogType::ColumnVisibilityMenu { position });
        cx.notify();
    }

    /// Toggle a column's visibility
    pub fn toggle_column_visibility(&mut self, column: String, cx: &mut Context<Self>) {
        match column.as_str() {
            "Date Modified" => {
                self.state.main_view.column_visibility.date =
                    !self.state.main_view.column_visibility.date;
            }
            "Type" => {
                self.state.main_view.column_visibility.type_col =
                    !self.state.main_view.column_visibility.type_col;
            }
            "Size" => {
                self.state.main_view.column_visibility.size =
                    !self.state.main_view.column_visibility.size;
            }
            _ => {}
        }
        cx.notify();
    }

    pub fn enter_compact_overlay(&mut self, cx: &mut Context<Self>) {
        tracing::info!("Entering compact overlay mode");
        let current_mode = if self.state.is_fullscreen {
            crate::ui::components::compact_overlay::WindowMode::Fullscreen
        } else {
            crate::ui::components::compact_overlay::WindowMode::Normal
        };
        self.state.compact_overlay.enter(current_mode);
        // Compact mode hides sidebar for minimal UI
        self.state.sidebar_visible = false;
        cx.notify();
    }

    pub fn exit_compact_overlay(&mut self, cx: &mut Context<Self>) {
        tracing::info!("Exiting compact overlay mode");
        if let Some(_mode) = self.state.compact_overlay.exit() {
            self.state.sidebar_visible = true;
        }
        cx.notify();
    }

    pub fn set_theme(&mut self, theme: String, cx: &mut Context<Self>) {
        let settings = self.state.settings.clone();
        settings.update(cx, move |s, _cx| {
            s.config.appearance.theme = theme;
            let _ = s.config.save();
        });
        cx.notify();
    }

    pub fn reorder_tab(&mut self, from_idx: usize, to_idx: usize, cx: &mut Context<Self>) {
        self.state.navigation.update(cx, |nav, _cx| {
            if from_idx < nav.tabs.tabs.len() && to_idx < nav.tabs.tabs.len() {
                let tab = nav.tabs.tabs.remove(from_idx);
                nav.tabs.tabs.insert(to_idx, tab);
                if nav.tabs.active_tab_index == from_idx {
                    nav.tabs.active_tab_index = to_idx;
                } else if from_idx < nav.tabs.active_tab_index
                    && to_idx >= nav.tabs.active_tab_index
                {
                    nav.tabs.active_tab_index -= 1;
                } else if from_idx > nav.tabs.active_tab_index
                    && to_idx <= nav.tabs.active_tab_index
                {
                    nav.tabs.active_tab_index += 1;
                }
            }
        });
        self.save_session(cx);
        cx.notify();
    }

    pub fn set_view_mode(&mut self, mode: ViewMode, cx: &mut Context<Self>) {
        self.state.main_view.view_mode = mode.clone();
        self.state.navigation.update(cx, |nav, _cx| {
            nav.tabs.active_tab_mut().view_mode = mode.clone();
        });

        // Save last used view mode (always) + per-directory preferences if lock is enabled
        let path = self.state.main_view.primary_path.clone();
        let zoom_level = self.state.main_view.zoom_level;
        let view_lock = self.state.main_view.view_lock_enabled;
        let settings = self.state.settings.clone();
        settings.update(cx, move |s, _cx| {
            // Always persist as the global "last used" view mode
            s.config.view.last_view_mode = mode.clone();
            s.config.view.last_zoom_level = zoom_level;

            // Save to per-directory preferences if lock is enabled
            if view_lock {
                s.config.set_folder_preferences(
                    &path,
                    crate::config::config::FolderPreferences {
                        view_mode: mode,
                        zoom_level,
                        sort_by: s.config.view.sort_by.clone(),
                        sort_ascending: s.config.view.sort_ascending,
                        folder_on_top: s.config.view.folder_on_top,
                        group_by: s.config.view.group_by,
                    },
                );
            }
            let _ = s.config.save();
        });

        cx.notify();
    }

    /// Toggle the per-directory view/zoom lock on/off
    pub fn toggle_view_lock(&mut self, cx: &mut Context<Self>) {
        self.state.main_view.view_lock_enabled = !self.state.main_view.view_lock_enabled;

        // If enabling the lock, save current directory's prefs immediately
        if self.state.main_view.view_lock_enabled {
            let path = self.state.main_view.primary_path.clone();
            let view_mode = self.state.main_view.view_mode.clone();
            let zoom_level = self.state.main_view.zoom_level;
            let settings = self.state.settings.clone();
            settings.update(cx, move |s, _cx| {
                s.config.set_folder_preferences(
                    &path,
                    crate::config::config::FolderPreferences {
                        view_mode,
                        zoom_level,
                        sort_by: s.config.view.sort_by.clone(),
                        sort_ascending: s.config.view.sort_ascending,
                        folder_on_top: s.config.view.folder_on_top,
                        group_by: s.config.view.group_by,
                    },
                );
            });
        }

        cx.notify();
    }

    pub fn toggle_setting(&mut self, key: &str, cx: &mut Context<Self>) {
        // Handle toolbar button visibility toggles (stored in main_view, not config)
        if key.starts_with("toolbar_") {
            let button_id = key.strip_prefix("toolbar_").unwrap_or(key);
            self.state.main_view.toolbar_customization.toggle(button_id);
            cx.notify();
            return;
        }

        let settings = self.state.settings.clone();
        settings.update(cx, |s, _cx| {
            match key {
                "show_file_extensions" => {
                    s.config.preferences.show_file_extensions =
                        !s.config.preferences.show_file_extensions
                }
                "show_hidden_files" => {
                    s.config.preferences.show_hidden_files = !s.config.preferences.show_hidden_files
                }
                "show_dot_files" => {
                    s.config.preferences.show_dot_files = !s.config.preferences.show_dot_files
                }
                "show_system_files" => {
                    s.config.preferences.show_system_files = !s.config.preferences.show_system_files
                }
                "confirm_delete" => {
                    s.config.preferences.confirm_delete = !s.config.preferences.confirm_delete
                }
                "mica_enabled" => {
                    s.config.appearance.mica_enabled = !s.config.appearance.mica_enabled
                }
                "mica_alt" => s.config.appearance.mica_alt = !s.config.appearance.mica_alt,
                "folder_on_top" => s.config.view.folder_on_top = !s.config.view.folder_on_top,
                _ => {}
            }
            let _ = s.config.save();
        });
        cx.notify();
    }

    pub fn set_settings_tab(&mut self, tab: SettingsTab, cx: &mut Context<Self>) {
        self.state.active_settings_tab = tab;
        cx.notify();
    }

    /// Load folder preview — reads first 12 entries for thumbnail grid
    fn load_folder_preview(&mut self, path: FilesPath, cx: &mut Context<Self>) {
        use crate::ui::views::main_view::{FolderPreviewItem, PreviewContent};

        let mut async_cx = cx.to_async();
        let this_weak = cx.weak_entity();
        let path_clone = path.clone();

        cx.foreground_executor()
            .spawn(async move {
                let mut items = Vec::new();
                if let Ok(entries) = std::fs::read_dir(path_clone.as_path()) {
                    for (count, entry) in entries.filter_map(|e| e.ok()).enumerate() {
                        if count >= 12 {
                            break;
                        }
                        let name = entry.file_name().to_string_lossy().to_string();
                        let is_dir = entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
                        items.push(FolderPreviewItem { name, is_dir });
                    }
                }

                this_weak
                    .update(&mut async_cx, |this, cx| {
                        let display_count = items.len();
                        this.state.main_view.preview_content = PreviewContent::Folder(items);
                        this.state.main_view.preview_text =
                            Some(format!("Folder contents: {} items shown", display_count));
                        cx.notify();
                    })
                    .ok();
            })
            .detach();
    }

    // ---- Recycle Bin Operations ----

    pub fn restore_recycle_bin_item(&mut self, index: usize, cx: &mut Context<Self>) {
        let items = &self.state.main_view.recycle_bin_items.clone();
        if let Some(item) = items.get(index) {
            let path = std::path::PathBuf::from(&item.original_path);
            let service = RecycleBinService::new();
            let mut async_cx = cx.to_async();
            let this_weak = cx.weak_entity();
            cx.foreground_executor()
                .spawn(async move {
                    let _ = service.restore_item(&path).await;
                    this_weak
                        .update(&mut async_cx, |this, cx| {
                            if index < this.state.main_view.recycle_bin_items.len() {
                                this.state.main_view.recycle_bin_items.remove(index);
                            }
                            cx.notify();
                        })
                        .ok();
                })
                .detach();
        }
    }

    pub fn empty_recycle_bin(&mut self, cx: &mut Context<Self>) {
        let service = RecycleBinService::new();
        let mut async_cx = cx.to_async();
        let this_weak = cx.weak_entity();
        cx.foreground_executor()
            .spawn(async move {
                let _ = service.empty().await;
                this_weak
                    .update(&mut async_cx, |this, cx| {
                        this.state.main_view.recycle_bin_items.clear();
                        cx.notify();
                    })
                    .ok();
            })
            .detach();
    }

    pub fn restore_all_recycle_bin(&mut self, cx: &mut Context<Self>) {
        let service = RecycleBinService::new();
        let mut async_cx = cx.to_async();
        let this_weak = cx.weak_entity();
        cx.foreground_executor()
            .spawn(async move {
                let _ = service.restore_all().await;
                this_weak
                    .update(&mut async_cx, |this, cx| {
                        this.state.main_view.recycle_bin_items.clear();
                        cx.notify();
                    })
                    .ok();
            })
            .detach();
    }

    /// Toggle the shortcut overlay / command palette.
    pub fn toggle_shortcut_overlay(&mut self, cx: &mut Context<Self>) {
        self.state.show_shortcut_overlay = !self.state.show_shortcut_overlay;
        cx.notify();
    }

    /// Dispatch an action from the keyboard shortcut system.
    /// Returns true if the action was handled.
    pub fn dispatch_action(
        &mut self,
        action: crate::core::action_registry::ActionKind,
        cx: &mut Context<Self>,
    ) -> bool {
        match action {
            crate::core::action_registry::ActionKind::NavigateBack => self.navigate_back(cx),
            crate::core::action_registry::ActionKind::NavigateForward => self.navigate_forward(cx),
            crate::core::action_registry::ActionKind::NavigateUp => self.navigate_up(cx),
            crate::core::action_registry::ActionKind::NavigateToRoot => self.navigate_to_root(cx),
            crate::core::action_registry::ActionKind::SelectAll => self.select_all(cx),
            crate::core::action_registry::ActionKind::InvertSelection => self.invert_selection(cx),
            crate::core::action_registry::ActionKind::Copy => self.copy_selected(cx),
            crate::core::action_registry::ActionKind::Cut => self.cut_selected(cx),
            crate::core::action_registry::ActionKind::Paste => self.paste(cx),
            crate::core::action_registry::ActionKind::Rename => {
                if let Some(&idx) = self.state.main_view.selected_indices.iter().next() {
                    self.start_rename(idx, cx);
                }
            }
            crate::core::action_registry::ActionKind::Delete => self.delete_selected(cx),
            crate::core::action_registry::ActionKind::PermanentDelete => {
                // Shift+Delete: permanent delete - fallback for now
                self.delete_selected(cx);
            }
            crate::core::action_registry::ActionKind::ClearSelection => {
                if self.state.main_view.search_state.is_searching {
                    self.clear_search(cx);
                } else if !self.state.main_view.filter_text.is_empty() {
                    self.set_filter_text(String::new(), cx);
                } else if !self.state.main_view.selected_indices.is_empty() {
                    self.clear_selection(cx);
                }
            }
            crate::core::action_registry::ActionKind::Refresh => self.refresh(cx),
            crate::core::action_registry::ActionKind::Search => {
                self.state.main_view.search_state.is_searching = true;
                tracing::debug!("Ctrl+F: Search activated");
            }
            crate::core::action_registry::ActionKind::ClearFilter => {
                if self.state.main_view.search_state.is_searching {
                    self.clear_search(cx);
                } else if !self.state.main_view.filter_text.is_empty() {
                    self.set_filter_text(String::new(), cx);
                }
            }
            crate::core::action_registry::ActionKind::ToggleHiddenFiles => {
                self.toggle_setting("show_hidden_files", cx)
            }
            crate::core::action_registry::ActionKind::EditPath => {
                // Set focus to path bar for editing - placeholder
                tracing::debug!("Edit path requested (Ctrl+L)");
            }
            crate::core::action_registry::ActionKind::OpenPreviewPane => {
                self.invoke_third_party_preview(cx)
            }
            crate::core::action_registry::ActionKind::CommandPalette => {
                // Toggle shortcut overlay / command palette
                self.toggle_shortcut_overlay(cx);
            }
            crate::core::action_registry::ActionKind::NewTab => {
                tracing::debug!("New tab requested (Ctrl+T) - not yet implemented");
            }
            crate::core::action_registry::ActionKind::CloseTab => {
                tracing::debug!("Close tab requested (Ctrl+W) - not yet implemented");
            }
            crate::core::action_registry::ActionKind::ReopenClosedTab => {
                tracing::debug!("Reopen closed tab (Ctrl+Shift+T) - not yet implemented");
            }
            crate::core::action_registry::ActionKind::NewWindow => {
                tracing::debug!("New window requested (Ctrl+N) - not yet implemented");
            }
            crate::core::action_registry::ActionKind::OpenHelp => {
                self.open_help(cx);
            }
            crate::core::action_registry::ActionKind::RestoreLastSession => {
                self.restore_last_session(cx);
            }
        }
        true
    }

    /// Build the keyboard shortcut overlay modal.
    pub fn build_shortcut_overlay(&self, cx: &mut Context<Self>) -> impl IntoElement {
        use gpui::*;
        let registry = &self.state.action_registry;
        let pairs = registry.iter();

        // Group actions by category
        let navigation: Vec<_> = pairs
            .iter()
            .filter(|(a, _)| {
                matches!(
                    a,
                    crate::core::action_registry::ActionKind::NavigateBack
                        | crate::core::action_registry::ActionKind::NavigateForward
                        | crate::core::action_registry::ActionKind::NavigateUp
                        | crate::core::action_registry::ActionKind::NavigateToRoot
                )
            })
            .cloned()
            .collect();

        let selection: Vec<_> = pairs
            .iter()
            .filter(|(a, _)| {
                matches!(
                    a,
                    crate::core::action_registry::ActionKind::SelectAll
                        | crate::core::action_registry::ActionKind::InvertSelection
                        | crate::core::action_registry::ActionKind::Copy
                        | crate::core::action_registry::ActionKind::Cut
                        | crate::core::action_registry::ActionKind::Paste
                        | crate::core::action_registry::ActionKind::Rename
                        | crate::core::action_registry::ActionKind::Delete
                        | crate::core::action_registry::ActionKind::PermanentDelete
                        | crate::core::action_registry::ActionKind::ClearSelection
                )
            })
            .cloned()
            .collect();

        let view_actions: Vec<_> = pairs
            .iter()
            .filter(|(a, _)| {
                matches!(
                    a,
                    crate::core::action_registry::ActionKind::Refresh
                        | crate::core::action_registry::ActionKind::Search
                        | crate::core::action_registry::ActionKind::ClearFilter
                        | crate::core::action_registry::ActionKind::ToggleHiddenFiles
                        | crate::core::action_registry::ActionKind::EditPath
                        | crate::core::action_registry::ActionKind::OpenPreviewPane
                )
            })
            .cloned()
            .collect();

        let tabs: Vec<_> = pairs
            .iter()
            .filter(|(a, _)| {
                matches!(
                    a,
                    crate::core::action_registry::ActionKind::NewTab
                        | crate::core::action_registry::ActionKind::CloseTab
                        | crate::core::action_registry::ActionKind::ReopenClosedTab
                        | crate::core::action_registry::ActionKind::NewWindow
                )
            })
            .cloned()
            .collect();

        let others: Vec<_> = pairs
            .iter()
            .filter(|(a, _)| matches!(a, crate::core::action_registry::ActionKind::CommandPalette))
            .cloned()
            .collect();

        let view = cx.entity().clone();

        div()
            .absolute()
            .top(px(0.0))
            .left(px(0.0))
            .right(px(0.0))
            .bottom(px(0.0))
            .bg(rgba(0x000000AAu32))
            .flex()
            .items_center()
            .justify_center()
            .on_mouse_down(MouseButton::Left, {
                let view = view.clone();
                move |_, _, cx| {
                    view.update(cx, |this, cx| {
                        this.state.show_shortcut_overlay = false;
                        cx.notify();
                    });
                }
            })
            .child(
                div()
                    .bg(rgb(0x2D2D2Du32))
                    .rounded(px(12.0))
                    .p(px(24.0))
                    .w(px(560.0))
                    .h(px(440.0))
                    .on_mouse_down(MouseButton::Left, {
                        let view = view.clone();
                        move |_, _, cx| {
                            // Clicking inside doesn't close
                            cx.stop_propagation();
                        }
                    })
                    .child(
                        div()
                            .flex_col()
                            .gap(px(16.0))
                            .h_full()
                            .overflow_hidden()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .child(
                                        div()
                                            .font_weight(FontWeight(700.0))
                                            .text_size(px(18.0))
                                            .child("Keyboard Shortcuts"),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(13.0))
                                            .text_color(rgba(0xFFFFFF99u32))
                                            .child("Press Escape to close"),
                                    ),
                            )
                            .child(div().h(px(1.0)).w_full().bg(rgba(0xFFFFFF30u32)))
                            .child({
                                // Build sections as Vec to handle different types
                                let mut sections: Vec<AnyElement> = Vec::new();
                                if !navigation.is_empty() {
                                    sections.push(
                                        Self::shortcut_section("Navigation", &navigation)
                                            .into_any_element(),
                                    );
                                }
                                if !selection.is_empty() {
                                    sections.push(
                                        Self::shortcut_section("Selection & Editing", &selection)
                                            .into_any_element(),
                                    );
                                }
                                if !view_actions.is_empty() {
                                    sections.push(
                                        Self::shortcut_section("View", &view_actions)
                                            .into_any_element(),
                                    );
                                }
                                if !tabs.is_empty() {
                                    sections.push(
                                        Self::shortcut_section("Tabs & Windows", &tabs)
                                            .into_any_element(),
                                    );
                                }
                                if !others.is_empty() {
                                    sections.push(
                                        Self::shortcut_section("Commands", &others)
                                            .into_any_element(),
                                    );
                                }
                                div()
                                    .flex_col()
                                    .gap(px(12.0))
                                    .flex_grow()
                                    .overflow_hidden()
                                    .children(sections)
                            }),
                    ),
            )
    }

    fn shortcut_section<'a>(
        title: &'a str,
        items: &'a [(
            crate::core::action_registry::ActionKind,
            crate::core::action_registry::KeyCombination,
        )],
    ) -> impl IntoElement + 'a {
        if items.is_empty() {
            return div().into_any_element();
        }
        div()
            .flex_col()
            .gap(px(6.0))
            .child(
                div()
                    .font_weight(FontWeight(600.0))
                    .text_size(px(14.0))
                    .text_color(rgba(0xFFFFFFCCu32))
                    .child(title.to_string()),
            )
            .child(
                div()
                    .flex_col()
                    .gap(px(2.0))
                    .children(items.iter().map(|(action, key)| {
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .py(px(3.0))
                            .px(px(4.0))
                            .child(div().text_size(px(13.0)).child(action.label()))
                            .child(
                                div()
                                    .bg(rgba(0xFFFFFF30u32))
                                    .px(px(6.0))
                                    .py(px(2.0))
                                    .rounded(px(4.0))
                                    .text_size(px(12.0))
                                    .child(key.to_string()),
                            )
                    })),
            )
            .into_any_element()
    }
}

impl Render for AppWindow {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // file_items is synced in the background task update callback, not during render

        // Sync icon_paths from icon_cache (cheap: Mutex lock + HashMap clone)
        // This makes extracted Windows Shell icons visible on the next render cycle
        if let Ok(cache) = self.state.icon_cache.lock() {
            self.state.main_view.icon_paths = cache.clone();
        }

        let (can_go_back, can_go_forward, pane_layout) = {
            let nav = self.state.navigation.read(cx);
            let can_go_back = nav.tabs.active_tab().can_go_back();
            let can_go_forward = nav.tabs.active_tab().can_go_forward();
            let pane_layout = nav.pane_layout;
            (can_go_back, can_go_forward, pane_layout)
        };

        self.state.main_view.pane_type = match pane_layout {
            PaneLayout::Single => crate::ui::views::main_view::PaneType::Single,
            _ => crate::ui::views::main_view::PaneType::Dual,
        };

        let state = &self.state;
        let _nav = state.navigation.read(cx);

        let mica_enabled = self
            .state
            .settings
            .read(cx)
            .config()
            .appearance
            .mica_enabled;
        let mica_alt = self.state.settings.read(cx).config().appearance.mica_alt;
        let font_family = self
            .state
            .settings
            .read(cx)
            .config()
            .appearance
            .font_family
            .clone();
        let font_size = self.state.settings.read(cx).config().appearance.font_size;

        let main_content = AppBackground::build(
            &state.main_view.folder_background,
            mica_enabled,
            mica_alt,
            vec![
                // Main content area
                div()
                    .flex()
                    .flex_col()
                    .w_full()
                    .h_full()
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .w_full()
                            .h_full()
                            .font_family(font_family.clone())
                            .text_size(px(font_size as f32))
                            .on_key_down({
                                let view = cx.entity().clone();
                                move |event: &gpui::KeyDownEvent,
                                      _window: &mut Window,
                                      cx: &mut App| {
                                    let keystroke = &event.keystroke;
                                    let key_combo =
                                        crate::core::action_registry::from_gpui_keystroke(
                                            keystroke,
                                        );
                                    view.update(cx, |this, cx| {
                                        use crate::ui::components::dialogs::DialogType;

                                        // Intercept AddItem dialog input typing
                                        if let Some(DialogType::AddItem { ref mut name, .. }) = this.state.active_dialog {
                                            let key = keystroke.key.as_str();
                                            match key {
                                                "escape" | "Escape" => {
                                                    this.close_dialog(cx);
                                                }
                                                "enter" | "return" | "Enter" | "Return" => {
                                                    this.confirm_add_item(cx);
                                                }
                                                "backspace" | "Backspace" => {
                                                    name.pop();
                                                    cx.notify();
                                                }
                                                "space" | "Space" => {
                                                    name.push(' ');
                                                    cx.notify();
                                                }
                                                _ => {
                                                    if !keystroke.modifiers.control && !keystroke.modifiers.alt && !keystroke.modifiers.platform {
                                                        if keystroke.key.chars().count() == 1 {
                                                            name.push_str(&keystroke.key);
                                                            cx.notify();
                                                        }
                                                    }
                                                }
                                            }
                                            return;
                                        }

                                        // Intercept inline renaming input typing
                                        if this.state.main_view.renaming_index.is_some() {
                                            let key = keystroke.key.as_str();
                                            match key {
                                                "escape" | "Escape" => {
                                                    this.cancel_rename(cx);
                                                }
                                                "enter" | "return" | "Enter" | "Return" => {
                                                    this.commit_rename(cx);
                                                }
                                                "backspace" | "Backspace" => {
                                                    this.state.main_view.rename_text.pop();
                                                    cx.notify();
                                                }
                                                "space" | "Space" => {
                                                    this.state.main_view.rename_text.push(' ');
                                                    cx.notify();
                                                }
                                                _ => {
                                                    if !keystroke.modifiers.control && !keystroke.modifiers.alt && !keystroke.modifiers.platform {
                                                        if keystroke.key.chars().count() == 1 {
                                                            this.state.main_view.rename_text.push_str(&keystroke.key);
                                                            cx.notify();
                                                        }
                                                    }
                                                }
                                            }
                                            return;
                                        }

                                        if let Some(action) =
                                            this.state.action_registry.lookup(&key_combo)
                                        {
                                            this.dispatch_action(action, cx);
                                        } else {
                                            if !keystroke.modifiers.modified() {
                                                match keystroke.key.as_str() {
                                                    "escape" | "Escape" => {
                                                        if this.state.splash_state.visible {
                                                            this.dismiss_splash(cx);
                                                        } else if this
                                                            .state
                                                            .main_view
                                                            .search_state
                                                            .is_searching
                                                        {
                                                            this.clear_search(cx);
                                                        } else if !this
                                                            .state
                                                            .main_view
                                                            .filter_text
                                                            .is_empty()
                                                        {
                                                            this.set_filter_text(String::new(), cx);
                                                        } else if !this
                                                            .state
                                                            .main_view
                                                            .selected_indices
                                                            .is_empty()
                                                        {
                                                            this.clear_selection(cx);
                                                        }
                                                    }
                                                    "ArrowDown" => {
                                                        let len = this.state.main_view.items.len();
                                                        let current = this
                                                            .state
                                                            .main_view
                                                            .last_clicked_index
                                                            .unwrap_or(0)
                                                            .min(len.saturating_sub(1));
                                                        let next = (current + 1)
                                                            .min(len.saturating_sub(1));
                                                        this.select_item(
                                                            next,
                                                            event.keystroke.modifiers.shift,
                                                            false,
                                                            cx,
                                                        );
                                                        this.state.main_view.last_clicked_index =
                                                            Some(next);
                                                    }
                                                    "ArrowUp" => {
                                                        let current = this
                                                            .state
                                                            .main_view
                                                            .last_clicked_index
                                                            .unwrap_or(0);
                                                        let prev = current.saturating_sub(1);
                                                        this.select_item(
                                                            prev,
                                                            event.keystroke.modifiers.shift,
                                                            false,
                                                            cx,
                                                        );
                                                        this.state.main_view.last_clicked_index =
                                                            Some(prev);
                                                    }
                                                    "Home" => {
                                                        if !this.state.main_view.items.is_empty() {
                                                            this.select_item(
                                                                0,
                                                                event.keystroke.modifiers.shift,
                                                                false,
                                                                cx,
                                                            );
                                                            this.state
                                                                .main_view
                                                                .last_clicked_index = Some(0);
                                                        }
                                                    }
                                                    "End" => {
                                                        let last = this
                                                            .state
                                                            .main_view
                                                            .items
                                                            .len()
                                                            .saturating_sub(1);
                                                        this.select_item(
                                                            last,
                                                            event.keystroke.modifiers.shift,
                                                            false,
                                                            cx,
                                                        );
                                                        this.state.main_view.last_clicked_index =
                                                            Some(last);
                                                    }
                                                    _ => {}
                                                }
                                            }
                                            let key = keystroke.key.as_str();
                                            if keystroke.modifiers.control
                                                && (key == "a" || key == "A")
                                            {
                                                this.select_all(cx);
                                            }
                                            if key == "enter" || key == "return" {
                                                if let Some(&idx) = this
                                                    .state
                                                    .main_view
                                                    .selected_indices
                                                    .iter()
                                                    .next()
                                                {
                                                    this.open_item(idx, cx);
                                                }
                                            }
                                            if key == "backspace" {
                                                this.navigate_up(cx);
                                            }
                                        }
                                    });
                                }
                            })
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .flex_grow()
                                    .h_full()
                                    .w_full()
                                    .overflow_hidden()
                                    .child(
                                        // TabBar (fixed height)
                                        div().flex_shrink_0().w_full().child(TabBarView::build(
                                            &state.navigation.read(cx),
                                            cx.entity(),
                                        )),
                                    )
                                    .child(
                                        // Navigation Bar (fixed height)
                                        div().flex_shrink_0().w_full().child(
                                            NavigationBarView::build(
                                                &state.nav_bar,
                                                can_go_back,
                                                can_go_forward,
                                                cx.entity(),
                                            ),
                                        ),
                                    )
                                    .child({
                                        let view = cx.entity();
                                        let mut row = div()
                                            .flex()
                                            .flex_row()
                                            .overflow_hidden()
                                            .flex_grow()
                                            .h_full()
                                            .w_full()
                                            // Track mouse moves ANYWHERE in this row during sidebar resize
                                            .on_mouse_move({
                                                let view = view.clone();
                                                move |event, win, cx| {
                                                    view.update(cx, |this, cx| {
                                                        if this
                                                            .state
                                                            .sidebar
                                                            .resize_start_x
                                                            .is_some()
                                                        {
                                                            let x: f32 =
                                                                win.mouse_position().x.into();
                                                            if this
                                                                .state
                                                                .sidebar
                                                                .update_resize_drag(x)
                                                            {
                                                                cx.notify();
                                                            }
                                                        }
                                                        if let (Some(start_x), Some(start_width)) = (
                                                            this.state.main_view.side_pane_resize_start_x,
                                                            this.state.main_view.side_pane_resize_start_width,
                                                        ) {
                                                            let x: f32 = win.mouse_position().x.into();
                                                            let delta = x - start_x;
                                                            let win_width: f32 = win.viewport_size().width.into();
                                                            let min_width = 150.0;
                                                            let max_width = win_width * 0.65;
                                                            let new_width = (start_width - delta).max(min_width).min(max_width);
                                                            this.state.main_view.side_pane_width = Some(new_width);
                                                            cx.notify();
                                                        }
                                                        // Handle column resize drag
                                                        if this.column_resize.is_some() {
                                                            let x: f32 =
                                                                win.mouse_position().x.into();
                                                            this.update_column_resize(x);
                                                            cx.notify();
                                                        }
                                                    });
                                                }
                                            })
                                            .on_mouse_up(MouseButton::Left, {
                                                let view = view.clone();
                                                move |_event, _win, cx| {
                                                    view.update(cx, |this, cx| {
                                                        if this
                                                            .state
                                                            .sidebar
                                                            .resize_start_x
                                                            .is_some()
                                                        {
                                                            this.state.sidebar.end_resize_drag();
                                                        }
                                                        if this.state.main_view.side_pane_resize_start_x.is_some() {
                                                            this.state.main_view.side_pane_resize_start_x = None;
                                                            this.state.main_view.side_pane_resize_start_width = None;
                                                            cx.notify();
                                                        }
                                                        if this.column_resize.is_some() {
                                                            this.end_column_resize();
                                                        }
                                                    });
                                                }
                                            });
                                        if state.sidebar_visible {
                                            #[cfg(windows)]
                                            {
                                                row = row
                                                    .child(SidebarView::build(
                                                        &state.sidebar,
                                                        state,
                                                        cx.entity(),
                                                    ))
                                                    .child(SidebarSplitter::build(
                                                        &state.sidebar,
                                                        cx.entity(),
                                                    ));
                                            }
                                        }
                                        row.child(
                                            div()
                                                .flex()
                                                .flex_col()
                                                .flex_grow()
                                                .w_full()
                                                .h_full()
                                                .overflow_hidden()
                                                .on_mouse_down(MouseButton::Left, {
                                                    let view = cx.entity();
                                                    move |_event, _window, cx| {
                                                        view.update(cx, |this, cx| {
                                                            if this.state.sidebar.reorder_mode {
                                                                this.state
                                                                    .sidebar
                                                                    .exit_reorder_mode();
                                                                cx.notify();
                                                            }
                                                        });
                                                    }
                                                })
                                                .child(
                                                    // Toolbar (fixed height)
                                                    div().flex_shrink_0().w_full().child(
                                                        ToolbarView::build(
                                                            &state.main_view.view_mode,
                                                            &state.main_view.toolbar_customization,
                                                            !state.main_view.selected_indices.is_empty(),
                                                            state.main_view.selected_indices.len() == 1,
                                                            !matches!(state.clipboard, ClipboardState::Empty),
                                                             state.main_view.filter_text.clone(),
                                                             cx.entity(),
                                                        ),
                                                    ),
                                                )
                                                .child(
                                                    // Main Content (flex_grow to fill remaining space)
                                                    div()
                                                        .flex()
                                                        .flex_col()
                                                        .flex_grow()
                                                        .w_full()
                                                        .h_full()
                                                        .child(MainView::build(
                                                            &state.main_view,
                                                            state.file_items.clone(),
                                                            cx.entity(),
                                                        )),
                                                )
                                                .child(
                                                    // Status bar (fixed height at bottom)
                                                    div()
                                                        .flex_shrink_0()
                                                        .w_full()
                                                        .h(px(UiConstants::STATUS_BAR_HEIGHT_RAW
                                                            as f32))
                                                        .child(StatusBarView::build(
                                                            &state.status_bar,
                                                            state,
                                                            cx.entity(),
                                                        )),
                                                ),
                                        )
                                    }),
                            ),
                    )
                    .into_any_element(),
            ],
        );

        if state.sidebar_visible && state.sidebar.anim_start_time.is_some() {
            if state.sidebar.anim_start_time.unwrap().elapsed()
                < std::time::Duration::from_millis(200)
            {
                _window.request_animation_frame();
            }
        }

        let mut base = div().relative().w_full().h_full();
        if let Some(dialog) = &state.active_dialog {
            // Pre-read settings dialog data to avoid BorrowMutError during render tree construction
            let settings_data = match dialog {
                crate::ui::components::dialogs::DialogType::Settings => Some(
                    crate::ui::components::dialogs::DialogsView::pre_read_settings_data(
                        &self.state,
                        cx,
                    ),
                ),
                _ => None,
            };
            base =
                base.child(main_content)
                    .child(crate::ui::components::dialogs::DialogsView::build(
                        dialog,
                        cx.entity(),
                        cx,
                        settings_data,
                    ));
        } else {
            base = base.child(main_content);
        }

        // Render sidebar collapsed-mode tooltip at app level (outside overflow-hidden areas)
        if state.sidebar.collapsed {
            if let Some(sid) = state.sidebar.tooltip_section {
                // Find the section index in the current display order to compute Y offset
                let section_idx = state.sidebar.sections.iter().position(|s| s.id == sid);
                if let (Some(idx), Some(section)) = (
                    section_idx,
                    state.sidebar.sections.iter().find(|s| s.id == sid),
                ) {
                    // Y offset: 40px home toggle + 12px scroll padding + idx * 36px per collapsed icon
                    let tooltip_y = 52.0 + idx as f32 * 36.0;
                    base = base.child({
                        let sidebar_w = px(UiConstants::SIDEBAR_WIDTH_COLLAPSED_RAW as f32);
                        let tooltip_left = sidebar_w + px(4.0);
                        crate::ui::components::sidebar::SidebarView::render_tooltip(
                            section,
                            tooltip_y,
                            tooltip_left,
                            cx.entity(),
                        )
                    });
                }
            }
        }

        // Render file preview tooltip on hover (at app level, outside overflow containers)
        if let Some(tip) = &self.hover_tooltip {
            use crate::ui::views::helpers::format_size;
            let item = &tip.item;
            let pos = tip.position;

            // ── Date formatting ──
            let date_str = item
                .modified
                .as_ref()
                .map(|t| {
                    use std::time::{SystemTime, UNIX_EPOCH};
                    let since_epoch = t
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    let secs_per_day: u64 = 86400;
                    let days = since_epoch / secs_per_day;
                    let remaining = since_epoch % secs_per_day;
                    let hours = remaining / 3600;
                    let minutes = (remaining % 3600) / 60;
                    let mut y = 2000i64;
                    let mut d = days as i64 - (11017i64);
                    loop {
                        let days_in_year =
                            if y % 4 == 0 && (y % 100 != 0 || y % 400 == 0) { 366 } else { 365 };
                        if d < days_in_year {
                            break;
                        }
                        d -= days_in_year;
                        y += 1;
                    }
                    let leap = y % 4 == 0 && (y % 100 != 0 || y % 400 == 0);
                    let month_days = [31u64, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
                    let mut m = 0usize;
                    while d as u64 >= month_days[m] {
                        d -= month_days[m] as i64;
                        m += 1;
                    }
                    let day = d + 1;
                    format!("{:04}-{:02}-{:02} {:02}:{:02}", y, m + 1, day, hours, minutes)
                })
                .unwrap_or_default();
            let size_bytes = item.size;
            let size_hr = format_size(size_bytes);

            // ── File type string ──
            let file_type_str = if matches!(item.file_type, FileType::Directory) {
                "Folder".to_string()
            } else {
                let ext = item
                    .name
                    .rsplit('.')
                    .next()
                    .map(|e| e.to_uppercase())
                    .unwrap_or_default();
                if ext.is_empty() {
                    "File".to_string()
                } else {
                    format!("{} File", ext)
                }
            };

            // ── Determine if this is an image with a cached thumbnail ──
            let extension = item
                .name
                .rsplit('.')
                .next()
                .map(|e| e.to_lowercase())
                .unwrap_or_default();
            let image_exts = [
                "jpg", "jpeg", "png", "gif", "bmp", "webp", "ico", "tiff", "tif", "heic", "heif",
            ];
            let is_image = image_exts.contains(&extension.as_str());
            let thumbnail_path = if is_image {
                self.state.main_view.icon_paths.get(&item.path).cloned()
            } else {
                None
            };

            // ── Tooltip sizing ──
            let tooltip_w = 280.0;
            let want_thumb_h = if thumbnail_path.is_some() { 110.0 } else { 0.0 };
            // Approximate text block height: 13px title + 3×11px lines + margins ≈ 70px
            let text_h = 70.0;
            let tooltip_h = 20.0 + want_thumb_h + text_h; // padding + thumb + info

            // ── Viewport-aware positioning ──
            let win_bounds = _window.bounds();
            let win_w = win_bounds.size.width;
            let win_h = win_bounds.size.height;

            // Preferred: tooltip to the RIGHT and ABOVE the cursor
            let mut tx = pos.x + px(16.0);
            let mut ty = pos.y - px(tooltip_h) - px(8.0);

            // If tooltip would go off right edge, flip to left of cursor
            if tx + px(tooltip_w) + px(12.0) > win_w {
                tx = pos.x - px(tooltip_w) - px(16.0);
            }
            // Clamp left edge
            if tx < px(8.0) {
                tx = px(8.0);
            }
            // If tooltip would go off top edge, show below cursor
            if ty < px(4.0) {
                ty = pos.y + px(16.0);
            }
            // Clamp bottom edge
            if ty + px(tooltip_h) + px(4.0) > win_h {
                ty = (win_h - px(tooltip_h) - px(4.0)).max(px(4.0));
            }

            // ── Build tooltip element ──
            let mut info_div = div()
                .flex()
                .flex_col();

            // Add thumbnail image preview if available
            if let Some(thumb_path) = &thumbnail_path {
                info_div = info_div.child(
                    div()
                        .w_full()
                        .h(px(want_thumb_h))
                        .flex()
                        .items_center()
                        .justify_center()
                        .border_b(px(1.0))
                        .border_color(rgb(0x3A3A3A))
                        .mb(px(6.0))
                        .child(
                            img(thumb_path.clone())
                                .w(px(130.0))
                                .h(px(want_thumb_h - 8.0))
                                .object_fit(ObjectFit::Contain)
                                .rounded(px(4.0)),
                        ),
                );
            }

            // File info text
            info_div = info_div
                .child(
                    div()
                        .text_size(px(13.0))
                        .font_family(UiConstants::FONT_MAIN)
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(0xFFFFFF))
                        .child(item.name.clone()),
                )
                .child(
                    div()
                        .mt(px(4.0))
                        .text_size(px(11.0))
                        .font_family(UiConstants::FONT_MAIN)
                        .text_color(rgb(0xAAAAAA))
                        .child(format!("Type: {}", file_type_str)),
                )
                .child(
                    div()
                        .text_size(px(11.0))
                        .font_family(UiConstants::FONT_MAIN)
                        .text_color(rgb(0xAAAAAA))
                        .child(format!("Size: {} ({} bytes)", size_hr, size_bytes)),
                )
                .child(
                    div()
                        .text_size(px(11.0))
                        .font_family(UiConstants::FONT_MAIN)
                        .text_color(rgb(0xAAAAAA))
                        .child(format!("Date: {}", date_str)),
                );

            // Tooltip container with fade-in animation
            let tooltip = div()
                .absolute()
                .left(tx)
                .top(ty)
                .w(px(tooltip_w))
                .bg(rgb(0x2B2B2B))
                .border(px(1.0))
                .border_color(rgb(0x444444))
                .rounded(px(8.0))
                .py(px(10.0))
                .px(px(12.0))
                .overflow_hidden()
                .child(info_div);

            base = base.child(
                tooltip.with_animation(
                    "tooltip-fade",
                    Animation::new(std::time::Duration::from_millis(120)),
                    |elem, delta| elem.opacity(delta),
                ),
            );
        }
        if state.show_shortcut_overlay {
            base = base.child(self.build_shortcut_overlay(cx));
        }

        if state.splash_state.visible {
            base = base.child(crate::ui::components::splash_screen::SplashScreen::build(
                &state.splash_state,
                cx.entity(),
            ));
        }

        // Opportunistic icon refresh check: if the viewport shows items
        // without cached icons (e.g. after scrolling), schedule a load.
        // This runs outside the render element tree and is idempotent.
        if !self.state.main_view.items.is_empty()
            && self.state.main_view.items.len() > self.state.main_view.icon_paths.len()
        {
            // Only check every ~500ms max to avoid hammering the executor
            let now = std::time::Instant::now();
            let last_check = self.state.main_view.icon_refresh_last_check;
            let elapsed = last_check.map(|t| now - t).unwrap_or(std::time::Duration::MAX);
            if elapsed > std::time::Duration::from_millis(500) {
                self.state.main_view.icon_refresh_last_check = Some(now);
                let mut trigger = cx.to_async();
                let weak = cx.weak_entity();
                // Defer to next tick to avoid side-effects inside render
                cx.foreground_executor()
                    .spawn(async move {
                        weak.update(&mut trigger, |this, cx| {
                            this.refresh_icons_for_viewport(cx);
                        })
                        .ok();
                    })
                    .detach();
            }
        }

        base
    }
}

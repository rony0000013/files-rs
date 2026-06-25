use crate::app_state::{AppState, AppWindow};
use crate::config::config::ViewMode;
use crate::core::navigation_service::NavigationService;
use crate::core::path_util::FilesPath;
use crate::domain::git_service::GitService;
use crate::storage::storage::{FileType, StorageService};
use crate::ui::components::dialogs::DialogType;
use gpui::*;
use std::sync::Arc;

#[cfg(windows)]
use crate::platform::windows::recycle_bin_service::RecycleBinService;

impl AppWindow {
    pub fn navigate(&mut self, path: FilesPath, cx: &mut Context<Self>) {
        // Update navigation history
        self.state.navigation.update(cx, |nav, _cx| {
            nav.tabs.active_tab_mut().navigate(path.clone());
        });

        self.state.main_view.primary_path = path.clone();
        self.state.nav_bar.update_path(path.clone());
        // Immediately notify so the omnibar dismisses before async item loading
        cx.notify();
        self.state.main_view.items.clear();
        // Clear icon_paths to avoid stale entries from previous folder during
        // the brief window before items are loaded and icon_cache is synced.
        self.state.main_view.icon_paths.clear();

        // Sync sidebar active item
        #[cfg(windows)]
        self.state.sidebar.set_active(&path);

        // Status bar counts will be updated when items finish loading (Phase 1 callback).
        // Don't count from the just-cleared items list — it would always show 0.
        self.state.status_bar.sync(0, 0, 0);

        // Update git info in status bar
        let branch = GitService::get_branch(&path);
        self.state.status_bar.show_git_actions = branch.is_some();
        self.state.status_bar.git_branch = branch;

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
            self.state.main_view.zoom_level =
                active_tab.get_zoom_for_view(active_tab.view_mode.clone());
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
                                        created: None,
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
                            // NOTE: Cannot use std::mem::swap here because main_view.items
                            // is read again on lines 168-176 for folder/file counting.
                            // Clone is required to keep both copies in sync.
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
                            let branch = GitService::get_branch(&p1);
                            this.state.status_bar.show_git_actions = branch.is_some();
                            this.state.status_bar.git_branch = branch;

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
                                    Option<std::time::SystemTime>,
                                    bool,
                                    bool,
                                )> = Vec::new();
                                for path in &chunk {
                                    if let Ok(meta) = std::fs::metadata(path.as_path()) {
                                        let size = meta.len();
                                        let modified = meta.modified().ok();
                                        let created = meta.created().ok();
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
                                            created,
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
                                    for (path, size, modified, created, is_hidden, is_system) in
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
                                            item.created = created;
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
                // PHASE 2.5: Compute git status for all items in background
                // =================================================================
                {
                    let git_paths = item_paths.clone();
                    let git_path = path_clone.clone();
                    let git_weak = this_weak.clone();
                    async_cx
                        .background_executor()
                        .spawn(async move {
                            use crate::domain::git_service::GitService;
                            let mut git_statuses: Vec<(FilesPath, crate::domain::git_service::GitFileStatus)> = Vec::new();
                            // Only compute git status if we're in a repo
                            if GitService::is_repo(&git_path) {
                                for path in &git_paths {
                                    let status = GitService::get_file_status(path);
                                    git_statuses.push((path.clone(), status));
                                }
                            }
                            git_statuses
                        })
                        .detach();
                    // Note: To properly wire this back, we'd need to use the same
                    // callback pattern as Phase 2. For now, the git status is computed
                    // synchronously on first access in details_view via the cache.
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
                    // Open file with default handler — use ShellExecuteW so the
                    // opened app runs as an independent process (not a child).
                    #[cfg(windows)]
                    {
                        use std::ffi::OsStr;
                        use std::os::windows::ffi::OsStrExt;
                        use windows::Win32::UI::Shell::ShellExecuteW;
                        use windows::Win32::UI::WindowsAndMessaging::SW_SHOW;

                        let path_wide: Vec<u16> = OsStr::new(item.path.as_path())
                            .encode_wide()
                            .chain(std::iter::once(0))
                            .collect();

                        unsafe {
                            ShellExecuteW(
                                None,
                                windows::core::PCWSTR::null(),
                                windows::core::PCWSTR(path_wide.as_ptr()),
                                windows::core::PCWSTR::null(),
                                None,
                                SW_SHOW,
                            );
                        }
                    }
                }
                FileType::Symlink => {
                    // TODO: Resolve symlink and open target
                    tracing::debug!("Opening symlink: {}", item.name);
                }
            }
        }
    }

    pub fn navigate_from_edit(&mut self, path: FilesPath, cx: &mut Context<Self>) {
        self.state.nav_bar.edit_mode = false;
        self.navigate(path, cx);
    }

    /// Open the sibling folders dropdown for a given parent path.
    pub fn open_sibling_dropdown(&mut self, parent_path: FilesPath, cx: &mut Context<Self>) {
        self.state.nav_bar.load_siblings(&parent_path);
        let siblings = self.state.nav_bar.siblings.clone();
        self.state.nav_bar.open_dropdown_parent = Some(parent_path.clone());
        self.state.active_dialog = Some(DialogType::SiblingDropdown {
            parent_path,
            siblings,
        });
        cx.notify();
    }

    /// Navigate to a sibling folder from the dropdown.
    pub fn navigate_to_sibling(&mut self, sibling_path: FilesPath, cx: &mut Context<Self>) {
        self.state.active_dialog = None;
        self.navigate(sibling_path, cx);
    }

    pub fn open_in_new_tab(&mut self, path: FilesPath, cx: &mut Context<Self>) {
        let mode = self.state.main_view.view_mode.clone();
        let zoom = self.state.main_view.zoom_level;
        self.state.navigation.update(cx, |nav, _cx| {
            nav.tabs.add_tab_with_zoom(path, mode, zoom);
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
}

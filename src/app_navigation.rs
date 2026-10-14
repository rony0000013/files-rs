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
        // Cancel any previous in-flight directory load immediately
        let cancel_token = self.state.nav_cancel_guard.reset();

        // Update navigation history
        self.state.navigation.update(cx, |nav, _cx| {
            nav.tabs.active_tab_mut().navigate(path.clone());
        });

        self.state.main_view.primary_path = path.clone();
        self.state.nav_bar.update_path(path.clone());
        cx.notify();

        // Sync sidebar active item across all platforms
        self.state.sidebar.set_active(&path);

        // Status bar counts will be updated when items finish loading (Phase 1 callback).
        self.state.status_bar.sync(0, 0, 0);

        // Update git info in status bar
        let branch = GitService::get_branch(&path);
        self.state.status_bar.show_git_actions = branch.is_some();
        self.state.status_bar.git_branch = branch;

        // Start watching this folder for changes
        self.stop_file_watcher(false);
        self.start_file_watcher(path.clone());

        // If in split view, also watch the secondary pane
        let pane_layout = self.state.navigation.read(cx).pane_layout;
        if pane_layout != crate::core::navigation_manager::PaneLayout::Single {
            if let Some(secondary_path) = self
                .state
                .navigation
                .read(cx)
                .tabs
                .active_tab()
                .secondary_pane_path
                .clone()
            {
                self.start_file_watcher_secondary(secondary_path);
            }
        }

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
                if cancel_token.is_cancelled() {
                    return;
                }
                tracing::info!("Progressive loading started for: {:?}", path_clone);
                // =================================================================
                // PHASE 1: Fast initial listing (uses entry.file_type() — no stat)
                // =================================================================
                let token_p1 = cancel_token.clone();
                let fast_items: Vec<crate::domain::metadata::FileItem> = async_cx
                    .background_executor()
                    .spawn({
                        let p = path_clone.clone();
                        async move {
                            let mut items = Vec::new();
                            if let Ok(rd) = std::fs::read_dir(p.as_path()) {
                                for entry in rd.filter_map(|e| e.ok()) {
                                    if token_p1.is_cancelled() {
                                        break;
                                    }
                                    let path = FilesPath::new(entry.path());
                                    let name = entry.file_name().to_string_lossy().into_owned();
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
                                    let is_hidden = name.starts_with('.');
                                    items.push(crate::domain::metadata::FileItem {
                                        name,
                                        path,
                                        file_type,
                                        size: 0,
                                        is_hidden,
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

                if cancel_token.is_cancelled() {
                    return;
                }

                let total_count = fast_items.len();
                let indexed_paths: Vec<(usize, FilesPath)> = fast_items
                    .iter()
                    .enumerate()
                    .map(|(idx, i)| (idx, i.path.clone()))
                    .collect();

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
                            this.state.main_view.bump_items_revision();
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
                            let file_count = this.state.main_view.items.len() - folder_count;
                            this.state.status_bar.sync(
                                this.state.main_view.items.len(),
                                folder_count,
                                file_count,
                            );
                            let branch = GitService::get_branch(&p1);
                            this.state.status_bar.show_git_actions = branch.is_some();
                            this.state.status_bar.git_branch = branch;

                            this.state.main_view.icon_paths = this
                                .state
                                .icon_cache
                                .lock()
                                .map(|cache| {
                                    crate::app_state::icon_paths_for_dir(
                                        &cache,
                                        p1.as_path(),
                                    )
                                })
                                .unwrap_or_default();
                            cx.notify();
                        })
                        .ok();
                }

                // =================================================================
                // PHASE 2: Enrich items with full metadata in O(1)-indexed batches
                // Uses 2,048-item batches and direct slice indexing (O(N) total,
                // eliminating the previous O(N^2) linear find + 500x vector clones).
                // =================================================================
                if !indexed_paths.is_empty() {
                    let batch_size = 2048;
                    for chunk in indexed_paths.chunks(batch_size) {
                        if cancel_token.is_cancelled() {
                            return;
                        }
                        let chunk = chunk.to_vec();
                        let token_p2 = cancel_token.clone();
                        let enriched = async_cx
                            .background_executor()
                            .spawn(async move {
                                let mut results: Vec<(
                                    usize,
                                    u64,
                                    Option<std::time::SystemTime>,
                                    Option<std::time::SystemTime>,
                                    bool,
                                    bool,
                                )> = Vec::with_capacity(chunk.len());
                                for (idx, path) in &chunk {
                                    if token_p2.is_cancelled() {
                                        break;
                                    }
                                    if let Ok(meta) = std::fs::metadata(path.as_path()) {
                                        let size = meta.len();
                                        let modified = meta.modified().ok();
                                        let created = meta.created().ok();
                                        let mut is_hidden = path
                                            .file_name()
                                            .map(|n| n.to_string_lossy().starts_with('.'))
                                            .unwrap_or(false);
                                        let mut is_system = false;
                                        #[cfg(windows)]
                                        {
                                            use std::os::windows::fs::MetadataExt;
                                            let attrs = meta.file_attributes();
                                            is_hidden = is_hidden
                                                || (attrs
                                                    & windows::Win32::Storage::FileSystem::FILE_ATTRIBUTE_HIDDEN.0)
                                                    != 0;
                                            is_system = (attrs
                                                & windows::Win32::Storage::FileSystem::FILE_ATTRIBUTE_SYSTEM.0)
                                                != 0;
                                        }
                                        results.push((
                                            *idx,
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

                        if cancel_token.is_cancelled() {
                            return;
                        }

                        if !enriched.is_empty() {
                            let target_dir = path_clone.clone();
                            this_weak
                                .update(&mut async_cx, |this, cx| {
                                    if this.state.main_view.primary_path != target_dir {
                                        return;
                                    }
                                    for (idx, size, modified, created, is_hidden, is_system) in
                                        enriched
                                    {
                                        if let Some(item) =
                                            this.state.main_view.items.get_mut(idx)
                                        {
                                            item.size = size;
                                            item.modified = modified;
                                            item.created = created;
                                            item.is_hidden = is_hidden;
                                            item.is_system = is_system;
                                        }
                                    }
                                    this.state.main_view.bump_items_revision();
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
                // PHASE 2.5: Single-pass Git status scan in background
                // =================================================================
                if !cancel_token.is_cancelled() {
                    let git_path = path_clone.clone();
                    let git_weak = this_weak.clone();
                    let mut git_cx = async_cx.clone();
                    let token_git = cancel_token.clone();
                    let token_outer = cancel_token.clone();
                    let fg = git_cx.foreground_executor().clone();
                    let bg = git_cx.background_executor().clone();
                    fg.spawn(async move {
                        let git_statuses = bg
                            .spawn(async move {
                                if token_git.is_cancelled() {
                                    return std::collections::HashMap::new();
                                }
                                GitService::get_directory_statuses(&git_path)
                            })
                            .await;
                        if token_outer.is_cancelled() || git_statuses.is_empty() {
                            return;
                        }
                        git_weak
                            .update(&mut git_cx, |this, cx| {
                                this.state.main_view.git_status_cache.extend(git_statuses);
                                cx.notify();
                            })
                            .ok();
                    })
                    .detach();
                }

                // =================================================================
                // PHASE 3: Start icon extraction for visible viewport items
                // =================================================================
                if !cancel_token.is_cancelled() {
                    let p3 = path_clone.clone();
                    this_weak
                        .update(&mut async_cx, |this, cx| {
                            tracing::info!(
                                "Phase 3: Starting icon extraction for {} items in {:?}",
                                this.state.main_view.items.len(),
                                p3
                            );
                            this.load_icons(cx);
                            this.state.main_view.icon_paths = this
                                .state
                                .icon_cache
                                .lock()
                                .map(|cache| {
                                    crate::app_state::icon_paths_for_dir(
                                        &cache,
                                        p3.as_path(),
                                    )
                                })
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
        }

        // Check if we should switch to Home or Recycle Bin view
        let path_str = path.to_string_lossy();
        if path_str.is_empty() || path_str == "Home" {
            self.state.main_view.target = crate::ui::views::main_view::ViewTarget::Home;
            if let Ok(tags) = self.state.tag_service.get_all_tags() {
                self.state.main_view.home_tags_cache = tags
                    .into_iter()
                    .map(|(name, count)| (name, "#808080".to_string(), count))
                    .collect();
            } else {
                self.state.main_view.home_tags_cache = Vec::new();
            }
        } else if path_str == "shell:RecycleBinFolder" || path_str.contains("RecycleBin") {
            self.state.main_view.target = crate::ui::views::main_view::ViewTarget::RecycleBin;
            #[cfg(windows)]
            {
                let service = RecycleBinService::new();
                if let Ok(items) = service.list_contents_sync() {
                    self.state.main_view.recycle_bin_items = items;
                } else {
                    self.state.main_view.recycle_bin_items = Vec::new();
                }
            }
            #[cfg(not(windows))]
            {
                self.state.main_view.recycle_bin_items = Vec::new();
            }
        } else {
            self.state.main_view.target =
                crate::ui::views::main_view::ViewTarget::Directory(path.clone());
        }

        self.save_session(cx);
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
                self.state.navigation.update(cx, |nav, _cx| {
                    nav.tabs.active_tab_mut().navigate(home_path.clone());
                });
                self.navigate(home_path, cx);
            }
            Err(_) => {
                if let Some(home) = dirs::home_dir() {
                    self.navigate(crate::core::path_util::FilesPath::from(home), cx);
                }
            }
        }
    }

    /// Navigate to the root of the current drive/path
    pub fn navigate_to_root(&mut self, cx: &mut Context<Self>) {
        let path = self.state.main_view.primary_path.clone();
        let mut current = path.clone();
        while let Ok(parent) = NavigationService::get_parent_path(&current) {
            if parent.as_path().to_string_lossy() == current.as_path().to_string_lossy() {
                break;
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
                    self.navigate(item.path.clone(), cx);
                }
                FileType::File => {
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
                    #[cfg(target_os = "macos")]
                    {
                        let _ = std::process::Command::new("open")
                            .arg(item.path.as_path())
                            .spawn();
                    }
                    #[cfg(all(unix, not(target_os = "macos")))]
                    {
                        let _ = std::process::Command::new("xdg-open")
                            .arg(item.path.as_path())
                            .spawn();
                    }
                }
                FileType::Symlink => {
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
        let progress_engine = self.state.progress_engine.clone();
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
            path.clone(),
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

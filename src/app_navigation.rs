use crate::app_state::{AppState, AppWindow};
use crate::config::config::ViewMode;
use crate::core::navigation_manager::PaneFocus;
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
    /// Navigate the currently active pane (Primary or Secondary in Dual-Pane mode).
    pub fn navigate(&mut self, path: FilesPath, cx: &mut Context<Self>) {
        let pane_focus = self.state.active_pane_focus();
        self.navigate_pane(path, pane_focus, cx);
    }

    /// Navigate a specific pane (`PaneFocus::Primary` or `PaneFocus::Secondary`) independently.
    pub fn navigate_pane(
        &mut self,
        path: FilesPath,
        pane_focus: PaneFocus,
        cx: &mut Context<Self>,
    ) {
        let is_secondary = pane_focus == PaneFocus::Secondary
            && self.state.main_view.pane_type == crate::ui::views::main_view::PaneType::Dual;

        let current_pane_path = if is_secondary {
            self.state.secondary_view.primary_path.clone()
        } else {
            self.state.main_view.primary_path.clone()
        };

        let same_path_refresh = path == current_pane_path;

        // Same-path refresh (file-watcher events): keep in-flight pipelines alive.
        // Directory change: reset the cancellation guard for that pane and reset scroll/selection.
        let cancel_token = if is_secondary {
            if same_path_refresh {
                self.state.nav_cancel_guard_secondary.token()
            } else {
                self.state.secondary_view.selected_indices.clear();
                self.state.secondary_view.selection_anchor_index = None;
                self.state.secondary_view.last_clicked_index = None;
                self.state.secondary_view.renaming_index = None;
                self.state.secondary_view.bouncy_scroll.set_immediate_offset(0.0);
                self.state
                    .secondary_view
                    .uniform_list_scroll_handle
                    .0
                    .borrow()
                    .base_handle
                    .set_offset(point(px(0.0), px(0.0)));
                self.state.nav_cancel_guard_secondary.reset()
            }
        } else if same_path_refresh {
            self.state.nav_cancel_guard.token()
        } else {
            self.state.main_view.selected_indices.clear();
            self.state.main_view.selection_anchor_index = None;
            self.state.main_view.last_clicked_index = None;
            self.state.main_view.renaming_index = None;
            self.state.main_view.bouncy_scroll.set_immediate_offset(0.0);
            self.state.main_view.side_pane_bouncy_scroll.set_immediate_offset(0.0);
            self.state.main_view.side_pane_scroll_handle.set_offset(gpui::point(px(0.0), px(0.0)));
            self.state
                .main_view
                .uniform_list_scroll_handle
                .0
                .borrow()
                .base_handle
                .set_offset(point(px(0.0), px(0.0)));
            self.state.nav_cancel_guard.reset()
        };

        // Update navigation history on the active tab for the target pane
        self.state.navigation.update(cx, |nav, _cx| {
            if is_secondary {
                nav.tabs.active_tab_mut().navigate_secondary(path.clone());
            } else {
                nav.tabs.active_tab_mut().navigate(path.clone());
            }
        });

        if is_secondary {
            self.state.secondary_view.primary_path = path.clone();
            self.state.main_view.secondary_path = Some(path.clone());
        } else {
            self.state.main_view.primary_path = path.clone();
        }

        // If the navigated pane is the currently focused pane, sync nav_bar, sidebar, and status_bar
        let is_focused_pane = self.state.active_pane_focus() == pane_focus
            || (!is_secondary && self.state.main_view.pane_type == crate::ui::views::main_view::PaneType::Single);
        if is_focused_pane {
            self.state.nav_bar.update_path(path.clone());
            self.state.sidebar.set_active(&path);
            if !same_path_refresh {
                self.state.status_bar.sync(0, 0, 0);
                self.state.status_bar.show_git_actions = false;
                self.state.status_bar.git_branch = None;
            }
        }
        cx.notify();

        // Start watching this folder for changes (only recreate watcher if path changed or not yet watching)
        if is_secondary {
            if !same_path_refresh || self.file_watcher_secondary.is_none() {
                self.stop_file_watcher(true);
                self.start_file_watcher_secondary(path.clone());
            }
        } else if !same_path_refresh || self.file_watcher.is_none() {
            self.stop_file_watcher(false);
            self.start_file_watcher(path.clone());
        }

        let active_tab = self.state.navigation.read(cx).tabs.active_tab().clone();
        let settings_model = self.state.settings.read(cx);
        let prefs = settings_model.config().get_folder_preferences(&path);
        let app_prefs = settings_model.config().preferences.clone();
        let calc_folder_sizes = settings_model.config().view.calculate_folder_sizes;
        drop(settings_model);

        {
            let target_view = if is_secondary {
                &mut self.state.secondary_view
            } else {
                &mut self.state.main_view
            };

            if target_view.view_lock_enabled {
                target_view.view_mode = prefs.view_mode.clone();
                target_view.zoom_level = prefs.zoom_level;
            } else {
                target_view.view_mode = active_tab.view_mode.clone();
                target_view.zoom_level =
                    active_tab.get_zoom_for_view(active_tab.view_mode.clone());
            }

            if !same_path_refresh {
                target_view.folder_background =
                    crate::ui::views::main_view::FolderBackground::default();
            }

            target_view.show_dot_files = app_prefs.show_dot_files;
            target_view.show_hidden_files = app_prefs.show_hidden_files;
            target_view.show_extensions = app_prefs.show_file_extensions;
            target_view.show_system_files = app_prefs.show_system_files;
        }

        tracing::info!(
            "Navigating {:?} pane to {:?}, using view mode: {:?}",
            pane_focus,
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
                // =================================================================
                // PHASE 1: Fast initial listing (uses entry.file_type() — no stat)
                // =================================================================
                let token_p1 = cancel_token.clone();
                let (fast_items, desktop_ini_opt): (
                    Vec<crate::domain::metadata::FileItem>,
                    Option<crate::storage::desktop_ini::DesktopIni>,
                ) = async_cx
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
                                    let size = if matches!(file_type, FileType::Directory) {
                                        crate::storage::folder_size::peek_cached_size(path.as_path())
                                            .unwrap_or(0)
                                    } else {
                                        0
                                    };
                                    let is_hidden = name.starts_with('.');
                                    items.push(crate::domain::metadata::FileItem {
                                        name,
                                        path,
                                        file_type,
                                        size,
                                        is_hidden,
                                        is_system: false,
                                        modified: None,
                                        created: None,
                                        tags: Vec::new(),
                                    });
                                }
                            }
                            let ini = crate::storage::desktop_ini::DesktopIni::load(p.as_path());
                            (items, ini)
                        }
                    })
                    .await;

                if cancel_token.is_cancelled() {
                    return;
                }

                let indexed_paths: Vec<(usize, FilesPath)> = fast_items
                    .iter()
                    .enumerate()
                    .map(|(idx, i)| (idx, i.path.clone()))
                    .collect();

                // Show items immediately so the user sees content without delay
                {
                    let p1 = path_clone.clone();
                    this_weak
                        .update(&mut async_cx, |this, cx| {
                            if is_secondary {
                                this.state.secondary_view.items = fast_items;
                                this.state.secondary_view.bump_items_revision();
                                this.state.secondary_file_items.update(cx, |m, _| {
                                    m.items = this.state.secondary_view.items.clone();
                                    m.selected_indices =
                                        this.state.secondary_view.selected_indices.clone();
                                });
                                this.state.secondary_view.primary_path = p1.clone();
                                this.state.main_view.secondary_path = Some(p1.clone());
                            } else {
                                this.state.main_view.items = fast_items;
                                this.state.main_view.bump_items_revision();
                                this.state.file_items.update(cx, |m, _| {
                                    m.items = this.state.main_view.items.clone();
                                    m.selected_indices =
                                        this.state.main_view.selected_indices.clone();
                                });
                                this.state.main_view.primary_path = p1.clone();
                            }

                            {
                                let tv = if is_secondary {
                                    &mut this.state.secondary_view
                                } else {
                                    &mut this.state.main_view
                                };
                                if let Some(desktop_ini) = desktop_ini_opt {
                                    tv.folder_background.image_path = desktop_ini
                                        .background_image
                                        .map(crate::core::path_util::FilesPath::new);
                                    if let Some(opacity) = desktop_ini.background_opacity {
                                        tv.folder_background.opacity = opacity;
                                    }
                                } else {
                                    tv.folder_background =
                                        crate::ui::views::main_view::FolderBackground::default();
                                }
                            }

                            if this.state.active_pane_focus() == pane_focus {
                                this.state.nav_bar.update_path(p1.clone());
                                this.state.sidebar.set_active(&p1);
                                let items_ref = &this.state.pane_view(pane_focus).items;
                                let folder_count = items_ref
                                    .iter()
                                    .filter(|i| matches!(i.file_type, FileType::Directory))
                                    .count();
                                let file_count = items_ref.len() - folder_count;
                                this.state
                                    .status_bar
                                    .sync(items_ref.len(), folder_count, file_count);
                            }

                            this.sync_directory_tags(cx);
                            this.sync_pane_icon_paths();
                            cx.notify();
                        })
                        .ok();
                }

                // =================================================================
                // PHASE 2: Enrich items with full metadata in O(1)-indexed batches
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
                                        let size = if meta.is_dir() {
                                            crate::storage::folder_size::cached_size(path.as_path())
                                                .unwrap_or(0)
                                        } else {
                                            meta.len()
                                        };
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
                                    if is_secondary {
                                        if this.state.secondary_view.primary_path != target_dir {
                                            return;
                                        }
                                        for (idx, size, modified, created, is_hidden, is_system) in
                                            enriched
                                        {
                                            if let Some(item) =
                                                this.state.secondary_view.items.get_mut(idx)
                                            {
                                                item.size = size;
                                                item.modified = modified;
                                                item.created = created;
                                                item.is_hidden = is_hidden;
                                                item.is_system = is_system;
                                            }
                                        }
                                        this.state.secondary_view.bump_items_revision();
                                        this.state.secondary_file_items.update(cx, |m, _| {
                                            m.items = this.state.secondary_view.items.clone();
                                        });
                                    } else {
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
                                    }
                                    cx.notify();
                                })
                                .ok();
                        }
                    }
                }

                // =================================================================
                // PHASE 2.5: Background Git branch + single-pass status scan
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
                        let (branch, git_statuses) = bg
                            .spawn(async move {
                                if token_git.is_cancelled() {
                                    return (None, std::collections::HashMap::new());
                                }
                                let b = GitService::get_branch(&git_path);
                                let statuses = if b.is_some() {
                                    GitService::get_directory_statuses(&git_path)
                                } else {
                                    std::collections::HashMap::new()
                                };
                                (b, statuses)
                            })
                            .await;
                        if token_outer.is_cancelled() {
                            return;
                        }
                        git_weak
                            .update(&mut git_cx, |this, cx| {
                                if this.state.active_pane_focus() == pane_focus {
                                    this.state.status_bar.show_git_actions = branch.is_some();
                                    this.state.status_bar.git_branch = branch;
                                }
                                if !git_statuses.is_empty() {
                                    this.state
                                        .main_view
                                        .git_status_cache
                                        .extend(git_statuses.clone());
                                    this.state
                                        .secondary_view
                                        .git_status_cache
                                        .extend(git_statuses);
                                }
                                cx.notify();
                            })
                            .ok();
                    })
                    .detach();
                }

                // =================================================================
                // PHASE 2.6: Background subfolder size pre-index for current directory
                // =================================================================
                if calc_folder_sizes && !cancel_token.is_cancelled() {
                    let dir_target = path_clone.clone();
                    let sz_weak = this_weak.clone();
                    let mut sz_cx = async_cx.clone();
                    let token_sz = cancel_token.clone();
                    let token_sz_outer = cancel_token.clone();
                    let fg = sz_cx.foreground_executor().clone();
                    let bg = sz_cx.background_executor().clone();
                    let uncached_dirs: Vec<(usize, FilesPath)> = sz_weak
                        .update(&mut sz_cx, |this, _| {
                            let items_ref = if is_secondary {
                                &this.state.secondary_view.items
                            } else {
                                &this.state.main_view.items
                            };
                            items_ref
                                .iter()
                                .enumerate()
                                .filter(|(_, it)| {
                                    matches!(it.file_type, FileType::Directory)
                                        && crate::storage::folder_size::peek_cached_size(
                                            it.path.as_path(),
                                        )
                                        .is_none()
                                })
                                .take(64)
                                .map(|(idx, it)| (idx, it.path.clone()))
                                .collect()
                        })
                        .unwrap_or_default();

                    if !uncached_dirs.is_empty() {
                        fg.spawn(async move {
                            let computed = bg
                                .spawn(async move {
                                    let mut out = Vec::with_capacity(uncached_dirs.len());
                                    for (idx, fp) in uncached_dirs {
                                        if token_sz.is_cancelled() {
                                            break;
                                        }
                                        let sz = crate::storage::folder_size::compute_and_store(
                                            fp.as_path(),
                                        );
                                        out.push((idx, fp, sz));
                                    }
                                    out
                                })
                                .await;
                            if token_sz_outer.is_cancelled() || computed.is_empty() {
                                return;
                            }
                            sz_weak
                                .update(&mut sz_cx, |this, cx| {
                                    let view = if is_secondary {
                                        &mut this.state.secondary_view
                                    } else {
                                        &mut this.state.main_view
                                    };
                                    if view.primary_path != dir_target {
                                        return;
                                    }
                                    for (idx, fp, sz) in computed {
                                        if let Some(it) = view.items.get_mut(idx) {
                                            if it.path == fp {
                                                it.size = sz;
                                            }
                                        }
                                    }
                                    view.bump_items_revision();
                                    cx.notify();
                                })
                                .ok();
                        })
                        .detach();
                    }
                }

                // =================================================================
                // PHASE 3: Start icon extraction for visible viewport items
                // =================================================================
                if !cancel_token.is_cancelled() {
                    this_weak
                        .update(&mut async_cx, |this, cx| {
                            this.load_icons(cx);
                            this.sync_pane_icon_paths();
                            cx.notify();
                        })
                        .ok();
                }
            })
            .detach();

        // Check if we should switch to Home or Recycle Bin view (Primary pane target)
        if !is_secondary {
            let path_str = path.to_string_lossy();
            if path_str.is_empty() || path_str == "Home" {
                self.state.main_view.target = crate::ui::views::main_view::ViewTarget::Home;
                self.refresh_home_tags_cache(cx);
            } else if path_str == "shell:RecycleBinFolder" || path_str.contains("RecycleBin") {
                self.state.main_view.target = crate::ui::views::main_view::ViewTarget::RecycleBin;
                self.state.main_view.selected_indices.clear();
                let rb_weak = cx.weak_entity();
                let mut rb_cx = cx.to_async();
                let bg = cx.background_executor().spawn(async move {
                    let service = RecycleBinService::new();
                    service.list_contents_sync().unwrap_or_default()
                });
                cx.foreground_executor()
                    .spawn(async move {
                        let items = bg.await;
                        rb_weak
                            .update(&mut rb_cx, |this, cx| {
                                this.state.main_view.recycle_bin_items = items;
                                cx.notify();
                            })
                            .ok();
                    })
                    .detach();
            } else {
                self.state.main_view.target =
                    crate::ui::views::main_view::ViewTarget::Directory(path.clone());
            }
        } else {
            self.state.secondary_view.target =
                crate::ui::views::main_view::ViewTarget::Directory(path.clone());
        }

        self.save_session(cx);
        self.save_session_path(cx);
    }

    /// Synchronize `icon_paths` for both Primary and Secondary panes from the shared LRU cache.
    pub fn sync_pane_icon_paths(&mut self) {
        if let Ok(cache) = self.state.icon_cache.lock() {
            let mut map = crate::app_state::icon_paths_for_dir(
                &cache,
                self.state.main_view.primary_path.as_path(),
            );
            if self.state.main_view.pane_type == crate::ui::views::main_view::PaneType::Dual {
                let sec_map = crate::app_state::icon_paths_for_dir(
                    &cache,
                    self.state.secondary_view.primary_path.as_path(),
                );
                self.state.secondary_view.icon_paths = sec_map.clone();
                map.extend(sec_map);
            }
            self.state.main_view.icon_paths = map;
        }
    }

    pub fn navigate_back(&mut self, cx: &mut Context<Self>) {
        let pane_focus = self.state.active_pane_focus();
        let maybe_path = self.state.navigation.update(cx, |nav, _cx| {
            if pane_focus == PaneFocus::Secondary {
                nav.tabs.active_tab_mut().navigate_back_secondary()
            } else {
                nav.tabs.active_tab_mut().navigate_back()
            }
        });
        if let Some(path) = maybe_path {
            self.navigate_pane(path, pane_focus, cx);
        }
    }

    pub fn navigate_forward(&mut self, cx: &mut Context<Self>) {
        let pane_focus = self.state.active_pane_focus();
        let maybe_path = self.state.navigation.update(cx, |nav, _cx| {
            if pane_focus == PaneFocus::Secondary {
                nav.tabs.active_tab_mut().navigate_forward_secondary()
            } else {
                nav.tabs.active_tab_mut().navigate_forward()
            }
        });
        if let Some(path) = maybe_path {
            self.navigate_pane(path, pane_focus, cx);
        }
    }

    pub fn navigate_up(&mut self, cx: &mut Context<Self>) {
        let pane_focus = self.state.active_pane_focus();
        let current = self.state.pane_view(pane_focus).primary_path.clone();
        if let Ok(parent) = NavigationService::get_parent_path(&current) {
            self.navigate_pane(parent, pane_focus, cx);
        }
    }

    /// Navigate to the Home view (quick access dashboard)
    pub fn navigate_home(&mut self, cx: &mut Context<Self>) {
        match crate::core::navigation_service::NavigationService::get_home_path() {
            Ok(home_path) => {
                self.navigate(home_path, cx);
            }
            Err(_) => {
                if let Some(home) = dirs::home_dir() {
                    self.navigate(crate::core::path_util::FilesPath::from(home), cx);
                }
            }
        }
    }

    /// Navigate to the root of the current drive/path in the active pane
    pub fn navigate_to_root(&mut self, cx: &mut Context<Self>) {
        let pane_focus = self.state.active_pane_focus();
        let path = self.state.pane_view(pane_focus).primary_path.clone();
        let mut current = path.clone();
        while let Ok(parent) = NavigationService::get_parent_path(&current) {
            if parent.as_path().to_string_lossy() == current.as_path().to_string_lossy() {
                break;
            }
            current = parent;
        }
        if current != path {
            self.navigate_pane(current, pane_focus, cx);
        }
    }

    /// Open the selected item (file or folder) in the currently active pane.
    pub fn open_item(&mut self, index: usize, cx: &mut Context<Self>) {
        let pane_focus = self.state.active_pane_focus();
        self.open_item_in_pane(pane_focus, index, cx);
    }

    /// Open the item at `index` in `pane_focus` (navigating into directories in that pane).
    pub fn open_item_in_pane(
        &mut self,
        pane_focus: PaneFocus,
        index: usize,
        cx: &mut Context<Self>,
    ) {
        self.focus_pane(pane_focus, cx);
        let item_opt = self.state.pane_view(pane_focus).items.get(index).cloned();
        if let Some(item) = item_opt {
            match item.file_type {
                FileType::Directory => {
                    self.navigate_pane(item.path.clone(), pane_focus, cx);
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
                    if item.path.as_path().is_dir() {
                        self.navigate_pane(item.path.clone(), pane_focus, cx);
                    } else {
                        tracing::debug!("Opening symlink: {}", item.name);
                    }
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
        let mode = self.state.active_view().view_mode.clone();
        let zoom = self.state.active_view().zoom_level;
        let new_idx = self.state.navigation.update(cx, |nav, _cx| {
            nav.tabs.add_tab_with_zoom(path, mode, zoom);
            nav.tabs.active_tab_index
        });
        self.state.tab_drag.new_tab_anim = Some((new_idx, 0.0));
        self.ensure_active_tab_visible(cx);
        self.load_tab_thumbnails(cx);
        self.sync_active_tab_to_views(cx);
        cx.notify();
    }

    pub fn open_in_new_window(&mut self, path: FilesPath, cx: &mut Context<Self>) {
        let mode = self.state.active_view().view_mode.clone();
        let zoom = self.state.active_view().zoom_level;
        let tab = crate::core::navigation_manager::TabState::new_with_zoom(path, mode, zoom);
        self.open_tab_state_in_new_window(tab, None, cx);
    }

    /// Open a new independent `AppWindow` initialized with `tab` (preserving its full history,
    /// dual-pane secondary path, view mode, zoom level, and pin state) at optional screen coordinates.
    pub fn open_tab_state_in_new_window(
        &mut self,
        tab: crate::core::navigation_manager::TabState,
        screen_origin: Option<(f32, f32)>,
        cx: &mut Context<Self>,
    ) {
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
        let primary_path = tab.path.clone();
        let view_mode = tab.view_mode.clone();
        let has_secondary = tab.secondary_pane_path.is_some();
        let primary_path_for_nav = primary_path.clone();

        let new_nav = cx.new(move |_cx| {
            let mut nav =
                crate::core::navigation_manager::NavigationState::new(primary_path_for_nav.clone(), view_mode);
            nav.tabs.tabs = vec![tab];
            nav.tabs.active_tab_index = 0;
            if has_secondary {
                nav.pane_layout = crate::core::navigation_manager::PaneLayout::Vertical;
            }
            nav
        });

        let (mica_enabled, mica_alt, transparent_window) = {
            let s = self.state.settings.read(cx);
            let app = &s.config().appearance;
            (app.mica_enabled, app.mica_alt, app.transparent_window)
        };

        let app_state = AppState::new(
            primary_path,
            file_items,
            self.state.settings.clone(),
            self.state.localization.clone(),
            new_nav,
            progress_engine.clone(),
            db,
            cx,
        );

        let origin = if let Some((sx, sy)) = screen_origin {
            gpui::point(gpui::px((sx - 100.0).max(0.0)), gpui::px((sy - 16.0).max(0.0)))
        } else {
            let cur = self.state.last_window_origin;
            gpui::point(cur.x + gpui::px(36.0), cur.y + gpui::px(36.0))
        };

        let window_background = if transparent_window {
            gpui::WindowBackgroundAppearance::Transparent
        } else {
            gpui::WindowBackgroundAppearance::Opaque
        };

        let options = gpui::WindowOptions {
            window_bounds: Some(gpui::WindowBounds::Windowed(gpui::Bounds {
                origin,
                size: gpui::size(gpui::Pixels::from(1200.0), gpui::Pixels::from(760.0)),
            })),
            window_min_size: Some(gpui::size(gpui::Pixels::from(500.0), gpui::Pixels::from(340.0))),
            titlebar: None,
            window_background,
            ..gpui::WindowOptions::default()
        };

        cx.open_window(options, move |window, cx| {
            #[cfg(windows)]
            if mica_enabled {
                if mica_alt {
                    let _ = window_vibrancy::apply_tabbed(&*window, Some(true));
                } else {
                    let _ = window_vibrancy::apply_mica(&*window, Some(true));
                }
            }
            let rx = progress_engine.subscribe();
            cx.new(|cx| AppWindow::new(app_state, rx, window, cx))
        })
        .expect("Failed to open window");
    }
}

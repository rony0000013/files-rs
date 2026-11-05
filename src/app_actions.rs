use crate::app_state::{AppWindow, SidebarPinnedDragState, TagState};
use crate::config::config::ViewMode;
use crate::config::config::SortBy;
use crate::core::path_util::FilesPath;
use crate::ui::components::clipboard_manager::{ClipboardItem, ClipboardState};
use crate::ui::components::dialogs::{DialogType, SettingsTab};
use crate::ui::views::main_view::GroupDragState;
use gpui::*;

#[cfg(windows)]
use crate::platform::windows::recycle_bin_service::RecycleBinService;

impl AppWindow {
    pub fn toggle_group_collapse(&mut self, key: String, cx: &mut Context<Self>) {
        let pane_focus = self.state.active_pane_focus();
        let view = self.state.pane_view_mut(pane_focus);
        if view.collapsed_groups.contains(&key) {
            view.collapsed_groups.remove(&key);
        } else {
            view.collapsed_groups.insert(key);
        }
        cx.notify();
    }

    /// Synchronize `tags_cache` and `item.tags` across both panes from `tag_service`.
    pub fn sync_directory_tags(&mut self, cx: &mut Context<Self>) {
        let all_map = self
            .state
            .tag_service
            .get_all_file_tags_map()
            .unwrap_or_default();

        for is_secondary in [false, true] {
            let view = if is_secondary {
                &mut self.state.secondary_view
            } else {
                &mut self.state.main_view
            };
            view.tags_cache = all_map.clone();
            for item in &mut view.items {
                let key = item.path.to_string_lossy().to_string();
                item.tags = all_map.get(&key).cloned().unwrap_or_default();
            }
            view.bump_items_revision();
        }
        self.state.file_items.update(cx, |m, _| {
            m.items = self.state.main_view.items.clone();
        });
        self.state.secondary_file_items.update(cx, |m, _| {
            m.items = self.state.secondary_view.items.clone();
        });
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
    pub fn add_tag_to_selected(&mut self, tag: String, cx: &mut Context<Self>) {
        let active_view = self.state.active_view();
        let paths: Vec<FilesPath> = active_view
            .selected_indices
            .iter()
            .filter_map(|&idx| active_view.items.get(idx).map(|it| it.path.clone()))
            .collect();
        for path in paths {
            if let Err(e) = self
                .state
                .tag_service
                .add_tag(path.as_path().to_string_lossy().as_ref(), &tag)
            {
                tracing::error!("Failed to add tag {} to {:?}: {}", tag, path, e);
            }
        }
        self.sync_directory_tags(cx);
        self.refresh_home_tags_cache(cx);
        cx.notify();
    }

    /// Remove a tag from selected items
    pub fn remove_tag_from_selected(&mut self, tag: &str, cx: &mut Context<Self>) {
        let active_view = self.state.active_view();
        let paths: Vec<FilesPath> = active_view
            .selected_indices
            .iter()
            .filter_map(|&idx| active_view.items.get(idx).map(|it| it.path.clone()))
            .collect();
        for path in paths {
            if let Err(e) = self
                .state
                .tag_service
                .remove_tag(path.as_path().to_string_lossy().as_ref(), tag)
            {
                tracing::error!("Failed to remove tag {} from {:?}: {}", tag, path, e);
            }
        }
        self.sync_directory_tags(cx);
        self.refresh_home_tags_cache(cx);
        cx.notify();
    }

    /// Add a tag to a specific file
    pub fn add_tag_to_file(&mut self, path: &FilesPath, tag: String, cx: &mut Context<Self>) {
        if let Err(e) = self
            .state
            .tag_service
            .add_tag(path.as_path().to_string_lossy().as_ref(), &tag)
        {
            tracing::error!("Failed to add tag {} to {:?}: {}", tag, path, e);
        }
        self.sync_directory_tags(cx);
        self.refresh_home_tags_cache(cx);
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
                    // Open file with default handler — independent process (no parent-child)
                    #[cfg(windows)]
                    {
                        use std::ffi::OsStr;
                        use std::os::windows::ffi::OsStrExt;
                        use windows::Win32::UI::Shell::ShellExecuteW;
                        use windows::Win32::UI::WindowsAndMessaging::SW_SHOW;

                        let path_wide: Vec<u16> = OsStr::new(path.as_path())
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
        let pane_focus = self.state.active_pane_focus();
        let current = self.state.pane_view(pane_focus).group_by;
        let next = match current {
            GroupColumn::None => GroupColumn::Name,
            GroupColumn::Name => GroupColumn::FileType,
            GroupColumn::FileType => GroupColumn::Size,
            GroupColumn::Size => GroupColumn::DateModified,
            GroupColumn::DateModified => GroupColumn::DateCreated,
            GroupColumn::DateCreated => GroupColumn::SyncStatus,
            GroupColumn::SyncStatus => GroupColumn::Tag,
            GroupColumn::Tag => GroupColumn::None,
        };
        self.set_group_by(next, cx);
    }

    /// Select all items belonging to `group_key` in the active pane (triggered by group header "Select group" pill).
    pub fn select_all_in_group(&mut self, group_key: &str, additive: bool, cx: &mut Context<Self>) {
        let pane_focus = self.state.active_pane_focus();
        {
            let view = self.state.pane_view_mut(pane_focus);
            let visible = view.visible_items_rc();
            if !additive {
                view.selected_indices.clear();
            }
            for &idx in visible.iter() {
                if let Some(item) = view.items.get(idx) {
                    if view.get_group_key(item).as_deref() == Some(group_key) {
                        view.selected_indices.insert(idx);
                    }
                }
            }
        }
        self.sync_pane_selection_model(pane_focus, cx);
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
    pub(crate) fn reorder_pinned_items(
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

    // === Clipboard operations ===

    pub fn cut_selected(&mut self, cx: &mut Context<Self>) {
        let active_view = self.state.active_view();
        let indices: Vec<usize> = active_view
            .selected_indices
            .iter()
            .copied()
            .collect();
        let items: Vec<ClipboardItem> = indices
            .iter()
            .filter_map(|&idx| {
                active_view
                    .items
                    .get(idx)
                    .map(|item| ClipboardItem::new(item.path.as_path().to_path_buf()))
            })
            .collect();

        if !items.is_empty() {
            let cut_set: std::collections::HashSet<std::path::PathBuf> =
                items.iter().map(|i| i.path.clone()).collect();
            self.state.main_view.cut_paths = cut_set.clone();
            self.state.secondary_view.cut_paths = cut_set;
            self.state.clipboard = ClipboardState::Cut(items.clone());
            tracing::info!("Cut {} items to clipboard", items.len());
        }
        cx.notify();
    }

    pub fn copy_selected(&mut self, cx: &mut Context<Self>) {
        self.state.main_view.cut_paths.clear();
        self.state.secondary_view.cut_paths.clear();
        let active_view = self.state.active_view();
        let indices: Vec<usize> = active_view
            .selected_indices
            .iter()
            .copied()
            .collect();
        let items: Vec<ClipboardItem> = indices
            .iter()
            .filter_map(|&idx| {
                active_view
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

    /// Copy the first selected item's full path as text (Files parity:
    /// "Copy item path", Ctrl+Shift+C). Falls back to the current folder.
    pub fn copy_item_path(&mut self, cx: &mut Context<Self>) {
        let active_view = self.state.active_view();
        let path_str = active_view
            .selected_indices
            .iter()
            .next()
            .and_then(|&idx| active_view.items.get(idx))
            .map(|item| item.path.as_path().to_string_lossy().to_string())
            .unwrap_or_else(|| {
                active_view
                    .primary_path
                    .as_path()
                    .to_string_lossy()
                    .to_string()
            });
        cx.write_to_clipboard(gpui::ClipboardItem::new_string(path_str));
        cx.notify();
    }

    /// Create a "New folder" in the current directory and move the current
    /// selection into it (Files "Create folder with selection").
    pub fn create_folder_with_selection(&mut self, cx: &mut Context<Self>) {
        let dir = self
            .state
            .main_view
            .primary_path
            .as_path()
            .to_path_buf();
        let mut name = "New folder".to_string();
        let mut i = 2;
        while dir.join(&name).exists() {
            name = format!("New folder ({})", i);
            i += 1;
        }
        let new_dir = dir.join(&name);
        if std::fs::create_dir(&new_dir).is_err() {
            return;
        }
        let selected: Vec<std::path::PathBuf> = self
            .state
            .main_view
            .selected_indices
            .iter()
            .filter_map(|&idx| self.state.main_view.items.get(idx))
            .map(|item| item.path.as_path().to_path_buf())
            .collect();
        for src in selected {
            if let Some(file_name) = src.file_name() {
                let _ = std::fs::rename(&src, new_dir.join(file_name));
            }
        }
        self.close_dialog(cx);
        self.refresh(cx);
    }

    /// Create a .lnk shortcut in the current directory for every clipboard
    /// item (Files "Paste shortcut").
    pub fn paste_as_shortcut(&mut self, cx: &mut Context<Self>) {
        use crate::core::shortcut_service::{ShortcutOptions, ShortcutService};
        let dir = self
            .state
            .main_view
            .primary_path
            .as_path()
            .to_path_buf();
        let targets: Vec<std::path::PathBuf> = match &self.state.clipboard {
            crate::ui::components::clipboard_manager::ClipboardState::Cut(items)
            | crate::ui::components::clipboard_manager::ClipboardState::Copy(items) => {
                items.iter().map(|item| item.path.clone()).collect()
            }
            _ => Vec::new(),
        };
        if targets.is_empty() {
            self.close_dialog(cx);
            return;
        }
        for target in targets {
            let stem = target
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "Shortcut".to_string());
            let mut link_name = format!("{} - Shortcut.lnk", stem);
            let mut i = 2;
            while dir.join(&link_name).exists() {
                link_name = format!("{} - Shortcut ({}).lnk", stem, i);
                i += 1;
            }
            let target_fp = crate::core::path_util::FilesPath::from(target);
            let link_fp =
                crate::core::path_util::FilesPath::from(dir.join(&link_name));
            ShortcutService::create_shortcut(ShortcutOptions::new(target_fp, link_fp));
        }
        self.close_dialog(cx);
        self.refresh(cx);
    }

    pub fn paste(&mut self, cx: &mut Context<Self>) {
        let target_dir = self.state.active_view().primary_path.clone();
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
                use crate::core::progress::OpKind;
                use crate::core::transfer::{TransferOutcome, run_copy_job_rt, run_move_job_rt};
                let kind = if is_cut { OpKind::Move } else { OpKind::Copy };
                let job_id = progress_engine
                    .start_job_with_meta(
                        format!("{} ({} items)", op_type, total),
                        kind,
                        Some(crate::core::path_util::FilesPath::from(target_path.clone())),
                        total as u64,
                    )
                    .await;
                let sources: Vec<std::path::PathBuf> = clipboard_items
                    .iter()
                    .map(|item| item.path.as_path().to_path_buf())
                    .collect();
                let invalidation_sources = sources.clone();
                let invalidation_target = target_path.clone();
                let outcome = if is_cut {
                    run_move_job_rt(progress_engine.clone(), job_id, sources, target_path).await
                } else {
                    run_copy_job_rt(progress_engine.clone(), job_id, sources, target_path).await
                };
                crate::storage::folder_size::refresh_changed_branch_background(invalidation_target);
                if is_cut {
                    for src in invalidation_sources {
                        if let Some(parent) = src.parent() {
                            crate::storage::folder_size::refresh_changed_branch_background(
                                parent.to_path_buf(),
                            );
                        }
                    }
                }
                match outcome {
                    TransferOutcome::Done => {
                        progress_engine.complete_job(job_id).await;
                    }
                    TransferOutcome::Cancelled => {
                        this_weak
                            .update(&mut async_cx, |this, cx| {
                                this.state.status_center.update_job(
                                    job_id,
                                    crate::core::progress::OperationStatus::Cancelled,
                                );
                                cx.notify();
                            })
                            .ok();
                    }
                    TransferOutcome::Failed(reason) => {
                        progress_engine.fail_job(job_id, reason).await;
                    }
                }

                this_weak
                    .update(&mut async_cx, |this, cx| {
                        this.state.clipboard = ClipboardState::Empty;
                        this.state.main_view.cut_paths.clear();
                        this.state.secondary_view.cut_paths.clear();
                        this.refresh(cx);
                    })
                    .ok();
            })
            .detach();
    }

    // === Preview loading ===

    pub(crate) fn load_preview_content(&mut self, path: FilesPath, cx: &mut Context<Self>) {
        use crate::storage::preview_service;
        use crate::ui::views::main_view::{ImagePreviewInfo, PreviewContent, SvgPreviewInfo};

        // Reset scroll positions and active video playback when switching preview item
        self.state.main_view.side_pane_bouncy_scroll.set_immediate_offset(0.0);
        self.state
            .main_view
            .side_pane_scroll_handle
            .set_offset(gpui::point(px(0.0), px(0.0)));
        self.state.main_view.peek_bouncy_scroll.set_immediate_offset(0.0);
        self.state
            .main_view
            .peek_scroll_handle
            .set_offset(gpui::point(px(0.0), px(0.0)));
        self.state.main_view.video_playing = false;
        self.state.main_view.video_progress = 0.0;
        self.state.main_view.video_frame = None;
        self.state.video_player = None;
        self.state.main_view.office_preview.hide();

        // If `path` is a directory, always load rich folder contents preview!
        if path.as_path().is_dir() {
            self.load_folder_preview(path, cx);
            return;
        }

        self.state.main_view.preview_request_id =
            self.state.main_view.preview_request_id.wrapping_add(1);
        let req_id = self.state.main_view.preview_request_id;

        let file_name_lower = path
            .as_path()
            .file_name()
            .and_then(|n| n.to_str())
            .map(|s| s.to_ascii_lowercase())
            .unwrap_or_default();

        // Combine extension + magic-byte sniffing (`infer` + `mime_guess`) for widest compatibility
        let (mime_type, ext) = preview_service::detect_mime_and_extension(&path);
        self.state.main_view.details_cache.mime_type = Some(mime_type);

        // Check special extensionless filenames first (Dockerfile, Makefile, .gitignore, etc.)
        if ext.is_empty() {
            if matches!(
                file_name_lower.as_str(),
                "dockerfile" | "makefile" | "cmakelists.txt" | "justfile" | "vagrantfile" | "procfile" | "gemfile" | "rakefile"
            ) {
                self.state.main_view.preview_content = PreviewContent::Code {
                    language: file_name_lower,
                };
                self.load_text_preview(path, req_id, true, cx);
                return;
            }
        }

        match ext.as_str() {
            // Markdown & Markup Docs
            "md" | "markdown" | "mdx" | "rst" | "adoc" => {
                self.state.main_view.preview_content = PreviewContent::Markdown;
                self.load_text_preview(path, req_id, false, cx);
            }

            // HTML / Web Documents
            "html" | "htm" | "xhtml" => {
                self.state.main_view.preview_content = PreviewContent::Html;
                self.load_text_preview(path, req_id, false, cx);
            }

            // Rich Text Format
            "rtf" => {
                self.state.main_view.preview_content = PreviewContent::RichText;
                self.load_text_preview(path, req_id, false, cx);
            }

            // Tabular Data (CSV / TSV) -> Spreadsheet Table Preview
            "csv" | "tsv" => {
                let delimiter = if ext == "tsv" { '\t' } else { ',' };
                let kind_label = if ext == "tsv" { "TSV" } else { "CSV" }.to_string();
                self.state.main_view.preview_text = Some("Loading table preview...".to_string());
                self.state.main_view.preview_content = PreviewContent::CsvTable {
                    headers: Vec::new(),
                    rows: Vec::new(),
                    total_rows: 0,
                    delimiter,
                    kind: kind_label,
                };
                let mut async_cx = cx.to_async();
                let this_weak = cx.weak_entity();
                let path_clone = path.clone();
                let bg = cx.background_executor().spawn(async move {
                    preview_service::extract_csv_preview(&path_clone, delimiter)
                });
                cx.foreground_executor()
                    .spawn(async move {
                        let res = bg.await;
                        this_weak
                            .update(&mut async_cx, |this, cx| {
                                if this.state.main_view.preview_request_id != req_id {
                                    return;
                                }
                                if let Ok((headers, rows, total_rows)) = res {
                                    this.state.main_view.preview_text = Some(format!(
                                        "{} columns × {} rows",
                                        headers.len(),
                                        total_rows
                                    ));
                                    this.state.main_view.preview_content = PreviewContent::CsvTable {
                                        headers,
                                        rows,
                                        total_rows,
                                        delimiter,
                                        kind: if delimiter == '\t' {
                                            "TSV".to_string()
                                        } else {
                                            "CSV".to_string()
                                        },
                                    };
                                } else {
                                    this.state.main_view.preview_text =
                                        Some("Unable to parse CSV/TSV file.".to_string());
                                }
                                cx.notify();
                            })
                            .ok();
                    })
                    .detach();
            }

            // Vector Graphics (.svg / .svgz) -> resvg + tiny-skia rasterizer + XML source
            "svg" | "svgz" => {
                self.state.main_view.preview_text = Some("Rendering SVG vector preview...".to_string());
                self.state.main_view.preview_content = PreviewContent::Svg(SvgPreviewInfo::default());
                let mut async_cx = cx.to_async();
                let this_weak = cx.weak_entity();
                let path_clone = path.clone();
                let bg = cx.background_executor().spawn(async move {
                    preview_service::extract_svg_preview(&path_clone)
                });
                cx.foreground_executor()
                    .spawn(async move {
                        let res = bg.await;
                        this_weak
                            .update(&mut async_cx, |this, cx| {
                                if this.state.main_view.preview_request_id != req_id {
                                    return;
                                }
                                if let Ok(info) = res {
                                    this.state.main_view.preview_text = Some(info.xml_snippet.clone());
                                    this.state.main_view.preview_content = PreviewContent::Svg(info);
                                } else {
                                    this.state.main_view.preview_text =
                                        Some("Unable to render SVG vector file.".to_string());
                                }
                                cx.notify();
                            })
                            .ok();
                    })
                    .detach();
            }

            // Wide Programming Languages & Configs -> Syntax-Highlighted Code Preview
            "rs" | "py" | "pyw" | "pyi" | "js" | "mjs" | "cjs" | "jsx" | "ts" | "mts" | "cts"
            | "tsx" | "c" | "h" | "cpp" | "cc" | "cxx" | "hpp" | "hxx" | "cs" | "go" | "java"
            | "kt" | "kts" | "swift" | "rb" | "php" | "scala" | "r" | "lua" | "pl" | "pm"
            | "sh" | "bash" | "zsh" | "fish" | "ps1" | "psm1" | "bat" | "cmd" | "sql"
            | "graphql" | "gql" | "proto" | "vue" | "svelte" | "astro" | "dart" | "zig" | "nim"
            | "ex" | "exs" | "erl" | "hs" | "ml" | "mli" | "clj" | "lisp" | "scm" | "vim"
            | "cmake" | "gradle" | "glsl" | "hlsl" | "wgsl" | "css" | "scss" | "sass" | "less"
            | "json" | "jsonc" | "json5" | "jsonl" | "ndjson" | "xml" | "plist" | "xaml"
            | "yaml" | "yml" | "toml" | "ini" | "cfg" | "conf" | "config" | "env"
            | "properties" | "gitignore" | "gitattributes" | "editorconfig" | "npmrc" | "lock" => {
                let lang = match ext.as_str() {
                    "rs" => "rust",
                    "py" | "pyw" | "pyi" => "python",
                    "js" | "mjs" | "cjs" | "jsx" => "javascript",
                    "ts" | "mts" | "cts" | "tsx" => "typescript",
                    "c" | "h" => "c",
                    "cpp" | "cc" | "cxx" | "hpp" | "hxx" => "c++",
                    "cs" => "c#",
                    "go" => "go",
                    "java" => "java",
                    "kt" | "kts" => "kotlin",
                    "swift" => "swift",
                    "rb" => "ruby",
                    "php" => "php",
                    "sh" | "bash" | "zsh" | "fish" => "shell",
                    "ps1" | "psm1" => "powershell",
                    "json" | "jsonc" | "json5" | "jsonl" | "ndjson" => "json",
                    "xml" | "plist" | "xaml" => "xml",
                    "yaml" | "yml" => "yaml",
                    "toml" | "ini" | "cfg" | "conf" | "config" | "lock" => "toml",
                    other => other,
                };
                self.state.main_view.preview_content = PreviewContent::Code {
                    language: lang.to_string(),
                };
                self.load_text_preview(path, req_id, true, cx);
            }

            // Plain text & log files
            "txt" | "log" | "out" | "err" | "nfo" | "sub" | "srt" | "vtt" | "lic" | "license" => {
                self.state.main_view.preview_content = PreviewContent::Text(String::new());
                self.load_text_preview(path, req_id, false, cx);
            }

            // Excel workbooks: real cell grid (up to 1000 rows), scrollable both ways
            "xlsx" => {
                self.state.main_view.preview_text = Some("Loading spreadsheet...".to_string());
                self.state.main_view.preview_content = PreviewContent::CsvTable {
                    headers: Vec::new(),
                    rows: Vec::new(),
                    total_rows: 0,
                    delimiter: ',',
                    kind: "XLSX".to_string(),
                };
                let mut async_cx = cx.to_async();
                let this_weak = cx.weak_entity();
                let path_clone = path.clone();
                let bg = cx.background_executor().spawn(async move {
                    preview_service::extract_xlsx_table_preview(&path_clone)
                });
                cx.foreground_executor()
                    .spawn(async move {
                        let res = bg.await;
                        this_weak
                            .update(&mut async_cx, |this, cx| {
                                if this.state.main_view.preview_request_id != req_id {
                                    return;
                                }
                                if let Ok((label, _sheets, headers, rows, total)) = res {
                                    this.state.main_view.preview_text = Some(format!(
                                        "{} columns × {} rows",
                                        headers.len(),
                                        total
                                    ));
                                    this.state.main_view.preview_content = PreviewContent::CsvTable {
                                        headers,
                                        rows,
                                        total_rows: total,
                                        delimiter: ',',
                                        kind: label,
                                    };
                                } else {
                                    this.state.main_view.preview_text =
                                        Some("Unable to read worksheet data.".to_string());
                                }
                                cx.notify();
                            })
                            .ok();
                    })
                    .detach();
            }

            // PowerPoint: real slide images (shapes, images, theme, fonts).
            "pptx" => {
                self.state.main_view.preview_text = Some("Rendering slides...".to_string());
                self.state.main_view.preview_content = PreviewContent::Office {
                    pages: Vec::new(),
                    label: "Slide".to_string(),
                };
                let mut async_cx = cx.to_async();
                let this_weak = cx.weak_entity();
                let path_clone = path.clone();
                let bg = cx.background_executor().spawn(async move {
                    let cache_dir = crate::app_state::icons_cache_dir();
                    let key = crate::app_state::disk_cache_key(&path_clone);
                    preview_service::render_pptx_pages(&path_clone, &cache_dir, &key, 12)
                });
                cx.foreground_executor()
                    .spawn(async move {
                        let res = bg.await;
                        this_weak
                            .update(&mut async_cx, |this, cx| {
                                if this.state.main_view.preview_request_id != req_id {
                                    return;
                                }
                                match res {
                                    Ok(pages) if !pages.is_empty() => {
                                        this.state.main_view.preview_text = None;
                                        this.state.main_view.preview_content =
                                            PreviewContent::Office {
                                                pages,
                                                label: "Slide".to_string(),
                                            };
                                    }
                                    _ => {
                                        this.state.main_view.preview_text = Some(
                                            "Unable to render this presentation.".to_string(),
                                        );
                                        this.state.main_view.preview_content =
                                            PreviewContent::None;
                                    }
                                }
                                cx.notify();
                            })
                            .ok();
                    })
                    .detach();
            }

            // Word / ODF / EPUB: structured Markdown (headings, bold/italic,
            // lists, tables). DOCX goes through the BetterOffice facade first.
            "docx" | "odt" | "epub" => {
                self.state.main_view.preview_text = Some("Loading document preview...".to_string());
                self.state.main_view.preview_content = PreviewContent::Markdown;
                let mut async_cx = cx.to_async();
                let this_weak = cx.weak_entity();
                let path_clone = path.clone();
                let ext_clone = ext.clone();
                let bg = cx.background_executor().spawn(async move {
                    preview_service::extract_office_markdown(&path_clone, &ext_clone)
                });
                cx.foreground_executor()
                    .spawn(async move {
                        let res = bg.await;
                        this_weak
                            .update(&mut async_cx, |this, cx| {
                                if this.state.main_view.preview_request_id != req_id {
                                    return;
                                }
                                match res {
                                    Ok(md) => {
                                        this.state.main_view.preview_text = Some(md);
                                        this.state.main_view.preview_content =
                                            PreviewContent::Markdown;
                                    }
                                    Err(_) => {
                                        this.state.main_view.preview_text = Some(
                                            "Unable to extract document text.".to_string(),
                                        );
                                        this.state.main_view.preview_content =
                                            PreviewContent::Text(String::new());
                                    }
                                }
                                cx.notify();
                            })
                            .ok();
                    })
                    .detach();
            }

            // Office containers without a dedicated handler (.xlsm, .xls, .ods, .odp):
            // visual sheet grid / paginated document preview with rendered page canvas.
            "xlsm" | "xls" | "ods" | "odp" => {
                self.state.main_view.preview_text = None;
                self.state.main_view.preview_content = PreviewContent::OfficeVisual(
                    crate::ui::views::main_view::OfficePreviewInfo::default(),
                );
                let mut async_cx = cx.to_async();
                let this_weak = cx.weak_entity();
                let path_clone = path.clone();
                let ext_clone = ext.clone();
                let bg = cx.background_executor().spawn(async move {
                    preview_service::extract_office_visual_preview(&path_clone, &ext_clone, 0)
                });
                cx.foreground_executor()
                    .spawn(async move {
                        let res = bg.await;
                        this_weak
                            .update(&mut async_cx, |this, cx| {
                                if this.state.main_view.preview_request_id != req_id {
                                    return;
                                }
                                if let Ok(info) = res {
                                    this.state.main_view.preview_content =
                                        PreviewContent::OfficeVisual(info);
                                } else {
                                    this.state.main_view.preview_text =
                                        Some("Unable to render Office document preview.".to_string());
                                }
                                cx.notify();
                            })
                            .ok();
                    })
                    .detach();
            }

            // SQLite Databases (.sqlite, .sqlite3, .db)
            "sqlite" | "sqlite3" | "db" => {
                self.state.main_view.preview_text = Some("Inspecting SQLite schema...".to_string());
                self.state.main_view.preview_content = PreviewContent::Database {
                    tables: Vec::new(),
                    page_size: 4096,
                };
                let mut async_cx = cx.to_async();
                let this_weak = cx.weak_entity();
                let path_clone = path.clone();
                let bg = cx.background_executor().spawn(async move {
                    preview_service::extract_sqlite_preview(&path_clone)
                });
                cx.foreground_executor()
                    .spawn(async move {
                        let res = bg.await;
                        this_weak
                            .update(&mut async_cx, |this, cx| {
                                if this.state.main_view.preview_request_id != req_id {
                                    return;
                                }
                                if let Ok((tables, page_size)) = res {
                                    this.state.main_view.preview_text =
                                        Some(format!("{} tables", tables.len()));
                                    this.state.main_view.preview_content =
                                        PreviewContent::Database { tables, page_size };
                                } else if let Ok((desc, arch, hex_lines)) =
                                    preview_service::extract_binary_hex_preview(&path)
                                {
                                    this.state.main_view.preview_content =
                                        PreviewContent::BinaryHex {
                                            format_description: desc,
                                            arch,
                                            hex_lines,
                                        };
                                }
                                cx.notify();
                            })
                            .ok();
                    })
                    .detach();
            }

            // Typography Fonts (.ttf, .otf, .woff, .woff2)
            "ttf" | "otf" | "woff" | "woff2" => {
                let family_name = path
                    .as_path()
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| "Font".to_string());
                let file_size = std::fs::metadata(path.as_path())
                    .map(|m| m.len())
                    .unwrap_or(0);
                self.state.main_view.preview_content = PreviewContent::Font {
                    family_name,
                    format: ext.to_uppercase(),
                    file_size,
                };
                self.state.main_view.preview_text = None;
                cx.notify();
            }

            // PDF files - render visual PDF page image in background (no raw text dump)
            "pdf" => {
                self.state.main_view.preview_text = None;
                let mut init_pdf = crate::ui::views::main_view::PdfPreviewInfo::default();
                init_pdf.current_page = 1;
                init_pdf.page_count = 1;
                init_pdf.rendered_page_path = self.state.main_view.icon_paths.get(&path).cloned();
                self.state.main_view.preview_content = PreviewContent::Pdf(init_pdf);

                let mut async_cx = cx.to_async();
                let this_weak = cx.weak_entity();
                let path_clone = path.clone();
                let bg = cx.background_executor().spawn(async move {
                    let mut info = preview_service::extract_pdf_metadata(&path_clone)?;
                    // Rasterize the first pages so the pane shows real page
                    // images instead of raw extracted text.
                    #[cfg(windows)]
                    {
                        let cache_dir = crate::app_state::icons_cache_dir();
                        let key = crate::app_state::disk_cache_key(&path_clone);
                        if let Ok(pages) =
                            preview_service::render_pdf_pages(&path_clone, &cache_dir, &key, 10, 640)
                        {
                            info.pages = pages;
                        }
                    }
                    Ok::<_, anyhow::Error>(info)
                });
                cx.foreground_executor()
                    .spawn(async move {
                        let res = bg.await;
                        this_weak
                            .update(&mut async_cx, |this, cx| {
                                if this.state.main_view.preview_request_id != req_id {
                                    return;
                                }
                                match res {
                                    Ok(mut info) => {
                                        let mut content_lines = vec![
                                            format!("PDF Document (v{})", info.pdf_version.as_deref().unwrap_or("1.7")),
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
                                        if let Some(ref txt) = info.extracted_text {
                                            content_lines.push(format!("\n─── Extracted Page Text ───\n{}", txt));
                                        }
                                        this.state.main_view.preview_text =
                                            Some(content_lines.join("\n"));
                                        // Prefer the visual page render when available.
                                        if info.rendered_page_path.is_none() {
                                            if let Some(first) = info.pages.first().cloned() {
                                                info.rendered_page_path = Some(first);
                                            }
                                        }
                                        this.state.main_view.preview_content =
                                            PreviewContent::Pdf(info);
                                        cx.notify();
                                    }
                                    Err(e) => {
                                        this.state.main_view.preview_text = Some(format!(
                                            "PDF Preview\n\nFailed to read PDF metadata: {}",
                                            e
                                        ));
                                        cx.notify();
                                    }
                                }
                            })
                            .ok();
                    })
                    .detach();
            }

            // HEIC/HEIF photos (iPhone): OS WIC codec, shell thumbnail fallback
            "heic" | "heif" => {
                self.load_rasterized_image_preview(path, req_id, "heic", cx);
            }

            // Exotic/proprietary rasters: OS shell thumbnail -> PNG cache
            "avif" | "tga" | "dds" | "raw" | "cr2" | "nef" | "arw" | "psd" | "ai" | "eps" => {
                self.load_rasterized_image_preview(path, req_id, "shell", cx);
            }

            // Raster & Camera Image files -> Bounded <=1024px VRAM-safe preview + EXIF & header metadata
            "jpg" | "jpeg" | "png" | "gif" | "bmp" | "webp" | "ico" | "tiff" | "tif" | "tga"
            | "dds" | "pnm" | "pbm" | "pgm" | "ppm" | "hdr" | "avif" | "raw" | "cr2" | "nef"
            | "arw" | "psd" | "ai" | "eps" | "heic" | "heif" => {
                let mut init_img = ImagePreviewInfo::default();
                init_img.display_path = self
                    .state
                    .main_view
                    .icon_paths
                    .get(&path)
                    .cloned()
                    .unwrap_or_else(|| path.as_path().to_path_buf());
                init_img.format = ext.to_uppercase();
                self.state.main_view.preview_text = None;
                self.state.main_view.preview_content = PreviewContent::Image(init_img);

                let mut async_cx = cx.to_async();
                let this_weak = cx.weak_entity();
                let path_clone = path.clone();
                let ext_clone = ext.clone();
                let thumb_svc = self.state.thumbnail_service.clone();
                let bg = cx.background_executor().spawn(async move {
                    match preview_service::extract_image_preview(&path_clone, &ext_clone) {
                        Ok(mut info) => {
                            // If dimensions are still 0 (e.g. RAW/HEIC/PSD not decoded by `image` crate),
                            // try OS shell thumbnail fallback (`IShellItemImageFactory` on Windows)
                            if info.width == 0 && info.height == 0 {
                                if let Some(poster) = thumb_svc.extract_and_save_poster_png(
                                    &path_clone,
                                    crate::platform::windows::thumbnail_service::ThumbnailSize::ExtraLarge,
                                ) {
                                    info.display_path = poster;
                                }
                            }
                            Ok(info)
                        }
                        Err(e) => Err(e),
                    }
                });
                cx.foreground_executor()
                    .spawn(async move {
                        let res = bg.await;
                        this_weak
                            .update(&mut async_cx, |this, cx| {
                                if this.state.main_view.preview_request_id != req_id {
                                    return;
                                }
                                if let Ok(info) = res {
                                    this.state.main_view.preview_content =
                                        PreviewContent::Image(info);
                                    cx.notify();
                                }
                            })
                            .ok();
                    })
                    .detach();
            }

            // SVG vectors: resvg raster to PNG cache on background executor
            "svg" => {
                self.state.main_view.preview_text = Some("Rendering vector preview...".to_string());
                self.state.main_view.preview_content =
                    PreviewContent::Image(ImagePreviewInfo {
                        display_path: path.as_path().to_path_buf(),
                        ..Default::default()
                    });
                let mut async_cx = cx.to_async();
                let this_weak = cx.weak_entity();
                let path_clone = path.clone();
                let fallback_display = path.as_path().to_path_buf();
                let bg = cx.background_executor().spawn(async move {
                    let cache_dir = crate::app_state::icons_cache_dir();
                    let out_path = cache_dir.join(format!(
                        "{}_svg.png",
                        crate::app_state::disk_cache_key(&path_clone)
                    ));
                    if !(out_path.exists()
                        || preview_service::rasterize_svg_preview(&path_clone, &out_path, 1024)
                            .is_ok())
                    {
                        return None;
                    }
                    Some(out_path)
                });
                cx.foreground_executor()
                    .spawn(async move {
                        let png = bg.await;
                        this_weak
                            .update(&mut async_cx, |this, cx| {
                                if this.state.main_view.preview_request_id != req_id {
                                    return;
                                }
                                this.state.main_view.preview_text = None;
                                this.state.main_view.preview_content =
                                    PreviewContent::Image(ImagePreviewInfo {
                                        display_path: png.unwrap_or(fallback_display),
                                        ..Default::default()
                                    });
                                cx.notify();
                            })
                            .ok();
                    })
                    .detach();
            }

            // Audio files - extract metadata & embedded ID3 APIC cover art on background executor
            "mp3" | "wav" | "flac" | "ogg" | "m4a" | "aac" | "opus" | "wma" | "ape" | "alac"
            | "aiff" => {
                self.state.main_view.preview_text = Some("Loading audio metadata...".to_string());
                self.state.main_view.preview_content =
                    PreviewContent::Audio(crate::ui::views::main_view::MediaPreviewInfo::default());
                let mut async_cx = cx.to_async();
                let this_weak = cx.weak_entity();
                let path_clone = path.clone();
                let ext_clone = ext.clone();
                let bg = cx.background_executor().spawn(async move {
                    preview_service::extract_audio_metadata(&path_clone, &ext_clone)
                });
                cx.foreground_executor()
                    .spawn(async move {
                        let res = bg.await;
                        this_weak
                            .update(&mut async_cx, |this, cx| {
                                if this.state.main_view.preview_request_id != req_id {
                                    return;
                                }
                                if let Ok(info) = res {
                                    let mut content_lines = Vec::new();
                                    if let Some(ref title) = info.title {
                                        content_lines.push(format!("Title: {}", title));
                                    }
                                    if let Some(ref artist) = info.artist {
                                        content_lines.push(format!("Artist: {}", artist));
                                    }
                                    if let Some(ref album) = info.album {
                                        content_lines.push(format!("Album: {}", album));
                                    }
                                    this.state.main_view.preview_text = if content_lines.is_empty() {
                                        None
                                    } else {
                                        Some(content_lines.join("\n"))
                                    };
                                    this.state.main_view.preview_content =
                                        PreviewContent::Audio(info);
                                    cx.notify();
                                }
                            })
                            .ok();
                    })
                    .detach();
            }

            // Video files - native MF playback (+ metadata & poster on background executor)
            "mp4" | "mkv" | "avi" | "mov" | "wmv" | "webm" | "m4v" | "mpg" | "mpeg" | "3gp"
            | "flv" | "vob" | "ogv" | "ts" | "m2ts" => {
                self.state.main_view.preview_text = Some("Loading video stream...".to_string());
                let cached_poster = self.state.main_view.icon_paths.get(&path).cloned();
                let mut init_info = crate::ui::views::main_view::MediaPreviewInfo::default();
                init_info.poster_path = cached_poster.clone();
                self.state.main_view.preview_content = PreviewContent::Video(init_info);
                // Native playback session (dropped on next selection change).
                {
                    use crate::platform::windows::media_player::{MfPlayer, VideoBackend};
                    let muted = self.state.main_view.video_muted;
                    match VideoBackend::selected() {
                        VideoBackend::MediaFoundation => {
                            match MfPlayer::open(path.as_path().to_path_buf(), None) {
                                Ok(p) => {
                                    p.set_muted(muted);
                                    self.state.video_player = Some(p);
                                }
                                Err(e) => {
                                    tracing::warn!("MF player unavailable: {}", e);
                                    self.state.video_player = None;
                                }
                            }
                        }
                        VideoBackend::Ffmpeg => {
                            // Reserved hook: ffmpeg-sidecar pump plugs in here.
                            tracing::warn!(
                                "FFmpeg backend not bundled yet — timeline simulation active"
                            );
                            self.state.video_player = None;
                        }
                    }
                }

                let mut async_cx = cx.to_async();
                let this_weak = cx.weak_entity();
                let path_clone = path.clone();
                let ext_clone = ext.clone();
                let thumb_svc = self.state.thumbnail_service.clone();
                let bg_path = path_clone.clone();
                let bg = cx.background_executor().spawn(async move {
                    let mut info = preview_service::extract_video_metadata(&bg_path, &ext_clone)
                        .unwrap_or_default();
                    if info.poster_path.is_none() {
                        info.poster_path = cached_poster.or_else(|| {
                            thumb_svc.extract_and_save_poster_png(
                                &bg_path,
                                crate::platform::windows::thumbnail_service::ThumbnailSize::Large,
                            )
                        });
                    }
                    info
                });
                cx.foreground_executor()
                    .spawn(async move {
                        let info = bg.await;
                        this_weak
                            .update(&mut async_cx, |this, cx| {
                                if this.state.main_view.preview_request_id != req_id {
                                    return;
                                }
                                if let Some(ref poster) = info.poster_path {
                                    this.state
                                        .main_view
                                        .icon_paths
                                        .insert(path_clone.clone(), poster.clone());
                                }
                                this.state.main_view.preview_text = info.container_info.clone();
                                this.state.main_view.preview_content = PreviewContent::Video(info);
                                cx.notify();
                            })
                            .ok();
                    })
                    .detach();
            }

            // Archive files - list contents & compression ratio on background executor
            "zip" | "rar" | "7z" | "tar" | "gz" | "bz2" | "xz" | "tgz" | "tbz2" | "zst" | "jar"
            | "apk" | "whl" | "nupkg" | "vsix" => {
                self.load_archive_preview(path, req_id, cx);
            }

            // Known Binary / Executable / System files -> PE/ELF/WASM Header + Hex Inspector
            // Windows PE binaries: show the real icon/thumbnail, never hex.
            "exe" | "dll" | "sys" | "msi" | "cpl" | "scr" | "com" | "ocx" => {
                self.load_rasterized_image_preview(path, req_id, "shell", cx);
            }
            "so" | "dylib" | "wasm" | "bin" | "iso" | "img"
            | "class" | "pyc" | "pyd" | "o" | "obj" | "lib" | "a" | "pak" | "dat" => {
                self.load_binary_hex_preview(path, req_id, cx);
            }

            // Shortcut files (.lnk / .url)
            "lnk" | "url" => {
                self.load_text_preview(path, req_id, false, cx);
            }

            // Any other file extension: auto-detect bounded text vs binary Hex Inspector!
            _ => {
                self.load_text_preview(path, req_id, false, cx);
            }
        }
    }

    fn load_binary_hex_preview(&mut self, path: FilesPath, req_id: u64, cx: &mut Context<Self>) {
        use crate::storage::preview_service;
        use crate::ui::views::main_view::PreviewContent;

        self.state.main_view.preview_text = Some("Inspecting binary header...".to_string());
        let mut async_cx = cx.to_async();
        let this_weak = cx.weak_entity();
        let path_clone = path.clone();
        let bg = cx.background_executor().spawn(async move {
            preview_service::extract_binary_hex_preview(&path_clone)
        });
        cx.foreground_executor()
            .spawn(async move {
                let res = bg.await;
                this_weak
                    .update(&mut async_cx, |this, cx| {
                        if this.state.main_view.preview_request_id != req_id {
                            return;
                        }
                        if let Ok((format_description, arch, hex_lines)) = res {
                            this.state.main_view.preview_text = Some(format_description.clone());
                            this.state.main_view.preview_content = PreviewContent::BinaryHex {
                                format_description,
                                arch,
                                hex_lines,
                            };
                        }
                        cx.notify();
                    })
                    .ok();
            })
            .detach();
    }

    /// Rasterize HEIC/SVG/exotic images to a PNG cache file on the background
    /// executor, then show it via `PreviewContent::Image`.
    fn load_rasterized_image_preview(
        &mut self,
        path: FilesPath,
        req_id: u64,
        kind: &str,
        cx: &mut Context<Self>,
    ) {
        use crate::storage::preview_service;
        use crate::ui::views::main_view::{ImagePreviewInfo, PreviewContent};
        self.state.main_view.preview_text = Some("Rendering image preview...".to_string());
        self.state.main_view.preview_content =
            PreviewContent::Image(ImagePreviewInfo {
                display_path: path.as_path().to_path_buf(),
                ..Default::default()
            });
        let mut async_cx = cx.to_async();
        let this_weak = cx.weak_entity();
        let path_clone = path.clone();
        let kind_owned = kind.to_string();
        let thumb_svc = self.state.thumbnail_service.clone();
        let bg = cx.background_executor().spawn(async move {
            let cache_dir = crate::app_state::icons_cache_dir();
            let out_path = cache_dir.join(format!(
                "{}_{}.png",
                crate::app_state::disk_cache_key(&path_clone),
                kind_owned
            ));
            if out_path.exists() {
                return Some(out_path);
            }
            let rasterized = if kind_owned == "heic" {
                #[cfg(windows)]
                {
                    preview_service::decode_heic_wic_preview(&path_clone, &out_path).is_ok()
                }
                #[cfg(not(windows))]
                {
                    false
                }
            } else {
                false
            };
            if rasterized || out_path.exists() {
                return Some(out_path);
            }
            // Universal fallback: OS shell thumbnail -> PNG cache.
            if let Ok(res) = thumb_svc.extract_thumbnail(
                &path_clone,
                crate::platform::windows::thumbnail_service::ThumbnailSize::Large,
            ) {
                if !res.data.is_empty()
                    && preview_service::save_bgra_png(&res.data, res.width, res.height, &out_path)
                {
                    return Some(out_path);
                }
            }
            None
        });
        cx.foreground_executor()
            .spawn(async move {
                let png = bg.await;
                this_weak
                    .update(&mut async_cx, |this, cx| {
                        if this.state.main_view.preview_request_id != req_id {
                            return;
                        }
                        this.state.main_view.preview_text = None;
                        this.state.main_view.preview_content = match png {
                            Some(cached) => PreviewContent::Image(ImagePreviewInfo {
                                display_path: cached,
                                ..Default::default()
                            }),
                            // Total failure: generic file card instead of a blank canvas.
                            None => PreviewContent::None,
                        };
                        cx.notify();
                    })
                    .ok();
            })
            .detach();
    }

    fn load_text_preview(
        &mut self,
        path: FilesPath,
        req_id: u64,
        prehighlight_code: bool,
        cx: &mut Context<Self>,
    ) {
        use crate::storage::preview_service;
        use crate::ui::views::main_view::{MainView, PreviewContent};

        self.state.main_view.preview_text = Some("Loading preview...".to_string());

        let mut async_cx = cx.to_async();
        let this_weak = cx.weak_entity();
        let path_clone = path.clone();
        let bg = cx.background_executor().spawn(async move {
            let loaded = preview_service::read_bounded_text_or_detect_binary(&path_clone)?;
            match loaded {
                Ok(text) => {
                    let highlighted = if prehighlight_code {
                        // Cap syntax highlighting to first 300 lines & max 200 chars/line for instant 120 FPS rendering
                        let capped: String = text
                            .lines()
                            .take(300)
                            .map(|l| {
                                if l.len() > 200 {
                                    let cut = l.char_indices().nth(200).map(|(i, _)| i).unwrap_or(l.len());
                                    format!("{} …", &l[..cut])
                                } else {
                                    l.to_string()
                                }
                            })
                            .collect::<Vec<_>>()
                            .join("\n");
                        std::sync::Arc::new(MainView::highlight_code(&capped))
                    } else {
                        std::sync::Arc::new(Vec::new())
                    };
                    Ok::<_, anyhow::Error>(Ok((text, highlighted)))
                }
                Err(binary_sample) => {
                    let (desc, arch) = preview_service::extract_binary_hex_preview(&path_clone)
                        .map(|(d, a, _)| (d, a))
                        .unwrap_or_else(|_| ("Binary File".to_string(), None));
                    let hex_lines = preview_service::format_hex_lines(&binary_sample);
                    Ok(Err((desc, arch, hex_lines)))
                }
            }
        });

        cx.foreground_executor()
            .spawn(async move {
                let result = bg.await;
                this_weak
                    .update(&mut async_cx, |this, cx| {
                        if this.state.main_view.preview_request_id != req_id {
                            return;
                        }
                        match result {
                            Ok(Ok((text, highlighted))) => {
                                this.state.main_view.preview_highlighted_lines = highlighted;
                                this.state.main_view.preview_text = Some(text);
                            }
                            Ok(Err((format_description, arch, hex_lines))) => {
                                this.state.main_view.preview_text =
                                    Some(format_description.clone());
                                this.state.main_view.preview_content = PreviewContent::BinaryHex {
                                    format_description,
                                    arch,
                                    hex_lines,
                                };
                            }
                            Err(_) => {
                                this.state.main_view.preview_text =
                                    Some("Unable to load file preview.".to_string());
                            }
                        }
                        cx.notify();
                    })
                    .ok();
            })
            .detach();
    }

    fn load_archive_preview(&mut self, path: FilesPath, req_id: u64, cx: &mut Context<Self>) {
        use crate::storage::archive_service::ArchiveService;
        use crate::storage::preview_service;
        use crate::ui::views::main_view::{ArchivePreviewInfo, PreviewContent};

        self.state.main_view.preview_content =
            PreviewContent::Archive(ArchivePreviewInfo::default());
        self.state.main_view.preview_text = Some("Loading archive contents...".to_string());

        let ext = path
            .as_path()
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_ascii_lowercase())
            .unwrap_or_default();

        let mut async_cx = cx.to_async();
        let this_weak = cx.weak_entity();
        let path_clone = path.clone();
        let bg = cx.background_executor().spawn(async move {
            if let Ok(info) = preview_service::extract_archive_preview(&path_clone, &ext) {
                return Ok(info);
            }
            let entries = ArchiveService::list_contents(&path_clone)?;
            let entry_count = entries.len();
            let display_entries: Vec<String> = entries.into_iter().take(120).collect();
            Ok::<ArchivePreviewInfo, anyhow::Error>(ArchivePreviewInfo {
                entry_count,
                entries: display_entries,
                uncompressed_size: None,
                compression_ratio: None,
            })
        });
        cx.foreground_executor()
            .spawn(async move {
                match bg.await {
                    Ok(info) => {
                        this_weak
                            .update(&mut async_cx, |this, cx| {
                                if this.state.main_view.preview_request_id != req_id {
                                    return;
                                }
                                let content = if info.entry_count > info.entries.len() {
                                    format!(
                                        "Archive Contents ({})\n\n{}\n\n... and {} more entries",
                                        info.entry_count,
                                        info.entries.join("\n"),
                                        info.entry_count - info.entries.len()
                                    )
                                } else {
                                    format!(
                                        "Archive Contents ({})\n\n{}",
                                        info.entry_count,
                                        info.entries.join("\n")
                                    )
                                };
                                this.state.main_view.preview_text = Some(content);
                                this.state.main_view.preview_content =
                                    PreviewContent::Archive(info);
                                cx.notify();
                            })
                            .ok();
                    }
                    Err(e) => {
                        this_weak
                            .update(&mut async_cx, |this, cx| {
                                if this.state.main_view.preview_request_id != req_id {
                                    return;
                                }
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
    pub fn check_third_party_preview_handler(
        &self,
    ) -> crate::ui::views::main_view::ThirdPartyHandler {
        use crate::ui::views::main_view::ThirdPartyHandler;
        use std::process::Command;
        use std::sync::OnceLock;

        static CACHED_HANDLER: OnceLock<ThirdPartyHandler> = OnceLock::new();
        CACHED_HANDLER
            .get_or_init(|| {
                if Command::new("peek.exe").arg("--version").output().is_ok() {
                    return ThirdPartyHandler::Peek;
                }
                if Command::new("quicklook.exe").arg("-v").output().is_ok() {
                    return ThirdPartyHandler::QuickLook;
                }
                if Command::new("seer.exe").arg("--version").output().is_ok() {
                    return ThirdPartyHandler::SeerPro;
                }
                ThirdPartyHandler::None
            })
            .clone()
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
                                                    let truncate_at = text
                                                        .char_indices()
                                                        .nth(10000)
                                                        .map(|(i, _)| i)
                                                        .unwrap_or(text.len());
                                                    format!("{}\n\n[Preview truncated at 10,000 characters]", &text[..truncate_at])
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

    // === Toggle operations ===

    pub fn toggle_preview_pane(&mut self, cx: &mut Context<Self>) {
        use crate::ui::views::main_view::SidePaneMode;
        self.state.main_view.side_pane = match self.state.main_view.side_pane {
            SidePaneMode::Preview => SidePaneMode::None,
            _ => SidePaneMode::Preview,
        };
        if !matches!(self.state.main_view.side_pane, SidePaneMode::None) {
            self.load_preview_for_selected_item(cx);
        }
        cx.notify();
    }

    /// Toggle the Info/Details pane.
    pub fn toggle_info_pane(&mut self, cx: &mut Context<Self>) {
        use crate::ui::views::main_view::SidePaneMode;
        self.state.main_view.side_pane = match self.state.main_view.side_pane {
            SidePaneMode::Info => SidePaneMode::None,
            _ => SidePaneMode::Info,
        };
        if !matches!(self.state.main_view.side_pane, SidePaneMode::None) {
            self.load_preview_for_selected_item(cx);
        }
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
    pub fn toggle_shelf_pane(&mut self, cx: &mut Context<Self>) {
        self.state.main_view.shelf_visible = !self.state.main_view.shelf_visible;
        cx.notify();
    }
    pub fn toggle_details_pane(&mut self, cx: &mut Context<Self>) {
        use crate::ui::views::main_view::SidePaneMode;
        self.state.main_view.side_pane = match self.state.main_view.side_pane {
            SidePaneMode::Details => SidePaneMode::None,
            _ => SidePaneMode::Details,
        };
        if !matches!(self.state.main_view.side_pane, SidePaneMode::None) {
            self.load_preview_for_selected_item(cx);
        }
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
    pub fn toggle_fullscreen(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.state.is_fullscreen = !self.state.is_fullscreen;
        // TODO: Fix fullscreen toggle for GPUI 0.2.x
        // window.set_fullscreen(self.state.is_fullscreen);
        cx.notify();
    }
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

    /// Switch the right-click menu presentation (Settings › General).
    pub fn set_context_menu_style(
        &mut self,
        style: crate::config::config::ContextMenuStyle,
        cx: &mut Context<Self>,
    ) {
        let settings = self.state.settings.clone();
        settings.update(cx, |s, _cx| {
            s.config.preferences.context_menu_style = style;
            let _ = s.config.save();
        });
    }

    pub fn toggle_setting(&mut self, key: &str, cx: &mut Context<Self>) {
        // Handle toolbar button visibility toggles (stored in main_view, not config)
        if key.starts_with("toolbar_") {
            let button_id = key.strip_prefix("toolbar_").unwrap_or(key);
            self.state.main_view.toolbar_customization.toggle(button_id);
            cx.notify();
            return;
        }

        let mut needs_refresh = false;
        let mut needs_resort = false;
        let settings = self.state.settings.clone();
        settings.update(cx, |s, _cx| {
            match key {
                "open_tab_in_existing_instance" => {
                    s.config.session.open_tab_in_existing_instance =
                        !s.config.session.open_tab_in_existing_instance
                }
                "always_switch_to_new_tab" => {
                    s.config.session.always_switch_to_new_tab =
                        !s.config.session.always_switch_to_new_tab
                }
                "mru_tab_cycle" => s.config.session.mru_tab_cycle = !s.config.session.mru_tab_cycle,
                "widget_quick_access" => {
                    s.config.home_widgets.show_quick_access =
                        !s.config.home_widgets.show_quick_access
                }
                "widget_drives" => {
                    s.config.home_widgets.show_drives = !s.config.home_widgets.show_drives
                }
                "widget_network" => {
                    s.config.home_widgets.show_network_locations =
                        !s.config.home_widgets.show_network_locations
                }
                "widget_file_tags" => {
                    s.config.home_widgets.show_file_tags = !s.config.home_widgets.show_file_tags
                }
                "widget_recent_files" => {
                    s.config.home_widgets.show_recent_files =
                        !s.config.home_widgets.show_recent_files
                }
                "ctx_show_open_in_new_tab" => {
                    s.config.preferences.ctx_show_open_in_new_tab =
                        !s.config.preferences.ctx_show_open_in_new_tab
                }
                "ctx_show_open_in_new_window" => {
                    s.config.preferences.ctx_show_open_in_new_window =
                        !s.config.preferences.ctx_show_open_in_new_window
                }
                "ctx_show_open_in_new_pane" => {
                    s.config.preferences.ctx_show_open_in_new_pane =
                        !s.config.preferences.ctx_show_open_in_new_pane
                }
                "ctx_show_copy_path" => {
                    s.config.preferences.ctx_show_copy_path =
                        !s.config.preferences.ctx_show_copy_path
                }
                "ctx_show_create_folder_with_selection" => {
                    s.config.preferences.ctx_show_create_folder_with_selection =
                        !s.config.preferences.ctx_show_create_folder_with_selection
                }
                "ctx_show_create_shortcut" => {
                    s.config.preferences.ctx_show_create_shortcut =
                        !s.config.preferences.ctx_show_create_shortcut
                }
                "ctx_show_pin_to_sidebar" => {
                    s.config.preferences.ctx_show_pin_to_sidebar =
                        !s.config.preferences.ctx_show_pin_to_sidebar
                }
                "ctx_show_compression_options" => {
                    s.config.preferences.ctx_show_compression_options =
                        !s.config.preferences.ctx_show_compression_options
                }
                "ctx_show_send_to" => {
                    s.config.preferences.ctx_show_send_to = !s.config.preferences.ctx_show_send_to
                }
                "ctx_show_open_terminal" => {
                    s.config.preferences.ctx_show_open_terminal =
                        !s.config.preferences.ctx_show_open_terminal
                }
                "ctx_show_edit_tags" => {
                    s.config.preferences.ctx_show_edit_tags =
                        !s.config.preferences.ctx_show_edit_tags
                }
                "move_shell_extensions_to_submenu" => {
                    s.config.preferences.move_shell_extensions_to_submenu =
                        !s.config.preferences.move_shell_extensions_to_submenu
                }
                "enable_smooth_scrolling" => {
                    s.config.preferences.enable_smooth_scrolling =
                        !s.config.preferences.enable_smooth_scrolling
                }
                "reverse_tab_scroll_direction" => {
                    s.config.preferences.reverse_tab_scroll_direction =
                        !s.config.preferences.reverse_tab_scroll_direction
                }
                "show_file_extensions" => {
                    s.config.preferences.show_file_extensions =
                        !s.config.preferences.show_file_extensions
                }
                "show_hidden_files" => {
                    s.config.preferences.show_hidden_files = !s.config.preferences.show_hidden_files;
                    needs_refresh = true;
                }
                "show_dot_files" => {
                    s.config.preferences.show_dot_files = !s.config.preferences.show_dot_files;
                    needs_refresh = true;
                }
                "show_system_files" => {
                    s.config.preferences.show_system_files = !s.config.preferences.show_system_files;
                    needs_refresh = true;
                }
                "show_alternate_data_streams" => {
                    s.config.preferences.show_alternate_data_streams =
                        !s.config.preferences.show_alternate_data_streams
                }
                "show_thumbnails" => {
                    s.config.preferences.show_thumbnails = !s.config.preferences.show_thumbnails
                }
                "show_checkboxes_when_selecting" => {
                    s.config.preferences.show_checkboxes_when_selecting =
                        !s.config.preferences.show_checkboxes_when_selecting
                }
                "show_button_tooltips" => {
                    s.config.preferences.show_button_tooltips =
                        !s.config.preferences.show_button_tooltips
                }
                "show_file_extension_warning" => {
                    s.config.preferences.show_file_extension_warning =
                        !s.config.preferences.show_file_extension_warning
                }
                "confirm_delete" => {
                    s.config.preferences.confirm_delete = !s.config.preferences.confirm_delete
                }
                "mica_enabled" => {
                    s.config.appearance.mica_enabled = !s.config.appearance.mica_enabled
                }
                "mica_alt" => s.config.appearance.mica_alt = !s.config.appearance.mica_alt,
                "transparent_window" => {
                    s.config.appearance.transparent_window = !s.config.appearance.transparent_window
                }
                "show_shelf_button" => {
                    s.config.appearance.show_shelf_button = !s.config.appearance.show_shelf_button
                }
                "show_toolbar_labels" => {
                    s.config.appearance.show_toolbar_labels =
                        !s.config.appearance.show_toolbar_labels
                }
                "show_tab_actions" => {
                    s.config.appearance.show_tab_actions = !s.config.appearance.show_tab_actions
                }
                "show_toolbar" => {
                    s.config.appearance.show_toolbar = !s.config.appearance.show_toolbar
                }
                "show_status_bar" => {
                    s.config.appearance.show_status_bar = !s.config.appearance.show_status_bar
                }
                "always_open_dual_pane_in_new_tab" => {
                    s.config.view.always_open_dual_pane_in_new_tab =
                        !s.config.view.always_open_dual_pane_in_new_tab
                }
                "adaptive_layout_enabled" => {
                    s.config.view.adaptive_layout_enabled = !s.config.view.adaptive_layout_enabled
                }
                "sync_folder_preferences" => {
                    s.config.view.sync_folder_preferences = !s.config.view.sync_folder_preferences
                }
                "folder_on_top" => {
                    s.config.view.folder_on_top = !s.config.view.folder_on_top;
                    needs_resort = true;
                }
                "sort_descending" => {
                    s.config.view.sort_ascending = !s.config.view.sort_ascending;
                    needs_resort = true;
                }
                "group_descending" => {
                    s.config.view.group_ascending = !s.config.view.group_ascending;
                    needs_resort = true;
                }
                "show_date_column" => {
                    s.config.view.show_date_column = !s.config.view.show_date_column
                }
                "show_date_created_column" => {
                    s.config.view.show_date_created_column =
                        !s.config.view.show_date_created_column
                }
                "show_type_column" => {
                    s.config.view.show_type_column = !s.config.view.show_type_column
                }
                "show_size_column" => {
                    s.config.view.show_size_column = !s.config.view.show_size_column
                }
                "show_tag_column" => {
                    s.config.view.show_tag_column = !s.config.view.show_tag_column
                }
                "auto_size_columns" => {
                    s.config.view.auto_size_columns = !s.config.view.auto_size_columns
                }
                "calculate_folder_sizes" => {
                    s.config.view.calculate_folder_sizes = !s.config.view.calculate_folder_sizes
                }
                "index_all_drives_folder_sizes" => {
                    s.config.view.index_all_drives_folder_sizes =
                        !s.config.view.index_all_drives_folder_sizes
                }
                "open_files_with_single_click" => {
                    s.config.actions.open_files_with_single_click =
                        !s.config.actions.open_files_with_single_click
                }
                "open_folders_with_single_click" => {
                    s.config.actions.open_folders_with_single_click =
                        !s.config.actions.open_folders_with_single_click
                }
                "open_folders_in_columns_with_single_click" => {
                    s.config.actions.open_folders_in_columns_with_single_click =
                        !s.config.actions.open_folders_in_columns_with_single_click
                }
                "open_folders_in_new_tab" => {
                    s.config.actions.open_folders_in_new_tab =
                        !s.config.actions.open_folders_in_new_tab
                }
                "select_files_on_hover" => {
                    s.config.actions.select_files_on_hover =
                        !s.config.actions.select_files_on_hover
                }
                "double_click_empty_space_go_up" => {
                    s.config.actions.double_click_empty_space_go_up =
                        !s.config.actions.double_click_empty_space_go_up
                }
                "scroll_to_previous_folder_when_navigating_up" => {
                    s.config.actions.scroll_to_previous_folder_when_navigating_up =
                        !s.config.actions.scroll_to_previous_folder_when_navigating_up
                }
                "show_fps_telemetry" => {
                    s.config.devtools.show_fps_telemetry = !s.config.devtools.show_fps_telemetry
                }
                "open_on_windows_startup" => {
                    s.config.experimental.open_on_windows_startup =
                        !s.config.experimental.open_on_windows_startup
                }
                "leave_app_running_in_background" => {
                    s.config.experimental.leave_app_running_in_background =
                        !s.config.experimental.leave_app_running_in_background
                }
                "show_system_tray_icon" => {
                    s.config.experimental.show_system_tray_icon =
                        !s.config.experimental.show_system_tray_icon
                }
                "set_as_default_file_manager" => {
                    s.config.experimental.set_as_default_file_manager =
                        !s.config.experimental.set_as_default_file_manager
                }
                "replace_open_file_dialog" => {
                    s.config.experimental.replace_open_file_dialog =
                        !s.config.experimental.replace_open_file_dialog
                }
                "enable_thumbnail_cache" => {
                    s.config.experimental.enable_thumbnail_cache =
                        !s.config.experimental.enable_thumbnail_cache
                }
                "show_flatten_options" => {
                    s.config.experimental.show_flatten_options =
                        !s.config.experimental.show_flatten_options
                }
                _ => {}
            }
            let _ = s.config.save();
        });

        // Sync column visibility flags with MainViewState
        let (show_hidden, show_dot, show_sys, folder_on_top, sort_asc, show_date, show_type, show_size) = {
            let cfg = &self.state.settings.read(cx).config;
            (
                cfg.preferences.show_hidden_files,
                cfg.preferences.show_dot_files,
                cfg.preferences.show_system_files,
                cfg.view.folder_on_top,
                cfg.view.sort_ascending,
                cfg.view.show_date_column,
                cfg.view.show_type_column,
                cfg.view.show_size_column,
            )
        };
        self.state.main_view.show_hidden_files = show_hidden;
        self.state.main_view.show_dot_files = show_dot;
        self.state.main_view.show_system_files = show_sys;
        self.state.main_view.sort.folders_first = folder_on_top;
        self.state.main_view.sort.ascending = sort_asc;
        self.state.main_view.column_visibility.date = show_date;
        self.state.main_view.column_visibility.type_col = show_type;
        self.state.main_view.column_visibility.size = show_size;
        self.state.secondary_view.show_hidden_files = show_hidden;
        self.state.secondary_view.show_dot_files = show_dot;
        self.state.secondary_view.show_system_files = show_sys;
        self.state.secondary_view.sort.folders_first = folder_on_top;
        self.state.secondary_view.sort.ascending = sort_asc;

        let show_chk = self.state.settings.read(cx).config.preferences.show_checkboxes_when_selecting;
        let sel_mode = if show_chk {
            crate::ui::views::main_view::SelectionMode::Checkbox
        } else {
            crate::ui::views::main_view::SelectionMode::Normal
        };
        self.state.main_view.selection_mode = sel_mode.clone();
        self.state.secondary_view.selection_mode = sel_mode;

        if key == "index_all_drives_folder_sizes"
            && self.state.settings.read(cx).config.view.index_all_drives_folder_sizes
        {
            self.trigger_index_all_drives(false, cx);
        }

        if needs_refresh {
            self.refresh(cx);
        } else if needs_resort {
            self.state.main_view.apply_sort();
            self.state.main_view.bump_items_revision();
        }
        cx.notify();
    }

    /// Trigger a background pass that indexes folder sizes across all local fixed drives once
    /// and stores them in the persistent mtime-hashed cache.
    pub fn trigger_index_all_drives(&mut self, force_full: bool, cx: &mut Context<Self>) {
        use crate::platform::windows::drive_service::DriveType;
        let mut roots: Vec<std::path::PathBuf> = self
            .state
            .main_view
            .cached_drives
            .iter()
            .filter(|d| d.is_ready && matches!(d.drive_type, DriveType::Fixed))
            .map(|d| d.root.as_path().to_path_buf())
            .collect();
        if roots.is_empty() {
            for d in crate::platform::windows::drive_service::enumerate_drives() {
                if d.is_ready && matches!(d.drive_type, DriveType::Fixed) {
                    roots.push(d.root.as_path().to_path_buf());
                }
            }
        }
        crate::storage::folder_size::index_all_drives_background(roots, force_full);
        cx.notify();
    }

    /// Clear the persistent & in-memory folder size cache.
    pub fn clear_folder_size_cache(&mut self, cx: &mut Context<Self>) {
        crate::storage::folder_size::clear_cache();
        self.state.status_center.push_notification(
            "Folder Size Cache Cleared".to_string(),
            "Cleared persistent folder size index.".to_string(),
            crate::ui::components::status_center::ToastSeverity::Info,
            None,
        );
        cx.notify();
    }
    pub(crate) fn load_folder_preview(&mut self, path: FilesPath, cx: &mut Context<Self>) {
        use crate::ui::views::main_view::{FolderPreviewItem, PreviewContent};

        self.state.main_view.preview_request_id =
            self.state.main_view.preview_request_id.wrapping_add(1);
        let req_id = self.state.main_view.preview_request_id;
        self.state.main_view.side_pane_bouncy_scroll.set_immediate_offset(0.0);
        self.state
            .main_view
            .side_pane_scroll_handle
            .set_offset(gpui::point(px(0.0), px(0.0)));
        self.state.main_view.peek_bouncy_scroll.set_immediate_offset(0.0);
        self.state
            .main_view
            .peek_scroll_handle
            .set_offset(gpui::point(px(0.0), px(0.0)));

        let mut async_cx = cx.to_async();
        let this_weak = cx.weak_entity();
        let path_clone = path.clone();

        let bg = cx.background_executor().spawn(async move {
            let mut items = Vec::new();
            let mut subfolders = 0usize;
            let mut files = 0usize;
            let mut total_size = 0u64;

            if let Ok(entries) = std::fs::read_dir(path_clone.as_path()) {
                for entry in entries.filter_map(|e| e.ok()).take(200) {
                    let name = entry.file_name().to_string_lossy().to_string();
                    let meta = entry.metadata().ok();
                    let is_dir = meta.as_ref().map(|m| m.is_dir()).unwrap_or(false);
                    let size = if is_dir {
                        0
                    } else {
                        meta.as_ref().map(|m| m.len()).unwrap_or(0)
                    };
                    if is_dir {
                        subfolders += 1;
                    } else {
                        files += 1;
                        total_size += size;
                    }
                    if items.len() < 64 {
                        let modified = meta
                            .and_then(|m| m.modified().ok())
                            .map(|st| {
                                let dt: chrono::DateTime<chrono::Local> = st.into();
                                dt.format("%Y-%m-%d %H:%M").to_string()
                            });
                        let ext = if is_dir {
                            "Folder".to_string()
                        } else {
                            entry
                                .path()
                                .extension()
                                .and_then(|e| e.to_str())
                                .map(|s| s.to_uppercase())
                                .unwrap_or_else(|| "File".to_string())
                        };
                        items.push(FolderPreviewItem {
                            name,
                            is_dir,
                            path: FilesPath::from(entry.path()),
                            size,
                            modified,
                            ext,
                        });
                    }
                }
            }
            items.sort_by(|a, b| {
                b.is_dir
                    .cmp(&a.is_dir)
                    .then_with(|| crate::ui::views::helpers::natural_cmp(&a.name, &b.name))
            });
            let recursive_total = crate::storage::folder_size::cached_size(path_clone.as_path())
                .unwrap_or_else(|| {
                    crate::storage::folder_size::compute_and_store(path_clone.as_path())
                });
            (items, subfolders, files, recursive_total.max(total_size))
        });

        cx.foreground_executor()
            .spawn(async move {
                let (items, subfolders, files, total_size) = bg.await;
                this_weak
                    .update(&mut async_cx, |this, cx| {
                        if this.state.main_view.preview_request_id != req_id {
                            return;
                        }
                        this.state.main_view.details_cache.folder_subfolders = subfolders;
                        this.state.main_view.details_cache.folder_files = files;
                        this.state.main_view.details_cache.folder_total_size = total_size;
                        this.state.main_view.preview_content = PreviewContent::Folder(items);
                        this.state.main_view.preview_text = Some(format!(
                            "{} folders, {} files",
                            subfolders, files
                        ));
                        cx.notify();
                    })
                    .ok();
            })
            .detach();
    }
    pub fn toggle_shortcut_overlay(&mut self, cx: &mut Context<Self>) {
        self.state.show_shortcut_overlay = !self.state.show_shortcut_overlay;
        cx.notify();
    }

    /// Undo the last file operation (Ctrl+Z).
    /// Stack pop happens on the UI thread; the blocking filesystem mutation
    /// runs on the background executor, then the outcome is committed back
    /// on the foreground with a view refresh — the UI thread never blocks.
    pub fn undo(&mut self, cx: &mut Context<Self>) {
        let Some(action) = self.state.undo_redo.pop_undo_action() else {
            tracing::debug!("Nothing to undo");
            cx.notify();
            return;
        };
        let weak_entity = cx.weak_entity();
        let mut async_cx = cx.to_async();
        let bg = cx.background_executor().clone();
        cx.foreground_executor()
            .spawn(async move {
                // Blocking fs mutation runs on the thread pool; only Send
                // state (the action) crosses into it, and comes back with the result.
                let (action, result) =
                    bg.spawn(async move { let r = action.undo(); (action, r) }).await;
                let _ = weak_entity.update(&mut async_cx, |this, cx| {
                    match result {
                        Ok(()) => {
                            tracing::info!("Undo successful");
                            this.state.undo_redo.commit_undo_success(action);
                            this.state.main_view.bump_items_revision();
                            this.refresh(cx);
                        }
                        Err(e) => {
                            tracing::warn!("Undo failed: {}", e);
                            this.state.undo_redo.commit_undo_failure(action);
                        }
                    }
                    cx.notify();
                });
            })
            .detach();
    }

    /// Redo the last undone operation (Ctrl+Y). Same off-thread pattern as [`Self::undo`].
    pub fn redo(&mut self, cx: &mut Context<Self>) {
        let Some(action) = self.state.undo_redo.pop_redo_action() else {
            tracing::debug!("Nothing to redo");
            cx.notify();
            return;
        };
        let weak_entity = cx.weak_entity();
        let mut async_cx = cx.to_async();
        let bg = cx.background_executor().clone();
        cx.foreground_executor()
            .spawn(async move {
                // Blocking fs mutation runs on the thread pool; only Send
                // state (the action) crosses into it, and comes back with the result.
                let (action, result) =
                    bg.spawn(async move { let r = action.execute(); (action, r) }).await;
                let _ = weak_entity.update(&mut async_cx, |this, cx| {
                    match result {
                        Ok(()) => {
                            tracing::info!("Redo successful");
                            this.state.undo_redo.commit_redo_success(action);
                            this.state.main_view.bump_items_revision();
                            this.refresh(cx);
                        }
                        Err(e) => {
                            tracing::warn!("Redo failed: {}", e);
                            this.state.undo_redo.commit_redo_failure(action);
                        }
                    }
                    cx.notify();
                });
            })
            .detach();
    }

    pub fn new_folder(&mut self, cx: &mut Context<Self>) {
        self.state.active_dialog = Some(DialogType::AddItem {
            is_folder: true,
            name: "New folder".to_string(),
            selected_all: true,
        });
        cx.notify();
    }

    pub fn new_file(&mut self, cx: &mut Context<Self>) {
        self.state.active_dialog = Some(DialogType::AddItem {
            is_folder: false,
            name: "New Text Document.txt".to_string(),
            selected_all: true,
        });
        cx.notify();
    }

    pub fn rename_selected(&mut self, cx: &mut Context<Self>) {
        if let Some(&first_selected) = self.state.active_view().selected_indices.iter().next() {
            self.start_rename(first_selected, cx);
        }
    }

    pub fn share_selected(&mut self, cx: &mut Context<Self>) {
        let hwnd = self.state.window_hwnd;
        let selected_paths: Vec<FilesPath> = self
            .state
            .active_view()
            .selected_indices
            .iter()
            .filter_map(|&idx| self.state.active_view().items.get(idx).map(|item| item.path.clone()))
            .collect();
        if !selected_paths.is_empty() {
            #[cfg(windows)]
            {
                cx.background_executor()
                    .spawn(async move {
                        let service =
                            crate::platform::windows::share_service::ShareService::new();
                        let _ = service.share_files(hwnd, &selected_paths).await;
                    })
                    .detach();
            }
            #[cfg(not(windows))]
            {
                let _ = (hwnd, selected_paths);
                tracing::info!("Share not implemented on this platform");
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
                    .start_job(format!("Bulk Rename ({} files)", total), crate::core::progress::OpKind::BulkRename)
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
        self.commit_rename_if_active(cx);
        let view = self.state.active_view_mut();
        if let Some(item) = view.items.get(index) {
            view.renaming_index = Some(index);
            view.rename_text = item.name.clone();
            view.rename_selected_all = true;
            view.rename_cursor_pos = view.rename_text.chars().count();
            let cpl = match view.view_mode {
                crate::config::config::ViewMode::Grid => {
                    let metrics = crate::ui::views::main_view::MainViewState::grid_layout_params(view.zoom_level);
                    let font_size = 11.5 + (view.zoom_level - 1.0).clamp(-0.4, 0.6) * 2.0;
                    let avail_w = metrics.item_width - 12.0;
                    ((avail_w.max(60.0) / (font_size * 0.58)).floor() as usize).max(8)
                }
                _ => 18,
            };
            view.rename_chars_per_line = cpl;
            cx.notify();
        }
    }

    pub fn commit_rename(&mut self, cx: &mut Context<Self>) {
        let active_focus = self.state.active_pane_focus();
        let target_pane = if self.state.pane_view(active_focus).renaming_index.is_some() {
            Some(active_focus)
        } else if self.state.main_view.renaming_index.is_some() {
            Some(crate::core::navigation_manager::PaneFocus::Primary)
        } else if self.state.secondary_view.renaming_index.is_some() {
            Some(crate::core::navigation_manager::PaneFocus::Secondary)
        } else {
            None
        };

        let Some(pane_focus) = target_pane else {
            return;
        };

        let rename_info = {
            let view = self.state.pane_view_mut(pane_focus);
            if let Some(idx) = view.renaming_index.take() {
                let new_name = view.rename_text.trim().to_string();
                view.rename_selected_all = false;
                view.rename_cursor_pos = 0;
                view.items.get(idx).map(|item| (idx, item.path.clone(), item.name.clone(), new_name))
            } else {
                None
            }
        };
        if let Some((idx, old_path, old_name, new_name)) = rename_info {
            if !new_name.is_empty() && new_name != old_name {
                if let Some(parent) = old_path.as_path().parent() {
                    let new_path = FilesPath::new(parent.join(&new_name));
                    let action = crate::core::undo_redo::RenameAction {
                        old_path: old_path.clone(),
                        new_path: new_path.clone(),
                    };
                    if let Err(e) = self.state.undo_redo.perform(Box::new(action)) {
                        tracing::error!("Failed to rename {:?} to {:?}: {}", old_path, new_path, e);
                    } else {
                        let view = self.state.pane_view_mut(pane_focus);
                        if let Some(item) = view.items.get_mut(idx) {
                            item.name = new_name;
                            item.path = new_path;
                        }
                        view.bump_items_revision();
                        let updated_items = view.items.clone();
                        self.state.pane_file_items(pane_focus).update(cx, |m, _| {
                            m.items = updated_items;
                        });
                    }
                }
            }
            cx.notify();
        }
    }

    pub fn cancel_rename(&mut self, cx: &mut Context<Self>) {
        self.state.main_view.renaming_index = None;
        self.state.secondary_view.renaming_index = None;
        self.state.main_view.rename_selected_all = false;
        self.state.secondary_view.rename_selected_all = false;
        self.state.main_view.rename_cursor_pos = 0;
        self.state.secondary_view.rename_cursor_pos = 0;
        cx.notify();
    }

    pub fn show_context_menu(
        &mut self,
        item_index: Option<usize>,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Clamp the menu inside the viewport (flip-up when near the bottom
        // edge) so tall menus never spill past the window like before.
        let vp = window.viewport_size();
        // Viewport-derived menu cap: tall (Expanded) menus scroll internally
        // instead of spilling past the main view, even on tiny windows.
        let max_h = (f32::from(vp.height) - 16.0).max(200.0);
        // Single settings read for the style: Expanded inlines the whole
        // classic list, so it reserves the full viewport (the menu body
        // scrolls internally past max_h instead of spilling).
        let menu_style = self
            .state
            .settings
            .read(cx)
            .config
            .preferences
            .context_menu_style;
        let expanded = menu_style == crate::config::config::ContextMenuStyle::Expanded;
        let est_h: f32 = if expanded {
            max_h
        } else if item_index.is_some() {
            720.0
        } else {
            380.0
        };
        let mut x: f32 = position.x.into();
        let mut y: f32 = position.y.into();
        let vpw: f32 = vp.width.into();
        let vph: f32 = vp.height.into();
        let top_limit = 40.0f32; // Below ribbon
        let bottom_limit = (vph - 36.0).max(top_limit + 120.0); // Above status bar
        if x + 300.0 > vpw {
            x = (vpw - 300.0).max(8.0);
        }
        if y + est_h > bottom_limit {
            y = (bottom_limit - est_h).max(top_limit);
        }
        let position = Point { x: px(x), y: px(y) };
        // Flyouts open left when there is no room on the right.
        let flip = x + 292.0 + 268.0 > vpw;
        // Pre-read selection data to avoid view.read(cx) crash during dialog render
        let selected_indices = &self.state.main_view.selected_indices;
        let has_selection = !selected_indices.is_empty();
        let selected_path = selected_indices
            .iter()
            .next()
            .and_then(|&idx| self.state.main_view.items.get(idx))
            .map(|item| item.path.clone());
        let selected_count = selected_indices.len();
        let is_dir = item_index
            .and_then(|idx| self.state.main_view.items.get(idx))
            .map(|item| {
                matches!(
                    item.file_type,
                    crate::storage::storage::FileType::Directory
                )
            })
            .unwrap_or(false);

        // A menu opening on top kills any visible hover tooltip (Files never
        // shows the preview popup above an open context menu). It also
        // invalidates any in-flight flyout timers from a previous menu.
        self.hover_tooltip = None;
        self.hover_item_index = None;
        self.hover_timer_task = None;
        self.submenu_grace = None;
        self.submenu_intent = None;

        let selected_paths: Vec<std::path::PathBuf> = self
            .state
            .main_view
            .selected_indices
            .iter()
            .filter_map(|&idx| self.state.main_view.items.get(idx))
            .map(|item| item.path.as_path().to_path_buf())
            .collect();
        let current_dir: std::path::PathBuf = self
            .state
            .main_view
            .primary_path
            .as_path()
            .to_path_buf();

        // "Windows classic" style: skip the GPUI menu entirely and show the
        // native shell popup (real verbs, real owners, zero porting).
        #[cfg(windows)]
        {
            if menu_style == crate::config::config::ContextMenuStyle::WindowsClassic {
                let (sx, sy) = {
                    use windows::Win32::Foundation::POINT;
                    use windows::Win32::Graphics::Gdi::ClientToScreen;
                    use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
                    let hwnd = unsafe { GetForegroundWindow() };
                    let mut pt = POINT {
                        x: f32::from(position.x) as i32,
                        y: f32::from(position.y) as i32,
                    };
                    if unsafe { ClientToScreen(hwnd, &mut pt) }.as_bool() {
                        (pt.x, pt.y)
                    } else {
                        (i32::MIN, i32::MIN)
                    }
                };
                crate::platform::windows::shell_menu_service::show_classic_menu(
                    selected_paths,
                    current_dir,
                    sx,
                    sy,
                );
                return;
            }
        }

        self.state.active_dialog = Some(crate::ui::components::dialogs::DialogType::ContextMenu {
            item_index,
            position,
            has_selection,
            selected_path,
            selected_count,
            is_dir,
            open_submenu: None,
            selected_paths,
            current_dir,
            classic: None,
            classic_loading: false,
            classic_open: Vec::new(),
            flip,
            expanded,
            max_h,
            win_w: vpw,
            win_h: vph,
        });
        // Preload the classic items the moment the menu opens (all styles):
        // phase 1 (labels) lands in ~50ms so the flyout renders instantly on
        // hover; phase 2 fills in the real icons right after.
        #[cfg(windows)]
        {
            if let Some(crate::ui::components::dialogs::DialogType::ContextMenu {
                selected_paths,
                current_dir,
                ..
            }) = self.state.active_dialog.as_ref()
            {
                let lp = selected_paths.clone();
                let ld = current_dir.clone();
                self.load_classic_entries(lp, ld, cx);
            }
        }
        self.menu_tip = None;
        cx.notify();
    }

    /// Switch the open flyout submenu inside the context menu (hover-driven,
    /// Files-community parity). No-op unless a context menu is showing.
    pub fn set_open_submenu(
        &mut self,
        kind: Option<crate::ui::components::dialogs::SubmenuKind>,
        cx: &mut Context<Self>,
    ) {
        // Any submenu (re)entry invalidates a pending grace-close AND any
        // pending intent-switch.
        self.submenu_grace_cancel();
        self.submenu_intent_cancel();
        let mut start_load: Option<(Vec<std::path::PathBuf>, std::path::PathBuf)> = None;
        if let Some(crate::ui::components::dialogs::DialogType::ContextMenu {
            open_submenu,
            selected_paths,
            current_dir,
            classic_open,
            ..
        }) = self.state.active_dialog.as_mut()
        {
            if *open_submenu != kind {
                *open_submenu = kind;
                cx.notify();
            }
            // Leaving "Show more options" collapses nested classic paths.
            if kind != Some(crate::ui::components::dialogs::SubmenuKind::ShowMore) {
                classic_open.clear();
            }
            // "Show more options" loads the real classic items on first hover
            // (Files expands them inline). Runs on a background task; the
            // dialog re-renders when they arrive.
            if kind == Some(crate::ui::components::dialogs::SubmenuKind::ShowMore) {
                start_load = Some((selected_paths.clone(), current_dir.clone()));
            }
        }
        if let Some((lp, ld)) = start_load {
            self.load_classic_entries(lp, ld, cx);
        }
    }

    /// Load the real classic shell items on a background task. Used by the
    /// ShowMore first-hover and by Expanded-style menus at open time.
    pub fn load_classic_entries(
        &mut self,
        paths: Vec<std::path::PathBuf>,
        dir: std::path::PathBuf,
        cx: &mut Context<Self>,
    ) {
        let should_load = matches!(
            self.state.active_dialog.as_ref(),
            Some(crate::ui::components::dialogs::DialogType::ContextMenu {
                classic: None,
                classic_loading: false,
                ..
            })
        );
        if !should_load {
            return;
        }
        if let Some(crate::ui::components::dialogs::DialogType::ContextMenu {
            classic_loading,
            classic_open,
            ..
        }) = self.state.active_dialog.as_mut()
        {
            *classic_loading = true;
            *classic_open = Vec::new();
        }
        let mut async_app = cx.to_async();
        let task = cx.spawn(
            move |weak: WeakEntity<Self>, _: &mut AsyncApp| async move {
                // Phase 1: labels/states only (no icon extraction, ~50ms) so
                // the flyout renders instantly; phase 2 fills in real icons.
                let fast =
                    crate::platform::windows::classic_menu::enumerate_classic_labels(
                        &paths, &dir,
                    );
                weak.update(&mut async_app, |this, cx| {
                    if let Some(
                        crate::ui::components::dialogs::DialogType::ContextMenu {
                            classic,
                            ..
                        },
                    ) = this.state.active_dialog.as_mut()
                    {
                        *classic = Some(fast);
                        cx.notify();
                    }
                })
                .ok();
                let full =
                    crate::platform::windows::classic_menu::enumerate_classic_menu(
                        &paths, &dir,
                    );
                weak.update(&mut async_app, |this, cx| {
                    if let Some(
                        crate::ui::components::dialogs::DialogType::ContextMenu {
                            classic,
                            classic_loading,
                            ..
                        },
                    ) = this.state.active_dialog.as_mut()
                    {
                        *classic = Some(full);
                        *classic_loading = false;
                        cx.notify();
                    }
                })
                .ok();
            },
        );
        self.menu_task = Some(task);
        cx.notify();
    }

    /// Set the open path inside nested classic ("Show more options") submenus.
    pub fn set_classic_open(&mut self, path: Vec<usize>, cx: &mut Context<Self>) {
        self.submenu_grace_cancel();
        self.submenu_intent_cancel();
        if let Some(crate::ui::components::dialogs::DialogType::ContextMenu {
            classic_open,
            ..
        }) = self.state.active_dialog.as_mut()
        {
            if *classic_open != path {
                *classic_open = path;
                cx.notify();
            }
        }
    }

    /// Invalidate any pending submenu grace-close (cursor re-entered).
    pub fn submenu_grace_cancel(&mut self) {
        self.submenu_grace_gen += 1;
        self.submenu_grace = None;
    }

    /// Hover-intent switch for top-level flyouts. First open (or re-entry)
    /// is instant; switching from one open flyout to a sibling waits ~200ms
    /// so diagonal travel toward the open panel is not killed by an instant
    /// takeover when the cursor brushes the sibling row.
    pub fn set_open_submenu_intent(
        &mut self,
        kind: crate::ui::components::dialogs::SubmenuKind,
        cx: &mut Context<Self>,
    ) {
        let switching = matches!(
            self.state.active_dialog.as_ref(),
            Some(crate::ui::components::dialogs::DialogType::ContextMenu {
                open_submenu: Some(_),
                ..
            })
        );
        if !switching {
            self.set_open_submenu(Some(kind), cx);
            return;
        }
        let already = matches!(
            self.state.active_dialog.as_ref(),
            Some(crate::ui::components::dialogs::DialogType::ContextMenu {
                open_submenu,
                ..
            }) if *open_submenu == Some(kind)
        );
        if already {
            self.set_open_submenu(Some(kind), cx);
            return;
        }
        self.submenu_intent_gen += 1;
        let intent_gen = self.submenu_intent_gen;
        let mut async_app = cx.to_async();
        let task = cx.spawn(
            move |weak: WeakEntity<Self>, _: &mut AsyncApp| async move {
                async_app
                    .background_executor()
                    .timer(std::time::Duration::from_millis(200))
                    .await;
                weak.update(&mut async_app, |this, cx| {
                    if this.submenu_intent_gen == intent_gen {
                        this.set_open_submenu(Some(kind), cx);
                    }
                })
                .ok();
            },
        );
        self.submenu_intent = Some(task);
    }

    /// Hover-intent switch for nested classic paths. Descending into (or
    /// back out of) the open path is instant; jumping to a diverging sibling
    /// path waits ~200ms for the same diagonal-travel reason as above.
    pub fn set_classic_open_intent(&mut self, path: Vec<usize>, cx: &mut Context<Self>) {
        let open: Vec<usize> = match self.state.active_dialog.as_ref() {
            Some(crate::ui::components::dialogs::DialogType::ContextMenu {
                classic_open,
                ..
            }) => classic_open.clone(),
            _ => Vec::new(),
        };
        let along = path.starts_with(&open) || open.starts_with(&path);
        if open.is_empty() || along {
            self.set_classic_open(path, cx);
            return;
        }
        self.submenu_intent_gen += 1;
        let intent_gen = self.submenu_intent_gen;
        let mut async_app = cx.to_async();
        let task = cx.spawn(
            move |weak: WeakEntity<Self>, _: &mut AsyncApp| async move {
                async_app
                    .background_executor()
                    .timer(std::time::Duration::from_millis(200))
                    .await;
                weak.update(&mut async_app, |this, cx| {
                    if this.submenu_intent_gen == intent_gen {
                        this.set_classic_open(path, cx);
                    }
                })
                .ok();
            },
        );
        self.submenu_intent = Some(task);
    }

    /// Cancel a pending hover-intent switch (panel entered, menu closing).
    pub fn submenu_intent_cancel(&mut self) {
        self.submenu_intent_gen += 1;
        self.submenu_intent = None;
    }

    /// Close the open flyout after a short grace delay so diagonal cursor
    /// travel from a row into its (outside-bounds) flyout panel doesn't kill
    /// it mid-flight. `close` runs on expiry unless invalidated first.
    pub fn submenu_grace_close(
        &mut self,
        close: impl FnOnce(&mut Self, &mut Context<Self>) + Send + 'static,
        cx: &mut Context<Self>,
    ) {
        self.submenu_grace_gen += 1;
        let grace = self.submenu_grace_gen;
        let mut async_app = cx.to_async();
        let task = cx.spawn(
            move |weak: WeakEntity<Self>, _: &mut AsyncApp| async move {
                async_app
                    .background_executor()
                    .timer(std::time::Duration::from_millis(500))
                    .await;
                weak
                    .update(&mut async_app, |this, cx| {
                        if this.submenu_grace_gen == grace {
                            if this.submenu_panel_hovered || this.submenu_row_hovered {
                                return;
                            }
                            close(this, cx);
                        }
                    })
                    .ok();
            },
        );
        self.submenu_grace = Some(task);
    }

    /// Invoke a classic verb, then close the menu. The invoke runs on a
    /// background task (fresh STA query + label guard, never a stale offset).
    pub fn invoke_classic(
        &mut self,
        offset: u32,
        label: String,
        paths: Vec<std::path::PathBuf>,
        dir: std::path::PathBuf,
        cx: &mut Context<Self>,
    ) {
        self.close_dialog(cx);
        let mut async_app = cx.to_async();
        let task = cx.spawn(
            move |_weak: WeakEntity<Self>, _: &mut AsyncApp| async move {
            if let Err(e) = crate::platform::windows::classic_menu::invoke_classic_verb(
                &paths,
                &dir,
                offset,
                &label,
            ) {
                tracing::warn!("Classic verb invoke failed: {:#}", e);
            }
            let _ = &mut async_app;
        });
        self.menu_task = Some(task);
    }

    /// Create "New Folder" in the current directory (used by the New › flyout).
    pub fn create_new_folder(&mut self, cx: &mut Context<Self>) {
        let current_dir = self
            .state
            .main_view
            .primary_path
            .as_path()
            .to_path_buf();
        let mut folder_name = "New Folder".to_string();
        let mut counter = 1;
        while current_dir.join(&folder_name).exists() {
            folder_name = format!("New Folder ({})", counter);
            counter += 1;
        }
        let new_folder_path = current_dir.join(&folder_name);
        match std::fs::create_dir(&new_folder_path) {
            Ok(_) => {
                tracing::info!("Created folder: {:?}", new_folder_path);
                self.refresh(cx);
            }
            Err(e) => {
                tracing::error!("Failed to create folder {:?}: {}", new_folder_path, e);
            }
        }
        self.close_dialog(cx);
    }

    /// Create "New Text Document.txt" in the current directory (New › flyout).
    pub fn create_new_text_document(&mut self, cx: &mut Context<Self>) {
        let current_dir = self
            .state
            .main_view
            .primary_path
            .as_path()
            .to_path_buf();
        let mut file_name = "New Text Document.txt".to_string();
        let mut counter = 1;
        while current_dir.join(&file_name).exists() {
            file_name = format!("New Text Document ({}).txt", counter);
            counter += 1;
        }
        let new_file_path = current_dir.join(&file_name);
        match std::fs::File::create(&new_file_path) {
            Ok(_) => {
                tracing::info!("Created file: {:?}", new_file_path);
                self.refresh(cx);
            }
            Err(e) => {
                tracing::error!("Failed to create file {:?}: {}", new_file_path, e);
            }
        }
        self.close_dialog(cx);
    }

    /// Dispatch the current selection to a SendTo target (Send to › flyout).
    /// Folders receive a copy of the files; anything else is launched with
    /// the selected paths as arguments (Explorer semantics, simplified).
    pub fn send_to_target(&mut self, target: std::path::PathBuf, cx: &mut Context<Self>) {
        let selected: Vec<std::path::PathBuf> = self
            .state
            .main_view
            .selected_indices
            .iter()
            .filter_map(|&idx| self.state.main_view.items.get(idx))
            .map(|item| item.path.as_path().to_path_buf())
            .collect();
        if target.is_dir() {
            for src in &selected {
                if let Some(name) = src.file_name() {
                    let dest = target.join(name);
                    if src.is_dir() {
                        let _ = copy_dir_recursive(src, &dest);
                    } else {
                        let _ = std::fs::copy(src, &dest);
                    }
                }
            }
            self.close_dialog(cx);
            self.refresh(cx);
        } else if !selected.is_empty() {
            #[cfg(windows)]
            {
                use std::os::windows::ffi::OsStrExt;
                let verb_w: Vec<u16> = "open".encode_utf16().chain([0]).collect();
                let file_w: Vec<u16> = target
                    .as_os_str()
                    .encode_wide()
                    .chain([0])
                    .collect();
                let args: String = selected
                    .iter()
                    .map(|p| format!("\"{}\"", p.to_string_lossy()))
                    .collect::<Vec<_>>()
                    .join(" ");
                let args_w: Vec<u16> = args.encode_utf16().chain([0]).collect();
                let mut sei = windows::Win32::UI::Shell::SHELLEXECUTEINFOW::default();
                sei.cbSize = std::mem::size_of::<windows::Win32::UI::Shell::SHELLEXECUTEINFOW>() as u32;
                sei.lpVerb = windows::core::PCWSTR(verb_w.as_ptr());
                sei.lpFile = windows::core::PCWSTR(file_w.as_ptr());
                sei.lpParameters = windows::core::PCWSTR(args_w.as_ptr());
                sei.nShow = windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL.0;
                unsafe {
                    let _ = windows::Win32::UI::Shell::ShellExecuteExW(&mut sei);
                }
            }
            self.close_dialog(cx);
        } else {
            self.close_dialog(cx);
        }
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

    /// Cycle Details -> List -> Grid -> Columns (toolbar View button).
    pub fn cycle_view_mode(&mut self, cx: &mut Context<Self>) {
        use crate::config::config::ViewMode as VM;
        let next = match self.state.main_view.view_mode {
            VM::Details => VM::List,
            VM::List => VM::Cards,
            VM::Cards => VM::Grid,
            VM::Grid => VM::Columns,
            VM::Columns => VM::Details,
        };
        self.set_view_mode(next, cx);
    }

    pub fn set_view_mode(&mut self, mode: ViewMode, cx: &mut Context<Self>) {
        let old_mode = self.state.main_view.view_mode.clone();
        let current_zoom = self.state.main_view.zoom_level;
        let new_zoom = self.state.navigation.update(cx, |nav, _cx| {
            let tab = nav.tabs.active_tab_mut();
            // Save the old mode's zoom level
            tab.per_view_zoom_levels.insert(old_mode, current_zoom);
            // Switch to the new view mode
            tab.view_mode = mode.clone();
            // Restore the new mode's saved zoom, or keep current zoom as default
            let mode_zoom = tab
                .per_view_zoom_levels
                .get(&mode)
                .copied()
                .unwrap_or(current_zoom);
            tab.zoom_level = mode_zoom;
            mode_zoom
        });
        self.state.main_view.view_mode = mode.clone();
        self.state.main_view.zoom_level = new_zoom;
        self.state.secondary_view.view_mode = mode.clone();
        self.state.secondary_view.zoom_level = new_zoom;

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

        // Persist session so per-tab view mode and zoom are saved
        self.save_session(cx);

        cx.notify();
    }

    /// Toggle the per-directory view/zoom lock on/off

    pub fn set_settings_tab(&mut self, tab: SettingsTab, cx: &mut Context<Self>) {
        self.state.active_settings_tab = tab;
        cx.notify();
    }

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

    pub fn delete_recycle_bin_item(&mut self, index: usize, cx: &mut Context<Self>) {
        let items = self.state.main_view.recycle_bin_items.clone();
        if let Some(item) = items.get(index) {
            let path = std::path::PathBuf::from(&item.original_path);
            let service = RecycleBinService::new();
            let mut async_cx = cx.to_async();
            let this_weak = cx.weak_entity();
            cx.foreground_executor()
                .spawn(async move {
                    let _ = service.delete_item(&path).await;
                    this_weak
                        .update(&mut async_cx, |this, cx| {
                            if index < this.state.main_view.recycle_bin_items.len() {
                                this.state.main_view.recycle_bin_items.remove(index);
                            }
                            this.state.main_view.selected_indices.remove(&index);
                            cx.notify();
                        })
                        .ok();
                })
                .detach();
        }
    }

    pub fn restore_selected_recycle_bin(&mut self, cx: &mut Context<Self>) {
        let mut indices: Vec<usize> = self
            .state
            .main_view
            .selected_indices
            .iter()
            .copied()
            .collect();
        if indices.is_empty() {
            return;
        }
        indices.sort_unstable_by(|a, b| b.cmp(a));
        let paths: Vec<(usize, std::path::PathBuf)> = indices
            .into_iter()
            .filter_map(|idx| {
                self.state
                    .main_view
                    .recycle_bin_items
                    .get(idx)
                    .map(|it| (idx, std::path::PathBuf::from(&it.original_path)))
            })
            .collect();

        let service = RecycleBinService::new();
        let mut async_cx = cx.to_async();
        let this_weak = cx.weak_entity();
        cx.foreground_executor()
            .spawn(async move {
                for (_, p) in &paths {
                    let _ = service.restore_item(p).await;
                }
                this_weak
                    .update(&mut async_cx, |this, cx| {
                        for (idx, _) in &paths {
                            if *idx < this.state.main_view.recycle_bin_items.len() {
                                this.state.main_view.recycle_bin_items.remove(*idx);
                            }
                        }
                        this.state.main_view.selected_indices.clear();
                        cx.notify();
                    })
                    .ok();
            })
            .detach();
    }

    pub fn delete_selected_recycle_bin(&mut self, cx: &mut Context<Self>) {
        let mut indices: Vec<usize> = self
            .state
            .main_view
            .selected_indices
            .iter()
            .copied()
            .collect();
        if indices.is_empty() {
            return;
        }
        indices.sort_unstable_by(|a, b| b.cmp(a));
        let paths: Vec<(usize, std::path::PathBuf)> = indices
            .into_iter()
            .filter_map(|idx| {
                self.state
                    .main_view
                    .recycle_bin_items
                    .get(idx)
                    .map(|it| (idx, std::path::PathBuf::from(&it.original_path)))
            })
            .collect();

        let service = RecycleBinService::new();
        let mut async_cx = cx.to_async();
        let this_weak = cx.weak_entity();
        cx.foreground_executor()
            .spawn(async move {
                for (_, p) in &paths {
                    let _ = service.delete_item(p).await;
                }
                this_weak
                    .update(&mut async_cx, |this, cx| {
                        for (idx, _) in &paths {
                            if *idx < this.state.main_view.recycle_bin_items.len() {
                                this.state.main_view.recycle_bin_items.remove(*idx);
                            }
                        }
                        this.state.main_view.selected_indices.clear();
                        cx.notify();
                    })
                    .ok();
            })
            .detach();
    }

    pub fn set_string_setting(&mut self, key: &str, value: String, cx: &mut Context<Self>) {
        let mut new_pane_layout: Option<String> = None;
        let mut new_sort_priority: Option<String> = None;
        let settings = self.state.settings.clone();
        settings.update(cx, |s, _cx| {
            match key {
                "locale" => s.config.preferences.locale = value,
                "startup_mode" => s.config.session.startup_mode = value,
                "date_format" => s.config.preferences.date_format = value,
                "size_unit_format" => s.config.preferences.size_unit_format = value,
                "status_center_visibility" => {
                s.config.appearance.status_center_visibility = value
                }
                "delete_confirmation_policy" => {
                    s.config.preferences.confirm_delete = value != "never";
                    s.config.preferences.delete_confirmation_policy = value;
                }
                "theme_preset" => s.config.appearance.theme_preset = value,
                "sort_priority" => {
                    s.config.view.folder_on_top = value == "folders_first";
                    s.config.view.sort_priority = value.clone();
                    new_sort_priority = Some(value);
                }
                "group_by_date_unit" => s.config.view.group_by_date_unit = value,
                "pane_layout" => {
                    new_pane_layout = Some(value);
                }
                "mouse4_action" => s.config.actions.mouse4_action = value,
                "mouse5_action" => s.config.actions.mouse5_action = value,
                "middle_click_folder_action" => {
                    s.config.actions.middle_click_folder_action = value
                }
                "middle_click_tab_action" => s.config.actions.middle_click_tab_action = value,
                "ctrl_click_folder_action" => s.config.actions.ctrl_click_folder_action = value,
                "double_click_empty_action" => {
                    s.config.actions.double_click_empty_space_go_up = value == "navigate_up";
                    s.config.actions.double_click_empty_action = value;
                }
                "ctrl_wheel_action" => s.config.actions.ctrl_wheel_action = value,
                "shift_wheel_action" => s.config.actions.shift_wheel_action = value,
                "open_in_ide_visibility" => s.config.devtools.open_in_ide_visibility = value,
                "default_ide_name" => s.config.devtools.ide_name = value,
                "default_ide_path" => s.config.devtools.ide_path = value,
                _ => {}
            }
            let _ = s.config.save();
        });

        if let Some(layout_str) = new_pane_layout {
            use crate::core::navigation_manager::PaneLayout;
            let layout = match layout_str.as_str() {
                "vertical" => PaneLayout::DualVertical,
                "horizontal" => PaneLayout::DualHorizontal,
                _ => PaneLayout::Single,
            };
            self.state.navigation.update(cx, |nav, _cx| {
                nav.pane_layout = layout;
            });
            self.state.main_view.pane_type = match layout {
                PaneLayout::Single => crate::ui::views::main_view::PaneType::Single,
                _ => crate::ui::views::main_view::PaneType::Dual,
            };
            if self.state.main_view.pane_type == crate::ui::views::main_view::PaneType::Dual {
                self.sync_secondary_pane_on_split(cx);
            }
        }

        if let Some(prio) = new_sort_priority {
            let ff = prio == "folders_first";
            self.state.main_view.sort.folders_first = ff;
            self.state.main_view.apply_sort();
            self.state.main_view.bump_items_revision();
            self.state.secondary_view.sort.folders_first = ff;
            self.state.secondary_view.apply_sort();
            self.state.secondary_view.bump_items_revision();
            self.sync_pane_selection_model(crate::core::navigation_manager::PaneFocus::Primary, cx);
            self.sync_pane_selection_model(crate::core::navigation_manager::PaneFocus::Secondary, cx);
        }

        cx.notify();
    }

    pub fn set_backdrop_material(&mut self, material: String, cx: &mut Context<Self>) {
        let settings = self.state.settings.clone();
        settings.update(cx, move |s, _cx| {
            match material.as_str() {
                "MicaAlt" => {
                    s.config.appearance.mica_enabled = true;
                    s.config.appearance.mica_alt = true;
                    s.config.appearance.transparent_window = false;
                }
                "Mica" => {
                    s.config.appearance.mica_enabled = true;
                    s.config.appearance.mica_alt = false;
                    s.config.appearance.transparent_window = false;
                }
                "Acrylic" | "Transparent" => {
                    s.config.appearance.mica_enabled = false;
                    s.config.appearance.mica_alt = false;
                    s.config.appearance.transparent_window = true;
                }
                _ => {
                    s.config.appearance.mica_enabled = false;
                    s.config.appearance.mica_alt = false;
                    s.config.appearance.transparent_window = false;
                }
            }
            s.config.appearance.backdrop_material = material;
            let _ = s.config.save();
        });
        cx.notify();
    }

    pub fn add_startup_page(&mut self, path: String, cx: &mut Context<Self>) {
        if path.trim().is_empty() {
            return;
        }
        let settings = self.state.settings.clone();
        settings.update(cx, move |s, _cx| {
            if !s.config.session.startup_pages.contains(&path) {
                s.config.session.startup_pages.push(path);
                let _ = s.config.save();
            }
        });
        cx.notify();
    }

    pub fn open_keybinding_recorder(
        &mut self,
        action: crate::core::action_registry::ActionKind,
        is_add_mode: bool,
        cx: &mut Context<Self>,
    ) {
        let current = self.state.action_registry.get_key(action).cloned();
        self.state.settings_ui.recorder = Some(crate::ui::components::dialogs::KeyBindingRecorderState {
            action,
            is_add_mode,
            recorded_combo: current,
            conflict_action: None,
            validation_error: None,
        });
        cx.notify();
    }

    pub fn record_candidate_binding(
        &mut self,
        combo: crate::core::action_registry::KeyCombination,
        cx: &mut Context<Self>,
    ) {
        if let Some(ref mut rec) = self.state.settings_ui.recorder {
            let conflict = self
                .state
                .action_registry
                .find_conflict(&combo, Some(rec.action));
            rec.recorded_combo = Some(combo);
            rec.conflict_action = conflict;
            rec.validation_error = None;
            cx.notify();
        }
    }

    pub fn commit_keybinding_recorder(&mut self, cx: &mut Context<Self>) {
        if let Some(rec) = self.state.settings_ui.recorder.take() {
            if let Some(combo) = rec.recorded_combo {
                if !combo.is_empty() {
                    self.state.action_registry.rebind(rec.action, combo);
                    self.persist_action_bindings(cx);
                }
            }
            cx.notify();
        }
    }

    pub fn reset_single_action_binding(
        &mut self,
        action: crate::core::action_registry::ActionKind,
        cx: &mut Context<Self>,
    ) {
        if let Some(default_combo) = crate::core::action_registry::ActionRegistry::default_binding(action) {
            self.state.action_registry.rebind(action, default_combo);
        } else {
            self.state.action_registry.unbind(action);
        }
        self.persist_action_bindings(cx);
        cx.notify();
    }

    pub fn reset_all_action_bindings(&mut self, cx: &mut Context<Self>) {
        self.state.action_registry = crate::core::action_registry::ActionRegistry::new();
        self.state.settings_ui.recorder = None;
        self.persist_action_bindings(cx);
        cx.notify();
    }

    pub fn unbind_action(
        &mut self,
        action: crate::core::action_registry::ActionKind,
        cx: &mut Context<Self>,
    ) {
        self.state.action_registry.unbind(action);
        if let Some(ref rec) = self.state.settings_ui.recorder {
            if rec.action == action {
                self.state.settings_ui.recorder = None;
            }
        }
        self.persist_action_bindings(cx);
        cx.notify();
    }

    pub(crate) fn persist_action_bindings(&mut self, cx: &mut Context<Self>) {
        let mut map = std::collections::HashMap::new();
        for (action, combo) in self.state.action_registry.iter() {
            map.insert(format!("{:?}", action), combo.to_string());
        }
        let reg_clone = self.state.action_registry.clone();
        let settings = self.state.settings.clone();
        settings.update(cx, move |s, _cx| {
            s.config.keybindings.bindings = map;
            s.config.action_bindings = Some(reg_clone);
            let _ = s.config.save();
        });
    }

    pub fn refresh_home_tags_cache(&mut self, cx: &mut Context<Self>) {
        let defined_tags = self.state.settings.read(cx).config.tag_definitions.clone();
        let db_counts: std::collections::HashMap<String, i64> = self
            .state
            .tag_service
            .get_all_tags()
            .unwrap_or_default()
            .into_iter()
            .collect();

        let mut merged: Vec<(String, String, i64)> = Vec::new();
        for def in &defined_tags {
            let count = db_counts.get(&def.name).copied().unwrap_or(0);
            merged.push((def.name.clone(), def.color.clone(), count));
        }
        for (tag_name, count) in db_counts {
            if !merged.iter().any(|(n, _, _)| n.eq_ignore_ascii_case(&tag_name)) {
                merged.push((tag_name, "#0078D4".to_string(), count));
            }
        }
        self.state.main_view.home_tags_cache = merged;
    }

    pub fn commit_create_tag(&mut self, cx: &mut Context<Self>) {
        let name = self.state.settings_ui.new_tag_name.trim().to_string();
        let color = self.state.settings_ui.new_tag_color.clone();
        if name.is_empty() {
            return;
        }
        let settings = self.state.settings.clone();
        settings.update(cx, move |s, _cx| {
            if !s
                .config
                .tag_definitions
                .iter()
                .any(|t| t.name.eq_ignore_ascii_case(&name))
            {
                s.config.tag_definitions.push(crate::config::config::TagDefinition {
                    id: format!("tag_{}", s.config.tag_definitions.len() + 1),
                    name,
                    color,
                });
                let _ = s.config.save();
            }
        });
        self.state.settings_ui.new_tag_name.clear();
        self.state.settings_ui.is_creating_tag = false;
        self.refresh_home_tags_cache(cx);
        cx.notify();
    }

    pub fn update_tag_color(&mut self, tag_name: String, color: String, cx: &mut Context<Self>) {
        let settings = self.state.settings.clone();
        settings.update(cx, move |s, _cx| {
            if let Some(t) = s
                .config
                .tag_definitions
                .iter_mut()
                .find(|t| t.name.eq_ignore_ascii_case(&tag_name))
            {
                t.color = color;
                let _ = s.config.save();
            }
        });
        self.refresh_home_tags_cache(cx);
        cx.notify();
    }

    pub fn delete_tag_definition(&mut self, tag_name: String, cx: &mut Context<Self>) {
        let _ = self.state.tag_service.delete_tag(&tag_name);
        let settings = self.state.settings.clone();
        settings.update(cx, move |s, _cx| {
            s.config
                .tag_definitions
                .retain(|t| !t.name.eq_ignore_ascii_case(&tag_name));
            let _ = s.config.save();
        });
        self.sync_directory_tags(cx);
        self.refresh_home_tags_cache(cx);
        cx.notify();
    }

    pub fn clear_thumbnail_cache(&mut self, cx: &mut Context<Self>) {
        if let Ok(mut cache) = self.state.icon_cache.lock() {
            cache.clear();
        }
        self.state.main_view.icon_paths.clear();
        self.state.secondary_view.icon_paths.clear();
        let cache_dir = crate::app_state::icons_cache_dir();
        let _ = std::fs::remove_dir_all(&cache_dir);
        let _ = std::fs::create_dir_all(&cache_dir);
        self.state.status_center.push_notification(
            "Thumbnail Cache Cleared".to_string(),
            "All cached thumbnails and icons have been removed.".to_string(),
            crate::ui::components::status_center::ToastSeverity::Success,
            None,
        );
        cx.notify();
    }

    pub fn export_settings_backup(&mut self, cx: &mut Context<Self>) {
        let backup_path = dirs::download_dir()
            .or_else(dirs::home_dir)
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("files-rs-settings-backup.json");
        let cfg = self.state.settings.read(cx).config.clone();
        if let Ok(json) = serde_json::to_string_pretty(&cfg) {
            if std::fs::write(&backup_path, json).is_ok() {
                self.state.status_center.push_notification(
                    "Settings Exported".to_string(),
                    format!("Saved backup to {}", backup_path.display()),
                    crate::ui::components::status_center::ToastSeverity::Success,
                    None,
                );
            }
        }
        cx.notify();
    }

    pub fn import_settings_backup(&mut self, cx: &mut Context<Self>) {
        let backup_path = dirs::download_dir()
            .or_else(dirs::home_dir)
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("files-rs-settings-backup.json");
        if let Ok(content) = std::fs::read_to_string(&backup_path) {
            if let Ok(imported) = serde_json::from_str::<crate::config::config::AppConfig>(&content)
            {
                let settings = self.state.settings.clone();
                settings.update(cx, move |s, _cx| {
                    s.config = imported;
                    let _ = s.config.save();
                });
                self.state.status_center.push_notification(
                    "Settings Imported".to_string(),
                    format!("Restored configuration from {}", backup_path.display()),
                    crate::ui::components::status_center::ToastSeverity::Success,
                    None,
                );
            }
        }
        cx.notify();
    }

    pub fn open_settings_json_file(&mut self, cx: &mut Context<Self>) {
        let config_path = crate::config::config::AppConfig::config_path();
        if let Some(parent) = config_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if !config_path.exists() {
            let _ = self.state.settings.read(cx).config.save();
        }
        self.open_in_ide(Some(FilesPath::from(config_path)), cx);
    }

    pub fn remove_recent_file(&mut self, path: String, cx: &mut Context<Self>) {
        let settings = self.state.settings.clone();
        settings.update(cx, move |s, _cx| {
            s.config.session.recent_paths.retain(|p| p != &path);
            let _ = s.config.save();
        });
        cx.notify();
    }

    pub fn clear_recent_files(&mut self, cx: &mut Context<Self>) {
        let settings = self.state.settings.clone();
        settings.update(cx, |s, _cx| {
            s.config.session.recent_paths.clear();
            let _ = s.config.save();
        });
        cx.notify();
    }

    pub fn set_sort_column(&mut self, col: crate::config::config::SortBy, cx: &mut Context<Self>) {
        let pane_focus = self.state.active_pane_focus();
        let asc = {
            let view = self.state.pane_view_mut(pane_focus);
            view.toggle_sort(col.clone());
            view.bump_items_revision();
            view.sort.ascending
        };
        // Keep main_view synced if primary
        if pane_focus != crate::core::navigation_manager::PaneFocus::Primary {
            self.state.main_view.sort = self.state.secondary_view.sort.clone();
            self.state.main_view.bump_items_revision();
        }
        self.sync_pane_selection_model(pane_focus, cx);
        let settings = self.state.settings.clone();
        settings.update(cx, move |s, _cx| {
            s.config.view.sort_by = col;
            s.config.view.sort_ascending = asc;
            let _ = s.config.save();
        });
        cx.notify();
    }

    pub fn cycle_sort_column(&mut self, cx: &mut Context<Self>) {
        use crate::config::config::SortBy;
        let pane_focus = self.state.active_pane_focus();
        let next = match self.state.pane_view(pane_focus).sort.column {
            SortBy::Name => SortBy::DateModified,
            SortBy::DateModified => SortBy::Size,
            SortBy::Size => SortBy::Type,
            SortBy::Type => SortBy::DateCreated,
            SortBy::DateCreated => SortBy::Tag,
            SortBy::Tag => SortBy::Name,
        };
        {
            let view = self.state.pane_view_mut(pane_focus);
            view.sort.column = next.clone();
            view.bump_items_revision();
        }
        self.sync_pane_selection_model(pane_focus, cx);
        let settings = self.state.settings.clone();
        settings.update(cx, move |s, _cx| {
            s.config.view.sort_by = next;
            let _ = s.config.save();
        });
        cx.notify();
    }

    pub fn set_sort_ascending(&mut self, ascending: bool, cx: &mut Context<Self>) {
        let pane_focus = self.state.active_pane_focus();
        {
            let view = self.state.pane_view_mut(pane_focus);
            view.sort.ascending = ascending;
            view.bump_items_revision();
        }
        self.state.main_view.sort.ascending = ascending;
        self.state.main_view.bump_items_revision();
        self.sync_pane_selection_model(pane_focus, cx);
        let settings = self.state.settings.clone();
        settings.update(cx, move |s, _cx| {
            s.config.view.sort_ascending = ascending;
            let _ = s.config.save();
        });
        cx.notify();
    }

    pub fn set_group_by(&mut self, group: crate::config::config::GroupColumn, cx: &mut Context<Self>) {
        if group == crate::config::config::GroupColumn::Tag {
            self.sync_directory_tags(cx);
        }
        let pane_focus = self.state.active_pane_focus();
        {
            let view = self.state.pane_view_mut(pane_focus);
            if view.group_by == group && group != crate::config::config::GroupColumn::None {
                view.group_ascending = !view.group_ascending;
            } else {
                view.group_by = group;
                view.group_ascending = true;
                view.collapsed_groups.clear();
                view.bouncy_scroll.set_immediate_offset(0.0);
                view.uniform_list_scroll_handle
                    .0
                    .borrow()
                    .base_handle
                    .set_offset(gpui::point(px(0.0), px(0.0)));
            }
            view.bump_items_revision();
        }
        self.sync_pane_selection_model(pane_focus, cx);
        let settings = self.state.settings.clone();
        settings.update(cx, move |s, _cx| {
            s.config.view.group_by = group;
            let _ = s.config.save();
        });
        cx.notify();
    }

    pub fn toggle_toolbar_flyout(&mut self, flyout_id: &str, cx: &mut Context<Self>) {
        self.commit_rename_if_active(cx);
        if self.state.main_view.toolbar_customization.active_flyout.as_deref() == Some(flyout_id) {
            self.state.main_view.toolbar_customization.active_flyout = None;
        } else {
            self.state.main_view.toolbar_customization.active_flyout = Some(flyout_id.to_string());
        }
        cx.notify();
    }

    pub fn close_toolbar_flyout(&mut self, cx: &mut Context<Self>) {
        if self.state.main_view.toolbar_customization.active_flyout.is_some() {
            self.state.main_view.toolbar_customization.active_flyout = None;
            cx.notify();
        }
    }

    pub fn compress_selected_items(&mut self, cx: &mut Context<Self>) {
        let sources: Vec<std::path::PathBuf> = self
            .state
            .active_view()
            .selected_indices
            .iter()
            .filter_map(|&i| self.state.active_view().items.get(i))
            .map(|item| item.path.as_path().to_path_buf())
            .collect();
        let current_dir = self.state.active_view().primary_path.as_path().to_path_buf();
        let first_name = sources
            .first()
            .and_then(|p| p.file_name())
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "Archive".to_string());
        let destination = current_dir.join(format!("{}.zip", first_name));
        self.state.active_dialog = Some(crate::ui::components::dialogs::DialogType::CreateArchive {
            state: Box::new(
                crate::ui::dialog_services::archive_dialog::CreateArchiveState::new(
                    sources,
                    destination,
                ),
            ),
        });
        cx.notify();
    }

    pub fn extract_selected_archive(&mut self, cx: &mut Context<Self>) {
        if let Some(&idx) = self.state.active_view().selected_indices.iter().next() {
            if let Some(item) = self.state.active_view().items.get(idx) {
                let archive_path = item.path.as_path().to_path_buf();
                let current_dir = self.state.active_view().primary_path.as_path().to_path_buf();
                self.state.active_dialog =
                    Some(crate::ui::components::dialogs::DialogType::DecompressArchive {
                        state: Box::new(
                            crate::ui::dialog_services::decompress_dialog::DecompressState::new(
                                archive_path,
                                current_dir,
                            ),
                        ),
                    });
                cx.notify();
            }
        }
    }

    /// Load preview content & details cache for the currently selected item in the active pane.
    pub fn load_preview_for_selected_item(&mut self, cx: &mut Context<Self>) {
        let pane_focus = self.state.active_pane_focus();
        let selected_path = {
            let v = self.state.pane_view(pane_focus);
            v.selected_indices
                .iter()
                .next()
                .and_then(|&idx| v.items.get(idx))
                .map(|it| it.path.clone())
        };
        if let Some(p) = selected_path {
            self.load_preview_content(p.clone(), cx);
            self.update_details_cache_from_path(&p, cx);
        }
    }

    /// Toggle the Spacebar Peek Preview Modal (`QuickLook` / Finder Spacebar parity).
    pub fn toggle_peek_modal(&mut self, cx: &mut Context<Self>) {
        if self.state.main_view.show_peek_modal {
            self.close_peek_modal(cx);
            return;
        }
        let pane_focus = self.state.active_pane_focus();
        let has_items = !self.state.pane_view(pane_focus).items.is_empty();
        if !has_items {
            return;
        }
        // If no item is currently selected, select the focused or first visible item
        if self.state.pane_view(pane_focus).selected_indices.is_empty() {
            let visible = self.state.pane_view(pane_focus).visible_items_rc();
            let target_idx = self
                .state
                .pane_view(pane_focus)
                .last_clicked_index
                .or_else(|| visible.first().copied())
                .unwrap_or(0);
            self.select_item_in_pane(pane_focus, target_idx, false, false, cx);
        }
        self.state.main_view.show_peek_modal = true;
        self.load_preview_for_selected_item(cx);
        cx.notify();
    }

    pub fn close_peek_modal(&mut self, cx: &mut Context<Self>) {
        if self.state.main_view.show_peek_modal {
            self.state.main_view.show_peek_modal = false;
            self.state.main_view.video_playing = false;
            cx.notify();
        }
    }

    pub fn peek_navigate_prev(&mut self, cx: &mut Context<Self>) {
        let pane_focus = self.state.active_pane_focus();
        let visual = self.state.pane_view(pane_focus).visual_order_items();
        if visual.is_empty() {
            return;
        }
        let cur_idx = self
            .state
            .pane_view(pane_focus)
            .selected_indices
            .iter()
            .next()
            .copied()
            .unwrap_or(visual[0]);
        let pos = visual.iter().position(|&i| i == cur_idx).unwrap_or(0);
        let prev_pos = if pos > 0 { pos - 1 } else { visual.len() - 1 };
        let next_idx = visual[prev_pos];
        self.select_item_in_pane(pane_focus, next_idx, false, false, cx);
        self.scroll_item_into_view(pane_focus, next_idx);
    }

    pub fn peek_navigate_next(&mut self, cx: &mut Context<Self>) {
        let pane_focus = self.state.active_pane_focus();
        let visual = self.state.pane_view(pane_focus).visual_order_items();
        if visual.is_empty() {
            return;
        }
        let cur_idx = self
            .state
            .pane_view(pane_focus)
            .selected_indices
            .iter()
            .next()
            .copied()
            .unwrap_or(visual[0]);
        let pos = visual.iter().position(|&i| i == cur_idx).unwrap_or(0);
        let next_pos = (pos + 1) % visual.len();
        let next_idx = visual[next_pos];
        self.select_item_in_pane(pane_focus, next_idx, false, false, cx);
        self.scroll_item_into_view(pane_focus, next_idx);
    }

    pub fn toggle_video_playback(&mut self, cx: &mut Context<Self>) {
        if let Some(player) = &self.state.video_player {
            if player.is_playing() || self.state.main_view.video_playing {
                player.pause();
                self.state.main_view.video_playing = false;
            } else {
                player.play();
                self.state.main_view.video_playing = true;
            }
        } else {
            self.state.main_view.video_playing = !self.state.main_view.video_playing;
        }
        cx.notify();
        if self.state.video_player.is_none() && self.state.main_view.video_playing {
            self.start_video_playback_loop(cx);
        }
    }

    fn start_video_playback_loop(&mut self, cx: &mut Context<Self>) {
        let req_id = self.state.main_view.preview_request_id;
        let selected_path = self
            .state
            .main_view
            .selected_indices
            .iter()
            .next()
            .and_then(|&idx| self.state.main_view.items.get(idx))
            .map(|it| it.path.clone());
        let Some(video_path) = selected_path else {
            self.state.main_view.video_playing = false;
            return;
        };

        let mut async_cx = cx.to_async();
        let this_weak = cx.weak_entity();
        let bg_exec = cx.background_executor().clone();

        cx.foreground_executor()
            .spawn(async move {
                loop {
                    bg_exec.timer(std::time::Duration::from_millis(320)).await;
                    let mut should_continue = false;
                    let mut next_prog = 0.0f32;
                    let mut duration_secs = 120.0f64;
                    let mut base_poster = None;

                    let _ = this_weak.update(&mut async_cx, |this, cx| {
                        if !this.state.main_view.video_playing
                            || this.state.main_view.preview_request_id != req_id
                        {
                            this.state.main_view.video_playing = false;
                            return;
                        }
                        if let crate::ui::views::main_view::PreviewContent::Video(ref info) =
                            this.state.main_view.preview_content
                        {
                            duration_secs = info.duration_secs.unwrap_or(120.0).max(5.0);
                            base_poster = info.poster_path.clone();
                        } else {
                            this.state.main_view.video_playing = false;
                            return;
                        }
                        let step = (1.5 / duration_secs as f32).clamp(0.015, 0.08);
                        next_prog = this.state.main_view.video_progress + step;
                        if next_prog >= 1.0 {
                            next_prog = 0.0;
                        }
                        this.state.main_view.video_progress = next_prog;
                        should_continue = true;
                        cx.notify();
                    });

                    if !should_continue {
                        break;
                    }

                    let v_path = video_path.clone();
                    let target_ts = duration_secs * (next_prog as f64);
                    let frame_task = bg_exec.spawn(async move {
                        crate::storage::preview_service::extract_video_frame_at_timestamp(
                            &v_path,
                            target_ts,
                            duration_secs,
                            base_poster.as_deref(),
                        )
                    });
                    if let Some(new_frame) = frame_task.await {
                        let _ = this_weak.update(&mut async_cx, |this, cx| {
                            if this.state.main_view.preview_request_id == req_id {
                                if let crate::ui::views::main_view::PreviewContent::Video(
                                    ref mut info,
                                ) = this.state.main_view.preview_content
                                {
                                    info.poster_path = Some(new_frame);
                                    cx.notify();
                                }
                            }
                        });
                    }
                }
            })
            .detach();
    }

    pub fn set_video_progress(&mut self, progress: f32, cx: &mut Context<Self>) {
        let clamped = progress.clamp(0.0, 1.0);
        self.state.main_view.video_progress = clamped;
        if let Some(player) = &self.state.video_player {
            player.seek(clamped);
        }
        cx.notify();

        let req_id = self.state.main_view.preview_request_id;
        let selected_path = self
            .state
            .main_view
            .selected_indices
            .iter()
            .next()
            .and_then(|&idx| self.state.main_view.items.get(idx))
            .map(|it| it.path.clone());
        let (duration_secs, base_poster) =
            if let crate::ui::views::main_view::PreviewContent::Video(ref info) =
                self.state.main_view.preview_content
            {
                (
                    info.duration_secs.unwrap_or(120.0).max(1.0),
                    info.poster_path.clone(),
                )
            } else {
                return;
            };
        let Some(video_path) = selected_path else {
            return;
        };

        let target_ts = duration_secs * (clamped as f64);
        let mut async_cx = cx.to_async();
        let this_weak = cx.weak_entity();
        let bg = cx.background_executor().spawn(async move {
            crate::storage::preview_service::extract_video_frame_at_timestamp(
                &video_path,
                target_ts,
                duration_secs,
                base_poster.as_deref(),
            )
        });
        cx.foreground_executor()
            .spawn(async move {
                if let Some(frame_path) = bg.await {
                    this_weak
                        .update(&mut async_cx, |this, cx| {
                            if this.state.main_view.preview_request_id == req_id {
                                if let crate::ui::views::main_view::PreviewContent::Video(
                                    ref mut info,
                                ) = this.state.main_view.preview_content
                                {
                                    info.poster_path = Some(frame_path);
                                    cx.notify();
                                }
                            }
                        })
                        .ok();
                }
            })
            .detach();
    }

    pub fn step_video_time(&mut self, delta_secs: f64, cx: &mut Context<Self>) {
        let total = if let crate::ui::views::main_view::PreviewContent::Video(ref info) =
            self.state.main_view.preview_content
        {
            info.duration_secs.unwrap_or(120.0).max(5.0)
        } else {
            120.0
        };
        let delta_prog = (delta_secs / total) as f32;
        let next = (self.state.main_view.video_progress + delta_prog).clamp(0.0, 1.0);
        self.set_video_progress(next, cx);
    }

    pub fn toggle_video_mute(&mut self, cx: &mut Context<Self>) {
        self.state.main_view.video_muted = !self.state.main_view.video_muted;
        if let Some(player) = &self.state.video_player {
            player.set_muted(self.state.main_view.video_muted);
        }
        cx.notify();
    }

    /// Navigate to a specific 1-based page in the visual PDF previewer.
    pub fn pdf_select_page(&mut self, page_num: u32, cx: &mut Context<Self>) {
        let req_id = self.state.main_view.preview_request_id;
        let selected_path = self
            .state
            .main_view
            .selected_indices
            .iter()
            .next()
            .and_then(|&idx| self.state.main_view.items.get(idx))
            .map(|it| it.path.clone());
        let Some(path) = selected_path else {
            return;
        };
        let mut async_cx = cx.to_async();
        let this_weak = cx.weak_entity();
        let bg = cx.background_executor().spawn(async move {
            crate::storage::preview_service::extract_pdf_page_preview(&path, page_num)
        });
        cx.foreground_executor()
            .spawn(async move {
                if let Ok(info) = bg.await {
                    this_weak
                        .update(&mut async_cx, |this, cx| {
                            if this.state.main_view.preview_request_id == req_id {
                                this.state.main_view.preview_content =
                                    crate::ui::views::main_view::PreviewContent::Pdf(info);
                                cx.notify();
                            }
                        })
                        .ok();
                }
            })
            .detach();
    }

    pub fn pdf_prev_page(&mut self, cx: &mut Context<Self>) {
        if let crate::ui::views::main_view::PreviewContent::Pdf(ref info) =
            self.state.main_view.preview_content
        {
            if info.current_page > 1 {
                let target = info.current_page - 1;
                self.pdf_select_page(target, cx);
            }
        }
    }

    pub fn pdf_next_page(&mut self, cx: &mut Context<Self>) {
        if let crate::ui::views::main_view::PreviewContent::Pdf(ref info) =
            self.state.main_view.preview_content
        {
            if info.current_page < info.page_count {
                let target = info.current_page + 1;
                self.pdf_select_page(target, cx);
            }
        }
    }

    /// Switch the active sheet tab (`.xlsx`/`.ods`), slide (`.pptx`/`.odp`), or page (`.docx`/`.odt`/`.epub`).
    pub fn office_select_index(&mut self, target_idx: usize, cx: &mut Context<Self>) {
        let req_id = self.state.main_view.preview_request_id;
        let selected_path = self
            .state
            .main_view
            .selected_indices
            .iter()
            .next()
            .and_then(|&idx| self.state.main_view.items.get(idx))
            .map(|it| it.path.clone());
        let Some(path) = selected_path else {
            return;
        };
        let (_, ext) = crate::storage::preview_service::detect_mime_and_extension(&path);
        let mut async_cx = cx.to_async();
        let this_weak = cx.weak_entity();
        let bg = cx.background_executor().spawn(async move {
            crate::storage::preview_service::extract_office_visual_preview(&path, &ext, target_idx)
        });
        cx.foreground_executor()
            .spawn(async move {
                if let Ok(info) = bg.await {
                    this_weak
                        .update(&mut async_cx, |this, cx| {
                            if this.state.main_view.preview_request_id == req_id {
                                this.state.main_view.preview_content =
                                    crate::ui::views::main_view::PreviewContent::OfficeVisual(info);
                                cx.notify();
                            }
                        })
                        .ok();
                }
            })
            .detach();
    }
}


/// Recursively copy a directory (SendTo folder targets, folder-with-selection
/// support). Best-effort: ignores individual failures.
fn copy_dir_recursive(src: &std::path::Path, dest: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dest)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let from = entry.path();
        let to = dest.join(entry.file_name());
        if from.is_dir() {
            copy_dir_recursive(&from, &to)?;
        } else {
            std::fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

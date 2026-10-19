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
        let current = &self.state.main_view.group_by;
        let next = match current {
            GroupColumn::None => GroupColumn::Name,
            GroupColumn::Name => GroupColumn::FileType,
            GroupColumn::FileType => GroupColumn::Size,
            GroupColumn::Size => GroupColumn::SyncStatus,
            GroupColumn::SyncStatus => GroupColumn::DateModified,
            GroupColumn::DateModified => GroupColumn::DateCreated,
            GroupColumn::DateCreated => GroupColumn::Tag,
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

    /// Copy the first selected item's full path as text (Files parity:
    /// "Copy item path", Ctrl+Shift+C). Falls back to the current folder.
    pub fn copy_item_path(&mut self, cx: &mut Context<Self>) {
        let path_str = self
            .state
            .main_view
            .selected_indices
            .iter()
            .next()
            .and_then(|&idx| self.state.main_view.items.get(idx))
            .map(|item| item.path.as_path().to_string_lossy().to_string())
            .unwrap_or_else(|| {
                self.state
                    .main_view
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
                use crate::core::progress::OpKind;
                use crate::core::transfer::{TransferOutcome, run_copy_job, run_move_job};
                let kind = if is_cut { OpKind::Move } else { OpKind::Copy };
                let job_id = progress_engine
                    .start_job(format!("{} ({} items)", op_type, total), kind)
                    .await;
                let sources: Vec<std::path::PathBuf> = clipboard_items
                    .iter()
                    .map(|item| item.path.as_path().to_path_buf())
                    .collect();
                let outcome = if is_cut {
                    run_move_job(&progress_engine, job_id, sources, target_path).await
                } else {
                    run_copy_job(&progress_engine, job_id, sources, target_path).await
                };
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
                        this.refresh(cx);
                    })
                    .ok();
            })
            .detach();
    }

    // === Preview loading ===

    pub(crate) fn load_preview_content(&mut self, path: FilesPath, cx: &mut Context<Self>) {
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
                                    // Safe UTF-8 truncation: find the char boundary at or before byte 10000
                                    let truncate_at = text
                                        .char_indices()
                                        .nth(10000)
                                        .map(|(i, _)| i)
                                        .unwrap_or(text.len());
                                    format!(
                                        "{}\n\n[Preview truncated at 10,000 characters]",
                                        &text[..truncate_at]
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
                "transparent_window" => {
                    s.config.appearance.transparent_window = !s.config.appearance.transparent_window
                }
                "folder_on_top" => s.config.view.folder_on_top = !s.config.view.folder_on_top,
                "mru_tab_cycle" => s.config.session.mru_tab_cycle = !s.config.session.mru_tab_cycle,
                _ => {}
            }
            let _ = s.config.save();
        });
        cx.notify();
    }
    pub(crate) fn load_folder_preview(&mut self, path: FilesPath, cx: &mut Context<Self>) {
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
                    cx.background_executor()
                        .spawn(async move {
                            let service =
                                crate::platform::windows::share_service::ShareService::new();
                            let _ = service.share_file(0, &share_path).await;
                        })
                        .detach();
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
                    .start_job(format!("Bulk Rename ({} files)", total), crate::core::progress::OpKind::Other)
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
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Clamp the menu inside the viewport (flip-up when near the bottom
        // edge) so tall menus never spill past the window like before.
        let vp = window.viewport_size();
        let est_h: f32 = if item_index.is_some() { 620.0 } else { 380.0 };
        let mut x: f32 = position.x.into();
        let mut y: f32 = position.y.into();
        if x + 300.0 > vp.width.into() {
            x = (f32::from(vp.width) - 300.0).max(8.0);
        }
        if y + est_h > vp.height.into() {
            y = (f32::from(vp.height) - est_h).max(8.0);
        }
        let position = Point { x: px(x), y: px(y) };
        // Flyouts open left when there is no room on the right.
        let vpw: f32 = vp.width.into();
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
        // shows the preview popup above an open context menu).
        self.hover_tooltip = None;
        self.hover_item_index = None;
        self.hover_timer_task = None;

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

        let menu_style = self
            .state
            .settings
            .read(cx)
            .config
            .preferences
            .context_menu_style;
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
        let expanded = menu_style == crate::config::config::ContextMenuStyle::Expanded;

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
        // Any submenu hover (re)entry invalidates a pending grace-close.
        self.submenu_grace_cancel();
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
                    .timer(std::time::Duration::from_millis(350))
                    .await;
                weak
                    .update(&mut async_app, |this, cx| {
                        if this.submenu_grace_gen == grace {
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
            VM::List => VM::Grid,
            VM::Grid => VM::Columns,
            VM::Columns => VM::Details,
            _ => VM::Details,
        };
        self.set_view_mode(next, cx);
    }

    /// Cycle Name -> DateModified -> Size -> Type (toolbar Sort button).
    /// Same column twice flips direction via toggle_sort.
    pub fn cycle_sort_column(&mut self, cx: &mut Context<Self>) {
        use crate::config::config::SortBy as SB;
        let next = match self.state.main_view.sort.column {
            SB::Name => SB::DateModified,
            SB::DateModified => SB::Size,
            SB::Size => SB::Type,
            _ => SB::Name,
        };
        self.state.main_view.toggle_sort(next);
        self.state.main_view.bump_items_revision();
        cx.notify();
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

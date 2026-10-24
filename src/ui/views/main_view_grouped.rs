// Grouped view methods for MainViewState and MainView (`Files-community` Grouped CollectionView parity)

use crate::config::config::GroupColumn;
use crate::core::navigation_manager::PaneFocus;
use crate::domain::metadata::FileItem;
use crate::storage::storage::FileType;
use crate::ui::constants::UiConstants;
use gpui::*;
use std::collections::BTreeMap;

use super::helpers::natural_cmp;
use super::main_view::MainView;
use super::types::*;

impl MainViewState {
    /// Get both the logical sort rank (`i64`) and group key (`String`) for `item`
    /// based on `self.group_by`. Preserves chronological order for Date groups and
    /// ascending byte-bucket order for Size groups instead of scrambling alphabetically.
    pub fn get_group_key_and_rank(&self, item: &FileItem) -> Option<(i64, String)> {
        match self.group_by {
            GroupColumn::None => None,
            GroupColumn::Name => {
                let first = item.name.chars().next().unwrap_or('#').to_ascii_uppercase();
                if first.is_ascii_alphabetic() {
                    let rank = (first as i64) - ('A' as i64) + 1;
                    Some((rank, first.to_string()))
                } else {
                    Some((0, "#".to_string()))
                }
            }
            GroupColumn::DateModified => Some(self.get_date_group_rank_and_key(item.modified)),
            GroupColumn::DateCreated => {
                Some(self.get_date_group_rank_and_key(item.created.or(item.modified)))
            }
            GroupColumn::Size => {
                if matches!(item.file_type, FileType::Directory) {
                    Some((0, "Folders".to_string()))
                } else {
                    Some(self.get_size_bucket_rank_and_key(item.size))
                }
            }
            GroupColumn::FileType => Some(self.get_file_type_category_rank_and_key(item)),
            GroupColumn::SyncStatus => {
                if let Some(status) = self.git_status_cache.get(&item.path) {
                    use crate::domain::git_service::GitFileStatus;
                    match status {
                        GitFileStatus::Modified | GitFileStatus::Conflicted => {
                            Some((0, "Modified (Git)".to_string()))
                        }
                        GitFileStatus::Staged | GitFileStatus::Added => {
                            Some((1, "Staged (Git)".to_string()))
                        }
                        GitFileStatus::Untracked => Some((2, "Untracked".to_string())),
                        _ => Some((3, "Synced / Clean".to_string())),
                    }
                } else {
                    Some((3, "Local / Synced".to_string()))
                }
            }
            GroupColumn::Tag => {
                let path_str = item.path.to_string_lossy().to_string();
                let tags = self.tags_cache.get(&path_str);
                match tags.and_then(|t| t.first()) {
                    Some(tag) => Some((0, tag.clone())),
                    None => Some((999, "Untagged".to_string())),
                }
            }
        }
    }

    /// Get the group key for an item based on the current `group_by` setting.
    pub fn get_group_key(&self, item: &FileItem) -> Option<String> {
        self.get_group_key_and_rank(item).map(|(_, k)| k)
    }

    /// Get the date group key for a specific item based on its modification time.
    pub fn get_date_group_key_for_item(&self, item: &FileItem) -> String {
        self.get_date_group_rank_and_key(item.modified).1
    }

    fn get_date_group_rank_and_key(
        &self,
        timestamp: Option<std::time::SystemTime>,
    ) -> (i64, String) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let day_secs: u64 = 86_400;

        let item_secs = timestamp
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);

        if item_secs == 0 {
            return (8, "Unknown".to_string());
        }

        let age_days = now.saturating_sub(item_secs) / day_secs;
        if age_days == 0 {
            (0, "Today".to_string())
        } else if age_days == 1 {
            (1, "Yesterday".to_string())
        } else if age_days < 7 {
            (2, "Earlier This Week".to_string())
        } else if age_days < 14 {
            (3, "Last Week".to_string())
        } else if age_days < 31 {
            (4, "Earlier This Month".to_string())
        } else if age_days < 62 {
            (5, "Last Month".to_string())
        } else if age_days < 365 {
            (6, "Earlier This Year".to_string())
        } else {
            (7, "Older".to_string())
        }
    }

    fn get_size_bucket_rank_and_key(&self, size: u64) -> (i64, String) {
        const KB: u64 = 1024;
        const MB: u64 = KB * 1024;
        const GB: u64 = MB * 1024;
        if size == 0 {
            (1, "Empty (0 B)".to_string())
        } else if size < 16 * KB {
            (2, "Tiny (< 16 KB)".to_string())
        } else if size < MB {
            (3, "Small (16 KB – 1 MB)".to_string())
        } else if size < 128 * MB {
            (4, "Medium (1 MB – 128 MB)".to_string())
        } else if size < GB {
            (5, "Large (128 MB – 1 GB)".to_string())
        } else {
            (6, "Huge (> 1 GB)".to_string())
        }
    }

    fn get_file_type_category_rank_and_key(&self, item: &FileItem) -> (i64, String) {
        if matches!(item.file_type, FileType::Directory) {
            return (0, "Folders".to_string());
        }
        let ext = item
            .name
            .rsplit_once('.')
            .map(|(_, e)| e.to_ascii_lowercase())
            .unwrap_or_default();
        match ext.as_str() {
            "pdf" | "doc" | "docx" | "odt" | "rtf" | "txt" | "md" | "markdown" | "epub" => {
                (1, "Documents".to_string())
            }
            "xls" | "xlsx" | "xlsm" | "csv" | "tsv" | "ods" => (2, "Spreadsheets".to_string()),
            "ppt" | "pptx" | "odp" => (3, "Presentations".to_string()),
            "jpg" | "jpeg" | "png" | "gif" | "bmp" | "webp" | "ico" | "tiff" | "tif" | "svg"
            | "avif" | "heic" | "heif" | "raw" | "psd" | "tga" | "hdr" => {
                (4, "Images & Graphics".to_string())
            }
            "mp4" | "mkv" | "avi" | "mov" | "wmv" | "webm" | "m4v" | "flv" | "ts" => {
                (5, "Videos".to_string())
            }
            "mp3" | "wav" | "flac" | "ogg" | "m4a" | "aac" | "opus" | "wma" => {
                (6, "Audio & Music".to_string())
            }
            "zip" | "rar" | "7z" | "tar" | "gz" | "bz2" | "xz" | "tgz" | "zst" | "iso" => {
                (7, "Archives & Packages".to_string())
            }
            "rs" | "py" | "js" | "ts" | "jsx" | "tsx" | "c" | "cpp" | "h" | "hpp" | "cs" | "go"
            | "java" | "kt" | "swift" | "rb" | "php" | "sh" | "ps1" | "sql" | "html" | "css"
            | "json" | "toml" | "yaml" | "yml" | "xml" => (8, "Source Code & Config".to_string()),
            "exe" | "msi" | "dll" | "sys" | "so" | "dylib" | "wasm" | "bat" | "cmd" => {
                (9, "Applications & Binaries".to_string())
            }
            "" => (10, "Files".to_string()),
            other => (10, format!("{} Files", other.to_uppercase())),
        }
    }

    /// Toggle a group's collapsed state.
    pub fn toggle_group(&mut self, key: String) {
        if self.collapsed_groups.contains(&key) {
            self.collapsed_groups.remove(&key);
        } else {
            self.collapsed_groups.insert(key);
        }
    }

    /// Partition visible indices into logically ordered groups.
    /// Returns `(group_headers, group_contents)` where `group_headers[i] = (group_key, group_label, item_count)`.
    pub fn partition_groups(
        &self,
        visible: &[usize],
    ) -> (Vec<(String, String, usize)>, Vec<Vec<usize>>) {
        if self.group_by == GroupColumn::None {
            return (vec![], vec![]);
        }

        // Keyed by `(rank, group_key)` so Date/Size/Type groups preserve their natural bucket order!
        let mut groups: BTreeMap<(i64, String), Vec<usize>> = BTreeMap::new();
        for &idx in visible {
            if let Some(item) = self.items.get(idx) {
                if let Some((rank, key)) = self.get_group_key_and_rank(item) {
                    groups.entry((rank, key)).or_default().push(idx);
                }
            }
        }

        let mut ordered: Vec<((i64, String), Vec<usize>)> = groups.into_iter().collect();
        ordered.sort_by(|a, b| {
            let cmp = a.0 .0.cmp(&b.0 .0).then_with(|| natural_cmp(&a.0 .1, &b.0 .1));
            if self.group_ascending {
                cmp
            } else {
                cmp.reverse()
            }
        });

        let mut headers = Vec::with_capacity(ordered.len());
        let mut contents = Vec::with_capacity(ordered.len());
        for ((_rank, key), indices) in ordered {
            let count = indices.len();
            let label = key.clone();
            headers.push((key, label, count));
            contents.push(indices);
        }

        (headers, contents)
    }

    pub fn start_group_drag(&mut self, item_index: usize) -> Option<String> {
        if self.group_by == GroupColumn::None {
            return None;
        }
        let item = self.items.get(item_index)?;
        let source_key = self.get_group_key(item)?;
        self.group_drag_state.dragged_index = Some(item_index);
        self.group_drag_state.source_group_key = Some(source_key.clone());
        self.group_drag_state.target_group_key = None;
        self.group_drag_state.insert_position = None;
        self.group_drag_state.is_dragging = true;
        Some(source_key)
    }

    pub fn update_group_drag(
        &mut self,
        target_group_key: Option<String>,
        insert_position: Option<usize>,
    ) {
        if !self.group_drag_state.is_dragging {
            return;
        }
        self.group_drag_state.target_group_key = target_group_key;
        self.group_drag_state.insert_position = insert_position;
    }

    pub fn end_group_drag(&mut self) -> bool {
        if !self.group_drag_state.is_dragging {
            return false;
        }
        let dragged_idx = self.group_drag_state.dragged_index;
        let target_key = self.group_drag_state.target_group_key.take();
        let insert_pos = self.group_drag_state.insert_position;
        self.group_drag_state = GroupDragState::default();

        let Some(dragged_idx) = dragged_idx else {
            return false;
        };
        let Some(target_key) = target_key else {
            return false;
        };
        let source_key = self.items.get(dragged_idx).and_then(|it| self.get_group_key(it));
        if source_key.as_ref() == Some(&target_key) && insert_pos.is_none() {
            return false;
        }
        true
    }

    pub fn cancel_group_drag(&mut self) {
        self.group_drag_state = GroupDragState::default();
    }

    pub fn is_group_dragging(&self) -> bool {
        self.group_drag_state.is_dragging
    }

    pub fn get_group_drag_target(&self) -> Option<(String, Option<usize>)> {
        if !self.group_drag_state.is_dragging {
            return None;
        }
        self.group_drag_state
            .target_group_key
            .clone()
            .map(|key| (key, self.group_drag_state.insert_position))
    }

    /// Flatten grouped items into a list of `GroupedItemType::Header` and `GroupedItemType::Item`.
    pub fn flatten_grouped_items(&self, visible: &[usize]) -> Vec<GroupedItemType> {
        let (headers, groups) = self.partition_groups(visible);
        let mut flat = Vec::new();
        for (i, (key, label, count)) in headers.into_iter().enumerate() {
            let is_collapsed = self.collapsed_groups.contains(&key);
            flat.push(GroupedItemType::Header { key, label, count });
            if !is_collapsed {
                for &idx in &groups[i] {
                    flat.push(GroupedItemType::Item(idx));
                }
            }
        }
        flat
    }

    /// Flatten grouped grid/multi-column items into a list of `(Header | Row(Vec<usize>))`.
    pub fn flatten_grouped_grid_items(
        &self,
        visible: &[usize],
        items_per_row: usize,
    ) -> Vec<GroupedGridItem> {
        let step = items_per_row.max(1);
        let (headers, groups) = self.partition_groups(visible);
        let mut flat = Vec::new();
        for (i, (key, label, count)) in headers.into_iter().enumerate() {
            let is_collapsed = self.collapsed_groups.contains(&key);
            flat.push(GroupedGridItem::Header { key, label, count });
            if !is_collapsed {
                let group_indices = &groups[i];
                for row_start in (0..group_indices.len()).step_by(step) {
                    let row_end = (row_start + step).min(group_indices.len());
                    flat.push(GroupedGridItem::Row(
                        group_indices[row_start..row_end].to_vec(),
                    ));
                }
            }
        }
        flat
    }

    pub fn reorder_item_within_group(
        &mut self,
        source_visual_index: usize,
        target_visual_index: usize,
    ) {
        if source_visual_index == target_visual_index {
            return;
        }
        let visible = self.visible_items_rc();
        let flattened = self.flatten_grouped_items(&visible);
        let item_indices: Vec<usize> = flattened
            .iter()
            .filter_map(|item| {
                if let GroupedItemType::Item(idx) = item {
                    Some(*idx)
                } else {
                    None
                }
            })
            .collect();

        if source_visual_index >= item_indices.len() || target_visual_index >= item_indices.len() {
            return;
        }
        let source_idx = item_indices[source_visual_index];
        let target_idx = item_indices[target_visual_index];
        if source_idx < self.items.len() && target_idx < self.items.len() {
            let item = self.items.remove(source_idx);
            self.items.insert(target_idx, item);
            let mut new_selected = std::collections::HashSet::new();
            for &old_idx in &self.selected_indices {
                let new_idx = if old_idx == source_idx {
                    target_idx
                } else if source_idx < target_idx && old_idx > source_idx && old_idx <= target_idx {
                    old_idx - 1
                } else if source_idx > target_idx && old_idx >= target_idx && old_idx < source_idx {
                    old_idx + 1
                } else {
                    old_idx
                };
                new_selected.insert(new_idx);
            }
            self.selected_indices = new_selected;
            self.bump_items_revision();
        }
    }
}

impl MainView {
    /// Unified `Files-community` Group Header Row used across `Details`, `List`, `Cards`, and `Grid` views.
    /// Features:
    /// - Clickable chevron + group title to collapse/expand the group
    /// - Item count badge
    /// - Full-width subtle horizontal rule
    /// - `"Select"` action pill on the right to select all items in this group at once
    pub fn render_group_header_row(
        id_num: usize,
        key: &str,
        label: &str,
        count: usize,
        is_collapsed: bool,
        row_height: f32,
        horizontal_pad: f32,
        pane_focus: PaneFocus,
        view: Entity<crate::app::AppWindow>,
    ) -> AnyElement {
        let key_toggle = key.to_string();
        let key_select = key.to_string();
        let label_str = label.to_string();
        let v_toggle = view.clone();
        let v_select = view;

        div()
            .id(ElementId::from(id_num))
            .w_full()
            .h(px(row_height.max(28.0)))
            .px(px(horizontal_pad))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.0))
            .bg(rgba(0xFFFFFF04u32))
            .hover(|s| s.bg(rgba(0xFFFFFF0Au32)))
            .cursor_pointer()
            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                cx.stop_propagation();
                v_toggle.update(cx, |this, cx| {
                    this.focus_pane(pane_focus, cx);
                    this.toggle_group_collapse(key_toggle.clone(), cx);
                });
            })
            .child(
                div()
                    .w(px(18.0))
                    .h(px(18.0))
                    .rounded(px(4.0))
                    .bg(rgba(0xFFFFFF0Cu32))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .text_size(px(10.0))
                            .font_family(UiConstants::FONT_MDL2)
                            .text_color(rgb(0x60CDFF))
                            .child(if is_collapsed { "\u{E76C}" } else { "\u{E70D}" }),
                    ),
            )
            .child(
                div()
                    .text_size(px(12.5))
                    .font_weight(FontWeight(600.0))
                    .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                    .child(label_str),
            )
            .child(
                div()
                    .px(px(7.0))
                    .py(px(1.5))
                    .rounded(px(10.0))
                    .bg(rgba(0x60CDFF1Eu32))
                    .border(px(1.0))
                    .border_color(rgba(0x60CDFF44u32))
                    .text_size(px(10.5))
                    .font_weight(FontWeight(600.0))
                    .text_color(rgb(0x60CDFF))
                    .child(format!("{}", count)),
            )
            .child(
                div()
                    .flex_1()
                    .h(px(1.0))
                    .mx(px(4.0))
                    .bg(rgba(UiConstants::DIVIDER_RGBA)),
            )
            .child(
                div()
                    .px(px(8.0))
                    .py(px(2.0))
                    .rounded(px(4.0))
                    .bg(rgba(0xFFFFFF08u32))
                    .hover(|s| s.bg(rgba(0x60CDFF33u32)))
                    .text_size(px(10.5))
                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                    .on_mouse_down(MouseButton::Left, move |ev, _, cx| {
                        cx.stop_propagation();
                        let additive = ev.modifiers.control || ev.modifiers.shift;
                        v_select.update(cx, |this, cx| {
                            this.focus_pane(pane_focus, cx);
                            this.select_all_in_group(&key_select, additive, cx);
                        });
                    })
                    .child("Select group"),
            )
            .into_any_element()
    }
}

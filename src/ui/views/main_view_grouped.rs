// Grouped view methods for MainViewState and MainView (`Files-community` Grouped CollectionView parity)

use crate::config::config::{GroupColumn, ViewMode};
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
                let tag_opt = self.item_tags(item).and_then(|t| t.first());
                match tag_opt {
                    Some(tag) if !tag.is_empty() => Some((0, tag.clone())),
                    _ => Some((999, "Untagged".to_string())),
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

    /// Return all visible item indices in their exact visual display order
    /// (respecting `group_by` bucket ordering and excluding items inside collapsed groups).
    pub fn visual_order_items(&self) -> Vec<usize> {
        let visible = self.visible_items_rc();
        if self.group_by == GroupColumn::None {
            return visible.as_ref().clone();
        }
        self.flatten_grouped_items(&visible)
            .into_iter()
            .filter_map(|entry| match entry {
                GroupedItemType::Item(idx) => Some(idx),
                GroupedItemType::Header { .. } => None,
            })
            .collect()
    }

    /// Return visual 2D rows of item indices for keyboard navigation (`Up`/`Down`/`PageUp`/`PageDown`),
    /// accounting for view mode column counts, group row breaks, and collapsed groups.
    pub fn visual_rows(&self, pane_w: f32) -> Vec<Vec<usize>> {
        let visible = self.visible_items_rc();
        let cols = match self.view_mode {
            ViewMode::Grid => Self::grid_columns_for_width(pane_w, self.zoom_level).max(1),
            ViewMode::Cards => {
                Self::cards_layout_metrics(pane_w, self.zoom_level, self.compact_mode)
                    .0
                    .max(1)
            }
            ViewMode::List => {
                Self::list_layout_metrics(pane_w, self.zoom_level, self.compact_mode)
                    .0
                    .max(1)
            }
            ViewMode::Details | ViewMode::Columns | ViewMode::Tree => 1,
        };

        if self.group_by == GroupColumn::None {
            return visible.chunks(cols).map(|c| c.to_vec()).collect();
        }

        self.flatten_grouped_grid_items(&visible, cols)
            .into_iter()
            .filter_map(|entry| match entry {
                GroupedGridItem::Row(row) if !row.is_empty() => Some(row),
                _ => None,
            })
            .collect()
    }

    /// Compute exact `(item_index, (x0, y0, x1, y1))` bounds in scroll-content coordinates
    /// for all non-collapsed items when `group_by != GroupColumn::None`.
    /// Used by both rubber-band lasso selection (`apply_rect_selection_for_view`) and
    /// `scroll_item_into_view`.
    pub fn grouped_item_bounds_for_view(&self, pane_w: f32) -> Vec<(usize, (f32, f32, f32, f32))> {
        let visible = self.visible_items_rc();
        let zoom = self.zoom_level;
        let compact = self.compact_mode;
        let mut bounds = Vec::with_capacity(visible.len());

        match self.view_mode {
            ViewMode::Grid => {
                let cols = Self::grid_columns_for_width(pane_w, zoom);
                let params = Self::grid_layout_params_for_width_pub(pane_w, zoom, cols);
                let header_h = if compact { 28.0 } else { 32.0 };
                let flattened =
                    self.flatten_grouped_grid_items(&visible, params.items_per_row.max(1));
                let mut y = 0.0_f32;
                for entry in flattened {
                    match entry {
                        GroupedGridItem::Header { .. } => {
                            y += header_h;
                        }
                        GroupedGridItem::Row(indices) => {
                            for (col, &idx) in indices.iter().enumerate() {
                                let x0 = params.padding
                                    + col as f32 * (params.item_width + params.gap);
                                let y0 = y;
                                let x1 = x0 + params.item_width;
                                let y1 = y0 + params.item_height;
                                bounds.push((idx, (x0, y0, x1, y1)));
                            }
                            y += params.row_height;
                        }
                    }
                }
            }
            ViewMode::Cards => {
                let (cols, card_w, row_h, gap, pad) =
                    Self::cards_layout_metrics(pane_w, zoom, compact);
                let header_h = if compact { 28.0 } else { 32.0 };
                let flattened = self.flatten_grouped_grid_items(&visible, cols.max(1));
                let mut y = 0.0_f32;
                for entry in flattened {
                    match entry {
                        GroupedGridItem::Header { .. } => {
                            y += header_h;
                        }
                        GroupedGridItem::Row(indices) => {
                            for (col, &idx) in indices.iter().enumerate() {
                                let x0 = pad + col as f32 * (card_w + gap);
                                let y0 = y;
                                let x1 = x0 + card_w;
                                let y1 = y0 + row_h - 4.0;
                                bounds.push((idx, (x0, y0, x1, y1)));
                            }
                            y += row_h;
                        }
                    }
                }
            }
            ViewMode::List => {
                let (cols, col_w, row_h, gap, pad) =
                    Self::list_layout_metrics(pane_w, zoom, compact);
                let header_h = row_h.max(30.0);
                let flattened = self.flatten_grouped_grid_items(&visible, cols.max(1));
                let mut y = 0.0_f32;
                for entry in flattened {
                    match entry {
                        GroupedGridItem::Header { .. } => {
                            y += header_h;
                        }
                        GroupedGridItem::Row(indices) => {
                            for (col, &idx) in indices.iter().enumerate() {
                                let x0 = pad + col as f32 * (col_w + gap);
                                let y0 = y;
                                let x1 = x0 + col_w;
                                let y1 = y0 + row_h - 2.0;
                                bounds.push((idx, (x0, y0, x1, y1)));
                            }
                            y += row_h;
                        }
                    }
                }
            }
            ViewMode::Details | ViewMode::Tree => {
                let base_row_h = if compact { 22.0 } else { 26.0 };
                let row_h = (base_row_h + zoom * 22.0).round();
                let header_h = row_h.max(30.0);
                let flattened = self.flatten_grouped_items(&visible);
                let mut y = 0.0_f32;
                for entry in flattened {
                    match entry {
                        GroupedItemType::Header { .. } => {
                            y += header_h;
                        }
                        GroupedItemType::Item(idx) => {
                            bounds.push((idx, (8.0, y, (pane_w - 8.0).max(80.0), y + row_h)));
                            y += row_h;
                        }
                    }
                }
            }
            ViewMode::Columns => {
                let base_row_h = if compact { 20.0 } else { 22.0 };
                let row_h = (base_row_h + zoom * 22.0).round();
                let header_h = row_h.max(30.0);
                let zoom_scale = 0.80 + zoom.clamp(0.0, 1.0) * 0.55;
                let col2_w = (self.column_widths.miller_col2 * zoom_scale)
                    .clamp(160.0, (pane_w * 0.55).max(240.0));
                let flattened = self.flatten_grouped_items(&visible);
                let mut y = 0.0_f32;
                for entry in flattened {
                    match entry {
                        GroupedItemType::Header { .. } => {
                            y += header_h;
                        }
                        GroupedItemType::Item(idx) => {
                            bounds.push((idx, (4.0, y, (col2_w - 4.0).max(80.0), y + row_h)));
                            y += row_h;
                        }
                    }
                }
            }
        }

        bounds
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
    /// Variable-height virtualized vertical scroll container for grouped views
    /// (`Grid`, `Cards`, `List`, `Details`).
    ///
    /// Replaces GPUI's `uniform_list` when `group_by != GroupColumn::None` because
    /// `uniform_list` measures only item `0` (which in grouped views is always a `28px..32px`
    /// `Header`) and forces every subsequent grid/card/list row to that `32px` height—causing
    /// grid tiles and cards to stack `32px` apart vertically and overlap across the viewport.
    pub fn render_virtualized_grouped_list<F>(
        id: impl Into<ElementId>,
        scroll_handle: &UniformListScrollHandle,
        pane_w: f32,
        item_heights: &[f32],
        mut render_item: F,
    ) -> AnyElement
    where
        F: FnMut(usize) -> AnyElement,
    {
        let count = item_heights.len();
        let mut y_offsets = Vec::with_capacity(count + 1);
        let mut total_h = 0.0_f32;
        for &h in item_heights {
            y_offsets.push(total_h);
            total_h += h.max(1.0);
        }
        y_offsets.push(total_h);

        let (scroll_top, vp_h, base_handle) = {
            let mut st = scroll_handle.0.borrow_mut();
            let base = st.base_handle.clone();
            let raw_off_y: f32 = base.offset().y.into();
            let b_h: f32 = base.bounds().size.height.into();
            let prev_vp_h: f32 = st
                .last_item_size
                .map(|s| s.item.height.into())
                .unwrap_or(0.0);
            let effective_vp_h = if b_h > 20.0 {
                b_h
            } else if prev_vp_h > 20.0 {
                prev_vp_h
            } else {
                720.0
            };
            let max_scroll = (total_h - effective_vp_h).max(0.0);
            let clamped_scroll_top = (-raw_off_y).clamp(0.0, max_scroll);
            if (-raw_off_y) > max_scroll + 1.0 {
                base.set_offset(point(px(0.0), px(-clamped_scroll_top)));
            }
            st.last_item_size = Some(ItemSize {
                item: size(px(pane_w.max(100.0)), px(effective_vp_h)),
                contents: size(px(pane_w.max(100.0)), px(total_h)),
            });
            (clamped_scroll_top, effective_vp_h, base)
        };

        let overscan_px = (vp_h * 0.65).max(320.0);
        let vis_top = (scroll_top - overscan_px).max(0.0);
        let vis_bottom = (scroll_top + vp_h + overscan_px).min(total_h);

        let first_idx = if count == 0 {
            0
        } else {
            match y_offsets[..count].binary_search_by(|y| {
                y.partial_cmp(&vis_top).unwrap_or(std::cmp::Ordering::Equal)
            }) {
                Ok(i) => i,
                Err(i) => i.saturating_sub(1),
            }
        };

        let last_idx = if count == 0 {
            0
        } else {
            match y_offsets[..count].binary_search_by(|y| {
                y.partial_cmp(&vis_bottom).unwrap_or(std::cmp::Ordering::Equal)
            }) {
                Ok(i) => (i + 1).min(count),
                Err(i) => i.min(count),
            }
        };

        let top_spacer_h = y_offsets.get(first_idx).copied().unwrap_or(0.0);
        let rendered_end_y = y_offsets.get(last_idx).copied().unwrap_or(total_h);
        let bottom_spacer_h = (total_h - rendered_end_y).max(0.0);

        let mut children: Vec<AnyElement> =
            Vec::with_capacity((last_idx.saturating_sub(first_idx)) + 2);

        if top_spacer_h > 0.0 {
            children.push(
                div()
                    .w_full()
                    .h(px(top_spacer_h))
                    .flex_shrink_0()
                    .into_any_element(),
            );
        }

        for i in first_idx..last_idx {
            let h = item_heights[i].max(1.0);
            children.push(
                div()
                    .w_full()
                    .h(px(h))
                    .flex_shrink_0()
                    .child(render_item(i))
                    .into_any_element(),
            );
        }

        if bottom_spacer_h > 0.0 {
            children.push(
                div()
                    .w_full()
                    .h(px(bottom_spacer_h))
                    .flex_shrink_0()
                    .into_any_element(),
            );
        }

        div()
            .id(id)
            .w_full()
            .h_full()
            .flex()
            .flex_col()
            .overflow_y_scroll()
            .track_scroll(&base_handle)
            .children(children)
            .into_any_element()
    }

    /// Unified `Files-community` Group Header Row used across `Details`, `List`, `Cards`, and `Grid` views.
    /// Features:
    /// - Clickable chevron + group title to collapse/expand the group
    /// - Item count badge
    /// - Full-width subtle horizontal rule
    /// - `"Select group"` action pill on the right to select all items in this group at once
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
        let elem_id = id_num
            + match pane_focus {
                PaneFocus::Primary => 900_000,
                PaneFocus::Secondary => 950_000,
            };

        div()
            .id(ElementId::from(elem_id))
            .w_full()
            .h(px(row_height.max(28.0)))
            .flex_shrink_0()
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
                    .flex_shrink_0()
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
                    .flex_shrink_0()
                    .text_size(px(12.5))
                    .font_weight(FontWeight(600.0))
                    .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                    .child(label_str),
            )
            .child(
                div()
                    .flex_shrink_0()
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
                    .flex_shrink_0()
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

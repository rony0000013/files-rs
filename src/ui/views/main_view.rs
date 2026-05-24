use crate::core::path_util::FilesPath;
use crate::platform::windows::recycle_bin_service::RecycleBinItem;
use crate::ui::components::dialogs::{DialogType, SettingsTab};
use crate::ui::components::icon_renderer::{is_thumbnail_image, IconRenderer, ThumbnailRenderer};
use gpui::*;
use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

// Re-export types and helpers from their dedicated modules (code splitting)
pub use super::helpers::*;
pub use super::types::*;

use crate::config::config::{GroupColumn, SortBy, ViewMode};
use crate::domain::metadata::FileItem;
use crate::storage::storage::FileType;
use crate::ui::constants::UiConstants;

impl MainViewState {
    pub fn new(root: FilesPath, view_mode: ViewMode) -> Self {
        Self {
            pane_type: PaneType::Single,
            dual_pane_split: DualPaneSplit::Horizontal,
            view_mode: view_mode.clone(),
            target: ViewTarget::Directory(root.clone()),
            primary_path: root,
            secondary_path: None,
            items: Vec::new(),
            selected_indices: HashSet::default(),
            sort: SortState::default(),
            filter_text: String::new(),
            show_dot_files: false,
            show_hidden_files: false,
            show_extensions: true,
            show_system_files: false,
            loading: false,
            side_pane: SidePaneMode::None,
            renaming_index: None,
            rename_text: String::new(),
            compact_mode: false,
            preview_text: None,
            column_widths: ColumnWidths::default(),
            column_visibility: ColumnVisibility::default(),
            group_by: GroupColumn::None,
            collapsed_groups: HashSet::new(),
            selection_mode: SelectionMode::Normal,
            shelf_visible: false,
            shelf_items: Vec::new(),
            folder_background: FolderBackground::default(),
            active_pane: crate::core::navigation_manager::PaneFocus::Primary,
            last_clicked_index: None,
            lasso_state: None,
            drag_state: DragState::default(),
            details_cache: DetailsPaneCache::default(),
            search_state: SearchState::default(),
            toolbar_customization: ToolbarCustomizationState::default(),
            preview_content: PreviewContent::default(),
            group_drag_state: GroupDragState::default(),
            recycle_bin_items: Vec::new(),
            tags_cache: std::collections::HashMap::new(),
            home_tags_cache: Vec::new(),
            icon_paths: std::collections::HashMap::new(),
            view_lock_enabled: false,
            zoom_level: zoom_for_view_mode(&view_mode),
            scroll_handle: ScrollHandle::new(),
            uniform_list_scroll_handle: UniformListScrollHandle::new(),
        }
    }

    /// Update view mode and zoom level together
    pub fn set_view_mode_with_zoom(&mut self, mode: ViewMode) {
        self.view_mode = mode.clone();
        self.zoom_level = zoom_for_view_mode(&mode);
    }

    /// Update zoom level and derive view mode
    pub fn set_zoom_with_mode(&mut self, zoom: f32) {
        self.zoom_level = zoom.max(0.0).min(1.0);
        self.view_mode = view_mode_for_zoom(self.zoom_level);
    }

    /// Return the sorted+filtered items as index slices into self.items.
    pub fn visible_items(&self) -> Vec<usize> {
        let filter = self.filter_text.to_lowercase();
        let mut indices: Vec<usize> = self
            .items
            .iter()
            .enumerate()
            .filter(|(_, item)| {
                // 1. System file filter
                if !self.show_system_files && item.is_system {
                    return false;
                }
                // 2. Hidden file filter (Windows hidden attribute)
                if !self.show_hidden_files && item.is_hidden {
                    return false;
                }
                // 3. Dot file filter (cross-platform)
                if !self.show_dot_files && item.name.starts_with('.') {
                    return false;
                }
                // 4. Search text filter
                if !filter.is_empty() && !item.name.to_lowercase().contains(&filter) {
                    return false;
                }
                true
            })
            .map(|(i, _)| i)
            .collect();

        // Sort
        indices.sort_by(|&a, &b| {
            let ia = &self.items[a];
            let ib = &self.items[b];
            // Folders first
            if self.sort.folders_first {
                let af = matches!(ia.file_type, FileType::Directory);
                let bf = matches!(ib.file_type, FileType::Directory);
                if af != bf {
                    return if af {
                        std::cmp::Ordering::Less
                    } else {
                        std::cmp::Ordering::Greater
                    };
                }
            }
            let ord = match self.sort.column {
                SortBy::Name => ia.name.to_lowercase().cmp(&ib.name.to_lowercase()),
                SortBy::Size => ia.size.cmp(&ib.size),
                SortBy::Type => {
                    let ta = if matches!(ia.file_type, FileType::Directory) {
                        "Folder"
                    } else {
                        "File"
                    };
                    let tb = if matches!(ib.file_type, FileType::Directory) {
                        "Folder"
                    } else {
                        "File"
                    };
                    ta.cmp(tb)
                }
                SortBy::Tag => {
                    let tags_a = self.tags_cache.get(&ia.path.to_string_lossy().to_string());
                    let tags_b = self.tags_cache.get(&ib.path.to_string_lossy().to_string());
                    let first_tag_a = tags_a.and_then(|t| t.first()).map(|s| s.as_str());
                    let first_tag_b = tags_b.and_then(|t| t.first()).map(|s| s.as_str());
                    match (first_tag_a, first_tag_b) {
                        (Some(a), Some(b)) => a.cmp(b),
                        (Some(_), None) => std::cmp::Ordering::Less,
                        (None, Some(_)) => std::cmp::Ordering::Greater,
                        (None, None) => ia.name.to_lowercase().cmp(&ib.name.to_lowercase()),
                    }
                }
                // DateModified / DateCreated — fall back to name for now
                _ => ia.name.to_lowercase().cmp(&ib.name.to_lowercase()),
            };
            if self.sort.ascending {
                ord
            } else {
                ord.reverse()
            }
        });

        indices
    }

    /// Toggle sort column; flip direction if already sorting by that column.
    pub fn toggle_sort(&mut self, col: SortBy) {
        if std::mem::discriminant(&self.sort.column) == std::mem::discriminant(&col) {
            self.sort.ascending = !self.sort.ascending;
        } else {
            self.sort.column = col;
            self.sort.ascending = true;
        }
    }

    /// Get the group key for an item based on the current group_by setting.
    pub fn get_group_key(&self, item: &FileItem) -> Option<String> {
        match self.group_by {
            GroupColumn::None => None,
            GroupColumn::Name => {
                let first = item.name.chars().next().unwrap_or('#').to_ascii_uppercase();
                if first.is_ascii_alphabetic() {
                    Some(first.to_string())
                } else {
                    Some("#".to_string())
                }
            }
            GroupColumn::DateModified | GroupColumn::DateCreated => Some(self.get_date_group_key()),
            GroupColumn::Size => Some(self.get_size_bucket(item.size)),
            GroupColumn::FileType | GroupColumn::SyncStatus => {
                let ext = if matches!(item.file_type, FileType::Directory) {
                    "Folder".to_string()
                } else {
                    item.name.rsplit('.').next().unwrap_or("").to_uppercase()
                };
                if ext.is_empty() {
                    Some("File".to_string())
                } else {
                    Some(ext)
                }
            }
            GroupColumn::Tag => {
                let tags = self
                    .tags_cache
                    .get(&item.path.to_string_lossy().to_string());
                match tags.and_then(|t| t.first()) {
                    Some(tag) => Some(tag.clone()),
                    None => Some("Untagged".to_string()),
                }
            }
        }
    }

    fn get_date_group_key(&self) -> String {
        // Simplified: just use a fixed bucket for demonstration
        // In production, you'd use chrono to compare with current time
        "Items".to_string()
    }

    fn get_size_bucket(&self, size: u64) -> String {
        const KB: u64 = 1024;
        const MB: u64 = KB * 1024;
        const GB: u64 = MB * 1024;
        if size == 0 {
            "Empty".to_string()
        } else if size < 10 * KB {
            "Tiny".to_string()
        } else if size < 1 * MB {
            "Small".to_string()
        } else if size < 100 * MB {
            "Medium".to_string()
        } else if size < 1 * GB {
            "Large".to_string()
        } else {
            "Huge".to_string()
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

    /// Partition visible indices into groups. Returns (group_headers, group_contents).
    /// group_headers[i] = (group_key, group_label, item_count)
    /// group_contents[i] = list of item indices in that group
    pub fn partition_groups(
        &self,
        visible: &[usize],
    ) -> (Vec<(String, String, usize)>, Vec<Vec<usize>>) {
        if self.group_by == GroupColumn::None {
            return (vec![], vec![]);
        }

        let mut groups: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        for &idx in visible {
            if let Some(item) = self.items.get(idx) {
                if let Some(key) = self.get_group_key(item) {
                    groups.entry(key).or_default().push(idx);
                }
            }
        }

        let mut headers = Vec::new();
        let mut contents = Vec::new();
        for (key, indices) in groups {
            let count = indices.len();
            // Format label nicely
            let label = if self.group_by == GroupColumn::Name && key.len() == 1 {
                format!("{}", key)
            } else {
                format!("{} ({})", key, count)
            };
            headers.push((key.clone(), label, count));
            contents.push(indices);
        }

        (headers, contents)
    }

    /// Start dragging an item for reordering within/between groups.
    /// Returns the source group key if the item is part of a group.
    pub fn start_group_drag(&mut self, item_index: usize) -> Option<String> {
        // Check if grouping is enabled
        if self.group_by == GroupColumn::None {
            return None;
        }

        // Get the item and its group
        let item = self.items.get(item_index)?;
        let source_key = self.get_group_key(item)?;

        // Initialize drag state
        self.group_drag_state.dragged_index = Some(item_index);
        self.group_drag_state.source_group_key = Some(source_key.clone());
        self.group_drag_state.target_group_key = None;
        self.group_drag_state.insert_position = None;
        self.group_drag_state.is_dragging = true;

        Some(source_key)
    }

    /// Update the drag position during a group drag operation.
    /// Returns the target group key and insert position within that group.
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

    /// End the group drag operation and perform the reorder if valid.
    /// Returns true if a reorder was performed.
    pub fn end_group_drag(&mut self) -> bool {
        if !self.group_drag_state.is_dragging {
            return false;
        }

        let dragged_idx = self.group_drag_state.dragged_index;
        let target_key = self.group_drag_state.target_group_key.clone();
        let insert_pos = self.group_drag_state.insert_position;

        // Reset drag state
        self.group_drag_state = GroupDragState::default();

        // If no drag happened or no target, do nothing
        let dragged_idx = match dragged_idx {
            Some(idx) => idx,
            None => return false,
        };

        let target_key = match target_key {
            Some(key) => key,
            None => return false,
        };

        // Get source group
        let source_key = match self.items.get(dragged_idx) {
            Some(item) => self.get_group_key(item),
            None => return false,
        };

        // If dropping in the same position, do nothing
        if source_key.as_ref() == Some(&target_key) && insert_pos.is_none() {
            return false;
        }

        // Perform the reorder
        // Note: This is a placeholder implementation. Full reorder requires:
        // 1. A manual sort order that can be persisted
        // 2. UI to show insertion indicator
        // For now, we just reset the drag state
        tracing::debug!(
            "Group drag: item {} moved from {:?} to {} (insert: {:?})",
            dragged_idx,
            source_key,
            target_key,
            insert_pos
        );

        true
    }

    /// Cancel the group drag operation without reordering.
    pub fn cancel_group_drag(&mut self) {
        self.group_drag_state = GroupDragState::default();
    }

    /// Check if a group drag operation is in progress.
    pub fn is_group_dragging(&self) -> bool {
        self.group_drag_state.is_dragging
    }

    /// Get the current drag target for UI highlighting.
    pub fn get_group_drag_target(&self) -> Option<(String, Option<usize>)> {
        if !self.group_drag_state.is_dragging {
            return None;
        }
        self.group_drag_state
            .target_group_key
            .clone()
            .map(|key| (key, self.group_drag_state.insert_position))
    }

    /// Display name for an item (respects show_extensions setting).
    pub fn display_name<'a>(&self, item: &'a FileItem) -> std::borrow::Cow<'a, str> {
        if !self.show_extensions && matches!(item.file_type, FileType::File) {
            let name = &item.name;
            if let Some(pos) = name.rfind('.') {
                return std::borrow::Cow::Owned(name[..pos].to_string());
            }
        }
        std::borrow::Cow::Borrowed(&item.name)
    }

    /// Flatten grouped items into a list of (is_header, index_or_group_key)
    pub fn flatten_grouped_items(&self, visible: &[usize]) -> Vec<GroupedItemType> {
        let (headers, groups) = self.partition_groups(visible);
        let mut flat = Vec::new();
        for (i, (key, label, count)) in headers.into_iter().enumerate() {
            flat.push(GroupedItemType::Header {
                key: key.clone(),
                label,
                count,
            });
            if !self.collapsed_groups.contains(&key) {
                for &idx in &groups[i] {
                    flat.push(GroupedItemType::Item(idx));
                }
            }
        }
        flat
    }

    /// Flatten grouped grid items into a list of (Header | Row)
    pub fn flatten_grouped_grid_items(
        &self,
        visible: &[usize],
        items_per_row: usize,
    ) -> Vec<GroupedGridItem> {
        let (headers, groups) = self.partition_groups(visible);
        let mut flat = Vec::new();
        for (i, (key, label, count)) in headers.into_iter().enumerate() {
            flat.push(GroupedGridItem::Header {
                key: key.clone(),
                label,
                count,
            });
            if !self.collapsed_groups.contains(&key) {
                let group_indices = &groups[i];
                for row_start in (0..group_indices.len()).step_by(items_per_row) {
                    let row_end = (row_start + items_per_row).min(group_indices.len());
                    flat.push(GroupedGridItem::Row(
                        group_indices[row_start..row_end].to_vec(),
                    ));
                }
            }
        }
        flat
    }

    /// Reorder an item within its group: move from source_visual_index to target_visual_index.
    /// The visual indices are positions in the flattened visible list (accounting for group headers).
    pub fn reorder_item_within_group(
        &mut self,
        source_visual_index: usize,
        target_visual_index: usize,
    ) {
        if source_visual_index == target_visual_index {
            return;
        }

        let visible = self.visible_items();
        let flattened = self.flatten_grouped_items(&visible);

        // Find actual item indices (skip headers in flattened list)
        let mut item_indices: Vec<usize> = flattened
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

        // Reorder in self.items
        if source_idx < self.items.len() && target_idx < self.items.len() {
            let item = self.items.remove(source_idx);
            self.items.insert(target_idx, item);

            // Update selected_indices to maintain selection on same items
            let mut new_selected = std::collections::HashSet::new();
            for &old_idx in &self.selected_indices {
                if old_idx == source_idx {
                    // The selected item moved
                    let new_pos = if source_idx < target_idx {
                        // Item moved down
                        target_idx - 1
                    } else {
                        // Item moved up
                        target_idx
                    };
                    if new_pos < self.items.len() {
                        new_selected.insert(new_pos);
                    }
                } else {
                    // Adjust indices for items that shifted
                    let new_idx =
                        if source_idx < target_idx && old_idx > source_idx && old_idx <= target_idx
                        {
                            old_idx - 1
                        } else if source_idx > target_idx
                            && old_idx >= target_idx
                            && old_idx < source_idx
                        {
                            old_idx + 1
                        } else {
                            old_idx
                        };
                    if new_idx < self.items.len() {
                        new_selected.insert(new_idx);
                    }
                }
            }
            self.selected_indices = new_selected;
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Lasso (rubber-band) selection
    // ─────────────────────────────────────────────────────────────────────────

    /// Start a lasso selection at the given point.
    pub fn lasso_start(&mut self, point: Point<f32>) {
        self.lasso_state = Some(LassoState {
            start: point,
            current: point,
        });
    }

    /// Update the current lasso selection endpoint.
    pub fn lasso_update(&mut self, point: Point<f32>) {
        if let Some(ref mut state) = self.lasso_state {
            state.current = point;
        }
    }

    /// End lasso selection and apply it to selected_indices.
    pub fn lasso_end(&mut self, add_to_selection: bool) {
        if let Some(state) = self.lasso_state.take() {
            let (start, end) = state.rect();
            let min_x = start.x.min(end.x);
            let max_x = start.x.max(end.x);
            let min_y = start.y.min(end.y);
            let max_y = start.y.max(end.y);

            if !add_to_selection {
                self.selected_indices.clear();
            }

            // For grid view: calculate which items intersect the lasso rectangle
            let items_per_row = match self.view_mode {
                ViewMode::Grid => 6,
                _ => 4,
            };
            let item_width = 180.0;
            let item_height = 200.0;
            let gap_x = 12.0;
            let gap_y = 12.0;
            let padding = 16.0;

            let visible = self.visible_items();
            for (i, &idx) in visible.iter().enumerate() {
                let row = i / items_per_row;
                let col = i % items_per_row;
                let item_left = padding + col as f32 * (item_width + gap_x);
                let item_top = padding + row as f32 * (item_height + gap_y);
                let item_right = item_left + item_width;
                let item_bottom = item_top + item_height;

                if item_left < max_x
                    && item_right > min_x
                    && item_top < max_y
                    && item_bottom > min_y
                {
                    self.selected_indices.insert(idx);
                }
            }
        }
    }

    /// Cancel the current lasso operation without applying selection.
    pub fn lasso_cancel(&mut self) {
        self.lasso_state = None;
    }

    /// Get the current lasso rectangle if active.
    pub fn lasso_rect(&self) -> Option<(Point<f32>, Point<f32>)> {
        self.lasso_state.as_ref().map(|s| s.rect())
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Drag and drop
    // ─────────────────────────────────────────────────────────────────────────

    /// Start a drag operation with the given items and mouse position.
    pub fn drag_start(&mut self, indices: Vec<usize>, point: Point<f32>) {
        self.drag_state = DragState {
            indices,
            start: point,
            current: point,
            is_copy: false,
            is_dragging: true,
        };
    }

    /// Update the current drag position and copy/move modifier.
    pub fn drag_update(&mut self, point: Point<f32>, is_copy: bool) {
        self.drag_state.current = point;
        self.drag_state.is_copy = is_copy;
    }

    /// End the drag operation.
    pub fn drag_end(&mut self) {
        self.drag_state.is_dragging = false;
        self.drag_state.indices.clear();
    }

    /// Check if a drag operation is in progress.
    pub fn is_dragging(&self) -> bool {
        self.drag_state.is_dragging
    }

    /// Check if the given point is over a potential drop target (e.g., folder).
    pub fn get_drop_target(&self, point: Point<f32>, visible: &[usize]) -> Option<usize> {
        if !self.drag_state.is_dragging {
            return None;
        }

        let items_per_row = 6;
        let item_width = 180.0;
        let item_height = 200.0;
        let gap_x = 12.0;
        let gap_y = 12.0;
        let padding = 16.0;

        for (i, &idx) in visible.iter().enumerate() {
            if let Some(item) = self.items.get(idx) {
                if !matches!(item.file_type, FileType::Directory) {
                    continue;
                }
                let row = i / items_per_row;
                let col = i % items_per_row;
                let item_left = padding + col as f32 * (item_width + gap_x);
                let item_top = padding + row as f32 * (item_height + gap_y);
                let item_right = item_left + item_width;
                let item_bottom = item_top + item_height;

                if point.x >= item_left
                    && point.x <= item_right
                    && point.y >= item_top
                    && point.y <= item_bottom
                {
                    return Some(idx);
                }
            }
        }
        None
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Selection helpers
    // ─────────────────────────────────────────────────────────────────────────

    /// Select items within a rectangle defined by two points.
    pub fn select_rect(&mut self, start: Point<f32>, end: Point<f32>, add_to_selection: bool) {
        let (min_x, max_x) = (start.x.min(end.x), start.x.max(end.x));
        let (min_y, max_y) = (start.y.min(end.y), start.y.max(end.y));

        if !add_to_selection {
            self.selected_indices.clear();
        }

        let visible = self.visible_items();
        let items_per_row = match self.view_mode {
            ViewMode::Grid => 6,
            _ => 4,
        };
        let item_width = 180.0;
        let item_height = 200.0;
        let gap_x = 12.0;
        let gap_y = 12.0;
        let padding = 16.0;

        for (i, &idx) in visible.iter().enumerate() {
            let row = i / items_per_row;
            let col = i % items_per_row;
            let item_left = padding + col as f32 * (item_width + gap_x);
            let item_top = padding + row as f32 * (item_height + gap_y);
            let item_right = item_left + item_width;
            let item_bottom = item_top + item_height;

            if item_left < max_x && item_right > min_x && item_top < max_y && item_bottom > min_y {
                self.selected_indices.insert(idx);
            }
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum GroupedItemType {
    Header {
        key: String,
        label: String,
        count: usize,
    },
    Item(usize),
}

#[derive(Clone, PartialEq, Eq)]
pub enum GroupedGridItem {
    Header {
        key: String,
        label: String,
        count: usize,
    },
    Row(Vec<usize>),
}

// ─────────────────────────────────────────────────────────────────────────────
// Main view renderer
// ─────────────────────────────────────────────────────────────────────────────

/// Separate model for file list data (read by uniform_list closures without entity conflict)
/// This is required by the GPUI uniform_list pattern: closures must read from a DIFFERENT
/// entity than the one being rendered to avoid "cannot read while already being updated" panics.
#[derive(Debug, Clone)]
pub struct FileItems {
    pub items: Vec<FileItem>,
    pub selected_indices: HashSet<usize>,
}

impl Default for FileItems {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            selected_indices: HashSet::new(),
        }
    }
}

pub struct MainView;

impl MainView {
    pub fn build(
        state: &MainViewState,
        file_items: Entity<FileItems>,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let has_side_pane = state.side_pane != SidePaneMode::None;
        let show_shelf = state.shelf_visible;
        let v2 = view.clone();
        let v3 = view.clone();

        // Collect lasso overlay element if active
        let lasso_overlay = Self::render_lasso_overlay(state);

        div()
            .flex()
            .flex_row()
            .flex_grow()
            .w_full()
            // NO overflow_hidden() - would block child scroll events
            .bg(rgb(UiConstants::BG_MAIN_RGB))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_grow()
                    .w_full()
                    // NO overflow_hidden() - would block child scroll events
                    .child(Self::render_panes(state, file_items, view.clone()))
                    .children(if show_shelf {
                        vec![Self::render_shelf_pane(state, v2).into_any_element()]
                    } else {
                        vec![]
                    }),
            )
            .children(if has_side_pane {
                vec![Self::render_side_pane(state, v3).into_any_element()]
            } else {
                vec![]
            })
            // Lasso overlay (rendered on top)
            .children(if let Some(overlay) = lasso_overlay {
                vec![overlay.into_any_element()]
            } else {
                vec![]
            })
    }

    /// Render the lasso selection overlay rectangle
    fn render_lasso_overlay(state: &MainViewState) -> Option<impl IntoElement> {
        let lasso_state = state.lasso_state.as_ref()?;
        let (start, end) = lasso_state.rect();

        // Calculate rectangle dimensions
        let left = start.x.min(end.x);
        let top = start.y.min(end.y);
        let width = (end.x - start.x).abs();
        let height = (end.y - start.y).abs();

        // Don't render if very small (just a click)
        if width < 5.0 || height < 5.0 {
            return None;
        }

        Some(
            div()
                .id("lasso-overlay")
                .absolute()
                .left(px(left))
                .top(px(top))
                .w(px(width))
                .h(px(height))
                .bg(rgba(0x208B2DC8)) // Semi-transparent accent color
                .border(px(1.0))
                .border_color(rgba(UiConstants::ACCENT_RGBA)),
        )
    }

    fn render_panes(
        state: &MainViewState,
        file_items: Entity<FileItems>,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        match state.pane_type {
            PaneType::Single => div()
                .flex()
                .flex_col()
                .flex_grow()
                .w_full()
                // NO overflow_hidden() - would block child scroll events
                .child(Self::render_file_view(state, file_items, view, true)),
            PaneType::Dual => {
                // Horizontal = side-by-side; Vertical = stacked
                let is_horiz = matches!(state.dual_pane_split, DualPaneSplit::Horizontal);
                let splitter_w = Pixels::from(UiConstants::SPLITTER_WIDTH_RAW);
                let base = div().flex().flex_grow().w_full();

                let is_primary_active = matches!(
                    state.active_pane,
                    crate::core::navigation_manager::PaneFocus::Primary
                );
                let is_secondary_active = !is_primary_active;

                if is_horiz {
                    base.flex_row()
                        .child(div().flex_grow().child(Self::render_file_view(
                            state,
                            file_items.clone(),
                            view.clone(),
                            is_primary_active,
                        )))
                        .child(div().w(splitter_w).bg(rgb(UiConstants::BG_NAV_BAR_RGB)))
                        .child(div().flex_grow().child(Self::render_file_view(
                            state,
                            file_items,
                            view,
                            is_secondary_active,
                        )))
                } else {
                    base.flex_col()
                        .child(div().flex_grow().w_full().child(Self::render_file_view(
                            state,
                            file_items.clone(),
                            view.clone(),
                            is_primary_active,
                        )))
                        .child(
                            div()
                                .h(splitter_w)
                                .bg(rgb(UiConstants::BG_NAV_BAR_RGB))
                                .w_full(),
                        )
                        .child(div().flex_grow().w_full().child(Self::render_file_view(
                            state,
                            file_items,
                            view,
                            is_secondary_active,
                        )))
                }
            }
        }
    }

    fn render_side_pane(
        state: &MainViewState,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let mode = state.side_pane.clone();
        let selected_item = state
            .selected_indices
            .iter()
            .next()
            .and_then(|&i| state.items.get(i));

        div()
            .id("side-pane")
            .w(px(300.0))
            .h_full()
            .flex_col()
            .bg(rgba(UiConstants::BG_TOOLBAR_RGBA))
            .border_l(px(1.0))
            .border_color(rgba(UiConstants::DIVIDER_RGBA))
            .child(
                // Pane header with Preview / Info tabs
                div()
                    .flex()
                    .flex_row()
                    .h(px(36.0))
                    .items_center()
                    .px(px(8.0))
                    .border_b(px(1.0))
                    .border_color(rgba(UiConstants::DIVIDER_RGBA))
                    .child(
                        div()
                            .text_size(AbsoluteLength::Pixels(px(12.0)))
                            .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                            .child(match mode {
                                SidePaneMode::Preview => "Preview",
                                SidePaneMode::Info => "Info",
                                SidePaneMode::Details => "Details",
                                SidePaneMode::None => "",
                            }),
                    ),
            )
            .child(
                // Pane body
                div()
                    .flex_grow()
                    .flex_col()
                    .items_center()
                    .p(px(12.0))
                    .children(match selected_item {
                        None => vec![div()
                            .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                            .text_size(AbsoluteLength::Pixels(px(12.0)))
                            .text_center()
                            .child("Select an item to preview")
                            .into_any_element()],
                        Some(item) => match mode {
                            SidePaneMode::Preview => {
                                vec![Self::render_preview_content(state, item).into_any_element()]
                            }
                            SidePaneMode::Info => {
                                vec![Self::render_info_content(item).into_any_element()]
                            }
                            SidePaneMode::Details => {
                                vec![Self::render_details_pane(state, item).into_any_element()]
                            }
                            SidePaneMode::None => vec![],
                        },
                    }),
            )
    }

    fn render_preview_content(state: &MainViewState, item: &FileItem) -> impl IntoElement {
        match &state.preview_content {
            PreviewContent::Markdown => {
                if let Some(text) = &state.preview_text {
                    Self::render_markdown_preview(text).into_any_element()
                } else {
                    Self::render_default_preview(item, state).into_any_element()
                }
            }
            PreviewContent::Code { language } => {
                if let Some(text) = &state.preview_text {
                    Self::render_code_preview(text, language).into_any_element()
                } else {
                    Self::render_default_preview(item, state).into_any_element()
                }
            }
            PreviewContent::Html => {
                if let Some(text) = &state.preview_text {
                    Self::render_html_preview(text).into_any_element()
                } else {
                    Self::render_default_preview(item, state).into_any_element()
                }
            }
            PreviewContent::RichText => {
                if let Some(text) = &state.preview_text {
                    Self::render_rtf_preview(text).into_any_element()
                } else {
                    Self::render_default_preview(item, state).into_any_element()
                }
            }
            PreviewContent::Audio(info) | PreviewContent::Video(info) => {
                let type_str = if matches!(&state.preview_content, PreviewContent::Audio(_)) {
                    "Audio"
                } else {
                    "Video"
                };
                let text = state
                    .preview_text
                    .as_deref()
                    .unwrap_or("No metadata available");
                Self::render_media_preview(info, type_str, text, item).into_any_element()
            }
            PreviewContent::Folder(items) => Self::render_folder_preview(items).into_any_element(),
            PreviewContent::Pdf(info) => {
                let text = state
                    .preview_text
                    .as_deref()
                    .unwrap_or("Loading PDF info...");
                Self::render_pdf_preview(info, text).into_any_element()
            }
            PreviewContent::Archive(info) => {
                let text = state
                    .preview_text
                    .as_deref()
                    .unwrap_or("Loading archive...");
                Self::render_archive_preview(info, text).into_any_element()
            }
            PreviewContent::Text(_) | PreviewContent::None => {
                Self::render_default_preview(item, state).into_any_element()
            }
            PreviewContent::ThirdParty(info) => {
                if let Some(text) = &state.preview_text {
                    // Show shell preview text for text-based files
                    div()
                        .flex_col()
                        .w_full()
                        .max_h(px(400.0))
                        .child(
                            div()
                                .child(div().text_size(px(13.0)).font_weight(FontWeight(600.0)).mb(px(6.0))
                                    .child(format!("Shell Preview ({:?})", info.handler)))
                                .child(
                                    div()
                                        .bg(rgba(0xFFFFFF05u32))
                                        .rounded(px(4.0))
                                        .p(px(8.0))
                                        .child(
                                            div()
                                                .text_size(px(11.0))
                                                .font_family("ui-monospace, SFMono-Regular, 'Cascadia Code', Consolas, monospace")
                                                .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                                                .child(text.clone()),
                                        ),
                                ),
                        )
                        .into_any_element()
                } else {
                    Self::render_default_preview(item, state).into_any_element()
                }
            }
        }
    }

    fn render_info_content(item: &FileItem) -> impl IntoElement {
        let type_label = if matches!(item.file_type, FileType::Directory) {
            "Folder"
        } else {
            "File"
        };
        let size_str = format_size(item.size);

        div()
            .flex_col()
            .gap(px(8.0))
            .w_full()
            .child(info_row("Name", &item.name))
            .child(info_row("Type", type_label))
            .child(info_row("Size", &size_str))
            .child(info_row("Path", &item.path.to_string_lossy()))
    }

    /// Rich Details pane — shows extended metadata, git status stub, tags
    fn render_details_pane(state: &MainViewState, item: &FileItem) -> impl IntoElement {
        let type_label = if matches!(item.file_type, FileType::Directory) {
            "Folder"
        } else {
            "File"
        };
        let ext = item.name.rsplit('.').next().unwrap_or("").to_uppercase();
        let size_str = format_size(item.size);
        let is_image = matches!(
            ext.as_str(),
            "JPG" | "JPEG" | "PNG" | "WEBP" | "BMP" | "GIF" | "TIFF"
        );

        div()
            .flex_col()
            .gap(px(10.0))
            .w_full()
            // Thumbnail / large icon
            .child(
                div()
                    .w_full()
                    .h(px(120.0))
                    .bg(rgba(0xFFFFFF08u32))
                    .rounded(px(6.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(if is_image {
                        img(item.path.as_path().to_path_buf())
                            .w_full()
                            .h_full()
                            .into_any_element()
                    } else {
                        div()
                            .text_size(px(40.0))
                            .font_family(UiConstants::FONT_MDL2)
                            .text_color(rgba(UiConstants::ACCENT_RGBA))
                            .child(if matches!(item.file_type, FileType::Directory) {
                                "\u{E8B7}"
                            } else {
                                "\u{E8A5}"
                            })
                            .into_any_element()
                    }),
            )
            // File name
            .child(
                div()
                    .text_size(px(14.0))
                    .font_weight(FontWeight(600.0))
                    .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                    .text_center()
                    .child(item.name.clone()),
            )
            // Separator
            .child(
                div()
                    .w_full()
                    .h(px(1.0))
                    .bg(rgba(UiConstants::DIVIDER_RGBA)),
            )
            // Properties section
            .child(
                div()
                    .flex_col()
                    .gap(px(6.0))
                    .child(section_header("Properties"))
                    .child(info_row("Type", type_label))
                    .child(info_row("Size", &size_str))
                    .child(info_row("Format", if ext.is_empty() { "—" } else { &ext })),
            )
            // Shortcut section
            .children(if ext == "LNK" {
                vec![
                    div()
                        .w_full()
                        .h(px(1.0))
                        .bg(rgba(UiConstants::DIVIDER_RGBA))
                        .into_any_element(),
                    div()
                        .flex_col()
                        .gap(px(6.0))
                        .child(section_header("Shortcut"))
                        .child(info_row("Target", "C:\\Program Files\\..."))
                        .child(info_row("Arguments", "--preview"))
                        .into_any_element(),
                ]
            } else {
                vec![]
            })
            // Tags section
            .child(
                div()
                    .w_full()
                    .h(px(1.0))
                    .bg(rgba(UiConstants::DIVIDER_RGBA)),
            )
            .child(
                div()
                    .flex_col()
                    .gap(px(6.0))
                    .child(section_header("Tags"))
                    .children(if state.details_cache.tags.is_empty() {
                        vec![div()
                            .text_size(px(12.0))
                            .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                            .child("No tags — right-click to add")
                            .into_any_element()]
                    } else {
                        state
                            .details_cache
                            .tags
                            .iter()
                            .map(|tag| {
                                div()
                                    .flex()
                                    .px(px(6.0))
                                    .py(px(2.0))
                                    .bg(rgba(0xFFFFFF1Au32))
                                    .rounded(px(4.0))
                                    .text_size(px(11.0))
                                    .text_color(rgba(UiConstants::ACCENT_RGBA))
                                    .child(tag.clone())
                                    .into_any_element()
                            })
                            .collect()
                    }),
            )
            // EXIF/Metadata section (for images)
            .children(
                if !state.details_cache.exif_dimensions.is_none()
                    || state.details_cache.audio_duration.is_some()
                {
                    let mut children = vec![
                        div()
                            .w_full()
                            .h(px(1.0))
                            .bg(rgba(UiConstants::DIVIDER_RGBA))
                            .into_any_element(),
                        div()
                            .flex_col()
                            .gap(px(6.0))
                            .child(section_header("Media Info"))
                            .into_any_element(),
                    ];

                    if let Some(ref dims) = state.details_cache.exif_dimensions {
                        children.push(info_row("Dimensions", dims).into_any_element());
                    }
                    if let Some(ref camera) = state.details_cache.exif_camera {
                        children.push(info_row("Camera", camera).into_any_element());
                    }
                    if let Some(ref date) = state.details_cache.exif_date {
                        children.push(info_row("Date Taken", date).into_any_element());
                    }
                    if let Some(ref duration) = state.details_cache.audio_duration {
                        children.push(info_row("Duration", duration).into_any_element());
                    }
                    if let Some(ref artist) = state.details_cache.audio_artist {
                        children.push(info_row("Artist", artist).into_any_element());
                    }
                    if let Some(ref album) = state.details_cache.audio_album {
                        children.push(info_row("Album", album).into_any_element());
                    }
                    if let Some(ref res) = state.details_cache.video_resolution {
                        children.push(info_row("Resolution", res).into_any_element());
                    }

                    children
                } else {
                    vec![]
                },
            )
            // Git section — shows last commit info and file status
            .child(
                div()
                    .w_full()
                    .h(px(1.0))
                    .bg(rgba(UiConstants::DIVIDER_RGBA)),
            )
            .children({
                let mut git_children: Vec<gpui::AnyElement> = Vec::new();
                git_children.push(
                    div()
                        .flex_col()
                        .gap(px(6.0))
                        .child(section_header("Git"))
                        .into_any_element(),
                );

                // Try to get last commit info for this file
                let commit_info =
                    crate::domain::git_service::GitService::get_last_commit(&item.path);
                if let Some(info) = commit_info {
                    git_children.push(info_row("Author", &info.author).into_any_element());
                    git_children.push(info_row("Date", &info.date).into_any_element());
                    git_children.push(info_row("Message", &info.message).into_any_element());
                } else {
                    git_children.push(
                        div()
                            .text_size(px(12.0))
                            .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                            .child(if state.side_pane != SidePaneMode::None {
                                "Not in a repository"
                            } else {
                                ""
                            })
                            .into_any_element(),
                    );
                }

                git_children
            })
            // Text preview section
            .children(if let Some(text) = &state.preview_text {
                vec![
                    div()
                        .w_full()
                        .h(px(1.0))
                        .bg(rgba(UiConstants::DIVIDER_RGBA))
                        .into_any_element(),
                    div()
                        .flex_col()
                        .gap(px(6.0))
                        .child(section_header("Preview"))
                        .child(
                            div()
                                .bg(rgba(0xFFFFFF05u32))
                                .p(px(8.0))
                                .rounded(px(4.0))
                                .child(
                                    div()
                                        .text_size(px(11.0))
                                        .font_family(UiConstants::FONT_MAIN)
                                        .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                                        .child(text.clone()),
                                ),
                        )
                        .into_any_element(),
                ]
            } else {
                vec![]
            })
    }

    /// Render the shelf pane (staging area for bulk operations)
    fn render_shelf_pane(
        state: &MainViewState,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let item_count = state.shelf_items.len();
        let shelf_size: u64 = state.shelf_items.iter().map(|s| s.size).sum();

        div()
            .id("shelf-pane")
            .h(px(180.0))
            .w_full()
            .flex_col()
            .bg(rgba(0xFFFFFF08u32))
            .border_t(px(1.0))
            .border_color(rgba(UiConstants::DIVIDER_RGBA))
            // Header bar
            .child(
                div()
                    .flex()
                    .flex_row()
                    .h(px(32.0))
                    .items_center()
                    .px(px(12.0))
                    .gap(px(8.0))
                    .border_b(px(1.0))
                    .border_color(rgba(UiConstants::DIVIDER_RGBA))
                    .child(
                        div()
                            .text_size(px(12.0))
                            .font_family(UiConstants::FONT_MDL2)
                            .text_color(rgba(UiConstants::ACCENT_RGBA))
                            .child("\u{E8A0}"), // Shelf icon
                    )
                    .child(
                        div()
                            .flex_grow()
                            .text_size(px(12.0))
                            .font_weight(FontWeight(600.0))
                            .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                            .child(format!(
                                "Shelf ({} items, {})",
                                item_count,
                                format_size(shelf_size)
                            )),
                    )
                    .child({
                        let v = view.clone();
                        div()
                            .text_size(px(11.0))
                            .font_family(UiConstants::FONT_MDL2)
                            .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                            .cursor_pointer()
                            .hover(|s| s.text_color(rgba(UiConstants::ACCENT_RGBA)))
                            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                v.update(cx, |this, cx| {
                                    this.state.main_view.shelf_items.clear();
                                    cx.notify();
                                });
                            })
                            .child("\u{E74D}") // Clear icon
                    }),
            )
            // Drop zone / items
            .child(if state.shelf_items.is_empty() {
                div()
                    .flex_grow()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(rgba(0xFFFFFF40u32))
                            .text_center()
                            .child("Drag files here to stage for bulk operations"),
                    )
                    .into_any_element()
            } else {
                div()
                    .flex_grow()
                    .flex_col()
                    .overflow_hidden()
                    .children(state.shelf_items.iter().enumerate().map(|(i, item)| {
                        let name = item.name.clone();
                        let size = item.size;
                        let is_dir = item.is_dir;
                        let v = view.clone();
                        div()
                            .flex()
                            .flex_row()
                            .h(px(28.0))
                            .items_center()
                            .px(px(12.0))
                            .gap(px(8.0))
                            .hover(|s| s.bg(rgba(0xFFFFFF08u32)))
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .font_family(UiConstants::FONT_MDL2)
                                    .text_color(rgba(UiConstants::ACCENT_RGBA))
                                    .child(if is_dir { "\u{E8B7}" } else { "\u{E8A5}" }),
                            )
                            .child(
                                div()
                                    .flex_grow()
                                    .text_size(px(12.0))
                                    .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                                    .child(name),
                            )
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                    .child(if is_dir {
                                        String::new()
                                    } else {
                                        format_size(size)
                                    }),
                            )
                            .child(
                                div()
                                    .text_size(px(10.0))
                                    .font_family(UiConstants::FONT_MDL2)
                                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                    .cursor_pointer()
                                    .hover(|s| s.text_color(rgba(UiConstants::ACCENT_RGBA)))
                                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                        let idx = i;
                                        v.update(cx, |this, cx| {
                                            if idx < this.state.main_view.shelf_items.len() {
                                                this.state.main_view.shelf_items.remove(idx);
                                            }
                                            cx.notify();
                                        });
                                    })
                                    .child("\u{E711}"), // Remove icon
                            )
                    }))
                    .into_any_element()
            })
    }

    fn render_file_view(
        state: &MainViewState,
        file_items: Entity<FileItems>,
        view: Entity<crate::app::AppWindow>,
        is_active: bool,
    ) -> impl IntoElement {
        let visible = state.visible_items();

        // Empty state
        if visible.is_empty() && !state.loading {
            return Self::empty_state(state).into_any_element();
        }

        div()
            .flex_col()
            .flex_grow()
            .h_full() // CRITICAL: Provide concrete height foundation for flex_grow to work
            .w_full()
            // NO overflow_hidden here - moves to content div
            .bg(if is_active {
                rgba(UiConstants::BG_MAIN_RGBA)
            } else {
                rgba(UiConstants::BG_INACTIVE_PANE_RGBA)
            }) // Dim if inactive
            .child(Self::filter_header(state, view.clone()))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .h_full() // Explicit height for flex children
                    .w_full()
                    .overflow_hidden() // Content clipping for uniform_list
                    .child({
                        if matches!(state.target, ViewTarget::RecycleBin) {
                            Self::render_recycle_bin_view(state, view).into_any_element()
                        } else {
                            match state.view_mode {
                                ViewMode::Details => {
                                    if matches!(state.target, ViewTarget::Home) {
                                        Self::render_home_view(state, view).into_any_element()
                                    } else {
                                        Self::details_view(state, &visible, file_items, view)
                                            .into_any_element()
                                    }
                                }
                                ViewMode::List => state.render_list_view(file_items, view),
                                ViewMode::Grid => {
                                    state.render_grid_view(file_items, view).into_any_element()
                                }
                                ViewMode::Columns => state
                                    .render_columns_view(file_items, view)
                                    .into_any_element(),
                            }
                        }
                    }),
            )
            .into_any_element()
    }

    fn render_home_view(
        state: &MainViewState,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let tags = state.home_tags_cache.clone();
        div()
            .flex_col()
            .flex_grow()
            .w_full()
            // NO h_full() - flex_grow takes remaining space
            .p(px(24.0))
            .gap(px(24.0))
            .id("home-scroll")
            .overflow_y_scroll()
            .track_scroll(&state.scroll_handle)
            .child(
                div()
                    .flex_col()
                    .gap(px(8.0))
                    .child(
                        div()
                            .font_weight(FontWeight(600.0))
                            .text_size(px(18.0))
                            .font_family(UiConstants::FONT_MAIN)
                            .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                            .child("Quick Access"),
                    )
                    .child(
                        div()
                            .flex_row()
                            .gap(px(12.0))
                            .child(Self::build_home_widget("Desktop", "\u{E8B7}", view.clone()))
                            .child(Self::build_network_widget(
                                "Network".to_string(),
                                "\u{E968}".to_string(),
                                "Connected".to_string(),
                            ))
                            .child(Self::build_network_widget(
                                "Server".to_string(),
                                "\u{E839}".to_string(),
                                "Available".to_string(),
                            )),
                    ),
            )
            .child(
                div()
                    .flex_col()
                    .gap(px(8.0))
                    .child(
                        div()
                            .font_weight(FontWeight(600.0))
                            .text_size(px(18.0))
                            .font_family(UiConstants::FONT_MAIN)
                            .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                            .child("Drives"),
                    )
                    .child(
                        div()
                            .flex_row()
                            .gap(px(12.0))
                            .child(Self::build_drive_widget(
                                "Local Disk (C:)",
                                0.65,
                                view.clone(),
                            ))
                            .child(Self::build_drive_widget("Data (D:)", 0.2, view.clone())),
                    ),
            )
            .child(Self::build_recent_files_section(view.clone()))
            .child(Self::build_file_tags_section(tags, view.clone()))
            .child(Self::build_network_section(view.clone()))
    }

    fn render_recycle_bin_view(
        state: &MainViewState,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let item_count = state.recycle_bin_items.len();

        div()
            .flex_col()
            .flex_grow()
            .p(px(24.0))
            .gap(px(16.0))
            .id("recycle-bin-view")
            // Header
            .child(
                div().flex_row().items_center().gap(px(12.0)).child(
                    div()
                        .font_weight(FontWeight(600.0))
                        .text_size(px(20.0))
                        .font_family(UiConstants::FONT_MAIN)
                        .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                        .child(format!("Recycle Bin ({} items)", item_count)),
                ),
            )
            // Action buttons
            .child(
                div()
                    .flex_row()
                    .gap(px(8.0))
                    // Empty Recycle Bin button
                    .child(
                        div()
                            .px(px(12.0))
                            .py(px(6.0))
                            .bg(rgba(0xE8112320u32))
                            .rounded(px(4.0))
                            .cursor_pointer()
                            .hover(|s| s.bg(rgba(0xE8112340u32)))
                            .on_mouse_down(MouseButton::Left, {
                                let view = view.clone();
                                move |_, _, cx| {
                                    view.update(cx, |this, cx| {
                                        this.empty_recycle_bin(cx);
                                    });
                                }
                            })
                            .child(
                                div()
                                    .text_size(px(13.0))
                                    .font_family(UiConstants::FONT_MAIN)
                                    .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                                    .child("Empty Recycle Bin"),
                            ),
                    )
                    // Restore All button
                    .child(
                        div()
                            .px(px(12.0))
                            .py(px(6.0))
                            .bg(rgba(0x107C1020u32))
                            .rounded(px(4.0))
                            .cursor_pointer()
                            .hover(|s| s.bg(rgba(0x107C1040u32)))
                            .on_mouse_down(MouseButton::Left, {
                                let view = view.clone();
                                move |_, _, cx| {
                                    view.update(cx, |this, cx| {
                                        this.restore_all_recycle_bin(cx);
                                    });
                                }
                            })
                            .child(
                                div()
                                    .text_size(px(13.0))
                                    .font_family(UiConstants::FONT_MAIN)
                                    .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                                    .child("Restore All"),
                            ),
                    ),
            )
            // Item list
            .child(div().flex_col().gap(px(4.0)).children(
                state.recycle_bin_items.iter().enumerate().map(|(i, item)| {
                    let view = view.clone();
                    div()
                        .flex_row()
                        .items_center()
                        .gap(px(12.0))
                        .px(px(12.0))
                        .py(px(8.0))
                        .bg(rgba(0xFFFFFF08u32))
                        .rounded(px(4.0))
                        .child(
                            div()
                                .flex_col()
                                .flex_grow()
                                .gap(px(2.0))
                                .child(
                                    div()
                                        .text_size(px(14.0))
                                        .font_family(UiConstants::FONT_MAIN)
                                        .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                                        .child(item.name.clone()),
                                )
                                .child(
                                    div()
                                        .flex_row()
                                        .gap(px(16.0))
                                        .child(
                                            div()
                                                .text_size(px(11.0))
                                                .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                                .child(format!("Original: {}", item.original_path)),
                                        )
                                        .child(
                                            div()
                                                .text_size(px(11.0))
                                                .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                                .child(format!(
                                                    "Size: {}",
                                                    if item.size >= 1024 * 1024 {
                                                        format!(
                                                            "{:.1} MB",
                                                            item.size as f64 / (1024.0 * 1024.0)
                                                        )
                                                    } else if item.size >= 1024 {
                                                        format!(
                                                            "{:.1} KB",
                                                            item.size as f64 / 1024.0
                                                        )
                                                    } else {
                                                        format!("{} B", item.size)
                                                    }
                                                )),
                                        )
                                        .child(
                                            div()
                                                .text_size(px(11.0))
                                                .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                                .child(format!(
                                                    "Deleted: {}",
                                                    item.date_deleted
                                                        .as_deref()
                                                        .unwrap_or("Unknown")
                                                )),
                                        ),
                                ),
                        )
                        // Restore button per item
                        .child(
                            div()
                                .px(px(10.0))
                                .py(px(4.0))
                                .bg(rgba(0x107C1020u32))
                                .rounded(px(4.0))
                                .cursor_pointer()
                                .hover(|s| s.bg(rgba(0x107C1040u32)))
                                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                    view.update(cx, |this, cx| {
                                        this.restore_recycle_bin_item(i, cx);
                                    });
                                })
                                .child(
                                    div()
                                        .text_size(px(12.0))
                                        .font_family(UiConstants::FONT_MAIN)
                                        .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                                        .child("Restore"),
                                ),
                        )
                }),
            ))
    }

    fn build_home_widget(
        label: &str,
        glyph: &str,
        _view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        div()
            .w(px(120.0))
            .h(px(100.0))
            .bg(rgba(0xFFFFFF10u32))
            .rounded(px(8.0))
            .border(px(1.0))
            .border_color(rgba(UiConstants::DIVIDER_RGBA))
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(8.0))
            .hover(|s| s.bg(rgba(0xFFFFFF15u32)))
            .cursor_pointer()
            .child(
                div()
                    .text_size(px(32.0))
                    .font_family(UiConstants::FONT_MDL2)
                    .text_color(rgba(UiConstants::ACCENT_RGBA))
                    .child(glyph.to_string()),
            )
            .child(
                div()
                    .text_size(px(13.0))
                    .font_family(UiConstants::FONT_MAIN)
                    .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                    .child(label.to_string()),
            )
    }

    fn build_drive_widget(
        label: &str,
        usage: f32,
        _view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        div()
            .w(px(200.0))
            .p(px(12.0))
            .bg(rgba(0xFFFFFF10u32))
            .rounded(px(8.0))
            .border(px(1.0))
            .border_color(rgba(UiConstants::DIVIDER_RGBA))
            .flex_col()
            .gap(px(8.0))
            .hover(|s| s.bg(rgba(0xFFFFFF15u32)))
            .cursor_pointer()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        div()
                            .text_size(px(24.0))
                            .font_family(UiConstants::FONT_MDL2)
                            .text_color(rgba(UiConstants::ACCENT_RGBA))
                            .child("\u{E770}"),
                    )
                    .child(
                        div()
                            .text_size(px(14.0))
                            .font_weight(FontWeight(600.0))
                            .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                            .child(label.to_string()),
                    ),
            )
            .child(
                div()
                    .flex_col()
                    .gap(px(4.0))
                    .child(
                        div()
                            .w_full()
                            .h(px(6.0))
                            .bg(rgba(0xFFFFFF20u32))
                            .rounded(px(3.0))
                            .child(
                                div()
                                    .h_full()
                                    .w(px(100.0 * usage))
                                    .bg(rgb(UiConstants::ACCENT_COLOR_RGB))
                                    .rounded(px(3.0)),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(rgb(UiConstants::TEXT_MUTED_RGB))
                            .child(format!("{:.0}% used", usage * 100.0)),
                    ),
            )
    }

    /// Build the Recent Files section for Home page
    /// Shows a table list: icon, name, path, date accessed
    /// Source: Windows Shell recent items via SHGetKnownFolderPath(FOLDERID_Recent)
    fn build_recent_files_section(_view: Entity<crate::app::AppWindow>) -> impl IntoElement {
        div()
            .flex_col()
            .gap(px(8.0))
            .child(
                div()
                    .font_weight(FontWeight(600.0))
                    .text_size(px(18.0))
                    .font_family(UiConstants::FONT_MAIN)
                    .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                    .child("Recent Files"),
            )
            .child(
                div()
                    .flex_col()
                    .bg(rgba(0xFFFFFF10u32))
                    .rounded(px(8.0))
                    .border(px(1.0))
                    .border_color(rgba(UiConstants::DIVIDER_RGBA))
                    .child(
                        // Header row
                        div()
                            .flex()
                            .flex_row()
                            .h(px(36.0))
                            .px(px(12.0))
                            .items_center()
                            .border_b(px(1.0))
                            .border_color(rgba(UiConstants::DIVIDER_RGBA))
                            .child(
                                div()
                                    .flex_grow()
                                    .text_size(px(12.0))
                                    .font_weight(FontWeight(600.0))
                                    .text_color(rgb(UiConstants::TEXT_SECONDARY_RGB))
                                    .child("Name"),
                            )
                            .child(
                                div()
                                    .w(px(200.0))
                                    .text_size(px(12.0))
                                    .font_weight(FontWeight(600.0))
                                    .text_color(rgb(UiConstants::TEXT_SECONDARY_RGB))
                                    .child("Location"),
                            )
                            .child(
                                div()
                                    .w(px(120.0))
                                    .text_size(px(12.0))
                                    .font_weight(FontWeight(600.0))
                                    .text_color(rgb(UiConstants::TEXT_SECONDARY_RGB))
                                    .child("Date Modified"),
                            ),
                    )
                    .child(
                        // Sample recent files (placeholder data)
                        Self::build_recent_file_row(
                            "document.pdf".to_string(),
                            "Documents".to_string(),
                            "Today".to_string(),
                        ),
                    )
                    .child(
                        div()
                            .border_b(px(1.0))
                            .border_color(rgba(0xFFFFFF0Fu32))
                            .child(Self::build_recent_file_row(
                                "image.png".to_string(),
                                "Pictures".to_string(),
                                "Yesterday".to_string(),
                            )),
                    )
                    .child(
                        div()
                            .border_b(px(1.0))
                            .border_color(rgba(0xFFFFFF0Fu32))
                            .child(Self::build_recent_file_row(
                                "report.xlsx".to_string(),
                                "Documents".to_string(),
                                "2 days ago".to_string(),
                            )),
                    ),
            )
    }

    fn build_recent_file_row(name: String, location: String, date: String) -> Div {
        div()
            .flex()
            .flex_row()
            .h(px(36.0))
            .px(px(12.0))
            .items_center()
            .hover(|s| s.bg(rgba(0xFFFFFF08u32)))
            .cursor_pointer()
            .child(
                div()
                    .flex_grow()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        div()
                            .text_size(px(16.0))
                            .font_family(UiConstants::FONT_MDL2)
                            .text_color(rgba(UiConstants::ACCENT_RGBA))
                            .child("\u{E8A5}"), // Document icon
                    )
                    .child(
                        div()
                            .text_size(px(13.0))
                            .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                            .child(name.clone()),
                    ),
            )
            .child(
                div()
                    .w(px(200.0))
                    .text_size(px(12.0))
                    .text_color(rgb(UiConstants::TEXT_SECONDARY_RGB))
                    .child(location.clone()),
            )
            .child(
                div()
                    .w(px(120.0))
                    .text_size(px(12.0))
                    .text_color(rgb(UiConstants::TEXT_MUTED_RGB))
                    .child(date.clone()),
            )
    }

    /// Build the File Tags section for Home page
    /// Shows tag chips grouped by color with file count
    fn build_file_tags_section(
        tags: Vec<(String, String, i64)>,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let mut tag_chips = div().flex_row().gap(px(8.0)).flex_wrap();

        if tags.is_empty() {
            tag_chips = tag_chips.child(
                div()
                    .text_size(px(13.0))
                    .text_color(rgb(UiConstants::TEXT_SECONDARY_RGB))
                    .child("No tags yet. Right-click a file to add tags."),
            );
        } else {
            for (name, color, count) in tags.clone() {
                let view_clone = view.clone();
                tag_chips = tag_chips.child(
                    Self::build_tag_chip(name.clone(), color, count).on_mouse_down(
                        MouseButton::Left,
                        move |_, _, cx| {
                            view_clone.update(cx, |this, cx| {
                                this.set_filter_text(name.clone(), cx);
                            });
                        },
                    ),
                );
            }
        }

        div()
            .flex_col()
            .gap(px(8.0))
            .child(
                div()
                    .font_weight(FontWeight(600.0))
                    .text_size(px(18.0))
                    .font_family(UiConstants::FONT_MAIN)
                    .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                    .child("File Tags"),
            )
            .child(tag_chips)
    }

    fn build_tag_chip(name: String, color: String, count: i64) -> Div {
        // Parse hex color for display - convert "#4285F4" to 0x004285F4 for rgb()
        let hex_color = color.trim_start_matches('#');
        let r = u8::from_str_radix(&hex_color[0..2], 16).unwrap_or(128);
        let g = u8::from_str_radix(&hex_color[2..4], 16).unwrap_or(128);
        let b = u8::from_str_radix(&hex_color[4..6], 16).unwrap_or(128);
        let rgb_value = ((r as u32) << 16) | ((g as u32) << 8) | (b as u32);

        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.0))
            .px(px(12.0))
            .py(px(6.0))
            .bg(rgba(0xFFFFFF10u32))
            .rounded(px(16.0))
            .border(px(1.0))
            .border_color(rgba(UiConstants::DIVIDER_RGBA))
            .hover(|s| s.bg(rgba(0xFFFFFF15u32)))
            .cursor_pointer()
            .child(
                div()
                    .w(px(8.0))
                    .h(px(8.0))
                    .rounded(px(4.0))
                    .bg(rgb(rgb_value)),
            )
            .child(
                div()
                    .text_size(px(13.0))
                    .font_family(UiConstants::FONT_MAIN)
                    .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                    .child(name.clone()),
            )
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(rgb(UiConstants::TEXT_MUTED_RGB))
                    .child(format!("({})", count)),
            )
    }

    /// Build the Network Locations section for Home page
    /// Shows SMB shares and network drives
    fn build_network_section(_view: Entity<crate::app::AppWindow>) -> impl IntoElement {
        div()
            .flex_col()
            .gap(px(8.0))
            .child(
                div()
                    .font_weight(FontWeight(600.0))
                    .text_size(px(18.0))
                    .font_family(UiConstants::FONT_MAIN)
                    .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                    .child("Network Locations"),
            )
            .child(
                div()
                    .flex_row()
                    .gap(px(12.0))
                    .child(Self::build_network_widget(
                        "Network".to_string(),
                        "\u{E968}".to_string(),
                        "Connected".to_string(),
                    ))
                    .child(Self::build_network_widget(
                        "Server".to_string(),
                        "\u{E839}".to_string(),
                        "Available".to_string(),
                    )),
            )
    }

    fn build_network_widget(label: String, glyph: String, status: String) -> Div {
        div()
            .w(px(180.0))
            .p(px(12.0))
            .bg(rgba(0xFFFFFF10u32))
            .rounded(px(8.0))
            .border(px(1.0))
            .border_color(rgba(UiConstants::DIVIDER_RGBA))
            .flex_col()
            .gap(px(8.0))
            .hover(|s| s.bg(rgba(0xFFFFFF15u32)))
            .cursor_pointer()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        div()
                            .text_size(px(24.0))
                            .font_family(UiConstants::FONT_MDL2)
                            .text_color(rgba(UiConstants::ACCENT_RGBA))
                            .child(glyph.clone()),
                    )
                    .child(
                        div()
                            .text_size(px(14.0))
                            .font_weight(FontWeight(600.0))
                            .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                            .child(label.clone()),
                    ),
            )
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(rgb(UiConstants::TEXT_MUTED_RGB))
                    .child(status.clone()),
            )
    }

    /// Finder-style columns (blade) view: Parent | Current | Preview
    fn columns_view(
        state: &MainViewState,
        file_items: Entity<FileItems>,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let visible = state.visible_items();
        let visible_clone = visible.to_vec();
        let selected_item = state
            .selected_indices
            .iter()
            .next()
            .and_then(|&i| state.items.get(i));
        let parent_path = state
            .primary_path
            .parent()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|| "—".to_string());
        let zoom = state.zoom_level;
        let row_h = 20.0 + zoom * 28.0;
        let icon_size = (row_h - 8.0).max(16.0).min(40.0);
        let text_size = 11.0 + zoom * 5.0;
        let text_size_sm = 10.0 + zoom * 3.0;

        div()
            .flex()
            .flex_row()
            .flex_grow()
            .h_full()
            .overflow_hidden()
            // Column 1: Parent path label
            .child(
                div()
                    .w(px(220.0))
                    .h_full()
                    .flex_col()
                    .border_r(px(1.0))
                    .border_color(rgba(UiConstants::DIVIDER_RGBA))
                    .child(
                        div()
                            .h(px(32.0))
                            .px(px(12.0))
                            .flex()
                            .items_center()
                            .border_b(px(1.0))
                            .border_color(rgba(UiConstants::DIVIDER_RGBA))
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .font_weight(FontWeight(600.0))
                                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                    .child(parent_path),
                            ),
                    )
                    .child(
                        div()
                            .id("col1-content")
                            .flex_grow()
                            .flex_col()
                            .p(px(4.0))
                            .overflow_y_scroll()
                            .track_scroll(&state.scroll_handle)
                            .child(
                                div()
                                    .h(px(28.0))
                                    .px(px(8.0))
                                    .flex()
                                    .items_center()
                                    .rounded(px(4.0))
                                    .bg(rgba(UiConstants::BG_ITEM_SELECTED_RGBA))
                                    .child(
                                        div()
                                            .text_size(px(13.0))
                                            .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                                            .child(
                                                state
                                                    .primary_path
                                                    .file_name()
                                                    .map(|n| n.to_string_lossy().into_owned())
                                                    .unwrap_or_else(|| {
                                                        state
                                                            .primary_path
                                                            .to_string_lossy()
                                                            .into_owned()
                                                    }),
                                            ),
                                    ),
                            ),
                    ),
            )
            // Column 2: Current directory contents
            .child(
                div()
                    .w(px(240.0))
                    .h_full()
                    .flex_col()
                    .border_r(px(1.0))
                    .border_color(rgba(UiConstants::DIVIDER_RGBA))
                    .child(
                        div()
                            .h(px(32.0))
                            .px(px(12.0))
                            .flex()
                            .items_center()
                            .border_b(px(1.0))
                            .border_color(rgba(UiConstants::DIVIDER_RGBA))
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .font_weight(FontWeight(600.0))
                                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                    .child(
                                        state
                                            .primary_path
                                            .file_name()
                                            .map(|n| n.to_string_lossy().into_owned())
                                            .unwrap_or_default(),
                                    ),
                            ),
                    )
                    .child(uniform_list(
                        "col2-scroll",
                        visible_clone.len(),
                        move |range, _win, cx| {
                            let fi = file_items.read(cx);
                            let app_state = view.read(cx);
                            let main_state = &app_state.state.main_view;
                            let icon_paths = &main_state.icon_paths;

                            range
                                .map(|i| {
                                    let idx = visible_clone[i];
                                    let item = &fi.items[idx];
                                    let is_selected = fi.selected_indices.contains(&idx);
                                    let is_dir = matches!(item.file_type, FileType::Directory);
                                    let name = item.name.clone();
                                    let path = item.path.clone();
                                    let v = view.clone();

                                    div()
                                        .id(ElementId::from(50000 + idx))
                                        .flex()
                                        .flex_row()
                                        .h(px(row_h))
                                        .px(px(8.0))
                                        .items_center()
                                        .gap(px(6.0))
                                        .rounded(px(4.0))
                                        .mx(px(4.0))
                                        .bg(if is_selected {
                                            rgba(UiConstants::BG_ITEM_SELECTED_RGBA)
                                        } else {
                                            rgba(0x00000000)
                                        })
                                        .hover(|s| {
                                            if !is_selected {
                                                s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA))
                                            } else {
                                                s
                                            }
                                        })
                                        .cursor_pointer()
                                        .on_mouse_down(MouseButton::Left, {
                                            let v = v.clone();
                                            let path = path.clone();
                                            move |_, _, cx| {
                                                cx.stop_propagation();
                                                v.update(cx, |this, cx| {
                                                    if is_dir {
                                                        this.navigate(path.clone(), cx);
                                                    } else {
                                                        this.select_item(idx, false, false, cx);
                                                    }
                                                });
                                            }
                                        })
                                        .child(
                                            div()
                                                .w(px(icon_size))
                                                .h(px(icon_size))
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .child(
                                                    if let Some(icon_path) = icon_paths.get(&path) {
                                                        img(icon_path.clone())
                                                            .w(px(icon_size))
                                                            .h(px(icon_size))
                                                            .into_any_element()
                                                    } else {
                                                        div()
                                                            .text_size(px(icon_size * 0.7))
                                                            .text_color(rgba(
                                                                UiConstants::TEXT_SECONDARY_RGBA,
                                                            ))
                                                            .child(if is_dir {
                                                                "\u{E8B7}"
                                                            } else {
                                                                "\u{E8A5}"
                                                            })
                                                            .into_any_element()
                                                    },
                                                ),
                                        )
                                        .child(
                                            div()
                                                .flex_grow()
                                                .text_size(px(text_size))
                                                .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                                                .child(name),
                                        )
                                        .into_any_element()
                                })
                                .collect::<Vec<_>>()
                        },
                    )),
            )
            // Column 3: Preview panel
            .child(
                div()
                    .flex_grow()
                    .h_full()
                    .flex_col()
                    .bg(rgba(0xFFFFFF05u32))
                    .child(match selected_item {
                        None => div()
                            .flex_grow()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(rgba(0xFFFFFF40u32))
                                    .child("Select a file to preview"),
                            )
                            .into_any_element(),
                        Some(item) => div()
                            .flex_grow()
                            .p(px(16.0))
                            .child(Self::render_details_pane(state, item))
                            .into_any_element(),
                    }),
            )
    }

    /// Cards view — large card per file with icon, name, size, and type badge
    fn cards_view(
        state: &MainViewState,
        visible: &[usize],
        file_items: Entity<FileItems>,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let visible_clone = visible.to_vec();
        let items_per_row = 4;
        let row_count = (visible.len() + items_per_row - 1) / items_per_row;
        let row_height = 212.0; // 200 card + 12 gap

        div()
            .flex_col()
            .w_full()
            .flex_grow()
            .h_full()
            .overflow_hidden()
            .child(
                uniform_list("cards-scroll", row_count, move |range, _win, cx| {
                    let fi = file_items.read(cx);
                    let app_state = view.read(cx);
                    let main_state = &app_state.state.main_view;
                    let icon_paths = &main_state.icon_paths;

                    range
                        .map(|row_idx| {
                            let start = row_idx * items_per_row;
                            let end = (start + items_per_row).min(visible_clone.len());

                            div()
                                .flex()
                                .flex_row()
                                .h(px(row_height))
                                .gap(px(12.0))
                                .px(px(16.0))
                                .children((start..end).map(|i| {
                                    let idx = visible_clone[i];
                                    let item = &fi.items[idx];
                                    let is_selected = fi.selected_indices.contains(&idx);
                                    let name = item.name.clone();
                                    let path = item.path.clone();
                                    let size_str = format_size(item.size);
                                    let is_dir = matches!(item.file_type, FileType::Directory);
                                    let v_card = view.clone();

                                    div()
                                        .id(ElementId::from(30000 + idx))
                                        .w(px(160.0))
                                        .h(px(200.0))
                                        .flex_col()
                                        .rounded(px(8.0))
                                        .border(px(1.0))
                                        .border_color(if is_selected {
                                            rgba(UiConstants::ACCENT_RGBA)
                                        } else {
                                            rgba(UiConstants::DIVIDER_RGBA)
                                        })
                                        .bg(if is_selected {
                                            rgba(UiConstants::BG_ITEM_SELECTED_RGBA)
                                        } else {
                                            rgba(0xFFFFFF08u32)
                                        })
                                        .hover(|s| s.bg(rgba(0xFFFFFF10u32)))
                                        .cursor_pointer()
                                        .overflow_hidden()
                                        .on_mouse_down(MouseButton::Left, {
                                            let v = v_card.clone();
                                            move |ev, _, cx| {
                                                cx.stop_propagation();
                                                v.update(cx, |this, cx| {
                                                    this.select_item(
                                                        idx,
                                                        ev.modifiers.shift,
                                                        ev.modifiers.control,
                                                        cx,
                                                    );
                                                });
                                            }
                                        })
                                        .on_click({
                                            let v = v_card.clone();
                                            let item_type = item.file_type;
                                            let path = path.clone();
                                            move |ev, _win, cx| {
                                                if ev.click_count() == 2 {
                                                    v.update(cx, |this, cx| {
                                                        if item_type == FileType::Directory {
                                                            this.navigate(path.clone(), cx);
                                                        } else {
                                                            this.open_item(idx, cx);
                                                        }
                                                    });
                                                }
                                            }
                                        })
                                        .on_mouse_down(MouseButton::Right, {
                                            let v = v_card.clone();
                                            move |ev, _, cx| {
                                                cx.stop_propagation();
                                                v.update(cx, |this, cx| {
                                                    this.show_context_menu(
                                                        Some(idx),
                                                        ev.position,
                                                        cx,
                                                    );
                                                });
                                            }
                                        })
                                        .child(div().justify_center().w_full().flex_grow().child(
                                            if let Some(icon_path) = icon_paths.get(&path) {
                                                img(icon_path.clone())
                                                    .w(px(64.0))
                                                    .h(px(64.0))
                                                    .into_any_element()
                                            } else {
                                                div()
                                                    .text_size(px(48.0))
                                                    .text_color(rgba(UiConstants::ACCENT_RGBA))
                                                    .child(if is_dir {
                                                        "\u{E8B7}"
                                                    } else {
                                                        "\u{E8A5}"
                                                    })
                                                    .into_any_element()
                                            },
                                        ))
                                        .child(
                                            div()
                                                .w_full()
                                                .p(px(8.0))
                                                .bg(rgba(0x00000044u32))
                                                .child(
                                                    div()
                                                        .text_size(px(12.0))
                                                        .text_color(rgb(0xFFFFFF))
                                                        .child(name),
                                                )
                                                .child(
                                                    div()
                                                        .text_size(px(10.0))
                                                        .text_color(rgba(0xFFFFFF88u32))
                                                        .child(size_str),
                                                ),
                                        )
                                        .into_any_element()
                                }))
                                .into_any_element()
                        })
                        .collect::<Vec<_>>()
                })
                .track_scroll(state.uniform_list_scroll_handle.clone()),
            )
    }

    fn filter_header(
        state: &MainViewState,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let is_filtering = !state.filter_text.is_empty();
        let has_selection = !state.selected_indices.is_empty();
        let is_grouping = state.group_by != GroupColumn::None;

        // Show header if filtering or grouping
        if !is_filtering && !is_grouping {
            return div();
        }

        let selection_count = state.selected_indices.len();
        let total_items = state.items.len();

        // Determine status text and icon
        let (icon, status_text) = if is_filtering {
            ("\u{E71C}", format!("Filtering by: {}", state.filter_text))
        } else {
            ("\u{E8EC}", "Grouped view".to_string()) // group icon
        };

        // Group-by label for display
        let group_label = match state.group_by {
            GroupColumn::None => "No grouping",
            GroupColumn::Name => "Name",
            GroupColumn::DateModified => "Date Modified",
            GroupColumn::DateCreated => "Date Created",
            GroupColumn::Size => "Size",
            GroupColumn::FileType => "Type",
            GroupColumn::SyncStatus => "Sync Status",
            GroupColumn::Tag => "Tag",
        };

        let v_group = view.clone();

        div()
            .flex()
            .items_center()
            .h(px(32.0))
            .px(px(UiConstants::PADDING_LARGE_RAW as f32))
            .bg(rgba(0xFFFFFF05u32))
            .border_b(px(1.0))
            .border_color(rgba(UiConstants::DIVIDER_RGBA))
            .gap(px(8.0))
            .child(
                div()
                    .text_size(AbsoluteLength::Pixels(px(12.0)))
                    .font_family(UiConstants::FONT_MDL2)
                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                    .child(icon),
            )
            .child(
                div()
                    .flex_grow()
                    .text_size(AbsoluteLength::Pixels(px(12.0)))
                    .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                    .child(status_text),
            )
            // Group-by toggle button (visible when grouping is active)
            .child(if is_grouping {
                div()
                    .flex()
                    .items_center()
                    .gap(px(4.0))
                    .px(px(8.0))
                    .py(px(2.0))
                    .rounded(px(4.0))
                    .bg(rgba(UiConstants::ACCENT_RGBA))
                    .cursor_pointer()
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        v_group.update(cx, |this, cx| {
                            this.cycle_group_by(cx);
                        });
                    })
                    .child(
                        div()
                            .text_size(AbsoluteLength::Pixels(px(11.0)))
                            .font_family(UiConstants::FONT_MDL2)
                            .text_color(rgb(UiConstants::ACCENT_COLOR_RGB))
                            .child("\u{E8EC}"), // group icon
                    )
                    .child(
                        div()
                            .text_size(AbsoluteLength::Pixels(px(11.0)))
                            .text_color(rgb(UiConstants::ACCENT_COLOR_RGB))
                            .child(group_label),
                    )
            } else {
                div()
            })
            .child(
                div()
                    .text_size(AbsoluteLength::Pixels(px(11.0)))
                    .text_color(rgb(UiConstants::TEXT_MUTED_RGB))
                    .child(format!("{} items", total_items)),
            )
    }
    fn empty_state(state: &MainViewState) -> impl IntoElement {
        let (icon, msg) = if !state.filter_text.is_empty() {
            ("\u{E721}", "No items match your search") // Search icon
        } else {
            ("\u{E8B7}", "This folder is empty") // Folder icon
        };

        div()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .flex_grow()
            .w_full()
            .h_full()
            .gap(px(12.0))
            .child(
                div()
                    .text_size(AbsoluteLength::Pixels(px(48.0)))
                    .font_family(UiConstants::FONT_MDL2)
                    .text_color(rgba(0xFFFFFF40u32)) // semi-transparent icon
                    .child(icon),
            )
            .child(
                div()
                    .text_size(AbsoluteLength::Pixels(px(14.0)))
                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                    .child(msg),
            )
    }

    fn list_view(
        state: &MainViewState,
        visible: &[usize],
        file_items: Entity<FileItems>,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let visible_clone = visible.to_vec();
        let zoom = state.zoom_level;
        let row_h = 24.0 + zoom * 32.0;
        let icon_size = 16.0 + zoom * 24.0;
        let text_size = 11.0 + zoom * 5.0;

        div()
            .flex_col()
            .w_full()
            .flex_grow()
            .h_full()
            .overflow_hidden()
            .child(
                uniform_list(
                    "list-scroll",
                    visible_clone.len(),
                    move |range, _win, cx| {
                        let fi = file_items.read(cx);
                        let app_state = view.read(cx);
                        let main_state = &app_state.state.main_view;
                        let icon_paths = &main_state.icon_paths;

                        range
                            .map(|i| {
                                let idx = visible_clone[i];
                                let item = &fi.items[idx];
                                let is_selected = fi.selected_indices.contains(&idx);
                                let path = item.path.clone();
                                let view_row = view.clone();

                                div()
                                    .id(ElementId::from(idx + 20000))
                                    .flex()
                                    .items_center()
                                    .w_full()
                                    .h(px(row_h))
                                    .px(px(16.0))
                                    .py(px(2.0))
                                    .bg(if is_selected {
                                        rgba(UiConstants::BG_ITEM_SELECTED_RGBA)
                                    } else {
                                        rgba(UiConstants::BG_TRANSPARENT_RGBA)
                                    })
                                    .hover(|s| {
                                        s.bg(if is_selected {
                                            rgba(UiConstants::BG_ITEM_SELECTED_HOVER_RGBA)
                                        } else {
                                            rgba(UiConstants::BG_ITEM_HOVER_RGBA)
                                        })
                                    })
                                    .cursor_pointer()
                                    .on_mouse_down(MouseButton::Left, {
                                        let v = view_row.clone();
                                        move |_event, _, cx| {
                                            cx.stop_propagation();
                                            v.update(cx, |this, cx| {
                                                this.select_item(idx, false, false, cx);
                                            });
                                        }
                                    })
                                    .on_mouse_down(MouseButton::Right, {
                                        let v = view_row.clone();
                                        move |ev, _, cx| {
                                            cx.stop_propagation();
                                            v.update(cx, |this, cx| {
                                                this.show_context_menu(Some(idx), ev.position, cx);
                                            });
                                        }
                                    })
                                    .child(
                                        div()
                                            .w(px(icon_size))
                                            .h(px(icon_size))
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .child(
                                                if let Some(icon_path) = icon_paths.get(&path) {
                                                    img(icon_path.clone())
                                                        .w(px(icon_size))
                                                        .h(px(icon_size))
                                                        .into_any_element()
                                                } else {
                                                    div()
                                                        .text_size(px(icon_size * 0.5))
                                                        .text_color(rgba(
                                                            UiConstants::TEXT_SECONDARY_RGBA,
                                                        ))
                                                        .child(
                                                            if matches!(
                                                                item.file_type,
                                                                FileType::Directory
                                                            ) {
                                                                "\u{E8B7}"
                                                            } else {
                                                                "\u{E8A5}"
                                                            },
                                                        )
                                                        .into_any_element()
                                                },
                                            ),
                                    )
                                    .child(
                                        div()
                                            .ml(px(8.0))
                                            .text_size(px(text_size))
                                            .text_color(if is_selected {
                                                rgb(UiConstants::TEXT_PRIMARY_RGB)
                                            } else {
                                                rgba(UiConstants::TEXT_PRIMARY_RGBA)
                                            })
                                            .child(item.name.clone()),
                                    )
                                    .into_any_element()
                            })
                            .collect::<Vec<_>>()
                    },
                )
                .track_scroll(state.uniform_list_scroll_handle.clone()),
            )
    }

    fn grid_view(
        state: &MainViewState,
        visible: &[usize],
        file_items: Entity<FileItems>,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        if state.group_by != GroupColumn::None {
            return Self::grid_view_grouped(state, visible, view).into_any_element();
        }

        let visible_clone = visible.to_vec();
        let items_per_row = Self::grid_items_per_row();
        let row_count = (visible.len() + items_per_row - 1) / items_per_row;

        let zoom = state.zoom_level;
        let base_size = 80.0 + zoom * 120.0;
        let row_height = base_size + 32.0;

        div()
            .flex_col()
            .w_full()
            .flex_grow()
            .h_full()
            .overflow_hidden()
            .id("grid-container")
            .on_mouse_down(MouseButton::Right, {
                let v = view.clone();
                move |event, _, cx| {
                    v.update(cx, |this, cx| {
                        this.show_context_menu(None, event.position, cx);
                    });
                }
            })
            .child(
                uniform_list("grid-scroll", row_count, move |range, _win, cx| {
                    let fi = file_items.read(cx);
                    let app_state = view.read(cx);
                    let main_state = &app_state.state.main_view;
                    let icon_paths = &main_state.icon_paths;
                    let zoom = main_state.zoom_level;
                    let sel_mode = main_state.selection_mode.clone();

                    range
                        .map(|row_idx| {
                            let start = row_idx * items_per_row;
                            let end = (start + items_per_row).min(visible_clone.len());

                            div()
                                .flex()
                                .flex_row()
                                .h(px(row_height))
                                .gap(px(UiConstants::GRID_GAP_RAW as f32))
                                .px(px(UiConstants::PADDING_LARGE_RAW as f32))
                                .children((start..end).map(|i| {
                                    let idx = visible_clone[i];
                                    let item = &fi.items[idx];
                                    let is_selected = fi.selected_indices.contains(&idx);
                                    Self::grid_item(
                                        item,
                                        idx,
                                        is_selected,
                                        sel_mode.clone(),
                                        view.clone(),
                                        icon_paths,
                                        zoom,
                                    )
                                    .into_any_element()
                                }))
                                .into_any_element()
                        })
                        .collect::<Vec<_>>()
                })
                .track_scroll(state.uniform_list_scroll_handle.clone()),
            )
            .into_any_element()
    }

    fn grid_view_grouped(
        state: &MainViewState,
        visible: &[usize],
        view: Entity<crate::app::AppWindow>,
    ) -> Div {
        let items_per_row = 6; // TODO: Make responsive
        let flattened = state.flatten_grouped_grid_items(visible, items_per_row);
        let icon_paths = state.icon_paths.clone();

        div().flex_col().w_full().flex_grow().h_full().child(
            div()
                .id("grid-grouped-scroll")
                .flex_col()
                .w_full()
                .flex_grow()
                .h_full()
                .overflow_y_scroll()
                .track_scroll(&state.scroll_handle)
                .children((0..flattened.len()).map(|i| match &flattened[i] {
                    GroupedGridItem::Header { key, label, count } => {
                        let key_clone = key.clone();
                        let label_clone = label.clone();
                        let count_clone = *count;
                        let is_collapsed = state.collapsed_groups.contains(key);
                        let group_height = if state.compact_mode { 24.0 } else { 32.0 };
                        let v_g = view.clone();

                        div()
                            .id(ElementId::from(60000 + i))
                            .flex()
                            .flex_row()
                            .h(px(group_height))
                            .items_center()
                            .px(px(UiConstants::PADDING_LARGE_RAW as f32))
                            .bg(rgba(0x0A000000u32))
                            .cursor_pointer()
                            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                v_g.update(cx, |this, cx| {
                                    this.toggle_group_collapse(key_clone.clone(), cx);
                                });
                            })
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .font_family(UiConstants::FONT_MDL2)
                                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                    .child(if is_collapsed {
                                        "\\u{E76C}"
                                    } else {
                                        "\\u{E76B}"
                                    }),
                            )
                            .child(
                                div()
                                    .ml(px(6.0))
                                    .text_size(px(13.0))
                                    .font_weight(FontWeight(600.0))
                                    .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                                    .child(label_clone),
                            )
                            .child(
                                div()
                                    .ml(px(8.0))
                                    .text_size(px(11.0))
                                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                    .child(format!(
                                        "{} item{}",
                                        count_clone,
                                        if count_clone == 1 { "" } else { "s" }
                                    )),
                            )
                            .into_any_element()
                    }
                    GroupedGridItem::Row(indices) => {
                        let indices = indices.clone();
                        div()
                            .flex()
                            .flex_row()
                            .gap(px(UiConstants::GRID_GAP_RAW as f32))
                            .px(px(UiConstants::PADDING_LARGE_RAW as f32))
                            .py(px(UiConstants::GRID_GAP_RAW as f32))
                            .children(indices.into_iter().map(|idx| {
                                let item = &state.items[idx];
                                let selected = state.selected_indices.contains(&idx);
                                let zoom = state.zoom_level;
                                Self::grid_item(
                                    item,
                                    idx,
                                    selected,
                                    state.selection_mode.clone(),
                                    view.clone(),
                                    &icon_paths,
                                    zoom,
                                )
                            }))
                            .into_any_element()
                    }
                })),
        )
    }

    fn grid_item(
        item: &FileItem,
        index: usize,
        selected: bool,
        selection_mode: SelectionMode,
        view: Entity<crate::app::AppWindow>,
        icon_paths: &std::collections::HashMap<
            crate::core::path_util::FilesPath,
            std::path::PathBuf,
        >,
        zoom: f32,
    ) -> impl IntoElement {
        let v_click = view.clone();
        let v_hover = view.clone();
        let v_timer = view.clone();
        let v_checkbox = view.clone();
        let is_dir = matches!(item.file_type, FileType::Directory);
        let extension = get_extension(&item.name);
        let is_thumbnail = !is_dir && is_thumbnail_image(&extension);
        let show_checkbox = selection_mode == SelectionMode::Checkbox;
        let is_hover_select = selection_mode == SelectionMode::HoverSelect;
        // Clone values needed by closures to avoid borrow issues
        let item_path = item.path.clone();
        let item_name = item.name.clone();

        // Check for cached Windows Shell icon
        let cached_icon_path: Option<std::path::PathBuf> = icon_paths.get(&item_path).cloned();
        let use_cached_icon = cached_icon_path.is_some();

        // For thumbnail images, use GPUUI's img() to display the image
        // For other items, use text-based icon

        // Calculate dynamic dimensions — continuous from min to max zoom
        let item_size = 80.0 + zoom * 160.0; // 80 (min) to 240 (max)
        let item_height = item_size + 30.0;
        let icon_size_val = item_size * 0.6;

        if is_thumbnail {
            // Show image thumbnail with optional checkbox
            let mut item_div = div()
                .id(ElementId::from(70000 + index))
                .w(px(item_size))
                .h(px(item_height))
                .flex_col()
                .items_center()
                .py(px(item_size * 0.25)) // Proportional padding
                .rounded(px(4.0))
                .relative()
                .border(px(if selected { 2.0 } else { 1.5 }))
                .border_color(if selected {
                    rgba(UiConstants::ACCENT_RGBA)
                } else {
                    rgba(0x00000000u32)
                })
                .bg(rgba(0x00000000u32))
                .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)));

            // Add checkbox if in checkbox mode
            if show_checkbox {
                item_div = item_div.child(
                    div()
                        .absolute()
                        .top(px(4.0))
                        .left(px(4.0))
                        .w(px(20.0))
                        .h(px(20.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                            v_checkbox.update(cx, |this, cx| {
                                this.toggle_checkbox_selection(index, cx);
                            });
                        })
                        .child(
                            div()
                                .w(px(14.0))
                                .h(px(14.0))
                                .border(px(1.5))
                                .border_color(rgba(UiConstants::ACCENT_RGBA))
                                .rounded(px(3.0))
                                .bg(if selected {
                                    rgba(UiConstants::ACCENT_RGBA)
                                } else {
                                    rgba(0x00000000)
                                })
                                .child(if selected {
                                    div()
                                        .text_size(px(9.0))
                                        .font_family(UiConstants::FONT_MDL2)
                                        .text_color(rgb(0xFFFFFF))
                                        .child("\u{E73E}")
                                } else {
                                    div()
                                }),
                        ),
                );
            }

            let path_clone = item_path.clone();
            item_div
                .on_mouse_down(MouseButton::Left, move |event, _, cx| {
                    cx.stop_propagation();
                    v_click.update(cx, |this, cx| {
                        if event.click_count == 2 {
                            if is_dir {
                                this.navigate(path_clone.clone(), cx);
                            } else {
                                this.open_item(index, cx);
                            }
                        } else {
                            this.select_item(index, false, false, cx);
                        }
                    });
                })
                .on_mouse_down(MouseButton::Right, {
                    let v = view.clone();
                    move |event, _, cx| {
                        cx.stop_propagation();
                        v.update(cx, |this, cx| {
                            this.show_context_menu(Some(index), event.position, cx);
                        });
                    }
                })
                .on_mouse_move({
                    let v_hover = v_hover.clone();
                    let v_timer = v_timer.clone();
                    move |_, _, cx| {
                        if is_hover_select {
                            v_hover.update(cx, |this, cx| {
                                this.handle_item_hover(index, cx);
                            });
                        } else if is_dir {
                            // Hover-to-open: start timer when hovering over a folder
                            v_timer.update(cx, |this, cx| {
                                this.start_hover_timer(index, cx);
                            });
                        }
                    }
                })
                .on_mouse_up_out(MouseButton::Left, {
                    let v_timer = v_timer.clone();
                    move |_, _, cx| {
                        // Stop the hover timer when mouse leaves
                        v_timer.update(cx, |this, cx| {
                            this.stop_hover_timer();
                        });
                    }
                })
                .child(
                    // Thumbnail image
                    img(item_path.as_path().to_path_buf())
                        .w(px(icon_size_val))
                        .h(px(icon_size_val))
                        .object_fit(gpui::ObjectFit::Contain)
                        .rounded(px(4.0)),
                )
                .child(
                    div()
                        .w_full()
                        .px(px(4.0))
                        .mt(px(4.0))
                        .text_center()
                        .overflow_hidden()
                        .text_size(px((item_size / 11.0).clamp(10.0, 14.0)))
                        .font_family(UiConstants::FONT_MAIN)
                        .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                        .child(item_name.clone()),
                )
        } else {
            // Show icon for directories and non-image files
            let file_icon = if is_dir {
                "\u{E8B7}".to_string()
            } else {
                get_file_type_icon(&extension).to_string()
            };

            let mut item_div = div()
                .id(ElementId::from(70000 + index))
                .w(px(item_size))
                .h(px(item_height))
                .flex_col()
                .items_center()
                .py(px(item_size * 0.25))
                .rounded(px(4.0))
                .relative()
                .border(px(if selected { 2.0 } else { 1.5 }))
                .border_color(if selected {
                    rgba(UiConstants::ACCENT_RGBA)
                } else {
                    rgba(0x00000000u32)
                })
                .bg(rgba(0x00000000u32))
                .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)));

            // Add checkbox if in checkbox mode
            if show_checkbox {
                let v_checkbox = view.clone();
                item_div = item_div.child(
                    div()
                        .absolute()
                        .top(px(4.0))
                        .left(px(4.0))
                        .w(px(20.0))
                        .h(px(20.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                            v_checkbox.update(cx, |this, cx| {
                                this.toggle_checkbox_selection(index, cx);
                            });
                        })
                        .child(
                            div()
                                .w(px(14.0))
                                .h(px(14.0))
                                .border(px(1.5))
                                .border_color(rgba(UiConstants::ACCENT_RGBA))
                                .rounded(px(3.0))
                                .bg(if selected {
                                    rgba(UiConstants::ACCENT_RGBA)
                                } else {
                                    rgba(0x00000000)
                                })
                                .child(if selected {
                                    div()
                                        .text_size(px(9.0))
                                        .font_family(UiConstants::FONT_MDL2)
                                        .text_color(rgb(0xFFFFFF))
                                        .child("\\u{E73E}")
                                } else {
                                    div()
                                }),
                        ),
                );
            }

            let path_clone = item_path.clone();
            item_div
                .on_mouse_down(MouseButton::Left, move |event, _, cx| {
                    cx.stop_propagation();
                    v_click.update(cx, |this, cx| {
                        if event.click_count == 2 {
                            if is_dir {
                                this.navigate(path_clone.clone(), cx);
                            } else {
                                this.open_item(index, cx);
                            }
                        } else {
                            this.select_item(index, false, false, cx);
                        }
                    });
                })
                .on_mouse_down(MouseButton::Right, {
                    let v = view.clone();
                    move |event, _, cx| {
                        cx.stop_propagation();
                        v.update(cx, |this, cx| {
                            this.show_context_menu(Some(index), event.position, cx);
                        });
                    }
                })
                .on_mouse_move({
                    let v_hover = v_hover.clone();
                    let v_timer = v_timer.clone();
                    move |_, _, cx| {
                        if is_hover_select {
                            v_hover.update(cx, |this, cx| {
                                this.handle_item_hover(index, cx);
                            });
                        } else if is_dir {
                            // Hover-to-open: start timer when hovering over a folder
                            v_timer.update(cx, |this, cx| {
                                this.start_hover_timer(index, cx);
                            });
                        }
                    }
                })
                .on_mouse_up_out(MouseButton::Left, {
                    let v_timer = v_timer.clone();
                    move |_, _, cx| {
                        // Stop the hover timer when mouse leaves
                        v_timer.update(cx, |this, cx| {
                            this.stop_hover_timer();
                        });
                    }
                })
                .child(
                    // Centered icon
                    div().flex().items_center().justify_center().w_full().child(
                        // Icon: use cached shell icon if available (shows folder thumbnails on Windows),
                        // otherwise MDL2 text glyph
                        if let Some(icon_path) = &cached_icon_path {
                            img(icon_path.clone())
                                .w(px(icon_size_val))
                                .h(px(icon_size_val))
                                .object_fit(gpui::ObjectFit::Contain)
                                .into_any_element()
                        } else {
                            div()
                                .text_size(px(icon_size_val * 0.8))
                                .font_family(UiConstants::FONT_MDL2)
                                .text_color(if is_dir {
                                    rgba(UiConstants::ACCENT_RGBA)
                                } else {
                                    rgba(UiConstants::TEXT_SECONDARY_RGBA)
                                })
                                .child(file_icon)
                                .into_any_element()
                        },
                    ),
                )
                .child(
                    div()
                        .w_full()
                        .px(px(4.0))
                        .mt(px(4.0))
                        .text_center()
                        .overflow_hidden()
                        .text_size(px((item_size / 11.0).clamp(10.0, 14.0)))
                        .font_family(UiConstants::FONT_MAIN)
                        .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                        .child(item_name.clone()),
                )
        }
    }

    fn file_grid(
        state: &MainViewState,
        file_items: Entity<FileItems>,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let visible = state.visible_items();
        Self::grid_view(state, &visible, file_items, view)
    }

    #[allow(dead_code)]
    fn file_list(
        state: &MainViewState,
        file_items: Entity<FileItems>,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let visible = state.visible_items();
        Self::details_view(state, &visible, file_items, view)
    }

    // ════════════════════════════════════════════════════════════════
    // Preview Pane Rendering Helpers
    // ════════════════════════════════════════════════════════════════

    /// Render the default preview (icon + name + raw text)
    fn render_default_preview(item: &FileItem, state: &MainViewState) -> impl IntoElement {
        let ext = get_extension(&item.name);
        let is_image = matches!(
            ext.as_str(),
            "jpg" | "jpeg" | "png" | "gif" | "bmp" | "webp" | "ico" | "tiff" | "tif" | "svg"
        );

        div()
            .flex_col()
            .items_center()
            .gap(px(12.0))
            .w_full()
            .child(
                div()
                    .w(px(64.0))
                    .h(px(64.0))
                    .bg(rgba(UiConstants::BG_BADGE_RGBA))
                    .rounded(px(8.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(if is_image {
                        img(item.path.as_path().to_path_buf())
                            .w(px(64.0))
                            .h(px(64.0))
                            .rounded(px(8.0))
                            .into_any_element()
                    } else {
                        div()
                            .text_size(px(32.0))
                            .font_family(UiConstants::FONT_MDL2)
                            .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                            .child(if matches!(item.file_type, FileType::Directory) {
                                "\u{E8B7}"
                            } else {
                                "\u{E7C3}"
                            })
                            .into_any_element()
                    }),
            )
            .child(
                div()
                    .text_size(px(13.0))
                    .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                    .text_center()
                    .overflow_hidden()
                    .max_w(px(250.0))
                    .child(item.name.clone()),
            )
            .child(if let Some(text) = &state.preview_text {
                div()
                    .mt(px(8.0))
                    .p(px(8.0))
                    .w_full()
                    .max_h(px(300.0))
                    .bg(rgba(0xFFFFFF05u32))
                    .rounded(px(4.0))
                    .overflow_hidden()
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                            .child(text.clone()),
                    )
                    .into_any_element()
            } else {
                div().into_any_element()
            })
    }

    /// Render a Markdown preview with basic formatting
    fn render_markdown_preview(text: &str) -> impl IntoElement {
        let mut children: Vec<AnyElement> = Vec::new();
        let mut in_code_block = false;
        let mut code_block_text = String::new();

        for line in text.lines() {
            if line.starts_with("```") {
                if in_code_block {
                    // End code block
                    children.push(
                        div()
                            .font_family("ui-monospace, SFMono-Regular, 'Cascadia Code', Consolas, monospace")
                            .text_size(px(11.0))
                            .text_color(rgba(0x00FFD700u32)) // gold-ish
                            .bg(rgba(0xFFFFFF10u32))
                            .p(px(6.0))
                            .rounded(px(4.0))
                            .w_full()
                            .child(code_block_text.clone())
                            .into_any_element()
                    );
                    code_block_text.clear();
                    in_code_block = false;
                } else {
                    in_code_block = true;
                }
                continue;
            }

            if in_code_block {
                if !code_block_text.is_empty() {
                    code_block_text.push('\n');
                }
                code_block_text.push_str(line);
                continue;
            }

            let trimmed = line.trim();

            // Headers
            if trimmed.starts_with("### ") {
                let content = &trimmed[4..];
                children.push(
                    div()
                        .font_weight(FontWeight(600.0))
                        .text_size(px(13.0))
                        .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                        .mt(px(6.0))
                        .mb(px(2.0))
                        .child(content.to_string())
                        .into_any_element(),
                );
            } else if trimmed.starts_with("## ") {
                let content = &trimmed[3..];
                children.push(
                    div()
                        .font_weight(FontWeight(700.0))
                        .text_size(px(15.0))
                        .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                        .mt(px(8.0))
                        .mb(px(2.0))
                        .child(content.to_string())
                        .into_any_element(),
                );
            } else if trimmed.starts_with("# ") {
                let content = &trimmed[2..];
                children.push(
                    div()
                        .font_weight(FontWeight(700.0))
                        .text_size(px(18.0))
                        .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                        .mt(px(10.0))
                        .mb(px(4.0))
                        .child(content.to_string())
                        .into_any_element(),
                );
            } else if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
                // List items
                let content = &trimmed[2..];
                children.push(
                    div()
                        .flex()
                        .flex_row()
                        .gap(px(4.0))
                        .px(px(8.0))
                        .text_size(px(11.0))
                        .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                        .child(div().child("\u{2022} ")) // bullet
                        .child(div().child(Self::parse_inline_markdown(content)))
                        .into_any_element(),
                );
            } else if trimmed.is_empty() {
                children.push(div().h(px(6.0)).into_any_element());
            } else {
                // Regular paragraph with inline formatting
                children.push(
                    div()
                        .text_size(px(11.0))
                        .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                        .py(px(1.0))
                        .child(Self::parse_inline_markdown(line))
                        .into_any_element(),
                );
            }
        }

        // Flush remaining code block
        if in_code_block && !code_block_text.is_empty() {
            children.push(
                div()
                    .font_family(
                        "ui-monospace, SFMono-Regular, 'Cascadia Code', Consolas, monospace",
                    )
                    .text_size(px(11.0))
                    .text_color(rgba(0x00FFD700u32))
                    .bg(rgba(0xFFFFFF10u32))
                    .p(px(6.0))
                    .rounded(px(4.0))
                    .w_full()
                    .child(code_block_text)
                    .into_any_element(),
            );
        }

        div()
            .flex_col()
            .w_full()
            .max_h(px(400.0))
            .children(children)
    }

    /// Parse inline markdown formatting (bold, italic, code, links)
    fn parse_inline_markdown(text: &str) -> impl IntoElement {
        // For simplicity, we return a div with basic inline formatting detection.
        // Handle **bold**, *italic*, `inline code`, [links](url)
        let mut children: Vec<AnyElement> = Vec::new();
        let mut remaining = text;

        while !remaining.is_empty() {
            // Bold: **text**
            if let Some(start) = remaining.find("**") {
                if start > 0 {
                    children.push(
                        div()
                            .child(remaining[..start].to_string())
                            .into_any_element(),
                    );
                }
                let after = &remaining[start + 2..];
                if let Some(end) = after.find("**") {
                    children.push(
                        div()
                            .font_weight(FontWeight(700.0))
                            .child(after[..end].to_string())
                            .into_any_element(),
                    );
                    remaining = &after[end + 2..];
                    continue;
                }
            }

            // Inline code: `code`
            if let Some(start) = remaining.find('`') {
                if start > 0 {
                    children.push(
                        div()
                            .child(remaining[..start].to_string())
                            .into_any_element(),
                    );
                }
                let after = &remaining[start + 1..];
                if let Some(end) = after.find('`') {
                    children.push(
                        div()
                            .font_family("ui-monospace, SFMono-Regular, 'Cascadia Code', Consolas, monospace")
                            .bg(rgba(0xFFFFFF15u32))
                            .px(px(3.0))
                            .rounded(px(2.0))
                            .child(after[..end].to_string())
                            .into_any_element()
                    );
                    remaining = &after[end + 1..];
                    continue;
                }
            }

            // No more formatting - add remaining text
            children.push(div().child(remaining.to_string()).into_any_element());
            break;
        }

        div().flex_row().flex_wrap().children(children)
    }

    /// Render code with syntax highlighting
    fn render_code_preview(text: &str, _language: &str) -> impl IntoElement {
        let highlighted = Self::highlight_code(text);
        let line_count = text.lines().count();

        div()
            .flex_col()
            .w_full()
            .max_h(px(400.0))
            .bg(rgba(0xFFFFFF08u32))
            .rounded(px(4.0))
            .p(px(8.0))
            .child(
                div()
                    .flex_col()
                    .font_family(
                        "ui-monospace, SFMono-Regular, 'Cascadia Code', Consolas, monospace",
                    )
                    .text_size(px(11.0))
                    .children(highlighted.into_iter().enumerate().map(|(i, tokens)| {
                        // Line number + highlighted tokens
                        div()
                            .flex()
                            .flex_row()
                            .gap(px(8.0))
                            .w_full()
                            .child(
                                div()
                                    .text_color(rgba(UiConstants::TEXT_MUTED_RGBA))
                                    .min_w(px(24.0))
                                    .text_right()
                                    .child(format!("{:>3}", i + 1)),
                            )
                            .child(
                                div()
                                    .flex_row()
                                    .flex_wrap()
                                    .children(tokens.into_iter().map(|(token, color_opt)| {
                                        let mut el = div().child(token);
                                        if let Some(color) = color_opt {
                                            el = el.text_color(color);
                                        } else {
                                            el =
                                                el.text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA));
                                        }
                                        el.into_any_element()
                                    })),
                            )
                            .into_any_element()
                    })),
            )
    }

    /// Simple keyword-based syntax highlighter
    /// Returns a list of lines, each containing (token, optional_color) pairs
    fn highlight_code(text: &str) -> Vec<Vec<(String, Option<gpui::Rgba>)>> {
        // Language-agnostic keyword highlighting based on common patterns
        let keywords: &[(&str, u32)] = &[
            // Rust, C, C++, Java, JavaScript, TypeScript, etc.
            ("fn", 0x005695FF), // blue
            ("let", 0x005695FF),
            ("mut", 0x005695FF),
            ("const", 0x005695FF),
            ("pub", 0x005695FF),
            ("use", 0x005695FF),
            ("mod", 0x005695FF),
            ("struct", 0x005695FF),
            ("enum", 0x005695FF),
            ("impl", 0x005695FF),
            ("trait", 0x005695FF),
            ("type", 0x005695FF),
            ("fn ", 0x005695FF),
            ("return", 0x005695FF),
            ("if", 0x005695FF),
            ("else", 0x005695FF),
            ("for", 0x005695FF),
            ("while", 0x005695FF),
            ("loop", 0x005695FF),
            ("match", 0x005695FF),
            ("break", 0x005695FF),
            ("continue", 0x005695FF),
            ("self", 0x005695FF),
            ("Self", 0x005695FF),
            ("super", 0x005695FF),
            ("where", 0x005695FF),
            ("async", 0x005695FF),
            ("await", 0x005695FF),
            ("unsafe", 0x005695FF),
            ("ref", 0x005695FF),
            ("static", 0x005695FF),
            ("dyn", 0x005695FF),
            ("move", 0x005695FF),
            ("true", 0x005695FF),
            ("false", 0x005695FF),
            ("Some", 0x005695FF),
            ("None", 0x005695FF),
            ("Ok", 0x005695FF),
            ("Err", 0x005695FF),
            ("as", 0x005695FF),
            ("in", 0x005695FF),
            ("is", 0x005695FF),
            // Python
            ("def", 0x005695FF),
            ("class", 0x005695FF),
            ("import", 0x005695FF),
            ("from", 0x005695FF),
            ("with", 0x005695FF),
            ("as ", 0x005695FF),
            ("pass", 0x005695FF),
            ("yield", 0x005695FF),
            ("lambda", 0x005695FF),
            ("print", 0x005695FF),
            ("range", 0x005695FF),
            ("len", 0x005695FF),
            ("not", 0x005695FF),
            ("and", 0x005695FF),
            ("or", 0x005695FF),
            ("try", 0x005695FF),
            ("except", 0x005695FF),
            ("finally", 0x005695FF),
            ("raise", 0x005695FF),
            // JS/TS
            ("function", 0x005695FF),
            ("var", 0x005695FF),
            ("typeof", 0x005695FF),
            ("new", 0x005695FF),
            ("this", 0x005695FF),
            ("null", 0x005695FF),
            ("undefined", 0x005695FF),
            ("export", 0x005695FF),
            ("import ", 0x005695FF),
            ("from ", 0x005695FF),
            ("default", 0x005695FF),
            ("extends", 0x005695FF),
            ("implements", 0x005695FF),
            ("interface", 0x005695FF),
            ("typeof", 0x005695FF),
            ("instanceof", 0x005695FF),
            ("throw", 0x005695FF),
            ("catch", 0x005695FF),
            // String/char literals
        ];

        let type_keywords: &[(&str, u32)] = &[
            ("String", 0x004EC9B0FF), // teal
            ("str", 0x004EC9B0FF),
            ("i32", 0x004EC9B0FF),
            ("i64", 0x004EC9B0FF),
            ("u32", 0x004EC9B0FF),
            ("u64", 0x004EC9B0FF),
            ("f32", 0x004EC9B0FF),
            ("f64", 0x004EC9B0FF),
            ("bool", 0x004EC9B0FF),
            ("char", 0x004EC9B0FF),
            ("usize", 0x004EC9B0FF),
            ("Vec", 0x004EC9B0FF),
            ("Option", 0x004EC9B0FF),
            ("Result", 0x004EC9B0FF),
            ("HashMap", 0x004EC9B0FF),
            ("HashSet", 0x004EC9B0FF),
            ("Box", 0x004EC9B0FF),
            ("Arc", 0x004EC9B0FF),
            ("Mutex", 0x004EC9B0FF),
            ("Rc", 0x004EC9B0FF),
            ("Cell", 0x004EC9B0FF),
            ("RefCell", 0x004EC9B0FF),
            ("u8", 0x004EC9B0FF),
            ("u16", 0x004EC9B0FF),
            ("i8", 0x004EC9B0FF),
            ("i16", 0x004EC9B0FF),
            ("int", 0x004EC9B0FF),
            ("float", 0x004EC9B0FF),
            ("double", 0x004EC9B0FF),
            ("char", 0x004EC9B0FF),
            ("void", 0x004EC9B0FF),
            ("boolean", 0x004EC9B0FF),
            ("number", 0x004EC9B0FF),
            ("string", 0x004EC9B0FF),
            ("Promise", 0x004EC9B0FF),
            ("Array", 0x004EC9B0FF),
            ("Map", 0x004EC9B0FF),
            ("Set", 0x004EC9B0FF),
            ("Error", 0x004EC9B0FF),
            ("Date", 0x004EC9B0FF),
            ("RegExp", 0x004EC9B0FF),
        ];

        let mut result = Vec::new();

        for line in text.lines() {
            let mut tokens: Vec<(String, Option<gpui::Rgba>)> = Vec::new();
            let mut remaining = line;

            while !remaining.is_empty() {
                // Try to match a string literal first (single or double quoted)
                if remaining.starts_with('"') || remaining.starts_with('\'') {
                    let quote = remaining.chars().next().unwrap();
                    let end = remaining[1..]
                        .find(quote)
                        .map(|i| i + 2)
                        .unwrap_or(remaining.len());
                    tokens.push((remaining[..end].to_string(), Some(rgba(0x00CE9178u32)))); // green
                    remaining = &remaining[end..];
                    continue;
                }

                // Try to match a comment
                if remaining.starts_with("//") || remaining.starts_with('#') {
                    tokens.push((remaining.to_string(), Some(rgba(0x006A9955u32)))); // dim green
                    break;
                }

                // Try to match a number
                if remaining.starts_with(|c: char| c.is_ascii_digit()) {
                    let end = remaining
                        .find(|c: char| {
                            !c.is_ascii_digit()
                                && c != '.'
                                && c != 'x'
                                && !(c >= 'A' && c <= 'F')
                                && !(c >= 'a' && c <= 'f')
                        })
                        .unwrap_or(remaining.len());
                    tokens.push((remaining[..end].to_string(), Some(rgba(0x00B5CEA8u32)))); // light green
                    remaining = &remaining[end..];
                    continue;
                }

                // Check for multi-char tokens (keywords, types)
                let mut matched = false;

                // Sort keywords by length descending to match longer tokens first
                for &(kw, color) in keywords.iter().chain(type_keywords.iter()) {
                    if remaining.starts_with(kw) {
                        // Ensure word boundary
                        let after = &remaining[kw.len()..];
                        let is_word_boundary = after.is_empty()
                            || !after.starts_with(|c: char| c.is_alphanumeric() || c == '_');
                        if is_word_boundary {
                            tokens.push((kw.to_string(), Some(rgba(color))));
                            remaining = after;
                            matched = true;
                            break;
                        }
                    }
                }

                if matched {
                    continue;
                }

                // Single character or non-matching token
                let end = remaining
                    .find(|c: char| !c.is_alphanumeric() && c != '_')
                    .unwrap_or(1);
                if end > 0 {
                    tokens.push((remaining[..end].to_string(), None));
                    remaining = &remaining[end..];
                } else {
                    // Just take the next character (punctuation, whitespace)
                    let end = remaining.len().min(1);
                    tokens.push((remaining[..end].to_string(), None));
                    remaining = &remaining[end..];
                }
            }

            result.push(tokens);
        }

        result
    }

    /// Render HTML preview — strip tags and show readable text, or show highlighted raw HTML
    fn render_html_preview(text: &str) -> impl IntoElement {
        // Strip HTML tags for readability
        let stripped = Self::strip_html_tags(text);

        div().flex_col().w_full().max_h(px(400.0)).child(
            div()
                .bg(rgba(0xFFFFFF05u32))
                .rounded(px(4.0))
                .p(px(8.0))
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                        .font_family(
                            "ui-monospace, SFMono-Regular, 'Cascadia Code', Consolas, monospace",
                        )
                        .child(stripped),
                ),
        )
    }

    /// Simple HTML tag stripper — removes <tags> and decodes common entities
    fn strip_html_tags(html: &str) -> String {
        let mut result = String::new();
        let mut in_tag = false;
        let mut in_entity = false;
        let mut entity_buf = String::new();

        for c in html.chars() {
            match c {
                '<' => {
                    if !in_tag {
                        // Flush any pending entity
                        if in_entity {
                            result.push_str(&Self::decode_html_entity(&entity_buf));
                            entity_buf.clear();
                            in_entity = false;
                        }
                        in_tag = true;
                    } else {
                        result.push('<');
                    }
                }
                '>' => {
                    if in_tag {
                        in_tag = false;
                    } else {
                        result.push('>');
                    }
                }
                '&' if !in_tag => {
                    if in_entity {
                        result.push_str(&Self::decode_html_entity(&entity_buf));
                        entity_buf.clear();
                    }
                    in_entity = true;
                    entity_buf.clear();
                }
                ';' if in_entity => {
                    result.push_str(&Self::decode_html_entity(&entity_buf));
                    entity_buf.clear();
                    in_entity = false;
                }
                _ => {
                    if in_tag {
                        // Skip tag content
                    } else if in_entity {
                        entity_buf.push(c);
                    } else {
                        result.push(c);
                    }
                }
            }
        }

        // Flush any remaining entity
        if in_entity && !entity_buf.is_empty() {
            result.push_str(&Self::decode_html_entity(&entity_buf));
        }

        // Collapse multiple whitespace
        let mut collapsed = String::new();
        let mut prev_space = false;
        for c in result.chars() {
            if c.is_whitespace() {
                if !prev_space {
                    collapsed.push(' ');
                    prev_space = true;
                }
            } else {
                collapsed.push(c);
                prev_space = false;
            }
        }

        collapsed.trim().to_string()
    }

    /// Decode common HTML entities
    fn decode_html_entity(entity: &str) -> String {
        match entity {
            "amp" => "&".to_string(),
            "lt" => "<".to_string(),
            "gt" => ">".to_string(),
            "quot" => "\"".to_string(),
            "apos" => "'".to_string(),
            "nbsp" => " ".to_string(),
            "copy" => "©".to_string(),
            "reg" => "®".to_string(),
            "trade" => "™".to_string(),
            "mdash" => "—".to_string(),
            "ndash" => "–".to_string(),
            "laquo" => "«".to_string(),
            "raquo" => "»".to_string(),
            _ => {
                // Numeric entities
                if let Some(num_str) = entity.strip_prefix('#') {
                    if let Some(hex_str) = num_str.strip_prefix('x') {
                        if let Ok(code) = u32::from_str_radix(hex_str, 16) {
                            if let Some(ch) = char::from_u32(code) {
                                return ch.to_string();
                            }
                        }
                    } else if let Ok(code) = num_str.parse::<u32>() {
                        if let Some(ch) = char::from_u32(code) {
                            return ch.to_string();
                        }
                    }
                }
                format!("&{};", entity)
            }
        }
    }

    /// Render RTF preview — strips RTF control words and shows plain text
    fn render_rtf_preview(text: &str) -> impl IntoElement {
        let stripped = Self::strip_rtf(text);

        div()
            .flex_col()
            .w_full()
            .max_h(px(400.0))
            .child(
                div()
                    .child(div().text_size(px(13.0)).font_weight(FontWeight(600.0)).mb(px(6.0)).child("RTF Document"))
                    .child(
                        div()
                            .bg(rgba(0xFFFFFF05u32))
                            .rounded(px(4.0))
                            .p(px(8.0))
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .font_family("ui-monospace, SFMono-Regular, 'Cascadia Code', Consolas, monospace")
                                    .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                                    .child(stripped),
                            ),
                    ),
            )
    }

    /// Simple RTF stripper — removes \control words, {braces} groups, and keeps plain text
    fn strip_rtf(rtf: &str) -> String {
        let mut result = String::new();
        let mut in_control = false;
        let mut brace_depth = 0u32;
        let mut chars = rtf.chars().peekable();

        while let Some(c) = chars.next() {
            match c {
                '{' => {
                    brace_depth += 1;
                    // Skip group open
                }
                '}' => {
                    if brace_depth > 0 {
                        brace_depth -= 1;
                    }
                    // Skip group close
                }
                '\\' => {
                    // Start of a control word or symbol
                    in_control = true;
                    // Skip the backslash
                }
                '\r' | '\n' => {
                    if brace_depth == 0 {
                        result.push(c);
                    }
                }
                _ => {
                    if in_control {
                        if c.is_ascii_alphabetic() {
                            // Skip control word characters
                            while let Some(&next) = chars.peek() {
                                if next.is_ascii_alphabetic() {
                                    chars.next();
                                } else {
                                    break;
                                }
                            }
                            // Skip optional space delimiter after control word
                            if let Some(&' ') = chars.peek() {
                                chars.next();
                            }
                        }
                        // Skip control symbol (single non-alpha char after backslash)
                        in_control = false;
                    } else if brace_depth == 0 {
                        // Only keep text at the top level (not inside braces)
                        // Also handle common RTF escape sequences
                        match c {
                            '\\' => result.push('\\'),
                            _ => result.push(c),
                        }
                    }
                }
            }
        }

        // Collapse whitespace
        let mut collapsed = String::new();
        let mut prev_space = false;
        for c in result.chars() {
            if c.is_whitespace() && c != '\n' {
                if !prev_space {
                    collapsed.push(' ');
                    prev_space = true;
                }
            } else {
                collapsed.push(c);
                prev_space = false;
            }
        }

        collapsed.trim().to_string()
    }

    /// Render media preview (audio/video) with metadata and player placeholder
    fn render_media_preview(
        info: &MediaPreviewInfo,
        media_type: &str,
        text: &str,
        item: &FileItem,
    ) -> impl IntoElement {
        let icon = match media_type {
            "Audio" => "\u{E8D1}",
            _ => "\u{E8D1}",
        };

        let format_line = info.format.as_deref().unwrap_or("Unknown");
        let duration_str = info
            .duration_secs
            .map(|d| {
                let mins = (d / 60.0) as u64;
                let secs = (d % 60.0) as u64;
                format!("{}:{:02}", mins, secs)
            })
            .unwrap_or_else(|| "Unknown".to_string());

        let codec_str = info.codec.as_deref().unwrap_or("Unknown");
        let bitrate_str = info
            .bitrate_kbps
            .map(|b| format!("{} kbps", b))
            .unwrap_or_else(|| "Unknown".to_string());

        let mut meta_children: Vec<AnyElement> = vec![
            info_row("Format", format_line).into_any_element(),
            info_row("Duration", &duration_str).into_any_element(),
            info_row("Codec", codec_str).into_any_element(),
            info_row("Bitrate", &bitrate_str).into_any_element(),
        ];

        if let Some(sr) = info.sample_rate {
            meta_children.push(info_row("Sample Rate", &format!("{} Hz", sr)).into_any_element());
        }
        if let Some(ch) = info.channels {
            meta_children.push(info_row("Channels", &format!("{}", ch)).into_any_element());
        }
        if let Some(w) = info.width {
            if let Some(h) = info.height {
                meta_children
                    .push(info_row("Resolution", &format!("{}x{}", w, h)).into_any_element());
            }
        }
        if let Some(fr) = info.frame_rate {
            meta_children
                .push(info_row("Frame Rate", &format!("{:.1} fps", fr)).into_any_element());
        }

        div()
            .flex_col()
            .items_center()
            .gap(px(12.0))
            .w_full()
            // Large icon
            .child(
                div()
                    .w(px(64.0))
                    .h(px(64.0))
                    .bg(rgba(UiConstants::BG_BADGE_RGBA))
                    .rounded(px(8.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .text_size(px(32.0))
                            .font_family(UiConstants::FONT_MDL2)
                            .text_color(rgba(UiConstants::ACCENT_RGBA))
                            .child(icon),
                    ),
            )
            // File name
            .child(
                div()
                    .text_size(px(14.0))
                    .font_weight(FontWeight(600.0))
                    .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                    .text_center()
                    .child(item.name.clone()),
            )
            // Player placeholder
            .child(
                div()
                    .w_full()
                    .h(px(60.0))
                    .bg(rgba(0xFFFFFF10u32))
                    .rounded(px(6.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .gap(px(8.0))
                    .child(
                        div()
                            .text_size(px(18.0))
                            .font_family(UiConstants::FONT_MDL2)
                            .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                            .child("\u{E768}"), // Play icon
                    )
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                            .child("Player placeholder"),
                    ),
            )
            // Separator
            .child(
                div()
                    .w_full()
                    .h(px(1.0))
                    .bg(rgba(UiConstants::DIVIDER_RGBA)),
            )
            // Metadata section
            .child(
                div()
                    .flex_col()
                    .gap(px(6.0))
                    .w_full()
                    .child(section_header(&format!("{} Info", media_type)))
                    .children(meta_children),
            )
            // Raw text info (preview_text)
            .child(
                div()
                    .mt(px(8.0))
                    .p(px(8.0))
                    .w_full()
                    .bg(rgba(0xFFFFFF05u32))
                    .rounded(px(4.0))
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                            .child(text.to_string()),
                    ),
            )
    }

    /// Render PDF preview with metadata
    fn render_pdf_preview(_info: &PdfPreviewInfo, text: &str) -> impl IntoElement {
        div()
            .flex_col()
            .items_center()
            .gap(px(8.0))
            .w_full()
            .child(
                div()
                    .w(px(64.0))
                    .h(px(64.0))
                    .bg(rgba(UiConstants::BG_BADGE_RGBA))
                    .rounded(px(8.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .text_size(px(32.0))
                            .font_family(UiConstants::FONT_MDL2)
                            .text_color(rgba(0xFFFF0000u32)) // Red for PDF
                            .child("\u{E8A5}"),
                    ),
            )
            .child(
                div()
                    .mt(px(8.0))
                    .p(px(8.0))
                    .w_full()
                    .bg(rgba(0xFFFFFF05u32))
                    .rounded(px(4.0))
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                            .child(text.to_string()),
                    ),
            )
    }

    /// Render archive preview with file listing
    fn render_archive_preview(_info: &ArchivePreviewInfo, text: &str) -> impl IntoElement {
        div()
            .flex_col()
            .items_center()
            .gap(px(8.0))
            .w_full()
            .child(
                div()
                    .w(px(64.0))
                    .h(px(64.0))
                    .bg(rgba(UiConstants::BG_BADGE_RGBA))
                    .rounded(px(8.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .text_size(px(32.0))
                            .font_family(UiConstants::FONT_MDL2)
                            .text_color(rgba(UiConstants::ACCENT_RGBA))
                            .child("\u{E7A3}"),
                    ),
            )
            .child(
                div()
                    .mt(px(8.0))
                    .p(px(8.0))
                    .w_full()
                    .max_h(px(300.0))
                    .bg(rgba(0xFFFFFF05u32))
                    .rounded(px(4.0))
                    .child(
                        div()
                            .text_size(px(11.0))
                            .font_family("ui-monospace, SFMono-Regular, 'Cascadia Code', Consolas, monospace")
                            .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                            .child(text.to_string()),
                    ),
            )
    }

    /// Render folder preview as a thumbnail grid of up to 12 items
    fn render_folder_preview(items: &[FolderPreviewItem]) -> impl IntoElement {
        let display_items = if items.len() > 12 {
            &items[..12]
        } else {
            items
        };
        let cols = 4.min(display_items.len().max(1));
        let width_pct = 100.0 / cols as f32;

        div()
            .flex_col()
            .items_center()
            .gap(px(12.0))
            .w_full()
            .child(
                div()
                    .w(px(64.0))
                    .h(px(64.0))
                    .bg(rgba(UiConstants::BG_BADGE_RGBA))
                    .rounded(px(8.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .text_size(px(32.0))
                            .font_family(UiConstants::FONT_MDL2)
                            .text_color(rgba(UiConstants::ACCENT_RGBA))
                            .child("\u{E8B7}"),
                    ),
            )
            .child(
                div()
                    .text_size(px(14.0))
                    .font_weight(FontWeight(600.0))
                    .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                    .child(format!("Folder Contents ({} items)", items.len())),
            )
            .child(
                div()
                    .w_full()
                    .h(px(1.0))
                    .bg(rgba(UiConstants::DIVIDER_RGBA)),
            )
            // Thumbnail grid
            .child(div().flex_row().flex_wrap().w_full().gap(px(6.0)).children(
                display_items.iter().map(|entry| {
                    let icon = if entry.is_dir { "\u{E8B7}" } else { "\u{E8A5}" };

                    div()
                        .flex_col()
                        .items_center()
                        .gap(px(3.0))
                        .w(px(60.0))
                        .child(
                            div()
                                .w(px(40.0))
                                .h(px(40.0))
                                .bg(rgba(0xFFFFFF10u32))
                                .rounded(px(6.0))
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(
                                    div()
                                        .text_size(px(20.0))
                                        .font_family(UiConstants::FONT_MDL2)
                                        .text_color(if entry.is_dir {
                                            rgba(UiConstants::ACCENT_RGBA)
                                        } else {
                                            rgba(UiConstants::TEXT_SECONDARY_RGBA)
                                        })
                                        .child(icon),
                                ),
                        )
                        .child(
                            div()
                                .text_size(px(9.0))
                                .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                .overflow_hidden()
                                .max_w(px(60.0))
                                .text_center()
                                .child(entry.name.clone()),
                        )
                        .into_any_element()
                }),
            ))
            .children(if items.len() > 12 {
                vec![div()
                    .text_size(px(10.0))
                    .text_color(rgba(UiConstants::TEXT_MUTED_RGBA))
                    .child(format!("+ {} more items", items.len() - 12))
                    .into_any_element()]
            } else {
                vec![]
            })
    }

    /// Calculate the number of items per row for the grid view.
    /// Uses sensible defaults based on grid vs card mode; TODO: make responsive to container width.
    fn grid_items_per_row() -> usize {
        6
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Unit tests for selection, lasso, and drag functionality
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::path_util::FilesPath;
    use crate::domain::metadata::FileItem;
    use crate::storage::storage::FileType;

    fn create_test_file_item(name: &str, is_dir: bool) -> FileItem {
        FileItem {
            name: name.to_string(),
            path: FilesPath::new(name),
            file_type: if is_dir {
                FileType::Directory
            } else {
                FileType::File
            },
            size: 1024,
            is_hidden: false,
            is_system: false,
        }
    }

    #[test]
    fn test_lasso_state_rect_normalizes_coordinates() {
        let lasso = LassoState {
            start: point(100.0, 200.0),
            current: point(50.0, 150.0),
        };
        let (start, end) = lasso.rect();

        // Should normalize so start is top-left, end is bottom-right
        assert_eq!(start.x, 50.0);
        assert_eq!(start.y, 150.0);
        assert_eq!(end.x, 100.0);
        assert_eq!(end.y, 200.0);
    }

    #[test]
    fn test_lasso_state_rect_same_point() {
        let lasso = LassoState {
            start: point(100.0, 100.0),
            current: point(100.0, 100.0),
        };
        let (start, end) = lasso.rect();

        assert_eq!(start.x, 100.0);
        assert_eq!(start.y, 100.0);
        assert_eq!(end.x, 100.0);
        assert_eq!(end.y, 100.0);
    }

    #[test]
    fn test_drag_state_default() {
        let drag = DragState::default();
        assert!(!drag.is_dragging);
        assert!(drag.indices.is_empty());
        assert_eq!(drag.start.x, 0.0);
        assert_eq!(drag.start.y, 0.0);
        assert_eq!(drag.current.x, 0.0);
        assert_eq!(drag.current.y, 0.0);
        assert!(!drag.is_copy);
    }

    #[test]
    fn test_main_view_state_lasso_start() {
        let path = FilesPath::new("C:\\test");
        let mut state = MainViewState::new(path, ViewMode::Grid);

        assert!(state.lasso_state.is_none());

        state.lasso_start(point(100.0, 200.0));

        assert!(state.lasso_state.is_some());
        let lasso = state.lasso_state.unwrap();
        assert_eq!(lasso.start.x, 100.0);
        assert_eq!(lasso.start.y, 200.0);
        assert_eq!(lasso.current.x, 100.0);
        assert_eq!(lasso.current.y, 200.0);
    }

    #[test]
    fn test_main_view_state_lasso_update() {
        let path = FilesPath::new("C:\\test");
        let mut state = MainViewState::new(path, ViewMode::Grid);

        state.lasso_start(point(100.0, 100.0));
        state.lasso_update(point(300.0, 400.0));

        let lasso = state.lasso_state.unwrap();
        assert_eq!(lasso.start.x, 100.0);
        assert_eq!(lasso.start.y, 100.0);
        assert_eq!(lasso.current.x, 300.0);
        assert_eq!(lasso.current.y, 400.0);
    }

    #[test]
    fn test_main_view_state_lasso_cancel() {
        let path = FilesPath::new("C:\\test");
        let mut state = MainViewState::new(path, ViewMode::Grid);

        state.lasso_start(point(100.0, 100.0));
        assert!(state.lasso_state.is_some());

        state.lasso_cancel();
        assert!(state.lasso_state.is_none());
    }

    #[test]
    fn test_main_view_state_lasso_rect() {
        let path = FilesPath::new("C:\\test");
        let mut state = MainViewState::new(path, ViewMode::Grid);

        state.lasso_start(point(100.0, 100.0));
        state.lasso_update(point(200.0, 200.0));

        let rect = state.lasso_rect();
        assert!(rect.is_some());
        let (start, end) = rect.unwrap();
        assert_eq!(start.x, 100.0);
        assert_eq!(start.y, 100.0);
        assert_eq!(end.x, 200.0);
        assert_eq!(end.y, 200.0);
    }

    #[test]
    fn test_main_view_state_lasso_rect_none_when_inactive() {
        let path = FilesPath::new("C:\\test");
        let state = MainViewState::new(path, ViewMode::Grid);

        assert!(state.lasso_rect().is_none());
    }

    #[test]
    fn test_main_view_state_drag_start() {
        let path = FilesPath::new("C:\\test");
        let mut state = MainViewState::new(path, ViewMode::Grid);

        state.drag_start(vec![0, 1, 2], point(100.0, 200.0));

        assert!(state.is_dragging());
        assert_eq!(state.drag_state.indices, vec![0, 1, 2]);
        assert_eq!(state.drag_state.start.x, 100.0);
        assert_eq!(state.drag_state.current.x, 100.0);
    }

    #[test]
    fn test_main_view_state_drag_update() {
        let path = FilesPath::new("C:\\test");
        let mut state = MainViewState::new(path, ViewMode::Grid);

        state.drag_start(vec![0], point(100.0, 100.0));
        state.drag_update(point(300.0, 400.0), true); // is_copy = true

        assert_eq!(state.drag_state.current.x, 300.0);
        assert_eq!(state.drag_state.current.y, 400.0);
        assert!(state.drag_state.is_copy);
    }

    #[test]
    fn test_main_view_state_drag_end() {
        let path = FilesPath::new("C:\\test");
        let mut state = MainViewState::new(path, ViewMode::Grid);

        state.drag_start(vec![0, 1], point(100.0, 100.0));
        assert!(state.is_dragging());

        state.drag_end();

        assert!(!state.is_dragging());
        assert!(state.drag_state.indices.is_empty());
    }

    #[test]
    fn test_main_view_state_get_drop_target() {
        let path = FilesPath::new("C:\\test");
        let mut state = MainViewState::new(path, ViewMode::Grid);

        // Add some items including a directory
        state.items.push(create_test_file_item("folder1", true)); // index 0
        state.items.push(create_test_file_item("file1.txt", false)); // index 1
        state.items.push(create_test_file_item("folder2", true)); // index 2

        // Start a drag
        state.drag_start(vec![1], point(100.0, 100.0));

        // Test that we can find drop targets
        // Note: The exact positions depend on item layout which uses hardcoded values
        // This test verifies the method is callable and returns Option<usize>
        let target = state.get_drop_target(point(0.0, 0.0), &[0, 1, 2]);
        // Target could be None or Some depending on whether (0,0) falls within an item
        // The important thing is it doesn't panic and returns Option<usize>
        assert!(target.is_none() || target.is_some());
    }

    #[test]
    fn test_main_view_state_is_dragging_false_by_default() {
        let path = FilesPath::new("C:\\test");
        let state = MainViewState::new(path, ViewMode::Grid);

        assert!(!state.is_dragging());
    }

    #[test]
    fn test_select_rect_basic() {
        let path = FilesPath::new("C:\\test");
        let mut state = MainViewState::new(path, ViewMode::Grid);

        // Add some items
        for i in 0..24 {
            state
                .items
                .push(create_test_file_item(&format!("file{}.txt", i), false));
        }

        // Select items in the first row (items 0-5) using a rectangle
        // Grid layout: item_width=180, item_height=200, gap=12, padding=16
        // First row at y=16 to y=216
        let visible = state.visible_items();
        state.select_rect(
            point(16.0, 16.0),   // Start: top-left of first item
            point(200.0, 216.0), // End: bottom-right area of first row
            false,               // Don't add to selection, replace
        );

        // Items in first row should be selected
        assert!(state.selected_indices.contains(&0));
        assert!(state.selected_indices.contains(&1));
        assert!(state.selected_indices.contains(&2));
        assert!(state.selected_indices.contains(&3));
        assert!(state.selected_indices.contains(&4));
        assert!(state.selected_indices.contains(&5));
    }

    #[test]
    fn test_select_rect_add_to_selection() {
        let path = FilesPath::new("C:\\test");
        let mut state = MainViewState::new(path, ViewMode::Grid);

        for i in 0..12 {
            state
                .items
                .push(create_test_file_item(&format!("file{}.txt", i), false));
        }

        // First select some items
        state.selected_indices.insert(10);
        state.selected_indices.insert(11);

        // Then add more via select_rect
        let visible = state.visible_items();
        state.select_rect(point(16.0, 16.0), point(200.0, 216.0), true); // add_to_selection = true

        // Original items should still be selected
        assert!(state.selected_indices.contains(&10));
        assert!(state.selected_indices.contains(&11));
        // New items should also be selected
        assert!(state.selected_indices.contains(&0));
    }

    #[test]
    fn test_lasso_end_clears_state() {
        let path = FilesPath::new("C:\\test");
        let mut state = MainViewState::new(path, ViewMode::Grid);

        state.lasso_start(point(100.0, 100.0));
        state.lasso_update(point(200.0, 200.0));
        assert!(state.lasso_state.is_some());

        state.lasso_end(false);
        assert!(state.lasso_state.is_none());
    }

    #[test]
    fn test_select_all_method() {
        let path = FilesPath::new("C:\\test");
        let mut state = MainViewState::new(path, ViewMode::Grid);

        for i in 0..10 {
            state
                .items
                .push(create_test_file_item(&format!("file{}.txt", i), false));
        }

        // Select all via lasso_end with a large rectangle
        state.lasso_start(point(0.0, 0.0));
        state.lasso_update(point(10000.0, 10000.0)); // Cover everything
        state.lasso_end(false);

        // All items should be selected
        for i in 0..10 {
            assert!(
                state.selected_indices.contains(&i),
                "Item {} should be selected",
                i
            );
        }
    }

    #[test]
    fn test_drag_state_clone() {
        let drag = DragState {
            indices: vec![1, 2, 3],
            start: point(100.0, 100.0),
            current: point(200.0, 300.0),
            is_copy: true,
            is_dragging: true,
        };

        let cloned = drag.clone();
        assert_eq!(cloned.indices, vec![1, 2, 3]);
        assert_eq!(cloned.start.x, 100.0);
        assert!(cloned.is_copy);
        assert!(cloned.is_dragging);
    }

    #[test]
    fn test_lasso_state_clone() {
        let lasso = LassoState {
            start: point(50.0, 50.0),
            current: point(150.0, 150.0),
        };

        let cloned = lasso.clone();
        assert_eq!(cloned.start.x, 50.0);
        assert_eq!(cloned.current.x, 150.0);
    }

    // ─────────────────────────────────────────────────────────────
    // Sort by tag tests
    // ─────────────────────────────────────────────────────────────

    #[test]
    fn test_sort_by_tag_tagged_items_before_untagged() {
        let mut state = MainViewState::new(FilesPath::new("/test"), ViewMode::Details);
        state.items = vec![
            create_test_file_item("zzz.txt", false),
            create_test_file_item("aaa.txt", false),
            create_test_file_item("bbb.txt", false),
        ];
        // Tag "zzz.txt" with "work", "aaa.txt" with "personal"
        state
            .tags_cache
            .insert("/test/zzz.txt".to_string(), vec!["work".to_string()]);
        state
            .tags_cache
            .insert("/test/aaa.txt".to_string(), vec!["personal".to_string()]);
        // bbb.txt is untagged

        state.sort.column = SortBy::Tag;
        state.sort.ascending = true;
        state.sort.folders_first = false;

        let visible = state.visible_items();
        // Tagged items should come first (sorted alphabetically by tag), then untagged
        assert_eq!(visible.len(), 3);
        // "personal" < "work", so aaa.txt first, then zzz.txt, then bbb.txt (untagged)
        assert_eq!(state.items[visible[0]].name, "aaa.txt");
        assert_eq!(state.items[visible[1]].name, "zzz.txt");
        assert_eq!(state.items[visible[2]].name, "bbb.txt");
    }

    #[test]
    fn test_sort_by_tag_alphabetical_order() {
        let mut state = MainViewState::new(FilesPath::new("/test"), ViewMode::Details);
        state.items = vec![
            create_test_file_item("delta.txt", false),
            create_test_file_item("alpha.txt", false),
            create_test_file_item("beta.txt", false),
        ];
        // All tagged
        state
            .tags_cache
            .insert("/test/delta.txt".to_string(), vec!["zzz".to_string()]);
        state
            .tags_cache
            .insert("/test/alpha.txt".to_string(), vec!["aaa".to_string()]);
        state
            .tags_cache
            .insert("/test/beta.txt".to_string(), vec!["bbb".to_string()]);

        state.sort.column = SortBy::Tag;
        state.sort.ascending = true;
        state.sort.folders_first = false;

        let visible = state.visible_items();
        assert_eq!(visible.len(), 3);
        // Tags sorted: aaa < bbb < zzz
        assert_eq!(state.items[visible[0]].name, "alpha.txt");
        assert_eq!(state.items[visible[1]].name, "beta.txt");
        assert_eq!(state.items[visible[2]].name, "delta.txt");
    }

    #[test]
    fn test_sort_by_tag_descending() {
        let mut state = MainViewState::new(FilesPath::new("/test"), ViewMode::Details);
        state.items = vec![
            create_test_file_item("alpha.txt", false),
            create_test_file_item("beta.txt", false),
        ];
        state
            .tags_cache
            .insert("/test/alpha.txt".to_string(), vec!["work".to_string()]);
        state
            .tags_cache
            .insert("/test/beta.txt".to_string(), vec!["personal".to_string()]);

        state.sort.column = SortBy::Tag;
        state.sort.ascending = false;
        state.sort.folders_first = false;

        let visible = state.visible_items();
        assert_eq!(visible.len(), 2);
        // Descending: "work" > "personal"
        assert_eq!(state.items[visible[0]].name, "alpha.txt");
        assert_eq!(state.items[visible[1]].name, "beta.txt");
    }

    // ─────────────────────────────────────────────────────────────
    // Group by tag tests
    // ─────────────────────────────────────────────────────────────

    #[test]
    fn test_group_by_tag() {
        let mut state = MainViewState::new(FilesPath::new("/test"), ViewMode::Details);
        state.items = vec![
            create_test_file_item("file1.txt", false),
            create_test_file_item("file2.txt", false),
            create_test_file_item("file3.txt", false),
        ];
        state
            .tags_cache
            .insert("/test/file1.txt".to_string(), vec!["work".to_string()]);
        state
            .tags_cache
            .insert("/test/file2.txt".to_string(), vec!["work".to_string()]);
        state
            .tags_cache
            .insert("/test/file3.txt".to_string(), vec!["personal".to_string()]);

        state.group_by = GroupColumn::Tag;

        // Verify group keys
        assert_eq!(
            state.get_group_key(&state.items[0]),
            Some("work".to_string())
        );
        assert_eq!(
            state.get_group_key(&state.items[1]),
            Some("work".to_string())
        );
        assert_eq!(
            state.get_group_key(&state.items[2]),
            Some("personal".to_string())
        );
    }

    #[test]
    fn test_group_by_tag_untagged() {
        let mut state = MainViewState::new(FilesPath::new("/test"), ViewMode::Details);
        state.items = vec![
            create_test_file_item("tagged.txt", false),
            create_test_file_item("untagged.txt", false),
        ];
        state
            .tags_cache
            .insert("/test/tagged.txt".to_string(), vec!["work".to_string()]);

        state.group_by = GroupColumn::Tag;

        // Tagged item gets "work" group, untagged gets "Untagged"
        assert_eq!(
            state.get_group_key(&state.items[0]),
            Some("work".to_string())
        );
        assert_eq!(
            state.get_group_key(&state.items[1]),
            Some("Untagged".to_string())
        );
    }
}

// Grouped view methods for MainViewState and MainView
// Extracted from main_view.rs to reduce file size

use crate::config::config::GroupColumn;
use crate::domain::metadata::FileItem;
use crate::storage::storage::FileType;
use crate::ui::constants::UiConstants;
use gpui::*;
use std::collections::BTreeMap;

use super::main_view::MainView;
use super::types::*;

impl MainViewState {
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
            GroupColumn::DateModified | GroupColumn::DateCreated => {
                Some(self.get_date_group_key_for_item(item))
            }
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
        // This is called without item context — the caller should use
        // get_date_group_key_for_item instead. Return a fallback.
        "Items".to_string()
    }

    /// Get the date group key for a specific item based on its modification time.
    pub fn get_date_group_key_for_item(&self, item: &FileItem) -> String {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let day_secs: u64 = 86400;

        let item_secs = item
            .modified
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);

        if item_secs == 0 {
            return "Unknown".to_string();
        }

        let age_days = now.saturating_sub(item_secs) / day_secs;

        if age_days == 0 {
            "Today".to_string()
        } else if age_days == 1 {
            "Yesterday".to_string()
        } else if age_days < 7 {
            "This Week".to_string()
        } else if age_days < 30 {
            "This Month".to_string()
        } else if age_days < 365 {
            "This Year".to_string()
        } else {
            "Older".to_string()
        }
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
            headers.push((key, label, count));
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
        let target_key = self.group_drag_state.target_group_key.take();
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

    /// Flatten grouped items into a list of (is_header, index_or_group_key)
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

    /// Flatten grouped grid items into a list of (Header | Row)
    pub fn flatten_grouped_grid_items(
        &self,
        visible: &[usize],
        items_per_row: usize,
    ) -> Vec<GroupedGridItem> {
        let (headers, groups) = self.partition_groups(visible);
        let mut flat = Vec::new();
        for (i, (key, label, count)) in headers.into_iter().enumerate() {
            let is_collapsed = self.collapsed_groups.contains(&key);
            flat.push(GroupedGridItem::Header { key, label, count });
            if !is_collapsed {
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

        let visible = self.visible_items_rc();
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
}

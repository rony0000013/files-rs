use crate::core::path_util::FilesPath;
use anyhow::Result;
use std::collections::HashSet;

pub struct SelectionManager {
    selected: HashSet<FilesPath>,
}

impl SelectionManager {
    pub fn new() -> Self {
        Self {
            selected: HashSet::new(),
        }
    }

    pub fn select_all(&mut self, all_items: &[FilesPath]) {
        self.selected = all_items.iter().cloned().collect();
    }

    pub fn clear_selection(&mut self) {
        self.selected.clear();
    }

    pub fn invert_selection(&mut self, all_items: &[FilesPath]) {
        let current = self.selected.clone();
        self.selected.clear();
        for item in all_items {
            if !current.contains(item) {
                self.selected.insert(item.clone());
            }
        }
    }

    pub fn toggle_select(&mut self, item: FilesPath) {
        if self.selected.contains(&item) {
            self.selected.remove(&item);
        } else {
            self.selected.insert(item);
        }
    }

    pub fn is_selected(&self, item: &FilesPath) -> bool {
        self.selected.contains(item)
    }

    pub fn get_selected(&self) -> Vec<FilesPath> {
        self.selected.iter().cloned().collect()
    }
}

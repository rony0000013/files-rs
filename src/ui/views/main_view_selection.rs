//! Selection-related methods for MainViewState.
//!
//! Extracted from main_view.rs for maintainability:
//! - Lasso (rubber-band) selection
//! - Drag and drop operations
//! - Rectangular selection helpers

use gpui::*;

use crate::config::config::ViewMode;
use crate::storage::storage::FileType;

use super::main_view::MainViewState;
use super::types::{DragState, LassoState};

impl MainViewState {
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

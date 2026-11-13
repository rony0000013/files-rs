//! Selection & Drag-and-Drop methods for `MainViewState`.
//!
//! Provides:
//! - View-mode-aware Lasso (rubber-band) selection across `Details`, `List`, `Cards`, `Grid`, and `Columns` views
//! - Live selection updates during mouse drag (accounting for scroll offset, grouped layouts, and additive Ctrl/Shift)
//! - Internal + Out-of-Window Drag-and-Drop state management (`DragState`)
//! - Shared layout metrics for `List` and `Cards` (Tiles) views

use gpui::*;
use std::collections::HashSet;

use crate::config::config::{GroupColumn, ViewMode};
use crate::core::navigation_manager::PaneFocus;
use crate::core::path_util::FilesPath;
use crate::storage::storage::FileType;

use super::main_view::MainViewState;
use super::types::{rects_intersect, DragState, LassoState};

/// Pixel movement threshold to distinguish a click from a file/folder drag gesture.
pub const FILE_DRAG_THRESHOLD_PX: f32 = 6.0;

impl MainViewState {
    // ─────────────────────────────────────────────────────────────────────────
    // Lasso (rubber-band) selection
    // ─────────────────────────────────────────────────────────────────────────

    /// Start a lasso selection at the given point (in local coordinates).
    pub fn lasso_start(&mut self, point: Point<f32>) {
        self.lasso_state = Some(LassoState {
            start: point,
            current: point,
            pane_origin: gpui::point(0.0, 0.0),
            additive: false,
            initial_selection: HashSet::new(),
        });
    }

    /// Start a lasso selection from window-space mouse coordinates with pane origin
    /// and optional additive (`Ctrl`/`Shift`) modifier.
    pub fn lasso_start_at_window_pos(
        &mut self,
        window_pos: Point<f32>,
        pane_origin: Point<f32>,
        additive: bool,
    ) {
        let local = point(
            (window_pos.x - pane_origin.x).max(0.0),
            (window_pos.y - pane_origin.y).max(0.0),
        );
        let initial_selection = if additive {
            self.selected_indices.clone()
        } else {
            self.selected_indices.clear();
            HashSet::new()
        };
        self.lasso_state = Some(LassoState {
            start: local,
            current: local,
            pane_origin,
            additive,
            initial_selection,
        });
    }

    /// Update the current lasso selection endpoint (local coordinates).
    pub fn lasso_update(&mut self, point: Point<f32>) {
        if let Some(ref mut state) = self.lasso_state {
            state.current = point;
        }
    }

    /// Update the current lasso selection from a window-space mouse position
    /// and immediately recompute `selected_indices` for live rubber-band highlighting.
    pub fn lasso_update_at_window_pos(&mut self, window_pos: Point<f32>, pane_w: f32) -> bool {
        let Some((rect, additive, initial)) = self.lasso_state.as_mut().map(|lasso| {
            lasso.current = point(
                (window_pos.x - lasso.pane_origin.x).max(0.0),
                (window_pos.y - lasso.pane_origin.y).max(0.0),
            );
            (
                lasso.rect(),
                lasso.additive,
                lasso.initial_selection.clone(),
            )
        }) else {
            return false;
        };

        let dx = (rect.1.x - rect.0.x).abs();
        let dy = (rect.1.y - rect.0.y).abs();
        if dx <= super::types::LASSO_MIN_DELTA && dy <= super::types::LASSO_MIN_DELTA {
            return false;
        }

        let scroll_y = {
            let (curr_y, _, _, _) =
                crate::ui::components::bouncy_scroll::uniformlist_scroll_info(
                    &self.uniform_list_scroll_handle,
                );
            (-curr_y).max(0.0)
        };

        let content_rect = (
            point(rect.0.x, rect.0.y + scroll_y),
            point(rect.1.x, rect.1.y + scroll_y),
        );

        self.apply_rect_selection_for_view(content_rect, pane_w, additive, &initial)
    }

    /// Compute intersecting items for the current `view_mode` given a rectangle
    /// in scroll-content coordinates. Respects grouped header offsets and row breaks
    /// when `self.group_by != GroupColumn::None`.
    /// Returns `true` if `self.selected_indices` changed.
    pub fn apply_rect_selection_for_view(
        &mut self,
        content_rect: (Point<f32>, Point<f32>),
        pane_w: f32,
        additive: bool,
        initial_selection: &HashSet<usize>,
    ) -> bool {
        let mut next_selected = if additive {
            initial_selection.clone()
        } else {
            HashSet::new()
        };

        let effective_w = if pane_w > 50.0 { pane_w } else { 1000.0 };

        if self.group_by != GroupColumn::None {
            for (idx, (x0, y0, x1, y1)) in self.grouped_item_bounds_for_view(effective_w) {
                let item_rect = (point(x0, y0), point(x1, y1));
                if rects_intersect(item_rect, content_rect) {
                    next_selected.insert(idx);
                }
            }
        } else {
            let visible = self.visible_items_rc();
            match self.view_mode {
                ViewMode::Details | ViewMode::Tree => {
                    let base_row_h = if self.compact_mode { 22.0 } else { 26.0 };
                    let row_h = (base_row_h + self.zoom_level * 22.0).round();
                    for (i, &vi) in visible.iter().enumerate() {
                        let y0 = i as f32 * row_h;
                        let item_rect = (point(0.0, y0), point(effective_w, y0 + row_h));
                        if rects_intersect(item_rect, content_rect) {
                            next_selected.insert(vi);
                        }
                    }
                }
                ViewMode::Columns => {
                    let base_r_h = if self.compact_mode { 20.0 } else { 22.0 };
                    let row_h = (base_r_h + self.zoom_level * 22.0).round();
                    let zoom_scale = 0.80 + self.zoom_level * 0.55;
                    let col2_w = (self.column_widths.miller_col2 * zoom_scale)
                        .clamp(160.0, (effective_w * 0.55).max(240.0));
                    for (i, &vi) in visible.iter().enumerate() {
                        let y0 = i as f32 * row_h;
                        let item_rect = (point(0.0, y0), point(col2_w, y0 + row_h));
                        if rects_intersect(item_rect, content_rect) {
                            next_selected.insert(vi);
                        }
                    }
                }
                ViewMode::List => {
                    let (cols, col_w, row_h, gap, padding) =
                        Self::list_layout_metrics(effective_w, self.zoom_level, self.compact_mode);
                    let cols_safe = cols.max(1);
                    for (i, &vi) in visible.iter().enumerate() {
                        let row = i / cols_safe;
                        let col = i % cols_safe;
                        let x0 = padding + col as f32 * (col_w + gap);
                        let y0 = row as f32 * row_h;
                        let item_rect = (point(x0, y0), point(x0 + col_w, y0 + row_h));
                        if rects_intersect(item_rect, content_rect) {
                            next_selected.insert(vi);
                        }
                    }
                }
                ViewMode::Cards => {
                    let (cols, card_w, row_h, gap, padding) =
                        Self::cards_layout_metrics(effective_w, self.zoom_level, self.compact_mode);
                    let cols_safe = cols.max(1);
                    for (i, &vi) in visible.iter().enumerate() {
                        let row = i / cols_safe;
                        let col = i % cols_safe;
                        let x0 = padding + col as f32 * (card_w + gap);
                        let y0 = row as f32 * row_h;
                        let item_rect = (point(x0, y0), point(x0 + card_w, y0 + row_h));
                        if rects_intersect(item_rect, content_rect) {
                            next_selected.insert(vi);
                        }
                    }
                }
                ViewMode::Grid => {
                    let cols = Self::grid_columns_for_width(effective_w, self.zoom_level);
                    let params =
                        Self::grid_layout_params_for_width_pub(effective_w, self.zoom_level, cols);
                    let item_h = if self.compact_mode && self.zoom_level >= 0.22 {
                        params.item_height - 4.0
                    } else {
                        params.item_height
                    };
                    for (i, &vi) in visible.iter().enumerate() {
                        let row = i / params.items_per_row.max(1);
                        let col = i % params.items_per_row.max(1);
                        let x0 = params.padding + col as f32 * (params.item_width + params.gap);
                        let y0 = row as f32 * params.row_height;
                        let item_rect = (
                            point(x0, y0),
                            point(x0 + params.item_width, y0 + item_h),
                        );
                        if rects_intersect(item_rect, content_rect) {
                            next_selected.insert(vi);
                        }
                    }
                }
            }
        }

        if let Some(&first) = next_selected.iter().next() {
            if self.last_clicked_index.is_none() {
                self.last_clicked_index = Some(first);
            }
        }
        let changed = self.selected_indices != next_selected;
        if changed {
            self.selected_indices = next_selected;
        }
        changed
    }

    /// Multi-column list layout metrics shared between `ListView` rendering,
    /// rubber-band lasso hit-testing, and 2D keyboard navigation.
    /// Reserves vertical height for up to 3 wrapped lines of filename text so cells never overlap.
    /// Returns `(cols, col_width, row_height, gap, horizontal_padding)`.
    pub fn list_layout_metrics(
        pane_w: f32,
        zoom: f32,
        compact: bool,
    ) -> (usize, f32, f32, f32, f32) {
        let z = zoom.clamp(0.0, 1.0);
        let text_size = (10.0 + z * 5.0).clamp(10.0, 15.5);
        let line_h = (text_size * 1.25).ceil();
        let row_h = if compact {
            (line_h + 6.0 + z * 36.0).round()
        } else {
            (line_h * 1.5 + 8.0 + z * 44.0).round()
        };
        let padding = 10.0;
        let gap = (4.0 + z * 8.0).round();
        let min_col_w = (130.0 + z * 280.0).clamp(120.0, 480.0);
        let avail = (pane_w - padding * 2.0).max(min_col_w);
        let cols = (((avail + gap) / (min_col_w + gap)).floor() as usize).clamp(1, 8);
        let col_w = ((avail - gap * (cols.saturating_sub(1) as f32)) / cols as f32)
            .floor()
            .max(120.0);
        (cols, col_w, row_h, gap, padding)
    }

    /// `Files-community` Cards (Tiles) view layout metrics.
    /// Reserves height for title + metadata subtitle across a wide dynamic zoom range (0% to 100%).
    /// Returns `(cols, card_width, row_height, gap, horizontal_padding)`.
    pub fn cards_layout_metrics(
        pane_w: f32,
        zoom: f32,
        compact: bool,
    ) -> (usize, f32, f32, f32, f32) {
        let z = zoom.clamp(0.0, 1.0);
        let title_size = (10.5 + z * 5.0).clamp(10.5, 16.0);
        let title_line_h = (title_size * 1.25).ceil();
        let sub_size = (9.0 + z * 3.5).clamp(9.0, 13.0);
        let sub_line_h = (sub_size * 1.25).ceil();
        let padding_vert = if compact { 8.0 + z * 10.0 } else { 12.0 + z * 24.0 };
        let row_h = (title_line_h * 2.0 + sub_line_h + padding_vert + z * 56.0).round().clamp(48.0, 150.0);
        let padding = 12.0;
        let gap = (6.0 + z * 10.0).round();
        let min_card_w = (170.0 + z * 290.0).clamp(160.0, 520.0);
        let avail = (pane_w - padding * 2.0).max(min_card_w);
        let cols = (((avail + gap) / (min_card_w + gap)).floor() as usize).clamp(1, 8);
        let card_w = ((avail - gap * (cols.saturating_sub(1) as f32)) / cols as f32)
            .floor()
            .max(150.0);
        (cols, card_w, row_h, gap, padding)
    }

    /// End lasso selection and apply it to `selected_indices`.
    pub fn lasso_end(&mut self, add_to_selection: bool) {
        if let Some(state) = self.lasso_state.take() {
            if !state.has_exceeded_threshold() {
                return;
            }
            let (start, end) = state.rect();
            let min_x = start.x.min(end.x);
            let max_x = start.x.max(end.x);
            let min_y = start.y.min(end.y);
            let max_y = start.y.max(end.y);

            if !add_to_selection && !state.additive {
                self.selected_indices.clear();
            } else if state.additive {
                self.selected_indices = state.initial_selection;
            }

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

    pub fn is_lasso_active(&self) -> bool {
        self.lasso_state.is_some()
    }

    pub fn check_empty_space_double_click(&mut self) -> bool {
        const DOUBLE_CLICK_INTERVAL: std::time::Duration = std::time::Duration::from_millis(500);
        let now = std::time::Instant::now();
        if let Some(prev) = self.last_empty_click {
            if now.duration_since(prev) < DOUBLE_CLICK_INTERVAL {
                self.last_empty_click = None;
                return true;
            }
        }
        self.last_empty_click = Some(now);
        false
    }

    pub fn finish_live_lasso(&mut self) {
        self.lasso_state = None;
    }

    pub fn lasso_cancel(&mut self) {
        self.lasso_state = None;
    }

    pub fn lasso_rect(&self) -> Option<(Point<f32>, Point<f32>)> {
        self.lasso_state.as_ref().map(|s| s.rect())
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Drag and drop (Internal + Out-of-Window OS Native Drag)
    // ─────────────────────────────────────────────────────────────────────────

    /// Record a mouse-down on item `item_idx` in `pane_focus` as a potential drag gesture.
    pub fn record_drag_candidate(
        &mut self,
        pane_focus: PaneFocus,
        item_idx: usize,
        window_pos: Point<f32>,
    ) {
        self.drag_state.pending_candidate = Some((pane_focus, item_idx, window_pos));
        self.drag_state.pending_path_candidate = None;
    }

    /// Record a mouse-down on an arbitrary `FilesPath` (such as Miller Column 1 or Column 3 items)
    /// so moving >= 6px initiates a file/folder drag operation.
    pub fn record_path_drag_candidate(
        &mut self,
        pane_focus: PaneFocus,
        path: FilesPath,
        window_pos: Point<f32>,
    ) {
        self.drag_state.pending_path_candidate = Some((pane_focus, path, window_pos));
        self.drag_state.pending_candidate = None;
    }

    /// Start a drag operation with the given items and mouse position.
    pub fn drag_start(&mut self, indices: Vec<usize>, point: Point<f32>) {
        let paths: Vec<FilesPath> = indices
            .iter()
            .filter_map(|&idx| self.items.get(idx))
            .map(|it| it.path.clone())
            .collect();
        self.drag_state = DragState {
            indices,
            paths,
            source_pane: PaneFocus::Primary,
            start: point,
            current: point,
            is_copy: false,
            is_link: false,
            is_dragging: true,
            pending_candidate: None,
            pending_path_candidate: None,
            hovered_target_folder: None,
            hovered_target_label: None,
            os_drag_dispatched: false,
        };
    }

    /// Update the current drag position and copy/move modifier.
    pub fn drag_update(&mut self, point: Point<f32>, is_copy: bool) {
        self.drag_state.current = point;
        self.drag_state.is_copy = is_copy;
    }

    /// End the drag operation.
    pub fn drag_end(&mut self) {
        self.drag_state = DragState::default();
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
        // First check if a folder row/tile/card explicitly registered itself as hovered
        if let Some(ref target_path) = self.drag_state.hovered_target_folder {
            if let Some(idx) = self.items.iter().position(|it| &it.path == target_path) {
                if !self.drag_state.indices.contains(&idx) {
                    return Some(idx);
                }
            }
        }

        let items_per_row = 6;
        let item_width = 180.0;
        let item_height = 200.0;
        let gap_x = 12.0;
        let gap_y = 12.0;
        let padding = 16.0;

        for (i, &idx) in visible.iter().enumerate() {
            if self.drag_state.indices.contains(&idx) {
                continue;
            }
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

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

/// Maps normalized `zoom` (`0.0..=1.0`) and `compact_mode` onto the 5-step size scale
/// (`Compact=0`, `Small=1`, `Medium=2`, `Large=3`, `ExtraLarge=4`) used by
/// `Files-community` Details, List, and Columns views.
#[inline]
pub fn size_step_5(zoom: f32, compact: bool) -> usize {
    if compact {
        return 0;
    }
    let z = zoom.clamp(0.0, 1.0);
    if z < 0.15 {
        0
    } else if z < 0.35 {
        1
    } else if z < 0.62 {
        2
    } else if z < 0.88 {
        3
    } else {
        4
    }
}

/// Canonical `Files-community` `DetailsLayoutPage` metrics for a size step.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DetailsLayoutSpec {
    pub step: usize,
    pub row_height: f32,
    pub icon_size: f32,
    pub icon_box_size: f32,
    pub header_height: f32,
    pub name_text_size: f32,
    pub col_text_size: f32,
}

/// Canonical `Files-community` `ListView` (`GridLayoutPage`) metrics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ListLayoutSpec {
    pub step: usize,
    pub cols: usize,
    pub col_width: f32,
    pub row_height: f32,
    pub gap: f32,
    pub padding: f32,
    pub icon_size: f32,
    pub icon_box_size: f32,
    pub text_size: f32,
}

/// Canonical `Files-community` `CardsView` (`CardsBrowserTemplate`) metrics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CardsLayoutSpec {
    pub step: usize,
    pub is_horizontal: bool,
    pub cols: usize,
    pub card_width: f32,
    pub card_height: f32,
    pub row_height: f32,
    pub preview_width: f32,
    pub preview_height: f32,
    pub details_width: f32,
    pub details_height: f32,
    pub icon_size: f32,
    pub max_lines: usize,
    pub show_contextual: bool,
    pub gap: f32,
    pub padding: f32,
}

/// Canonical `Files-community` `ColumnLayoutPage` / `ColumnsLayoutPage` metrics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ColumnsLayoutSpec {
    pub step: usize,
    pub row_height: f32,
    pub icon_size: f32,
    pub icon_box_size: f32,
    pub text_size: f32,
    pub header_height: f32,
    pub zoom_scale: f32,
}

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
                    let row_h = Self::details_layout_spec(self.zoom_level, self.compact_mode).row_height;
                    for (i, &vi) in visible.iter().enumerate() {
                        let y0 = i as f32 * row_h;
                        let item_rect = (point(0.0, y0), point(effective_w, y0 + row_h));
                        if rects_intersect(item_rect, content_rect) {
                            next_selected.insert(vi);
                        }
                    }
                }
                ViewMode::Columns => {
                    let spec = Self::columns_layout_spec(self.zoom_level, self.compact_mode);
                    let row_h = spec.row_height;
                    let col2_w = (self.column_widths.miller_col2 * spec.zoom_scale)
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
                    let spec = Self::list_layout_spec(effective_w, self.zoom_level, self.compact_mode);
                    let avail_h = self.effective_list_viewport_h();
                    let total_rows =
                        Self::list_rows_per_col(visible.len(), spec.cols, spec.row_height, avail_h)
                            .max(1);
                    for (i, &vi) in visible.iter().enumerate() {
                        let col = i / total_rows;
                        let row = i % total_rows;
                        let x0 = spec.padding + col as f32 * (spec.col_width + spec.gap);
                        let y0 = row as f32 * spec.row_height;
                        let item_rect = (
                            point(x0, y0),
                            point(x0 + spec.col_width, y0 + spec.row_height),
                        );
                        if rects_intersect(item_rect, content_rect) {
                            next_selected.insert(vi);
                        }
                    }
                }
                ViewMode::Cards => {
                    let spec =
                        Self::cards_layout_spec(effective_w, self.zoom_level, self.compact_mode);
                    let cols_safe = spec.cols.max(1);
                    for (i, &vi) in visible.iter().enumerate() {
                        let row = i / cols_safe;
                        let col = i % cols_safe;
                        let x0 = spec.padding + col as f32 * (spec.card_width + spec.gap);
                        let y0 = row as f32 * spec.row_height;
                        let item_rect = (
                            point(x0, y0),
                            point(x0 + spec.card_width, y0 + spec.card_height),
                        );
                        if rects_intersect(item_rect, content_rect) {
                            next_selected.insert(vi);
                        }
                    }
                }
                ViewMode::Grid => {
                    let cols = Self::grid_columns_for_width(effective_w, self.zoom_level);
                    let params =
                        Self::grid_layout_params_for_width_pub(effective_w, self.zoom_level, cols);
                    let item_h = if self.compact_mode {
                        (params.item_height - 6.0).max(params.item_width + 44.0)
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

    /// Returns the active list viewport height used by column-major `ListView` wrapping.
    #[inline]
    pub fn effective_list_viewport_h(&self) -> f32 {
        if self.bouncy_scroll.viewport_height > 40.0 {
            self.bouncy_scroll.viewport_height
        } else {
            760.0
        }
    }

    /// Canonical `Files-community` `DetailsViewSizeKind` (Compact..ExtraLarge) metrics.
    pub fn details_layout_spec(zoom: f32, compact: bool) -> DetailsLayoutSpec {
        const ROW_HEIGHTS: [f32; 5] = [28.0, 36.0, 40.0, 44.0, 48.0];
        const ICON_SIZES: [f32; 5] = [16.0, 16.0, 20.0, 24.0, 32.0];
        let step = size_step_5(zoom, compact);
        let row_height = ROW_HEIGHTS[step];
        let icon_size = ICON_SIZES[step];
        DetailsLayoutSpec {
            step,
            row_height,
            icon_size,
            icon_box_size: icon_size + 4.0,
            header_height: 40.0,
            name_text_size: if step == 0 { 13.0 } else { 14.0 },
            col_text_size: 12.0,
        }
    }

    /// Canonical `Files-community` `ColumnsViewSizeKind` (Compact..ExtraLarge) metrics.
    pub fn columns_layout_spec(zoom: f32, compact: bool) -> ColumnsLayoutSpec {
        const ROW_HEIGHTS: [f32; 5] = [24.0, 32.0, 36.0, 40.0, 44.0];
        const ICON_SIZES: [f32; 5] = [16.0, 16.0, 20.0, 24.0, 32.0];
        let step = size_step_5(zoom, compact);
        let row_height = ROW_HEIGHTS[step];
        let icon_size = ICON_SIZES[step];
        let z = zoom.clamp(0.0, 1.0);
        ColumnsLayoutSpec {
            step,
            row_height,
            icon_size,
            icon_box_size: icon_size + 4.0,
            text_size: if step == 0 { 13.0 } else { 14.0 },
            header_height: 36.0,
            // Keep blade columns readable even at Compact/Small instead of shrinking to 130px.
            zoom_scale: (0.94 + z * 0.18).clamp(0.94, 1.12),
        }
    }

    /// Canonical `Files-community` `ListViewSizeKind` (`260px` column width, `24..44px` row heights).
    pub fn list_layout_spec(pane_w: f32, zoom: f32, compact: bool) -> ListLayoutSpec {
        const ROW_HEIGHTS: [f32; 5] = [24.0, 32.0, 36.0, 40.0, 44.0];
        const ICON_SIZES: [f32; 5] = [16.0, 16.0, 20.0, 24.0, 32.0];
        let step = size_step_5(zoom, compact);
        let row_height = ROW_HEIGHTS[step];
        let icon_size = ICON_SIZES[step];
        let padding = 8.0;
        let gap = 4.0;
        let avail = (pane_w - padding * 2.0).max(160.0);
        let col_width = 260.0_f32.min(avail).max(160.0);
        let cols = (((avail + gap) / (col_width + gap)).floor() as usize).clamp(1, 12);
        ListLayoutSpec {
            step,
            cols,
            col_width,
            row_height,
            gap,
            padding,
            icon_size,
            icon_box_size: icon_size + 4.0,
            text_size: if step == 0 { 13.0 } else { 14.0 },
        }
    }

    /// Computes vertical-wrap row count per column for column-major `ListView` layout
    /// so smaller/medium lists stack in Column 0 and wrap into Column 1 when viewport height fills,
    /// while huge directories stay evenly distributed and virtualized under `uniform_list`.
    #[inline]
    pub fn list_rows_per_col(total_items: usize, cols_fit: usize, row_h: f32, avail_h: f32) -> usize {
        if total_items == 0 {
            return 0;
        }
        let cols = cols_fit.max(1);
        let safe_h = if avail_h > 40.0 { avail_h } else { 760.0 };
        let rows_fit = ((safe_h / row_h.max(1.0)).floor() as usize).max(1);
        let min_rows_for_cols = total_items.div_ceil(cols);
        rows_fit.max(min_rows_for_cols).min(total_items)
    }

    /// Multi-column list layout metrics shared between `ListView` rendering,
    /// rubber-band lasso hit-testing, and 2D keyboard navigation.
    /// Returns `(cols, col_width, row_height, gap, horizontal_padding)`.
    pub fn list_layout_metrics(
        pane_w: f32,
        zoom: f32,
        compact: bool,
    ) -> (usize, f32, f32, f32, f32) {
        let s = Self::list_layout_spec(pane_w, zoom, compact);
        (s.cols, s.col_width, s.row_height, s.gap, s.padding)
    }

    /// Canonical `Files-community` `CardsViewSizeKind` (Small..ExtraLarge) metrics.
    pub fn cards_layout_spec(pane_w: f32, zoom: f32, compact: bool) -> CardsLayoutSpec {
        let z = zoom.clamp(0.0, 1.0);
        let step = if compact || z < 0.28 {
            0 // Small (Horizontal 300x104)
        } else if z < 0.62 {
            1 // Medium (Vertical 240x240)
        } else if z < 0.88 {
            2 // Large (Vertical 280x272)
        } else {
            3 // ExtraLarge (Vertical 320x288)
        };

        let (
            is_horizontal,
            raw_card_w,
            card_h,
            raw_preview_w,
            preview_h,
            raw_details_w,
            details_h,
            icon_size,
            max_lines,
            show_contextual,
        ): (bool, f32, f32, f32, f32, f32, f32, f32, usize, bool) = match step {
            0 => (true, 300.0, 104.0, 104.0, 104.0, 196.0, 104.0, 64.0, 2, false),
            1 => (false, 240.0, 240.0, 240.0, 96.0, 240.0, 144.0, 64.0, 2, true),
            2 => (false, 280.0, 272.0, 280.0, 128.0, 280.0, 144.0, 80.0, 2, true),
            _ => (false, 320.0, 288.0, 320.0, 160.0, 320.0, 128.0, 96.0, 1, true),
        };

        let padding = 8.0;
        let gap = 8.0;
        let avail = (pane_w - padding * 2.0).max(160.0);
        let card_width = raw_card_w.min(avail).max(160.0);
        let cols = (((avail + gap) / (card_width + gap)).floor() as usize).clamp(1, 8);
        let (preview_width, details_width) = if is_horizontal {
            let pw = raw_preview_w.min(card_width * 0.45);
            let dw = (card_width - pw).max(80.0);
            (pw, dw)
        } else {
            let _ = (raw_preview_w, raw_details_w);
            (card_width, card_width)
        };

        CardsLayoutSpec {
            step,
            is_horizontal,
            cols,
            card_width,
            card_height: card_h,
            row_height: card_h + gap,
            preview_width,
            preview_height: preview_h,
            details_width,
            details_height: details_h,
            icon_size,
            max_lines,
            show_contextual,
            gap,
            padding,
        }
    }

    /// `Files-community` Cards (Tiles) view layout metrics.
    /// Returns `(cols, card_width, row_height, gap, horizontal_padding)`.
    pub fn cards_layout_metrics(
        pane_w: f32,
        zoom: f32,
        compact: bool,
    ) -> (usize, f32, f32, f32, f32) {
        let s = Self::cards_layout_spec(pane_w, zoom, compact);
        (s.cols, s.card_width, s.row_height, s.gap, s.padding)
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

#[cfg(test)]
mod view_layout_tests {
    use super::MainViewState;

    #[::core::prelude::v1::test]
    fn grid_view_matches_files_community_steps_and_square_bounds() {
        let small = MainViewState::grid_layout_params(0.0);
        assert_eq!(small.item_width, 80.0);
        assert_eq!(small.icon_size, 60.0);
        assert_eq!(small.rounded, 8.0);
        assert_eq!(small.item_height, 136.0);

        let mid = MainViewState::grid_layout_params(0.45);
        assert_eq!(mid.item_width, 180.0);
        assert_eq!(mid.icon_size, 156.0);
        assert_eq!(mid.item_height, 236.0);

        let large = MainViewState::grid_layout_params(1.0);
        assert_eq!(large.item_width, 300.0);
        assert_eq!(large.icon_size, 276.0);
        assert_eq!(large.item_height, 356.0);

        // No 1.32x width distortion when fitting wide panes
        let cols = MainViewState::grid_columns_for_width(1400.0, 0.45);
        let fitted = MainViewState::grid_layout_params_for_width_pub(1400.0, 0.45, cols);
        assert_eq!(fitted.item_width, 180.0);
    }

    #[test]
    fn details_view_matches_files_community_5_steps() {
        let compact = MainViewState::details_layout_spec(0.0, false);
        assert_eq!((compact.row_height, compact.icon_size), (28.0, 16.0));
        assert_eq!(compact.header_height, 40.0);

        let small = MainViewState::details_layout_spec(0.25, false);
        assert_eq!((small.row_height, small.icon_size), (36.0, 16.0));

        let mid = MainViewState::details_layout_spec(0.45, false);
        assert_eq!((mid.row_height, mid.icon_size), (40.0, 20.0));

        let large = MainViewState::details_layout_spec(0.75, false);
        assert_eq!((large.row_height, large.icon_size), (44.0, 24.0));

        let xl = MainViewState::details_layout_spec(1.0, false);
        assert_eq!((xl.row_height, xl.icon_size), (48.0, 32.0));

        let forced_compact = MainViewState::details_layout_spec(1.0, true);
        assert_eq!((forced_compact.row_height, forced_compact.icon_size), (28.0, 16.0));
    }

    #[test]
    fn cards_view_matches_files_community_4_steps_and_orientation() {
        let small = MainViewState::cards_layout_spec(1400.0, 0.0, false);
        assert!(small.is_horizontal);
        assert_eq!((small.card_width, small.card_height), (300.0, 104.0));
        assert_eq!((small.preview_width, small.preview_height, small.icon_size), (104.0, 104.0, 64.0));
        assert!(!small.show_contextual);

        let mid = MainViewState::cards_layout_spec(1400.0, 0.4, false);
        assert!(!mid.is_horizontal);
        assert_eq!((mid.card_width, mid.card_height), (240.0, 240.0));
        assert_eq!((mid.preview_width, mid.preview_height, mid.icon_size), (240.0, 96.0, 64.0));
        assert!(mid.show_contextual);

        let large = MainViewState::cards_layout_spec(1400.0, 0.75, false);
        assert!(!large.is_horizontal);
        assert_eq!((large.card_width, large.card_height), (280.0, 272.0));
        assert_eq!((large.preview_width, large.preview_height, large.icon_size), (280.0, 128.0, 80.0));

        let xl = MainViewState::cards_layout_spec(1400.0, 1.0, false);
        assert!(!xl.is_horizontal);
        assert_eq!((xl.card_width, xl.card_height), (320.0, 288.0));
        assert_eq!((xl.preview_width, xl.preview_height, xl.icon_size), (320.0, 160.0, 96.0));
        assert_eq!(xl.max_lines, 1);
    }

    #[test]
    fn list_view_matches_files_community_260px_col_and_vertical_wrap() {
        let compact = MainViewState::list_layout_spec(1400.0, 0.0, false);
        assert_eq!(compact.col_width, 260.0);
        assert_eq!((compact.row_height, compact.icon_size), (24.0, 16.0));

        let mid = MainViewState::list_layout_spec(1400.0, 0.45, false);
        assert_eq!((mid.row_height, mid.icon_size), (36.0, 20.0));

        let xl = MainViewState::list_layout_spec(1400.0, 1.0, false);
        assert_eq!((xl.row_height, xl.icon_size), (44.0, 32.0));

        // 27 items at 44px row height in 1040px tall viewport -> 23 rows in col 0, 4 in col 1
        let rows = MainViewState::list_rows_per_col(27, xl.cols, xl.row_height, 1040.0);
        assert_eq!(rows, 23);

        // 27 items at 24px row height in 1040px tall viewport -> all 27 fit in col 0
        let rows_compact = MainViewState::list_rows_per_col(27, compact.cols, compact.row_height, 1040.0);
        assert_eq!(rows_compact, 27);
    }

    #[test]
    fn columns_view_matches_files_community_5_steps() {
        let compact = MainViewState::columns_layout_spec(0.0, false);
        assert_eq!((compact.row_height, compact.icon_size), (24.0, 16.0));

        let mid = MainViewState::columns_layout_spec(0.45, false);
        assert_eq!((mid.row_height, mid.icon_size), (36.0, 20.0));

        let xl = MainViewState::columns_layout_spec(1.0, false);
        assert_eq!((xl.row_height, xl.icon_size), (44.0, 32.0));
    }
}

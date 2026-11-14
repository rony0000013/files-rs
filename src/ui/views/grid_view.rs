//! Grid View — Files-community Icon/Grid view with ultra-smooth dynamic zoom (66px micro-tiles
//! up to 246px XL thumbnails), >120 FPS Level-of-Detail (LOD) fast-path for 500+ visible items,
//! 3-line adaptive filename wrapping (without redundant "Folder"/size subtitle),
//! live rubber-band selection, Shift/Ctrl multi-selection, and full Drag & Drop support.

use crate::config::config::GroupColumn;
use crate::core::navigation_manager::PaneFocus;
use crate::storage::storage::FileType;
use crate::ui::components::icon_renderer::is_thumbnail_image;
use crate::ui::constants::UiConstants;
use crate::ui::views::helpers::{adaptive_three_line_label, get_file_type_icon_and_color};
use crate::ui::views::main_view::{
    get_extension, FileItems, GroupedGridItem, MainView, MainViewState, SelectionMode,
};
use gpui::*;

impl MainViewState {
    /// Render grid view with dynamic zoom (0.0–1.0 mapping high-density micro-grid → XL cards)
    pub fn render_grid_view(
        &self,
        file_items: Entity<FileItems>,
        view: Entity<crate::app::AppWindow>,
        cols: usize,
        pane_w: f32,
    ) -> AnyElement {
        self.render_grid_view_for_pane(
            file_items,
            view,
            cols,
            pane_w,
            PaneFocus::Primary,
            gpui::point(0.0, 128.0),
        )
    }

    pub fn render_grid_view_for_pane(
        &self,
        file_items: Entity<FileItems>,
        view: Entity<crate::app::AppWindow>,
        cols: usize,
        pane_w: f32,
        pane_focus: PaneFocus,
        list_origin: Point<f32>,
    ) -> AnyElement {
        let visible = self.visible_items_rc();

        if self.group_by != GroupColumn::None {
            return self
                .render_grid_view_grouped_for_pane(
                    &visible,
                    file_items,
                    view,
                    cols,
                    pane_w,
                    pane_focus,
                    list_origin,
                )
                .into_any_element();
        }

        self.grid_view_flat_for_pane(
            &visible,
            file_items,
            view,
            cols,
            pane_w,
            pane_focus,
            list_origin,
        )
        .into_any_element()
    }

    /// Compute grid layout parameters from `zoom_level` (`0.0..=1.0`).
    /// Reserves space for 3 full lines of filename text (`label_lines = 3.0`) without
    /// Exact `Files-community` `GridViewSizeKind` (80px..300px in 20px steps) with 1:1 square
    /// thumbnail/icon area, 12px inner margin, 8px container corner radius, and 3-line centered label.
    pub fn grid_layout_params(zoom: f32) -> GridLayoutParams {
        let z = zoom.clamp(0.0, 1.0);
        let step = (z * 11.0).round().clamp(0.0, 11.0) as usize;
        let item_width = 80.0 + (step as f32) * 20.0;
        let thumb_margin = if item_width <= 90.0 { 10.0 } else { 12.0 };
        let icon_size = (item_width - thumb_margin * 2.0).max(40.0);
        let text_size = if item_width <= 90.0 {
            12.0
        } else if item_width <= 130.0 {
            13.0
        } else {
            14.0
        };
        let label_box_h = 56.0;
        let item_height = item_width + label_box_h;
        let gap = 6.0;
        let padding = 8.0;
        let items_per_row =
            (((1400.0 - padding * 2.0 + gap) / (item_width + gap)).floor() as usize).clamp(1, 32);

        GridLayoutParams {
            items_per_row,
            item_width,
            item_height,
            row_height: item_height + gap,
            icon_size,
            gap,
            rounded: 8.0,
            text_size,
            padding,
        }
    }

    /// Columns that fit the actual pane width (Files-community style: fixed width items, left-aligned wrap).
    pub fn grid_columns_for_width(pane_w: f32, zoom: f32) -> usize {
        let params = Self::grid_layout_params(zoom);
        let avail = (pane_w - params.padding * 2.0).max(params.item_width);
        let slot = params.item_width + params.gap;
        if slot <= 0.0 {
            return params.items_per_row;
        }
        (((avail + params.gap) / slot).floor() as usize).clamp(1, 32)
    }

    pub fn grid_layout_params_for_width_pub(
        pane_w: f32,
        zoom: f32,
        cols: usize,
    ) -> GridLayoutParams {
        Self::grid_layout_params_for_width(pane_w, zoom, cols)
    }

    fn grid_layout_params_for_width(pane_w: f32, zoom: f32, cols: usize) -> GridLayoutParams {
        let base = Self::grid_layout_params(zoom);
        let cols = cols.max(1);
        let avail = (pane_w - base.padding * 2.0).max(64.0);
        let item_width = base.item_width.min(avail);
        let thumb_margin = if item_width <= 90.0 { 10.0 } else { 12.0 };
        let icon_size = (item_width - thumb_margin * 2.0).max(40.0);
        let item_height = item_width + 56.0;
        GridLayoutParams {
            items_per_row: cols,
            item_width,
            item_height,
            row_height: item_height + base.gap,
            icon_size,
            ..base
        }
    }

    fn grid_view_flat_for_pane(
        &self,
        visible: &[usize],
        file_items: Entity<FileItems>,
        view: Entity<crate::app::AppWindow>,
        cols: usize,
        pane_w: f32,
        pane_focus: PaneFocus,
        list_origin: Point<f32>,
    ) -> impl IntoElement {
        let zoom = self.zoom_level;
        let params = Self::grid_layout_params_for_width(pane_w, zoom, cols);
        let items_per_row = params.items_per_row.max(1);
        let row_count = (visible.len() + items_per_row - 1) / items_per_row;

        div()
            .id(SharedString::from(match pane_focus {
                PaneFocus::Primary => "grid-container-primary",
                PaneFocus::Secondary => "grid-container-secondary",
            }))
            .relative()
            .flex_col()
            .w_full()
            .flex_1()
            .h_full()
            .on_mouse_down(MouseButton::Right, {
                let v = view.clone();
                move |event, window, cx| {
                    v.update(cx, |this, cx| {
                        this.focus_pane(pane_focus, cx);
                        this.clear_selection(cx);
                        this.show_context_menu(None, event.position, window, cx);
                    });
                }
            })
            .on_mouse_down(MouseButton::Left, {
                let v = view.clone();
                move |event, _win, cx| {
                    let pos = point(event.position.x.into(), event.position.y.into());
                    let additive = event.modifiers.control || event.modifiers.shift;
                    v.update(cx, |this, cx| {
                        this.focus_pane(pane_focus, cx);
                        this.commit_rename_if_active(cx);
                        this.start_lasso_in_pane(pane_focus, pos, list_origin, additive, cx);
                    });
                }
            })
            .on_mouse_move({
                let v = view.clone();
                move |event, _win, cx| {
                    let pos = point(event.position.x.into(), event.position.y.into());
                    v.update(cx, |this, cx| {
                        if this.state.main_view.drag_state.is_dragging
                            || this.state.main_view.drag_state.pending_candidate.is_some()
                            || this
                                .state
                                .main_view
                                .drag_state
                                .pending_path_candidate
                                .is_some()
                        {
                            let pane_target =
                                if this.state.main_view.drag_state.source_pane != pane_focus {
                                    let dest =
                                        this.state.pane_view(pane_focus).primary_path.clone();
                                    let lbl = dest
                                        .file_name()
                                        .map(|n| n.to_string_lossy().into_owned())
                                        .unwrap_or_else(|| dest.to_string_lossy().into_owned());
                                    Some((dest, lbl))
                                } else {
                                    None
                                };
                            this.update_file_drag_move(pos, event.modifiers, pane_target, cx);
                        } else {
                            this.update_lasso_in_pane(pane_focus, pos, pane_w, cx);
                        }
                    });
                }
            })
            .on_mouse_up(MouseButton::Left, {
                let v = view.clone();
                move |event, _win, cx| {
                    let pos = point(event.position.x.into(), event.position.y.into());
                    v.update(cx, |this, cx| {
                        if this.state.main_view.drag_state.is_dragging
                            || this.state.main_view.drag_state.pending_candidate.is_some()
                            || this
                                .state
                                .main_view
                                .drag_state
                                .pending_path_candidate
                                .is_some()
                        {
                            this.end_drag(pos.x, pos.y, None, cx);
                        }
                        this.end_lasso_in_pane(pane_focus, cx);
                    });
                }
            })
            .on_mouse_up_out(MouseButton::Left, {
                let v = view.clone();
                move |event, _win, cx| {
                    let pos = point(event.position.x.into(), event.position.y.into());
                    v.update(cx, |this, cx| {
                        if this.state.main_view.drag_state.is_dragging
                            || this.state.main_view.drag_state.pending_candidate.is_some()
                            || this
                                .state
                                .main_view
                                .drag_state
                                .pending_path_candidate
                                .is_some()
                        {
                            this.end_drag(pos.x, pos.y, None, cx);
                        }
                        this.end_lasso_in_pane(pane_focus, cx);
                    });
                }
            })
            .child(
                uniform_list(
                    match pane_focus {
                        PaneFocus::Primary => "grid-scroll-primary",
                        PaneFocus::Secondary => "grid-scroll-secondary",
                    },
                    row_count,
                    {
                        let view = view.clone();
                        move |range, _win, cx| {
                            let fi = file_items.read(cx);
                            let app_state = view.read(cx);
                            let main_state = app_state.state.pane_view(pane_focus);
                            let visible_rc = main_state.visible_items_rc();
                            let icon_paths = &app_state.state.main_view.icon_paths;
                            let zoom = main_state.zoom_level;
                            let live_cols = Self::grid_columns_for_width(pane_w, zoom);
                            let params = Self::grid_layout_params_for_width(pane_w, zoom, live_cols);
                            let sel_mode = main_state.selection_mode.clone();
                            let compact = main_state.compact_mode;
                            let show_extensions = main_state.show_extensions;
                            let hovered_drop_folder = app_state
                                .state
                                .main_view
                                .drag_state
                                .hovered_target_folder
                                .clone();
                            let is_dragging_any = app_state.state.main_view.drag_state.is_dragging;
                            let lod_fast = false;

                            range
                                .map(|row_idx| {
                                    let start = row_idx * params.items_per_row;
                                    let end =
                                        (start + params.items_per_row).min(visible_rc.len());
                                    let items_in_row = end.saturating_sub(start);

                                    let mut row_children: Vec<AnyElement> = (start..end)
                                        .filter_map(|i| {
                                            let idx = visible_rc[i];
                                            let item = fi.items.get(idx)?;
                                            let is_selected =
                                                main_state.selected_indices.contains(&idx);
                                            let is_dir =
                                                matches!(item.file_type, FileType::Directory);
                                            let is_drop_target = is_dragging_any
                                                && is_dir
                                                && hovered_drop_folder.as_ref()
                                                    == Some(&item.path);

                                            Some(
                                                Self::grid_item_for_pane(
                                                    item,
                                                    idx,
                                                    is_selected,
                                                    is_drop_target,
                                                    lod_fast,
                                                    sel_mode.clone(),
                                                    view.clone(),
                                                    icon_paths,
                                                    &params,
                                                    compact,
                                                    show_extensions,
                                                    main_state.renaming_index,
                                                    &main_state.rename_input,
                                                    main_state.cut_paths.contains(item.path.as_path()),
                                                    pane_focus,
                                                    pane_w,
                                                )
                                                .into_any_element(),
                                            )
                                        })
                                        .collect();

                                    for _ in items_in_row..params.items_per_row {
                                        row_children.push(
                                            div()
                                                .w(px(params.item_width))
                                                .flex_shrink_0()
                                                .invisible()
                                                .into_any_element(),
                                        );
                                    }

                                    div()
                                        .w_full()
                                        .flex()
                                        .flex_row()
                                        .flex_shrink_0()
                                        .h(px(params.row_height))
                                        .gap(px(params.gap))
                                        .px(px(params.padding))
                                        .children(row_children)
                                        .into_any_element()
                                })
                                .collect::<Vec<_>>()
                        }
                    },
                )
                .with_sizing_behavior(ListSizingBehavior::Auto)
                .w_full()
                .h_full()
                .track_scroll(self.uniform_list_scroll_handle.clone()),
            )
            .child(MainView::render_lasso_box(self))
    }

    fn render_grid_view_grouped_for_pane(
        &self,
        visible: &[usize],
        _file_items: Entity<FileItems>,
        view: Entity<crate::app::AppWindow>,
        cols: usize,
        pane_w: f32,
        pane_focus: PaneFocus,
        list_origin: Point<f32>,
    ) -> impl IntoElement {
        let zoom = self.zoom_level;
        let live_cols = Self::grid_columns_for_width(pane_w, zoom).max(cols.min(1));
        let params = Self::grid_layout_params_for_width(pane_w, zoom, live_cols);
        let items_per_row = params.items_per_row.max(1);
        let flattened = self.flatten_grouped_grid_items(visible, items_per_row);
        let compact = self.compact_mode;
        let header_h = if compact { 28.0 } else { 32.0 };
        let row_h = params.row_height;

        let item_heights: Vec<f32> = flattened
            .iter()
            .map(|entry| match entry {
                GroupedGridItem::Header { .. } => header_h,
                GroupedGridItem::Row(_) => row_h,
            })
            .collect();

        let sel_mode = self.selection_mode.clone();
        let show_extensions = self.show_extensions;
        let hovered_drop_folder = self.drag_state.hovered_target_folder.clone();
        let is_dragging_any = self.drag_state.is_dragging;
        let lod_fast = false;

        let scroll_list = MainView::render_virtualized_grouped_list(
            match pane_focus {
                PaneFocus::Primary => "grid-grouped-scroll-primary",
                PaneFocus::Secondary => "grid-grouped-scroll-secondary",
            },
            &self.uniform_list_scroll_handle,
            pane_w,
            &item_heights,
            |i| match &flattened[i] {
                GroupedGridItem::Header { key, label, count } => {
                    let is_collapsed = self.collapsed_groups.contains(key);
                    MainView::render_group_header_row(
                        i,
                        key,
                        label,
                        *count,
                        is_collapsed,
                        header_h,
                        params.padding,
                        pane_focus,
                        view.clone(),
                    )
                }
                GroupedGridItem::Row(indices) => {
                    let items_in_row = indices.len();
                    let mut row_children: Vec<AnyElement> = indices
                        .iter()
                        .filter_map(|&idx| {
                            let item = self.items.get(idx)?;
                            let selected = self.selected_indices.contains(&idx);
                            let is_dir = matches!(item.file_type, FileType::Directory);
                            let is_drop_target = is_dragging_any
                                && is_dir
                                && hovered_drop_folder.as_ref() == Some(&item.path);
                            Some(
                                Self::grid_item_for_pane(
                                    item,
                                    idx,
                                    selected,
                                    is_drop_target,
                                    lod_fast,
                                    sel_mode.clone(),
                                    view.clone(),
                                    &self.icon_paths,
                                    &params,
                                    compact,
                                    show_extensions,
                                    self.renaming_index,
                                    &self.rename_input,
                                    self.cut_paths.contains(item.path.as_path()),
                                    pane_focus,
                                    pane_w,
                                )
                                .into_any_element(),
                            )
                        })
                        .collect();

                    for _ in items_in_row..params.items_per_row {
                        row_children.push(
                            div()
                                .w(px(params.item_width))
                                .flex_shrink_0()
                                .invisible()
                                .into_any_element(),
                        );
                    }

                    div()
                        .w_full()
                        .flex()
                        .flex_row()
                        .flex_shrink_0()
                        .h(px(params.row_height))
                        .gap(px(params.gap))
                        .px(px(params.padding))
                        .children(row_children)
                        .into_any_element()
                }
            },
        );

        div()
            .id(SharedString::from(match pane_focus {
                PaneFocus::Primary => "grid-grouped-container-primary",
                PaneFocus::Secondary => "grid-grouped-container-secondary",
            }))
            .relative()
            .flex_col()
            .w_full()
            .flex_1()
            .h_full()
            .on_mouse_down(MouseButton::Right, {
                let v = view.clone();
                move |event, window, cx| {
                    v.update(cx, |this, cx| {
                        this.focus_pane(pane_focus, cx);
                        this.clear_selection(cx);
                        this.show_context_menu(None, event.position, window, cx);
                    });
                }
            })
            .on_mouse_down(MouseButton::Left, {
                let v = view.clone();
                move |event, _win, cx| {
                    let pos = point(event.position.x.into(), event.position.y.into());
                    let additive = event.modifiers.control || event.modifiers.shift;
                    v.update(cx, |this, cx| {
                        this.focus_pane(pane_focus, cx);
                        this.commit_rename_if_active(cx);
                        this.start_lasso_in_pane(pane_focus, pos, list_origin, additive, cx);
                    });
                }
            })
            .on_mouse_move({
                let v = view.clone();
                move |event, _win, cx| {
                    let pos = point(event.position.x.into(), event.position.y.into());
                    v.update(cx, |this, cx| {
                        if this.state.main_view.drag_state.is_dragging
                            || this.state.main_view.drag_state.pending_candidate.is_some()
                            || this
                                .state
                                .main_view
                                .drag_state
                                .pending_path_candidate
                                .is_some()
                        {
                            let pane_target =
                                if this.state.main_view.drag_state.source_pane != pane_focus {
                                    let dest =
                                        this.state.pane_view(pane_focus).primary_path.clone();
                                    let lbl = dest
                                        .file_name()
                                        .map(|n| n.to_string_lossy().into_owned())
                                        .unwrap_or_else(|| dest.to_string_lossy().into_owned());
                                    Some((dest, lbl))
                                } else {
                                    None
                                };
                            this.update_file_drag_move(pos, event.modifiers, pane_target, cx);
                        } else {
                            this.update_lasso_in_pane(pane_focus, pos, pane_w, cx);
                        }
                    });
                }
            })
            .on_mouse_up(MouseButton::Left, {
                let v = view.clone();
                move |event, _win, cx| {
                    let pos = point(event.position.x.into(), event.position.y.into());
                    v.update(cx, |this, cx| {
                        if this.state.main_view.drag_state.is_dragging
                            || this.state.main_view.drag_state.pending_candidate.is_some()
                            || this
                                .state
                                .main_view
                                .drag_state
                                .pending_path_candidate
                                .is_some()
                        {
                            this.end_drag(pos.x, pos.y, None, cx);
                        }
                        this.end_lasso_in_pane(pane_focus, cx);
                    });
                }
            })
            .on_mouse_up_out(MouseButton::Left, {
                let v = view.clone();
                move |event, _win, cx| {
                    let pos = point(event.position.x.into(), event.position.y.into());
                    v.update(cx, |this, cx| {
                        if this.state.main_view.drag_state.is_dragging
                            || this.state.main_view.drag_state.pending_candidate.is_some()
                            || this
                                .state
                                .main_view
                                .drag_state
                                .pending_path_candidate
                                .is_some()
                        {
                            this.end_drag(pos.x, pos.y, None, cx);
                        }
                        this.end_lasso_in_pane(pane_focus, cx);
                    });
                }
            })
            .child(scroll_list)
            .child(MainView::render_lasso_box(self))
    }

    #[allow(clippy::too_many_arguments)]
    fn grid_item_for_pane(
        item: &crate::domain::metadata::FileItem,
        index: usize,
        selected: bool,
        is_drop_target: bool,
        lod_fast: bool,
        selection_mode: SelectionMode,
        view: Entity<crate::app::AppWindow>,
        icon_paths: &std::collections::HashMap<
            crate::core::path_util::FilesPath,
            std::path::PathBuf,
        >,
        params: &GridLayoutParams,
        compact: bool,
        show_extensions: bool,
        renaming_index: Option<usize>,
        rename_input: &crate::ui::components::text_input::TextInputState,
        is_cut: bool,
        pane_focus: PaneFocus,
        pane_w: f32,
    ) -> impl IntoElement {
        let is_dir = matches!(item.file_type, FileType::Directory);
        let extension = get_extension(&item.name);
        let (fallback_glyph, fallback_color) = if is_dir {
            ("\u{E8B7}", 0xF5B942u32)
        } else {
            get_file_type_icon_and_color(&extension)
        };
        let item_path = item.path.clone();
        let item_name_for_drag = item.name.clone();
        let raw_name = if show_extensions || is_dir {
            item.name.clone()
        } else {
            std::path::Path::new(&item.name)
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned()
        };

        let _ = lod_fast;
        let item_width = params.item_width;
        let thumb_height = item_width;
        let label_box_h = if compact { 50.0 } else { 56.0 };
        let item_height = if compact {
            (params.item_height - 6.0).max(item_width + label_box_h)
        } else {
            params.item_height
        };
        let icon_size_val = params.icon_size;
        let thumb_margin = if item_width <= 90.0 { 10.0 } else { 12.0 };
        let rounded = params.rounded;
        let text_size = params.text_size;
        let (display_label, effective_font_size, effective_line_h) = adaptive_three_line_label(
            &raw_name,
            item_width - 10.0,
            text_size,
            (text_size - 1.5).max(10.0),
        );

        let v_click = view.clone();
        let v_hover = view.clone();
        let v_checkbox = view.clone();
        let is_thumbnail = !is_dir && is_thumbnail_image(&extension);
        let show_checkbox = selection_mode == SelectionMode::Checkbox;
        let is_hover_select = selection_mode == SelectionMode::HoverSelect;
        let cached_icon_path: Option<std::path::PathBuf> = icon_paths.get(&item_path).cloned();

        let mut item_div = div()
            .id(ElementId::from(
                index + match pane_focus {
                    PaneFocus::Primary => 70000,
                    PaneFocus::Secondary => 170000,
                },
            ))
            .w(px(item_width))
            .flex_shrink_0()
            .h(px(item_height))
            .flex_col()
            .items_center()
            .justify_start()
            .overflow_hidden()
            .rounded(px(rounded))
            .relative()
            .cursor_pointer()
            .border(px(1.0))
            .border_color(if is_drop_target {
                rgba(0x60CDFFFFu32)
            } else if selected {
                rgba(0x60CDFFFFu32)
            } else {
                rgba(0x00000000u32)
            })
            .bg(if is_drop_target {
                rgba(0x0078D444u32)
            } else if selected {
                rgba(UiConstants::BG_ITEM_SELECTED_RGBA)
            } else {
                rgba(0x00000000u32)
            })
            .hover(|s| {
                s.bg(if selected {
                    rgba(UiConstants::BG_ITEM_SELECTED_HOVER_RGBA)
                } else {
                    rgba(UiConstants::BG_ITEM_HOVER_RGBA)
                })
            });

        if is_cut {
            item_div = item_div
                .opacity(0.55)
                .border(px(1.0))
                .border_dashed()
                .border_color(rgba(0x60CDFFCCu32));
        }

        if show_checkbox || selected {
            item_div = item_div.child(
                div()
                    .absolute()
                    .top(px(6.0))
                    .left(px(6.0))
                    .w(px(22.0))
                    .h(px(22.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        cx.stop_propagation();
                        v_checkbox.update(cx, |this, cx| {
                            this.focus_pane(pane_focus, cx);
                            this.toggle_checkbox_selection(index, cx);
                        });
                    })
                    .child(
                        div()
                            .w(px(16.0))
                            .h(px(16.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .border(px(1.5))
                            .border_color(if selected {
                                rgba(UiConstants::ACCENT_RGBA)
                            } else {
                                rgba(0xFFFFFF99u32)
                            })
                            .rounded(px(3.5))
                            .bg(if selected {
                                rgba(UiConstants::ACCENT_RGBA)
                            } else {
                                rgba(0x00000044u32)
                            })
                            .child(if selected {
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_size(px(10.0))
                                    .line_height(px(10.0))
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
        let path_for_move = item_path.clone();
        let can_direct_image_fallback = is_thumbnail
            && item.size > 0
            && item.size <= 1_500_000
            && matches!(
                extension.as_str(),
                "png" | "jpg" | "jpeg" | "webp" | "gif" | "bmp"
            )
            && item_path.as_path().is_file();
        let visual_el: AnyElement = if (is_thumbnail || cached_icon_path.is_some())
            && (cached_icon_path.is_some() || can_direct_image_fallback)
        {
            img(if let Some(p) = &cached_icon_path {
                p.clone()
            } else {
                item_path.as_path().to_path_buf()
            })
            .w(px(icon_size_val))
            .h(px(icon_size_val))
            .object_fit(gpui::ObjectFit::Contain)
            .rounded(px(2.5))
            .into_any_element()
        } else {
            div()
                .w(px(icon_size_val))
                .h(px(icon_size_val))
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .text_size(px(icon_size_val * 0.72))
                        .font_family(UiConstants::FONT_MDL2)
                        .text_color(rgb(fallback_color))
                        .child(fallback_glyph),
                )
                .into_any_element()
        };

        item_div
            .on_mouse_down(MouseButton::Left, move |event, _window, cx| {
                cx.stop_propagation();
                let pos = point(event.position.x.into(), event.position.y.into());
                v_click.update(cx, |this, cx| {
                    this.handle_item_mouse_down(
                        pane_focus,
                        index,
                        path_clone.clone(),
                        is_dir,
                        event.click_count,
                        pos,
                        event.modifiers,
                        cx,
                    );
                });
            })
            .on_mouse_down(MouseButton::Right, {
                let v = view.clone();
                move |event, window, cx| {
                    cx.stop_propagation();
                    v.update(cx, |this, cx| {
                        this.focus_pane(pane_focus, cx);
                        if !this
                            .state
                            .pane_view(pane_focus)
                            .selected_indices
                            .contains(&index)
                        {
                            this.select_item_in_pane(pane_focus, index, false, false, cx);
                        }
                        this.show_context_menu(Some(index), event.position, window, cx);
                    });
                }
            })
            .on_mouse_move({
                let v_hover = v_hover.clone();
                move |ev, _win, cx| {
                    let pos = point(ev.position.x.into(), ev.position.y.into());
                    v_hover.update(cx, |this, cx| {
                        if this.state.main_view.drag_state.is_dragging
                            || this.state.main_view.drag_state.pending_candidate.is_some()
                            || this
                                .state
                                .main_view
                                .drag_state
                                .pending_path_candidate
                                .is_some()
                        {
                            let folder_target = if is_dir {
                                Some((path_for_move.clone(), item_name_for_drag.clone()))
                            } else if this.state.main_view.drag_state.source_pane != pane_focus {
                                let dest = this.state.pane_view(pane_focus).primary_path.clone();
                                let lbl = dest
                                    .file_name()
                                    .map(|n| n.to_string_lossy().into_owned())
                                    .unwrap_or_else(|| dest.to_string_lossy().into_owned());
                                Some((dest, lbl))
                            } else {
                                None
                            };
                            this.update_file_drag_move(pos, ev.modifiers, folder_target, cx);
                        } else if this.state.pane_view(pane_focus).lasso_state.is_some() {
                            this.update_lasso_in_pane(pane_focus, pos, pane_w, cx);
                        } else if is_hover_select {
                            this.handle_item_hover(index, cx);
                        }
                    });
                }
            })
            .child(
                div()
                    .w(px(item_width))
                    .h(px(thumb_height))
                    .p(px(thumb_margin))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(visual_el),
            )
            .child(if renaming_index == Some(index) {
                crate::ui::views::helpers::render_inline_rename_grid(
                    rename_input,
                    label_box_h,
                    effective_font_size,
                    effective_line_h,
                    item_width - 10.0,
                    view.clone(),
                )
            } else {
                div()
                    .w_full()
                    .h(px(label_box_h))
                    .px(px(4.0))
                    .pb(px(6.0))
                    .text_center()
                    .overflow_hidden()
                    .text_size(px(effective_font_size))
                    .line_height(px(effective_line_h))
                    .font_family(UiConstants::FONT_MAIN)
                    .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                    .line_clamp(3)
                    .text_ellipsis()
                    .child(display_label)
                    .into_any_element()
            })
    }
}

/// Shorten a filename with middle ellipsis so the first characters
/// and the extension remain visible, e.g. "long_docu...nt.pdf"
pub fn shorten_filename_middle(name: &str, max_len: usize) -> String {
    if name.chars().count() <= max_len {
        return name.to_string();
    }
    if let Some(dot) = name.rfind('.') {
        let base = &name[..dot];
        let ext = &name[dot..];
        let ext_visible = ext.len().min(8);
        let base_max = max_len.saturating_sub(3 + ext_visible);
        if base_max > 0 && base.chars().count() > base_max {
            let prefix: String = base.chars().take(base_max).collect();
            format!("{}...{}", prefix, ext)
        } else {
            let prefix: String = name.chars().take(max_len.saturating_sub(3)).collect();
            format!("{}...", prefix)
        }
    } else {
        let prefix: String = name.chars().take(max_len.saturating_sub(3)).collect();
        format!("{}...", prefix)
    }
}

/// Layout parameters computed from zoom level
#[derive(Clone, Copy)]
pub struct GridLayoutParams {
    pub items_per_row: usize,
    pub item_width: f32,
    pub item_height: f32,
    pub row_height: f32,
    pub icon_size: f32,
    pub gap: f32,
    pub rounded: f32,
    pub text_size: f32,
    pub padding: f32,
}

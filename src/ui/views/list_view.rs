//! List View & Cards (Tiles) View — Files-community List & Cards/Tiles parity
//!
//! Features:
//! - `ViewMode::List`: Responsive multi-column file listing supporting up to 3 lines of filename
//!   before reducing font size, with strict cell clipping so text never overlaps neighboring cells.
//! - `ViewMode::Cards`: Horizontal metadata cards with rounded icon/thumbnail badge,
//!   up to 3-line primary title, `Type • Size • Date` subtitle, tag pills, and multi-column grouped flow.
//! - Full Drag & Drop initiation, folder drop-target highlighting, Shift/Ctrl multi-select,
//!   and live rubber-band (lasso) selection.

use crate::config::config::GroupColumn;
use crate::core::navigation_manager::PaneFocus;
use crate::storage::storage::FileType;
use crate::ui::components::icon_renderer::is_thumbnail_image;
use crate::ui::constants::UiConstants;
use crate::ui::views::helpers::{
    adaptive_three_line_label, format_system_time, get_file_type_icon_and_color,
};
use crate::ui::views::main_view::{
    format_size, get_extension, FileItems, GroupedGridItem, MainView, MainViewState, SelectionMode,
};
use gpui::*;

impl MainViewState {
    pub fn render_list_view(
        &self,
        file_items: Entity<FileItems>,
        view: Entity<crate::app::AppWindow>,
    ) -> AnyElement {
        self.render_list_view_for_pane(
            file_items,
            view,
            PaneFocus::Primary,
            gpui::point(0.0, 128.0),
            1000.0,
        )
    }

    pub fn render_list_view_for_pane(
        &self,
        file_items: Entity<FileItems>,
        view: Entity<crate::app::AppWindow>,
        pane_focus: PaneFocus,
        list_origin: Point<f32>,
        pane_w: f32,
    ) -> AnyElement {
        let visible = self.visible_items_rc();

        if self.group_by != GroupColumn::None {
            return self
                .render_list_view_grouped_for_pane(
                    &visible,
                    file_items,
                    view,
                    pane_focus,
                    list_origin,
                    pane_w,
                )
                .into_any_element();
        }

        self.list_view_flat_for_pane(
            &visible,
            file_items,
            view,
            pane_focus,
            list_origin,
            pane_w,
        )
        .into_any_element()
    }

    #[allow(clippy::too_many_arguments)]
    fn render_single_list_cell(
        item: &crate::domain::metadata::FileItem,
        idx: usize,
        is_selected: bool,
        is_drop_target: bool,
        col_w: f32,
        row_h: f32,
        icon_size: f32,
        text_size: f32,
        is_checkbox: bool,
        is_hover_select: bool,
        lod_fast: bool,
        main_state: &MainViewState,
        icon_paths: &std::collections::HashMap<
            crate::core::path_util::FilesPath,
            std::path::PathBuf,
        >,
        pane_focus: PaneFocus,
        pane_w: f32,
        view_row: Entity<crate::app::AppWindow>,
    ) -> AnyElement {
        let is_dir = matches!(item.file_type, FileType::Directory);
        let is_cut = main_state.cut_paths.contains(item.path.as_path());
        let path = item.path.clone();
        let item_name_for_drag = item.name.clone();
        let display_name = main_state.display_name(item).into_owned();
        let ext = get_extension(&item.name);
        let (fallback_glyph, fallback_color) = if is_dir {
            ("\u{E8B7}", 0xF5B942u32)
        } else {
            get_file_type_icon_and_color(&ext)
        };

        let show_size_badge = !is_dir && !lod_fast && col_w >= 210.0;
        let avail_text_w = (col_w
            - icon_size
            - 26.0
            - if show_size_badge { 48.0 } else { 0.0 }
            - if is_checkbox && !lod_fast { 22.0 } else { 0.0 })
        .max(54.0);
        let (display_label, effective_font_size, effective_line_h) = adaptive_three_line_label(
            &display_name,
            avail_text_w,
            text_size,
            (text_size - 2.2).max(8.5),
        );

        let mut cell = div()
            .id(ElementId::from(
                idx + match pane_focus {
                    PaneFocus::Primary => 20000,
                    PaneFocus::Secondary => 120000,
                },
            ))
            .flex()
            .flex_shrink_0()
            .items_center()
            .overflow_hidden()
            .w(px(col_w))
            .h(px(row_h - 3.0))
            .px(px(8.0))
            .py(px(2.0))
            .gap(px(7.0))
            .rounded(px(5.0))
            .border(px(1.0))
            .border_color(if is_drop_target {
                rgba(0x60CDFFFFu32)
            } else if is_selected {
                rgba(0x60CDFF66u32)
            } else {
                rgba(0x00000000u32)
            })
            .bg(if is_drop_target {
                rgba(0x0078D444u32)
            } else if is_selected {
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
                let p_nav = path.clone();
                move |ev, _, cx| {
                    cx.stop_propagation();
                    let pos = point(ev.position.x.into(), ev.position.y.into());
                    v.update(cx, |this, cx| {
                        this.handle_item_mouse_down(
                            pane_focus,
                            idx,
                            p_nav.clone(),
                            is_dir,
                            ev.click_count,
                            pos,
                            ev.modifiers,
                            cx,
                        );
                    });
                }
            })
            .on_mouse_move({
                let v = view_row.clone();
                let p_drop = path.clone();
                let n_drop = item_name_for_drag;
                move |ev, _win, cx| {
                    let pos = point(ev.position.x.into(), ev.position.y.into());
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
                            let folder_target = if is_dir {
                                Some((p_drop.clone(), n_drop.clone()))
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
                            this.handle_item_hover(idx, cx);
                        }
                    });
                }
            })
            .on_mouse_down(MouseButton::Right, {
                let v = view_row.clone();
                move |ev, window, cx| {
                    cx.stop_propagation();
                    v.update(cx, |this, cx| {
                        this.focus_pane(pane_focus, cx);
                        if !this
                            .state
                            .pane_view(pane_focus)
                            .selected_indices
                            .contains(&idx)
                        {
                            this.select_item_in_pane(pane_focus, idx, false, false, cx);
                        }
                        this.show_context_menu(Some(idx), ev.position, window, cx);
                    });
                }
            });

        if is_selected {
            cell = cell.child(
                div()
                    .w(px(3.0))
                    .h(px(16.0))
                    .flex_shrink_0()
                    .rounded(px(1.5))
                    .bg(rgb(UiConstants::ACCENT_COLOR_RGB)),
            );
        }

        if is_cut {
            cell = cell
                .opacity(0.55)
                .border(px(1.0))
                .border_dashed()
                .border_color(rgba(0x60CDFFCCu32));
        }

        if is_checkbox && !lod_fast {
            let v_cb = view_row.clone();
            cell = cell.child(
                div()
                    .w(px(16.0))
                    .h(px(16.0))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        cx.stop_propagation();
                        v_cb.update(cx, |this, cx| {
                            this.focus_pane(pane_focus, cx);
                            this.toggle_checkbox_selection(idx, cx);
                        });
                    })
                    .child(
                        div()
                            .w(px(15.0))
                            .h(px(15.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .border(px(1.5))
                            .border_color(if is_selected {
                                rgba(UiConstants::ACCENT_RGBA)
                            } else {
                                rgba(0xFFFFFF80u32)
                            })
                            .rounded(px(3.0))
                            .bg(if is_selected {
                                rgba(UiConstants::ACCENT_RGBA)
                            } else {
                                rgba(0x00000000)
                            })
                            .child(if is_selected {
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

        cell = cell
            .child(
                div()
                    .w(px(icon_size))
                    .h(px(icon_size))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(if !lod_fast {
                        if let Some(icon_path) = icon_paths.get(&path) {
                            img(icon_path.clone())
                                .w(px(icon_size))
                                .h(px(icon_size))
                                .into_any_element()
                        } else {
                            div()
                                .text_size(px(icon_size * 0.72))
                                .font_family(UiConstants::FONT_MDL2)
                                .text_color(rgb(fallback_color))
                                .child(fallback_glyph)
                                .into_any_element()
                        }
                    } else {
                        div()
                            .text_size(px(icon_size * 0.72))
                            .font_family(UiConstants::FONT_MDL2)
                            .text_color(rgb(fallback_color))
                            .child(fallback_glyph)
                            .into_any_element()
                    }),
            )
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.0))
                    .max_h(px(row_h - 6.0))
                    .overflow_hidden()
                    .text_size(px(effective_font_size))
                    .line_height(px(effective_line_h))
                    .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                    .line_clamp(3)
                    .text_ellipsis()
                    .child(if main_state.renaming_index == Some(idx) {
                        crate::ui::views::helpers::render_inline_rename_element(
                            &main_state.rename_text,
                            main_state.rename_selected_all,
                            main_state.rename_cursor_pos,
                            view_row.clone(),
                        )
                    } else {
                        display_label.into_any_element()
                    }),
            );

        if show_size_badge {
            cell = cell.child(
                div()
                    .flex_shrink_0()
                    .text_size(px(10.0))
                    .line_height(px(13.0))
                    .text_color(rgba(UiConstants::TEXT_MUTED_RGBA))
                    .child(format_size(item.size)),
            );
        }

        cell.into_any_element()
    }

    fn list_view_flat_for_pane(
        &self,
        visible: &[usize],
        file_items: Entity<FileItems>,
        view: Entity<crate::app::AppWindow>,
        pane_focus: PaneFocus,
        list_origin: Point<f32>,
        pane_w: f32,
    ) -> impl IntoElement {
        let zoom = self.zoom_level;
        let compact = self.compact_mode;
        let (cols, _col_w, _row_h, _gap, _padding) = Self::list_layout_metrics(pane_w, zoom, compact);
        let row_count = (visible.len() + cols - 1) / cols.max(1);

        div()
            .id(SharedString::from(match pane_focus {
                PaneFocus::Primary => "list-container-primary",
                PaneFocus::Secondary => "list-container-secondary",
            }))
            .relative()
            .size_full()
            .flex_col()
            .on_mouse_down(MouseButton::Right, {
                let v = view.clone();
                move |event, window, cx| {
                    v.update(cx, |this, cx| {
                        this.focus_pane(pane_focus, cx);
                        this.commit_rename_if_active(cx);
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
                        PaneFocus::Primary => "list-scroll-primary",
                        PaneFocus::Secondary => "list-scroll-secondary",
                    },
                    row_count,
                    move |range, _win, cx| {
                        let fi = file_items.read(cx);
                        let app_state = view.read(cx);
                        let main_state = app_state.state.pane_view(pane_focus);
                        let visible_rc = main_state.visible_items_rc();
                        let icon_paths = &app_state.state.main_view.icon_paths;
                        let zoom = main_state.zoom_level;
                        let compact = main_state.compact_mode;
                        let (cols, col_w, row_h, gap, padding) =
                            Self::list_layout_metrics(pane_w, zoom, compact);
                        let icon_size = (18.0 + zoom * 10.0).clamp(18.0, 28.0);
                        let text_size = (11.0 + zoom * 2.5).clamp(11.0, 13.5);
                        let is_checkbox = main_state.selection_mode == SelectionMode::Checkbox;
                        let is_hover_select =
                            main_state.selection_mode == SelectionMode::HoverSelect;
                        let hovered_drop_folder = app_state
                            .state
                            .main_view
                            .drag_state
                            .hovered_target_folder
                            .clone();
                        let is_dragging_any = app_state.state.main_view.drag_state.is_dragging;
                        let lod_fast = zoom < 0.20;

                        range
                            .map(|row_idx| {
                                let start = row_idx * cols;
                                let end = (start + cols).min(visible_rc.len());
                                let items_in_row = end.saturating_sub(start);

                                let mut row_cells: Vec<AnyElement> = (start..end)
                                    .filter_map(|i| {
                                        let idx = visible_rc[i];
                                        let item = fi.items.get(idx)?;
                                        let is_selected = main_state.selected_indices.contains(&idx);
                                        let is_dir = matches!(item.file_type, FileType::Directory);
                                        let is_drop_target = is_dragging_any
                                            && is_dir
                                            && hovered_drop_folder.as_ref() == Some(&item.path);

                                        Some(Self::render_single_list_cell(
                                            item,
                                            idx,
                                            is_selected,
                                            is_drop_target,
                                            col_w,
                                            row_h,
                                            icon_size,
                                            text_size,
                                            is_checkbox,
                                            is_hover_select,
                                            lod_fast,
                                            main_state,
                                            icon_paths,
                                            pane_focus,
                                            pane_w,
                                            view.clone(),
                                        ))
                                    })
                                    .collect();

                                for _ in items_in_row..cols {
                                    row_cells.push(
                                        div()
                                            .w(px(col_w))
                                            .flex_shrink_0()
                                            .invisible()
                                            .into_any_element(),
                                    );
                                }

                                div()
                                    .w_full()
                                    .h(px(row_h))
                                    .flex_shrink_0()
                                    .px(px(padding))
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap(px(gap))
                                    .children(row_cells)
                                    .into_any_element()
                            })
                            .collect::<Vec<_>>()
                    },
                )
                .with_sizing_behavior(ListSizingBehavior::Auto)
                .w_full()
                .h_full()
                .track_scroll(self.uniform_list_scroll_handle.clone()),
            )
            .child(MainView::render_lasso_box(self))
    }

    fn render_list_view_grouped_for_pane(
        &self,
        visible: &[usize],
        _file_items: Entity<FileItems>,
        view: Entity<crate::app::AppWindow>,
        pane_focus: PaneFocus,
        list_origin: Point<f32>,
        pane_w: f32,
    ) -> impl IntoElement {
        let zoom = self.zoom_level;
        let compact = self.compact_mode;
        let (cols, col_w, row_h, gap, padding) = Self::list_layout_metrics(pane_w, zoom, compact);
        let flattened = self.flatten_grouped_grid_items(visible, cols.max(1));
        let header_h = if compact { 28.0 } else { 32.0 };
        let icon_size = (18.0 + zoom * 10.0).clamp(18.0, 28.0);
        let text_size = (11.0 + zoom * 2.5).clamp(11.0, 13.5);
        let is_checkbox = self.selection_mode == SelectionMode::Checkbox;
        let is_hover_select = self.selection_mode == SelectionMode::HoverSelect;
        let hovered_drop_folder = self.drag_state.hovered_target_folder.clone();
        let is_dragging_any = self.drag_state.is_dragging;
        let lod_fast = zoom < 0.20;

        let item_heights: Vec<f32> = flattened
            .iter()
            .map(|entry| match entry {
                GroupedGridItem::Header { .. } => header_h,
                GroupedGridItem::Row(_) => row_h,
            })
            .collect();

        let scroll_list = MainView::render_virtualized_grouped_list(
            match pane_focus {
                PaneFocus::Primary => "list-grouped-scroll-primary",
                PaneFocus::Secondary => "list-grouped-scroll-secondary",
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
                        padding,
                        pane_focus,
                        view.clone(),
                    )
                }
                GroupedGridItem::Row(indices) => {
                    let items_in_row = indices.len();
                    let mut row_cells: Vec<AnyElement> = indices
                        .iter()
                        .filter_map(|&idx| {
                            let item = self.items.get(idx)?;
                            let is_selected = self.selected_indices.contains(&idx);
                            let is_dir = matches!(item.file_type, FileType::Directory);
                            let is_drop_target = is_dragging_any
                                && is_dir
                                && hovered_drop_folder.as_ref() == Some(&item.path);

                            Some(Self::render_single_list_cell(
                                item,
                                idx,
                                is_selected,
                                is_drop_target,
                                col_w,
                                row_h,
                                icon_size,
                                text_size,
                                is_checkbox,
                                is_hover_select,
                                lod_fast,
                                self,
                                &self.icon_paths,
                                pane_focus,
                                pane_w,
                                view.clone(),
                            ))
                        })
                        .collect();

                    for _ in items_in_row..cols {
                        row_cells.push(
                            div()
                                .w(px(col_w))
                                .flex_shrink_0()
                                .invisible()
                                .into_any_element(),
                        );
                    }

                    div()
                        .w_full()
                        .h(px(row_h))
                        .flex_shrink_0()
                        .px(px(padding))
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(gap))
                        .children(row_cells)
                        .into_any_element()
                }
            },
        );

        div()
            .id(SharedString::from(match pane_focus {
                PaneFocus::Primary => "list-grouped-container-primary",
                PaneFocus::Secondary => "list-grouped-container-secondary",
            }))
            .relative()
            .flex_col()
            .w_full()
            .flex_grow()
            .h_full()
            .on_mouse_down(MouseButton::Right, {
                let v = view.clone();
                move |event, window, cx| {
                    v.update(cx, |this, cx| {
                        this.focus_pane(pane_focus, cx);
                        this.commit_rename_if_active(cx);
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

    // ═════════════════════════════════════════════════════════════════════════
    // CARDS / TILES VIEW (`ViewMode::Cards` — Files-community Tiles/Cards parity)
    // ═════════════════════════════════════════════════════════════════════════

    pub fn render_cards_view_for_pane(
        &self,
        file_items: Entity<FileItems>,
        view: Entity<crate::app::AppWindow>,
        pane_focus: PaneFocus,
        list_origin: Point<f32>,
        pane_w: f32,
    ) -> AnyElement {
        let visible = self.visible_items_rc();
        let zoom = self.zoom_level;
        let compact = self.compact_mode;
        let (cols, col_w, row_h, gap, padding) = Self::cards_layout_metrics(pane_w, zoom, compact);

        if self.group_by != GroupColumn::None {
            let flattened = self.flatten_grouped_grid_items(&visible, cols.max(1));
            let header_h = if compact { 28.0 } else { 32.0 };
            let hovered_drop_folder = self.drag_state.hovered_target_folder.clone();
            let is_dragging_any = self.drag_state.is_dragging;

            let item_heights: Vec<f32> = flattened
                .iter()
                .map(|entry| match entry {
                    GroupedGridItem::Header { .. } => header_h,
                    GroupedGridItem::Row(_) => row_h,
                })
                .collect();

            let scroll_list = MainView::render_virtualized_grouped_list(
                match pane_focus {
                    PaneFocus::Primary => "cards-grouped-scroll-primary",
                    PaneFocus::Secondary => "cards-grouped-scroll-secondary",
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
                            padding,
                            pane_focus,
                            view.clone(),
                        )
                    }
                    GroupedGridItem::Row(indices) => {
                        let items_in_row = indices.len();
                        let mut row_cells: Vec<AnyElement> = indices
                            .iter()
                            .filter_map(|&idx| {
                                let item = self.items.get(idx)?;
                                let is_selected = self.selected_indices.contains(&idx);
                                let is_dir = matches!(item.file_type, FileType::Directory);
                                let is_drop_target = is_dragging_any
                                    && is_dir
                                    && hovered_drop_folder.as_ref() == Some(&item.path);
                                Some(Self::render_single_card_cell(
                                    item,
                                    idx,
                                    is_selected,
                                    is_drop_target,
                                    col_w,
                                    row_h,
                                    zoom,
                                    self,
                                    &self.icon_paths,
                                    pane_focus,
                                    pane_w,
                                    view.clone(),
                                ))
                            })
                            .collect();

                        for _ in items_in_row..cols {
                            row_cells.push(
                                div()
                                    .w(px(col_w))
                                    .flex_shrink_0()
                                    .invisible()
                                    .into_any_element(),
                            );
                        }

                        div()
                            .w_full()
                            .h(px(row_h))
                            .flex_shrink_0()
                            .px(px(padding))
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(gap))
                            .children(row_cells)
                            .into_any_element()
                    }
                },
            );

            return div()
                .id(SharedString::from(match pane_focus {
                    PaneFocus::Primary => "cards-grouped-container-primary",
                    PaneFocus::Secondary => "cards-grouped-container-secondary",
                }))
                .relative()
                .size_full()
                .flex_col()
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
                                let pane_target = if this.state.main_view.drag_state.source_pane
                                    != pane_focus
                                {
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
                .into_any_element();
        }

        let row_count = (visible.len() + cols - 1) / cols.max(1);

        div()
            .id(SharedString::from(match pane_focus {
                PaneFocus::Primary => "cards-container-primary",
                PaneFocus::Secondary => "cards-container-secondary",
            }))
            .relative()
            .size_full()
            .flex_col()
            .on_mouse_down(MouseButton::Right, {
                let v = view.clone();
                move |event, window, cx| {
                    v.update(cx, |this, cx| {
                        this.focus_pane(pane_focus, cx);
                        this.commit_rename_if_active(cx);
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
                        PaneFocus::Primary => "cards-scroll-primary",
                        PaneFocus::Secondary => "cards-scroll-secondary",
                    },
                    row_count,
                    move |range, _win, cx| {
                        let fi = file_items.read(cx);
                        let app_state = view.read(cx);
                        let main_state = app_state.state.pane_view(pane_focus);
                        let visible_rc = main_state.visible_items_rc();
                        let icon_paths = &app_state.state.main_view.icon_paths;
                        let zoom = main_state.zoom_level;
                        let compact = main_state.compact_mode;
                        let (cols, col_w, row_h, gap, padding) =
                            Self::cards_layout_metrics(pane_w, zoom, compact);
                        let hovered_drop_folder = app_state
                            .state
                            .main_view
                            .drag_state
                            .hovered_target_folder
                            .clone();
                        let is_dragging_any = app_state.state.main_view.drag_state.is_dragging;

                        range
                            .map(|row_idx| {
                                let start = row_idx * cols;
                                let end = (start + cols).min(visible_rc.len());
                                let items_in_row = end.saturating_sub(start);

                                let mut row_cells: Vec<AnyElement> = (start..end)
                                    .filter_map(|i| {
                                        let idx = visible_rc[i];
                                        let item = fi.items.get(idx)?;
                                        let is_selected = main_state.selected_indices.contains(&idx);
                                        let is_dir = matches!(item.file_type, FileType::Directory);
                                        let is_drop_target = is_dragging_any
                                            && is_dir
                                            && hovered_drop_folder.as_ref() == Some(&item.path);

                                        Some(Self::render_single_card_cell(
                                            item,
                                            idx,
                                            is_selected,
                                            is_drop_target,
                                            col_w,
                                            row_h,
                                            zoom,
                                            main_state,
                                            icon_paths,
                                            pane_focus,
                                            pane_w,
                                            view.clone(),
                                        ))
                                    })
                                    .collect();

                                for _ in items_in_row..cols {
                                    row_cells.push(
                                        div()
                                            .w(px(col_w))
                                            .flex_shrink_0()
                                            .invisible()
                                            .into_any_element(),
                                    );
                                }

                                div()
                                    .w_full()
                                    .h(px(row_h))
                                    .flex_shrink_0()
                                    .px(px(padding))
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap(px(gap))
                                    .children(row_cells)
                                    .into_any_element()
                            })
                            .collect::<Vec<_>>()
                    },
                )
                .with_sizing_behavior(ListSizingBehavior::Auto)
                .w_full()
                .h_full()
                .track_scroll(self.uniform_list_scroll_handle.clone()),
            )
            .child(MainView::render_lasso_box(self))
            .into_any_element()
    }

    #[allow(clippy::too_many_arguments)]
    fn render_single_card_cell(
        item: &crate::domain::metadata::FileItem,
        idx: usize,
        is_selected: bool,
        is_drop_target: bool,
        col_w: f32,
        row_h: f32,
        zoom: f32,
        main_state: &MainViewState,
        icon_paths: &std::collections::HashMap<
            crate::core::path_util::FilesPath,
            std::path::PathBuf,
        >,
        pane_focus: PaneFocus,
        pane_w: f32,
        view: Entity<crate::app::AppWindow>,
    ) -> AnyElement {
        let is_dir = matches!(item.file_type, FileType::Directory);
        let is_cut = main_state.cut_paths.contains(item.path.as_path());
        let path = item.path.clone();
        let item_name_for_drag = item.name.clone();
        let display_name = main_state.display_name(item).into_owned();
        let ext = get_extension(&item.name);
        let is_thumb = !is_dir && is_thumbnail_image(&ext);
        let (fallback_glyph, fallback_color) = if is_dir {
            ("\u{E8B7}", 0xF5B942u32)
        } else {
            get_file_type_icon_and_color(&ext)
        };
        let is_checkbox = main_state.selection_mode == SelectionMode::Checkbox;
        let is_hover_select = main_state.selection_mode == SelectionMode::HoverSelect;
        let tag_badge = main_state
            .item_tags(item)
            .and_then(|t| t.first())
            .cloned();

        let card_h = (row_h - 6.0).max(58.0);
        let badge_box = (card_h - 18.0).clamp(36.0, 54.0);
        let title_size = (11.5 + zoom * 2.0).clamp(11.5, 13.5);
        let sub_size = (10.0 + zoom * 1.5).clamp(10.0, 11.5);
        let avail_title_w = (col_w
            - badge_box
            - 36.0
            - if is_checkbox { 22.0 } else { 0.0 }
            - if tag_badge.is_some() { 44.0 } else { 0.0 })
        .max(64.0);
        let (display_label, effective_title_size, effective_title_line_h) =
            adaptive_three_line_label(
                &display_name,
                avail_title_w,
                title_size,
                (title_size - 2.0).max(9.0),
            );

        let date_str = format_system_time(item.modified);
        let subtitle = if is_dir {
            format!("File folder • {}", date_str)
        } else {
            let type_lbl = if ext.is_empty() {
                "File".to_string()
            } else {
                ext.to_uppercase()
            };
            format!("{} • {} • {}", type_lbl, format_size(item.size), date_str)
        };

        let icon_el: AnyElement = if is_thumb {
            let img_src = icon_paths
                .get(&path)
                .cloned()
                .unwrap_or_else(|| path.as_path().to_path_buf());
            img(img_src)
                .w(px(badge_box))
                .h(px(badge_box))
                .object_fit(ObjectFit::Cover)
                .rounded(px(6.0))
                .into_any_element()
        } else if let Some(icon_path) = icon_paths.get(&path) {
            img(icon_path.clone())
                .w(px(badge_box * 0.78))
                .h(px(badge_box * 0.78))
                .object_fit(ObjectFit::Contain)
                .into_any_element()
        } else {
            div()
                .text_size(px(badge_box * 0.56))
                .font_family(UiConstants::FONT_MDL2)
                .text_color(rgb(fallback_color))
                .child(fallback_glyph)
                .into_any_element()
        };

        let v_cb = view.clone();
        let mut card = div()
            .id(ElementId::from(
                idx + match pane_focus {
                    PaneFocus::Primary => 30000,
                    PaneFocus::Secondary => 130000,
                },
            ))
            .w(px(col_w))
            .h(px(card_h))
            .flex_shrink_0()
            .overflow_hidden()
            .flex()
            .flex_row()
            .items_center()
            .px(px(10.0))
            .py(px(4.0))
            .gap(px(10.0))
            .rounded(px(7.0))
            .cursor_pointer()
            .border(px(1.0))
            .border_color(if is_drop_target {
                rgba(0x60CDFFFFu32)
            } else if is_selected {
                rgba(0x60CDFF99u32)
            } else {
                rgba(0xFFFFFF10u32)
            })
            .bg(if is_drop_target {
                rgba(0x0078D444u32)
            } else if is_selected {
                rgba(UiConstants::BG_ITEM_SELECTED_RGBA)
            } else {
                rgba(0xFFFFFF06u32)
            })
            .hover(|s| {
                s.bg(if is_selected {
                    rgba(UiConstants::BG_ITEM_SELECTED_HOVER_RGBA)
                } else {
                    rgba(0xFFFFFF12u32)
                })
            })
            .on_mouse_down(MouseButton::Left, {
                let v = view.clone();
                let p_nav = path.clone();
                move |ev, _, cx| {
                    cx.stop_propagation();
                    let pos = point(ev.position.x.into(), ev.position.y.into());
                    v.update(cx, |this, cx| {
                        this.handle_item_mouse_down(
                            pane_focus,
                            idx,
                            p_nav.clone(),
                            is_dir,
                            ev.click_count,
                            pos,
                            ev.modifiers,
                            cx,
                        );
                    });
                }
            })
            .on_mouse_move({
                let v = view.clone();
                let p_drop = path.clone();
                let n_drop = item_name_for_drag;
                move |ev, _win, cx| {
                    let pos = point(ev.position.x.into(), ev.position.y.into());
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
                            let folder_target = if is_dir {
                                Some((p_drop.clone(), n_drop.clone()))
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
                            this.handle_item_hover(idx, cx);
                        }
                    });
                }
            })
            .on_mouse_down(MouseButton::Right, {
                let v = view.clone();
                move |ev, window, cx| {
                    cx.stop_propagation();
                    v.update(cx, |this, cx| {
                        this.focus_pane(pane_focus, cx);
                        if !this
                            .state
                            .pane_view(pane_focus)
                            .selected_indices
                            .contains(&idx)
                        {
                            this.select_item_in_pane(pane_focus, idx, false, false, cx);
                        }
                        this.show_context_menu(Some(idx), ev.position, window, cx);
                    });
                }
            });

        if is_selected {
            card = card.child(
                div()
                    .w(px(3.0))
                    .h(px((card_h * 0.55).clamp(16.0, 28.0)))
                    .flex_shrink_0()
                    .rounded(px(1.5))
                    .bg(rgb(UiConstants::ACCENT_COLOR_RGB)),
            );
        }

        if is_cut {
            card = card
                .opacity(0.55)
                .border(px(1.0))
                .border_dashed()
                .border_color(rgba(0x60CDFFCCu32));
        }

        if is_checkbox {
            card = card.child(
                div()
                    .w(px(16.0))
                    .h(px(16.0))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        cx.stop_propagation();
                        v_cb.update(cx, |this, cx| {
                            this.focus_pane(pane_focus, cx);
                            this.toggle_checkbox_selection(idx, cx);
                        });
                    })
                    .child(
                        div()
                            .w(px(15.0))
                            .h(px(15.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .border(px(1.5))
                            .border_color(if is_selected {
                                rgba(UiConstants::ACCENT_RGBA)
                            } else {
                                rgba(0xFFFFFF80u32)
                            })
                            .rounded(px(3.0))
                            .bg(if is_selected {
                                rgba(UiConstants::ACCENT_RGBA)
                            } else {
                                rgba(0x00000000)
                            })
                            .child(if is_selected {
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

        card.child(
            div()
                .w(px(badge_box))
                .h(px(badge_box))
                .flex_shrink_0()
                .rounded(px(6.0))
                .bg(rgba(0xFFFFFF08u32))
                .flex()
                .items_center()
                .justify_center()
                .overflow_hidden()
                .child(icon_el),
        )
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .max_h(px(card_h - 6.0))
                .flex_col()
                .justify_center()
                .gap(px(2.0))
                .overflow_hidden()
                .child(
                    div()
                        .w_full()
                        .flex()
                        .items_start()
                        .gap(px(6.0))
                        .overflow_hidden()
                        .child(if main_state.renaming_index == Some(idx) {
                            crate::ui::views::helpers::render_inline_rename_element(
                                &main_state.rename_text,
                                main_state.rename_selected_all,
                                main_state.rename_cursor_pos,
                                view.clone(),
                            )
                        } else {
                            div()
                                .flex_1()
                                .min_w(px(0.0))
                                .overflow_hidden()
                                .text_size(px(effective_title_size))
                                .line_height(px(effective_title_line_h))
                                .font_weight(FontWeight(600.0))
                                .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                                .line_clamp(3)
                                .text_ellipsis()
                                .child(display_label)
                                .into_any_element()
                        })
                        .children(tag_badge.map(|tag| {
                            div()
                                .flex_shrink_0()
                                .px(px(5.0))
                                .py(px(1.0))
                                .rounded(px(3.0))
                                .bg(rgba(0xB4A0FF28u32))
                                .text_size(px(9.5))
                                .line_height(px(12.0))
                                .text_color(rgb(0xB4A0FF))
                                .child(tag)
                        })),
                )
                .child(
                    div()
                        .w_full()
                        .truncate()
                        .text_size(px(sub_size))
                        .line_height(px(14.0))
                        .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                        .child(subtitle),
                ),
        )
        .into_any_element()
    }
}

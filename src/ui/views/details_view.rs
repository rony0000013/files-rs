use crate::config::config::{GroupColumn, SortBy};
use crate::core::navigation_manager::PaneFocus;
use crate::storage::storage::FileType;
use crate::ui::constants::UiConstants;
use crate::ui::views::helpers::{format_system_time, get_file_type_icon_and_color};
use crate::ui::views::main_view::{
    format_size, get_extension, FileItems, GroupedItemType, MainView, MainViewState, SelectionMode,
};
use gpui::*;

impl MainView {
    pub fn details_view(
        state: &MainViewState,
        visible: &[usize],
        file_items: Entity<FileItems>,
        view: Entity<crate::app::AppWindow>,
    ) -> AnyElement {
        Self::details_view_for_pane(
            state,
            visible,
            file_items,
            view,
            PaneFocus::Primary,
            gpui::point(0.0, 158.0),
            1000.0,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn render_details_row(
        item: &crate::domain::metadata::FileItem,
        idx: usize,
        is_selected: bool,
        is_drop_target: bool,
        row_height: f32,
        zoom: f32,
        main_state: &MainViewState,
        icon_paths: &std::collections::HashMap<
            crate::core::path_util::FilesPath,
            std::path::PathBuf,
        >,
        pane_focus: PaneFocus,
        pane_w: f32,
        view_row: Entity<crate::app::AppWindow>,
    ) -> AnyElement {
        let widths = main_state.column_widths;
        let col_vis = main_state.column_visibility;
        let is_checkbox = main_state.selection_mode == SelectionMode::Checkbox;
        let is_hover_select = main_state.selection_mode == SelectionMode::HoverSelect;
        let renaming_idx = main_state.renaming_index;
        let rename_text = &main_state.rename_text;
        let lod_fast = zoom < 0.20;

        let is_dir = matches!(item.file_type, FileType::Directory);
        let display_name = main_state.display_name(item).into_owned();
        let ext = get_extension(&item.name);
        let (fallback_glyph, fallback_color) = if is_dir {
            ("\u{E8B7}", 0xF5B942u32)
        } else {
            get_file_type_icon_and_color(&ext)
        };

        let icon_size = (row_height - 8.0).clamp(15.0, 36.0);
        let text_size = (11.5 + zoom * 3.5).clamp(11.0, 15.0);
        let single_line_chars = ((widths.name / (text_size * 0.56)).floor() as usize).max(16);
        let effective_name_size = if display_name.chars().count() > single_line_chars {
            (text_size - 1.2).max(10.0)
        } else {
            text_size
        };
        let name_line_h = (effective_name_size * 1.24).ceil();
        let text_size_sm = (10.5 + zoom * 2.5).clamp(10.0, 13.5);
        let sm_line_h = (text_size_sm * 1.24).ceil();
        let item_path = item.path.clone();
        let item_name_for_drag = item.name.clone();
        let tag_badge = main_state
            .item_tags(item)
            .and_then(|t| t.first())
            .cloned();

        let mut row = div()
            .w_full()
            .id(ElementId::from(
                idx + match pane_focus {
                    PaneFocus::Primary => 10000,
                    PaneFocus::Secondary => 110000,
                },
            ))
            .flex()
            .flex_row()
            .flex_shrink_0()
            .overflow_hidden()
            .h(px(row_height))
            .items_center()
            .rounded(px(5.0))
            .px(px(UiConstants::PADDING_LARGE_RAW as f32))
            .cursor_pointer()
            .border(px(1.0))
            .border_color(if is_drop_target {
                rgba(0x60CDFFFFu32)
            } else if is_selected {
                rgba(0x60CDFF44u32)
            } else {
                rgba(0x00000000u32)
            })
            .on_mouse_down(MouseButton::Left, {
                let v = view_row.clone();
                let path = item_path.clone();
                move |ev, _window, cx| {
                    cx.stop_propagation();
                    let pos = point(ev.position.x.into(), ev.position.y.into());
                    v.update(cx, |this, cx| {
                        this.handle_item_mouse_down(
                            pane_focus,
                            idx,
                            path.clone(),
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
                let p_drop = item_path.clone();
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

        if is_drop_target {
            row = row.bg(rgba(0x0078D444u32));
        } else if is_selected {
            row = row
                .bg(rgba(UiConstants::BG_ITEM_SELECTED_RGBA))
                .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_SELECTED_HOVER_RGBA)))
                .child(
                    div()
                        .w(px(3.0))
                        .h(px((row_height * 0.58).clamp(12.0, 22.0)))
                        .flex_shrink_0()
                        .mr(px(6.0))
                        .rounded(px(1.5))
                        .bg(rgb(UiConstants::ACCENT_COLOR_RGB)),
                );
        } else {
            row = row.hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)));
        }

        let mut name_row = div()
            .flex_grow()
            .min_w(px(100.0))
            .overflow_hidden()
            .flex()
            .items_center()
            .gap(px(8.0));

        if is_checkbox && !lod_fast {
            let v_cb = view_row;
            name_row = name_row.child(
                div()
                    .w(px(18.0))
                    .h(px(18.0))
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
                            .w(px(14.0))
                            .h(px(14.0))
                            .border(px(1.5))
                            .border_color(rgba(UiConstants::ACCENT_RGBA))
                            .rounded(px(3.0))
                            .bg(if is_selected {
                                rgba(UiConstants::ACCENT_RGBA)
                            } else {
                                rgba(0x00000000)
                            })
                            .child(if is_selected {
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

        name_row = name_row
            .child(
                div()
                    .w(px(icon_size))
                    .h(px(icon_size))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(if !lod_fast {
                        if let Some(icon_path) = icon_paths.get(&item.path) {
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
                    .w(px(widths.name))
                    .flex_grow()
                    .min_w(px(60.0))
                    .overflow_hidden()
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .text_size(px(effective_name_size))
                    .line_height(px(name_line_h))
                    .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                    .child(if renaming_idx == Some(idx) {
                        div()
                            .bg(rgba(0xFFFFFF20u32))
                            .border(px(1.0))
                            .border_color(rgba(UiConstants::ACCENT_RGBA))
                            .rounded(px(3.0))
                            .px(px(6.0))
                            .py(px(1.0))
                            .child(rename_text.clone())
                            .into_any_element()
                    } else {
                        div()
                            .flex_1()
                            .min_w(px(0.0))
                            .truncate()
                            .child(display_name)
                            .into_any_element()
                    })
                    .children(if !lod_fast {
                        tag_badge.map(|tag| {
                            div()
                                .flex_shrink_0()
                                .px(px(5.0))
                                .py(px(1.0))
                                .rounded(px(3.0))
                                .bg(rgba(0xB4A0FF28u32))
                                .text_size(px(10.0))
                                .line_height(px(12.0))
                                .text_color(rgb(0xB4A0FF))
                                .child(tag)
                                .into_any_element()
                        })
                    } else {
                        None
                    }),
            );

        let mut row_children: Vec<AnyElement> = vec![name_row.into_any_element()];

        if col_vis.date {
            let date_str = format_system_time(item.modified);
            row_children.push(div().flex_grow().min_w(px(6.0)).into_any_element());
            row_children.push(
                div()
                    .w(px(widths.date))
                    .flex_shrink_0()
                    .truncate()
                    .text_size(px(text_size_sm))
                    .line_height(px(sm_line_h))
                    .text_color(rgb(UiConstants::TEXT_SECONDARY_RGB))
                    .child(date_str)
                    .into_any_element(),
            );
        }
        if col_vis.type_col {
            let type_str = if is_dir {
                "File folder".to_string()
            } else if ext.is_empty() {
                "File".to_string()
            } else {
                format!("{} File", ext.to_uppercase())
            };
            row_children.push(div().w(px(6.0)).into_any_element());
            row_children.push(
                div()
                    .w(px(widths.type_col))
                    .flex_shrink_0()
                    .truncate()
                    .text_size(px(text_size_sm))
                    .line_height(px(sm_line_h))
                    .text_color(rgb(UiConstants::TEXT_SECONDARY_RGB))
                    .child(type_str)
                    .into_any_element(),
            );
        }
        if col_vis.size {
            row_children.push(div().w(px(6.0)).into_any_element());
            row_children.push(
                div()
                    .w(px(widths.size))
                    .flex_shrink_0()
                    .truncate()
                    .text_size(px(text_size_sm))
                    .line_height(px(sm_line_h))
                    .text_color(rgb(UiConstants::TEXT_SECONDARY_RGB))
                    .child(if is_dir {
                        String::new()
                    } else {
                        format_size(item.size)
                    })
                    .into_any_element(),
            );
        }

        row.children(row_children).into_any_element()
    }

    pub fn details_view_for_pane(
        state: &MainViewState,
        visible: &[usize],
        file_items: Entity<FileItems>,
        view: Entity<crate::app::AppWindow>,
        pane_focus: PaneFocus,
        list_origin: Point<f32>,
        pane_w: f32,
    ) -> AnyElement {
        if state.group_by != GroupColumn::None {
            return Self::details_view_grouped_for_pane(
                state,
                visible,
                file_items,
                view,
                pane_focus,
                list_origin,
                pane_w,
            )
            .into_any_element();
        }

        let item_count = visible.len();

        div()
            .flex_col()
            .w_full()
            .flex_grow()
            .h_full()
            .child(Self::column_header_for_pane(state, view.clone(), pane_focus))
            .child(
                div()
                    .id(SharedString::from(match pane_focus {
                        PaneFocus::Primary => "details-list-container-primary",
                        PaneFocus::Secondary => "details-list-container-secondary",
                    }))
                    .flex_col()
                    .w_full()
                    .flex_grow()
                    .h_full()
                    .relative()
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
                    .child(
                        uniform_list(
                            match pane_focus {
                                PaneFocus::Primary => "details-scroll-primary",
                                PaneFocus::Secondary => "details-scroll-secondary",
                            },
                            item_count,
                            move |range, _win, cx| {
                                let fi = file_items.read(cx);
                                let app_state = view.read(cx);
                                let main_state = app_state.state.pane_view(pane_focus);
                                let visible_rc = main_state.visible_items_rc();
                                let zoom = main_state.zoom_level;
                                let compact = main_state.compact_mode;
                                let icon_paths = &app_state.state.main_view.icon_paths;
                                let hovered_drop_folder = app_state
                                    .state
                                    .main_view
                                    .drag_state
                                    .hovered_target_folder
                                    .clone();
                                let is_dragging_any = app_state.state.main_view.drag_state.is_dragging;
                                let base_row_h = if compact { 22.0 } else { 26.0 };
                                let row_height = (base_row_h + zoom * 22.0).round();

                                range
                                    .map(|i| {
                                        if i >= visible_rc.len() {
                                            return div().into_any_element();
                                        }
                                        let idx = visible_rc[i];
                                        if idx >= fi.items.len() {
                                            return div().into_any_element();
                                        }
                                        let item = &fi.items[idx];
                                        let is_selected = main_state.selected_indices.contains(&idx);
                                        let is_dir = matches!(item.file_type, FileType::Directory);
                                        let is_drop_target = is_dragging_any
                                            && is_dir
                                            && hovered_drop_folder.as_ref() == Some(&item.path);

                                        Self::render_details_row(
                                            item,
                                            idx,
                                            is_selected,
                                            is_drop_target,
                                            row_height,
                                            zoom,
                                            main_state,
                                            icon_paths,
                                            pane_focus,
                                            pane_w,
                                            view.clone(),
                                        )
                                    })
                                    .collect::<Vec<_>>()
                            },
                        )
                        .with_sizing_behavior(ListSizingBehavior::Auto)
                        .w_full()
                        .h_full()
                        .track_scroll(state.uniform_list_scroll_handle.clone()),
                    )
                    .child(Self::render_lasso_box(state)),
            )
            .into_any_element()
    }

    /// Render the translucent rubber-band lasso selection box inside a list/grid container.
    pub fn render_lasso_box(state: &MainViewState) -> AnyElement {
        if let Some(ref lasso) = state.lasso_state {
            if lasso.has_exceeded_threshold() {
                let rect = lasso.rect();
                return div()
                    .absolute()
                    .left(px(rect.0.x))
                    .top(px(rect.0.y))
                    .w(px((rect.1.x - rect.0.x).max(1.0)))
                    .h(px((rect.1.y - rect.0.y).max(1.0)))
                    .bg(rgba(0x0078D438u32))
                    .border(px(1.0))
                    .border_color(rgba(0x60CDFFFFu32))
                    .rounded(px(2.0))
                    .into_any_element();
            }
        }
        div().into_any_element()
    }

    pub fn details_view_grouped(
        state: &MainViewState,
        visible: &[usize],
        file_items: Entity<FileItems>,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        Self::details_view_grouped_for_pane(
            state,
            visible,
            file_items,
            view,
            PaneFocus::Primary,
            gpui::point(0.0, 158.0),
            1000.0,
        )
    }

    pub fn details_view_grouped_for_pane(
        state: &MainViewState,
        visible: &[usize],
        _file_items: Entity<FileItems>,
        view: Entity<crate::app::AppWindow>,
        pane_focus: PaneFocus,
        list_origin: Point<f32>,
        pane_w: f32,
    ) -> impl IntoElement {
        let flattened = state.flatten_grouped_items(visible);
        let zoom = state.zoom_level;
        let compact = state.compact_mode;
        let base_row_h = if compact { 22.0 } else { 26.0 };
        let row_height = (base_row_h + zoom * 22.0).round();
        let header_h = row_height.max(30.0);
        let hovered_drop_folder = state.drag_state.hovered_target_folder.clone();
        let is_dragging_any = state.drag_state.is_dragging;

        let item_heights: Vec<f32> = flattened
            .iter()
            .map(|entry| match entry {
                GroupedItemType::Header { .. } => header_h,
                GroupedItemType::Item(_) => row_height,
            })
            .collect();

        let scroll_list = Self::render_virtualized_grouped_list(
            match pane_focus {
                PaneFocus::Primary => "details-grouped-scroll-primary",
                PaneFocus::Secondary => "details-grouped-scroll-secondary",
            },
            &state.uniform_list_scroll_handle,
            pane_w,
            &item_heights,
            |i| match &flattened[i] {
                GroupedItemType::Header { key, label, count } => {
                    let is_collapsed = state.collapsed_groups.contains(key);
                    Self::render_group_header_row(
                        i,
                        key,
                        label,
                        *count,
                        is_collapsed,
                        header_h,
                        UiConstants::PADDING_LARGE_RAW as f32,
                        pane_focus,
                        view.clone(),
                    )
                }
                GroupedItemType::Item(idx) => {
                    let idx = *idx;
                    let Some(item) = state.items.get(idx) else {
                        return div().into_any_element();
                    };
                    let is_selected = state.selected_indices.contains(&idx);
                    let is_dir = matches!(item.file_type, FileType::Directory);
                    let is_drop_target = is_dragging_any
                        && is_dir
                        && hovered_drop_folder.as_ref() == Some(&item.path);

                    Self::render_details_row(
                        item,
                        idx,
                        is_selected,
                        is_drop_target,
                        row_height,
                        zoom,
                        state,
                        &state.icon_paths,
                        pane_focus,
                        pane_w,
                        view.clone(),
                    )
                }
            },
        );

        div()
            .flex_col()
            .w_full()
            .flex_grow()
            .h_full()
            .child(Self::column_header_for_pane(state, view.clone(), pane_focus))
            .child(
                div()
                    .id(SharedString::from(match pane_focus {
                        PaneFocus::Primary => "details-grouped-list-container-primary",
                        PaneFocus::Secondary => "details-grouped-list-container-secondary",
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
                    .child(Self::render_lasso_box(state)),
            )
    }

    pub fn column_header(
        state: &MainViewState,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        Self::column_header_for_pane(state, view, PaneFocus::Primary)
    }

    pub fn column_header_for_pane(
        state: &MainViewState,
        view: Entity<crate::app::AppWindow>,
        pane_focus: PaneFocus,
    ) -> impl IntoElement {
        let sort_col = state.sort.column.clone();
        let sort_asc = state.sort.ascending;
        let widths = state.column_widths;
        let col_vis = state.column_visibility;

        let is_col_active = |col: &SortBy| -> bool {
            std::mem::discriminant(col) == std::mem::discriminant(&sort_col)
        };

        let sort_chevron = if sort_asc { "\u{E70E}" } else { "\u{E70D}" };

        let vn = view.clone();
        let vd = view.clone();
        let vt = view.clone();
        let vs = view.clone();
        let v_header = view.clone();

        let resize_handle = |_label: &'static str, col: &str| {
            let v_rh = view.clone();
            let col_name = col.to_string();
            div()
                .w(px(5.0))
                .h(px(16.0))
                .cursor_col_resize()
                .rounded(px(2.0))
                .bg(rgba(UiConstants::DIVIDER_RGBA))
                .hover(|s| s.bg(rgb(UiConstants::ACCENT_COLOR_RGB)))
                .on_mouse_down(MouseButton::Left, move |_event, win, cx| {
                    cx.stop_propagation();
                    let start_x: f32 = win.mouse_position().x.into();
                    v_rh.update(cx, |this, cx| {
                        this.focus_pane(pane_focus, cx);
                        this.start_column_resize(col_name.clone(), start_x, cx);
                    });
                })
        };

        let make_col_button = |label: &'static str,
                               col_variant: SortBy,
                               active: bool,
                               width_opt: Option<f32>,
                               v_btn: Entity<crate::app::AppWindow>| {
            let mut btn = div()
                .h(px(24.0))
                .px(px(6.0))
                .rounded(px(4.0))
                .flex()
                .items_center()
                .gap(px(5.0))
                .cursor_pointer()
                .bg(if active {
                    rgba(0x0078D422u32)
                } else {
                    rgba(0x00000000u32)
                })
                .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
                .on_mouse_down(MouseButton::Left, move |ev, _, cx| {
                    cx.stop_propagation();
                    let c = col_variant.clone();
                    v_btn.update(cx, |this, cx| {
                        this.focus_pane(pane_focus, cx);
                        this.sort_by(c, cx);
                    });
                    let _ = ev;
                })
                .child(
                    div()
                        .font_weight(FontWeight(600.0))
                        .text_size(px(11.5))
                        .text_color(if active {
                            rgb(0x60CDFF)
                        } else {
                            rgb(UiConstants::TEXT_SECONDARY_RGB)
                        })
                        .child(label),
                );

            if let Some(w) = width_opt {
                btn = btn.w(px(w)).flex_shrink_0();
            } else {
                btn = btn.flex_grow().min_w(px(widths.name));
            }

            if active {
                btn = btn.child(
                    div()
                        .text_size(px(9.5))
                        .font_family(UiConstants::FONT_MDL2)
                        .text_color(rgb(0x60CDFF))
                        .child(sort_chevron),
                );
            }
            btn
        };

        let mut header = div()
            .flex()
            .flex_row()
            .h(px(30.0))
            .items_center()
            .px(px(UiConstants::PADDING_LARGE_RAW as f32))
            .bg(rgba(UiConstants::BG_TOOLBAR_RGBA))
            .border_b(px(1.0))
            .border_color(rgba(UiConstants::DIVIDER_RGBA))
            .on_mouse_down(MouseButton::Right, {
                let v = v_header.clone();
                move |ev, _window, cx| {
                    v.update(cx, |this, cx| {
                        this.focus_pane(pane_focus, cx);
                        this.show_column_visibility_menu(ev.position, cx);
                    });
                }
            })
            .child(make_col_button(
                "Name",
                SortBy::Name,
                is_col_active(&SortBy::Name),
                None,
                vn,
            ))
            .child(resize_handle("Name", "name"));

        if col_vis.date {
            header = header
                .child(make_col_button(
                    "Date modified",
                    SortBy::DateModified,
                    is_col_active(&SortBy::DateModified),
                    Some(widths.date),
                    vd,
                ))
                .child(resize_handle("Date", "date"));
        }
        if col_vis.type_col {
            header = header
                .child(make_col_button(
                    "Type",
                    SortBy::Type,
                    is_col_active(&SortBy::Type),
                    Some(widths.type_col),
                    vt,
                ))
                .child(resize_handle("Type", "type"));
        }
        if col_vis.size {
            header = header.child(make_col_button(
                "Size",
                SortBy::Size,
                is_col_active(&SortBy::Size),
                Some(widths.size),
                vs,
            ));
        }

        header
    }
}

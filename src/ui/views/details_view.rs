use crate::config::config::{GroupColumn, SortBy};
use crate::core::navigation_manager::PaneFocus;
use crate::storage::storage::FileType;
use crate::ui::constants::UiConstants;
use crate::ui::views::helpers::format_system_time;
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
            gpui::point(0.0, 160.0),
            1000.0,
        )
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

        let visible_clone = visible.to_vec();

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
                                this.update_lasso_in_pane(pane_focus, pos, pane_w, cx);
                            });
                        }
                    })
                    .on_mouse_up(MouseButton::Left, {
                        let v = view.clone();
                        move |_event, _win, cx| {
                            v.update(cx, |this, cx| {
                                this.end_lasso_in_pane(pane_focus, cx);
                            });
                        }
                    })
                    .on_mouse_up_out(MouseButton::Left, {
                        let v = view.clone();
                        move |_event, _win, cx| {
                            v.update(cx, |this, cx| {
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
                            visible_clone.len(),
                            move |range, _win, cx| {
                                let fi = file_items.read(cx);
                                let app_state = view.read(cx);
                                let main_state = app_state.state.pane_view(pane_focus);
                                let widths = main_state.column_widths;
                                let col_vis = main_state.column_visibility;
                                let zoom = main_state.zoom_level;
                                let show_extensions = main_state.show_extensions;
                                let icon_paths = &app_state.state.main_view.icon_paths;
                                let is_checkbox = main_state.selection_mode == SelectionMode::Checkbox;
                                let is_hover_select =
                                    main_state.selection_mode == SelectionMode::HoverSelect;
                                let renaming_idx = main_state.renaming_index;
                                let rename_text = main_state.rename_text.clone();

                                range
                                    .map(|i| {
                                        if i >= visible_clone.len() {
                                            return div().into_any_element();
                                        }
                                        let idx = visible_clone[i];
                                        if idx >= fi.items.len() {
                                            return div().into_any_element();
                                        }
                                        let item = &fi.items[idx];
                                        let is_selected = fi.selected_indices.contains(&idx);
                                        let is_dir = matches!(item.file_type, FileType::Directory);
                                        let display_name = if show_extensions || is_dir {
                                            item.name.clone()
                                        } else {
                                            std::path::Path::new(&item.name)
                                                .file_stem()
                                                .unwrap_or_default()
                                                .to_string_lossy()
                                                .into_owned()
                                        };
                                        let date_str = format_system_time(item.modified);
                                        let ext_upper = get_extension(&item.name).to_uppercase();
                                        let type_str = if is_dir {
                                            "File folder".to_string()
                                        } else if ext_upper.is_empty() {
                                            "File".to_string()
                                        } else {
                                            format!("{} File", ext_upper)
                                        };

                                        let row_height = 20.0 + zoom * 28.0;
                                        let icon_size = (row_height - 8.0).max(16.0).min(40.0);
                                        let text_size = 11.0 + zoom * 5.0;
                                        let text_size_sm = 10.0 + zoom * 3.0;
                                        let view_row = view.clone();
                                        let item_path = item.path.clone();

                                        let mut row = div().w_full()
                                            .id(ElementId::from(
                                                idx + match pane_focus {
                                                    PaneFocus::Primary => 10000,
                                                    PaneFocus::Secondary => 110000,
                                                },
                                            ))
                                            .flex()
                                            .flex_row()
                                            .h(px(row_height))
                                            .items_center()
                                            .rounded(px(UiConstants::BORDER_RADIUS_SMALL_RAW as f32))
                                            .px(px(UiConstants::PADDING_LARGE_RAW as f32))
                                            .cursor_pointer()
                                            .on_mouse_down(MouseButton::Left, {
                                                let v = view_row.clone();
                                                let path = item_path.clone();
                                                move |ev, _window, cx| {
                                                    cx.stop_propagation();
                                                    v.update(cx, |this, cx| {
                                                        this.focus_pane(pane_focus, cx);
                                                        if ev.click_count == 2 {
                                                            if is_dir {
                                                                this.navigate_pane(
                                                                    path.clone(),
                                                                    pane_focus,
                                                                    cx,
                                                                );
                                                            } else {
                                                                this.open_item_in_pane(
                                                                    pane_focus, idx, cx,
                                                                );
                                                            }
                                                        } else {
                                                            this.select_item_in_pane(
                                                                pane_focus,
                                                                idx,
                                                                ev.modifiers.shift,
                                                                ev.modifiers.control,
                                                                cx,
                                                            );
                                                        }
                                                    });
                                                }
                                            })
                                            .on_mouse_move({
                                                let v = view_row.clone();
                                                move |ev, _win, cx| {
                                                    let pos = point(
                                                        ev.position.x.into(),
                                                        ev.position.y.into(),
                                                    );
                                                    v.update(cx, |this, cx| {
                                                        if this
                                                            .state
                                                            .pane_view(pane_focus)
                                                            .lasso_state
                                                            .is_some()
                                                        {
                                                            this.update_lasso_in_pane(
                                                                pane_focus, pos, pane_w, cx,
                                                            );
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
                                                            this.select_item_in_pane(
                                                                pane_focus, idx, false, false, cx,
                                                            );
                                                        }
                                                        this.show_context_menu(
                                                            Some(idx),
                                                            ev.position,
                                                            window,
                                                            cx,
                                                        );
                                                    });
                                                }
                                            });

                                        if is_selected {
                                            row = row
                                                .bg(rgba(UiConstants::BG_ITEM_SELECTED_RGBA))
                                                .hover(|s| {
                                                    s.bg(rgba(
                                                        UiConstants::BG_ITEM_SELECTED_HOVER_RGBA,
                                                    ))
                                                })
                                                .child(
                                                    div()
                                                        .w(px(3.0))
                                                        .h(px(16.0))
                                                        .mr(px(6.0))
                                                        .rounded(px(1.5))
                                                        .bg(rgb(UiConstants::ACCENT_COLOR_RGB)),
                                                );
                                        } else {
                                            row = row.hover(|s| {
                                                s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA))
                                            });
                                        }

                                        let mut name_row = div()
                                            .flex_grow()
                                            .min_w(px(100.0))
                                            .flex()
                                            .items_center()
                                            .gap(px(8.0));

                                        if is_checkbox {
                                            let v_cb = view_row.clone();
                                            name_row = name_row.child(
                                                div()
                                                    .w(px(18.0))
                                                    .h(px(18.0))
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .on_mouse_down(
                                                        MouseButton::Left,
                                                        move |_, _, cx| {
                                                            cx.stop_propagation();
                                                            v_cb.update(cx, |this, cx| {
                                                                this.focus_pane(pane_focus, cx);
                                                                this.toggle_checkbox_selection(
                                                                    idx, cx,
                                                                );
                                                            });
                                                        },
                                                    )
                                                    .child(
                                                        div()
                                                            .w(px(14.0))
                                                            .h(px(14.0))
                                                            .border(px(1.5))
                                                            .border_color(rgba(
                                                                UiConstants::ACCENT_RGBA,
                                                            ))
                                                            .rounded(px(3.0))
                                                            .bg(if is_selected {
                                                                rgba(UiConstants::ACCENT_RGBA)
                                                            } else {
                                                                rgba(0x00000000)
                                                            })
                                                            .child(if is_selected {
                                                                div()
                                                                    .text_size(px(9.0))
                                                                    .font_family(
                                                                        UiConstants::FONT_MDL2,
                                                                    )
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
                                                    .child(
                                                        if let Some(icon_path) =
                                                            icon_paths.get(&item.path)
                                                        {
                                                            img(icon_path.clone())
                                                                .w(px(icon_size))
                                                                .h(px(icon_size))
                                                                .into_any_element()
                                                        } else {
                                                            div()
                                                                .text_size(px(icon_size * 0.65))
                                                                .font_family(UiConstants::FONT_MDL2)
                                                                .text_color(if is_dir {
                                                                    rgb(0xF5B942)
                                                                } else {
                                                                    rgb(UiConstants::ACCENT_COLOR_RGB)
                                                                })
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
                                                    .w(px(widths.name))
                                                    .flex_grow()
                                                    .min_w(px(60.0))
                                                    .overflow_hidden()
                                                    .text_size(px(text_size))
                                                    .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                                                    .child(if renaming_idx == Some(idx) {
                                                        div()
                                                            .bg(rgba(0xFFFFFF20u32))
                                                            .border(px(1.0))
                                                            .border_color(rgba(
                                                                UiConstants::ACCENT_RGBA,
                                                            ))
                                                            .rounded(px(3.0))
                                                            .px(px(6.0))
                                                            .py(px(1.0))
                                                            .child(rename_text.clone())
                                                            .into_any_element()
                                                    } else {
                                                        div()
                                                            .child(display_name)
                                                            .into_any_element()
                                                    }),
                                            );

                                        let mut row_children: Vec<AnyElement> =
                                            vec![name_row.into_any_element()];

                                        if col_vis.date {
                                            row_children.push(
                                                div().flex_grow().min_w(px(6.0)).into_any_element(),
                                            );
                                            row_children.push(
                                                div()
                                                    .w(px(widths.date))
                                                    .flex_shrink_0()
                                                    .text_size(px(text_size_sm))
                                                    .text_color(rgb(
                                                        UiConstants::TEXT_SECONDARY_RGB,
                                                    ))
                                                    .child(date_str)
                                                    .into_any_element(),
                                            );
                                        }
                                        if col_vis.type_col {
                                            row_children.push(div().w(px(6.0)).into_any_element());
                                            row_children.push(
                                                div()
                                                    .w(px(widths.type_col))
                                                    .flex_shrink_0()
                                                    .overflow_hidden()
                                                    .text_size(px(text_size_sm))
                                                    .text_color(rgb(
                                                        UiConstants::TEXT_SECONDARY_RGB,
                                                    ))
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
                                                    .text_size(px(text_size_sm))
                                                    .text_color(rgb(
                                                        UiConstants::TEXT_SECONDARY_RGB,
                                                    ))
                                                    .child(if is_dir {
                                                        String::new()
                                                    } else {
                                                        format_size(item.size)
                                                    })
                                                    .into_any_element(),
                                            );
                                        }

                                        row.children(row_children).into_any_element()
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
            gpui::point(0.0, 160.0),
            1000.0,
        )
    }

    pub fn details_view_grouped_for_pane(
        state: &MainViewState,
        visible: &[usize],
        file_items: Entity<FileItems>,
        view: Entity<crate::app::AppWindow>,
        pane_focus: PaneFocus,
        list_origin: Point<f32>,
        pane_w: f32,
    ) -> impl IntoElement {
        let flattened = state.flatten_grouped_items(visible);
        let item_count = flattened.len();

        div()
            .flex_col()
            .w_full()
            .flex_grow()
            .h_full()
            .child(Self::column_header_for_pane(state, view.clone(), pane_focus))
            .child(
                div()
                    .relative()
                    .flex_col()
                    .w_full()
                    .flex_grow()
                    .h_full()
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
                                this.update_lasso_in_pane(pane_focus, pos, pane_w, cx);
                            });
                        }
                    })
                    .on_mouse_up(MouseButton::Left, {
                        let v = view.clone();
                        move |_event, _win, cx| {
                            v.update(cx, |this, cx| {
                                this.end_lasso_in_pane(pane_focus, cx);
                            });
                        }
                    })
                    .child(
                        uniform_list(
                            match pane_focus {
                                PaneFocus::Primary => "details-grouped-scroll-primary",
                                PaneFocus::Secondary => "details-grouped-scroll-secondary",
                            },
                            item_count,
                            move |range, _win, cx| {
                                let fi = file_items.read(cx);
                                let app_state = view.read(cx);
                                let main_state = app_state.state.pane_view(pane_focus);
                                let widths = main_state.column_widths;
                                let zoom = main_state.zoom_level;
                                let row_height = 20.0 + zoom * 28.0;
                                let icon_paths = &app_state.state.main_view.icon_paths;

                                range
                                    .map(|i| match &flattened[i] {
                                        GroupedItemType::Header { key, label, count } => {
                                            let key_clone = key.clone();
                                            let label_clone = label.clone();
                                            let count_clone = *count;
                                            let is_collapsed =
                                                main_state.collapsed_groups.contains(key);
                                            let v_g = view.clone();

                                            div()
                                                .id(ElementId::from(50000 + i))
                                                .flex()
                                                .flex_row()
                                                .h(px(row_height))
                                                .items_center()
                                                .px(px(UiConstants::PADDING_LARGE_RAW as f32))
                                                .bg(rgba(0x0000000Au32))
                                                .cursor_pointer()
                                                .on_mouse_down(
                                                    MouseButton::Left,
                                                    move |_, _, cx| {
                                                        cx.stop_propagation();
                                                        v_g.update(cx, |this, cx| {
                                                            this.focus_pane(pane_focus, cx);
                                                            this.toggle_group_collapse(
                                                                key_clone.clone(),
                                                                cx,
                                                            );
                                                        });
                                                    },
                                                )
                                                .child(
                                                    div()
                                                        .text_size(px(12.0))
                                                        .font_family(UiConstants::FONT_MDL2)
                                                        .text_color(rgba(
                                                            UiConstants::TEXT_SECONDARY_RGBA,
                                                        ))
                                                        .child(if is_collapsed {
                                                            "\u{E76C}"
                                                        } else {
                                                            "\u{E76B}"
                                                        }),
                                                )
                                                .child(
                                                    div()
                                                        .ml(px(6.0))
                                                        .text_size(px(13.0))
                                                        .font_weight(FontWeight(600.0))
                                                        .text_color(rgba(
                                                            UiConstants::TEXT_PRIMARY_RGBA,
                                                        ))
                                                        .child(label_clone),
                                                )
                                                .child(
                                                    div()
                                                        .ml(px(8.0))
                                                        .text_size(px(11.0))
                                                        .text_color(rgba(
                                                            UiConstants::TEXT_SECONDARY_RGBA,
                                                        ))
                                                        .child(format!(
                                                            "{} item{}",
                                                            count_clone,
                                                            if count_clone == 1 {
                                                                ""
                                                            } else {
                                                                "s"
                                                            }
                                                        )),
                                                )
                                                .into_any_element()
                                        }
                                        GroupedItemType::Item(idx) => {
                                            let idx = *idx;
                                            if idx >= fi.items.len() {
                                                return div().into_any_element();
                                            }
                                            let item = &fi.items[idx];
                                            let is_selected = fi.selected_indices.contains(&idx);
                                            let display_name =
                                                main_state.display_name(item).into_owned();
                                            let item_type = item.file_type;
                                            let is_dir = matches!(item_type, FileType::Directory);
                                            let path = item.path.clone();
                                            let date_str = format_system_time(item.modified);
                                            let size_str = if is_dir {
                                                String::new()
                                            } else {
                                                format_size(item.size)
                                            };
                                            let icon_size =
                                                (row_height - 8.0).max(16.0).min(40.0);
                                            let text_size = 11.0 + zoom * 5.0;
                                            let text_size_sm = 10.0 + zoom * 3.0;
                                            let view_row = view.clone();

                                            let mut row = div().w_full()
                                                .id(ElementId::from(idx + 10000))
                                                .flex()
                                                .flex_row()
                                                .h(px(row_height))
                                                .items_center()
                                                .rounded(px(
                                                    UiConstants::BORDER_RADIUS_SMALL_RAW as f32,
                                                ))
                                                .px(px(UiConstants::PADDING_LARGE_RAW as f32))
                                                .cursor_pointer()
                                                .on_mouse_down(MouseButton::Left, {
                                                    let view = view_row.clone();
                                                    let path = path.clone();
                                                    move |ev, _window, cx| {
                                                        cx.stop_propagation();
                                                        view.update(cx, |this, cx| {
                                                            this.focus_pane(pane_focus, cx);
                                                            if ev.click_count == 2 {
                                                                if is_dir {
                                                                    this.navigate_pane(
                                                                        path.clone(),
                                                                        pane_focus,
                                                                        cx,
                                                                    );
                                                                } else {
                                                                    this.open_item_in_pane(
                                                                        pane_focus, idx, cx,
                                                                    );
                                                                }
                                                            } else {
                                                                this.select_item_in_pane(
                                                                    pane_focus,
                                                                    idx,
                                                                    ev.modifiers.shift,
                                                                    ev.modifiers.control,
                                                                    cx,
                                                                );
                                                            }
                                                        });
                                                    }
                                                })
                                                .on_mouse_down(MouseButton::Right, {
                                                    let view = view_row.clone();
                                                    move |ev, window, cx| {
                                                        cx.stop_propagation();
                                                        view.update(cx, |this, cx| {
                                                            this.focus_pane(pane_focus, cx);
                                                            if !this
                                                                .state
                                                                .pane_view(pane_focus)
                                                                .selected_indices
                                                                .contains(&idx)
                                                            {
                                                                this.select_item_in_pane(
                                                                    pane_focus, idx, false, false,
                                                                    cx,
                                                                );
                                                            }
                                                            this.show_context_menu(
                                                                Some(idx),
                                                                ev.position,
                                                                window,
                                                                cx,
                                                            );
                                                        });
                                                    }
                                                });

                                            if is_selected {
                                                row = row
                                                    .bg(rgba(UiConstants::BG_ITEM_SELECTED_RGBA))
                                                    .child(
                                                        div()
                                                            .w(px(3.0))
                                                            .h(px(16.0))
                                                            .rounded(px(1.5))
                                                            .bg(rgb(
                                                                UiConstants::ACCENT_COLOR_RGB,
                                                            )),
                                                    );
                                            } else {
                                                row = row.hover(|s| {
                                                    s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA))
                                                });
                                            }

                                            row.children([
                                                div()
                                                    .flex_grow()
                                                    .flex()
                                                    .items_center()
                                                    .gap(px(8.0))
                                                    .pl(px(20.0))
                                                    .children([
                                                        div()
                                                            .w(px(icon_size))
                                                            .h(px(icon_size))
                                                            .flex()
                                                            .items_center()
                                                            .justify_center()
                                                            .child(
                                                                if let Some(icon_path) =
                                                                    icon_paths.get(&item.path)
                                                                {
                                                                    img(icon_path.clone())
                                                                        .w(px(icon_size))
                                                                        .h(px(icon_size))
                                                                        .into_any_element()
                                                                } else {
                                                                    div()
                                                                        .text_size(px(
                                                                            icon_size * 0.65,
                                                                        ))
                                                                        .font_family(
                                                                            UiConstants::FONT_MDL2,
                                                                        )
                                                                        .text_color(if is_dir {
                                                                            rgb(0xF5B942)
                                                                        } else {
                                                                            rgb(
                                                                                UiConstants::ACCENT_COLOR_RGB,
                                                                            )
                                                                        })
                                                                        .child(if is_dir {
                                                                            "\u{E8B7}"
                                                                        } else {
                                                                            "\u{E8A5}"
                                                                        })
                                                                        .into_any_element()
                                                                },
                                                            ),
                                                        div()
                                                            .w(px(widths.name))
                                                            .overflow_hidden()
                                                            .text_size(px(text_size))
                                                            .text_color(rgb(
                                                                UiConstants::TEXT_PRIMARY_RGB,
                                                            ))
                                                            .child(
                                                                if main_state.renaming_index
                                                                    == Some(idx)
                                                                {
                                                                    div()
                                                                        .bg(rgba(0xFFFFFF20u32))
                                                                        .border(px(1.0))
                                                                        .border_color(rgba(
                                                                            UiConstants::ACCENT_RGBA,
                                                                        ))
                                                                        .rounded(px(2.0))
                                                                        .px(px(4.0))
                                                                        .child(
                                                                            main_state
                                                                                .rename_text
                                                                                .clone(),
                                                                        )
                                                                        .into_any_element()
                                                                } else {
                                                                    div()
                                                                        .child(display_name)
                                                                        .into_any_element()
                                                                },
                                                            ),
                                                    ]),
                                                div().w(px(6.0)),
                                                div()
                                                    .w(px(widths.date))
                                                    .text_size(px(text_size_sm))
                                                    .text_color(rgb(
                                                        UiConstants::TEXT_SECONDARY_RGB,
                                                    ))
                                                    .child(date_str),
                                                div().w(px(6.0)),
                                                div()
                                                    .w(px(widths.type_col))
                                                    .text_size(px(text_size_sm))
                                                    .text_color(rgb(
                                                        UiConstants::TEXT_SECONDARY_RGB,
                                                    ))
                                                    .child(if is_dir {
                                                        "File folder"
                                                    } else {
                                                        "File"
                                                    }),
                                                div().w(px(6.0)),
                                                div()
                                                    .w(px(widths.size))
                                                    .text_size(px(text_size_sm))
                                                    .text_color(rgb(
                                                        UiConstants::TEXT_SECONDARY_RGB,
                                                    ))
                                                    .child(size_str),
                                            ])
                                            .into_any_element()
                                        }
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

        let sort_indicator = |col: &SortBy| -> &'static str {
            if std::mem::discriminant(col) == std::mem::discriminant(&sort_col) {
                if sort_asc { " ▲" } else { " ▼" }
            } else {
                ""
            }
        };

        let name_label = format!("Name{}", sort_indicator(&SortBy::Name));
        let date_label = format!("Date modified{}", sort_indicator(&SortBy::DateModified));
        let type_label = format!("Type{}", sort_indicator(&SortBy::Type));
        let size_label = format!("Size{}", sort_indicator(&SortBy::Size));

        let vn = view.clone();
        let vd = view.clone();
        let vt = view.clone();
        let vs = view.clone();
        let v_header = view.clone();

        let resize_handle = |_label: &'static str, col: &str| {
            let v_rh = view.clone();
            let col_name = col.to_string();
            div()
                .w(px(6.0))
                .h_full()
                .cursor_col_resize()
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

        let mut header = div()
            .flex()
            .flex_row()
            .h(px(28.0))
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
            .child(
                div()
                    .flex_grow()
                    .min_w(px(widths.name))
                    .cursor_pointer()
                    .font_weight(FontWeight(600.0))
                    .text_size(AbsoluteLength::Pixels(px(12.0)))
                    .text_color(rgb(UiConstants::TEXT_SECONDARY_RGB))
                    .hover(|s| s.text_color(rgb(UiConstants::TEXT_PRIMARY_RGB)))
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        vn.update(cx, |this, cx| {
                            this.focus_pane(pane_focus, cx);
                            this.sort_by(SortBy::Name, cx);
                        });
                    })
                    .child(name_label),
            )
            .child(resize_handle("Name", "name"));

        if col_vis.date {
            header = header
                .child(
                    div()
                        .w(px(widths.date))
                        .flex_shrink_0()
                        .cursor_pointer()
                        .font_weight(FontWeight(600.0))
                        .text_size(AbsoluteLength::Pixels(px(12.0)))
                        .text_color(rgb(UiConstants::TEXT_SECONDARY_RGB))
                        .hover(|s| s.text_color(rgb(UiConstants::TEXT_PRIMARY_RGB)))
                        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                            vd.update(cx, |this, cx| {
                                this.focus_pane(pane_focus, cx);
                                this.sort_by(SortBy::DateModified, cx);
                            });
                        })
                        .child(date_label),
                )
                .child(resize_handle("Date", "date"));
        }
        if col_vis.type_col {
            header = header
                .child(
                    div()
                        .w(px(widths.type_col))
                        .flex_shrink_0()
                        .cursor_pointer()
                        .font_weight(FontWeight(600.0))
                        .text_size(AbsoluteLength::Pixels(px(12.0)))
                        .text_color(rgb(UiConstants::TEXT_SECONDARY_RGB))
                        .hover(|s| s.text_color(rgb(UiConstants::TEXT_PRIMARY_RGB)))
                        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                            vt.update(cx, |this, cx| {
                                this.focus_pane(pane_focus, cx);
                                this.sort_by(SortBy::Type, cx);
                            });
                        })
                        .child(type_label),
                )
                .child(resize_handle("Type", "type"));
        }
        if col_vis.size {
            header = header.child(
                div()
                    .w(px(widths.size))
                    .flex_shrink_0()
                    .cursor_pointer()
                    .font_weight(FontWeight(600.0))
                    .text_size(AbsoluteLength::Pixels(px(12.0)))
                    .text_color(rgb(UiConstants::TEXT_SECONDARY_RGB))
                    .hover(|s| s.text_color(rgb(UiConstants::TEXT_PRIMARY_RGB)))
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        vs.update(cx, |this, cx| {
                            this.focus_pane(pane_focus, cx);
                            this.sort_by(SortBy::Size, cx);
                        });
                    })
                    .child(size_label),
            );
        }

        header
    }
}

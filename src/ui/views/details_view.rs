use crate::config::config::{GroupColumn, SortBy};
use crate::storage::storage::FileType;
use crate::ui::constants::UiConstants;
use crate::ui::views::main_view::{
    format_size, rects_intersect, ColumnWidths, FileItems, GroupedItemType, LassoState, MainView,
    MainViewState, SelectionMode,
};
use gpui::*;

impl MainView {
    pub fn details_view(
        state: &MainViewState,
        visible: &[usize],
        file_items: Entity<FileItems>,
        view: Entity<crate::app::AppWindow>,
    ) -> AnyElement {
        // When grouping is active, render with group headers instead of flat list
        if state.group_by != GroupColumn::None {
            return Self::details_view_grouped(state, visible, file_items, view).into_any_element();
        }

        let visible_clone = visible.to_vec();
        let row_height = 20.0 + state.zoom_level * 28.0;

        div()
            .flex_col()
            .w_full()
            .flex_grow()
            .h_full()
            .child(Self::column_header(state, view.clone()))
            .child(
                div()
                    .id("details-list-container")
                    .flex_col()
                    .w_full()
                    .flex_grow()
                    .h_full()
                    .relative()
                    .on_mouse_down(MouseButton::Right, {
                        let v = view.clone();
                        move |event, _, cx| {
                            v.update(cx, |this, cx| {
                                this.show_context_menu(None, event.position, cx);
                            });
                        }
                    })
                    .on_mouse_down(MouseButton::Left, {
                        let v = view.clone();
                        move |event, _win, cx| {
                            v.update(cx, |this, cx| {
                                this.state.main_view.lasso_state = Some(LassoState {
                                    start: point(event.position.x.into(), event.position.y.into()),
                                    current: point(
                                        event.position.x.into(),
                                        event.position.y.into(),
                                    ),
                                });
                                cx.notify();
                            });
                        }
                    })
                    .on_mouse_move({
                        let v = view.clone();
                        move |event, _win, cx| {
                            v.update(cx, |this, cx| {
                                if let Some(ref mut lasso) = this.state.main_view.lasso_state {
                                    lasso.current =
                                        point(event.position.x.into(), event.position.y.into());
                                    cx.notify();
                                }
                            });
                        }
                    })
                    .on_mouse_up(MouseButton::Left, {
                        let v = view.clone();
                        let visible_for_lasso = visible_clone.clone();
                        move |_event, _win, cx| {
                            v.update(cx, |this, cx| {
                                if let Some(ref lasso) = this.state.main_view.lasso_state {
                                    if lasso.has_exceeded_threshold() {
                                        let rect = lasso.rect();
                                        let row_height =
                                            20.0 + this.state.main_view.zoom_level * 28.0;
                                        this.state.main_view.selected_indices.clear();
                                        for (i, &vi) in visible_for_lasso.iter().enumerate() {
                                            let item_y = i as f32 * row_height;
                                            let item_rect = (
                                                point(0.0, item_y),
                                                point(2000.0, item_y + row_height),
                                            );
                                            if rects_intersect(
                                                item_rect,
                                                (lasso.rect().0, lasso.rect().1),
                                            ) {
                                                this.state.main_view.selected_indices.insert(vi);
                                            }
                                        }
                                        this.state.file_items.update(cx, |m, _| {
                                            m.selected_indices =
                                                this.state.main_view.selected_indices.clone();
                                        });
                                    }
                                }
                                this.state.main_view.lasso_state = None;
                                cx.notify();
                            });
                        }
                    })
                    .child(
                        uniform_list(
                            "details-scroll",
                            visible_clone.len(),
                            move |range, _win, cx| {
                                let fi = file_items.read(cx);
                                let app_state = view.read(cx);
                                let main_state = &app_state.state.main_view;
                                let widths = main_state.column_widths;
                                let zoom = main_state.zoom_level;
                                let show_extensions = main_state.show_extensions;
                                let icon_paths = &main_state.icon_paths;

                                range
                                    .map(|i| {
                                        let idx = visible_clone[i];
                                        let item = &fi.items[idx];
                                        let is_selected = fi.selected_indices.contains(&idx);
                                        let display_name = if show_extensions {
                                            item.name.clone()
                                        } else {
                                            std::path::Path::new(&item.name)
                                                .file_stem()
                                                .unwrap_or_default()
                                                .to_string_lossy()
                                                .into_owned()
                                        };

                                        let row_height = 20.0 + zoom * 28.0;
                                        let icon_size = (row_height - 8.0).max(16.0).min(40.0);
                                        let text_size = 11.0 + zoom * 5.0;
                                        let text_size_sm = 10.0 + zoom * 3.0;
                                        let view_row = view.clone();

                                        let mut row = div()
                                            .id(ElementId::from(idx + 10000))
                                            .flex()
                                            .flex_row()
                                            .h(px(row_height))
                                            .items_center()
                                            .px(px(UiConstants::PADDING_LARGE_RAW as f32))
                                            .cursor_pointer()
                                            .on_mouse_down(MouseButton::Left, {
                                                let v = view_row.clone();
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
                                                let v = view_row.clone();
                                                let item_type = item.file_type;
                                                let path = item.path.clone();
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
                                                let v = view_row.clone();
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
                                            });

                                        if is_selected {
                                            row = row
                                                .bg(rgba(UiConstants::BG_ITEM_SELECTED_RGBA))
                                                .border_l(px(
                                                    UiConstants::SELECTED_BORDER_WIDTH as f32
                                                ))
                                                .border_color(rgba(UiConstants::ACCENT_RGBA));
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
                                                                .text_size(px(icon_size * 0.5))
                                                                .text_color(rgb(
                                                                    UiConstants::ACCENT_COLOR_RGB,
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
                                                div()
                                                    .w(px(widths.name))
                                                    .overflow_hidden()
                                                    .text_size(px(text_size))
                                                    .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                                                    .child(display_name),
                                            ]),
                                            div()
                                                .w(px(widths.date))
                                                .text_center()
                                                .text_size(px(text_size_sm))
                                                .text_color(rgb(UiConstants::TEXT_SECONDARY_RGB))
                                                .child("\u{2014}"),
                                            div()
                                                .w(px(widths.type_col))
                                                .text_center()
                                                .text_size(px(text_size_sm))
                                                .text_color(rgb(UiConstants::TEXT_SECONDARY_RGB))
                                                .child(
                                                    if matches!(item.file_type, FileType::Directory)
                                                    {
                                                        "Folder"
                                                    } else {
                                                        "File"
                                                    },
                                                ),
                                            div()
                                                .w(px(widths.size))
                                                .text_center()
                                                .text_size(px(text_size_sm))
                                                .text_color(rgb(UiConstants::TEXT_SECONDARY_RGB))
                                                .child(
                                                    if matches!(item.file_type, FileType::Directory)
                                                    {
                                                        String::new()
                                                    } else {
                                                        format_size(item.size)
                                                    },
                                                ),
                                        ])
                                        .into_any_element()
                                    })
                                    .collect::<Vec<_>>()
                            },
                        )
                        .h_full()
                        .track_scroll(state.uniform_list_scroll_handle.clone()),
                    )
                    .child(if let Some(ref lasso) = state.lasso_state {
                        let rect = lasso.rect();
                        div()
                            .absolute()
                            .left(px(rect.0.x))
                            .top(px(rect.0.y))
                            .w(px(rect.1.x - rect.0.x))
                            .h(px(rect.1.y - rect.0.y))
                            .bg(rgba(0x0078d433u32))
                            .border(px(1.0))
                            .border_color(rgba(0x0078d4ffu32))
                    } else {
                        div()
                    }),
            )
            .into_any_element()
    }

    pub fn details_view_grouped(
        state: &MainViewState,
        visible: &[usize],
        file_items: Entity<FileItems>,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let flattened = state.flatten_grouped_items(visible);
        let widths = state.column_widths;

        div()
            .flex_col()
            .w_full()
            .flex_grow()
            .h_full()
            .child(Self::column_header(state, view.clone()))
            .child(
                div()
                    .id("details-grouped-scroll")
                    .flex_col()
                    .w_full()
                    .flex_grow()
                    .h_full()
                    .overflow_y_scroll()
                    .track_scroll(&state.scroll_handle)
                    .children(
                        (0..flattened.len()).map(|i| {
                            match &flattened[i] {
                                GroupedItemType::Header { key, label, count } => {
                                    let key_clone = key.clone();
                                    let label_clone = label.clone();
                                    let count_clone = *count;
                                    let is_collapsed = state.collapsed_groups.contains(key);
                                    let group_height = if state.compact_mode { 24.0 } else { 32.0 };
                                    let v_g = view.clone();

                                    div()
                                        .id(ElementId::from(50000 + i))
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
                                                .child(if is_collapsed { "\\u{E76C}" } else { "\\u{E76B}" }),
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
                                                .child(format!("{} item{}", count_clone, if count_clone == 1 { "" } else { "s" })),
                                        )
                                        .into_any_element()
                                }
                                GroupedItemType::Item(idx) => {
                                    let idx = *idx;
                                    let visual_index = i; // Position in flattened list (for drag)
                                    let item = &state.items[idx];
                                    let is_selected = state.selected_indices.contains(&idx);
                                    let display_name = state.display_name(item).into_owned();
                                    let item_type = item.file_type;
                                    let path = item.path.clone();
                                    let size_str = if matches!(item.file_type, FileType::Directory) {
                                        String::new()
                                    } else {
                                        format_size(item.size)
                                    };
                                    let zoom = state.zoom_level;
                                    let row_height = 20.0 + zoom * 28.0;
                                    let icon_size = (row_height - 8.0).max(16.0).min(40.0);
                                    let text_size = 11.0 + zoom * 5.0;
                                    let text_size_sm = 10.0 + zoom * 3.0;
                                    let view_row = view.clone();
                                    let is_dragging = state.group_drag_state.is_dragging;
                                    let drag_target = state.group_drag_state.insert_position;
                                    let dragged_idx = state.group_drag_state.dragged_index;

                                    // Determine if this row is the drop target
                                    let is_drop_target = is_dragging
                                        && drag_target == Some(visual_index)
                                        && dragged_idx != Some(idx);

                                    // Determine if this row is being dragged
                                    let is_being_dragged = dragged_idx == Some(idx);

                                    let mut row = div()
                                        .id(ElementId::from(idx + 10000))
                                        .flex()
                                        .flex_row()
                                        .h(px(row_height))
                                        .items_center()
                                        .px(px(UiConstants::PADDING_LARGE_RAW as f32))
                                        .cursor_pointer()
                                        // Opacity reduce if being dragged
                                        .opacity(if is_being_dragged { 0.5 } else { 1.0 })
                                        // Show drop indicator at top if this is the target
                                        .border_t(if is_drop_target { px(2.0) } else { px(0.0) })
                                        .border_color(if is_drop_target { rgba(UiConstants::ACCENT_RGBA) } else { rgba(0x00000000) })
                                        .on_mouse_down(MouseButton::Left, {
                                            let path = path.clone();
                                            let view = view_row.clone();
                                            move |event, _, cx| {
                                                cx.stop_propagation();
                                                // Start drag tracking on mouse down
                                                let y: f32 = event.position.y / Pixels::from(1.0);
                                                view.update(cx, |this, cx| {
                                                    this.group_drag_start(idx, y);
                                                });
                                            }
                                        })
                                        .on_mouse_move({
                                            let view = view_row.clone();
                                            move |event, _, cx| {
                                                let y: f32 = event.position.y / Pixels::from(1.0);
                                                view.update(cx, |this, _cx| {
                                                    this.group_drag_update(y);
                                                });
                                            }
                                        })
                                        .on_mouse_up(MouseButton::Left, {
                                            let path = path.clone();
                                            let view = view_row.clone();
                                            move |_, _, cx| {
                                                cx.stop_propagation();
                                                view.update(cx, |this, cx| {
                                                    if this.is_group_dragging() {
                                                        // Complete the drag operation
                                                        this.group_drag_end(cx);
                                                    } else {
                                                        // Normal click action
                                                        if item_type == FileType::Directory {
                                                            this.navigate(path.clone(), cx);
                                                        } else {
                                                            this.select_item(idx, false, false, cx);
                                                        }
                                                    }
                                                });
                                            }
                                        })
                                        .on_mouse_down(MouseButton::Right, {
                                            let view = view_row.clone();
                                            move |ev, _, cx| {
                                                cx.stop_propagation();
                                                view.update(cx, |this, cx| {
                                                    this.show_context_menu(Some(idx), ev.position, cx);
                                                });
                                            }
                                        });

                                    if is_selected {
                                        row = row
                                            .bg(rgba(UiConstants::BG_ITEM_SELECTED_RGBA))
                                            .border_l(px(UiConstants::SELECTED_BORDER_WIDTH as f32))
                                            .border_color(rgba(UiConstants::ACCENT_RGBA));
                                    } else {
                                        row = row.hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)));
                                    }

                                    row.children([
                                        div()
                                            .flex_grow()
                                            .flex()
                                            .items_center()
                                            .gap(px(8.0))
                                            .pl(px(20.0))
                                            .children({
                                                let mut cells = Vec::new();

                                                // Checkbox column
                                                if state.selection_mode == SelectionMode::Checkbox {
                                                    cells.push(
                                                        div()
                                                            .w(px(24.0))
                                                            .h(px(24.0))
                                                            .flex()
                                                            .items_center()
                                                            .justify_center()
                                                            .child(
                                                                div()
                                                                    .w(px(16.0))
                                                                .h(px(16.0))
                                                                .border(px(1.0))
                                                                .border_color(rgba(UiConstants::ACCENT_RGBA))
                                                                .rounded(px(2.0))
                                                                .bg(if is_selected { rgba(UiConstants::ACCENT_RGBA) } else { rgba(0x00000000) })
                                                                .child(if is_selected {
                                                                    div()
                                                                        .text_size(px(10.0))
                                                                        .font_family(UiConstants::FONT_MDL2)
                                                                        .text_color(rgb(0xFFFFFF))
                                                                        .child("\u{E73E}")
                                                                } else {
                                                                    div()
                                                                })
                                                        )
                                                        .into_any_element()
                                                );
                                            }

                                            // Icon
                                            cells.push(
                                                div()
                                                    .w(px(icon_size))
                                                    .h(px(icon_size))
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .child(
                                                        div()
                                                            .text_size(px(icon_size * 0.5))
                                                            .text_color(rgb(UiConstants::ACCENT_COLOR_RGB))
                                                            .child(if matches!(item_type, FileType::Directory) {
                                                                "\u{E8B7}"
                                                            } else {
                                                                "\u{E8A5}"
                                                            }),
                                                    ).into_any_element()
                                            );

                                            // Git status icon
                                            let git_status = crate::domain::git_service::GitService::get_file_status(&path);
                                            if git_status != crate::domain::git_service::GitFileStatus::Clean && git_status != crate::domain::git_service::GitFileStatus::Unknown {
                                                cells.push(
                                                    div()
                                                        .w(px(18.0))
                                                        .flex()
                                                        .items_center()
                                                        .justify_center()
                                                        .child(
                                                            div()
                                                                .text_size(AbsoluteLength::Pixels(px(12.0)))
                                                                .text_color(rgb(0xFFAA44))
                                                                .child(git_status.label()),
                                                        ).into_any_element()
                                                );
                                            }

                                            // Name
                                            cells.push(
                                                div()
                                                    .w(px(widths.name))
                                                    .overflow_hidden()
                                                    .text_size(px(text_size))
                                                    .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                                                    .child(if state.renaming_index == Some(idx) {
                                                        div()
                                                            .bg(rgba(0xFFFFFF20u32))
                                                            .border(px(1.0))
                                                            .border_color(rgba(UiConstants::ACCENT_RGBA))
                                                            .rounded(px(2.0))
                                                            .px(px(4.0))
                                                            .child(state.rename_text.clone())
                                                            .into_any_element()
                                                    } else {
                                                        div().child(display_name).into_any_element()
                                                    }).into_any_element()
                                            );
                                            cells
                                        }),
                                    div()
                                        .w(px(widths.date))
                                        .text_center()
                                        .text_size(px(text_size_sm))
                                        .text_color(rgb(UiConstants::TEXT_SECONDARY_RGB))
                                        .child("\u{2014}"),
                                    div()
                                        .w(px(widths.type_col))
                                        .text_center()
                                        .text_size(px(text_size_sm))
                                        .text_color(rgb(UiConstants::TEXT_SECONDARY_RGB))
                                        .child(if matches!(item_type, FileType::Directory) {
                                            "Folder"
                                        } else {
                                            "File"
                                        }),
                                    div()
                                        .w(px(widths.size))
                                        .text_center()
                                        .text_size(px(text_size_sm))
                                        .text_color(rgb(UiConstants::TEXT_SECONDARY_RGB))
                                        .child(size_str),
                                ])
                                .into_any_element()
                            }
                        }
                    })
            ))
    }
}

impl MainView {
    pub fn column_header(
        state: &MainViewState,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let sort_col = state.sort.column.clone();
        let sort_asc = state.sort.ascending;
        let widths = state.column_widths;

        let sort_indicator = |col: &SortBy| -> &'static str {
            if std::mem::discriminant(col) == std::mem::discriminant(&sort_col) {
                if sort_asc {
                    " ▲"
                } else {
                    " ▼"
                }
            } else {
                ""
            }
        };

        let name_label = format!("Name{}", sort_indicator(&SortBy::Name));
        let date_label = format!("Date Modified{}", sort_indicator(&SortBy::DateModified));
        let type_label = format!("Type{}", sort_indicator(&SortBy::Type));
        let size_label = format!("Size{}", sort_indicator(&SortBy::Size));

        let vn = view.clone();
        let vd = view.clone();
        let vt = view.clone();
        let vs = view.clone();
        let v_header = view.clone();

        let resize_handle = |label: &'static str, col: &str| {
            let v_rh = view.clone();
            let col_name = col.to_string();
            div()
                .w(px(8.0))
                .h_full()
                .cursor_col_resize()
                .bg(rgba(UiConstants::DIVIDER_RGBA))
                .hover(|s| s.bg(rgb(UiConstants::ACCENT_COLOR_RGB)))
                .on_mouse_down(MouseButton::Left, move |event, win, cx| {
                    let start_x: f32 = win.mouse_position().x.into();
                    v_rh.update(cx, |this, cx| {
                        this.start_column_resize(col_name.clone(), start_x, cx);
                    });
                })
        };

        div()
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
                move |ev, _, cx| {
                    v.update(cx, |this, cx| {
                        this.show_column_visibility_menu(ev.position, cx);
                    });
                }
            })
            // Name
            .child(
                div()
                    .flex_grow()
                    .cursor_pointer()
                    .font_weight(FontWeight(600.0))
                    .text_size(AbsoluteLength::Pixels(px(12.0)))
                    .text_color(rgb(UiConstants::TEXT_SECONDARY_RGB))
                    .hover(|s| s.text_color(rgb(UiConstants::TEXT_PRIMARY_RGB)))
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        vn.update(cx, |this, cx| this.sort_by(SortBy::Name, cx));
                    })
                    .child(name_label),
            )
            .child(resize_handle("Name", "name"))
            // Date Modified
            .child(
                div()
                    .w(px(widths.date))
                    .cursor_pointer()
                    .font_weight(FontWeight(600.0))
                    .text_size(AbsoluteLength::Pixels(px(12.0)))
                    .text_color(rgb(UiConstants::TEXT_SECONDARY_RGB))
                    .hover(|s| s.text_color(rgb(UiConstants::TEXT_PRIMARY_RGB)))
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        vd.update(cx, |this, cx| this.sort_by(SortBy::DateModified, cx));
                    })
                    .child(date_label),
            )
            .child(resize_handle("Date", "date"))
            // Type
            .child(
                div()
                    .w(px(widths.type_col))
                    .cursor_pointer()
                    .font_weight(FontWeight(600.0))
                    .text_size(AbsoluteLength::Pixels(px(12.0)))
                    .text_color(rgb(UiConstants::TEXT_SECONDARY_RGB))
                    .hover(|s| s.text_color(rgb(UiConstants::TEXT_PRIMARY_RGB)))
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        vt.update(cx, |this, cx| this.sort_by(SortBy::Type, cx));
                    })
                    .child(type_label),
            )
            .child(resize_handle("Type", "type"))
            // Size
            .child(
                div()
                    .w(px(widths.size))
                    .cursor_pointer()
                    .font_weight(FontWeight(600.0))
                    .text_size(AbsoluteLength::Pixels(px(12.0)))
                    .text_color(rgb(UiConstants::TEXT_SECONDARY_RGB))
                    .hover(|s| s.text_color(rgb(UiConstants::TEXT_PRIMARY_RGB)))
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        vs.update(cx, |this, cx| this.sort_by(SortBy::Size, cx));
                    })
                    .child(size_label),
            )
    }
}

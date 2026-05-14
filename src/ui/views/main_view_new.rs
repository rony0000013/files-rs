fn details_view(
        state: &MainViewState,
        visible: &[usize],
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        // When grouping is active, render with group headers instead of flat list
        if state.group_by != GroupColumn::None {
            return Self::details_view_grouped(state, visible, view);
        }

        div()
            .flex_col()
            .w_full()
            .flex_grow()
            .child(Self::column_header(state, view.clone()))
            .child(
                div()
                    .flex_col()
                    .flex_grow()
                    .w_full()
                    .child({
                        let visible = visible.to_vec();
                        let view_list = view.clone();
                        uniform_list("details-scroll", visible.len(), move |range, _win, cx| {
                            let app_state = view_list.read(cx);
                            let state = &app_state.state.main_view;
                            range.map(|i| {
                                let idx = visible[i];
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
                                let row_height = if state.compact_mode { 28.0 } else { 36.0 };
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
                                        let path = path.clone();
                                        let view = view_row.clone();
                                        move |_ev, _win, cx| {
                                            view.update(cx, |this, cx| {
                                                if item_type == FileType::Directory {
                                                    this.navigate(path.clone(), cx);
                                                } else {
                                                    this.select_item(idx, false, false, cx);
                                                }
                                            });
                                        }
                                    })
                                    .on_mouse_down(MouseButton::Right, {
                                        let view = view_row.clone();
                                        move |ev, _win, cx| {
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
                                        .children([
                                            div()
                                                .w(px(UiConstants::ICON_SIZE_RAW as f32))
                                                .h(px(UiConstants::ICON_SIZE_RAW as f32))
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .child(
                                                    div()
                                                        .text_size(AbsoluteLength::Pixels(px(14.0)))
                                                        .text_color(rgb(UiConstants::ACCENT_COLOR_RGB))
                                                        .child(if matches!(item_type, FileType::Directory) {
                                                            "\u{E8B7}"
                                                        } else {
                                                            "\u{E8A5}"
                                                        }),
                                                ),
                                            div()
                                                .flex_grow()
                                                .overflow_hidden()
                                                .text_size(AbsoluteLength::Pixels(px(14.0)))
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
                                                }),
                                        ]),
                                    div()
                                        .w(px(180.0))
                                        .text_size(AbsoluteLength::Pixels(px(12.0)))
                                        .text_color(rgb(UiConstants::TEXT_SECONDARY_RGB))
                                        .child("—"),
                                    div()
                                        .w(px(120.0))
                                        .text_size(AbsoluteLength::Pixels(px(12.0)))
                                        .text_color(rgb(UiConstants::TEXT_SECONDARY_RGB))
                                        .child(if matches!(item_type, FileType::Directory) {
                                            "Folder"
                                        } else {
                                            "File"
                                        }),
                                    div()
                                        .w(px(80.0))
                                        .text_right()
                                        .text_size(AbsoluteLength::Pixels(px(12.0)))
                                        .text_color(rgb(UiConstants::TEXT_SECONDARY_RGB))
                                        .child(size_str),
                                ])
                            }).collect::<Vec<_>>()
                        })
                    }),
            )
    }
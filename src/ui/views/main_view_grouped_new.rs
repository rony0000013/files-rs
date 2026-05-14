/// Render details view WITH collapsible group headers.
    /// Shows column header, then group headers with their items.
    fn details_view_grouped(
        state: &MainViewState,
        visible: &[usize],
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let (headers, groups) = state.partition_groups(visible);
        let view_vc = view.clone();

        div()
            .flex_col()
            .w_full()
            .flex_grow()
            // Column header row (same as flat view)
            .child(Self::column_header(state, view.clone()))
            // Scrollable groups container
            .child(
                div()
                    .flex_col()
                    .flex_grow()
                    .w_full()
                    .overflow_hidden()
                    .child({
                        // Build all elements for the scrollable area
                        let mut all_elements: Vec<AnyElement> = Vec::new();

                        for (gi, (key, label, count)) in headers.into_iter().enumerate() {
                            let is_collapsed = state.collapsed_groups.contains(&key);
                            let group_indices = &groups[gi];
                            let group_height = if state.compact_mode { 24.0 } else { 32.0 };
                            let v_g = view_vc.clone();

                            // Group header element
                            all_elements.push(
                                div()
                                    .id(ElementId::from(format!("group-header-{}", key)))
                                    .flex()
                                    .flex_row()
                                    .h(px(group_height))
                                    .items_center()
                                    .px(px(UiConstants::PADDING_LARGE_RAW as f32))
                                    .bg(rgba(0x0A000000u32))
                                    .cursor_pointer()
                                    .on_mouse_down(MouseButton::Left, {
                                        let key = key.clone();
                                        move |_, _, cx| {
                                            v_g.update(cx, |this, cx| {
                                                this.toggle_group_collapse(key.clone(), cx);
                                            });
                                        }
                                    })
                                    .child(
                                        div()
                                            .text_size(px(12.0))
                                            .font_family(UiConstants::FONT_MDL2)
                                            .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                            .child(if is_collapsed { "\u{E76C}" } else { "\u{E76B}" })
                                    )
                                    .child(
                                        div()
                                            .ml(px(6.0))
                                            .text_size(px(13.0))
                                            .font_weight(FontWeight(600.0))
                                            .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                                            .child(label)
                                    )
                                    .child(
                                        div()
                                            .ml(px(8.0))
                                            .text_size(px(11.0))
                                            .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                            .child(format!("{} item{}", count, if count == 1 { "" } else { "s" }))
                                    )
                                    .into_any_element()
                            );

                            // Item rows (only if not collapsed)
                            if !is_collapsed {
                                for &idx in group_indices {
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
                                    let view_row = view_vc.clone();

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
                                            move |_, _, cx| {
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
                                            move |ev, _, cx| {
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

                                    all_elements.push(
                                        row.children([
                                            // Name column (indented)
                                            div()
                                                .flex_grow()
                                                .flex()
                                                .items_center()
                                                .gap(px(8.0))
                                                .pl(px(20.0))
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
                                                                })
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
                                                .child(if matches!(item_type, FileType::Directory) { "Folder" } else { "File" }),
                                            div()
                                                .w(px(80.0))
                                                .text_right()
                                                .text_size(AbsoluteLength::Pixels(px(12.0)))
                                                .text_color(rgb(UiConstants::TEXT_SECONDARY_RGB))
                                                .child(size_str),
                                        ]).into_any_element()
                                    );
                                }
                            }
                        }

                        // Return a div containing all elements
                        div()
                            .flex_col()
                            .w_full()
                            .children(all_elements)
                            .into_any_element()
                    })
            )
    }
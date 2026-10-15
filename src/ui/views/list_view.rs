//! List View - Compact single-column file listing with uniform_list for performance
//!
//! This view displays files in a compact list format with optional grouping support.
//! Uses GPUI's uniform_list for efficient rendering of large file lists.

use crate::config::config::GroupColumn;
use crate::storage::storage::FileType;
use crate::ui::components::icon_renderer::is_thumbnail_image;
use crate::ui::constants::UiConstants;
use crate::ui::views::main_view::{
    format_size, get_extension, get_file_type_icon, rects_intersect, ColumnWidths, FileItems,
    GroupedItemType, LassoState, MainViewState, SelectionMode,
};
use gpui::*;

impl MainViewState {
    /// Render list view with optional grouping
    pub fn render_list_view(
        &self,
        file_items: Entity<FileItems>,
        view: Entity<crate::app::AppWindow>,
    ) -> AnyElement {
        let visible = self.visible_items_rc();

        if self.group_by != GroupColumn::None {
            return self
                .render_list_view_grouped(&visible, file_items, view)
                .into_any_element();
        }

        self.list_view_flat(&visible, file_items, view)
            .into_any_element()
    }

    /// Flat list view using uniform_list for maximum performance
    fn list_view_flat(
        &self,
        visible: &[usize],
        file_items: Entity<FileItems>,
        view: Entity<crate::app::AppWindow>,
    ) -> Div {
        let visible_clone = self.visible_items_rc();
        let zoom = self.zoom_level;
        let row_h = 24.0 + zoom * 32.0;
        let icon_size = 16.0 + zoom * 24.0;
        let text_size = 11.0 + zoom * 5.0;
        let is_checkbox_mode = self.selection_mode == SelectionMode::Checkbox;

        div()
            .size_full()
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
                let visible_for_lasso = visible_clone.clone();
                move |event, _win, cx| {
                    v.update(cx, |this, cx| {
                        this.state.main_view.lasso_state = Some(LassoState {
                            start: point(event.position.x.into(), event.position.y.into()),
                            current: point(event.position.x.into(), event.position.y.into()),
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
                            lasso.current = point(event.position.x.into(), event.position.y.into());
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
                                let row_height = 24.0 + this.state.main_view.zoom_level * 32.0;
                                this.state.main_view.selected_indices.clear();
                                for (i, &vi) in visible_for_lasso.iter().enumerate() {
                                    let item_y = i as f32 * row_height;
                                    let item_rect =
                                        (point(0.0, item_y), point(2000.0, item_y + row_height));
                                    if rects_intersect(item_rect, (lasso.rect().0, lasso.rect().1))
                                    {
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
                    "list-scroll",
                    visible_clone.len(),
                    move |range, _win, cx| {
                        let fi = file_items.read(cx);
                        let app_state = view.read(cx);
                        let main_state = &app_state.state.main_view;
                        let icon_paths = &main_state.icon_paths;
                        let zoom = main_state.zoom_level;
                        let row_h = 24.0 + zoom * 32.0;
                        let icon_size = 16.0 + zoom * 24.0;
                        let text_size = 11.0 + zoom * 5.0;
                        let is_checkbox = main_state.selection_mode == SelectionMode::Checkbox;

                        range
                            .map(|i| {
                                let idx = visible_clone[i];
                                let item = &fi.items[idx];
                                let is_selected = fi.selected_indices.contains(&idx);
                                let path = item.path.clone();
                                let view_row = view.clone();

                                let row_height = row_h;
                                let i_size = icon_size;
                                let t_size = text_size;

                                div()
                                    .id(ElementId::from(idx + 20000))
                                    .flex()
                                    .items_center()
                                    .w_full()
                                    .h(px(row_height))
                                    .px(px(16.0))
                                    .py(px(2.0))
                                    .bg(if is_selected {
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
                                    .child(if is_selected {
                                        // 3x16 Fluent selection pill.
                                        div()
                                            .w(px(3.0))
                                            .h(px(16.0))
                                            .rounded(px(1.5))
                                            .bg(rgb(UiConstants::ACCENT_COLOR_RGB))
                                    } else {
                                        div()
                                    })
                                    .on_mouse_down(MouseButton::Left, {
                                        let v = view_row.clone();
                                        move |_event, _, cx| {
                                            cx.stop_propagation();
                                            v.update(cx, |this, cx| {
                                                this.select_item(idx, false, false, cx);
                                            });
                                        }
                                    })
                                    .on_mouse_down(MouseButton::Right, {
                                        let v = view_row.clone();
                                        move |ev, _, cx| {
                                            cx.stop_propagation();
                                            v.update(cx, |this, cx| {
                                                this.show_context_menu(Some(idx), ev.position, cx);
                                            });
                                        }
                                    })
                                    .child(
                                        div()
                                            .w(px(i_size))
                                            .h(px(i_size))
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .child(
                                                if let Some(icon_path) = icon_paths.get(&path) {
                                                    img(icon_path.clone())
                                                        .w(px(i_size))
                                                        .h(px(i_size))
                                                        .into_any_element()
                                                } else {
                                                    let is_dir = matches!(
                                                        item.file_type,
                                                        FileType::Directory
                                                    );
                                                    div()
                                                        .text_size(px(i_size * 0.5))
                                                        .text_color(rgba(
                                                            UiConstants::TEXT_SECONDARY_RGBA,
                                                        ))
                                                        .child(if is_dir {
                                                            "\u{E8B7}"
                                                        } else {
                                                            "\u{E8A5}"
                                                        })
                                                        .into_any_element()
                                                },
                                            ),
                                    )
                                    .child(if main_state.renaming_index == Some(idx) {
                                        div()
                                            .bg(rgba(0xFFFFFF20u32))
                                            .border(px(1.0))
                                            .border_color(rgba(UiConstants::ACCENT_RGBA))
                                            .rounded(px(2.0))
                                            .px(px(4.0))
                                            .child(main_state.rename_text.clone())
                                            .into_any_element()
                                    } else {
                                        div().child(item.name.clone()).into_any_element()
                                    })
                                    .into_any_element()
                            })
                            .collect::<Vec<_>>()
                    },
                )
                .h_full()
                .track_scroll(self.uniform_list_scroll_handle.clone()),
            )
    }

    /// Grouped list view with collapsible group headers
    fn render_list_view_grouped(
        &self,
        visible: &[usize],
        file_items: Entity<FileItems>,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let flattened = self.flatten_grouped_items(visible);
        let item_count = flattened.len();
        let zoom = self.zoom_level;
        let row_h = 24.0 + zoom * 32.0;

        div().flex_col().w_full().flex_grow().child(
            uniform_list("list-grouped-scroll", item_count, {
                let view = view.clone();
                let visible_clone = self.visible_items_rc();
                move |range, _win, cx| {
                    let fi = file_items.read(cx);
                    let app_state = view.read(cx);
                    let main_state = &app_state.state.main_view;
                    let icon_paths = &main_state.icon_paths;
                    let zoom = main_state.zoom_level;
                    let row_h = 24.0 + zoom * 32.0;
                    let collapsed_groups = main_state.collapsed_groups.clone();

                    range
                        .map(|i| match &flattened[i] {
                            GroupedItemType::Header { key, label, count } => {
                                let group_height =
                                    if main_state.compact_mode { 24.0 } else { 32.0 };
                                let key_clone = key.clone();
                                let v = view.clone();

                                div()
                                    .id(ElementId::from(60000 + i))
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .h(px(group_height))
                                    .px(px(16.0))
                                    .bg(rgba(0x0A000000u32))
                                    .cursor_pointer()
                                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                        v.update(cx, |this, cx| {
                                            this.toggle_group_collapse(key_clone.clone(), cx);
                                        });
                                    })
                                    .child(
                                        div()
                                            .text_size(px(12.0))
                                            .font_family(UiConstants::FONT_MDL2)
                                            .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                            .child(if collapsed_groups.contains(key) {
                                                "\u{E76C}"
                                            } else {
                                                "\u{E76B}"
                                            }),
                                    )
                                    .child(
                                        div()
                                            .ml(px(8.0))
                                            .text_size(px(13.0))
                                            .font_weight(FontWeight(600.0))
                                            .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                                            .child(label.clone()),
                                    )
                                    .child(
                                        div()
                                            .ml(px(8.0))
                                            .text_size(px(11.0))
                                            .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                            .child(format!(
                                                "{} item{}",
                                                count,
                                                if *count == 1 { "" } else { "s" }
                                            )),
                                    )
                                    .into_any_element()
                            }
                            GroupedItemType::Item(idx) => {
                                let idx = *idx;
                                let item = &fi.items[idx];
                                let is_selected = fi.selected_indices.contains(&idx);
                                let path = item.path.clone();
                                let v = view.clone();
                                let icon_size = 16.0 + zoom * 24.0;
                                let text_size = 11.0 + zoom * 5.0;

                                div()
                                    .id(ElementId::from(idx + 20000))
                                    .flex()
                                    .items_center()
                                    .w_full()
                                    .h(px(row_h))
                                    .px(px(16.0))
                                    .py(px(2.0))
                                    .bg(if is_selected {
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
                                    .child(if is_selected {
                                        // 3x16 Fluent selection pill.
                                        div()
                                            .w(px(3.0))
                                            .h(px(16.0))
                                            .rounded(px(1.5))
                                            .bg(rgb(UiConstants::ACCENT_COLOR_RGB))
                                    } else {
                                        div()
                                    })
                                    .on_mouse_down(MouseButton::Left, {
                                        let v = v.clone();
                                        move |_event, _, cx| {
                                            cx.stop_propagation();
                                            v.update(cx, |this, cx| {
                                                this.select_item(idx, false, false, cx);
                                            });
                                        }
                                    })
                                    .on_mouse_down(MouseButton::Right, {
                                        let v = v.clone();
                                        move |ev, _, cx| {
                                            cx.stop_propagation();
                                            v.update(cx, |this, cx| {
                                                this.show_context_menu(Some(idx), ev.position, cx);
                                            });
                                        }
                                    })
                                    .child(
                                        div()
                                            .w(px(icon_size))
                                            .h(px(icon_size))
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .child(
                                                if let Some(icon_path) = icon_paths.get(&path) {
                                                    img(icon_path.clone())
                                                        .w(px(icon_size))
                                                        .h(px(icon_size))
                                                        .into_any_element()
                                                } else {
                                                    let is_dir = matches!(
                                                        item.file_type,
                                                        FileType::Directory
                                                    );
                                                    div()
                                                        .text_size(px(icon_size * 0.5))
                                                        .text_color(rgba(
                                                            UiConstants::TEXT_SECONDARY_RGBA,
                                                        ))
                                                        .child(if is_dir {
                                                            "\u{E8B7}"
                                                        } else {
                                                            "\u{E8A5}"
                                                        })
                                                        .into_any_element()
                                                },
                                            ),
                                    )
                                    .child(if main_state.renaming_index == Some(idx) {
                                        div()
                                            .bg(rgba(0xFFFFFF20u32))
                                            .border(px(1.0))
                                            .border_color(rgba(UiConstants::ACCENT_RGBA))
                                            .rounded(px(2.0))
                                            .px(px(4.0))
                                            .child(main_state.rename_text.clone())
                                            .into_any_element()
                                    } else {
                                        div().child(item.name.clone()).into_any_element()
                                    })
                                    .into_any_element()
                            }
                        })
                        .collect::<Vec<_>>()
                }
            })
            .track_scroll(self.uniform_list_scroll_handle.clone()),
        )
    }
}

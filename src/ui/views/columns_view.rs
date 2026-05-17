//! Columns View - Finder-style three-pane view with uniform_list for performance
//!
//! This view displays files in a Finder-style three-column layout:
//! - Left column: Parent path
//! - Middle column: Current directory contents
//! - Right column: Preview pane
//!
//! Uses GPUI's uniform_list for efficient rendering.

use crate::config::config::GroupColumn;
use crate::storage::storage::FileType;
use crate::ui::constants::UiConstants;
use crate::ui::views::main_view::{FileItems, MainViewState};
use gpui::*;

impl MainViewState {
    /// Render columns view (Finder-style) with optional grouping
    pub fn render_columns_view(&self, file_items: Entity<FileItems>, view: Entity<crate::app::AppWindow>) -> AnyElement {
        let visible = self.visible_items();

        if self.group_by != GroupColumn::None {
            return self.render_columns_view_grouped(&visible, file_items, view).into_any_element();
        }

        self.columns_view_flat(&visible, file_items, view).into_any_element()
    }

    /// Flat columns view using uniform_list
    fn columns_view_flat(
        &self,
        visible: &[usize],
        file_items: Entity<FileItems>,
        view: Entity<crate::app::AppWindow>,
    ) -> Div {
        let visible_clone = visible.to_vec();
        let selected_item = self.selected_indices.iter().next()
            .and_then(|&i| self.items.get(i));
        let parent_path = self.primary_path.parent()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|| "—".to_string());
        let zoom = self.zoom_level;
        let row_h = 20.0 + zoom * 28.0;
        let icon_size = (row_h - 8.0).max(16.0).min(40.0);
        let text_size = 11.0 + zoom * 5.0;

        div()
            .flex()
            .flex_row()
            .flex_grow()
            // NO overflow_hidden() - would block child scroll events
            // Column 1: Parent path label
            .child(
                div()
                    .w(px(220.0))
                    .h_full()
                    .flex_col()
                    .border_r(px(1.0))
                    .border_color(rgba(UiConstants::DIVIDER_RGBA))
                    .child(
                        div()
                            .h(px(32.0))
                            .px(px(12.0))
                            .flex()
                            .items_center()
                            .border_b(px(1.0))
                            .border_color(rgba(UiConstants::DIVIDER_RGBA))
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .font_weight(FontWeight(600.0))
                                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                    .child(parent_path),
                            ),
                    )
                    .child(
                        div()
                            .id("col1-content")
                            .flex_grow()
                            .flex_col()
                            .p(px(4.0))
                            .overflow_y_scroll()
                            .track_scroll(&self.scroll_handle)
                            .child(
                                div()
                                    .h(px(28.0))
                                    .px(px(8.0))
                                    .flex()
                                    .items_center()
                                    .rounded(px(4.0))
                                    .bg(rgba(UiConstants::BG_ITEM_SELECTED_RGBA))
                                    .child(
                                        div()
                                            .text_size(px(13.0))
                                            .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                                            .child(
                                                self.primary_path
                                                    .file_name()
                                                    .map(|n| n.to_string_lossy().into_owned())
                                                    .unwrap_or_else(|| self.primary_path.to_string_lossy().into_owned())
                                            ),
                                    ),
                            ),
                    ),
            )
            // Column 2: Current directory contents
            .child(
                div()
                    .w(px(240.0))
                    .h_full()
                    .flex_col()
                    .border_r(px(1.0))
                    .border_color(rgba(UiConstants::DIVIDER_RGBA))
                    .child(
                        div()
                            .h(px(32.0))
                            .px(px(12.0))
                            .flex()
                            .items_center()
                            .border_b(px(1.0))
                            .border_color(rgba(UiConstants::DIVIDER_RGBA))
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .font_weight(FontWeight(600.0))
                                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                    .child(
                                        self.primary_path
                                            .file_name()
                                            .map(|n| n.to_string_lossy().into_owned())
                                            .unwrap_or_default()
                                    ),
                            ),
                    )
                    .child(
                        uniform_list("col2-scroll", visible_clone.len(), {
                            let view = view.clone();
                            move |range, _win, cx| {
                                let fi = file_items.read(cx);
                                let app_state = view.read(cx);
                                let main_state = &app_state.state.main_view;
                                let icon_paths = &main_state.icon_paths;
                                let r_h = 20.0 + main_state.zoom_level * 28.0;
                                let i_size = (r_h - 8.0).max(16.0).min(40.0);
                                let t_size = 11.0 + main_state.zoom_level * 5.0;

                                range
                                    .map(|i| {
                                        let idx = visible_clone[i];
                                        let item = &fi.items[idx];
                                        let is_selected = fi.selected_indices.contains(&idx);
                                        let is_dir = matches!(item.file_type, FileType::Directory);
                                        let name = item.name.clone();
                                        let path = item.path.clone();
                                        let v = view.clone();

                                        div()
                                            .id(ElementId::from(50000 + idx))
                                            .flex()
                                            .flex_row()
                                            .h(px(r_h))
                                            .px(px(8.0))
                                            .items_center()
                                            .gap(px(6.0))
                                            .rounded(px(4.0))
                                            .mx(px(4.0))
                                            .bg(if is_selected {
                                                rgba(UiConstants::BG_ITEM_SELECTED_RGBA)
                                            } else {
                                                rgba(0x00000000)
                                            })
                                            .hover(|s| {
                                                if !is_selected {
                                                    s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA))
                                                } else {
                                                    s
                                                }
                                            })
                                            .cursor_pointer()
                                            .on_mouse_down(MouseButton::Left, {
                                                let v = v.clone();
                                                let path = path.clone();
                                                move |_, _, cx| {
                                                    cx.stop_propagation();
                                                    v.update(cx, |this, cx| {
                                                        if is_dir {
                                                            this.navigate(path.clone(), cx);
                                                        } else {
                                                            this.select_item(idx, false, false, cx);
                                                        }
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
                                                            div()
                                                                .text_size(px(i_size * 0.7))
                                                                .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                                                .child(if is_dir { "\u{E8B7}" } else { "\u{E8A5}" })
                                                                .into_any_element()
                                                        }
                                                    )
                                            )
                                            .child(
                                                div()
                                                    .flex_grow()
                                                    .text_size(px(t_size))
                                                    .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                                                    .child(name)
                                            ).into_any_element()
                                    }).collect::<Vec<_>>()
                            }
                        })
                        .track_scroll(self.uniform_list_scroll_handle.clone()),
                    ),
            )
            // Column 3: Preview panel
            .child(
                div()
                    .flex_grow()
                    .h_full()
                    .flex_col()
                    .bg(rgba(0xFFFFFF05u32))
                    .child(match selected_item {
                        None => div()
                            .flex_grow()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(rgba(0xFFFFFF40u32))
                                    .child("Select a file to preview"),
                            )
                            .into_any_element(),
                        Some(item) => div()
                            .flex_grow()
                            .p(px(16.0))
                            .child(Self::render_details_pane_internal(item))
                            .into_any_element(),
                    }),
            )
    }

    /// Grouped columns view (simplified - groups items in the middle column)
    fn render_columns_view_grouped(
        &self,
        visible: &[usize],
        file_items: Entity<FileItems>,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        // For grouped columns, we flatten the grouped items and render them with headers
        let flattened = self.flatten_grouped_items(visible);
        let item_count = flattened.len();

        let parent_path = self.primary_path.parent()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|| "—".to_string());
        let selected_item = self.selected_indices.iter().next()
            .and_then(|&i| self.items.get(i));

        div()
            .flex()
            .flex_row()
            .flex_grow()
            // NO overflow_hidden() - would block child scroll events
            // Column 1: Parent path label
            .child(
                div()
                    .w(px(220.0))
                    .h_full()
                    .flex_col()
                    .border_r(px(1.0))
                    .border_color(rgba(UiConstants::DIVIDER_RGBA))
                    .child(
                        div()
                            .h(px(32.0))
                            .px(px(12.0))
                            .flex()
                            .items_center()
                            .border_b(px(1.0))
                            .border_color(rgba(UiConstants::DIVIDER_RGBA))
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .font_weight(FontWeight(600.0))
                                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                    .child(parent_path),
                            ),
                    )
                    .child(
                        div()
                            .id("col1-content")
                            .flex_grow()
                            .flex_col()
                            .p(px(4.0))
                            .overflow_y_scroll()
                            .track_scroll(&self.scroll_handle)
                            .child(
                                div()
                                    .h(px(28.0))
                                    .px(px(8.0))
                                    .flex()
                                    .items_center()
                                    .rounded(px(4.0))
                                    .bg(rgba(UiConstants::BG_ITEM_SELECTED_RGBA))
                                    .child(
                                        div()
                                            .text_size(px(13.0))
                                            .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                                            .child(
                                                self.primary_path
                                                    .file_name()
                                                    .map(|n| n.to_string_lossy().into_owned())
                                                    .unwrap_or_else(|| self.primary_path.to_string_lossy().into_owned())
                                            ),
                                    ),
                            ),
                    ),
            )
            // Column 2: Current directory contents (with grouping)
            .child(
                div()
                    .w(px(240.0))
                    .h_full()
                    .flex_col()
                    .border_r(px(1.0))
                    .border_color(rgba(UiConstants::DIVIDER_RGBA))
                    .child(
                        div()
                            .h(px(32.0))
                            .px(px(12.0))
                            .flex()
                            .items_center()
                            .border_b(px(1.0))
                            .border_color(rgba(UiConstants::DIVIDER_RGBA))
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .font_weight(FontWeight(600.0))
                                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                    .child(
                                        self.primary_path
                                            .file_name()
                                            .map(|n| n.to_string_lossy().into_owned())
                                            .unwrap_or_default()
                                    ),
                            ),
                    )
                    .child(
                        uniform_list("col2-grouped-scroll", item_count, {
                            use crate::ui::views::main_view::GroupedItemType;
                            let view = view.clone();
                            move |range, _win, cx| {
                                let fi = file_items.read(cx);
                                let app_state = view.read(cx);
                                let main_state = &app_state.state.main_view;
                                let icon_paths = &main_state.icon_paths;
                                let collapsed_groups = main_state.collapsed_groups.clone();

                                range
                                    .map(|i| match &flattened[i] {
                                        GroupedItemType::Header { key, label, count } => {
                                            let group_height = if main_state.compact_mode { 24.0 } else { 28.0 };
                                            let key_clone = key.clone();
                                            let v = view.clone();

                                            div()
                                                .id(ElementId::from(60000 + i))
                                                .flex()
                                                .flex_row()
                                                .h(px(group_height))
                                                .px(px(8.0))
                                                .items_center()
                                                .gap(px(4.0))
                                                .bg(rgba(0x0A000000u32))
                                                .cursor_pointer()
                                                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                                    v.update(cx, |this, cx| {
                                                        this.toggle_group_collapse(key_clone.clone(), cx);
                                                    });
                                                })
                                                .child(
                                                    div()
                                                        .text_size(px(10.0))
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
                                                        .flex_grow()
                                                        .text_size(px(12.0))
                                                        .font_weight(FontWeight(600.0))
                                                        .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                                                        .child(label.clone()),
                                                )
                                                .child(
                                                    div()
                                                        .text_size(px(10.0))
                                                        .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                                        .child(format!("{}", count)),
                                                )
                                                .into_any_element()
                                        }
                                        GroupedItemType::Item(idx) => {
                                            let idx = *idx;
                                            let item = &fi.items[idx];
                                            let is_selected = fi.selected_indices.contains(&idx);
                                            let is_dir = matches!(item.file_type, FileType::Directory);
                                            let name = item.name.clone();
                                            let path = item.path.clone();
                                            let v = view.clone();
                                            let r_h = 20.0 + main_state.zoom_level * 28.0;
                                            let i_size = (r_h - 8.0).max(16.0).min(40.0);
                                            let t_size = 11.0 + main_state.zoom_level * 5.0;

                                            div()
                                                .id(ElementId::from(50000 + idx))
                                                .flex()
                                                .flex_row()
                                                .h(px(r_h))
                                                .px(px(8.0))
                                                .items_center()
                                                .gap(px(6.0))
                                                .rounded(px(4.0))
                                                .mx(px(4.0))
                                                .bg(if is_selected {
                                                    rgba(UiConstants::BG_ITEM_SELECTED_RGBA)
                                                } else {
                                                    rgba(0x00000000)
                                                })
                                                .hover(|s| {
                                                    if !is_selected {
                                                        s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA))
                                                    } else {
                                                        s
                                                    }
                                                })
                                                .cursor_pointer()
                                                .on_mouse_down(MouseButton::Left, {
                                                    let v = v.clone();
                                                    let path = path.clone();
                                                    move |_, _, cx| {
                                                        cx.stop_propagation();
                                                        v.update(cx, |this, cx| {
                                                            if is_dir {
                                                                this.navigate(path.clone(), cx);
                                                            } else {
                                                                this.select_item(idx, false, false, cx);
                                                            }
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
                                                                div()
                                                                    .text_size(px(i_size * 0.7))
                                                                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                                                    .child(if is_dir { "\u{E8B7}" } else { "\u{E8A5}" })
                                                                    .into_any_element()
                                                            }
                                                        )
                                                )
                                                .child(
                                                    div()
                                                        .flex_grow()
                                                        .text_size(px(t_size))
                                                        .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                                                        .child(name)
                                                ).into_any_element()
                                        }
                                    }).collect::<Vec<_>>()
                            }
                        })
                        .track_scroll(self.uniform_list_scroll_handle.clone()),
                    ),
            )
            // Column 3: Preview panel
            .child(
                div()
                    .flex_grow()
                    .h_full()
                    .flex_col()
                    .bg(rgba(0xFFFFFF05u32))
                    .child(match selected_item {
                        None => div()
                            .flex_grow()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(rgba(0xFFFFFF40u32))
                                    .child("Select a file to preview"),
                            )
                            .into_any_element(),
                        Some(item) => div()
                            .flex_grow()
                            .p(px(16.0))
                            .child(Self::render_details_pane_internal(item))
                            .into_any_element(),
                    }),
            )
    }

    /// Internal helper to render details pane for the columns view
    fn render_details_pane_internal(item: &crate::domain::metadata::FileItem) -> impl IntoElement {
        use crate::storage::storage::FileType;
        use crate::ui::views::helpers::format_size;

        let is_dir = matches!(item.file_type, FileType::Directory);

        div()
            .flex_col()
            .gap(px(16.0))
            .w_full()
            .child(
                div()
                    .flex_col()
                    .items_center()
                    .gap(px(12.0))
                    .child(
                        div()
                            .w(px(64.0))
                            .h(px(64.0))
                            .bg(rgba(UiConstants::BG_BADGE_RGBA))
                            .rounded(px(8.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_size(px(32.0))
                            .font_family(UiConstants::FONT_MDL2)
                            .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                            .child(if is_dir { "\u{E8B7}" } else { "\u{E8A5}" }),
                    )
                    .child(
                        div()
                            .text_size(px(16.0))
                            .font_weight(FontWeight(600.0))
                            .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                            .child(item.name.clone()),
                    ),
            )
            .child(
                div()
                    .flex_col()
                    .gap(px(8.0))
                    .child(
                        div()
                            .text_size(px(11.0))
                            .font_weight(FontWeight(600.0))
                            .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                            .child("Details".to_uppercase()),
                    )
                    .child(
                        div()
                            .flex_col()
                            .gap(px(4.0))
                            .child(
                                div()
                                    .flex()
                                    .justify_between()
                                    .child(
                                        div()
                                            .text_size(px(12.0))
                                            .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                            .child("Type"),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(12.0))
                                            .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                                            .child(if is_dir { "Folder" } else { "File" }),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .justify_between()
                                    .child(
                                        div()
                                            .text_size(px(12.0))
                                            .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                            .child("Size"),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(12.0))
                                            .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                                            .child(format_size(item.size)),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .justify_between()
                                    .child(
                                        div()
                                            .text_size(px(12.0))
                                            .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                            .child("Location"),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(12.0))
                                            .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                                            .overflow_hidden()
                                            .child(item.path.parent().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default()),
                                    ),
                            ),
                    ),
            )
    }
}
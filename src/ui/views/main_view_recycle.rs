//! Recycle bin view rendering for MainView.
//!
//! Extracted from main_view.rs for maintainability:
//! - Recycle bin view layout with header, action buttons, and item list
//! - Per-item restore buttons

use gpui::*;

use crate::ui::constants::UiConstants;

use super::main_view::{MainView, MainViewState};

impl MainView {
    pub(crate) fn render_recycle_bin_view(
        state: &MainViewState,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let item_count = state.recycle_bin_items.len();

        div()
            .flex_col()
            .flex_grow()
            .p(px(24.0))
            .gap(px(16.0))
            .id("recycle-bin-view")
            // Header
            .child(
                div().flex_row().items_center().gap(px(12.0)).child(
                    div()
                        .font_weight(FontWeight(600.0))
                        .text_size(px(20.0))
                        .font_family(UiConstants::FONT_MAIN)
                        .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                        .child(format!("Recycle Bin ({} items)", item_count)),
                ),
            )
            // Action buttons
            .child(
                div()
                    .flex_row()
                    .gap(px(8.0))
                    // Empty Recycle Bin button
                    .child(
                        div()
                            .px(px(12.0))
                            .py(px(6.0))
                            .bg(rgba(0xE8112320u32))
                            .rounded(px(4.0))
                            .cursor_pointer()
                            .hover(|s| s.bg(rgba(0xE8112340u32)))
                            .on_mouse_down(MouseButton::Left, {
                                let view = view.clone();
                                move |_, _, cx| {
                                    view.update(cx, |this, cx| {
                                        this.empty_recycle_bin(cx);
                                    });
                                }
                            })
                            .child(
                                div()
                                    .text_size(px(13.0))
                                    .font_family(UiConstants::FONT_MAIN)
                                    .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                                    .child("Empty Recycle Bin"),
                            ),
                    )
                    // Restore All button
                    .child(
                        div()
                            .px(px(12.0))
                            .py(px(6.0))
                            .bg(rgba(0x107C1020u32))
                            .rounded(px(4.0))
                            .cursor_pointer()
                            .hover(|s| s.bg(rgba(0x107C1040u32)))
                            .on_mouse_down(MouseButton::Left, {
                                let view = view.clone();
                                move |_, _, cx| {
                                    view.update(cx, |this, cx| {
                                        this.restore_all_recycle_bin(cx);
                                    });
                                }
                            })
                            .child(
                                div()
                                    .text_size(px(13.0))
                                    .font_family(UiConstants::FONT_MAIN)
                                    .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                                    .child("Restore All"),
                            ),
                    ),
            )
            // Item list
            .child(div().flex_col().gap(px(4.0)).children(
                state.recycle_bin_items.iter().enumerate().map(|(i, item)| {
                    let view = view.clone();
                    div()
                        .flex_row()
                        .items_center()
                        .gap(px(12.0))
                        .px(px(12.0))
                        .py(px(8.0))
                        .bg(rgba(0xFFFFFF08u32))
                        .rounded(px(4.0))
                        .child(
                            div()
                                .flex_col()
                                .flex_grow()
                                .gap(px(2.0))
                                .child(
                                    div()
                                        .text_size(px(14.0))
                                        .font_family(UiConstants::FONT_MAIN)
                                        .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                                        .child(item.name.clone()),
                                )
                                .child(
                                    div()
                                        .flex_row()
                                        .gap(px(16.0))
                                        .child(
                                            div()
                                                .text_size(px(11.0))
                                                .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                                .child(format!("Original: {}", item.original_path)),
                                        )
                                        .child(
                                            div()
                                                .text_size(px(11.0))
                                                .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                                .child(format!(
                                                    "Size: {}",
                                                    if item.size >= 1024 * 1024 {
                                                        format!(
                                                            "{:.1} MB",
                                                            item.size as f64 / (1024.0 * 1024.0)
                                                        )
                                                    } else if item.size >= 1024 {
                                                        format!(
                                                            "{:.1} KB",
                                                            item.size as f64 / 1024.0
                                                        )
                                                    } else {
                                                        format!("{} B", item.size)
                                                    }
                                                )),
                                        )
                                        .child(
                                            div()
                                                .text_size(px(11.0))
                                                .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                                .child(format!(
                                                    "Deleted: {}",
                                                    item.date_deleted
                                                        .as_deref()
                                                        .unwrap_or("Unknown")
                                                )),
                                        ),
                                ),
                        )
                        // Restore button per item
                        .child(
                            div()
                                .px(px(10.0))
                                .py(px(4.0))
                                .bg(rgba(0x107C1020u32))
                                .rounded(px(4.0))
                                .cursor_pointer()
                                .hover(|s| s.bg(rgba(0x107C1040u32)))
                                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                    view.update(cx, |this, cx| {
                                        this.restore_recycle_bin_item(i, cx);
                                    });
                                })
                                .child(
                                    div()
                                        .text_size(px(12.0))
                                        .font_family(UiConstants::FONT_MAIN)
                                        .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                                        .child("Restore"),
                                ),
                        )
                }),
            ))
    }
}

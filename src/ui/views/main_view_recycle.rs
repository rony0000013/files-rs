//! Recycle Bin view rendering for MainView (`Files-community` RecycleBin parity).
//!
//! Features:
//! - Recycle Bin summary bar with item count, total size, and contextual actions
//!   (`Empty Recycle Bin`, `Restore All`, `Restore Selected`, `Delete Selected`)
//! - Full `Files-community` Details table layout:
//!   Checkbox | Name | Original Location | Date Deleted | Size | Type | Quick Actions
//! - Row selection (`Click`, `Ctrl+Click`), filter query matching (`state.filter_text`),
//!   and per-item `Restore` / `Delete Permanently` buttons
//! - Authentic `Files-community` Empty Recycle Bin illustration state when empty

use gpui::*;

use crate::ui::constants::UiConstants;

use super::main_view::{format_size, MainView, MainViewState};

impl MainView {
    pub(crate) fn render_recycle_bin_view(
        state: &MainViewState,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let filter_q = state.filter_text.trim().to_ascii_lowercase();
        let filtered_items: Vec<(usize, &crate::platform::windows::recycle_bin_service::RecycleBinItem)> =
            state
                .recycle_bin_items
                .iter()
                .enumerate()
                .filter(|(_, item)| {
                    if filter_q.is_empty() {
                        true
                    } else {
                        item.name.to_ascii_lowercase().contains(&filter_q)
                            || item.original_path.to_ascii_lowercase().contains(&filter_q)
                    }
                })
                .collect();

        let total_count = state.recycle_bin_items.len();
        let total_bytes: u64 = state.recycle_bin_items.iter().map(|i| i.size).sum();
        let selected_count = state.selected_indices.len();
        let all_selected = total_count > 0 && selected_count == total_count;

        let v_empty = view.clone();
        let v_restore_all = view.clone();
        let v_restore_sel = view.clone();
        let v_delete_sel = view.clone();
        let v_toggle_all = view.clone();

        // Top Summary & Action Header Card
        let header_card = div()
            .w_full()
            .px(px(16.0))
            .py(px(12.0))
            .rounded(px(8.0))
            .bg(rgba(0xFFFFFF09u32))
            .border(px(1.0))
            .border_color(rgba(UiConstants::CARD_STROKE_RGBA))
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .gap(px(16.0))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(12.0))
                    .child(
                        div()
                            .w(px(38.0))
                            .h(px(38.0))
                            .rounded(px(8.0))
                            .bg(rgba(0x60CDFF1Eu32))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                div()
                                    .text_size(px(19.0))
                                    .font_family(UiConstants::FONT_ICONS)
                                    .text_color(rgb(0x60CDFF))
                                    .child("\u{E74D}"),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(2.0))
                            .child(
                                div()
                                    .text_size(px(15.0))
                                    .font_weight(FontWeight(600.0))
                                    .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                                    .child("Recycle Bin"),
                            )
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                    .child(if total_count == 0 {
                                        "Recycle Bin is empty".to_string()
                                    } else if selected_count > 0 {
                                        format!(
                                            "{} of {} items selected • Total size {}",
                                            selected_count,
                                            total_count,
                                            format_size(total_bytes)
                                        )
                                    } else {
                                        format!(
                                            "{} items • Total size {}",
                                            total_count,
                                            format_size(total_bytes)
                                        )
                                    }),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.0))
                    .children(if selected_count > 0 {
                        vec![
                            div()
                                .px(px(12.0))
                                .py(px(6.0))
                                .rounded(px(6.0))
                                .bg(rgba(0x4CCB5E26u32))
                                .border(px(1.0))
                                .border_color(rgba(0x4CCB5E66u32))
                                .hover(|s| s.bg(rgba(0x4CCB5E3Au32)))
                                .cursor_pointer()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap(px(6.0))
                                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                    v_restore_sel.update(cx, |this, cx| {
                                        this.restore_selected_recycle_bin(cx);
                                    });
                                })
                                .child(
                                    div()
                                        .text_size(px(12.5))
                                        .font_family(UiConstants::FONT_ICONS)
                                        .text_color(rgb(0x4CCB5E))
                                        .child("\u{E777}"),
                                )
                                .child(
                                    div()
                                        .text_size(px(12.5))
                                        .font_weight(FontWeight(600.0))
                                        .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                                        .child(format!("Restore Selected ({})", selected_count)),
                                )
                                .into_any_element(),
                            div()
                                .px(px(12.0))
                                .py(px(6.0))
                                .rounded(px(6.0))
                                .bg(rgba(0xE8112328u32))
                                .border(px(1.0))
                                .border_color(rgba(0xE8112366u32))
                                .hover(|s| s.bg(rgba(0xE8112340u32)))
                                .cursor_pointer()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap(px(6.0))
                                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                    v_delete_sel.update(cx, |this, cx| {
                                        this.delete_selected_recycle_bin(cx);
                                    });
                                })
                                .child(
                                    div()
                                        .text_size(px(12.5))
                                        .font_family(UiConstants::FONT_ICONS)
                                        .text_color(rgb(0xFF8080))
                                        .child("\u{E74D}"),
                                )
                                .child(
                                    div()
                                        .text_size(px(12.5))
                                        .font_weight(FontWeight(600.0))
                                        .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                                        .child("Delete Permanently"),
                                )
                                .into_any_element(),
                        ]
                    } else {
                        vec![]
                    })
                    .child(
                        div()
                            .px(px(12.0))
                            .py(px(6.0))
                            .rounded(px(6.0))
                            .bg(rgba(0xFFFFFF10u32))
                            .border(px(1.0))
                            .border_color(rgba(UiConstants::CARD_STROKE_RGBA))
                            .opacity(if total_count > 0 { 1.0 } else { 0.45 })
                            .hover(|s| {
                                if total_count > 0 {
                                    s.bg(rgba(0xFFFFFF1Cu32))
                                } else {
                                    s
                                }
                            })
                            .cursor_pointer()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(6.0))
                            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                if total_count > 0 {
                                    v_restore_all.update(cx, |this, cx| {
                                        this.restore_all_recycle_bin(cx);
                                    });
                                }
                            })
                            .child(
                                div()
                                    .text_size(px(12.5))
                                    .font_family(UiConstants::FONT_ICONS)
                                    .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                                    .child("\u{E777}"),
                            )
                            .child(
                                div()
                                    .text_size(px(12.5))
                                    .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                                    .child("Restore all items"),
                            ),
                    )
                    .child(
                        div()
                            .px(px(12.0))
                            .py(px(6.0))
                            .rounded(px(6.0))
                            .bg(rgba(0xE8112328u32))
                            .border(px(1.0))
                            .border_color(rgba(0xE8112366u32))
                            .opacity(if total_count > 0 { 1.0 } else { 0.45 })
                            .hover(|s| {
                                if total_count > 0 {
                                    s.bg(rgba(0xE8112344u32))
                                } else {
                                    s
                                }
                            })
                            .cursor_pointer()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(6.0))
                            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                if total_count > 0 {
                                    v_empty.update(cx, |this, cx| {
                                        this.empty_recycle_bin(cx);
                                    });
                                }
                            })
                            .child(
                                div()
                                    .text_size(px(12.5))
                                    .font_family(UiConstants::FONT_ICONS)
                                    .text_color(rgb(0xFF8080))
                                    .child("\u{E74D}"),
                            )
                            .child(
                                div()
                                    .text_size(px(12.5))
                                    .font_weight(FontWeight(600.0))
                                    .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                                    .child("Empty Recycle Bin"),
                            ),
                    ),
            );

        if filtered_items.is_empty() {
            return div()
                .id("recycle-bin-view")
                .flex()
                .flex_col()
                .flex_grow()
                .w_full()
                .h_full()
                .px(px(20.0))
                .py(px(16.0))
                .gap(px(16.0))
                .child(header_card)
                .child(
                    div()
                        .flex_1()
                        .w_full()
                        .rounded(px(8.0))
                        .bg(rgba(0xFFFFFF06u32))
                        .border(px(1.0))
                        .border_color(rgba(UiConstants::CARD_STROKE_RGBA))
                        .flex()
                        .flex_col()
                        .items_center()
                        .justify_center()
                        .gap(px(10.0))
                        .child(
                            div()
                                .w(px(64.0))
                                .h(px(64.0))
                                .rounded(px(32.0))
                                .bg(rgba(0xFFFFFF08u32))
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(
                                    div()
                                        .text_size(px(28.0))
                                        .font_family(UiConstants::FONT_ICONS)
                                        .text_color(rgba(UiConstants::TEXT_MUTED_RGBA))
                                        .child("\u{E74D}"),
                                ),
                        )
                        .child(
                            div()
                                .text_size(px(15.0))
                                .font_weight(FontWeight(600.0))
                                .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                                .child(if filter_q.is_empty() {
                                    "Recycle Bin is empty"
                                } else {
                                    "No Recycle Bin items match your filter"
                                }),
                        )
                        .child(
                            div()
                                .text_size(px(12.5))
                                .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                .child(if filter_q.is_empty() {
                                    "Items you delete from your drives will appear here for easy recovery."
                                } else {
                                    "Try clearing the filter text to see all deleted items."
                                }),
                        ),
                );
        }

        // Table Header Row (Files-community Recycle Bin columns: Checkbox | Name | Original Location | Date Deleted | Size | Type | Actions)
        let table_header = div()
            .w_full()
            .h(px(34.0))
            .px(px(12.0))
            .flex()
            .flex_row()
            .items_center()
            .bg(rgba(0xFFFFFF07u32))
            .border_b(px(1.0))
            .border_color(rgba(UiConstants::CARD_STROKE_RGBA))
            .child(
                // Select all checkbox
                div()
                    .w(px(30.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        v_toggle_all.update(cx, |this, cx| {
                            let len = this.state.main_view.recycle_bin_items.len();
                            if this.state.main_view.selected_indices.len() == len {
                                this.state.main_view.selected_indices.clear();
                            } else {
                                this.state.main_view.selected_indices = (0..len).collect();
                            }
                            cx.notify();
                        });
                    })
                    .child(
                        div()
                            .w(px(16.0))
                            .h(px(16.0))
                            .rounded(px(3.5))
                            .border(px(1.0))
                            .border_color(if all_selected {
                                rgba(UiConstants::ACCENT_RGBA)
                            } else {
                                rgba(0xFFFFFF44u32)
                            })
                            .bg(if all_selected {
                                rgba(UiConstants::ACCENT_RGBA)
                            } else {
                                rgba(0x00000000u32)
                            })
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                div()
                                    .text_size(px(10.0))
                                    .font_family(UiConstants::FONT_ICONS)
                                    .text_color(rgb(0xFFFFFF))
                                    .child(if all_selected { "\u{E73E}" } else { "" }),
                            ),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_w(px(180.0))
                    .text_size(px(11.5))
                    .font_weight(FontWeight(600.0))
                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                    .child("Name"),
            )
            .child(
                div()
                    .w(px(260.0))
                    .text_size(px(11.5))
                    .font_weight(FontWeight(600.0))
                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                    .child("Original Location"),
            )
            .child(
                div()
                    .w(px(140.0))
                    .text_size(px(11.5))
                    .font_weight(FontWeight(600.0))
                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                    .child("Date Deleted"),
            )
            .child(
                div()
                    .w(px(90.0))
                    .text_right()
                    .pr(px(12.0))
                    .text_size(px(11.5))
                    .font_weight(FontWeight(600.0))
                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                    .child("Size"),
            )
            .child(
                div()
                    .w(px(110.0))
                    .text_size(px(11.5))
                    .font_weight(FontWeight(600.0))
                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                    .child("Type"),
            )
            .child(
                div()
                    .w(px(130.0))
                    .text_right()
                    .text_size(px(11.5))
                    .font_weight(FontWeight(600.0))
                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                    .child("Actions"),
            );

        let rows: Vec<AnyElement> = filtered_items
            .into_iter()
            .map(|(orig_idx, item)| {
                let is_selected = state.selected_indices.contains(&orig_idx);
                let name = item.name.clone();
                let orig_loc = item
                    .original_parent
                    .clone()
                    .unwrap_or_else(|| item.original_path.clone());
                let date_del = item
                    .date_deleted
                    .clone()
                    .unwrap_or_else(|| "Recently deleted".to_string());
                let size_str = if item.is_dir {
                    "—".to_string()
                } else {
                    format_size(item.size)
                };
                let type_str = item.item_type_label();
                let is_dir = item.is_dir;

                let v_row = view.clone();
                let v_check = view.clone();
                let v_restore = view.clone();
                let v_delete = view.clone();

                div()
                    .w_full()
                    .h(px(38.0))
                    .px(px(12.0))
                    .flex()
                    .flex_row()
                    .items_center()
                    .border_b(px(1.0))
                    .border_color(rgba(0xFFFFFF08u32))
                    .bg(if is_selected {
                        rgba(UiConstants::BG_ITEM_SELECTED_RGBA)
                    } else {
                        rgba(0x00000000u32)
                    })
                    .hover(|s| {
                        if is_selected {
                            s.bg(rgba(UiConstants::BG_ITEM_SELECTED_RGBA))
                        } else {
                            s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA))
                        }
                    })
                    .cursor_pointer()
                    .on_mouse_down(MouseButton::Left, move |ev, _, cx| {
                        let ctrl = ev.modifiers.control;
                        v_row.update(cx, |this, cx| {
                            if ctrl {
                                if this.state.main_view.selected_indices.contains(&orig_idx) {
                                    this.state.main_view.selected_indices.remove(&orig_idx);
                                } else {
                                    this.state.main_view.selected_indices.insert(orig_idx);
                                }
                            } else {
                                this.state.main_view.selected_indices.clear();
                                this.state.main_view.selected_indices.insert(orig_idx);
                            }
                            cx.notify();
                        });
                    })
                    // Checkbox cell
                    .child(
                        div()
                            .w(px(30.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                cx.stop_propagation();
                                v_check.update(cx, |this, cx| {
                                    if this.state.main_view.selected_indices.contains(&orig_idx) {
                                        this.state.main_view.selected_indices.remove(&orig_idx);
                                    } else {
                                        this.state.main_view.selected_indices.insert(orig_idx);
                                    }
                                    cx.notify();
                                });
                            })
                            .child(
                                div()
                                    .w(px(16.0))
                                    .h(px(16.0))
                                    .rounded(px(3.5))
                                    .border(px(1.0))
                                    .border_color(if is_selected {
                                        rgba(UiConstants::ACCENT_RGBA)
                                    } else {
                                        rgba(0xFFFFFF44u32)
                                    })
                                    .bg(if is_selected {
                                        rgba(UiConstants::ACCENT_RGBA)
                                    } else {
                                        rgba(0x00000000u32)
                                    })
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(
                                        div()
                                            .text_size(px(10.0))
                                            .font_family(UiConstants::FONT_ICONS)
                                            .text_color(rgb(0xFFFFFF))
                                            .child(if is_selected { "\u{E73E}" } else { "" }),
                                    ),
                            ),
                    )
                    // Name + Icon
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(180.0))
                            .pr(px(12.0))
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(10.0))
                            .child(
                                div()
                                    .text_size(px(16.0))
                                    .font_family(UiConstants::FONT_ICONS)
                                    .text_color(if is_dir {
                                        rgb(0xFCCB45)
                                    } else {
                                        rgb(0x60CDFF)
                                    })
                                    .child(if is_dir { "\u{E8B7}" } else { "\u{E8A5}" }),
                            )
                            .child(
                                div()
                                    .text_size(px(13.0))
                                    .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                                    .overflow_hidden()
                                    .whitespace_nowrap()
                                    .text_ellipsis()
                                    .child(name),
                            ),
                    )
                    // Original Location
                    .child(
                        div()
                            .w(px(260.0))
                            .pr(px(12.0))
                            .text_size(px(12.0))
                            .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .child(orig_loc),
                    )
                    // Date Deleted
                    .child(
                        div()
                            .w(px(140.0))
                            .text_size(px(12.0))
                            .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                            .child(date_del),
                    )
                    // Size
                    .child(
                        div()
                            .w(px(90.0))
                            .pr(px(12.0))
                            .text_right()
                            .text_size(px(12.0))
                            .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                            .child(size_str),
                    )
                    // Type
                    .child(
                        div()
                            .w(px(110.0))
                            .text_size(px(12.0))
                            .text_color(rgba(UiConstants::TEXT_MUTED_RGBA))
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .child(type_str),
                    )
                    // Per-item Restore & Delete Permanently buttons
                    .child(
                        div()
                            .w(px(130.0))
                            .flex()
                            .flex_row()
                            .items_center()
                            .justify_end()
                            .gap(px(6.0))
                            .child(
                                div()
                                    .px(px(8.0))
                                    .py(px(3.5))
                                    .rounded(px(4.0))
                                    .bg(rgba(0x4CCB5E22u32))
                                    .hover(|s| s.bg(rgba(0x4CCB5E40u32)))
                                    .cursor_pointer()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap(px(4.0))
                                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                        cx.stop_propagation();
                                        v_restore.update(cx, |this, cx| {
                                            this.restore_recycle_bin_item(orig_idx, cx);
                                        });
                                    })
                                    .child(
                                        div()
                                            .text_size(px(11.0))
                                            .font_family(UiConstants::FONT_ICONS)
                                            .text_color(rgb(0x4CCB5E))
                                            .child("\u{E777}"),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(11.5))
                                            .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                                            .child("Restore"),
                                    ),
                            )
                            .child(
                                div()
                                    .w(px(26.0))
                                    .h(px(26.0))
                                    .rounded(px(4.0))
                                    .bg(rgba(0xE8112322u32))
                                    .hover(|s| s.bg(rgba(0xE8112344u32)))
                                    .cursor_pointer()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                        cx.stop_propagation();
                                        v_delete.update(cx, |this, cx| {
                                            this.delete_recycle_bin_item(orig_idx, cx);
                                        });
                                    })
                                    .child(
                                        div()
                                            .text_size(px(11.5))
                                            .font_family(UiConstants::FONT_ICONS)
                                            .text_color(rgb(0xFF8080))
                                            .child("\u{E74D}"),
                                    ),
                            ),
                    )
                    .into_any_element()
            })
            .collect();

        div()
            .id("recycle-bin-view")
            .flex()
            .flex_col()
            .flex_grow()
            .w_full()
            .h_full()
            .px(px(20.0))
            .py(px(16.0))
            .gap(px(12.0))
            .child(header_card)
            .child(
                div()
                    .flex_1()
                    .w_full()
                    .rounded(px(8.0))
                    .bg(rgba(0xFFFFFF06u32))
                    .border(px(1.0))
                    .border_color(rgba(UiConstants::CARD_STROKE_RGBA))
                    .overflow_hidden()
                    .flex()
                    .flex_col()
                    .child(table_header)
                    .child(
                        div()
                            .id("recycle-bin-table-scroll")
                            .flex_1()
                            .w_full()
                            .overflow_y_scroll()
                            .flex()
                            .flex_col()
                            .children(rows),
                    ),
            )
    }
}

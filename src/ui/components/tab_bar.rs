//! Tab Bar component — window chrome + tab management.
//! Per DESIGN_SPEC.md section 4.
//!
//! In GPUI 0.2.x, `on_mouse_down` attaches per-element (no bubbling).
//! Structure:
//! [TabActions btn][Tab1][Tab2][+btn]  [DRAG ZONE (flex-grow)]  [─][□][✕]
//!
//! Each tab has its own `on_mouse_down` handler. The drag zone catches
//! window drag events on the transparent area right of tabs.

use crate::core::navigation_manager::{NavigationState, TabManagerState};
use crate::core::path_util::FilesPath;
use crate::ui::constants::UiConstants;
use gpui::*;

/// Helper to start window dragging. On Windows, GPUI's start_window_move()
/// is a no-op, so we send WM_NCLBUTTONDOWN + HTCAPTION directly via Win32 API.
/// On other platforms, fall back to GPUI's method.
#[cfg(target_os = "windows")]
fn start_window_move(window: &Window) {
    use raw_window_handle::HasWindowHandle;
    use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
    use windows::Win32::UI::Input::KeyboardAndMouse::ReleaseCapture;
    use windows::Win32::UI::WindowsAndMessaging::{SendMessageW, HTCAPTION, WM_NCLBUTTONDOWN};
    if let Ok(raw_handle) = HasWindowHandle::window_handle(window) {
        if let raw_window_handle::RawWindowHandle::Win32(win32) = raw_handle.as_raw() {
            unsafe {
                let _ = ReleaseCapture();
                let _ = SendMessageW(
                    HWND(win32.hwnd.get()),
                    WM_NCLBUTTONDOWN,
                    WPARAM(HTCAPTION as usize),
                    LPARAM(0),
                );
            }
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn start_window_move(window: &Window) {
    window.start_window_move();
}

pub struct TabBarView;

impl TabBarView {
    pub fn build(state: &NavigationState, view: Entity<crate::app::AppWindow>) -> impl IntoElement {
        let tab_count = state.tabs.tabs.len();
        let active_idx = state.tabs.active_tab_index;
        let view_dual_pane = view.clone();
        let view_add_tab = view.clone();

        // ── Build the tab actions button (dual pane toggle)
        let tab_actions_btn = div()
            .flex()
            .items_center()
            .justify_center()
            .w(px(32.0))
            .h(px(32.0))
            .mx(px(4.0))
            .cursor_pointer()
            .rounded(px(4.0))
            .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
            .on_mouse_down(MouseButton::Left, move |_ev, _win, cx| {
                view_dual_pane.update(cx, |this, cx| {
                    this.toggle_dual_pane(cx);
                });
            })
            .child(
                div()
                    .text_size(px(14.0))
                    .font_family(UiConstants::FONT_ICONS)
                    .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                    .child("\u{E762}"),
            );

        // ── Build tabs row
        let tabs_closure = {
            let view_for_tabs = view.clone();
            let tc = tab_count;
            state.tabs.tabs.iter().enumerate().map(move |(i, tab)| {
                let is_active = i == active_idx;
                let path_name = tab
                    .path
                    .as_path()
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                let label = if path_name.is_empty() {
                    tab.path.to_string_lossy().to_string()
                } else {
                    path_name
                };

                let v_click = view_for_tabs.clone();
                let v_close = view_for_tabs.clone();

                let text_color = if is_active {
                    rgba(UiConstants::TEXT_PRIMARY_RGBA)
                } else {
                    rgba(UiConstants::TEXT_SECONDARY_RGBA)
                };

                // Active tab background matching Files community — slightly elevated card feel
                let bg = if is_active {
                    rgba(0xFFFFFF14u32) // ~8% white overlay on Mica = LayerFillColorDefault
                } else {
                    rgba(0x00000000u32) // Fully transparent when inactive
                };

                // Active tab gets a bottom accent indicator line (like Files-community)
                let border_b_color = if is_active {
                    rgba(UiConstants::ACCENT_RGBA)
                } else {
                    rgba(0x00000000u32)
                };

                let radius = px(8.0);
                let show_close = tc > 1;
                let close_opacity: f32 = if is_active { 1.0 } else { 0.0 };

                let mut tab_div = div()
                    .flex()
                    .items_center()
                    .h(px(32.0))
                    .px(px(10.0))
                    .min_w(px(120.0))
                    .max_w(px(240.0))
                    .bg(bg)
                    .rounded_t(px(6.0)) // rounded only on top for tab look
                    .border_b(px(2.0))
                    .border_color(border_b_color)
                    .cursor_pointer()
                    .on_mouse_down(MouseButton::Left, move |_ev, _win, cx| {
                        v_click.update(cx, |this, cx| {
                            this.switch_tab(i, cx);
                        });
                    })
                    .on_mouse_down(MouseButton::Middle, {
                        let v = v_close.clone();
                        move |_ev, _win, cx| {
                            if show_close {
                                v.update(cx, |this, cx| {
                                    this.close_tab(i, cx);
                                });
                            }
                        }
                    })
                    .on_mouse_down(MouseButton::Right, {
                        let v_ctx = view_for_tabs.clone();
                        move |ev, _win, cx| {
                            v_ctx.update(cx, |this, cx| {
                                this.show_tab_context_menu(i, ev.position, cx);
                            });
                        }
                    });

                // Tab Icon (Folder Glyph)
                tab_div = tab_div.child(
                    div()
                        .text_size(px(14.0))
                        .font_family(UiConstants::FONT_MDL2)
                        .text_color(if is_active {
                            rgba(UiConstants::ACCENT_RGBA)
                        } else {
                            rgba(UiConstants::TEXT_SECONDARY_RGBA)
                        })
                        .mr(px(8.0))
                        .child("\u{E8B7}"),
                );

                // Tab Label
                tab_div = tab_div.child(
                    div()
                        .flex_grow()
                        .text_size(px(12.0))
                        .text_color(text_color)
                        .line_height(px(16.0))
                        .overflow_hidden()
                        .child(label),
                );

                // Close button
                let v_c = v_close.clone();
                tab_div.child(
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .w(px(20.0))
                        .h(px(20.0))
                        .ml(px(4.0))
                        .rounded(px(4.0))
                        .opacity(close_opacity)
                        .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
                        .on_mouse_down(MouseButton::Left, {
                            let v = v_c.clone();
                            move |_ev, _win, cx| {
                                if show_close {
                                    v.update(cx, |this, cx| {
                                        this.close_tab(i, cx);
                                    });
                                }
                            }
                        })
                        .child(
                            div()
                                .text_size(px(10.0))
                                .font_family(UiConstants::FONT_ICONS)
                                .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                .child("\u{E8BB}"),
                        ),
                )
            })
        };

        // ── Add tab button
        let add_btn = div()
            .flex()
            .items_center()
            .justify_center()
            .w(px(32.0))
            .h(px(32.0))
            .ml(px(4.0))
            .cursor_pointer()
            .rounded(px(4.0))
            .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
            .on_mouse_down(MouseButton::Left, {
                let v = view_add_tab.clone();
                move |_ev, _win, cx| {
                    v.update(cx, |this, cx| {
                        this.new_tab(cx);
                    });
                }
            })
            .child(
                div()
                    .text_size(px(12.0))
                    .font_family(UiConstants::FONT_ICONS)
                    .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                    .child("\u{E710}"),
            );

        // ── DRAG ZONE
        let drag_zone = div().id("drag-zone").flex_grow().h_full().on_mouse_down(
            MouseButton::Left,
            move |event, window, _cx| {
                if event.click_count == 2 {
                    window.zoom_window();
                } else {
                    start_window_move(window);
                }
            },
        );

        // ── Caption buttons
        let caption_buttons = div()
            .flex()
            .items_center()
            .id("caption-buttons")
            .child({
                let btn = div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .w(px(46.0))
                    .h(px(32.0))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)));
                btn.on_mouse_down(MouseButton::Left, move |_ev, window, _cx| {
                    window.minimize_window();
                })
                .child(
                    div()
                        .text_size(px(10.0))
                        .font_family(UiConstants::FONT_ICONS)
                        .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                        .child("\u{E921}"),
                )
            })
            .child({
                let btn = div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .w(px(46.0))
                    .h(px(32.0))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)));
                btn.on_mouse_down(MouseButton::Left, move |_ev, window, _cx| {
                    window.zoom_window();
                })
                .child(
                    div()
                        .text_size(px(10.0))
                        .font_family(UiConstants::FONT_ICONS)
                        .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                        .child("\u{E922}"),
                )
            })
            .child({
                let btn = div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .w(px(46.0))
                    .h(px(32.0))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgba(0xE81123FFu32)));
                btn.on_mouse_down(MouseButton::Left, move |_ev, _win, cx| {
                    cx.quit();
                })
                .child(
                    div()
                        .text_size(px(10.0))
                        .font_family(UiConstants::FONT_ICONS)
                        .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                        .child("\u{E8BB}"),
                )
            });

        // ── Complete tab bar layout
        div()
            .id("tab-bar")
            .flex()
            .items_center()
            .h(px(40.0))
            .w_full()
            .bg(rgb(UiConstants::BG_MAIN_RGB))
            .border_b(px(1.0))
            .border_color(rgba(UiConstants::DIVIDER_RGBA))
            .child(tab_actions_btn)
            .child(
                div()
                    .id("tab-strip")
                    .flex()
                    .items_center()
                    .h_full()
                    .gap(px(4.0))
                    .children(tabs_closure)
                    .child(add_btn),
            )
            .child(drag_zone)
            .child(caption_buttons)
    }
}

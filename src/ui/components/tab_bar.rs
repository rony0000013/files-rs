//! Tab Bar — window chrome + tab management.
//!
//! Scroll: tabs slide by ml(offset) within a filtered window.

use crate::core::navigation_manager::NavigationState;
use crate::ui::constants::UiConstants;
use gpui::*;

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

const TAB_H: f32 = 32.0;
const TAB_W: f32 = 150.0;
const TAB_MIN_W: f32 = 100.0;
const TAB_MAX_W: f32 = 220.0;
const SCROLL_W: f32 = 24.0;
const GAP_W: f32 = 80.0;
const CAPTION_W: f32 = 46.0;

pub struct TabBarView;

fn handle_scroll(event: &ScrollWheelEvent, cx: &mut App, view: &Entity<crate::app::AppWindow>) {
    let dy = match event.delta {
        ScrollDelta::Pixels(p) => f32::from(p.y),
        ScrollDelta::Lines(l) => l.y,
    };
    if dy > 0.0 {
        view.update(cx, |this, cx| this.scroll_tab_bar_right(cx));
    } else if dy < 0.0 {
        view.update(cx, |this, cx| this.scroll_tab_bar_left(cx));
    }
}

impl TabBarView {
    pub fn build(
        state: &NavigationState,
        view: Entity<crate::app::AppWindow>,
        tab_thumbnail_cache: &std::sync::Arc<
            std::sync::Mutex<
                std::collections::HashMap<crate::core::path_util::FilesPath, std::path::PathBuf>,
            >,
        >,
        tab_drag: &crate::app_state::TabDragState,
        is_maximized: bool,
    ) -> impl IntoElement {
        let tab_count = state.tabs.tabs.len();
        let active_idx = state.tabs.active_tab_index;
        let dragged_index = tab_drag.dragged_index;
        let is_dragging = tab_drag.is_dragging;
        let scroll_offset = tab_drag.scroll_offset;

        // Scroll mode: active when user has scrolled, or when tabs at minimum width
        // would overflow the available strip area.
        // Available strip ≈ window_width - chrome (actions 36 + 2×scroll 48 + add 36 + gap 80 + caption 138 ≈ 338)
        // We estimate 800px as a conservative strip width for most window sizes.
        let tabs_min_total_w = (tab_count as f32) * TAB_MIN_W;
        let estimated_strip_w = 800.0;
        let tabs_overflow = scroll_offset > 0.0 || tabs_min_total_w > estimated_strip_w;
        let vref = view.clone();
        let drag_active = tab_drag.drag_active;

        // ── Caption buttons
        let caption = {
            let vm = vref.clone();
            let max_icon = if is_maximized { "\u{E923}" } else { "\u{E922}" };
            div()
                .flex()
                .items_center()
                .flex_shrink_0()
                .child({
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .w(px(CAPTION_W))
                        .h(px(TAB_H))
                        .cursor_pointer()
                        .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
                        .on_mouse_down(MouseButton::Left, move |_ev, w, _cx| {
                            w.minimize_window();
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
                    let vm = vm.clone();
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .w(px(CAPTION_W))
                        .h(px(TAB_H))
                        .cursor_pointer()
                        .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
                        .on_mouse_down(MouseButton::Left, move |_ev, w, cx| {
                            vm.update(cx, |this, cx| this.toggle_maximize(w, cx));
                        })
                        .child(
                            div()
                                .text_size(px(10.0))
                                .font_family(UiConstants::FONT_ICONS)
                                .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                                .child(max_icon),
                        )
                })
                .child({
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .w(px(CAPTION_W))
                        .h(px(TAB_H))
                        .cursor_pointer()
                        .hover(|s| s.bg(rgba(0xE81123FFu32)))
                        .on_mouse_down(MouseButton::Left, move |_ev, _win, cx| {
                            cx.quit();
                        })
                        .child(
                            div()
                                .text_size(px(10.0))
                                .font_family(UiConstants::FONT_ICONS)
                                .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                                .child("\u{E8BB}"),
                        )
                })
        };

        // ── Build all tabs
        let mut all_tabs: Vec<gpui::AnyElement> = Vec::new();
        for i in 0..tab_count {
            let tab = &state.tabs.tabs[i];
            let is_active = i == active_idx;
            let is_dragged = is_dragging && dragged_index == Some(i);
            let is_settings =
                matches!(tab.target, crate::ui::views::types::ViewTarget::Settings);
            let show_drop = is_dragging && !is_dragged && dragged_index.is_some();

            let label = if is_settings {
                "Settings".to_string()
            } else {
                let p = tab
                    .path
                    .as_path()
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                if p.is_empty() {
                    tab.path.to_string_lossy().to_string()
                } else {
                    p
                }
            };

            let vc = vref.clone();
            let vd = vref.clone();
            let vcc = vref.clone();
            let v_scroll = vref.clone();

            let tc = if is_active {
                rgba(UiConstants::TEXT_PRIMARY_RGBA)
            } else {
                rgba(UiConstants::TEXT_SECONDARY_RGBA)
            };
            let bg = if is_active {
                rgba(0xFFFFFF14u32)
            } else {
                rgba(0x00000000u32)
            };
            let bc = if is_active {
                rgba(UiConstants::ACCENT_RGBA)
            } else {
                rgba(0x00000000u32)
            };

            let mut t = div()
                .flex()
                .items_center()
                .h(px(TAB_H))
                .px(px(10.0))
                .min_w(px(TAB_MIN_W))
                .max_w(px(TAB_MAX_W))
                .w(px(TAB_W)) // Fixed width for smooth scroll
                .bg(bg)
                .rounded_t(px(6.0))
                .border_b(px(2.0))
                .border_color(bc)
                .cursor_pointer();

            if is_dragged {
                t = t.opacity(0.4);
            }
            if show_drop {
                t = t
                    .border_l(px(3.0))
                    .border_color(rgba(UiConstants::ACCENT_RGBA));
            }

            t = t
                .on_mouse_down(MouseButton::Left, {
                    let vd2 = vd.clone();
                    let vc2 = vc.clone();
                    move |ev, _w, cx| {
                        vd2.update(cx, |this, cx| this.start_tab_drag(i, cx));
                        vc2.update(cx, |this, cx| this.switch_tab(i, cx));
                    }
                })
                .on_mouse_move({
                    let vd2 = vd.clone();
                    move |ev, _w, cx| {
                        let p = ev.position;
                        vd2.update(cx, |this, cx| {
                            this.update_tab_drag(p.x.into(), p.y.into(), cx);
                        });
                    }
                })
                .on_mouse_up(MouseButton::Left, {
                    let vd2 = vd.clone();
                    move |_ev, _w, cx| {
                        vd2.update(cx, |this, cx| this.end_tab_drag(Some(i), cx));
                    }
                })
                .on_mouse_down(MouseButton::Middle, {
                    let v = vcc.clone();
                    move |_ev, _w, cx| {
                        v.update(cx, |this, cx| this.close_tab(i, cx));
                    }
                })
                .on_mouse_down(MouseButton::Right, {
                    let vr = vref.clone();
                    move |ev, _w, cx| {
                        vr.update(cx, |this, cx| {
                            this.show_tab_context_menu(i, ev.position, cx);
                        });
                    }
                })
                .on_scroll_wheel(move |event, _w, cx| {
                    if !tabs_overflow { return; }
                    handle_scroll(event, cx, &v_scroll);
                    cx.stop_propagation();
                });

            let icon_el: gpui::AnyElement = if is_settings {
                div()
                    .text_size(px(14.0))
                    .font_family(UiConstants::FONT_MDL2)
                    .text_color(rgba(UiConstants::ACCENT_RGBA))
                    .child("\u{E713}")
                    .into_any_element()
            } else {
                let thumb = tab_thumbnail_cache
                    .lock()
                    .ok()
                    .and_then(|c| c.get(&tab.path).cloned());
                if let Some(path) = thumb {
                    gpui::img(path)
                        .w(px(16.0))
                        .h(px(16.0))
                        .ml(px(2.0))
                        .mr(px(6.0))
                        .rounded(px(2.0))
                        .object_fit(gpui::ObjectFit::Cover)
                        .into_any_element()
                } else {
                    div()
                        .text_size(px(14.0))
                        .font_family(UiConstants::FONT_MDL2)
                        .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                        .mr(px(8.0))
                        .child("\u{E8B7}")
                        .into_any_element()
                }
            };
            t = t.child(icon_el);
            t = t.child(
                div()
                    .flex_grow()
                    .text_size(px(12.0))
                    .text_color(tc)
                    .line_height(px(16.0))
                    .overflow_hidden()
                    .child(label),
            );
            t = t.child(
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .w(px(20.0))
                    .h(px(20.0))
                    .ml(px(4.0))
                    .rounded(px(4.0))
                    .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
                    .on_mouse_down(MouseButton::Left, {
                        let v = vcc.clone();
                        move |_ev, _w, cx| {
                            v.update(cx, |this, cx| this.close_tab(i, cx));
                        }
                    })
                    .child(
                        div()
                            .text_size(px(10.0))
                            .font_family(UiConstants::FONT_ICONS)
                            .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                            .child("\u{E8BB}"),
                    ),
            );
            all_tabs.push(t.into_any_element());
        }

        // ── Buttons
        let actions_btn = {
            let v = vref.clone();
            div()
                .flex()
                .items_center()
                .justify_center()
                .flex_shrink_0()
                .w(px(36.0))
                .h(px(TAB_H))
                .ml(px(4.0))
                .cursor_pointer()
                .rounded(px(4.0))
                .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
                .on_mouse_down(MouseButton::Left, move |_ev, _w, cx| {
                    v.update(cx, |this, cx| this.toggle_tab_actions_menu(cx));
                })
                .child(
                    div()
                        .text_size(px(14.0))
                        .font_family(UiConstants::FONT_ICONS)
                        .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                        .child("\u{E762}"),
                )
        };
        let add_btn = {
            let v = vref.clone();
            div()
                .flex()
                .items_center()
                .justify_center()
                .flex_shrink_0()
                .w(px(36.0))
                .h(px(TAB_H))
                .ml(px(4.0))
                .cursor_pointer()
                .rounded(px(4.0))
                .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
                .on_mouse_down(MouseButton::Left, move |_ev, _w, cx| {
                    v.update(cx, |this, cx| this.new_tab(cx));
                })
                .child(
                    div()
                        .text_size(px(14.0))
                        .font_family(UiConstants::FONT_ICONS)
                        .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                        .child("\u{E710}"),
                )
        };
        let scroll_l = {
            let v = vref.clone();
            div()
                .flex()
                .items_center()
                .justify_center()
                .flex_shrink_0()
                .w(px(SCROLL_W))
                .h(px(TAB_H))
                .cursor_pointer()
                .rounded(px(4.0))
                .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
                .on_mouse_down(MouseButton::Left, move |_ev, _w, cx| {
                    v.update(cx, |this, cx| this.scroll_tab_bar_left(cx));
                })
                .child(
                    div()
                        .text_size(px(12.0))
                        .font_family(UiConstants::FONT_ICONS)
                        .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                        .child("\u{E76B}"),
                )
        };
        let scroll_r = {
            let v = vref.clone();
            div()
                .flex()
                .items_center()
                .justify_center()
                .flex_shrink_0()
                .w(px(SCROLL_W))
                .h(px(TAB_H))
                .cursor_pointer()
                .rounded(px(4.0))
                .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
                .on_mouse_down(MouseButton::Left, move |_ev, _w, cx| {
                    v.update(cx, |this, cx| this.scroll_tab_bar_right(cx));
                })
                .child(
                    div()
                        .text_size(px(12.0))
                        .font_family(UiConstants::FONT_ICONS)
                        .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                        .child("\u{E76C}"),
                )
        };
        let drag_gap = div()
            .flex_shrink_0()
            .w(px(GAP_W))
            .h(px(TAB_H))
            .on_mouse_down(MouseButton::Left, move |ev, window, _cx| {
                if ev.click_count == 2 {
                    window.zoom_window();
                } else {
                    start_window_move(window);
                }
            });
        // ── Tab bar with mouse wheel scroll (only when overflow)
        let bar = {
            let v_scroll = vref.clone();
            let overflow = tabs_overflow;
            div()
                .id("tab-bar")
                .flex()
                .flex_nowrap()
                .items_center()
                .h(px(40.0))
                .w_full()
                .bg(rgb(UiConstants::BG_MAIN_RGB))
                .border_b(px(1.0))
                .border_color(rgba(UiConstants::DIVIDER_RGBA))
                .on_scroll_wheel(move |event, _w, cx| {
                    if !overflow { return; }
                    handle_scroll(event, cx, &v_scroll);
                    cx.stop_propagation();
                })
        };

        if tabs_overflow {
            // Smooth scroll: render a window of tabs, slide by fractional pixel offset.
            // tab_idx shifts only when scroll_offset crosses a full tab width.
            // fractional gives the pixel-level slide within the current window.
            let tab_window = 8;
            let tab_idx = (scroll_offset / TAB_W).floor() as usize;
            let tab_idx = tab_idx.min(tab_count.saturating_sub(1));
            let fractional = scroll_offset - (tab_idx as f32 * TAB_W);
            let end = (tab_idx + tab_window).min(tab_count);

            let mut visible_tabs: Vec<gpui::AnyElement> = Vec::new();
            for i in tab_idx..end {
                let tab = &state.tabs.tabs[i];
                let is_active = i == active_idx;
                let is_dragged = drag_active && dragged_index == Some(i);
                let is_settings =
                    matches!(tab.target, crate::ui::views::types::ViewTarget::Settings);
                let show_drop = drag_active && !is_dragged && dragged_index.is_some();

                let label = if is_settings {
                    "Settings".to_string()
                } else {
                    let p = tab
                        .path
                        .as_path()
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();
                    if p.is_empty() {
                        tab.path.to_string_lossy().to_string()
                    } else {
                        p
                    }
                };

                let vc = vref.clone();
                let vd = vref.clone();
                let vcc = vref.clone();
                let v_scroll = vref.clone();

                let tc = if is_active {
                    rgba(UiConstants::TEXT_PRIMARY_RGBA)
                } else {
                    rgba(UiConstants::TEXT_SECONDARY_RGBA)
                };
                let bg = if is_active {
                    rgba(0xFFFFFF14u32)
                } else {
                    rgba(0x00000000u32)
                };
                let bc = if is_active {
                    rgba(UiConstants::ACCENT_RGBA)
                } else {
                    rgba(0x00000000u32)
                };

                let mut t = div()
                    .flex()
                    .items_center()
                    .h(px(TAB_H))
                    .px(px(10.0))
                    .min_w(px(TAB_MIN_W))
                    .max_w(px(TAB_MAX_W))
                    .w(px(TAB_W))
                    .bg(bg)
                    .rounded_t(px(6.0))
                    .border_b(px(2.0))
                    .border_color(bc)
                    .cursor_pointer();

                if is_dragged {
                    t = t.opacity(0.4);
                }
                if show_drop {
                    t = t
                        .border_l(px(3.0))
                        .border_color(rgba(UiConstants::ACCENT_RGBA));
                }

                t = t
                    .on_mouse_down(MouseButton::Left, {
                        let vd2 = vd.clone();
                        let vc2 = vc.clone();
                        move |ev, _w, cx| {
                            vd2.update(cx, |this, cx| this.start_tab_drag(i, cx));
                            vc2.update(cx, |this, cx| this.switch_tab(i, cx));
                        }
                    })
                    .on_mouse_move({
                        let vd2 = vd.clone();
                        move |ev, _w, cx| {
                            let p = ev.position;
                            vd2.update(cx, |this, cx| {
                                this.update_tab_drag(p.x.into(), p.y.into(), cx);
                            });
                        }
                    })
                    .on_mouse_up(MouseButton::Left, {
                        let vd2 = vd.clone();
                        move |_ev, _w, cx| {
                            vd2.update(cx, |this, cx| this.end_tab_drag(Some(i), cx));
                        }
                    })
                    .on_mouse_down(MouseButton::Middle, {
                        let v = vcc.clone();
                        move |_ev, _w, cx| {
                            v.update(cx, |this, cx| this.close_tab(i, cx));
                        }
                    })
                    .on_mouse_down(MouseButton::Right, {
                        let vr = vref.clone();
                        move |ev, _w, cx| {
                            vr.update(cx, |this, cx| {
                                this.show_tab_context_menu(i, ev.position, cx);
                            });
                        }
                    })
                    .on_scroll_wheel(move |event, _w, cx| {
                        handle_scroll(event, cx, &v_scroll);
                        cx.stop_propagation();
                    });

                let icon_el: gpui::AnyElement = if is_settings {
                    div()
                        .text_size(px(14.0))
                        .font_family(UiConstants::FONT_MDL2)
                        .text_color(rgba(UiConstants::ACCENT_RGBA))
                        .child("\u{E713}")
                        .into_any_element()
                } else {
                    let thumb = tab_thumbnail_cache
                        .lock()
                        .ok()
                        .and_then(|c| c.get(&tab.path).cloned());
                    if let Some(path) = thumb {
                        gpui::img(path)
                            .w(px(16.0))
                            .h(px(16.0))
                            .ml(px(2.0))
                            .mr(px(6.0))
                            .rounded(px(2.0))
                            .object_fit(gpui::ObjectFit::Cover)
                            .into_any_element()
                    } else {
                        div()
                            .text_size(px(14.0))
                            .font_family(UiConstants::FONT_MDL2)
                            .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                            .mr(px(8.0))
                            .child("\u{E8B7}")
                            .into_any_element()
                    }
                };
                t = t.child(icon_el);
                t = t.child(
                    div()
                        .flex_grow()
                        .text_size(px(12.0))
                        .text_color(tc)
                        .line_height(px(16.0))
                        .overflow_hidden()
                        .child(label),
                );
                t = t.child(
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .w(px(20.0))
                        .h(px(20.0))
                        .ml(px(4.0))
                        .rounded(px(4.0))
                        .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
                        .on_mouse_down(MouseButton::Left, {
                            let v = vcc.clone();
                            move |_ev, _w, cx| {
                                v.update(cx, |this, cx| this.close_tab(i, cx));
                            }
                        })
                        .child(
                            div()
                                .text_size(px(10.0))
                                .font_family(UiConstants::FONT_ICONS)
                                .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                .child("\u{E8BB}"),
                        ),
                );
                visible_tabs.push(t.into_any_element());
            }

            bar.child(actions_btn)
                .child(scroll_l)
                .child(
                    div()
                        .id("tab-strip")
                        .flex()
                        .flex_grow()
                        .h_full()
                        .overflow_hidden()
                        .child(
                            div()
                                .flex()
                                .flex_nowrap()
                                .items_center()
                                .h_full()
                                .ml(px(-fractional))
                                .children(visible_tabs),
                        ),
                )
                .child(scroll_r)
                .child(add_btn)
                .child(drag_gap)
                .child(caption)
        } else {
            // NO SCROLL: tabs take natural width, +btn right after last tab,
            // drag zone fills remaining space pushing caption buttons to right
            bar.child(actions_btn)
                .child(
                    div()
                        .id("tab-strip")
                        .flex()
                        .flex_nowrap()
                        .items_center()
                        .h_full()
                        .children(all_tabs),
                )
                .child(add_btn)
                .child(
                    div()
                        .flex_grow() // fills remaining space for window dragging
                        .h(px(TAB_H))
                        .on_mouse_down(MouseButton::Left, move |ev, window, _cx| {
                            if ev.click_count == 2 {
                                window.zoom_window();
                            } else {
                                start_window_move(window);
                            }
                        }),
                )
                .child(caption)
        }
    }
}

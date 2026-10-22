//! Tab Bar — window chrome + Files-community style dynamic tab strip.
//!
//! Features:
//! - Dynamic tab width scaling (`TAB_MAX_W` = 200px down to `TAB_MIN_W` = 96px) based on live window width.
//! - Automatic overflow detection into smooth scrollable strip mode when `tab_count * TAB_MIN_W > avail_strip_w`.
//! - Guaranteed visibility of right-hand window caption buttons (Minimize, Maximize/Restore, Close),
//!   `+` New Tab button, and Split/Actions button at all window widths (even tiny widths).
//! - Real-time live tab reordering during drag, edge auto-scroll, and drag-out-of-strip to detach into a new window.
//! - Titlebar drag via client-area click + asynchronous `WM_SYSCOMMAND(SC_MOVE | HTCAPTION)` (double-click toggles maximize).

use crate::core::navigation_manager::NavigationState;
use crate::ui::constants::UiConstants;
use gpui::*;

/// Handle titlebar drag or double-click maximize/restore on the caption drag zones.
#[cfg(target_os = "windows")]
fn handle_titlebar_drag_mouse_down(
    click_count: usize,
    window: &mut Window,
    cx: &mut App,
    view: &Entity<crate::app::AppWindow>,
) {
    use raw_window_handle::HasWindowHandle;
    use windows::Win32::Foundation::{HWND, LPARAM, POINT, WPARAM};
    use windows::Win32::UI::Input::KeyboardAndMouse::ReleaseCapture;
    use windows::Win32::UI::WindowsAndMessaging::{
        GetCursorPos, PostMessageW, HTCAPTION, SC_MOVE, WM_SYSCOMMAND,
    };
    if let Ok(raw_handle) = HasWindowHandle::window_handle(window) {
        if let raw_window_handle::RawWindowHandle::Win32(win32) = raw_handle.as_raw() {
            let hwnd = HWND(win32.hwnd.get() as *mut _);
            unsafe {
                let _ = ReleaseCapture();
                if click_count == 2 {
                    view.update(cx, |this, cx| this.toggle_maximize(window, cx));
                } else {
                    let mut pt = POINT::default();
                    let _ = GetCursorPos(&mut pt);
                    let lparam =
                        (((pt.y as usize) & 0xFFFF) << 16) | ((pt.x as usize) & 0xFFFF);
                    let _ = PostMessageW(
                        Some(hwnd),
                        WM_SYSCOMMAND,
                        WPARAM((SC_MOVE as usize) | (HTCAPTION as usize)),
                        LPARAM(lparam as isize),
                    );
                }
            }
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn handle_titlebar_drag_mouse_down(
    click_count: usize,
    window: &mut Window,
    cx: &mut App,
    view: &Entity<crate::app::AppWindow>,
) {
    if click_count == 2 {
        view.update(cx, |this, cx| this.toggle_maximize(window, cx));
    } else {
        window.start_window_move();
    }
}

pub const TAB_H: f32 = 32.0;
pub const TAB_W: f32 = 160.0;
pub const TAB_SCROLL_W: f32 = 120.0;
pub const TAB_MIN_W: f32 = 96.0;
pub const TAB_MAX_W: f32 = 200.0;
pub const PINNED_TAB_W: f32 = 44.0;
pub const SCROLL_W: f32 = 26.0;
pub const MIN_DRAG_GAP_W: f32 = 44.0;
pub const CAPTION_W: f32 = 46.0;
pub const ACTIONS_BTN_W: f32 = 40.0;
pub const ADD_BTN_W: f32 = 36.0;

/// Computed layout metrics for the tab strip at a given window width and tab count.
#[derive(Debug, Clone, Copy)]
pub struct TabStripMetrics {
    /// Whether the tabs exceed available width at `TAB_MIN_W` and require scrolling.
    pub tabs_overflow: bool,
    /// Width of each regular (unpinned) tab in pixels.
    pub tab_w: f32,
    /// Width of the visible tab strip viewport in pixels.
    pub strip_viewport_w: f32,
    /// Total scrollable width of all tabs combined in pixels.
    pub total_tabs_w: f32,
    /// Maximum scroll offset (`(total_tabs_w - strip_viewport_w).max(0.0)`).
    pub max_scroll: f32,
    /// X coordinate where the tab strip starts in window coordinates.
    pub strip_start_x: f32,
}

/// Compute exact tab strip layout metrics from the live window width and tab list.
pub fn compute_strip_metrics(window_width: f32, tab_count: usize, pinned_count: usize) -> TabStripMetrics {
    let win_w = if window_width > 100.0 { window_width } else { 1280.0 };
    let caption_total_w = CAPTION_W * 3.0; // 138px (Min, Max, Close)
    let base_chrome_w = ACTIONS_BTN_W + ADD_BTN_W + MIN_DRAG_GAP_W + caption_total_w; // 258px
    let avail_no_scroll = (win_w - base_chrome_w).max(48.0);

    let unpinned_count = tab_count.saturating_sub(pinned_count);
    let pinned_total_w = pinned_count as f32 * PINNED_TAB_W;
    let min_needed_w = pinned_total_w + unpinned_count as f32 * TAB_MIN_W;

    let tabs_overflow = tab_count > 1 && min_needed_w > avail_no_scroll;

    if !tabs_overflow {
        let avail_for_unpinned = (avail_no_scroll - pinned_total_w).max(40.0);
        let tab_w = if unpinned_count > 0 {
            (avail_for_unpinned / unpinned_count as f32)
                .floor()
                .clamp(40.0, TAB_MAX_W)
        } else {
            TAB_MAX_W
        };
        let total_tabs_w = pinned_total_w + unpinned_count as f32 * tab_w;
        let strip_viewport_w = total_tabs_w.min(avail_no_scroll);
        TabStripMetrics {
            tabs_overflow: false,
            tab_w,
            strip_viewport_w,
            total_tabs_w,
            max_scroll: 0.0,
            strip_start_x: ACTIONS_BTN_W,
        }
    } else {
        let scroll_chrome_w = base_chrome_w + SCROLL_W * 2.0; // +52px for left/right scroll buttons
        let strip_viewport_w = (win_w - scroll_chrome_w).max(48.0);
        let tab_w = TAB_SCROLL_W.min(strip_viewport_w);
        let total_tabs_w = pinned_total_w + unpinned_count as f32 * tab_w;
        let max_scroll = (total_tabs_w - strip_viewport_w).max(0.0);
        TabStripMetrics {
            tabs_overflow: true,
            tab_w,
            strip_viewport_w,
            total_tabs_w,
            max_scroll,
            strip_start_x: ACTIONS_BTN_W + SCROLL_W,
        }
    }
}

pub struct TabBarView;

fn handle_scroll(event: &ScrollWheelEvent, cx: &mut App, view: &Entity<crate::app::AppWindow>) {
    let dy = match event.delta {
        ScrollDelta::Pixels(p) => {
            let py: f32 = p.y.into();
            let px_val: f32 = p.x.into();
            if py.abs() >= px_val.abs() { py } else { px_val }
        }
        ScrollDelta::Lines(l) => {
            if l.y.abs() >= l.x.abs() { l.y * 24.0 } else { l.x * 24.0 }
        }
    };
    if dy < -0.1 {
        view.update(cx, |this, cx| this.scroll_tab_bar_right(cx));
    } else if dy > 0.1 {
        view.update(cx, |this, cx| this.scroll_tab_bar_left(cx));
    }
}

/// Floating drag ghost + tooltip when dragging a tab out of the strip to create a new window.
fn drag_overlay(label: String, x: f32, y: f32, out: bool, tab_w: f32) -> Vec<gpui::AnyElement> {
    if !out {
        return vec![];
    }
    let w = tab_w.clamp(110.0, 180.0);
    let ghost_left = (x - w / 2.0).max(4.0);
    let ghost_top = y.max(TAB_H + 4.0);
    let ghost = div()
        .absolute()
        .left(px(ghost_left))
        .top(px(ghost_top))
        .w(px(w))
        .h(px(TAB_H))
        .flex()
        .items_center()
        .px(px(10.0))
        .bg(rgba(0x2D2D2DFFu32))
        .border(px(1.5))
        .border_color(rgba(UiConstants::ACCENT_RGBA))
        .rounded(px(6.0))
        .shadow_lg()
        .child(
            div()
                .text_size(px(12.0))
                .font_weight(FontWeight(600.0))
                .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                .overflow_hidden()
                .child(label),
        )
        .into_any_element();
    let tip = div()
        .absolute()
        .left(px(ghost_left))
        .top(px(ghost_top + TAB_H + 6.0))
        .px(px(8.0))
        .py(px(4.0))
        .bg(rgba(0x1F1F1FFFu32))
        .border(px(1.0))
        .border_color(rgba(UiConstants::ACCENT_RGBA))
        .rounded(px(4.0))
        .text_size(px(11.0))
        .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
        .child("Release to open in new window")
        .into_any_element();
    vec![ghost, tip]
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
        window_width: f32,
    ) -> impl IntoElement {
        let tab_count = state.tabs.tabs.len();
        let pinned_count = state.tabs.tabs.iter().filter(|t| t.pinned).count();
        let active_idx = state.tabs.active_tab_index;
        let dragged_index = tab_drag.dragged_index;
        let drag_active = tab_drag.drag_active;
        let drop_target_index = tab_drag.drop_target_index;

        let metrics = compute_strip_metrics(window_width, tab_count, pinned_count);
        let tabs_overflow = metrics.tabs_overflow;
        let clamped_scroll = if tabs_overflow {
            tab_drag.scroll_offset.clamp(0.0, metrics.max_scroll)
        } else {
            0.0
        };
        let can_scroll_left = tabs_overflow && clamped_scroll > 0.5;
        let can_scroll_right = tabs_overflow && clamped_scroll < metrics.max_scroll - 0.5;

        let vref = view.clone();

        // ── Rightmost Caption buttons (always flex_shrink_0 so they never vanish on small windows)
        let caption = {
            let vm = vref.clone();
            let max_icon = if is_maximized { "\u{E923}" } else { "\u{E922}" };
            div()
                .id("window-caption-buttons")
                .flex()
                .items_center()
                .flex_shrink_0()
                .w(px(CAPTION_W * 3.0))
                .h_full()
                .child({
                    div()
                        .id("caption-minimize")
                        .flex()
                        .items_center()
                        .justify_center()
                        .flex_shrink_0()
                        .w(px(CAPTION_W))
                        .h_full()
                        .cursor_pointer()
                        .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
                        .on_mouse_down(MouseButton::Left, move |_ev, w, cx| {
                            cx.stop_propagation();
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
                        .id("caption-maximize")
                        .flex()
                        .items_center()
                        .justify_center()
                        .flex_shrink_0()
                        .w(px(CAPTION_W))
                        .h_full()
                        .cursor_pointer()
                        .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
                        .on_mouse_down(MouseButton::Left, move |_ev, w, cx| {
                            cx.stop_propagation();
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
                        .id("caption-close")
                        .flex()
                        .items_center()
                        .justify_center()
                        .flex_shrink_0()
                        .w(px(CAPTION_W))
                        .h_full()
                        .cursor_pointer()
                        .hover(|s| s.bg(rgba(0xE81123FFu32)))
                        .on_mouse_down(MouseButton::Left, move |_ev, _win, cx| {
                            cx.stop_propagation();
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

        // ── Build all tab elements
        let mut tab_elements: Vec<gpui::AnyElement> = Vec::with_capacity(tab_count);
        for i in 0..tab_count {
            let tab = &state.tabs.tabs[i];
            let is_active = i == active_idx;
            let is_dragged = drag_active && dragged_index == Some(i);
            let is_drop_target = drag_active
                && !tab_drag.is_dragging_out
                && drop_target_index == Some(i)
                && dragged_index != Some(i);
            let is_settings = matches!(tab.target, crate::ui::views::types::ViewTarget::Settings);
            let is_home = matches!(tab.target, crate::ui::views::types::ViewTarget::Home)
                || tab.path.to_string_lossy() == "Home";
            let is_pinned = tab.pinned;

            let label = if is_settings {
                "Settings".to_string()
            } else if is_home {
                "Home".to_string()
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
                rgba(0x2B2B2BFFu32)
            } else {
                rgba(0x00000000u32)
            };

            let item_w = if is_pinned {
                PINNED_TAB_W
            } else {
                metrics.tab_w
            };

            let mut t = div()
                .id(ElementId::from(90000 + i))
                .relative()
                .flex()
                .flex_shrink_0()
                .items_center()
                .h(px(TAB_H))
                .px(px(if is_pinned { 8.0 } else { 10.0 }))
                .w(px(item_w))
                .bg(bg)
                .rounded_t(px(8.0))
                .border(px(1.0))
                .border_color(if is_dragged {
                    rgba(UiConstants::ACCENT_RGBA)
                } else if is_active {
                    rgba(0xFFFFFF1Eu32)
                } else {
                    rgba(0xFFFFFF0Cu32)
                })
                .border_b(px(0.0))
                .cursor_pointer();

            if is_dragged {
                t = t.bg(rgba(0x323232FFu32));
            }
            if is_drop_target {
                t = t
                    .border_l(px(3.0))
                    .border_color(rgba(UiConstants::ACCENT_RGBA));
            }

            if !is_active {
                t = t.hover(|s| s.bg(rgba(0xFFFFFF0Eu32)));
            }

            t = t
                .on_mouse_down(MouseButton::Left, {
                    let vd2 = vd.clone();
                    let vc2 = vc.clone();
                    move |ev, _w, cx| {
                        cx.stop_propagation();
                        let pos_x: f32 = ev.position.x.into();
                        let pos_y: f32 = ev.position.y.into();
                        vd2.update(cx, |this, cx| {
                            this.start_tab_drag_at(i, pos_x, pos_y, cx);
                        });
                        vc2.update(cx, |this, cx| this.switch_tab(i, cx));
                    }
                })
                .on_mouse_move({
                    let vd2 = vd.clone();
                    move |ev, win, cx| {
                        let p = ev.position;
                        let win_w: f32 = win.bounds().size.width.into();
                        vd2.update(cx, |this, cx| {
                            this.update_tab_drag_with_width(p.x.into(), p.y.into(), win_w, cx);
                        });
                    }
                })
                .on_mouse_up(MouseButton::Left, {
                    let vd2 = vd.clone();
                    move |_ev, _w, cx| {
                        vd2.update(cx, |this, cx| this.end_tab_drag(Some(i), cx));
                    }
                })
                .on_mouse_up_out(MouseButton::Left, {
                    let vd2 = vd.clone();
                    move |_ev, _w, cx| {
                        vd2.update(cx, |this, cx| this.end_tab_drag(None, cx));
                    }
                })
                .on_mouse_down(MouseButton::Middle, {
                    let v = vcc.clone();
                    move |_ev, _w, cx| {
                        cx.stop_propagation();
                        v.update(cx, |this, cx| this.close_tab(i, cx));
                    }
                })
                .on_mouse_down(MouseButton::Right, {
                    let vr = vref.clone();
                    move |ev, _w, cx| {
                        cx.stop_propagation();
                        vr.update(cx, |this, cx| {
                            this.show_tab_context_menu(i, ev.position, cx);
                        });
                    }
                })
                .on_scroll_wheel(move |event, _w, cx| {
                    cx.stop_propagation();
                    handle_scroll(event, cx, &v_scroll);
                });

            // Bottom active indicator pill (Files-community WinUI 3 TabView style)
            if is_active {
                t = t.child(
                    div()
                        .absolute()
                        .bottom(px(0.0))
                        .left(px(8.0))
                        .right(px(8.0))
                        .h(px(2.0))
                        .rounded(px(1.0))
                        .bg(rgb(UiConstants::ACCENT_COLOR_RGB)),
                );
            }

            let icon_el: gpui::AnyElement = if is_settings {
                div()
                    .text_size(px(14.0))
                    .font_family(UiConstants::FONT_MDL2)
                    .text_color(rgba(UiConstants::ACCENT_RGBA))
                    .mr(px(if is_pinned { 0.0 } else { 6.0 }))
                    .child("\u{E713}")
                    .into_any_element()
            } else if is_home {
                div()
                    .text_size(px(14.0))
                    .font_family(UiConstants::FONT_MDL2)
                    .text_color(rgba(UiConstants::ACCENT_RGBA))
                    .mr(px(if is_pinned { 0.0 } else { 6.0 }))
                    .child("\u{E10F}")
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
                        .mr(px(if is_pinned { 0.0 } else { 6.0 }))
                        .rounded(px(2.0))
                        .object_fit(gpui::ObjectFit::Cover)
                        .into_any_element()
                } else {
                    div()
                        .text_size(px(14.0))
                        .font_family(UiConstants::FONT_MDL2)
                        .text_color(rgb(0xF5B942u32))
                        .mr(px(if is_pinned { 0.0 } else { 6.0 }))
                        .child("\u{E8B7}")
                        .into_any_element()
                }
            };
            t = t.child(icon_el);

            if !is_pinned {
                t = t.child(
                    div()
                        .flex_1()
                        .min_w(px(0.0))
                        .text_size(px(12.0))
                        .font_weight(if is_active {
                            FontWeight(600.0)
                        } else {
                            FontWeight(400.0)
                        })
                        .text_color(tc)
                        .line_height(px(16.0))
                        .overflow_hidden()
                        .child(label),
                );
                // Show close button when tab is wide enough (> 68px) or active
                if item_w >= 68.0 || is_active {
                    t = t.child(
                        div()
                            .flex()
                            .flex_shrink_0()
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
                                    cx.stop_propagation();
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
                }
            } else {
                // Pinned badge indicator
                t = t.child(
                    div()
                        .text_size(px(9.0))
                        .font_family(UiConstants::FONT_ICONS)
                        .text_color(rgba(UiConstants::ACCENT_RGBA))
                        .ml(px(2.0))
                        .child("\u{E718}"),
                );
            }

            tab_elements.push(t.into_any_element());
        }

        // ── Left Split/Actions button (`flex_shrink_0`)
        let actions_btn = {
            let v = vref.clone();
            div()
                .id("tab-actions-btn")
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
                    cx.stop_propagation();
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

        // ── New Tab "+" button (`flex_shrink_0`)
        let add_btn = {
            let v = vref.clone();
            div()
                .id("new-tab-btn")
                .flex()
                .items_center()
                .justify_center()
                .flex_shrink_0()
                .w(px(32.0))
                .h(px(TAB_H))
                .ml(px(4.0))
                .cursor_pointer()
                .rounded(px(4.0))
                .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
                .on_mouse_down(MouseButton::Left, move |_ev, _w, cx| {
                    cx.stop_propagation();
                    v.update(cx, |this, cx| this.new_tab(cx));
                })
                .child(
                    div()
                        .text_size(px(13.0))
                        .font_family(UiConstants::FONT_ICONS)
                        .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                        .child("\u{E710}"),
                )
        };

        let scroll_l = {
            let v = vref.clone();
            let mut btn = div()
                .id("tab-scroll-left")
                .flex()
                .items_center()
                .justify_center()
                .flex_shrink_0()
                .w(px(SCROLL_W))
                .h(px(TAB_H))
                .rounded(px(4.0))
                .child(
                    div()
                        .text_size(px(11.0))
                        .font_family(UiConstants::FONT_ICONS)
                        .text_color(if can_scroll_left {
                            rgba(UiConstants::TEXT_PRIMARY_RGBA)
                        } else {
                            rgba(0xFFFFFF38u32)
                        })
                        .child("\u{E76B}"),
                );
            if can_scroll_left {
                btn = btn
                    .cursor_pointer()
                    .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
                    .on_mouse_down(MouseButton::Left, move |_ev, _w, cx| {
                        cx.stop_propagation();
                        v.update(cx, |this, cx| this.scroll_tab_bar_left(cx));
                    });
            }
            btn
        };

        let scroll_r = {
            let v = vref.clone();
            let mut btn = div()
                .id("tab-scroll-right")
                .flex()
                .items_center()
                .justify_center()
                .flex_shrink_0()
                .w(px(SCROLL_W))
                .h(px(TAB_H))
                .rounded(px(4.0))
                .child(
                    div()
                        .text_size(px(11.0))
                        .font_family(UiConstants::FONT_ICONS)
                        .text_color(if can_scroll_right {
                            rgba(UiConstants::TEXT_PRIMARY_RGBA)
                        } else {
                            rgba(0xFFFFFF38u32)
                        })
                        .child("\u{E76C}"),
                );
            if can_scroll_right {
                btn = btn
                    .cursor_pointer()
                    .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
                    .on_mouse_down(MouseButton::Left, move |_ev, _w, cx| {
                        cx.stop_propagation();
                        v.update(cx, |this, cx| this.scroll_tab_bar_right(cx));
                    });
            }
            btn
        };

        let make_overlay = || -> Vec<gpui::AnyElement> {
            if !drag_active {
                return vec![];
            }
            let Some(di) = dragged_index else {
                return vec![];
            };
            let Some(tab) = state.tabs.tabs.get(di) else {
                return vec![];
            };
            let label = if matches!(tab.target, crate::ui::views::types::ViewTarget::Settings) {
                "Settings".to_string()
            } else {
                tab.path
                    .as_path()
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string()
            };
            drag_overlay(
                label,
                tab_drag.current_x,
                tab_drag.current_y,
                tab_drag.is_dragging_out,
                metrics.tab_w,
            )
        };

        // ── Outer Tab Bar container (tracks tab drag globally across the bar + wheel scroll)
        let bar = {
            let v_scroll = vref.clone();
            let v_move = vref.clone();
            let v_up = vref.clone();
            div()
                .id("tab-bar")
                .relative()
                .flex()
                .flex_nowrap()
                .items_center()
                .h(px(40.0))
                .w_full()
                .overflow_hidden()
                .bg(rgba(UiConstants::BG_MICA_CHROME_RGBA))
                .border_b(px(1.0))
                .border_color(rgba(UiConstants::DIVIDER_RGBA))
                .on_mouse_move(move |ev, win, cx| {
                    let p = ev.position;
                    let win_w: f32 = win.bounds().size.width.into();
                    v_move.update(cx, |this, cx| {
                        if this.state.tab_drag.is_dragging {
                            this.update_tab_drag_with_width(p.x.into(), p.y.into(), win_w, cx);
                        }
                    });
                })
                .on_mouse_up(MouseButton::Left, move |_ev, _w, cx| {
                    v_up.update(cx, |this, cx| {
                        if this.state.tab_drag.is_dragging {
                            this.end_tab_drag(None, cx);
                        }
                    });
                })
                .on_scroll_wheel(move |event, _w, cx| {
                    cx.stop_propagation();
                    handle_scroll(event, cx, &v_scroll);
                })
        };

        let v_drag = vref.clone();
        let drag_spacer = div()
            .id("titlebar-drag-spacer")
            .flex_grow()
            .flex_shrink_0()
            .min_w(px(MIN_DRAG_GAP_W))
            .h_full()
            .on_mouse_down(MouseButton::Left, move |ev, window, cx| {
                handle_titlebar_drag_mouse_down(ev.click_count, window, cx, &v_drag);
            });

        if tabs_overflow {
            bar.child(actions_btn)
                .child(scroll_l)
                .child(
                    div()
                        .id("tab-strip")
                        .flex()
                        .flex_shrink()
                        .w(px(metrics.strip_viewport_w))
                        .min_w(px(0.0))
                        .h_full()
                        .overflow_hidden()
                        .child(
                            div()
                                .flex()
                                .flex_nowrap()
                                .items_center()
                                .h_full()
                                .gap(px(2.0))
                                .ml(px(-clamped_scroll))
                                .children(tab_elements),
                        ),
                )
                .child(scroll_r)
                .child(add_btn)
                .child(drag_spacer)
                .children(make_overlay())
                .child(caption)
        } else {
            bar.child(actions_btn)
                .child(
                    div()
                        .id("tab-strip")
                        .flex()
                        .flex_nowrap()
                        .flex_shrink()
                        .min_w(px(0.0))
                        .max_w(px(metrics.strip_viewport_w + (tab_count as f32 * 2.0)))
                        .items_center()
                        .h_full()
                        .gap(px(2.0))
                        .overflow_hidden()
                        .children(tab_elements),
                )
                .child(add_btn)
                .child(drag_spacer)
                .children(make_overlay())
                .child(caption)
        }
    }
}

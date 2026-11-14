//! Tab Bar — Window chrome + Files-community (WinUI 3 `TabView`) multitasking tab strip.
//!
//! Complete Files-community feature set:
//! - Dynamic tab width scaling (`TAB_MAX_W` = 210px down to `TAB_MIN_W` = 96px) based on live window width.
//! - Smooth 60–240Hz spring/lerp scrollable tab strip when tabs overflow, with `.flex_shrink_0().w(total_tabs_w)`
//!   track sizing so Taffy never squashes overflowing tabs, left/right chevron buttons with off-screen tab
//!   counter badges, edge fade indicators, mouse-wheel scrolling, and wheel-scroll-to-switch when not overflowing.
//! - Real-time animated tab drag-and-drop:
//!   - Live floating dragged tab pill tracking exact `current_x - grab_offset_x` inside the strip.
//!   - Smooth neighbor slot-shift animations (`slot_shift_offsets`) when tabs swap positions.
//!   - Dragging a tab outside the strip (`is_dragging_out`) shows a root-level floating window preview card
//!     and spawns a new window at the cursor's screen position on release.
//!   - Cross-window tab docking: dragging a tab over another open `files-rs` window's tab bar highlights
//!     the insertion slot in the target window and merges the tab (with full history & state) into that window.
//! - File/folder drag-and-drop onto tabs (500ms hover-to-switch tab + direct drop to copy/move into tab's folder)
//!   and onto the `+` New Tab button (drops folder(s) to open them in new tabs).
//! - Rich Tab Hover Preview Card (folder icon/thumbnail, title, full path, view mode, and pin/split badges).
//! - Guaranteed visibility of right-hand window caption buttons (`Minimize`, `Maximize/Restore`, `Close`)
//!   at all window sizes, and multi-window-safe window close (`window.remove_window()` when other windows exist).

use crate::core::navigation_manager::NavigationState;
use crate::ui::constants::UiConstants;
use gpui::*;

/// Handle titlebar drag or double-click maximize/restore on the caption drag zones.
///
/// The zones stay plain client areas (no `window_control_area(Drag)`): with an
/// `HTCAPTION` hit-test the OS move loop never starts here, because gpui calls
/// `prevent_default()` on every in-window click. The click arrives as client
/// `WM_LBUTTONDOWN` and the move starts **asynchronously** via
/// `PostMessageW(WM_SYSCOMMAND, SC_MOVE | HTCAPTION)` (double-click maximizes).
/// Never synchronous `SendMessageW` (it starves `bounds_changed`).
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

pub const TAB_BAR_H: f32 = 40.0;
pub const TAB_H: f32 = 32.0;
pub const TAB_W: f32 = 165.0;
pub const TAB_SCROLL_W: f32 = 132.0;
pub const TAB_MIN_W: f32 = 96.0;
pub const TAB_MAX_W: f32 = 210.0;
pub const PINNED_TAB_W: f32 = 42.0;
pub const SCROLL_W: f32 = 28.0;
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
pub fn compute_strip_metrics(
    window_width: f32,
    tab_count: usize,
    pinned_count: usize,
) -> TabStripMetrics {
    let win_w = if window_width > 100.0 {
        window_width
    } else {
        1280.0
    };
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
        let scroll_chrome_w = base_chrome_w + SCROLL_W * 2.0; // +56px for left/right scroll buttons
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

/// Compute the left X offset (relative to the start of the tab track) and width of tab `index`.
#[inline]
pub fn tab_slot_x_and_width(index: usize, pinned_count: usize, tab_w: f32) -> (f32, f32) {
    if index < pinned_count {
        (index as f32 * PINNED_TAB_W, PINNED_TAB_W)
    } else {
        let x = (pinned_count as f32 * PINNED_TAB_W) + ((index - pinned_count) as f32 * tab_w);
        (x, tab_w)
    }
}

/// Extract display label for a `TabState`.
pub fn tab_display_label(tab: &crate::core::navigation_manager::TabState) -> String {
    let path_str = tab.path.to_string_lossy();
    if matches!(tab.target, crate::ui::views::types::ViewTarget::Settings) {
        "Settings".to_string()
    } else if matches!(tab.target, crate::ui::views::types::ViewTarget::Home)
        || path_str == "Home"
        || (!matches!(tab.target, crate::ui::views::types::ViewTarget::RecycleBin)
            && path_str.is_empty())
    {
        "Home".to_string()
    } else if matches!(tab.target, crate::ui::views::types::ViewTarget::RecycleBin)
        || path_str.contains("RecycleBin")
    {
        "Recycle Bin".to_string()
    } else {
        let p = tab
            .path
            .as_path()
            .file_name()
            .unwrap_or_default()
            .to_string_lossy();
        if p.is_empty() {
            crate::ui::components::sidebar_state::format_drive_root_label(tab.path.as_path())
                .unwrap_or_else(|| path_str.into_owned())
        } else {
            p.into_owned()
        }
    }
}

pub struct TabBarView;

fn handle_scroll(
    event: &ScrollWheelEvent,
    tabs_overflow: bool,
    cx: &mut App,
    view: &Entity<crate::app::AppWindow>,
) {
    let raw_delta = match event.delta {
        ScrollDelta::Pixels(p) => {
            let py: f32 = p.y.into();
            let px_val: f32 = p.x.into();
            if py.abs() >= px_val.abs() {
                py
            } else {
                px_val
            }
        }
        ScrollDelta::Lines(l) => {
            if l.y.abs() >= l.x.abs() {
                l.y * 36.0
            } else {
                l.x * 36.0
            }
        }
    };

    // Ctrl+Wheel or Shift+Wheel always switches active tab (and auto-scrolls it into view).
    // When tabs do NOT overflow, regular wheel scrolling over the tab bar also switches tabs
    // (Files-community `TabView_PointerWheelChanged` parity).
    let switch_tabs =
        event.modifiers.control || event.modifiers.shift || !tabs_overflow;

    if switch_tabs {
        if raw_delta < -0.5 {
            view.update(cx, |this, cx| {
                if this.state.tab_drag.wheel_switch_throttle.should_switch() {
                    this.state.tab_drag.wheel_switch_throttle.record_scroll();
                    this.cycle_tab_next(cx);
                }
            });
        } else if raw_delta > 0.5 {
            view.update(cx, |this, cx| {
                if this.state.tab_drag.wheel_switch_throttle.should_switch() {
                    this.state.tab_drag.wheel_switch_throttle.record_scroll();
                    this.cycle_tab_prev(cx);
                }
            });
        }
    } else {
        // Smooth pixel-accurate scroll of the overflowing tab strip
        let step = if raw_delta.abs() > 4.0 {
            raw_delta * 1.35
        } else {
            raw_delta.signum() * 54.0
        };
        view.update(cx, |this, cx| {
            this.scroll_tab_bar_by(-step, cx);
        });
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
        window_width: f32,
    ) -> impl IntoElement {
        let tab_count = state.tabs.tabs.len();
        let pinned_count = state.tabs.tabs.iter().filter(|t| t.pinned).count();
        let active_idx = state.tabs.active_tab_index;
        let dragged_index = tab_drag.dragged_index;
        let drag_active = tab_drag.drag_active;
        let is_dragging_out = tab_drag.is_dragging_out;
        let external_preview = tab_drag.external_drop_preview.clone();
        let effective_tab_count = if external_preview.is_some() {
            tab_count + 1
        } else {
            tab_count
        };

        let metrics = compute_strip_metrics(window_width, effective_tab_count, pinned_count);
        let tabs_overflow = metrics.tabs_overflow;
        let clamped_scroll = if tabs_overflow {
            tab_drag.scroll_offset.clamp(0.0, metrics.max_scroll)
        } else {
            0.0
        };
        let can_scroll_left = tabs_overflow && clamped_scroll > 1.0;
        let can_scroll_right = tabs_overflow && clamped_scroll < metrics.max_scroll - 1.0;

        // Calculate how many tabs are currently scrolled off-screen to the left/right
        let hidden_left_count = if can_scroll_left {
            ((clamped_scroll / metrics.tab_w.max(1.0)).ceil() as usize).max(1)
        } else {
            0
        };
        let hidden_right_count = if can_scroll_right {
            let rem = (metrics.max_scroll - clamped_scroll).max(0.0);
            ((rem / metrics.tab_w.max(1.0)).ceil() as usize).max(1)
        } else {
            0
        };

        let vref = view.clone();
        // Lock the thumbnail cache once per render pass (not once per tab).
        let thumb_cache_guard = tab_thumbnail_cache.lock().ok();

        // ── Rightmost Caption buttons (always flex_shrink_0 so they never vanish on small windows)
        let caption = {
            let vm = vref.clone();
            let vc = vref.clone();
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
                        .on_mouse_down(MouseButton::Left, move |_ev, win, cx| {
                            cx.stop_propagation();
                            vc.update(cx, |this, cx| this.close_window_or_quit(win, cx));
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

        // ── Build all tab elements (plus optional cross-window incoming preview slot)
        let mut tab_elements: Vec<gpui::AnyElement> = Vec::with_capacity(effective_tab_count);
        for i in 0..tab_count {
            // If another window is hovering a dragged tab over this window at slot `i`,
            // render the live incoming drop slot before tab `i`!
            if let Some((ext_idx, ref ext_label, _)) = external_preview {
                if ext_idx == i {
                    tab_elements.push(Self::build_external_drop_slot(ext_label, metrics.tab_w));
                }
            }

            let tab = &state.tabs.tabs[i];
            let is_active = i == active_idx;
            let is_dragged = drag_active && dragged_index == Some(i);
            let is_hovered = tab_drag.hovered_tab_index == Some(i);
            let is_file_drop_target = tab_drag.file_drag_over_tab == Some(i);
            let is_settings = matches!(tab.target, crate::ui::views::types::ViewTarget::Settings);
            let is_home = matches!(tab.target, crate::ui::views::types::ViewTarget::Home)
                || tab.path.to_string_lossy() == "Home";
            let is_recycle = matches!(tab.target, crate::ui::views::types::ViewTarget::RecycleBin)
                || tab.path.to_string_lossy().contains("RecycleBin");
            let is_pinned = tab.pinned;
            let has_dual_pane = tab.secondary_pane_path.is_some();

            let label = tab_display_label(tab);

            let vc = vref.clone();
            let vd = vref.clone();
            let vcc = vref.clone();
            let v_scroll = vref.clone();

            let tc = if is_active {
                rgba(UiConstants::TEXT_PRIMARY_RGBA)
            } else if is_hovered {
                rgba(0xF5F5F5FFu32)
            } else {
                rgba(UiConstants::TEXT_SECONDARY_RGBA)
            };

            // WinUI 3 TabView surface colors (`App.xaml` Dark Theme):
            // `TabViewItemHeaderBackgroundSelected` = `LayerOnMicaBaseAltFillColorDefault` (`BG_NAV_BAR_RGBA`),
            // so the active tab seamlessly merges into the `NavigationToolbar` below it without a border seam.
            let bg = if is_dragged && !is_dragging_out {
                rgba(0x0078D41Au32) // subtle recessed drop well while floating pill is above
            } else if is_active {
                rgba(UiConstants::BG_NAV_BAR_RGBA)
            } else if is_file_drop_target {
                rgba(0x0078D42Au32)
            } else {
                rgba(0x00000000u32)
            };

            let base_item_w = if is_pinned {
                PINNED_TAB_W
            } else {
                metrics.tab_w
            };

            // Entrance animation when a new tab is created/duplicated/docked
            let (item_w, enter_opacity) = match tab_drag.new_tab_anim {
                Some((anim_idx, p)) if anim_idx == i => {
                    let eased = 1.0 - (1.0 - p).powi(3);
                    (base_item_w * (0.45 + 0.55 * eased), (0.3 + 0.7 * eased).clamp(0.0, 1.0))
                }
                _ => (base_item_w, 1.0),
            };

            // Smooth neighbor shift offset when tabs swap positions during drag-reorder
            let shift_x = tab_drag
                .slot_shift_offsets
                .get(i)
                .copied()
                .unwrap_or(0.0);

            let (slot_x, _) = tab_slot_x_and_width(i, pinned_count, metrics.tab_w);
            let tab_window_left_x = metrics.strip_start_x + slot_x - clamped_scroll;

            let mut t = div()
                .id(ElementId::from(90000 + i))
                .relative()
                .left(px(shift_x))
                .flex()
                .flex_shrink_0()
                .items_center()
                .h(px(TAB_H))
                .mt(px(6.0))
                .px(px(if is_pinned { 8.0 } else { 10.0 }))
                .w(px(item_w))
                .bg(bg)
                .opacity(if is_dragged && is_dragging_out {
                    0.35
                } else {
                    enter_opacity
                })
                .rounded_t(px(8.0))
                .border_t(px(1.0))
                .border_l(px(1.0))
                .border_r(px(1.0))
                .border_color(if is_file_drop_target || (is_dragged && !is_dragging_out) {
                    rgba(UiConstants::ACCENT_RGBA)
                } else if is_active {
                    rgba(UiConstants::CARD_STROKE_RGBA)
                } else {
                    rgba(0x00000000u32)
                })
                .cursor_pointer();

            if !is_active && !is_dragged {
                t = t.hover(|s| {
                    s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA))
                        .border_color(rgba(0xFFFFFF10u32))
                });
            }

            // WinUI 3 TabSeparator between adjacent inactive tabs (hidden when active or hovered)
            let next_is_active = i + 1 == active_idx;
            let next_is_hovered = tab_drag.hovered_tab_index == Some(i + 1);
            if !is_active
                && !is_hovered
                && !next_is_active
                && !next_is_hovered
                && i + 1 < tab_count
            {
                t = t.child(
                    div()
                        .absolute()
                        .right(px(0.0))
                        .top(px(8.0))
                        .w(px(1.0))
                        .h(px(16.0))
                        .bg(rgba(UiConstants::DIVIDER_RGBA)),
                );
            }

            // WinUI 3 Selected Tab bottom flare ear arcs (seamlessly blending into NavigationToolbar)
            if is_active && !(is_dragged && !is_dragging_out) {
                // Bottom-left outward flare ear
                t = t.child(
                    div()
                        .absolute()
                        .left(px(-4.0))
                        .bottom(px(0.0))
                        .w(px(4.0))
                        .h(px(4.0))
                        .bg(rgba(UiConstants::BG_NAV_BAR_RGBA)),
                );
                // Bottom-right outward flare ear
                t = t.child(
                    div()
                        .absolute()
                        .right(px(-4.0))
                        .bottom(px(0.0))
                        .w(px(4.0))
                        .h(px(4.0))
                        .bg(rgba(UiConstants::BG_NAV_BAR_RGBA)),
                );
            }

            t = t
                .on_mouse_down(MouseButton::Left, {
                    let vd2 = vd.clone();
                    let vc2 = vc.clone();
                    move |ev, _w, cx| {
                        cx.stop_propagation();
                        let pos_x: f32 = ev.position.x.into();
                        let pos_y: f32 = ev.position.y.into();
                        let grab_offset = (pos_x - tab_window_left_x).clamp(8.0, item_w - 8.0);
                        vd2.update(cx, |this, cx| {
                            this.start_tab_drag_with_offset(i, pos_x, pos_y, grab_offset, cx);
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
                            this.on_tab_mouse_move(
                                i,
                                tab_window_left_x,
                                p.x.into(),
                                p.y.into(),
                                win_w,
                                cx,
                            );
                        });
                    }
                })
                .on_mouse_up(MouseButton::Left, {
                    let vd2 = vd.clone();
                    move |_ev, _w, cx| {
                        vd2.update(cx, |this, cx| {
                            if this.is_dragging() {
                                this.drop_files_onto_tab(i, cx);
                            } else {
                                this.end_tab_drag(Some(i), cx);
                            }
                        });
                    }
                })
                .on_mouse_up_out(MouseButton::Left, {
                    let vd2 = vd.clone();
                    move |_ev, _w, cx| {
                        vd2.update(cx, |this, cx| {
                            if this.state.tab_drag.is_dragging {
                                this.end_tab_drag(None, cx);
                            }
                        });
                    }
                })
                .on_mouse_down(MouseButton::Middle, {
                    let v = vcc.clone();
                    move |_ev, win, cx| {
                        cx.stop_propagation();
                        v.update(cx, |this, cx| this.close_tab_with_window(i, win, cx));
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
                    handle_scroll(event, tabs_overflow, cx, &v_scroll);
                });

            // If this tab is currently lifted as an in-strip floating pill, leave the underlying
            // slot as a clean drop-well placeholder without duplicating the text/icon inside it.
            if is_dragged && !is_dragging_out {
                tab_elements.push(t.into_any_element());
                continue;
            }

            let thumb_path = thumb_cache_guard.as_ref().and_then(|c| c.get(&tab.path).cloned());
            let icon_el: gpui::AnyElement = Self::build_tab_icon(
                tab,
                is_settings,
                is_home,
                is_recycle,
                is_pinned,
                thumb_path,
            );
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
                        .whitespace_nowrap()
                        .child(label),
                );
                // Dual-pane split badge indicator on tabs that have an active split pane
                if has_dual_pane && item_w >= 110.0 {
                    t = t.child(
                        div()
                            .flex_shrink_0()
                            .mr(px(2.0))
                            .px(px(3.0))
                            .py(px(1.0))
                            .rounded(px(3.0))
                            .bg(rgba(0x0078D428u32))
                            .text_size(px(9.0))
                            .font_family(UiConstants::FONT_ICONS)
                            .text_color(rgb(UiConstants::ACCENT_COLOR_RGB))
                            .child("\u{E8A0}"),
                    );
                }
                // Show close button when tab is wide enough (> 68px), active, or hovered
                if item_w >= 68.0 || is_active || is_hovered {
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
                            .hover(|s| {
                                s.bg(rgba(0xFFFFFF1Cu32))
                                    .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                            })
                            .on_mouse_down(MouseButton::Left, {
                                let v = vcc.clone();
                                move |_ev, win, cx| {
                                    cx.stop_propagation();
                                    v.update(cx, |this, cx| {
                                        this.close_tab_with_window(i, win, cx);
                                    });
                                }
                            })
                            .child(
                                div()
                                    .text_size(px(10.0))
                                    .font_family(UiConstants::FONT_ICONS)
                                    .text_color(if is_active || is_hovered {
                                        rgba(UiConstants::TEXT_PRIMARY_RGBA)
                                    } else {
                                        rgba(UiConstants::TEXT_SECONDARY_RGBA)
                                    })
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

        // If external drop preview is at the very end (`ext_idx >= tab_count`), append it after the last tab
        if let Some((ext_idx, ref ext_label, _)) = external_preview {
            if ext_idx >= tab_count {
                tab_elements.push(Self::build_external_drop_slot(ext_label, metrics.tab_w));
            }
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
                .w(px(34.0))
                .h(px(30.0))
                .ml(px(6.0))
                .mt(px(4.0))
                .cursor_pointer()
                .rounded(px(6.0))
                .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
                .on_mouse_down(MouseButton::Left, move |_ev, _w, cx| {
                    cx.stop_propagation();
                    v.update(cx, |this, cx| this.toggle_tab_actions_menu(cx));
                })
                .child(
                    div()
                        .text_size(px(13.0))
                        .font_family(UiConstants::FONT_ICONS)
                        .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                        .child("\u{E762}"),
                )
        };

        // ── New Tab "+" button (`flex_shrink_0`) — also accepts dropped folders to open in new tabs!
        let add_btn = {
            let v_click = vref.clone();
            let v_move = vref.clone();
            let v_up = vref.clone();
            let is_drop_over = tab_drag.file_drag_over_new_tab_btn;
            div()
                .id("new-tab-btn")
                .flex()
                .items_center()
                .justify_center()
                .flex_shrink_0()
                .w(px(30.0))
                .h(px(30.0))
                .ml(px(4.0))
                .mt(px(4.0))
                .cursor_pointer()
                .rounded(px(6.0))
                .bg(if is_drop_over {
                    rgba(0x0078D433u32)
                } else {
                    rgba(0x00000000u32)
                })
                .border(px(if is_drop_over { 1.5 } else { 0.0 }))
                .border_color(rgba(UiConstants::ACCENT_RGBA))
                .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
                .on_mouse_down(MouseButton::Left, move |_ev, _w, cx| {
                    cx.stop_propagation();
                    v_click.update(cx, |this, cx| this.new_tab(cx));
                })
                .on_mouse_move(move |_ev, _w, cx| {
                    v_move.update(cx, |this, cx| {
                        if this.is_dragging() && !this.state.tab_drag.file_drag_over_new_tab_btn {
                            this.state.tab_drag.file_drag_over_new_tab_btn = true;
                            cx.notify();
                        }
                    });
                })
                .on_mouse_up(MouseButton::Left, move |_ev, _w, cx| {
                    v_up.update(cx, |this, cx| {
                        if this.is_dragging() {
                            this.drop_files_onto_new_tab_button(cx);
                        }
                    });
                })
                .child(
                    div()
                        .text_size(px(12.0))
                        .font_family(UiConstants::FONT_ICONS)
                        .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                        .child("\u{E710}"),
                )
        };

        let scroll_l = {
            let v = vref.clone();
            let mut btn = div()
                .id("tab-scroll-left")
                .relative()
                .flex()
                .items_center()
                .justify_center()
                .flex_shrink_0()
                .w(px(SCROLL_W))
                .h(px(30.0))
                .mt(px(4.0))
                .rounded(px(6.0))
                .child(
                    div()
                        .text_size(px(11.0))
                        .font_family(UiConstants::FONT_ICONS)
                        .text_color(if can_scroll_left {
                            rgba(UiConstants::TEXT_PRIMARY_RGBA)
                        } else {
                            rgba(0xFFFFFF2Eu32)
                        })
                        .child("\u{E76B}"),
                );
            if hidden_left_count > 0 {
                btn = btn.child(
                    div()
                        .absolute()
                        .top(px(1.0))
                        .right(px(1.0))
                        .px(px(3.0))
                        .rounded(px(4.0))
                        .bg(rgba(0x0078D4CCu32))
                        .text_size(px(8.0))
                        .font_weight(FontWeight(700.0))
                        .text_color(rgb(0xFFFFFFu32))
                        .child(format!("{}", hidden_left_count)),
                );
            }
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
                .relative()
                .flex()
                .items_center()
                .justify_center()
                .flex_shrink_0()
                .w(px(SCROLL_W))
                .h(px(30.0))
                .mt(px(4.0))
                .rounded(px(6.0))
                .child(
                    div()
                        .text_size(px(11.0))
                        .font_family(UiConstants::FONT_ICONS)
                        .text_color(if can_scroll_right {
                            rgba(UiConstants::TEXT_PRIMARY_RGBA)
                        } else {
                            rgba(0xFFFFFF2Eu32)
                        })
                        .child("\u{E76C}"),
                );
            if hidden_right_count > 0 {
                btn = btn.child(
                    div()
                        .absolute()
                        .top(px(1.0))
                        .left(px(1.0))
                        .px(px(3.0))
                        .rounded(px(4.0))
                        .bg(rgba(0x0078D4CCu32))
                        .text_size(px(8.0))
                        .font_weight(FontWeight(700.0))
                        .text_color(rgb(0xFFFFFFu32))
                        .child(format!("{}", hidden_right_count)),
                );
            }
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

        // ── Live In-Strip Floating Drag Pill (when dragging horizontally within the tab bar)
        let in_strip_drag_pill: Option<gpui::AnyElement> =
            if drag_active && !is_dragging_out {
                dragged_index
                    .and_then(|di| state.tabs.tabs.get(di).map(|t| (di, t)))
                    .map(|(_di, tab)| {
                        let is_settings =
                            matches!(tab.target, crate::ui::views::types::ViewTarget::Settings);
                        let is_home = matches!(tab.target, crate::ui::views::types::ViewTarget::Home)
                            || tab.path.to_string_lossy() == "Home";
                        let is_recycle =
                            matches!(tab.target, crate::ui::views::types::ViewTarget::RecycleBin)
                                || tab.path.to_string_lossy().contains("RecycleBin");
                        let pill_w = if tab.pinned {
                            PINNED_TAB_W
                        } else {
                            metrics.tab_w
                        };
                        let min_x = metrics.strip_start_x;
                        let max_x = (metrics.strip_start_x + metrics.strip_viewport_w - pill_w)
                            .max(min_x);
                        let pill_left =
                            (tab_drag.current_x - tab_drag.grab_offset_x).clamp(min_x, max_x);
                        let label = tab_display_label(tab);
                        let thumb_path = thumb_cache_guard.as_ref().and_then(|c| c.get(&tab.path).cloned());
                        let icon_el = Self::build_tab_icon(
                            tab,
                            is_settings,
                            is_home,
                            is_recycle,
                            tab.pinned,
                            thumb_path,
                        );
                        div()
                            .absolute()
                            .left(px(pill_left))
                            .top(px(5.0))
                            .w(px(pill_w))
                            .h(px(TAB_H))
                            .flex()
                            .items_center()
                            .px(px(if tab.pinned { 8.0 } else { 10.0 }))
                            .bg(rgba(0x323232FFu32))
                            .rounded_t(px(8.0))
                            .border(px(1.5))
                            .border_color(rgba(UiConstants::ACCENT_RGBA))
                            .shadow_lg()
                            .child(icon_el)
                            .children(if !tab.pinned {
                                Some(
                                    div()
                                        .flex_1()
                                        .min_w(px(0.0))
                                        .text_size(px(12.0))
                                        .font_weight(FontWeight(600.0))
                                        .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                                        .overflow_hidden()
                                        .whitespace_nowrap()
                                        .child(label),
                                )
                            } else {
                                None
                            })
                            .child(
                                div()
                                    .absolute()
                                    .bottom(px(0.0))
                                    .left(px(8.0))
                                    .right(px(8.0))
                                    .h(px(2.5))
                                    .rounded(px(1.5))
                                    .bg(rgb(UiConstants::ACCENT_COLOR_RGB)),
                            )
                            .into_any_element()
                    })
            } else {
                None
            };

        // ── Outer Tab Bar container
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
                .h(px(TAB_BAR_H))
                .w_full()
                .overflow_hidden()
                .bg(rgba(UiConstants::BG_MICA_CHROME_RGBA))
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
                    handle_scroll(event, tabs_overflow, cx, &v_scroll);
                })
        };

        let v_drag = vref.clone();
        let v_clear_hover = vref.clone();
        let drag_spacer = div()
            .id("titlebar-drag-spacer")
            .flex_grow()
            .flex_shrink_0()
            .min_w(px(MIN_DRAG_GAP_W))
            .h_full()
            .on_mouse_move(move |_ev, _w, cx| {
                v_clear_hover.update(cx, |this, cx| {
                    this.clear_tab_hover(cx);
                });
            })
            .on_mouse_down(MouseButton::Left, move |ev, window, cx| {
                handle_titlebar_drag_mouse_down(ev.click_count, window, cx, &v_drag);
            });

        if tabs_overflow {
            // Scrollable overflow strip: `#tab-strip` is a fixed-width clipped viewport, and its
            // inner track has `.flex_shrink_0().w(px(metrics.total_tabs_w))` so Taffy never shrinks
            // the tabs below `tab_w`!
            bar.child(actions_btn)
                .child(scroll_l)
                .child(
                    div()
                        .id("tab-strip")
                        .relative()
                        .flex()
                        .w(px(metrics.strip_viewport_w))
                        .min_w(px(0.0))
                        .flex_shrink_0()
                        .h_full()
                        .overflow_hidden()
                        .child(
                            div()
                                .flex()
                                .flex_nowrap()
                                .flex_shrink_0()
                                .items_center()
                                .w(px(metrics.total_tabs_w))
                                .h_full()
                                .ml(px(-clamped_scroll))
                                .children(tab_elements),
                        )
                        .children(if can_scroll_left {
                            Some(
                                div()
                                    .absolute()
                                    .left(px(0.0))
                                    .top(px(0.0))
                                    .bottom(px(0.0))
                                    .w(px(12.0))
                                    .bg(rgba(0x00000038u32)),
                            )
                        } else {
                            None
                        })
                        .children(if can_scroll_right {
                            Some(
                                div()
                                    .absolute()
                                    .right(px(0.0))
                                    .top(px(0.0))
                                    .bottom(px(0.0))
                                    .w(px(12.0))
                                    .bg(rgba(0x00000038u32)),
                            )
                        } else {
                            None
                        }),
                )
                .child(scroll_r)
                .child(add_btn)
                .child(drag_spacer)
                .children(in_strip_drag_pill)
                .child(caption)
        } else {
            bar.child(actions_btn)
                .child(
                    div()
                        .id("tab-strip")
                        .flex()
                        .flex_nowrap()
                        .min_w(px(0.0))
                        .max_w(px(metrics.strip_viewport_w))
                        .h_full()
                        .overflow_hidden()
                        .items_center()
                        .children(tab_elements),
                )
                .child(add_btn)
                .child(drag_spacer)
                .children(in_strip_drag_pill)
                .child(caption)
        }
    }

    /// Build the icon element for a tab (Settings gear, Home icon/glyph, Recycle Bin glyph, or cached folder thumbnail).
    fn build_tab_icon(
        _tab: &crate::core::navigation_manager::TabState,
        is_settings: bool,
        is_home: bool,
        is_recycle: bool,
        is_pinned: bool,
        thumb_path: Option<std::path::PathBuf>,
    ) -> gpui::AnyElement {
        let mr = if is_pinned { 0.0 } else { 7.0 };
        if is_settings {
            div()
                .flex_shrink_0()
                .text_size(px(14.0))
                .font_family(UiConstants::FONT_MDL2)
                .text_color(rgba(UiConstants::ACCENT_RGBA))
                .mr(px(mr))
                .child("\u{E713}")
                .into_any_element()
        } else if is_home {
            if let Some(path) =
                crate::ui::components::sidebar_state::bundled_icon_path("sidebar_home.png")
            {
                gpui::img(path)
                    .flex_shrink_0()
                    .w(px(15.0))
                    .h(px(15.0))
                    .mr(px(mr))
                    .object_fit(gpui::ObjectFit::Contain)
                    .into_any_element()
            } else {
                div()
                    .flex_shrink_0()
                    .text_size(px(14.0))
                    .font_family(UiConstants::FONT_MDL2)
                    .text_color(rgba(UiConstants::ACCENT_RGBA))
                    .mr(px(mr))
                    .child("\u{E10F}")
                    .into_any_element()
            }
        } else if is_recycle {
            div()
                .flex_shrink_0()
                .text_size(px(14.0))
                .font_family(UiConstants::FONT_MDL2)
                .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                .mr(px(mr))
                .child("\u{E74D}")
                .into_any_element()
        } else {
            if let Some(path) = thumb_path {
                gpui::img(path)
                    .flex_shrink_0()
                    .w(px(16.0))
                    .h(px(16.0))
                    .mr(px(mr))
                    .rounded(px(2.0))
                    .object_fit(gpui::ObjectFit::Cover)
                    .into_any_element()
            } else {
                div()
                    .flex_shrink_0()
                    .text_size(px(14.0))
                    .font_family(UiConstants::FONT_MDL2)
                    .text_color(rgb(0xF5B942u32))
                    .mr(px(mr))
                    .child("\u{E8B7}")
                    .into_any_element()
            }
        }
    }

    /// Build a glowing placeholder slot in the tab strip when another window is dragging a tab over this window.
    fn build_external_drop_slot(label: &str, tab_w: f32) -> gpui::AnyElement {
        div()
            .flex()
            .flex_shrink_0()
            .items_center()
            .h(px(TAB_H))
            .mt(px(6.0))
            .mx(px(2.0))
            .px(px(10.0))
            .w(px(tab_w.clamp(96.0, 175.0)))
            .bg(rgba(0x0078D430u32))
            .rounded_t(px(8.0))
            .border(px(1.5))
            .border_color(rgba(UiConstants::ACCENT_RGBA))
            .child(
                div()
                    .text_size(px(12.0))
                    .font_family(UiConstants::FONT_ICONS)
                    .text_color(rgb(UiConstants::ACCENT_COLOR_RGB))
                    .mr(px(6.0))
                    .child("\u{E710}"),
            )
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.0))
                    .text_size(px(12.0))
                    .font_weight(FontWeight(600.0))
                    .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .child(label.to_string()),
            )
            .into_any_element()
    }

    /// Render root-level floating overlays (outside `#tab-bar` so they are never clipped by `overflow_hidden`):
    /// 1. Detached Tab Window Preview Ghost when dragging a tab outside the tab strip (`is_dragging_out`).
    /// 2. Rich Tab Hover Preview Card when hovering over a tab for 400ms.
    pub fn render_floating_overlays(
        state: &NavigationState,
        tab_drag: &crate::app_state::TabDragState,
        tab_thumbnail_cache: &std::sync::Arc<
            std::sync::Mutex<
                std::collections::HashMap<crate::core::path_util::FilesPath, std::path::PathBuf>,
            >,
        >,
        window_width: f32,
        window_height: f32,
    ) -> Vec<gpui::AnyElement> {
        let mut overlays: Vec<gpui::AnyElement> = Vec::new();

        // 1. Detached Tab Window Preview Ghost (`drag_active && is_dragging_out`)
        if tab_drag.drag_active && tab_drag.is_dragging_out {
            if let Some(di) = tab_drag.dragged_index {
                if let Some(tab) = state.tabs.tabs.get(di) {
                    let label = tab_display_label(tab);
                    let full_path = tab.path.to_string_lossy().to_string();
                    let card_w = 240.0f32;
                    let card_h = 86.0f32;
                    let left = (tab_drag.current_x - card_w * 0.5)
                        .clamp(8.0, (window_width - card_w - 8.0).max(8.0));
                    let top = (tab_drag.current_y - 18.0)
                        .clamp(4.0, (window_height - card_h - 8.0).max(4.0));

                    let (badge_text, badge_bg) = if let Some(ref target) = tab_drag.cross_window_target
                    {
                        (
                            format!(
                                "Dock into window ({}) • Tab #{}",
                                target.target_window_title,
                                target.insert_index + 1
                            ),
                            rgba(0x107C41EEu32),
                        )
                    } else if state.tabs.tabs.len() > 1 {
                        (
                            "Release to open in new window".to_string(),
                            rgba(0x0078D4EEu32),
                        )
                    } else {
                        (
                            "Release to move window here".to_string(),
                            rgba(0x3A3A3AEEu32),
                        )
                    };

                    let ghost_card = div()
                        .absolute()
                        .left(px(left))
                        .top(px(top))
                        .w(px(card_w))
                        .flex()
                        .flex_col()
                        .bg(rgba(0x252525F8u32))
                        .border(px(1.5))
                        .border_color(rgba(UiConstants::ACCENT_RGBA))
                        .rounded(px(8.0))
                        .shadow_lg()
                        .overflow_hidden()
                        // Mini window titlebar header
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .px(px(10.0))
                                .py(px(6.0))
                                .bg(rgba(0x1E1E1EFFu32))
                                .border_b(px(1.0))
                                .border_color(rgba(UiConstants::DIVIDER_RGBA))
                                .child(
                                    div()
                                        .text_size(px(13.0))
                                        .font_family(UiConstants::FONT_MDL2)
                                        .text_color(rgb(0xF5B942u32))
                                        .mr(px(8.0))
                                        .child("\u{E8B7}"),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .text_size(px(12.0))
                                        .font_weight(FontWeight(600.0))
                                        .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                                        .overflow_hidden()
                                        .whitespace_nowrap()
                                        .child(label),
                                ),
                        )
                        // Path & action badge
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .px(px(10.0))
                                .py(px(8.0))
                                .gap(px(6.0))
                                .child(
                                    div()
                                        .text_size(px(11.0))
                                        .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                        .overflow_hidden()
                                        .whitespace_nowrap()
                                        .child(full_path),
                                )
                                .child(
                                    div()
                                        .px(px(8.0))
                                        .py(px(3.0))
                                        .rounded(px(4.0))
                                        .bg(badge_bg)
                                        .text_size(px(11.0))
                                        .font_weight(FontWeight(600.0))
                                        .text_color(rgb(0xFFFFFFu32))
                                        .child(badge_text),
                                ),
                        )
                        .into_any_element();

                    overlays.push(ghost_card);
                }
            }
            return overlays;
        }

        // 2. Rich Tab Hover Preview Card (`show_tab_hover_card && !is_dragging`)
        if tab_drag.show_tab_hover_card && !tab_drag.is_dragging {
            if let Some(hi) = tab_drag.hovered_tab_index {
                if let Some(tab) = state.tabs.tabs.get(hi) {
                    let is_settings =
                        matches!(tab.target, crate::ui::views::types::ViewTarget::Settings);
                    let is_home = matches!(tab.target, crate::ui::views::types::ViewTarget::Home)
                        || tab.path.to_string_lossy() == "Home";
                    let is_recycle =
                        matches!(tab.target, crate::ui::views::types::ViewTarget::RecycleBin)
                            || tab.path.to_string_lossy().contains("RecycleBin");

                    let title = tab_display_label(tab);
                    let subtitle = if is_settings {
                        "Application Settings & Preferences".to_string()
                    } else if is_home {
                        "Quick Access, Drives & Recent Files".to_string()
                    } else {
                        tab.path.to_string_lossy().to_string()
                    };
                    let view_label = match tab.view_mode {
                        crate::config::config::ViewMode::Details => "Details View",
                        crate::config::config::ViewMode::List => "List View",
                        crate::config::config::ViewMode::Cards => "Cards View",
                        crate::config::config::ViewMode::Grid => "Grid View",
                        crate::config::config::ViewMode::Columns => "Columns View",
                        crate::config::config::ViewMode::Tree => "Tree View",
                    };

                    let card_w = 248.0f32;
                    let left = tab_drag
                        .hovered_tab_left_x
                        .clamp(8.0, (window_width - card_w - 8.0).max(8.0));
                    let top = TAB_BAR_H + 4.0;

                    let thumb_path = tab_thumbnail_cache.lock().ok().and_then(|c| c.get(&tab.path).cloned());
                    let icon_el = Self::build_tab_icon(
                        tab,
                        is_settings,
                        is_home,
                        is_recycle,
                        false,
                        thumb_path,
                    );

                    let mut badges = div().flex().items_center().gap(px(6.0)).mt(px(4.0)).child(
                        div()
                            .px(px(6.0))
                            .py(px(2.0))
                            .rounded(px(4.0))
                            .bg(rgba(0xFFFFFF12u32))
                            .text_size(px(10.0))
                            .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                            .child(view_label),
                    );
                    if tab.pinned {
                        badges = badges.child(
                            div()
                                .px(px(6.0))
                                .py(px(2.0))
                                .rounded(px(4.0))
                                .bg(rgba(0x0078D428u32))
                                .text_size(px(10.0))
                                .text_color(rgb(UiConstants::ACCENT_COLOR_RGB))
                                .child("Pinned"),
                        );
                    }
                    if tab.secondary_pane_path.is_some() {
                        badges = badges.child(
                            div()
                                .px(px(6.0))
                                .py(px(2.0))
                                .rounded(px(4.0))
                                .bg(rgba(0x0078D428u32))
                                .text_size(px(10.0))
                                .text_color(rgb(UiConstants::ACCENT_COLOR_RGB))
                                .child("Dual Pane"),
                        );
                    }

                    let card = div()
                        .absolute()
                        .left(px(left))
                        .top(px(top))
                        .w(px(card_w))
                        .p(px(10.0))
                        .bg(rgba(0x282828F8u32))
                        .border(px(1.0))
                        .border_color(rgba(0x454545FFu32))
                        .rounded(px(8.0))
                        .shadow_lg()
                        .flex()
                        .flex_col()
                        .gap(px(4.0))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .child(icon_el)
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w(px(0.0))
                                        .text_size(px(12.5))
                                        .font_weight(FontWeight(600.0))
                                        .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                                        .overflow_hidden()
                                        .whitespace_nowrap()
                                        .child(title),
                                ),
                        )
                        .child(
                            div()
                                .text_size(px(11.0))
                                .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .child(subtitle),
                        )
                        .child(badges)
                        .with_animation(
                            "tab-hover-card-fade",
                            Animation::new(std::time::Duration::from_millis(120)),
                            |elem, delta| elem.opacity(delta),
                        )
                        .into_any_element();

                    overlays.push(card);
                }
            }
        }

        overlays
    }
}

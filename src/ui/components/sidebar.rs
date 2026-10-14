//! Sidebar UI component.
//! Sections: Favorites, File Tags, Libraries, This PC (drives), Cloud Drives, Network, WSL.
//!
//! Features:
//! - Collapse toggle (hamburger) → compact icon-only mode
//! - "Home" item for quick navigation
//! - Collapsed mode: icon-only section headers, click to show tooltip/flyout with items
//! - Draggable/reorderable section groups
//! - Settings button pinned to bottom

use super::sidebar_state::*;
use crate::app::AppState;
use crate::ui::components::dialogs::DialogType;
use crate::ui::constants::UiConstants;
use gpui::*;
use std::time::Duration;

// ─────────────────────────────────────────────────────────────────────────────
// Layout constants
// ─────────────────────────────────────────────────────────────────────────────

const SECTION_HEADER_HEIGHT: f32 = 28.0;
const SECTION_ITEM_HEIGHT: f32 = 30.0;
const SECTION_MARGIN_BOTTOM: f32 = 8.0;
const COLLAPSED_SECTION_ICON_SIZE: f32 = 36.0;
const TOOLTIP_WIDTH: f32 = 220.0;
/// Estimated max height of the context menu, used for flip-up positioning
const SIDEBAR_MENU_HEIGHT: f32 = 300.0;

// ─────────────────────────────────────────────────────────────────────────────
// Icon lookup for sections (used in collapsed mode)
// ─────────────────────────────────────────────────────────────────────────────

fn section_icon(section_id: SectionId) -> &'static str {
    match section_id {
        SectionId::Favorites => "\u{E734}",   // Favorite star
        SectionId::Tags => "\u{E8EC}",        // Tag
        SectionId::Libraries => "\u{E8B7}",   // Folder
        SectionId::ThisPc => "\u{E770}",      // HDD/This PC
        SectionId::CloudDrives => "\u{E753}", // Cloud
        SectionId::Network => "\u{E802}",     // Network
        SectionId::Wsl => "\u{E756}",         // Terminal/WSL
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// SidebarView
// ─────────────────────────────────────────────────────────────────────────────

pub struct SidebarView;

impl SidebarView {
    pub fn build(
        state: &SidebarState,
        _app_state: &AppState,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let collapsed = state.collapsed;
        let reorder_mode = state.reorder_mode;
        let width = if collapsed {
            px(UiConstants::SIDEBAR_WIDTH_COLLAPSED_RAW as f32)
        } else {
            px(state.get_display_width())
        };

        // Precompute section Y offsets for tooltip positioning
        let section_offsets: Vec<f32> = if collapsed {
            state
                .sections
                .iter()
                .filter(|s| !state.hidden_sections.contains(&s.id))
                .scan(0.0f32, |offset, _| {
                    let start = *offset;
                    *offset += COLLAPSED_SECTION_ICON_SIZE + 4.0;
                    Some(start)
                })
                .collect()
        } else {
            state
                .sections
                .iter()
                .filter(|s| !state.hidden_sections.contains(&s.id))
                .scan(COLLAPSED_SECTION_ICON_SIZE + 4.0, |offset, section| {
                    let h = section_height(section);
                    let start = *offset;
                    *offset += h;
                    Some(start)
                })
                .collect()
        };

        // Find current tooltip's Y offset for positioning (used by app.rs)
        #[allow(unused_variables)]
        let tooltip_y = state.tooltip_section.and_then(|sid| {
            state
                .sections
                .iter()
                .filter(|s| !state.hidden_sections.contains(&s.id))
                .position(|s| s.id == sid)
                .and_then(|idx| section_offsets.get(idx))
                .copied()
        });
        let collapsed = state.collapsed;
        let width = if collapsed {
            px(UiConstants::SIDEBAR_WIDTH_COLLAPSED_RAW as f32)
        } else {
            px(state.get_display_width())
        };

        let mut sidebar_el = div()
            .id("sidebar")
            .w(width)
            .h_full()
            .flex()
            .flex_col()
            .flex_shrink_0() // Prevent sidebar from shrinking when content needs space
            .overflow_hidden() // Clip content to explicit width
            .bg(rgb(UiConstants::BG_SIDEBAR_RGB))
            .border_r(px(1.0))
            .border_color(rgba(UiConstants::DIVIDER_RGBA))
            .relative() // For absolute-positioned tooltip
            .on_mouse_down(MouseButton::Right, {
                let view = view.clone();
                move |event, win, cx| {
                    cx.stop_propagation();
                    let position = event.position;
                    let flip_up: f32 = position.y.into();
                    let win_h: f32 = win.viewport_size().height.into();
                    let bottom_y = if flip_up > (win_h - SIDEBAR_MENU_HEIGHT) {
                        win_h - flip_up
                    } else {
                        0.0
                    };
                    view.update(cx, |this, cx| {
                        let reorder_mode = this.state.sidebar.reorder_mode;
                        let hidden_sections = this.state.sidebar.hidden_sections.clone();
                        this.state.active_dialog = Some(DialogType::SidebarContextMenu {
                            target: crate::ui::components::dialogs::SidebarMenuTarget::Background,
                            position,
                            reorder_mode,
                            hidden_sections,
                            bottom_y,
                        });
                        cx.notify();
                    });
                }
            })
            .on_mouse_down(MouseButton::Left, {
                let view = view.clone();
                move |_event, _win, cx| {
                    view.update(cx, |this, cx| {
                        if this.state.sidebar.reorder_mode {
                            this.state.sidebar.exit_reorder_mode();
                            cx.notify();
                        }
                    });
                }
            })
            // Collapse toggle at top
            .child(Self::render_collapse_toggle(state, view.clone()))
            // Home item
            .child(Self::render_home_item(collapsed, state, view.clone()))
            // Scrollable sections area
            .child(
                div()
                    .id("sidebar-scroll-container")
                    .flex_grow()
                    .flex_col()
                    .w_full()
                    .overflow_hidden()
                    .relative()
                    .on_scroll_wheel({
                        let view = view.clone();
                        move |event: &ScrollWheelEvent, window: &mut Window, cx: &mut App| {
                            let line_height = px(16.0);
                            let delta = event.delta.pixel_delta(line_height);
                            let delta_y: f32 = delta.y.into();

                            view.update(cx, |this, cx| {
                                let sidebar = &mut this.state.sidebar;

                                // Calculate content height from visible sections
                                let content_h = {
                                    let padding = UiConstants::PADDING_BASE as f32; // top padding
                                    let bottom_padding = 60.0; // bottom padding (extra space for rubber band visibility)
                                    let mut h = padding + bottom_padding;
                                    for section in &sidebar.sections {
                                        if sidebar.hidden_sections.contains(&section.id) {
                                            continue;
                                        }
                                        h += SECTION_HEADER_HEIGHT;
                                        if sidebar.collapsed {
                                            h += COLLAPSED_SECTION_ICON_SIZE;
                                        } else if !section.collapsed {
                                            h += section.items.len() as f32 * SECTION_ITEM_HEIGHT;
                                        }
                                        h += SECTION_MARGIN_BOTTOM;
                                    }
                                    h
                                };

                                // Viewport ≈ window content height minus fixed elements
                                let win_h: f32 = window.viewport_size().height.into();
                                let viewport_h = (win_h - 100.0).max(100.0);

                                // Scale scroll delta for more responsive feel
                                let scaled_delta = delta_y * 1.5;

                                // Update scroll bounds
                                let bs = &mut sidebar.bouncy_scroll;
                                bs.update_bounds(content_h, viewport_h);

                                // Apply scroll delta — sets offset directly for 1:1 tracking,
                                // accumulates velocity for momentum, resets grace counter.
                                // This increments the interaction epoch.
                                bs.apply_scroll_delta(scaled_delta);

                                // Capture epoch AFTER apply so the animation loop's epoch guard
                                // correctly detects when new input supersedes it.
                                let epoch = bs.epoch();

                                // Always start the animation loop. It handles:
                                // - Grace period (while is_active and recent input)
                                // - Overscroll spring (after grace period)
                                // - Momentum (after grace period, with velocity)
                                bs.animating = true;
                                crate::ui::components::bouncy_scroll::start_bouncy_animation(
                                    &view,
                                    cx,
                                    |app_state: &mut crate::app::AppState| {
                                        &mut app_state.sidebar.bouncy_scroll
                                    },
                                    None,
                                    epoch,
                                );
                                cx.notify();
                            });
                        }
                    })
                    // ─── Mouse hold-drag scroll ───────────────────────────
                    .on_mouse_down(MouseButton::Left, {
                        let view = view.clone();
                        move |_event, win, cx| {
                            view.update(cx, |this, cx| {
                                if this.state.sidebar.reorder_mode {
                                    return; // Don't start drag scroll in reorder mode
                                }
                                let y: f32 = win.mouse_position().y.into();
                                this.state.sidebar.bouncy_scroll.start_mouse_drag(y);
                                cx.notify();
                            });
                        }
                    })
                    .on_mouse_move({
                        let view = view.clone();
                        move |_event, win, cx| {
                            view.update(cx, |this, cx| {
                                let bs = &mut this.state.sidebar.bouncy_scroll;
                                if bs.drag_active {
                                    let y: f32 = win.mouse_position().y.into();
                                    let delta = bs.apply_mouse_drag_delta(y);
                                    // Same 1.5x scaling as scroll wheel
                                    let scaled_delta = delta * 1.5;
                                    // Update bounds from current widget sizes
                                    let win_h: f32 = win.viewport_size().height.into();
                                    let viewport_h = (win_h - 100.0).max(100.0);
                                    bs.update_bounds(bs.content_height, viewport_h);
                                    bs.apply_scroll_delta(scaled_delta);
                                    let epoch = bs.epoch();
                                    bs.animating = true;
                                    crate::ui::components::bouncy_scroll::start_bouncy_animation(
                                        &view,
                                        cx,
                                        |app_state: &mut crate::app::AppState| {
                                            &mut app_state.sidebar.bouncy_scroll
                                        },
                                        None,
                                        epoch,
                                        );
                                    cx.notify();
                                }
                            });
                        }
                    })
                    .on_mouse_up(MouseButton::Left, {
                        let view = view.clone();
                        move |_event, _win, cx| {
                            view.update(cx, |this, cx| {
                                let bs = &mut this.state.sidebar.bouncy_scroll;
                                if bs.drag_active {
                                    // Update bounds one last time before release
                                    bs.update_bounds(bs.content_height, bs.viewport_height);
                                    bs.release();
                                    if bs.animating {
                                        let epoch = bs.epoch();
                                        crate::ui::components::bouncy_scroll::start_bouncy_animation(
                                            &view,
                                            cx,
                                            |app_state: &mut crate::app::AppState| {
                                                &mut app_state.sidebar.bouncy_scroll
                                            },
                                            None,
                                            epoch,
                                            );
                                        cx.notify();
                                    }
                                }
                            });
                        }
                    })
                    .child(
                        crate::ui::components::bouncy_scroll::bouncy_scroll_content(
                            &state.bouncy_scroll,
                            [div()
                                .flex_col()
                                .w_full()
                                .pt(px(UiConstants::PADDING_BASE as f32))
                                .pb(px(60.0))
                                // Sections
                                .children(
                                    state
                                        .sections
                                        .iter()
                                        .filter(|s| !state.hidden_sections.contains(&s.id))
                                        .enumerate()
                                        .map(|(idx, section)| {
                                            if collapsed {
                                                Self::render_collapsed_section(
                                                    section,
                                                    idx,
                                                    state,
                                                    view.clone(),
                                                )
                                                .into_any_element()
                                            } else {
                                                Self::render_section(section, idx, state, view.clone())
                                                    .into_any_element()
                                            }
                                        }),
                                )],
                        ),
                    )
                    // Minimal scrollbar
                    .child(
                        crate::ui::components::bouncy_scroll::bouncy_scroll_scrollbar(
                            &state.bouncy_scroll,
                        ),
                    ),
            )
            // Settings button (pinned to bottom)
            .child(Self::render_settings_button(collapsed, state, view.clone()));

        if reorder_mode {
            // During drag-and-drop reorder, we listen for mouse up globally on the sidebar
            sidebar_el = sidebar_el.on_mouse_up(MouseButton::Left, {
                let view = view.clone();
                move |_, win, cx| {
                    view.update(cx, |this, cx| {
                        if this.state.sidebar.drag_section_idx.is_some() {
                            this.state.sidebar.end_section_drag();
                            cx.notify();
                        }
                        if this.state.sidebar.drag_item.is_some() {
                            this.state.sidebar.end_item_drag();
                            cx.notify();
                        }
                    });
                }
            });
        }

        sidebar_el
    }

    fn render_collapse_toggle(
        state: &SidebarState,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let v = view.clone();
        let collapsed = state.collapsed;
        let reorder_mode = state.reorder_mode;

        let mut toggle = div()
            .id("sidebar-collapse-toggle")
            .h(px(40.0))
            .flex()
            .items_center()
            .justify_center()
            .px(px(if collapsed { 0.0 } else { 12.0 }))
            .w_full()
            .child(
                div()
                    .text_size(px(16.0))
                    .font_family(UiConstants::FONT_ICONS)
                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                    .child("\u{E700}"), // Hamburger
            );

        if reorder_mode {
            toggle = toggle.opacity(0.4).cursor_not_allowed();
        } else {
            toggle = toggle
                .cursor_pointer()
                .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
                .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
                    v.update(cx, |this, cx| {
                        this.state.sidebar.toggle_collapsed();
                        cx.notify();
                    });
                });
        }

        toggle
    }

    // ─── Home item ────────────────────────────────────────────────────────

    fn render_home_item(
        collapsed: bool,
        state: &SidebarState,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let v = view.clone();
        let icon = "\u{E10F}"; // Home glyph (MDL2)
        let reorder_mode = state.reorder_mode;

        let mut item = if collapsed {
            // Just the icon, no text
            div()
                .id("sidebar-home")
                .h(px(COLLAPSED_SECTION_ICON_SIZE))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(4.0))
                .child(
                    div()
                        .text_size(px(16.0))
                        .font_family(UiConstants::FONT_MDL2)
                        .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                        .child(icon),
                )
        } else {
            // Icon + text
            div()
                .id("sidebar-home")
                .h(px(SECTION_ITEM_HEIGHT))
                .mx(px(8.0))
                .flex()
                .items_center()
                .px(px(8.0))
                .gap(px(12.0))
                .rounded(px(4.0))
                .child(
                    div()
                        .w(px(20.0))
                        .h(px(20.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_size(px(16.0))
                        .font_family(UiConstants::FONT_MDL2)
                        .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                        .child(icon),
                )
                .child(
                    div()
                        .flex_grow()
                        .overflow_hidden()
                        .text_size(px(UiConstants::ITEM_FONT_SIZE_RAW as f32))
                        .font_weight(FontWeight(600.0))
                        .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                        .child("Home"),
                )
        };

        if reorder_mode {
            item = item.opacity(0.4).cursor_not_allowed();
        } else {
            item = item
                .cursor_pointer()
                .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
                .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
                    v.update(cx, |this, cx| {
                        this.navigate_home(cx);
                    });
                });
        }

        item
    }

    // ─── Expanded section ─────────────────────────────────────────────────

    fn render_section(
        section: &SidebarSectionData,
        section_idx: usize,
        state: &SidebarState,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let collapsed = section.collapsed;

        div()
            .id(SharedString::from(format!(
                "sidebar-section-{}",
                section_idx
            )))
            .flex_col()
            .mb(px(SECTION_MARGIN_BOTTOM))
            .child(Self::render_section_header(
                section,
                section_idx,
                state,
                view.clone(),
            ))
            .children(if collapsed {
                vec![]
            } else {
                section
                    .items
                    .iter()
                    .enumerate()
                    .map(|(idx, item)| Self::render_item(item, idx, section, state, view.clone()))
                    .collect::<Vec<_>>()
            })
    }

    /// Colored Fluent glyph per sidebar section (Files-community parity).
    /// Returns (glyph, rgb color).
    fn section_glyph(section_id: SectionId) -> (&'static str, u32) {
        match section_id {
            SectionId::Favorites => ("\u{E718}", 0x60CDFF),
            SectionId::Tags => ("\u{E8EC}", 0xB4A0FF),
            SectionId::Libraries => ("\u{E8B7}", 0xFCE100),
            SectionId::ThisPc => ("\u{E770}", 0x4CC2FF),
            SectionId::CloudDrives => ("\u{E753}", 0x60CDFF),
            SectionId::Network => ("\u{E802}", 0x6CCB5F),
            SectionId::Wsl => ("\u{E756}", 0xFF99A4),
        }
    }

    /// Fluent icon tint per sidebar item kind (replaces flat grey).
    fn item_icon_color(item: &SidebarItem) -> u32 {
        match &item.id {
            SidebarItemId::Tag(_) => 0xB4A0FF,
            SidebarItemId::Library(_) => 0xFCE100,
            SidebarItemId::Drive { .. } => 0x60CDFF,
            SidebarItemId::CloudDrive(_) => 0x60CDFF,
            SidebarItemId::Network => 0x6CCB5F,
            SidebarItemId::WslDistro(_) => 0xFF99A4,
            SidebarItemId::RecycleBin => 0x4CC2FF,
            SidebarItemId::Pinned(path) => {
                let name = path
                    .as_path()
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_lowercase();
                match name.as_str() {
                    "downloads" => 0x4CC2FF,
                    "pictures" => 0xB4A0FF,
                    "videos" => 0x60CDFF,
                    "music" => 0xFF8C00,
                    _ => 0xF5B942,
                }
            }
        }
    }

    fn render_section_header(
        section: &SidebarSectionData,
        section_idx: usize,
        state: &SidebarState,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let title = section.title.clone();
        let section_id = section.id;
        let reorder_mode = state.reorder_mode;
        let is_dragging_this = state.drag_section_idx == Some(section_idx);
        let collapsed = section.collapsed;

        // Chevron (Fluent convention: down = expanded, up = collapsed) +
        // colored section glyph, replacing the old right-aligned count badge.
        let (glyph, glyph_color) = Self::section_glyph(section_id);
        let chevron = if collapsed { "\u{E76C}" } else { "\u{E70D}" };

        let mut header = div()
            .id(SharedString::from(format!(
                "sidebar-section-header-{}",
                section_idx
            )))
            .h(px(SECTION_HEADER_HEIGHT))
            .flex()
            .items_center()
            .gap(px(6.0))
            .px(px(16.0))
            .child(
                div()
                    .w(px(16.0))
                    .text_size(px(10.0))
                    .font_family(UiConstants::FONT_ICONS)
                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                    .child(chevron.to_string()),
            )
            .child(
                div()
                    .w(px(18.0))
                    .text_size(px(14.0))
                    .font_family(UiConstants::FONT_ICONS)
                    .text_color(rgb(glyph_color))
                    .child(glyph.to_string()),
            )
            .child(
                div()
                    .text_size(px(12.0))
                    .font_weight(FontWeight(600.0))
                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                    .child(title.to_string()),
            );

        if reorder_mode {
            header = header
                .cursor_row_resize()
                .bg(if is_dragging_this {
                    rgba(0xFFFFFF20u32)
                } else {
                    rgba(0x00000000u32)
                })
                .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
                .on_mouse_down(MouseButton::Left, {
                    let view = view.clone();
                    move |ev: &MouseDownEvent, _win, cx| {
                        cx.stop_propagation();
                        let y: f32 = ev.position.y.into();
                        view.update(cx, |this, cx| {
                            this.state.sidebar.drag_section_current_y = y;
                            this.state.sidebar.start_section_drag(section_idx);
                            cx.notify();
                        });
                    }
                })
                .on_mouse_move({
                    let view = view.clone();
                    move |_, _, cx| {
                        view.update(cx, |this, cx| {
                            if let Some(dragged_idx) = this.state.sidebar.drag_section_idx {
                                if dragged_idx != section_idx {
                                    this.state.sidebar.swap_sections(dragged_idx, section_idx);
                                    this.state.sidebar.drag_section_idx = Some(section_idx);
                                    cx.notify();
                                }
                            }
                        });
                    }
                });
        } else {
            header = header
                .cursor_pointer()
                .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
                .on_mouse_down(MouseButton::Left, {
                    let view = view.clone();
                    move |_ev: &MouseDownEvent, _win, cx| {
                        cx.stop_propagation();
                        view.update(cx, |this, cx| {
                            this.state.sidebar.toggle_section(section_id);
                            cx.notify();
                        });
                    }
                });
        }

        // Right-click context menu on group header
        header = header.on_mouse_down(MouseButton::Right, {
            let view = view.clone();
            move |event, win, cx| {
                cx.stop_propagation();
                let position = event.position;
                let flip_up: f32 = position.y.into();
                let win_h: f32 = win.viewport_size().height.into();
                let bottom_y = if flip_up > (win_h - SIDEBAR_MENU_HEIGHT) {
                    win_h - flip_up
                } else {
                    0.0
                };
                view.update(cx, |this, cx| {
                    let reorder_mode = this.state.sidebar.reorder_mode;
                    let hidden_sections = this.state.sidebar.hidden_sections.clone();
                    this.state.active_dialog = Some(DialogType::SidebarContextMenu {
                        target: crate::ui::components::dialogs::SidebarMenuTarget::Group {
                            section_id,
                        },
                        position,
                        reorder_mode,
                        hidden_sections,
                        bottom_y,
                    });
                    cx.notify();
                });
            }
        });

        header
    }

    // ─── Collapsed section (icon only) ────────────────────────────────────

    fn render_collapsed_section(
        section: &SidebarSectionData,
        section_idx: usize,
        state: &SidebarState,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let section_id = section.id;
        let icon = section_icon(section.id);
        let is_tooltip_open = state.tooltip_section == Some(section_id);

        let mut collapsed_sec = div()
            .id(SharedString::from(format!(
                "sidebar-section-collapsed-{}",
                section_idx
            )))
            .h(px(COLLAPSED_SECTION_ICON_SIZE))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(4.0))
            .mx(px(6.0))
            .bg(if is_tooltip_open {
                rgba(UiConstants::BG_ITEM_SELECTED_RGBA)
            } else {
                rgba(0x00000000u32)
            })
            .child(
                div()
                    .text_size(px(16.0))
                    .font_family(UiConstants::FONT_ICONS)
                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                    .child(icon),
            );

        if state.reorder_mode {
            collapsed_sec = collapsed_sec.opacity(0.4).cursor_not_allowed();
        } else {
            collapsed_sec = collapsed_sec
                .cursor_pointer()
                .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
                .on_mouse_down(MouseButton::Left, {
                    let view = view.clone();
                    move |_ev, _win, cx| {
                        view.update(cx, |this, cx| {
                            let state = &mut this.state.sidebar;
                            if state.tooltip_section == Some(section_id) {
                                state.set_tooltip(None);
                            } else {
                                state.set_tooltip(Some(section_id));
                            }
                            cx.notify();
                        });
                    }
                });
        }

        // Right-click context menu on collapsed section
        collapsed_sec = collapsed_sec.on_mouse_down(MouseButton::Right, {
            let view = view.clone();
            move |event, win, cx| {
                cx.stop_propagation();
                let position = event.position;
                let flip_up: f32 = position.y.into();
                let win_h: f32 = win.viewport_size().height.into();
                let bottom_y = if flip_up > (win_h - SIDEBAR_MENU_HEIGHT) {
                    win_h - flip_up
                } else {
                    0.0
                };
                view.update(cx, |this, cx| {
                    let reorder_mode = this.state.sidebar.reorder_mode;
                    let hidden_sections = this.state.sidebar.hidden_sections.clone();
                    this.state.active_dialog = Some(DialogType::SidebarContextMenu {
                        target: crate::ui::components::dialogs::SidebarMenuTarget::Group {
                            section_id,
                        },
                        position,
                        reorder_mode,
                        hidden_sections,
                        bottom_y,
                    });
                    cx.notify();
                });
            }
        });

        collapsed_sec
    }

    // ─── Tooltip / flyout for collapsed mode ──────────────────────────────

    pub fn render_tooltip(
        section: &SidebarSectionData,
        top_offset: f32,
        sidebar_width: Pixels,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let v_close = view.clone();
        let tooltip_left = sidebar_width + px(4.0);

        div()
            .id("sidebar-tooltip")
            .absolute()
            .left(tooltip_left)
            .top(px(top_offset.max(0.0)))
            .w(px(TOOLTIP_WIDTH))
            .bg(rgb(0x2C2C2C)) // Slightly lighter than sidebar
            .rounded(px(8.0))
            .border(px(1.0))
            .border_color(rgba(UiConstants::DIVIDER_RGBA))
            .shadow_lg()
            .flex_col()
            .p(px(8.0))
            // Tooltip header
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px(px(8.0))
                    .py(px(6.0))
                    .border_b(px(1.0))
                    .border_color(rgba(UiConstants::DIVIDER_RGBA))
                    .child(
                        div()
                            .text_size(px(12.0))
                            .font_weight(FontWeight(600.0))
                            .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                            .child(section.title.to_string()),
                    )
                    .child(
                        div()
                            .cursor_pointer()
                            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                v_close.update(cx, |this, _cx| {
                                    this.state.sidebar.set_tooltip(None);
                                });
                            })
                            .child(
                                div()
                                    .text_size(px(14.0))
                                    .font_family(UiConstants::FONT_MDL2)
                                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                    .child("\u{E8BB}"), // Close X
                            ),
                    ),
            )
            // Tooltip items
            .children(
                section
                    .items
                    .iter()
                    .enumerate()
                    .map(|(idx, item)| Self::render_tooltip_item(item, idx, view.clone())),
            )
    }

    fn render_tooltip_item(
        item: &SidebarItem,
        idx: usize,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let icon_src = item.icon.clone();
        let label = item.label.clone();
        let item_id = item.id.clone();
        let v_click = view.clone();

        div()
            .id(SharedString::from(format!("sidebar-tooltip-item-{}", idx)))
            .h(px(32.0))
            .flex()
            .items_center()
            .px(px(8.0))
            .gap(px(10.0))
            .cursor_pointer()
            .rounded(px(4.0))
            .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
            .on_mouse_down(MouseButton::Left, {
                let item_id = item_id.clone();
                move |_event, _window, cx| {
                    if let Some(path) = item_id.get_path() {
                        v_click.update(cx, |this, cx| {
                            this.navigate(path, cx);
                            // Close tooltip after navigation
                            this.state.sidebar.set_tooltip(None);
                        });
                    }
                }
            })
            .child(
                div()
                    .w(px(18.0))
                    .h(px(18.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(14.0))
                    .font_family(UiConstants::FONT_ICONS)
                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                    .child(icon_src),
            )
            .child(
                div()
                    .flex_grow()
                    .overflow_hidden()
                    .text_size(px(13.0))
                    .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                    .child(label.to_string()),
            )
    }

    // ─── Normal item rendering (expanded mode) ────────────────────────────

    fn render_item(
        item: &SidebarItem,
        idx: usize,
        section: &SidebarSectionData,
        state: &SidebarState,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let icon_src = item.icon.clone();
        let label = item.label.clone();
        let item_id = item.id.clone();
        let v_click = view.clone();
        let is_active = state
            .active_item
            .as_ref()
            .map_or(false, |id| *id == item_id);
        let reorder_mode = state.reorder_mode;

        // Pinned locations and Tags are draggable/reorderable!
        let is_draggable = section.id == SectionId::Favorites || section.id == SectionId::Tags;

        let is_dragging_this = state.drag_item == Some((section.id, idx));

        let mut item_el = div()
            .id(SharedString::from(format!(
                "sidebar-item-{:?}-{}",
                section.id, idx
            )))
            .h(px(SECTION_ITEM_HEIGHT))
            .mx(px(8.0))
            .flex()
            .items_center()
            .px(px(8.0))
            .gap(px(12.0))
            .rounded(px(4.0))
            .child(
                div()
                    .w(px(if is_active { 3.0 } else { 0.0 }))
                    .h(px(16.0))
                    .bg(rgb(UiConstants::ACCENT_COLOR_RGB))
                    .rounded(px(2.0))
                    .absolute()
                    .left(px(0.0)),
            )
            .child(
                div()
                    .w(px(20.0))
                    .h(px(20.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(16.0))
                    .font_family(UiConstants::FONT_ICONS)
                    .text_color(if is_active {
                        rgb(UiConstants::ACCENT_COLOR_RGB)
                    } else {
                        rgb(Self::item_icon_color(item))
                    })
                    .child(icon_src),
            )
            .child(
                div()
                    .flex_grow()
                    .overflow_hidden()
                    .text_size(px(UiConstants::ITEM_FONT_SIZE_RAW as f32))
                    .font_weight(if is_active {
                        FontWeight(600.0)
                    } else {
                        FontWeight::default()
                    })
                    .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                    .child(label.to_string()),
            )
            .children(item.badge.as_ref().map(|badge| Self::render_badge(badge)));

        if reorder_mode {
            if is_draggable {
                // Draggable: grab cursor, swap on mouse enter, start drag on mouse down
                item_el = item_el
                    .cursor_row_resize()
                    .bg(if is_dragging_this {
                        rgba(0xFFFFFF20u32)
                    } else if is_active {
                        rgba(UiConstants::BG_ITEM_SELECTED_RGBA)
                    } else {
                        rgba(0x00000000u32)
                    })
                    .hover(|style| {
                        style.bg(if is_active {
                            rgba(UiConstants::BG_ITEM_SELECTED_HOVER_RGBA)
                        } else {
                            rgba(UiConstants::BG_ITEM_HOVER_RGBA)
                        })
                    })
                    .on_mouse_down(MouseButton::Left, {
                        let view = view.clone();
                        let section_id = section.id;
                        move |ev, win, cx| {
                            cx.stop_propagation();
                            view.update(cx, |this, cx| {
                                this.state.sidebar.start_item_drag(section_id, idx);
                                cx.notify();
                            });
                        }
                    })
                    .on_mouse_move({
                        let view = view.clone();
                        let section_id = section.id;
                        move |_, _, cx| {
                            view.update(cx, |this, cx| {
                                if let Some((drag_sect, drag_idx)) = this.state.sidebar.drag_item {
                                    if drag_sect == section_id && drag_idx != idx {
                                        this.state.sidebar.swap_items(section_id, drag_idx, idx);
                                        this.state.sidebar.drag_item = Some((section_id, idx));
                                        cx.notify();
                                    }
                                }
                            });
                        }
                    });
            } else {
                // Non-draggable item during reorder phase: greyish and non-interactable
                item_el = item_el.opacity(0.4).cursor_not_allowed();
            }
        } else {
            // Normal interaction mode
            item_el = item_el
                .cursor_pointer()
                .bg(if is_active {
                    rgba(UiConstants::BG_ITEM_SELECTED_RGBA)
                } else {
                    rgba(0x00000000u32)
                })
                .hover(|style| {
                    style.bg(if is_active {
                        rgba(UiConstants::BG_ITEM_SELECTED_HOVER_RGBA)
                    } else {
                        rgba(UiConstants::BG_ITEM_HOVER_RGBA)
                    })
                })
                .on_mouse_down(MouseButton::Left, {
                    let item_id = item_id.clone();
                    move |_event, _window, cx| {
                        cx.stop_propagation();
                        if let Some(path) = item_id.get_path() {
                            v_click.update(cx, |this, cx| {
                                this.navigate(path, cx);
                            });
                        }
                    }
                });
        }

        // Right click context menu on item (always available outside and inside reorder mode)
        let is_pinned = matches!(item_id, SidebarItemId::Pinned(_));
        let path_str = item_id.get_path().map(|p| p.to_string_lossy().into_owned());
        item_el = item_el.on_mouse_down(MouseButton::Right, {
            let view = view.clone();
            let label = label.clone();
            move |event, win, cx| {
                cx.stop_propagation();
                let position = event.position;
                let flip_up: f32 = position.y.into();
                let win_h: f32 = win.viewport_size().height.into();
                let bottom_y = if flip_up > (win_h - SIDEBAR_MENU_HEIGHT) {
                    win_h - flip_up
                } else {
                    0.0
                };
                view.update(cx, |this, cx| {
                    let reorder_mode = this.state.sidebar.reorder_mode;
                    let hidden_sections = this.state.sidebar.hidden_sections.clone();
                    this.state.active_dialog = Some(DialogType::SidebarContextMenu {
                        target: crate::ui::components::dialogs::SidebarMenuTarget::Item {
                            label: label.clone(),
                            path: path_str.clone(),
                            is_pinned,
                        },
                        position,
                        reorder_mode,
                        hidden_sections,
                        bottom_y,
                    });
                    cx.notify();
                });
            }
        });

        item_el
    }

    fn render_badge(badge: &SidebarBadge) -> impl IntoElement {
        match badge {
            SidebarBadge::Count(n) => div()
                .min_w(px(20.0))
                .h(px(18.0))
                .flex()
                .items_center()
                .justify_center()
                .px(px(6.0))
                .rounded(px(10.0))
                .bg(rgba(UiConstants::BG_BADGE_RGBA))
                .child(
                    div()
                        .text_size(px(10.0))
                        .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                        .child(n.to_string()),
                ),
            SidebarBadge::Usage(ratio) => {
                let fill_width = 36.0 * (*ratio).max(0.0).min(1.0) as f32;
                let fill_color = if *ratio > 0.9 {
                    UiConstants::DANGER_RGBA
                } else {
                    UiConstants::ACCENT_RGBA
                };
                div()
                    .w(px(36.0))
                    .h(px(4.0))
                    .rounded(px(2.0))
                    .bg(rgba(UiConstants::BG_BADGE_RGBA))
                    .child(
                        div()
                            .h_full()
                            .w(px(fill_width))
                            .rounded(px(2.0))
                            .bg(rgba(fill_color)),
                    )
            }
        }
    }

    // ─── Settings button ──────────────────────────────────────────────────

    fn render_settings_button(
        collapsed: bool,
        state: &SidebarState,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let v = view.clone();
        let reorder_mode = state.reorder_mode;
        let mut btn = div()
            .id("sidebar-settings")
            .h(px(40.0))
            .w_full()
            .flex()
            .items_center()
            .gap(px(8.0))
            .px(px(if collapsed { 0.0 } else { 12.0 }))
            .justify_center()
            .border_t(px(1.0))
            .border_color(rgba(UiConstants::DIVIDER_RGBA))
            .child(
                div()
                    .text_size(px(14.0))
                    .font_family(UiConstants::FONT_MDL2)
                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                    .child("\u{E713}"),
            );

        if !collapsed {
            btn = btn.child(
                div()
                    .text_size(px(UiConstants::ITEM_FONT_SIZE_RAW as f32))
                    .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                    .child("Settings"),
            );
        }

        if reorder_mode {
            btn = btn.opacity(0.4).cursor_not_allowed();
        } else {
            btn = btn
                .cursor_pointer()
                .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
                .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
                    v.update(cx, |this, cx| {
                        this.open_settings_tab(cx);
                    });
                });
        }

        btn
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// SidebarSplitter
// ─────────────────────────────────────────────────────────────────────────────

pub struct SidebarSplitter;

impl SidebarSplitter {
    pub fn build(sidebar: &SidebarState, view: Entity<crate::app::AppWindow>) -> impl IntoElement {
        let is_resizing = sidebar.resize_start_x.is_some();

        div()
            .id("sidebar-splitter")
            .w(px(4.0))
            .h_full()
            .cursor_col_resize()
            .bg(if is_resizing {
                rgba(UiConstants::ACCENT_RGBA)
            } else {
                rgba(UiConstants::BG_TRANSPARENT_RGBA)
            })
            .hover(|s| s.bg(rgba(0xFFFFFF11u32)))
            .on_mouse_down(MouseButton::Left, {
                let view = view.clone();
                move |event, _win, cx| {
                    let x: f32 = event.position.x.into();
                    view.update(cx, |this, _cx| {
                        this.state.sidebar.start_resize_drag(x);
                    });
                }
            })
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

fn section_height(section: &SidebarSectionData) -> f32 {
    let items_height = if section.collapsed {
        0.0
    } else {
        section.items.len() as f32 * SECTION_ITEM_HEIGHT
    };
    SECTION_HEADER_HEIGHT + items_height + SECTION_MARGIN_BOTTOM
}

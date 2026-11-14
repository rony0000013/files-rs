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
use crate::app_state::SidebarTooltipData;
use crate::core::path_util::FilesPath;
use crate::ui::components::dialogs::DialogType;
use crate::ui::constants::UiConstants;
use gpui::*;

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
        app_state: &AppState,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let collapsed = state.collapsed;
        let reorder_mode = state.reorder_mode;
        let active_view = app_state.active_view();
        let is_home_active = matches!(
            active_view.target,
            crate::ui::views::types::ViewTarget::Home
        );
        let active_path: Option<FilesPath> = match &active_view.target {
            crate::ui::views::types::ViewTarget::Directory(p) => Some(p.clone()),
            _ => None,
        };
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
                    let h = section_height(section, state);
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

        let mut sidebar_el = div()
            .id("sidebar")
            .w(width)
            .h_full()
            .pt(px(4.0))
            .flex()
            .flex_col()
            .flex_shrink_0() // Prevent sidebar from shrinking when content needs space
            .overflow_hidden() // Clip content to explicit width
            .bg(rgba(UiConstants::BG_TRANSPARENT_RGBA))
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
            // Home item (top of navigation pane; Hamburger toggle is in NavigationToolbar per Files-community)
            .child(Self::render_home_item(
                collapsed,
                is_home_active,
                state,
                view.clone(),
            ))
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
                            cx.stop_propagation();
                            let line_height = px(20.0);
                            let delta = event.delta.pixel_delta(line_height);
                            let delta_y: f32 = delta.y.into();

                            view.update(cx, |this, cx| {
                                let sidebar = &mut this.state.sidebar;
                                let content_h = compute_sidebar_content_height(sidebar);
                                let b_h: f32 = window.bounds().size.height.into();
                                let vp_h: f32 = window.viewport_size().height.into();
                                let win_h = if b_h > 1.0 { b_h } else { vp_h };
                                let viewport_h = compute_sidebar_viewport_height(win_h, sidebar.collapsed);

                                let bs = &mut sidebar.bouncy_scroll;
                                bs.update_bounds(content_h, viewport_h);
                                if !bs.can_scroll() {
                                    return;
                                }
                                bs.apply_scroll_delta(delta_y);
                                if bs.is_active {
                                    bs.animating = true;
                                    cx.notify();
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
                                .pb(px(UiConstants::PADDING_BASE as f32))
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
                                                Self::render_section(
                                                    section,
                                                    idx,
                                                    state,
                                                    active_path.as_ref(),
                                                    view.clone(),
                                                )
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
                move |_, _win, cx| {
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

    // ─── Home item ────────────────────────────────────────────────────────

    fn render_active_indicator(is_active: bool) -> Option<AnyElement> {
        if is_active {
            Some(
                div()
                    .absolute()
                    .left(px(0.0))
                    .top(px(7.0))
                    .w(px(3.0))
                    .h(px(16.0))
                    .rounded(px(1.5))
                    .bg(rgb(UiConstants::ACCENT_COLOR_RGB))
                    .into_any_element(),
            )
        } else {
            None
        }
    }

    fn render_home_icon(is_active: bool) -> AnyElement {
        if let Some(path) = bundled_icon_path("sidebar_home.png") {
            gpui::img(path)
                .w(px(16.0))
                .h(px(16.0))
                .object_fit(gpui::ObjectFit::Contain)
                .into_any_element()
        } else {
            div()
                .text_size(px(16.0))
                .font_family(UiConstants::FONT_MDL2)
                .text_color(if is_active {
                    rgb(0x60CDFF)
                } else {
                    rgba(UiConstants::TEXT_SECONDARY_RGBA)
                })
                .child("\u{E10F}")
                .into_any_element()
        }
    }

    fn render_home_item(
        collapsed: bool,
        is_active: bool,
        state: &SidebarState,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let v = view.clone();
        let reorder_mode = state.reorder_mode;

        let mut item = if collapsed {
            // Just the icon, no text
            div()
                .id("sidebar-home")
                .h(px(COLLAPSED_SECTION_ICON_SIZE))
                .mx(px(6.0))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(4.0))
                .bg(if is_active {
                    rgba(UiConstants::BG_ITEM_SELECTED_RGBA)
                } else {
                    rgba(0x00000000u32)
                })
                .child(Self::render_home_icon(is_active))
        } else {
            // Icon + text + WinUI 3 3px active selection indicator pill
            div()
                .id("sidebar-home")
                .relative()
                .h(px(SECTION_ITEM_HEIGHT))
                .mx(px(8.0))
                .flex()
                .items_center()
                .px(px(8.0))
                .gap(px(12.0))
                .rounded(px(4.0))
                .bg(if is_active {
                    rgba(UiConstants::BG_ITEM_SELECTED_RGBA)
                } else {
                    rgba(0x00000000u32)
                })
                .children(Self::render_active_indicator(is_active))
                .child(
                    div()
                        .w(px(20.0))
                        .h(px(20.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .flex_shrink_0()
                        .child(Self::render_home_icon(is_active)),
                )
                .child(
                    div()
                        .flex_grow()
                        .min_w(px(0.0))
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .text_size(px(UiConstants::ITEM_FONT_SIZE_RAW as f32))
                        .font_weight(if is_active {
                            FontWeight(600.0)
                        } else {
                            FontWeight(400.0)
                        })
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
                        this.stop_sidebar_tooltip(cx);
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
        active_path: Option<&FilesPath>,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let collapsed = section.collapsed;
        let children_els: Vec<AnyElement> = if collapsed {
            Vec::new()
        } else {
            let mut rows = Vec::new();
            for (idx, item) in section.items.iter().enumerate() {
                rows.push(
                    Self::render_item(item, idx, section, state, active_path, view.clone())
                        .into_any_element(),
                );
                if let Some(exp_path) = SidebarState::item_expandable_path(item) {
                    for (child_idx, (depth, child)) in state
                        .visible_tree_rows_for_root(exp_path)
                        .into_iter()
                        .enumerate()
                    {
                        rows.push(
                            Self::render_tree_child(
                                section.id,
                                idx,
                                child_idx,
                                depth,
                                &child,
                                state,
                                active_path,
                                view.clone(),
                            )
                            .into_any_element(),
                        );
                    }
                }
            }
            rows
        };

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
                view,
            ))
            .children(children_els)
    }

    /// Bundled 16x16 PNG filename for each sidebar section header.
    fn section_icon_name(section_id: SectionId) -> &'static str {
        match section_id {
            SectionId::Favorites => "sidebar_pinned.png",
            SectionId::ThisPc => "sidebar_drives.png",
            SectionId::CloudDrives => "sidebar_cloud.png",
            SectionId::Network => "sidebar_network.png",
            SectionId::Wsl => "sidebar_wsl.png",
            SectionId::Tags => "sidebar_tags.png",
            SectionId::Libraries => "sidebar_libraries.png",
        }
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

        // Chevron (Fluent convention: down = expanded, right = collapsed) +
        // bundled section PNG icon with colored Fluent glyph fallback.
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
                    .h(px(16.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .flex_shrink_0()
                    .text_size(px(10.0))
                    .font_family(UiConstants::FONT_ICONS)
                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                    .child(chevron.to_string()),
            )
            .child(
                div()
                    .w(px(18.0))
                    .h(px(18.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .flex_shrink_0()
                    .child(
                        if let Some(path) = bundled_icon_path(Self::section_icon_name(section_id)) {
                            gpui::img(path)
                                .w(px(16.0))
                                .h(px(16.0))
                                .object_fit(gpui::ObjectFit::Contain)
                                .into_any_element()
                        } else {
                            div()
                                .text_size(px(14.0))
                                .font_family(UiConstants::FONT_ICONS)
                                .text_color(rgb(glyph_color))
                                .child(glyph.to_string())
                                .into_any_element()
                        },
                    ),
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
                            this.stop_sidebar_tooltip(cx);
                            this.state.sidebar.toggle_section(section_id);
                            let ch = compute_sidebar_content_height(&this.state.sidebar);
                            let vh = this.state.sidebar.bouncy_scroll.viewport_height.max(100.0);
                            this.state.sidebar.bouncy_scroll.update_bounds(ch, vh);
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
                    this.stop_sidebar_tooltip(cx);
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
                if let Some(path) = bundled_icon_path(Self::section_icon_name(section_id)) {
                    gpui::img(path)
                        .w(px(16.0))
                        .h(px(16.0))
                        .object_fit(gpui::ObjectFit::Contain)
                        .into_any_element()
                } else {
                    div()
                        .text_size(px(16.0))
                        .font_family(UiConstants::FONT_ICONS)
                        .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                        .child(icon)
                        .into_any_element()
                },
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
            .bg(rgba(UiConstants::BG_FLYOUT_MICA_RGBA))
            .rounded(px(8.0))
            .border(px(1.0))
            .border_color(rgba(0xFFFFFF18u32))
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

    fn render_tree_chevron(
        id: SharedString,
        expanded: bool,
        interactive: bool,
        path: FilesPath,
        view: Entity<crate::app::AppWindow>,
    ) -> AnyElement {
        let mut chev = div()
            .id(id)
            .w(px(16.0))
            .h(px(16.0))
            .flex_shrink_0()
            .rounded(px(4.0))
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .text_size(px(10.0))
                    .font_family(UiConstants::FONT_ICONS)
                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                    .child(if expanded { "\u{E70D}" } else { "\u{E76C}" }),
            );

        if interactive {
            chev = chev
                .cursor_pointer()
                .hover(|s| s.bg(rgba(0xFFFFFF1Cu32)))
                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                    cx.stop_propagation();
                    let p = path.clone();
                    view.update(cx, |this, cx| {
                        this.stop_sidebar_tooltip(cx);
                        this.toggle_sidebar_item_expand(p, cx);
                    });
                });
        }

        chev.into_any_element()
    }

    fn render_item_icon(
        item: &SidebarItem,
        exp_path: Option<&FilesPath>,
        is_active: bool,
        state: &SidebarState,
    ) -> AnyElement {
        if let Some(png_path) = exp_path.and_then(|p| state.shell_icons.get(p)) {
            return gpui::img(png_path.clone())
                .w(px(16.0))
                .h(px(16.0))
                .object_fit(gpui::ObjectFit::Contain)
                .into_any_element();
        }
        if matches!(item.id, SidebarItemId::Pinned(_) | SidebarItemId::Library(_)) {
            if let Some(folder_png) = bundled_icon_path("folder_icon.png") {
                return gpui::img(folder_png)
                    .w(px(16.0))
                    .h(px(16.0))
                    .object_fit(gpui::ObjectFit::Contain)
                    .into_any_element();
            }
        }
        div()
            .text_size(px(16.0))
            .font_family(UiConstants::FONT_ICONS)
            .text_color(if is_active {
                rgb(UiConstants::ACCENT_COLOR_RGB)
            } else {
                rgb(Self::item_icon_color(item))
            })
            .child(item.icon.clone())
            .into_any_element()
    }

    fn render_tree_child_icon(
        path: &FilesPath,
        is_active: bool,
        state: &SidebarState,
    ) -> AnyElement {
        if let Some(png_path) = state.shell_icons.get(path) {
            return gpui::img(png_path.clone())
                .w(px(16.0))
                .h(px(16.0))
                .object_fit(gpui::ObjectFit::Contain)
                .into_any_element();
        }
        if let Some(folder_png) = bundled_icon_path("folder_icon.png") {
            return gpui::img(folder_png)
                .w(px(16.0))
                .h(px(16.0))
                .object_fit(gpui::ObjectFit::Contain)
                .into_any_element();
        }
        div()
            .text_size(px(15.0))
            .font_family(UiConstants::FONT_ICONS)
            .text_color(if is_active {
                rgb(UiConstants::ACCENT_COLOR_RGB)
            } else {
                rgb(0xF5B942)
            })
            .child("\u{E8B7}")
            .into_any_element()
    }

    fn item_tooltip_data(item: &SidebarItem, state: &SidebarState) -> Option<SidebarTooltipData> {
        let pos = Point::default();
        match &item.id {
            SidebarItemId::Drive { root, .. } => {
                if let Some(meta) = state.drive_meta_by_root.get(root) {
                    Some(SidebarTooltipData {
                        title: meta.display_name.clone(),
                        subtitle: Some(format!(
                            "File system: {} • {:.1}% used",
                            meta.file_system, meta.usage_percent
                        )),
                        usage_percent: Some(meta.usage_percent),
                        detail_line: Some(meta.capacity_summary.clone()),
                        position: pos,
                    })
                } else {
                    Some(SidebarTooltipData {
                        title: item.label.clone(),
                        subtitle: Some(root.to_string_lossy().to_string()),
                        usage_percent: None,
                        detail_line: None,
                        position: pos,
                    })
                }
            }
            SidebarItemId::Pinned(path) => Some(SidebarTooltipData {
                title: item.label.clone(),
                subtitle: Some(path.to_string_lossy().to_string()),
                usage_percent: None,
                detail_line: None,
                position: pos,
            }),
            SidebarItemId::Library(_) => Some(SidebarTooltipData {
                title: item.label.clone(),
                subtitle: item.id.get_path().map(|p| p.to_string_lossy().to_string()),
                usage_percent: None,
                detail_line: None,
                position: pos,
            }),
            SidebarItemId::CloudDrive(_) => Some(SidebarTooltipData {
                title: item.label.clone(),
                subtitle: Some("Cloud Drive".to_string()),
                usage_percent: None,
                detail_line: None,
                position: pos,
            }),
            SidebarItemId::Network => Some(SidebarTooltipData {
                title: item.label.clone(),
                subtitle: Some("Network Locations".to_string()),
                usage_percent: None,
                detail_line: None,
                position: pos,
            }),
            SidebarItemId::WslDistro(distro) => Some(SidebarTooltipData {
                title: item.label.clone(),
                subtitle: Some(format!(r"\\wsl$\{}", distro)),
                usage_percent: None,
                detail_line: None,
                position: pos,
            }),
            SidebarItemId::Tag(_) | SidebarItemId::RecycleBin => None,
        }
    }

    fn tree_child_tooltip_data(child: &SidebarTreeChild) -> SidebarTooltipData {
        SidebarTooltipData {
            title: child.label.clone(),
            subtitle: Some(child.path.to_string_lossy().to_string()),
            usage_percent: None,
            detail_line: None,
            position: Point::default(),
        }
    }

    fn attach_sidebar_tooltip(
        el: Stateful<Div>,
        tip_template: SidebarTooltipData,
        view: Entity<crate::app::AppWindow>,
    ) -> Stateful<Div> {
        el.on_hover(move |hovered: &bool, window: &mut Window, cx: &mut App| {
            let pos = window.mouse_position();
            if *hovered {
                let mut data = tip_template.clone();
                data.position = pos;
                view.update(cx, |this, cx| {
                    this.start_sidebar_tooltip(data, cx);
                });
            } else {
                view.update(cx, |this, cx| {
                    this.stop_sidebar_tooltip(cx);
                });
            }
        })
    }

    fn attach_file_drag_handlers(
        el: Stateful<Div>,
        path: FilesPath,
        label: String,
        view: Entity<crate::app::AppWindow>,
    ) -> Stateful<Div> {
        let v_move = view.clone();
        let path_move = path.clone();
        let label_move = label.clone();
        el.on_mouse_move(move |ev, _win, cx| {
            let pos = gpui::point(ev.position.x.into(), ev.position.y.into());
            let p = path_move.clone();
            let l = label_move.clone();
            v_move.update(cx, |this, cx| {
                if this.state.main_view.drag_state.is_dragging
                    || this.state.main_view.drag_state.pending_candidate.is_some()
                {
                    this.update_file_drag_move(pos, ev.modifiers, Some((p, l)), cx);
                }
            });
        })
        .on_mouse_up(MouseButton::Left, move |ev, _win, cx| {
            let pos = gpui::point(ev.position.x.into(), ev.position.y.into());
            let p = path.clone();
            let l = label.clone();
            view.update(cx, |this, cx| {
                if this.state.main_view.drag_state.is_dragging {
                    tracing::info!(
                        "drop: sidebar-up row={} target={:?}",
                        l,
                        p.to_string_lossy().to_string()
                    );
                    this.end_drag(pos.x, pos.y, Some((p, l)), cx);
                }
            });
        })
    }

    fn attach_item_context_menu(
        el: Stateful<Div>,
        label: String,
        path_str: Option<String>,
        is_pinned: bool,
        view: Entity<crate::app::AppWindow>,
    ) -> Stateful<Div> {
        el.on_mouse_down(MouseButton::Right, move |event, win, cx| {
            cx.stop_propagation();
            let position = event.position;
            let flip_up: f32 = position.y.into();
            let win_h: f32 = win.viewport_size().height.into();
            let bottom_y = if flip_up > (win_h - SIDEBAR_MENU_HEIGHT) {
                win_h - flip_up
            } else {
                0.0
            };
            let label_cloned = label.clone();
            let path_cloned = path_str.clone();
            view.update(cx, |this, cx| {
                this.stop_sidebar_tooltip(cx);
                let reorder_mode = this.state.sidebar.reorder_mode;
                let hidden_sections = this.state.sidebar.hidden_sections.clone();
                this.state.active_dialog = Some(DialogType::SidebarContextMenu {
                    target: crate::ui::components::dialogs::SidebarMenuTarget::Item {
                        label: label_cloned,
                        path: path_cloned,
                        is_pinned,
                    },
                    position,
                    reorder_mode,
                    hidden_sections,
                    bottom_y,
                });
                cx.notify();
            });
        })
    }

    fn render_item(
        item: &SidebarItem,
        idx: usize,
        section: &SidebarSectionData,
        state: &SidebarState,
        active_path: Option<&FilesPath>,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let label = item.label.clone();
        let item_id = item.id.clone();
        let expandable_path = SidebarState::item_expandable_path(item).cloned();
        let is_active = state.active_item.as_ref().is_some_and(|id| *id == item_id)
            || (state.active_item.is_none()
                && active_path.is_some_and(|ap| {
                    item_id
                        .get_path()
                        .as_ref()
                        .is_some_and(|ip| paths_match(ap, ip))
                }));
        let reorder_mode = state.reorder_mode;

        // Pinned locations and Tags are draggable/reorderable!
        let is_draggable = section.id == SectionId::Favorites || section.id == SectionId::Tags;
        let is_dragging_this = state.drag_item == Some((section.id, idx));

        let mut item_el = div()
            .id(SharedString::from(format!(
                "sidebar-item-{:?}-{}",
                section.id, idx
            )))
            .relative()
            .h(px(SECTION_ITEM_HEIGHT))
            .mx(px(8.0))
            .flex()
            .items_center()
            .px(px(8.0))
            .gap(px(6.0))
            .rounded(px(4.0))
            .children(Self::render_active_indicator(is_active))
            .child(if let Some(ref exp_path) = expandable_path {
                Self::render_tree_chevron(
                    SharedString::from(format!("sidebar-item-chevron-{:?}-{}", section.id, idx)),
                    state.is_path_expanded(exp_path),
                    !reorder_mode,
                    exp_path.clone(),
                    view.clone(),
                )
            } else {
                div().w(px(16.0)).h(px(16.0)).flex_shrink_0().into_any_element()
            })
            .child(
                div()
                    .w(px(18.0))
                    .h(px(18.0))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(Self::render_item_icon(
                        item,
                        expandable_path.as_ref(),
                        is_active,
                        state,
                    )),
            )
            .child(
                div()
                    .flex_grow()
                    .min_w(px(0.0))
                    .ml(px(2.0))
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .text_size(px(UiConstants::ITEM_FONT_SIZE_RAW as f32))
                    .font_weight(if is_active {
                        FontWeight(600.0)
                    } else {
                        FontWeight::default()
                    })
                    .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                    .child(label.to_string()),
            )
            .children(if matches!(item_id, SidebarItemId::Drive { .. }) {
                None
            } else {
                item.badge.as_ref().and_then(Self::render_badge)
            });

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
                        move |_ev, _win, cx| {
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
            let v_click = view.clone();
            let nav_item_id = item_id.clone();
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
                .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
                    cx.stop_propagation();
                    if let Some(path) = nav_item_id.get_path() {
                        v_click.update(cx, |this, cx| {
                            this.stop_sidebar_tooltip(cx);
                            this.navigate(path, cx);
                        });
                    }
                });

            if let Some(tip_data) = Self::item_tooltip_data(item, state) {
                item_el = Self::attach_sidebar_tooltip(item_el, tip_data, view.clone());
            }

            if let Some(path) = item_id.get_path() {
                item_el = Self::attach_file_drag_handlers(
                    item_el,
                    path,
                    label.clone(),
                    view.clone(),
                );
            }
        }

        let is_pinned = matches!(item_id, SidebarItemId::Pinned(_));
        let path_str = item_id.get_path().map(|p| p.to_string_lossy().into_owned());
        Self::attach_item_context_menu(item_el, label, path_str, is_pinned, view)
    }

    #[allow(clippy::too_many_arguments)]
    fn render_tree_child(
        section_id: SectionId,
        parent_idx: usize,
        child_idx: usize,
        depth: usize,
        child: &SidebarTreeChild,
        state: &SidebarState,
        active_path: Option<&FilesPath>,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let is_active = active_path.is_some_and(|ap| paths_match(ap, &child.path));
        let reorder_mode = state.reorder_mode;

        let mut row_el = div()
            .id(SharedString::from(format!(
                "sidebar-tree-{:?}-{}-{}",
                section_id, parent_idx, child_idx
            )))
            .relative()
            .h(px(SECTION_ITEM_HEIGHT))
            .mx(px(8.0))
            .flex()
            .items_center()
            .pl(px(12.0 + (depth as f32) * 14.0))
            .pr(px(8.0))
            .gap(px(6.0))
            .rounded(px(4.0))
            .children(Self::render_active_indicator(is_active))
            .child(if child.has_subfolders {
                Self::render_tree_chevron(
                    SharedString::from(format!(
                        "sidebar-tree-chevron-{:?}-{}-{}",
                        section_id, parent_idx, child_idx
                    )),
                    state.is_path_expanded(&child.path),
                    !reorder_mode,
                    child.path.clone(),
                    view.clone(),
                )
            } else {
                div().w(px(16.0)).h(px(16.0)).flex_shrink_0().into_any_element()
            })
            .child(
                div()
                    .w(px(18.0))
                    .h(px(18.0))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(Self::render_tree_child_icon(&child.path, is_active, state)),
            )
            .child(
                div()
                    .flex_grow()
                    .min_w(px(0.0))
                    .ml(px(2.0))
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .text_size(px(UiConstants::ITEM_FONT_SIZE_RAW as f32))
                    .font_weight(if is_active {
                        FontWeight(600.0)
                    } else {
                        FontWeight::default()
                    })
                    .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                    .child(child.label.clone()),
            );

        if reorder_mode {
            row_el = row_el.opacity(0.4).cursor_not_allowed();
        } else {
            let v_click = view.clone();
            let nav_path = child.path.clone();
            row_el = row_el
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
                .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
                    cx.stop_propagation();
                    let p = nav_path.clone();
                    v_click.update(cx, |this, cx| {
                        this.stop_sidebar_tooltip(cx);
                        this.navigate(p, cx);
                    });
                });

            row_el = Self::attach_sidebar_tooltip(
                row_el,
                Self::tree_child_tooltip_data(child),
                view.clone(),
            );
            row_el = Self::attach_file_drag_handlers(
                row_el,
                child.path.clone(),
                child.label.clone(),
                view.clone(),
            );
        }

        Self::attach_item_context_menu(
            row_el,
            child.label.clone(),
            Some(child.path.to_string_lossy().into_owned()),
            false,
            view,
        )
    }

    fn render_badge(badge: &SidebarBadge) -> Option<AnyElement> {
        match badge {
            SidebarBadge::Count(n) => Some(
                div()
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
                    )
                    .into_any_element(),
            ),
            SidebarBadge::Usage(_) => None,
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
                        this.stop_sidebar_tooltip(cx);
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

fn normalize_path_key(path: &FilesPath) -> String {
    let raw = path.to_string_lossy();
    #[cfg(windows)]
    {
        let mut s = raw.replace('/', "\\").to_ascii_lowercase();
        while s.len() > 3 && s.ends_with('\\') {
            s.pop();
        }
        s
    }
    #[cfg(not(windows))]
    {
        let mut s = raw.to_string();
        while s.len() > 1 && s.ends_with('/') {
            s.pop();
        }
        s
    }
}

fn paths_match(a: &FilesPath, b: &FilesPath) -> bool {
    a == b || normalize_path_key(a) == normalize_path_key(b)
}

fn section_height(section: &SidebarSectionData, state: &SidebarState) -> f32 {
    let items_height = if section.collapsed {
        0.0
    } else {
        let mut count = section.items.len();
        for item in &section.items {
            if let Some(exp_path) = SidebarState::item_expandable_path(item) {
                count += state.visible_tree_rows_for_root(exp_path).len();
            }
        }
        count as f32 * SECTION_ITEM_HEIGHT
    };
    SECTION_HEADER_HEIGHT + items_height + SECTION_MARGIN_BOTTOM
}

/// Compute the exact scrollable content height of the sidebar's sections area.
pub fn compute_sidebar_content_height(sidebar: &SidebarState) -> f32 {
    let padding = (UiConstants::PADDING_BASE as f32) * 2.0;
    let mut h = padding;
    for section in &sidebar.sections {
        if sidebar.hidden_sections.contains(&section.id) {
            continue;
        }
        if sidebar.collapsed {
            h += COLLAPSED_SECTION_ICON_SIZE;
        } else {
            h += section_height(section, sidebar);
        }
    }
    h
}

/// Compute the exact visible viewport height of the sidebar's scrollable container.
/// Subtracts TabBar (40) + NavBar (46) + Home (30/36) + Settings (40).
pub fn compute_sidebar_viewport_height(win_h: f32, collapsed: bool) -> f32 {
    let home_h = if collapsed {
        COLLAPSED_SECTION_ICON_SIZE
    } else {
        SECTION_ITEM_HEIGHT
    };
    (win_h - 40.0 - 46.0 - home_h - 40.0).max(60.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[::core::prelude::rust_2024::test]
    fn test_section_icon_names_and_bundled_assets() {
        let mapping = [
            (SectionId::Favorites, "sidebar_pinned.png"),
            (SectionId::ThisPc, "sidebar_drives.png"),
            (SectionId::CloudDrives, "sidebar_cloud.png"),
            (SectionId::Network, "sidebar_network.png"),
            (SectionId::Wsl, "sidebar_wsl.png"),
            (SectionId::Tags, "sidebar_tags.png"),
            (SectionId::Libraries, "sidebar_libraries.png"),
        ];
        for (sid, expected) in mapping {
            assert_eq!(SidebarView::section_icon_name(sid), expected);
            assert!(
                bundled_icon_path(expected).is_some(),
                "Expected bundled icon {}",
                expected
            );
        }
        assert!(bundled_icon_path("sidebar_home.png").is_some());
        assert!(bundled_icon_path("folder_icon.png").is_some());
    }

    #[::core::prelude::rust_2024::test]
    fn test_compute_sidebar_content_height_accounts_for_expanded_tree() {
        let root = FilesPath::new(r"C:\Projects");
        let child_a = FilesPath::new(r"C:\Projects\Alpha");
        let child_b = FilesPath::new(r"C:\Projects\Beta");
        let grand = FilesPath::new(r"C:\Projects\Alpha\Sub");

        let mut state = SidebarState::default();
        state.sections = vec![
            SidebarSectionData {
                id: SectionId::Favorites,
                title: "Pinned".to_string(),
                items: vec![SidebarItem {
                    id: SidebarItemId::Pinned(root.clone()),
                    label: "Projects".to_string(),
                    icon: "\u{E8B7}".to_string(),
                    badge: None,
                }],
                collapsed: false,
            },
            SidebarSectionData {
                id: SectionId::Network,
                title: "Network".to_string(),
                items: vec![SidebarItem {
                    id: SidebarItemId::Network,
                    label: "Network".to_string(),
                    icon: "\u{E802}".to_string(),
                    badge: None,
                }],
                collapsed: true,
            },
        ];

        let padding = (UiConstants::PADDING_BASE as f32) * 2.0;
        let base_expected = padding
            + (SECTION_HEADER_HEIGHT + SECTION_ITEM_HEIGHT + SECTION_MARGIN_BOTTOM)
            + (SECTION_HEADER_HEIGHT + SECTION_MARGIN_BOTTOM);
        assert!((compute_sidebar_content_height(&state) - base_expected).abs() < f32::EPSILON);

        state.tree_children.insert(
            root.clone(),
            vec![
                SidebarTreeChild {
                    path: child_a.clone(),
                    label: "Alpha".to_string(),
                    has_subfolders: true,
                },
                SidebarTreeChild {
                    path: child_b,
                    label: "Beta".to_string(),
                    has_subfolders: false,
                },
            ],
        );
        state.tree_children.insert(
            child_a.clone(),
            vec![SidebarTreeChild {
                path: grand,
                label: "Sub".to_string(),
                has_subfolders: false,
            }],
        );
        state.expanded_paths.insert(root.clone());
        state.expanded_paths.insert(child_a);

        // 3 visible tree children (Alpha, Sub, Beta) * 30.0px added to Pinned section
        let expanded_expected = base_expected + 3.0 * SECTION_ITEM_HEIGHT;
        assert!((compute_sidebar_content_height(&state) - expanded_expected).abs() < f32::EPSILON);

        // Collapsing the Pinned section hides both parent item and tree children
        state.sections[0].collapsed = true;
        let collapsed_sec_expected = padding
            + (SECTION_HEADER_HEIGHT + SECTION_MARGIN_BOTTOM)
            + (SECTION_HEADER_HEIGHT + SECTION_MARGIN_BOTTOM);
        assert!(
            (compute_sidebar_content_height(&state) - collapsed_sec_expected).abs() < f32::EPSILON
        );

        // Collapsing entire sidebar switches to icon-only height per visible section
        state.sections[0].collapsed = false;
        state.collapsed = true;
        assert!(
            (compute_sidebar_content_height(&state) - (padding + 2.0 * COLLAPSED_SECTION_ICON_SIZE))
                .abs()
                < f32::EPSILON
        );
    }

    #[::core::prelude::rust_2024::test]
    fn test_tooltip_builders_for_items_and_tree_children() {
        let mut state = SidebarState::default();
        let drive_root = FilesPath::new(r"C:\");
        state.drive_meta_by_root.insert(
            drive_root.clone(),
            SidebarDriveMeta {
                display_name: "Windows (C:)".to_string(),
                file_system: "NTFS".to_string(),
                total_bytes: 1_000,
                free_bytes: 375,
                usage_percent: 62.5,
                capacity_summary: "375 B free of 1000 B".to_string(),
            },
        );

        #[cfg(windows)]
        let drive_type = crate::platform::windows::drive_service::DriveType::Fixed;
        #[cfg(not(windows))]
        let drive_type = ();

        let drive_item = SidebarItem {
            id: SidebarItemId::Drive {
                root: drive_root,
                drive_type,
            },
            label: "Windows (C:)".to_string(),
            icon: "\u{E770}".to_string(),
            badge: None,
        };
        let drive_tip = SidebarView::item_tooltip_data(&drive_item, &state)
            .expect("Drive item should produce tooltip data");
        assert_eq!(drive_tip.title, "Windows (C:)");
        assert_eq!(
            drive_tip.subtitle.as_deref(),
            Some("File system: NTFS • 62.5% used")
        );
        assert_eq!(drive_tip.usage_percent, Some(62.5));
        assert_eq!(
            drive_tip.detail_line.as_deref(),
            Some("375 B free of 1000 B")
        );

        let pinned_item = SidebarItem {
            id: SidebarItemId::Pinned(FilesPath::new(r"D:\Work")),
            label: "Work".to_string(),
            icon: "\u{E8B7}".to_string(),
            badge: None,
        };
        let pinned_tip = SidebarView::item_tooltip_data(&pinned_item, &state).unwrap();
        assert_eq!(pinned_tip.title, "Work");
        assert_eq!(pinned_tip.subtitle.as_deref(), Some(r"D:\Work"));
        assert!(pinned_tip.usage_percent.is_none());
        assert!(pinned_tip.detail_line.is_none());

        #[cfg(windows)]
        let cloud_provider = crate::platform::windows::cloud_drive_service::CloudProvider::OneDrive;
        #[cfg(not(windows))]
        let cloud_provider = ();
        let cloud_item = SidebarItem {
            id: SidebarItemId::CloudDrive(cloud_provider),
            label: "OneDrive".to_string(),
            icon: "\u{E753}".to_string(),
            badge: None,
        };
        assert_eq!(
            SidebarView::item_tooltip_data(&cloud_item, &state)
                .unwrap()
                .title,
            "OneDrive"
        );

        let network_item = SidebarItem {
            id: SidebarItemId::Network,
            label: "Network".to_string(),
            icon: "\u{E802}".to_string(),
            badge: None,
        };
        assert!(SidebarView::item_tooltip_data(&network_item, &state).is_some());

        let wsl_item = SidebarItem {
            id: SidebarItemId::WslDistro("Ubuntu".to_string()),
            label: "Ubuntu".to_string(),
            icon: "\u{E756}".to_string(),
            badge: None,
        };
        assert_eq!(
            SidebarView::item_tooltip_data(&wsl_item, &state)
                .unwrap()
                .subtitle
                .as_deref(),
            Some(r"\\wsl$\Ubuntu")
        );

        let tag_item = SidebarItem {
            id: SidebarItemId::Tag("Red".to_string()),
            label: "Red".to_string(),
            icon: "\u{E8EC}".to_string(),
            badge: None,
        };
        assert!(SidebarView::item_tooltip_data(&tag_item, &state).is_none());

        let child = SidebarTreeChild {
            path: FilesPath::new(r"D:\Work\Sub"),
            label: "Sub".to_string(),
            has_subfolders: true,
        };
        let child_tip = SidebarView::tree_child_tooltip_data(&child);
        assert_eq!(child_tip.title, "Sub");
        assert_eq!(child_tip.subtitle.as_deref(), Some(r"D:\Work\Sub"));
    }

    #[::core::prelude::rust_2024::test]
    fn test_render_badge_suppresses_usage_bar_and_paths_match() {
        assert!(SidebarView::render_badge(&SidebarBadge::Usage(0.75)).is_none());
        assert!(SidebarView::render_badge(&SidebarBadge::Count(4)).is_some());

        #[cfg(windows)]
        {
            assert!(paths_match(
                &FilesPath::new(r"C:\Work\Sub\"),
                &FilesPath::new(r"c:/work/sub")
            ));
            assert!(paths_match(
                &FilesPath::new(r"C:\"),
                &FilesPath::new(r"c:/")
            ));
        }
        #[cfg(not(windows))]
        {
            assert!(paths_match(
                &FilesPath::new("/work/sub/"),
                &FilesPath::new("/work/sub")
            ));
        }
    }
}

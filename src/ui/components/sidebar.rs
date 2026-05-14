//! Sidebar UI component.
//! Sections: Favorites, File Tags, Libraries, This PC (drives), Cloud Drives, Network, WSL.
//! Uses the function-based builder pattern matching existing components (ToolbarView, TabBarView, etc.).

use super::sidebar_state::*;
use crate::app::AppState;
use crate::ui::components::dialogs::DialogType;
use crate::ui::constants::UiConstants;
use gpui::*;

// ─────────────────────────────────────────────────────────────────────────────
// SidebarView — the main sidebar container
// ─────────────────────────────────────────────────────────────────────────────

pub struct SidebarView;

impl SidebarView {
    pub fn build(
        state: &SidebarState,
        _app_state: &AppState,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let width = if state.collapsed {
            px(UiConstants::SIDEBAR_WIDTH_COLLAPSED_RAW as f32)
        } else {
            px(state.width)
        };

        div()
            .id("sidebar")
            .w(width)
            .h_full()
            .flex_col()
            .bg(rgb(UiConstants::BG_SIDEBAR_RGB))
            .border_r(px(1.0))
            .border_color(rgba(UiConstants::DIVIDER_RGBA))
            .child(Self::render_collapse_toggle(state))
            // Scrollable sections
            .child(
                div()
                    .id("sidebar-scroll-container")
                    .flex_grow()
                    .flex_col()
                    .overflow_y_scroll() // Ensure this is present
                    .pt(px(UiConstants::PADDING_BASE as f32))
                    .pb(px(UiConstants::PADDING_BASE as f32))
                    .children(state.sections.iter().enumerate().map(|(idx, section)| {
                        Self::render_section(section, idx, state.active_item.as_ref(), view.clone())
                    })),
            )
            // Settings button fixed at the bottom (outside scroll area)
            .child(Self::render_settings_button(view))
    }

    fn render_collapse_toggle(state: &SidebarState) -> impl IntoElement {
        let collapsed = state.collapsed;

        div()
            .id("sidebar-collapse-toggle")
            .h(px(40.0))
            .flex()
            .items_center()
            .px(px(12.0))
            .gap(px(12.0))
            .cursor_pointer()
            .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
            .on_mouse_down(MouseButton::Left, move |_event, _window, _cx| {
                // Collapse toggle will be wired to state toggle
            })
            .child(
                div()
                    .text_size(px(16.0))
                    .font_family(UiConstants::FONT_ICONS)
                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                    .child("\u{E700}"), // GlobalNavigationButton (Hamburger)
            )
            .child(
                div()
                    .text_size(px(UiConstants::ITEM_FONT_SIZE_RAW as f32))
                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                    .child("Files"),
            )
    }

    fn render_section(
        section: &SidebarSectionData,
        section_idx: usize,
        active_id: Option<&SidebarItemId>,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let collapsed = section.collapsed;
        let title = section.title.clone();

        div()
            .id(ElementId::from(section_idx))
            .flex_col()
            .mb(px(8.0))
            // Header
            .child(
                div()
                    .id(ElementId::from(section_idx * 1000 + 1))
                    .h(px(32.0))
                    .flex()
                    .items_center()
                    .justify_between()
                    .px(px(16.0))
                    .cursor_pointer()
                    .on_mouse_down(MouseButton::Left, {
                        let view = view.clone();
                        let section_id = section.id;
                        move |_ev, _win, cx| {
                            view.update(cx, |this, cx| {
                                this.state.sidebar.toggle_section(section_id);
                                cx.notify();
                            });
                        }
                    })
                    .child(
                        div()
                            .text_size(px(12.0))
                            .font_weight(FontWeight(600.0))
                            .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                            .child(title.to_string()),
                    )
            )
            // Items (hidden when collapsed)
            .children(if collapsed {
                vec![]
            } else {
                section
                    .items
                    .iter()
                    .enumerate()
                    .map(|(idx, item)| Self::render_item(item, idx, active_id, view.clone()))
                    .collect::<Vec<_>>()
            })
    }

    fn render_item(
        item: &SidebarItem,
        idx: usize,
        active_id: Option<&SidebarItemId>,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let icon_src = item.icon.clone();
        let label = item.label.clone();
        let item_id = item.id.clone();
        let v_click = view.clone();
        let is_active = active_id.map_or(false, |id| *id == item_id);

        div()
            .id(ElementId::from(10000 + idx))
            .h(px(36.0))
            .mx(px(8.0))
            .flex()
            .items_center()
            .px(px(8.0))
            .gap(px(12.0))
            .cursor_pointer()
            .rounded(px(4.0))
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
                    if let Some(path) = item_id.get_path() {
                        v_click.update(cx, |this, cx| {
                            this.navigate(path, cx);
                        });
                    }
                }
            })
            // Accent indicator
            .child(
                div()
                    .w(px(if is_active { 3.0 } else { 0.0 }))
                    .h(px(16.0))
                    .bg(rgb(UiConstants::ACCENT_COLOR_RGB))
                    .rounded(px(2.0))
                    .absolute()
                    .left(px(0.0)),
            )
            // Icon
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
                        rgba(UiConstants::TEXT_SECONDARY_RGBA)
                    })
                    .child(icon_src),
            )
            // Label
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
            // Badge
            .children(item.badge.as_ref().map(|badge| Self::render_badge(badge)))
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

    /// Render a fixed settings button at the bottom of the sidebar
    fn render_settings_button(view: Entity<crate::app::AppWindow>) -> impl IntoElement {
        let v = view.clone();
        div()
            .id("sidebar-settings")
            .h(px(40.0))
            .w_full()
            .flex()
            .items_center()
            .gap(px(8.0))
            .px(px(UiConstants::PADDING_BASE as f32))
            .border_t(px(1.0))
            .border_color(rgba(UiConstants::DIVIDER_RGBA))
            .cursor_pointer()
            .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
            .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
                v.update(cx, |this, cx| {
                    this.state.active_dialog = Some(DialogType::Settings);
                    cx.notify();
                });
            })
            .child(
                div()
                    .text_size(AbsoluteLength::Pixels(px(14.0)))
                    .font_family(UiConstants::FONT_MDL2)
                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                    .child("\u{E713}"), // Gear/settings icon
            )
            .child(
                div()
                    .text_size(AbsoluteLength::Pixels(px(UiConstants::ITEM_FONT_SIZE_RAW as f32)))
                    .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                    .child("Settings"),
            )
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// SidebarSplitter — draggable resize handle
// ─────────────────────────────────────────────────────────────────────────────

pub struct SidebarSplitter;

impl SidebarSplitter {
    pub fn build<F>(_on_resize: F) -> impl IntoElement
    where
        F: Fn(f32) + 'static,
    {
        div()
            .id("sidebar-splitter")
            .w(px(UiConstants::SPLITTER_WIDTH_RAW as f32))
            .h_full()
            .cursor_col_resize()
            .bg(rgba(UiConstants::BG_TRANSPARENT_RGBA))
            .on_mouse_down(MouseButton::Left, move |_event, _window, _cx| {
                // In a real implementation, we would capture the mouse and
                // track movement until MouseUp. For this scaffold, we'll
                // trigger a basic resize or log the event.
                tracing::info!("Sidebar resize started");
            })
    }
}

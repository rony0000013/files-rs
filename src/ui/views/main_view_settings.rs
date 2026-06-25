//! Settings page rendering for MainView.
//! Renders the full settings page as a tab content (not a modal dialog).
//! Follows the Files-community design: sidebar navigation + content area.

use crate::config::config::AppConfig;
use crate::ui::components::dialogs::{SettingsPreReadData, SettingsTab};
use crate::ui::constants::UiConstants;
use gpui::*;

use super::main_view::MainView;

impl MainView {
    /// Render the settings page as a full tab content area.
    /// Layout: [Settings Sidebar 240px] [Settings Content flex-grow]
    pub fn render_settings_view(
        data: &SettingsPreReadData,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let active_tab = data.active_tab;
        let config = &data.config;

        div()
            .flex()
            .flex_row()
            .w_full()
            .h_full()
            .overflow_hidden()
            .child(
                // Settings Sidebar (left navigation)
                Self::build_settings_sidebar(active_tab, view.clone()),
            )
            .child(
                // Settings Content (right, scrollable)
                div()
                    .id("settings-content-scroll")
                    .flex_col()
                    .flex_grow()
                    .h_full()
                    .overflow_y_scroll()
                    .px(px(32.0))
                    .py(px(24.0))
                    .child(
                        // Page title
                        div()
                            .text_size(px(28.0))
                            .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                            .font_weight(FontWeight(600.0))
                            .mb(px(8.0))
                            .child(Self::settings_page_title(active_tab)),
                    )
                    .child(
                        // Page description
                        div()
                            .text_size(px(14.0))
                            .text_color(rgb(UiConstants::TEXT_SECONDARY_RGB))
                            .mb(px(24.0))
                            .child(Self::settings_page_description(active_tab)),
                    )
                    .child(crate::ui::components::dialogs::DialogsView::build_settings_content(
                        active_tab,
                        config,
                        data,
                        view.clone(),
                    )),
            )
    }

    /// Build the settings sidebar navigation
    fn build_settings_sidebar(
        active_tab: SettingsTab,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        div()
            .w(px(240.0))
            .h_full()
            .flex_shrink_0()
            .bg(rgb(UiConstants::BG_SIDEBAR_RGB))
            .border_r(px(1.0))
            .border_color(rgba(UiConstants::DIVIDER_RGBA))
            .child(
                // Sidebar header
                div()
                    .px(px(20.0))
                    .py(px(16.0))
                    .border_b(px(1.0))
                    .border_color(rgba(UiConstants::DIVIDER_RGBA))
                    .child(
                        div()
                            .text_size(px(16.0))
                            .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                            .font_weight(FontWeight(600.0))
                            .child("Settings"),
                    ),
            )
            .child(
                // Navigation items
                div()
                    .flex_col()
                    .p(px(8.0))
                    .gap(px(2.0))
                    .child(Self::settings_nav_item(
                        "\u{E713}",
                        "General",
                        SettingsTab::General,
                        active_tab == SettingsTab::General,
                        view.clone(),
                    ))
                    .child(Self::settings_nav_item(
                        "\u{E771}",
                        "Appearance",
                        SettingsTab::Appearance,
                        active_tab == SettingsTab::Appearance,
                        view.clone(),
                    ))
                    .child(Self::settings_nav_item(
                        "\u{E8A9}",
                        "Layout",
                        SettingsTab::Layout,
                        active_tab == SettingsTab::Layout,
                        view.clone(),
                    ))
                    .child(Self::settings_nav_item(
                        "\u{E8B7}",
                        "Folders",
                        SettingsTab::Folders,
                        active_tab == SettingsTab::Folders,
                        view.clone(),
                    ))
                    .child(Self::settings_nav_item(
                        "\u{E8EC}",
                        "Tags",
                        SettingsTab::Tags,
                        active_tab == SettingsTab::Tags,
                        view.clone(),
                    ))
                    .child(Self::settings_nav_item(
                        "\u{E765}",
                        "Actions",
                        SettingsTab::Actions,
                        active_tab == SettingsTab::Actions,
                        view.clone(),
                    ))
                    .child(Self::settings_nav_item(
                        "\u{E700}",
                        "Toolbar",
                        SettingsTab::Toolbar,
                        active_tab == SettingsTab::Toolbar,
                        view.clone(),
                    ))
                    .child(Self::settings_nav_item(
                        "\u{E783}",
                        "Advanced",
                        SettingsTab::Advanced,
                        active_tab == SettingsTab::Advanced,
                        view.clone(),
                    ))
                    .child(Self::settings_nav_item(
                        "\u{E7B8}",
                        "DevTools",
                        SettingsTab::DevTools,
                        active_tab == SettingsTab::DevTools,
                        view.clone(),
                    ))
                    .child(Self::settings_nav_item(
                        "\u{E946}",
                        "About",
                        SettingsTab::About,
                        active_tab == SettingsTab::About,
                        view.clone(),
                    )),
            )
    }

    /// A single settings navigation item (icon + label)
    fn settings_nav_item(
        icon: &str,
        label: &str,
        tab: SettingsTab,
        active: bool,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let v = view.clone();
        div()
            .id(SharedString::from(format!("settings-nav-{:?}", tab)))
            .flex()
            .items_center()
            .gap(px(12.0))
            .px(px(12.0))
            .py(px(10.0))
            .rounded(px(6.0))
            .cursor_pointer()
            .bg(if active {
                rgba(0xFFFFFF14u32) // SubtleFillColorTertiary
            } else {
                rgba(0x00000000u32)
            })
            .hover(|s| {
                if !active {
                    s.bg(rgba(0xFFFFFF0Au32))
                } else {
                    s
                }
            })
            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                v.update(cx, |this, cx| this.set_settings_tab(tab, cx));
            })
            .child(
                // Icon
                div()
                    .text_size(px(16.0))
                    .font_family(UiConstants::FONT_MDL2)
                    .text_color(if active {
                        rgb(UiConstants::ACCENT_COLOR_RGB)
                    } else {
                        rgb(UiConstants::TEXT_SECONDARY_RGB)
                    })
                    .w(px(20.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(icon.to_string()),
            )
            .child(
                // Label
                div()
                    .text_size(px(14.0))
                    .text_color(if active {
                        rgb(UiConstants::TEXT_PRIMARY_RGB)
                    } else {
                        rgb(UiConstants::TEXT_SECONDARY_RGB)
                    })
                    .child(label.to_string()),
            )
    }

    /// Get the page title for a settings tab
    fn settings_page_title(tab: SettingsTab) -> &'static str {
        match tab {
            SettingsTab::General => "General",
            SettingsTab::Appearance => "Appearance",
            SettingsTab::Layout => "Layout",
            SettingsTab::Folders => "Folders",
            SettingsTab::Tags => "Tags",
            SettingsTab::Actions => "Actions",
            SettingsTab::Toolbar => "Toolbar",
            SettingsTab::Advanced => "Advanced",
            SettingsTab::DevTools => "DevTools",
            SettingsTab::About => "About",
        }
    }

    /// Get the page description for a settings tab
    fn settings_page_description(tab: SettingsTab) -> &'static str {
        match tab {
            SettingsTab::General => "Configure general application behavior and startup options.",
            SettingsTab::Appearance => "Customize the visual appearance, theme, and fonts.",
            SettingsTab::Layout => "Set default view modes, sorting, and grouping options.",
            SettingsTab::Folders => "Control folder visibility and file display settings.",
            SettingsTab::Tags => "Manage file tags for organization.",
            SettingsTab::Actions => "View and customize keyboard shortcuts.",
            SettingsTab::Toolbar => "Choose which buttons appear on the toolbar.",
            SettingsTab::Advanced => "Experimental features and advanced options.",
            SettingsTab::DevTools => "Developer tools and logging.",
            SettingsTab::About => "Application information and links.",
        }
    }
}

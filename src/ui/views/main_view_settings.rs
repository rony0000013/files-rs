//! Settings page rendering for MainView.
//! Renders the full settings page as a tab content area matching `Files-community` (`SettingsPage.xaml`):
//! - Left 240px `SettingsSidebar` with title, interactive Settings Search box, and 10 navigation items with active left indicator pill
//! - Right scrollable `SettingsContentFrame` with WinUI 3 `SubtitleTextBlockStyle` page heading and `SettingsCard` / `SettingsExpander` blocks.

use crate::ui::components::dialogs::{SettingsPreReadData, SettingsTab};
use crate::ui::components::text_input::{
    render_text_input, TextInputMode, TextInputState, TextInputStyle,
};
use crate::ui::constants::UiConstants;
use gpui::*;
use std::rc::Rc;

use super::main_view::{MainView, MainViewState};

impl MainView {
    /// Render the settings page as a full tab content area (`SettingsPage.xaml` parity).
    pub fn render_settings_view(
        state: &MainViewState,
        data: &SettingsPreReadData,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let active_tab = data.active_tab;
        let config = &data.config;
        let is_searching = !data.ui_state.search_query.trim().is_empty();

        div()
            .flex()
            .flex_row()
            .w_full()
            .h_full()
            .overflow_hidden()
            .child(Self::build_settings_sidebar(
                active_tab,
                &data.ui_state.search_input,
                data.ui_state.search_focused,
                view.clone(),
            ))
            .child(
                div()
                    .id("settings-content-scroll")
                    .flex()
                    .flex_col()
                    .flex_grow()
                    .min_h(px(0.0))
                    .h_full()
                    .overflow_y_scroll()
                    .track_scroll(&state.scroll_handle)
                    .on_scroll_wheel(crate::ui::components::bouncy_scroll::smooth_scroll_handle_wheel(
                        view.clone(),
                        |app_state| &mut app_state.main_view.bouncy_scroll,
                        |app_state| &app_state.main_view.scroll_handle,
                    ))
                    .px(px(28.0))
                    .py(px(20.0))
                    .child(
                        div()
                            .max_w(px(1000.0))
                            .w_full()
                            .flex()
                            .flex_col()
                            .children(if !is_searching {
                                vec![
                                    div()
                                        .text_size(px(22.0))
                                        .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                                        .font_weight(FontWeight(600.0))
                                        .pb(px(14.0))
                                        .child(Self::settings_page_title(active_tab))
                                        .into_any_element(),
                                ]
                            } else {
                                vec![]
                            })
                            .child(
                                crate::ui::components::dialogs::DialogsView::build_settings_content(
                                    active_tab,
                                    config,
                                    data,
                                    view.clone(),
                                ),
                            ),
                    ),
            )
    }

    /// Build the 240px left Settings navigation sidebar (`SettingsPage.xaml` `SettingsSidebar` parity).
    fn build_settings_sidebar(
        active_tab: SettingsTab,
        search_input: &TextInputState,
        search_focused: bool,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let v_clear_search = view.clone();
        let has_query = !search_input.text.is_empty();

        div()
            .w(px(240.0))
            .h_full()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .bg(rgba(0x00000022u32))
            .border_r(px(1.0))
            .border_color(rgba(UiConstants::CARD_STROKE_RGBA))
            .child(
                // Header + Search Box
                div()
                    .px(px(16.0))
                    .pt(px(14.0))
                    .pb(px(10.0))
                    .flex()
                    .flex_col()
                    .gap(px(10.0))
                    .child(
                        div()
                            .text_size(px(18.0))
                            .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                            .font_weight(FontWeight(600.0))
                            .child("Settings"),
                    )
                    .child(
                        div()
                            .w_full()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(6.0))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w(px(0.0))
                                    .child(render_text_input(
                                        "settings-sidebar-search",
                                        search_input,
                                        "Find a setting",
                                        search_focused,
                                        false,
                                        TextInputMode::SingleLine,
                                        TextInputStyle::compact_search(),
                                        view.clone(),
                                        Rc::new(|app| Some(&mut app.state.settings_ui.search_input)),
                                        Some(Rc::new(|app, _win, _cx| {
                                            app.state.settings_ui.search_focused = true;
                                            app.state.settings_ui.actions_search_focused = false;
                                        })),
                                    )),
                            )
                            .children(if has_query {
                                vec![div()
                                    .px(px(6.0))
                                    .py(px(6.0))
                                    .rounded(px(4.0))
                                    .bg(rgba(0xFFFFFF0Eu32))
                                    .text_size(px(10.0))
                                    .font_family(UiConstants::FONT_ICONS)
                                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                    .cursor_pointer()
                                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                        cx.stop_propagation();
                                        v_clear_search.update(cx, |this, cx| {
                                            this.state.settings_ui.clear_search();
                                            cx.notify();
                                        });
                                    })
                                    .child("\u{E711}")
                                    .into_any_element()]
                            } else {
                                vec![]
                            }),
                    ),
            )
            .child(
                // Navigation items matching Files-community SettingsPageViewModel
                div()
                    .flex()
                    .flex_col()
                    .px(px(8.0))
                    .py(px(4.0))
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
                        "\u{E8BA}",
                        "Layout",
                        SettingsTab::Layout,
                        active_tab == SettingsTab::Layout,
                        view.clone(),
                    ))
                    .child(Self::settings_nav_item(
                        "\u{E8B7}",
                        "Files & folders",
                        SettingsTab::Folders,
                        active_tab == SettingsTab::Folders,
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
                        "\u{E8EC}",
                        "File tags",
                        SettingsTab::Tags,
                        active_tab == SettingsTab::Tags,
                        view.clone(),
                    ))
                    .child(Self::settings_nav_item(
                        "\u{E712}",
                        "Toolbar",
                        SettingsTab::Toolbar,
                        active_tab == SettingsTab::Toolbar,
                        view.clone(),
                    ))
                    .child(Self::settings_nav_item(
                        "\u{E7AC}",
                        "Dev tools",
                        SettingsTab::DevTools,
                        active_tab == SettingsTab::DevTools,
                        view.clone(),
                    ))
                    .child(Self::settings_nav_item(
                        "\u{E90F}",
                        "Advanced",
                        SettingsTab::Advanced,
                        active_tab == SettingsTab::Advanced,
                        view.clone(),
                    ))
                    .child(
                        div()
                            .h(px(1.0))
                            .w_full()
                            .my(px(4.0))
                            .bg(rgba(UiConstants::DIVIDER_RGBA)),
                    )
                    .child(Self::settings_nav_item(
                        "\u{E946}",
                        "About",
                        SettingsTab::About,
                        active_tab == SettingsTab::About,
                        view.clone(),
                    )),
            )
    }

    /// A single settings navigation item (active indicator pill + icon + label).
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
            .flex_row()
            .items_center()
            .h(px(36.0))
            .px(px(8.0))
            .gap(px(10.0))
            .rounded(px(6.0))
            .cursor_pointer()
            .bg(if active {
                rgba(0xFFFFFF12u32)
            } else {
                rgba(0x00000000u32)
            })
            .hover(|s| {
                if !active {
                    s.bg(rgba(0xFFFFFF09u32))
                } else {
                    s
                }
            })
            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                v.update(cx, |this, cx| {
                    this.state.settings_ui.search_query.clear();
                    this.state.settings_ui.search_focused = false;
                    this.set_settings_tab(tab, cx);
                });
            })
            .child(
                // WinUI 3 3px Left Selection Indicator Pill
                div()
                    .w(px(3.0))
                    .h(px(16.0))
                    .rounded(px(1.5))
                    .bg(if active {
                        rgb(0x60CDFF)
                    } else {
                        rgba(0x00000000u32)
                    }),
            )
            .child(
                div()
                    .text_size(px(15.0))
                    .font_family(UiConstants::FONT_ICONS)
                    .text_color(if active {
                        rgb(0x60CDFF)
                    } else {
                        rgba(UiConstants::TEXT_SECONDARY_RGBA)
                    })
                    .w(px(20.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(icon.to_string()),
            )
            .child(
                div()
                    .text_size(px(13.5))
                    .font_weight(if active {
                        FontWeight(600.0)
                    } else {
                        FontWeight(400.0)
                    })
                    .text_color(if active {
                        rgba(UiConstants::TEXT_PRIMARY_RGBA)
                    } else {
                        rgba(UiConstants::TEXT_SECONDARY_RGBA)
                    })
                    .child(label.to_string()),
            )
    }

    /// Get the page title for a settings tab (`Files-community` headers).
    fn settings_page_title(tab: SettingsTab) -> &'static str {
        match tab {
            SettingsTab::General => "General",
            SettingsTab::Appearance => "Appearance",
            SettingsTab::Layout => "Layout",
            SettingsTab::Folders => "Files & folders",
            SettingsTab::Tags => "File tags",
            SettingsTab::Actions => "Actions",
            SettingsTab::Toolbar => "Toolbar",
            SettingsTab::Advanced => "Advanced",
            SettingsTab::DevTools => "Dev tools",
            SettingsTab::About => "About",
        }
    }
}

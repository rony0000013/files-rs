#![allow(dead_code)]

use crate::config::config::{ContextMenuStyle, GroupColumn, SortBy, ViewMode};
use crate::core::action_registry::{ActionCategory, ActionKind, KeyCombination};
use crate::ui::components::dialogs::{
    KeyBindingRecorderState, SettingsPreReadData, SettingsTab,
};
use crate::ui::constants::UiConstants;
use gpui::*;

impl crate::ui::components::dialogs::DialogsView {
    // ─────────────────────────────────────────────────────────────────────────
    // WinUI 3 Fluent Settings Primitives (SettingsCard, SettingsExpander, Keycaps)
    // ─────────────────────────────────────────────────────────────────────────

    pub(crate) fn settings_section_heading(label: &str) -> impl IntoElement {
        div()
            .text_size(px(14.0))
            .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
            .font_weight(FontWeight(600.0))
            .mt(px(16.0))
            .mb(px(6.0))
            .child(label.to_string())
    }

    pub(crate) fn settings_group_header(label: &str) -> impl IntoElement {
        Self::settings_section_heading(label)
    }

    /// WinUI 3 `SettingsCard` container with optional Fluent icon, header, description, and right-aligned control.
    pub(crate) fn settings_card(
        icon: Option<&str>,
        title: &str,
        description: Option<&str>,
        control: impl IntoElement,
    ) -> Div {
        let mut left = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(16.0))
            .flex_1()
            .min_w(px(0.0));

        if let Some(glyph) = icon {
            left = left.child(
                div()
                    .w(px(24.0))
                    .h(px(24.0))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(16.0))
                    .font_family(UiConstants::FONT_ICONS)
                    .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                    .child(glyph.to_string()),
            );
        }

        let mut text_col = div().flex().flex_col().gap(px(2.0)).flex_1().min_w(px(0.0));
        text_col = text_col.child(
            div()
                .text_size(px(13.5))
                .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                .child(title.to_string()),
        );
        if let Some(desc) = description {
            if !desc.is_empty() {
                text_col = text_col.child(
                    div()
                        .text_size(px(11.5))
                        .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                        .child(desc.to_string()),
                );
            }
        }
        left = left.child(text_col);

        div()
            .w_full()
            .min_h(px(62.0))
            .px(px(16.0))
            .py(px(12.0))
            .bg(rgba(0xFFFFFF09u32))
            .border(px(1.0))
            .border_color(rgba(UiConstants::CARD_STROKE_RGBA))
            .rounded(px(6.0))
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .gap(px(16.0))
            .child(left)
            .child(div().flex_shrink_0().flex().items_center().child(control))
    }

    /// Clickable WinUI 3 `SettingsCard` with action icon on the right.
    pub(crate) fn settings_clickable_card(
        icon: Option<&str>,
        title: &str,
        description: Option<&str>,
        action_glyph: &str,
        on_click: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    ) -> Div {
        let right = div()
            .text_size(px(14.0))
            .font_family(UiConstants::FONT_ICONS)
            .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
            .child(action_glyph.to_string());
        Self::settings_card(icon, title, description, right)
            .cursor_pointer()
            .hover(|s| s.bg(rgba(0xFFFFFF12u32)))
            .on_mouse_down(MouseButton::Left, on_click)
    }

    /// Nested row inside a `SettingsExpander`.
    pub(crate) fn settings_expander_row(
        title: &str,
        description: Option<&str>,
        control: impl IntoElement,
    ) -> Div {
        let mut text_col = div().flex().flex_col().gap(px(2.0)).flex_1().min_w(px(0.0));
        text_col = text_col.child(
            div()
                .text_size(px(13.0))
                .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                .child(title.to_string()),
        );
        if let Some(desc) = description {
            if !desc.is_empty() {
                text_col = text_col.child(
                    div()
                        .text_size(px(11.5))
                        .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                        .child(desc.to_string()),
                );
            }
        }

        div()
            .w_full()
            .min_h(px(52.0))
            .pl(px(56.0))
            .pr(px(16.0))
            .py(px(10.0))
            .border_t(px(1.0))
            .border_color(rgba(UiConstants::CARD_STROKE_RGBA))
            .bg(rgba(0x00000018u32))
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .gap(px(16.0))
            .child(text_col)
            .child(div().flex_shrink_0().flex().items_center().child(control))
    }

    /// WinUI 3 `SettingsExpander` with collapsible header and seamless child rows.
    pub(crate) fn settings_expander(
        expander_id: &str,
        expanded: bool,
        icon: &str,
        title: &str,
        description: Option<&str>,
        header_control: Option<AnyElement>,
        items: Vec<AnyElement>,
        view: Entity<crate::app::AppWindow>,
    ) -> Div {
        let key_owned = expander_id.to_string();
        let v_toggle = view.clone();

        let mut left = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(16.0))
            .flex_1()
            .min_w(px(0.0))
            .cursor_pointer()
            .on_mouse_down(MouseButton::Left, {
                let k = key_owned.clone();
                let v = v_toggle.clone();
                move |_, _, cx| {
                    v.update(cx, |this, cx| {
                        this.state.settings_ui.toggle_expander(&k);
                        cx.notify();
                    });
                }
            })
            .child(
                div()
                    .w(px(24.0))
                    .h(px(24.0))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(16.0))
                    .font_family(UiConstants::FONT_ICONS)
                    .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                    .child(icon.to_string()),
            );

        let mut text_col = div().flex().flex_col().gap(px(2.0)).flex_1().min_w(px(0.0));
        text_col = text_col.child(
            div()
                .text_size(px(13.5))
                .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                .child(title.to_string()),
        );
        if let Some(desc) = description {
            if !desc.is_empty() {
                text_col = text_col.child(
                    div()
                        .text_size(px(11.5))
                        .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                        .child(desc.to_string()),
                );
            }
        }
        left = left.child(text_col);

        let mut right = div().flex().flex_row().items_center().gap(px(12.0)).flex_shrink_0();
        if let Some(ctrl) = header_control {
            right = right.child(ctrl);
        }
        right = right.child(
            div()
                .w(px(28.0))
                .h(px(28.0))
                .rounded(px(4.0))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                    v_toggle.update(cx, |this, cx| {
                        this.state.settings_ui.toggle_expander(&key_owned);
                        cx.notify();
                    });
                })
                .child(
                    div()
                        .text_size(px(12.0))
                        .font_family(UiConstants::FONT_ICONS)
                        .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                        .child(if expanded { "\u{E70E}" } else { "\u{E70D}" }),
                ),
        );

        let mut container = div()
            .w_full()
            .bg(rgba(0xFFFFFF09u32))
            .border(px(1.0))
            .border_color(rgba(UiConstants::CARD_STROKE_RGBA))
            .rounded(px(6.0))
            .overflow_hidden()
            .flex()
            .flex_col()
            .child(
                div()
                    .w_full()
                    .min_h(px(62.0))
                    .px(px(16.0))
                    .py(px(12.0))
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .gap(px(16.0))
                    .hover(|s| s.bg(rgba(0xFFFFFF06u32)))
                    .child(left)
                    .child(right),
            );

        if expanded && !items.is_empty() {
            container = container.children(items);
        }

        container
    }

    /// WinUI 3 `ToggleSwitch` with `"On"` / `"Off"` state label and pill thumb.
    pub(crate) fn settings_toggle_switch(
        on: bool,
        key: &'static str,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let v_toggle = view.clone();
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(10.0))
            .cursor_pointer()
            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                cx.stop_propagation();
                v_toggle.update(cx, |this, cx| this.toggle_setting(key, cx));
                Self::apply_chrome_live(key, window, &v_toggle, cx);
            })
            .child(
                div()
                    .text_size(px(12.5))
                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                    .min_w(px(24.0))
                    .text_right()
                    .child(if on { "On" } else { "Off" }),
            )
            .child(
                div()
                    .w(px(40.0))
                    .h(px(20.0))
                    .bg(if on {
                        rgba(UiConstants::ACCENT_RGBA)
                    } else {
                        rgba(0xFFFFFF18u32)
                    })
                    .border(px(1.0))
                    .border_color(if on {
                        rgba(UiConstants::ACCENT_RGBA)
                    } else {
                        rgba(0xFFFFFF45u32)
                    })
                    .rounded(px(10.0))
                    .flex()
                    .items_center()
                    .px(px(3.0))
                    .child(
                        div()
                            .w(px(12.0))
                            .h(px(12.0))
                            .bg(if on {
                                rgb(0xFFFFFF)
                            } else {
                                rgb(0xCCCCCC)
                            })
                            .rounded(px(6.0))
                            .ml(if on { px(20.0) } else { px(0.0) }),
                    ),
            )
    }

    pub(crate) fn build_compact_btn(
        label: impl Into<String>,
        active: bool,
        on_click: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    ) -> impl IntoElement {
        let mut btn = div()
            .px(px(10.0))
            .py(px(5.0))
            .rounded(px(5.0))
            .cursor_pointer()
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(12.0))
            .on_mouse_down(MouseButton::Left, move |ev, win, cx| {
                cx.stop_propagation();
                on_click(ev, win, cx);
            });

        if active {
            btn = btn
                .bg(rgba(UiConstants::ACCENT_RGBA))
                .text_color(rgb(0xFFFFFF))
                .font_weight(FontWeight(600.0));
        } else {
            btn = btn
                .bg(rgba(0xFFFFFF10u32))
                .border(px(1.0))
                .border_color(rgba(UiConstants::STROKE_RGBA))
                .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                .hover(|s| s.bg(rgba(0xFFFFFF20u32)));
        }
        btn.child(label.into())
    }

    /// Render WinUI 3 `KeyboardShortcut` keycaps (e.g. `[Ctrl] + [Shift] + [N]` or `[🖱 Mouse 4 (Back)]`).
    pub(crate) fn render_keybinding_pills(combo: Option<&KeyCombination>) -> impl IntoElement {
        let Some(k) = combo.filter(|c| !c.is_empty()) else {
            return div()
                .px(px(8.0))
                .py(px(3.0))
                .rounded(px(4.0))
                .bg(rgba(0xFFFFFF08u32))
                .border(px(1.0))
                .border_color(rgba(0xFFFFFF15u32))
                .text_size(px(11.5))
                .text_color(rgba(UiConstants::TEXT_MUTED_RGBA))
                .child("Unbound")
                .into_any_element();
        };

        let tokens = k.display_tokens();
        let is_mouse = k.is_mouse();
        let mut row = div().flex().flex_row().items_center().gap(px(4.0));

        for (i, tok) in tokens.iter().enumerate() {
            if i > 0 {
                row = row.child(
                    div()
                        .text_size(px(11.0))
                        .text_color(rgba(UiConstants::TEXT_MUTED_RGBA))
                        .child("+"),
                );
            }
            let is_last_mouse = is_mouse && i + 1 == tokens.len();
            let mut keycap = div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(4.0))
                .px(px(7.0))
                .py(px(2.5))
                .rounded(px(4.0))
                .bg(if is_last_mouse {
                    rgba(0x0078D428u32)
                } else {
                    rgba(0xFFFFFF14u32)
                })
                .border(px(1.0))
                .border_b(px(2.0))
                .border_color(if is_last_mouse {
                    rgba(0x60CDFF66u32)
                } else {
                    rgba(0xFFFFFF28u32)
                });

            if is_last_mouse {
                keycap = keycap.child(
                    div()
                        .text_size(px(11.0))
                        .font_family(UiConstants::FONT_ICONS)
                        .text_color(rgb(0x60CDFF))
                        .child("\u{E962}"),
                );
            }
            keycap = keycap.child(
                div()
                    .text_size(px(11.5))
                    .font_weight(FontWeight(500.0))
                    .text_color(if is_last_mouse {
                        rgb(0x60CDFF)
                    } else {
                        rgba(UiConstants::TEXT_PRIMARY_RGBA)
                    })
                    .child(tok.clone()),
            );
            row = row.child(keycap);
        }

        row.into_any_element()
    }

    pub(crate) fn settings_tab_btn(
        label: &str,
        tab: SettingsTab,
        active: bool,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        div()
            .px(px(10.0))
            .py(px(8.0))
            .rounded(px(6.0))
            .bg(if active {
                rgba(0xFFFFFF15u32)
            } else {
                rgba(0x00000000u32)
            })
            .hover(|s| s.bg(rgba(0xFFFFFF10u32)))
            .cursor_pointer()
            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                view.update(cx, |this, cx| this.set_settings_tab(tab, cx));
            })
            .child(
                div()
                    .text_size(AbsoluteLength::Pixels(px(14.0)))
                    .text_color(if active {
                        rgb(UiConstants::TEXT_PRIMARY_RGB)
                    } else {
                        rgb(UiConstants::TEXT_SECONDARY_RGB)
                    })
                    .child(label.to_string()),
            )
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Main Settings Content Dispatcher + Global Search Results
    // ─────────────────────────────────────────────────────────────────────────

    pub(crate) fn build_settings_content<'a>(
        tab: SettingsTab,
        config: &'a crate::config::config::AppConfig,
        data: &'a SettingsPreReadData,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement + 'a {
        if !data.ui_state.search_query.trim().is_empty() {
            return Self::build_settings_search_results(
                &data.ui_state.search_query,
                data,
                view,
            )
            .into_any_element();
        }

        match tab {
            SettingsTab::General => {
                Self::build_general_settings(config, data, view).into_any_element()
            }
            SettingsTab::Appearance => {
                Self::build_appearance_settings(config, data, view).into_any_element()
            }
            SettingsTab::Layout => {
                Self::build_layout_settings(config, data, view).into_any_element()
            }
            SettingsTab::Folders => {
                Self::build_folders_settings(config, data, view).into_any_element()
            }
            SettingsTab::Actions => {
                Self::build_actions_settings(config, data, view).into_any_element()
            }
            SettingsTab::Tags => {
                Self::build_tags_settings(config, data, view).into_any_element()
            }
            SettingsTab::Toolbar => {
                Self::build_toolbar_settings(config, data, view).into_any_element()
            }
            SettingsTab::DevTools => {
                Self::build_devtools_settings(config, data, view).into_any_element()
            }
            SettingsTab::Advanced => {
                Self::build_advanced_settings(config, data, view).into_any_element()
            }
            SettingsTab::About => {
                Self::build_about_settings(config, data, view).into_any_element()
            }
        }
    }

    /// Global Settings Search Results (`Files-community` `SettingsPage.xaml` search repeater).
    fn build_settings_search_results(
        query: &str,
        data: &SettingsPreReadData,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let q = query.trim().to_lowercase();
        let catalog: &[(&str, &str, SettingsTab, Option<&str>)] = &[
            ("Language", "General › Language", SettingsTab::General, None),
            ("Date format", "General › Date format", SettingsTab::General, None),
            ("Startup settings", "General › Startup settings", SettingsTab::General, Some("general_startup")),
            ("Continue where you left off", "General › Startup settings", SettingsTab::General, Some("general_startup")),
            ("Always switch to newly opened tab", "General › Tabs", SettingsTab::General, None),
            ("Widgets (Quick Access, Drives, Tags, Recent Files)", "General › Widgets", SettingsTab::General, Some("general_widgets")),
            ("Dual pane mode & split direction", "General › Dual Pane", SettingsTab::General, None),
            ("Right-click context menu options", "General › Context Menu", SettingsTab::General, Some("general_context_menu")),
            ("Properties dialog style (Files modern vs Windows classic)", "General › Properties dialog", SettingsTab::General, None),
            ("Smooth scrolling & tab wheel direction", "General › Scrolling", SettingsTab::General, None),
            ("Theme (Dark, Light, System)", "Appearance › Theme", SettingsTab::Appearance, None),
            ("Backdrop material (Mica Alt, Mica, Acrylic)", "Appearance › Backdrop material", SettingsTab::Appearance, None),
            ("Background color & theme presets", "Appearance › Background color", SettingsTab::Appearance, Some("appearance_theme_colors")),
            ("Font family & font size", "Appearance › Font", SettingsTab::Appearance, None),
            ("Address bar & Status Center button", "Appearance › Address bar", SettingsTab::Appearance, Some("appearance_address_bar")),
            ("Toolbar & Status bar visibility", "Appearance › Toolbar", SettingsTab::Appearance, Some("appearance_toolbar")),
            ("Sync folder preferences across directories", "Layout › Folder overrides", SettingsTab::Layout, None),
            ("Default layout type (Details, List, Grid, Columns, Adaptive)", "Layout › Layout type", SettingsTab::Layout, None),
            ("Default sorting & sort priority", "Layout › Sorting & grouping › Sort by", SettingsTab::Layout, Some("layout_sort")),
            ("Default grouping & date unit", "Layout › Sorting & grouping › Group by", SettingsTab::Layout, Some("layout_group")),
            ("Details view columns & auto-size", "Layout › Details view › Columns", SettingsTab::Layout, Some("layout_columns")),
            ("Show hidden files, dot files, protected system files & ADS", "Files & folders › Display › Hidden items", SettingsTab::Folders, Some("folders_hidden")),
            ("Show file extensions, thumbnails & selection checkboxes", "Files & folders › Display", SettingsTab::Folders, None),
            ("Single-click vs double-click to open files and folders", "Files & folders › Behaviors › Single-click to open", SettingsTab::Folders, Some("folders_single_click")),
            ("Delete confirmation policy", "Files & folders › Behaviors › Confirm delete", SettingsTab::Folders, None),
            ("Double-click blank space to go up", "Files & folders › Behaviors", SettingsTab::Folders, None),
            ("Calculate folder sizes & size units", "Files & folders › Behaviors › Calculate folder sizes", SettingsTab::Folders, Some("folders_sizes")),
            ("Keyboard shortcuts & customizable commands", "Actions › Commands", SettingsTab::Actions, Some("actions_commands")),
            ("Mouse combinations (Middle-click, Mouse 4/5, Ctrl+Wheel, Double-click)", "Actions › Mouse Combinations", SettingsTab::Actions, Some("actions_mouse")),
            ("File tags & custom tag colors", "File tags › Manage tags", SettingsTab::Tags, Some("tags_list")),
            ("Customize toolbar buttons", "Toolbar › Context commands", SettingsTab::Toolbar, None),
            ("Open in IDE (VS Code, Zed, Cursor, RustRover)", "Dev tools › IDE integration", SettingsTab::DevTools, Some("devtools_ide")),
            ("Export / Import settings & edit settings.json", "Advanced › Settings file", SettingsTab::Advanced, None),
            ("Thumbnail cache size & clear cache", "Advanced › Experimental › Cache thumbnails", SettingsTab::Advanced, Some("advanced_cache")),
            ("Set as default file manager & startup", "Advanced › System integration", SettingsTab::Advanced, None),
            ("Version info, GitHub repo, documentation & licenses", "About › Application info", SettingsTab::About, Some("about_oss")),
        ];

        let mut results: Vec<AnyElement> = Vec::new();
        for &(header, path_desc, target_tab, expander_opt) in catalog {
            if header.to_lowercase().contains(&q) || path_desc.to_lowercase().contains(&q) {
                let v = view.clone();
                let exp_owned = expander_opt.map(|s| s.to_string());
                results.push(
                    Self::settings_clickable_card(
                        Some("\u{E721}"),
                        header,
                        Some(path_desc),
                        "\u{E76C}",
                        move |_, _, cx| {
                            v.update(cx, |this, cx| {
                                this.state.settings_ui.search_query.clear();
                                this.state.settings_ui.search_focused = false;
                                if let Some(ref k) = exp_owned {
                                    this.state.settings_ui.expanded_sections.insert(k.clone());
                                }
                                this.set_settings_tab(target_tab, cx);
                            });
                        },
                    )
                    .into_any_element(),
                );
            }
        }

        // Also match individual keyboard/mouse actions!
        for (act, combo_opt, _) in &data.all_actions {
            let combo_str = combo_opt
                .as_ref()
                .map(|c| c.to_string())
                .unwrap_or_default();
            if act.label().to_lowercase().contains(&q)
                || act.description().to_lowercase().contains(&q)
                || combo_str.to_lowercase().contains(&q)
            {
                let v = view.clone();
                let act_label = format!("Action: {} ({})", act.label(), if combo_str.is_empty() { "Unbound" } else { &combo_str });
                let act_desc = format!("Actions › {} — {}", act.category().label(), act.description());
                let filter_q = act.label().to_string();
                results.push(
                    Self::settings_clickable_card(
                        Some(act.icon()),
                        &act_label,
                        Some(&act_desc),
                        "\u{E70F}",
                        move |_, _, cx| {
                            v.update(cx, |this, cx| {
                                this.state.settings_ui.search_query.clear();
                                this.state.settings_ui.search_focused = false;
                                this.state.settings_ui.actions_search_query = filter_q.clone();
                                this.state
                                    .settings_ui
                                    .expanded_sections
                                    .insert("actions_commands".to_string());
                                this.set_settings_tab(SettingsTab::Actions, cx);
                            });
                        },
                    )
                    .into_any_element(),
                );
            }
        }

        div()
            .flex()
            .flex_col()
            .gap(px(6.0))
            .child(
                div()
                    .text_size(px(16.0))
                    .font_weight(FontWeight(600.0))
                    .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                    .mb(px(8.0))
                    .child(format!(
                        "Search results for \"{}\" ({})",
                        query,
                        results.len()
                    )),
            )
            .children(if results.is_empty() {
                vec![div()
                    .py(px(24.0))
                    .text_size(px(13.0))
                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                    .child("No settings or actions matched your search query.")
                    .into_any_element()]
            } else {
                results
            })
    }

    // ─────────────────────────────────────────────────────────────────────────
    // 1. GENERAL PAGE (Files-community GeneralPage.xaml parity)
    // ─────────────────────────────────────────────────────────────────────────

    fn build_general_settings(
        config: &crate::config::config::AppConfig,
        data: &SettingsPreReadData,
        view: Entity<crate::app::AppWindow>,
    ) -> Div {
        let locales = [
            ("en", "English (US)"),
            ("es", "Español"),
            ("fr", "Français"),
            ("de", "Deutsch"),
            ("ja", "日本語"),
        ];
        let mut locale_pills = div().flex().flex_row().gap(px(4.0)).flex_wrap();
        for (code, label) in locales {
            let v = view.clone();
            let c_str = code.to_string();
            let is_sel = config.preferences.locale == code;
            locale_pills = locale_pills.child(Self::build_compact_btn(
                label,
                is_sel,
                move |_, _, cx| {
                    v.update(cx, |this, cx| {
                        this.set_string_setting("locale", c_str.clone(), cx);
                    });
                },
            ));
        }

        let date_formats = [
            ("application", "Application (Relative)"),
            ("system", "System (2026-10-23 14:30)"),
            ("iso8601", "ISO 8601"),
            ("universal", "Universal"),
        ];
        let mut date_pills = div().flex().flex_row().gap(px(4.0)).flex_wrap();
        for (code, label) in date_formats {
            let v = view.clone();
            let c_str = code.to_string();
            let is_sel = config.preferences.date_format == code;
            date_pills = date_pills.child(Self::build_compact_btn(
                label,
                is_sel,
                move |_, _, cx| {
                    v.update(cx, |this, cx| {
                        this.set_string_setting("date_format", c_str.clone(), cx);
                    });
                },
            ));
        }

        // Startup Settings Expander
        let startup_modes = [
            ("new_tab", "Open a new tab"),
            ("continue_session", "Continue where you left off"),
            ("specific_pages", "Open specific page(s)"),
        ];
        let mut startup_pills = div().flex().flex_row().gap(px(4.0)).flex_wrap();
        for (code, label) in startup_modes {
            let v = view.clone();
            let c_str = code.to_string();
            let is_sel = config.session.startup_mode == code;
            startup_pills = startup_pills.child(Self::build_compact_btn(
                label,
                is_sel,
                move |_, _, cx| {
                    v.update(cx, |this, cx| {
                        this.set_string_setting("startup_mode", c_str.clone(), cx);
                    });
                },
            ));
        }

        let mut startup_items: Vec<AnyElement> = vec![Self::settings_expander_row(
            "Open tab in existing instance",
            Some("Reuse the running Files-Rust window when opening folders from external apps"),
            Self::settings_toggle_switch(
                config.session.open_tab_in_existing_instance,
                "open_tab_in_existing_instance",
                view.clone(),
            ),
        )
        .into_any_element()];

        if config.session.startup_mode == "specific_pages" {
            let v_add_home = view.clone();
            let v_add_cur = view.clone();
            startup_items.push(
                Self::settings_expander_row(
                    "Startup pages",
                    Some(&format!(
                        "Configured: {}",
                        if config.session.startup_pages.is_empty() {
                            "Home".to_string()
                        } else {
                            config.session.startup_pages.join(", ")
                        }
                    )),
                    div()
                        .flex()
                        .flex_row()
                        .gap(px(6.0))
                        .child(Self::build_compact_btn(
                            "+ Add Home",
                            false,
                            move |_, _, cx| {
                                v_add_home.update(cx, |this, cx| {
                                    this.add_startup_page("Home".to_string(), cx);
                                });
                            },
                        ))
                        .child(Self::build_compact_btn(
                            "+ Add Current Folder",
                            false,
                            move |_, _, cx| {
                                v_add_cur.update(cx, |this, cx| {
                                    let p = this
                                        .state
                                        .main_view
                                        .primary_path
                                        .to_string_lossy()
                                        .to_string();
                                    this.add_startup_page(p, cx);
                                });
                            },
                        )),
                )
                .into_any_element(),
            );
        }

        // Widgets Expander
        let widgets_items: Vec<AnyElement> = vec![
            Self::settings_expander_row(
                "Quick Access",
                Some("Display pinned folders and libraries on the Home page"),
                Self::settings_toggle_switch(
                    config.home_widgets.show_quick_access,
                    "widget_quick_access",
                    view.clone(),
                ),
            )
            .into_any_element(),
            Self::settings_expander_row(
                "Drives",
                Some("Display local, removable, and cloud storage drives with capacity bars"),
                Self::settings_toggle_switch(
                    config.home_widgets.show_drives,
                    "widget_drives",
                    view.clone(),
                ),
            )
            .into_any_element(),
            Self::settings_expander_row(
                "Network Locations",
                Some("Display mapped SMB shares and network locations"),
                Self::settings_toggle_switch(
                    config.home_widgets.show_network_locations,
                    "widget_network",
                    view.clone(),
                ),
            )
            .into_any_element(),
            Self::settings_expander_row(
                "File Tags",
                Some("Display tagged file containers grouped by tag color"),
                Self::settings_toggle_switch(
                    config.home_widgets.show_file_tags,
                    "widget_file_tags",
                    view.clone(),
                ),
            )
            .into_any_element(),
            Self::settings_expander_row(
                "Recent Files",
                Some("Display recently accessed files on the Home page"),
                Self::settings_toggle_switch(
                    config.home_widgets.show_recent_files,
                    "widget_recent_files",
                    view.clone(),
                ),
            )
            .into_any_element(),
        ];

        // Context Menu Options Expander
        let ctx_items: Vec<AnyElement> = vec![
            Self::settings_expander_row(
                "Show \"Open in new tab\"",
                None,
                Self::settings_toggle_switch(
                    config.preferences.ctx_show_open_in_new_tab,
                    "ctx_show_open_in_new_tab",
                    view.clone(),
                ),
            )
            .into_any_element(),
            Self::settings_expander_row(
                "Show \"Open in new window\"",
                None,
                Self::settings_toggle_switch(
                    config.preferences.ctx_show_open_in_new_window,
                    "ctx_show_open_in_new_window",
                    view.clone(),
                ),
            )
            .into_any_element(),
            Self::settings_expander_row(
                "Show \"Open in new pane\"",
                None,
                Self::settings_toggle_switch(
                    config.preferences.ctx_show_open_in_new_pane,
                    "ctx_show_open_in_new_pane",
                    view.clone(),
                ),
            )
            .into_any_element(),
            Self::settings_expander_row(
                "Show \"Copy path\"",
                None,
                Self::settings_toggle_switch(
                    config.preferences.ctx_show_copy_path,
                    "ctx_show_copy_path",
                    view.clone(),
                ),
            )
            .into_any_element(),
            Self::settings_expander_row(
                "Show \"Create folder with selection\"",
                None,
                Self::settings_toggle_switch(
                    config.preferences.ctx_show_create_folder_with_selection,
                    "ctx_show_create_folder_with_selection",
                    view.clone(),
                ),
            )
            .into_any_element(),
            Self::settings_expander_row(
                "Show \"Create shortcut\"",
                None,
                Self::settings_toggle_switch(
                    config.preferences.ctx_show_create_shortcut,
                    "ctx_show_create_shortcut",
                    view.clone(),
                ),
            )
            .into_any_element(),
            Self::settings_expander_row(
                "Show \"Pin to Sidebar\"",
                None,
                Self::settings_toggle_switch(
                    config.preferences.ctx_show_pin_to_sidebar,
                    "ctx_show_pin_to_sidebar",
                    view.clone(),
                ),
            )
            .into_any_element(),
            Self::settings_expander_row(
                "Show compression options (Compress / Extract)",
                None,
                Self::settings_toggle_switch(
                    config.preferences.ctx_show_compression_options,
                    "ctx_show_compression_options",
                    view.clone(),
                ),
            )
            .into_any_element(),
            Self::settings_expander_row(
                "Show \"Send To\" menu",
                None,
                Self::settings_toggle_switch(
                    config.preferences.ctx_show_send_to,
                    "ctx_show_send_to",
                    view.clone(),
                ),
            )
            .into_any_element(),
            Self::settings_expander_row(
                "Show \"Open in Terminal\"",
                None,
                Self::settings_toggle_switch(
                    config.preferences.ctx_show_open_terminal,
                    "ctx_show_open_terminal",
                    view.clone(),
                ),
            )
            .into_any_element(),
            Self::settings_expander_row(
                "Show \"Edit file tags\" menu",
                None,
                Self::settings_toggle_switch(
                    config.preferences.ctx_show_edit_tags,
                    "ctx_show_edit_tags",
                    view.clone(),
                ),
            )
            .into_any_element(),
        ];

        let mut page = div()
            .flex()
            .flex_col()
            .gap(px(4.0))
            .child(Self::settings_card(
                Some("\u{F2B7}"),
                "Language",
                Some("Display language for the application interface"),
                locale_pills,
            ))
            .child(Self::settings_card(
                Some("\u{EC92}"),
                "Date format",
                Some("Yesterday • Friday, October 23, 2026"),
                date_pills,
            ))
            .child(Self::settings_expander(
                "general_startup",
                data.ui_state.is_expanded("general_startup"),
                "\u{E7E8}",
                "Startup settings",
                Some("Choose what happens when launching Files"),
                Some(startup_pills.into_any_element()),
                startup_items,
                view.clone(),
            ))
            .child(Self::settings_card(
                Some("\u{E8AB}"),
                "Always switch to newly opened tab",
                Some("Immediately focus new tabs when opening folders in a new tab"),
                Self::settings_toggle_switch(
                    config.session.always_switch_to_new_tab,
                    "always_switch_to_new_tab",
                    view.clone(),
                ),
            ))
            .child(Self::settings_card(
                Some("\u{E8AB}"),
                "Cycle tabs in recently-used order (Ctrl+Tab)",
                Some("Switch tabs in MRU order instead of left-to-right sequential order"),
                Self::settings_toggle_switch(
                    config.session.mru_tab_cycle,
                    "mru_tab_cycle",
                    view.clone(),
                ),
            ))
            .child(Self::settings_expander(
                "general_widgets",
                data.ui_state.is_expanded("general_widgets"),
                "\u{F246}",
                "Widgets",
                Some("Manage which widget sections appear on the Home page"),
                None,
                widgets_items,
                view.clone(),
            ));

        page = page
            // Dual Pane
            .child(Self::settings_section_heading("Dual Pane"))
            .child(Self::settings_card(
                Some("\u{E89F}"),
                "Always open new tabs in dual pane mode",
                Some("Automatically split new tabs into Primary and Secondary panes"),
                Self::settings_toggle_switch(
                    config.view.always_open_dual_pane_in_new_tab,
                    "always_open_dual_pane_in_new_tab",
                    view.clone(),
                ),
            ))
            .child(Self::settings_card(
                Some("\u{E784}"),
                "Dual pane split direction",
                Some("Choose side-by-side (vertical divider) or stacked (horizontal divider)"),
                div()
                    .flex()
                    .flex_row()
                    .gap(px(4.0))
                    .child(Self::build_compact_btn(
                        "Side-by-Side (Vertical)",
                        config.session.pane_layout != "horizontal",
                        {
                            let v = view.clone();
                            move |_, _, cx| {
                                v.update(cx, |this, cx| {
                                    this.state.main_view.dual_pane_split =
                                        crate::ui::views::main_view::DualPaneSplit::Horizontal;
                                    this.set_string_setting(
                                        "pane_layout",
                                        "vertical".to_string(),
                                        cx,
                                    );
                                });
                            }
                        },
                    ))
                    .child(Self::build_compact_btn(
                        "Top-and-Bottom (Horizontal)",
                        config.session.pane_layout == "horizontal",
                        {
                            let v = view.clone();
                            move |_, _, cx| {
                                v.update(cx, |this, cx| {
                                    this.state.main_view.dual_pane_split =
                                        crate::ui::views::main_view::DualPaneSplit::Vertical;
                                    this.set_string_setting(
                                        "pane_layout",
                                        "horizontal".to_string(),
                                        cx,
                                    );
                                });
                            }
                        },
                    )),
            ));

        page
            // Right-Click Menu
            .child(Self::settings_section_heading("Right-click menu"))
            .child(Self::settings_card(
                Some("\u{E74C}"),
                "Context menu presentation style",
                Some("Choose between Files modern flyout menu, expanded inline menu, or native Windows shell menu"),
                div()
                    .flex()
                    .flex_row()
                    .gap(px(4.0))
                    .child(Self::build_compact_btn(
                        "Files style",
                        config.preferences.context_menu_style == ContextMenuStyle::Files,
                        {
                            let v = view.clone();
                            move |_, _, cx| {
                                v.update(cx, |this, cx| {
                                    this.set_context_menu_style(ContextMenuStyle::Files, cx);
                                });
                            }
                        },
                    ))
                    .child(Self::build_compact_btn(
                        "Expanded",
                        config.preferences.context_menu_style == ContextMenuStyle::Expanded,
                        {
                            let v = view.clone();
                            move |_, _, cx| {
                                v.update(cx, |this, cx| {
                                    this.set_context_menu_style(ContextMenuStyle::Expanded, cx);
                                });
                            }
                        },
                    ))
                    .child(Self::build_compact_btn(
                        "Windows classic",
                        config.preferences.context_menu_style == ContextMenuStyle::WindowsClassic,
                        {
                            let v = view.clone();
                            move |_, _, cx| {
                                v.update(cx, |this, cx| {
                                    this.set_context_menu_style(
                                        ContextMenuStyle::WindowsClassic,
                                        cx,
                                    );
                                });
                            }
                        },
                    )),
            ))
            .child(Self::settings_expander(
                "general_context_menu",
                data.ui_state.is_expanded("general_context_menu"),
                "\u{E74C}",
                "Context menu options",
                Some("Show or hide specific actions in the right-click context menu"),
                None,
                ctx_items,
                view.clone(),
            ))
            .child(Self::settings_card(
                Some("\u{E712}"),
                "Move shell extensions into a sub menu",
                Some("Keep third-party Windows shell extensions grouped inside \"Show more options\""),
                Self::settings_toggle_switch(
                    config.preferences.move_shell_extensions_to_submenu,
                    "move_shell_extensions_to_submenu",
                    view.clone(),
                ),
            ))
            // Properties Dialog
            .child(Self::settings_section_heading("Properties dialog"))
            .child(Self::settings_card(
                Some("\u{E946}"),
                "Properties dialog style",
                Some("Choose between the modern redesigned Files properties panel or revert to the original Windows properties dialog"),
                div()
                    .flex()
                    .flex_row()
                    .gap(px(4.0))
                    .child(Self::build_compact_btn(
                        "Files modern",
                        !config.preferences.use_classic_properties_dialog,
                        {
                            let v = view.clone();
                            move |_, _, cx| {
                                v.update(cx, |this, cx| {
                                    this.set_use_classic_properties_dialog(false, cx);
                                });
                            }
                        },
                    ))
                    .child(Self::build_compact_btn(
                        "Windows classic (Original)",
                        config.preferences.use_classic_properties_dialog,
                        {
                            let v = view.clone();
                            move |_, _, cx| {
                                v.update(cx, |this, cx| {
                                    this.set_use_classic_properties_dialog(true, cx);
                                });
                            }
                        },
                    )),
            ))
            // Scrolling
            .child(Self::settings_section_heading("Scrolling"))
            .child(Self::settings_card(
                Some("\u{EC8F}"),
                "Enable smooth scrolling",
                Some("Use display-refresh-rate spring physics for smooth scrolling"),
                Self::settings_toggle_switch(
                    config.preferences.enable_smooth_scrolling,
                    "enable_smooth_scrolling",
                    view.clone(),
                ),
            ))
            .child(Self::settings_card(
                Some("\u{ECE7}"),
                "Reverse tab scroll direction",
                Some("Control which direction the mouse wheel scrolls the tab strip"),
                div()
                    .flex()
                    .flex_row()
                    .gap(px(4.0))
                    .child(Self::build_compact_btn(
                        "Down scrolls right",
                        !config.preferences.reverse_tab_scroll_direction,
                        {
                            let v = view.clone();
                            move |_, _, cx| {
                                v.update(cx, |this, cx| {
                                    if this
                                        .state
                                        .settings
                                        .read(cx)
                                        .config()
                                        .preferences
                                        .reverse_tab_scroll_direction
                                    {
                                        this.toggle_setting("reverse_tab_scroll_direction", cx);
                                    }
                                });
                            }
                        },
                    ))
                    .child(Self::build_compact_btn(
                        "Down scrolls left",
                        config.preferences.reverse_tab_scroll_direction,
                        {
                            let v = view.clone();
                            move |_, _, cx| {
                                v.update(cx, |this, cx| {
                                    if !this
                                        .state
                                        .settings
                                        .read(cx)
                                        .config()
                                        .preferences
                                        .reverse_tab_scroll_direction
                                    {
                                        this.toggle_setting("reverse_tab_scroll_direction", cx);
                                    }
                                });
                            }
                        },
                    )),
            ))
    }

    // ─────────────────────────────────────────────────────────────────────────
    // 2. APPEARANCE PAGE (Files-community AppearancePage.xaml parity)
    // ─────────────────────────────────────────────────────────────────────────

    fn build_appearance_settings(
        config: &crate::config::config::AppConfig,
        data: &SettingsPreReadData,
        view: Entity<crate::app::AppWindow>,
    ) -> Div {
        let themes = [
            ("system", "Default (System)"),
            ("dark", "Dark"),
            ("light", "Light"),
            ("high_contrast", "High Contrast"),
        ];
        let mut theme_pills = div().flex().flex_row().gap(px(4.0)).flex_wrap();
        for (code, label) in themes {
            let v = view.clone();
            let c_str = code.to_string();
            let is_sel = config.appearance.theme == code;
            theme_pills = theme_pills.child(Self::build_compact_btn(
                label,
                is_sel,
                move |_, _, cx| {
                    v.update(cx, |this, cx| {
                        this.set_theme(c_str.clone(), cx);
                    });
                },
            ));
        }

        let backdrops = [
            ("mica_alt", "Mica Alt"),
            ("mica", "Mica"),
            ("acrylic", "Acrylic"),
            ("solid", "None (Solid)"),
        ];
        let mut backdrop_pills = div().flex().flex_row().gap(px(4.0)).flex_wrap();
        for (code, label) in backdrops {
            let v = view.clone();
            let c_str = code.to_string();
            let is_sel = config.appearance.backdrop_material == code;
            backdrop_pills = backdrop_pills.child(Self::build_compact_btn(
                label,
                is_sel,
                move |_, win, cx| {
                    v.update(cx, |this, cx| {
                        this.set_backdrop_material(c_str.clone(), cx);
                    });
                    Self::apply_chrome_live("mica_enabled", win, &v, cx);
                },
            ));
        }

        // Visual mini-window theme preset cards (Files-community AppThemeResourcesItemTemplate parity)
        let presets: &[(&str, u32, u32)] = &[
            ("Default", 0x1F1F1F, 0x0078D4),
            ("Cool Blue", 0x132235, 0x3B9EFF),
            ("Emerald", 0x132A22, 0x10B981),
            ("Amethyst", 0x241734, 0xA855F7),
            ("Sunset Gold", 0x2B2114, 0xF59E0B),
            ("Crimson", 0x2D161B, 0xEF4444),
            ("Slate", 0x1E242B, 0x64748B),
        ];
        let mut preset_grid = div()
            .w_full()
            .p(px(12.0))
            .bg(rgba(0x00000018u32))
            .border_t(px(1.0))
            .border_color(rgba(UiConstants::CARD_STROKE_RGBA))
            .flex()
            .flex_row()
            .flex_wrap()
            .gap(px(10.0));

        for &(name, bg_hex, accent_hex) in presets {
            let is_sel = config.appearance.theme_preset == name;
            let v = view.clone();
            let n_str = name.to_string();
            preset_grid = preset_grid.child(
                div()
                    .w(px(116.0))
                    .rounded(px(6.0))
                    .border(px(if is_sel { 2.0 } else { 1.0 }))
                    .border_color(if is_sel {
                        rgb(accent_hex)
                    } else {
                        rgba(UiConstants::STROKE_RGBA)
                    })
                    .overflow_hidden()
                    .cursor_pointer()
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        v.update(cx, |this, cx| {
                            this.set_string_setting("theme_preset", n_str.clone(), cx);
                        });
                    })
                    .child(
                        // Mini Unibody Window Preview
                        div()
                            .h(px(58.0))
                            .w_full()
                            .bg(rgb(bg_hex))
                            .flex()
                            .flex_col()
                            .child(
                                // Mini tab strip
                                div()
                                    .h(px(14.0))
                                    .px(px(6.0))
                                    .pt(px(3.0))
                                    .flex()
                                    .items_end()
                                    .gap(px(4.0))
                                    .child(
                                        div()
                                            .w(px(34.0))
                                            .h(px(11.0))
                                            .bg(rgba(0xFFFFFF1Eu32))
                                            .rounded_t(px(3.0))
                                            .border_b(px(2.0))
                                            .border_color(rgb(accent_hex)),
                                    )
                                    .child(
                                        div()
                                            .w(px(24.0))
                                            .h(px(9.0))
                                            .bg(rgba(0xFFFFFF0Au32))
                                            .rounded_t(px(3.0)),
                                    ),
                            )
                            .child(
                                // Mini content area
                                div()
                                    .flex_1()
                                    .m(px(4.0))
                                    .rounded(px(4.0))
                                    .bg(rgba(0xFFFFFF10u32))
                                    .border(px(1.0))
                                    .border_color(rgba(0xFFFFFF14u32)),
                            ),
                    )
                    .child(
                        div()
                            .py(px(5.0))
                            .px(px(6.0))
                            .bg(rgba(0xFFFFFF08u32))
                            .text_size(px(11.5))
                            .text_center()
                            .font_weight(if is_sel {
                                FontWeight(600.0)
                            } else {
                                FontWeight(400.0)
                            })
                            .text_color(if is_sel {
                                rgb(0xFFFFFF)
                            } else {
                                rgba(UiConstants::TEXT_SECONDARY_RGBA)
                            })
                            .child(name),
                    ),
            );
        }

        let theme_color_items: Vec<AnyElement> = vec![
            preset_grid.into_any_element(),
            Self::settings_expander_row(
                "Enable DWM Mica backdrop",
                Some("Sample desktop wallpaper blur behind the unibody window frame"),
                Self::settings_toggle_switch(
                    config.appearance.mica_enabled,
                    "mica_enabled",
                    view.clone(),
                ),
            )
            .into_any_element(),
            Self::settings_expander_row(
                "Use saturated Mica Alt tint",
                Some("Deeper contrast between the unibody tab bar/sidebar and file canvas"),
                Self::settings_toggle_switch(
                    config.appearance.mica_alt,
                    "mica_alt",
                    view.clone(),
                ),
            )
            .into_any_element(),
            Self::settings_expander_row(
                "Per-pixel transparent swapchain",
                Some("Allow translucent surface alpha to blend directly with Windows DWM"),
                Self::settings_toggle_switch(
                    config.appearance.transparent_window,
                    "transparent_window",
                    view.clone(),
                ),
            )
            .into_any_element(),
        ];

        // Fonts
        let fonts = [
            "Segoe UI Variable Text",
            "Segoe UI",
            "Cascadia Code",
            "Inter",
            "Consolas",
        ];
        let mut font_pills = div().flex().flex_row().gap(px(4.0)).flex_wrap();
        for f in fonts {
            let v = view.clone();
            let f_str = f.to_string();
            let is_sel = config.appearance.font_family == f;
            font_pills = font_pills.child(Self::build_compact_btn(
                f,
                is_sel,
                move |_, _, cx| {
                    v.update(cx, |this, cx| {
                        let se = this.state.settings.clone();
                        let eid = se.entity_id();
                        se.update(cx, |s, cx| {
                            s.set_font_family(f_str.clone(), cx, eid);
                        });
                        cx.notify();
                    });
                },
            ));
        }

        let cur_size = config.appearance.font_size;
        let v_dec = view.clone();
        let v_inc = view.clone();
        let font_size_ctrl = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.0))
            .child(Self::build_compact_btn("−", false, move |_, _, cx| {
                v_dec.update(cx, |this, cx| {
                    let se = this.state.settings.clone();
                    let eid = se.entity_id();
                    let next = (se.read(cx).config().appearance.font_size - 1.0).max(10.0);
                    se.update(cx, |s, cx| s.set_font_size(next, cx, eid));
                    cx.notify();
                });
            }))
            .child(
                div()
                    .min_w(px(48.0))
                    .text_center()
                    .text_size(px(13.0))
                    .font_weight(FontWeight(600.0))
                    .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                    .child(format!("{:.0} px", cur_size)),
            )
            .child(Self::build_compact_btn("+", false, move |_, _, cx| {
                v_inc.update(cx, |this, cx| {
                    let se = this.state.settings.clone();
                    let eid = se.entity_id();
                    let next = (se.read(cx).config().appearance.font_size + 1.0).min(20.0);
                    se.update(cx, |s, cx| s.set_font_size(next, cx, eid));
                    cx.notify();
                });
            }));

        // Address bar expander
        let address_bar_items: Vec<AnyElement> = vec![
            Self::settings_expander_row(
                "Show Shelf pane toggle button",
                Some("Display the file staging Shelf toggle button in the chrome"),
                Self::settings_toggle_switch(
                    config.appearance.show_shelf_button,
                    "show_shelf_button",
                    view.clone(),
                ),
            )
            .into_any_element(),
            Self::settings_expander_row(
                "Show Status Center button",
                Some("Control when the ongoing operations & notifications button is visible"),
                div()
                    .flex()
                    .flex_row()
                    .gap(px(4.0))
                    .child(Self::build_compact_btn(
                        "Always",
                        config.appearance.status_center_visibility == "always",
                        {
                            let v = view.clone();
                            move |_, _, cx| {
                                v.update(cx, |this, cx| {
                                    this.set_string_setting(
                                        "status_center_visibility",
                                        "always".to_string(),
                                        cx,
                                    );
                                });
                            }
                        },
                    ))
                    .child(Self::build_compact_btn(
                        "When active",
                        config.appearance.status_center_visibility == "active_only",
                        {
                            let v = view.clone();
                            move |_, _, cx| {
                                v.update(cx, |this, cx| {
                                    this.set_string_setting(
                                        "status_center_visibility",
                                        "active_only".to_string(),
                                        cx,
                                    );
                                });
                            }
                        },
                    ))
                    .child(Self::build_compact_btn(
                        "Never",
                        config.appearance.status_center_visibility == "never",
                        {
                            let v = view.clone();
                            move |_, _, cx| {
                                v.update(cx, |this, cx| {
                                    this.set_string_setting(
                                        "status_center_visibility",
                                        "never".to_string(),
                                        cx,
                                    );
                                });
                            }
                        },
                    )),
            )
            .into_any_element(),
        ];

        // Toolbar expander
        let v_goto_tb = view.clone();
        let toolbar_items: Vec<AnyElement> = vec![
            Self::settings_expander_row(
                "Show command labels alongside icons",
                Some("Display text labels next to icons in the top ribbon command bar"),
                Self::settings_toggle_switch(
                    config.appearance.show_toolbar_labels,
                    "show_toolbar_labels",
                    view.clone(),
                ),
            )
            .into_any_element(),
            Self::settings_expander_row(
                "Customize toolbar buttons",
                Some("Choose which commands appear on the inner command bar ribbon"),
                Self::build_compact_btn("Customize Toolbar \u{E8A7}", false, move |_, _, cx| {
                    v_goto_tb.update(cx, |this, cx| {
                        this.set_settings_tab(SettingsTab::Toolbar, cx);
                    });
                }),
            )
            .into_any_element(),
        ];

        div()
            .flex()
            .flex_col()
            .gap(px(4.0))
            .child(Self::settings_card(
                Some("\u{E790}"),
                "Theme",
                Some("Select the color theme used across the unibody shell"),
                theme_pills,
            ))
            .child(Self::settings_card(
                Some("\u{EF1F}"),
                "Backdrop material",
                Some("Select the window translucency material behind the app"),
                backdrop_pills,
            ))
            .child(Self::settings_expander(
                "appearance_theme_colors",
                data.ui_state.is_expanded("appearance_theme_colors"),
                "\u{E771}",
                "Background color & theme presets",
                Some(&format!("Active preset: {}", config.appearance.theme_preset)),
                None,
                theme_color_items,
                view.clone(),
            ))
            .child(Self::settings_card(
                Some("\u{E8D2}"),
                "Font family",
                Some("Typography used across file lists, breadcrumbs, and panels"),
                font_pills,
            ))
            .child(Self::settings_card(
                Some("\u{E8E9}"),
                "Font size",
                Some("Base font size in pixels for the interface"),
                font_size_ctrl,
            ))
            .child(Self::settings_card(
                Some("\u{E8AB}"),
                "Show tab actions button",
                Some("Display the open-tabs switcher and tab actions button in the tab bar"),
                Self::settings_toggle_switch(
                    config.appearance.show_tab_actions,
                    "show_tab_actions",
                    view.clone(),
                ),
            ))
            .child(Self::settings_expander(
                "appearance_address_bar",
                data.ui_state.is_expanded("appearance_address_bar"),
                "\u{E71B}",
                "Address bar",
                Some("Configure buttons shown in the navigation & omnibar row"),
                None,
                address_bar_items,
                view.clone(),
            ))
            .child(Self::settings_expander(
                "appearance_toolbar",
                data.ui_state.is_expanded("appearance_toolbar"),
                "\u{E712}",
                "Toolbar",
                Some("Configure the top command bar ribbon above the file area"),
                Some(
                    Self::settings_toggle_switch(
                        config.appearance.show_toolbar,
                        "show_toolbar",
                        view.clone(),
                    )
                    .into_any_element(),
                ),
                toolbar_items,
                view.clone(),
            ))
            .child(Self::settings_card(
                Some("\u{E946}"),
                "Show status bar",
                Some("Display item counts, selection size, Git branch, and zoom controls at the bottom"),
                Self::settings_toggle_switch(
                    config.appearance.show_status_bar,
                    "show_status_bar",
                    view.clone(),
                ),
            ))
    }

    // ─────────────────────────────────────────────────────────────────────────
    // 3. LAYOUT PAGE (Files-community LayoutPage.xaml parity)
    // ─────────────────────────────────────────────────────────────────────────

    fn build_layout_settings(
        config: &crate::config::config::AppConfig,
        data: &SettingsPreReadData,
        view: Entity<crate::app::AppWindow>,
    ) -> Div {
        let layout_modes = [
            (ViewMode::Details, "Details"),
            (ViewMode::List, "List"),
            (ViewMode::Cards, "Cards"),
            (ViewMode::Grid, "Grid"),
            (ViewMode::Columns, "Columns"),
        ];
        let mut mode_pills = div().flex().flex_row().gap(px(4.0)).flex_wrap();
        for (m, label) in layout_modes {
            let v = view.clone();
            let is_sel = config.view.default_view_mode == m && !config.view.adaptive_layout_enabled;
            mode_pills = mode_pills.child(Self::build_compact_btn(
                label,
                is_sel,
                move |_, _, cx| {
                    v.update(cx, |this, cx| {
                        this.state.settings.update(cx, |s, _| {
                            s.config.view.default_view_mode = m;
                            s.config.view.adaptive_layout_enabled = false;
                            let _ = s.config.save();
                        });
                        this.set_view_mode(m, cx);
                    });
                },
            ));
        }
        {
            let v = view.clone();
            mode_pills = mode_pills.child(Self::build_compact_btn(
                "Adaptive",
                config.view.adaptive_layout_enabled,
                move |_, _, cx| {
                    v.update(cx, |this, cx| {
                        this.toggle_setting("adaptive_layout_enabled", cx);
                    });
                },
            ));
        }

        // Sort By Expander
        let sort_cols = [
            (SortBy::Name, "Name"),
            (SortBy::DateModified, "Date modified"),
            (SortBy::DateCreated, "Date created"),
            (SortBy::Size, "Size"),
            (SortBy::Type, "Type"),
            (SortBy::Tag, "Tag"),
        ];
        let mut sort_pills = div().flex().flex_row().gap(px(4.0)).flex_wrap();
        for (col, label) in sort_cols {
            let v = view.clone();
            let col_clone = col.clone();
            let is_sel = config.view.sort_by == col;
            sort_pills = sort_pills.child(Self::build_compact_btn(
                label,
                is_sel,
                move |_, _, cx| {
                    let c = col_clone.clone();
                    v.update(cx, move |this, cx| {
                        this.state.main_view.sort.column = c.clone();
                        this.state.main_view.bump_items_revision();
                        this.state.settings.update(cx, |s, _| {
                            s.config.view.sort_by = c.clone();
                            let _ = s.config.save();
                        });
                        cx.notify();
                    });
                },
            ));
        }

        let sort_items: Vec<AnyElement> = vec![
            Self::settings_expander_row(
                "Sort in descending order",
                Some("Reverse default sort order (Z to A, newest first, largest first)"),
                Self::settings_toggle_switch(
                    !config.view.sort_ascending,
                    "sort_descending",
                    view.clone(),
                ),
            )
            .into_any_element(),
            Self::settings_expander_row(
                "Sort priority",
                Some("Control how folders and files are ordered relative to each other"),
                div()
                    .flex()
                    .flex_row()
                    .gap(px(4.0))
                    .child(Self::build_compact_btn(
                        "Sort folders first",
                        config.view.sort_priority == "folders_first",
                        {
                            let v = view.clone();
                            move |_, _, cx| {
                                v.update(cx, |this, cx| {
                                    this.set_string_setting(
                                        "sort_priority",
                                        "folders_first".to_string(),
                                        cx,
                                    );
                                });
                            }
                        },
                    ))
                    .child(Self::build_compact_btn(
                        "Sort files first",
                        config.view.sort_priority == "files_first",
                        {
                            let v = view.clone();
                            move |_, _, cx| {
                                v.update(cx, |this, cx| {
                                    this.set_string_setting(
                                        "sort_priority",
                                        "files_first".to_string(),
                                        cx,
                                    );
                                });
                            }
                        },
                    ))
                    .child(Self::build_compact_btn(
                        "Sort files & folders together",
                        config.view.sort_priority == "together",
                        {
                            let v = view.clone();
                            move |_, _, cx| {
                                v.update(cx, |this, cx| {
                                    this.set_string_setting(
                                        "sort_priority",
                                        "together".to_string(),
                                        cx,
                                    );
                                });
                            }
                        },
                    )),
            )
            .into_any_element(),
        ];

        // Group By Expander
        let group_cols = [
            (GroupColumn::None, "None"),
            (GroupColumn::Name, "Name"),
            (GroupColumn::DateModified, "Date modified"),
            (GroupColumn::DateCreated, "Date created"),
            (GroupColumn::Size, "Size"),
            (GroupColumn::FileType, "Type"),
            (GroupColumn::SyncStatus, "Sync status"),
            (GroupColumn::Tag, "Tag"),
        ];
        let mut group_pills = div().flex().flex_row().gap(px(4.0)).flex_wrap();
        for (gc, label) in group_cols {
            let v = view.clone();
            let is_sel = config.view.group_by == gc;
            group_pills = group_pills.child(Self::build_compact_btn(
                label,
                is_sel,
                move |_, _, cx| {
                    v.update(cx, move |this, cx| {
                        this.state.main_view.group_by = gc;
                        this.state.main_view.collapsed_groups.clear();
                        this.state.settings.update(cx, |s, _| {
                            s.config.view.group_by = gc;
                            let _ = s.config.save();
                        });
                        cx.notify();
                    });
                },
            ));
        }

        let group_items: Vec<AnyElement> = vec![
            Self::settings_expander_row(
                "Group in descending order",
                None,
                Self::settings_toggle_switch(
                    !config.view.group_ascending,
                    "group_descending",
                    view.clone(),
                ),
            )
            .into_any_element(),
            Self::settings_expander_row(
                "Group by date unit",
                Some("Granularity when grouping by Date modified or Date created"),
                div()
                    .flex()
                    .flex_row()
                    .gap(px(4.0))
                    .child(Self::build_compact_btn(
                        "Year",
                        config.view.group_by_date_unit == "year",
                        {
                            let v = view.clone();
                            move |_, _, cx| {
                                v.update(cx, |this, cx| {
                                    this.set_string_setting(
                                        "group_by_date_unit",
                                        "year".to_string(),
                                        cx,
                                    );
                                });
                            }
                        },
                    ))
                    .child(Self::build_compact_btn(
                        "Month",
                        config.view.group_by_date_unit == "month",
                        {
                            let v = view.clone();
                            move |_, _, cx| {
                                v.update(cx, |this, cx| {
                                    this.set_string_setting(
                                        "group_by_date_unit",
                                        "month".to_string(),
                                        cx,
                                    );
                                });
                            }
                        },
                    ))
                    .child(Self::build_compact_btn(
                        "Day",
                        config.view.group_by_date_unit == "day",
                        {
                            let v = view.clone();
                            move |_, _, cx| {
                                v.update(cx, |this, cx| {
                                    this.set_string_setting(
                                        "group_by_date_unit",
                                        "day".to_string(),
                                        cx,
                                    );
                                });
                            }
                        },
                    )),
            )
            .into_any_element(),
        ];

        // Columns Expander
        let column_items: Vec<AnyElement> = vec![
            Self::settings_expander_row(
                "Date modified column",
                None,
                Self::settings_toggle_switch(
                    config.view.show_date_column,
                    "show_date_column",
                    view.clone(),
                ),
            )
            .into_any_element(),
            Self::settings_expander_row(
                "Date created column",
                None,
                Self::settings_toggle_switch(
                    config.view.show_date_created_column,
                    "show_date_created_column",
                    view.clone(),
                ),
            )
            .into_any_element(),
            Self::settings_expander_row(
                "Type column",
                None,
                Self::settings_toggle_switch(
                    config.view.show_type_column,
                    "show_type_column",
                    view.clone(),
                ),
            )
            .into_any_element(),
            Self::settings_expander_row(
                "Size column",
                None,
                Self::settings_toggle_switch(
                    config.view.show_size_column,
                    "show_size_column",
                    view.clone(),
                ),
            )
            .into_any_element(),
            Self::settings_expander_row(
                "File tag column",
                None,
                Self::settings_toggle_switch(
                    config.view.show_tag_column,
                    "show_tag_column",
                    view.clone(),
                ),
            )
            .into_any_element(),
        ];

        div()
            .flex()
            .flex_col()
            .gap(px(4.0))
            .child(Self::settings_card(
                Some("\u{E621}"),
                "Sync folder preferences across directories",
                Some("Use the same layout, sorting, and grouping mode across every folder"),
                Self::settings_toggle_switch(
                    config.view.sync_folder_preferences,
                    "sync_folder_preferences",
                    view.clone(),
                ),
            ))
            .child(Self::settings_card(
                Some("\u{E8BA}"),
                "Layout type",
                Some("Default layout mode for directories"),
                mode_pills,
            ))
            .child(Self::settings_section_heading("Sorting & grouping"))
            .child(Self::settings_expander(
                "layout_sort",
                data.ui_state.is_expanded("layout_sort"),
                "\u{E8CB}",
                "Sort by",
                Some("Default sort column and priority"),
                Some(sort_pills.into_any_element()),
                sort_items,
                view.clone(),
            ))
            .child(Self::settings_expander(
                "layout_group",
                data.ui_state.is_expanded("layout_group"),
                "\u{F168}",
                "Group by",
                Some("Default item grouping column"),
                Some(group_pills.into_any_element()),
                group_items,
                view.clone(),
            ))
            .child(Self::settings_section_heading("Details View"))
            .child(Self::settings_card(
                Some("\u{E784}"),
                "Auto-size columns in Details layout",
                Some("Automatically adjust column widths to fit visible filenames and metadata"),
                Self::settings_toggle_switch(
                    config.view.auto_size_columns,
                    "auto_size_columns",
                    view.clone(),
                ),
            ))
            .child(Self::settings_expander(
                "layout_columns",
                data.ui_state.is_expanded("layout_columns"),
                "\u{E71D}",
                "Columns",
                Some("Toggle which columns are displayed in Details layout"),
                None,
                column_items,
                view.clone(),
            ))
    }

    // ─────────────────────────────────────────────────────────────────────────
    // 4. FILES & FOLDERS PAGE (Files-community FoldersPage.xaml parity)
    // ─────────────────────────────────────────────────────────────────────────

    fn build_folders_settings(
        config: &crate::config::config::AppConfig,
        data: &SettingsPreReadData,
        view: Entity<crate::app::AppWindow>,
    ) -> Div {
        let hidden_items: Vec<AnyElement> = vec![
            Self::settings_expander_row(
                "Show hidden files and folders",
                Some("Display files and folders marked with the Hidden attribute (Ctrl+H)"),
                Self::settings_toggle_switch(
                    config.preferences.show_hidden_files,
                    "show_hidden_files",
                    view.clone(),
                ),
            )
            .into_any_element(),
            Self::settings_expander_row(
                "Show dot files",
                Some("Display files and directories starting with a period (e.g. .git, .env)"),
                Self::settings_toggle_switch(
                    config.preferences.show_dot_files,
                    "show_dot_files",
                    view.clone(),
                ),
            )
            .into_any_element(),
            Self::settings_expander_row(
                "Show protected operating system files",
                Some("Display critical system files marked with FILE_ATTRIBUTE_SYSTEM"),
                Self::settings_toggle_switch(
                    config.preferences.show_system_files,
                    "show_system_files",
                    view.clone(),
                ),
            )
            .into_any_element(),
            Self::settings_expander_row(
                "Show alternate data streams (ADS)",
                Some("Display NTFS Alternate Data Streams attached to files"),
                Self::settings_toggle_switch(
                    config.preferences.show_alternate_data_streams,
                    "show_alternate_data_streams",
                    view.clone(),
                ),
            )
            .into_any_element(),
        ];

        let single_click_items: Vec<AnyElement> = vec![
            Self::settings_expander_row(
                "Files",
                Some("Open files with a single click instead of double-clicking"),
                Self::settings_toggle_switch(
                    config.actions.open_files_with_single_click,
                    "open_files_with_single_click",
                    view.clone(),
                ),
            )
            .into_any_element(),
            Self::settings_expander_row(
                "Folders",
                Some("Navigate into folders with a single click instead of double-clicking"),
                Self::settings_toggle_switch(
                    config.actions.open_folders_with_single_click,
                    "open_folders_with_single_click",
                    view.clone(),
                ),
            )
            .into_any_element(),
            Self::settings_expander_row(
                "Open folders with a single click in Columns view",
                Some("Expand Miller column directories immediately on single click"),
                Self::settings_toggle_switch(
                    config.actions.open_folders_in_columns_with_single_click,
                    "open_folders_in_columns_with_single_click",
                    view.clone(),
                ),
            )
            .into_any_element(),
        ];

        let cached_folders = crate::storage::folder_size::cached_folder_count();
        let is_indexing_drives = crate::storage::folder_size::is_drive_indexing();
        let cache_status_desc = if is_indexing_drives {
            format!(
                "Indexing local drives in background... ({} folders cached so far)",
                cached_folders
            )
        } else {
            format!(
                "{} folders stored in persistent mtime-hashed index (unchanged folders are never recomputed)",
                cached_folders
            )
        };

        let calc_size_items: Vec<AnyElement> = vec![
            Self::settings_expander_row(
                "Index all drives once & update incrementally",
                Some("Pre-index folder sizes across all local drives in the background and only recompute folders when their contents change"),
                Self::settings_toggle_switch(
                    config.view.index_all_drives_folder_sizes,
                    "index_all_drives_folder_sizes",
                    view.clone(),
                ),
            )
            .into_any_element(),
            Self::settings_expander_row(
                "Persistent Folder Size Cache",
                Some(&cache_status_desc),
                div()
                    .flex()
                    .flex_row()
                    .gap(px(6.0))
                    .child(Self::build_compact_btn(
                        if is_indexing_drives {
                            "Indexing Drives..."
                        } else {
                            "Index All Drives Now"
                        },
                        is_indexing_drives,
                        {
                            let v = view.clone();
                            move |_, _, cx| {
                                v.update(cx, |this, cx| {
                                    this.trigger_index_all_drives(true, cx);
                                });
                            }
                        },
                    ))
                    .child(Self::build_compact_btn(
                        "Clear Cache",
                        false,
                        {
                            let v = view.clone();
                            move |_, _, cx| {
                                v.update(cx, |this, cx| {
                                    this.clear_folder_size_cache(cx);
                                });
                            }
                        },
                    )),
            )
            .into_any_element(),
        ];

        div()
            .flex()
            .flex_col()
            .gap(px(4.0))
            .child(Self::settings_section_heading("Display"))
            .child(Self::settings_expander(
                "folders_hidden",
                data.ui_state.is_expanded("folders_hidden"),
                "\u{E7B3}",
                "Hidden items",
                Some("Control visibility of hidden, dot, system, and ADS files"),
                None,
                hidden_items,
                view.clone(),
            ))
            .child(Self::settings_card(
                Some("\u{E8F9}"),
                "Show file extensions",
                Some("Display file type extensions (e.g. .rs, .docx, .png) at the end of filenames"),
                Self::settings_toggle_switch(
                    config.preferences.show_file_extensions,
                    "show_file_extensions",
                    view.clone(),
                ),
            ))
            .child(Self::settings_card(
                Some("\u{E91B}"),
                "Show thumbnails",
                Some("Extract rich image, document, and video previews for file icons"),
                Self::settings_toggle_switch(
                    config.preferences.show_thumbnails,
                    "show_thumbnails",
                    view.clone(),
                ),
            ))
            .child(Self::settings_card(
                Some("\u{E73A}"),
                "Show checkboxes when selecting items",
                Some("Display multi-select checkboxes next to items in Details, List, and Grid views"),
                Self::settings_toggle_switch(
                    config.preferences.show_checkboxes_when_selecting,
                    "show_checkboxes_when_selecting",
                    view.clone(),
                ),
            ))
            .child(Self::settings_card(
                Some("\u{E7C5}"),
                "Show command and button tooltips",
                Some("Show informative hover tooltips with names and keyboard shortcuts on buttons and icons"),
                Self::settings_toggle_switch(
                    config.preferences.show_button_tooltips,
                    "show_button_tooltips",
                    view.clone(),
                ),
            ))
            .child(Self::settings_section_heading("Behaviors"))
            .child(Self::settings_expander(
                "folders_single_click",
                data.ui_state.is_expanded("folders_single_click"),
                "\u{ED25}",
                "Single-click to open",
                Some("Configure single-click vs double-click activation for files and folders"),
                None,
                single_click_items,
                view.clone(),
            ))
            .child(Self::settings_card(
                Some("\u{E710}"),
                "Open folders in a new tab",
                Some("Open double-clicked folders in a new tab instead of navigating in-place"),
                Self::settings_toggle_switch(
                    config.actions.open_folders_in_new_tab,
                    "open_folders_in_new_tab",
                    view.clone(),
                ),
            ))
            .child(Self::settings_card(
                Some("\u{E74D}"),
                "Show confirmation dialog when deleting items",
                Some("Choose when to prompt before moving items to Trash or permanently deleting"),
                div()
                    .flex()
                    .flex_row()
                    .gap(px(4.0))
                    .child(Self::build_compact_btn(
                        "Always",
                        config.preferences.delete_confirmation_policy == "always",
                        {
                            let v = view.clone();
                            move |_, _, cx| {
                                v.update(cx, |this, cx| {
                                    this.set_string_setting(
                                        "delete_confirmation_policy",
                                        "always".to_string(),
                                        cx,
                                    );
                                });
                            }
                        },
                    ))
                    .child(Self::build_compact_btn(
                        "Permanent deletion only",
                        config.preferences.delete_confirmation_policy == "permanent_only",
                        {
                            let v = view.clone();
                            move |_, _, cx| {
                                v.update(cx, |this, cx| {
                                    this.set_string_setting(
                                        "delete_confirmation_policy",
                                        "permanent_only".to_string(),
                                        cx,
                                    );
                                });
                            }
                        },
                    ))
                    .child(Self::build_compact_btn(
                        "Never",
                        config.preferences.delete_confirmation_policy == "never",
                        {
                            let v = view.clone();
                            move |_, _, cx| {
                                v.update(cx, |this, cx| {
                                    this.set_string_setting(
                                        "delete_confirmation_policy",
                                        "never".to_string(),
                                        cx,
                                    );
                                });
                            }
                        },
                    )),
            ))
            .child(Self::settings_card(
                Some("\u{E8AC}"),
                "Show warning when changing file extension",
                Some("Warn before renaming a file in a way that changes its extension"),
                Self::settings_toggle_switch(
                    config.preferences.show_file_extension_warning,
                    "show_file_extension_warning",
                    view.clone(),
                ),
            ))
            .child(Self::settings_card(
                Some("\u{E8B3}"),
                "Select files and folders on hover",
                Some("Automatically select items when hovering the mouse pointer over them"),
                Self::settings_toggle_switch(
                    config.actions.select_files_on_hover,
                    "select_files_on_hover",
                    view.clone(),
                ),
            ))
            .child(Self::settings_card(
                Some("\u{E8B0}"),
                "Double-click blank space to go up",
                Some("Double-clicking empty background space in a folder navigates to its parent folder"),
                Self::settings_toggle_switch(
                    config.actions.double_click_empty_space_go_up,
                    "double_click_empty_space_go_up",
                    view.clone(),
                ),
            ))
            .child(Self::settings_card(
                Some("\u{ECE7}"),
                "Scroll to previous folder when navigating up",
                Some("Automatically scroll to and highlight the child folder you just came from"),
                Self::settings_toggle_switch(
                    config.actions.scroll_to_previous_folder_when_navigating_up,
                    "scroll_to_previous_folder_when_navigating_up",
                    view.clone(),
                ),
            ))
            .child(Self::settings_card(
                Some("\u{E67A}"),
                "Size format",
                Some("Choose binary (KiB/MiB/GiB) or decimal (KB/MB/GB) units"),
                div()
                    .flex()
                    .flex_row()
                    .gap(px(4.0))
                    .child(Self::build_compact_btn(
                        "Binary (KiB / MiB / GiB)",
                        config.preferences.size_unit_format == "binary",
                        {
                            let v = view.clone();
                            move |_, _, cx| {
                                v.update(cx, |this, cx| {
                                    this.set_string_setting(
                                        "size_unit_format",
                                        "binary".to_string(),
                                        cx,
                                    );
                                });
                            }
                        },
                    ))
                    .child(Self::build_compact_btn(
                        "Decimal (KB / MB / GB)",
                        config.preferences.size_unit_format == "decimal",
                        {
                            let v = view.clone();
                            move |_, _, cx| {
                                v.update(cx, |this, cx| {
                                    this.set_string_setting(
                                        "size_unit_format",
                                        "decimal".to_string(),
                                        cx,
                                    );
                                });
                            }
                        },
                    ))
                    .child(Self::build_compact_btn(
                        "Exact Bytes",
                        config.preferences.size_unit_format == "bytes",
                        {
                            let v = view.clone();
                            move |_, _, cx| {
                                v.update(cx, |this, cx| {
                                    this.set_string_setting(
                                        "size_unit_format",
                                        "bytes".to_string(),
                                        cx,
                                    );
                                });
                            }
                        },
                    )),
            ))
            .child(Self::settings_expander(
                "folders_sizes",
                data.ui_state.is_expanded("folders_sizes"),
                "\u{EE40}",
                "Calculate folder sizes",
                Some("Compute recursive directory sizes in the background"),
                Some(
                    Self::settings_toggle_switch(
                        config.view.calculate_folder_sizes,
                        "calculate_folder_sizes",
                        view.clone(),
                    )
                    .into_any_element(),
                ),
                calc_size_items,
                view.clone(),
            ))
    }

    // ─────────────────────────────────────────────────────────────────────────
    // 5. ACTIONS PAGE (Files-community ActionsPage.xaml + Keyboard & Mouse Combinations)
    // ─────────────────────────────────────────────────────────────────────────

    fn build_actions_settings(
        config: &crate::config::config::AppConfig,
        data: &SettingsPreReadData,
        view: Entity<crate::app::AppWindow>,
    ) -> Div {
        // 1. Mouse Combinations & Click Gestures Expander
        let mouse_items: Vec<AnyElement> = vec![
            Self::settings_expander_row(
                "Middle-click on folder, breadcrumb, or sidebar item",
                Some("Action performed when middle-clicking a folder with the mouse wheel button"),
                div()
                    .flex()
                    .flex_row()
                    .gap(px(4.0))
                    .flex_wrap()
                    .children(
                        [
                            ("new_tab", "Open in New Tab"),
                            ("new_pane", "Open in Split Pane"),
                            ("new_window", "Open in New Window"),
                            ("none", "Disabled"),
                        ]
                        .into_iter()
                        .map(|(val, lbl)| {
                            let v = view.clone();
                            let s_val = val.to_string();
                            let active = config.actions.middle_click_folder_action == val;
                            Self::build_compact_btn(lbl, active, move |_, _, cx| {
                                v.update(cx, |this, cx| {
                                    this.set_string_setting(
                                        "middle_click_folder_action",
                                        s_val.clone(),
                                        cx,
                                    );
                                });
                            })
                        }),
                    ),
            )
            .into_any_element(),
            Self::settings_expander_row(
                "Middle-click on tab in Tab Bar",
                Some("Action performed when middle-clicking a tab in the top tab strip"),
                div()
                    .flex()
                    .flex_row()
                    .gap(px(4.0))
                    .flex_wrap()
                    .children(
                        [
                            ("close_tab", "Close Tab"),
                            ("duplicate_tab", "Duplicate Tab"),
                            ("pin_tab", "Pin / Unpin Tab"),
                            ("none", "Disabled"),
                        ]
                        .into_iter()
                        .map(|(val, lbl)| {
                            let v = view.clone();
                            let s_val = val.to_string();
                            let active = config.actions.middle_click_tab_action == val;
                            Self::build_compact_btn(lbl, active, move |_, _, cx| {
                                v.update(cx, |this, cx| {
                                    this.set_string_setting(
                                        "middle_click_tab_action",
                                        s_val.clone(),
                                        cx,
                                    );
                                });
                            })
                        }),
                    ),
            )
            .into_any_element(),
            Self::settings_expander_row(
                "Ctrl + Left-Click on folder",
                Some("Choose whether Ctrl+Click toggles multi-selection or opens the folder in a new tab"),
                div()
                    .flex()
                    .flex_row()
                    .gap(px(4.0))
                    .children(
                        [
                            ("select", "Toggle Selection"),
                            ("new_tab", "Open in New Tab"),
                        ]
                        .into_iter()
                        .map(|(val, lbl)| {
                            let v = view.clone();
                            let s_val = val.to_string();
                            let active = config.actions.ctrl_click_folder_action == val;
                            Self::build_compact_btn(lbl, active, move |_, _, cx| {
                                v.update(cx, |this, cx| {
                                    this.set_string_setting(
                                        "ctrl_click_folder_action",
                                        s_val.clone(),
                                        cx,
                                    );
                                });
                            })
                        }),
                    ),
            )
            .into_any_element(),
            Self::settings_expander_row(
                "Double-click on empty background space",
                Some("Action triggered when double-clicking blank space in any file view"),
                div()
                    .flex()
                    .flex_row()
                    .gap(px(4.0))
                    .flex_wrap()
                    .children(
                        [
                            ("navigate_up", "Navigate Up One Level"),
                            ("select_all", "Select All Items"),
                            ("new_folder", "Create New Folder"),
                            ("none", "Disabled"),
                        ]
                        .into_iter()
                        .map(|(val, lbl)| {
                            let v = view.clone();
                            let s_val = val.to_string();
                            let active = config.actions.double_click_empty_action == val;
                            Self::build_compact_btn(lbl, active, move |_, _, cx| {
                                v.update(cx, |this, cx| {
                                    this.set_string_setting(
                                        "double_click_empty_action",
                                        s_val.clone(),
                                        cx,
                                    );
                                });
                            })
                        }),
                    ),
            )
            .into_any_element(),
            Self::settings_expander_row(
                "Mouse 4 (Back thumb button / XButton1)",
                Some("Action performed when pressing the Mouse 4 side button"),
                div()
                    .flex()
                    .flex_row()
                    .gap(px(4.0))
                    .flex_wrap()
                    .children(
                        [
                            ("navigate_back", "Navigate Back"),
                            ("navigate_up", "Navigate Up"),
                            ("previous_tab", "Previous Tab"),
                            ("none", "Disabled"),
                        ]
                        .into_iter()
                        .map(|(val, lbl)| {
                            let v = view.clone();
                            let s_val = val.to_string();
                            let active = config.actions.mouse4_action == val;
                            Self::build_compact_btn(lbl, active, move |_, _, cx| {
                                v.update(cx, |this, cx| {
                                    this.set_string_setting("mouse4_action", s_val.clone(), cx);
                                });
                            })
                        }),
                    ),
            )
            .into_any_element(),
            Self::settings_expander_row(
                "Mouse 5 (Forward thumb button / XButton2)",
                Some("Action performed when pressing the Mouse 5 side button"),
                div()
                    .flex()
                    .flex_row()
                    .gap(px(4.0))
                    .flex_wrap()
                    .children(
                        [
                            ("navigate_forward", "Navigate Forward"),
                            ("next_tab", "Next Tab"),
                            ("none", "Disabled"),
                        ]
                        .into_iter()
                        .map(|(val, lbl)| {
                            let v = view.clone();
                            let s_val = val.to_string();
                            let active = config.actions.mouse5_action == val;
                            Self::build_compact_btn(lbl, active, move |_, _, cx| {
                                v.update(cx, |this, cx| {
                                    this.set_string_setting("mouse5_action", s_val.clone(), cx);
                                });
                            })
                        }),
                    ),
            )
            .into_any_element(),
            Self::settings_expander_row(
                "Ctrl + Mouse Wheel",
                Some("Action performed when scrolling the mouse wheel while holding Ctrl"),
                div()
                    .flex()
                    .flex_row()
                    .gap(px(4.0))
                    .flex_wrap()
                    .children(
                        [
                            ("zoom", "Zoom View / Icon Size"),
                            ("cycle_view", "Cycle Layout Modes"),
                            ("none", "Disabled"),
                        ]
                        .into_iter()
                        .map(|(val, lbl)| {
                            let v = view.clone();
                            let s_val = val.to_string();
                            let active = config.actions.ctrl_wheel_action == val;
                            Self::build_compact_btn(lbl, active, move |_, _, cx| {
                                v.update(cx, |this, cx| {
                                    this.set_string_setting(
                                        "ctrl_wheel_action",
                                        s_val.clone(),
                                        cx,
                                    );
                                });
                            })
                        }),
                    ),
            )
            .into_any_element(),
            Self::settings_expander_row(
                "Shift + Mouse Wheel",
                Some("Action performed when scrolling the mouse wheel while holding Shift"),
                div()
                    .flex()
                    .flex_row()
                    .gap(px(4.0))
                    .flex_wrap()
                    .children(
                        [
                            ("scroll_horizontal", "Scroll Horizontally"),
                            ("switch_tabs", "Switch Tabs"),
                            ("none", "Disabled"),
                        ]
                        .into_iter()
                        .map(|(val, lbl)| {
                            let v = view.clone();
                            let s_val = val.to_string();
                            let active = config.actions.shift_wheel_action == val;
                            Self::build_compact_btn(lbl, active, move |_, _, cx| {
                                v.update(cx, |this, cx| {
                                    this.set_string_setting(
                                        "shift_wheel_action",
                                        s_val.clone(),
                                        cx,
                                    );
                                });
                            })
                        }),
                    ),
            )
            .into_any_element(),
        ];

        // 2. Commands (Keyboard & Mouse Shortcuts) Expander Header Buttons
        let v_add_cmd = view.clone();
        let v_restore_prompt = view.clone();
        let commands_header_ctrl = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.0))
            .child(Self::build_compact_btn(
                "\u{E710} Add command",
                true,
                move |_, _, cx| {
                    v_add_cmd.update(cx, |this, cx| {
                        this.open_keybinding_recorder(ActionKind::NewFolder, true, cx);
                    });
                },
            ))
            .child(Self::build_compact_btn(
                "\u{E777} Restore defaults",
                false,
                move |_, _, cx| {
                    v_restore_prompt.update(cx, |this, cx| {
                        this.state.settings_ui.confirming_restore_defaults = true;
                        cx.notify();
                    });
                },
            ));

        let mut command_expander_children: Vec<AnyElement> = Vec::new();

        // Optional Restore Defaults Confirmation TeachingTip Banner
        if data.ui_state.confirming_restore_defaults {
            let v_yes = view.clone();
            let v_no = view.clone();
            command_expander_children.push(
                div()
                    .w_full()
                    .px(px(20.0))
                    .py(px(12.0))
                    .bg(rgba(0xFFB90018u32))
                    .border_t(px(1.0))
                    .border_color(rgba(0xFFB90055u32))
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .gap(px(16.0))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(2.0))
                            .child(
                                div()
                                    .text_size(px(13.0))
                                    .font_weight(FontWeight(600.0))
                                    .text_color(rgb(0xFFB900))
                                    .child("Restore default keyboard & mouse combinations?"),
                            )
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                    .child("All customized shortcuts and mouse combinations will be reset to Files-community defaults."),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap(px(8.0))
                            .child(Self::build_compact_btn("Yes, restore defaults", true, move |_, _, cx| {
                                v_yes.update(cx, |this, cx| {
                                    this.reset_all_action_bindings(cx);
                                });
                            }))
                            .child(Self::build_compact_btn("No, cancel", false, move |_, _, cx| {
                                v_no.update(cx, |this, cx| {
                                    this.state.settings_ui.confirming_restore_defaults = false;
                                    cx.notify();
                                });
                            })),
                    )
                    .into_any_element(),
            );
        }

        // Interactive Key & Mouse Binding Editor / Recorder Card (KeyBindingEditorDialog parity)
        if let Some(ref rec) = data.ui_state.recorder {
            command_expander_children.push(
                Self::build_keybinding_recorder_card(rec, &data.all_actions, view.clone())
                    .into_any_element(),
            );
        }

        // Filter Bar (Search Box + Category Filter Pills)
        let is_search_focused = data.ui_state.actions_search_focused;
        let search_q = data.ui_state.actions_search_query.clone();
        let v_focus_search = view.clone();
        let v_clear_search = view.clone();

        let filter_pills = [
            ("all", "All"),
            ("keyboard", "Keyboard"),
            ("mouse", "Mouse"),
            ("customized", "Customized"),
            ("Navigation", "Navigation"),
            ("Files & Editing", "Files & Editing"),
            ("Selection", "Selection"),
            ("Layout & View", "Layout & View"),
            ("Tabs & Windows", "Tabs & Windows"),
            ("Dual Pane", "Dual Pane"),
            ("System & Tools", "System"),
        ];
        let mut filter_row = div().flex().flex_row().gap(px(4.0)).flex_wrap();
        for (f_code, f_label) in filter_pills {
            let v = view.clone();
            let code_str = f_code.to_string();
            let active = data.ui_state.actions_filter == f_code;
            filter_row = filter_row.child(Self::build_compact_btn(
                f_label,
                active,
                move |_, _, cx| {
                    v.update(cx, |this, cx| {
                        this.state.settings_ui.actions_filter = code_str.clone();
                        cx.notify();
                    });
                },
            ));
        }

        command_expander_children.push(
            div()
                .w_full()
                .px(px(16.0))
                .py(px(12.0))
                .bg(rgba(0x00000024u32))
                .border_t(px(1.0))
                .border_color(rgba(UiConstants::CARD_STROKE_RGBA))
                .flex()
                .flex_col()
                .gap(px(10.0))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .justify_between()
                        .gap(px(12.0))
                        .child(
                            div()
                                .text_size(px(13.0))
                                .font_weight(FontWeight(600.0))
                                .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                                .child("Keyboard Shortcuts & Mouse Combinations"),
                        )
                        .child(
                            // Interactive Search Box
                            div()
                                .w(px(260.0))
                                .h(px(32.0))
                                .px(px(10.0))
                                .rounded(px(5.0))
                                .bg(rgba(0xFFFFFF10u32))
                                .border(px(1.0))
                                .border_b(px(if is_search_focused { 2.0 } else { 1.0 }))
                                .border_color(if is_search_focused {
                                    rgb(0x60CDFF)
                                } else {
                                    rgba(UiConstants::STROKE_RGBA)
                                })
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap(px(8.0))
                                .cursor_text()
                                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                    cx.stop_propagation();
                                    v_focus_search.update(cx, |this, cx| {
                                        this.state.settings_ui.actions_search_focused = true;
                                        this.state.settings_ui.search_focused = false;
                                        cx.notify();
                                    });
                                })
                                .child(
                                    div()
                                        .text_size(px(12.0))
                                        .font_family(UiConstants::FONT_ICONS)
                                        .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                        .child("\u{E721}"),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .text_size(px(12.5))
                                        .text_color(if search_q.is_empty() {
                                            rgba(UiConstants::TEXT_MUTED_RGBA)
                                        } else {
                                            rgba(UiConstants::TEXT_PRIMARY_RGBA)
                                        })
                                        .child(if search_q.is_empty() {
                                            if is_search_focused {
                                                "Type to filter actions... |".to_string()
                                            } else {
                                                "Search actions or keys...".to_string()
                                            }
                                        } else if is_search_focused {
                                            format!("{}|", search_q)
                                        } else {
                                            search_q.clone()
                                        }),
                                )
                                .children(if !search_q.is_empty() {
                                    vec![div()
                                        .text_size(px(10.0))
                                        .font_family(UiConstants::FONT_ICONS)
                                        .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                        .cursor_pointer()
                                        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                            cx.stop_propagation();
                                            v_clear_search.update(cx, |this, cx| {
                                                this.state.settings_ui.actions_search_query.clear();
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
                .child(filter_row)
                .into_any_element(),
        );

        // Filtered Action Rows
        let q_lower = data.ui_state.actions_search_query.trim().to_lowercase();
        let active_filter = data.ui_state.actions_filter.as_str();

        let mut matching_rows: Vec<AnyElement> = Vec::new();
        for (action, combo_opt, is_customized) in &data.all_actions {
            let is_mouse = combo_opt.as_ref().map(|c| c.is_mouse()).unwrap_or(false)
                || matches!(action.category(), ActionCategory::MouseGestures);
            let cat_label = action.category().label();

            let pass_filter = match active_filter {
                "all" => true,
                "keyboard" => !is_mouse,
                "mouse" => is_mouse,
                "customized" => *is_customized,
                other => cat_label == other,
            };
            if !pass_filter {
                continue;
            }

            if !q_lower.is_empty() {
                let combo_str = combo_opt
                    .as_ref()
                    .map(|c| c.to_string().to_lowercase())
                    .unwrap_or_default();
                let pretty_str = combo_opt
                    .as_ref()
                    .map(|c| c.display_tokens().join("+").to_lowercase())
                    .unwrap_or_default();
                if !action.label().to_lowercase().contains(&q_lower)
                    && !action.description().to_lowercase().contains(&q_lower)
                    && !cat_label.to_lowercase().contains(&q_lower)
                    && !combo_str.contains(&q_lower)
                    && !pretty_str.contains(&q_lower)
                {
                    continue;
                }
            }

            let act = *action;
            let is_editing_this = data
                .ui_state
                .recorder
                .as_ref()
                .map(|r| r.action == act && !r.is_add_mode)
                .unwrap_or(false);
            let v_edit = view.clone();
            let v_reset = view.clone();
            let v_unbind = view.clone();
            let v_row_click = view.clone();

            matching_rows.push(
                div()
                    .w_full()
                    .min_h(px(52.0))
                    .px(px(16.0))
                    .py(px(8.0))
                    .border_t(px(1.0))
                    .border_color(rgba(UiConstants::CARD_STROKE_RGBA))
                    .bg(if is_editing_this {
                        rgba(0x0078D422u32)
                    } else {
                        rgba(0x00000014u32)
                    })
                    .hover(|s| s.bg(rgba(0xFFFFFF0Cu32)))
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .gap(px(12.0))
                    .cursor_pointer()
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        v_row_click.update(cx, |this, cx| {
                            this.open_keybinding_recorder(act, false, cx);
                        });
                    })
                    .child(
                        // Left: Icon + Title + Description + Category / Customized badges
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(12.0))
                            .flex_1()
                            .min_w(px(0.0))
                            .child(
                                div()
                                    .w(px(28.0))
                                    .h(px(28.0))
                                    .rounded(px(5.0))
                                    .bg(rgba(0xFFFFFF0Cu32))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .flex_shrink_0()
                                    .text_size(px(14.0))
                                    .font_family(UiConstants::FONT_ICONS)
                                    .text_color(if *is_customized {
                                        rgb(0x60CDFF)
                                    } else {
                                        rgba(UiConstants::TEXT_SECONDARY_RGBA)
                                    })
                                    .child(act.icon()),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(2.0))
                                    .flex_1()
                                    .min_w(px(0.0))
                                    .child(
                                        div()
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .gap(px(8.0))
                                            .child(
                                                div()
                                                    .text_size(px(13.0))
                                                    .font_weight(FontWeight(600.0))
                                                    .text_color(rgba(
                                                        UiConstants::TEXT_PRIMARY_RGBA,
                                                    ))
                                                    .child(act.label()),
                                            )
                                            .child(
                                                div()
                                                    .px(px(6.0))
                                                    .py(px(1.0))
                                                    .rounded(px(3.0))
                                                    .bg(rgba(0xFFFFFF0Eu32))
                                                    .text_size(px(10.0))
                                                    .text_color(rgba(
                                                        UiConstants::TEXT_SECONDARY_RGBA,
                                                    ))
                                                    .child(cat_label),
                                            )
                                            .children(if *is_customized {
                                                vec![div()
                                                    .px(px(6.0))
                                                    .py(px(1.0))
                                                    .rounded(px(3.0))
                                                    .bg(rgba(0x0078D433u32))
                                                    .border(px(1.0))
                                                    .border_color(rgba(0x60CDFF55u32))
                                                    .text_size(px(10.0))
                                                    .font_weight(FontWeight(600.0))
                                                    .text_color(rgb(0x60CDFF))
                                                    .child("Customized")
                                                    .into_any_element()]
                                            } else {
                                                vec![]
                                            }),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(11.5))
                                            .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                            .child(act.description()),
                                    ),
                            ),
                    )
                    .child(
                        // Middle: WinUI 3 Keycap pills
                        div()
                            .flex_shrink_0()
                            .child(Self::render_keybinding_pills(combo_opt.as_ref())),
                    )
                    .child(
                        // Right: Edit, Reset, Delete buttons (ActionsPage.xaml RowActions parity)
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(4.0))
                            .flex_shrink_0()
                            .child(
                                div()
                                    .w(px(30.0))
                                    .h(px(28.0))
                                    .rounded(px(4.0))
                                    .bg(rgba(0xFFFFFF0Cu32))
                                    .border(px(1.0))
                                    .border_color(rgba(UiConstants::STROKE_RGBA))
                                    .hover(|s| s.bg(rgba(0xFFFFFF22u32)))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .cursor_pointer()
                                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                        cx.stop_propagation();
                                        v_edit.update(cx, |this, cx| {
                                            this.open_keybinding_recorder(act, false, cx);
                                        });
                                    })
                                    .child(
                                        div()
                                            .text_size(px(12.5))
                                            .font_family(UiConstants::FONT_ICONS)
                                            .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                                            .child("\u{E70F}"), // Edit
                                    ),
                            )
                            .children(if *is_customized {
                                vec![div()
                                    .w(px(30.0))
                                    .h(px(28.0))
                                    .rounded(px(4.0))
                                    .bg(rgba(0xFFFFFF0Cu32))
                                    .border(px(1.0))
                                    .border_color(rgba(UiConstants::STROKE_RGBA))
                                    .hover(|s| s.bg(rgba(0xFFFFFF22u32)))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .cursor_pointer()
                                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                        cx.stop_propagation();
                                        v_reset.update(cx, |this, cx| {
                                            this.reset_single_action_binding(act, cx);
                                        });
                                    })
                                    .child(
                                        div()
                                            .text_size(px(12.5))
                                            .font_family(UiConstants::FONT_ICONS)
                                            .text_color(rgb(0x60CDFF))
                                            .child("\u{E777}"), // Reset to default
                                    )
                                    .into_any_element()]
                            } else {
                                vec![]
                            })
                            .child(
                                div()
                                    .w(px(30.0))
                                    .h(px(28.0))
                                    .rounded(px(4.0))
                                    .bg(rgba(0xFFFFFF0Cu32))
                                    .border(px(1.0))
                                    .border_color(rgba(UiConstants::STROKE_RGBA))
                                    .hover(|s| s.bg(rgba(0xE8112344u32)))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .cursor_pointer()
                                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                        cx.stop_propagation();
                                        v_unbind.update(cx, |this, cx| {
                                            this.unbind_action(act, cx);
                                        });
                                    })
                                    .child(
                                        div()
                                            .text_size(px(12.5))
                                            .font_family(UiConstants::FONT_ICONS)
                                            .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                            .child("\u{E74D}"), // Delete / Unbind
                                    ),
                            ),
                    )
                    .into_any_element(),
            );
        }

        command_expander_children.extend(matching_rows);

        div()
            .flex()
            .flex_col()
            .gap(px(6.0))
            .child(Self::settings_expander(
                "actions_mouse",
                data.ui_state.is_expanded("actions_mouse"),
                "\u{E962}",
                "Mouse Combinations & Click Gestures",
                Some("Configure Middle-Click, Ctrl+Click, Double-Click, Mouse 4/5, and Mouse Wheel actions"),
                None,
                mouse_items,
                view.clone(),
            ))
            .child(Self::settings_expander(
                "actions_commands",
                data.ui_state.is_expanded("actions_commands"),
                "\u{E765}",
                "Commands (Keyboard & Mouse Shortcuts)",
                Some("Click any command or press Edit to record a new keyboard shortcut or mouse combination"),
                Some(commands_header_ctrl.into_any_element()),
                command_expander_children,
                view,
            ))
    }

    /// Interactive Key & Mouse Combination Recorder Card (`KeyBindingEditorDialog.xaml` parity).
    fn build_keybinding_recorder_card(
        rec: &KeyBindingRecorderState,
        all_actions: &[(ActionKind, Option<KeyCombination>, bool)],
        view: Entity<crate::app::AppWindow>,
    ) -> Div {
        let act = rec.action;
        let v_save = view.clone();
        let v_cancel = view.clone();
        let v_reset = view.clone();
        let v_mouse_mid = view.clone();
        let v_mouse_m4 = view.clone();
        let v_mouse_m5 = view.clone();
        let v_mouse_dbl = view.clone();
        let v_mouse_wheel = view.clone();

        // Preset chips for quick mouse / special combination assignment
        let quick_combos: &[(&str, KeyCombination)] = &[
            ("Mouse 4 (Back)", KeyCombination::mouse("Mouse4")),
            ("Mouse 5 (Forward)", KeyCombination::mouse("Mouse5")),
            ("Middle Click", KeyCombination::mouse("MiddleClick")),
            ("Middle Click Tab", KeyCombination::mouse("MiddleClickTab")),
            ("Double-Click Blank", KeyCombination::mouse("DoubleClickBlank")),
            ("Ctrl + Wheel Up", KeyCombination::mouse("WheelUp").with_ctrl()),
            ("Ctrl + Wheel Down", KeyCombination::mouse("WheelDown").with_ctrl()),
            ("Ctrl + Shift + N", KeyCombination::new("n").with_ctrl_shift()),
            ("Alt + Enter", KeyCombination::new("enter").with_alt()),
            ("Ctrl + Enter", KeyCombination::new("enter").with_ctrl()),
        ];

        let mut preset_row = div().flex().flex_row().items_center().gap(px(4.0)).flex_wrap();
        for (lbl, combo) in quick_combos {
            let v = view.clone();
            let c_clone = combo.clone();
            let is_cur = rec.recorded_combo.as_ref().map(|r| r.matches(&c_clone)).unwrap_or(false);
            preset_row = preset_row.child(Self::build_compact_btn(
                lbl.to_string(),
                is_cur,
                move |_, _, cx| {
                    let cc = c_clone.clone();
                    v.update(cx, |this, cx| {
                        this.record_candidate_binding(cc, cx);
                    });
                },
            ));
        }

        // Action selector pills when in Add Command mode
        let mut action_selector = div();
        if rec.is_add_mode {
            let mut pills = div().flex().flex_row().gap(px(4.0)).flex_wrap().max_h(px(110.0)).id("tag-pills-scroll").overflow_y_scroll();
            for (candidate, _, _) in all_actions.iter().take(24) {
                let c_act = *candidate;
                let v = view.clone();
                pills = pills.child(Self::build_compact_btn(
                    c_act.label(),
                    c_act == act,
                    move |_, _, cx| {
                        v.update(cx, |this, cx| {
                            this.open_keybinding_recorder(c_act, true, cx);
                        });
                    },
                ));
            }
            action_selector = div()
                .flex()
                .flex_col()
                .gap(px(4.0))
                .child(
                    div()
                        .text_size(px(11.5))
                        .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                        .child("Choose an action:"),
                )
                .child(pills);
        }

        div()
            .w_full()
            .p(px(16.0))
            .bg(rgba(0x0078D418u32))
            .border_t(px(1.5))
            .border_b(px(1.5))
            .border_color(rgb(0x0078D4))
            .flex()
            .flex_col()
            .gap(px(12.0))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(2.0))
                            .child(
                                div()
                                    .text_size(px(14.0))
                                    .font_weight(FontWeight(600.0))
                                    .text_color(rgb(0x60CDFF))
                                    .child(if rec.is_add_mode {
                                        format!("Add Shortcut / Mouse Combination — {}", act.label())
                                    } else {
                                        format!("Edit Shortcut / Mouse Combination — {}", act.label())
                                    }),
                            )
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                    .child(act.description()),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap(px(6.0))
                            .child(Self::build_compact_btn(
                                "Save",
                                true,
                                move |_, _, cx| {
                                    v_save.update(cx, |this, cx| {
                                        this.commit_keybinding_recorder(cx);
                                    });
                                },
                            ))
                            .child(Self::build_compact_btn(
                                "Reset Default",
                                false,
                                move |_, _, cx| {
                                    v_reset.update(cx, |this, cx| {
                                        this.reset_single_action_binding(act, cx);
                                        this.state.settings_ui.recorder = None;
                                        cx.notify();
                                    });
                                },
                            ))
                            .child(Self::build_compact_btn(
                                "Cancel (Esc)",
                                false,
                                move |_, _, cx| {
                                    v_cancel.update(cx, |this, cx| {
                                        this.state.settings_ui.recorder = None;
                                        cx.notify();
                                    });
                                },
                            )),
                    ),
            )
            .children(if rec.is_add_mode {
                vec![action_selector.into_any_element()]
            } else {
                vec![]
            })
            .child(
                // Live Keyboard & Mouse Capture Box
                div()
                    .id("keybinding-recorder-capture-box")
                    .w_full()
                    .min_h(px(56.0))
                    .px(px(16.0))
                    .py(px(10.0))
                    .rounded(px(6.0))
                    .bg(rgba(0x00000048u32))
                    .border(px(1.5))
                    .border_color(rgb(0x60CDFF))
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    // Capture Middle-Click inside recorder box
                    .on_mouse_down(MouseButton::Middle, move |ev, _, cx| {
                        cx.stop_propagation();
                        let combo = KeyCombination {
                            key: "MiddleClick".to_string(),
                            ctrl: ev.modifiers.control,
                            alt: ev.modifiers.alt,
                            shift: ev.modifiers.shift,
                        };
                        v_mouse_mid.update(cx, |this, cx| {
                            this.record_candidate_binding(combo, cx);
                        });
                    })
                    // Capture Mouse 4 (Back) inside recorder box
                    .on_mouse_down(
                        MouseButton::Navigate(NavigationDirection::Back),
                        move |ev, _, cx| {
                            cx.stop_propagation();
                            let combo = KeyCombination {
                                key: "Mouse4".to_string(),
                                ctrl: ev.modifiers.control,
                                alt: ev.modifiers.alt,
                                shift: ev.modifiers.shift,
                            };
                            v_mouse_m4.update(cx, |this, cx| {
                                this.record_candidate_binding(combo, cx);
                            });
                        },
                    )
                    // Capture Mouse 5 (Forward) inside recorder box
                    .on_mouse_down(
                        MouseButton::Navigate(NavigationDirection::Forward),
                        move |ev, _, cx| {
                            cx.stop_propagation();
                            let combo = KeyCombination {
                                key: "Mouse5".to_string(),
                                ctrl: ev.modifiers.control,
                                alt: ev.modifiers.alt,
                                shift: ev.modifiers.shift,
                            };
                            v_mouse_m5.update(cx, |this, cx| {
                                this.record_candidate_binding(combo, cx);
                            });
                        },
                    )
                    // Capture Double-Click or modifier+LeftClick inside recorder box
                    .on_mouse_down(MouseButton::Left, move |ev, _, cx| {
                        cx.stop_propagation();
                        if ev.click_count >= 2
                            || ev.modifiers.control
                            || ev.modifiers.alt
                            || ev.modifiers.shift
                        {
                            let key = if ev.click_count >= 2 {
                                "DoubleClick"
                            } else {
                                "LeftClick"
                            };
                            let combo = KeyCombination {
                                key: key.to_string(),
                                ctrl: ev.modifiers.control,
                                alt: ev.modifiers.alt,
                                shift: ev.modifiers.shift,
                            };
                            v_mouse_dbl.update(cx, |this, cx| {
                                this.record_candidate_binding(combo, cx);
                            });
                        }
                    })
                    // Capture Mouse Wheel (e.g. Ctrl+WheelUp / Ctrl+WheelDown) inside recorder box
                    .on_scroll_wheel(move |ev, _, cx| {
                        cx.stop_propagation();
                        let dy: f32 = ev.delta.pixel_delta(px(16.0)).y.into();
                        if dy.abs() > 0.1 {
                            let key = if dy > 0.0 { "WheelUp" } else { "WheelDown" };
                            let combo = KeyCombination {
                                key: key.to_string(),
                                ctrl: ev.modifiers.control,
                                alt: ev.modifiers.alt,
                                shift: ev.modifiers.shift,
                            };
                            v_mouse_wheel.update(cx, |this, cx| {
                                this.record_candidate_binding(combo, cx);
                            });
                        }
                    })
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(2.0))
                            .child(
                                div()
                                    .text_size(px(12.5))
                                    .font_weight(FontWeight(600.0))
                                    .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                                    .child(
                                        "Listening for input: press any keyboard shortcut or click/scroll here with your mouse",
                                    ),
                            )
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                    .child(
                                        "Supports Ctrl/Alt/Shift + Key, F1–F12, Middle-Click, Mouse 4/5, Double-Click, or Ctrl + Mouse Wheel",
                                    ),
                            ),
                    )
                    .child(Self::render_keybinding_pills(rec.recorded_combo.as_ref())),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(4.0))
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                            .child("Or click a Mouse / Shortcut preset chip:"),
                    )
                    .child(preset_row),
            )
            .children(if let Some(conflict) = rec.conflict_action {
                vec![div()
                    .w_full()
                    .px(px(12.0))
                    .py(px(8.0))
                    .rounded(px(5.0))
                    .bg(rgba(0xFFB90020u32))
                    .border(px(1.0))
                    .border_color(rgba(0xFFB90066u32))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        div()
                            .text_size(px(13.0))
                            .font_family(UiConstants::FONT_ICONS)
                            .text_color(rgb(0xFFB900))
                            .child("\u{E7BA}"),
                    )
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                            .child(format!(
                                "This combination is currently assigned to \"{}\". Clicking Save will reassign it to \"{}\".",
                                conflict.label(),
                                act.label()
                            )),
                    )
                    .into_any_element()]
            } else {
                vec![]
            })
    }

    // ─────────────────────────────────────────────────────────────────────────
    // 6. FILE TAGS PAGE (Files-community TagsPage.xaml parity)
    // ─────────────────────────────────────────────────────────────────────────

    fn build_tags_settings(
        config: &crate::config::config::AppConfig,
        data: &SettingsPreReadData,
        view: Entity<crate::app::AppWindow>,
    ) -> Div {
        let v_new_btn = view.clone();
        let header_btn = Self::build_compact_btn("\u{E710} New tag", true, move |_, _, cx| {
            v_new_btn.update(cx, |this, cx| {
                this.state.settings_ui.is_creating_tag = true;
                this.state.settings_ui.new_tag_name = "New Tag".to_string();
                cx.notify();
            });
        });

        let palette: &[(&str, u32)] = &[
            ("#E81123", 0xE81123),
            ("#FF8C00", 0xFF8C00),
            ("#FFB900", 0xFFB900),
            ("#107C10", 0x107C10),
            ("#00B7C3", 0x00B7C3),
            ("#0078D4", 0x0078D4),
            ("#881798", 0x881798),
            ("#E3008C", 0xE3008C),
        ];

        let mut tag_rows: Vec<AnyElement> = Vec::new();

        if data.ui_state.is_creating_tag {
            let v_create = view.clone();
            let v_cancel = view.clone();
            let cur_color = data.ui_state.new_tag_color.clone();
            let cur_name = data.ui_state.new_tag_name.clone();

            let mut swatches = div().flex().flex_row().items_center().gap(px(6.0));
            for &(hex_str, rgb_val) in palette {
                let v = view.clone();
                let h = hex_str.to_string();
                let selected = cur_color.eq_ignore_ascii_case(hex_str);
                swatches = swatches.child(
                    div()
                        .w(px(20.0))
                        .h(px(20.0))
                        .rounded(px(10.0))
                        .bg(rgb(rgb_val))
                        .border(px(if selected { 2.5 } else { 1.0 }))
                        .border_color(if selected {
                            rgb(0xFFFFFF)
                        } else {
                            rgba(0x00000066u32)
                        })
                        .cursor_pointer()
                        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                            v.update(cx, |this, cx| {
                                this.state.settings_ui.new_tag_color = h.clone();
                                cx.notify();
                            });
                        }),
                );
            }

            let preset_names = ["Project", "Review", "Urgent", "Design", "Client", "Reference"];
            let mut name_cycle = div().flex().flex_row().gap(px(4.0));
            for pn in preset_names {
                let v = view.clone();
                let pn_s = pn.to_string();
                name_cycle = name_cycle.child(Self::build_compact_btn(
                    pn,
                    cur_name == pn,
                    move |_, _, cx| {
                        v.update(cx, |this, cx| {
                            this.state.settings_ui.new_tag_name = pn_s.clone();
                            cx.notify();
                        });
                    },
                ));
            }

            tag_rows.push(
                div()
                    .w_full()
                    .px(px(20.0))
                    .py(px(12.0))
                    .bg(rgba(0x0078D418u32))
                    .border_t(px(1.0))
                    .border_color(rgba(0x60CDFF55u32))
                    .flex()
                    .flex_col()
                    .gap(px(10.0))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .justify_between()
                            .gap(px(12.0))
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap(px(12.0))
                                    .child(
                                        div()
                                            .text_size(px(13.0))
                                            .font_weight(FontWeight(600.0))
                                            .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                                            .child(format!("Create new tag: \"{}\"", cur_name)),
                                    )
                                    .child(swatches),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .gap(px(6.0))
                                    .child(Self::build_compact_btn("Create", true, move |_, _, cx| {
                                        v_create.update(cx, |this, cx| {
                                            this.commit_create_tag(cx);
                                        });
                                    }))
                                    .child(Self::build_compact_btn("Cancel", false, move |_, _, cx| {
                                        v_cancel.update(cx, |this, cx| {
                                            this.state.settings_ui.is_creating_tag = false;
                                            cx.notify();
                                        });
                                    })),
                            ),
                    )
                    .child(name_cycle)
                    .into_any_element(),
            );
        }

        for tag_def in config.tag_definitions.iter() {
            let rgb_val = parse_hex_rgb(&tag_def.color);
            let count = data.tag_counts.get(&tag_def.name).copied().unwrap_or(0);
            let tag_name_for_filter = tag_def.name.clone();
            let v_filter = view.clone();
            let v_delete = view.clone();
            let tag_name_for_ops = tag_def.name.clone();

            let mut row_swatches = div().flex().flex_row().items_center().gap(px(4.0));
            for &(hex_str, swatch_rgb) in palette {
                let v = view.clone();
                let h = hex_str.to_string();
                let tag_for_swatch = tag_name_for_ops.clone();
                let is_cur = tag_def.color.eq_ignore_ascii_case(hex_str);
                row_swatches = row_swatches.child(
                    div()
                        .w(px(14.0))
                        .h(px(14.0))
                        .rounded(px(7.0))
                        .bg(rgb(swatch_rgb))
                        .border(px(if is_cur { 2.0 } else { 1.0 }))
                        .border_color(if is_cur {
                            rgb(0xFFFFFF)
                        } else {
                            rgba(0x00000044u32)
                        })
                        .cursor_pointer()
                        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                            v.update(cx, |this, cx| {
                                this.update_tag_color(tag_for_swatch.clone(), h.clone(), cx);
                            });
                        }),
                );
            }

            tag_rows.push(
                div()
                    .w_full()
                    .min_h(px(48.0))
                    .px(px(20.0))
                    .py(px(8.0))
                    .border_t(px(1.0))
                    .border_color(rgba(UiConstants::CARD_STROKE_RGBA))
                    .bg(rgba(0x00000014u32))
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .gap(px(12.0))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(10.0))
                            .child(
                                div()
                                    .text_size(px(15.0))
                                    .font_family(UiConstants::FONT_ICONS)
                                    .text_color(rgb(rgb_val))
                                    .child("\u{E8EC}"),
                            )
                            .child(
                                div()
                                    .text_size(px(13.5))
                                    .font_weight(FontWeight(500.0))
                                    .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                                    .child(tag_def.name.clone()),
                            )
                            .child(
                                div()
                                    .px(px(6.0))
                                    .py(px(1.5))
                                    .rounded(px(10.0))
                                    .bg(rgba(0xFFFFFF10u32))
                                    .text_size(px(11.0))
                                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                    .child(format!("{} items", count)),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(12.0))
                            .child(row_swatches)
                            .child(Self::build_compact_btn(
                                "Search Tag",
                                false,
                                move |_, _, cx| {
                                    let t = tag_name_for_filter.clone();
                                    v_filter.update(cx, |this, cx| {
                                        this.activate_omnibar_search_mode(cx);
                                        let q = format!("tag:{}", t);
                                        this.state.nav_bar.set_active_text(q.clone(), false);
                                        this.start_search(q, cx);
                                    });
                                },
                            ))
                            .child(Self::build_compact_btn(
                                "Delete",
                                false,
                                move |_, _, cx| {
                                    let tag_for_delete = tag_name_for_ops.clone();
                                    v_delete.update(cx, |this, cx| {
                                        this.delete_tag_definition(tag_for_delete.clone(), cx);
                                    });
                                },
                            )),
                    )
                    .into_any_element(),
            );
        }

        div()
            .flex()
            .flex_col()
            .gap(px(4.0))
            .child(Self::settings_expander(
                "tags_list",
                data.ui_state.is_expanded("tags_list"),
                "\u{E8EC}",
                "File tags",
                Some("Create, recolor, and manage tags for organizing files and folders"),
                Some(header_btn.into_any_element()),
                tag_rows,
                view,
            ))
    }

    // ─────────────────────────────────────────────────────────────────────────
    // 7. TOOLBAR PAGE (Files-community ToolbarCustomizationPage.xaml parity)
    // ─────────────────────────────────────────────────────────────────────────

    fn build_toolbar_settings(
        config: &crate::config::config::AppConfig,
        data: &SettingsPreReadData,
        view: Entity<crate::app::AppWindow>,
    ) -> Div {
        div()
            .flex()
            .flex_col()
            .gap(px(4.0))
            .child(Self::settings_card(
                Some("\u{E712}"),
                "Show inner command bar ribbon",
                Some("Display the contextual toolbar ribbon directly above the file view"),
                Self::settings_toggle_switch(
                    config.appearance.show_toolbar,
                    "show_toolbar",
                    view.clone(),
                ),
            ))
            .child(Self::settings_card(
                Some("\u{E8D2}"),
                "Show command labels on toolbar",
                Some("Display text labels next to icons in the top ribbon (Files-community DefaultLabelPosition=Right)"),
                Self::settings_toggle_switch(
                    config.appearance.show_toolbar_labels,
                    "show_toolbar_labels",
                    view.clone(),
                ),
            ))
            .child(Self::settings_section_heading("Always Visible Context Commands"))
            .child(Self::settings_card(
                Some("\u{E710}"),
                "New Item button (New Folder / File / Archive / Shortcut)",
                None,
                Self::settings_toggle_switch(
                    data.toolbar_new_folder,
                    "toolbar_new_folder",
                    view.clone(),
                ),
            ))
            .child(Self::settings_card(
                Some("\u{E8C6}"),
                "Cut (Ctrl+X)",
                None,
                Self::settings_toggle_switch(data.toolbar_cut, "toolbar_cut", view.clone()),
            ))
            .child(Self::settings_card(
                Some("\u{E8C8}"),
                "Copy (Ctrl+C)",
                None,
                Self::settings_toggle_switch(data.toolbar_copy, "toolbar_copy", view.clone()),
            ))
            .child(Self::settings_card(
                Some("\u{E77F}"),
                "Paste (Ctrl+V)",
                None,
                Self::settings_toggle_switch(data.toolbar_paste, "toolbar_paste", view.clone()),
            ))
            .child(Self::settings_card(
                Some("\u{E8AC}"),
                "Rename (F2)",
                None,
                Self::settings_toggle_switch(data.toolbar_rename, "toolbar_rename", view.clone()),
            ))
            .child(Self::settings_card(
                Some("\u{E74D}"),
                "Delete (Del)",
                None,
                Self::settings_toggle_switch(data.toolbar_delete, "toolbar_delete", view.clone()),
            ))
            .child(Self::settings_section_heading("Right-Side View & Pane Commands"))
            .child(Self::settings_card(
                Some("\u{E890}"),
                "Details & Preview InfoPane toggle button",
                None,
                Self::settings_toggle_switch(
                    data.toolbar_preview_pane,
                    "toolbar_preview_pane",
                    view,
                ),
            ))
    }

    // ─────────────────────────────────────────────────────────────────────────
    // 8. DEV TOOLS PAGE (Files-community DevToolsPage.xaml parity)
    // ─────────────────────────────────────────────────────────────────────────

    fn build_devtools_settings(
        config: &crate::config::config::AppConfig,
        data: &SettingsPreReadData,
        view: Entity<crate::app::AppWindow>,
    ) -> Div {
        let ide_presets = [
            ("Visual Studio Code", "code"),
            ("Zed", "zed"),
            ("Cursor", "cursor"),
            ("RustRover", "rustrover"),
            ("Visual Studio", "devenv"),
            ("Neovim", "nvim"),
        ];
        let mut preset_pills = div().flex().flex_row().gap(px(4.0)).flex_wrap();
        for (name, path) in ide_presets {
            let v = view.clone();
            let n_s = name.to_string();
            let p_s = path.to_string();
            let is_sel = config.devtools.ide_path == path;
            preset_pills = preset_pills.child(Self::build_compact_btn(
                name,
                is_sel,
                move |_, _, cx| {
                    v.update(cx, |this, cx| {
                        this.state.settings.update(cx, |s, _| {
                            s.config.devtools.ide_name = n_s.clone();
                            s.config.devtools.ide_path = p_s.clone();
                            let _ = s.config.save();
                        });
                        cx.notify();
                    });
                },
            ));
        }

        let v_test_ide = view.clone();
        let ide_items: Vec<AnyElement> = vec![
            Self::settings_expander_row(
                "Code editor / IDE preset",
                Some(&format!(
                    "Name: {} • Command/Path: {}",
                    config.devtools.ide_name, config.devtools.ide_path
                )),
                preset_pills,
            )
            .into_any_element(),
            Self::settings_expander_row(
                "Test IDE integration",
                Some("Launch the configured IDE in the active folder to verify your PATH"),
                Self::build_compact_btn("Test Integration \u{E8A7}", false, move |_, _, cx| {
                    v_test_ide.update(cx, |this, cx| {
                        this.open_in_ide(None, cx);
                    });
                }),
            )
            .into_any_element(),
        ];

        div()
            .flex()
            .flex_col()
            .gap(px(4.0))
            .child(Self::settings_expander(
                "devtools_ide",
                data.ui_state.is_expanded("devtools_ide"),
                "\u{E7AC}",
                "Display Open in IDE button",
                Some("Configure when the Open in IDE button appears and which editor it launches"),
                Some(
                    div()
                        .flex()
                        .flex_row()
                        .gap(px(4.0))
                        .child(Self::build_compact_btn(
                            "Always",
                            config.devtools.open_in_ide_visibility == "always",
                            {
                                let v = view.clone();
                                move |_, _, cx| {
                                    v.update(cx, |this, cx| {
                                        this.set_string_setting(
                                            "open_in_ide_visibility",
                                            "always".to_string(),
                                            cx,
                                        );
                                    });
                                }
                            },
                        ))
                        .child(Self::build_compact_btn(
                            "Git repositories",
                            config.devtools.open_in_ide_visibility == "git_only",
                            {
                                let v = view.clone();
                                move |_, _, cx| {
                                    v.update(cx, |this, cx| {
                                        this.set_string_setting(
                                            "open_in_ide_visibility",
                                            "git_only".to_string(),
                                            cx,
                                        );
                                    });
                                }
                            },
                        ))
                        .child(Self::build_compact_btn(
                            "Never",
                            config.devtools.open_in_ide_visibility == "never",
                            {
                                let v = view.clone();
                                move |_, _, cx| {
                                    v.update(cx, |this, cx| {
                                        this.set_string_setting(
                                            "open_in_ide_visibility",
                                            "never".to_string(),
                                            cx,
                                        );
                                    });
                                }
                            },
                        ))
                        .into_any_element(),
                ),
                ide_items,
                view.clone(),
            ))
            .child(Self::settings_card(
                Some("\u{E756}"),
                "Default terminal application",
                Some("Terminal launched by \"Open in Terminal\" (Ctrl+`)"),
                div()
                    .flex()
                    .flex_row()
                    .gap(px(4.0))
                    .child(Self::build_compact_btn(
                        "Windows Terminal",
                        matches!(
                            config.terminal_app,
                            crate::core::terminal_service::TerminalType::WindowsTerminal
                        ),
                        {
                            let v = view.clone();
                            move |_, _, cx| {
                                v.update(cx, |this, cx| {
                                    this.state.settings.update(cx, |s, _| {
                                        s.config.terminal_app =
                                            crate::core::terminal_service::TerminalType::WindowsTerminal;
                                        let _ = s.config.save();
                                    });
                                    cx.notify();
                                });
                            }
                        },
                    ))
                    .child(Self::build_compact_btn(
                        "PowerShell",
                        matches!(
                            config.terminal_app,
                            crate::core::terminal_service::TerminalType::PowerShell
                        ),
                        {
                            let v = view.clone();
                            move |_, _, cx| {
                                v.update(cx, |this, cx| {
                                    this.state.settings.update(cx, |s, _| {
                                        s.config.terminal_app =
                                            crate::core::terminal_service::TerminalType::PowerShell;
                                        let _ = s.config.save();
                                    });
                                    cx.notify();
                                });
                            }
                        },
                    ))
                    .child(Self::build_compact_btn(
                        "Command Prompt",
                        matches!(
                            config.terminal_app,
                            crate::core::terminal_service::TerminalType::CommandPrompt
                        ),
                        {
                            let v = view.clone();
                            move |_, _, cx| {
                                v.update(cx, |this, cx| {
                                    this.state.settings.update(cx, |s, _| {
                                        s.config.terminal_app =
                                            crate::core::terminal_service::TerminalType::CommandPrompt;
                                        let _ = s.config.save();
                                    });
                                    cx.notify();
                                });
                            }
                        },
                    )),
            ))
            .child(Self::settings_card(
                Some("\u{E9D9}"),
                "GPU & Thumbnail Cache Diagnostics",
                Some(&format!(
                    "LRU Icon Cache: {} / {} entries • GPU Texture Pool: {} • Target: 120–144 FPS (< 6.94 ms frame time)",
                    data.cached_icon_count,
                    data.ram_thumb_cap,
                    data.gpu_thumb_cap
                )),
                Self::settings_toggle_switch(
                    config.devtools.show_fps_telemetry,
                    "show_fps_telemetry",
                    view,
                ),
            ))
    }

    // ─────────────────────────────────────────────────────────────────────────
    // 9. ADVANCED PAGE (Files-community AdvancedPage.xaml parity)
    // ─────────────────────────────────────────────────────────────────────────

    fn format_cache_bytes(bytes: u64) -> String {
        const KB: f64 = 1024.0;
        const MB: f64 = 1024.0 * 1024.0;
        const GB: f64 = 1024.0 * 1024.0 * 1024.0;
        let b = bytes as f64;
        if b >= GB {
            format!("{:.2} GB", b / GB)
        } else if b >= MB {
            format!("{:.0} MB", b / MB)
        } else if b >= KB {
            format!("{:.0} KB", b / KB)
        } else {
            format!("{} B", bytes)
        }
    }

    fn format_count_commas(n: u32) -> String {
        let s = n.to_string();
        let mut out = String::with_capacity(s.len() + 2);
        for (i, ch) in s.chars().rev().enumerate() {
            if i > 0 && i % 3 == 0 {
                out.push(',');
            }
            out.push(ch);
        }
        out.chars().rev().collect()
    }

    fn build_advanced_settings(
        config: &crate::config::config::AppConfig,
        data: &SettingsPreReadData,
        view: Entity<crate::app::AppWindow>,
    ) -> Div {
        let v_export = view.clone();
        let v_import = view.clone();
        let v_edit_json = view.clone();
        let v_clear_cache = view.clone();

        const MIN_LIMIT: f32 = 1024.0;
        const MAX_LIMIT: f32 = 32768.0;
        const STEP_LIMIT: f32 = 512.0;
        let current_limit = config.experimental.thumbnail_cache_size_limit.clamp(1024, 32768);
        let pct = ((current_limit as f32 - MIN_LIMIT) / (MAX_LIMIT - MIN_LIMIT)).clamp(0.0, 1.0);
        let track_w: f32 = 220.0;
        let thumb_d: f32 = 16.0;
        let thumb_left = (pct * (track_w - thumb_d)).clamp(0.0, track_w - thumb_d);
        let fill_w = (thumb_left + thumb_d * 0.5).clamp(4.0, track_w);

        // Estimate ~40 KB per 256px PNG on disk, 256 KB per GPU texture, plus <= 32 MB bitmap pool
        let est_disk_bytes = u64::from(current_limit) * 40 * 1024;
        let gpu_cap = crate::ui::components::bounded_image_cache::gpu_cache_capacity_for_limit(
            current_limit,
        );
        let est_vram_bytes = (gpu_cap as u64) * 256 * 1024;
        let est_ram_index_bytes = u64::from(current_limit) * 256;
        let est_total_mem_bytes = est_ram_index_bytes + 32 * 1024 * 1024;
        let actual_disk_text = if data.disk_thumb_count > 0 || data.disk_thumb_bytes > 0 {
            format!(
                "Actual on disk: {} files ({})",
                Self::format_count_commas(data.disk_thumb_count as u32),
                Self::format_cache_bytes(data.disk_thumb_bytes),
            )
        } else {
            format!("In-memory index: {} entries", data.cached_icon_count)
        };

        let track_bounds_for_canvas = data.ui_state.thumb_slider_track_bounds.clone();
        let track_bounds_for_down = data.ui_state.thumb_slider_track_bounds.clone();
        let v_slider = view.clone();

        let cache_items: Vec<AnyElement> = vec![
            Self::settings_expander_row(
                "Thumbnail cache capacity",
                Some(&format!(
                    "{} thumbnails • Est. disk: ~{} • GPU pool: {} textures (~{}) • RAM cap: ~{}",
                    Self::format_count_commas(current_limit),
                    Self::format_cache_bytes(est_disk_bytes),
                    gpu_cap,
                    Self::format_cache_bytes(est_vram_bytes),
                    Self::format_cache_bytes(est_total_mem_bytes),
                )),
                div()
                    .flex()
                    .flex_col()
                    .items_end()
                    .gap(px(6.0))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(10.0))
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .font_weight(FontWeight(600.0))
                                    .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                                    .child(format!(
                                        "{} (~{})",
                                        Self::format_count_commas(current_limit),
                                        Self::format_cache_bytes(est_disk_bytes),
                                    )),
                            )
                            .child(
                                div()
                                    .relative()
                                    .w(px(track_w))
                                    .h(px(22.0))
                                    .flex()
                                    .items_center()
                                    .cursor_pointer()
                                    .on_mouse_down(MouseButton::Left, move |event, _win, cx| {
                                        let click_x: f32 = event.position.x.into();
                                        let (left, width) = track_bounds_for_down
                                            .lock()
                                            .map(|g| *g)
                                            .unwrap_or((0.0, track_w));
                                        let w = if width > 1.0 { width } else { track_w };
                                        let ratio = ((click_x - left) / w).clamp(0.0, 1.0);
                                        let raw = MIN_LIMIT + ratio * (MAX_LIMIT - MIN_LIMIT);
                                        let snapped = ((raw / STEP_LIMIT).round() * STEP_LIMIT)
                                            .clamp(MIN_LIMIT, MAX_LIMIT) as u32;
                                        v_slider.update(cx, |this, cx| {
                                            this.state.settings_ui.thumb_slider_dragging = true;
                                            this.preview_thumbnail_cache_limit(snapped, cx);
                                        });
                                    })
                                    .child(
                                        gpui::canvas(
                                            move |bounds, _, _| {
                                                if let Ok(mut g) = track_bounds_for_canvas.lock() {
                                                    *g = (
                                                        f32::from(bounds.origin.x),
                                                        f32::from(bounds.size.width),
                                                    );
                                                }
                                            },
                                            |_, _, _, _| {},
                                        )
                                        .absolute()
                                        .inset_0(),
                                    )
                                    .child(
                                        div()
                                            .w(px(track_w))
                                            .h(px(4.0))
                                            .rounded(px(2.0))
                                            .bg(rgba(0xFFFFFF28u32)),
                                    )
                                    .child(
                                        div()
                                            .absolute()
                                            .left(px(0.0))
                                            .w(px(fill_w))
                                            .h(px(4.0))
                                            .rounded(px(2.0))
                                            .bg(rgb(0x4CC2FF)),
                                    )
                                    .child(
                                        div()
                                            .absolute()
                                            .left(px(thumb_left))
                                            .w(px(thumb_d))
                                            .h(px(thumb_d))
                                            .rounded(px(thumb_d * 0.5))
                                            .bg(rgb(0x4CC2FF))
                                            .border_2()
                                            .border_color(rgb(0xFFFFFF)),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap(px(4.0))
                            .children([2048u32, 8192, 16384, 32768].into_iter().map(|limit| {
                                let v = view.clone();
                                let is_sel = config.experimental.thumbnail_cache_size_limit == limit;
                                let label = format!("{}k", limit / 1024);
                                Self::build_compact_btn(label, is_sel, move |_, _, cx| {
                                    v.update(cx, move |this, cx| {
                                        this.apply_thumbnail_cache_limit(limit, cx);
                                    });
                                })
                            })),
                    ),
            )
            .into_any_element(),
            Self::settings_expander_row(
                "Clear cached thumbnails",
                Some(&format!(
                    "{} • RAM icon LRU: {} / {}",
                    actual_disk_text,
                    data.cached_icon_count,
                    current_limit
                )),
                Self::build_compact_btn("Clear Cache", false, move |_, _, cx| {
                    v_clear_cache.update(cx, |this, cx| {
                        this.clear_thumbnail_cache(cx);
                    });
                }),
            )
            .into_any_element(),
        ];

        div()
            .flex()
            .flex_col()
            .gap(px(4.0))
            .child(Self::settings_clickable_card(
                Some("\u{EDE1}"),
                "Export settings",
                Some("Save a JSON backup of your settings, tags, and custom key/mouse bindings"),
                "\u{E8A7}",
                move |_, _, cx| {
                    v_export.update(cx, |this, cx| {
                        this.export_settings_backup(cx);
                    });
                },
            ))
            .child(Self::settings_clickable_card(
                Some("\u{EDE2}"),
                "Import settings",
                Some("Restore settings and custom bindings from your exported backup file"),
                "\u{E8A7}",
                move |_, _, cx| {
                    v_import.update(cx, |this, cx| {
                        this.import_settings_backup(cx);
                    });
                },
            ))
            .child(Self::settings_clickable_card(
                Some("\u{E8DA}"),
                "Edit settings file (settings.json)",
                Some(&format!(
                    "Open {}",
                    crate::config::config::AppConfig::config_path().display()
                )),
                "\u{E8A7}",
                move |_, _, cx| {
                    v_edit_json.update(cx, |this, cx| {
                        this.open_settings_json_file(cx);
                    });
                },
            ))
            .child(Self::settings_card(
                Some("\u{E7E8}"),
                "Open Files on Windows startup",
                Some("Automatically launch Files-Rust when signing into Windows"),
                Self::settings_toggle_switch(
                    config.experimental.open_on_windows_startup,
                    "open_on_windows_startup",
                    view.clone(),
                ),
            ))
            .child(Self::settings_card(
                Some("\u{E8E6}"),
                "Leave app running in background when window is closed",
                Some("Keep Files-Rust pre-warmed in the background for instant <10ms window launch"),
                Self::settings_toggle_switch(
                    config.experimental.leave_app_running_in_background,
                    "leave_app_running_in_background",
                    view.clone(),
                ),
            ))
            .child(Self::settings_card(
                Some("\u{E75B}"),
                "Show system tray icon",
                Some("Display a quick-access icon in the notification area"),
                Self::settings_toggle_switch(
                    config.experimental.show_system_tray_icon,
                    "show_system_tray_icon",
                    view.clone(),
                ),
            ))
            .child(Self::settings_section_heading("Experimental feature flags"))
            .child(Self::settings_card(
                Some("\u{EC50}"),
                "Set Files as default file manager",
                Some("Intercept Explorer folder launches and open them in Files-Rust"),
                Self::settings_toggle_switch(
                    config.experimental.set_as_default_file_manager,
                    "set_as_default_file_manager",
                    view.clone(),
                ),
            ))
            .child(Self::settings_card(
                Some("\u{E8E5}"),
                "Replace Open / Save file dialog",
                Some("Use Files-Rust when applications open the system file picker"),
                Self::settings_toggle_switch(
                    config.experimental.replace_open_file_dialog,
                    "replace_open_file_dialog",
                    view.clone(),
                ),
            ))
            .child(Self::settings_expander(
                "advanced_cache",
                data.ui_state.is_expanded("advanced_cache"),
                "\u{EDA2}",
                "Cache thumbnails",
                Some("Persist extracted shell thumbnails for zero-latency directory reloads"),
                Some(
                    Self::settings_toggle_switch(
                        config.experimental.enable_thumbnail_cache,
                        "enable_thumbnail_cache",
                        view.clone(),
                    )
                    .into_any_element(),
                ),
                cache_items,
                view.clone(),
            ))
            .child(Self::settings_card(
                Some("\u{E8B7}"),
                "Show flatten folder options",
                Some("Enable flattening nested subfolders into the current directory from the context menu"),
                Self::settings_toggle_switch(
                    config.experimental.show_flatten_options,
                    "show_flatten_options",
                    view,
                ),
            ))
    }

    // ─────────────────────────────────────────────────────────────────────────
    // 10. ABOUT PAGE (Files-community AboutPage.xaml parity)
    // ─────────────────────────────────────────────────────────────────────────

    fn build_about_settings(
        _config: &crate::config::config::AppConfig,
        data: &SettingsPreReadData,
        view: Entity<crate::app::AppWindow>,
    ) -> Div {
        let v_copy_ver = view.clone();
        let v_docs = view.clone();
        let v_repo = view.clone();
        let v_logs = view.clone();

        let oss_crates = [
            ("gpui 0.2.2", "Apache-2.0 / MIT"),
            ("tokio 1.53", "MIT"),
            ("windows-rs 0.62", "MIT / Apache-2.0"),
            ("git2 0.21", "MIT / Apache-2.0"),
            ("rusqlite 0.40", "MIT"),
            ("jwalk 0.9", "MIT"),
            ("nucleo 0.5", "MPL-2.0"),
            ("rayon 1.12", "MIT / Apache-2.0"),
            ("window-vibrancy 0.8", "MIT"),
            ("image 0.25", "MIT / Apache-2.0"),
            ("zip 9.0", "MIT"),
            ("lopdf 0.45", "MIT"),
        ];
        let mut crate_grid = div()
            .w_full()
            .p(px(16.0))
            .bg(rgba(0x00000018u32))
            .border_t(px(1.0))
            .border_color(rgba(UiConstants::CARD_STROKE_RGBA))
            .flex()
            .flex_row()
            .flex_wrap()
            .gap(px(8.0));

        for (crate_name, license) in oss_crates {
            crate_grid = crate_grid.child(
                div()
                    .w(px(180.0))
                    .px(px(10.0))
                    .py(px(6.0))
                    .rounded(px(4.0))
                    .bg(rgba(0xFFFFFF08u32))
                    .border(px(1.0))
                    .border_color(rgba(UiConstants::CARD_STROKE_RGBA))
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .text_size(px(12.5))
                            .font_weight(FontWeight(600.0))
                            .text_color(rgb(0x60CDFF))
                            .child(crate_name),
                    )
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                            .child(license),
                    ),
            );
        }

        let feedback_items: Vec<AnyElement> = vec![
            Self::settings_expander_row(
                "Submit feature request",
                Some("Share ideas for new features and workflows"),
                Self::build_compact_btn("Open GitHub Issues \u{E8A7}", false, {
                    let v = view.clone();
                    move |_, _, cx| {
                        v.update(cx, |this, cx| this.open_help(cx));
                    }
                }),
            )
            .into_any_element(),
            Self::settings_expander_row(
                "Submit bug report",
                Some("Report an issue or unexpected behavior"),
                Self::build_compact_btn("Report Bug \u{E8A7}", false, {
                    let v = view.clone();
                    move |_, _, cx| {
                        v.update(cx, |this, cx| this.open_help(cx));
                    }
                }),
            )
            .into_any_element(),
        ];

        div()
            .flex()
            .flex_col()
            .gap(px(4.0))
            .child(Self::settings_card(
                Some("\u{E946}"),
                "Files-Rust (Files-community Unibody Edition)",
                Some("Version 0.1.0 • Rust 2024 Edition • GPUI 0.2.2 Hardware-Accelerated Renderer"),
                Self::build_compact_btn("Copy Version Info", false, move |_, _, cx| {
                    v_copy_ver.update(cx, |this, cx| {
                        cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                            "Files-Rust v0.1.0 (GPUI 0.2.2, Rust 2024, Files-community Unibody Architecture)".to_string(),
                        ));
                        this.state.status_center.push_notification(
                            "Version info copied",
                            "Files-Rust v0.1.0 copied to clipboard".to_string(),
                            crate::ui::components::status_center::NotificationSeverity::Info,
                            None,
                        );
                        cx.notify();
                    });
                }),
            ))
            .child(Self::settings_clickable_card(
                Some("\u{EB51}"),
                "Sponsor Files Community on GitHub",
                Some("Support open-source development of the Files project"),
                "\u{E8A7}",
                {
                    let v = view.clone();
                    move |_, _, cx| {
                        v.update(cx, |this, cx| this.open_help(cx));
                    }
                },
            ))
            .child(Self::settings_section_heading("Help and support"))
            .child(Self::settings_clickable_card(
                Some("\u{E736}"),
                "Documentation",
                Some("Learn about keyboard shortcuts, AQS search syntax, and unibody features"),
                "\u{E8A7}",
                move |_, _, cx| {
                    v_docs.update(cx, |this, cx| this.open_help(cx));
                },
            ))
            .child(Self::settings_expander(
                "about_feedback",
                data.ui_state.is_expanded("about_feedback"),
                "\u{ED15}",
                "Feedback",
                Some("Submit feature requests or bug reports"),
                None,
                feedback_items,
                view.clone(),
            ))
            .child(Self::settings_clickable_card(
                Some("\u{E838}"),
                "Open config & log location",
                Some(&format!(
                    "{}",
                    crate::config::config::AppConfig::config_path()
                        .parent()
                        .unwrap_or_else(|| std::path::Path::new("."))
                        .display()
                )),
                "\u{E8A7}",
                move |_, _, cx| {
                    v_logs.update(cx, |this, cx| {
                        if let Some(parent) =
                            crate::config::config::AppConfig::config_path().parent()
                        {
                            this.navigate(
                                crate::core::path_util::FilesPath::from(parent.to_path_buf()),
                                cx,
                            );
                        }
                    });
                },
            ))
            .child(Self::settings_section_heading("Open source"))
            .child(Self::settings_expander(
                "about_oss",
                data.ui_state.is_expanded("about_oss"),
                "\u{E90F}",
                "Third-party libraries",
                Some("Open-source Rust crates powering Files-Rust"),
                None,
                vec![crate_grid.into_any_element()],
                view.clone(),
            ))
            .child(Self::settings_clickable_card(
                Some("\u{E774}"),
                "Open GitHub repository",
                Some("View source code and architecture specifications"),
                "\u{E8A7}",
                move |_, _, cx| {
                    v_repo.update(cx, |this, cx| this.open_help(cx));
                },
            ))
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Legacy Helpers & Live Chrome Apply
    // ─────────────────────────────────────────────────────────────────────────

    pub(crate) fn settings_toggle(
        label: &str,
        on: bool,
        key: &'static str,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        Self::settings_card(
            None,
            label,
            None,
            Self::settings_toggle_switch(on, key, view),
        )
    }

    pub(crate) fn settings_menu_style_row(
        label: &str,
        desc: &str,
        style: crate::config::config::ContextMenuStyle,
        current: crate::config::config::ContextMenuStyle,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let v_set = view.clone();
        let selected = style == current;
        Self::settings_card(
            None,
            label,
            Some(desc),
            Self::build_compact_btn(
                if selected { "Selected" } else { "Select" },
                selected,
                move |_, _, cx| {
                    v_set.update(cx, |this, cx| this.set_context_menu_style(style, cx));
                },
            ),
        )
    }

    pub(crate) fn apply_chrome_live(
        key: &str,
        window: &mut Window,
        view: &Entity<crate::app::AppWindow>,
        cx: &mut App,
    ) {
        if !matches!(key, "mica_enabled" | "mica_alt" | "transparent_window") {
            return;
        }
        let (mica, alt, transp) = {
            let app = view.read(cx);
            let s = app.state.settings.read(cx);
            let a = &s.config.appearance;
            (a.mica_enabled, a.mica_alt, a.transparent_window)
        };
        #[cfg(windows)]
        {
            use gpui::WindowBackgroundAppearance;
            window.set_background_appearance(if transp {
                WindowBackgroundAppearance::Transparent
            } else {
                WindowBackgroundAppearance::Opaque
            });
            if mica {
                if alt {
                    let _ = window_vibrancy::apply_tabbed(&*window, Some(true));
                } else if let Err(e) = window_vibrancy::apply_mica(&*window, Some(true)) {
                    tracing::warn!("apply_mica failed: {:?}", e);
                }
            } else {
                let _ = window_vibrancy::clear_mica(&*window);
            }
        }
        let _ = (mica, alt, transp);
    }
}

fn parse_hex_rgb(hex: &str) -> u32 {
    let clean = hex.trim_start_matches('#');
    if clean.len() >= 6 {
        u32::from_str_radix(&clean[0..6], 16).unwrap_or(0x0078D4)
    } else {
        0x0078D4
    }
}

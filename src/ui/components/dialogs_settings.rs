#![allow(dead_code)]

use crate::ui::components::dialogs::{SettingsPreReadData, SettingsTab};
use crate::ui::constants::UiConstants;
use gpui::*;

impl crate::ui::components::dialogs::DialogsView {
    pub(crate) fn settings_group_header(label: &str) -> impl IntoElement {
        div()
            .text_size(px(11.0))
            .text_color(rgb(UiConstants::ACCENT_COLOR_RGB))
            .font_weight(FontWeight(600.0))
            .mt(px(12.0))
            .mb(px(4.0))
            .child(label.to_string())
    }

    pub(crate) fn build_compact_btn(
        label: &str,
        active: bool,
        on_click: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    ) -> impl IntoElement {
        let mut btn = div()
            .px(px(10.0))
            .py(px(4.0))
            .rounded(px(4.0))
            .cursor_pointer()
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(12.0))
            .on_mouse_down(MouseButton::Left, on_click);

        if active {
            btn = btn.bg(rgba(UiConstants::ACCENT_RGBA));
        } else {
            btn = btn
                .bg(rgba(0xFFFFFF15u32))
                .border(px(1.0))
                .border_color(rgba(UiConstants::STROKE_RGBA))
                .hover(|s| s.bg(rgba(0xFFFFFF25u32)));
        }
        btn.child(label.to_string())
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

    pub(crate) fn build_settings_content<'a>(
        tab: SettingsTab,
        config: &'a crate::config::config::AppConfig,
        data: &'a SettingsPreReadData,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement + 'a {
        match tab {
            SettingsTab::General => {
                div().flex_col().gap(px(4.0))
                    // Group: Behavior
                    .child(Self::settings_group_header("Behavior"))
                    .child(Self::settings_toggle("Show hidden files", config.preferences.show_hidden_files, "show_hidden_files", view.clone()))
                    .child(Self::settings_toggle("Show dot files", config.preferences.show_dot_files, "show_dot_files", view.clone()))
                    .child(Self::settings_toggle("Show file extensions", config.preferences.show_file_extensions, "show_file_extensions", view.clone()))
                    .child(Self::settings_toggle("Confirm on delete", config.preferences.confirm_delete, "confirm_delete", view.clone()))
                    .child(div().h(px(1.0)).w_full().bg(rgba(UiConstants::DIVIDER_RGBA)).my(px(8.0)))
                    // Group: Tabs
                    .child(Self::settings_group_header("Tabs"))
                    .child(Self::settings_toggle("Cycle tabs in recently-used order (Ctrl+Tab)", config.session.mru_tab_cycle, "mru_tab_cycle", view.clone()))
                    .child(div().h(px(1.0)).w_full().bg(rgba(UiConstants::DIVIDER_RGBA)).my(px(8.0)))
                    // Group: Startup
                    .child(Self::settings_group_header("Startup"))
                    .child(div().flex().items_center().justify_between()
                        .child(div().text_size(px(13.0)).text_color(rgb(UiConstants::TEXT_PRIMARY_RGB)).child("Restore last session"))
                        .child(div().text_size(px(12.0)).text_color(rgb(UiConstants::TEXT_MUTED_RGB)).child("Coming soon"))
                    )
            }
            SettingsTab::Appearance => {
                div().flex_col().gap(px(4.0))
                    // Group: Theme
                    .child(Self::settings_group_header("Theme"))
                    .child(Self::settings_toggle("Mica effect", config.appearance.mica_enabled, "mica_enabled", view.clone()))
                    .child(Self::settings_toggle("Mica alt backdrop (saturated)", config.appearance.mica_alt, "mica_alt", view.clone()))
                    .child(div().h(px(1.0)).w_full().bg(rgba(UiConstants::DIVIDER_RGBA)).my(px(8.0)))
                    // Group: Fonts
                    .child(Self::settings_group_header("Fonts"))
                    .child(Self::settings_text_input("Font family", &config.appearance.font_family, "font_family", view.clone()))
                    .child(Self::settings_number_input("Font size", config.appearance.font_size, "font_size", view.clone()))
            }
            SettingsTab::Layout => {
                div().flex_col().gap(px(4.0))
                    // Group: Default View
                    .child(Self::settings_group_header("Default View"))
                    .child(div().text_size(px(12.0)).text_color(rgb(UiConstants::TEXT_SECONDARY_RGB)).mb(px(4.0)).child("View mode"))
                    .child(div().flex().gap(px(4.0)).flex_wrap()
                        .child(Self::build_compact_btn("Details", config.view.default_view_mode == crate::config::config::ViewMode::Details, {
                            let v = view.clone(); move |_, _, cx| { v.update(cx, |this, _| { this.state.main_view.view_mode = crate::config::config::ViewMode::Details; }); }
                        }))
                        .child(Self::build_compact_btn("Grid", config.view.default_view_mode == crate::config::config::ViewMode::Grid, {
                            let v = view.clone(); move |_, _, cx| { v.update(cx, |this, _| { this.state.main_view.view_mode = crate::config::config::ViewMode::Grid; }); }
                        }))
                        .child(Self::build_compact_btn("List", config.view.default_view_mode == crate::config::config::ViewMode::List, {
                            let v = view.clone(); move |_, _, cx| { v.update(cx, |this, _| { this.state.main_view.view_mode = crate::config::config::ViewMode::List; }); }
                        }))
                        .child(Self::build_compact_btn("Columns", config.view.default_view_mode == crate::config::config::ViewMode::Columns, {
                            let v = view.clone(); move |_, _, cx| { v.update(cx, |this, _| { this.state.main_view.view_mode = crate::config::config::ViewMode::Columns; }); }
                        }))
                    )
                    .child(div().h(px(1.0)).w_full().bg(rgba(UiConstants::DIVIDER_RGBA)).my(px(8.0)))
                    // Group: Sorting
                    .child(Self::settings_group_header("Sorting"))
                    .child(Self::settings_toggle("Folders on top", config.view.folder_on_top, "folder_on_top", view.clone()))
                    .child(div().mt(px(8.0)).text_size(px(12.0)).text_color(rgb(UiConstants::TEXT_SECONDARY_RGB)).child("Sort by"))
                    .child(div().flex().gap(px(4.0)).flex_wrap()
                        .child(Self::build_compact_btn("Name", config.view.sort_by == crate::config::config::SortBy::Name, {
                            let v = view.clone(); move |_, _, cx| { v.update(cx, |this, _| { this.state.main_view.sort.column = crate::config::config::SortBy::Name; }); }
                        }))
                        .child(Self::build_compact_btn("Date modified", config.view.sort_by == crate::config::config::SortBy::DateModified, {
                            let v = view.clone(); move |_, _, cx| { v.update(cx, |this, _| { this.state.main_view.sort.column = crate::config::config::SortBy::DateModified; }); }
                        }))
                        .child(Self::build_compact_btn("Date created", config.view.sort_by == crate::config::config::SortBy::DateCreated, {
                            let v = view.clone(); move |_, _, cx| { v.update(cx, |this, _| { this.state.main_view.sort.column = crate::config::config::SortBy::DateCreated; }); }
                        }))
                        .child(Self::build_compact_btn("Size", config.view.sort_by == crate::config::config::SortBy::Size, {
                            let v = view.clone(); move |_, _, cx| { v.update(cx, |this, _| { this.state.main_view.sort.column = crate::config::config::SortBy::Size; }); }
                        }))
                        .child(Self::build_compact_btn("Type", config.view.sort_by == crate::config::config::SortBy::Type, {
                            let v = view.clone(); move |_, _, cx| { v.update(cx, |this, _| { this.state.main_view.sort.column = crate::config::config::SortBy::Type; }); }
                        }))
                        .child(Self::build_compact_btn("Tag", config.view.sort_by == crate::config::config::SortBy::Tag, {
                            let v = view.clone(); move |_, _, cx| { v.update(cx, |this, _| { this.state.main_view.sort.column = crate::config::config::SortBy::Tag; }); }
                        }))
                    )
                    .child(div().mt(px(8.0)).text_size(px(12.0)).text_color(rgb(UiConstants::TEXT_SECONDARY_RGB)).child("Group by"))
                    .child(div().flex().gap(px(4.0)).flex_wrap()
                        .child(Self::build_compact_btn("None", config.view.group_by == crate::config::config::GroupColumn::None, {
                            let v = view.clone(); move |_, _, cx| { v.update(cx, |this, _| { this.state.main_view.group_by = crate::config::config::GroupColumn::None; }); }
                        }))
                        .child(Self::build_compact_btn("Name", config.view.group_by == crate::config::config::GroupColumn::Name, {
                            let v = view.clone(); move |_, _, cx| { v.update(cx, |this, _| { this.state.main_view.group_by = crate::config::config::GroupColumn::Name; }); }
                        }))
                        .child(Self::build_compact_btn("Date modified", config.view.group_by == crate::config::config::GroupColumn::DateModified, {
                            let v = view.clone(); move |_, _, cx| { v.update(cx, |this, _| { this.state.main_view.group_by = crate::config::config::GroupColumn::DateModified; }); }
                        }))
                        .child(Self::build_compact_btn("Date created", config.view.group_by == crate::config::config::GroupColumn::DateCreated, {
                            let v = view.clone(); move |_, _, cx| { v.update(cx, |this, _| { this.state.main_view.group_by = crate::config::config::GroupColumn::DateCreated; }); }
                        }))
                        .child(Self::build_compact_btn("Size", config.view.group_by == crate::config::config::GroupColumn::Size, {
                            let v = view.clone(); move |_, _, cx| { v.update(cx, |this, _| { this.state.main_view.group_by = crate::config::config::GroupColumn::Size; }); }
                        }))
                        .child(Self::build_compact_btn("File type", config.view.group_by == crate::config::config::GroupColumn::FileType, {
                            let v = view.clone(); move |_, _, cx| { v.update(cx, |this, _| { this.state.main_view.group_by = crate::config::config::GroupColumn::FileType; }); }
                        }))
                        .child(Self::build_compact_btn("Sync status", config.view.group_by == crate::config::config::GroupColumn::SyncStatus, {
                            let v = view.clone(); move |_, _, cx| { v.update(cx, |this, _| { this.state.main_view.group_by = crate::config::config::GroupColumn::SyncStatus; }); }
                        }))
                        .child(Self::build_compact_btn("Tag", config.view.group_by == crate::config::config::GroupColumn::Tag, {
                            let v = view.clone(); move |_, _, cx| { v.update(cx, |this, _| { this.state.main_view.group_by = crate::config::config::GroupColumn::Tag; }); }
                        }))
                    )
                    .child(div().h(px(1.0)).w_full().bg(rgba(UiConstants::DIVIDER_RGBA)).my(px(8.0)))
                    // Group: Columns
                    .child(Self::settings_group_header("Columns"))
                    .child(Self::build_settings_row("Column width", "Auto (adaptive)"))
            }
            SettingsTab::Folders => {
                div().flex_col().gap(px(4.0))
                    // Group: Visibility
                    .child(Self::settings_group_header("Visibility"))
                    .child(Self::settings_toggle("Show hidden files", config.preferences.show_hidden_files, "show_hidden_files", view.clone()))
                    .child(Self::settings_toggle("Show dot files", config.preferences.show_dot_files, "show_dot_files", view.clone()))
                    .child(Self::settings_toggle("Show protected system files", config.preferences.show_system_files, "show_system_files", view.clone()))
                    .child(Self::settings_toggle("Show file extensions", config.preferences.show_file_extensions, "show_file_extensions", view.clone()))
            }
            SettingsTab::Tags => {
                div().flex_col().gap(px(4.0))
                    // Group: File Tags
                    .child(Self::settings_group_header("File Tags"))
                    .child(div().text_size(px(12.0)).text_color(rgb(UiConstants::TEXT_SECONDARY_RGB)).child("Manage your file tags"))
                    .child(div().flex_col().gap(px(4.0)).mt(px(8.0))
                        .child(Self::build_tag_item("Work", 0xFF5555))
                        .child(Self::build_tag_item("Personal", 0x55FF55))
                        .child(Self::build_tag_item("Important", 0x5555FF))
                        .child(Self::build_tag_item("Documents", 0xFFAA00))
                        .child(Self::build_tag_item("Media", 0xAA00FF))
                    )
                    .child(div().mt(px(12.0)).child(Self::build_button("Add new tag", false, |_, _, _| {})))
            }
            SettingsTab::Actions => {
                div().flex_col().gap(px(4.0))
                    // Group: Shortcuts
                    .child(Self::settings_group_header("Shortcuts"))
                    .child(div().text_size(px(12.0)).text_color(rgb(UiConstants::TEXT_SECONDARY_RGB)).child("All action-keyboard shortcut pairs"))
                    .child(
                        div().flex_col().gap(px(2.0)).mt(px(8.0))
                            .children(data.action_pairs.iter().map(|(action, key)| {
                                let action_label = action.label().to_string();
                                let key_str = key.to_string();
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .py(px(4.0))
                                    .px(px(8.0))
                                    .rounded(px(4.0))
                                    .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
                                    .child(div().text_size(px(13.0)).text_color(rgb(UiConstants::TEXT_PRIMARY_RGB)).child(action_label))
                                    .child(
                                        div()
                                            .bg(rgba(0xFFFFFF20u32))
                                            .px(px(8.0))
                                            .py(px(2.0))
                                            .rounded(px(4.0))
                                            .text_size(px(12.0))
                                            .child(key_str)
                                    )
                            }))
                    )
                    .child(
                        div().mt(px(12.0)).flex().gap(px(8.0))
                            .child(Self::build_button("Reset to defaults", false, {
                                let v = view.clone();
                                move |_, _, cx| {
                                    v.update(cx, |this, _| {
                                        this.state.action_registry.reset_all_defaults();
                                    });
                                }
                            }))
                    )
            }
            SettingsTab::Toolbar => {
                div().flex_col().gap(px(4.0))
                    // Group: Toolbar Buttons
                    .child(Self::settings_group_header("Toolbar Buttons"))
                    .child(div().text_size(px(12.0)).text_color(rgb(UiConstants::TEXT_SECONDARY_RGB)).child("Toggle which buttons appear on the toolbar"))
                    .child(Self::settings_toggle("New Folder", data.toolbar_new_folder, "toolbar_new_folder", view.clone()))
                    .child(Self::settings_toggle("New File", data.toolbar_new_file, "toolbar_new_file", view.clone()))
                    .child(Self::settings_toggle("Cut", data.toolbar_cut, "toolbar_cut", view.clone()))
                    .child(Self::settings_toggle("Copy", data.toolbar_copy, "toolbar_copy", view.clone()))
                    .child(Self::settings_toggle("Paste", data.toolbar_paste, "toolbar_paste", view.clone()))
                    .child(Self::settings_toggle("Details View", data.toolbar_details_view, "toolbar_details_view", view.clone()))
                    .child(Self::settings_toggle("Grid View", data.toolbar_grid_view, "toolbar_grid_view", view.clone()))
                    .child(Self::settings_toggle("Columns View", data.toolbar_columns_view, "toolbar_columns_view", view.clone()))
                    .child(Self::settings_toggle("Preview Pane", data.toolbar_preview_pane, "toolbar_preview_pane", view.clone()))
                    .child(Self::settings_toggle("Info Pane", data.toolbar_info_pane, "toolbar_info_pane", view.clone()))
            }
            SettingsTab::Advanced => {
                div().flex_col().gap(px(4.0))
                    .child(Self::settings_group_header("Advanced"))
                    .child(Self::settings_toggle("Enable experimental tabs", config.experimental.enable_tabs, "enable_tabs", view.clone()))
                    .child(Self::settings_toggle("Enable preview pane", config.experimental.enable_preview_pane, "enable_preview_pane", view.clone()))
            }
            SettingsTab::DevTools => {
                div().flex_col().gap(px(4.0))
                    .child(Self::settings_group_header("Developer Tools"))
                    .child(div().bg(rgb(0x101010)).p(px(8.0)).rounded(px(4.0)).child(
                        div().text_size(px(12.0)).text_color(rgb(0xAAAAAA)).child("Log Viewer (Mock)\n[INFO] App started\n[DEBUG] Loading drive C:\\\n[WARN] Thumbnail extraction failed for image.psd")
                    ))
                    .child(div().mt(px(8.0)).child(Self::build_button("Export Logs", false, |_, _, _| {})))
            }
            SettingsTab::About => {
                div().flex_col().gap(px(4.0))
                    // Group: Application
                    .child(Self::settings_group_header("Application"))
                    .child(
                        div().flex_col().gap(px(4.0)).mt(px(8.0))
                            .child(Self::build_settings_row("Application", "Files-Rust"))
                            .child(Self::build_settings_row("Version", "0.1.0 (Alpha)"))
                            .child(Self::build_settings_row("Build", "Debug (GPUI + Rust)"))
                            .child(div().h(px(1.0)).w_full().bg(rgba(UiConstants::DIVIDER_RGBA)).my(px(8.0)))
                            .child(
                                div().text_size(px(13.0)).text_color(rgb(UiConstants::ACCENT_COLOR_RGB))
                                    .cursor_pointer()
                                    .child("GitHub Repository")
                                    .on_mouse_down(MouseButton::Left, {
                                        let v = view.clone();
                                        move |_, _, cx| {
                                            v.update(cx, |this, cx| this.open_help(cx));
                                        }
                                    })
                            )
                            .child(
                                div().mt(px(4.0)).text_size(px(12.0)).text_color(rgb(UiConstants::TEXT_MUTED_RGB))
                                    .child("Files-Rust is a high-performance, idiomatic Rust port of the Files file manager for Windows.")
                            )
                    )
            }
        }
    }

    pub(crate) fn build_tag_item(label: &str, color: u32) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .justify_between()
            .py(px(4.0))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        div()
                            .w(px(12.0))
                            .h(px(12.0))
                            .bg(rgb(color))
                            .rounded(px(6.0)),
                    )
                    .child(label.to_string()),
            )
            .child(
                div()
                    .text_color(rgb(UiConstants::TEXT_MUTED_RGB))
                    .child("\u{E70F}"),
            ) // Edit icon
    }

    pub(crate) fn build_shortcut_item(label: &str, shortcut: &str) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .justify_between()
            .py(px(4.0))
            .child(label.to_string())
            .child(
                div()
                    .bg(rgba(0xFFFFFF20u32))
                    .px(px(6.0))
                    .py(px(2.0))
                    .rounded(px(4.0))
                    .text_size(px(12.0))
                    .child(shortcut.to_string()),
            )
    }
    pub(crate) fn settings_tab(label: &str, active: bool) -> impl IntoElement {
        div()
            .px(px(8.0))
            .py(px(6.0))
            .rounded(px(4.0))
            .bg(if active {
                rgba(0xFFFFFF15u32)
            } else {
                rgba(0x00000000u32)
            })
            .hover(|s| s.bg(rgba(0xFFFFFF10u32)))
            .cursor_pointer()
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

    pub(crate) fn settings_toggle(
        label: &str,
        on: bool,
        key: &'static str,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let v_toggle = view.clone();
        div()
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .text_size(AbsoluteLength::Pixels(px(13.0)))
                    .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                    .child(label.to_string()),
            )
            .child(
                div()
                    .w(px(32.0))
                    .h(px(16.0))
                    .bg(if on {
                        rgba(UiConstants::ACCENT_RGBA)
                    } else {
                        rgba(0xFFFFFF30u32)
                    })
                    .rounded(px(8.0))
                    .flex()
                    .items_center()
                    .px(px(2.0))
                    .cursor_pointer()
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        v_toggle.update(cx, |this, cx| this.toggle_setting(key, cx));
                    })
                    .child(
                        div()
                            .w(px(12.0))
                            .h(px(12.0))
                            .bg(rgb(0xFFFFFF))
                            .rounded(px(6.0))
                            .ml(if on { px(16.0) } else { px(0.0) }),
                    ),
            )
    }

    pub(crate) fn settings_text_input(
        label: &str,
        value: impl Into<String>,
        key: &'static str,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let v_edit = view.clone();
        let label_str = label.to_string();
        let val = value.into();
        div()
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .text_size(AbsoluteLength::Pixels(px(13.0)))
                    .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                    .child(label_str),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .h(px(28.0))
                    .min_w(px(160.0))
                    .px(px(8.0))
                    .bg(rgba(0xFFFFFF15u32))
                    .border(px(1.0))
                    .border_color(rgba(UiConstants::STROKE_RGBA))
                    .rounded(px(4.0))
                    .text_size(px(13.0))
                    .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                    .child(val.clone())
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        // For now, just log; real input would use a more complex GPUI text input
                        tracing::debug!("Settings text input clicked: {} = {}", key, val);
                    }),
            )
    }

    pub(crate) fn settings_number_input(
        label: &str,
        value: f64,
        key: &'static str,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let val_str = format!("{:.1}", value);
        Self::settings_text_input(label, val_str, key, view)
    }
}

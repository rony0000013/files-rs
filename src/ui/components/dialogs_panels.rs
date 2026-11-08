use crate::ui::constants::UiConstants;
use crate::ui::dialog_services::custom_properties_dialog::{
    CustomPropertiesState, PropertiesInputField, PropertiesTab,
};
use gpui::prelude::FluentBuilder;
use gpui::*;

impl crate::ui::components::dialogs::DialogsView {
    /// Redesigned modern Properties panel matching `Files-community` (`MainPropertiesPage.xaml`)
    /// while preserving all options from the standard Windows Properties dialog and supporting
    /// single-item, folder, drive, shortcut, and multi-item selections.
    pub(crate) fn build_custom_properties_dialog(
        state: &CustomPropertiesState,
        view: Entity<crate::app::AppWindow>,
    ) -> Div {
        let v_close = view.clone();
        let v_back = view.clone();
        let active_tab = state.active_tab;
        let tabs = PropertiesTab::contextual_tabs(state);
        let can_go_back = !state.tab_history.is_empty();

        // ── 1. Titlebar Area (`TitlebarArea` in MainPropertiesPage.xaml, Height=36) ──
        let back_btn = div()
            .w(px(36.0))
            .h(px(28.0))
            .rounded(px(4.0))
            .flex()
            .items_center()
            .justify_center()
            .when(can_go_back, |d| {
                d.cursor_pointer()
                    .hover(|s| s.bg(rgba(0xFFFFFF10u32)))
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        cx.stop_propagation();
                        v_back.update(cx, |this, cx| {
                            if let Some(
                                crate::ui::components::dialogs::DialogType::CustomProperties {
                                    ref mut state,
                                },
                            ) = this.state.active_dialog
                            {
                                state.navigate_back();
                                cx.notify();
                            }
                        });
                    })
            })
            .child(
                div()
                    .font_family(UiConstants::FONT_ICONS)
                    .text_size(px(12.0))
                    .text_color(if can_go_back {
                        rgb(UiConstants::TEXT_PRIMARY_RGB)
                    } else {
                        rgb(0x666666)
                    })
                    .child("\u{E72B}"),
            );

        let close_btn = div()
            .w(px(44.0))
            .h(px(36.0))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .hover(|s| s.bg(rgb(0xC42B1C)))
            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                cx.stop_propagation();
                v_close.update(cx, |this, cx| this.close_dialog(cx));
            })
            .child(
                div()
                    .font_family(UiConstants::FONT_ICONS)
                    .text_size(px(10.0))
                    .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                    .child("\u{E8BB}"),
            );

        let titlebar = div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .h(px(36.0))
            .pl(px(4.0))
            .flex_shrink_0()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(4.0))
                    .child(back_btn)
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(rgb(0xE0E0E0))
                            .child("Properties"),
                    ),
            )
            .child(close_btn);

        // ── 2. Left Navigation Sidebar ──
        let sidebar = div()
            .w(px(172.0))
            .flex_shrink_0()
            .flex()
            .flex_col()
            .px(px(6.0))
            .pt(px(2.0))
            .pb(px(8.0))
            .gap(px(2.0))
            .children(tabs.into_iter().map(|tab| {
                let v_tab = view.clone();
                let is_active = tab == active_tab;
                Self::properties_tab_btn(tab.display_name(), tab, is_active, v_tab)
            }));

        // ── 3. Bottom Save / Cancel Footer Bar (Height=52) ──
        let v_classic = view.clone();
        let classic_btn = div()
            .px(px(10.0))
            .h(px(30.0))
            .rounded(px(4.0))
            .bg(rgba(0xFFFFFF08u32))
            .border(px(1.0))
            .border_color(rgba(0xFFFFFF14u32))
            .hover(|s| s.bg(rgba(0xFFFFFF14u32)))
            .cursor_pointer()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.0))
            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                cx.stop_propagation();
                v_classic.update(cx, |this, cx| {
                    this.open_classic_properties_from_dialog(cx);
                });
            })
            .child(
                div()
                    .font_family(UiConstants::FONT_ICONS)
                    .text_size(px(11.5))
                    .text_color(rgb(UiConstants::TEXT_SECONDARY_RGB))
                    .child("\u{E8A7}"),
            )
            .child(
                div()
                    .text_size(px(12.0))
                    .text_color(rgb(UiConstants::TEXT_SECONDARY_RGB))
                    .child("Windows Properties"),
            );

        let v_save = view.clone();
        let save_btn = div()
            .w(px(88.0))
            .h(px(32.0))
            .rounded(px(4.0))
            .bg(rgb(0x4CC2FF))
            .hover(|s| s.bg(rgb(0x60CDFF)))
            .border(px(1.0))
            .border_color(rgba(0xFFFFFF22u32))
            .cursor_pointer()
            .flex()
            .items_center()
            .justify_center()
            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                cx.stop_propagation();
                v_save.update(cx, |this, cx| {
                    this.save_and_close_properties_dialog(cx);
                });
            })
            .child(
                div()
                    .text_size(px(13.0))
                    .font_weight(FontWeight(500.0))
                    .text_color(rgb(0x000000))
                    .child("Save"),
            );

        let v_cancel = view.clone();
        let cancel_btn = div()
            .w(px(88.0))
            .h(px(32.0))
            .rounded(px(4.0))
            .bg(rgb(0x323232))
            .hover(|s| s.bg(rgb(0x3C3C3C)))
            .border(px(1.0))
            .border_color(rgba(0xFFFFFF18u32))
            .cursor_pointer()
            .flex()
            .items_center()
            .justify_center()
            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                cx.stop_propagation();
                v_cancel.update(cx, |this, cx| {
                    this.close_dialog(cx);
                });
            })
            .child(
                div()
                    .text_size(px(13.0))
                    .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                    .child("Cancel"),
            );

        let footer = div()
            .h(px(52.0))
            .flex_shrink_0()
            .px(px(12.0))
            .bg(rgb(0x212121))
            .border_t(px(1.0))
            .border_color(rgba(0xFFFFFF12u32))
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .child(classic_btn)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(10.0))
                    .child(save_btn)
                    .child(cancel_btn),
            );

        let right_pane = div()
            .flex_1()
            .min_w(px(0.0))
            .min_h(px(0.0))
            .flex()
            .flex_col()
            .bg(rgb(0x272727))
            .rounded_tl(px(8.0))
            .border_t(px(1.0))
            .border_l(px(1.0))
            .border_color(rgba(0xFFFFFF12u32))
            .overflow_hidden()
            .child(
                div()
                    .id("properties-scroll-body")
                    .flex_1()
                    .min_h(px(0.0))
                    .overflow_y_scroll()
                    .p(px(12.0))
                    .child(Self::build_properties_tab_content(state, view.clone())),
            )
            .child(footer);

        // Outer window shell (620x530, Mica Alt dark base #1F1F1F)
        div()
            .flex()
            .flex_col()
            .w(px(620.0))
            .h(px(530.0))
            .bg(rgb(0x1F1F1F))
            .rounded(px(8.0))
            .overflow_hidden()
            .on_mouse_down(MouseButton::Left, {
                let v = view.clone();
                move |_, _, cx| {
                    cx.stop_propagation();
                    v.update(cx, |this, cx| {
                        if let Some(crate::ui::components::dialogs::DialogType::CustomProperties {
                            ref mut state,
                        }) = this.state.active_dialog
                        {
                            if state.focused_input.is_some() || state.show_hash_filter_menu {
                                state.focused_input = None;
                                state.show_hash_filter_menu = false;
                                cx.notify();
                            }
                        }
                    });
                }
            })
            .child(titlebar)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_1()
                    .min_h(px(0.0))
                    .overflow_hidden()
                    .child(sidebar)
                    .child(right_pane),
            )
    }

    /// Render a navigation item in the Properties left sidebar (`PropertiesSidebar` in MainPropertiesPage.xaml)
    pub(crate) fn properties_tab_btn(
        label: &str,
        tab: PropertiesTab,
        active: bool,
        view: Entity<crate::app::AppWindow>,
    ) -> Div {
        div()
            .h(px(36.0))
            .px(px(8.0))
            .rounded(px(5.0))
            .bg(if active {
                rgba(0xFFFFFF12u32)
            } else {
                rgba(0x00000000u32)
            })
            .hover(|s| {
                s.bg(if active {
                    rgba(0xFFFFFF16u32)
                } else {
                    rgba(0xFFFFFF0Au32)
                })
            })
            .cursor_pointer()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(10.0))
            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                cx.stop_propagation();
                view.update(cx, |this, cx| {
                    if let Some(crate::ui::components::dialogs::DialogType::CustomProperties {
                        ref mut state,
                    }) = this.state.active_dialog
                    {
                        state.set_tab(tab);
                        cx.notify();
                    }
                });
            })
            // Left accent indicator pill (`3px x 16px` in #4CC2FF)
            .child(
                div()
                    .w(px(3.0))
                    .h(px(16.0))
                    .rounded(px(1.5))
                    .bg(if active {
                        rgba(0x4CC2FFFFu32)
                    } else {
                        rgba(0x00000000u32)
                    }),
            )
            // Tab Icon
            .child(
                div()
                    .w(px(18.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .font_family(UiConstants::FONT_ICONS)
                    .text_size(px(14.0))
                    .text_color(if active {
                        rgb(0x4CC2FF)
                    } else {
                        rgb(UiConstants::TEXT_SECONDARY_RGB)
                    })
                    .child(tab.icon_glyph()),
            )
            // Tab Label
            .child(
                div()
                    .text_size(px(13.0))
                    .font_weight(if active {
                        FontWeight(500.0)
                    } else {
                        FontWeight(400.0)
                    })
                    .text_color(if active {
                        rgb(UiConstants::TEXT_PRIMARY_RGB)
                    } else {
                        rgb(0xD0D0D0)
                    })
                    .child(label.to_string()),
            )
    }

    /// Render content for the currently active properties tab
    pub(crate) fn build_properties_tab_content(
        state: &CustomPropertiesState,
        view: Entity<crate::app::AppWindow>,
    ) -> AnyElement {
        match state.active_tab {
            PropertiesTab::General => {
                Self::build_properties_general_tab(state, view).into_any_element()
            }
            PropertiesTab::Security => {
                Self::build_properties_security_tab(state, view).into_any_element()
            }
            PropertiesTab::Hashes => {
                Self::build_properties_hashes_tab(state, view).into_any_element()
            }
            PropertiesTab::Details => {
                Self::build_properties_details_tab(state, view).into_any_element()
            }
            PropertiesTab::Shortcut => {
                Self::build_properties_shortcut_tab(state, view).into_any_element()
            }
            PropertiesTab::Customization => {
                Self::build_properties_customization_tab(state, view).into_any_element()
            }
            PropertiesTab::Compatibility => {
                Self::build_properties_compatibility_tab(state, view).into_any_element()
            }
            PropertiesTab::Signatures => {
                Self::build_properties_signatures_tab(state, view).into_any_element()
            }
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // 1. General Tab (`GeneralPage.xaml` + Windows Properties General parity)
    // ─────────────────────────────────────────────────────────────────────────
    pub(crate) fn build_properties_general_tab(
        state: &CustomPropertiesState,
        view: Entity<crate::app::AppWindow>,
    ) -> Div {
        let is_multi = state.is_multi();
        let is_dir = state.is_dir;
        let is_drive = state.is_drive;
        let name_focused = state.focused_input == Some(PropertiesInputField::ItemName);

        // ── Card 1: Item Overview Card (`GeneralPage.xaml` lines 45-196) ──
        let icon_box = div()
            .w(px(56.0))
            .h(px(56.0))
            .rounded(px(4.0))
            .bg(rgba(0xFFFFFF06u32))
            .border(px(1.0))
            .border_color(rgba(0xFFFFFF16u32))
            .flex()
            .items_center()
            .justify_center()
            .overflow_hidden()
            .child(if let Some(ref thumb) = state.thumbnail {
                img(thumb.clone())
                    .w(px(46.0))
                    .h(px(46.0))
                    .object_fit(ObjectFit::Contain)
                    .into_any_element()
            } else if let Some(ref thumb_path) = state.thumbnail_path {
                img(thumb_path.clone())
                    .w(px(46.0))
                    .h(px(46.0))
                    .object_fit(ObjectFit::Contain)
                    .into_any_element()
            } else if is_multi {
                div()
                    .font_family(UiConstants::FONT_ICONS)
                    .text_size(px(28.0))
                    .text_color(rgb(0x4CC2FF))
                    .child("\u{E8B7}") // Multi-items icon
                    .into_any_element()
            } else if is_drive {
                div()
                    .font_family(UiConstants::FONT_ICONS)
                    .text_size(px(28.0))
                    .text_color(rgb(0x4CC2FF))
                    .child("\u{EDA2}") // Hard drive icon
                    .into_any_element()
            } else if is_dir {
                div()
                    .font_family(UiConstants::FONT_ICONS)
                    .text_size(px(28.0))
                    .text_color(rgb(0xFFB900))
                    .child("\u{E8B7}") // Folder icon
                    .into_any_element()
            } else {
                div()
                    .font_family(UiConstants::FONT_ICONS)
                    .text_size(px(28.0))
                    .text_color(rgb(0xE0E0E0))
                    .child("\u{E7C3}") // File icon
                    .into_any_element()
            });

        let mut overview_right = div().flex_1().min_w(px(0.0)).flex().flex_col().gap(px(6.0));
        if !is_multi && !is_drive {
            // Editable Item File Name TextBox
            overview_right = overview_right.child(Self::properties_text_input(
                &state.edited_name,
                "Item name",
                name_focused,
                state.name_selected_all && name_focused,
                PropertiesInputField::ItemName,
                view.clone(),
            ));
        } else if is_drive {
            overview_right = overview_right.child(
                div()
                    .text_size(px(14.0))
                    .font_weight(FontWeight(600.0))
                    .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                    .child(state.name.clone()),
            );
        }

        if is_multi || is_dir {
            let count_text = if state.calculating_size && state.files_count == 0 && state.folders_count == 0 {
                "Calculating items...".to_string()
            } else {
                state.counts_summary_string()
            };
            overview_right = overview_right.child(
                div()
                    .text_size(if is_multi { px(13.5) } else { px(12.0) })
                    .font_weight(if is_multi {
                        FontWeight(600.0)
                    } else {
                        FontWeight(400.0)
                    })
                    .text_color(if is_multi {
                        rgb(UiConstants::TEXT_PRIMARY_RGB)
                    } else {
                        rgb(UiConstants::TEXT_SECONDARY_RGB)
                    })
                    .child(count_text),
            );
        }

        let mut overview_card = Self::properties_card()
            .gap(px(10.0))
            // Top row: 56x56 Icon + Name TextBox / Count
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.0))
                    .child(div().w(px(110.0)).flex_shrink_0().child(icon_box))
                    .child(overview_right),
            )
            // Row 2: Type
            .child(Self::properties_kv_row("Type:", &state.file_type, true));

        // Row 3: Opens with (for single file) or File system (for drive)
        if let Some(ref fs_name) = state.drive_file_system {
            overview_card =
                overview_card.child(Self::properties_kv_row("File system:", fs_name, true));
        } else if !is_multi && !is_dir {
            if let Some(ref app_name) = state.opens_with {
                let path_for_openwith = state.path.clone();
                let v_ow = view.clone();
                overview_card = overview_card.child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(8.0))
                        .child(
                            div()
                                .w(px(110.0))
                                .flex_shrink_0()
                                .text_size(px(13.0))
                                .font_weight(FontWeight(600.0))
                                .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                                .child("Opens with:"),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(0.0))
                                .flex()
                                .flex_row()
                                .items_center()
                                .justify_between()
                                .gap(px(8.0))
                                .child(
                                    div()
                                        .text_size(px(13.0))
                                        .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                                        .truncate()
                                        .child(app_name.clone()),
                                )
                                .child(
                                    div()
                                        .px(px(10.0))
                                        .py(px(3.0))
                                        .rounded(px(4.0))
                                        .bg(rgba(0xFFFFFF10u32))
                                        .border(px(1.0))
                                        .border_color(rgba(0xFFFFFF1Cu32))
                                        .hover(|s| s.bg(rgba(0xFFFFFF1Cu32)))
                                        .cursor_pointer()
                                        .text_size(px(12.0))
                                        .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                                        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                            cx.stop_propagation();
                                            let fp = crate::core::path_util::FilesPath::new(
                                                &path_for_openwith,
                                            );
                                            v_ow.update(cx, |this, cx| {
                                                let hwnd = this.state.window_hwnd;
                                                cx.background_executor()
                                                    .spawn(async move {
                                                        let _ = crate::platform::windows::shell_service::ShellService::open_with_dialog(&fp, Some(hwnd));
                                                    })
                                                    .detach();
                                            });
                                        })
                                        .child("Change..."),
                                ),
                        ),
                );
            }
        }

        // ── Card 2: "More details" Expander (`GeneralPage.xaml` lines 199-392) ──
        let size_display = if state.calculating_size {
            format!("{} (Calculating...)", state.formatted_size_with_bytes())
        } else {
            state.formatted_size_with_bytes()
        };
        let disk_size_display = if state.calculating_size {
            format!(
                "{} (Calculating...)",
                state.formatted_size_on_disk_with_bytes()
            )
        } else {
            state.formatted_size_on_disk_with_bytes()
        };

        let mut more_details_body = div().flex().flex_col().gap(px(10.0));
        more_details_body =
            more_details_body.child(Self::properties_kv_row("Location:", &state.location, false));
        more_details_body =
            more_details_body.child(Self::properties_kv_row("Size:", &size_display, false));
        more_details_body = more_details_body.child(Self::properties_kv_row(
            "Size on disk:",
            &disk_size_display,
            false,
        ));
        if is_dir || is_multi {
            more_details_body = more_details_body.child(Self::properties_kv_row(
                "Contains:",
                &state.counts_summary_string(),
                false,
            ));
        }

        if !is_multi && !is_drive {
            // Divider before timestamps (matching screenshot & GeneralPage.xaml line 334)
            more_details_body = more_details_body.child(
                div()
                    .h(px(1.0))
                    .mx(px(-12.0))
                    .my(px(2.0))
                    .bg(rgba(0xFFFFFF12u32)),
            );
            if let Some(ref created) = state.created {
                more_details_body =
                    more_details_body.child(Self::properties_kv_row("Created:", created, false));
            }
            if let Some(ref modified) = state.modified {
                more_details_body =
                    more_details_body.child(Self::properties_kv_row("Modified:", modified, false));
            }
            if let Some(ref accessed) = state.accessed {
                more_details_body =
                    more_details_body.child(Self::properties_kv_row("Accessed:", accessed, false));
            }
        }

        let more_details_expander = Self::properties_expander_card(
            "More details",
            None,
            state.more_details_expanded,
            {
                let v = view.clone();
                move |_, _, cx| {
                    v.update(cx, |this, cx| {
                        if let Some(crate::ui::components::dialogs::DialogType::CustomProperties {
                            ref mut state,
                        }) = this.state.active_dialog
                        {
                            state.more_details_expanded = !state.more_details_expanded;
                            cx.notify();
                        }
                    });
                }
            },
            more_details_body,
        );

        let mut page = div()
            .flex()
            .flex_col()
            .gap(px(6.0))
            .child(overview_card)
            .child(more_details_expander);

        // ── Card 3 (For Drives): Disk Capacity + Cleanup / Format (`GeneralPage.xaml` lines 395-621) ──
        if is_drive && state.drive_total_bytes > 0 {
            let pct = ((state.drive_used_bytes as f64 / state.drive_total_bytes as f64) * 100.0)
                .clamp(0.0, 100.0) as u32;
            let drive_path_cleanup = state.path.to_string_lossy().to_string();
            let drive_path_format = state.path.to_string_lossy().to_string();

            page = page
                .child(
                    Self::properties_card()
                        .gap(px(10.0))
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .justify_between()
                                .child(
                                    div()
                                        .text_size(px(13.0))
                                        .font_weight(FontWeight(600.0))
                                        .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                                        .child(format!("Storage Usage ({}% used)", pct)),
                                )
                                .child(
                                    div()
                                        .text_size(px(12.0))
                                        .text_color(rgb(0x4CC2FF))
                                        .font_weight(FontWeight(600.0))
                                        .child(format!("{}%", pct)),
                                ),
                        )
                        // Progress bar
                        .child(
                            div()
                                .w_full()
                                .h(px(8.0))
                                .rounded(px(4.0))
                                .bg(rgba(0xFFFFFF14u32))
                                .overflow_hidden()
                                .child(
                                    div()
                                        .w(relative((pct as f32) / 100.0))
                                        .h_full()
                                        .bg(if pct >= 90 {
                                            rgb(0xE81123)
                                        } else {
                                            rgb(0x4CC2FF)
                                        }),
                                ),
                        )
                        .child(Self::properties_kv_row(
                            "Used space:",
                            &crate::ui::dialog_services::custom_properties_dialog::format_size_with_bytes(
                                state.drive_used_bytes,
                            ),
                            true,
                        ))
                        .child(Self::properties_kv_row(
                            "Free space:",
                            &crate::ui::dialog_services::custom_properties_dialog::format_size_with_bytes(
                                state.drive_free_bytes,
                            ),
                            true,
                        ))
                        .child(Self::properties_kv_row(
                            "Capacity:",
                            &crate::ui::dialog_services::custom_properties_dialog::format_size_with_bytes(
                                state.drive_total_bytes,
                            ),
                            true,
                        )),
                )
                .child(Self::properties_action_card("Disk Cleanup", "\u{E8A7}", move |_, _, _| {
                    let _ = crate::platform::windows::shell_service::ShellService::cleanup_drive(
                        &drive_path_cleanup,
                    );
                }))
                .child({
                    let v_fmt = view.clone();
                    Self::properties_action_card("Format Drive...", "\u{E8A7}", move |_, _, cx| {
                        let letter = drive_path_format
                            .chars()
                            .next()
                            .unwrap_or('C')
                            .to_string();
                        v_fmt.update(cx, |_this, cx| {
                            cx.background_executor()
                                .spawn(async move {
                                    let _ = crate::platform::windows::shell_service::ShellService::format_drive(&letter).await;
                                })
                                .detach();
                        });
                    })
                });
        }

        // ── Card 4 (Downloaded File): Security Unblock (`GeneralPage.xaml` lines 624-664) ──
        if state.has_zone_identifier {
            let unblock_checked = Some(state.unblock_file);
            let v_unblock = view.clone();
            let sec_body = div().flex().flex_row().items_center().justify_between().gap(px(12.0)).child(
                div()
                    .flex_1()
                    .text_size(px(12.5))
                    .text_color(rgb(UiConstants::TEXT_SECONDARY_RGB))
                    .child("This file came from another computer and might be blocked to help protect this computer."),
            ).child(
                Self::properties_tristate_checkbox("Unblock", unblock_checked, move |_, _, cx| {
                    v_unblock.update(cx, |this, cx| {
                        if let Some(crate::ui::components::dialogs::DialogType::CustomProperties {
                            ref mut state,
                        }) = this.state.active_dialog
                        {
                            state.unblock_file = !state.unblock_file;
                            cx.notify();
                        }
                    });
                }),
            );

            page = page.child(Self::properties_expander_card(
                "Security",
                Some("Downloaded file"),
                state.security_unblock_expanded,
                {
                    let v = view.clone();
                    move |_, _, cx| {
                        v.update(cx, |this, cx| {
                            if let Some(
                                crate::ui::components::dialogs::DialogType::CustomProperties {
                                    ref mut state,
                                },
                            ) = this.state.active_dialog
                            {
                                state.security_unblock_expanded = !state.security_unblock_expanded;
                                cx.notify();
                            }
                        });
                    }
                },
                sec_body,
            ));
        }

        // ── Card 5: "Attributes" Expander (`GeneralPage.xaml` lines 667-765 + Windows Advanced Attributes) ──
        if !is_drive {
            let attr_summary = state.attribute_summary();
            let attrs_body = div()
                .flex()
                .flex_col()
                .gap(px(8.0))
                // Read-only
                .child(Self::properties_attribute_row(
                    "Read-only",
                    state.attr_readonly,
                    {
                        let v = view.clone();
                        move |_, _, cx| {
                            v.update(cx, |this, cx| {
                                if let Some(
                                    crate::ui::components::dialogs::DialogType::CustomProperties {
                                        ref mut state,
                                    },
                                ) = this.state.active_dialog
                                {
                                    CustomPropertiesState::toggle_tristate_attr(
                                        &mut state.attr_readonly,
                                    );
                                    state.is_readonly = state.attr_readonly == Some(true);
                                    cx.notify();
                                }
                            });
                        }
                    },
                ))
                .child(div().h(px(1.0)).mx(px(-12.0)).bg(rgba(0xFFFFFF10u32)))
                // Hidden
                .child(Self::properties_attribute_row(
                    "Hidden",
                    state.attr_hidden,
                    {
                        let v = view.clone();
                        move |_, _, cx| {
                            v.update(cx, |this, cx| {
                                if let Some(
                                    crate::ui::components::dialogs::DialogType::CustomProperties {
                                        ref mut state,
                                    },
                                ) = this.state.active_dialog
                                {
                                    CustomPropertiesState::toggle_tristate_attr(
                                        &mut state.attr_hidden,
                                    );
                                    state.is_hidden = state.attr_hidden == Some(true);
                                    cx.notify();
                                }
                            });
                        }
                    },
                ))
                .child(div().h(px(1.0)).mx(px(-12.0)).bg(rgba(0xFFFFFF10u32)))
                // Archive (Windows Advanced Attributes)
                .child(Self::properties_attribute_row(
                    "File is ready for archiving (Archive)",
                    state.attr_archive,
                    {
                        let v = view.clone();
                        move |_, _, cx| {
                            v.update(cx, |this, cx| {
                                if let Some(
                                    crate::ui::components::dialogs::DialogType::CustomProperties {
                                        ref mut state,
                                    },
                                ) = this.state.active_dialog
                                {
                                    CustomPropertiesState::toggle_tristate_attr(
                                        &mut state.attr_archive,
                                    );
                                    state.is_archive = state.attr_archive == Some(true);
                                    cx.notify();
                                }
                            });
                        }
                    },
                ))
                .child(div().h(px(1.0)).mx(px(-12.0)).bg(rgba(0xFFFFFF10u32)))
                // Index contents (Windows Advanced Attributes)
                .child(Self::properties_attribute_row(
                    "Allow contents to be indexed for fast search",
                    state.attr_indexed,
                    {
                        let v = view.clone();
                        move |_, _, cx| {
                            v.update(cx, |this, cx| {
                                if let Some(
                                    crate::ui::components::dialogs::DialogType::CustomProperties {
                                        ref mut state,
                                    },
                                ) = this.state.active_dialog
                                {
                                    CustomPropertiesState::toggle_tristate_attr(
                                        &mut state.attr_indexed,
                                    );
                                    cx.notify();
                                }
                            });
                        }
                    },
                ))
                .child(div().h(px(1.0)).mx(px(-12.0)).bg(rgba(0xFFFFFF10u32)))
                // Compress contents
                .child(Self::properties_attribute_row(
                    "Compress contents to save disk space",
                    state.attr_compressed,
                    {
                        let v = view.clone();
                        move |_, _, cx| {
                            v.update(cx, |this, cx| {
                                if let Some(
                                    crate::ui::components::dialogs::DialogType::CustomProperties {
                                        ref mut state,
                                    },
                                ) = this.state.active_dialog
                                {
                                    CustomPropertiesState::toggle_tristate_attr(
                                        &mut state.attr_compressed,
                                    );
                                    state.is_compressed = state.attr_compressed == Some(true);
                                    cx.notify();
                                }
                            });
                        }
                    },
                ));

            page = page.child(Self::properties_expander_card(
                "Attributes",
                Some(&attr_summary),
                state.attributes_expanded,
                {
                    let v = view.clone();
                    move |_, _, cx| {
                        v.update(cx, |this, cx| {
                            if let Some(
                                crate::ui::components::dialogs::DialogType::CustomProperties {
                                    ref mut state,
                                },
                            ) = this.state.active_dialog
                            {
                                state.attributes_expanded = !state.attributes_expanded;
                                cx.notify();
                            }
                        });
                    }
                },
                attrs_body,
            ));
        }

        page
    }

    // ─────────────────────────────────────────────────────────────────────────
    // 2. Security Tab (`SecurityPage.xaml` + Windows Security ACL tab parity)
    // ─────────────────────────────────────────────────────────────────────────
    pub(crate) fn build_properties_security_tab(
        state: &CustomPropertiesState,
        view: Entity<crate::app::AppWindow>,
    ) -> Div {
        let sec = &state.security;
        let sel_idx = sec
            .selected_principal
            .min(sec.principals.len().saturating_sub(1));
        let selected_entry = sec.principals.get(sel_idx);
        let target_path = state.path.clone();

        // Card 1: Group or user names list (`PrincipalListGrid` in SecurityPage.xaml)
        let principals_card = div()
            .w_full()
            .bg(rgba(0xFFFFFF08u32))
            .border(px(1.0))
            .border_color(rgba(0xFFFFFF12u32))
            .rounded(px(5.0))
            .overflow_hidden()
            .flex()
            .flex_col()
            // Header
            .child(
                div()
                    .px(px(12.0))
                    .py(px(8.0))
                    .border_b(px(1.0))
                    .border_color(rgba(0xFFFFFF12u32))
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(px(13.0))
                            .font_weight(FontWeight(600.0))
                            .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                            .child("Group or user names"),
                    )
                    .child(if let Some(ref owner) = sec.owner {
                        div()
                            .text_size(px(11.5))
                            .text_color(rgb(UiConstants::TEXT_SECONDARY_RGB))
                            .child(format!("Owner: {}", owner))
                            .into_any_element()
                    } else {
                        div().into_any_element()
                    }),
            )
            // Principal rows
            .child(
                div()
                    .p(px(4.0))
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .children(sec.principals.iter().enumerate().map(|(idx, p)| {
                        let is_sel = idx == sel_idx;
                        let v_sel = view.clone();
                        div()
                            .px(px(10.0))
                            .py(px(6.0))
                            .rounded(px(4.0))
                            .bg(if is_sel {
                                rgba(0xFFFFFF14u32)
                            } else {
                                rgba(0x00000000u32)
                            })
                            .hover(|s| s.bg(rgba(0xFFFFFF10u32)))
                            .cursor_pointer()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(10.0))
                            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                cx.stop_propagation();
                                v_sel.update(cx, |this, cx| {
                                    if let Some(
                                        crate::ui::components::dialogs::DialogType::CustomProperties {
                                            ref mut state,
                                        },
                                    ) = this.state.active_dialog
                                    {
                                        state.security.selected_principal = idx;
                                        cx.notify();
                                    }
                                });
                            })
                            .child(
                                div()
                                    .font_family(UiConstants::FONT_ICONS)
                                    .text_size(px(14.0))
                                    .text_color(if is_sel {
                                        rgb(0x4CC2FF)
                                    } else {
                                        rgb(UiConstants::TEXT_SECONDARY_RGB)
                                    })
                                    .child(p.glyph.clone()),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap(px(6.0))
                                    .child(
                                        div()
                                            .text_size(px(13.0))
                                            .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                                            .child(p.display_name.clone()),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(12.0))
                                            .text_color(rgb(UiConstants::TEXT_SECONDARY_RGB))
                                            .child(format!("({})", p.account_name)),
                                    ),
                            )
                    })),
            );

        // Card 2: Permissions for selected principal (`PermissionsForSelectedAccessControlEntryGrid`)
        let perm_header_title = selected_entry
            .map(|e| format!("Permissions for {}", e.display_name))
            .unwrap_or_else(|| "Permissions".to_string());

        let perm_rows: Vec<(&str, bool, bool)> = if let Some(e) = selected_entry {
            vec![
                ("Full control", e.allow_full_control, e.deny_full_control),
                ("Modify", e.allow_modify, e.deny_modify),
                (
                    "Read & execute",
                    e.allow_read_execute,
                    e.deny_read_execute,
                ),
                ("Read", e.allow_read, e.deny_read),
                ("Write", e.allow_write, e.deny_write),
                ("Special permissions", e.allow_special, false),
            ]
        } else {
            Vec::new()
        };

        let v_adv = view.clone();
        let permissions_card = div()
            .w_full()
            .bg(rgba(0xFFFFFF08u32))
            .border(px(1.0))
            .border_color(rgba(0xFFFFFF12u32))
            .rounded(px(5.0))
            .overflow_hidden()
            .flex()
            .flex_col()
            // Header + "Advanced permissions >" button
            .child(
                div()
                    .px(px(12.0))
                    .py(px(8.0))
                    .border_b(px(1.0))
                    .border_color(rgba(0xFFFFFF12u32))
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(px(13.0))
                            .font_weight(FontWeight(600.0))
                            .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                            .child(perm_header_title),
                    )
                    .child(
                        div()
                            .px(px(10.0))
                            .py(px(4.0))
                            .rounded(px(4.0))
                            .bg(rgba(0xFFFFFF10u32))
                            .border(px(1.0))
                            .border_color(rgba(0xFFFFFF1Au32))
                            .hover(|s| s.bg(rgba(0xFFFFFF1Au32)))
                            .cursor_pointer()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(6.0))
                            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                cx.stop_propagation();
                                let fp = crate::core::path_util::FilesPath::new(&target_path);
                                v_adv.update(cx, |this, cx| {
                                    let hwnd = this.state.window_hwnd;
                                    cx.background_executor()
                                        .spawn(async move {
                                            let _ = crate::platform::windows::shell_service::ShellService::open_properties_page_with_hwnd(
                                                &fp,
                                                Some("Security"),
                                                Some(hwnd),
                                            );
                                        })
                                        .detach();
                                });
                            })
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                                    .child("Advanced permissions"),
                            )
                            .child(
                                div()
                                    .font_family(UiConstants::FONT_ICONS)
                                    .text_size(px(10.0))
                                    .text_color(rgb(UiConstants::TEXT_SECONDARY_RGB))
                                    .child("\u{E76C}"),
                            ),
                    ),
            )
            // Allow / Deny Column Headers
            .child(
                div()
                    .px(px(12.0))
                    .pt(px(8.0))
                    .pb(px(4.0))
                    .flex()
                    .flex_row()
                    .items_center()
                    .child(div().flex_1())
                    .child(
                        div()
                            .w(px(56.0))
                            .text_center()
                            .text_size(px(12.0))
                            .text_color(rgb(UiConstants::TEXT_SECONDARY_RGB))
                            .child("Allow"),
                    )
                    .child(
                        div()
                            .w(px(56.0))
                            .text_center()
                            .text_size(px(12.0))
                            .text_color(rgb(UiConstants::TEXT_SECONDARY_RGB))
                            .child("Deny"),
                    ),
            )
            // Permission rows
            .child(
                div()
                    .px(px(12.0))
                    .pb(px(10.0))
                    .flex()
                    .flex_col()
                    .gap(px(6.0))
                    .children(perm_rows.into_iter().map(|(label, allow, deny)| {
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .py(px(2.0))
                            .child(
                                div()
                                    .flex_1()
                                    .text_size(px(13.0))
                                    .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                                    .child(label.to_string()),
                            )
                            .child(
                                div()
                                    .w(px(56.0))
                                    .flex()
                                    .justify_center()
                                    .child(Self::properties_readonly_check_indicator(allow, false)),
                            )
                            .child(
                                div()
                                    .w(px(56.0))
                                    .flex()
                                    .justify_center()
                                    .child(Self::properties_readonly_check_indicator(deny, true)),
                            )
                    })),
            );

        div()
            .flex()
            .flex_col()
            .gap(px(8.0))
            .child(principals_card)
            .child(permissions_card)
    }

    // ─────────────────────────────────────────────────────────────────────────
    // 3. Hashes Tab (`HashesPage.xaml` parity)
    // ─────────────────────────────────────────────────────────────────────────
    pub(crate) fn build_properties_hashes_tab(
        state: &CustomPropertiesState,
        view: Entity<crate::app::AppWindow>,
    ) -> Div {
        let hashes = &state.hashes;
        let cmp_focused = state.focused_input == Some(PropertiesInputField::HashCompare);
        let matched_alg = hashes.match_hash(&state.hash_compare_input);
        let has_cmp_query = !state.hash_compare_input.trim().is_empty();

        // 1. Top Hash Comparison Card (`HashComparisonGrid` in HashesPage.xaml)
        let compare_card = Self::properties_card().child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.0))
                .child(
                    div().flex_1().child(Self::properties_text_input(
                        &state.hash_compare_input,
                        "Enter a hash to compare...",
                        cmp_focused,
                        false,
                        PropertiesInputField::HashCompare,
                        view.clone(),
                    )),
                )
                .child({
                    let v_paste = view.clone();
                    div()
                        .px(px(12.0))
                        .h(px(32.0))
                        .rounded(px(4.0))
                        .bg(rgba(0xFFFFFF10u32))
                        .border(px(1.0))
                        .border_color(rgba(0xFFFFFF1Cu32))
                        .hover(|s| s.bg(rgba(0xFFFFFF1Cu32)))
                        .cursor_pointer()
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_size(px(12.5))
                        .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                            cx.stop_propagation();
                            if let Some(clip) = cx.read_from_clipboard().and_then(|c| c.text()) {
                                v_paste.update(cx, |this, cx| {
                                    if let Some(
                                        crate::ui::components::dialogs::DialogType::CustomProperties {
                                            ref mut state,
                                        },
                                    ) = this.state.active_dialog
                                    {
                                        state.hash_compare_input = clip.trim().to_string();
                                        cx.notify();
                                    }
                                });
                            }
                        })
                        .child("Paste & Compare")
                }),
        );

        // 2. Match / Mismatch InfoBar (`HashMatchInfoBar` in HashesPage.xaml)
        let match_infobar = if has_cmp_query && !state.loading_hashes {
            let (bg_col, border_col, icon, msg) = if let Some(alg) = matched_alg {
                (
                    rgba(0x107C1030u32),
                    rgba(0x6CCB5F60u32),
                    "\u{E930}",
                    format!("Hashes match! Verified against {}.", alg),
                )
            } else {
                (
                    rgba(0xC42B1C30u32),
                    rgba(0xFF99A460u32),
                    "\u{EA39}",
                    "Hash does not match any computed algorithm.".to_string(),
                )
            };
            Some(
                div()
                    .w_full()
                    .px(px(12.0))
                    .py(px(8.0))
                    .rounded(px(5.0))
                    .bg(bg_col)
                    .border(px(1.0))
                    .border_color(border_col)
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        div()
                            .font_family(UiConstants::FONT_ICONS)
                            .text_size(px(14.0))
                            .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                            .child(icon),
                    )
                    .child(
                        div()
                            .text_size(px(12.5))
                            .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                            .child(msg),
                    ),
            )
        } else {
            None
        };

        // 3. Hashes List Card (`HashesListGrid` in HashesPage.xaml)
        let alg_rows: [(usize, &str, Option<&str>); 6] = [
            (0, "CRC32", hashes.crc32.as_deref()),
            (1, "MD5", hashes.md5.as_deref()),
            (2, "SHA-1", hashes.sha1.as_deref()),
            (3, "SHA-256", hashes.sha256.as_deref()),
            (4, "SHA-384", hashes.sha384.as_deref()),
            (5, "SHA-512", hashes.sha512.as_deref()),
        ];

        let v_filter = view.clone();
        let mut hashes_card = div()
            .w_full()
            .bg(rgba(0xFFFFFF08u32))
            .border(px(1.0))
            .border_color(rgba(0xFFFFFF12u32))
            .rounded(px(5.0))
            .overflow_hidden()
            .flex()
            .flex_col()
            // Table Header: Algorithm | Hash value | Eye filter button
            .child(
                div()
                    .h(px(40.0))
                    .px(px(12.0))
                    .border_b(px(1.0))
                    .border_color(rgba(0xFFFFFF12u32))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(12.0))
                    .child(
                        div()
                            .w(px(68.0))
                            .flex_shrink_0()
                            .text_size(px(12.5))
                            .font_weight(FontWeight(600.0))
                            .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                            .child("Algorithm"),
                    )
                    .child(div().w(px(1.0)).h(px(24.0)).bg(rgba(0xFFFFFF14u32)))
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(12.5))
                            .font_weight(FontWeight(600.0))
                            .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                            .child("Hash value"),
                    )
                    .child(
                        div()
                            .w(px(28.0))
                            .h(px(28.0))
                            .rounded(px(4.0))
                            .hover(|s| s.bg(rgba(0xFFFFFF12u32)))
                            .cursor_pointer()
                            .flex()
                            .items_center()
                            .justify_center()
                            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                cx.stop_propagation();
                                v_filter.update(cx, |this, cx| {
                                    if let Some(
                                        crate::ui::components::dialogs::DialogType::CustomProperties {
                                            ref mut state,
                                        },
                                    ) = this.state.active_dialog
                                    {
                                        state.show_hash_filter_menu = !state.show_hash_filter_menu;
                                        cx.notify();
                                    }
                                });
                            })
                            .child(
                                div()
                                    .font_family(UiConstants::FONT_ICONS)
                                    .text_size(px(13.0))
                                    .text_color(rgb(UiConstants::TEXT_SECONDARY_RGB))
                                    .child("\u{E7B3}"), // Eye icon
                            ),
                    ),
            );

        // Optional inline algorithm filter bar when Eye button is clicked
        if state.show_hash_filter_menu {
            hashes_card = hashes_card.child(
                div()
                    .px(px(12.0))
                    .py(px(6.0))
                    .bg(rgba(0x00000028u32))
                    .border_b(px(1.0))
                    .border_color(rgba(0xFFFFFF12u32))
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .gap(px(6.0))
                    .children(alg_rows.iter().map(|&(idx, alg_name, _)| {
                        let enabled = state.enabled_hashes[idx];
                        let v_tog = view.clone();
                        div()
                            .px(px(8.0))
                            .py(px(3.0))
                            .rounded(px(4.0))
                            .bg(if enabled {
                                rgba(0x4CC2FF28u32)
                            } else {
                                rgba(0xFFFFFF08u32)
                            })
                            .border(px(1.0))
                            .border_color(if enabled {
                                rgba(0x4CC2FF80u32)
                            } else {
                                rgba(0xFFFFFF18u32)
                            })
                            .cursor_pointer()
                            .text_size(px(11.5))
                            .text_color(if enabled {
                                rgb(0x4CC2FF)
                            } else {
                                rgb(UiConstants::TEXT_SECONDARY_RGB)
                            })
                            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                cx.stop_propagation();
                                v_tog.update(cx, |this, cx| {
                                    if let Some(
                                        crate::ui::components::dialogs::DialogType::CustomProperties {
                                            ref mut state,
                                        },
                                    ) = this.state.active_dialog
                                    {
                                        state.enabled_hashes[idx] = !state.enabled_hashes[idx];
                                        cx.notify();
                                    }
                                });
                            })
                            .child(alg_name.to_string())
                    })),
            );
        }

        // Hash rows
        for (idx, alg_name, val_opt) in alg_rows {
            if !state.enabled_hashes[idx] {
                continue;
            }
            let is_matched_row = matched_alg == Some(alg_name);
            hashes_card = hashes_card.child(Self::build_hash_row(
                alg_name,
                val_opt,
                state.loading_hashes,
                is_matched_row,
            ));
        }

        let mut page = div().flex().flex_col().gap(px(8.0)).child(compare_card);
        if let Some(bar) = match_infobar {
            page = page.child(bar);
        }
        page.child(hashes_card)
    }

    pub(crate) fn build_hash_row(
        label: &str,
        value: Option<&str>,
        loading: bool,
        highlighted: bool,
    ) -> Div {
        let val_str = value.unwrap_or("").to_string();
        let has_val = value.is_some();

        div()
            .px(px(12.0))
            .py(px(8.0))
            .border_b(px(1.0))
            .border_color(rgba(0xFFFFFF0Au32))
            .bg(if highlighted {
                rgba(0x107C1028u32)
            } else {
                rgba(0x00000000u32)
            })
            .hover(|s| s.bg(rgba(0xFFFFFF08u32)))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(12.0))
            .child(
                div()
                    .w(px(68.0))
                    .flex_shrink_0()
                    .text_size(px(12.5))
                    .font_weight(FontWeight(600.0))
                    .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                    .child(label.to_string()),
            )
            .child(div().w(px(1.0)).h(px(18.0)).bg(rgba(0xFFFFFF10u32)))
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.0))
                    .text_size(px(12.0))
                    .font_family("Consolas")
                    .text_color(if loading {
                        rgb(UiConstants::TEXT_SECONDARY_RGB)
                    } else {
                        rgb(0xE0E0E0)
                    })
                    .truncate()
                    .child(if loading {
                        "Calculating...".to_string()
                    } else if has_val {
                        val_str.clone()
                    } else {
                        "—".to_string()
                    }),
            )
            .child(if has_val && !loading {
                div()
                    .w(px(28.0))
                    .h(px(28.0))
                    .rounded(px(4.0))
                    .hover(|s| s.bg(rgba(0xFFFFFF16u32)))
                    .cursor_pointer()
                    .flex()
                    .items_center()
                    .justify_center()
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        cx.stop_propagation();
                        cx.write_to_clipboard(gpui::ClipboardItem::new_string(val_str.clone()));
                    })
                    .child(
                        div()
                            .font_family(UiConstants::FONT_ICONS)
                            .text_size(px(13.0))
                            .text_color(rgb(UiConstants::TEXT_SECONDARY_RGB))
                            .child("\u{E8C8}"), // Copy icon
                    )
                    .into_any_element()
            } else {
                div().w(px(28.0)).into_any_element()
            })
    }

    // ─────────────────────────────────────────────────────────────────────────
    // 4. Details Tab (`DetailsPage.xaml` + Windows Properties Details tab parity)
    // ─────────────────────────────────────────────────────────────────────────
    pub(crate) fn build_properties_details_tab(
        state: &CustomPropertiesState,
        view: Entity<crate::app::AppWindow>,
    ) -> Div {
        let mut page = div().flex().flex_col().gap(px(8.0));

        for (sec_idx, section) in state.details_sections.iter().enumerate() {
            let v_tog = view.clone();
            let rows_body = div()
                .flex()
                .flex_col()
                .gap(px(8.0))
                .children(section.rows.iter().map(|(k, v)| {
                    Self::properties_kv_row(&format!("{}:", k), v, false)
                }));

            page = page.child(Self::properties_expander_card(
                &section.title,
                None,
                section.expanded,
                move |_, _, cx| {
                    v_tog.update(cx, |this, cx| {
                        if let Some(
                            crate::ui::components::dialogs::DialogType::CustomProperties {
                                ref mut state,
                            },
                        ) = this.state.active_dialog
                        {
                            if let Some(sec) = state.details_sections.get_mut(sec_idx) {
                                sec.expanded = !sec.expanded;
                                cx.notify();
                            }
                        }
                    });
                },
                rows_body,
            ));
        }

        // Bottom actions: Copy All Details + Clear Properties (DetailsPage.xaml Footer)
        let all_details_text: String = state
            .details_sections
            .iter()
            .flat_map(|s| s.rows.iter().map(|(k, v)| format!("{}: {}", k, v)))
            .collect::<Vec<_>>()
            .join("\r\n");

        page.child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.0))
                .mt(px(4.0))
                .child(
                    div()
                        .flex_1()
                        .h(px(32.0))
                        .rounded(px(4.0))
                        .bg(rgba(0xFFFFFF0Cu32))
                        .border(px(1.0))
                        .border_color(rgba(0xFFFFFF18u32))
                        .hover(|s| s.bg(rgba(0xFFFFFF16u32)))
                        .cursor_pointer()
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_size(px(12.5))
                        .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                            cx.stop_propagation();
                            cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                                all_details_text.clone(),
                            ));
                        })
                        .child("Copy all properties"),
                ),
        )
    }

    // ─────────────────────────────────────────────────────────────────────────
    // 5. Shortcut Tab (`ShortcutPage.xaml` + Windows Shortcut tab parity)
    // ─────────────────────────────────────────────────────────────────────────
    pub(crate) fn build_properties_shortcut_tab(
        state: &CustomPropertiesState,
        view: Entity<crate::app::AppWindow>,
    ) -> Div {
        let sc = &state.shortcut;
        let target_path_for_nav = sc.target_path.clone();
        let v_nav = view.clone();
        let window_modes = ["Normal window", "Minimized", "Maximized"];

        div()
            .flex()
            .flex_col()
            .gap(px(8.0))
            // Card 1: Target Type, Location, and Target Path
            .child(
                Self::properties_card()
                    .gap(px(10.0))
                    .child(Self::properties_kv_row("Target type:", &sc.target_type, true))
                    .child(Self::properties_kv_row(
                        "Target location:",
                        &sc.target_location,
                        true,
                    ))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(8.0))
                            .child(
                                div()
                                    .w(px(110.0))
                                    .flex_shrink_0()
                                    .text_size(px(13.0))
                                    .font_weight(FontWeight(600.0))
                                    .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                                    .child("Target:"),
                            )
                            .child(
                                div().flex_1().child(Self::properties_text_input(
                                    &sc.target_path,
                                    "Target path",
                                    state.focused_input
                                        == Some(PropertiesInputField::ShortcutTarget),
                                    false,
                                    PropertiesInputField::ShortcutTarget,
                                    view.clone(),
                                )),
                            ),
                    ),
            )
            // Card 2: Arguments, Start in (Working directory), Comment
            .child(
                Self::properties_card()
                    .gap(px(10.0))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(8.0))
                            .child(
                                div()
                                    .w(px(110.0))
                                    .flex_shrink_0()
                                    .text_size(px(13.0))
                                    .font_weight(FontWeight(600.0))
                                    .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                                    .child("Arguments:"),
                            )
                            .child(
                                div().flex_1().child(Self::properties_text_input(
                                    &sc.arguments,
                                    "Command-line arguments",
                                    state.focused_input == Some(PropertiesInputField::ShortcutArgs),
                                    false,
                                    PropertiesInputField::ShortcutArgs,
                                    view.clone(),
                                )),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(8.0))
                            .child(
                                div()
                                    .w(px(110.0))
                                    .flex_shrink_0()
                                    .text_size(px(13.0))
                                    .font_weight(FontWeight(600.0))
                                    .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                                    .child("Start in:"),
                            )
                            .child(
                                div().flex_1().child(Self::properties_text_input(
                                    &sc.working_dir,
                                    "Working directory",
                                    state.focused_input
                                        == Some(PropertiesInputField::ShortcutWorkingDir),
                                    false,
                                    PropertiesInputField::ShortcutWorkingDir,
                                    view.clone(),
                                )),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(8.0))
                            .child(
                                div()
                                    .w(px(110.0))
                                    .flex_shrink_0()
                                    .text_size(px(13.0))
                                    .font_weight(FontWeight(600.0))
                                    .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                                    .child("Comment:"),
                            )
                            .child(
                                div().flex_1().child(Self::properties_text_input(
                                    &sc.comment,
                                    "Description / comment",
                                    state.focused_input
                                        == Some(PropertiesInputField::ShortcutComment),
                                    false,
                                    PropertiesInputField::ShortcutComment,
                                    view.clone(),
                                )),
                            ),
                    ),
            )
            // Card 3: Run window state selector
            .child(
                Self::properties_card().child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .text_size(px(13.0))
                                .font_weight(FontWeight(600.0))
                                .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                                .child("Run window state"),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .gap(px(4.0))
                                .children(window_modes.into_iter().map(|mode| {
                                    let is_cur = sc.window_style == mode;
                                    let v_m = view.clone();
                                    let mode_str = mode.to_string();
                                    Self::build_compact_btn(mode, is_cur, move |_, _, cx| {
                                        v_m.update(cx, |this, cx| {
                                            if let Some(
                                                crate::ui::components::dialogs::DialogType::CustomProperties {
                                                    ref mut state,
                                                },
                                            ) = this.state.active_dialog
                                            {
                                                state.shortcut.window_style = mode_str.clone();
                                                cx.notify();
                                            }
                                        });
                                    })
                                })),
                        ),
                ),
            )
            // Card 4: Run as Administrator toggle
            .child(
                Self::properties_card().child(Self::properties_attribute_row(
                    "Run as administrator",
                    Some(sc.run_as_admin),
                    {
                        let v = view.clone();
                        move |_, _, cx| {
                            v.update(cx, |this, cx| {
                                if let Some(
                                    crate::ui::components::dialogs::DialogType::CustomProperties {
                                        ref mut state,
                                    },
                                ) = this.state.active_dialog
                                {
                                    state.shortcut.run_as_admin = !state.shortcut.run_as_admin;
                                    cx.notify();
                                }
                            });
                        }
                    },
                )),
            )
            // Card 5: Open File Location button (`ShortcutPage.xaml` lines 153-164)
            .child(Self::properties_action_card(
                "Open File Location",
                "\u{E8A7}",
                move |_, _, cx| {
                    if !target_path_for_nav.is_empty() {
                        let p = std::path::PathBuf::from(&target_path_for_nav);
                        if let Some(parent) = p.parent() {
                            let fp = crate::core::path_util::FilesPath::new(parent);
                            v_nav.update(cx, |this, cx| {
                                this.close_dialog(cx);
                                this.navigate(fp, cx);
                            });
                        }
                    }
                },
            ))
    }

    // ─────────────────────────────────────────────────────────────────────────
    // 6. Customization Tab (`CustomizationPage.xaml` + Windows Customize tab)
    // ─────────────────────────────────────────────────────────────────────────
    pub(crate) fn build_properties_customization_tab(
        state: &CustomPropertiesState,
        view: Entity<crate::app::AppWindow>,
    ) -> Div {
        let cust = &state.customization;
        let folder_types = [
            "General items",
            "Documents",
            "Pictures",
            "Music",
            "Videos",
        ];
        let cur_folder_type = cust.folder_type.as_deref().unwrap_or("General items");
        let icon_path_str = cust
            .folder_icon_path
            .as_deref()
            .unwrap_or("C:\\Windows\\System32\\imageres.dll");

        let preset_icons: [(i32, &str, &str); 8] = [
            (3, "\u{E8B7}", "Standard Folder"),
            (112, "\u{E8A5}", "Documents"),
            (113, "\u{EB9F}", "Pictures"),
            (108, "\u{EC4F}", "Music"),
            (189, "\u{E714}", "Videos"),
            (184, "\u{E896}", "Downloads"),
            (102, "\u{E734}", "Favorites"),
            (175, "\u{E753}", "Cloud / Network"),
        ];

        div()
            .flex()
            .flex_col()
            .gap(px(8.0))
            // Card 1: Folder optimization template (Windows Folder Customize tab)
            .child(
                Self::properties_card()
                    .gap(px(10.0))
                    .child(
                        div()
                            .text_size(px(13.0))
                            .font_weight(FontWeight(600.0))
                            .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                            .child("Optimize this folder for:"),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .flex_wrap()
                            .gap(px(6.0))
                            .children(folder_types.into_iter().map(|ft| {
                                let is_cur = cur_folder_type == ft;
                                let v_ft = view.clone();
                                let ft_owned = ft.to_string();
                                Self::build_compact_btn(ft, is_cur, move |_, _, cx| {
                                    v_ft.update(cx, |this, cx| {
                                        if let Some(
                                            crate::ui::components::dialogs::DialogType::CustomProperties {
                                                ref mut state,
                                            },
                                        ) = this.state.active_dialog
                                        {
                                            state.customization.folder_type =
                                                Some(ft_owned.clone());
                                            cx.notify();
                                        }
                                    });
                                })
                            })),
                    )
                    .child(Self::properties_attribute_row(
                        "Also apply this template to all subfolders",
                        Some(cust.apply_to_subfolders),
                        {
                            let v = view.clone();
                            move |_, _, cx| {
                                v.update(cx, |this, cx| {
                                    if let Some(
                                        crate::ui::components::dialogs::DialogType::CustomProperties {
                                            ref mut state,
                                        },
                                    ) = this.state.active_dialog
                                    {
                                        state.customization.apply_to_subfolders =
                                            !state.customization.apply_to_subfolders;
                                        cx.notify();
                                    }
                                });
                            }
                        },
                    )),
            )
            // Card 2: Choose Custom Icon (`CustomizationPage.xaml` lines 30-125)
            .child(
                Self::properties_card()
                    .gap(px(10.0))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .text_size(px(13.0))
                                    .font_weight(FontWeight(600.0))
                                    .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                                    .child("Choose a custom icon"),
                            )
                            .child({
                                let v_restore = view.clone();
                                Self::build_compact_btn(
                                    "Restore default",
                                    false,
                                    move |_, _, cx| {
                                        v_restore.update(cx, |this, cx| {
                                            if let Some(
                                                crate::ui::components::dialogs::DialogType::CustomProperties {
                                                    ref mut state,
                                                },
                                            ) = this.state.active_dialog
                                            {
                                                state.customization.folder_icon_path = None;
                                                state.customization.folder_icon_index = None;
                                                cx.notify();
                                            }
                                        });
                                    },
                                )
                            }),
                    )
                    .child(Self::properties_text_input(
                        icon_path_str,
                        "Icon file or DLL path (e.g. C:\\Windows\\System32\\imageres.dll)",
                        state.focused_input == Some(PropertiesInputField::CustomIconPath),
                        false,
                        PropertiesInputField::CustomIconPath,
                        view.clone(),
                    ))
                    .child(
                        div()
                            .grid()
                            .grid_cols(4)
                            .gap(px(8.0))
                            .children(preset_icons.into_iter().map(|(idx, glyph, name)| {
                                let is_sel = cust.folder_icon_index == Some(idx);
                                let v_ico = view.clone();
                                div()
                                    .p(px(8.0))
                                    .rounded(px(5.0))
                                    .bg(if is_sel {
                                        rgba(0x4CC2FF24u32)
                                    } else {
                                        rgba(0xFFFFFF06u32)
                                    })
                                    .border(px(1.0))
                                    .border_color(if is_sel {
                                        rgba(0x4CC2FF90u32)
                                    } else {
                                        rgba(0xFFFFFF12u32)
                                    })
                                    .hover(|s| s.bg(rgba(0xFFFFFF12u32)))
                                    .cursor_pointer()
                                    .flex()
                                    .flex_col()
                                    .items_center()
                                    .gap(px(4.0))
                                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                        cx.stop_propagation();
                                        v_ico.update(cx, |this, cx| {
                                            if let Some(
                                                crate::ui::components::dialogs::DialogType::CustomProperties {
                                                    ref mut state,
                                                },
                                            ) = this.state.active_dialog
                                            {
                                                if state.customization.folder_icon_path.is_none() {
                                                    state.customization.folder_icon_path = Some(
                                                        "C:\\Windows\\System32\\imageres.dll"
                                                            .to_string(),
                                                    );
                                                }
                                                state.customization.folder_icon_index = Some(idx);
                                                cx.notify();
                                            }
                                        });
                                    })
                                    .child(
                                        div()
                                            .font_family(UiConstants::FONT_ICONS)
                                            .text_size(px(20.0))
                                            .text_color(rgb(0xFFB900))
                                            .child(glyph.to_string()),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(11.0))
                                            .text_color(rgb(UiConstants::TEXT_SECONDARY_RGB))
                                            .truncate()
                                            .child(name.to_string()),
                                    )
                            })),
                    ),
            )
    }

    // ─────────────────────────────────────────────────────────────────────────
    // 7. Compatibility Tab (`CompatibilityPage.xaml` + Windows Compatibility tab)
    // ─────────────────────────────────────────────────────────────────────────
    pub(crate) fn build_properties_compatibility_tab(
        state: &CustomPropertiesState,
        view: Entity<crate::app::AppWindow>,
    ) -> Div {
        let compat = &state.compatibility;
        let os_modes: [(&str, Option<&str>); 5] = [
            ("Disabled", None),
            ("Windows 8", Some("Windows 8")),
            ("Windows 7", Some("Windows 7")),
            ("Windows Vista", Some("Windows Vista")),
            ("Windows XP (SP3)", Some("Windows XP SP3")),
        ];
        let color_modes: [(&str, Option<&str>); 3] = [
            ("None", None),
            ("8-bit (256) color", Some("8-bit (256) color")),
            ("16-bit (65536) color", Some("16-bit (65536) color")),
        ];
        let dpi_modes: [(&str, Option<&str>); 4] = [
            ("None", None),
            ("Application", Some("Application")),
            ("System", Some("System")),
            ("System (Enhanced)", Some("System (Enhanced)")),
        ];

        div()
            .flex()
            .flex_col()
            .gap(px(6.0))
            // Compatibility Mode Card
            .child(
                Self::properties_card()
                    .gap(px(8.0))
                    .child(
                        div()
                            .text_size(px(13.0))
                            .font_weight(FontWeight(600.0))
                            .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                            .child("Compatibility mode (Run this program in compatibility mode for):"),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .flex_wrap()
                            .gap(px(4.0))
                            .children(os_modes.into_iter().map(|(label, val)| {
                                let is_cur = compat.emulation_mode.as_deref() == val;
                                let v_m = view.clone();
                                let val_owned = val.map(|s| s.to_string());
                                Self::build_compact_btn(label, is_cur, move |_, _, cx| {
                                    v_m.update(cx, |this, cx| {
                                        if let Some(
                                            crate::ui::components::dialogs::DialogType::CustomProperties {
                                                ref mut state,
                                            },
                                        ) = this.state.active_dialog
                                        {
                                            state.compatibility.emulation_mode = val_owned.clone();
                                            cx.notify();
                                        }
                                    });
                                })
                            })),
                    ),
            )
            // Reduced Color Mode Card
            .child(
                Self::properties_card()
                    .gap(px(8.0))
                    .child(
                        div()
                            .text_size(px(13.0))
                            .font_weight(FontWeight(600.0))
                            .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                            .child("Reduced color mode"),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .flex_wrap()
                            .gap(px(4.0))
                            .children(color_modes.into_iter().map(|(label, val)| {
                                let is_cur = compat.color_mode.as_deref() == val;
                                let v_c = view.clone();
                                let val_owned = val.map(|s| s.to_string());
                                Self::build_compact_btn(label, is_cur, move |_, _, cx| {
                                    v_c.update(cx, |this, cx| {
                                        if let Some(
                                            crate::ui::components::dialogs::DialogType::CustomProperties {
                                                ref mut state,
                                            },
                                        ) = this.state.active_dialog
                                        {
                                            state.compatibility.color_mode = val_owned.clone();
                                            cx.notify();
                                        }
                                    });
                                })
                            })),
                    ),
            )
            // Settings Toggles Card (`CompatibilityPage.xaml` lines 57-76)
            .child(
                Self::properties_card()
                    .gap(px(8.0))
                    .child(Self::properties_attribute_row(
                        "Run in 640 × 480 screen resolution",
                        Some(compat.run_640x480),
                        {
                            let v = view.clone();
                            move |_, _, cx| {
                                v.update(cx, |this, cx| {
                                    if let Some(
                                        crate::ui::components::dialogs::DialogType::CustomProperties {
                                            ref mut state,
                                        },
                                    ) = this.state.active_dialog
                                    {
                                        state.compatibility.run_640x480 =
                                            !state.compatibility.run_640x480;
                                        cx.notify();
                                    }
                                });
                            }
                        },
                    ))
                    .child(div().h(px(1.0)).mx(px(-12.0)).bg(rgba(0xFFFFFF10u32)))
                    .child(Self::properties_attribute_row(
                        "Disable fullscreen optimizations",
                        Some(compat.disable_fullscreen_optimizations),
                        {
                            let v = view.clone();
                            move |_, _, cx| {
                                v.update(cx, |this, cx| {
                                    if let Some(
                                        crate::ui::components::dialogs::DialogType::CustomProperties {
                                            ref mut state,
                                        },
                                    ) = this.state.active_dialog
                                    {
                                        state.compatibility.disable_fullscreen_optimizations =
                                            !state.compatibility.disable_fullscreen_optimizations;
                                        cx.notify();
                                    }
                                });
                            }
                        },
                    ))
                    .child(div().h(px(1.0)).mx(px(-12.0)).bg(rgba(0xFFFFFF10u32)))
                    .child(Self::properties_attribute_row(
                        "Run this program as an administrator",
                        Some(compat.run_as_admin),
                        {
                            let v = view.clone();
                            move |_, _, cx| {
                                v.update(cx, |this, cx| {
                                    if let Some(
                                        crate::ui::components::dialogs::DialogType::CustomProperties {
                                            ref mut state,
                                        },
                                    ) = this.state.active_dialog
                                    {
                                        state.compatibility.run_as_admin =
                                            !state.compatibility.run_as_admin;
                                        cx.notify();
                                    }
                                });
                            }
                        },
                    )),
            )
            // High DPI Scaling Override Card
            .child(
                Self::properties_card()
                    .gap(px(8.0))
                    .child(
                        div()
                            .text_size(px(13.0))
                            .font_weight(FontWeight(600.0))
                            .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                            .child("Override high DPI scaling behavior (Scaling performed by):"),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .flex_wrap()
                            .gap(px(4.0))
                            .children(dpi_modes.into_iter().map(|(label, val)| {
                                let is_cur = compat.dpi_settings.as_deref() == val;
                                let v_d = view.clone();
                                let val_owned = val.map(|s| s.to_string());
                                Self::build_compact_btn(label, is_cur, move |_, _, cx| {
                                    v_d.update(cx, |this, cx| {
                                        if let Some(
                                            crate::ui::components::dialogs::DialogType::CustomProperties {
                                                ref mut state,
                                            },
                                        ) = this.state.active_dialog
                                        {
                                            state.compatibility.dpi_settings = val_owned.clone();
                                            cx.notify();
                                        }
                                    });
                                })
                            })),
                    ),
            )
    }

    // ─────────────────────────────────────────────────────────────────────────
    // 8. Signatures Tab (`SignaturesPage.xaml` + Windows Digital Signatures)
    // ─────────────────────────────────────────────────────────────────────────
    pub(crate) fn build_properties_signatures_tab(
        state: &CustomPropertiesState,
        _view: Entity<crate::app::AppWindow>,
    ) -> AnyElement {
        let sig = &state.signature;

        if !sig.is_signed {
            return div()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .py(px(64.0))
                .gap(px(12.0))
                .child(
                    div()
                        .font_family(UiConstants::FONT_ICONS)
                        .text_size(px(36.0))
                        .text_color(rgb(UiConstants::TEXT_MUTED_RGB))
                        .child("\u{EB95}"),
                )
                .child(
                    div()
                        .text_size(px(16.0))
                        .font_weight(FontWeight(600.0))
                        .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                        .child("No signature found"),
                )
                .child(
                    div()
                        .text_size(px(12.5))
                        .text_color(rgb(UiConstants::TEXT_SECONDARY_RGB))
                        .child("This file does not contain an embedded Authenticode digital signature."),
                )
                .into_any_element();
        }

        div()
            .flex()
            .flex_col()
            .gap(px(8.0))
            .child(
                Self::properties_card()
                    .gap(px(10.0))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(12.0))
                            .child(
                                div()
                                    .font_family(UiConstants::FONT_ICONS)
                                    .text_size(px(24.0))
                                    .text_color(if sig.is_valid {
                                        rgb(0x6CCB5F)
                                    } else {
                                        rgb(0xFF99A4)
                                    })
                                    .child(if sig.is_valid {
                                        "\u{E930}"
                                    } else {
                                        "\u{EA39}"
                                    }),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(2.0))
                                    .child(
                                        div()
                                            .text_size(px(14.0))
                                            .font_weight(FontWeight(600.0))
                                            .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                                            .child(
                                                sig.signer_name
                                                    .as_deref()
                                                    .unwrap_or("Signed Publisher")
                                                    .to_string(),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(12.0))
                                            .text_color(rgb(UiConstants::TEXT_SECONDARY_RGB))
                                            .child(if sig.is_valid {
                                                "This digital signature is OK."
                                            } else {
                                                "This digital signature could not be verified."
                                            }),
                                    ),
                            ),
                    )
                    .child(div().h(px(1.0)).mx(px(-12.0)).bg(rgba(0xFFFFFF12u32)))
                    .child(Self::properties_kv_row(
                        "Version:",
                        sig.version.as_deref().unwrap_or("V3"),
                        false,
                    ))
                    .child(Self::properties_kv_row(
                        "Issued by:",
                        sig.issuer_name.as_deref().unwrap_or("—"),
                        false,
                    ))
                    .child(Self::properties_kv_row(
                        "Issued to:",
                        sig.signer_name.as_deref().unwrap_or("—"),
                        false,
                    ))
                    .child(Self::properties_kv_row(
                        "Serial number:",
                        sig.serial_number.as_deref().unwrap_or("—"),
                        false,
                    ))
                    .child(Self::properties_kv_row(
                        "Thumbprint:",
                        sig.thumbprint.as_deref().unwrap_or("—"),
                        false,
                    )),
            )
            .into_any_element()
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Reusable Properties Card & Control Primitives (`PropertiesStyles.xaml`)
    // ─────────────────────────────────────────────────────────────────────────

    pub(crate) fn properties_card() -> Div {
        div()
            .w_full()
            .p(px(12.0))
            .bg(rgba(0xFFFFFF08u32))
            .border(px(1.0))
            .border_color(rgba(0xFFFFFF12u32))
            .rounded(px(5.0))
            .flex()
            .flex_col()
    }

    pub(crate) fn properties_expander_card(
        title: &str,
        subtitle: Option<&str>,
        expanded: bool,
        on_toggle: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
        body: impl IntoElement,
    ) -> Div {
        let mut container = div()
            .w_full()
            .bg(rgba(0xFFFFFF08u32))
            .border(px(1.0))
            .border_color(rgba(0xFFFFFF12u32))
            .rounded(px(5.0))
            .overflow_hidden()
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(44.0))
                    .px(px(12.0))
                    .hover(|s| s.bg(rgba(0xFFFFFF06u32)))
                    .cursor_pointer()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .on_mouse_down(MouseButton::Left, move |ev, win, cx| {
                        cx.stop_propagation();
                        on_toggle(ev, win, cx);
                    })
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(10.0))
                            .child(
                                div()
                                    .text_size(px(13.0))
                                    .font_weight(FontWeight(600.0))
                                    .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                                    .child(title.to_string()),
                            )
                            .children(subtitle.filter(|s| !expanded && !s.is_empty()).map(|s| {
                                div()
                                    .text_size(px(12.0))
                                    .text_color(rgb(UiConstants::TEXT_SECONDARY_RGB))
                                    .child(format!("• {}", s))
                            })),
                    )
                    .child(
                        div()
                            .font_family(UiConstants::FONT_ICONS)
                            .text_size(px(11.0))
                            .text_color(rgb(UiConstants::TEXT_SECONDARY_RGB))
                            .child(if expanded { "\u{E70E}" } else { "\u{E70D}" }),
                    ),
            );

        if expanded {
            container = container.child(
                div()
                    .p(px(12.0))
                    .border_t(px(1.0))
                    .border_color(rgba(0xFFFFFF12u32))
                    .bg(rgba(0x00000018u32))
                    .child(body),
            );
        }
        container
    }

    pub(crate) fn properties_action_card(
        title: &str,
        glyph: &str,
        on_click: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    ) -> Div {
        div()
            .w_full()
            .h(px(46.0))
            .px(px(14.0))
            .bg(rgba(0xFFFFFF08u32))
            .border(px(1.0))
            .border_color(rgba(0xFFFFFF12u32))
            .rounded(px(5.0))
            .hover(|s| s.bg(rgba(0xFFFFFF12u32)))
            .cursor_pointer()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .on_mouse_down(MouseButton::Left, move |ev, win, cx| {
                cx.stop_propagation();
                on_click(ev, win, cx);
            })
            .child(
                div()
                    .text_size(px(13.0))
                    .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                    .child(title.to_string()),
            )
            .child(
                div()
                    .font_family(UiConstants::FONT_ICONS)
                    .text_size(px(13.0))
                    .text_color(rgb(UiConstants::TEXT_SECONDARY_RGB))
                    .child(glyph.to_string()),
            )
    }

    pub(crate) fn properties_kv_row(label: &str, value: &str, bold_label: bool) -> Div {
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.0))
            .child(
                div()
                    .w(px(110.0))
                    .flex_shrink_0()
                    .text_size(px(13.0))
                    .font_weight(if bold_label {
                        FontWeight(600.0)
                    } else {
                        FontWeight(400.0)
                    })
                    .text_color(rgb(0xE0E0E0))
                    .child(label.to_string()),
            )
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.0))
                    .text_size(px(13.0))
                    .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                    .child(value.to_string()),
            )
    }

    pub(crate) fn properties_text_input(
        value: &str,
        placeholder: &str,
        focused: bool,
        selected_all: bool,
        field: PropertiesInputField,
        view: Entity<crate::app::AppWindow>,
    ) -> Div {
        let is_empty = value.is_empty();
        div()
            .w_full()
            .h(px(32.0))
            .px(px(10.0))
            .rounded(px(4.0))
            .bg(if focused {
                rgba(0x181818E0u32)
            } else {
                rgba(0xFFFFFF0Au32)
            })
            .border(px(1.0))
            .border_b(px(if focused { 2.0 } else { 1.0 }))
            .border_color(if focused {
                rgba(0x4CC2FFFFu32)
            } else {
                rgba(0xFFFFFF22u32)
            })
            .cursor_text()
            .flex()
            .flex_row()
            .items_center()
            .overflow_hidden()
            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                cx.stop_propagation();
                view.update(cx, |this, cx| {
                    if let Some(crate::ui::components::dialogs::DialogType::CustomProperties {
                        ref mut state,
                    }) = this.state.active_dialog
                    {
                        state.focused_input = Some(field);
                        state.name_selected_all = false;
                        cx.notify();
                    }
                });
            })
            .child(
                div()
                    .px(px(if selected_all && !is_empty { 2.0 } else { 0.0 }))
                    .rounded(px(2.0))
                    .bg(if selected_all && !is_empty {
                        rgba(0x0078D488u32)
                    } else {
                        rgba(0x00000000u32)
                    })
                    .text_size(px(13.0))
                    .text_color(if is_empty {
                        rgb(UiConstants::TEXT_MUTED_RGB)
                    } else {
                        rgb(UiConstants::TEXT_PRIMARY_RGB)
                    })
                    .truncate()
                    .child(if is_empty {
                        placeholder.to_string()
                    } else if focused && !selected_all {
                        format!("{}|", value)
                    } else {
                        value.to_string()
                    }),
            )
    }

    pub(crate) fn properties_attribute_row(
        label: &str,
        checked: Option<bool>,
        on_toggle: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    ) -> Div {
        div()
            .w_full()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .cursor_pointer()
            .on_mouse_down(MouseButton::Left, move |ev, win, cx| {
                cx.stop_propagation();
                on_toggle(ev, win, cx);
            })
            .child(
                div()
                    .text_size(px(13.0))
                    .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                    .child(label.to_string()),
            )
            .child(Self::properties_tristate_box(checked))
    }

    pub(crate) fn properties_tristate_checkbox(
        label: &str,
        checked: Option<bool>,
        on_toggle: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    ) -> Div {
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.0))
            .cursor_pointer()
            .on_mouse_down(MouseButton::Left, move |ev, win, cx| {
                cx.stop_propagation();
                on_toggle(ev, win, cx);
            })
            .child(
                div()
                    .text_size(px(13.0))
                    .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                    .child(label.to_string()),
            )
            .child(Self::properties_tristate_box(checked))
    }

    pub(crate) fn properties_tristate_box(checked: Option<bool>) -> Div {
        let is_active = matches!(checked, Some(true) | None);
        div()
            .w(px(18.0))
            .h(px(18.0))
            .rounded(px(4.0))
            .border(px(1.0))
            .border_color(if is_active {
                rgba(0x4CC2FFFFu32)
            } else {
                rgba(0xFFFFFF45u32)
            })
            .bg(if is_active {
                rgba(0x4CC2FFFFu32)
            } else {
                rgba(0x00000020u32)
            })
            .flex()
            .items_center()
            .justify_center()
            .child(match checked {
                Some(true) => div()
                    .font_family(UiConstants::FONT_ICONS)
                    .text_size(px(11.0))
                    .text_color(rgb(0x000000))
                    .child("\u{E8FB}"), // Checkmark
                None => div()
                    .font_family(UiConstants::FONT_ICONS)
                    .text_size(px(11.0))
                    .text_color(rgb(0x000000))
                    .child("\u{E738}"), // Indeterminate dash (multi-selection mixed state)
                Some(false) => div(),
            })
    }

    pub(crate) fn properties_readonly_check_indicator(checked: bool, is_deny: bool) -> Div {
        div()
            .w(px(18.0))
            .h(px(18.0))
            .rounded(px(4.0))
            .border(px(1.0))
            .border_color(if checked {
                if is_deny {
                    rgba(0xFF99A4AAu32)
                } else {
                    rgba(0x4CC2FFAAu32)
                }
            } else {
                rgba(0xFFFFFF24u32)
            })
            .bg(if checked {
                if is_deny {
                    rgba(0xC42B1C66u32)
                } else {
                    rgba(0x4CC2FF44u32)
                }
            } else {
                rgba(0x00000018u32)
            })
            .flex()
            .items_center()
            .justify_center()
            .child(if checked {
                div()
                    .font_family(UiConstants::FONT_ICONS)
                    .text_size(px(11.0))
                    .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                    .child("\u{E8FB}")
            } else {
                div()
            })
    }

    pub(crate) fn properties_checkbox(
        label: &str,
        checked: bool,
        on_toggle: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    ) -> Div {
        Self::properties_tristate_checkbox(label, Some(checked), on_toggle)
    }
}

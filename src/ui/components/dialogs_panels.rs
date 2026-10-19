use crate::ui::constants::UiConstants;
use gpui::*;

impl crate::ui::components::dialogs::DialogsView {
    pub(crate) fn build_custom_properties_dialog(
        state: &crate::ui::dialog_services::custom_properties_dialog::CustomPropertiesState,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let v_close = view.clone();
        let active_tab = state.active_tab;
        let tabs = crate::ui::dialog_services::custom_properties_dialog::PropertiesTab::all();

        div()
            .flex_col()
            .w(px(600.0))
            .h(px(500.0))
            .bg(rgb(0x202020))
            .rounded(px(8.0))
            .border(px(1.0))
            .border_color(rgba(UiConstants::DIVIDER_RGBA))
            // Header
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .px(px(16.0))
                    .py(px(12.0))
                    .border_b(px(1.0))
                    .border_color(rgba(UiConstants::DIVIDER_RGBA))
                    .child(
                        div()
                            .text_size(px(18.0))
                            .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                            .font_weight(FontWeight(600.0))
                            .child(format!("{} Properties", state.name)),
                    )
                    .child(
                        div()
                            .cursor_pointer()
                            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                v_close.update(cx, |this, cx| this.close_dialog(cx));
                            })
                            .child("\u{E8BB}"), // Close icon (X)
                    ),
            )
            // Tab bar + content
            .child(
                div()
                    .flex()
                    .flex_grow()
                    .overflow_hidden()
                    // Tab sidebar
                    .child(
                        div()
                            .w(px(160.0))
                            .flex_col()
                            .border_r(px(1.0))
                            .border_color(rgba(UiConstants::DIVIDER_RGBA))
                            .p(px(8.0))
                            .gap(px(4.0))
                            .children(tabs.iter().map(|&tab| {
                                let v_tab = view.clone();
                                let is_active = tab == active_tab;
                                Self::properties_tab_btn(tab.display_name(), tab, is_active, v_tab)
                            })),
                    )
                    // Tab content
                    .child(
                        div()
                            .flex_grow()
                            .flex_col()
                            .p(px(16.0))
                            .gap(px(12.0))
                            .child(Self::build_properties_tab_content(state, view.clone())),
                    ),
            )
            // Footer with OK/Cancel buttons
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap(px(8.0))
                    .px(px(16.0))
                    .py(px(12.0))
                    .border_t(px(1.0))
                    .border_color(rgba(UiConstants::DIVIDER_RGBA))
                    .child({
                        let v = view.clone();
                        Self::build_button("Cancel", false, move |_, _, cx| {
                            v.update(cx, |this, cx| this.close_dialog(cx));
                        })
                    })
                    .child({
                        let v = view.clone();
                        Self::build_button("OK", true, move |_, _, cx| {
                            v.update(cx, |this, cx| this.close_dialog(cx));
                        })
                    }),
            )
    }

    /// Render a tab button in the properties dialog sidebar
    pub(crate) fn properties_tab_btn(
        label: &str,
        tab: crate::ui::dialog_services::custom_properties_dialog::PropertiesTab,
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
                view.update(cx, |this, cx| {
                    // The state lives in DialogType::CustomProperties
                    if let Some(crate::ui::components::dialogs::DialogType::CustomProperties {
                        ref mut state,
                    }) = this.state.active_dialog
                    {
                        state.set_tab(tab);
                        cx.notify();
                    }
                });
            })
            .child(
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

    /// Render content for the currently active properties tab
    pub(crate) fn build_properties_tab_content(
        state: &crate::ui::dialog_services::custom_properties_dialog::CustomPropertiesState,
        view: Entity<crate::app::AppWindow>,
    ) -> AnyElement {
        match state.active_tab {
            crate::ui::dialog_services::custom_properties_dialog::PropertiesTab::General => {
                Self::build_properties_general_tab(state).into_any_element()
            }
            crate::ui::dialog_services::custom_properties_dialog::PropertiesTab::Security => {
                Self::build_properties_security_tab(state).into_any_element()
            }
            crate::ui::dialog_services::custom_properties_dialog::PropertiesTab::Customization => {
                Self::build_properties_customization_tab(state).into_any_element()
            }
            crate::ui::dialog_services::custom_properties_dialog::PropertiesTab::Hashes => {
                Self::build_properties_hashes_tab(state).into_any_element()
            }
            crate::ui::dialog_services::custom_properties_dialog::PropertiesTab::Compatibility => {
                Self::build_properties_compatibility_tab(state, view).into_any_element()
            }
            crate::ui::dialog_services::custom_properties_dialog::PropertiesTab::Signatures => {
                Self::build_properties_signatures_tab(state).into_any_element()
            }
        }
    }

    // ────────── General Tab ──────────
    pub(crate) fn build_properties_general_tab(
        state: &crate::ui::dialog_services::custom_properties_dialog::CustomPropertiesState,
    ) -> impl IntoElement {
        div()
            .flex_col()
            .gap(px(8.0))
            .child(
                div()
                    .text_size(px(16.0))
                    .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                    .font_weight(FontWeight(600.0))
                    .child("General"),
            )
            .child(Self::build_settings_row("Name", &state.name))
            .child(Self::build_settings_row("Type", &state.file_type))
            .child(Self::build_settings_row("Size", &state.formatted_size()))
            .child(if let Some(ref created) = state.created {
                Self::build_settings_row("Created", created).into_any_element()
            } else {
                div().into_any_element()
            })
            .child(if let Some(ref modified) = state.modified {
                Self::build_settings_row("Modified", modified).into_any_element()
            } else {
                div().into_any_element()
            })
            .child(if let Some(ref accessed) = state.accessed {
                Self::build_settings_row("Accessed", accessed).into_any_element()
            } else {
                div().into_any_element()
            })
            .child(Self::build_settings_row(
                "Attributes",
                &state.attribute_summary(),
            ))
    }

    // ────────── Security Tab ──────────
    pub(crate) fn build_properties_security_tab(
        state: &crate::ui::dialog_services::custom_properties_dialog::CustomPropertiesState,
    ) -> impl IntoElement {
        let sec = &state.security;
        div()
            .flex_col()
            .gap(px(8.0))
            .child(
                div()
                    .text_size(px(16.0))
                    .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                    .font_weight(FontWeight(600.0))
                    .child("Security"),
            )
            .child(if let Some(ref owner) = sec.owner {
                Self::build_settings_row("Owner", owner).into_any_element()
            } else {
                div().into_any_element()
            })
            .child(Self::build_settings_row(
                "Permissions",
                &sec.permissions_summary,
            ))
            .child(Self::properties_checkbox(
                "Readable",
                sec.can_read,
                |_, _, _| {},
            ))
            .child(Self::properties_checkbox(
                "Writable",
                sec.can_write,
                |_, _, _| {},
            ))
            .child(Self::properties_checkbox(
                "Deletable",
                sec.can_delete,
                |_, _, _| {},
            ))
            .child(Self::properties_checkbox(
                "Read-only",
                sec.is_readonly,
                |_, _, _| {},
            ))
            .child(Self::properties_checkbox(
                "Hidden",
                sec.is_hidden,
                |_, _, _| {},
            ))
            .child(Self::properties_checkbox(
                "System",
                sec.is_system,
                |_, _, _| {},
            ))
    }

    // ────────── Customization Tab ──────────
    pub(crate) fn build_properties_customization_tab(
        state: &crate::ui::dialog_services::custom_properties_dialog::CustomPropertiesState,
    ) -> impl IntoElement {
        let cust = &state.customization;
        div()
            .flex_col()
            .gap(px(8.0))
            .child(
                div()
                    .text_size(px(16.0))
                    .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                    .font_weight(FontWeight(600.0))
                    .child("Customization"),
            )
            .child(if let Some(ref icon_path) = cust.folder_icon_path {
                Self::build_settings_row("Icon", icon_path).into_any_element()
            } else {
                div().into_any_element()
            })
            .child(if let Some(ref icon_index) = cust.folder_icon_index {
                Self::build_settings_row("Icon Index", &icon_index.to_string()).into_any_element()
            } else {
                div().into_any_element()
            })
            .child(if let Some(ref bg_img) = cust.folder_background_image {
                Self::build_settings_row("Background", bg_img).into_any_element()
            } else {
                div().into_any_element()
            })
            .child(Self::build_settings_row(
                "Opacity",
                &format!("{}%", cust.folder_background_opacity),
            ))
            .child(if let Some(ref ftype) = cust.folder_type {
                Self::build_settings_row("Folder Type", ftype).into_any_element()
            } else {
                div().into_any_element()
            })
            .child(Self::properties_checkbox(
                "Apply to subfolders",
                cust.apply_to_subfolders,
                |_, _, _| {},
            ))
    }

    // ────────── Hashes Tab ──────────
    pub(crate) fn build_properties_hashes_tab(
        state: &crate::ui::dialog_services::custom_properties_dialog::CustomPropertiesState,
    ) -> impl IntoElement {
        let hashes = &state.hashes;
        div()
            .flex_col()
            .gap(px(8.0))
            .child(
                div()
                    .text_size(px(16.0))
                    .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                    .font_weight(FontWeight(600.0))
                    .child("File Hashes"),
            )
            .child(if state.loading_hashes {
                div()
                    .text_color(rgb(UiConstants::TEXT_MUTED_RGB))
                    .child("Computing hashes...")
            } else {
                div()
                    .flex_col()
                    .gap(px(4.0))
                    .child(Self::build_hash_row("MD5", hashes.md5.as_deref()))
                    .child(Self::build_hash_row("SHA-1", hashes.sha1.as_deref()))
                    .child(Self::build_hash_row("SHA-256", hashes.sha256.as_deref()))
                    .child(Self::build_hash_row("SHA-384", hashes.sha384.as_deref()))
                    .child(Self::build_hash_row("SHA-512", hashes.sha512.as_deref()))
                    .child(Self::build_hash_row("CRC32", hashes.crc32.as_deref()))
            })
    }

    pub(crate) fn build_hash_row(label: &str, value: Option<&str>) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .gap(px(8.0))
            .child(
                div()
                    .w(px(70.0))
                    .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                    .child(label.to_string()),
            )
            .child(
                div()
                    .text_size(px(12.0))
                    .font_family("Consolas")
                    .text_color(rgb(UiConstants::TEXT_MUTED_RGB))
                    .child(value.unwrap_or("—").to_string()),
            )
    }

    // ────────── Compatibility Tab ──────────
    pub(crate) fn build_properties_compatibility_tab(
        state: &crate::ui::dialog_services::custom_properties_dialog::CustomPropertiesState,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let compat = &state.compatibility;
        let emulation_options = [
            ("None", ""),
            ("Windows 95", "Windows 95"),
            ("Windows 98/ME", "Windows 98"),
            ("Windows XP (SP2)", "Windows XP"),
            ("Windows XP (SP3)", "Windows XP SP3"),
            ("Windows Vista", "Windows Vista"),
            ("Windows 7", "Windows 7"),
            ("Windows 8", "Windows 8"),
        ];

        let dpi_options = [
            ("None", ""),
            ("Application", "Application"),
            ("System", "System"),
            ("System (Enhanced)", "System (Enhanced)"),
        ];

        div()
            .flex_col()
            .gap(px(12.0))
            .child(
                div()
                    .text_size(px(16.0))
                    .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                    .font_weight(FontWeight(600.0))
                    .child("Compatibility"),
            )
            // Run as administrator checkbox
            .child(Self::properties_checkbox(
                "Run this program as an administrator",
                compat.run_as_admin,
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
            ))
            // Reduced color mode checkbox
            .child(Self::properties_checkbox(
                "Reduced color mode",
                compat.color_mode.is_some(),
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
                                state.compatibility.color_mode =
                                    if state.compatibility.color_mode.is_some() {
                                        None
                                    } else {
                                        Some("256-color".to_string())
                                    };
                                cx.notify();
                            }
                        });
                    }
                },
            ))
            // Disable theming checkbox
            .child(Self::properties_checkbox(
                "Disable visual themes",
                compat.disable_theming,
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
                                state.compatibility.disable_theming =
                                    !state.compatibility.disable_theming;
                                cx.notify();
                            }
                        });
                    }
                },
            ))
            // Disable desktop composition checkbox
            .child(Self::properties_checkbox(
                "Disable desktop composition",
                compat.disable_visual_features,
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
                                state.compatibility.disable_visual_features =
                                    !state.compatibility.disable_visual_features;
                                cx.notify();
                            }
                        });
                    }
                },
            ))
            // Run in 640x480 screen resolution checkbox
            .child(Self::properties_checkbox(
                "Run in 640×480 screen resolution",
                false,
                |_, _, _| {},
            ))
            // Override high DPI scaling behavior
            .child(
                div().mt(px(8.0)).child(
                    div()
                        .text_color(rgb(UiConstants::TEXT_SECONDARY_RGB))
                        .child("Override high DPI scaling behavior:"),
                ),
            )
            // Emulation mode dropdown
            .child(
                div().mt(px(4.0)).child(
                    div()
                        .text_color(rgb(UiConstants::TEXT_SECONDARY_RGB))
                        .child("Run this program in compatibility mode for:"),
                ),
            )
            .child(Self::properties_dropdown(
                emulation_options.iter().map(|(l, v)| (*l, *v)).collect(),
                compat.emulation_mode.as_deref().unwrap_or(""),
                {
                    let v = view.clone();
                    let val = std::sync::Arc::new(std::sync::Mutex::new(None::<String>));
                    let val_clone = val.clone();
                    move |selected_value: &str| {
                        let v = v.clone();
                        let new_val = if selected_value.is_empty() {
                            None
                        } else {
                            Some(selected_value.to_string())
                        };
                        *val_clone.lock().unwrap() = new_val;
                        // The dropdown callback runs in GPUI event context; state will be read on next render
                    }
                },
            ))
            // DPI settings dropdown
            .child(
                div().mt(px(8.0)).child(
                    div()
                        .text_color(rgb(UiConstants::TEXT_SECONDARY_RGB))
                        .child("Change high DPI settings:"),
                ),
            )
            .child(Self::properties_dropdown(
                dpi_options.iter().map(|(l, v)| (*l, *v)).collect(),
                compat.dpi_settings.as_deref().unwrap_or(""),
                {
                    let v = view.clone();
                    let val = std::sync::Arc::new(std::sync::Mutex::new(None::<String>));
                    let val_clone = val.clone();
                    move |selected_value: &str| {
                        let v = v.clone();
                        let new_val = if selected_value.is_empty() {
                            None
                        } else {
                            Some(selected_value.to_string())
                        };
                        *val_clone.lock().unwrap() = new_val;
                    }
                },
            ))
            // Run in sandbox checkbox
            .child(Self::properties_checkbox(
                "Run this program in a sandbox",
                compat.run_in_sandbox,
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
                                state.compatibility.run_in_sandbox =
                                    !state.compatibility.run_in_sandbox;
                                cx.notify();
                            }
                        });
                    }
                },
            ))
    }

    // ────────── Signatures Tab ──────────
    pub(crate) fn build_properties_signatures_tab(
        state: &crate::ui::dialog_services::custom_properties_dialog::CustomPropertiesState,
    ) -> impl IntoElement {
        let sig = &state.signature;

        div()
            .flex_col()
            .gap(px(12.0))
            .child(
                div()
                    .text_size(px(16.0))
                    .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                    .font_weight(FontWeight(600.0))
                    .child("Digital Signatures"),
            )
            // Signature status summary
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        div()
                            .text_size(px(32.0))
                            .text_color(if sig.is_signed && sig.is_valid {
                                rgb(0x66DD66)
                            } else if sig.is_signed && !sig.is_valid {
                                rgb(0xDD6666)
                            } else {
                                rgb(UiConstants::TEXT_MUTED_RGB)
                            })
                            .child(if sig.is_signed && sig.is_valid {
                                "\u{E930}" // Checkmark
                            } else if sig.is_signed {
                                "\u{E92B}" // Warning
                            } else {
                                "\u{E946}" // Info
                            }),
                    )
                    .child(
                        div()
                            .flex_col()
                            .child(
                                div()
                                    .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                                    .child(if sig.is_signed {
                                        if sig.is_valid {
                                            "✓ Valid Signature".to_string()
                                        } else {
                                            "✗ Invalid Signature".to_string()
                                        }
                                    } else {
                                        "No digital signature".to_string()
                                    }),
                            )
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(rgb(UiConstants::TEXT_MUTED_RGB))
                                    .child(if sig.is_signed {
                                        format!("Signed by: {}", sig.signer_name.as_deref().unwrap_or("Unknown"))
                                    } else {
                                        "This file is not digitally signed.".to_string()
                                    }),
                            ),
                    ),
            )
            // Signer details (expandable section)
            .child(if sig.is_signed {
                div()
                    .flex_col()
                    .gap(px(8.0))
                    .mt(px(8.0))
                    .child(
                        div()
                            .text_size(px(14.0))
                            .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                            .font_weight(FontWeight(600.0))
                            .child("Signer Details"),
                    )
                    .child(Self::build_settings_row("Signer", sig.signer_name.as_deref().unwrap_or("—")))
                    .child(Self::build_settings_row("Issuer", sig.issuer_name.as_deref().unwrap_or("—")))
                    .child(Self::build_settings_row("Serial #", sig.serial_number.as_deref().unwrap_or("—")))
                    .child(Self::build_settings_row("Thumbprint", sig.thumbprint.as_deref().unwrap_or("—")))
                    .child(if let Some(ref from) = sig.valid_from {
                        Self::build_settings_row("Valid From", from).into_any_element()
                    } else {
                        div().into_any_element()
                    })
                    .child(if let Some(ref to) = sig.valid_to {
                        Self::build_settings_row("Valid To", to).into_any_element()
                    } else {
                        div().into_any_element()
                    })
                    // View Details button placeholder
                    .child(
                        div()
                            .mt(px(12.0))
                            .child(Self::build_button("View Details", false, |_, _, _| {})),
                    )
            } else {
                div()
                    .mt(px(4.0))
                    .text_color(rgb(UiConstants::TEXT_MUTED_RGB))
                    .child("Digital signatures help verify the authenticity and integrity of files. Only signed files from trusted publishers are shown.")
            })
    }
    pub(crate) fn properties_checkbox(
        label: &str,
        checked: bool,
        on_toggle: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    ) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .gap(px(8.0))
            .cursor_pointer()
            .on_mouse_down(MouseButton::Left, on_toggle)
            .child(
                div()
                    .w(px(18.0))
                    .h(px(18.0))
                    .border(px(1.0))
                    .border_color(rgba(UiConstants::STROKE_RGBA))
                    .rounded(px(3.0))
                    .bg(if checked {
                        rgba(UiConstants::ACCENT_RGBA)
                    } else {
                        rgba(0xFFFFFF12u32)
                    })
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(if checked {
                        div()
                            .text_size(px(12.0))
                            .text_color(rgb(0xFFFFFF))
                            .child("\u{E8FB}") // Checkmark glyph
                    } else {
                        div()
                    }),
            )
            .child(
                div()
                    .text_size(px(13.0))
                    .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                    .child(label.to_string()),
            )
    }

    // ────────── Helper: Dropdown ──────────
    pub(crate) fn properties_dropdown<'a>(
        options: Vec<(&'a str, &'a str)>,
        current_value: &'a str,
        on_select: impl Fn(&str) + 'static,
    ) -> impl IntoElement + 'a {
        let v_on_select = std::sync::Arc::new(on_select);
        let display = options
            .iter()
            .find(|(_, v)| *v == current_value)
            .map(|(l, _)| (*l).to_string())
            .unwrap_or_else(|| "None".to_string());
        div()
            .flex_col()
            .gap(px(4.0))
            .child(
                div()
                    .flex()
                    .items_center()
                    .h(px(32.0))
                    .px(px(8.0))
                    .bg(rgba(0xFFFFFF12u32))
                    .border(px(1.0))
                    .border_color(rgba(UiConstants::STROKE_RGBA))
                    .rounded(px(4.0))
                    .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                    .text_size(px(13.0))
                    .child(div().child(display))
                    .child(
                        div()
                            .ml(px(8.0))
                            .text_size(px(10.0))
                            .text_color(rgb(UiConstants::TEXT_MUTED_RGB))
                            .child("\u{E70D}"), // dropdown arrow
                    ),
            )
            .child(
                div()
                    .flex_col()
                    .gap(px(2.0))
                    .children(options.iter().map(|(label, value)| {
                        let v_on_select = v_on_select.clone();
                        let val_owned = (*value).to_string();
                        let label_owned = (*label).to_string();
                        let is_current = val_owned == current_value;
                        div()
                            .px(px(8.0))
                            .py(px(2.0))
                            .rounded(px(2.0))
                            .hover(|s| s.bg(rgba(0xFFFFFF15u32)))
                            .cursor_pointer()
                            .on_mouse_down(MouseButton::Left, move |_, _, _| {
                                (v_on_select)(&val_owned);
                            })
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(if is_current {
                                        rgb(UiConstants::ACCENT_COLOR_RGB)
                                    } else {
                                        rgb(UiConstants::TEXT_SECONDARY_RGB)
                                    })
                                    .child(label_owned),
                            )
                    })),
            )
    }
}

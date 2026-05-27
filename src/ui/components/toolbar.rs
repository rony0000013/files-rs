use crate::config::config::ViewMode;
use crate::ui::constants::UiConstants;
use crate::ui::views::main_view::ToolbarCustomizationState;
use gpui::*;

pub struct ToolbarView;

impl ToolbarView {
    pub fn build(
        view_mode: &ViewMode,
        toolbar_customization: &ToolbarCustomizationState,
        has_selection: bool,
        single_selection: bool,
        has_clipboard: bool,
        filter_text: String,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let view_new_folder = view.clone();
        let view_new_file = view.clone();
        let view_cut = view.clone();
        let view_copy = view.clone();
        let view_paste = view.clone();
        let view_rename = view.clone();
        let view_share = view.clone();
        let view_delete = view.clone();

        let view_filter = view.clone();
        let view_select_all = view.clone();
        let view_invert = view.clone();
        let view_clear_sel = view.clone();

        let view_info = view.clone();

        let mut toolbar = div()
            .flex()
            .items_center()
            .h(Pixels::from(40.0))
            .w_full()
            .px(Pixels::from(8.0))
            .bg(rgb(UiConstants::BG_TOOLBAR_RGB))
            .border_b(Pixels::from(1.0))
            .border_color(rgba(UiConstants::DIVIDER_RGBA))
            .on_mouse_move({
                let v = view.clone();
                move |_ev, _window, cx| {
                    v.update(cx, |this, cx| {
                        this.stop_hover_timer(cx);
                    });
                }
            });

        // LEFT-ALIGNED GROUP: Context Commands
        let mut left_group = div()
            .flex()
            .items_center()
            .gap(px(2.0));

        // New Folder button
        if toolbar_customization.is_visible("new_folder") {
            left_group = left_group.child(ToolbarButton::build(
                "\u{E8F4}",
                "New Folder (Ctrl+Shift+N)",
                true,
                move |_event, _window, cx| {
                    view_new_folder.update(cx, |this, cx| {
                        this.new_folder(cx);
                    });
                },
            ));
        }

        // New File button
        if toolbar_customization.is_visible("new_file") {
            left_group = left_group.child(ToolbarButton::build(
                "\u{E7C3}",
                "New File",
                true,
                move |_event, _window, cx| {
                    view_new_file.update(cx, |this, cx| {
                        this.new_file(cx);
                    });
                },
            ));
        }

        if toolbar_customization.is_visible("new_folder")
            || toolbar_customization.is_visible("new_file")
        {
            left_group = left_group.child(ToolbarDivider::build());
        }

        // Cut button
        if toolbar_customization.is_visible("cut") {
            left_group = left_group.child(ToolbarButton::build(
                "\u{E8C6}",
                "Cut (Ctrl+X)",
                has_selection,
                move |_event, _window, cx| {
                    view_cut.update(cx, |this, cx| {
                        this.cut_selected(cx);
                    });
                },
            ));
        }

        // Copy button
        if toolbar_customization.is_visible("copy") {
            left_group = left_group.child(ToolbarButton::build(
                "\u{E8C8}",
                "Copy (Ctrl+C)",
                has_selection,
                move |_event, _window, cx| {
                    view_copy.update(cx, |this, cx| {
                        this.copy_selected(cx);
                    });
                },
            ));
        }

        // Paste button
        if toolbar_customization.is_visible("paste") {
            left_group = left_group.child(ToolbarButton::build(
                "\u{E77F}",
                "Paste (Ctrl+V)",
                has_clipboard,
                move |_event, _window, cx| {
                    view_paste.update(cx, |this, cx| {
                        this.paste(cx);
                    });
                },
            ));
        }

        if toolbar_customization.is_visible("cut")
            || toolbar_customization.is_visible("copy")
            || toolbar_customization.is_visible("paste")
        {
            left_group = left_group.child(ToolbarDivider::build());
        }

        // Rename button
        if toolbar_customization.is_visible("rename") {
            left_group = left_group.child(ToolbarButton::build(
                "\u{E8AC}",
                "Rename (F2)",
                single_selection,
                move |_event, _window, cx| {
                    view_rename.update(cx, |this, cx| {
                        this.rename_selected(cx);
                    });
                },
            ));
        }

        // Share button
        left_group = left_group.child(ToolbarButton::build(
            "\u{E72D}",
            "Share",
            has_selection,
            move |_event, _window, cx| {
                view_share.update(cx, |this, cx| {
                    this.share_selected(cx);
                });
            },
        ));

        // Delete button
        if toolbar_customization.is_visible("delete") {
            left_group = left_group.child(ToolbarButton::build(
                "\u{E74D}",
                "Delete (Del)",
                has_selection,
                move |_event, _window, cx| {
                    view_delete.update(cx, |this, cx| {
                        this.delete_selected(cx);
                    });
                },
            ));
        }

        toolbar = toolbar.child(left_group);

        // Spacer to separate left and right groups
        toolbar = toolbar.child(div().flex_grow());

        // RIGHT-ALIGNED GROUP: Base/View Commands
        let mut right_group = div()
            .flex()
            .items_center()
            .gap(px(2.0));

        // Filter button
        let filter_active = !filter_text.is_empty();
        right_group = right_group.child(ToolbarButton::build(
            "\u{E71C}",
            if filter_active { "Clear Filter" } else { "Filter" },
            true,
            move |_event, _window, cx| {
                let active = filter_active;
                view_filter.update(cx, |this, cx| {
                    if active {
                        this.set_filter_text(String::new(), cx);
                    } else {
                        // Open a dynamic dialog explaining how to filter
                        this.state.active_dialog = Some(crate::ui::components::dialogs::DialogType::Dynamic {
                            title: "Filter Items".to_string(),
                            message: "To filter items, please press Esc and use the search bar in the Omnibar, or enter filter criteria in the details view.".to_string(),
                        });
                        cx.notify();
                    }
                });
            },
        ));

        right_group = right_group.child(ToolbarDivider::build());

        // Select All button
        right_group = right_group.child(ToolbarButton::build(
            "\u{E8B3}",
            "Select All (Ctrl+A)",
            true,
            move |_event, _window, cx| {
                view_select_all.update(cx, |this, cx| {
                    this.select_all(cx);
                });
            },
        ));

        // Invert Selection button
        right_group = right_group.child(ToolbarButton::build(
            "\u{E71D}",
            "Invert Selection",
            true,
            move |_event, _window, cx| {
                view_invert.update(cx, |this, cx| {
                    this.invert_selection(cx);
                });
            },
        ));

        // Clear Selection button
        right_group = right_group.child(ToolbarButton::build(
            "\u{E894}",
            "Clear Selection",
            has_selection,
            move |_event, _window, cx| {
                view_clear_sel.update(cx, |this, cx| {
                    this.clear_selection(cx);
                });
            },
        ));

        right_group = right_group.child(ToolbarDivider::build());

        // View Mode Switchers
        right_group = right_group.child(ViewModeButton::build(
            "\u{E8C0}",
            "Details View",
            ViewMode::Details,
            view_mode,
            view.clone(),
        ));
        right_group = right_group.child(ViewModeButton::build(
            "\u{E8B9}",
            "List View",
            ViewMode::List,
            view_mode,
            view.clone(),
        ));
        right_group = right_group.child(ViewModeButton::build(
            "\u{E80A}",
            "Grid View",
            ViewMode::Grid,
            view_mode,
            view.clone(),
        ));
        right_group = right_group.child(ViewModeButton::build(
            "\u{E8F1}",
            "Columns View",
            ViewMode::Columns,
            view_mode,
            view.clone(),
        ));

        right_group = right_group.child(ToolbarDivider::build());

        // Preview Pane button
        if toolbar_customization.is_visible("preview_pane") {
            right_group = right_group.child(ToolbarButton::build(
                "\u{E8A0}",
                "Preview Pane (Ctrl+P)",
                true,
                {
                    let v = view.clone();
                    move |_event, _window, cx| {
                        v.update(cx, |this, cx| {
                            this.toggle_preview_pane(cx);
                        });
                    }
                },
            ));
        }

        // Info/Details Pane button
        if toolbar_customization.is_visible("info_pane") {
            right_group = right_group.child(ToolbarButton::build(
                "\u{E946}",
                "Info Pane",
                true,
                move |_event, _window, cx| {
                    view_info.update(cx, |this, cx| {
                        this.toggle_info_pane(cx);
                    });
                },
            ));
        }

        toolbar = toolbar.child(right_group);

        toolbar
    }
}

struct ToolbarButton;

impl ToolbarButton {
    fn build(
        glyph: &str,
        _tooltip: &str,
        is_enabled: bool,
        on_click: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    ) -> impl IntoElement {
        let mut btn = div()
            .flex()
            .items_center()
            .justify_center()
            .min_w(Pixels::from(32.0))
            .h(Pixels::from(32.0))
            .px(Pixels::from(6.0))
            .rounded(Pixels::from(6.0));

        if is_enabled {
            btn = btn
                .cursor_pointer()
                .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
                .on_mouse_down(MouseButton::Left, on_click);
        }

        btn.child(
            div()
                .text_size(AbsoluteLength::Pixels(Pixels::from(16.0)))
                .font_family(UiConstants::FONT_ICONS)
                .text_color(if is_enabled {
                    rgba(UiConstants::TEXT_PRIMARY_RGBA)
                } else {
                    rgba(UiConstants::TEXT_MUTED_RGBA)
                })
                .child(glyph.to_string()),
        )
    }
}

struct ToolbarDivider;

impl ToolbarDivider {
    fn build() -> impl IntoElement {
        div()
            .w(Pixels::from(1.0))
            .h(Pixels::from(20.0))
            .mx(Pixels::from(6.0))
            .bg(rgba(UiConstants::DIVIDER_RGBA))
    }
}

struct ViewModeButton;

impl ViewModeButton {
    fn build(
        glyph: &str,
        _label: &str,
        mode: ViewMode,
        current: &ViewMode,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let is_active = std::mem::discriminant(current) == std::mem::discriminant(&mode);
        let glyph = glyph.to_string();
        let mut btn = div()
            .flex()
            .items_center()
            .justify_center()
            .w(Pixels::from(32.0))
            .h(Pixels::from(28.0))
            .cursor_pointer()
            .rounded(Pixels::from(4.0));

        if is_active {
            btn = btn.bg(rgba(UiConstants::BG_ITEM_SELECTED_RGBA));
        } else {
            btn = btn.hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)));
        }

        btn.on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
            let mode = mode.clone();
            view.update(cx, |this, cx| {
                this.set_view_mode(mode, cx);
            });
        })
        .child(
            div()
                .text_size(AbsoluteLength::Pixels(Pixels::from(16.0)))
                .font_family(UiConstants::FONT_ICONS)
                .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                .child(glyph),
        )
    }
}

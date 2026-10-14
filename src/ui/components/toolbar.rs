use crate::config::config::ViewMode;
use crate::ui::constants::UiConstants;
use crate::ui::views::main_view::ToolbarCustomizationState;
use gpui::*;

pub struct ToolbarView;

impl ToolbarView {
    pub fn build(
        _view_mode: &ViewMode,
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
        let mut left_group = div().flex().items_center().gap(px(2.0));

        // Files-style labeled [ ⊕ New ⌄ ] split pill: label creates a folder,
        // chevron creates a file. Reuses the existing AddItem dialogs.
        if toolbar_customization.is_visible("new_folder")
            || toolbar_customization.is_visible("new_file")
        {
            let show_folder = toolbar_customization.is_visible("new_folder");
            let show_file = toolbar_customization.is_visible("new_file");
            let v_label = view_new_folder.clone();
            let v_chev = view_new_file.clone();
            let pill = div()
                .flex()
                .items_center()
                .h(px(32.0))
                .rounded(px(6.0))
                .bg(rgba(0xFFFFFF0Du32))
                .border(px(1.0))
                .border_color(rgba(0xFFFFFF18u32))
                .hover(|s| s.bg(rgba(0xFFFFFF18u32)))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.0))
                        .h_full()
                        .px(px(10.0))
                        .cursor_pointer()
                        .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
                            v_label.update(cx, |this, cx| {
                                if show_folder {
                                    this.new_folder(cx);
                                } else {
                                    this.new_file(cx);
                                }
                            });
                        })
                        .child(
                            div()
                                .text_size(px(14.0))
                                .font_family(UiConstants::FONT_ICONS)
                                .text_color(rgb(UiConstants::ACCENT_COLOR_RGB))
                                .child("\u{E710}"),
                        )
                        .child(
                            div()
                                .text_size(px(13.0))
                                .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                                .child("New"),
                        ),
                )
                .child(
                    div()
                        .w(px(1.0))
                        .h(px(18.0))
                        .bg(rgba(0xFFFFFF18u32)),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .h_full()
                        .w(px(24.0))
                        .cursor_pointer()
                        .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
                            v_chev.update(cx, |this, cx| {
                                if show_file {
                                    this.new_file(cx);
                                } else {
                                    this.new_folder(cx);
                                }
                            });
                        })
                        .child(
                            div()
                                .text_size(px(10.0))
                                .font_family(UiConstants::FONT_ICONS)
                                .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                .child("\u{E70D}"),
                        ),
                );
            left_group = left_group.child(pill);
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
        let mut right_group = div().flex().items_center().gap(px(2.0));

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

        // Sort + View buttons (compact replacements for the 4 separate
        // view-mode icons). Preview/Info pane buttons follow below unchanged.
        right_group = right_group.child(ToolbarButton::build(
            "\u{E8CB}",
            "Sort: cycle Name / Date / Size / Type",
            true,
            {
                let v = view.clone();
                move |_event, _window, cx| {
                    v.update(cx, |this, cx| {
                        this.cycle_sort_column(cx);
                    });
                }
            },
        ));
        right_group = right_group.child(ToolbarButton::build(
            "\u{E80A}",
            "View: cycle Details / List / Grid / Columns",
            true,
            {
                let v = view.clone();
                move |_event, _window, cx| {
                    v.update(cx, |this, cx| {
                        this.cycle_view_mode(cx);
                    });
                }
            },
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

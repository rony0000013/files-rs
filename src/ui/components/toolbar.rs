use crate::config::config::ViewMode;
use crate::ui::constants::UiConstants;
use crate::ui::views::main_view::ToolbarCustomizationState;
use gpui::*;

pub struct ToolbarView;

impl ToolbarView {
    pub fn build(
        view_mode: &ViewMode,
        toolbar_customization: &ToolbarCustomizationState,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let view_clone = view.clone();
        let vc = view.clone();
        let vc2 = view.clone();
        let vc3 = view.clone();
        let vc4 = view.clone();
        let vc5 = view.clone();
        let vc6 = view.clone();
        let vp = view.clone();
        let vi = view.clone();
        let vcomp = view.clone();

        let mut toolbar = div()
            .flex()
            .items_center()
            .h(Pixels::from(40.0))
            .w_full()
            .px(Pixels::from(8.0))
            .bg(rgb(UiConstants::BG_TOOLBAR_RGB))
            .border_b(Pixels::from(1.0))
            .border_color(rgba(UiConstants::DIVIDER_RGBA));

        // New Folder button
        if toolbar_customization.is_visible("new_folder") {
            toolbar = toolbar.child(ToolbarButton::build(
                "\u{E8F4}",
                "New Folder (Ctrl+Shift+N)",
                move |_event, _window, cx| {
                    view_clone.update(cx, |this, cx| {
                        this.new_folder(cx);
                    });
                },
            ));
        }

        // New File button
        if toolbar_customization.is_visible("new_file") {
            toolbar = toolbar.child(ToolbarButton::build(
                "\u{E7C3}",
                "New File",
                move |_event, _window, cx| {
                    vc.update(cx, |this, cx| {
                        this.new_file(cx);
                    });
                },
            ));
        }

        if toolbar_customization.is_visible("new_folder")
            || toolbar_customization.is_visible("new_file")
        {
            toolbar = toolbar.child(ToolbarDivider::build());
        }

        // Cut button
        if toolbar_customization.is_visible("cut") {
            toolbar = toolbar.child(ToolbarButton::build(
                "\u{E8C6}",
                "Cut",
                move |_event, _window, cx| {
                    vc2.update(cx, |this, cx| {
                        this.cut_selected(cx);
                    });
                },
            ));
        }

        // Copy button
        if toolbar_customization.is_visible("copy") {
            toolbar = toolbar.child(ToolbarButton::build(
                "\u{E8C8}",
                "Copy",
                move |_event, _window, cx| {
                    vc3.update(cx, |this, cx| {
                        this.copy_selected(cx);
                    });
                },
            ));
        }

        // Paste button
        if toolbar_customization.is_visible("paste") {
            toolbar = toolbar.child({
                ToolbarButton::build("\u{E77F}", "Paste", move |_event, _window, cx| {
                    vc4.update(cx, |this, cx| {
                        this.paste(cx);
                    });
                })
            });
        }

        if toolbar_customization.is_visible("cut")
            || toolbar_customization.is_visible("copy")
            || toolbar_customization.is_visible("paste")
        {
            toolbar = toolbar.child(ToolbarDivider::build());
        }

        // Preview Pane button
        if toolbar_customization.is_visible("preview_pane") {
            toolbar = toolbar.child({
                ToolbarButton::build(
                    "\u{E8A0}",
                    "Preview Pane (Ctrl+P)",
                    move |_event, _window, cx| {
                        vc5.update(cx, |this, cx| {
                            this.toggle_preview_pane(cx);
                        });
                    },
                )
            });
        }

        // Info Pane button
        if toolbar_customization.is_visible("info_pane") {
            toolbar = toolbar.child({
                ToolbarButton::build("\u{E946}", "Info Pane", move |_event, _window, cx| {
                    vc6.update(cx, |this, cx| {
                        this.toggle_info_pane(cx);
                    });
                })
            });
        }

        toolbar = toolbar.child(div().flex_grow());

        // Search bar
        if toolbar_customization.is_visible("search") {
            toolbar = toolbar.child(
                div()
                    .w(Pixels::from(240.0))
                    .h(Pixels::from(32.0))
                    .bg(rgba(UiConstants::BG_BADGE_RGBA))
                    .rounded(Pixels::from(6.0))
                    .border(Pixels::from(1.0))
                    .border_color(rgba(UiConstants::STROKE_RGBA))
                    .flex()
                    .items_center()
                    .px(Pixels::from(10.0))
                    .child(
                        div()
                            .text_size(Pixels::from(14.0))
                            .font_family(UiConstants::FONT_ICONS)
                            .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                            .child("\u{E721}"),
                    )
                    .child(
                        div()
                            .flex_grow()
                            .ml(Pixels::from(8.0))
                            .text_size(Pixels::from(13.0))
                            .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                            .child("Search..."),
                    ),
            );
        }

        toolbar
    }
}

struct ToolbarButton;

impl ToolbarButton {
    fn build(
        glyph: &str,
        _tooltip: &str,
        on_click: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    ) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .justify_center()
            .min_w(Pixels::from(32.0))
            .h(Pixels::from(32.0))
            .px(Pixels::from(6.0))
            .cursor_pointer()
            .rounded(Pixels::from(6.0))
            .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
            .on_mouse_down(MouseButton::Left, on_click)
            .child(
                div()
                    .text_size(AbsoluteLength::Pixels(Pixels::from(16.0)))
                    .font_family(UiConstants::FONT_ICONS)
                    .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
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

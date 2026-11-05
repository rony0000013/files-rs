//! Top Ribbon / Command Bar above Main View (`Files-community` `Toolbar.xaml` & `ToolbarDefaultsTemplate` parity).
//!
//! Implements the Unibody `InnerNavigationToolbar` card:
//! - Rounded card (`CornerRadius=8`, `1px` `CARD_STROKE_RGBA` border)
//! - Left `ContextCommandBar`:
//!   - `+ New ⌄` Split Button & Flyout (`Folder`, `Text Document`, `Shortcut`, `ZIP Archive`)
//!   - `Cut`, `Copy`, `Paste`, separator, `Rename`, `Share`, `Delete`, `Properties`
//!   - Contextual `Recycle Bin` commands (`Empty Recycle Bin`, `Restore all`, `Restore`)
//!   - Contextual `Archive` commands (`Extract`, `Compress`) when an archive is selected
//!   - Contextual `Image` commands (`Rotate Left`, `Rotate Right`, `Set Wallpaper`) when an image is selected
//!   - `Terminal` & `IDE` quick buttons
//! - Right `BaseCommandBar`:
//!   - `Filter` toggle button
//!   - `Selection` flyout (`Select All`, `Invert Selection`, `Clear Selection`, `Show Checkboxes`)
//!   - `Sort & Group` flyout (`Sort by Name/DateModified/DateCreated/Size/Type`, `Ascending/Descending`, `Folders on top`, `Group by None/Name/Date/Size/Type/Tag`)
//!   - `Layout & View` flyout (`Details`, `List`, `Cards/Grid`, `Columns`, Icon Size presets, `Show Hidden Items`, `Show File Extensions`, `Dual Pane`)
//!   - `Details / Preview` InfoPane toggle button (`PanelRight`)

use crate::app_state::AppState;
use crate::config::config::{GroupColumn, SortBy, ViewMode};
use crate::ui::components::clipboard_manager::ClipboardState;
use crate::ui::constants::UiConstants;
use crate::ui::views::main_view::{SidePaneMode, ToolbarCustomizationState, ViewTarget};
use gpui::*;
use std::sync::LazyLock;

pub struct ToolbarView;

impl ToolbarView {
    /// Legacy compatibility builder.
    pub fn build(
        view_mode: &ViewMode,
        toolbar_customization: &ToolbarCustomizationState,
        has_selection: bool,
        single_selection: bool,
        has_clipboard: bool,
        filter_text: String,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        static FALLBACK_TARGET: LazyLock<ViewTarget> =
            LazyLock::new(|| ViewTarget::Directory(crate::core::path_util::FilesPath::new("")));
        Self::build_card(
            view_mode,
            toolbar_customization,
            &FALLBACK_TARGET,
            has_selection,
            single_selection,
            has_clipboard,
            filter_text,
            false,
            true,
            &SortBy::Name,
            true,
            true,
            &GroupColumn::None,
            0.5,
            &SidePaneMode::None,
            false,
            false,
            0,
            view,
        )
    }

    /// Full `Files-community` Unibody Command Bar builder driven by `AppState`.
    pub fn build_from_state(
        state: &AppState,
        show_labels: bool,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let mv = &state.main_view;
        let has_selection = !mv.selected_indices.is_empty();
        let single_selection = mv.selected_indices.len() == 1;
        let has_clipboard = !matches!(state.clipboard, ClipboardState::Empty);

        // Detect contextual selection type (Archive or Image)
        let mut selected_is_archive = false;
        let mut selected_is_image = false;
        if let Some(&first_idx) = mv.selected_indices.iter().next() {
            if let Some(item) = mv.items.get(first_idx) {
                let ext = item
                    .path
                    .as_path()
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_ascii_lowercase();
                selected_is_archive =
                    matches!(ext.as_str(), "zip" | "7z" | "tar" | "gz" | "tgz" | "rar" | "bz2" | "xz");
                selected_is_image =
                    matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "ico" | "tiff");
            }
        }

        Self::build_card(
            &mv.view_mode,
            &mv.toolbar_customization,
            &mv.target,
            has_selection,
            single_selection,
            has_clipboard,
            mv.filter_text.clone(),
            mv.show_filter_bar,
            show_labels,
            &mv.sort.column,
            mv.sort.ascending,
            mv.sort.folders_first,
            &mv.group_by,
            mv.zoom_level,
            &mv.side_pane,
            selected_is_archive,
            selected_is_image,
            mv.recycle_bin_items.len(),
            view,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn build_card(
        view_mode: &ViewMode,
        toolbar_customization: &ToolbarCustomizationState,
        target: &ViewTarget,
        has_selection: bool,
        single_selection: bool,
        has_clipboard: bool,
        filter_text: String,
        show_filter_bar: bool,
        show_labels: bool,
        sort_col: &SortBy,
        sort_asc: bool,
        _folder_on_top: bool,
        group_by: &GroupColumn,
        _zoom_level: f32,
        side_pane: &SidePaneMode,
        selected_is_archive: bool,
        selected_is_image: bool,
        recycle_bin_count: usize,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let is_recycle_bin = matches!(target, ViewTarget::RecycleBin);
        let is_home = matches!(target, ViewTarget::Home);
        let is_settings = matches!(target, ViewTarget::Settings);
        let can_modify_dir = !is_recycle_bin && !is_home && !is_settings;
        let active_flyout = toolbar_customization.active_flyout.as_deref();

        let mut toolbar = div()
            .id("unibody-command-bar")
            .flex()
            .flex_row()
            .items_center()
            .h(px(40.0))
            .w_full()
            .px(px(8.0))
            .rounded(px(8.0))
            .bg(rgba(0xFFFFFF08u32))
            .border(px(1.0))
            .border_color(rgba(UiConstants::CARD_STROKE_RGBA))
            .on_mouse_move({
                let v = view.clone();
                move |_ev, _window, cx| {
                    v.update(cx, |this, cx| {
                        this.stop_hover_timer(cx);
                    });
                }
            });

        // ═════════════════════════════════════════════════════════════════════
        // LEFT-ALIGNED GROUP: ContextCommandBar (`Toolbar.xaml` parity)
        // ═════════════════════════════════════════════════════════════════════
        let mut left_group = div().flex().flex_row().items_center().gap(px(3.0));

        // 1. `+ New ⌄` Split Button
        if toolbar_customization.is_visible("new_folder")
            || toolbar_customization.is_visible("new_file")
        {
            let new_flyout_open = active_flyout == Some("new");
            let v_new_primary = view.clone();
            let v_new_flyout = view.clone();

            let split_pill = div()
                .flex()
                .flex_row()
                .items_center()
                .h(px(30.0))
                .rounded(px(6.0))
                .bg(if new_flyout_open {
                    rgba(0x60CDFF24u32)
                } else {
                    rgba(0xFFFFFF0Du32)
                })
                .border(px(1.0))
                .border_color(if new_flyout_open {
                    rgba(0x60CDFF66u32)
                } else {
                    rgba(0xFFFFFF18u32)
                })
                .opacity(if can_modify_dir { 1.0 } else { 0.45 })
                .child(
                    div()
                        .id("tb-new-btn")
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(6.0))
                        .h_full()
                        .px(px(10.0))
                        .cursor_pointer()
                        .hover(|s| {
                            if can_modify_dir {
                                s.bg(rgba(0xFFFFFF14u32))
                            } else {
                                s
                            }
                        })
                        .on_hover({
                            let v_tip = view.clone();
                            move |hovering: &bool, window: &mut Window, cx: &mut App| {
                                let pos = window.mouse_position();
                                if *hovering {
                                    v_tip.update(cx, |this, cx| {
                                        this.start_button_tooltip(
                                            "New folder".to_string(),
                                            Some("Ctrl+Shift+N".to_string()),
                                            pos,
                                            cx,
                                        );
                                    });
                                } else {
                                    v_tip.update(cx, |this, cx| {
                                        this.stop_button_tooltip(cx);
                                    });
                                }
                            }
                        })
                        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                            cx.stop_propagation();
                            if can_modify_dir {
                                v_new_primary.update(cx, |this, cx| {
                                    this.stop_button_tooltip(cx);
                                    this.close_toolbar_flyout(cx);
                                    this.new_folder(cx);
                                });
                            }
                        })
                        .child(
                            div()
                                .text_size(px(13.5))
                                .font_family(UiConstants::FONT_ICONS)
                                .text_color(rgb(0x60CDFF))
                                .child("\u{E710}"),
                        )
                        .child(
                            div()
                                .text_size(px(12.5))
                                .font_weight(FontWeight(500.0))
                                .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                                .child("New"),
                        ),
                )
                .child(div().w(px(1.0)).h(px(16.0)).bg(rgba(0xFFFFFF18u32)))
                .child(
                    div()
                        .id("tb-new-arrow")
                        .flex()
                        .items_center()
                        .justify_center()
                        .h_full()
                        .w(px(24.0))
                        .cursor_pointer()
                        .hover(|s| {
                            if can_modify_dir {
                                s.bg(rgba(0xFFFFFF14u32))
                            } else {
                                s
                            }
                        })
                        .on_hover({
                            let v_tip = view.clone();
                            move |hovering: &bool, window: &mut Window, cx: &mut App| {
                                let pos = window.mouse_position();
                                if *hovering {
                                    v_tip.update(cx, |this, cx| {
                                        this.start_button_tooltip(
                                            "Create new item".to_string(),
                                            None,
                                            pos,
                                            cx,
                                        );
                                    });
                                } else {
                                    v_tip.update(cx, |this, cx| {
                                        this.stop_button_tooltip(cx);
                                    });
                                }
                            }
                        })
                        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                            cx.stop_propagation();
                            if can_modify_dir {
                                v_new_flyout.update(cx, |this, cx| {
                                    this.stop_button_tooltip(cx);
                                    this.toggle_toolbar_flyout("new", cx);
                                });
                            }
                        })
                        .child(
                            div()
                                .text_size(px(9.5))
                                .font_family(UiConstants::FONT_ICONS)
                                .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                .child("\u{E70D}"),
                        ),
                );

            left_group = left_group.child(split_pill);
            left_group = left_group.child(ToolbarDivider::build());
        }

        // 2. Cut / Copy / Paste
        if toolbar_customization.is_visible("cut") {
            let v = view.clone();
            left_group = left_group.child(ToolbarButton::build_with_tooltip(
                "tb-cut",
                "\u{E8C6}",
                if show_labels { Some("Cut") } else { None },
                "Cut",
                Some("Ctrl+X"),
                has_selection && can_modify_dir,
                false,
                view.clone(),
                move |_, _, cx| {
                    v.update(cx, |this, cx| {
                        this.close_toolbar_flyout(cx);
                        this.cut_selected(cx);
                    });
                },
            ));
        }

        if toolbar_customization.is_visible("copy") {
            let v = view.clone();
            left_group = left_group.child(ToolbarButton::build_with_tooltip(
                "tb-copy",
                "\u{E8C8}",
                if show_labels { Some("Copy") } else { None },
                "Copy",
                Some("Ctrl+C"),
                has_selection && can_modify_dir,
                false,
                view.clone(),
                move |_, _, cx| {
                    v.update(cx, |this, cx| {
                        this.close_toolbar_flyout(cx);
                        this.copy_selected(cx);
                    });
                },
            ));
        }

        if toolbar_customization.is_visible("paste") {
            let v = view.clone();
            left_group = left_group.child(ToolbarButton::build_with_tooltip(
                "tb-paste",
                "\u{E77F}",
                if show_labels { Some("Paste") } else { None },
                "Paste",
                Some("Ctrl+V"),
                has_clipboard && can_modify_dir,
                false,
                view.clone(),
                move |_, _, cx| {
                    v.update(cx, |this, cx| {
                        this.close_toolbar_flyout(cx);
                        this.paste(cx);
                    });
                },
            ));
        }

        // 3. Rename / Share / Delete / Properties
        if toolbar_customization.is_visible("rename") {
            let v = view.clone();
            left_group = left_group.child(ToolbarButton::build_with_tooltip(
                "tb-rename",
                "\u{E8AC}",
                if show_labels { Some("Rename") } else { None },
                "Rename",
                Some("F2"),
                single_selection && can_modify_dir,
                false,
                view.clone(),
                move |_, _, cx| {
                    v.update(cx, |this, cx| {
                        this.close_toolbar_flyout(cx);
                        this.rename_selected(cx);
                    });
                },
            ));
        }

        {
            let v = view.clone();
            left_group = left_group.child(ToolbarButton::build_with_tooltip(
                "tb-share",
                "\u{E72D}",
                if show_labels { Some("Share") } else { None },
                "Share",
                None,
                has_selection && can_modify_dir,
                false,
                view.clone(),
                move |_, _, cx| {
                    v.update(cx, |this, cx| {
                        this.close_toolbar_flyout(cx);
                        this.share_selected(cx);
                    });
                },
            ));
        }

        if toolbar_customization.is_visible("delete") {
            let v = view.clone();
            left_group = left_group.child(ToolbarButton::build_with_tooltip(
                "tb-delete",
                "\u{E74D}",
                if show_labels { Some("Delete") } else { None },
                "Delete",
                Some("Del"),
                has_selection && (!is_home && !is_settings),
                false,
                view.clone(),
                move |_, _, cx| {
                    v.update(cx, |this, cx| {
                        this.close_toolbar_flyout(cx);
                        if matches!(this.state.main_view.target, ViewTarget::RecycleBin) {
                            this.delete_selected_recycle_bin(cx);
                        } else {
                            this.delete_selected(cx);
                        }
                    });
                },
            ));
        }

        {
            let v = view.clone();
            left_group = left_group.child(ToolbarButton::build_with_tooltip(
                "tb-properties",
                "\u{E946}",
                if show_labels { Some("Properties") } else { None },
                "Properties",
                Some("Alt+Enter"),
                can_modify_dir,
                false,
                view.clone(),
                move |_, _, cx| {
                    v.update(cx, |this, cx| {
                        this.close_toolbar_flyout(cx);
                        this.open_properties_for_selected(cx);
                    });
                },
            ));
        }

        // 4. Contextual Recycle Bin Commands (`Toolbar.xaml` RecycleBin group parity)
        if is_recycle_bin {
            left_group = left_group.child(ToolbarDivider::build());
            let v_empty = view.clone();
            let v_restore_all = view.clone();
            let v_restore_sel = view.clone();

            left_group = left_group.child(ToolbarButton::build(
                "\u{E74D}",
                Some("Empty Recycle Bin"),
                recycle_bin_count > 0,
                false,
                move |_, _, cx| {
                    v_empty.update(cx, |this, cx| {
                        this.empty_recycle_bin(cx);
                    });
                },
            ));
            left_group = left_group.child(ToolbarButton::build(
                "\u{E777}",
                Some("Restore all"),
                recycle_bin_count > 0,
                false,
                move |_, _, cx| {
                    v_restore_all.update(cx, |this, cx| {
                        this.restore_all_recycle_bin(cx);
                    });
                },
            ));
            if has_selection {
                left_group = left_group.child(ToolbarButton::build(
                    "\u{E777}",
                    Some("Restore"),
                    true,
                    true,
                    move |_, _, cx| {
                        v_restore_sel.update(cx, |this, cx| {
                            this.restore_selected_recycle_bin(cx);
                        });
                    },
                ));
            }
        }

        // 5. Contextual Archive Commands when an archive is selected
        if selected_is_archive && can_modify_dir {
            left_group = left_group.child(ToolbarDivider::build());
            let v_ext = view.clone();
            left_group = left_group.child(ToolbarButton::build(
                "\u{F012}",
                Some("Extract"),
                true,
                true,
                move |_, _, cx| {
                    v_ext.update(cx, |this, cx| {
                        this.close_toolbar_flyout(cx);
                        this.extract_selected_archive(cx);
                    });
                },
            ));
        }

        // 6. Contextual Image Commands when an image is selected
        if selected_is_image && can_modify_dir {
            left_group = left_group.child(ToolbarDivider::build());
            let v_rot_l = view.clone();
            let v_rot_r = view.clone();
            let v_wall = view.clone();
            left_group = left_group.child(ToolbarButton::build(
                "\u{E7AD}",
                if show_labels { Some("Rotate Left") } else { None },
                true,
                false,
                move |_, _, cx| {
                    v_rot_l.update(cx, |this, cx| {
                        this.dispatch_action(
                            crate::core::action_registry::ActionKind::RotateImageLeft,
                            cx,
                        );
                    });
                },
            ));
            left_group = left_group.child(ToolbarButton::build(
                "\u{E7AD}",
                if show_labels { Some("Rotate Right") } else { None },
                true,
                false,
                move |_, _, cx| {
                    v_rot_r.update(cx, |this, cx| {
                        this.dispatch_action(
                            crate::core::action_registry::ActionKind::RotateImageRight,
                            cx,
                        );
                    });
                },
            ));
            left_group = left_group.child(ToolbarButton::build(
                "\u{EB9F}",
                Some("Set as background"),
                true,
                false,
                move |_, _, cx| {
                    v_wall.update(cx, |this, cx| {
                        this.dispatch_action(
                            crate::core::action_registry::ActionKind::SetAsWallpaper,
                            cx,
                        );
                    });
                },
            ));
        }

        toolbar = toolbar.child(left_group);
        toolbar = toolbar.child(div().flex_grow());

        // ═════════════════════════════════════════════════════════════════════
        // RIGHT-ALIGNED GROUP: BaseCommandBar (`Toolbar.xaml` parity)
        // ═════════════════════════════════════════════════════════════════════
        let mut right_group = div().flex().flex_row().items_center().gap(px(4.0));

        // 1. Selection Options Flyout (`\u{E762} ⌄`)
        {
            let sel_open = active_flyout == Some("selection");
            let v_sel = view.clone();
            right_group = right_group.child(Self::build_dropdown_trigger_with_tooltip(
                "tb-sel",
                "\u{E762}",
                if show_labels { Some("Select") } else { None },
                "Selection options",
                Some("Ctrl+A"),
                sel_open,
                view.clone(),
                move |_, _, cx| {
                    cx.stop_propagation();
                    v_sel.update(cx, |this, cx| {
                        this.toggle_toolbar_flyout("selection", cx);
                    });
                },
            ));
        }

        // 2. Sort Flyout (`\u{E8CB} ⌄` — SortOptions in `Toolbar.xaml`)
        {
            let sort_open = active_flyout == Some("sort");
            let sort_label = match sort_col {
                SortBy::Name => "Name",
                SortBy::DateModified => "Date",
                SortBy::DateCreated => "Created",
                SortBy::Size => "Size",
                SortBy::Type => "Type",
                SortBy::Tag => "Tag",
            };
            let dir_arrow = if sort_asc { "↑" } else { "↓" };
            let badge = format!("{} {}", sort_label, dir_arrow);
            let v_sort = view.clone();
            right_group = right_group.child(Self::build_dropdown_trigger_with_tooltip(
                "tb-sort",
                "\u{E8CB}",
                if show_labels {
                    Some(badge.as_str())
                } else {
                    None
                },
                "Sort options",
                None,
                sort_open,
                view.clone(),
                move |_, _, cx| {
                    cx.stop_propagation();
                    v_sort.update(cx, |this, cx| {
                        this.toggle_toolbar_flyout("sort", cx);
                    });
                },
            ));
        }

        // 3. Group by Flyout (`\u{F168} ⌄` — dedicated GroupOptions button beside Sort)
        {
            let group_open = active_flyout == Some("group");
            let group_label = match group_by {
                GroupColumn::None => "Group",
                GroupColumn::Name => "Group: Name",
                GroupColumn::DateModified => "Group: Date",
                GroupColumn::DateCreated => "Group: Created",
                GroupColumn::Size => "Group: Size",
                GroupColumn::FileType => "Group: Type",
                GroupColumn::SyncStatus => "Group: Sync",
                GroupColumn::Tag => "Group: Tag",
            };
            let is_grouped = *group_by != GroupColumn::None;
            let v_group = view.clone();
            right_group = right_group.child(Self::build_dropdown_trigger_with_tooltip(
                "tb-group",
                if is_grouped { "\u{EA1F}" } else { "\u{F168}" },
                if show_labels {
                    Some(group_label)
                } else if is_grouped {
                    Some("Grouped")
                } else {
                    None
                },
                "Group by options",
                None,
                group_open,
                view.clone(),
                move |_, _, cx| {
                    cx.stop_propagation();
                    v_group.update(cx, |this, cx| {
                        this.toggle_toolbar_flyout("group", cx);
                    });
                },
            ));
        }

        // 4. Layout & View Mode Flyout (`LayoutOptionsButton` in `Toolbar.xaml`)
        {
            let layout_open = active_flyout == Some("layout");
            let (layout_icon, layout_label) = match view_mode {
                ViewMode::Details => ("\u{E8FD}", "Details"),
                ViewMode::List => ("\u{E8EF}", "List"),
                ViewMode::Cards => ("\u{E8A9}", "Cards"),
                ViewMode::Grid => ("\u{E80A}", "Grid"),
                ViewMode::Columns => ("\u{E8F1}", "Columns"),
            };
            let v_layout = view.clone();
            right_group = right_group.child(Self::build_dropdown_trigger_with_tooltip(
                "tb-layout",
                layout_icon,
                if show_labels {
                    Some(layout_label)
                } else {
                    None
                },
                "View layout options",
                None,
                layout_open,
                view.clone(),
                move |_, _, cx| {
                    cx.stop_propagation();
                    v_layout.update(cx, |this, cx| {
                        this.toggle_toolbar_flyout("layout", cx);
                    });
                },
            ));
        }

        right_group = right_group.child(ToolbarDivider::build());

        // 5. Details / Preview InfoPane Toggle Button (`PanelRight` icon `\u{E890}` / `\u{E946}`)
        if toolbar_customization.is_visible("preview_pane")
            || toolbar_customization.is_visible("info_pane")
        {
            let pane_open = !matches!(side_pane, SidePaneMode::None);
            let v_pane = view.clone();
            right_group = right_group.child(ToolbarButton::build_with_tooltip(
                "tb-pane",
                "\u{E890}",
                if show_labels { Some("Pane") } else { None },
                "Details pane",
                Some("Ctrl+Shift+P"),
                true,
                pane_open,
                view.clone(),
                move |_, _, cx| {
                    v_pane.update(cx, |this, cx| {
                        this.close_toolbar_flyout(cx);
                        if matches!(this.state.main_view.side_pane, SidePaneMode::None) {
                            this.state.main_view.side_pane = SidePaneMode::Details;
                            this.load_preview_for_selected_item(cx);
                            cx.notify();
                        } else {
                            this.state.main_view.side_pane = SidePaneMode::None;
                            cx.notify();
                        }
                    });
                },
            ));
        }

        toolbar = toolbar.child(right_group);
        toolbar
    }

    fn build_dropdown_trigger(
        glyph: &str,
        label: Option<&str>,
        is_open: bool,
        on_click: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    ) -> impl IntoElement {
        let mut btn = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(5.0))
            .h(px(30.0))
            .px(px(8.0))
            .rounded(px(6.0))
            .cursor_pointer()
            .on_mouse_down(MouseButton::Left, on_click);

        if is_open {
            btn = btn
                .bg(rgba(0x60CDFF24u32))
                .border(px(1.0))
                .border_color(rgba(0x60CDFF66u32));
        } else {
            btn = btn.hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)));
        }

        btn = btn.child(
            div()
                .text_size(px(14.5))
                .font_family(UiConstants::FONT_ICONS)
                .text_color(if is_open {
                    rgb(0x60CDFF)
                } else {
                    rgba(UiConstants::TEXT_PRIMARY_RGBA)
                })
                .child(glyph.to_string()),
        );

        if let Some(lbl) = label {
            btn = btn.child(
                div()
                    .text_size(px(12.0))
                    .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                    .child(lbl.to_string()),
            );
        }

        btn.child(
            div()
                .text_size(px(9.0))
                .font_family(UiConstants::FONT_ICONS)
                .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                .child("\u{E70D}"),
        )
    }

    fn build_dropdown_trigger_with_tooltip(
        id: impl Into<ElementId>,
        glyph: &str,
        label: Option<&str>,
        tip_text: &'static str,
        tip_shortcut: Option<&'static str>,
        is_open: bool,
        view: Entity<crate::app::AppWindow>,
        on_click: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    ) -> impl IntoElement {
        let v_tip = view.clone();
        let v_click = view.clone();
        let text = tip_text.to_string();
        let shortcut = tip_shortcut.map(|s| s.to_string());

        let mut btn = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(5.0))
            .h(px(30.0))
            .px(px(8.0))
            .rounded(px(6.0))
            .cursor_pointer();

        if is_open {
            btn = btn
                .bg(rgba(0x60CDFF24u32))
                .border(px(1.0))
                .border_color(rgba(0x60CDFF66u32));
        } else {
            btn = btn.hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)));
        }

        let mut final_btn = btn.id(id)
            .on_hover(move |hovering: &bool, window: &mut Window, cx: &mut App| {
                let pos = window.mouse_position();
                let t = text.clone();
                let sc = shortcut.clone();
                if *hovering {
                    v_tip.update(cx, |this, cx| {
                        this.start_button_tooltip(t, sc, pos, cx);
                    });
                } else {
                    v_tip.update(cx, |this, cx| {
                        this.stop_button_tooltip(cx);
                    });
                }
            })
            .on_mouse_down(MouseButton::Left, move |e, w, cx| {
                v_click.update(cx, |this, cx| {
                    this.stop_button_tooltip(cx);
                });
                on_click(e, w, cx);
            })
            .child(
                div()
                    .text_size(px(14.5))
                    .font_family(UiConstants::FONT_ICONS)
                    .text_color(if is_open {
                        rgb(0x60CDFF)
                    } else {
                        rgba(UiConstants::TEXT_PRIMARY_RGBA)
                    })
                    .child(glyph.to_string()),
            );

        if let Some(lbl) = label {
            final_btn = final_btn.child(
                div()
                    .text_size(px(12.0))
                    .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                    .child(lbl.to_string()),
            );
        }

        final_btn.child(
            div()
                .text_size(px(9.0))
                .font_family(UiConstants::FONT_ICONS)
                .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                .child("\u{E70D}"),
        )
    }

    /// Render the floating WinUI 3 Flyout Menu when a Top Ribbon dropdown (`"new"`, `"selection"`, `"sort"`, `"layout"`) is open.
    pub fn build_flyout_overlay(
        state: &AppState,
        view: Entity<crate::app::AppWindow>,
        mica_geo: Option<crate::wallpaper_mica::MicaGeo>,
        win_w: f32,
    ) -> Option<AnyElement> {
        let flyout_id = state.main_view.toolbar_customization.active_flyout.as_deref()?;
        let sidebar_w = if state.sidebar_visible {
            state.sidebar.width
        } else {
            0.0
        };

        let menu_card: AnyElement = match flyout_id {
            "new" => Self::build_new_item_flyout(sidebar_w, view.clone(), mica_geo.as_ref()).into_any_element(),
            "selection" => Self::build_selection_flyout(state, view.clone(), mica_geo.as_ref(), win_w).into_any_element(),
            "sort" => Self::build_sort_flyout(state, view.clone(), mica_geo.as_ref(), win_w).into_any_element(),
            "group" => Self::build_group_flyout(state, view.clone(), mica_geo.as_ref(), win_w).into_any_element(),
            "layout" => Self::build_layout_flyout(state, view.clone(), mica_geo.as_ref(), win_w).into_any_element(),
            _ => return None,
        };

        let v_backdrop = view;
        Some(
            div()
                .id("toolbar-flyout-backdrop")
                .absolute()
                .top(px(0.0))
                .left(px(0.0))
                .right(px(0.0))
                .bottom(px(0.0))
                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                    v_backdrop.update(cx, |this, cx| {
                        this.close_toolbar_flyout(cx);
                    });
                })
                .child(menu_card)
                .into_any_element(),
        )
    }

    fn build_new_item_flyout(
        sidebar_w: f32,
        view: Entity<crate::app::AppWindow>,
        mica_geo: Option<&crate::wallpaper_mica::MicaGeo>,
    ) -> impl IntoElement {
        let v_folder = view.clone();
        let v_file = view.clone();
        let v_shortcut = view.clone();
        let v_zip = view.clone();

        let abs_x = sidebar_w + 12.0;
        let abs_y = 124.0;
        let card = crate::ui::components::dialogs_context::flyout_card_shell(abs_x, abs_y, 230.0, 248.0, mica_geo);
        card.absolute()
            .top(px(abs_y))
            .left(px(abs_x))
            .p(px(6.0))
            .flex()
            .flex_col()
            .gap(px(2.0))
            .child(Self::flyout_menu_item(
                "\u{E8B7}",
                "Folder",
                Some("Ctrl+Shift+N"),
                false,
                move |_, _, cx| {
                    v_folder.update(cx, |this, cx| {
                        this.close_toolbar_flyout(cx);
                        this.new_folder(cx);
                    });
                },
            ))
            .child(Self::flyout_menu_item(
                "\u{E8A5}",
                "Text Document",
                None,
                false,
                move |_, _, cx| {
                    v_file.update(cx, |this, cx| {
                        this.close_toolbar_flyout(cx);
                        this.new_file(cx);
                    });
                },
            ))
            .child(div().h(px(1.0)).my(px(3.0)).bg(rgba(0xFFFFFF14u32)))
            .child(Self::flyout_menu_item(
                "\u{E71B}",
                "Shortcut",
                None,
                false,
                move |_, _, cx| {
                    v_shortcut.update(cx, |this, cx| {
                        this.close_toolbar_flyout(cx);
                        this.paste_as_shortcut(cx);
                    });
                },
            ))
            .child(Self::flyout_menu_item(
                "\u{F012}",
                "ZIP Archive",
                None,
                false,
                move |_, _, cx| {
                    v_zip.update(cx, |this, cx| {
                        this.close_toolbar_flyout(cx);
                        this.compress_selected_items(cx);
                    });
                },
            ))
    }

    fn build_selection_flyout(
        state: &AppState,
        view: Entity<crate::app::AppWindow>,
        mica_geo: Option<&crate::wallpaper_mica::MicaGeo>,
        win_w: f32,
    ) -> impl IntoElement {
        let v_all = view.clone();
        let v_inv = view.clone();
        let v_clr = view.clone();
        let v_chk = view.clone();
        let has_sel = !state.main_view.selected_indices.is_empty();
        let is_chk_active = state.main_view.selection_mode == crate::ui::views::main_view::SelectionMode::Checkbox;

        let abs_x = (win_w - 240.0 - 240.0).max(10.0);
        let abs_y = 124.0;
        let card = crate::ui::components::dialogs_context::flyout_card_shell(abs_x, abs_y, 240.0, 152.0, mica_geo);
        card.absolute()
            .top(px(abs_y))
            .right(px(240.0))
            .p(px(6.0))
            .flex()
            .flex_col()
            .gap(px(2.0))
            .child(Self::flyout_menu_item(
                "\u{E8B3}",
                "Select All",
                Some("Ctrl+A"),
                false,
                move |_, _, cx| {
                    v_all.update(cx, |this, cx| {
                        this.close_toolbar_flyout(cx);
                        this.select_all(cx);
                    });
                },
            ))
            .child(Self::flyout_menu_item(
                "\u{E71D}",
                "Invert Selection",
                None,
                false,
                move |_, _, cx| {
                    v_inv.update(cx, |this, cx| {
                        this.close_toolbar_flyout(cx);
                        this.invert_selection(cx);
                    });
                },
            ))
            .child(Self::flyout_menu_item(
                "\u{E894}",
                "Clear Selection",
                Some("Esc"),
                false,
                move |_, _, cx| {
                    if has_sel {
                        v_clr.update(cx, |this, cx| {
                            this.close_toolbar_flyout(cx);
                            this.clear_selection(cx);
                        });
                    }
                },
            ))
            .child(div().h(px(1.0)).my(px(3.0)).bg(rgba(0xFFFFFF14u32)))
            .child(Self::flyout_menu_item(
                if is_chk_active { "\u{E73E}" } else { "\u{E73A}" },
                "Toggle Selection Checkboxes",
                None,
                is_chk_active,
                move |_, _, cx| {
                    v_chk.update(cx, |this, cx| {
                        this.close_toolbar_flyout(cx);
                        this.toggle_selection_checkboxes(cx);
                    });
                },
            ))
    }

    fn build_sort_flyout(
        state: &AppState,
        view: Entity<crate::app::AppWindow>,
        mica_geo: Option<&crate::wallpaper_mica::MicaGeo>,
        win_w: f32,
    ) -> impl IntoElement {
        let cur_sort = &state.main_view.sort.column;
        let cur_asc = state.main_view.sort.ascending;
        let cur_folders_top = state.main_view.sort.folders_first;

        let sort_options = [
            (SortBy::Name, "Name"),
            (SortBy::DateModified, "Date modified"),
            (SortBy::DateCreated, "Date created"),
            (SortBy::Size, "Size"),
            (SortBy::Type, "Type"),
            (SortBy::Tag, "File tag"),
        ];

        let abs_x = (win_w - 150.0 - 236.0).max(10.0);
        let abs_y = 124.0;
        let card = crate::ui::components::dialogs_context::flyout_card_shell(abs_x, abs_y, 236.0, 360.0, mica_geo);
        let mut col = card
            .absolute()
            .top(px(abs_y))
            .right(px(150.0))
            .p(px(6.0))
            .flex()
            .flex_col()
            .gap(px(1.5))
            .child(Self::flyout_section_header("Sort by"));

        for (s_col, label) in sort_options {
            let active = cur_sort == &s_col;
            let v = view.clone();
            col = col.child(Self::flyout_menu_item(
                if active { "\u{E915}" } else { "" },
                label,
                None,
                active,
                move |_, _, cx| {
                    let c = s_col.clone();
                    v.update(cx, |this, cx| {
                        this.close_toolbar_flyout(cx);
                        this.set_sort_column(c, cx);
                    });
                },
            ));
        }

        let v_asc = view.clone();
        let v_desc = view.clone();
        let v_folders = view.clone();
        col = col
            .child(div().h(px(1.0)).my(px(3.0)).bg(rgba(0xFFFFFF14u32)))
            .child(Self::flyout_menu_item(
                if cur_asc { "\u{E915}" } else { "" },
                "Ascending",
                None,
                cur_asc,
                move |_, _, cx| {
                    v_asc.update(cx, |this, cx| {
                        this.close_toolbar_flyout(cx);
                        this.set_sort_ascending(true, cx);
                    });
                },
            ))
            .child(Self::flyout_menu_item(
                if !cur_asc { "\u{E915}" } else { "" },
                "Descending",
                None,
                !cur_asc,
                move |_, _, cx| {
                    v_desc.update(cx, |this, cx| {
                        this.close_toolbar_flyout(cx);
                        this.set_sort_ascending(false, cx);
                    });
                },
            ))
            .child(Self::flyout_menu_item(
                if cur_folders_top { "\u{E73E}" } else { "" },
                "Sort directories first",
                None,
                cur_folders_top,
                move |_, _, cx| {
                    v_folders.update(cx, |this, cx| {
                        this.close_toolbar_flyout(cx);
                        this.toggle_setting("folder_on_top", cx);
                    });
                },
            ));

        col
    }

    fn build_group_flyout(
        state: &AppState,
        view: Entity<crate::app::AppWindow>,
        mica_geo: Option<&crate::wallpaper_mica::MicaGeo>,
        win_w: f32,
    ) -> impl IntoElement {
        let cur_group = &state.main_view.group_by;

        let group_options = [
            (GroupColumn::None, "None"),
            (GroupColumn::Name, "Name"),
            (GroupColumn::DateModified, "Date modified"),
            (GroupColumn::DateCreated, "Date created"),
            (GroupColumn::Size, "Size"),
            (GroupColumn::FileType, "Type"),
            (GroupColumn::SyncStatus, "Sync status"),
            (GroupColumn::Tag, "File tag"),
        ];

        let abs_x = (win_w - 70.0 - 236.0).max(10.0);
        let abs_y = 124.0;
        let card = crate::ui::components::dialogs_context::flyout_card_shell(abs_x, abs_y, 236.0, 360.0, mica_geo);
        let mut col = card
            .absolute()
            .top(px(abs_y))
            .right(px(70.0))
            .p(px(6.0))
            .flex()
            .flex_col()
            .gap(px(1.5))
            .child(Self::flyout_section_header("Group by"));

        for (g_col, label) in group_options {
            let active = cur_group == &g_col;
            let v = view.clone();
            col = col.child(Self::flyout_menu_item(
                if active { "\u{E915}" } else { "" },
                label,
                None,
                active,
                move |_, _, cx| {
                    v.update(cx, |this, cx| {
                        this.close_toolbar_flyout(cx);
                        this.set_group_by(g_col, cx);
                    });
                },
            ));
        }

        col
    }

    fn build_layout_flyout(
        state: &AppState,
        view: Entity<crate::app::AppWindow>,
        mica_geo: Option<&crate::wallpaper_mica::MicaGeo>,
        win_w: f32,
    ) -> impl IntoElement {
        let cur_mode = &state.main_view.view_mode;
        let cur_zoom = state.main_view.zoom_level;
        let show_hidden = state.main_view.show_hidden_files;
        let is_dual = state.main_view.pane_type == crate::ui::views::main_view::PaneType::Dual;

        let modes = [
            (ViewMode::Details, "\u{E8FD}", "Details", "Ctrl+Shift+1"),
            (ViewMode::List, "\u{E8EF}", "List", "Ctrl+Shift+2"),
            (ViewMode::Cards, "\u{E8A9}", "Cards (Tiles)", "Ctrl+Shift+3"),
            (ViewMode::Grid, "\u{E80A}", "Grid (Icons)", "Ctrl+Shift+4"),
            (ViewMode::Columns, "\u{E8F1}", "Columns", "Ctrl+Shift+5"),
        ];

        let sizes = [
            (0.0f32, "Compact (Small)"),
            (0.4f32, "Medium (Default)"),
            (0.75f32, "Large"),
            (1.0f32, "Extra Large"),
        ];

        let abs_x = (win_w - 48.0 - 248.0).max(10.0);
        let abs_y = 124.0;
        let card = crate::ui::components::dialogs_context::flyout_card_shell(abs_x, abs_y, 248.0, 480.0, mica_geo);
        let mut col = card
            .absolute()
            .top(px(abs_y))
            .right(px(48.0))
            .p(px(6.0))
            .flex()
            .flex_col()
            .gap(px(1.5))
            .child(Self::flyout_section_header("Layout mode"));

        for (m, icon, label, shortcut) in modes {
            let active = cur_mode == &m;
            let v = view.clone();
            col = col.child(Self::flyout_menu_item(
                icon,
                label,
                Some(shortcut),
                active,
                move |_, _, cx| {
                    let mode_clone = m.clone();
                    v.update(cx, |this, cx| {
                        this.close_toolbar_flyout(cx);
                        this.set_view_mode(mode_clone, cx);
                    });
                },
            ));
        }

        col = col
            .child(div().h(px(1.0)).my(px(3.0)).bg(rgba(0xFFFFFF14u32)))
            .child(Self::flyout_section_header("Grid / row size"));

        for (z_val, label) in sizes {
            let active = (cur_zoom - z_val).abs() < 0.16;
            let v = view.clone();
            col = col.child(Self::flyout_menu_item(
                if active { "\u{E915}" } else { "" },
                label,
                None,
                active,
                move |_, _, cx| {
                    v.update(cx, |this, cx| {
                        this.close_toolbar_flyout(cx);
                        this.set_zoom_level(z_val, cx);
                    });
                },
            ));
        }

        let v_hidden = view.clone();
        let v_ext = view.clone();
        let v_dual = view.clone();
        col = col
            .child(div().h(px(1.0)).my(px(3.0)).bg(rgba(0xFFFFFF14u32)))
            .child(Self::flyout_menu_item(
                if show_hidden { "\u{E73E}" } else { "\u{E7B3}" },
                "Show hidden items",
                Some("Ctrl+H"),
                show_hidden,
                move |_, _, cx| {
                    v_hidden.update(cx, |this, cx| {
                        this.close_toolbar_flyout(cx);
                        this.toggle_setting("show_hidden_files", cx);
                    });
                },
            ))
            .child(Self::flyout_menu_item(
                "\u{E8A5}",
                "Toggle file extensions",
                None,
                false,
                move |_, _, cx| {
                    v_ext.update(cx, |this, cx| {
                        this.close_toolbar_flyout(cx);
                        this.toggle_setting("show_file_extensions", cx);
                    });
                },
            ))
            .child(Self::flyout_menu_item(
                "\u{E140}",
                "Dual Pane Split View",
                Some("Ctrl+Shift+S"),
                is_dual,
                move |_, _, cx| {
                    v_dual.update(cx, |this, cx| {
                        this.close_toolbar_flyout(cx);
                        this.dispatch_action(
                            crate::core::action_registry::ActionKind::SplitVertical,
                            cx,
                        );
                    });
                },
            ));

        col
    }

    fn flyout_section_header(title: &str) -> Div {
        div()
            .px(px(10.0))
            .py(px(4.0))
            .text_size(px(11.0))
            .font_weight(FontWeight(600.0))
            .text_color(rgba(UiConstants::TEXT_MUTED_RGBA))
            .child(title.to_string())
    }

    fn flyout_menu_item(
        glyph: &str,
        label: &str,
        shortcut: Option<&str>,
        is_active: bool,
        on_click: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    ) -> Div {
        let mut row = div()
            .w_full()
            .h(px(30.0))
            .px(px(10.0))
            .rounded(px(5.0))
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .cursor_pointer()
            .bg(if is_active {
                rgba(0x60CDFF1Eu32)
            } else {
                rgba(0x00000000u32)
            })
            .hover(|s| s.bg(rgba(0xFFFFFF14u32)))
            .on_mouse_down(MouseButton::Left, on_click)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(10.0))
                    .child(
                        div()
                            .w(px(16.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_size(px(12.5))
                            .font_family(UiConstants::FONT_ICONS)
                            .text_color(if is_active {
                                rgb(0x60CDFF)
                            } else {
                                rgba(UiConstants::TEXT_PRIMARY_RGBA)
                            })
                            .child(glyph.to_string()),
                    )
                    .child(
                        div()
                            .text_size(px(12.5))
                            .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                            .child(label.to_string()),
                    ),
            );

        if let Some(sc) = shortcut {
            row = row.child(
                div()
                    .text_size(px(11.0))
                    .text_color(rgba(UiConstants::TEXT_MUTED_RGBA))
                    .child(sc.to_string()),
            );
        }

        row
    }
}

struct ToolbarButton;

impl ToolbarButton {
    fn build(
        glyph: &str,
        label: Option<&str>,
        is_enabled: bool,
        is_toggled: bool,
        on_click: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    ) -> impl IntoElement {
        let mut btn = div()
            .flex()
            .flex_row()
            .items_center()
            .justify_center()
            .gap(px(5.0))
            .min_w(px(30.0))
            .h(px(30.0))
            .px(if label.is_some() { px(9.0) } else { px(7.0) })
            .rounded(px(6.0));

        if is_toggled {
            btn = btn
                .bg(rgba(0x60CDFF22u32))
                .border(px(1.0))
                .border_color(rgba(0x60CDFF66u32));
        }

        if is_enabled {
            btn = btn
                .cursor_pointer()
                .hover(|s| {
                    if is_toggled {
                        s.bg(rgba(0x60CDFF32u32))
                    } else {
                        s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA))
                    }
                })
                .on_mouse_down(MouseButton::Left, on_click);
        }

        btn = btn.child(
            div()
                .text_size(px(15.0))
                .font_family(UiConstants::FONT_ICONS)
                .text_color(if is_toggled {
                    rgb(0x60CDFF)
                } else if is_enabled {
                    rgba(UiConstants::TEXT_PRIMARY_RGBA)
                } else {
                    rgba(UiConstants::TEXT_MUTED_RGBA)
                })
                .child(glyph.to_string()),
        );

        if let Some(lbl) = label {
            btn = btn.child(
                div()
                    .text_size(px(12.0))
                    .text_color(if is_enabled {
                        rgba(UiConstants::TEXT_PRIMARY_RGBA)
                    } else {
                        rgba(UiConstants::TEXT_MUTED_RGBA)
                    })
                    .child(lbl.to_string()),
            );
        }

        btn
    }

    fn build_with_tooltip(
        id: impl Into<ElementId>,
        glyph: &str,
        label: Option<&str>,
        tip_text: &'static str,
        tip_shortcut: Option<&'static str>,
        is_enabled: bool,
        is_toggled: bool,
        view: Entity<crate::app::AppWindow>,
        on_click: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    ) -> impl IntoElement {
        let v_tip = view.clone();
        let v_click = view.clone();
        let text = tip_text.to_string();
        let shortcut = tip_shortcut.map(|s| s.to_string());

        let mut btn = div()
            .flex()
            .flex_row()
            .items_center()
            .justify_center()
            .gap(px(5.0))
            .min_w(px(30.0))
            .h(px(30.0))
            .px(if label.is_some() { px(9.0) } else { px(7.0) })
            .rounded(px(6.0));

        if is_toggled {
            btn = btn
                .bg(rgba(0x60CDFF22u32))
                .border(px(1.0))
                .border_color(rgba(0x60CDFF66u32));
        }

        let stateful = btn.id(id);

        let final_btn = if is_enabled {
            stateful
                .cursor_pointer()
                .hover(|s| {
                    if is_toggled {
                        s.bg(rgba(0x60CDFF32u32))
                    } else {
                        s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA))
                    }
                })
                .on_hover(move |hovering: &bool, window: &mut Window, cx: &mut App| {
                    let pos = window.mouse_position();
                    let t = text.clone();
                    let sc = shortcut.clone();
                    if *hovering {
                        v_tip.update(cx, |this, cx| {
                            this.start_button_tooltip(t, sc, pos, cx);
                        });
                    } else {
                        v_tip.update(cx, |this, cx| {
                            this.stop_button_tooltip(cx);
                        });
                    }
                })
                .on_mouse_down(MouseButton::Left, move |e, w, cx| {
                    v_click.update(cx, |this, cx| {
                        this.stop_button_tooltip(cx);
                    });
                    on_click(e, w, cx);
                })
        } else {
            stateful
        };

        let mut elem = final_btn
            .child(
                div()
                    .text_size(px(15.0))
                    .font_family(UiConstants::FONT_ICONS)
                    .text_color(if is_toggled {
                        rgb(0x60CDFF)
                    } else if is_enabled {
                        rgba(UiConstants::TEXT_PRIMARY_RGBA)
                    } else {
                        rgba(UiConstants::TEXT_MUTED_RGBA)
                    })
                    .child(glyph.to_string()),
            );

        if let Some(lbl) = label {
            elem = elem.child(
                div()
                    .text_size(px(12.0))
                    .text_color(if is_enabled {
                        rgba(UiConstants::TEXT_PRIMARY_RGBA)
                    } else {
                        rgba(UiConstants::TEXT_MUTED_RGBA)
                    })
                    .child(lbl.to_string()),
            );
        }

        elem
    }
}

struct ToolbarDivider;

impl ToolbarDivider {
    fn build() -> impl IntoElement {
        div()
            .w(px(1.0))
            .h(px(18.0))
            .mx(px(4.0))
            .bg(rgba(UiConstants::DIVIDER_RGBA))
    }
}

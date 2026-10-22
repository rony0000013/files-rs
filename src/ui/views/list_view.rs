//! List View - Multi-column / compact file listing (Files-community List/Tiles parity)
//!
//! Features:
//! - Responsive multi-column grid-list layout that adapts to pane width and zoom level
//! - Double-click to open files / navigate into folders
//! - Shift+Click range selection & Ctrl+Click toggle selection
//! - Live rubber-band (lasso) drag selection with visual overlay
//! - Inline F2 rename and secondary metadata badges (type/size)
//! - Full dual-pane support

use crate::config::config::GroupColumn;
use crate::core::navigation_manager::PaneFocus;
use crate::storage::storage::FileType;
use crate::ui::constants::UiConstants;
use crate::ui::views::main_view::{
    format_size, get_extension, FileItems, GroupedItemType, MainView, MainViewState, SelectionMode,
};
use gpui::*;

impl MainViewState {
    pub fn render_list_view(
        &self,
        file_items: Entity<FileItems>,
        view: Entity<crate::app::AppWindow>,
    ) -> AnyElement {
        self.render_list_view_for_pane(
            file_items,
            view,
            PaneFocus::Primary,
            gpui::point(0.0, 128.0),
            1000.0,
        )
    }

    pub fn render_list_view_for_pane(
        &self,
        file_items: Entity<FileItems>,
        view: Entity<crate::app::AppWindow>,
        pane_focus: PaneFocus,
        list_origin: Point<f32>,
        pane_w: f32,
    ) -> AnyElement {
        let visible = self.visible_items_rc();

        if self.group_by != GroupColumn::None {
            return self
                .render_list_view_grouped_for_pane(
                    &visible,
                    file_items,
                    view,
                    pane_focus,
                    list_origin,
                    pane_w,
                )
                .into_any_element();
        }

        self.list_view_flat_for_pane(
            &visible,
            file_items,
            view,
            pane_focus,
            list_origin,
            pane_w,
        )
        .into_any_element()
    }

    fn list_view_flat_for_pane(
        &self,
        visible: &[usize],
        file_items: Entity<FileItems>,
        view: Entity<crate::app::AppWindow>,
        pane_focus: PaneFocus,
        list_origin: Point<f32>,
        pane_w: f32,
    ) -> impl IntoElement {
        let visible_clone = self.visible_items_rc();
        let zoom = self.zoom_level;
        let compact = self.compact_mode;
        let (cols, col_w, row_h, gap, padding) = Self::list_layout_metrics(pane_w, zoom, compact);
        let row_count = (visible.len() + cols - 1) / cols.max(1);

        div()
            .id(SharedString::from(match pane_focus {
                PaneFocus::Primary => "list-container-primary",
                PaneFocus::Secondary => "list-container-secondary",
            }))
            .relative()
            .size_full()
            .flex_col()
            .on_mouse_down(MouseButton::Right, {
                let v = view.clone();
                move |event, window, cx| {
                    v.update(cx, |this, cx| {
                        this.focus_pane(pane_focus, cx);
                        this.clear_selection(cx);
                        this.show_context_menu(None, event.position, window, cx);
                    });
                }
            })
            .on_mouse_down(MouseButton::Left, {
                let v = view.clone();
                move |event, _win, cx| {
                    let pos = point(event.position.x.into(), event.position.y.into());
                    let additive = event.modifiers.control || event.modifiers.shift;
                    v.update(cx, |this, cx| {
                        this.focus_pane(pane_focus, cx);
                        this.start_lasso_in_pane(pane_focus, pos, list_origin, additive, cx);
                    });
                }
            })
            .on_mouse_move({
                let v = view.clone();
                move |event, _win, cx| {
                    let pos = point(event.position.x.into(), event.position.y.into());
                    v.update(cx, |this, cx| {
                        this.update_lasso_in_pane(pane_focus, pos, pane_w, cx);
                    });
                }
            })
            .on_mouse_up(MouseButton::Left, {
                let v = view.clone();
                move |_event, _win, cx| {
                    v.update(cx, |this, cx| {
                        this.end_lasso_in_pane(pane_focus, cx);
                    });
                }
            })
            .on_mouse_up_out(MouseButton::Left, {
                let v = view.clone();
                move |_event, _win, cx| {
                    v.update(cx, |this, cx| {
                        this.end_lasso_in_pane(pane_focus, cx);
                    });
                }
            })
            .child(
                uniform_list(
                    match pane_focus {
                        PaneFocus::Primary => "list-scroll-primary",
                        PaneFocus::Secondary => "list-scroll-secondary",
                    },
                    row_count,
                    move |range, _win, cx| {
                        let fi = file_items.read(cx);
                        let app_state = view.read(cx);
                        let main_state = app_state.state.pane_view(pane_focus);
                        let icon_paths = &app_state.state.main_view.icon_paths;
                        let zoom = main_state.zoom_level;
                        let compact = main_state.compact_mode;
                        let (cols, col_w, row_h, gap, padding) =
                            Self::list_layout_metrics(pane_w, zoom, compact);
                        let icon_size = (row_h - 10.0).clamp(16.0, 32.0);
                        let text_size = (11.5 + zoom * 3.0).clamp(11.0, 14.0);
                        let is_checkbox = main_state.selection_mode == SelectionMode::Checkbox;
                        let is_hover_select =
                            main_state.selection_mode == SelectionMode::HoverSelect;

                        range
                            .map(|row_idx| {
                                // Column-major flow (Files List view): items fill each
                                // column top-to-bottom, then continue in the next column.
                                let row_items: Vec<usize> = (0..cols)
                                    .filter_map(|c| {
                                        let i = row_idx + c * row_count;
                                        (i < visible_clone.len()).then_some(i)
                                    })
                                    .collect();
                                let items_in_row = row_items.len();

                                let mut row_cells: Vec<AnyElement> = row_items
                                    .into_iter()
                                    .filter_map(|i| {
                                        let idx = visible_clone[i];
                                        let item = fi.items.get(idx)?;
                                        let is_selected = fi.selected_indices.contains(&idx);
                                        let is_dir = matches!(item.file_type, FileType::Directory);
                                        let path = item.path.clone();
                                        let display_name =
                                            main_state.display_name(item).into_owned();

                                        let view_row = view.clone();

                                        let mut cell = div()
                                            .id(ElementId::from(
                                                idx + match pane_focus {
                                                    PaneFocus::Primary => 20000,
                                                    PaneFocus::Secondary => 120000,
                                                },
                                            ))
                                            .flex()
                                            .flex_shrink_0()
                                            .items_center()
                                            .w(px(col_w))
                                            .h(px(row_h - 2.0))
                                            .px(px(10.0))
                                            .gap(px(8.0))
                                            .rounded(px(5.0))
                                            .border(px(1.0))
                                            .border_color(if is_selected {
                                                rgba(0x60CDFF88u32)
                                            } else {
                                                rgba(0x00000000u32)
                                            })
                                            .bg(if is_selected {
                                                rgba(UiConstants::BG_ITEM_SELECTED_RGBA)
                                            } else {
                                                rgba(UiConstants::BG_TRANSPARENT_RGBA)
                                            })
                                            .hover(|s| {
                                                s.bg(if is_selected {
                                                    rgba(UiConstants::BG_ITEM_SELECTED_HOVER_RGBA)
                                                } else {
                                                    rgba(UiConstants::BG_ITEM_HOVER_RGBA)
                                                })
                                            })
                                            .cursor_pointer()
                                            .on_mouse_down(MouseButton::Left, {
                                                let v = view_row.clone();
                                                let p_nav = path.clone();
                                                move |ev, _, cx| {
                                                    cx.stop_propagation();
                                                    v.update(cx, |this, cx| {
                                                        this.focus_pane(pane_focus, cx);
                                                        if ev.click_count == 2 {
                                                            if is_dir {
                                                                this.navigate_pane(
                                                                    p_nav.clone(),
                                                                    pane_focus,
                                                                    cx,
                                                                );
                                                            } else {
                                                                this.open_item_in_pane(
                                                                    pane_focus, idx, cx,
                                                                );
                                                            }
                                                        } else {
                                                            this.select_item_in_pane(
                                                                pane_focus,
                                                                idx,
                                                                ev.modifiers.shift,
                                                                ev.modifiers.control,
                                                                cx,
                                                            );
                                                        }
                                                    });
                                                }
                                            })
                                            .on_mouse_move({
                                                let v = view_row.clone();
                                                move |ev, _win, cx| {
                                                    let pos = point(
                                                        ev.position.x.into(),
                                                        ev.position.y.into(),
                                                    );
                                                    v.update(cx, |this, cx| {
                                                        if this
                                                            .state
                                                            .pane_view(pane_focus)
                                                            .lasso_state
                                                            .is_some()
                                                        {
                                                            this.update_lasso_in_pane(
                                                                pane_focus, pos, pane_w, cx,
                                                            );
                                                        } else if is_hover_select {
                                                            this.handle_item_hover(idx, cx);
                                                        }
                                                    });
                                                }
                                            })
                                            .on_mouse_down(MouseButton::Right, {
                                                let v = view_row.clone();
                                                move |ev, window, cx| {
                                                    cx.stop_propagation();
                                                    v.update(cx, |this, cx| {
                                                        this.focus_pane(pane_focus, cx);
                                                        if !this
                                                            .state
                                                            .pane_view(pane_focus)
                                                            .selected_indices
                                                            .contains(&idx)
                                                        {
                                                            this.select_item_in_pane(
                                                                pane_focus, idx, false, false, cx,
                                                            );
                                                        }
                                                        this.show_context_menu(
                                                            Some(idx),
                                                            ev.position,
                                                            window,
                                                            cx,
                                                        );
                                                    });
                                                }
                                            });

                                        if is_selected {
                                            cell = cell.child(
                                                div()
                                                    .w(px(3.0))
                                                    .h(px(14.0))
                                                    .rounded(px(1.5))
                                                    .bg(rgb(UiConstants::ACCENT_COLOR_RGB)),
                                            );
                                        }

                                        if is_checkbox {
                                            let v_cb = view_row.clone();
                                            cell = cell.child(
                                                div()
                                                    .w(px(16.0))
                                                    .h(px(16.0))
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .on_mouse_down(
                                                        MouseButton::Left,
                                                        move |_, _, cx| {
                                                            cx.stop_propagation();
                                                            v_cb.update(cx, |this, cx| {
                                                                this.focus_pane(pane_focus, cx);
                                                                this.toggle_checkbox_selection(
                                                                    idx, cx,
                                                                );
                                                            });
                                                        },
                                                    )
                                                    .child(
                                                        div()
                                                            .w(px(13.0))
                                                            .h(px(13.0))
                                                            .border(px(1.5))
                                                            .border_color(rgba(
                                                                UiConstants::ACCENT_RGBA,
                                                            ))
                                                            .rounded(px(3.0))
                                                            .bg(if is_selected {
                                                                rgba(UiConstants::ACCENT_RGBA)
                                                            } else {
                                                                rgba(0x00000000)
                                                            }),
                                                    ),
                                            );
                                        }

                                        cell = cell
                                            .child(
                                                div()
                                                    .w(px(icon_size))
                                                    .h(px(icon_size))
                                                    .flex_shrink_0()
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .child(
                                                        if let Some(icon_path) =
                                                            icon_paths.get(&path)
                                                        {
                                                            img(icon_path.clone())
                                                                .w(px(icon_size))
                                                                .h(px(icon_size))
                                                                .into_any_element()
                                                        } else {
                                                            div()
                                                                .text_size(px(icon_size * 0.65))
                                                                .font_family(UiConstants::FONT_MDL2)
                                                                .text_color(if is_dir {
                                                                    rgb(0xF5B942)
                                                                } else {
                                                                    rgba(
                                                                        UiConstants::TEXT_SECONDARY_RGBA,
                                                                    )
                                                                })
                                                                .child(if is_dir {
                                                                    "\u{E8B7}"
                                                                } else {
                                                                    "\u{E8A5}"
                                                                })
                                                                .into_any_element()
                                                        },
                                                    ),
                                            )
                                            .child(
                                                div()
                                                    .flex_1()
                                                    .min_w(px(0.0))
                                                    .overflow_hidden()
                                                    .text_size(px(text_size))
                                                    .text_color(rgba(
                                                        UiConstants::TEXT_PRIMARY_RGBA,
                                                    ))
                                                    .child(if main_state.renaming_index == Some(idx)
                                                    {
                                                        div()
                                                            .bg(rgba(0xFFFFFF20u32))
                                                            .border(px(1.0))
                                                            .border_color(rgba(
                                                                UiConstants::ACCENT_RGBA,
                                                            ))
                                                            .rounded(px(2.0))
                                                            .px(px(4.0))
                                                            .child(main_state.rename_text.clone())
                                                            .into_any_element()
                                                    } else {
                                                        div()
                                                            .child(display_name)
                                                            .into_any_element()
                                                    }),
                                            )
                                            ;

                                        Some(cell.into_any_element())
                                    })
                                    .collect();

                                for _ in items_in_row..cols {
                                    row_cells.push(
                                        div()
                                            .w(px(col_w))
                                            .flex_shrink_0()
                                            .invisible()
                                            .into_any_element(),
                                    );
                                }

                                div()
                                    .w_full()
                                    .h(px(row_h))
                                    .px(px(padding))
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap(px(gap))
                                    .children(row_cells)
                                    .into_any_element()
                            })
                            .collect::<Vec<_>>()
                    },
                )
                .with_sizing_behavior(ListSizingBehavior::Auto)
                .w_full()
                .h_full()
                .track_scroll(self.uniform_list_scroll_handle.clone()),
            )
            .child(MainView::render_lasso_box(self))
    }

    fn render_list_view_grouped_for_pane(
        &self,
        visible: &[usize],
        file_items: Entity<FileItems>,
        view: Entity<crate::app::AppWindow>,
        pane_focus: PaneFocus,
        list_origin: Point<f32>,
        pane_w: f32,
    ) -> impl IntoElement {
        let flattened = self.flatten_grouped_items(visible);
        let item_count = flattened.len();

        div()
            .relative()
            .flex_col()
            .w_full()
            .flex_grow()
            .h_full()
            .on_mouse_down(MouseButton::Left, {
                let v = view.clone();
                move |event, _win, cx| {
                    let pos = point(event.position.x.into(), event.position.y.into());
                    let additive = event.modifiers.control || event.modifiers.shift;
                    v.update(cx, |this, cx| {
                        this.focus_pane(pane_focus, cx);
                        this.start_lasso_in_pane(pane_focus, pos, list_origin, additive, cx);
                    });
                }
            })
            .on_mouse_move({
                let v = view.clone();
                move |event, _win, cx| {
                    let pos = point(event.position.x.into(), event.position.y.into());
                    v.update(cx, |this, cx| {
                        this.update_lasso_in_pane(pane_focus, pos, pane_w, cx);
                    });
                }
            })
            .on_mouse_up(MouseButton::Left, {
                let v = view.clone();
                move |_event, _win, cx| {
                    v.update(cx, |this, cx| {
                        this.end_lasso_in_pane(pane_focus, cx);
                    });
                }
            })
            .child(
                uniform_list(
                    match pane_focus {
                        PaneFocus::Primary => "list-grouped-scroll-primary",
                        PaneFocus::Secondary => "list-grouped-scroll-secondary",
                    },
                    item_count,
                    {
                        let view = view.clone();
                        move |range, _win, cx| {
                            let fi = file_items.read(cx);
                            let app_state = view.read(cx);
                            let main_state = app_state.state.pane_view(pane_focus);
                            let icon_paths = &app_state.state.main_view.icon_paths;
                            let zoom = main_state.zoom_level;
                            let row_h = 26.0 + zoom * 24.0;
                            let collapsed_groups = main_state.collapsed_groups.clone();

                            range
                                .map(|i| match &flattened[i] {
                                    GroupedItemType::Header { key, label, count } => {
                                        let group_height =
                                            if main_state.compact_mode { 24.0 } else { 32.0 };
                                        let key_clone = key.clone();
                                        let v = view.clone();

                                        div()
                                            .id(ElementId::from(60000 + i))
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .h(px(group_height))
                                            .px(px(16.0))
                                            .bg(rgba(0x0000000Au32))
                                            .cursor_pointer()
                                            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                                cx.stop_propagation();
                                                v.update(cx, |this, cx| {
                                                    this.focus_pane(pane_focus, cx);
                                                    this.toggle_group_collapse(
                                                        key_clone.clone(),
                                                        cx,
                                                    );
                                                });
                                            })
                                            .child(
                                                div()
                                                    .text_size(px(12.0))
                                                    .font_family(UiConstants::FONT_MDL2)
                                                    .text_color(rgba(
                                                        UiConstants::TEXT_SECONDARY_RGBA,
                                                    ))
                                                    .child(if collapsed_groups.contains(key) {
                                                        "\u{E76C}"
                                                    } else {
                                                        "\u{E76B}"
                                                    }),
                                            )
                                            .child(
                                                div()
                                                    .ml(px(8.0))
                                                    .text_size(px(13.0))
                                                    .font_weight(FontWeight(600.0))
                                                    .text_color(rgba(
                                                        UiConstants::TEXT_PRIMARY_RGBA,
                                                    ))
                                                    .child(label.clone()),
                                            )
                                            .child(
                                                div()
                                                    .ml(px(8.0))
                                                    .text_size(px(11.0))
                                                    .text_color(rgba(
                                                        UiConstants::TEXT_SECONDARY_RGBA,
                                                    ))
                                                    .child(format!(
                                                        "{} item{}",
                                                        count,
                                                        if *count == 1 { "" } else { "s" }
                                                    )),
                                            )
                                            .into_any_element()
                                    }
                                    GroupedItemType::Item(idx) => {
                                        let idx = *idx;
                                        if idx >= fi.items.len() {
                                            return div().into_any_element();
                                        }
                                        let item = &fi.items[idx];
                                        let is_selected = fi.selected_indices.contains(&idx);
                                        let is_dir = matches!(item.file_type, FileType::Directory);
                                        let path = item.path.clone();
                                        let display_name =
                                            main_state.display_name(item).into_owned();
                                        let v = view.clone();
                                        let icon_size = 16.0 + zoom * 20.0;
                                        let text_size = 11.5 + zoom * 3.0;

                                        div()
                                            .id(ElementId::from(idx + 20000))
                                            .flex()
                                            .items_center()
                                            .w_full()
                                            .h(px(row_h))
                                            .px(px(16.0))
                                            .gap(px(8.0))
                                            .bg(if is_selected {
                                                rgba(UiConstants::BG_ITEM_SELECTED_RGBA)
                                            } else {
                                                rgba(UiConstants::BG_TRANSPARENT_RGBA)
                                            })
                                            .hover(|s| {
                                                s.bg(if is_selected {
                                                    rgba(UiConstants::BG_ITEM_SELECTED_HOVER_RGBA)
                                                } else {
                                                    rgba(UiConstants::BG_ITEM_HOVER_RGBA)
                                                })
                                            })
                                            .cursor_pointer()
                                            .child(if is_selected {
                                                div()
                                                    .w(px(3.0))
                                                    .h(px(16.0))
                                                    .rounded(px(1.5))
                                                    .bg(rgb(UiConstants::ACCENT_COLOR_RGB))
                                            } else {
                                                div()
                                            })
                                            .on_mouse_down(MouseButton::Left, {
                                                let v = v.clone();
                                                let p_nav = path.clone();
                                                move |ev, _, cx| {
                                                    cx.stop_propagation();
                                                    v.update(cx, |this, cx| {
                                                        this.focus_pane(pane_focus, cx);
                                                        if ev.click_count == 2 {
                                                            if is_dir {
                                                                this.navigate_pane(
                                                                    p_nav.clone(),
                                                                    pane_focus,
                                                                    cx,
                                                                );
                                                            } else {
                                                                this.open_item_in_pane(
                                                                    pane_focus, idx, cx,
                                                                );
                                                            }
                                                        } else {
                                                            this.select_item_in_pane(
                                                                pane_focus,
                                                                idx,
                                                                ev.modifiers.shift,
                                                                ev.modifiers.control,
                                                                cx,
                                                            );
                                                        }
                                                    });
                                                }
                                            })
                                            .on_mouse_down(MouseButton::Right, {
                                                let v = v.clone();
                                                move |ev, window, cx| {
                                                    cx.stop_propagation();
                                                    v.update(cx, |this, cx| {
                                                        this.focus_pane(pane_focus, cx);
                                                        if !this
                                                            .state
                                                            .pane_view(pane_focus)
                                                            .selected_indices
                                                            .contains(&idx)
                                                        {
                                                            this.select_item_in_pane(
                                                                pane_focus, idx, false, false, cx,
                                                            );
                                                        }
                                                        this.show_context_menu(
                                                            Some(idx),
                                                            ev.position,
                                                            window,
                                                            cx,
                                                        );
                                                    });
                                                }
                                            })
                                            .child(
                                                div()
                                                    .w(px(icon_size))
                                                    .h(px(icon_size))
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .child(
                                                        if let Some(icon_path) =
                                                            icon_paths.get(&path)
                                                        {
                                                            img(icon_path.clone())
                                                                .w(px(icon_size))
                                                                .h(px(icon_size))
                                                                .into_any_element()
                                                        } else {
                                                            div()
                                                                .text_size(px(icon_size * 0.65))
                                                                .font_family(UiConstants::FONT_MDL2)
                                                                .text_color(if is_dir {
                                                                    rgb(0xF5B942)
                                                                } else {
                                                                    rgba(
                                                                        UiConstants::TEXT_SECONDARY_RGBA,
                                                                    )
                                                                })
                                                                .child(if is_dir {
                                                                    "\u{E8B7}"
                                                                } else {
                                                                    "\u{E8A5}"
                                                                })
                                                                .into_any_element()
                                                        },
                                                    ),
                                            )
                                            .child(if main_state.renaming_index == Some(idx) {
                                                div()
                                                    .bg(rgba(0xFFFFFF20u32))
                                                    .border(px(1.0))
                                                    .border_color(rgba(UiConstants::ACCENT_RGBA))
                                                    .rounded(px(2.0))
                                                    .px(px(4.0))
                                                    .child(main_state.rename_text.clone())
                                                    .into_any_element()
                                            } else {
                                                div()
                                                    .text_size(px(text_size))
                                                    .text_color(rgba(
                                                        UiConstants::TEXT_PRIMARY_RGBA,
                                                    ))
                                                    .child(display_name)
                                                    .into_any_element()
                                            })
                                            .into_any_element()
                                    }
                                })
                                .collect::<Vec<_>>()
                        }
                    },
                )
                .with_sizing_behavior(ListSizingBehavior::Auto)
                .w_full()
                .h_full()
                .track_scroll(self.uniform_list_scroll_handle.clone()),
            )
            .child(MainView::render_lasso_box(self))
    }
}

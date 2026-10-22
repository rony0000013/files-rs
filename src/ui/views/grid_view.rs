//! Grid View - Icon/grid/tile/cards display with dynamic zoom, live rubber-band selection,
//! Shift/Ctrl multi-selection, and dual-pane support.

use crate::config::config::GroupColumn;
use crate::core::navigation_manager::PaneFocus;
use crate::storage::storage::FileType;
use crate::ui::components::icon_renderer::is_thumbnail_image;
use crate::ui::constants::UiConstants;
use crate::ui::views::main_view::{
    format_size, get_extension, get_file_type_icon, FileItems, GroupedGridItem, MainView,
    MainViewState, SelectionMode,
};
use gpui::*;

impl MainViewState {
    /// Render grid view with dynamic zoom (0–1 mapping compact tiles → large cards)
    pub fn render_grid_view(
        &self,
        file_items: Entity<FileItems>,
        view: Entity<crate::app::AppWindow>,
        cols: usize,
        pane_w: f32,
    ) -> AnyElement {
        self.render_grid_view_for_pane(
            file_items,
            view,
            cols,
            pane_w,
            PaneFocus::Primary,
            gpui::point(0.0, 128.0),
        )
    }

    pub fn render_grid_view_for_pane(
        &self,
        file_items: Entity<FileItems>,
        view: Entity<crate::app::AppWindow>,
        cols: usize,
        pane_w: f32,
        pane_focus: PaneFocus,
        list_origin: Point<f32>,
    ) -> AnyElement {
        let visible = self.visible_items_rc();

        if self.group_by != GroupColumn::None {
            return self
                .render_grid_view_grouped_for_pane(
                    &visible,
                    file_items,
                    view,
                    cols,
                    pane_w,
                    pane_focus,
                    list_origin,
                )
                .into_any_element();
        }

        self.grid_view_flat_for_pane(
            &visible,
            file_items,
            view,
            cols,
            pane_w,
            pane_focus,
            list_origin,
        )
        .into_any_element()
    }

    /// Compute grid layout parameters from zoom_level
    pub fn grid_layout_params(zoom: f32) -> GridLayoutParams {
        let items_per_row = if zoom < 0.33 {
            7
        } else if zoom < 0.66 {
            5
        } else {
            4
        };

        let item_width = (128.0 + zoom * 124.0).min(252.0);
        let thumb_height = item_width * 0.72;
        let text_size = (item_width / 9.0).clamp(11.0, 13.0);
        let item_height = thumb_height + 20.0 + text_size * 1.3 * 2.0;
        let row_height = item_height + 8.0;
        let icon_size = thumb_height * 0.78;
        let gap = if zoom < 0.33 {
            6.0
        } else if zoom < 0.66 {
            10.0
        } else {
            14.0
        };
        let rounded = if zoom < 0.33 {
            4.0
        } else if zoom < 0.66 {
            6.0
        } else {
            8.0
        };
        let padding = if zoom < 0.33 { 10.0 } else { 14.0 };

        GridLayoutParams {
            items_per_row,
            item_width,
            item_height,
            row_height,
            icon_size,
            gap,
            rounded,
            text_size,
            padding,
        }
    }

    /// Columns that fit the actual pane width (Files-style: fill the row).
    pub fn grid_columns_for_width(pane_w: f32, zoom: f32) -> usize {
        let params = Self::grid_layout_params(zoom);
        let slot = params.item_width + params.gap;
        if pane_w <= slot || slot <= 0.0 {
            return params.items_per_row;
        }
        let avail = pane_w - params.padding * 2.0;
        (((avail + params.gap) / slot).floor() as usize).clamp(1, 32)
    }

    pub fn grid_layout_params_for_width_pub(
        pane_w: f32,
        zoom: f32,
        cols: usize,
    ) -> GridLayoutParams {
        Self::grid_layout_params_for_width(pane_w, zoom, cols)
    }

    fn grid_layout_params_for_width(pane_w: f32, zoom: f32, cols: usize) -> GridLayoutParams {
        let base = Self::grid_layout_params(zoom);
        let cols = cols.max(1);
        let avail = pane_w - base.padding * 2.0;
        if avail <= base.item_width {
            let mut params = base;
            params.items_per_row = cols;
            return params;
        }
        let total_gaps = base.gap * (cols.saturating_sub(1) as f32);
        let stretched_w = ((avail - total_gaps) / cols as f32)
            .floor()
            .clamp(base.item_width, base.item_width * 1.35);
        let thumb_height = stretched_w * 0.72;
        let fit_text = (stretched_w / 9.0).clamp(11.0, 13.0);
        let item_height = thumb_height + 20.0 + fit_text * 1.3 * 2.0;
        GridLayoutParams {
            items_per_row: cols,
            item_width: stretched_w,
            item_height,
            row_height: item_height + 8.0,
            icon_size: thumb_height * 0.78,
            text_size: fit_text,
            ..base
        }
    }

    fn grid_view_flat_for_pane(
        &self,
        visible: &[usize],
        file_items: Entity<FileItems>,
        view: Entity<crate::app::AppWindow>,
        cols: usize,
        pane_w: f32,
        pane_focus: PaneFocus,
        list_origin: Point<f32>,
    ) -> impl IntoElement {
        let visible_clone = self.visible_items_rc();
        let zoom = self.zoom_level;
        let params = Self::grid_layout_params_for_width(pane_w, zoom, cols);
        let items_per_row = params.items_per_row.max(1);
        let row_count = (visible.len() + items_per_row - 1) / items_per_row;

        div()
            .id(SharedString::from(match pane_focus {
                PaneFocus::Primary => "grid-container-primary",
                PaneFocus::Secondary => "grid-container-secondary",
            }))
            .relative()
            .flex_col()
            .w_full()
            .flex_1()
            .h_full()
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
                        PaneFocus::Primary => "grid-scroll-primary",
                        PaneFocus::Secondary => "grid-scroll-secondary",
                    },
                    row_count,
                    {
                        let view = view.clone();
                        move |range, _win, cx| {
                            let fi = file_items.read(cx);
                            let app_state = view.read(cx);
                            let main_state = app_state.state.pane_view(pane_focus);
                            let icon_paths = &app_state.state.main_view.icon_paths;
                            let params = params.clone();
                            let selected = main_state.selected_indices.clone();
                            let sel_mode = main_state.selection_mode.clone();
                            let compact = main_state.compact_mode;
                            let show_extensions = main_state.show_extensions;

                            range
                                .map(|row_idx| {
                                    let start = row_idx * params.items_per_row;
                                    let end =
                                        (start + params.items_per_row).min(visible_clone.len());
                                    let items_in_row = end.saturating_sub(start);

                                    let mut row_children: Vec<AnyElement> = (start..end)
                                        .filter_map(|i| {
                                            let idx = visible_clone[i];
                                            let item = fi.items.get(idx)?;
                                            let is_selected = selected.contains(&idx);
                                            Some(
                                                Self::grid_item_for_pane(
                                                    item,
                                                    idx,
                                                    is_selected,
                                                    sel_mode.clone(),
                                                    view.clone(),
                                                    icon_paths,
                                                    &params,
                                                    compact,
                                                    show_extensions,
                                                    main_state.renaming_index,
                                                    main_state.rename_text.clone(),
                                                    pane_focus,
                                                    pane_w,
                                                )
                                                .into_any_element(),
                                            )
                                        })
                                        .collect();

                                    for _ in items_in_row..params.items_per_row {
                                        row_children.push(
                                            div()
                                                .w(px(params.item_width))
                                                .flex_shrink_0()
                                                .invisible()
                                                .into_any_element(),
                                        );
                                    }

                                    div()
                                        .w_full()
                                        .flex()
                                        .flex_row()
                                        .h(px(params.row_height))
                                        .gap(px(params.gap))
                                        .px(px(params.padding))
                                        .children(row_children)
                                        .into_any_element()
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

    fn render_grid_view_grouped_for_pane(
        &self,
        visible: &[usize],
        file_items: Entity<FileItems>,
        view: Entity<crate::app::AppWindow>,
        cols: usize,
        pane_w: f32,
        pane_focus: PaneFocus,
        list_origin: Point<f32>,
    ) -> impl IntoElement {
        let zoom = self.zoom_level;
        let params = Self::grid_layout_params_for_width(pane_w, zoom, cols);
        let items_per_row = params.items_per_row.max(1);
        let flattened = self.flatten_grouped_grid_items(visible, items_per_row);
        let item_count = flattened.len();

        div()
            .relative()
            .flex_col()
            .w_full()
            .flex_1()
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
                        PaneFocus::Primary => "grid-grouped-scroll-primary",
                        PaneFocus::Secondary => "grid-grouped-scroll-secondary",
                    },
                    item_count,
                    {
                        let view = view.clone();
                        move |range, _win, cx| {
                            let fi = file_items.read(cx);
                            let app_state = view.read(cx);
                            let main_state = app_state.state.pane_view(pane_focus);
                            let icon_paths = &app_state.state.main_view.icon_paths;
                            let params = params.clone();
                            let sel_mode = main_state.selection_mode.clone();
                            let collapsed_groups = main_state.collapsed_groups.clone();
                            let compact = main_state.compact_mode;
                            let show_extensions = main_state.show_extensions;

                            range
                                .map(|i| match &flattened[i] {
                                    GroupedGridItem::Header { key, label, count } => {
                                        let group_height =
                                            if main_state.compact_mode { 24.0 } else { 32.0 };
                                        let key_clone = key.clone();
                                        let v = view.clone();

                                        div()
                                            .id(ElementId::from(60000 + i))
                                            .flex()
                                            .flex_row()
                                            .h(px(group_height))
                                            .items_center()
                                            .px(px(params.padding))
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
                                                    .ml(px(6.0))
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
                                    GroupedGridItem::Row(indices) => {
                                        let indices = indices.clone();
                                        let items_in_row = indices.len();

                                        let mut row_children: Vec<AnyElement> = indices
                                            .iter()
                                            .filter_map(|idx| {
                                                let item = fi.items.get(*idx)?;
                                                let selected = fi.selected_indices.contains(idx);
                                                Some(
                                                    Self::grid_item_for_pane(
                                                        item,
                                                        *idx,
                                                        selected,
                                                        sel_mode.clone(),
                                                        view.clone(),
                                                        icon_paths,
                                                        &params,
                                                        compact,
                                                        show_extensions,
                                                        main_state.renaming_index,
                                                        main_state.rename_text.clone(),
                                                        pane_focus,
                                                        pane_w,
                                                    )
                                                    .into_any_element(),
                                                )
                                            })
                                            .collect();

                                        for _ in items_in_row..params.items_per_row {
                                            row_children.push(
                                                div()
                                                    .w(px(params.item_width))
                                                    .flex_shrink_0()
                                                    .invisible()
                                                    .into_any_element(),
                                            );
                                        }

                                        div()
                                            .w_full()
                                            .flex()
                                            .flex_row()
                                            .gap(px(params.gap))
                                            .px(px(params.padding))
                                            .py(px(params.gap * 0.5))
                                            .children(row_children)
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

    #[allow(clippy::too_many_arguments)]
    fn grid_item_for_pane(
        item: &crate::domain::metadata::FileItem,
        index: usize,
        selected: bool,
        selection_mode: SelectionMode,
        view: Entity<crate::app::AppWindow>,
        icon_paths: &std::collections::HashMap<
            crate::core::path_util::FilesPath,
            std::path::PathBuf,
        >,
        params: &GridLayoutParams,
        compact: bool,
        show_extensions: bool,
        renaming_index: Option<usize>,
        rename_text: String,
        pane_focus: PaneFocus,
        pane_w: f32,
    ) -> impl IntoElement {
        let v_click = view.clone();
        let v_hover = view.clone();
        let v_timer = view.clone();
        let v_checkbox = view.clone();
        let is_dir = matches!(item.file_type, FileType::Directory);
        let extension = get_extension(&item.name);
        let is_thumbnail = !is_dir && is_thumbnail_image(&extension);
        let show_checkbox = selection_mode == SelectionMode::Checkbox;
        let is_hover_select = selection_mode == SelectionMode::HoverSelect;
        let item_path = item.path.clone();
        let raw_name = if show_extensions || is_dir {
            item.name.clone()
        } else {
            std::path::Path::new(&item.name)
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned()
        };
        let cached_icon_path: Option<std::path::PathBuf> = icon_paths.get(&item_path).cloned();

        let item_width = params.item_width;
        let thumb_height = item_width * 0.72;
        let item_height = if compact {
            params.item_height - 8.0
        } else {
            params.item_height
        };
        let icon_size_val = params.icon_size;
        let rounded = params.rounded;
        let text_size = params.text_size;
        let subtitle = if is_dir {
            "Folder".to_string()
        } else {
            format_size(item.size)
        };

        let mut item_div = div()
            .id(ElementId::from(
                index + match pane_focus {
                    PaneFocus::Primary => 70000,
                    PaneFocus::Secondary => 170000,
                },
            ))
            .w(px(item_width))
            .flex_shrink_0()
            .h(px(item_height))
            .flex_col()
            .items_center()
            .overflow_hidden()
            .py(px(6.0))
            .rounded(px(rounded))
            .relative()
            .cursor_pointer()
            .border(px(1.0))
            .border_color(if selected {
                rgba(0x60CDFFFFu32)
            } else {
                rgba(0x00000000u32)
            })
            .bg(if selected {
                rgba(UiConstants::BG_ITEM_SELECTED_RGBA)
            } else {
                rgba(0x00000000u32)
            })
            .hover(|s| {
                s.bg(if selected {
                    rgba(UiConstants::BG_ITEM_SELECTED_HOVER_RGBA)
                } else {
                    rgba(UiConstants::BG_ITEM_HOVER_RGBA)
                })
            });

        if show_checkbox || selected {
            item_div = item_div.child(
                div()
                    .absolute()
                    .top(px(4.0))
                    .left(px(4.0))
                    .w(px(20.0))
                    .h(px(20.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        cx.stop_propagation();
                        v_checkbox.update(cx, |this, cx| {
                            this.focus_pane(pane_focus, cx);
                            this.toggle_checkbox_selection(index, cx);
                        });
                    })
                    .child(
                        div()
                            .w(px(14.0))
                            .h(px(14.0))
                            .border(px(1.5))
                            .border_color(rgba(UiConstants::ACCENT_RGBA))
                            .rounded(px(3.0))
                            .bg(if selected {
                                rgba(UiConstants::ACCENT_RGBA)
                            } else {
                                rgba(0x00000000)
                            })
                            .child(if selected {
                                div()
                                    .text_size(px(9.0))
                                    .font_family(UiConstants::FONT_MDL2)
                                    .text_color(rgb(0xFFFFFF))
                                    .child("\u{E73E}")
                            } else {
                                div()
                            }),
                    ),
            );
        }

        let path_clone = item_path.clone();
        let visual_el: AnyElement = if is_thumbnail {
            img(if let Some(p) = &cached_icon_path {
                p.clone()
            } else {
                item_path.as_path().to_path_buf()
            })
            .w(px(icon_size_val))
            .h(px(icon_size_val))
            .object_fit(gpui::ObjectFit::Contain)
            .rounded(px(rounded))
            .into_any_element()
        } else if let Some(icon_path) = &cached_icon_path {
            div()
                .w(px(thumb_height * 0.82))
                .h(px(thumb_height * 0.82))
                .flex()
                .items_center()
                .justify_center()
                .child(
                    img(icon_path.clone())
                        .w_full()
                        .h_full()
                        .object_fit(gpui::ObjectFit::Contain)
                        .rounded(px(rounded)),
                )
                .into_any_element()
        } else {
            let file_icon = if is_dir {
                "\u{E8B7}".to_string()
            } else {
                get_file_type_icon(&extension).to_string()
            };
            div()
                .w_full()
                .h_full()
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .text_size(px(thumb_height * 0.72))
                        .font_family(UiConstants::FONT_MDL2)
                        .text_color(if is_dir {
                            rgb(0xF5B942)
                        } else {
                            rgba(UiConstants::TEXT_SECONDARY_RGBA)
                        })
                        .child(file_icon),
                )
                .into_any_element()
        };

        item_div
            .on_mouse_down(MouseButton::Left, move |event, _window, cx| {
                cx.stop_propagation();
                v_click.update(cx, |this, cx| {
                    this.focus_pane(pane_focus, cx);
                    if event.click_count == 2 {
                        if is_dir {
                            this.navigate_pane(path_clone.clone(), pane_focus, cx);
                        } else {
                            this.open_item_in_pane(pane_focus, index, cx);
                        }
                    } else {
                        this.select_item_in_pane(
                            pane_focus,
                            index,
                            event.modifiers.shift,
                            event.modifiers.control,
                            cx,
                        );
                    }
                });
            })
            .on_mouse_down(MouseButton::Right, {
                let v = view.clone();
                move |event, window, cx| {
                    cx.stop_propagation();
                    v.update(cx, |this, cx| {
                        this.focus_pane(pane_focus, cx);
                        if !this
                            .state
                            .pane_view(pane_focus)
                            .selected_indices
                            .contains(&index)
                        {
                            this.select_item_in_pane(pane_focus, index, false, false, cx);
                        }
                        this.show_context_menu(Some(index), event.position, window, cx);
                    });
                }
            })
            .on_mouse_move({
                let v_hover = v_hover.clone();
                let v_timer = v_timer.clone();
                move |ev, _window, cx| {
                    let pos = point(ev.position.x.into(), ev.position.y.into());
                    v_hover.update(cx, |this, cx| {
                        if this.state.pane_view(pane_focus).lasso_state.is_some() {
                            this.update_lasso_in_pane(pane_focus, pos, pane_w, cx);
                        } else if is_hover_select {
                            this.handle_item_hover(index, cx);
                        }
                    });
                    if !is_hover_select {
                        v_timer.update(cx, |this, cx| {
                            if this.state.pane_view(pane_focus).lasso_state.is_none() {
                                this.start_hover_timer(index, ev.position, cx);
                            }
                        });
                    }
                }
            })
            .on_hover({
                let v = view.clone();
                move |hovered, _, cx| {
                    if !hovered {
                        v.update(cx, |this, cx| {
                            this.stop_hover_timer(cx);
                        });
                    }
                }
            })
            .child(
                div()
                    .w_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .h(px(thumb_height))
                    .child(visual_el),
            )
            .child(
                div()
                    .w_full()
                    .px(px(6.0))
                    .mt(px(3.0))
                    .text_center()
                    .line_clamp(2)
                    .overflow_hidden()
                    .text_size(px(text_size))
                    .font_family(UiConstants::FONT_MAIN)
                    .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                    .child(if renaming_index == Some(index) {
                        div()
                            .bg(rgba(0xFFFFFF20u32))
                            .border(px(1.0))
                            .border_color(rgba(UiConstants::ACCENT_RGBA))
                            .rounded(px(2.0))
                            .px(px(4.0))
                            .child(rename_text)
                            .into_any_element()
                    } else {
                        div()
                            .child(shorten_filename_middle(&raw_name, 42))
                            .into_any_element()
                    }),
            )
            .child(
                div()
                    .w_full()
                    .px(px(6.0))
                    .text_center()
                    .text_size(px((text_size - 2.0).max(9.5)))
                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                    .child(subtitle),
            )
    }
}

/// Shorten a filename with middle ellipsis so the first characters
/// and the extension remain visible, e.g. "long_docu...nt.pdf"
pub fn shorten_filename_middle(name: &str, max_len: usize) -> String {
    if name.chars().count() <= max_len {
        return name.to_string();
    }
    if let Some(dot) = name.rfind('.') {
        let base = &name[..dot];
        let ext = &name[dot..];
        let ext_visible = ext.len().min(8);
        let base_max = max_len.saturating_sub(3 + ext_visible);
        if base_max > 0 && base.chars().count() > base_max {
            let prefix: String = base.chars().take(base_max).collect();
            format!("{}...{}", prefix, ext)
        } else {
            let prefix: String = name.chars().take(max_len.saturating_sub(3)).collect();
            format!("{}...", prefix)
        }
    } else {
        let prefix: String = name.chars().take(max_len.saturating_sub(3)).collect();
        format!("{}...", prefix)
    }
}

/// Layout parameters computed from zoom level
#[derive(Clone, Copy)]
pub struct GridLayoutParams {
    pub items_per_row: usize,
    pub item_width: f32,
    pub item_height: f32,
    pub row_height: f32,
    pub icon_size: f32,
    pub gap: f32,
    pub rounded: f32,
    pub text_size: f32,
    pub padding: f32,
}

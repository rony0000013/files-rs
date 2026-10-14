//! Grid View - Icon/grid/tile display with dynamic zoom and uniform_list for performance
//!
//! This view consolidates what were previously separate grid, cards, and tiles views
//! into a single view with dynamic zoom level (0.0 = compact tiles, 0.5 = medium grid,
//! 1.0 = large cards). Uses GPUI's uniform_list for efficient rendering.

use crate::config::config::GroupColumn;
use crate::storage::storage::FileType;
use crate::ui::components::icon_renderer::is_thumbnail_image;
use crate::ui::constants::UiConstants;
use crate::ui::views::main_view::{
    get_extension, get_file_type_icon, FileItems, GroupedGridItem, MainViewState, SelectionMode,
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
        let visible = self.visible_items_rc();

        if self.group_by != GroupColumn::None {
            return self
                .render_grid_view_grouped(&visible, file_items, view, cols, pane_w)
                .into_any_element();
        }

        self.grid_view_flat(&visible, file_items, view, cols, pane_w)
            .into_any_element()
    }

    /// Compute grid layout parameters from zoom_level
    fn grid_layout_params(zoom: f32) -> GridLayoutParams {
        // zoom: 0.0 = compact tiles, 0.5 = medium grid, 1.0 = large cards
        let items_per_row = if zoom < 0.33 {
            8
        } else if zoom < 0.66 {
            6
        } else {
            4
        };

        // Tile size: small (compact) to large (cards)
        // Files Community style: card has landscape thumbnail area + filename below
        let item_width = 80.0 + zoom * 140.0; // 80 → 220
        let thumb_height = item_width * 0.76; // landscape thumbnail area
        let item_height = thumb_height + 46.0; // thumbnail + space for 2-line filename
        let row_height = item_height + 8.0;
        // Thumbnail icon fits inside the thumbnail container
        let icon_size = thumb_height * 0.80;
        let gap = if zoom < 0.33 {
            6.0
        } else if zoom < 0.66 {
            10.0
        } else {
            14.0
        };
        let rounded = if zoom < 0.33 {
            3.0
        } else if zoom < 0.66 {
            6.0
        } else {
            10.0
        };
        let text_size = (item_width / 8.5).clamp(11.0, 13.0);
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
    /// Falls back to the zoom default when the width is unknown/tiny.
    pub fn grid_columns_for_width(pane_w: f32, zoom: f32) -> usize {
        let params = Self::grid_layout_params(zoom);
        let slot = params.item_width + params.gap;
        if pane_w <= slot || slot <= 0.0 {
            return params.items_per_row;
        }
        let avail = pane_w - params.padding * 2.0;
        (((avail + params.gap) / slot).floor() as usize).clamp(1, 32)
    }

    /// Width-fitted layout params: starts from the zoom defaults, then
    /// stretches tiles (up to 1.35x base width) to absorb leftover fractional
    /// column slack so full rows align flush with both margins, matching
    /// Files-community's ItemsWrapGrid behavior.
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
        let stretched_w =
            ((avail - total_gaps) / cols as f32).floor().clamp(base.item_width, base.item_width * 1.35);
        let thumb_height = stretched_w * 0.76;
        let item_height = thumb_height + 46.0;
        GridLayoutParams {
            items_per_row: cols,
            item_width: stretched_w,
            item_height,
            row_height: item_height + 8.0,
            icon_size: thumb_height * 0.80,
            text_size: (stretched_w / 8.5).clamp(11.0, 13.0),
            ..base
        }
    }

    /// Flat grid view using uniform_list for maximum performance
    fn grid_view_flat(
        &self,
        visible: &[usize],
        file_items: Entity<FileItems>,
        view: Entity<crate::app::AppWindow>,
        cols: usize,
        pane_w: f32,
    ) -> impl IntoElement {
        let visible_clone = self.visible_items_rc();
        let zoom = self.zoom_level;
        let params = Self::grid_layout_params_for_width(pane_w, zoom, cols);
        let items_per_row = params.items_per_row;
        let row_count = (visible.len() + items_per_row - 1) / items_per_row;

        div()
            .flex_col()
            .w_full()
            .flex_1()
            // NO overflow_hidden() - would block child scroll events
            .id("grid-container")
            .on_mouse_down(MouseButton::Right, {
                let v = view.clone();
                move |event, _, cx| {
                    v.update(cx, |this, cx| {
                        this.show_context_menu(None, event.position, cx);
                    });
                }
            })
            .child(
                uniform_list("grid-scroll", row_count, {
                    let view = view.clone();
                    move |range, _win, cx| {
                        let fi = file_items.read(cx);
                        let app_state = view.read(cx);
                        let main_state = &app_state.state.main_view;
                        let icon_paths = &main_state.icon_paths;
                        let zoom = main_state.zoom_level;
                        // Reuse the outer width-fitted params (same cols as row_count).
                        let params = params.clone();
                        let selected = main_state.selected_indices.clone();
                        let sel_mode = main_state.selection_mode.clone();
                        let compact = main_state.compact_mode;

                        range
                            .map(|row_idx| {
                                let start = row_idx * params.items_per_row;
                                let end = (start + params.items_per_row).min(visible_clone.len());
                                let items_in_row = end - start;

                                // Collect real grid items + add invisible fillers so every row has
                                // the same number of children — prevents last-row flex-grow stretch
                                let mut row_children: Vec<AnyElement> = (start..end)
                                    .map(|i| {
                                        let idx = visible_clone[i];
                                        let item = &fi.items[idx];
                                        let is_selected = selected.contains(&idx);
                                        Self::grid_item(
                                            item,
                                            idx,
                                            is_selected,
                                            sel_mode.clone(),
                                            view.clone(),
                                            icon_paths,
                                            &params,
                                            compact,
                                            main_state.renaming_index,
                                            main_state.rename_text.clone(),
                                        )
                                        .into_any_element()
                                    })
                                    .collect();

                                // Add invisible filler items so all rows have equal children count
                                for _ in items_in_row..params.items_per_row {
                                    row_children.push(
                                        div()
                                            // Fixed width like real tiles: rows left-align
                                            // Files-style instead of stretching full width.
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
                })
                .w_full()
                .h_full()
                .track_scroll(self.uniform_list_scroll_handle.clone()),
            )
    }

    /// Grouped grid view with collapsible group headers
    fn render_grid_view_grouped(
        &self,
        visible: &[usize],
        file_items: Entity<FileItems>,
        view: Entity<crate::app::AppWindow>,
        cols: usize,
        pane_w: f32,
    ) -> impl IntoElement {
        let zoom = self.zoom_level;
        let params = Self::grid_layout_params_for_width(pane_w, zoom, cols);
        let items_per_row = params.items_per_row;
        let flattened = self.flatten_grouped_grid_items(visible, items_per_row);
        let item_count = flattened.len();

        div()
            .flex_col()
            .w_full()
            .flex_1()
            // NO h_full() + overflow_hidden() - conflict with flex_grow and block scroll
            .child(
                uniform_list("grid-grouped-scroll", item_count, {
                    let view = view.clone();
                    let visible_clone = self.visible_items_rc();
                    move |range, _win, cx| {
                        let fi = file_items.read(cx);
                        let app_state = view.read(cx);
                        let main_state = &app_state.state.main_view;
                        let icon_paths = &main_state.icon_paths;
                        let zoom = main_state.zoom_level;
                        // Reuse the outer width-fitted params (same cols as row_count).
                        let params = params.clone();
                        let sel_mode = main_state.selection_mode.clone();
                        let collapsed_groups = main_state.collapsed_groups.clone();
                        let compact = main_state.compact_mode;

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
                                        .bg(rgba(0x0A000000u32))
                                        .cursor_pointer()
                                        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                            v.update(cx, |this, cx| {
                                                this.toggle_group_collapse(key_clone.clone(), cx);
                                            });
                                        })
                                        .child(
                                            div()
                                                .text_size(px(12.0))
                                                .font_family(UiConstants::FONT_MDL2)
                                                .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
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
                                                .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                                                .child(label.clone()),
                                        )
                                        .child(
                                            div()
                                                .ml(px(8.0))
                                                .text_size(px(11.0))
                                                .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
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

                                    // Collect real grid items + add invisible fillers so every row has
                                    // the same number of children — prevents last-row flex-grow stretch
                                    let mut row_children: Vec<AnyElement> = indices
                                        .iter()
                                        .map(|idx| {
                                            let item = &fi.items[*idx];
                                            let selected = fi.selected_indices.contains(idx);
                                            Self::grid_item(
                                                item,
                                                *idx,
                                                selected,
                                                sel_mode.clone(),
                                                view.clone(),
                                                icon_paths,
                                                &params,
                                                compact,
                                                main_state.renaming_index,
                                                main_state.rename_text.clone(),
                                            )
                                            .into_any_element()
                                        })
                                        .collect();

                                    // Add invisible filler items so all rows have equal children count
                                    for _ in items_in_row..params.items_per_row {
                                        row_children.push(
                                            div()
                                                // Fixed width like real tiles: rows left-align
                                                // Files-style instead of stretching full width.
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
                })
                .w_full()
                .h_full()
                .track_scroll(self.uniform_list_scroll_handle.clone()),
            )
    }

    /// Render a single grid item (icon + name), sized dynamically by zoom
    fn grid_item(
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
        renaming_index: Option<usize>,
        rename_text: String,
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
        let item_name = item.name.clone();
        let cached_icon_path: Option<std::path::PathBuf> = icon_paths.get(&item_path).cloned();

        let item_width = params.item_width;
        let thumb_height = item_width * 0.85;
        let item_height = if compact {
            params.item_height - 8.0
        } else {
            params.item_height
        };
        let icon_size_val = params.icon_size;
        let rounded = params.rounded;
        let text_size = params.text_size;

        if is_thumbnail {
            // Show image thumbnail with optional checkbox
            let mut item_div = div()
                .id(ElementId::from(70000 + index))
            // Fixed-width tiles (Files-style): box hugs the thumbnail
                // instead of stretching across the row.
                .w(px(item_width))
                .flex_shrink_0()
                .h(px(item_height))
                .flex_col()
                .items_center()
                .py(px(7.0))
                .rounded(px(rounded))
                .relative()
                .border(px(if selected { 2.0 } else { 1.5 }))
                .border_color(if selected {
                    rgba(UiConstants::ACCENT_RGBA)
                } else {
                    rgba(0x00000000u32)
                })
                .bg(rgba(0x00000000u32))
                .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)));

            if show_checkbox {
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
                            v_checkbox.update(cx, |this, cx| {
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
            item_div
                .on_mouse_down(MouseButton::Left, move |event, _, cx| {
                    cx.stop_propagation();
                    v_click.update(cx, |this, cx| {
                        if event.click_count == 2 {
                            if is_dir {
                                this.navigate(path_clone.clone(), cx);
                            } else {
                                this.open_item(index, cx);
                            }
                        } else {
                            this.select_item(index, false, false, cx);
                        }
                    });
                })
                .on_mouse_down(MouseButton::Right, {
                    let v = view.clone();
                    move |event, _, cx| {
                        cx.stop_propagation();
                        v.update(cx, |this, cx| {
                            this.show_context_menu(Some(index), event.position, cx);
                        });
                    }
                })
                .on_mouse_move({
                    let v_hover = v_hover.clone();
                    let v_timer = v_timer.clone();
                    move |ev, _, cx| {
                        if is_hover_select {
                            v_hover.update(cx, |this, cx| {
                                this.handle_item_hover(index, cx);
                            });
                        } else {
                            v_timer.update(cx, |this, cx| {
                                this.start_hover_timer(index, ev.position, cx);
                            });
                        }
                    }
                })
                .on_mouse_up_out(MouseButton::Left, {
                    let v_timer = v_timer.clone();
                    move |_, _, cx| {
                        v_timer.update(cx, |this, cx| {
                            this.stop_hover_timer(cx);
                        });
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
                    // Thumbnail container with fixed aspect ratio
                    div()
                        .w_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .h(px(thumb_height))
                        .child(
                            img(if let Some(p) = &cached_icon_path {
                                p.clone()
                            } else {
                                item_path.as_path().to_path_buf()
                            })
                            .w(px(icon_size_val))
                            .h(px(icon_size_val))
                            .object_fit(gpui::ObjectFit::Contain)
                            .rounded(px(rounded)),
                        ),
                )
                .child(
                    // 2-line filename below the thumbnail
                    div()
                        .w_full()
                        .px(px(4.0))
                        .mt(px(3.0))
                        .text_center()
                        .line_clamp(2)
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
                                .child(rename_text.clone())
                                .into_any_element()
                        } else {
                            div().child(item_name.clone()).into_any_element()
                        }),
                )
        } else {
            // Show icon for directories and non-image files
            let file_icon = if is_dir {
                "\u{E8B7}".to_string()
            } else {
                get_file_type_icon(&extension).to_string()
            };

            // Also show file size badge when zoom > 0.5 (cards mode)
            let show_size_badge = false; // TODO: determine from state

            let mut item_div = div()
                .id(ElementId::from(70000 + index))
            // Fixed-width tiles (Files-style): box hugs the thumbnail
                // instead of stretching across the row.
                .w(px(item_width))
                .flex_shrink_0()
                .h(px(item_height))
                .flex_col()
                .items_center()
                .py(px(7.0))
                .rounded(px(rounded))
                .relative()
                .border(px(if selected { 2.0 } else { 1.5 }))
                .border_color(if selected {
                    rgba(UiConstants::ACCENT_RGBA)
                } else {
                    rgba(0x00000000u32)
                })
                .bg(rgba(0x00000000u32))
                .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)));

            if show_checkbox {
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
                            let v = v_checkbox.clone();
                            v.update(cx, |this, cx| {
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
            item_div
                .on_mouse_down(MouseButton::Left, move |event, _, cx| {
                    cx.stop_propagation();
                    v_click.update(cx, |this, cx| {
                        if event.click_count == 2 {
                            if is_dir {
                                this.navigate(path_clone.clone(), cx);
                            } else {
                                this.open_item(index, cx);
                            }
                        } else {
                            this.select_item(index, false, false, cx);
                        }
                    });
                })
                .on_mouse_down(MouseButton::Right, {
                    let v = view.clone();
                    move |event, _, cx| {
                        cx.stop_propagation();
                        v.update(cx, |this, cx| {
                            this.show_context_menu(Some(index), event.position, cx);
                        });
                    }
                })
                .on_mouse_move({
                    let v_hover = v_hover.clone();
                    let v_timer = v_timer.clone();
                    move |ev, _, cx| {
                        if is_hover_select {
                            v_hover.update(cx, |this, cx| {
                                this.handle_item_hover(index, cx);
                            });
                        } else {
                            v_timer.update(cx, |this, cx| {
                                this.start_hover_timer(index, ev.position, cx);
                            });
                        }
                    }
                })
                .on_mouse_up_out(MouseButton::Left, {
                    let v_timer = v_timer.clone();
                    move |_, _, cx| {
                        v_timer.update(cx, |this, cx| {
                            this.stop_hover_timer(cx);
                        });
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
                    // Thumbnail container — icons fill the full space, centered
                    div()
                        .w_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .h(px(thumb_height))
                        .child(if let Some(icon_path) = &cached_icon_path {
                            // Wrap img in a sized div so explicit dimensions survive
                            // type erasure (into_any_element) and flex centering works.
                            div()
                                .w(px(thumb_height))
                                .h(px(thumb_height))
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
                            // Render MDL2 font icon centered in the full container
                            div()
                                .w_full()
                                .h_full()
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(
                                    div()
                                        .text_size(px(thumb_height))
                                        .font_family(UiConstants::FONT_MDL2)
                                        .text_color(if is_dir {
                                            rgba(UiConstants::ACCENT_RGBA)
                                        } else {
                                            rgba(UiConstants::TEXT_SECONDARY_RGBA)
                                        })
                                        .child(file_icon),
                                )
                                .into_any_element()
                        }),
                )
                .child(
                    // 2-line filename below the icon
                    div()
                        .w_full()
                        .px(px(4.0))
                        .mt(px(2.0))
                        .text_center()
                        .line_clamp(2)
                        .text_size(px(text_size))
                        .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                        .child(if renaming_index == Some(index) {
                            div()
                                .bg(rgba(0xFFFFFF20u32))
                                .border(px(1.0))
                                .border_color(rgba(UiConstants::ACCENT_RGBA))
                                .rounded(px(2.0))
                                .px(px(4.0))
                                .child(rename_text.clone())
                                .into_any_element()
                        } else {
                            div().child(item_name.clone()).into_any_element()
                        }),
                )
        }
    }
}

/// Shorten a filename with middle ellipsis so the first characters
/// and the extension remain visible, e.g. "long_docu...nt.pdf"
pub fn shorten_filename_middle(name: &str, max_len: usize) -> String {
    if name.len() <= max_len {
        return name.to_string();
    }
    // Find the last extension dot
    if let Some(dot) = name.rfind('.') {
        let base = &name[..dot];
        let ext = &name[dot..]; // includes "."
        let ext_visible = if ext.len() > 8 { 8 } else { ext.len() };
        let base_max = max_len.saturating_sub(3 + ext_visible);
        if base_max > 0 && base.len() > base_max {
            format!("{}...{}", &base[..base_max], ext)
        } else {
            format!("{}...", &name[..max_len.saturating_sub(3)])
        }
    } else {
        // No extension — just truncate end
        format!("{}...", &name[..max_len.saturating_sub(3)])
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

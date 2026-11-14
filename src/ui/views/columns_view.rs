//! Columns View (Miller Columns / Files-community Column View parity)
//!
//! Three-column interactive Miller Columns layout:
//! - All 3 column widths are independently drag-resizable via interactive vertical splitters
//!   (double-click any splitter to reset its column width).
//! - The bottom-right zoom/resizing slider (`zoom_level`) scales all 3 columns together
//!   (column widths, header heights, row heights, icon sizes, and font sizes).
//! - All 3 columns support full Drag & Drop (both initiating drags from Column 1, Column 2,
//!   or Column 3 and dropping files/folders onto any folder or column background).
//! - Column 2 supports `Group by` headers when grouping is active.

use crate::config::config::GroupColumn;
use crate::core::navigation_manager::PaneFocus;
use crate::core::path_util::FilesPath;
use crate::storage::storage::FileType;
use crate::ui::constants::UiConstants;
use crate::ui::views::helpers::{format_size, format_system_time, get_file_type_icon_and_color};
use crate::ui::views::main_view::{
    get_extension, FileItems, GroupedItemType, MainView, MainViewState,
};
use gpui::*;

impl MainViewState {
    pub fn render_columns_view(
        &self,
        file_items: Entity<FileItems>,
        view: Entity<crate::app::AppWindow>,
    ) -> AnyElement {
        self.render_columns_view_for_pane(
            file_items,
            view,
            PaneFocus::Primary,
            gpui::point(220.0, 160.0),
            1000.0,
        )
    }

    pub fn render_columns_view_for_pane(
        &self,
        file_items: Entity<FileItems>,
        view: Entity<crate::app::AppWindow>,
        pane_focus: PaneFocus,
        col2_origin: Point<f32>,
        pane_w: f32,
    ) -> AnyElement {
        let visible = self.visible_items_rc();
        self.columns_view_interactive(
            &visible,
            file_items,
            view,
            pane_focus,
            col2_origin,
            pane_w,
        )
        .into_any_element()
    }

    fn render_miller_splitter(
        col_id: &'static str,
        default_width: f32,
        pane_focus: PaneFocus,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let v_down = view;
        div()
            .w(px(6.0))
            .h_full()
            .flex_shrink_0()
            .cursor_col_resize()
            .flex()
            .items_center()
            .justify_center()
            .bg(rgba(0x00000000u32))
            .hover(|s| s.bg(rgba(0x60CDFF33u32)))
            .on_mouse_down(MouseButton::Left, move |ev, win, cx| {
                cx.stop_propagation();
                let start_x: f32 = win.mouse_position().x.into();
                v_down.update(cx, |this, cx| {
                    this.focus_pane(pane_focus, cx);
                    if ev.click_count == 2 {
                        let pv = this.state.pane_view_mut(pane_focus);
                        match col_id {
                            "miller_col1" => pv.column_widths.miller_col1 = default_width,
                            "miller_col2" => pv.column_widths.miller_col2 = default_width,
                            "miller_col3" => pv.column_widths.miller_col3 = default_width,
                            _ => {}
                        }
                        match col_id {
                            "miller_col1" => {
                                this.state.main_view.column_widths.miller_col1 = default_width
                            }
                            "miller_col2" => {
                                this.state.main_view.column_widths.miller_col2 = default_width
                            }
                            "miller_col3" => {
                                this.state.main_view.column_widths.miller_col3 = default_width
                            }
                            _ => {}
                        }
                        this.end_column_resize();
                        cx.notify();
                    } else {
                        this.start_column_resize(col_id.to_string(), start_x, cx);
                    }
                });
            })
            .child(
                div()
                    .w(px(1.0))
                    .h_full()
                    .bg(rgba(UiConstants::DIVIDER_RGBA)),
            )
    }

    #[allow(clippy::too_many_arguments)]
    fn render_col2_row(
        item: &crate::domain::metadata::FileItem,
        idx: usize,
        is_selected: bool,
        is_drop_target: bool,
        r_h: f32,
        i_size: f32,
        t_size: f32,
        line_h: f32,
        main_state: &MainViewState,
        icon_paths: &std::collections::HashMap<FilesPath, std::path::PathBuf>,
        pane_focus: PaneFocus,
        pane_w: f32,
        view: Entity<crate::app::AppWindow>,
    ) -> AnyElement {
        let is_dir = matches!(item.file_type, FileType::Directory);
        let is_cut = main_state.cut_paths.contains(item.path.as_path());
        let display_name = main_state.display_name(item).into_owned();
        let ext = get_extension(&item.name);
        let (fallback_glyph, fallback_color) = if is_dir {
            ("\u{E8B7}", 0xF5B942u32)
        } else {
            get_file_type_icon_and_color(&ext)
        };
        let path = item.path.clone();
        let item_name_for_drag = item.name.clone();
        let cur_dir_path = main_state.primary_path.clone();
        let cur_dir_lbl = cur_dir_path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| cur_dir_path.to_string_lossy().into_owned());
        let v = view;

        let mut row = div()
            .id(ElementId::from(
                idx + match pane_focus {
                    PaneFocus::Primary => 50000,
                    PaneFocus::Secondary => 150000,
                },
            ))
            .flex()
            .flex_row()
            .flex_shrink_0()
            .overflow_hidden()
            .h(px((r_h - 2.0).max(20.0)))
            .px(px(12.0))
            .items_center()
            .gap(px(6.0))
            .rounded(px(4.0))
            .mx(px(8.0))
            .border(px(1.0))
            .border_color(if is_drop_target {
                rgba(0x60CDFFFFu32)
            } else {
                rgba(0x00000000u32)
            })
            .bg(if is_drop_target {
                rgba(0x0078D444u32)
            } else if is_selected {
                rgba(UiConstants::BG_ITEM_SELECTED_RGBA)
            } else {
                rgba(0x00000000)
            })
            .hover(|s| {
                if is_selected {
                    s.bg(rgba(UiConstants::BG_ITEM_SELECTED_HOVER_RGBA))
                } else {
                    s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA))
                }
            })
            .cursor_pointer();

        if is_cut {
            row = row
                .opacity(0.55)
                .border(px(1.0))
                .border_dashed()
                .border_color(rgba(0x60CDFFCCu32));
        }

        row
            .on_mouse_down(MouseButton::Left, {
                let v = v.clone();
                let path = path.clone();
                move |ev, _, cx| {
                    cx.stop_propagation();
                    let pos = point(ev.position.x.into(), ev.position.y.into());
                    v.update(cx, |this, cx| {
                        this.handle_item_mouse_down(
                            pane_focus,
                            idx,
                            path.clone(),
                            is_dir,
                            ev.click_count,
                            pos,
                            ev.modifiers,
                            cx,
                        );
                    });
                }
            })
            .on_mouse_move({
                let v = v.clone();
                let p_drop = path.clone();
                let n_drop = item_name_for_drag;
                let fallback_dir = cur_dir_path;
                let fallback_lbl = cur_dir_lbl;
                move |ev, _win, cx| {
                    let pos = point(ev.position.x.into(), ev.position.y.into());
                    v.update(cx, |this, cx| {
                        if this.state.main_view.drag_state.is_dragging
                            || this.state.main_view.drag_state.pending_candidate.is_some()
                            || this
                                .state
                                .main_view
                                .drag_state
                                .pending_path_candidate
                                .is_some()
                        {
                            let folder_target = if is_dir {
                                Some((p_drop.clone(), n_drop.clone()))
                            } else if this
                                .state
                                .main_view
                                .drag_state
                                .paths
                                .first()
                                .and_then(|p| p.parent())
                                .as_ref()
                                != Some(&fallback_dir)
                            {
                                Some((fallback_dir.clone(), fallback_lbl.clone()))
                            } else {
                                None
                            };
                            this.update_file_drag_move(pos, ev.modifiers, folder_target, cx);
                        } else if this.state.pane_view(pane_focus).lasso_state.is_some() {
                            this.update_lasso_in_pane(pane_focus, pos, pane_w, cx);
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
                            this.select_item_in_pane(pane_focus, idx, false, false, cx);
                        }
                        this.show_context_menu(Some(idx), ev.position, window, cx);
                    });
                }
            })
            .child(if is_selected {
                div()
                    .w(px(3.0))
                    .h(px((r_h * 0.52).clamp(12.0, 20.0)))
                    .flex_shrink_0()
                    .rounded(px(1.5))
                    .bg(rgb(UiConstants::ACCENT_COLOR_RGB))
            } else {
                div()
            })
            .child(
                div()
                    .w(px(i_size + 4.0))
                    .h(px(i_size + 4.0))
                    .p(px(2.0))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(if let Some(icon_path) = icon_paths.get(&path) {
                        img(icon_path.clone())
                            .w(px(i_size))
                            .h(px(i_size))
                            .object_fit(gpui::ObjectFit::Contain)
                            .rounded(px(2.0))
                            .into_any_element()
                    } else {
                        div()
                            .text_size(px(i_size * 0.78))
                            .font_family(UiConstants::FONT_MDL2)
                            .text_color(rgb(fallback_color))
                            .child(fallback_glyph)
                            .into_any_element()
                    }),
            )
            .child(if main_state.renaming_index == Some(idx) {
                crate::ui::views::helpers::render_inline_rename_single(
                    &main_state.rename_input,
                    v.clone(),
                )
            } else {
                div()
                    .flex_1()
                    .min_w(px(0.0))
                    .truncate()
                    .text_size(px(t_size))
                    .line_height(px(line_h))
                    .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                    .child(display_name)
                    .into_any_element()
            })
            .child(if is_dir {
                div()
                    .flex_shrink_0()
                    .text_size(px((t_size * 0.75).clamp(8.5, 11.0)))
                    .font_family(UiConstants::FONT_MDL2)
                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                    .child("\u{E76C}")
                    .into_any_element()
            } else {
                div().into_any_element()
            })
            .into_any_element()
    }

    fn columns_view_interactive(
        &self,
        visible: &[usize],
        file_items: Entity<FileItems>,
        view: Entity<crate::app::AppWindow>,
        pane_focus: PaneFocus,
        col2_origin: Point<f32>,
        pane_w: f32,
    ) -> Div {
        let item_count = visible.len();
        let selected_item = self
            .selected_indices
            .iter()
            .next()
            .and_then(|&i| self.items.get(i))
            .cloned();

        let parent_dir = self.primary_path.parent();
        let parent_label = parent_dir
            .as_ref()
            .map(|p| {
                p.file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| p.to_string_lossy().into_owned())
            })
            .unwrap_or_else(|| "Root".to_string());

        // Canonical Files-community ColumnsViewSizeKind metrics + gentle blade scaling
        let spec = MainViewState::columns_layout_spec(self.zoom_level, self.compact_mode);
        let zoom_scale = spec.zoom_scale;
        let col1_w = (self.column_widths.miller_col1 * zoom_scale)
            .clamp(150.0, (pane_w * 0.45).max(200.0))
            .round();
        let col2_w = (self.column_widths.miller_col2 * zoom_scale)
            .clamp(160.0, (pane_w * 0.55).max(240.0))
            .round();
        let col3_w = (self.column_widths.miller_col3 * zoom_scale)
            .clamp(180.0, pane_w.max(260.0))
            .round();

        let r_h = spec.row_height;
        let i_size = spec.icon_size;
        let t_size = spec.text_size;
        let line_h = (t_size * 1.24).ceil();
        let header_h = spec.header_height;

        let v_up = view.clone();
        let parent_dir_for_up = parent_dir.clone();
        let parent_dir_for_bg = parent_dir.clone();
        let parent_lbl_for_bg = parent_label.clone();

        // Build Column 2 body (supports both flat and grouped views)
        let col2_body: AnyElement = if self.group_by != GroupColumn::None {
            let flattened = self.flatten_grouped_items(visible);
            let group_header_h = r_h.max(30.0);
            let hovered_drop = self.drag_state.hovered_target_folder.clone();
            let is_dragging_any = self.drag_state.is_dragging;
            let item_heights: Vec<f32> = flattened
                .iter()
                .map(|entry| match entry {
                    GroupedItemType::Header { .. } => group_header_h,
                    GroupedItemType::Item(_) => r_h,
                })
                .collect();

            MainView::render_virtualized_grouped_list(
                match pane_focus {
                    PaneFocus::Primary => "col2-grouped-scroll-primary",
                    PaneFocus::Secondary => "col2-grouped-scroll-secondary",
                },
                &self.uniform_list_scroll_handle,
                col2_w,
                &item_heights,
                |i| match &flattened[i] {
                    GroupedItemType::Header { key, label, count } => {
                        let is_collapsed = self.collapsed_groups.contains(key);
                        MainView::render_group_header_row(
                            i,
                            key,
                            label,
                            *count,
                            is_collapsed,
                            group_header_h,
                            8.0,
                            pane_focus,
                            view.clone(),
                        )
                    }
                    GroupedItemType::Item(idx) => {
                        let idx = *idx;
                        let Some(item) = self.items.get(idx) else {
                            return div().into_any_element();
                        };
                        let is_selected = self.selected_indices.contains(&idx);
                        let is_dir = matches!(item.file_type, FileType::Directory);
                        let is_drop_target = is_dragging_any
                            && is_dir
                            && hovered_drop.as_ref() == Some(&item.path);
                        Self::render_col2_row(
                            item,
                            idx,
                            is_selected,
                            is_drop_target,
                            r_h,
                            i_size,
                            t_size,
                            line_h,
                            self,
                            &self.icon_paths,
                            pane_focus,
                            pane_w,
                            view.clone(),
                        )
                    }
                },
            )
        } else {
            uniform_list(
                match pane_focus {
                    PaneFocus::Primary => "col2-scroll-primary",
                    PaneFocus::Secondary => "col2-scroll-secondary",
                },
                item_count,
                {
                    let view = view.clone();
                    move |range, _win, cx| {
                        let fi = file_items.read(cx);
                        let app_state = view.read(cx);
                        let main_state = app_state.state.pane_view(pane_focus);
                        let visible_rc = main_state.visible_items_rc();
                        let icon_paths = &app_state.state.main_view.icon_paths;
                        let spec = MainViewState::columns_layout_spec(
                            main_state.zoom_level,
                            main_state.compact_mode,
                        );
                        let row_h = spec.row_height;
                        let icon_sz = spec.icon_size;
                        let txt_sz = spec.text_size;
                        let ln_h = (txt_sz * 1.24).ceil();
                        let hovered_drop = app_state
                            .state
                            .main_view
                            .drag_state
                            .hovered_target_folder
                            .clone();
                        let is_dragging_any = app_state.state.main_view.drag_state.is_dragging;

                        range
                            .map(|i| {
                                if i >= visible_rc.len() {
                                    return div().into_any_element();
                                }
                                let idx = visible_rc[i];
                                if idx >= fi.items.len() {
                                    return div().into_any_element();
                                }
                                let item = &fi.items[idx];
                                let is_selected = main_state.selected_indices.contains(&idx);
                                let is_dir = matches!(item.file_type, FileType::Directory);
                                let is_drop_target = is_dragging_any
                                    && is_dir
                                    && hovered_drop.as_ref() == Some(&item.path);

                                Self::render_col2_row(
                                    item,
                                    idx,
                                    is_selected,
                                    is_drop_target,
                                    row_h,
                                    icon_sz,
                                    txt_sz,
                                    ln_h,
                                    main_state,
                                    icon_paths,
                                    pane_focus,
                                    pane_w,
                                    view.clone(),
                                )
                            })
                            .collect::<Vec<_>>()
                    }
                },
            )
            .with_sizing_behavior(ListSizingBehavior::Auto)
            .w_full()
            .min_h(px(0.0))
            .h_full()
            .track_scroll(self.uniform_list_scroll_handle.clone())
            .into_any_element()
        };

        div()
            .flex()
            .flex_row()
            .flex_grow()
            .w_full()
            .h_full()
            .overflow_hidden()
            // ── Column 1: Parent directory Miller column (resizable + zoom-scaled + draggable)
            .child(
                div()
                    .w(px(col1_w))
                    .flex_shrink_0()
                    .h_full()
                    .flex_col()
                    .overflow_hidden()
                    .on_mouse_move({
                        let v = view.clone();
                        let p_bg = parent_dir_for_bg.clone();
                        let l_bg = parent_lbl_for_bg.clone();
                        move |ev, _win, cx| {
                            let pos = point(ev.position.x.into(), ev.position.y.into());
                            v.update(cx, |this, cx| {
                                if this.state.main_view.drag_state.is_dragging
                                    || this.state.main_view.drag_state.pending_candidate.is_some()
                                    || this
                                        .state
                                        .main_view
                                        .drag_state
                                        .pending_path_candidate
                                        .is_some()
                                {
                                    let target = p_bg.clone().map(|p| (p, l_bg.clone()));
                                    this.update_file_drag_move(pos, ev.modifiers, target, cx);
                                }
                            });
                        }
                    })
                    .on_mouse_up(MouseButton::Left, {
                        let v = view.clone();
                        move |ev, _win, cx| {
                            let pos = point(ev.position.x.into(), ev.position.y.into());
                            v.update(cx, |this, cx| {
                                if this.state.main_view.drag_state.is_dragging
                                    || this.state.main_view.drag_state.pending_candidate.is_some()
                                    || this
                                        .state
                                        .main_view
                                        .drag_state
                                        .pending_path_candidate
                                        .is_some()
                                {
                                    this.end_drag(pos.x, pos.y, None, cx);
                                }
                            });
                        }
                    })
                    .child(
                        div()
                            .h(px(header_h))
                            .px(px(10.0))
                            .flex()
                            .items_center()
                            .gap(px(6.0))
                            .border_b(px(1.0))
                            .border_color(rgba(UiConstants::DIVIDER_RGBA))
                            .cursor_pointer()
                            .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
                            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                if let Some(ref p) = parent_dir_for_up {
                                    let target = p.clone();
                                    v_up.update(cx, |this, cx| {
                                        this.focus_pane(pane_focus, cx);
                                        this.navigate_pane(target, pane_focus, cx);
                                    });
                                }
                            })
                            .child(
                                div()
                                    .text_size(px((t_size - 0.5).max(10.5)))
                                    .font_family(UiConstants::FONT_MDL2)
                                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                    .child("\u{E74A}"),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w(px(0.0))
                                    .truncate()
                                    .text_size(px((t_size - 0.5).max(10.5)))
                                    .line_height(px(line_h))
                                    .font_weight(FontWeight(600.0))
                                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                    .child(parent_label),
                            ),
                    )
                    .child({
                        let siblings = self.col1_siblings.clone();
                        let n_sib = siblings.len();
                        let col1_handle = self.col1_scroll_handle.clone();
                        let col1_handle_w = self.col1_scroll_handle.clone();
                        let v_wheel = view.clone();
                        uniform_list(
                            match pane_focus {
                                PaneFocus::Primary => "col1-scroll-primary",
                                PaneFocus::Secondary => "col1-scroll-secondary",
                            },
                            n_sib,
                            {
                                let view = view.clone();
                                move |range, _win, cx| {
                                    let app_state = view.read(cx);
                                    let main_state = app_state.state.pane_view(pane_focus);
                                    let spec = MainViewState::columns_layout_spec(
                                        main_state.zoom_level,
                                        main_state.compact_mode,
                                    );
                                    let row_h = spec.row_height;
                                    let icon_sz = spec.icon_size;
                                    let txt_sz = spec.text_size;
                                    let ln_h = (txt_sz * 1.24).ceil();
                                    let hovered_drop = app_state
                                        .state
                                        .main_view
                                        .drag_state
                                        .hovered_target_folder
                                        .clone();
                                    let is_dragging_any =
                                        app_state.state.main_view.drag_state.is_dragging;
                                    range
                                        .map(|i| {
                                            let (name, sib_path, is_curr) = siblings
                                                .get(i)
                                                .cloned()
                                                .unwrap_or_else(|| {
                                                    (String::new(), FilesPath::new(""), false)
                                                });
                                            let is_drop_target = is_dragging_any
                                                && hovered_drop.as_ref() == Some(&sib_path);
                                            let v_down = view.clone();
                                            let v_move = view.clone();
                                            let v_up_drop = view.clone();
                                            let sib_for_down = sib_path.clone();
                                            let sib_for_move = sib_path.clone();
                                            let sib_for_up = sib_path;
                                            let name_for_move = name.clone();
                                            div()
                                                .h(px(row_h))
                                                .px(px(8.0))
                                                .flex()
                                                .flex_shrink_0()
                                                .overflow_hidden()
                                                .items_center()
                                                .gap(px(6.0))
                                                .rounded(px(4.0))
                                                .cursor_pointer()
                                                .border(px(1.0))
                                                .border_color(if is_drop_target {
                                                    rgba(0x60CDFFFFu32)
                                                } else {
                                                    rgba(0x00000000u32)
                                                })
                                                .bg(if is_drop_target {
                                                    rgba(0x0078D444u32)
                                                } else if is_curr {
                                                    rgba(UiConstants::BG_ITEM_SELECTED_RGBA)
                                                } else {
                                                    rgba(0x00000000)
                                                })
                                                .hover(|s| {
                                                    if is_curr {
                                                        s.bg(rgba(
                                                            UiConstants::BG_ITEM_SELECTED_HOVER_RGBA,
                                                        ))
                                                    } else {
                                                        s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA))
                                                    }
                                                })
                                                .on_mouse_down(
                                                    MouseButton::Left,
                                                    move |ev, _, cx| {
                                                        cx.stop_propagation();
                                                        let pos = point(
                                                            ev.position.x.into(),
                                                            ev.position.y.into(),
                                                        );
                                                        let target = sib_for_down.clone();
                                                        v_down.update(cx, |this, cx| {
                                                            this.focus_pane(pane_focus, cx);
                                                            this.state
                                                                .main_view
                                                                .record_path_drag_candidate(
                                                                    pane_focus, target, pos,
                                                                );
                                                        });
                                                    },
                                                )
                                                .on_mouse_move(move |ev, _win, cx| {
                                                    let pos = point(
                                                        ev.position.x.into(),
                                                        ev.position.y.into(),
                                                    );
                                                    v_move.update(cx, |this, cx| {
                                                        if this.state.main_view.drag_state.is_dragging
                                                            || this
                                                                .state
                                                                .main_view
                                                                .drag_state
                                                                .pending_candidate
                                                                .is_some()
                                                            || this
                                                                .state
                                                                .main_view
                                                                .drag_state
                                                                .pending_path_candidate
                                                                .is_some()
                                                        {
                                                            this.update_file_drag_move(
                                                                pos,
                                                                ev.modifiers,
                                                                Some((
                                                                    sib_for_move.clone(),
                                                                    name_for_move.clone(),
                                                                )),
                                                                cx,
                                                            );
                                                        }
                                                    });
                                                })
                                                .on_mouse_up(
                                                    MouseButton::Left,
                                                    move |ev, _, cx| {
                                                        cx.stop_propagation();
                                                        let pos = point(
                                                            ev.position.x.into(),
                                                            ev.position.y.into(),
                                                        );
                                                        let target = sib_for_up.clone();
                                                        v_up_drop.update(cx, |this, cx| {
                                                            if this
                                                                .state
                                                                .main_view
                                                                .drag_state
                                                                .is_dragging
                                                            {
                                                                this.end_drag(
                                                                    pos.x, pos.y, None, cx,
                                                                );
                                                            } else {
                                                                this.state
                                                                    .main_view
                                                                    .drag_state
                                                                    .pending_path_candidate = None;
                                                                this.navigate_pane(
                                                                    target, pane_focus, cx,
                                                                );
                                                            }
                                                        });
                                                    },
                                                )
                                                .child(if is_curr {
                                                    div()
                                                        .w(px(3.0))
                                                        .h(px((row_h * 0.52).clamp(12.0, 20.0)))
                                                        .flex_shrink_0()
                                                        .rounded(px(1.5))
                                                        .bg(rgb(UiConstants::ACCENT_COLOR_RGB))
                                                } else {
                                                    div()
                                                })
                                                .child(
                                                    div()
                                                        .w(px(icon_sz))
                                                        .h(px(icon_sz))
                                                        .flex_shrink_0()
                                                        .flex()
                                                        .items_center()
                                                        .justify_center()
                                                        .child(
                                                            div()
                                                                .text_size(px(icon_sz * 0.72))
                                                                .font_family(UiConstants::FONT_MDL2)
                                                                .text_color(rgb(0xF5B942))
                                                                .child("\u{E8B7}"),
                                                        ),
                                                )
                                                .child(
                                                    div()
                                                        .flex_1()
                                                        .min_w(px(0.0))
                                                        .truncate()
                                                        .text_size(px(txt_sz))
                                                        .line_height(px(ln_h))
                                                        .text_color(rgba(
                                                            UiConstants::TEXT_PRIMARY_RGBA,
                                                        ))
                                                        .child(name),
                                                )
                                                .child(
                                                    div()
                                                        .flex_shrink_0()
                                                        .text_size(px(
                                                            (txt_sz * 0.75).clamp(8.5, 11.0),
                                                        ))
                                                        .font_family(UiConstants::FONT_MDL2)
                                                        .text_color(rgba(
                                                            UiConstants::TEXT_SECONDARY_RGBA,
                                                        ))
                                                        .child("\u{E76C}"),
                                                )
                                                .into_any_element()
                                        })
                                        .collect::<Vec<gpui::AnyElement>>()
                                }
                            },
                        )
                        .min_h(px(0.0))
                        .h_full()
                        .p(px(4.0))
                        .track_scroll(col1_handle)
                        .on_scroll_wheel(move |event, _win, cx| {
                            if event.modifiers.control {
                                return;
                            }
                            cx.stop_propagation();
                            let delta = event.delta.pixel_delta(px(20.0));
                            let d: f32 = delta.y.into();
                            crate::ui::components::bouncy_scroll::wheel_scroll_uniform(
                                &v_wheel,
                                &col1_handle_w,
                                d,
                                cx,
                            );
                        })
                    }),
            )
            // ── Splitter 1: Resizes Column 1
            .child(Self::render_miller_splitter(
                "miller_col1",
                220.0,
                pane_focus,
                view.clone(),
            ))
            // ── Column 2: Current directory contents (resizable + zoom-scaled + draggable)
            .child(
                div()
                    .w(px(col2_w))
                    .flex_shrink_0()
                    .h_full()
                    .flex_col()
                    .overflow_hidden()
                    .child(
                        div()
                            .h(px(header_h))
                            .px(px(12.0))
                            .flex()
                            .items_center()
                            .justify_between()
                            .border_b(px(1.0))
                            .border_color(rgba(UiConstants::DIVIDER_RGBA))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w(px(0.0))
                                    .truncate()
                                    .text_size(px((t_size - 0.5).max(10.5)))
                                    .line_height(px(line_h))
                                    .font_weight(FontWeight(600.0))
                                    .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                                    .child(
                                        self.primary_path
                                            .file_name()
                                            .map(|n| n.to_string_lossy().into_owned())
                                            .filter(|s| !s.is_empty())
                                            .unwrap_or_else(|| {
                                                self.primary_path.to_string_lossy().into_owned()
                                            }),
                                    ),
                            )
                            .child(
                                div()
                                    .flex_shrink_0()
                                    .text_size(px((t_size - 1.0).max(10.0)))
                                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                    .child(format!("{} items", visible.len())),
                            ),
                    )
                    .child(
                        div()
                            .id(SharedString::from(match pane_focus {
                                PaneFocus::Primary => "col2-container-primary",
                                PaneFocus::Secondary => "col2-container-secondary",
                            }))
                            .relative()
                            .flex_1()
                            .w_full()
                            .h_full()
                            .overflow_hidden()
                            .on_mouse_down(MouseButton::Right, {
                                let v = view.clone();
                                move |event, window, cx| {
                                    v.update(cx, |this, cx| {
                                        this.focus_pane(pane_focus, cx);
                                        this.commit_rename_if_active(cx);
                                        this.clear_selection(cx);
                                        this.show_context_menu(None, event.position, window, cx);
                                    });
                                }
                            })
                            .on_mouse_down(MouseButton::Left, {
                                let v = view.clone();
                                move |event, _win, cx| {
                                    let pos =
                                        point(event.position.x.into(), event.position.y.into());
                                    let additive =
                                        event.modifiers.control || event.modifiers.shift;
                                    v.update(cx, |this, cx| {
                                        this.focus_pane(pane_focus, cx);
                                        this.commit_rename_if_active(cx);
                                        this.start_lasso_in_pane(
                                            pane_focus,
                                            pos,
                                            col2_origin,
                                            additive,
                                            cx,
                                        );
                                    });
                                }
                            })
                            .on_mouse_move({
                                let v = view.clone();
                                move |event, _win, cx| {
                                    let pos =
                                        point(event.position.x.into(), event.position.y.into());
                                    v.update(cx, |this, cx| {
                                        if this.state.main_view.drag_state.is_dragging
                                            || this
                                                .state
                                                .main_view
                                                .drag_state
                                                .pending_candidate
                                                .is_some()
                                            || this
                                                .state
                                                .main_view
                                                .drag_state
                                                .pending_path_candidate
                                                .is_some()
                                        {
                                            let dest = this
                                                .state
                                                .pane_view(pane_focus)
                                                .primary_path
                                                .clone();
                                            let pane_target = if this
                                                .state
                                                .main_view
                                                .drag_state
                                                .paths
                                                .first()
                                                .and_then(|p| p.parent())
                                                .as_ref()
                                                != Some(&dest)
                                            {
                                                let lbl = dest
                                                    .file_name()
                                                    .map(|n| n.to_string_lossy().into_owned())
                                                    .unwrap_or_else(|| {
                                                        dest.to_string_lossy().into_owned()
                                                    });
                                                Some((dest, lbl))
                                            } else {
                                                None
                                            };
                                            this.update_file_drag_move(
                                                pos,
                                                event.modifiers,
                                                pane_target,
                                                cx,
                                            );
                                        } else {
                                            this.update_lasso_in_pane(pane_focus, pos, pane_w, cx);
                                        }
                                    });
                                }
                            })
                            .on_mouse_up(MouseButton::Left, {
                                let v = view.clone();
                                move |event, _win, cx| {
                                    let pos =
                                        point(event.position.x.into(), event.position.y.into());
                                    v.update(cx, |this, cx| {
                                        if this.state.main_view.drag_state.is_dragging
                                            || this
                                                .state
                                                .main_view
                                                .drag_state
                                                .pending_candidate
                                                .is_some()
                                            || this
                                                .state
                                                .main_view
                                                .drag_state
                                                .pending_path_candidate
                                                .is_some()
                                        {
                                            this.end_drag(pos.x, pos.y, None, cx);
                                        }
                                        this.end_lasso_in_pane(pane_focus, cx);
                                    });
                                }
                            })
                            .on_mouse_up_out(MouseButton::Left, {
                                let v = view.clone();
                                move |event, _win, cx| {
                                    let pos =
                                        point(event.position.x.into(), event.position.y.into());
                                    v.update(cx, |this, cx| {
                                        if this.state.main_view.drag_state.is_dragging
                                            || this
                                                .state
                                                .main_view
                                                .drag_state
                                                .pending_candidate
                                                .is_some()
                                            || this
                                                .state
                                                .main_view
                                                .drag_state
                                                .pending_path_candidate
                                                .is_some()
                                        {
                                            this.end_drag(pos.x, pos.y, None, cx);
                                        }
                                        this.end_lasso_in_pane(pane_focus, cx);
                                    });
                                }
                            })
                            .child(col2_body)
                            .child(MainView::render_lasso_box(self)),
                    ),
            )
            // ── Splitter 2: Resizes Column 2
            .child(Self::render_miller_splitter(
                "miller_col2",
                340.0,
                pane_focus,
                view.clone(),
            ))
            // ── Column 3: Child Directory Miller Column OR File Inspector Preview (resizable + zoom-scaled + draggable)
            .child(
                div()
                    .w(px(col3_w))
                    .flex_1()
                    .min_w(px(180.0))
                    .h_full()
                    .flex_col()
                    .overflow_hidden()
                    .bg(rgba(0xFFFFFF04u32))
                    .child(match selected_item {
                        None => div()
                            .flex_grow()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                div()
                                    .text_size(px(t_size))
                                    .text_color(rgba(0xFFFFFF40u32))
                                    .child(if self.selected_indices.len() > 1 {
                                        format!("{} items selected", self.selected_indices.len())
                                    } else {
                                        "Select a file or folder to preview".to_string()
                                    }),
                            )
                            .into_any_element(),
                        Some(ref item) if matches!(item.file_type, FileType::Directory) => {
                            Self::render_column3_directory_preview(
                                item,
                                self.col3_children.clone(),
                                view.clone(),
                                pane_focus,
                                self.col3_scroll_handle.clone(),
                                r_h,
                                i_size,
                                t_size,
                                line_h,
                                header_h,
                            )
                            .into_any_element()
                        }
                        Some(ref item) => div()
                            .id("miller-col3-file-preview")
                            .flex_grow()
                            .overflow_y_scroll()
                            .p(px(16.0))
                            .child(MainView::render_details_pane(
                                self,
                                item,
                                view.clone(),
                            ))
                            .into_any_element(),
                    }),
            )
            // ── Splitter 3: Resizes Column 3
            .child(Self::render_miller_splitter(
                "miller_col3",
                380.0,
                pane_focus,
                view,
            ))
    }

    /// Refresh the Miller-column caches (Column 1 siblings, Column 3 child preview).
    pub fn refresh_columns_caches(&mut self) {
        if !matches!(self.view_mode, crate::config::config::ViewMode::Columns) {
            return;
        }
        let show_dot = self.show_dot_files;
        let show_hidden = self.show_hidden_files;
        let parent = self.primary_path.parent();
        if self.col1_siblings_for != parent {
            let siblings: Vec<(String, FilesPath, bool)> = parent
                .as_ref()
                .and_then(|p| std::fs::read_dir(p.as_path()).ok())
                .map(|rd| {
                    let mut dirs: Vec<(String, FilesPath, bool)> = rd
                        .filter_map(|e| e.ok())
                        .filter(|e| {
                            e.file_type()
                                .map(|t| t.is_dir())
                                .unwrap_or_else(|_| e.path().is_dir())
                        })
                        .filter_map(|e| {
                            let name = e.file_name().to_string_lossy().into_owned();
                            let fp = FilesPath::new(e.path());
                            let is_curr = fp == self.primary_path;
                            if !is_curr && (!show_dot || !show_hidden) && name.starts_with('.') {
                                return None;
                            }
                            Some((name, fp, is_curr))
                        })
                        .collect();
                    dirs.sort_by(|a, b| crate::ui::views::helpers::natural_cmp(&a.0, &b.0));
                    dirs
                })
                .unwrap_or_else(|| {
                    let curr_name = self
                        .primary_path
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_else(|| self.primary_path.to_string_lossy().into_owned());
                    vec![(curr_name, self.primary_path.clone(), true)]
                });
            self.col1_siblings_for = parent;
            self.col1_siblings = siblings;
        }
        let sel_folder: Option<FilesPath> = if self.selected_indices.len() == 1 {
            self.selected_indices
                .iter()
                .next()
                .and_then(|&i| self.items.get(i))
                .filter(|it| matches!(it.file_type, FileType::Directory))
                .map(|it| it.path.clone())
        } else {
            None
        };
        if self.col3_children_for != sel_folder {
            let children: Vec<(String, FilesPath, bool, u64)> = sel_folder
                .as_ref()
                .and_then(|p| std::fs::read_dir(p.as_path()).ok())
                .map(|rd| {
                    let mut entries: Vec<(String, FilesPath, bool, u64)> = rd
                        .filter_map(|e| e.ok())
                        .filter_map(|e| {
                            let name = e.file_name().to_string_lossy().into_owned();
                            if (!show_dot || !show_hidden) && name.starts_with('.') {
                                return None;
                            }
                            let path_buf = e.path();
                            let is_dir = e
                                .file_type()
                                .map(|t| t.is_dir())
                                .unwrap_or_else(|_| path_buf.is_dir());
                            let fp = FilesPath::new(path_buf);
                            let sz = if is_dir {
                                crate::storage::folder_size::peek_cached_size(fp.as_path())
                                    .unwrap_or(0)
                            } else {
                                e.metadata().map(|m| m.len()).unwrap_or(0)
                            };
                            Some((name, fp, is_dir, sz))
                        })
                        .take(150)
                        .collect();
                    entries.sort_by(|a, b| {
                        b.2.cmp(&a.2)
                            .then_with(|| crate::ui::views::helpers::natural_cmp(&a.0, &b.0))
                    });
                    entries
                })
                .unwrap_or_default();
            self.col3_children_for = sel_folder;
            self.col3_children = children;
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn render_column3_directory_preview(
        folder: &crate::domain::metadata::FileItem,
        children: Vec<(String, FilesPath, bool, u64)>,
        view: Entity<crate::app::AppWindow>,
        pane_focus: PaneFocus,
        scroll_handle: UniformListScrollHandle,
        r_h: f32,
        i_size: f32,
        t_size: f32,
        line_h: f32,
        header_h: f32,
    ) -> impl IntoElement {
        let v_enter = view.clone();
        let folder_path = folder.path.clone();
        let folder_path_bg = folder.path.clone();
        let folder_name_bg = folder.name.clone();

        div()
            .flex_col()
            .w_full()
            .h_full()
            .overflow_hidden()
            .on_mouse_move({
                let v = view.clone();
                let fp_bg = folder_path_bg.clone();
                let fn_bg = folder_name_bg.clone();
                move |ev, _win, cx| {
                    let pos = point(ev.position.x.into(), ev.position.y.into());
                    v.update(cx, |this, cx| {
                        if this.state.main_view.drag_state.is_dragging
                            || this.state.main_view.drag_state.pending_candidate.is_some()
                            || this
                                .state
                                .main_view
                                .drag_state
                                .pending_path_candidate
                                .is_some()
                        {
                            let target = if this
                                .state
                                .main_view
                                .drag_state
                                .paths
                                .first()
                                .and_then(|p| p.parent())
                                .as_ref()
                                != Some(&fp_bg)
                            {
                                Some((fp_bg.clone(), fn_bg.clone()))
                            } else {
                                None
                            };
                            this.update_file_drag_move(pos, ev.modifiers, target, cx);
                        }
                    });
                }
            })
            .on_mouse_up(MouseButton::Left, {
                let v = view.clone();
                move |ev, _win, cx| {
                    let pos = point(ev.position.x.into(), ev.position.y.into());
                    v.update(cx, |this, cx| {
                        if this.state.main_view.drag_state.is_dragging
                            || this.state.main_view.drag_state.pending_candidate.is_some()
                            || this
                                .state
                                .main_view
                                .drag_state
                                .pending_path_candidate
                                .is_some()
                        {
                            this.end_drag(pos.x, pos.y, None, cx);
                        }
                    });
                }
            })
            .child(
                div()
                    .h(px(header_h))
                    .px(px(12.0))
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(8.0))
                    .border_b(px(1.0))
                    .border_color(rgba(UiConstants::DIVIDER_RGBA))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.0))
                            .truncate()
                            .text_size(px((t_size - 0.5).max(10.5)))
                            .line_height(px(line_h))
                            .font_weight(FontWeight(600.0))
                            .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                            .child(folder.name.clone()),
                    )
                    .child(
                        div()
                            .flex_shrink_0()
                            .px(px(8.0))
                            .py(px(2.0))
                            .rounded(px(4.0))
                            .bg(rgba(UiConstants::ACCENT_RGBA))
                            .cursor_pointer()
                            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                let target = folder_path.clone();
                                v_enter.update(cx, |this, cx| {
                                    this.focus_pane(pane_focus, cx);
                                    this.navigate_pane(target, pane_focus, cx);
                                });
                            })
                            .child(
                                div()
                                    .text_size(px((t_size - 1.0).max(10.0)))
                                    .text_color(rgb(0xFFFFFF))
                                    .child("Open Folder →"),
                            ),
                    ),
            )
            .child(if children.is_empty() {
                div()
                    .flex_1()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .text_size(px(t_size))
                            .text_color(rgba(0xFFFFFF40u32))
                            .child("Folder is empty"),
                    )
                    .into_any_element()
            } else {
                let handle_w = scroll_handle.clone();
                let v_wheel = view.clone();
                let parent_folder_path = folder.path.clone();
                let parent_folder_name = folder.name.clone();
                uniform_list(
                    match pane_focus {
                        PaneFocus::Primary => "col3-scroll-primary",
                        PaneFocus::Secondary => "col3-scroll-secondary",
                    },
                    children.len(),
                    {
                        let view = view;
                        move |range, _win, cx| {
                            let app_state = view.read(cx);
                            let main_state = app_state.state.pane_view(pane_focus);
                            let spec = MainViewState::columns_layout_spec(
                                main_state.zoom_level,
                                main_state.compact_mode,
                            );
                            let row_h = spec.row_height;
                            let icon_sz = spec.icon_size;
                            let txt_sz = spec.text_size;
                            let ln_h = (txt_sz * 1.24).ceil();
                            let hovered_drop = app_state
                                .state
                                .main_view
                                .drag_state
                                .hovered_target_folder
                                .clone();
                            let is_dragging_any = app_state.state.main_view.drag_state.is_dragging;
                            range
                                .map(|i| {
                                    let (name, child_path, is_dir, sz) = children
                                        .get(i)
                                        .cloned()
                                        .unwrap_or_else(|| {
                                            (String::new(), FilesPath::new(""), false, 0)
                                        });
                                    let is_drop_target = is_dragging_any
                                        && is_dir
                                        && hovered_drop.as_ref() == Some(&child_path);
                                    let ext = get_extension(&name);
                                    let (glyph, color) = if is_dir {
                                        ("\u{E8B7}", 0xF5B942u32)
                                    } else {
                                        get_file_type_icon_and_color(&ext)
                                    };
                                    let v_down = view.clone();
                                    let v_move = view.clone();
                                    let v_up = view.clone();
                                    let cp_down = child_path.clone();
                                    let cp_move = child_path.clone();
                                    let cp_up = child_path;
                                    let nm_move = name.clone();
                                    let pf_path = parent_folder_path.clone();
                                    let pf_name = parent_folder_name.clone();

                                    div()
                                        .h(px(row_h))
                                        .px(px(8.0))
                                        .flex()
                                        .flex_shrink_0()
                                        .overflow_hidden()
                                        .items_center()
                                        .gap(px(6.0))
                                        .rounded(px(4.0))
                                        .cursor_pointer()
                                        .border(px(1.0))
                                        .border_color(if is_drop_target {
                                            rgba(0x60CDFFFFu32)
                                        } else {
                                            rgba(0x00000000u32)
                                        })
                                        .bg(if is_drop_target {
                                            rgba(0x0078D444u32)
                                        } else {
                                            rgba(0x00000000u32)
                                        })
                                        .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
                                        .on_mouse_down(MouseButton::Left, move |ev, _, cx| {
                                            cx.stop_propagation();
                                            let pos = point(
                                                ev.position.x.into(),
                                                ev.position.y.into(),
                                            );
                                            let target = cp_down.clone();
                                            v_down.update(cx, |this, cx| {
                                                this.focus_pane(pane_focus, cx);
                                                if ev.click_count == 2 {
                                                    this.state
                                                        .main_view
                                                        .drag_state
                                                        .pending_path_candidate = None;
                                                    if is_dir {
                                                        this.navigate_pane(target, pane_focus, cx);
                                                    } else {
                                                        crate::storage::preview_service::open_file_cross_platform(
                                                            target.as_path(),
                                                        );
                                                    }
                                                } else {
                                                    this.state
                                                        .main_view
                                                        .record_path_drag_candidate(
                                                            pane_focus, target, pos,
                                                        );
                                                }
                                            });
                                        })
                                        .on_mouse_move(move |ev, _win, cx| {
                                            let pos = point(
                                                ev.position.x.into(),
                                                ev.position.y.into(),
                                            );
                                            v_move.update(cx, |this, cx| {
                                                if this.state.main_view.drag_state.is_dragging
                                                    || this
                                                        .state
                                                        .main_view
                                                        .drag_state
                                                        .pending_candidate
                                                        .is_some()
                                                    || this
                                                        .state
                                                        .main_view
                                                        .drag_state
                                                        .pending_path_candidate
                                                        .is_some()
                                                {
                                                    let folder_target = if is_dir {
                                                        Some((cp_move.clone(), nm_move.clone()))
                                                    } else if this
                                                        .state
                                                        .main_view
                                                        .drag_state
                                                        .paths
                                                        .first()
                                                        .and_then(|p| p.parent())
                                                        .as_ref()
                                                        != Some(&pf_path)
                                                    {
                                                        Some((pf_path.clone(), pf_name.clone()))
                                                    } else {
                                                        None
                                                    };
                                                    this.update_file_drag_move(
                                                        pos,
                                                        ev.modifiers,
                                                        folder_target,
                                                        cx,
                                                    );
                                                }
                                            });
                                        })
                                        .on_mouse_up(MouseButton::Left, move |ev, _, cx| {
                                            cx.stop_propagation();
                                            let pos = point(
                                                ev.position.x.into(),
                                                ev.position.y.into(),
                                            );
                                            let target = cp_up.clone();
                                            v_up.update(cx, |this, cx| {
                                                if this.state.main_view.drag_state.is_dragging {
                                                    this.end_drag(pos.x, pos.y, None, cx);
                                                } else {
                                                    this.state
                                                        .main_view
                                                        .drag_state
                                                        .pending_path_candidate = None;
                                                    if is_dir && ev.click_count == 1 {
                                                        this.navigate_pane(target, pane_focus, cx);
                                                    }
                                                }
                                            });
                                        })
                                        .child(
                                            div()
                                                .w(px(icon_sz))
                                                .h(px(icon_sz))
                                                .flex_shrink_0()
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .child(
                                                    div()
                                                        .text_size(px(icon_sz * 0.72))
                                                        .font_family(UiConstants::FONT_MDL2)
                                                        .text_color(rgb(color))
                                                        .child(glyph),
                                                ),
                                        )
                                        .child(
                                            div()
                                                .flex_1()
                                                .min_w(px(0.0))
                                                .truncate()
                                                .text_size(px(txt_sz))
                                                .line_height(px(ln_h))
                                                .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                                                .child(name),
                                        )
                                        .child(if is_dir {
                                            div()
                                                .flex_shrink_0()
                                                .text_size(px((txt_sz * 0.75).clamp(8.5, 11.0)))
                                                .font_family(UiConstants::FONT_MDL2)
                                                .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                                .child("\u{E76C}")
                                                .into_any_element()
                                        } else {
                                            div()
                                                .flex_shrink_0()
                                                .text_size(px((txt_sz - 1.5).max(9.5)))
                                                .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                                .child(format_size(sz))
                                                .into_any_element()
                                        })
                                        .into_any_element()
                                })
                                .collect::<Vec<gpui::AnyElement>>()
                        }
                    },
                )
                .min_h(px(0.0))
                .h_full()
                .p(px(4.0))
                .track_scroll(scroll_handle)
                .on_scroll_wheel(move |event, _win, cx| {
                    if event.modifiers.control {
                        return;
                    }
                    cx.stop_propagation();
                    let delta = event.delta.pixel_delta(px(20.0));
                    let d: f32 = delta.y.into();
                    crate::ui::components::bouncy_scroll::wheel_scroll_uniform(
                        &v_wheel, &handle_w, d, cx,
                    );
                })
                .into_any_element()
            })
    }
}

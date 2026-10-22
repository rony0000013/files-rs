//! Columns View (Miller Columns / Files-community Column View parity)
//!
//! Three-column interactive Miller Columns layout:
//! - Column 1 (Left): Parent directory sibling folders (click any sibling folder to switch to it,
//!   or click the parent header to navigate up).
//! - Column 2 (Middle): Current directory contents with full selection (single-click select,
//!   Shift+Click range select, Ctrl+Click toggle select, double-click to open file / enter folder,
//!   right-click context menu, inline F2 rename, and live rubber-band drag selection).
//! - Column 3 (Right): When a folder is selected in Column 2, lists its child items (click a
//!   child folder to navigate into it); when a file is selected, displays a rich inspector card
//!   with thumbnail/icon, metadata, and preview snippet.

use crate::config::config::GroupColumn;
use crate::core::navigation_manager::PaneFocus;
use crate::core::path_util::FilesPath;
use crate::storage::storage::FileType;
use crate::ui::constants::UiConstants;
use crate::ui::views::helpers::{format_size, format_system_time};
use crate::ui::views::main_view::{FileItems, GroupedItemType, MainView, MainViewState};
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

    fn columns_view_interactive(
        &self,
        visible: &[usize],
        file_items: Entity<FileItems>,
        view: Entity<crate::app::AppWindow>,
        pane_focus: PaneFocus,
        col2_origin: Point<f32>,
        pane_w: f32,
    ) -> Div {
        let visible_clone = self.visible_items_rc();
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

        let col2_w = (pane_w * 0.38).clamp(220.0, 420.0);
        let v_up = view.clone();
        let parent_dir_for_up = parent_dir.clone();

        div()
            .flex()
            .flex_row()
            .flex_grow()
            .w_full()
            .h_full()
            // ── Column 1: Parent directory Miller column
            .child(
                div()
                    .w(px(210.0))
                    .flex_shrink_0()
                    .h_full()
                    .flex_col()
                    .border_r(px(1.0))
                    .border_color(rgba(UiConstants::DIVIDER_RGBA))
                    .child(
                        div()
                            .h(px(32.0))
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
                                    .text_size(px(11.0))
                                    .font_family(UiConstants::FONT_MDL2)
                                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                    .child("\u{E74A}"), // Up arrow
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .overflow_hidden()
                                    .text_size(px(11.0))
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
                                move |range, _win, _cx| {
                                    range
                                        .map(|i| {
                                let (name, sib_path, is_curr) = siblings
                                    .get(i)
                                    .cloned()
                                    .unwrap_or_else(|| {
                                        (String::new(), FilesPath::new(""), false)
                                    });
                                let v_sib = view.clone();
                                div()
                                    .h(px(26.0))
                                    .px(px(8.0))
                                    .flex()
                                    .items_center()
                                    .gap(px(6.0))
                                    .rounded(px(4.0))
                                    .cursor_pointer()
                                    .bg(if is_curr {
                                        rgba(UiConstants::BG_ITEM_SELECTED_RGBA)
                                    } else {
                                        rgba(0x00000000)
                                    })
                                    .hover(|s| {
                                        if is_curr {
                                            s.bg(rgba(UiConstants::BG_ITEM_SELECTED_HOVER_RGBA))
                                        } else {
                                            s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA))
                                        }
                                    })
                                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                        let target = sib_path.clone();
                                        v_sib.update(cx, |this, cx| {
                                            this.focus_pane(pane_focus, cx);
                                            this.navigate_pane(target, pane_focus, cx);
                                        });
                                    })
                                    .child(if is_curr {
                                        div()
                                            .w(px(3.0))
                                            .h(px(14.0))
                                            .rounded(px(1.5))
                                            .bg(rgb(UiConstants::ACCENT_COLOR_RGB))
                                    } else {
                                        div()
                                    })
                                    .child(
                                        div()
                                            .text_size(px(13.0))
                                            .font_family(UiConstants::FONT_MDL2)
                                            .text_color(rgb(0xF5B942))
                                            .child("\u{E8B7}"),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .overflow_hidden()
                                            .text_size(px(12.0))
                                            .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                                            .child(name),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(9.0))
                                            .font_family(UiConstants::FONT_MDL2)
                                            .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
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
            // ── Column 2: Current directory contents (with live rubber-band selection)
            .child(
                div()
                    .w(px(col2_w))
                    .flex_shrink_0()
                    .h_full()
                    .flex_col()
                    .border_r(px(1.0))
                    .border_color(rgba(UiConstants::DIVIDER_RGBA))
                    .child(
                        div()
                            .h(px(32.0))
                            .px(px(12.0))
                            .flex()
                            .items_center()
                            .justify_between()
                            .border_b(px(1.0))
                            .border_color(rgba(UiConstants::DIVIDER_RGBA))
                            .child(
                                div()
                                    .text_size(px(11.0))
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
                                    .text_size(px(10.5))
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
                                    let pos =
                                        point(event.position.x.into(), event.position.y.into());
                                    let additive =
                                        event.modifiers.control || event.modifiers.shift;
                                    v.update(cx, |this, cx| {
                                        this.focus_pane(pane_focus, cx);
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
                                        PaneFocus::Primary => "col2-scroll-primary",
                                        PaneFocus::Secondary => "col2-scroll-secondary",
                                    },
                                    visible_clone.len(),
                                    {
                                        let view = view.clone();
                                        move |range, _win, cx| {
                                            let fi = file_items.read(cx);
                                            let app_state = view.read(cx);
                                            let main_state = app_state.state.pane_view(pane_focus);
                                            let icon_paths = &app_state.state.main_view.icon_paths;
                                            let r_h = 20.0 + main_state.zoom_level * 28.0;
                                            let i_size = (r_h - 8.0).max(16.0).min(36.0);
                                            let t_size = 11.5 + main_state.zoom_level * 3.0;

                                            range
                                                .map(|i| {
                                                    if i >= visible_clone.len() {
                                                        return div().into_any_element();
                                                    }
                                                    let idx = visible_clone[i];
                                                    if idx >= fi.items.len() {
                                                        return div().into_any_element();
                                                    }
                                                    let item = &fi.items[idx];
                                                    let is_selected =
                                                        fi.selected_indices.contains(&idx);
                                                    let is_dir = matches!(
                                                        item.file_type,
                                                        FileType::Directory
                                                    );
                                                    let display_name = main_state
                                                        .display_name(item)
                                                        .into_owned();
                                                    let path = item.path.clone();
                                                    let v = view.clone();

                                                    div()
                                                        .id(ElementId::from(
                                                            idx + match pane_focus {
                                                                PaneFocus::Primary => 50000,
                                                                PaneFocus::Secondary => 150000,
                                                            },
                                                        ))
                                                        .flex()
                                                        .flex_row()
                                                        .h(px(r_h))
                                                        .px(px(8.0))
                                                        .items_center()
                                                        .gap(px(6.0))
                                                        .rounded(px(4.0))
                                                        .mx(px(4.0))
                                                        .bg(if is_selected {
                                                            rgba(UiConstants::BG_ITEM_SELECTED_RGBA)
                                                        } else {
                                                            rgba(0x00000000)
                                                        })
                                                        .hover(|s| {
                                                            if is_selected {
                                                                s.bg(rgba(
                                                                    UiConstants::BG_ITEM_SELECTED_HOVER_RGBA,
                                                                ))
                                                            } else {
                                                                s.bg(rgba(
                                                                    UiConstants::BG_ITEM_HOVER_RGBA,
                                                                ))
                                                            }
                                                        })
                                                        .cursor_pointer()
                                                        .on_mouse_down(MouseButton::Left, {
                                                            let v = v.clone();
                                                            let path = path.clone();
                                                            move |ev, _, cx| {
                                                                cx.stop_propagation();
                                                                v.update(cx, |this, cx| {
                                                                    this.focus_pane(pane_focus, cx);
                                                                    if ev.click_count == 2 {
                                                                        if is_dir {
                                                                            this.navigate_pane(
                                                                                path.clone(),
                                                                                pane_focus,
                                                                                cx,
                                                                            );
                                                                        } else {
                                                                            this.open_item_in_pane(
                                                                                pane_focus,
                                                                                idx,
                                                                                cx,
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
                                                            let v = v.clone();
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
                                                                            pane_focus,
                                                                            pos,
                                                                            pane_w,
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
                                                                            pane_focus,
                                                                            idx,
                                                                            false,
                                                                            false,
                                                                            cx,
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
                                                        .child(if is_selected {
                                                            div()
                                                                .w(px(3.0))
                                                                .h(px(14.0))
                                                                .rounded(px(1.5))
                                                                .bg(rgb(
                                                                    UiConstants::ACCENT_COLOR_RGB,
                                                                ))
                                                        } else {
                                                            div()
                                                        })
                                                        .child(
                                                            div()
                                                                .w(px(i_size))
                                                                .h(px(i_size))
                                                                .flex_shrink_0()
                                                                .flex()
                                                                .items_center()
                                                                .justify_center()
                                                                .child(
                                                                    if let Some(icon_path) =
                                                                        icon_paths.get(&path)
                                                                    {
                                                                        img(icon_path.clone())
                                                                            .w(px(i_size))
                                                                            .h(px(i_size))
                                                                            .into_any_element()
                                                                    } else {
                                                                        div()
                                                                            .text_size(px(
                                                                                i_size * 0.7,
                                                                            ))
                                                                            .font_family(
                                                                                UiConstants::FONT_MDL2,
                                                                            )
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
                                                            if main_state.renaming_index
                                                                == Some(idx)
                                                            {
                                                                div()
                                                                    .flex_1()
                                                                    .bg(rgba(0xFFFFFF20u32))
                                                                    .border(px(1.0))
                                                                    .border_color(rgba(
                                                                        UiConstants::ACCENT_RGBA,
                                                                    ))
                                                                    .rounded(px(2.0))
                                                                    .px(px(4.0))
                                                                    .child(
                                                                        main_state
                                                                            .rename_text
                                                                            .clone(),
                                                                    )
                                                                    .into_any_element()
                                                            } else {
                                                                div()
                                                                    .flex_1()
                                                                    .min_w(px(0.0))
                                                                    .overflow_hidden()
                                                                    .text_size(px(t_size))
                                                                    .text_color(rgba(
                                                                        UiConstants::TEXT_PRIMARY_RGBA,
                                                                    ))
                                                                    .child(display_name)
                                                                    .into_any_element()
                                                            },
                                                        )
                                                        .child(if is_dir {
                                                            div()
                                                                .text_size(px(9.0))
                                                                .font_family(UiConstants::FONT_MDL2)
                                                                .text_color(rgba(
                                                                    UiConstants::TEXT_SECONDARY_RGBA,
                                                                ))
                                                                .child("\u{E76C}")
                                                                .into_any_element()
                                                        } else {
                                                            div().into_any_element()
                                                        })
                                                        .into_any_element()
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
                                .on_scroll_wheel({
                                    let h = self.uniform_list_scroll_handle.clone();
                                    let v = view.clone();
                                    move |event, _win, cx| {
                                        cx.stop_propagation();
                                        let delta = event.delta.pixel_delta(px(20.0));
                                        let d: f32 = delta.y.into();
                                        crate::ui::components::bouncy_scroll::wheel_scroll_uniform(
                                            &v, &h, d, cx,
                                        );
                                    }
                                }),
                            )
                            .child(MainView::render_lasso_box(self)),
                    ),
            )
            // ── Column 3: Child Directory Miller Column OR File Inspector Preview
            .child(
                div()
                    .flex_1()
                    .min_w(px(180.0))
                    .h_full()
                    .flex_col()
                    .bg(rgba(0xFFFFFF04u32))
                    .child(match selected_item {
                        None => div()
                            .flex_grow()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                div()
                                    .text_size(px(12.0))
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
                            )
                            .into_any_element()
                        }
                        Some(ref item) => div()
                            .flex_grow()
                            .p(px(16.0))
                            .child(Self::render_details_pane_internal(
                                item,
                                self.preview_text.as_deref(),
                            ))
                            .into_any_element(),
                    }),
            )
    }

    /// Refresh the Miller-column caches (Column 1 siblings, Column 3 child preview).
    /// Keyed on the source path so steady-state frames do zero I/O; a single
    /// `read_dir` runs per navigation or selection change instead of per frame.
    pub fn refresh_columns_caches(&mut self) {
        if !matches!(self.view_mode, crate::config::config::ViewMode::Columns) {
            return;
        }
        let parent = self.primary_path.parent();
        if self.col1_siblings_for != parent {
            let siblings: Vec<(String, FilesPath, bool)> = parent
                .as_ref()
                .and_then(|p| std::fs::read_dir(p.as_path()).ok())
                .map(|rd| {
                    let mut dirs: Vec<(String, FilesPath, bool)> = rd
                        .filter_map(|e| e.ok())
                        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
                        .map(|e| {
                            let name = e.file_name().to_string_lossy().into_owned();
                            let fp = FilesPath::new(e.path());
                            let is_curr = fp == self.primary_path;
                            (name, fp, is_curr)
                        })
                        .collect();
                    dirs.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));
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
                        .take(80)
                        .map(|e| {
                            let name = e.file_name().to_string_lossy().into_owned();
                            let fp = FilesPath::new(e.path());
                            let is_dir = e.file_type().map(|t| t.is_dir()).unwrap_or(false);
                            let sz = if is_dir {
                                0
                            } else {
                                e.metadata().map(|m| m.len()).unwrap_or(0)
                            };
                            (name, fp, is_dir, sz)
                        })
                        .collect();
                    entries.sort_by(|a, b| {
                        b.2.cmp(&a.2)
                            .then_with(|| a.0.to_lowercase().cmp(&b.0.to_lowercase()))
                    });
                    entries
                })
                .unwrap_or_default();
            self.col3_children_for = sel_folder;
            self.col3_children = children;
        }
    }

    /// Column 3 when a Directory is selected in Column 2: lists the child items inside
    /// that folder so clicking a child folder navigates directly into it (Miller Columns parity).
    fn render_column3_directory_preview(
        folder: &crate::domain::metadata::FileItem,
        children: Vec<(String, FilesPath, bool, u64)>,
        view: Entity<crate::app::AppWindow>,
        pane_focus: PaneFocus,
        scroll_handle: UniformListScrollHandle,
    ) -> impl IntoElement {

        let v_enter = view.clone();
        let folder_path = folder.path.clone();

        div()
            .flex_col()
            .w_full()
            .h_full()
            .child(
                div()
                    .h(px(32.0))
                    .px(px(12.0))
                    .flex()
                    .items_center()
                    .justify_between()
                    .border_b(px(1.0))
                    .border_color(rgba(UiConstants::DIVIDER_RGBA))
                    .child(
                        div()
                            .text_size(px(11.0))
                            .font_weight(FontWeight(600.0))
                            .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                            .child(folder.name.clone()),
                    )
                    .child(
                        div()
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
                                    .text_size(px(10.5))
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
                            .text_size(px(12.0))
                            .text_color(rgba(0xFFFFFF40u32))
                            .child("Folder is empty"),
                    )
                    .into_any_element()
            } else {
                let handle_w = scroll_handle.clone();
                let v_wheel = view.clone();
                uniform_list(
                    match pane_focus {
                        PaneFocus::Primary => "col3-scroll-primary",
                        PaneFocus::Secondary => "col3-scroll-secondary",
                    },
                    children.len(),
                    {
                        let view = view.clone();
                        move |range, _win, _cx| {
                            range
                                .map(|i| {
                                    let (name, child_path, is_dir, sz) = children
                                        .get(i)
                                        .cloned()
                                        .unwrap_or_else(|| {
                                            (String::new(), FilesPath::new(""), false, 0)
                                        });
                                    let v_child = view.clone();
                        div()
                            .h(px(26.0))
                            .px(px(8.0))
                            .flex()
                            .items_center()
                            .gap(px(6.0))
                            .rounded(px(4.0))
                            .cursor_pointer()
                            .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
                            .on_mouse_down(MouseButton::Left, move |ev, _, cx| {
                                let target = child_path.clone();
                                v_child.update(cx, |this, cx| {
                                    this.focus_pane(pane_focus, cx);
                                    if is_dir {
                                        this.navigate_pane(target, pane_focus, cx);
                                    } else if ev.click_count == 2 {
                                        let _ = std::process::Command::new("cmd")
                                            .args(["/C", "start", "", &target.to_string_lossy()])
                                            .spawn();
                                    }
                                });
                            })
                            .child(
                                div()
                                    .text_size(px(13.0))
                                    .font_family(UiConstants::FONT_MDL2)
                                    .text_color(if is_dir {
                                        rgb(0xF5B942)
                                    } else {
                                        rgba(UiConstants::TEXT_SECONDARY_RGBA)
                                    })
                                    .child(if is_dir { "\u{E8B7}" } else { "\u{E8A5}" }),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .overflow_hidden()
                                    .text_size(px(12.0))
                                    .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                                    .child(name),
                            )
                            .child(
                                div()
                                    .text_size(px(10.0))
                                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                    .child(if is_dir {
                                        "\u{E76C}".to_string()
                                    } else {
                                        format_size(sz)
                                    }),
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
                .track_scroll(scroll_handle)
                .on_scroll_wheel(move |event, _win, cx| {
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

    fn render_details_pane_internal(
        item: &crate::domain::metadata::FileItem,
        preview_text: Option<&str>,
    ) -> impl IntoElement {
        let is_dir = matches!(item.file_type, FileType::Directory);
        let modified_str = format_system_time(item.modified);

        div()
            .flex_col()
            .gap(px(14.0))
            .w_full()
            .child(
                div()
                    .flex_col()
                    .items_center()
                    .gap(px(10.0))
                    .child(
                        div()
                            .w(px(64.0))
                            .h(px(64.0))
                            .bg(rgba(UiConstants::BG_BADGE_RGBA))
                            .rounded(px(8.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_size(px(32.0))
                            .font_family(UiConstants::FONT_MDL2)
                            .text_color(if is_dir {
                                rgb(0xF5B942)
                            } else {
                                rgb(UiConstants::ACCENT_COLOR_RGB)
                            })
                            .child(if is_dir { "\u{E8B7}" } else { "\u{E8A5}" }),
                    )
                    .child(
                        div()
                            .text_size(px(15.0))
                            .font_weight(FontWeight(600.0))
                            .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                            .child(item.name.clone()),
                    ),
            )
            .child(
                div()
                    .flex_col()
                    .gap(px(6.0))
                    .child(
                        div()
                            .text_size(px(11.0))
                            .font_weight(FontWeight(600.0))
                            .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                            .child("DETAILS"),
                    )
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                    .child("Type"),
                            )
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                                    .child(if is_dir { "Folder" } else { "File" }),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                    .child("Size"),
                            )
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                                    .child(format_size(item.size)),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                    .child("Modified"),
                            )
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                                    .child(modified_str),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .gap(px(8.0))
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                    .child("Location"),
                            )
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                                    .overflow_hidden()
                                    .child(
                                        item.path
                                            .parent()
                                            .map(|p| p.to_string_lossy().into_owned())
                                            .unwrap_or_default(),
                                    ),
                            ),
                    ),
            )
            .children(preview_text.map(|txt| {
                let snippet: String = txt.chars().take(600).collect();
                div()
                    .mt(px(6.0))
                    .p(px(10.0))
                    .rounded(px(6.0))
                    .bg(rgba(0x00000033u32))
                    .border(px(1.0))
                    .border_color(rgba(UiConstants::DIVIDER_RGBA))
                    .text_size(px(11.0))
                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                    .overflow_hidden()
                    .child(snippet)
            }))
    }
}

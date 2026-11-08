use crate::ui::components::dialogs::{DialogType, SidebarMenuTarget, SubmenuKind};
use crate::platform::windows::classic_menu::ClassicMenuEntry;
use crate::ui::constants::UiConstants;
use gpui::*;

/// Trailing affordance on a context menu row (Files parity: shortcut
/// hints like Ctrl+Shift+C, submenu chevrons).
pub(crate) enum MenuTrailing {
    None,
    Hint(&'static str),
    Chevron,
}

/// Builds a flyout card shell with authentic Windows 11 Fluent Acrylic / Mica
/// Builds a flyout card shell with authentic Windows 11 Fluent Acrylic / Mica
/// Gaussian wallpaper blur backing, translucent dark acrylic tint, 1px surface stroke,
/// explicit height bounds, and complete pointer event isolation so underlying items never bleed through.
pub(crate) fn flyout_card_shell(
    abs_x: f32,
    abs_y: f32,
    w: f32,
    h: f32,
    mica_geo: Option<&crate::wallpaper_mica::MicaGeo>,
) -> Div {
    let mut shell = div()
        .relative()
        .flex_col()
        .w(px(w))
        .h(px(h))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation());

    let mut bg_card = div()
        .absolute()
        .top_0()
        .left_0()
        .w(px(w))
        .h(px(h))
        .rounded(px(8.0))
        .overflow_hidden()
        .border(px(1.0))
        .border_color(rgba(UiConstants::SURFACE_STROKE_FLYOUT_RGBA))
        .shadow_lg()
        // Solid dark Fluent acrylic base (100% opaque) so underlying menus never bleed through
        .bg(rgb(UiConstants::BG_FLYOUT_MICA_RGB));

    if let Some(geo) = mica_geo {
        bg_card = bg_card.child(
            div()
                .absolute()
                .inset_0()
                .overflow_hidden()
                .child(
                    img(geo.path.clone())
                        .absolute()
                        .left(px(geo.left - abs_x))
                        .top(px(geo.top - abs_y))
                        .w(px(geo.w))
                        .h(px(geo.h)),
                ),
        );
        // Fluent acrylic dark tint (87.8% opacity) over the blurred wallpaper
        bg_card = bg_card.child(
            div()
                .absolute()
                .inset_0()
                .bg(rgba(0x202020D8)),
        );
    }

    shell = shell.child(bg_card);
    shell
}

impl crate::ui::components::dialogs::DialogsView {
    pub(crate) fn build_context_menu_dialog(
        item_index: Option<usize>,
        position: Point<Pixels>,
        has_selection: bool,
        selected_path: Option<crate::core::path_util::FilesPath>,
        selected_count: usize,
        is_dir: bool,
        open_submenu: Option<crate::ui::components::dialogs::SubmenuKind>,
        selected_paths: Vec<std::path::PathBuf>,
        current_dir: std::path::PathBuf,
        classic: Option<Vec<crate::platform::windows::classic_menu::ClassicMenuEntry>>,
        classic_loading: bool,
        classic_open: Vec<usize>,
        flip: bool,
        expanded: bool,
        max_h: f32,
        win_w: f32,
        win_h: f32,
        scroll_offset: f32,
        current_sort: crate::config::config::SortBy,
        current_group: crate::config::config::GroupColumn,
        is_pinned_to_sidebar: bool,
        available_tags: Vec<crate::config::config::TagDefinition>,
        item_tags: Vec<String>,
        view: Entity<crate::app::AppWindow>,
        cx: &mut Context<crate::app::AppWindow>,
        mica_geo: Option<crate::wallpaper_mica::MicaGeo>,
    ) -> impl IntoElement {
        // Files shows different menus for file / folder / multi-select.
        let is_multi = selected_count > 1;
        // Clone view for closures
        let v_close = view.clone();
        let v_open = view.clone();
        let v_copy = view.clone();
        let v_cut = view.clone();
        let v_paste = view.clone();
        let v_rename = view.clone();
        let v_delete = view.clone();
        let v_share = view.clone();
        let v_props = view.clone();
        let v_copypath = view.clone();

        #[allow(unused_mut)]
        let mut showmore_off = 0.0f32;
        let mut active_child_panel: Option<gpui::Stateful<Div>> = None;
        let menu_top: f32 = position.y.into();
        let menu_left: f32 = position.x.into();
        let win_w = if win_w > 100.0 { win_w } else { 1280.0 };
        let win_h = if win_h > 100.0 { win_h } else { 800.0 };

        let sel_paths: Vec<std::path::PathBuf> = if has_selection {
            selected_paths.clone()
        } else {
            Vec::new()
        };
        let cur_dir: std::path::PathBuf = current_dir.clone();

        let mut items: Vec<gpui::AnyElement> = Vec::new();
        let mut submenu_offsets: std::collections::HashMap<SubmenuKind, f32> =
            std::collections::HashMap::new();
        let mut items_acc_h = 0.0f32;

        let fixed_header: Option<Div>;
        let fixed_header_h: f32;

        if has_selection {
            // ═══════════════════════════════════════════════════════════
            // FILES LAYOUT — file/folder/multi-select context menu
            // ═══════════════════════════════════════════════════════════
            let v_sub = view.clone();

            // ── TOP ICON TOOLBAR (Cut/Copy/Paste/Rename/Share/Delete/Props)
            {
                let v = v_cut.clone();
                let cut_btn: Box<
                    dyn Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
                > = Box::new(move |_, _, cx| {
                    v.update(cx, |this, cx| {
                        this.cut_selected(cx);
                        this.close_dialog(cx);
                    });
                });
                let v = v_copy.clone();
                let copy_btn: Box<
                    dyn Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
                > = Box::new(move |_, _, cx| {
                    v.update(cx, |this, cx| {
                        this.copy_selected(cx);
                        this.close_dialog(cx);
                    });
                });
                let v = v_paste.clone();
                let paste_btn: Box<
                    dyn Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
                > = Box::new(move |_, _, cx| {
                    v.update(cx, |this, cx| {
                        this.paste(cx);
                        this.close_dialog(cx);
                    });
                });
                let v = v_rename.clone();
                let idx = item_index;
                let rename_btn: Box<
                    dyn Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
                > = Box::new(move |_, _, cx| {
                    v.update(cx, |this, cx| {
                        if let Some(i) = idx {
                            if this.state.main_view.selected_indices.len() > 1 {
                                this.bulk_rename(cx);
                            } else {
                                this.start_rename(i, cx);
                            }
                            this.close_dialog(cx);
                        }
                    });
                });
                let v = v_share.clone();
                let path = selected_path.clone();
                let share_btn: Box<
                    dyn Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
                > = Box::new(move |_, _, cx| {
                    v.update(cx, |this, cx| {
                        if let Some(ref p) = path {
                            #[cfg(windows)]
                            {
                                let share_path = p.clone();
                                cx.background_executor()
                                    .spawn(async move {
                                        let service = crate::platform::windows::share_service::ShareService::new();
                                        let _ = service.share_file(0, &share_path).await;
                                    })
                                    .detach();
                            }
                        }
                        this.close_dialog(cx);
                    });
                });
                let v = v_delete.clone();
                let delete_btn: Box<
                    dyn Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
                > = Box::new(move |_, _, cx| {
                    v.update(cx, |this, cx| {
                        this.delete_selected(cx);
                        this.close_dialog(cx);
                    });
                });
                let v = v_props.clone();
                let props_btn: Box<
                    dyn Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
                > = Box::new(move |_, _, cx| {
                    v.update(cx, |this, cx| {
                        this.close_dialog(cx);
                        this.open_properties_for_selected(cx);
                    });
                });
                fixed_header = Some(
                    div()
                        .relative()
                        .w_full()
                        .pt(px(4.0))
                        .child(Self::context_menu_toolbar(
                            vec![
                                ("\u{E8C6}", "Cut \u{2014} Ctrl+X", cut_btn),
                                ("\u{E8C8}", "Copy \u{2014} Ctrl+C", copy_btn),
                                ("\u{E77F}", "Paste \u{2014} Ctrl+V", paste_btn),
                                ("\u{E8AC}", "Rename \u{2014} F2", rename_btn),
                                ("\u{E72D}", "Share", share_btn),
                                ("\u{E74D}", "Delete \u{2014} Del", delete_btn),
                                ("\u{E713}", "Properties \u{2014} Alt+Enter", props_btn),
                            ],
                            view.clone(),
                        ))
                        .child(Self::context_menu_separator()),
                );
                fixed_header_h = 45.0f32;
            }

            // Open (single selection only)
            if !is_multi {
                items.push(Self::context_menu_row("\u{E197}", "Open", MenuTrailing::None, {
                    let path = selected_path.clone();
                    move |_, _, cx| {
                        if let Some(ref p) = path {
                            let path = p.clone();
                            v_open.update(cx, |this, cx| {
                                this.navigate(path, cx);
                                this.close_dialog(cx);
                            });
                        }
                    }
                }).into_any_element());
                items_acc_h += 32.0;
            }

            // Open with › (file, single)
            if !is_multi && !is_dir {
                let row_y = items_acc_h;
                submenu_offsets.insert(SubmenuKind::OpenWith, row_y);
                let v_fire = v_sub.clone();
                let path = selected_path.clone();
                items.push(Self::submenu_row(
                    "\u{E7AC}",
                    "Open With",
                    SubmenuKind::OpenWith,
                    v_sub.clone(),
                    move |_, _, cx| {
                        if let Some(ref p) = path {
                            let path_str = p.to_string_lossy().to_string();
                            let _ = std::process::Command::new("rundll32")
                                .args(["shell32.dll,OpenAs_RunDLL", &path_str])
                                .stdin(std::process::Stdio::null())
                                .stdout(std::process::Stdio::null())
                                .stderr(std::process::Stdio::null())
                                .spawn();
                        }
                        v_fire.update(cx, |this, cx| this.close_dialog(cx));
                    },
                ).into_any_element());
                items_acc_h += 32.0;
            }

            // Paste shortcut (from clipboard files)
            items.push(Self::context_menu_row("\u{E77F}", "Paste shortcut", MenuTrailing::None, {
                let v = v_paste.clone();
                move |_, _, cx| {
                    v.update(cx, |this, cx| this.paste_as_shortcut(cx));
                }
            }).into_any_element());
            items_acc_h += 32.0;

            // Copy item path (Ctrl+Shift+C)
            items.push(Self::context_menu_row("\u{F413}", "Copy item path", MenuTrailing::Hint("Ctrl+Shift+C"), {
                let v = v_copypath.clone();
                move |_, _, cx| {
                    v.update(cx, |this, cx| {
                        this.copy_item_path(cx);
                        this.close_dialog(cx);
                    });
                }
            }).into_any_element());
            items_acc_h += 32.0;

            // Create folder with selection
            items.push(Self::context_menu_row("\u{E8F4}", "Create folder with selection", MenuTrailing::None, {
                let v = v_close.clone();
                move |_, _, cx| {
                    v.update(cx, |this, cx| this.create_folder_with_selection(cx));
                }
            }).into_any_element());
            items_acc_h += 32.0;

            // Create Shortcut (single selection only)
            if !is_multi {
                items.push(Self::context_menu_row(
                    "\u{E71B}",
                    "Create Shortcut",
                    MenuTrailing::None,
                    {
                        let v = v_close.clone();
                        let path = selected_path.clone();
                        move |_, _, cx| {
                            v.update(cx, |this, cx| {
                                if let Some(ref p) = path {
                                    let target_path = p.as_path().to_path_buf();
                                    let current_dir = this.state.main_view.primary_path.as_path().to_path_buf();
                                    let file_name = p.file_name()
                                        .map(|s| s.to_string_lossy().to_string())
                                        .unwrap_or_else(|| "Shortcut".to_string());
                                    let shortcut_name = format!("{} - Shortcut", file_name);
                                    this.state.active_dialog = Some(DialogType::CreateShortcut {
                                        state: Box::new(
                                            crate::ui::dialog_services::shortcut_dialog::CreateShortcutState::new(
                                                target_path,
                                                current_dir,
                                                shortcut_name,
                                            )
                                        ),
                                    });
                                }
                                cx.notify();
                            });
                        }
                    },
                ).into_any_element());
                items_acc_h += 32.0;
            }

            // Compress › (ZIP creation + conditional Extract Here)
            {
                let row_y = items_acc_h;
                submenu_offsets.insert(SubmenuKind::Compress, row_y);
                let v_fire = v_sub.clone();
                items.push(Self::submenu_row(
                    "\u{F012}",
                    "Compress",
                    SubmenuKind::Compress,
                    v_sub.clone(),
                    move |_, _, cx| {
                        v_fire.update(cx, |this, cx| {
                            this.set_open_submenu(Some(SubmenuKind::Compress), cx);
                        });
                    },
                ).into_any_element());
                items_acc_h += 32.0;
            }

            // Send to › (Windows SendTo folder)
            {
                let row_y = items_acc_h;
                submenu_offsets.insert(SubmenuKind::SendTo, row_y);
                let v_fire = v_sub.clone();
                items.push(Self::submenu_row(
                    "",
                    "Send to",
                    SubmenuKind::SendTo,
                    v_sub.clone(),
                    move |_, _, cx| {
                        v_fire.update(cx, |this, cx| {
                            this.set_open_submenu(Some(SubmenuKind::SendTo), cx);
                        });
                    },
                ).into_any_element());
                items_acc_h += 32.0;
            }

            // Edit file tags ›
            {
                let row_y = items_acc_h;
                submenu_offsets.insert(SubmenuKind::FileTags, row_y);
                let v_fire = v_sub.clone();
                items.push(Self::submenu_row(
                    "\u{E1CB}",
                    "Edit file tags",
                    SubmenuKind::FileTags,
                    v_sub.clone(),
                    move |_, _, cx| {
                        v_fire.update(cx, |this, cx| {
                            this.set_open_submenu(Some(SubmenuKind::FileTags), cx);
                        });
                    },
                ).into_any_element());
                items_acc_h += 32.0;
            }

            items.push(Self::context_menu_separator().into_any_element());
            items_acc_h += 7.0;

            // Show more options › — the real classic shell items (WinRAR,
            // Kaspersky, ...) expand inline on hover (Files parity). Windows
            // only: other platforms have no classic provider yet.
            #[cfg(target_os = "windows")]
            {
                let row_y = items_acc_h;
                showmore_off = row_y;
                if !expanded {
                    submenu_offsets.insert(SubmenuKind::ShowMore, row_y);
                    let v_click = v_sub.clone();
                    items.push(Self::submenu_row(
                        "\u{E712}",
                        "Show more options",
                        SubmenuKind::ShowMore,
                        view.clone(),
                        move |_, _, cx| {
                            v_click.update(cx, |this, cx| {
                                this.set_open_submenu(Some(SubmenuKind::ShowMore), cx)
                            });
                        },
                    ).into_any_element());
                    items_acc_h += 32.0;
                } else {
                    // Expanded style: every classic item is inlined below the
                    // app rows - no hover needed. Nested verbs keep flyouts.
                    items.push(Self::context_menu_separator().into_any_element());
                    items_acc_h += 7.0;
                    match classic.as_ref() {
                        Some(entries) if !entries.is_empty() => {
                            let filtered: Vec<ClassicMenuEntry> = entries
                                .iter()
                                .filter(|e| {
                                    let l = e.label.to_lowercase();
                                    !l.contains("send to") && !l.contains("open with")
                                })
                                .cloned()
                                .collect();
                            let mut next_id: usize = 0;
                            for e in filtered.iter() {
                                items_acc_h += if e.separator { 7.0 } else { 32.0 };
                            }
                            let cur_scroll = scroll_offset;
                            let (inline, child_panel) = Self::classic_rows(
                                &filtered,
                                &classic_open,
                                &[],
                                view.clone(),
                                sel_paths.clone(),
                                cur_dir.clone(),
                                &mut next_id,
                                menu_left,
                                menu_top + fixed_header_h + showmore_off - cur_scroll,
                                fixed_header_h + showmore_off - cur_scroll,
                                284.0,
                                win_w,
                                win_h,
                                flip,
                                mica_geo.as_ref(),
                            );
                            for row in inline {
                                items.push(row.into_any_element());
                            }
                            if let Some(child) = child_panel {
                                active_child_panel = Some(child);
                            }
                        }
                        Some(_) => {
                            let mut nid = 0usize;
                            items.push(Self::classic_note_row(
                                "(No extended options)",
                                &mut nid,
                            ).into_any_element());
                            items_acc_h += 32.0;
                        }
                        None => {
                            let mut nid = 0usize;
                            let txt = if classic_loading {
                                "Loading…"
                            } else {
                                "(Unavailable)"
                            };
                            items.push(Self::classic_note_row(txt, &mut nid).into_any_element());
                            items_acc_h += 32.0;
                        }
                    }
                }
            }
        } else {
            // ═══════════════════════════════════════════════════════════
            // FILES LAYOUT — background (empty space) context menu
            // ═══════════════════════════════════════════════════════════
            fixed_header = None;
            fixed_header_h = 0.0f32;
            let v_sub = view.clone();

            // Layout ›
            {
                let row_y = items_acc_h;
                submenu_offsets.insert(SubmenuKind::Layout, row_y);
                let v_fire = v_sub.clone();
                items.push(Self::submenu_row(
                    "\u{F0E2}",
                    "Layout",
                    SubmenuKind::Layout,
                    v_sub.clone(),
                    move |_, _, cx| {
                        v_fire.update(cx, |this, cx| {
                            this.set_open_submenu(Some(SubmenuKind::Layout), cx);
                        });
                    },
                ).into_any_element());
                items_acc_h += 32.0;
            }

            // Sort by ›
            {
                let row_y = items_acc_h;
                submenu_offsets.insert(SubmenuKind::SortBy, row_y);
                let v_fire = v_sub.clone();
                items.push(Self::submenu_row(
                    "\u{E8CB}",
                    "Sort by",
                    SubmenuKind::SortBy,
                    v_sub.clone(),
                    move |_, _, cx| {
                        v_fire.update(cx, |this, cx| {
                            this.set_open_submenu(Some(SubmenuKind::SortBy), cx);
                        });
                    },
                ).into_any_element());
                items_acc_h += 32.0;
            }

            // Group by ›
            {
                let row_y = items_acc_h;
                submenu_offsets.insert(SubmenuKind::GroupBy, row_y);
                let v_fire = v_sub.clone();
                items.push(Self::submenu_row(
                    "\u{F168}",
                    "Group by",
                    SubmenuKind::GroupBy,
                    v_sub.clone(),
                    move |_, _, cx| {
                        v_fire.update(cx, |this, cx| {
                            this.set_open_submenu(Some(SubmenuKind::GroupBy), cx);
                        });
                    },
                ).into_any_element());
                items_acc_h += 32.0;
            }

            // Refresh (Ctrl+R)
            items.push(Self::context_menu_row("\u{E72C}", "Refresh", MenuTrailing::Hint("Ctrl+R"), {
                let v = v_close.clone();
                move |_, _, cx| {
                    v.update(cx, |this, cx| {
                        this.refresh(cx);
                        this.close_dialog(cx);
                    });
                }
            }).into_any_element());
            items_acc_h += 32.0;

            items.push(Self::context_menu_separator().into_any_element());
            items_acc_h += 7.0;

            // New ›
            {
                let row_y = items_acc_h;
                submenu_offsets.insert(SubmenuKind::New, row_y);
                let v_fire = v_sub.clone();
                items.push(Self::submenu_row(
                    "\u{E710}",
                    "New",
                    SubmenuKind::New,
                    v_sub.clone(),
                    move |_, _, cx| {
                        v_fire.update(cx, |this, cx| {
                            this.set_open_submenu(Some(SubmenuKind::New), cx);
                        });
                    },
                ).into_any_element());
                items_acc_h += 32.0;
            }

            // Paste shortcut (clipboard files pasted as shortcuts here)
            items.push(Self::context_menu_row("\u{E77F}", "Paste shortcut", MenuTrailing::None, {
                let v = v_sub.clone();
                move |_, _, cx| {
                    v.update(cx, |this, cx| this.paste_as_shortcut(cx));
                }
            }).into_any_element());
            items_acc_h += 32.0;

            // Pin / Unpin from Sidebar (current folder)
            let (pin_glyph, pin_label) = if is_pinned_to_sidebar {
                ("\u{E77A}", "Unpin from Sidebar")
            } else {
                ("\u{E718}", "Pin to Sidebar")
            };
            items.push(Self::context_menu_item(
                pin_glyph,
                pin_label,
                {
                    let v = v_close.clone();
                    let dir = cur_dir.clone();
                    let is_pinned = is_pinned_to_sidebar;
                    move |_, _, cx| {
                        let fp = crate::core::path_util::FilesPath::from(dir.clone());
                        v.update(cx, |this, cx| {
                            if is_pinned {
                                this.unpin_folder(fp, cx);
                            } else {
                                this.pin_folder(fp, cx);
                            }
                            this.close_dialog(cx);
                        });
                    }
                },
            ).into_any_element());
            items_acc_h += 32.0;

            items.push(Self::context_menu_separator().into_any_element());
            items_acc_h += 7.0;

            // Open in Windows Terminal (Ctrl+`)
            items.push(Self::context_menu_row("\u{E756}", "Open in Windows Terminal", MenuTrailing::Hint("Ctrl+`"), {
                let v = v_close.clone();
                move |_, _, cx| {
                    v.update(cx, |this, cx| {
                        this.open_terminal_at_current(cx);
                        this.close_dialog(cx);
                    });
                }
            }).into_any_element());
            items_acc_h += 32.0;

            // Properties (Alt+Enter) for current folder
            items.push(Self::context_menu_row("\u{E713}", "Properties", MenuTrailing::Hint("Alt+Enter"), {
                let v = v_close.clone();
                move |_, _, cx| {
                    v.update(cx, |this, cx| {
                        this.close_dialog(cx);
                        this.open_properties_for_selected(cx);
                    });
                }
            }).into_any_element());
            items_acc_h += 32.0;

            // Show more options › — the real classic shell items expand
            // inline on hover (Files parity). Windows only.
            #[cfg(target_os = "windows")]
            {
                let row_y = items_acc_h;
                showmore_off = row_y;
                if !expanded {
                    submenu_offsets.insert(SubmenuKind::ShowMore, row_y);
                    let v_click = v_sub.clone();
                    items.push(Self::submenu_row(
                        "\u{E712}",
                        "Show more options",
                        SubmenuKind::ShowMore,
                        view.clone(),
                        move |_, _, cx| {
                            v_click.update(cx, |this, cx| {
                                this.set_open_submenu(Some(SubmenuKind::ShowMore), cx)
                            });
                        },
                    ).into_any_element());
                    items_acc_h += 32.0;
                } else {
                    // Expanded style: every classic item is inlined below the
                    // app rows - no hover needed. Nested verbs keep flyouts.
                    items.push(Self::context_menu_separator().into_any_element());
                    items_acc_h += 7.0;
                    match classic.as_ref() {
                        Some(entries) if !entries.is_empty() => {
                            let mut next_id: usize = 0;
                            for e in entries.iter() {
                                items_acc_h += if e.separator { 7.0 } else { 32.0 };
                            }
                            let cur_scroll = scroll_offset;
                            let (inline, child_panel) = Self::classic_rows(
                                entries,
                                &classic_open,
                                &[],
                                view.clone(),
                                Vec::new(),
                                cur_dir.clone(),
                                &mut next_id,
                                menu_left,
                                menu_top + fixed_header_h + showmore_off - cur_scroll,
                                fixed_header_h + showmore_off - cur_scroll,
                                284.0,
                                win_w,
                                win_h,
                                flip,
                                mica_geo.as_ref(),
                            );
                            for row in inline {
                                items.push(row.into_any_element());
                            }
                            if let Some(child) = child_panel {
                                active_child_panel = Some(child);
                            }
                        }
                        Some(_) => {
                            let mut nid = 0usize;
                            items.push(Self::classic_note_row(
                                "(No extended options)",
                                &mut nid,
                            ).into_any_element());
                            items_acc_h += 32.0;
                        }
                        None => {
                            let mut nid = 0usize;
                            let txt = if classic_loading {
                                "Loading…"
                            } else {
                                "(Unavailable)"
                            };
                            items.push(Self::classic_note_row(txt, &mut nid).into_any_element());
                            items_acc_h += 32.0;
                        }
                    }
                }
            }
        }

        let total_items_h = items_acc_h + 4.0;
        let total_menu_h = fixed_header_h + total_items_h;
        let bottom_limit = (win_h - 36.0).max(100.0);
        let available_h = (bottom_limit - menu_top).max(160.0);
        let card_h = if total_menu_h > available_h {
            available_h
        } else {
            total_menu_h
        };
        let available_items_h = (card_h - fixed_header_h).max(40.0);
        let max_scroll = (total_items_h - available_items_h).max(0.0);
        let current_scroll = scroll_offset.clamp(0.0, max_scroll);

        // Build active submenu if open (attached to card shell, unclipped)
        let mut active_submenu_card: Option<gpui::Stateful<Div>> = active_child_panel;
        if active_submenu_card.is_none() {
            if let Some(kind) = open_submenu {
                if let Some(&row_y) = submenu_offsets.get(&kind) {
                    let header_or_pad_h = if has_selection { fixed_header_h } else { 4.0 };
                    let row_card_y = header_or_pad_h + row_y - current_scroll;
                    active_submenu_card = match kind {
                        SubmenuKind::Layout => Some(Self::layout_submenu(
                            view.clone(),
                            flip,
                            menu_top,
                            menu_left,
                            row_card_y,
                            win_w,
                            win_h,
                            mica_geo.as_ref(),
                        )),
                        SubmenuKind::SortBy => Some(Self::sort_submenu(
                            view.clone(),
                            &current_sort,
                            flip,
                            menu_top,
                            menu_left,
                            row_card_y,
                            win_w,
                            win_h,
                            mica_geo.as_ref(),
                        )),
                        SubmenuKind::GroupBy => Some(Self::group_submenu(
                            view.clone(),
                            &current_group,
                            flip,
                            menu_top,
                            menu_left,
                            row_card_y,
                            win_w,
                            win_h,
                            mica_geo.as_ref(),
                        )),
                        SubmenuKind::New => Some(Self::new_submenu(
                            view.clone(),
                            flip,
                            menu_top,
                            menu_left,
                            row_card_y,
                            win_w,
                            win_h,
                            mica_geo.as_ref(),
                        )),
                        SubmenuKind::OpenWith => {
                            let openwith_entry = classic.as_ref().and_then(|entries| {
                                entries.iter().find(|e| {
                                    let l = e.label.to_lowercase();
                                    l.contains("open with") || l.contains("openwith")
                                })
                            });
                            let openwith_children = openwith_entry.map(|e| e.children.as_slice());
                            Some(Self::openwith_submenu(
                                openwith_children,
                                selected_path.clone(),
                                sel_paths.clone(),
                                cur_dir.clone(),
                                view.clone(),
                                flip,
                                menu_top,
                                menu_left,
                                row_card_y,
                                win_w,
                                win_h,
                                mica_geo.as_ref(),
                            ))
                        }
                        SubmenuKind::Compress => {
                            let is_archive = selected_path
                                .as_ref()
                                .and_then(|p| p.file_name())
                                .map(|name| {
                                    let name = name.to_string_lossy().to_lowercase();
                                    name.ends_with(".zip")
                                        || name.ends_with(".7z")
                                        || name.ends_with(".tar")
                                        || name.ends_with(".tar.gz")
                                        || name.ends_with(".rar")
                                })
                                .unwrap_or(false);
                            Some(Self::compress_submenu(
                                view.clone(),
                                flip,
                                is_archive,
                                selected_path.clone(),
                                menu_top,
                                menu_left,
                                row_card_y,
                                win_w,
                                win_h,
                                mica_geo.as_ref(),
                            ))
                        }
                        SubmenuKind::SendTo => {
                            let sendto_entry = classic.as_ref().and_then(|entries| {
                                entries.iter().find(|e| {
                                    let l = e.label.to_lowercase();
                                    l.contains("send to") || l.contains("sendto")
                                })
                            });
                            let sendto_children = sendto_entry.map(|e| e.children.as_slice());
                            Some(Self::sendto_submenu(
                                sendto_children,
                                sel_paths.clone(),
                                cur_dir.clone(),
                                view.clone(),
                                flip,
                                menu_top,
                                menu_left,
                                row_card_y,
                                win_w,
                                win_h,
                                mica_geo.as_ref(),
                            ))
                        }
                        SubmenuKind::FileTags => Some(Self::file_tags_submenu(
                            view.clone(),
                            &available_tags,
                            &item_tags,
                            flip,
                            menu_top,
                            menu_left,
                            row_card_y,
                            win_w,
                            win_h,
                            mica_geo.as_ref(),
                        )),
                        SubmenuKind::ShowMore => {
                            #[cfg(target_os = "windows")]
                            {
                                let filtered_classic = classic.map(|entries| {
                                    entries
                                        .into_iter()
                                        .filter(|e| {
                                            let l = e.label.to_lowercase();
                                            !l.contains("send to") && !l.contains("open with")
                                        })
                                        .collect()
                                });
                                Some(Self::classic_submenu(
                                    filtered_classic,
                                    classic_loading,
                                    classic_open.clone(),
                                    view.clone(),
                                    sel_paths.clone(),
                                    cur_dir.clone(),
                                    flip,
                                    menu_top,
                                    menu_left,
                                    row_card_y,
                                    win_w,
                                    win_h,
                                    mica_geo.as_ref(),
                                ))
                            }
                            #[cfg(not(target_os = "windows"))]
                            {
                                None
                            }
                        }
                    };
                }
            }
        }

        let mut card = flyout_card_shell(menu_left, menu_top, 284.0, card_h, mica_geo.as_ref())
            .id("ctx-menu-card");

        // 1. Fixed Header (Toolbar + separator, stays anchored at top when scrolled)
        if let Some(header) = fixed_header {
            card = card.child(header);
        }

        // 2. Scrollable Items Viewport
        let mut items_inner = div()
            .relative()
            .w_full()
            .flex_col()
            .top(px(-current_scroll));
        if !has_selection {
            items_inner = items_inner.pt(px(4.0));
        }
        for row in items {
            items_inner = items_inner.child(row);
        }
        items_inner = items_inner.pb(px(4.0));

        let mut items_viewport = div()
            .id("ctx-items-viewport")
            .relative()
            .w_full()
            .h(px(available_items_h))
            .overflow_hidden()
            .child(items_inner);

        // 3. Sleek Windows 11 Fluent Scrollbar Thumb (visible when scrollable)
        if max_scroll > 1.0 {
            let scroll_ratio = (current_scroll / max_scroll).clamp(0.0, 1.0);
            let thumb_h = (available_items_h * (available_items_h / total_items_h))
                .clamp(28.0, (available_items_h - 8.0).max(28.0));
            let thumb_travel = (available_items_h - thumb_h).max(0.0);
            let thumb_top = scroll_ratio * thumb_travel;

            let thumb = div()
                .absolute()
                .right(px(3.0))
                .top(px(fixed_header_h + thumb_top))
                .w(px(3.0))
                .h(px(thumb_h))
                .rounded(px(1.5))
                .bg(rgba(0xFFFFFF40u32));
            card = card.child(thumb);
        }

        // 4. Mouse Wheel Scroll Handler (closes open submenus on scroll like WinUI 3)
        let v_wheel = view.clone();
        let wheel_handler = move |event: &ScrollWheelEvent, _: &mut Window, cx: &mut App| {
            cx.stop_propagation();
            let dy: f32 = event.delta.pixel_delta(px(24.0)).y.into();
            v_wheel.update(cx, |this, cx| {
                if let Some(crate::ui::components::dialogs::DialogType::ContextMenu {
                    scroll_offset,
                    open_submenu,
                    ..
                }) = this.state.active_dialog.as_mut() {
                    *open_submenu = None;
                    let max_s = (total_items_h - available_items_h).max(0.0);
                    if max_s > 0.0 {
                        *scroll_offset = (*scroll_offset - dy).clamp(0.0, max_s);
                        cx.notify();
                    }
                }
            });
        };
        card = card.on_scroll_wheel(wheel_handler.clone());
        items_viewport = items_viewport.on_scroll_wheel(wheel_handler);

        card = card.child(items_viewport);

        // 5. Active Submenu Flyout (attached directly to card shell, unclipped)
        if let Some(sub) = active_submenu_card {
            card = card.child(sub);
        }

        card.with_animation(
            "ctx-menu-entrance",
            Animation::new(std::time::Duration::from_millis(120)),
            |elem, delta| elem.opacity(delta),
        )
    }

    pub(crate) fn context_menu_item(
        glyph: &str,
        label: impl Into<String>,
        on_click: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    ) -> Div {
        div()
            .flex()
            .items_center()
            .gap(px(10.0))
            .mx(px(4.0))
            .px(px(8.0))
            .h(px(32.0))
            .rounded(px(4.0))
            .cursor_pointer()
            .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
            .on_mouse_down(MouseButton::Left, on_click)
            .child(
                div()
                    .w(px(20.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(14.0))
                    .font_family(UiConstants::FONT_MDL2)
                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                    .child(glyph.to_string()),
            )
            .child(
                div()
                    .text_size(px(13.0))
                    .font_family(UiConstants::FONT_MAIN)
                    .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                    .child(label.into()),
            )
    }

    pub(crate) fn context_menu_item_with_icon(
        icon_el: impl IntoElement,
        label: impl Into<String>,
        on_click: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    ) -> Div {
        div()
            .flex()
            .items_center()
            .gap(px(10.0))
            .mx(px(4.0))
            .px(px(8.0))
            .h(px(32.0))
            .rounded(px(4.0))
            .cursor_pointer()
            .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
            .on_mouse_down(MouseButton::Left, on_click)
            .child(
                div()
                    .w(px(20.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(icon_el),
            )
            .child(
                div()
                    .text_size(px(13.0))
                    .font_family(UiConstants::FONT_MAIN)
                    .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                    .child(label.into()),
            )
    }

    pub(crate) fn context_menu_item_full(
        icon_el: impl IntoElement,
        label: impl Into<String>,
        trailing: MenuTrailing,
        on_click: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    ) -> Div {
        let mut item = div()
            .flex()
            .items_center()
            .justify_between()
            .mx(px(4.0))
            .px(px(8.0))
            .h(px(32.0))
            .rounded(px(4.0))
            .cursor_pointer()
            .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
            .on_mouse_down(MouseButton::Left, on_click);

        let left = div()
            .flex()
            .items_center()
            .gap(px(10.0))
            .child(
                div()
                    .w(px(20.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(icon_el),
            )
            .child(
                div()
                    .text_size(px(13.0))
                    .font_family(UiConstants::FONT_MAIN)
                    .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                    .child(label.into()),
            );

        item = item.child(left);

        match trailing {
            MenuTrailing::None => {}
            MenuTrailing::Hint(hint) => {
                item = item.child(
                    div()
                        .text_size(px(12.0))
                        .font_family(UiConstants::FONT_MAIN)
                        .text_color(rgba(UiConstants::TEXT_MUTED_RGBA))
                        .child(hint),
                );
            }
            MenuTrailing::Chevron => {
                item = item.child(
                    div()
                        .text_size(px(10.0))
                        .font_family(UiConstants::FONT_MDL2)
                        .text_color(rgba(UiConstants::TEXT_MUTED_RGBA))
                        .child("\u{E76C}"),
                );
            }
        }

        item
    }

    /// 48px tall context menu item (for primary items like Open, New Folder, etc.)
    /// Per DESIGN_SPEC.md: primary items are 48px tall
    pub(crate) fn context_menu_item_48px(
        glyph: &str,
        label: &str,
        on_click: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    ) -> Div {
        div()
            .flex()
            .items_center()
            .gap(px(10.0))
            .mx(px(4.0))
            .px(px(8.0))
            .h(px(44.0))
            .rounded(px(4.0))
            .cursor_pointer()
            .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
            .on_mouse_down(MouseButton::Left, on_click)
            .child(
                div()
                    .w(px(20.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(16.0))
                    .font_family(UiConstants::FONT_MDL2)
                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                    .child(glyph.to_string()),
            )
            .child(
                div()
                    .text_size(px(13.5))
                    .font_family(UiConstants::FONT_MAIN)
                    .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                    .child(label.to_string()),
            )
    }

    /// Standard 32px context menu row with optional trailing hint/chevron.
    pub(crate) fn context_menu_row(
        glyph: &str,
        label: &str,
        trailing: MenuTrailing,
        on_click: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    ) -> Div {
        let trailing_el: Option<Div> = match trailing {
            MenuTrailing::None => None,
            MenuTrailing::Hint(hint) => Some(
                div()
                    .text_size(px(11.5))
                    .text_color(rgba(UiConstants::TEXT_MUTED_RGBA))
                    .child(hint.to_string()),
            ),
            MenuTrailing::Chevron => Some(
                div()
                    .text_size(px(11.0))
                    .font_family(UiConstants::FONT_MDL2)
                    .text_color(rgba(UiConstants::TEXT_MUTED_RGBA))
                    .child("\u{E76C}"),
            ),
        };
        div()
            .flex()
            .items_center()
            .gap(px(10.0))
            .mx(px(4.0))
            .px(px(8.0))
            .h(px(32.0))
            .rounded(px(4.0))
            .cursor_pointer()
            .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
            .on_mouse_down(MouseButton::Left, on_click)
            .child(
                div()
                    .w(px(20.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(14.0))
                    .font_family(UiConstants::FONT_MDL2)
                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                    .child(glyph.to_string()),
            )
            .child(
                div()
                    .text_size(px(13.0))
                    .font_family(UiConstants::FONT_MAIN)
                    .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                    .child(label.to_string()),
            )
            .child(div().flex_grow())
            .children(trailing_el)
    }

    /// Icon-only toolbar row at the top of the item menu (Files parity:
    /// Cut / Copy / Paste / Share / Delete / Properties, no labels).
    pub(crate) fn context_menu_toolbar(
        buttons: Vec<(
            &'static str,
            &'static str,
            Box<dyn Fn(&MouseDownEvent, &mut Window, &mut App) + 'static>,
        )>,
        view: Entity<crate::app::AppWindow>,
    ) -> Div {
        let mut row = div()
            .flex()
            .items_center()
            .gap(px(2.0))
            .px(px(8.0))
            .py(px(3.0));
        for (idx, (glyph, tip, action)) in buttons.into_iter().enumerate() {
            let v_tip = view.clone();
            let tip_text = tip.to_string();
            row = row.child(
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .w(px(34.0))
                    .h(px(28.0))
                    .rounded(px(4.0))
                    .cursor_pointer()
                    .id(("ctxtool", idx))
                    .on_hover(
                        move |hovering: &bool, window: &mut Window, cx: &mut App| {
                            let pos = window.mouse_position();
                            let text = if *hovering {
                                Some(tip_text.clone())
                            } else {
                                None
                            };
                            v_tip.update(cx, |this, _| {
                                this.menu_tip = text.map(|t| (t, pos));
                            });
                        },
                    )
                    .hover(|s| s.bg(rgba(0xFFFFFF15u32)))
                    .on_mouse_down(MouseButton::Left, action)
                    .child(
                        div()
                            .text_size(px(16.0))
                            .font_family(UiConstants::FONT_MDL2)
                            .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                            .child(glyph.to_string()),
                    ),
            );
        }
        row
    }

    /// Separator line for context menu sections
    pub(crate) fn context_menu_separator() -> Div {
        div()
            .h(px(1.0))
            .bg(rgba(UiConstants::DIVIDER_RGBA))
            .mx(px(8.0))
            .my(px(3.0))
    }

    /// Section header label for grouped context menu sections (e.g. "Sort By", "View")
    pub(crate) fn context_menu_section_header(label: &str) -> Div {
        div().flex().items_center().px(px(12.0)).py(px(6.0)).child(
            div()
                .text_size(px(11.0))
                .text_color(rgba(0xFFFFFF60u32))
                .font_weight(FontWeight::SEMIBOLD)
                .child(label.to_string()),
        )
    }

    /// Build the "Unhide Sections" dialog showing hidden groups with checkboxes.
    pub(crate) fn build_unhide_sections_dialog(
        hidden_sections: &[crate::ui::components::sidebar_state::SectionId],
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let v_close = view.clone();
        div()
            .flex_col()
            .gap(px(12.0))
            .min_w(px(280.0))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(px(16.0))
                            .font_weight(FontWeight(600.0))
                            .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                            .child("Hidden Groups"),
                    )
                    .child(
                        div()
                            .cursor_pointer()
                            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                v_close.update(cx, |this, cx| this.close_dialog(cx));
                            })
                            .child(
                                div()
                                    .text_size(px(14.0))
                                    .font_family(UiConstants::FONT_MDL2)
                                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                    .child("\u{E8BB}"),
                            ),
                    ),
            )
            .child(div().h(px(1.0)).bg(rgba(UiConstants::DIVIDER_RGBA)))
            .children(hidden_sections.iter().map(|section| {
                let v_unhide = view.clone();
                let label = format!("{:?}", section);
                let section_id = *section;
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .px(px(12.0))
                    .py(px(8.0))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        v_unhide.update(cx, |this, cx| {
                            this.state.sidebar.unhide_section(section_id);
                            this.close_dialog(cx);
                        });
                    })
                    .child(
                        div()
                            .text_size(px(14.0))
                            .font_family(UiConstants::FONT_MDL2)
                            .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                            .child("\u{E7B3}"),
                    )
                    .child(
                        div()
                            .text_size(px(13.0))
                            .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                            .child(label.to_string()),
                    )
            }))
            .child(
                div().flex().justify_center().mt(px(8.0)).child(
                    div()
                        .px(px(16.0))
                        .py(px(6.0))
                        .bg(rgba(0xFFFFFF10u32))
                        .rounded(px(4.0))
                        .cursor_pointer()
                        .hover(|s| s.bg(rgba(0xFFFFFF20u32)))
                        .on_mouse_down(MouseButton::Left, {
                            let v = view.clone();
                            let all_hidden: Vec<_> = hidden_sections.to_vec();
                            move |_, _, cx| {
                                v.update(cx, |this, cx| {
                                    for sid in &all_hidden {
                                        this.state.sidebar.unhide_section(*sid);
                                    }
                                    this.close_dialog(cx);
                                });
                            }
                        })
                        .child(
                            div()
                                .text_size(px(13.0))
                                .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                                .child("Unhide All"),
                        ),
                ),
            )
    }

    /// Build a context menu for sidebar items with actions like Open, Pin/Unpin, Open in New Tab
    pub(crate) fn build_sidebar_context_menu(
        target: &SidebarMenuTarget,
        _position: Point<Pixels>,
        reorder_mode: bool,
        hidden_sections: Vec<crate::ui::components::sidebar_state::SectionId>,
        view: Entity<crate::app::AppWindow>,
        cx: &mut Context<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let v_close = view.clone();
        let v_open = view.clone();
        let v_new_tab = view.clone();
        let v_new_window = view.clone();
        let v_pin = view.clone();
        let v_unpin = view.clone();
        let v_reorder = view.clone();
        let v_exit_reorder = view.clone();
        let v_hide = view.clone();

        match target {
            SidebarMenuTarget::Item {
                label,
                path,
                is_pinned,
            } => {
                let path_for_open = path.clone();
                let path_for_tab = path.clone();
                let path_for_window = path.clone();
                let path_for_unpin = path.clone();
                let path_for_pin_action = path.clone();

                let reorder_active = reorder_mode;

                let mut list = div().flex_col().py(px(4.0));

                if reorder_active {
                    list = list.child(Self::context_menu_item(
                        "\u{E711}",
                        "Exit Reorder Mode",
                        move |_, _, cx| {
                            v_exit_reorder.update(cx, |this, cx| {
                                this.state.sidebar.exit_reorder_mode();
                                this.close_dialog(cx);
                            });
                        },
                    ));
                } else {
                    list = list
                        .child(Self::context_menu_item(
                            "\u{E8DA}",
                            "Open",
                            move |_, _, cx| {
                                if let Some(ref p) = path_for_open {
                                    let path = crate::core::path_util::FilesPath::new(p);
                                    v_open.update(cx, |this, cx| {
                                        this.navigate(path, cx);
                                        this.close_dialog(cx);
                                    });
                                }
                            },
                        ))
                        .child(Self::context_menu_item(
                            "\u{E8A7}",
                            "Open in New Tab",
                            move |_, _, cx| {
                                if let Some(ref p) = path_for_tab {
                                    let path = crate::core::path_util::FilesPath::new(p);
                                    v_new_tab.update(cx, |this, cx| {
                                        this.open_in_new_tab(path, cx);
                                        this.close_dialog(cx);
                                    });
                                }
                            },
                        ))
                        .child(Self::context_menu_item(
                            "\u{E8A7}",
                            "Open in New Window",
                            move |_, _, cx| {
                                if let Some(ref p) = path_for_window {
                                    let path = crate::core::path_util::FilesPath::new(p);
                                    v_new_window.update(cx, |this, cx| {
                                        this.open_in_new_window(path, cx);
                                        this.close_dialog(cx);
                                    });
                                }
                            },
                        ))
                        .child(
                            div()
                                .h(px(1.0))
                                .bg(rgba(UiConstants::DIVIDER_RGBA))
                                .mx(px(8.0))
                                .my(px(4.0)),
                        )
                        .child(if *is_pinned {
                            let v = v_unpin.clone();
                            let p = path_for_unpin.clone();
                            div().child(Self::context_menu_item(
                                "\u{E718}",
                                "Unpin from sidebar",
                                move |_, _, cx| {
                                    if let Some(ref path_str) = p {
                                        let path = crate::core::path_util::FilesPath::new(path_str);
                                        v.update(cx, |this, cx| {
                                            this.unpin_folder(path, cx);
                                            this.close_dialog(cx);
                                        });
                                    }
                                },
                            ))
                        } else {
                            let v = v_pin.clone();
                            let p = path_for_pin_action.clone();
                            div().child(Self::context_menu_item(
                                "\u{E718}",
                                "Pin to sidebar",
                                move |_, _, cx| {
                                    if let Some(ref path_str) = p {
                                        let path = crate::core::path_util::FilesPath::new(path_str);
                                        v.update(cx, |this, cx| {
                                            this.pin_folder(path, cx);
                                            this.close_dialog(cx);
                                        });
                                    }
                                },
                            ))
                        })
                        .child(
                            div()
                                .h(px(1.0))
                                .bg(rgba(UiConstants::DIVIDER_RGBA))
                                .mx(px(8.0))
                                .my(px(4.0)),
                        )
                        .child(Self::context_menu_item(
                            "\u{E8C8}",
                            "Reorder Sidebar",
                            move |_, _, cx| {
                                v_reorder.update(cx, |this, cx| {
                                    this.state.sidebar.enter_reorder_mode();
                                    this.close_dialog(cx);
                                });
                            },
                        ))
                        .child(Self::context_menu_item(
                            "\u{E713}",
                            "Properties",
                            {
                                let v_props = view.clone();
                                let p_props = path.clone();
                                move |_, _, cx| {
                                    if let Some(ref p) = p_props {
                                        let path_buf = std::path::PathBuf::from(p);
                                        v_props.update(cx, |this, cx| {
                                            this.close_dialog(cx);
                                            this.open_properties_for_paths(vec![path_buf], cx);
                                        });
                                    }
                                }
                            },
                        ));
                }
                list
            }
            SidebarMenuTarget::Group { section_id } => {
                let sid = *section_id;
                let reorder_active = reorder_mode;
                let v_reorder_group = view.clone();

                let mut list = div().flex_col().py(px(4.0));

                if reorder_active {
                    list = list.child(Self::context_menu_item(
                        "\u{E711}",
                        "Exit Reorder Mode",
                        move |_, _, cx| {
                            v_exit_reorder.update(cx, |this, cx| {
                                this.state.sidebar.exit_reorder_mode();
                                this.close_dialog(cx);
                            });
                        },
                    ));
                } else {
                    list = list
                        .child(Self::context_menu_item(
                            "\u{E7B3}",
                            "Hide Group",
                            move |_, _, cx| {
                                v_hide.update(cx, |this, cx| {
                                    this.state.sidebar.hide_section(sid);
                                    this.close_dialog(cx);
                                });
                            },
                        ))
                        .child(Self::context_menu_item(
                            "\u{E8C8}",
                            "Reorder Sidebar",
                            move |_, _, cx| {
                                v_reorder_group.update(cx, |this, cx| {
                                    this.state.sidebar.enter_reorder_mode();
                                    this.close_dialog(cx);
                                });
                            },
                        ))
                        .child(Self::context_menu_item(
                            "\u{E8C8}",
                            "Reorder Items in Group",
                            move |_, _, cx| {
                                let v = view.clone();
                                v.update(cx, |this, cx| {
                                    // This will trigger the reorder items dialog
                                    this.state.active_dialog =
                                        Some(DialogType::ReorderSidebarItems);
                                    this.close_dialog(cx);
                                });
                            },
                        ));
                }
                list
            }
            SidebarMenuTarget::Background => {
                let reorder_active = reorder_mode;

                let mut list = div().flex_col().py(px(4.0));

                if reorder_active {
                    list = list.child(Self::context_menu_item(
                        "\u{E711}",
                        "Exit Reorder Mode",
                        move |_, _, cx| {
                            v_exit_reorder.update(cx, |this, cx| {
                                this.state.sidebar.exit_reorder_mode();
                                this.close_dialog(cx);
                            });
                        },
                    ));
                } else {
                    list = list.child(Self::context_menu_item(
                        "\u{E8C8}",
                        "Reorder Sidebar",
                        move |_, _, cx| {
                            v_reorder.update(cx, |this, cx| {
                                this.state.sidebar.enter_reorder_mode();
                                this.close_dialog(cx);
                            });
                        },
                    ));

                    // Add "Show Hidden Groups..." button (always visible)
                    let hidden_sections = hidden_sections.clone();
                    if !hidden_sections.is_empty() {
                        list = list.child(
                            div()
                                .h(px(1.0))
                                .bg(rgba(UiConstants::DIVIDER_RGBA))
                                .mx(px(8.0))
                                .my(px(4.0)),
                        );

                        for section in &hidden_sections {
                            let v_unhide = view.clone();
                            let section_id = *section;
                            let label = format!("Show {:?}", section);
                            list = list.child(Self::context_menu_item(
                                "\u{E7B3}",
                                &label,
                                move |_, _, cx| {
                                    v_unhide.update(cx, |this, cx| {
                                        this.state.sidebar.unhide_section(section_id);
                                        this.close_dialog(cx);
                                    });
                                },
                            ));
                        }
                        // Add a "Show All" option
                        let v_show_all = view.clone();
                        let all_hidden = hidden_sections.clone();
                        list = list.child(Self::context_menu_item(
                            "\u{E7B3}",
                            "Show All Hidden Groups",
                            move |_, _, cx| {
                                v_show_all.update(cx, |this, cx| {
                                    for sid in &all_hidden {
                                        this.state.sidebar.unhide_section(*sid);
                                    }
                                    this.close_dialog(cx);
                                });
                            },
                        ));
                    }
                }
                list
            }
        }
    }

    /// Build a context menu for tab bar right-click with full Files-community options.
    pub(crate) fn build_tab_context_menu(
        tab_index: usize,
        _position: Point<Pixels>,
        tab_count: usize,
        is_pinned: bool,
        can_reopen_closed: bool,
        other_windows: Vec<(u64, String)>,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let idx = tab_index;
        let pin_label = if is_pinned { "Unpin Tab" } else { "Pin Tab" };

        let mut menu = div()
            .flex_col()
            .py(px(4.0))
            // New Tab (Ctrl+T)
            .child(Self::context_menu_item_with_shortcut(
                "\u{E710}",
                "New Tab",
                "Ctrl+T",
                {
                    let v = view.clone();
                    move |_, _, cx| {
                        v.update(cx, |this, cx| {
                            this.new_tab(cx);
                            this.close_dialog(cx);
                        });
                    }
                },
            ))
            // New Tab to the Right
            .child(Self::context_menu_item(
                "\u{E710}",
                "New Tab to the Right",
                {
                    let v = view.clone();
                    move |_, _, cx| {
                        v.update(cx, |this, cx| {
                            this.new_tab_to_right(idx, cx);
                            this.close_dialog(cx);
                        });
                    }
                },
            ))
            // Duplicate Tab (Ctrl+Shift+K)
            .child(Self::context_menu_item_with_shortcut(
                "\u{E8C8}",
                "Duplicate Tab",
                "Ctrl+Shift+K",
                {
                    let v = view.clone();
                    move |_, _, cx| {
                        v.update(cx, |this, cx| {
                            this.duplicate_tab(idx, cx);
                            this.close_dialog(cx);
                        });
                    }
                },
            ))
            // Pin / Unpin Tab (Ctrl+Alt+P)
            .child(Self::context_menu_item_with_shortcut(
                "\u{E718}",
                pin_label,
                "Ctrl+Alt+P",
                {
                    let v = view.clone();
                    move |_, _, cx| {
                        v.update(cx, |this, cx| {
                            this.pin_tab(idx, cx);
                            this.close_dialog(cx);
                        });
                    }
                },
            ));

        // Move Tab to New Window (enabled when tab_count > 1)
        if tab_count > 1 {
            menu = menu.child(Self::context_menu_item(
                "\u{E8A7}",
                "Move Tab to New Window",
                {
                    let v = view.clone();
                    move |_, _, cx| {
                        v.update(cx, |this, cx| {
                            this.move_tab_to_new_window(idx, None, cx);
                            this.close_dialog(cx);
                        });
                    }
                },
            ));
        }

        // Move Tab to Another Open Window (if any other files-rs windows exist)
        for (target_win_id, target_title) in other_windows {
            let v = view.clone();
            let item_label = format!("Move Tab to Window ({})", target_title);
            menu = menu.child(
                div()
                    .flex()
                    .items_center()
                    .px(px(12.0))
                    .py(px(6.0))
                    .mx(px(4.0))
                    .rounded(px(4.0))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
                    .on_mouse_down(MouseButton::Left, move |_, win, cx| {
                        v.update(cx, |this, cx| {
                            this.move_tab_to_existing_window(idx, target_win_id, None, win, cx);
                            this.close_dialog(cx);
                        });
                    })
                    .child(
                        div()
                            .text_size(px(14.0))
                            .font_family(UiConstants::FONT_ICONS)
                            .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                            .mr(px(10.0))
                            .child("\u{E8A7}"),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(13.0))
                            .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .child(item_label),
                    ),
            );
        }

        menu = menu
            .child(
                div()
                    .h(px(1.0))
                    .bg(rgba(UiConstants::DIVIDER_RGBA))
                    .mx(px(8.0))
                    .my(px(4.0)),
            )
            // Close Tab (Ctrl+W)
            .child(Self::context_menu_item_with_shortcut(
                "\u{E8BB}",
                "Close Tab",
                "Ctrl+W",
                {
                    let v = view.clone();
                    move |_, win, cx| {
                        v.update(cx, |this, cx| {
                            this.close_tab_with_window(idx, win, cx);
                            this.close_dialog(cx);
                        });
                    }
                },
            ));

        if tab_count > 1 {
            menu = menu.child(Self::context_menu_item_with_shortcut(
                "\u{E8BB}",
                "Close Other Tabs",
                "Ctrl+Alt+W",
                {
                    let v = view.clone();
                    move |_, _, cx| {
                        v.update(cx, |this, cx| {
                            this.close_other_tabs(idx, cx);
                            this.close_dialog(cx);
                        });
                    }
                },
            ));
        }

        if tab_index > 0 {
            menu = menu.child(Self::context_menu_item(
                "\u{E72B}",
                "Close Tabs to the Left",
                {
                    let v = view.clone();
                    move |_, _, cx| {
                        v.update(cx, |this, cx| {
                            this.close_tabs_to_left(idx, cx);
                            this.close_dialog(cx);
                        });
                    }
                },
            ));
        }

        if tab_index + 1 < tab_count {
            menu = menu.child(Self::context_menu_item(
                "\u{E72A}",
                "Close Tabs to the Right",
                {
                    let v = view.clone();
                    move |_, _, cx| {
                        v.update(cx, |this, cx| {
                            this.close_tabs_to_right(idx, cx);
                            this.close_dialog(cx);
                        });
                    }
                },
            ));
        }

        if can_reopen_closed {
            menu = menu
                .child(
                    div()
                        .h(px(1.0))
                        .bg(rgba(UiConstants::DIVIDER_RGBA))
                        .mx(px(8.0))
                        .my(px(4.0)),
                )
                .child(Self::context_menu_item_with_shortcut(
                    "\u{E777}",
                    "Reopen Closed Tab",
                    "Ctrl+Shift+T",
                    {
                        let v = view.clone();
                        move |_, _, cx| {
                            v.update(cx, |this, cx| {
                                this.reopen_closed_tab(cx);
                                this.close_dialog(cx);
                            });
                        }
                    },
                ));
        }

        menu
    }

    /// Build the tab actions & all-tabs flyout menu (Open Tabs list + New Tab/Window + Split Pane options).
    pub(crate) fn build_tab_actions_dropdown(
        _position: Point<Pixels>,
        open_tabs: Vec<(usize, String, String, bool, bool)>,
        can_reopen_closed: bool,
        is_dual_pane: bool,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let mut menu = div().flex_col().py(px(6.0));

        // 1. Open Tabs Header + Quick Switcher List
        menu = menu.child(
            div()
                .px(px(12.0))
                .py(px(4.0))
                .text_size(px(11.0))
                .font_weight(FontWeight(600.0))
                .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                .child(format!("Open Tabs ({})", open_tabs.len())),
        );

        let mut tabs_list = div()
            .id("tab-actions-open-tabs-list")
            .flex()
            .flex_col()
            .max_h(px(220.0))
            .overflow_hidden();

        for (tab_idx, label, _path_str, is_active, is_pinned) in open_tabs.iter().take(10) {
            let t_idx = *tab_idx;
            let active = *is_active;
            let pinned = *is_pinned;
            let v_switch = view.clone();
            let v_close = view.clone();

            tabs_list = tabs_list.child(
                div()
                    .flex()
                    .items_center()
                    .px(px(10.0))
                    .py(px(5.0))
                    .mx(px(4.0))
                    .rounded(px(4.0))
                    .bg(if active {
                        rgba(0x0078D424u32)
                    } else {
                        rgba(0x00000000u32)
                    })
                    .cursor_pointer()
                    .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        v_switch.update(cx, |this, cx| {
                            this.switch_tab(t_idx, cx);
                            this.close_dialog(cx);
                        });
                    })
                    .child(
                        div()
                            .w(px(6.0))
                            .h(px(6.0))
                            .mr(px(8.0))
                            .rounded(px(3.0))
                            .bg(if active {
                                rgb(UiConstants::ACCENT_COLOR_RGB)
                            } else {
                                rgb(0x444444u32)
                            }),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.0))
                            .text_size(px(12.5))
                            .font_weight(if active {
                                FontWeight(600.0)
                            } else {
                                FontWeight(400.0)
                            })
                            .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .child(label.clone()),
                    )
                    .children(if pinned {
                        Some(
                            div()
                                .text_size(px(10.0))
                                .font_family(UiConstants::FONT_ICONS)
                                .text_color(rgb(UiConstants::ACCENT_COLOR_RGB))
                                .mr(px(6.0))
                                .child("\u{E718}"),
                        )
                    } else {
                        None
                    })
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_center()
                            .w(px(18.0))
                            .h(px(18.0))
                            .rounded(px(4.0))
                            .hover(|s| s.bg(rgba(0xFFFFFF22u32)))
                            .on_mouse_down(MouseButton::Left, move |_, win, cx| {
                                cx.stop_propagation();
                                v_close.update(cx, |this, cx| {
                                    this.close_tab_with_window(t_idx, win, cx);
                                    this.close_dialog(cx);
                                });
                            })
                            .child(
                                div()
                                    .text_size(px(9.0))
                                    .font_family(UiConstants::FONT_ICONS)
                                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                    .child("\u{E8BB}"),
                            ),
                    ),
            );
        }

        menu = menu
            .child(tabs_list)
            .child(
                div()
                    .h(px(1.0))
                    .bg(rgba(UiConstants::DIVIDER_RGBA))
                    .mx(px(8.0))
                    .my(px(4.0)),
            )
            // New Tab
            .child(Self::context_menu_item_with_shortcut(
                "\u{E710}",
                "New Tab",
                "Ctrl+T",
                {
                    let v = view.clone();
                    move |_, _, cx| {
                        v.update(cx, |this, cx| {
                            this.new_tab(cx);
                            this.close_dialog(cx);
                        });
                    }
                },
            ))
            // New Window
            .child(Self::context_menu_item_with_shortcut(
                "\u{E8A7}",
                "New Window",
                "Ctrl+N",
                {
                    let v = view.clone();
                    move |_, _, cx| {
                        v.update(cx, |this, cx| {
                            let path = this.state.active_view().primary_path.clone();
                            this.open_in_new_window(path, cx);
                            this.close_dialog(cx);
                        });
                    }
                },
            ));

        if can_reopen_closed {
            menu = menu.child(Self::context_menu_item_with_shortcut(
                "\u{E777}",
                "Reopen Closed Tab",
                "Ctrl+Shift+T",
                {
                    let v = view.clone();
                    move |_, _, cx| {
                        v.update(cx, |this, cx| {
                            this.reopen_closed_tab(cx);
                            this.close_dialog(cx);
                        });
                    }
                },
            ));
        }

        menu = menu
            .child(
                div()
                    .h(px(1.0))
                    .bg(rgba(UiConstants::DIVIDER_RGBA))
                    .mx(px(8.0))
                    .my(px(4.0)),
            )
            // Split Vertically (Ctrl+Shift+V)
            .child(Self::context_menu_item_with_shortcut(
                "\u{E8A0}",
                "Split Vertical",
                "Ctrl+Shift+V",
                {
                    let v = view.clone();
                    move |_, _, cx| {
                        v.update(cx, |this, cx| {
                            this.dispatch_action(
                                crate::core::action_registry::ActionKind::SplitVertical,
                                cx,
                            );
                            this.close_dialog(cx);
                        });
                    }
                },
            ))
            // Split Horizontally (Ctrl+Shift+H)
            .child(Self::context_menu_item_with_shortcut(
                "\u{E8A1}",
                "Split Horizontal",
                "Ctrl+Shift+H",
                {
                    let v = view.clone();
                    move |_, _, cx| {
                        v.update(cx, |this, cx| {
                            this.dispatch_action(
                                crate::core::action_registry::ActionKind::SplitHorizontal,
                                cx,
                            );
                            this.close_dialog(cx);
                        });
                    }
                },
            ))
            // Toggle Split Pane / Close Active Pane
            .child(Self::context_menu_item(
                "\u{E762}",
                if is_dual_pane {
                    "Close Active Split Pane"
                } else {
                    "Open Split Pane"
                },
                {
                    let v = view.clone();
                    move |_, _, cx| {
                        v.update(cx, |this, cx| {
                            this.toggle_dual_pane(cx);
                            this.close_dialog(cx);
                        });
                    }
                },
            ));

        menu
    }

    /// Helper to build a context menu item with shortcut label
    fn context_menu_item_with_shortcut(
        icon: &'static str,
        label: &'static str,
        shortcut: &'static str,
        on_click: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    ) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .px(px(8.0))
            .py(px(6.0))
            .rounded(px(4.0))
            .mx(px(4.0))
            .cursor_pointer()
            .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
            .on_mouse_down(MouseButton::Left, move |ev, win, cx| {
                cx.stop_propagation();
                on_click(ev, win, cx);
            })
            .child(
                div()
                    .text_size(px(14.0))
                    .font_family(UiConstants::FONT_ICONS)
                    .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                    .child(icon),
            )
            .child(
                div()
                    .flex_grow()
                    .ml(px(8.0))
                    .text_size(px(12.0))
                    .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                    .child(label),
            )
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(rgba(UiConstants::TEXT_MUTED_RGBA))
                    .child(shortcut),
            )
    }

    /// Build column visibility toggle menu (shown on right-click of column header)
    pub(crate) fn build_column_visibility_menu(
        _position: Point<Pixels>,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let v_close = view.clone();

        div()
            .flex_col()
            .py(px(4.0))
            .child(
                div()
                    .px(px(8.0))
                    .py(px(6.0))
                    .text_size(AbsoluteLength::Pixels(px(12.0)))
                    .text_color(rgb(UiConstants::TEXT_SECONDARY_RGB))
                    .child("Click column header to sort"),
            )
            .child(
                div()
                    .h(px(1.0))
                    .bg(rgba(UiConstants::DIVIDER_RGBA))
                    .mx(px(8.0))
                    .my(px(4.0)),
            )
            .child(Self::context_menu_item("\u{E8FB}", "Name", {
                let v = v_close.clone();
                move |_, _, cx| {
                    v.update(cx, |this, cx| {
                        this.close_dialog(cx);
                    });
                }
            }))
            .child(Self::context_menu_item("\u{E787}", "Date Modified", {
                let v = v_close.clone();
                move |_, _, cx| {
                    v.update(cx, |this, cx| {
                        this.toggle_column_visibility("Date Modified".to_string(), cx);
                        this.close_dialog(cx);
                    });
                }
            }))
            .child(Self::context_menu_item("\u{E8A5}", "Type", {
                let v = v_close.clone();
                move |_, _, cx| {
                    v.update(cx, |this, cx| {
                        this.toggle_column_visibility("Type".to_string(), cx);
                        this.close_dialog(cx);
                    });
                }
            }))
            .child(Self::context_menu_item("\u{E8FD}", "Size", {
                let v = v_close.clone();
                move |_, _, cx| {
                    v.update(cx, |this, cx| {
                        this.toggle_column_visibility("Size".to_string(), cx);
                        this.close_dialog(cx);
                    });
                }
            }))
    }
}

impl crate::ui::components::dialogs::DialogsView {
    /// Chevron row that opens a hover flyout submenu (Files parity).
    pub(crate) fn submenu_row(
        glyph: &str,
        label: &str,
        kind: SubmenuKind,
        view: Entity<crate::app::AppWindow>,
        on_click: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    ) -> gpui::Stateful<Div> {
        let v_hover = view.clone();
        let stateful = div()
            .relative()
            .w_full()
            .child(Self::context_menu_row(glyph, label, MenuTrailing::Chevron, on_click))
            .id(("ctx-submenu", kind as usize));
        stateful.on_hover(move |hovering: &bool, _, cx| {
            // Flyout panels live outside the row bounds, so an immediate
            // close on leave would kill the panel mid-travel (including
            // diagonal cursor paths). Grace-close instead; entering the
            // panel (or returning to the row) cancels it.
            if *hovering {
                v_hover.update(cx, |this, cx| {
                    this.submenu_row_hovered = true;
                    this.submenu_grace_cancel();
                    this.set_open_submenu(Some(kind), cx);
                });
            } else {
                let v = v_hover.clone();
                v.update(cx, |this, cx| {
                    this.submenu_row_hovered = false;
                    this.submenu_grace_close(
                        |this, cx| this.set_open_submenu(None, cx),
                        cx,
                    )
                });
            }
        })
    }

    /// Flyout panel anchored to the right of its parent row (or left if flipped).
    pub(crate) fn submenu_panel(
        rows: Vec<Div>,
        kind: SubmenuKind,
        view: Entity<crate::app::AppWindow>,
        flip: bool,
        menu_top: f32,
        menu_left: f32,
        row_card_y: f32,
        win_w: f32,
        win_h: f32,
        mica_geo: Option<&crate::wallpaper_mica::MicaGeo>,
    ) -> gpui::Stateful<Div> {
        let v = view.clone();
        let panel_w = 220.0f32;
        let fits_right = menu_left + 284.0 + panel_w <= win_w - 16.0;
        let (rel_left, panel_abs_x) = if fits_right && !flip {
            (276.0f32, menu_left + 276.0)
        } else {
            let abs_x = (menu_left - panel_w + 8.0).max(8.0);
            (abs_x - menu_left, abs_x)
        };

        let row_abs_y = menu_top + row_card_y;
        let bottom_limit = (win_h - 36.0).max(100.0);
        let fly_h = rows.len() as f32 * 32.0 + 8.0;
        let mut adjusted_top = -4.0f32;
        if row_abs_y - 4.0 + fly_h > bottom_limit {
            let overflow = (row_abs_y - 4.0 + fly_h) - bottom_limit;
            adjusted_top = (-4.0 - overflow).max(40.0 - row_abs_y);
        }
        let panel_abs_top = row_abs_y + adjusted_top;
        let available_h = (bottom_limit - panel_abs_top).max(120.0);
        let scroll = fly_h > available_h;
        let card_h = if scroll { available_h } else { fly_h };

        let mut card = flyout_card_shell(panel_abs_x, panel_abs_top, panel_w, card_h, mica_geo);
        card = card.absolute().top(px(row_card_y + adjusted_top)).left(px(rel_left));

        let mut content = div()
            .relative()
            .flex_col()
            .w_full()
            .py(px(4.0));
        for row in rows {
            content = content.child(row);
        }
        let content = content.id(("sub-content", kind as usize));
        let content = if scroll {
            content.max_h(px(available_h)).overflow_scroll()
        } else {
            content.h(px(card_h)).overflow_hidden()
        };
        card = card.child(content);

        let mut panel = card.id(("ctx-panel", kind as usize));
        panel = panel
            .on_hover(move |hovering: &bool, _, cx| {
                if *hovering {
                    v.update(cx, |this, cx| {
                        this.submenu_panel_hovered = true;
                        this.submenu_grace_cancel();
                        this.set_open_submenu(Some(kind), cx);
                    });
                } else {
                    let v2 = v.clone();
                    v2.update(cx, |this, cx| {
                        this.submenu_panel_hovered = false;
                        this.submenu_grace_close(
                            |this, cx| this.set_open_submenu(None, cx),
                            cx,
                        )
                    });
                }
            })
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation());
        panel
    }

    /// "Show more options" flyout: the real classic shell items with their
    /// real extension icons, expanded inline on hover (Files parity).
    pub(crate) fn classic_submenu(
        classic: Option<Vec<ClassicMenuEntry>>,
        loading: bool,
        open: Vec<usize>,
        view: Entity<crate::app::AppWindow>,
        paths: Vec<std::path::PathBuf>,
        dir: std::path::PathBuf,
        flip: bool,
        menu_top: f32,
        menu_left: f32,
        row_card_y: f32,
        win_w: f32,
        win_h: f32,
        mica_geo: Option<&crate::wallpaper_mica::MicaGeo>,
    ) -> gpui::Stateful<Div> {
        let row_abs_y = menu_top + row_card_y;
        let mut fly_h = 8.0f32;
        match classic.as_ref() {
            Some(entries) if !entries.is_empty() => {
                for e in entries.iter() {
                    fly_h += if e.separator { 7.0 } else { 32.0 };
                }
            }
            _ => {
                fly_h += 32.0;
            }
        }
        let panel_w = 260.0f32;
        let fits_right = menu_left + 284.0 + panel_w <= win_w - 16.0;
        let (rel_left, panel_abs_x, flipped_left) = if fits_right && !flip {
            (276.0f32, menu_left + 276.0, false)
        } else {
            let abs_x = (menu_left - panel_w + 8.0).max(8.0);
            (abs_x - menu_left, abs_x, true)
        };

        // Natural WinUI 3 placement: align top with invoking row (-4px),
        // smoothly shift up if extending below window bottom (above status bar).
        let bottom_limit = (win_h - 36.0).max(100.0);
        let mut adjusted_top = -4.0f32;
        if row_abs_y - 4.0 + fly_h > bottom_limit {
            let overflow = (row_abs_y - 4.0 + fly_h) - bottom_limit;
            adjusted_top = (-4.0 - overflow).max(40.0 - row_abs_y);
        }
        let panel_abs_top = row_abs_y + adjusted_top;
        let available_h = (bottom_limit - panel_abs_top).max(120.0);
        let scroll = fly_h > available_h;
        let card_h = if scroll { available_h } else { fly_h };

        let (rows, child_panel): (Vec<gpui::Stateful<Div>>, Option<gpui::Stateful<Div>>) = match classic {
            Some(entries) if !entries.is_empty() => {
                let mut next_id: usize = 0;
                Self::classic_rows(
                    &entries,
                    &open,
                    &[],
                    view.clone(),
                    paths,
                    dir,
                    &mut next_id,
                    panel_abs_x,
                    panel_abs_top,
                    0.0,
                    panel_w,
                    win_w,
                    win_h,
                    flipped_left,
                    mica_geo,
                )
            }
            Some(_) => {
                (vec![Self::classic_note_row("(No extended options)", &mut 0usize)], None)
            }
            None if loading => (vec![Self::classic_note_row("Loading\u{2026}", &mut 0usize)], None),
            None => (vec![Self::classic_note_row("(Unavailable)", &mut 0usize)], None),
        };

        let mut card = flyout_card_shell(panel_abs_x, panel_abs_top, panel_w, card_h, mica_geo);
        card = card.absolute().top(px(row_card_y + adjusted_top)).left(px(rel_left));

        let mut content = div()
            .relative()
            .flex_col()
            .w_full()
            .py(px(4.0));
        for row in rows {
            content = content.child(row);
        }
        let content = content.id(("sub-content", SubmenuKind::ShowMore as usize));
        let content = if scroll {
            content.max_h(px(available_h)).overflow_scroll()
        } else {
            content.h(px(card_h)).overflow_hidden()
        };
        card = card.child(content);
        if let Some(child) = child_panel {
            card = card.child(child);
        }

        let mut panel = card.id(("ctx-panel", SubmenuKind::ShowMore as usize));
        panel = panel
            .on_hover({
                let v = view.clone();
                move |hovering: &bool, _, cx| {
                    if *hovering {
                        v.update(cx, |this, cx| {
                            this.submenu_panel_hovered = true;
                            this.submenu_grace_cancel();
                            this.set_open_submenu(Some(SubmenuKind::ShowMore), cx)
                        });
                    } else {
                        let v2 = v.clone();
                        v2.update(cx, |this, cx| {
                            this.submenu_panel_hovered = false;
                            this.submenu_grace_close(
                                |this, cx| this.set_open_submenu(None, cx),
                                cx,
                            )
                        });
                    }
                }
            })
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation());
        panel
    }

    /// Non-interactive note row (loading / empty states).
    fn classic_note_row(label: &str, next_id: &mut usize) -> gpui::Stateful<Div> {
        let id = *next_id;
        *next_id += 1;
        div()
            .flex()
            .items_center()
            .mx(px(4.0))
            .px(px(8.0))
            .h(px(32.0))
            .rounded(px(4.0))
            .id(("classic", id))
            .child(
                div()
                    .text_size(px(13.0))
                    .text_color(rgba(0xFFFFFF60u32))
                    .child(label.to_string()),
            )
    }

    /// Fallback MDL2 glyph for classic verbs that ship no bitmap (Files shows
    /// an icon for every standard verb; the shell only provides bitmaps for
    /// some). Matched case-insensitively on the mnemonic-stripped label.
    fn classic_fallback_glyph(label: &str) -> Option<&'static str> {
        let l = label.to_lowercase();
        let has = |s: &str| l.contains(s);
        if has("pin to") || has("add to") && has("favorites") || l.starts_with("pin ") {
            Some("\u{E718}")
        } else if has("search") {
            Some("\u{E721}")
        } else if has("restore previous") || has("version history") {
            Some("\u{E7AD}")
        } else if has("give access to") || has("share") {
            Some("\u{E72D}")
        } else if has("rename") || has("powerrename") {
            Some("\u{E8AC}")
        } else if has("delete") || has("remove") {
            Some("\u{E74D}")
        } else if has("terminal") || has("powershell") || has("command prompt") || has("warp") {
            Some("\u{E756}")
        } else if has("properties") || has("file locksmith") {
            Some("\u{E946}")
        } else if l.starts_with("open") || has("play with") || has("run as") {
            Some("\u{E8DA}")
        } else if has("cut") {
            Some("\u{E8C6}")
        } else if has("copy") {
            Some("\u{E8C8}")
        } else if has("paste") {
            Some("\u{E77F}")
        } else if has("new folder") || has("new ") {
            Some("\u{E8B7}")
        } else if has("compress") || has("zip") || has("rar") || has("7-zip") {
            Some("\u{E8B7}")
        } else if has("extract") {
            Some("\u{E896}")
        } else if has("scan") || has("kaspersky") || has("antivirus") {
            Some("\u{E72E}")
        } else if has("edit") || has("notepad") {
            Some("\u{E70F}")
        } else if has("print") {
            Some("\u{E749}")
        } else if has("send to") || has("bluetooth") || has("mail recipient") {
            Some("\u{E725}")
        } else if has("run as administrator") || has("run as admin") || has("run with") {
            Some("\u{E7EF}")
        } else if has("open with") {
            Some("\u{E8DA}")
        } else if has("move to") || has("copy to") || has("move here") || has("copy here") {
            Some("\u{E8C8}")
        } else if has("include in library") || has("playlist") || has("enqueue") || has("play") {
            Some("\u{E8B2}")
        } else if has("add to") && !has("favorites") {
            Some("\u{E710}")
        } else if has("set as") || has("wallpaper") || has("background") || has("rotate") {
            Some("\u{E8B1}")
        } else if has("mount") || has("eject") {
            Some("\u{E8BB}")
        } else if has("install") || has("uninstall") {
            Some("\u{E8B7}")
        } else if has("troubleshoot") || has("compatibility") {
            Some("\u{E9BD}")
        } else if has("backup") || has("onedrive") || has("dropbox") {
            Some("\u{E895}")
        } else if has("copy link") || has("view online") || has("view in") {
            Some("\u{E8A1}")
        } else if has("compare") || has("checksum") || has("hash") || has("crc") || has("sha-") {
            Some("\u{E9D5}")
        } else if has("configure") || has("settings") || has("options") {
            Some("\u{E713}")
        } else if has("upload") || has("sharex") {
            Some("\u{E725}")
        } else if has("teracopy") || has("copy handler") {
            Some("\u{E8C8}")
        } else {
            None
        }
    }

    /// Build classic rows recursively. `prefix` is the index path of these
    /// rows; `open` is the currently expanded path.
    #[allow(clippy::too_many_arguments)]
    fn classic_rows(
        entries: &[ClassicMenuEntry],
        open: &[usize],
        prefix: &[usize],
        view: Entity<crate::app::AppWindow>,
        paths: Vec<std::path::PathBuf>,
        dir: std::path::PathBuf,
        next_id: &mut usize,
        panel_abs_x: f32,
        panel_abs_top: f32,
        panel_card_top: f32,
        panel_w: f32,
        win_w: f32,
        win_h: f32,
        flipped_left: bool,
        mica_geo: Option<&crate::wallpaper_mica::MicaGeo>,
    ) -> (Vec<gpui::Stateful<Div>>, Option<gpui::Stateful<Div>>) {
        let mut rows = Vec::new();
        let mut active_child_panel: Option<gpui::Stateful<Div>> = None;
        let mut acc_h = 0.0f32;
        for (i, entry) in entries.iter().enumerate() {
            let row_off = acc_h;
            acc_h += if entry.separator { 7.0 } else { 32.0 };
            let id = *next_id;
            *next_id += 1;
            let mut path = prefix.to_vec();
            path.push(i);
            if entry.separator {
                let sep = div()
                    .id(("classic", id))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(Self::context_menu_separator());
                rows.push(sep);
                continue;
            }
            let has_kids = !entry.children.is_empty();
            // A row's children show whenever the row is on the open path
            // (hovered itself, or an ancestor of the hovered nested row).
            let show_kids = has_kids
                && open.len() >= path.len()
                && open[..path.len()] == path[..];
            let v_hover = view.clone();
            let v_click = view.clone();
            let label = entry.label.clone();
            let tip = entry.label.clone();
            let icon_el: Div = match entry.icon.clone() {
                Some(arc) => div()
                    .w(px(16.0))
                    .h(px(16.0))
                    .overflow_hidden()
                    .flex_shrink_0()
                    .child(img(gpui::ImageSource::Render(arc))),
                None => match Self::classic_fallback_glyph(&entry.label) {
                    Some(glyph) => div()
                        .w(px(16.0))
                        .flex_shrink_0()
                        .text_size(px(15.0))
                        .font_family(UiConstants::FONT_MDL2)
                        .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                        .child(glyph.to_string()),
                    None => div().w(px(16.0)).h(px(16.0)).flex_shrink_0(),
                },
            };
            let mut inner = div()
                .flex()
                .items_center()
                .gap(px(10.0))
                .mx(px(4.0))
                .px(px(8.0))
                .h(px(32.0))
                .rounded(px(4.0));
            if entry.enabled {
                inner = inner.cursor_pointer().hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)));
            }
            inner = inner.child(icon_el);
            inner = inner.child(
                div()
                    .text_size(px(14.0))
                    .text_color(if entry.enabled {
                        rgb(UiConstants::TEXT_PRIMARY_RGB)
                    } else {
                        rgba(0xFFFFFF60u32)
                    })
                    .child(label),
            );
            inner = inner.child(div().flex_grow());
            if has_kids {
                inner = inner.child(
                    div()
                        .text_size(px(12.0))
                        .font_family(UiConstants::FONT_MDL2)
                        .text_color(rgba(0xFFFFFF60u32))
                        .child("\u{E76C}"),
                );
            }
            let mut wrapper = div()
                .relative()
                .id(("classic", id));
            let hover_path = path.clone();
            let row_abs_y = panel_abs_top + row_off;
            wrapper = wrapper
                .on_hover(
                    move |hovering: &bool, window: &mut Window, cx: &mut App| {
                        if *hovering {
                            let pos = window.mouse_position();
                            let tip = tip.clone();
                            let path = hover_path.clone();
                            v_hover.update(cx, |this, cx| {
                                this.submenu_row_hovered = true;
                                this.submenu_grace_cancel();
                                this.set_classic_open(path, cx);
                                this.menu_tip = Some((tip, pos));
                            });
                        } else {
                            let mut parent = hover_path.clone();
                            parent.pop();
                            v_hover.update(cx, |this, cx| {
                                this.submenu_row_hovered = false;
                                this.submenu_grace_close(
                                    move |this, cx| this.set_classic_open(parent, cx),
                                    cx,
                                )
                            });
                            v_hover.update(cx, |this, _| {
                                this.menu_tip = None;
                            });
                        }
                    },
                )
                .child(inner);
            if entry.enabled && !has_kids {
                let off = entry.offset;
                let lbl = entry.label.clone();
                let p = paths.clone();
                let d = dir.clone();
                wrapper = wrapper.on_mouse_down(
                    MouseButton::Left,
                    move |_, _, cx| {
                        let p = p.clone();
                        let d = d.clone();
                        v_click.update(cx, |this, cx| {
                            this.invoke_classic(off, lbl.clone(), p, d, cx)
                        });
                    },
                );
            } else {
                wrapper = wrapper
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation());
            }
            rows.push(wrapper);

            if show_kids {
                // Nested panel geometry: cascade with collision detection & WinUI 3 placement
                let child_w = 240.0f32;
                let child_flipped_left = if flipped_left {
                    let left_abs = panel_abs_x - child_w + 6.0;
                    left_abs >= 16.0
                } else {
                    let fits_right = panel_abs_x + panel_w + child_w <= win_w - 16.0;
                    !fits_right
                };
                let (child_rel_left, child_abs_x) = if !child_flipped_left {
                    (panel_w - 6.0, panel_abs_x + panel_w - 6.0)
                } else {
                    let left_abs = panel_abs_x - child_w + 6.0;
                    if left_abs >= 16.0 {
                        (-child_w + 6.0, left_abs)
                    } else {
                        let clamped_rel = (16.0 - panel_abs_x).min(panel_w - 6.0);
                        (clamped_rel, panel_abs_x + clamped_rel)
                    }
                };

                let mut nested_h = 8.0f32;
                for e in entry.children.iter() {
                    nested_h += if e.separator { 7.0 } else { 32.0 };
                }
                let bottom_limit = (win_h - 36.0).max(100.0);
                let mut adjusted_top = -4.0f32;
                if row_abs_y - 4.0 + nested_h > bottom_limit {
                    let overflow = (row_abs_y - 4.0 + nested_h) - bottom_limit;
                    adjusted_top = (-4.0 - overflow).max(40.0 - row_abs_y);
                }
                let child_abs_top = row_abs_y + adjusted_top;
                let available_h = (bottom_limit - child_abs_top).max(120.0);
                let nested_scroll = nested_h > available_h;
                let card_h = if nested_scroll { available_h } else { nested_h };

                let mut nid = *next_id;
                let (kids, grandchild_panel) = Self::classic_rows(
                    &entry.children,
                    open,
                    &path,
                    view.clone(),
                    paths.clone(),
                    dir.clone(),
                    &mut nid,
                    child_abs_x,
                    child_abs_top,
                    0.0,
                    child_w,
                    win_w,
                    win_h,
                    child_flipped_left,
                    mica_geo,
                );
                *next_id = nid;
                let mut card = flyout_card_shell(child_abs_x, child_abs_top, child_w, card_h, mica_geo);
                card = card
                    .absolute()
                    .top(px(panel_card_top + row_off + adjusted_top))
                    .left(px(child_rel_left));

                let mut content = div()
                    .relative()
                    .flex_col()
                    .w_full()
                    .py(px(4.0));
                for row in kids {
                    content = content.child(row);
                }
                let content = content.id(("nested-content", id));
                let content = if nested_scroll {
                    content.max_h(px(available_h)).overflow_scroll()
                } else {
                    content.h(px(card_h)).overflow_hidden()
                };
                card = card.child(content);
                if let Some(grandchild) = grandchild_panel {
                    card = card.child(grandchild);
                }
                let mut nested = card.id(("ctx-nested", id));
                nested = nested
                    .on_hover({
                        let v = view.clone();
                        let keep = path.clone();
                        let mut up = path.clone();
                        up.pop();
                        move |hovering: &bool, _, cx| {
                            if *hovering {
                                let keep = keep.clone();
                                v.update(cx, |this, cx| {
                                    this.submenu_panel_hovered = true;
                                    this.submenu_grace_cancel();
                                    this.set_classic_open(keep, cx)
                                });
                            } else {
                                let v2 = v.clone();
                                let up = up.clone();
                                v2.update(cx, |this, cx| {
                                    this.submenu_panel_hovered = false;
                                    this.submenu_grace_close(
                                        move |this, cx| this.set_classic_open(up, cx),
                                        cx,
                                    )
                                });
                            }
                        }
                    })
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation());

                active_child_panel = Some(nested);
            }
        }
        (rows, active_child_panel)
    }

    /// Layout › flyout (Details / List / Grid / Columns).
    pub(crate) fn layout_submenu(
        view: Entity<crate::app::AppWindow>,
        flip: bool,
        menu_top: f32,
        menu_left: f32,
        row_card_y: f32,
        win_w: f32,
        win_h: f32,
        mica_geo: Option<&crate::wallpaper_mica::MicaGeo>,
    ) -> gpui::Stateful<Div> {
        let modes = [
            ("\u{E8FD}", "Details", crate::config::config::ViewMode::Details),
            ("\u{EA37}", "List", crate::config::config::ViewMode::List),
            ("\u{E8A9}", "Cards", crate::config::config::ViewMode::Cards),
            ("\u{E80A}", "Grid", crate::config::config::ViewMode::Grid),
            ("\u{E8F1}", "Columns", crate::config::config::ViewMode::Columns),
        ];
        let mut rows = Vec::new();
        for (glyph, label, mode) in modes {
            let v = view.clone();
            rows.push(Self::context_menu_item(
                glyph,
                label,
                move |_, _, cx| {
                    v.update(cx, |this, cx| {
                        this.set_view_mode(mode.clone(), cx);
                        this.close_dialog(cx);
                    });
                },
            ));
        }
        Self::submenu_panel(
            rows,
            SubmenuKind::Layout,
            view.clone(),
            flip,
            menu_top,
            menu_left,
            row_card_y,
            win_w,
            win_h,
            mica_geo,
        )
    }

    /// Sort by › flyout.
    pub(crate) fn sort_submenu(
        view: Entity<crate::app::AppWindow>,
        current_sort: &crate::config::config::SortBy,
        flip: bool,
        menu_top: f32,
        menu_left: f32,
        row_card_y: f32,
        win_w: f32,
        win_h: f32,
        mica_geo: Option<&crate::wallpaper_mica::MicaGeo>,
    ) -> gpui::Stateful<Div> {
        let cols = [
            ("Name", crate::config::config::SortBy::Name),
            ("Date modified", crate::config::config::SortBy::DateModified),
            ("Date created", crate::config::config::SortBy::DateCreated),
            ("Type", crate::config::config::SortBy::Type),
            ("Size", crate::config::config::SortBy::Size),
        ];
        let mut rows = Vec::new();
        for (label, col) in cols {
            let glyph = if &col == current_sort {
                "\u{E73E}" // Fluent checkmark
            } else {
                ""
            };
            let v = view.clone();
            rows.push(Self::context_menu_item(
                glyph,
                label,
                move |_, _, cx| {
                    v.update(cx, |this, cx| {
                        this.sort_by(col.clone(), cx);
                        this.close_dialog(cx);
                    });
                },
            ));
        }
        Self::submenu_panel(
            rows,
            SubmenuKind::SortBy,
            view.clone(),
            flip,
            menu_top,
            menu_left,
            row_card_y,
            win_w,
            win_h,
            mica_geo,
        )
    }

    /// Group by › flyout.
    pub(crate) fn group_submenu(
        view: Entity<crate::app::AppWindow>,
        current_group: &crate::config::config::GroupColumn,
        flip: bool,
        menu_top: f32,
        menu_left: f32,
        row_card_y: f32,
        win_w: f32,
        win_h: f32,
        mica_geo: Option<&crate::wallpaper_mica::MicaGeo>,
    ) -> gpui::Stateful<Div> {
        let groups = [
            ("None", crate::config::config::GroupColumn::None),
            ("Name", crate::config::config::GroupColumn::Name),
            ("Date modified", crate::config::config::GroupColumn::DateModified),
            ("Date created", crate::config::config::GroupColumn::DateCreated),
            ("Size", crate::config::config::GroupColumn::Size),
            ("Type", crate::config::config::GroupColumn::FileType),
        ];
        let mut rows = Vec::new();
        for (label, col) in groups {
            let glyph = if &col == current_group {
                "\u{E73E}" // Fluent checkmark
            } else {
                ""
            };
            let v = view.clone();
            rows.push(Self::context_menu_item(
                glyph,
                label,
                move |_, _, cx| {
                    v.update(cx, |this, cx| {
                        this.state.main_view.group_by = col.clone();
                        this.state.main_view.collapsed_groups.clear();
                        cx.notify();
                        this.close_dialog(cx);
                    });
                },
            ));
        }
        Self::submenu_panel(
            rows,
            SubmenuKind::GroupBy,
            view.clone(),
            flip,
            menu_top,
            menu_left,
            row_card_y,
            win_w,
            win_h,
            mica_geo,
        )
    }

    /// New › flyout.
    pub(crate) fn new_submenu(
        view: Entity<crate::app::AppWindow>,
        flip: bool,
        menu_top: f32,
        menu_left: f32,
        row_card_y: f32,
        win_w: f32,
        win_h: f32,
        mica_geo: Option<&crate::wallpaper_mica::MicaGeo>,
    ) -> gpui::Stateful<Div> {
        let v1 = view.clone();
        let v2 = view.clone();
        let v3 = view.clone();
        Self::submenu_panel(
            vec![
                Self::context_menu_item("\u{E8F4}", "Folder", move |_, _, cx| {
                    v1.update(cx, |this, cx| this.create_new_folder(cx));
                }),
                Self::context_menu_item("\u{E7C3}", "Text Document", move |_, _, cx| {
                    v2.update(cx, |this, cx| this.create_new_text_document(cx));
                }),
                Self::context_menu_item("\u{E71B}", "Shortcut", move |_, _, cx| {
                    v3.update(cx, |this, cx| {
                        let current_dir = this.state.main_view.primary_path.as_path().to_path_buf();
                        this.state.active_dialog = Some(DialogType::CreateShortcut {
                            state: Box::new(
                                crate::ui::dialog_services::shortcut_dialog::CreateShortcutState::new(
                                    current_dir.clone(),
                                    current_dir,
                                    "New Shortcut".to_string(),
                                )
                            ),
                        });
                        cx.notify();
                    });
                }),
            ],
            SubmenuKind::New,
            view.clone(),
            flip,
            menu_top,
            menu_left,
            row_card_y,
            win_w,
            win_h,
            mica_geo,
        )
    }

    /// Open With › flyout
    pub(crate) fn openwith_submenu(
        openwith_children: Option<&[ClassicMenuEntry]>,
        selected_path: Option<crate::core::path_util::FilesPath>,
        sel_paths: Vec<std::path::PathBuf>,
        cur_dir: std::path::PathBuf,
        view: Entity<crate::app::AppWindow>,
        flip: bool,
        menu_top: f32,
        menu_left: f32,
        row_card_y: f32,
        win_w: f32,
        win_h: f32,
        mica_geo: Option<&crate::wallpaper_mica::MicaGeo>,
    ) -> gpui::Stateful<Div> {
        let mut rows = Vec::new();
        if let Some(children) = openwith_children {
            for e in children {
                if e.separator {
                    rows.push(Self::context_menu_separator());
                    continue;
                }
                let v = view.clone();
                let paths = sel_paths.clone();
                let dir = cur_dir.clone();
                let offset = e.offset;
                let label = e.label.clone();
                let icon_el = if let Some(ref icon) = e.icon {
                    div()
                        .w(px(16.0))
                        .h(px(16.0))
                        .child(img(icon.clone()).w(px(16.0)).h(px(16.0)))
                } else {
                    div()
                        .text_size(px(14.0))
                        .font_family(UiConstants::FONT_MDL2)
                        .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                        .child("\u{E7AC}")
                };
                rows.push(Self::context_menu_item_with_icon(
                    icon_el,
                    e.label.clone(),
                    move |_, _, cx| {
                        let paths = paths.clone();
                        let dir = dir.clone();
                        let label = label.clone();
                        cx.background_executor().spawn(async move {
                            let _ = crate::platform::windows::classic_menu::invoke_classic_verb(
                                &paths, &dir, offset, &label,
                            );
                        }).detach();
                        v.update(cx, |this, cx| this.close_dialog(cx));
                    },
                ));
            }
        }

        if !rows.is_empty() {
            rows.push(Self::context_menu_separator());
        }

        let ow_path = selected_path.clone();
        let v_ow = view.clone();
        rows.push(Self::context_menu_item(
            "\u{E7AC}",
            "Choose another app...",
            move |_, _, cx| {
                if let Some(ref p) = ow_path {
                    let path_str = p.to_string_lossy().to_string();
                    let _ = std::process::Command::new("rundll32")
                        .args(["shell32.dll,OpenAs_RunDLL", &path_str])
                        .stdin(std::process::Stdio::null())
                        .stdout(std::process::Stdio::null())
                        .stderr(std::process::Stdio::null())
                        .spawn();
                }
                v_ow.update(cx, |this, cx| this.close_dialog(cx));
            },
        ));

        Self::submenu_panel(
            rows,
            SubmenuKind::OpenWith,
            view.clone(),
            flip,
            menu_top,
            menu_left,
            row_card_y,
            win_w,
            win_h,
            mica_geo,
        )
    }

    /// Compress › flyout (ZIP creation + conditional Extract Here).
    pub(crate) fn compress_submenu(
        view: Entity<crate::app::AppWindow>,
        flip: bool,
        is_archive: bool,
        archive_path: Option<crate::core::path_util::FilesPath>,
        menu_top: f32,
        menu_left: f32,
        row_card_y: f32,
        win_w: f32,
        win_h: f32,
        mica_geo: Option<&crate::wallpaper_mica::MicaGeo>,
    ) -> gpui::Stateful<Div> {
        let v = view.clone();
        let mut rows = vec![Self::context_menu_item(
            "\u{F012}",
            "Compress to ZIP archive",
            move |_, _, cx| {
                v.update(cx, |this, cx| {
                    let sources: Vec<std::path::PathBuf> = this
                        .state
                        .main_view
                        .selected_indices
                        .iter()
                        .filter_map(|&i| this.state.main_view.items.get(i))
                        .map(|item| item.path.as_path().to_path_buf())
                        .collect();
                    if !sources.is_empty() {
                        let current_dir =
                            this.state.main_view.primary_path.as_path().to_path_buf();
                        let first_name = sources[0]
                            .file_name()
                            .map(|s| s.to_string_lossy().to_string())
                            .unwrap_or_else(|| "archive".to_string());
                        let destination = current_dir.join(format!("{}.zip", first_name));
                        this.state.active_dialog = Some(DialogType::CreateArchive {
                            state: Box::new(
                                crate::ui::dialog_services::archive_dialog::CreateArchiveState::new(
                                    sources,
                                    destination,
                                ),
                            ),
                        });
                    }
                    cx.notify();
                });
            },
        )];
        if is_archive {
            if let Some(path) = archive_path {
                let v2 = view.clone();
                rows.push(Self::context_menu_item(
                    "\u{F012}",
                    "Extract Here",
                    move |_, _, cx| {
                        let archive_path = path.as_path().to_path_buf();
                        v2.update(cx, |this, cx| {
                            let current_dir =
                                this.state.main_view.primary_path.as_path().to_path_buf();
                            this.state.active_dialog = Some(DialogType::DecompressArchive {
                                state: Box::new(
                                    crate::ui::dialog_services::decompress_dialog::DecompressState::new(
                                        archive_path,
                                        current_dir,
                                    ),
                                ),
                            });
                            cx.notify();
                        });
                    },
                ));
            }
        }
        Self::submenu_panel(
            rows,
            SubmenuKind::Compress,
            view.clone(),
            flip,
            menu_top,
            menu_left,
            row_card_y,
            win_w,
            win_h,
            mica_geo,
        )
    }

    /// Send to › flyout, populated from shell classic entry if available,
    /// or from the Windows SendTo folder with authentic icons.
    pub(crate) fn sendto_submenu(
        sendto_children: Option<&[ClassicMenuEntry]>,
        sel_paths: Vec<std::path::PathBuf>,
        cur_dir: std::path::PathBuf,
        view: Entity<crate::app::AppWindow>,
        flip: bool,
        menu_top: f32,
        menu_left: f32,
        row_card_y: f32,
        win_w: f32,
        win_h: f32,
        mica_geo: Option<&crate::wallpaper_mica::MicaGeo>,
    ) -> gpui::Stateful<Div> {
        let mut rows = Vec::new();
        if let Some(children) = sendto_children {
            for e in children {
                if e.separator {
                    rows.push(Self::context_menu_separator());
                    continue;
                }
                let v = view.clone();
                let paths = sel_paths.clone();
                let dir = cur_dir.clone();
                let offset = e.offset;
                let label = e.label.clone();
                let icon_el = if let Some(ref icon) = e.icon {
                    div()
                        .w(px(16.0))
                        .h(px(16.0))
                        .child(img(icon.clone()).w(px(16.0)).h(px(16.0)))
                } else {
                    let glyph = Self::guess_sendto_glyph(&e.label);
                    div()
                        .text_size(px(14.0))
                        .font_family(UiConstants::FONT_MDL2)
                        .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                        .child(glyph)
                };
                rows.push(Self::context_menu_item_with_icon(
                    icon_el,
                    e.label.clone(),
                    move |_, _, cx| {
                        let paths = paths.clone();
                        let dir = dir.clone();
                        let label = label.clone();
                        cx.background_executor().spawn(async move {
                            let _ = crate::platform::windows::classic_menu::invoke_classic_verb(
                                &paths, &dir, offset, &label,
                            );
                        }).detach();
                        v.update(cx, |this, cx| this.close_dialog(cx));
                    },
                ));
            }
        }

        if rows.is_empty() {
            #[cfg(windows)]
            {
                if let Ok(appdata) = std::env::var("APPDATA") {
                    let sendto = std::path::Path::new(&appdata).join("Microsoft/Windows/SendTo");
                    if let Ok(entries) = std::fs::read_dir(&sendto) {
                        let mut names: Vec<(String, std::path::PathBuf)> = entries
                            .filter_map(|e| e.ok())
                            .map(|e| {
                                let p = e.path();
                                let stem = p
                                    .file_stem()
                                    .map(|s| s.to_string_lossy().to_string())
                                    .unwrap_or_default();
                                (stem, p)
                            })
                            .filter(|(s, _)| !s.is_empty())
                            .collect();
                        names.sort();
                        for (stem, target) in names.into_iter().take(24) {
                            let v = view.clone();
                            let glyph = Self::guess_sendto_glyph(&stem);
                            rows.push(Self::context_menu_item(
                                glyph,
                                stem,
                                move |_, _, cx| {
                                    let target = target.clone();
                                    v.update(cx, |this, cx| this.send_to_target(target, cx));
                                },
                            ));
                        }
                    }
                }
            }
        }

        if rows.is_empty() {
            rows.push(Self::context_menu_item("\u{E8A5}", "(No targets)", |_, _, _| {}));
        }

        Self::submenu_panel(
            rows,
            SubmenuKind::SendTo,
            view.clone(),
            flip,
            menu_top,
            menu_left,
            row_card_y,
            win_w,
            win_h,
            mica_geo,
        )
    }

    /// Edit file tags › flyout (Files-community FileTagsContextMenu parity).
    pub(crate) fn file_tags_submenu(
        view: Entity<crate::app::AppWindow>,
        available_tags: &[crate::config::config::TagDefinition],
        item_tags: &[String],
        flip: bool,
        menu_top: f32,
        menu_left: f32,
        row_card_y: f32,
        win_w: f32,
        win_h: f32,
        mica_geo: Option<&crate::wallpaper_mica::MicaGeo>,
    ) -> gpui::Stateful<Div> {
        let mut rows = Vec::new();
        for tag in available_tags {
            let v = view.clone();
            let tag_name = tag.name.clone();
            let is_tagged = item_tags.iter().any(|t| t == &tag.name || t == &tag.id);
            let hex = tag.color.trim_start_matches('#');
            let color_rgb = u32::from_str_radix(hex, 16).unwrap_or(0x3B82F6);
            let dot = div()
                .w(px(12.0))
                .h(px(12.0))
                .rounded(px(6.0))
                .bg(rgb(color_rgb));
            let trailing = if is_tagged {
                MenuTrailing::Hint("\u{E73E}")
            } else {
                MenuTrailing::None
            };
            rows.push(Self::context_menu_item_full(
                dot,
                tag.name.clone(),
                trailing,
                move |_, _, cx| {
                    let name = tag_name.clone();
                    v.update(cx, |this, cx| {
                        if is_tagged {
                            this.remove_tag_from_selected(&name, cx);
                        } else {
                            this.add_tag_to_selected(name, cx);
                        }
                        this.close_dialog(cx);
                    });
                },
            ));
        }

        if !available_tags.is_empty() {
            rows.push(Self::context_menu_separator());
        }

        let v_clear = view.clone();
        rows.push(Self::context_menu_item(
            "\u{E74D}",
            "Remove tags",
            move |_, _, cx| {
                v_clear.update(cx, |this, cx| {
                    this.clear_tags_from_selected(cx);
                    this.close_dialog(cx);
                });
            },
        ));

        Self::submenu_panel(
            rows,
            SubmenuKind::FileTags,
            view.clone(),
            flip,
            menu_top,
            menu_left,
            row_card_y,
            win_w,
            win_h,
            mica_geo,
        )
    }

    fn guess_sendto_glyph(label: &str) -> &'static str {
        let lower = label.to_lowercase();
        if lower.contains("bluetooth") {
            "\u{E702}" // Bluetooth
        } else if lower.contains("compressed") || lower.contains("zip") {
            "\u{F012}" // Zip archive
        } else if lower.contains("desktop") {
            "\u{E7F4}" // Desktop
        } else if lower.contains("document") {
            "\u{E8A5}" // Documents
        } else if lower.contains("mail") {
            "\u{E715}" // Mail
        } else if lower.contains("drive") || lower.contains("hdd") || lower.contains("ssd") || lower.contains(":\\") || lower.contains("(:)") || lower.ends_with(")") {
            "\u{EDA2}" // Hard disk / drive
        } else if lower.contains("shortcut") {
            "\u{E71B}" // Shortcut
        } else {
            "\u{E7AC}" // General app
        }
    }
}

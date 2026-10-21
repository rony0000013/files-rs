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
        view: Entity<crate::app::AppWindow>,
        cx: &mut Context<crate::app::AppWindow>,
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

        // Context menu container with proper sizing per DESIGN_SPEC.md:
        // - max width 250px
        // - max height 480px
        let mut menu = div()
            .flex_col()
            .bg(rgba(0x1A1A1AF2u32)) // LayerFillColorDefault dark (#1A1A1A @ F2)
            .border(px(1.0))
            .border_color(rgba(UiConstants::STROKE_RGBA))
            .rounded(px(8.0))
            .shadow_lg()
            .py(px(4.0))
            .w(px(292.0));
        // Running content height + the ShowMore row offset (for flyout
        // geometry: bottom-anchored tall panels + on-screen caps).
        let mut menu_h = 0.0f32;
        #[allow(unused_mut)]
        let mut showmore_off = 0.0f32;
        #[allow(unused_variables)]
        let menu_top: f32 = position.y.into();
        #[allow(unused_variables)]
        let menu_left: f32 = position.x.into();

        if has_selection {
            // ═══════════════════════════════════════════════════════════
            // FILES LAYOUT — file/folder/multi-select context menu
            // ═══════════════════════════════════════════════════════════
            let v_sub = view.clone();
            let v_open2 = v_open.clone();
            // Pre-read selection paths + current dir for the classic menu.
            let sel_paths: Vec<std::path::PathBuf> = selected_paths.clone();
            let cur_dir: std::path::PathBuf = current_dir.clone();

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
                let path = selected_path.clone();
                let props_btn: Box<
                    dyn Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
                > = Box::new(move |_, _, cx| {
                    if let Some(ref p) = path {
                        let _ = crate::platform::windows::shell_service::ShellService::open_properties_sync(p);
                    }
                    v.update(cx, |this, cx| this.close_dialog(cx));
                });
                menu = menu.child(Self::context_menu_toolbar(
                    vec![
                        ("\u{E8C6}", "Cut \u{2014} Ctrl+X", cut_btn),
                        ("\u{E8C8}", "Copy \u{2014} Ctrl+C", copy_btn),
                        ("\u{E77F}", "Paste \u{2014} Ctrl+V", paste_btn),
                        ("\u{E8AC}", "Rename \u{2014} F2", rename_btn),
                        ("\u{E72D}", "Share", share_btn),
                        ("\u{E74D}", "Delete \u{2014} Del", delete_btn),
                        ("\u{E946}", "Properties \u{2014} Alt+Enter", props_btn),
                    ],
                    view.clone(),
                ));
                menu_h += 34.0;
                menu = menu.child(Self::context_menu_separator());
                menu_h += 9.0;
            }

            // Open (single selection only)
            if !is_multi {
                menu = menu.child(Self::context_menu_row("\u{E8DA}", "Open", MenuTrailing::None, {
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
                }));
                menu_h += 29.0;
            }

            // Open with › (file, single): flyout + default fires Open-As.
            if !is_multi && !is_dir {
                let ow_open = open_submenu == Some(SubmenuKind::OpenWith);
                let ow_path = selected_path.clone();
                let ow_path2 = selected_path.clone();
                let v_ow = v_sub.clone();
                let mut ow_rows = vec![Self::context_menu_item(
                    "\u{E8DA}",
                    "Default app",
                    {
                        let path = ow_path.clone();
                        move |_, _, cx| {
                            if let Some(ref p) = path {
                                let path = p.clone();
                                v_open2.update(cx, |this, cx| {
                                    this.navigate(path, cx);
                                    this.close_dialog(cx);
                                });
                            }
                        }
                    },
                )];
                ow_rows.push(Self::context_menu_item(
                    "\u{E8A5}",
                    "Choose another app...",
                    {
                        let path = ow_path2.clone();
                        move |_, _, cx| {
                            if let Some(ref p) = path {
                                let path_str = p.to_string_lossy().to_string();
                                let _ = std::process::Command::new("rundll32")
                                    .args(["shell32.dll,OpenAs_RunDLL", &path_str])
                                    .spawn();
                            }
                            v_ow.update(cx, |this, cx| this.close_dialog(cx));
                        }
                    },
                ));
                let v_fire = v_sub.clone();
                let path = selected_path.clone();
                menu = menu.child(Self::submenu_row(
                    "\u{E8DA}",
                    "Open With",
                    SubmenuKind::OpenWith,
                    ow_open,
                    Self::submenu_panel(ow_rows, SubmenuKind::OpenWith, v_sub.clone(), flip),
                    v_sub.clone(),
                    move |_, _, cx| {
                        if let Some(ref p) = path {
                            let path_str = p.to_string_lossy().to_string();
                            let _ = std::process::Command::new("rundll32")
                                .args(["shell32.dll,OpenAs_RunDLL", &path_str])
                                .spawn();
                        }
                        v_fire.update(cx, |this, cx| this.close_dialog(cx));
                    },
                ));
                menu_h += 29.0;
            }

            // Paste shortcut (from clipboard files)
            menu = menu.child(Self::context_menu_row("\u{E77F}", "Paste shortcut", MenuTrailing::None, {
                let v = v_paste.clone();
                move |_, _, cx| {
                    v.update(cx, |this, cx| this.paste_as_shortcut(cx));
                }
            }));
                menu_h += 29.0;

            // Copy item path (Ctrl+Shift+C)
            menu = menu.child(Self::context_menu_row("\u{E8C8}", "Copy item path", MenuTrailing::Hint("Ctrl+Shift+C"), {
                let v = v_copypath.clone();
                move |_, _, cx| {
                    v.update(cx, |this, cx| {
                        this.copy_item_path(cx);
                        this.close_dialog(cx);
                    });
                }
            }));
                menu_h += 29.0;

            // Create folder with selection
            menu = menu.child(Self::context_menu_row("\u{E8F4}", "Create folder with selection", MenuTrailing::None, {
                let v = v_close.clone();
                move |_, _, cx| {
                    v.update(cx, |this, cx| this.create_folder_with_selection(cx));
                }
            }));
                menu_h += 29.0;

            // Create Shortcut (single selection only)
            if !is_multi {
                menu = menu.child(Self::context_menu_row(
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
                ));
                menu_h += 29.0;
            }

            // Compress › (ZIP creation + conditional Extract Here)
            {
                let c_open = open_submenu == Some(SubmenuKind::Compress);
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
                let v_fire = v_sub.clone();
                menu = menu.child(Self::submenu_row(
                    "\u{E8B7}",
                    "Compress",
                    SubmenuKind::Compress,
                    c_open,
                    Self::compress_submenu(view.clone(), flip, is_archive, selected_path.clone()),
                    v_sub.clone(),
                    move |_, _, cx| {
                        v_fire.update(cx, |this, cx| {
                            this.set_open_submenu(Some(SubmenuKind::Compress), cx);
                        });
                    },
                ));
                menu_h += 29.0;
            }

            // Send to › (Windows SendTo folder)
            {
                let s_open = open_submenu == Some(SubmenuKind::SendTo);
                let v_fire = v_sub.clone();
                menu = menu.child(Self::submenu_row(
                    "\u{E8B7}",
                    "Send to",
                    SubmenuKind::SendTo,
                    s_open,
                    Self::sendto_submenu(view.clone(), flip),
                    v_sub.clone(),
                    move |_, _, cx| {
                        v_fire.update(cx, |this, cx| {
                            this.set_open_submenu(Some(SubmenuKind::SendTo), cx);
                        });
                    },
                ));
                menu_h += 29.0;
            }

            menu = menu.child(Self::context_menu_separator());
                menu_h += 9.0;

            // ── PIN SECTION ──
            menu = menu.child(Self::context_menu_item(
                "\u{E718}",
                "Pin to Quick Access",
                {
                    let v = v_close.clone();
                    let path = selected_path.clone();
                    move |_, _, cx| {
                        if let Some(ref p) = path {
                            let path_str = p.to_string_lossy().to_string();
                            tracing::info!("Pin to Quick Access: {}", path_str);
                        }
                        v.update(cx, |this, cx| this.close_dialog(cx));
                    }
                },
            ));
                menu_h += 29.0;
            menu = menu.child(Self::context_menu_item("\u{E77E}", "Pin to Start", {
                let v = v_close.clone();
                let path = selected_path.clone();
                move |_, _, cx| {
                    if let Some(ref p) = path {
                        let path_str = p.to_string_lossy().to_string();
                        tracing::info!("Pin to Start: {}", path_str);
                    }
                    v.update(cx, |this, cx| this.close_dialog(cx));
                }
            }));
                menu_h += 29.0;

            menu = menu.child(Self::context_menu_separator());
                menu_h += 9.0;

            // ── EXTRAS (app-specific actions) ──
            menu = menu.child(Self::context_menu_item("\u{E8A7}", "Open in VS Code", {
                let v = v_close.clone();
                let path = selected_path.clone();
                move |_, _, cx| {
                    v.update(cx, |this, cx| {
                        this.open_in_ide(path.clone(), cx);
                        this.close_dialog(cx);
                    });
                }
            }));
                menu_h += 29.0;
            menu = menu.child(Self::context_menu_item("\u{E896}", "Git Status", {
                let v = v_close.clone();
                let path = selected_path.clone();
                move |_, _, cx| {
                    if let Some(ref p) = path {
                        let path_str = p.to_string_lossy().to_string();
                        let dir = std::path::Path::new(&path_str)
                            .parent()
                            .map(|p| p.to_string_lossy().to_string())
                            .unwrap_or_else(|| path_str.clone());
                        let _ = std::process::Command::new("cmd")
                            .args([
                                "/C",
                                "start",
                                "cmd",
                                "/K",
                                &format!("cd /d {} && git status", dir),
                            ])
                            .spawn();
                    }
                    v.update(cx, |this, cx| this.close_dialog(cx));
                }
            }));
                menu_h += 29.0;
            menu = menu.child(Self::context_menu_item("\u{E74D}", "Delete Permanently", {
                let v = v_close.clone();
                move |_, _, cx| {
                    v.update(cx, |this, cx| this.delete_permanently(cx));
                }
            }));
                menu_h += 29.0;

            // Show more options › — the real classic shell items (WinRAR,
            // Kaspersky, ...) expand inline on hover (Files parity). Windows
            // only: other platforms have no classic provider yet.
            #[cfg(target_os = "windows")]
            {
                showmore_off = menu_h;
                if !expanded {
                let sm_open = open_submenu == Some(SubmenuKind::ShowMore);
                let v_click = v_sub.clone();
                menu = menu.child(Self::submenu_row(
                    "\u{E10C}",
                    "Show more options",
                    SubmenuKind::ShowMore,
                    sm_open,
                    Self::classic_submenu(
                        classic.clone(),
                        classic_loading,
                        classic_open.clone(),
                        view.clone(),
                        sel_paths.clone(),
                        cur_dir.clone(),
                        flip,
                        menu_top,
                        menu_left,
                        showmore_off,
                    ),
                    view.clone(),
                    move |_, _, cx| {
                        v_click.update(cx, |this, cx| {
                            this.set_open_submenu(Some(SubmenuKind::ShowMore), cx)
                        });
                    },
                ));
                menu_h += 29.0;
                } else {
                    // Expanded style: every classic item is inlined below the
                    // app rows - no hover needed. Nested verbs keep flyouts.
                    menu = menu.child(Self::context_menu_separator());
                    menu_h += 9.0;
                    match classic.as_ref() {
                        Some(entries) if !entries.is_empty() => {
                            let mut next_id: usize = 0;
                            let inline = Self::classic_rows(
                                entries,
                                &classic_open,
                                &[],
                                view.clone(),
                                sel_paths.clone(),
                                cur_dir.clone(),
                                &mut next_id,
                                flip,
                                menu_top + showmore_off,
                                menu_left,
                            );
                            for row in inline {
                                menu = menu.child(row);
                            }
                        }
                        Some(_) => {
                            let mut nid = 0usize;
                            menu = menu.child(Self::classic_note_row(
                                "(No extended options)",
                                &mut nid,
                            ));
                        }
                        None => {
                            let mut nid = 0usize;
                            let txt = if classic_loading {
                                "Loading…"
                            } else {
                                "(Unavailable)"
                            };
                            menu = menu.child(Self::classic_note_row(txt, &mut nid));
                        }
                    }
                }
            }
        } else {
            // ═══════════════════════════════════════════════════════════
            // FILES LAYOUT — background (empty space) context menu
            // ═══════════════════════════════════════════════════════════
            let v_sub = view.clone();
            let cur_dir: std::path::PathBuf = current_dir.clone();

            // Layout ›
            {
                let l_open = open_submenu == Some(SubmenuKind::Layout);
                let v_fire = v_sub.clone();
                menu = menu.child(Self::submenu_row(
                    "\u{E8A1}",
                    "Layout",
                    SubmenuKind::Layout,
                    l_open,
                    Self::layout_submenu(view.clone(), flip),
                    v_sub.clone(),
                    move |_, _, cx| {
                        v_fire.update(cx, |this, cx| {
                            this.set_open_submenu(Some(SubmenuKind::Layout), cx);
                        });
                    },
                ));
                menu_h += 29.0;
            }

            // Sort by ›
            {
                let s_open = open_submenu == Some(SubmenuKind::SortBy);
                let v_fire = v_sub.clone();
                menu = menu.child(Self::submenu_row(
                    "\u{E2C7}",
                    "Sort by",
                    SubmenuKind::SortBy,
                    s_open,
                    Self::sort_submenu(view.clone(), flip),
                    v_sub.clone(),
                    move |_, _, cx| {
                        v_fire.update(cx, |this, cx| {
                            this.set_open_submenu(Some(SubmenuKind::SortBy), cx);
                        });
                    },
                ));
                menu_h += 29.0;
            }

            // Group by ›
            {
                let g_open = open_submenu == Some(SubmenuKind::GroupBy);
                let v_fire = v_sub.clone();
                menu = menu.child(Self::submenu_row(
                    "\u{E2C7}",
                    "Group by",
                    SubmenuKind::GroupBy,
                    g_open,
                    Self::group_submenu(view.clone(), flip),
                    v_sub.clone(),
                    move |_, _, cx| {
                        v_fire.update(cx, |this, cx| {
                            this.set_open_submenu(Some(SubmenuKind::GroupBy), cx);
                        });
                    },
                ));
                menu_h += 29.0;
            }

            // Refresh (Ctrl+R)
            menu = menu.child(Self::context_menu_row("\u{E72C}", "Refresh", MenuTrailing::Hint("Ctrl+R"), {
                let v = v_close.clone();
                move |_, _, cx| {
                    v.update(cx, |this, cx| {
                        this.refresh(cx);
                        this.close_dialog(cx);
                    });
                }
            }));
                menu_h += 29.0;

            menu = menu.child(Self::context_menu_separator());
                menu_h += 9.0;

            // New ›
            {
                let n_open = open_submenu == Some(SubmenuKind::New);
                let v_fire = v_sub.clone();
                menu = menu.child(Self::submenu_row(
                    "\u{E8F4}",
                    "New",
                    SubmenuKind::New,
                    n_open,
                    Self::new_submenu(view.clone(), flip),
                    v_sub.clone(),
                    move |_, _, cx| {
                        v_fire.update(cx, |this, cx| {
                            this.set_open_submenu(Some(SubmenuKind::New), cx);
                        });
                    },
                ));
                menu_h += 29.0;
            }

            // Paste shortcut (clipboard files pasted as shortcuts here)
            menu = menu.child(Self::context_menu_row("\u{E77F}", "Paste shortcut", MenuTrailing::None, {
                let v = v_sub.clone();
                move |_, _, cx| {
                    v.update(cx, |this, cx| this.paste_as_shortcut(cx));
                }
            }));
                menu_h += 29.0;

            // Pin to Sidebar (current folder)
            menu = menu.child(Self::context_menu_item(
                "\u{E718}",
                "Pin to Sidebar",
                {
                    let v = v_close.clone();
                    let dir = cur_dir.clone();
                    move |_, _, cx| {
                        tracing::info!("Pin to Sidebar: {:?}", dir);
                        v.update(cx, |this, cx| this.close_dialog(cx));
                    }
                },
            ));
                menu_h += 29.0;
            menu = menu.child(Self::context_menu_separator());
                menu_h += 9.0;

            // Open in Windows Terminal (Ctrl+`)
            menu = menu.child(Self::context_menu_row("\u{E756}", "Open in Windows Terminal", MenuTrailing::Hint("Ctrl+`"), {
                let v = v_close.clone();
                move |_, _, cx| {
                    v.update(cx, |this, cx| {
                        this.open_terminal_at_current(cx);
                        this.close_dialog(cx);
                    });
                }
            }));
                menu_h += 29.0;

            // Show more options › — the real classic shell items expand
            // inline on hover (Files parity). Windows only.
            #[cfg(target_os = "windows")]
            {
                showmore_off = menu_h;
                if !expanded {
                let sm_open = open_submenu == Some(SubmenuKind::ShowMore);
                let v_click = v_sub.clone();
                menu = menu.child(Self::submenu_row(
                    "\u{E10C}",
                    "Show more options",
                    SubmenuKind::ShowMore,
                    sm_open,
                    Self::classic_submenu(
                        classic.clone(),
                        classic_loading,
                        classic_open.clone(),
                        view.clone(),
                        Vec::new(),
                        cur_dir.clone(),
                        flip,
                        menu_top,
                        menu_left,
                        showmore_off,
                    ),
                    view.clone(),
                    move |_, _, cx| {
                        v_click.update(cx, |this, cx| {
                            this.set_open_submenu(Some(SubmenuKind::ShowMore), cx)
                        });
                    },
                ));
                menu_h += 29.0;
                } else {
                    // Expanded style: every classic item is inlined below the
                    // app rows - no hover needed. Nested verbs keep flyouts.
                    menu = menu.child(Self::context_menu_separator());
                    menu_h += 9.0;
                    match classic.as_ref() {
                        Some(entries) if !entries.is_empty() => {
                            let mut next_id: usize = 0;
                            let inline = Self::classic_rows(
                                entries,
                                &classic_open,
                                &[],
                                view.clone(),
                                Vec::new(),
                                cur_dir.clone(),
                                &mut next_id,
                                flip,
                                menu_top + showmore_off,
                                menu_left,
                            );
                            for row in inline {
                                menu = menu.child(row);
                            }
                        }
                        Some(_) => {
                            let mut nid = 0usize;
                            menu = menu.child(Self::classic_note_row(
                                "(No extended options)",
                                &mut nid,
                            ));
                        }
                        None => {
                            let mut nid = 0usize;
                            let txt = if classic_loading {
                                "Loading…"
                            } else {
                                "(Unavailable)"
                            };
                            menu = menu.child(Self::classic_note_row(txt, &mut nid));
                        }
                    }
                }
            }
        }
        menu.with_animation(
            "ctx-menu-entrance",
            Animation::new(std::time::Duration::from_millis(120)),
            |elem, delta| elem.opacity(delta),
        )
    }

    /// Build context menu items for all available tags with toggle logic
    pub(crate) fn context_menu_tags_submenu(
        view: Entity<crate::app::AppWindow>,
        cx: &mut Context<crate::app::AppWindow>,
    ) -> Div {
        let app = view.read(cx);
        let common_tags = app.get_common_tags_for_selected();

        let mut container = div().flex_col();

        // "Tags" header
        container = container.child(
            div()
                .flex()
                .items_center()
                .gap(px(12.0))
                .px(px(12.0))
                .py(px(6.0))
                .child(
                    div()
                        .text_size(px(14.0))
                        .text_color(rgb(UiConstants::ACCENT_COLOR_RGB))
                        .child("\u{E8EC}"),
                )
                .child(
                    div()
                        .text_size(px(13.0))
                        .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                        .child("Tags"),
                ),
        );

        if common_tags.is_empty() {
            container = container.child(
                div().flex().items_center().px(px(12.0)).py(px(4.0)).child(
                    div()
                        .text_size(px(12.0))
                        .text_color(rgb(UiConstants::TEXT_SECONDARY_RGB))
                        .child("No tags available"),
                ),
            );
        } else {
            for (tag_name, tag_state) in &common_tags {
                let is_all = *tag_state == crate::app::TagState::All;
                let tag_name_clone = tag_name.clone();
                let v = view.clone();

                let (glyph, label) = if is_all {
                    ("\u{E73E}", format!("{} (remove)", tag_name))
                } else {
                    ("\u{E8EC}", format!("{} (add)", tag_name))
                };

                container = container.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(12.0))
                        .px(px(12.0))
                        .py(px(8.0))
                        .cursor_pointer()
                        .hover(|s| s.bg(rgba(0xFFFFFF15u32)))
                        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                            if is_all {
                                v.update(cx, |this, cx| {
                                    this.remove_tag_from_selected(&tag_name_clone, cx);
                                });
                            } else {
                                v.update(cx, |this, cx| {
                                    this.add_tag_to_selected(tag_name_clone.clone(), cx);
                                });
                            }
                        })
                        .child(
                            div()
                                .text_size(px(14.0))
                                .text_color(rgb(UiConstants::ACCENT_COLOR_RGB))
                                .child(glyph),
                        )
                        .child(
                            div()
                                .text_size(px(13.0))
                                .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                                .child(label),
                        ),
                );
            }
        }

        container
    }

    pub(crate) fn context_menu_item(
        glyph: &str,
        label: impl Into<String>,
        on_click: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    ) -> Div {
        div()
            .flex()
            .items_center()
            .gap(px(12.0))
            .px(px(12.0))
            .py(px(5.0))
            .cursor_pointer()
            .hover(|s| s.bg(rgba(0xFFFFFF15u32)))
            .on_mouse_down(MouseButton::Left, on_click)
            .child(
                div()
                    .text_size(px(16.0))
                    .font_family(UiConstants::FONT_MDL2)
                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                    .child(glyph.to_string()),
            )
            .child(
                div()
                    .text_size(px(14.0))
                    .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                    .child(label.into()),
            )
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
            .gap(px(12.0))
            .px(px(12.0))
            .py(px(10.0))
            .cursor_pointer()
            .hover(|s| s.bg(rgba(0xFFFFFF15u32)))
            .on_mouse_down(MouseButton::Left, on_click)
            .child(
                div()
                    .text_size(px(16.0))
                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                    .child(glyph.to_string()),
            )
            .child(
                div()
                    .text_size(px(14.0))
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
                    .text_size(px(12.0))
                    .text_color(rgba(0xFFFFFF60u32))
                    .child(hint.to_string()),
            ),
            MenuTrailing::Chevron => Some(
                div()
                    .text_size(px(12.0))
                    .font_family(UiConstants::FONT_MDL2)
                    .text_color(rgba(0xFFFFFF60u32))
                    .child("\u{E76C}"),
            ),
        };
        div()
            .flex()
            .items_center()
            .gap(px(12.0))
            .px(px(12.0))
            .py(px(5.0))
            .cursor_pointer()
            .hover(|s| s.bg(rgba(0xFFFFFF15u32)))
            .on_mouse_down(MouseButton::Left, on_click)
            .child(
                div()
                    .text_size(px(16.0))
                    .font_family(UiConstants::FONT_MDL2)
                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                    .child(glyph.to_string()),
            )
            .child(
                div()
                    .text_size(px(14.0))
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
            .my(px(4.0))
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

    /// Build a context menu for tab bar right-click with options like Close Tab, Close Other Tabs, etc.
    pub(crate) fn build_tab_context_menu(
        tab_index: usize,
        _position: Point<Pixels>,
        tab_count: usize,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let v_close = view.clone();
        let v_close_others = view.clone();
        let v_close_right = view.clone();
        let v_duplicate = view.clone();
        let v_pin = view.clone();
        let v_new_tab = view.clone();
        let idx = tab_index;

        div()
            .flex_col()
            .py(px(4.0))
            // Close Tab
            .child(Self::context_menu_item("\u{E8BB}", "Close Tab", {
                let v = v_close.clone();
                move |_, _, cx| {
                    v.update(cx, |this, cx| {
                        this.close_tab(idx, cx);
                        this.close_dialog(cx);
                    });
                }
            }))
            // Close Other Tabs (only show if more than 1 tab)
            .child(if tab_count > 1 {
                div().child(Self::context_menu_item("\u{E8BB}", "Close Other Tabs", {
                    let v = v_close_others.clone();
                    let idx = idx;
                    move |_, _, cx| {
                        v.update(cx, |this, cx| {
                            this.close_other_tabs(idx, cx);
                            this.close_dialog(cx);
                        });
                    }
                }))
            } else {
                div()
            })
            // Close Tabs to the Right (only show if not the last tab)
            .child(if tab_index < tab_count - 1 {
                div().child(Self::context_menu_item(
                    "\u{E72B}",
                    "Close Tabs to the Right",
                    {
                        let v = v_close_right.clone();
                        let idx = idx;
                        move |_, _, cx| {
                            v.update(cx, |this, cx| {
                                this.close_tabs_to_right(idx, cx);
                                this.close_dialog(cx);
                            });
                        }
                    },
                ))
            } else {
                div()
            })
            .child(
                div()
                    .h(px(1.0))
                    .bg(rgba(UiConstants::DIVIDER_RGBA))
                    .mx(px(8.0))
                    .my(px(4.0)),
            )
            // Duplicate Tab
            .child(Self::context_menu_item("\u{E8C8}", "Duplicate Tab", {
                let v = v_duplicate.clone();
                let idx = idx;
                move |_, _, cx| {
                    v.update(cx, |this, cx| {
                        this.duplicate_tab(idx, cx);
                        this.close_dialog(cx);
                    });
                }
            }))
            // Pin Tab
            .child(Self::context_menu_item("\u{E718}", "Pin Tab", {
                let v = v_pin.clone();
                let idx = idx;
                move |_, _, cx| {
                    v.update(cx, |this, cx| {
                        this.pin_tab(idx, cx);
                        this.close_dialog(cx);
                    });
                }
            }))
            .child(
                div()
                    .h(px(1.0))
                    .bg(rgba(UiConstants::DIVIDER_RGBA))
                    .mx(px(8.0))
                    .my(px(4.0)),
            )
            // New Tab
            .child(Self::context_menu_item("\u{E710}", "New Tab", {
                let v = v_new_tab.clone();
                move |_, _, cx| {
                    v.update(cx, |this, cx| {
                        this.new_tab(cx);
                        this.close_dialog(cx);
                    });
                }
            }))
    }

    /// Build the tab actions dropdown menu (split pane options)
    pub(crate) fn build_tab_actions_dropdown(
        _position: Point<Pixels>,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let v_split_v = view.clone();
        let v_split_h = view.clone();
        let v_toggle = view.clone();
        let v_new_tab = view.clone();

        div()
            .flex_col()
            .py(px(4.0))
            // New Tab
            .child(Self::context_menu_item("\u{E710}", "New Tab", {
                let v = v_new_tab.clone();
                move |_, _, cx| {
                    v.update(cx, |this, cx| {
                        this.new_tab(cx);
                        this.close_dialog(cx);
                    });
                }
            }))
            .child(
                div()
                    .h(px(1.0))
                    .bg(rgba(UiConstants::DIVIDER_RGBA))
                    .mx(px(8.0))
                    .my(px(4.0)),
            )
            // Split Vertically (Ctrl+Shift+V)
            .child(Self::context_menu_item_with_shortcut(
                "\u{E8A0}", // SplitVertical icon
                "Split Vertical",
                "Ctrl+Shift+V",
                {
                    let v = v_split_v.clone();
                    move |_, _, cx| {
                        v.update(cx, |this, cx| {
                            this.state.navigation.update(cx, |nav, _cx| {
                                nav.split_pane_vertical();
                            });
                            let layout = this.state.navigation.read(cx).pane_layout;
                            this.state.main_view.pane_type = match layout {
                                crate::core::navigation_manager::PaneLayout::Single => crate::ui::views::main_view::PaneType::Single,
                                _ => crate::ui::views::main_view::PaneType::Dual,
                            };
                            this.close_dialog(cx);
                        });
                    }
                },
            ))
            // Split Horizontally (Ctrl+Shift+H)
            .child(Self::context_menu_item_with_shortcut(
                "\u{E8A1}", // SplitHorizontal icon
                "Split Horizontal",
                "Ctrl+Shift+H",
                {
                    let v = v_split_h.clone();
                    move |_, _, cx| {
                        v.update(cx, |this, cx| {
                            this.state.navigation.update(cx, |nav, _cx| {
                                nav.split_pane_horizontal();
                            });
                            let layout = this.state.navigation.read(cx).pane_layout;
                            this.state.main_view.pane_type = match layout {
                                crate::core::navigation_manager::PaneLayout::Single => crate::ui::views::main_view::PaneType::Single,
                                _ => crate::ui::views::main_view::PaneType::Dual,
                            };
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
            // Toggle Split Pane
            .child(Self::context_menu_item("\u{E762}", "Toggle Split Pane", {
                let v = v_toggle.clone();
                move |_, _, cx| {
                    v.update(cx, |this, cx| {
                        this.toggle_dual_pane(cx);
                        this.close_dialog(cx);
                    });
                }
            }))
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
    /// The submenu panel is a child of the row wrapper, so hover stays
    /// engaged while the cursor moves into the flyout.
    pub(crate) fn submenu_row(
        glyph: &str,
        label: &str,
        kind: SubmenuKind,
        is_open: bool,
        submenu: impl IntoElement,
        view: Entity<crate::app::AppWindow>,
        on_click: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    ) -> gpui::Stateful<Div> {
        let v_hover = view.clone();
        let stateful = div()
            .relative()
            .child(Self::context_menu_row(glyph, label, MenuTrailing::Chevron, on_click))
            .id(("ctx-submenu", kind as usize));
        let mut wrapper = stateful.on_hover(move |hovering: &bool, _, cx| {
            // Flyout panels live outside the row bounds, so an immediate
            // close on leave would kill the panel mid-travel (including
            // diagonal cursor paths). Grace-close instead; entering the
            // panel (or returning to the row) cancels it.
            if *hovering {
                v_hover.update(cx, |this, cx| {
                    this.set_open_submenu_intent(kind, cx);
                });
            } else {
                let v = v_hover.clone();
                v.update(cx, |this, cx| {
                    this.submenu_grace_close(
                        |this, cx| this.set_open_submenu(None, cx),
                        cx,
                    )
                });
            }
        });
        if is_open {
            wrapper = wrapper.child(submenu);
        }
        wrapper
    }

    /// Flyout panel anchored to the right of its parent row.
    pub(crate) fn submenu_panel(
        rows: Vec<Div>,
        kind: SubmenuKind,
        view: Entity<crate::app::AppWindow>,
        flip: bool,
    ) -> gpui::Stateful<Div> {
        let v = view.clone();
        let left = if flip { -222.0 } else { 290.0 };
        let mut panel = div()
            .absolute()
            .top(px(-8.0))
            .left(px(left))
            .w(px(220.0))
            .flex_col()
            .bg(rgba(0x1A1A1AF2u32))
            .border(px(1.0))
            .border_color(rgba(UiConstants::STROKE_RGBA))
            .rounded(px(8.0))
            .shadow_lg()
            .py(px(4.0))
            .id(("ctx-panel", kind as usize))
            .on_hover(move |hovering: &bool, _, cx| {
                // Re-assert while the cursor travels the panel; grace-close
                // once it truly leaves, so diagonal entry never kills it.
                if *hovering {
                    v.update(cx, |this, cx| {
                        this.set_open_submenu(Some(kind), cx);
                    });
                } else {
                    let v2 = v.clone();
                    v2.update(cx, |this, cx| {
                        this.submenu_grace_close(
                            |this, cx| this.set_open_submenu(None, cx),
                            cx,
                        )
                    });
                }
            })
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation());
        for row in rows {
            panel = panel.child(row);
        }
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
        row_off: f32,
    ) -> gpui::Stateful<Div> {
        // Bottom-anchored: grows UP from the ShowMore row, so a 30-item
        // flyout can never spill past the window bottom. Capped so it never
        // passes the window top either; overflow scrolls inside.
        let row_bottom = menu_top + row_off + 29.0;
        let mut fly_h = 8.0f32;
        if let Some(ref entries) = classic {
            for e in entries.iter() {
                fly_h += if e.separator { 9.0 } else { 29.0 };
            }
        }
        let max_h = (row_bottom - 24.0).max(87.0);
        let panel_top = row_bottom - fly_h.min(max_h);
        let rows: Vec<gpui::Stateful<Div>> = match classic {
            Some(entries) if !entries.is_empty() => {
                let mut next_id: usize = 0;
                Self::classic_rows(
                    &entries, &open, &[], view.clone(), paths, dir, &mut next_id,
                    flip, panel_top, menu_left,
                )
            }
            Some(_) => {
                vec![Self::classic_note_row("(No extended options)", &mut 0usize)]
            }
            None if loading => vec![Self::classic_note_row("Loading\u{2026}", &mut 0usize)],
            None => vec![Self::classic_note_row("(Unavailable)", &mut 0usize)],
        };
        let left = if flip { -262.0 } else { 290.0 };
        let scroll = fly_h > max_h;
        let mut panel = div()
            .absolute()
            .bottom(px(4.0))
            .left(px(left))
            .w(px(260.0))
            .flex_col()
            .bg(rgba(0x1A1A1AF2u32))
            .border(px(1.0))
            .border_color(rgba(UiConstants::STROKE_RGBA))
            .rounded(px(8.0))
            .shadow_lg()
            .py(px(4.0));
        if scroll {
            panel = panel.max_h(px(max_h));
        }
        let mut panel = panel.id(("ctx-panel", SubmenuKind::ShowMore as usize));
        if scroll {
            panel = panel.overflow_scroll();
        }
        panel = panel
            .on_hover({
                let v = view.clone();
                move |hovering: &bool, _, cx| {
                    if *hovering {
                        v.update(cx, |this, cx| {
                            this.set_open_submenu(Some(SubmenuKind::ShowMore), cx)
                        });
                    } else {
                        let v2 = v.clone();
                        v2.update(cx, |this, cx| {
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
        for row in rows {
            panel = panel.child(row);
        }
        panel
    }

    /// Non-interactive note row (loading / empty states).
    fn classic_note_row(label: &str, next_id: &mut usize) -> gpui::Stateful<Div> {
        let id = *next_id;
        *next_id += 1;
        div()
            .flex()
            .items_center()
            .px(px(12.0))
            .py(px(5.0))
            .id(("classic", id))
            .child(
                div()
                    .text_size(px(14.0))
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
        flip: bool,
        panel_top: f32,
        menu_left: f32,
    ) -> Vec<gpui::Stateful<Div>> {
        let mut rows = Vec::new();
        let mut acc_h = 0.0f32;
        for (i, entry) in entries.iter().enumerate() {
            let row_off = acc_h;
            acc_h += if entry.separator { 9.0 } else { 29.0 };
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
                .gap(px(12.0))
                .px(px(12.0))
                .py(px(5.0));
            if entry.enabled {
                inner = inner.cursor_pointer().hover(|s| s.bg(rgba(0xFFFFFF15u32)));
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
            wrapper = wrapper
                .on_hover(
                    move |hovering: &bool, window: &mut Window, cx: &mut App| {
                        if *hovering {
                            let pos = window.mouse_position();
                            let tip = tip.clone();
                            let path = hover_path.clone();
                            v_hover.update(cx, |this, cx| {
                                this.set_classic_open_intent(path, cx);
                                this.menu_tip = Some((tip, pos));
                            });
                        } else {
                            let mut parent = hover_path.clone();
                            parent.pop();
                            v_hover.update(cx, |this, cx| {
                                // Diagonal travel into the nested panel gets
                                // the same grace as top-level flyouts.
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
            if show_kids {
                // Nested panel: same bottom-anchor treatment, positioned
                // from this row's absolute offset inside the parent panel.
                let nested_row_bottom = panel_top + row_off + 29.0;
                let mut nested_h = 8.0f32;
                for e in entry.children.iter() {
                    nested_h += if e.separator { 9.0 } else { 29.0 };
                }
                let nested_max = (nested_row_bottom - 16.0).max(87.0);
                let nested_top = nested_row_bottom - nested_h.min(nested_max);
                // Flipped panels open left — unless the menu itself hugs the
                // left edge, in which case opening right avoids clipping.
                let nested_left = if flip && menu_left > 250.0 {
                    -242.0
                } else {
                    258.0
                };
                let mut nid = *next_id;
                let kids = Self::classic_rows(
                    &entry.children,
                    open,
                    &path,
                    view.clone(),
                    paths.clone(),
                    dir.clone(),
                    &mut nid,
                    flip,
                    nested_top,
                    menu_left,
                );
                *next_id = nid;
                let mut nested = div()
                    .absolute()
                    .bottom(px(4.0))
                    .left(px(nested_left))
                    .w(px(240.0))
                    .flex_col()
                    .bg(rgba(0x1A1A1AF2u32))
                    .border(px(1.0))
                    .border_color(rgba(UiConstants::STROKE_RGBA))
                    .rounded(px(8.0))
                    .shadow_lg()
                    .py(px(4.0));
                let nested_scroll = nested_h > nested_max;
                if nested_scroll {
                    nested = nested.max_h(px(nested_max));
                }
                let mut nested = nested.id(("ctx-nested", id));
                if nested_scroll {
                    nested = nested.overflow_scroll();
                }
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
                                    this.set_classic_open(keep, cx)
                                });
                            } else {
                                let v2 = v.clone();
                                let up = up.clone();
                                v2.update(cx, |this, cx| {
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
                if nested_h > nested_max {
                    nested = nested.max_h(px(nested_max)).overflow_scroll();
                }
                for row in kids {
                    nested = nested.child(row);
                }
                wrapper = wrapper.child(nested);
            }
            rows.push(wrapper);
        }
        rows
    }

    /// Layout › flyout (Details / List / Grid / Columns).
    pub(crate) fn layout_submenu(view: Entity<crate::app::AppWindow>,
        flip: bool) -> gpui::Stateful<Div> {
        let modes = [
            ("\u{E8A1}", "Details", crate::config::config::ViewMode::Details),
            ("\u{E8A1}", "List", crate::config::config::ViewMode::List),
            ("\u{E8A1}", "Grid", crate::config::config::ViewMode::Grid),
            ("\u{E8A1}", "Columns", crate::config::config::ViewMode::Columns),
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
        Self::submenu_panel(rows, SubmenuKind::Layout, view.clone(), flip)
    }

    /// Sort by › flyout.
    pub(crate) fn sort_submenu(view: Entity<crate::app::AppWindow>,
        flip: bool) -> gpui::Stateful<Div> {
        let cols = [
            ("\u{E2C7}", "Name", crate::config::config::SortBy::Name),
            ("\u{E2C7}", "Date modified", crate::config::config::SortBy::DateModified),
            ("\u{E2C7}", "Date created", crate::config::config::SortBy::DateCreated),
            ("\u{E2C7}", "Type", crate::config::config::SortBy::Type),
            ("\u{E2C7}", "Size", crate::config::config::SortBy::Size),
        ];
        let mut rows = Vec::new();
        for (glyph, label, col) in cols {
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
        Self::submenu_panel(rows, SubmenuKind::SortBy, view.clone(), flip)
    }

    /// Group by › flyout.
    pub(crate) fn group_submenu(view: Entity<crate::app::AppWindow>,
        flip: bool) -> gpui::Stateful<Div> {
        let groups = [
            ("\u{E2C7}", "None", crate::config::config::GroupColumn::None),
            ("\u{E2C7}", "Name", crate::config::config::GroupColumn::Name),
            ("\u{E2C7}", "Date modified", crate::config::config::GroupColumn::DateModified),
            ("\u{E2C7}", "Date created", crate::config::config::GroupColumn::DateCreated),
            ("\u{E2C7}", "Size", crate::config::config::GroupColumn::Size),
            ("\u{E2C7}", "Type", crate::config::config::GroupColumn::FileType),
        ];
        let mut rows = Vec::new();
        for (glyph, label, col) in groups {
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
        Self::submenu_panel(rows, SubmenuKind::GroupBy, view.clone(), flip)
    }

    /// New › flyout.
    pub(crate) fn new_submenu(view: Entity<crate::app::AppWindow>,
        flip: bool) -> gpui::Stateful<Div> {
        let v1 = view.clone();
        let v2 = view.clone();
        Self::submenu_panel(
            vec![
                Self::context_menu_item("\u{E8F4}", "New Folder", move |_, _, cx| {
                    v1.update(cx, |this, cx| this.create_new_folder(cx));
                }),
                Self::context_menu_item("\u{E7C3}", "New Text Document", move |_, _, cx| {
                    v2.update(cx, |this, cx| this.create_new_text_document(cx));
                }),
            ],
            SubmenuKind::New,
            view.clone(),
            flip,
        )
    }

    /// Compress › flyout (ZIP creation + conditional Extract Here).
    pub(crate) fn compress_submenu(
        view: Entity<crate::app::AppWindow>,
        flip: bool,
        is_archive: bool,
        archive_path: Option<crate::core::path_util::FilesPath>,
    ) -> gpui::Stateful<Div> {
        let v = view.clone();
        let mut rows = vec![Self::context_menu_item(
            "\u{E8B7}",
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
                    "\u{E8B7}",
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
        Self::submenu_panel(rows, SubmenuKind::Compress, view.clone(), flip)
    }

    /// Send to › flyout, populated live from the Windows SendTo folder.
    /// Non-Windows builds show a single disabled row (native provider hook).
    pub(crate) fn sendto_submenu(view: Entity<crate::app::AppWindow>,
        flip: bool) -> gpui::Stateful<Div> {
        let mut rows = Vec::new();
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
                        let glyph = if target.is_dir() { "\u{E8B7}" } else { "\u{E8A5}" };
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
        if rows.is_empty() {
            rows.push(Self::context_menu_item("\u{E8A5}", "(No targets)", |_, _, _| {}));
        }
        Self::submenu_panel(rows, SubmenuKind::SendTo, view.clone(), flip)
    }
}

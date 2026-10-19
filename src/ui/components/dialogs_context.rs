use crate::ui::components::dialogs::{DialogType, SidebarMenuTarget, SubmenuKind};
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
        _position: Point<Pixels>,
        has_selection: bool,
        selected_path: Option<crate::core::path_util::FilesPath>,
        selected_count: usize,
        is_dir: bool,
        open_submenu: Option<crate::ui::components::dialogs::SubmenuKind>,
        selected_paths: Vec<std::path::PathBuf>,
        current_dir: std::path::PathBuf,
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
                    v.update(cx, |this, cx| this.cut_selected(cx));
                });
                let v = v_copy.clone();
                let copy_btn: Box<
                    dyn Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
                > = Box::new(move |_, _, cx| {
                    v.update(cx, |this, cx| this.copy_selected(cx));
                });
                let v = v_paste.clone();
                let paste_btn: Box<
                    dyn Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
                > = Box::new(move |_, _, cx| {
                    v.update(cx, |this, cx| this.paste(cx));
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
                        cx.notify();
                    });
                });
                let v = v_delete.clone();
                let delete_btn: Box<
                    dyn Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
                > = Box::new(move |_, _, cx| {
                    v.update(cx, |this, cx| this.delete_selected(cx));
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
                menu = menu.child(Self::context_menu_toolbar(vec![
                    ("\u{E8C6}", cut_btn),
                    ("\u{E8C8}", copy_btn),
                    ("\u{E77F}", paste_btn),
                    ("\u{E8AC}", rename_btn),
                    ("\u{E72D}", share_btn),
                    ("\u{E74D}", delete_btn),
                    ("\u{E946}", props_btn),
                ]));
                menu = menu.child(Self::context_menu_separator());
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
                    Self::submenu_panel(ow_rows),
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
            }

            // Paste shortcut (from clipboard files)
            menu = menu.child(Self::context_menu_row("\u{E77F}", "Paste shortcut", MenuTrailing::None, {
                let v = v_paste.clone();
                move |_, _, cx| {
                    v.update(cx, |this, cx| this.paste_as_shortcut(cx));
                }
            }));

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

            // Create folder with selection
            menu = menu.child(Self::context_menu_row("\u{E8F4}", "Create folder with selection", MenuTrailing::None, {
                let v = v_close.clone();
                move |_, _, cx| {
                    v.update(cx, |this, cx| this.create_folder_with_selection(cx));
                }
            }));

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
                    Self::compress_submenu(view.clone(), is_archive, selected_path.clone()),
                    v_sub.clone(),
                    move |_, _, cx| {
                        v_fire.update(cx, |this, cx| {
                            this.set_open_submenu(Some(SubmenuKind::Compress), cx);
                        });
                    },
                ));
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
                    Self::sendto_submenu(view.clone()),
                    v_sub.clone(),
                    move |_, _, cx| {
                        v_fire.update(cx, |this, cx| {
                            this.set_open_submenu(Some(SubmenuKind::SendTo), cx);
                        });
                    },
                ));
            }

            menu = menu.child(Self::context_menu_separator());

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

            menu = menu.child(Self::context_menu_separator());

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
            menu = menu.child(Self::context_menu_item("\u{E74D}", "Delete Permanently", {
                let v = v_close.clone();
                move |_, _, cx| {
                    v.update(cx, |this, cx| this.delete_permanently(cx));
                }
            }));

            // Show more options (native classic shell menu; Windows only)
            #[cfg(target_os = "windows")]
            {
                let paths = sel_paths.clone();
                let dir = cur_dir.clone();
                let v = v_sub.clone();
                menu = menu.child(Self::context_menu_row(
                    "\u{E10C}",
                    "Show more options",
                    MenuTrailing::Chevron,
                    move |event, _, cx| {
                        let ex: f32 = event.position.x.into();
                        let ey: f32 = event.position.y.into();
                        let paths = paths.clone();
                        let dir = dir.clone();
                        v.update(cx, |this, cx| {
                            this.show_classic_menu_for(paths, dir, Some((ex, ey)), cx)
                        });
                    },
                ));
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
                    Self::layout_submenu(view.clone()),
                    v_sub.clone(),
                    move |_, _, cx| {
                        v_fire.update(cx, |this, cx| {
                            this.set_open_submenu(Some(SubmenuKind::Layout), cx);
                        });
                    },
                ));
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
                    Self::sort_submenu(view.clone()),
                    v_sub.clone(),
                    move |_, _, cx| {
                        v_fire.update(cx, |this, cx| {
                            this.set_open_submenu(Some(SubmenuKind::SortBy), cx);
                        });
                    },
                ));
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
                    Self::group_submenu(view.clone()),
                    v_sub.clone(),
                    move |_, _, cx| {
                        v_fire.update(cx, |this, cx| {
                            this.set_open_submenu(Some(SubmenuKind::GroupBy), cx);
                        });
                    },
                ));
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

            menu = menu.child(Self::context_menu_separator());

            // New ›
            {
                let n_open = open_submenu == Some(SubmenuKind::New);
                let v_fire = v_sub.clone();
                menu = menu.child(Self::submenu_row(
                    "\u{E8F4}",
                    "New",
                    SubmenuKind::New,
                    n_open,
                    Self::new_submenu(view.clone()),
                    v_sub.clone(),
                    move |_, _, cx| {
                        v_fire.update(cx, |this, cx| {
                            this.set_open_submenu(Some(SubmenuKind::New), cx);
                        });
                    },
                ));
            }

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

            // Open in Terminal (Ctrl+`)
            menu = menu.child(Self::context_menu_row("\u{E756}", "Open in Terminal", MenuTrailing::Hint("Ctrl+`"), {
                let v = v_close.clone();
                move |_, _, cx| {
                    v.update(cx, |this, cx| {
                        this.open_terminal_at_current(cx);
                        this.close_dialog(cx);
                    });
                }
            }));

            // Show more options (native classic shell menu; Windows only)
            #[cfg(target_os = "windows")]
            {
                let dir = cur_dir.clone();
                let v = v_sub.clone();
                menu = menu.child(Self::context_menu_row(
                    "\u{E10C}",
                    "Show more options",
                    MenuTrailing::Chevron,
                    move |event, _, cx| {
                        let ex: f32 = event.position.x.into();
                        let ey: f32 = event.position.y.into();
                        let dir = dir.clone();
                        v.update(cx, |this, cx| {
                            this.show_classic_menu_for(Vec::new(), dir, Some((ex, ey)), cx)
                        });
                    },
                ));
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
            Box<dyn Fn(&MouseDownEvent, &mut Window, &mut App) + 'static>,
        )>,
    ) -> Div {
        let mut row = div()
            .flex()
            .items_center()
            .gap(px(2.0))
            .px(px(8.0))
            .py(px(3.0));
        for (glyph, action) in buttons {
            row = row.child(
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .w(px(34.0))
                    .h(px(28.0))
                    .rounded(px(4.0))
                    .cursor_pointer()
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
        submenu: Div,
        view: Entity<crate::app::AppWindow>,
        on_click: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    ) -> gpui::Stateful<Div> {
        let v_hover = view.clone();
        let stateful = div()
            .relative()
            .child(Self::context_menu_row(glyph, label, MenuTrailing::Chevron, on_click))
            .id(("ctx-submenu", kind as usize));
        let mut wrapper = stateful.on_hover(move |hovering: &bool, _, cx| {
            v_hover.update(cx, |this, cx| {
                this.set_open_submenu(if *hovering { Some(kind) } else { None }, cx);
            });
        });
        if is_open {
            wrapper = wrapper.child(submenu);
        }
        wrapper
    }

    /// Flyout panel anchored to the right of its parent row.
    pub(crate) fn submenu_panel(rows: Vec<Div>) -> Div {
        let mut panel = div()
            .absolute()
            .top(px(-8.0))
            .left(px(288.0))
            .w(px(220.0))
            .flex_col()
            .bg(rgba(0x1A1A1AF2u32))
            .border(px(1.0))
            .border_color(rgba(UiConstants::STROKE_RGBA))
            .rounded(px(8.0))
            .shadow_lg()
            .py(px(4.0))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation());
        for row in rows {
            panel = panel.child(row);
        }
        panel
    }

    /// Layout › flyout (Details / List / Grid / Columns).
    pub(crate) fn layout_submenu(view: Entity<crate::app::AppWindow>) -> Div {
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
        Self::submenu_panel(rows)
    }

    /// Sort by › flyout.
    pub(crate) fn sort_submenu(view: Entity<crate::app::AppWindow>) -> Div {
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
        Self::submenu_panel(rows)
    }

    /// Group by › flyout.
    pub(crate) fn group_submenu(view: Entity<crate::app::AppWindow>) -> Div {
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
        Self::submenu_panel(rows)
    }

    /// New › flyout.
    pub(crate) fn new_submenu(view: Entity<crate::app::AppWindow>) -> Div {
        let v1 = view.clone();
        let v2 = view.clone();
        Self::submenu_panel(vec![
            Self::context_menu_item("\u{E8F4}", "New Folder", move |_, _, cx| {
                v1.update(cx, |this, cx| this.create_new_folder(cx));
            }),
            Self::context_menu_item("\u{E7C3}", "New Text Document", move |_, _, cx| {
                v2.update(cx, |this, cx| this.create_new_text_document(cx));
            }),
        ])
    }

    /// Compress › flyout (ZIP creation + conditional Extract Here).
    pub(crate) fn compress_submenu(
        view: Entity<crate::app::AppWindow>,
        is_archive: bool,
        archive_path: Option<crate::core::path_util::FilesPath>,
    ) -> Div {
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
        Self::submenu_panel(rows)
    }

    /// Send to › flyout, populated live from the Windows SendTo folder.
    /// Non-Windows builds show a single disabled row (native provider hook).
    pub(crate) fn sendto_submenu(view: Entity<crate::app::AppWindow>) -> Div {
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
        Self::submenu_panel(rows)
    }
}

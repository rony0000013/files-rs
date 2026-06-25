use crate::ui::components::dialogs::{DialogType, SidebarMenuTarget};
use crate::ui::constants::UiConstants;
use gpui::*;

impl crate::ui::components::dialogs::DialogsView {
    pub(crate) fn build_context_menu_dialog(
        item_index: Option<usize>,
        _position: Point<Pixels>,
        has_selection: bool,
        selected_path: Option<crate::core::path_util::FilesPath>,
        view: Entity<crate::app::AppWindow>,
        cx: &mut Context<crate::app::AppWindow>,
    ) -> Div {
        // Clone view for closures
        let v_close = view.clone();
        let v_open = view.clone();
        let v_open_new_tab = view.clone();
        let v_open_new_window = view.clone();
        let v_copy = view.clone();
        let v_cut = view.clone();
        let v_paste = view.clone();
        let v_rename = view.clone();
        let v_delete = view.clone();

        // Context menu container with proper sizing per DESIGN_SPEC.md:
        // - max width 250px
        // - max height 480px
        let mut menu = div()
            .flex_col()
            .bg(rgba(0xF21A1A1Au32)) // LayerFillColorDefault dark
            .border(px(1.0))
            .border_color(rgba(UiConstants::STROKE_RGBA))
            .rounded(px(8.0))
            .shadow_lg()
            .w(px(250.0))
            .max_h(px(480.0));

        if has_selection {
            // ═══════════════════════════════════════════════════════════
            // FILES COMMUNITY LAYOUT — File/Folder context menu
            // Primary section (48px): Open actions
            // Secondary section (32px): Pins → Clipboard → Actions → Properties
            // ═══════════════════════════════════════════════════════════

            // ── PRIMARY ITEMS (48px) ──
            // Open
            menu = menu.child(Self::context_menu_item_48px("\u{E8DA}", "Open", {
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

            // Open With - opens the Windows "Open With" dialog
            menu = menu.child(Self::context_menu_item_48px("\u{E8DA}", "Open With", {
                let v = v_close.clone();
                let path = selected_path.clone();
                move |_, _, cx| {
                    if let Some(ref p) = path {
                        let path_str = p.to_string_lossy().to_string();
                        let _ = std::process::Command::new("rundll32")
                            .args(["shell32.dll,OpenAs_RunDLL", &path_str])
                            .spawn();
                    }
                    v.update(cx, |this, cx| this.close_dialog(cx));
                }
            }));

            // Open in New Tab
            menu = menu.child(Self::context_menu_item_48px(
                "\u{E8A7}",
                "Open in New Tab",
                {
                    let path = selected_path.clone();
                    move |_, _, cx| {
                        if let Some(ref p) = path {
                            let path = p.clone();
                            v_open_new_tab.update(cx, |this, cx| {
                                this.open_in_new_tab(path, cx);
                                this.close_dialog(cx);
                            });
                        }
                    }
                },
            ));

            // Open in New Window
            menu = menu.child(Self::context_menu_item_48px(
                "\u{E8A7}",
                "Open in New Window",
                {
                    let path = selected_path.clone();
                    move |_, _, cx| {
                        if let Some(ref p) = path {
                            let path = p.clone();
                            v_open_new_window.update(cx, |this, cx| {
                                this.open_in_new_window(path, cx);
                                this.close_dialog(cx);
                            });
                        }
                    }
                },
            ));

            // Open in Terminal (48px primary in Files Community)
            menu = menu.child(Self::context_menu_item_48px(
                "\u{E756}",
                "Open in Terminal",
                {
                    let v = v_close.clone();
                    let path = selected_path.clone();
                    move |_, _, cx| {
                        if let Some(ref p) = path {
                            // Open Windows Terminal as independent process
                            #[cfg(windows)]
                            {
                                use std::ffi::OsStr;
                                use std::os::windows::ffi::OsStrExt;
                                use windows::Win32::UI::Shell::ShellExecuteW;
                                use windows::Win32::UI::WindowsAndMessaging::SW_SHOW;

                                let path_str = format!("-d \"{}\"", p.to_string_lossy());
                                let params_wide: Vec<u16> = OsStr::new(&path_str)
                                    .encode_wide()
                                    .chain(std::iter::once(0))
                                    .collect();
                                let exe_wide: Vec<u16> = OsStr::new("wt.exe")
                                    .encode_wide()
                                    .chain(std::iter::once(0))
                                    .collect();

                                unsafe {
                                    ShellExecuteW(
                                        None,
                                        windows::core::PCWSTR::null(),
                                        windows::core::PCWSTR(exe_wide.as_ptr()),
                                        windows::core::PCWSTR(params_wide.as_ptr()),
                                        None,
                                        SW_SHOW,
                                    );
                                }
                            }
                            #[cfg(not(windows))]
                            {
                                let _ = std::process::Command::new("x-terminal-emulator")
                                    .arg(&p.to_string_lossy().to_string())
                                    .spawn();
                            }
                        }
                        v.update(cx, |this, cx| this.close_dialog(cx));
                    }
                },
            ));

            // ── SEPARATOR ──
            menu = menu.child(Self::context_menu_separator());

            // ── PIN SECTION (32px) ──
            // Pin to Quick Access
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
            // Pin to Start
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

            // ── SEPARATOR ──
            menu = menu.child(Self::context_menu_separator());

            // ── CLIPBOARD SECTION (32px) ──
            // Cut
            menu = menu.child(Self::context_menu_item("\u{E8C6}", "Cut", {
                let v = v_cut.clone();
                move |_, _, cx| {
                    v.update(cx, |this, cx| this.cut_selected(cx));
                }
            }));

            // Copy
            menu = menu.child(Self::context_menu_item("\u{E8C8}", "Copy", {
                let v = v_copy.clone();
                move |_, _, cx| {
                    v.update(cx, |this, cx| this.copy_selected(cx));
                }
            }));

            // Copy as Path - copies the full path to the system clipboard
            menu = menu.child(Self::context_menu_item("\u{E8C8}", "Copy as Path", {
                let v = v_close.clone();
                let path = selected_path.clone();
                move |_, _, cx| {
                    if let Some(ref p) = path {
                        let path_str = p.to_string_lossy().to_string();
                        cx.write_to_clipboard(ClipboardItem::new_string(path_str));
                    }
                    v.update(cx, |this, cx| this.close_dialog(cx));
                }
            }));

            // Paste
            menu = menu.child(Self::context_menu_item("\u{E77F}", "Paste", {
                let v = v_paste.clone();
                move |_, _, cx| {
                    v.update(cx, |this, cx| this.paste(cx));
                }
            }));

            // ── SEPARATOR ──
            menu = menu.child(Self::context_menu_separator());

            // ── ACTIONS SECTION (32px) ──
            // Rename
            menu = menu.child(Self::context_menu_item("\u{E8AC}", "Rename", {
                let v = v_rename.clone();
                let idx = item_index;
                move |_, _, cx| {
                    if let Some(i) = idx {
                        v.update(cx, |this, cx| this.start_rename(i, cx));
                    }
                }
            }));

            // Share - invokes the Windows Share dialog
            menu = menu.child(Self::context_menu_item(
                "\u{E72D}",
                "Share",
                {
                    let v = v_close.clone();
                    let path = selected_path.clone();
                    move |_, _, cx| {
                        v.update(cx, |this, cx| {
                            if let Some(ref p) = path {
                                #[cfg(windows)]
                                {
                                    let share_path = p.clone();
                                    cx.background_executor().spawn(async move {
                                        let service = crate::platform::windows::share_service::ShareService::new();
                                        let _ = service.share_file(0, &share_path).await;
                                    }).detach();
                                }
                                #[cfg(not(windows))]
                                {
                                    tracing::info!("Share not implemented on this platform");
                                }
                            }
                            cx.notify();
                        });
                    }
                },
            ));

            // Delete
            menu = menu.child(Self::context_menu_item("\u{E74D}", "Delete", {
                let v = v_delete.clone();
                move |_, _, cx| {
                    v.update(cx, |this, cx| this.delete_selected(cx));
                }
            }));

            // ── SEPARATOR ──
            menu = menu.child(Self::context_menu_separator());

            // ── PROPERTIES (32px) ──
            menu = menu.child(Self::context_menu_item("\u{E946}", "Properties", {
                let v = v_close.clone();
                let path = selected_path.clone();
                move |_, _, cx| {
                    if let Some(ref p) = path {
                        let path_str = p.to_string_lossy().to_string();
                        let _ = std::process::Command::new("rundll32")
                            .args(["shell32.dll,PropertiesDialogW", &path_str])
                            .spawn();
                    }
                    v.update(cx, |this, cx| this.close_dialog(cx));
                }
            }));

            // ── SEPARATOR ──
            menu = menu.child(Self::context_menu_separator());

            // ── BONUS SECTION (Files Community extras) ──
            // Delete Permanently
            menu = menu.child(Self::context_menu_item("\u{E74D}", "Delete Permanently", {
                let v = v_close.clone();
                move |_, _, cx| {
                    v.update(cx, |this, cx| this.delete_permanently(cx));
                }
            }));

            // Create Shortcut
            menu = menu.child(Self::context_menu_item(
                "\u{E71B}",
                "Create Shortcut",
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

            // Compress
            menu = menu.child(Self::context_menu_item(
                "\u{E8B7}",
                "Compress",
                {
                    let v = v_close.clone();
                    move |_, _, cx| {
                        v.update(cx, |this, cx| {
                            let sources: Vec<std::path::PathBuf> = this.state.main_view.selected_indices.iter()
                                .filter_map(|&i| this.state.main_view.items.get(i))
                                .map(|item| item.path.as_path().to_path_buf())
                                .collect();
                            if !sources.is_empty() {
                                let current_dir = this.state.main_view.primary_path.as_path().to_path_buf();
                                let first_name = sources[0]
                                    .file_name()
                                    .map(|s| s.to_string_lossy().to_string())
                                    .unwrap_or_else(|| "archive".to_string());
                                let archive_name = format!("{}.zip", first_name);
                                let destination = current_dir.join(&archive_name);
                                this.state.active_dialog = Some(DialogType::CreateArchive {
                                    state: Box::new(
                                        crate::ui::dialog_services::archive_dialog::CreateArchiveState::new(
                                            sources,
                                            destination,
                                        )
                                    ),
                                });
                            }
                            cx.notify();
                        });
                    }
                },
            ));

            // Extract Here (shown when archive is selected)
            if selected_path
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
                .unwrap_or(false)
            {
                menu = menu.child(Self::context_menu_item(
                    "\u{E8B7}",
                    "Extract Here",
                    {
                        let v = v_close.clone();
                        let path = selected_path.clone();
                        move |_, _, cx| {
                            v.update(cx, |this, cx| {
                                if let Some(ref p) = path {
                                    let archive_path = p.as_path().to_path_buf();
                                    let current_dir = this.state.main_view.primary_path.as_path().to_path_buf();
                                    this.state.active_dialog = Some(DialogType::DecompressArchive {
                                        state: Box::new(
                                            crate::ui::dialog_services::decompress_dialog::DecompressState::new(
                                                archive_path,
                                                current_dir,
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

            // Open in VS Code (bonus)
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

            // Git Status (bonus)
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
        } else {
            // ═══════════════════════════════════════════════════════════
            // FILES COMMUNITY LAYOUT — Empty space/background context menu
            // ═══════════════════════════════════════════════════════════

            // Refresh - refreshes the current view
            menu = menu.child(Self::context_menu_item("\u{E72C}", "Refresh", {
                let v = v_close.clone();
                move |_, _, cx| {
                    v.update(cx, |this, cx| {
                        this.refresh(cx);
                        this.close_dialog(cx);
                    });
                }
            }));

            // ── SEPARATOR ──
            menu = menu.child(Self::context_menu_separator());

            // ── SORT BY SECTION ──
            menu = menu.child(Self::context_menu_section_header("Sort By"));

            // Use a closure to capture sort state and view for each sort option
            {
                let v = v_close.clone();
                menu = menu.child(Self::context_menu_item(
                    "\u{E2C7}",
                    "Name",
                    move |_, _, cx| {
                        v.update(cx, |this, cx| {
                            this.sort_by(crate::config::config::SortBy::Name, cx);
                            this.close_dialog(cx);
                        });
                    },
                ));
            }
            {
                let v = v_close.clone();
                menu = menu.child(Self::context_menu_item(
                    "\u{E2C7}",
                    "Date modified",
                    move |_, _, cx| {
                        v.update(cx, |this, cx| {
                            this.sort_by(crate::config::config::SortBy::DateModified, cx);
                            this.close_dialog(cx);
                        });
                    },
                ));
            }
            {
                let v = v_close.clone();
                menu = menu.child(Self::context_menu_item(
                    "\u{E2C7}",
                    "Date created",
                    move |_, _, cx| {
                        v.update(cx, |this, cx| {
                            this.sort_by(crate::config::config::SortBy::DateCreated, cx);
                            this.close_dialog(cx);
                        });
                    },
                ));
            }
            {
                let v = v_close.clone();
                menu = menu.child(Self::context_menu_item(
                    "\u{E2C7}",
                    "Size",
                    move |_, _, cx| {
                        v.update(cx, |this, cx| {
                            this.sort_by(crate::config::config::SortBy::Size, cx);
                            this.close_dialog(cx);
                        });
                    },
                ));
            }
            {
                let v = v_close.clone();
                menu = menu.child(Self::context_menu_item(
                    "\u{E2C7}",
                    "Type",
                    move |_, _, cx| {
                        v.update(cx, |this, cx| {
                            this.sort_by(crate::config::config::SortBy::Type, cx);
                            this.close_dialog(cx);
                        });
                    },
                ));
            }

            // ── VIEW SECTION ──
            menu = menu.child(Self::context_menu_separator());
            menu = menu.child(Self::context_menu_section_header("View"));

            {
                let v = v_close.clone();
                menu = menu.child(Self::context_menu_item(
                    "\u{E8A1}",
                    "Details",
                    move |_, _, cx| {
                        v.update(cx, |this, cx| {
                            this.set_view_mode(crate::config::config::ViewMode::Details, cx);
                            this.close_dialog(cx);
                        });
                    },
                ));
            }
            {
                let v = v_close.clone();
                menu = menu.child(Self::context_menu_item(
                    "\u{E8A1}",
                    "List",
                    move |_, _, cx| {
                        v.update(cx, |this, cx| {
                            this.set_view_mode(crate::config::config::ViewMode::List, cx);
                            this.close_dialog(cx);
                        });
                    },
                ));
            }
            {
                let v = v_close.clone();
                menu = menu.child(Self::context_menu_item(
                    "\u{E8A1}",
                    "Grid",
                    move |_, _, cx| {
                        v.update(cx, |this, cx| {
                            this.set_view_mode(crate::config::config::ViewMode::Grid, cx);
                            this.close_dialog(cx);
                        });
                    },
                ));
            }
            {
                let v = v_close.clone();
                menu = menu.child(Self::context_menu_item(
                    "\u{E8A1}",
                    "Columns",
                    move |_, _, cx| {
                        v.update(cx, |this, cx| {
                            this.set_view_mode(crate::config::config::ViewMode::Columns, cx);
                            this.close_dialog(cx);
                        });
                    },
                ));
            }
            // ── SEPARATOR ──
            menu = menu.child(Self::context_menu_separator());

            // ── PASTE ──
            menu = menu.child(Self::context_menu_item("\u{E77F}", "Paste", {
                let v = v_paste.clone();
                move |_, _, cx| {
                    v.update(cx, |this, cx| this.paste(cx));
                }
            }));

            // ── SEPARATOR ──
            menu = menu.child(Self::context_menu_separator());

            // ── NEW SECTION (48px primary) ──
            // New Folder
            menu = menu.child(Self::context_menu_item_48px("\u{E8F4}", "New Folder", {
                let v = v_close.clone();
                move |_, _, cx| {
                    v.update(cx, |this, cx| {
                        let current_dir = this.state.main_view.primary_path.as_path().to_path_buf();
                        let mut folder_name = "New Folder".to_string();
                        let mut counter = 1;
                        while current_dir.join(&folder_name).exists() {
                            folder_name = format!("New Folder ({})", counter);
                            counter += 1;
                        }
                        let new_folder_path = current_dir.join(&folder_name);
                        match std::fs::create_dir(&new_folder_path) {
                            Ok(_) => {
                                tracing::info!("Created folder: {:?}", new_folder_path);
                                this.refresh(cx);
                            }
                            Err(e) => {
                                tracing::error!(
                                    "Failed to create folder {:?}: {}",
                                    new_folder_path,
                                    e
                                );
                            }
                        }
                        cx.notify();
                    });
                }
            }));

            // New File - creates a new empty text file at the current path
            menu = menu.child(Self::context_menu_item_48px(
                "\u{E7C3}",
                "New Text Document",
                {
                    let v = v_close.clone();
                    move |_, _, cx| {
                        v.update(cx, |this, cx| {
                            let current_dir =
                                this.state.main_view.primary_path.as_path().to_path_buf();
                            let mut file_name = "New Text Document.txt".to_string();
                            let mut counter = 1;
                            while current_dir.join(&file_name).exists() {
                                file_name = format!("New Text Document ({}).txt", counter);
                                counter += 1;
                            }
                            let new_file_path = current_dir.join(&file_name);
                            match std::fs::File::create(&new_file_path) {
                                Ok(_) => {
                                    tracing::info!("Created file: {:?}", new_file_path);
                                    this.refresh(cx);
                                }
                                Err(e) => {
                                    tracing::error!(
                                        "Failed to create file {:?}: {}",
                                        new_file_path,
                                        e
                                    );
                                }
                            }
                            cx.notify();
                        });
                    }
                },
            ));

            // ── SEPARATOR ──
            menu = menu.child(Self::context_menu_separator());

            // Properties (for current folder)
            menu = menu.child(Self::context_menu_item("\u{E946}", "Properties", {
                let v = v_close.clone();
                move |_, _, cx| {
                    v.update(cx, |this, cx| {
                        let path_str = this
                            .state
                            .main_view
                            .primary_path
                            .to_string_lossy()
                            .to_string();
                        let _ = std::process::Command::new("rundll32")
                            .args(["shell32.dll,PropertiesDialogW", &path_str])
                            .spawn();
                        this.close_dialog(cx);
                    });
                }
            }));

            // Open in Terminal - opens Windows Terminal at the current directory
            menu = menu.child(Self::context_menu_item("\u{E756}", "Open in Terminal", {
                let v = v_close.clone();
                move |_, _, cx| {
                    v.update(cx, |this, cx| {
                        let path_str = this
                            .state
                            .main_view
                            .primary_path
                            .to_string_lossy()
                            .to_string();
                        // Open Windows Terminal as independent process
                        #[cfg(windows)]
                        {
                            use std::ffi::OsStr;
                            use std::os::windows::ffi::OsStrExt;
                            use windows::Win32::UI::Shell::ShellExecuteW;
                            use windows::Win32::UI::WindowsAndMessaging::SW_SHOW;

                            let params = format!("-d \"{}\"", &path_str);
                            let params_wide: Vec<u16> = OsStr::new(&params)
                                .encode_wide()
                                .chain(std::iter::once(0))
                                .collect();
                            let exe_wide: Vec<u16> = OsStr::new("wt.exe")
                                .encode_wide()
                                .chain(std::iter::once(0))
                                .collect();

                            unsafe {
                                ShellExecuteW(
                                    None,
                                    windows::core::PCWSTR::null(),
                                    windows::core::PCWSTR(exe_wide.as_ptr()),
                                    windows::core::PCWSTR(params_wide.as_ptr()),
                                    None,
                                    SW_SHOW,
                                );
                            }
                        }
                        #[cfg(not(windows))]
                        {
                            let _ = std::process::Command::new("x-terminal-emulator")
                                .arg(&path_str)
                                .spawn();
                        }
                        this.close_dialog(cx);
                    });
                }
            }));
        }
        menu
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
        label: &str,
        on_click: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    ) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .gap(px(12.0))
            .px(px(12.0))
            .py(px(8.0))
            .cursor_pointer()
            .hover(|s| s.bg(rgba(0xFFFFFF15u32)))
            .on_mouse_down(MouseButton::Left, on_click)
            .child(
                div()
                    .text_size(px(14.0))
                    .text_color(rgb(UiConstants::ACCENT_COLOR_RGB))
                    .child(glyph.to_string()),
            )
            .child(
                div()
                    .text_size(px(13.0))
                    .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                    .child(label.to_string()),
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
                    .text_color(rgb(UiConstants::ACCENT_COLOR_RGB))
                    .child(glyph.to_string()),
            )
            .child(
                div()
                    .text_size(px(14.0))
                    .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                    .child(label.to_string()),
            )
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

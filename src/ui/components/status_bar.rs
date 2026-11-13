//! Bottom Status Bar (`StatusBar.xaml` in `Files-community` Unibody Design)
//!
//! Layout (`32px` height, integrated into the bottom of the `ModernShellPage` rounded card):
//! - **LEFT (`Spacing="8"`, `TextFillColorSecondaryBrush`)**:
//!   - `DirectoryItemCount` (`"X items"` or `"X item"`)
//!   - `1px × 18px` vertical divider + `SelectedItemsCountString` (`"Y items selected"`) when items are selected
//!   - `1px × 18px` vertical divider + `ItemSize` (`"Z MB"`) when selection size is available
//! - **RIGHT (`Spacing="4"`, subtle transparent command buttons)**:
//!   - Git Sync button (`\u{E895}` + sync status) + `1px × 18px` divider + Git Branch button (`\u{E8AD}` + branch name)
//!   - `Open in Terminal / IDE` button
//!   - Subtle zoom level button (`100%`)

use crate::app::AppState;
use crate::ui::constants::UiConstants;
use gpui::*;

/// Status bar state — tracks directory & drive telemetry.
#[derive(Debug, Clone, Default)]
pub struct StatusBarState {
    /// Total items in current directory
    pub total_count: usize,
    /// Number of folders
    pub folder_count: usize,
    /// Number of files
    pub file_count: usize,
    /// Number of selected items
    pub selected_count: usize,
    /// Total size of selected files (in bytes)
    pub selected_size_bytes: u64,
    /// Free bytes on the current drive
    pub free_bytes: u64,
    /// Total bytes on the current drive
    pub total_bytes: u64,
    /// Git branch name (if in a repo)
    pub git_branch: Option<String>,
    /// Whether git actions should be shown
    pub show_git_actions: bool,
    /// Current path (for display purposes)
    pub current_path: Option<String>,
    /// Set while an async folder-size walk is in flight for `folder_size_for`.
    pub folder_size_pending: bool,
    /// Path the pending/computed folder size belongs to (guards stale async results).
    pub folder_size_for: Option<crate::core::path_util::FilesPath>,
    /// Spinner animation frame (0..3) for the pending folder size.
    pub spinner_frame: u32,
    /// Monotonic request ID to discard stale async folder-size walks.
    pub folder_size_request_id: u64,
}

impl StatusBarState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Update from individual fields instead of &AppState to avoid borrow conflicts.
    pub fn sync(&mut self, item_count: usize, folder_count: usize, file_count: usize) {
        self.total_count = item_count;
        self.folder_count = folder_count;
        self.file_count = file_count;
        self.selected_count = 0;
        self.selected_size_bytes = 0;
        self.free_bytes = 0;
        self.total_bytes = 0;
        self.folder_size_pending = false;
        self.folder_size_for = None;
        self.spinner_frame = 0;
    }
}

/// Format bytes as human-readable string (KB, MB, GB, TB).
pub fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    const TB: u64 = GB * 1024;

    if bytes >= TB {
        format!("{:.1} TB", bytes as f64 / TB as f64)
    } else if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

pub struct StatusBarView;

impl StatusBarView {
    fn vertical_divider() -> Div {
        div()
            .w(px(1.0))
            .h(px(18.0))
            .bg(rgba(UiConstants::DIVIDER_RGBA))
    }

    pub fn build(
        state: &StatusBarState,
        app: &AppState,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        use crate::config::config::ViewMode;
        use crate::storage::storage::FileType;

        let active_pane = app.active_pane_focus();
        let pv = app.pane_view(active_pane);
        let zoom_level = pv.zoom_level.clamp(0.0, 1.0);
        let zoom_pct = (zoom_level * 100.0).round() as u32;

        let (view_icon, view_label) = match pv.view_mode {
            ViewMode::Details => ("\u{E8FD}", "Details"),
            ViewMode::List => ("\u{EA37}", "List"),
            ViewMode::Columns => ("\u{E8F1}", "Columns"),
            ViewMode::Cards => ("\u{E8A9}", "Cards"),
            ViewMode::Grid => ("\u{E80A}", "Icons"),
            ViewMode::Tree => ("\u{F0E2}", "Tree"),
        };

        let is_zoom_menu_open = matches!(
            app.active_dialog,
            Some(crate::ui::components::dialogs::DialogType::ZoomSlider { .. })
        );

        let mut selected_folders = 0usize;
        if state.selected_count > 0 {
            for &idx in &pv.selected_indices {
                if let Some(item) = pv.items.get(idx) {
                    if matches!(item.file_type, FileType::Directory) {
                        selected_folders += 1;
                    }
                }
            }
        }
        let selected_files = state.selected_count.saturating_sub(selected_folders);

        let folder_count_label = if state.selected_count > 0 {
            format!("{}/{}", selected_folders, state.folder_count)
        } else {
            format!("{}", state.folder_count)
        };
        let file_count_label = if state.selected_count > 0 {
            format!("{}/{}", selected_files, state.file_count)
        } else {
            format!("{}", state.file_count)
        };

        let (sort_prio_icon, sort_prio_badge, sort_prio_tooltip, sort_prio_active_accent) =
            if pv.sort.folders_first {
                (
                    "\u{E8B7}",
                    Some("\u{E74A}"),
                    "Sort: Folders first (click for Files first)",
                    false,
                )
            } else if pv.sort.files_first {
                (
                    "\u{E7C3}",
                    Some("\u{E74A}"),
                    "Sort: Files first (click for Interleaved)",
                    true,
                )
            } else {
                (
                    "\u{E8CB}",
                    None,
                    "Sort: Interleaved folders & files (click for Folders first)",
                    true,
                )
            };

        let item_count_text = if state.total_count == 1 {
            "1 item".to_string()
        } else {
            format!("{} items", state.total_count)
        };

        let selected_count_text = if state.selected_count > 0 {
            if state.selected_count == 1 {
                Some("1 item selected".to_string())
            } else {
                Some(format!("{} items selected", state.selected_count))
            }
        } else {
            None
        };

        let sel_size_label = if state.selected_size_bytes > 0 {
            Some(format_size(state.selected_size_bytes))
        } else if state.folder_size_pending {
            let dots = ".".repeat((state.spinner_frame % 4) as usize);
            Some(format!("Calculating{}", dots))
        } else {
            None
        };

        // ── LEFT STACK (`StatusBar.xaml` Left Column: Spacing="8", TextFillColorSecondaryBrush)
        let mut left_row = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.0))
            .overflow_hidden()
            .child(
                div()
                    .text_size(px(12.0))
                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                    .child(item_count_text),
            );

        if let Some(sel_text) = selected_count_text {
            left_row = left_row
                .child(Self::vertical_divider())
                .child(
                    div()
                        .text_size(px(12.0))
                        .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                        .child(sel_text),
                );
        }

        if let Some(size_text) = sel_size_label {
            left_row = left_row
                .child(Self::vertical_divider())
                .child(
                    div()
                        .text_size(px(12.0))
                        .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                        .child(size_text),
                );
        }

        // ── RIGHT STACK (Fluent WinUI 3 + File Pilot status bar controls)
        let mut right_row = div().flex().flex_row().items_center().gap(px(4.0));

        if state.show_git_actions {
            if let Some(branch) = state.git_branch.as_ref() {
                let v_git_sync = view.clone();
                right_row = right_row
                    .child(
                        div()
                            .id("status-git-sync")
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(6.0))
                            .px(px(6.0))
                            .py(px(3.0))
                            .rounded(px(4.0))
                            .cursor_pointer()
                            .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
                            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                v_git_sync.update(cx, |this, cx| {
                                    let _ = this.refresh(cx);
                                });
                            })
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .font_family(UiConstants::FONT_ICONS)
                                    .text_color(rgb(UiConstants::ACCENT_COLOR_RGB))
                                    .child("\u{E895}"),
                            )
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                    .child("0 ↑  0 ↓"),
                            ),
                    )
                    .child(Self::vertical_divider())
                    .child(
                        div()
                            .id("status-git-branch")
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(6.0))
                            .px(px(6.0))
                            .py(px(3.0))
                            .rounded(px(4.0))
                            .cursor_pointer()
                            .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .font_family(UiConstants::FONT_ICONS)
                                    .text_color(rgb(UiConstants::ACCENT_COLOR_RGB))
                                    .child("\u{E8AD}"),
                            )
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                    .child(branch.clone()),
                            ),
                    )
                    .child(Self::vertical_divider());
            }
        }

        // Open in Terminal button (`\u{E756}`)
        {
            let v_term = view.clone();
            let v_term_hover = view.clone();
            right_row = right_row
                .child(
                    div()
                        .id("status-open-terminal")
                        .flex()
                        .items_center()
                        .justify_center()
                        .px(px(6.0))
                        .py(px(4.0))
                        .rounded(px(4.0))
                        .cursor_pointer()
                        .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                        .hover(|s| {
                            s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA))
                                .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                        })
                        .on_hover(move |hovered, win, cx| {
                            let h = *hovered;
                            let pos = win.mouse_position();
                            let v = v_term_hover.clone();
                            v.update(cx, |this, cx| {
                                if h {
                                    this.start_button_tooltip(
                                        "Open in Terminal".to_string(),
                                        None,
                                        pos,
                                        cx,
                                    );
                                } else {
                                    this.stop_button_tooltip(cx);
                                }
                            });
                        })
                        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                            v_term.update(cx, |this, cx| {
                                this.stop_button_tooltip(cx);
                                this.dispatch_action(
                                    crate::core::action_registry::ActionKind::OpenTerminal,
                                    cx,
                                );
                            });
                        })
                        .child(
                            div()
                                .text_size(px(12.5))
                                .font_family(UiConstants::FONT_ICONS)
                                .child("\u{E756}"),
                        ),
                )
                .child(Self::vertical_divider());
        }

        // 1. Folders count indicator (`\u{E8B7}` + count)
        {
            let v_folders_hover = view.clone();
            let folder_tip = if state.selected_count > 0 {
                format!(
                    "{} of {} folders selected",
                    selected_folders, state.folder_count
                )
            } else {
                format!("{} folders", state.folder_count)
            };
            right_row = right_row.child(
                div()
                    .id("status-folders-count")
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(5.0))
                    .px(px(6.0))
                    .py(px(3.0))
                    .rounded(px(4.0))
                    .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
                    .on_hover(move |hovered, win, cx| {
                        let h = *hovered;
                        let tip = folder_tip.clone();
                        let pos = win.mouse_position();
                        let v = v_folders_hover.clone();
                        v.update(cx, |this, cx| {
                            if h {
                                this.start_button_tooltip(tip, None, pos, cx);
                            } else {
                                this.stop_button_tooltip(cx);
                            }
                        });
                    })
                    .child(
                        div()
                            .text_size(px(12.5))
                            .font_family(UiConstants::FONT_ICONS)
                            .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                            .child("\u{E8B7}"),
                    )
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                            .child(folder_count_label),
                    ),
            );
        }

        // 2. Files count indicator (`\u{E7C3}` + count)
        {
            let v_files_hover = view.clone();
            let file_tip = if state.selected_count > 0 {
                format!("{} of {} files selected", selected_files, state.file_count)
            } else {
                format!("{} files", state.file_count)
            };
            right_row = right_row.child(
                div()
                    .id("status-files-count")
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(5.0))
                    .px(px(6.0))
                    .py(px(3.0))
                    .rounded(px(4.0))
                    .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
                    .on_hover(move |hovered, win, cx| {
                        let h = *hovered;
                        let tip = file_tip.clone();
                        let pos = win.mouse_position();
                        let v = v_files_hover.clone();
                        v.update(cx, |this, cx| {
                            if h {
                                this.start_button_tooltip(tip, None, pos, cx);
                            } else {
                                this.stop_button_tooltip(cx);
                            }
                        });
                    })
                    .child(
                        div()
                            .text_size(px(12.5))
                            .font_family(UiConstants::FONT_ICONS)
                            .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                            .child("\u{E7C3}"),
                    )
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                            .child(file_count_label),
                    ),
            );
        }

        // 3. 3-state Folder/File Sort Placement Toggle (Folders first -> Files first -> Interleaved)
        {
            let v_sort_click = view.clone();
            let v_sort_hover = view.clone();
            let tip_str = sort_prio_tooltip.to_string();
            right_row = right_row.child(
                div()
                    .id("status-sort-priority-toggle")
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_center()
                    .gap(px(2.0))
                    .px(px(6.0))
                    .py(px(4.0))
                    .rounded(px(4.0))
                    .cursor_pointer()
                    .bg(if sort_prio_active_accent {
                        rgba(0x0078D422u32)
                    } else {
                        rgba(0x00000000u32)
                    })
                    .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
                    .on_hover(move |hovered, win, cx| {
                        let h = *hovered;
                        let tip = tip_str.clone();
                        let pos = win.mouse_position();
                        let v = v_sort_hover.clone();
                        v.update(cx, |this, cx| {
                            if h {
                                this.start_button_tooltip(tip, None, pos, cx);
                            } else {
                                this.stop_button_tooltip(cx);
                            }
                        });
                    })
                    .on_mouse_down(MouseButton::Left, move |event, _win, cx| {
                        let pos = event.position;
                        v_sort_click.update(cx, |this, cx| {
                            this.cycle_sort_priority(cx);
                            let new_tip = if this.state.main_view.sort.folders_first {
                                "Sort: Folders first (click for Files first)"
                            } else if this.state.main_view.sort.files_first {
                                "Sort: Files first (click for Interleaved)"
                            } else {
                                "Sort: Interleaved folders & files (click for Folders first)"
                            };
                            this.button_tooltip = Some(crate::app_state::ButtonTooltipData {
                                text: new_tip.to_string(),
                                shortcut: None,
                                position: pos,
                            });
                            cx.notify();
                        });
                    })
                    .child(
                        div()
                            .text_size(px(12.5))
                            .font_family(UiConstants::FONT_ICONS)
                            .text_color(if sort_prio_active_accent {
                                rgb(0x60CDFF)
                            } else {
                                rgba(UiConstants::TEXT_SECONDARY_RGBA).into()
                            })
                            .child(sort_prio_icon),
                    )
                    .children(sort_prio_badge.map(|badge| {
                        div()
                            .text_size(px(8.5))
                            .font_family(UiConstants::FONT_ICONS)
                            .text_color(rgb(0x60CDFF))
                            .child(badge)
                    })),
            );
        }

        // 4. View Mode Switcher, Lock Folder Option & Scrollable Zoom Controls (File Pilot + WinUI 3 Fluent style)
        {
            let is_locked = pv.view_lock_enabled;
            let v_lock = view.clone();
            let v_lock_hover = view.clone();
            let v_mode_click = view.clone();
            let v_mode_hover = view.clone();
            let v_mode_wheel = view.clone();
            let v_pct_click = view.clone();
            let v_pct_hover = view.clone();
            let v_pct_wheel = view;

            let zoom_clamped = zoom_level.clamp(0.0, 1.0);
            let btn_w = 56.0;
            let fill_w = (btn_w * zoom_clamped).round();

            right_row = right_row
                .child(Self::vertical_divider())
                .child(
                    div()
                        .id("status-lock-folder-view")
                        .flex()
                        .items_center()
                        .justify_center()
                        .px(px(6.0))
                        .py(px(4.0))
                        .rounded(px(4.0))
                        .cursor_pointer()
                        .bg(if is_locked {
                            rgba(0x0078D422u32)
                        } else {
                            rgba(0x00000000u32)
                        })
                        .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
                        .on_hover(move |hovered, win, cx| {
                            let h = *hovered;
                            let pos = win.mouse_position();
                            let v = v_lock_hover.clone();
                            v.update(cx, |this, cx| {
                                if h {
                                    let tip = if is_locked {
                                        "View locked for this folder (click to unlock)".to_string()
                                    } else {
                                        "Lock view and zoom for this folder".to_string()
                                    };
                                    this.start_button_tooltip(tip, None, pos, cx);
                                } else {
                                    this.stop_button_tooltip(cx);
                                }
                            });
                        })
                        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                            v_lock.update(cx, |this, cx| {
                                this.toggle_view_lock(cx);
                            });
                        })
                        .child(
                            div()
                                .text_size(px(12.0))
                                .font_family(UiConstants::FONT_ICONS)
                                .text_color(if is_locked {
                                    rgb(0x60CDFF)
                                } else {
                                    rgba(UiConstants::TEXT_SECONDARY_RGBA).into()
                                })
                                .child(if is_locked { "\u{E72E}" } else { "\u{E785}" }),
                        ),
                )
                .child(
                    div()
                        .id("status-view-mode")
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(6.0))
                        .px(px(7.0))
                        .py(px(3.0))
                        .rounded(px(4.0))
                        .cursor_pointer()
                        .bg(if is_zoom_menu_open {
                            rgba(UiConstants::BG_ITEM_HOVER_RGBA)
                        } else {
                            rgba(0x00000000u32)
                        })
                        .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
                        .on_hover(move |hovered, win, cx| {
                            let h = *hovered;
                            let pos = win.mouse_position();
                            let v = v_mode_hover.clone();
                            v.update(cx, |this, cx| {
                                if h {
                                    this.start_button_tooltip(
                                        "View layout (scroll on hover to zoom)".to_string(),
                                        None,
                                        pos,
                                        cx,
                                    );
                                } else {
                                    this.stop_button_tooltip(cx);
                                }
                            });
                        })
                        .on_scroll_wheel(move |event, _win, cx| {
                            cx.stop_propagation();
                            let delta = event.delta.pixel_delta(px(22.0));
                            let delta_y: f32 = delta.y.into();
                            if delta_y.abs() < 0.25 {
                                return;
                            }
                            let step = if delta_y > 0.0 { 0.05 } else { -0.05 };
                            v_mode_wheel.update(cx, |this, cx| {
                                let next =
                                    (this.state.main_view.zoom_level + step).clamp(0.0, 1.0);
                                this.set_zoom_level(next, cx);
                            });
                        })
                        .on_mouse_down(MouseButton::Left, move |event, _win, cx| {
                            v_mode_click.update(cx, |this, cx| {
                                this.stop_button_tooltip(cx);
                                let mut pos = event.position;
                                pos.y -= px(320.0);
                                pos.x -= px(260.0);
                                this.open_zoom_slider(pos, cx);
                            });
                        })
                        .child(
                            div()
                                .text_size(px(12.5))
                                .font_family(UiConstants::FONT_ICONS)
                                .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                .child(view_icon),
                        )
                        .child(
                            div()
                                .text_size(px(12.0))
                                .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                                .child(view_label),
                        ),
                )
                .child(
                    div()
                        .id("status-zoom-controls")
                        .relative()
                        .w(px(btn_w))
                        .h(px(22.0))
                        .rounded(px(4.0))
                        .overflow_hidden()
                        .border(px(1.0))
                        .border_color(rgba(UiConstants::SURFACE_STROKE_FLYOUT_RGBA))
                        .bg(rgba(0xFFFFFF0Cu32))
                        .cursor_pointer()
                        .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
                        .on_hover(move |hovered, win, cx| {
                            let h = *hovered;
                            let pos = win.mouse_position();
                            let v = v_pct_hover.clone();
                            v.update(cx, |this, cx| {
                                if h {
                                    this.start_button_tooltip(
                                        "Zoom level (scroll to adjust, double-click to reset)"
                                            .to_string(),
                                        None,
                                        pos,
                                        cx,
                                    );
                                } else {
                                    this.stop_button_tooltip(cx);
                                }
                            });
                        })
                        .on_scroll_wheel(move |event, _win, cx| {
                            cx.stop_propagation();
                            let delta = event.delta.pixel_delta(px(22.0));
                            let delta_y: f32 = delta.y.into();
                            if delta_y.abs() < 0.25 {
                                return;
                            }
                            let step = if delta_y > 0.0 { 0.05 } else { -0.05 };
                            v_pct_wheel.update(cx, |this, cx| {
                                let next =
                                    (this.state.main_view.zoom_level + step).clamp(0.0, 1.0);
                                this.set_zoom_level(next, cx);
                            });
                        })
                        .on_mouse_down(MouseButton::Left, move |event, _win, cx| {
                            v_pct_click.update(cx, |this, cx| {
                                this.stop_button_tooltip(cx);
                                if event.click_count == 2 {
                                    this.reset_zoom(cx);
                                } else {
                                    let mut pos = event.position;
                                    pos.y -= px(320.0);
                                    pos.x -= px(260.0);
                                    this.open_zoom_slider(pos, cx);
                                }
                            });
                        })
                        .child(
                            // Progress bar filling from left to right signifying the percentage
                            div()
                                .absolute()
                                .top_0()
                                .bottom_0()
                                .left_0()
                                .w(px(fill_w))
                                .bg(rgba(0x0078D433u32)),
                        )
                        .child(
                            // Solid accent active underline indicator filling from left to right
                            div()
                                .absolute()
                                .bottom_0()
                                .left_0()
                                .h(px(2.0))
                                .w(px(fill_w))
                                .bg(rgb(0x0078D4)),
                        )
                        .child(
                            // Centered percentage label
                            div()
                                .relative()
                                .size_full()
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_size(px(11.5))
                                .font_weight(FontWeight(500.0))
                                .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                                .child(format!("{}%", zoom_pct)),
                        ),
                );
        }

        div()
            .id("status-bar")
            .flex()
            .items_center()
            .justify_between()
            .h(px(UiConstants::STATUS_BAR_HEIGHT_RAW as f32))
            .w_full()
            .px(px(12.0))
            .bg(rgba(UiConstants::BG_STATUS_BAR_RGBA))
            .border_t(px(1.0))
            .border_color(rgba(UiConstants::DIVIDER_RGBA))
            .child(left_row)
            .child(right_row)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;

    #[test]
    fn test_status_bar_format_size() {
        assert_eq!(format_size(512), "512 B");
        assert_eq!(format_size(2048), "2.0 KB");
        assert_eq!(format_size(5 * 1024 * 1024), "5.0 MB");
    }

    #[test]
    fn test_status_bar_state_sync() {
        let mut st = StatusBarState::new();
        st.selected_count = 5;
        st.sync(120, 20, 100);
        assert_eq!(st.total_count, 120);
        assert_eq!(st.folder_count, 20);
        assert_eq!(st.file_count, 100);
        assert_eq!(st.selected_count, 0);
    }
}

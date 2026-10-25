//! Bottom Status Bar — FilePilot + Files-community Hybrid Telemetry & Control Bar
//!
//! Layout (`32px` height, unibody acrylic/mica surface):
//! - **LEFT**: Folder & file counts, live drive free-space mini-meter, selection count/size badge,
//!   and FilePilot-style quick selection actions (`All`, `Inv`, `Clear`).
//! - **CENTER**: Git branch pill, interactive `Sort: <Col> ▲/▼` pill, interactive `Group: <Col>` pill,
//!   and quick `Filter` bar toggle.
//! - **RIGHT**: 5-mode segmented View Switcher (`Details`, `List`, `Cards`, `Grid`, `Columns`),
//!   inline `-` / 8-segment clickable Zoom Bar / `+` + `%` badge, `View Lock`, `Shelf`, and `Preview` toggles.

use crate::app::AppState;
use crate::config::config::{GroupColumn, SortBy, ViewMode};
use crate::ui::constants::UiConstants;
use crate::ui::views::main_view::SidePaneMode;
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
    pub fn build(
        state: &StatusBarState,
        app: &AppState,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let active_pane = app.active_pane_focus();
        let pv = app.pane_view(active_pane);
        let view_mode = pv.view_mode.clone();
        let zoom_level = pv.zoom_level.clamp(0.0, 1.0);
        let zoom_pct = (zoom_level * 100.0).round() as u32;
        let sort_col = pv.sort.column.clone();
        let sort_asc = pv.sort.ascending;
        let group_by = pv.group_by;
        let filter_active = pv.show_filter_bar || !pv.filter_text.is_empty();
        let shelf_open = app.main_view.shelf_visible;
        let preview_open = app.main_view.side_pane != SidePaneMode::None;

        let sort_col_label = match sort_col {
            SortBy::Name => "Name",
            SortBy::DateModified => "Modified",
            SortBy::DateCreated => "Created",
            SortBy::Size => "Size",
            SortBy::Type => "Type",
            SortBy::Tag => "Tag",
        };
        let sort_dir_arrow = if sort_asc { "▲" } else { "▼" };

        let single_sel_name: Option<String> = if state.selected_count == 1 {
            pv.selected_indices
                .iter()
                .next()
                .and_then(|&i| pv.items.get(i))
                .map(|it| crate::ui::views::grid_view::shorten_filename_middle(&it.name, 24))
        } else {
            None
        };

        let sel_size_label = if state.selected_size_bytes > 0 {
            Some(format_size(state.selected_size_bytes))
        } else if state.folder_size_pending {
            let dots = ".".repeat((state.spinner_frame % 4) as usize);
            Some(format!("calc{}", dots))
        } else {
            None
        };

        // Drive capacity telemetry
        let drive_meter = if state.total_bytes > 0 {
            let used = state.total_bytes.saturating_sub(state.free_bytes);
            let ratio = (used as f64 / state.total_bytes as f64).clamp(0.0, 1.0) as f32;
            let bar_color = if ratio > 0.90 {
                0xFF99A4u32
            } else if ratio > 0.75 {
                0xFCE100u32
            } else {
                0x60CDFFu32
            };
            Some(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .px(px(7.0))
                    .py(px(2.0))
                    .rounded(px(4.0))
                    .bg(rgba(0xFFFFFF07u32))
                    .child(
                        div()
                            .w(px(42.0))
                            .h(px(5.0))
                            .rounded(px(2.5))
                            .bg(rgba(0xFFFFFF18u32))
                            .overflow_hidden()
                            .child(
                                div()
                                    .w(px((42.0 * ratio).max(3.0)))
                                    .h_full()
                                    .rounded(px(2.5))
                                    .bg(rgb(bar_color)),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                            .child(format!("{} free", format_size(state.free_bytes))),
                    ),
            )
        } else {
            None
        };

        div()
            .id("status-bar")
            .flex()
            .items_center()
            .justify_between()
            .h(px(UiConstants::STATUS_BAR_HEIGHT_RAW as f32))
            .w_full()
            .px(px(10.0))
            .bg(rgba(UiConstants::BG_STATUS_BAR_RGBA))
            .border_t(px(1.0))
            .border_color(rgba(UiConstants::DIVIDER_RGBA))
            // ── LEFT CLUSTER: Counts, Drive Space Meter & Selection Quick-Actions ──
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .overflow_hidden()
                    // Folder & File counts pill
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(5.0))
                            .px(px(6.0))
                            .py(px(2.0))
                            .rounded(px(4.0))
                            .bg(rgba(0xFFFFFF06u32))
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .font_family(UiConstants::FONT_MDL2)
                                    .text_color(rgb(0xF5B942))
                                    .child("\u{E8B7}"),
                            )
                            .child(
                                div()
                                    .text_size(px(11.5))
                                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                    .child(format!("{}", state.folder_count)),
                            )
                            .child(
                                div()
                                    .text_size(px(10.0))
                                    .text_color(rgba(UiConstants::TEXT_MUTED_RGBA))
                                    .child("•"),
                            )
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .font_family(UiConstants::FONT_MDL2)
                                    .text_color(rgb(0x60CDFF))
                                    .child("\u{E8A5}"),
                            )
                            .child(
                                div()
                                    .text_size(px(11.5))
                                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                    .child(format!("{}", state.file_count)),
                            ),
                    )
                    .children(drive_meter)
                    // Selection pill + Quick-action buttons (`All`, `Inv`, `Clear`)
                    .child(if state.selected_count > 0 {
                        let v_all = view.clone();
                        let v_inv = view.clone();
                        let v_clr = view.clone();
                        let mut sel_text = format!("{} selected", state.selected_count);
                        if let Some(ref sz) = sel_size_label {
                            sel_text.push_str(&format!(" ({})", sz));
                        }
                        if let Some(ref name) = single_sel_name {
                            sel_text.push_str(&format!(" · {}", name));
                        }
                        div()
                            .flex()
                            .items_center()
                            .gap(px(3.0))
                            .child(
                                div()
                                    .px(px(7.0))
                                    .py(px(2.0))
                                    .rounded(px(4.0))
                                    .bg(rgba(0x0078D433u32))
                                    .border(px(1.0))
                                    .border_color(rgba(0x60CDFF55u32))
                                    .text_size(px(11.5))
                                    .font_weight(FontWeight(600.0))
                                    .text_color(rgb(0x60CDFF))
                                    .child(sel_text),
                            )
                            .child(
                                div()
                                    .id("status-sel-all")
                                    .px(px(5.0))
                                    .py(px(2.0))
                                    .rounded(px(3.0))
                                    .cursor_pointer()
                                    .bg(rgba(0xFFFFFF08u32))
                                    .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
                                    .text_size(px(10.5))
                                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                        v_all.update(cx, |this, cx| {
                                            this.select_all(cx);
                                        });
                                    })
                                    .child("All"),
                            )
                            .child(
                                div()
                                    .id("status-sel-inv")
                                    .px(px(5.0))
                                    .py(px(2.0))
                                    .rounded(px(3.0))
                                    .cursor_pointer()
                                    .bg(rgba(0xFFFFFF08u32))
                                    .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
                                    .text_size(px(10.5))
                                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                        v_inv.update(cx, |this, cx| {
                                            this.invert_selection(cx);
                                        });
                                    })
                                    .child("Inv"),
                            )
                            .child(
                                div()
                                    .id("status-sel-clear")
                                    .px(px(5.0))
                                    .py(px(2.0))
                                    .rounded(px(3.0))
                                    .cursor_pointer()
                                    .bg(rgba(0xFFFFFF08u32))
                                    .hover(|s| s.bg(rgba(0xFF99A428u32)))
                                    .text_size(px(10.5))
                                    .text_color(rgb(0xFF99A4))
                                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                        v_clr.update(cx, |this, cx| {
                                            this.clear_selection(cx);
                                        });
                                    })
                                    .child("Clear"),
                            )
                            .into_any_element()
                    } else {
                        let v_all = view.clone();
                        div()
                            .id("status-sel-all-idle")
                            .px(px(6.0))
                            .py(px(2.0))
                            .rounded(px(4.0))
                            .cursor_pointer()
                            .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
                            .text_size(px(11.0))
                            .text_color(rgba(UiConstants::TEXT_MUTED_RGBA))
                            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                v_all.update(cx, |this, cx| {
                                    this.select_all(cx);
                                });
                            })
                            .child("Select All")
                            .into_any_element()
                    }),
            )
            // ── CENTER CLUSTER: Git Branch + Interactive Sort / Group / Filter Pills ──
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(5.0))
                    // Git branch badge
                    .children(if state.show_git_actions {
                        state.git_branch.as_ref().map(|branch| {
                            div()
                                .id("status-git-branch")
                                .flex()
                                .items_center()
                                .gap(px(5.0))
                                .rounded(px(4.0))
                                .px(px(7.0))
                                .py(px(2.0))
                                .bg(rgba(UiConstants::BG_BADGE_RGBA))
                                .child(
                                    div()
                                        .text_size(px(11.0))
                                        .font_family(UiConstants::FONT_MDL2)
                                        .text_color(rgb(UiConstants::ACCENT_COLOR_RGB))
                                        .child("\u{E8AD}"),
                                )
                                .child(
                                    div()
                                        .text_size(px(11.0))
                                        .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                        .child(branch.clone()),
                                )
                        })
                    } else {
                        None
                    })
                    // Interactive Sort Pill (Left-click: toggle direction, Right-click: cycle column)
                    .child({
                        let v_sort_l = view.clone();
                        let v_sort_r = view.clone();
                        let cur_sort = sort_col.clone();
                        div()
                            .id("status-sort-pill")
                            .flex()
                            .items_center()
                            .gap(px(4.0))
                            .px(px(7.0))
                            .py(px(2.0))
                            .rounded(px(4.0))
                            .cursor_pointer()
                            .bg(rgba(0xFFFFFF07u32))
                            .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
                            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                v_sort_l.update(cx, |this, cx| {
                                    let asc = !this
                                        .state
                                        .pane_view(this.state.active_pane_focus())
                                        .sort
                                        .ascending;
                                    this.set_sort_ascending(asc, cx);
                                });
                            })
                            .on_mouse_down(MouseButton::Right, move |_, _, cx| {
                                let next_col = match cur_sort {
                                    SortBy::Name => SortBy::DateModified,
                                    SortBy::DateModified => SortBy::Size,
                                    SortBy::Size => SortBy::Type,
                                    SortBy::Type => SortBy::DateCreated,
                                    SortBy::DateCreated => SortBy::Tag,
                                    SortBy::Tag => SortBy::Name,
                                };
                                v_sort_r.update(cx, |this, cx| {
                                    this.set_sort_column(next_col, cx);
                                });
                            })
                            .child(
                                div()
                                    .text_size(px(10.5))
                                    .font_family(UiConstants::FONT_MDL2)
                                    .text_color(rgb(0x60CDFF))
                                    .child("\u{E8CB}"),
                            )
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                    .child(format!("{} {}", sort_col_label, sort_dir_arrow)),
                            )
                    })
                    // Interactive Group Pill (Left-click: cycle group-by, Right-click: clear group-by)
                    .child({
                        let v_grp_l = view.clone();
                        let v_grp_r = view.clone();
                        let is_grouped = group_by != GroupColumn::None;
                        div()
                            .id("status-group-pill")
                            .flex()
                            .items_center()
                            .gap(px(4.0))
                            .px(px(7.0))
                            .py(px(2.0))
                            .rounded(px(4.0))
                            .cursor_pointer()
                            .bg(if is_grouped {
                                rgba(0x0078D430u32)
                            } else {
                                rgba(0xFFFFFF07u32)
                            })
                            .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
                            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                v_grp_l.update(cx, |this, cx| {
                                    this.cycle_group_by(cx);
                                });
                            })
                            .on_mouse_down(MouseButton::Right, move |_, _, cx| {
                                v_grp_r.update(cx, |this, cx| {
                                    this.set_group_by(GroupColumn::None, cx);
                                });
                            })
                            .child(
                                div()
                                    .text_size(px(10.5))
                                    .font_family(UiConstants::FONT_MDL2)
                                    .text_color(if is_grouped {
                                        rgb(0x60CDFF)
                                    } else {
                                        rgba(UiConstants::TEXT_SECONDARY_RGBA)
                                    })
                                    .child("\u{F168}"),
                            )
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .text_color(if is_grouped {
                                        rgb(0x60CDFF)
                                    } else {
                                        rgba(UiConstants::TEXT_SECONDARY_RGBA)
                                    })
                                    .child(if is_grouped {
                                        format!("Group: {}", group_by.display_name())
                                    } else {
                                        "Group".to_string()
                                    }),
                            )
                    })
                    // Filter & Group Bar Toggle Pill
                    .child({
                        let v_flt = view.clone();
                        div()
                            .id("status-filter-pill")
                            .flex()
                            .items_center()
                            .gap(px(4.0))
                            .px(px(6.0))
                            .py(px(2.0))
                            .rounded(px(4.0))
                            .cursor_pointer()
                            .bg(if filter_active {
                                rgba(0x0078D430u32)
                            } else {
                                rgba(0xFFFFFF07u32)
                            })
                            .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
                            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                v_flt.update(cx, |this, cx| {
                                    let p = this.state.active_pane_focus();
                                    let pv = this.state.pane_view_mut(p);
                                    pv.show_filter_bar = !pv.show_filter_bar;
                                    cx.notify();
                                });
                            })
                            .child(
                                div()
                                    .text_size(px(10.5))
                                    .font_family(UiConstants::FONT_MDL2)
                                    .text_color(if filter_active {
                                        rgb(0x60CDFF)
                                    } else {
                                        rgba(UiConstants::TEXT_SECONDARY_RGBA)
                                    })
                                    .child("\u{E71C}"),
                            )
                    }),
            )
            // ── RIGHT CLUSTER: 5-Mode View Switcher + Inline Zoom Bar + Pane Toggles ──
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(5.0))
                    // 5-Mode Segmented View Switcher Strip (`Details`, `List`, `Cards`, `Grid`, `Columns`)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(1.0))
                            .p(px(2.0))
                            .rounded(px(5.0))
                            .bg(rgba(0x00000028u32))
                            .border(px(1.0))
                            .border_color(rgba(0xFFFFFF0Cu32))
                            .children(
                                [
                                    (ViewMode::Details, "\u{E8FD}", "Details"),
                                    (ViewMode::List, "\u{E8EF}", "List"),
                                    (ViewMode::Cards, "\u{E8A9}", "Cards"),
                                    (ViewMode::Grid, "\u{F0E2}", "Grid"),
                                    (ViewMode::Columns, "\u{E8E4}", "Columns"),
                                ]
                                .into_iter()
                                .map(|(mode_opt, glyph, _tip)| {
                                    let active = view_mode == mode_opt;
                                    let v_m = view.clone();
                                    let m_target = mode_opt.clone();
                                    div()
                                        .w(px(24.0))
                                        .h(px(20.0))
                                        .rounded(px(3.5))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .cursor_pointer()
                                        .bg(if active {
                                            rgba(UiConstants::ACCENT_RGBA)
                                        } else {
                                            rgba(0x00000000u32)
                                        })
                                        .hover(|s| {
                                            if active {
                                                s
                                            } else {
                                                s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA))
                                            }
                                        })
                                        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                            let m = m_target.clone();
                                            v_m.update(cx, |this, cx| {
                                                this.set_view_mode(m, cx);
                                            });
                                        })
                                        .child(
                                            div()
                                                .text_size(px(11.5))
                                                .font_family(UiConstants::FONT_MDL2)
                                                .text_color(if active {
                                                    rgb(0xFFFFFF)
                                                } else {
                                                    rgba(UiConstants::TEXT_SECONDARY_RGBA)
                                                })
                                                .child(glyph),
                                        )
                                }),
                            ),
                    )
                    // Inline `-` / 8-Segment Interactive Zoom Track / `+` + `%` Badge
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(3.0))
                            .px(px(5.0))
                            .py(px(2.0))
                            .rounded(px(5.0))
                            .bg(rgba(0xFFFFFF06u32))
                            // Zoom Out `-`
                            .child({
                                let v_out = view.clone();
                                div()
                                    .id("status-zoom-out")
                                    .w(px(18.0))
                                    .h(px(18.0))
                                    .rounded(px(3.0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .cursor_pointer()
                                    .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
                                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                        v_out.update(cx, |this, cx| {
                                            this.zoom_out(cx);
                                        });
                                    })
                                    .child(
                                        div()
                                            .text_size(px(10.0))
                                            .font_family(UiConstants::FONT_MDL2)
                                            .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                            .child("\u{E738}"),
                                    )
                            })
                            // 8-Segment Clickable Zoom Bar
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(2.0))
                                    .children((0..8).map(|seg_idx| {
                                        let seg_val = seg_idx as f32 / 7.0;
                                        let filled = zoom_level + 0.06 >= seg_val;
                                        let v_seg = view.clone();
                                        div()
                                            .w(px(6.0))
                                            .h(px(10.0))
                                            .rounded(px(1.5))
                                            .cursor_pointer()
                                            .bg(if filled {
                                                rgb(0x60CDFF)
                                            } else {
                                                rgba(0xFFFFFF1Eu32)
                                            })
                                            .hover(|s| s.bg(rgb(0x98E0FF)))
                                            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                                v_seg.update(cx, |this, cx| {
                                                    let p = this.state.active_pane_focus();
                                                    this.state.pane_view_mut(p).zoom_level = seg_val;
                                                    if p == crate::core::navigation_manager::PaneFocus::Primary {
                                                        this.state.main_view.zoom_level = seg_val;
                                                    }
                                                    cx.notify();
                                                });
                                            })
                                    })),
                            )
                            // Zoom In `+`
                            .child({
                                let v_in = view.clone();
                                div()
                                    .id("status-zoom-in")
                                    .w(px(18.0))
                                    .h(px(18.0))
                                    .rounded(px(3.0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .cursor_pointer()
                                    .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
                                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                        v_in.update(cx, |this, cx| {
                                            this.zoom_in(cx);
                                        });
                                    })
                                    .child(
                                        div()
                                            .text_size(px(10.0))
                                            .font_family(UiConstants::FONT_MDL2)
                                            .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                            .child("\u{E710}"),
                                    )
                            })
                            // Zoom `%` Popover / Reset Button
                            .child({
                                let v_pct = view.clone();
                                div()
                                    .id("status-zoom-controls")
                                    .min_w(px(38.0))
                                    .px(px(5.0))
                                    .py(px(1.5))
                                    .rounded(px(3.5))
                                    .cursor_pointer()
                                    .bg(rgba(UiConstants::BG_BADGE_RGBA))
                                    .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .on_mouse_down(MouseButton::Left, move |event, _win, cx| {
                                        v_pct.update(cx, |this, cx| {
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
                                        div()
                                            .text_size(px(11.0))
                                            .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                            .child(format!("{}%", zoom_pct)),
                                    )
                            }),
                    )
                    // View Lock + Shelf + Preview Toggles
                    .child({
                        let v_lock = view.clone();
                        div()
                            .id("status-view-lock")
                            .w(px(24.0))
                            .h(px(22.0))
                            .rounded(px(4.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .bg(if app.main_view.view_lock_enabled {
                                rgba(0x005FB838u32)
                            } else {
                                rgba(0x00000000u32)
                            })
                            .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
                            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                v_lock.update(cx, |this, cx| {
                                    this.toggle_view_lock(cx);
                                });
                            })
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .font_family(UiConstants::FONT_MDL2)
                                    .text_color(if app.main_view.view_lock_enabled {
                                        rgb(0x60CDFF)
                                    } else {
                                        rgba(UiConstants::TEXT_SECONDARY_RGBA)
                                    })
                                    .child(if app.main_view.view_lock_enabled {
                                        "\u{E72E}"
                                    } else {
                                        "\u{E785}"
                                    }),
                            )
                    })
                    .child({
                        let v_shelf = view.clone();
                        div()
                            .id("status-shelf-toggle")
                            .w(px(24.0))
                            .h(px(22.0))
                            .rounded(px(4.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .bg(if shelf_open {
                                rgba(0x005FB838u32)
                            } else {
                                rgba(0x00000000u32)
                            })
                            .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
                            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                v_shelf.update(cx, |this, cx| {
                                    this.toggle_shelf(cx);
                                });
                            })
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .font_family(UiConstants::FONT_MDL2)
                                    .text_color(if shelf_open {
                                        rgb(0x60CDFF)
                                    } else {
                                        rgba(UiConstants::TEXT_SECONDARY_RGBA)
                                    })
                                    .child("\u{E7B8}"),
                            )
                    })
                    .child({
                        let v_prev = view;
                        div()
                            .id("status-preview-toggle")
                            .w(px(24.0))
                            .h(px(22.0))
                            .rounded(px(4.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .bg(if preview_open {
                                rgba(0x005FB838u32)
                            } else {
                                rgba(0x00000000u32)
                            })
                            .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
                            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                v_prev.update(cx, |this, cx| {
                                    this.toggle_preview_pane(cx);
                                });
                            })
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .font_family(UiConstants::FONT_MDL2)
                                    .text_color(if preview_open {
                                        rgb(0x60CDFF)
                                    } else {
                                        rgba(UiConstants::TEXT_SECONDARY_RGBA)
                                    })
                                    .child("\u{E89F}"),
                            )
                    }),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

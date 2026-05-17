//! Status Bar component — per DESIGN_SPEC.md section 9.
//! Displays: item count, selection info, free drive space.
//! Layout: 3-column grid [LEFT: info] [CENTER: git actions] [RIGHT: branch].

use crate::app::AppState;
use crate::ui::constants::UiConstants;
use gpui::*;

/// Status bar state — tracks what to display.
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
    /// Total size of selected files (in bytes) — only for files, not folders
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
    }
}

/// Format bytes as human-readable string (KB, MB, GB).
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
    /// Build the status bar per DESIGN_SPEC.md 9.
    /// Height: 32px, padding: 8px left.
    /// 3-column grid: [LEFT: info] [CENTER: git actions] [RIGHT: branch]
    pub fn build(state: &StatusBarState, _app: &AppState, view: Entity<crate::app::AppWindow>) -> impl IntoElement {
        use crate::ui::views::main_view::{zoom_mode_icon, zoom_mode_name};
        let view_mode = &_app.main_view.view_mode;
        let mode_name = zoom_mode_name(view_mode);
        let mode_icon = zoom_mode_icon(view_mode);
        let zoom_pct = (_app.main_view.zoom_level * 100.0) as u32;
        let v = view.clone();

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
            .child(
                // LEFT: folder count + file count
                div()
                    .flex()
                    .items_center()
                    .gap(px(16.0))
                    // Folder count
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.0))
                            .child(
                                div()
                                    .text_size(px(14.0))
                                    .font_family(UiConstants::FONT_MDL2)
                                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                    .child("\u{E8B7}"),
                            )
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                    .child(state.folder_count.to_string()),
                            )
                    )
                    // File count
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.0))
                            .child(
                                div()
                                    .text_size(px(14.0))
                                    .font_family(UiConstants::FONT_MDL2)
                                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                    .child("\u{E8A5}"),
                            )
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                    .child(state.file_count.to_string()),
                            )
                    )
                    .children(if state.selected_count > 0 {
                        Some(div()
                            .flex()
                            .items_center()
                            .gap(px(12.0))
                            .child(div().w(px(1.0)).h(px(14.0)).bg(rgba(UiConstants::DIVIDER_RGBA)))
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                                    .child(format!(
                                        "{} selected",
                                        state.selected_count
                                    ))
                            )
                        )
                    } else {
                        None
                    })
            )
            .child(
                // RIGHT: zoom mode button + zoom percentage
                div()
                    .flex()
                    .items_center()
                    .gap(px(4.0))
                    // Lock icon for per-directory view/zoom memory
                    .child(
                        div()
                            .id("status-view-lock")
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .rounded(px(4.0))
                            .px(px(6.0))
                            .py(px(4.0))
                            .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
                            .bg(if _app.main_view.view_lock_enabled { rgba(0x005FB830u32) } else { rgba(0x00000000u32) })
                            .on_mouse_down(MouseButton::Left, {
                                let v = v.clone();
                                move |_, _, cx| {
                                    v.update(cx, |this, cx| {
                                        this.toggle_view_lock(cx);
                                    });
                                }
                            })
                            .child(
                                div()
                                    .text_size(px(14.0))
                                    .font_family(UiConstants::FONT_MDL2)
                                    .text_color(if _app.main_view.view_lock_enabled {
                                        rgb(UiConstants::ACCENT_COLOR_RGB)
                                    } else {
                                        rgba(UiConstants::TEXT_SECONDARY_RGBA)
                                    })
                                    .child(if _app.main_view.view_lock_enabled { "\u{E73E}" } else { "\u{E785}" }),
                            ),
                    )
                    // Separator before view options
                    .child(div().w(px(1.0)).h(px(16.0)).bg(rgba(UiConstants::DIVIDER_RGBA)).mx(px(8.0)))
                    // View Mode Button
                    .child(
                        div()
                            .id("status-zoom-controls")
                            .flex()
                            .items_center()
                            .gap(px(8.0))
                            .cursor_pointer()
                            .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
                            .rounded(px(4.0))
                            .px(px(8.0))
                            .py(px(4.0))
                            .on_mouse_down(MouseButton::Left, {
                                let v = v.clone();
                                move |event, _win, cx| {
                                    v.update(cx, |this, cx| {
                                        // Open dialog above the button
                                        let mut pos = event.position;
                                        pos.y -= px(320.0); // Offset upwards
                                        pos.x -= px(260.0); // Offset left to prevent overflow
                                        this.open_zoom_slider(pos, cx);
                                    });
                                }
                            })
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(8.0))
                                    .child(
                                        div()
                                            .text_size(px(16.0))
                                            .font_family(UiConstants::FONT_MDL2)
                                            .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                            .child(mode_icon),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(12.0))
                                            .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                            .child(mode_name),
                                    )
                            )
                            .child(
                                // Percentage badge with progress bar behind it
                                div()
                                    .min_w(px(46.0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .relative()
                                    .bg(rgba(UiConstants::BG_BADGE_RGBA))
                                    .rounded(px(4.0))
                                    .px(px(6.0))
                                    .py(px(2.0))
                                    .overflow_hidden()
                                    .child(
                                        // Progress bar fill (clamped so it never exceeds badge width visually)
                                        div()
                                            .absolute()
                                            .inset_0()
                                            .w(px((zoom_pct as f32 * 0.46).clamp(4.0, 46.0)))
                                            .h_full()
                                            .bg(rgba(0x005FB840u32)) // accent at 25% opacity
                                            .rounded(px(4.0)),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(12.0))
                                            .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                            .child(format!("{}%", zoom_pct)),
                                    ),
                            ),
                    )
            )
    }
}

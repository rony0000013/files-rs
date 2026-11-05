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
        let active_pane = app.active_pane_focus();
        let pv = app.pane_view(active_pane);
        let zoom_level = pv.zoom_level.clamp(0.0, 1.0);
        let zoom_pct = ((0.5 + zoom_level * 1.25) * 100.0).round() as u32;

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

        // ── RIGHT STACK (`StatusBar.xaml` Right Column: Git Sync | Git Branch | Open in IDE/Terminal | Zoom)
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
                            .px(px(8.0))
                            .py(px(4.0))
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
                            .px(px(8.0))
                            .py(px(4.0))
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

        // Open in Terminal / IDE button (`OpenInIdeButton` in `StatusBar.xaml`)
        {
            let v_term = view.clone();
            right_row = right_row.child(
                div()
                    .id("status-open-terminal")
                    .flex()
                    .items_center()
                    .justify_center()
                    .px(px(8.0))
                    .py(px(4.0))
                    .rounded(px(4.0))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        v_term.update(cx, |this, cx| {
                            this.dispatch_action(
                                crate::core::action_registry::ActionKind::OpenTerminal,
                                cx,
                            );
                        });
                    })
                    .child(
                        div()
                            .text_size(px(12.0))
                            .font_family(UiConstants::FONT_ICONS)
                            .text_color(rgb(UiConstants::ACCENT_COLOR_RGB))
                            .child("\u{E756}"),
                    ),
            );
        }

        // Subtle Zoom Percentage Button (Click: open zoom popover, Double-click: reset zoom)
        {
            let v_pct = view;
            right_row = right_row
                .child(Self::vertical_divider())
                .child(
                    div()
                        .id("status-zoom-controls")
                        .px(px(8.0))
                        .py(px(4.0))
                        .rounded(px(4.0))
                        .cursor_pointer()
                        .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
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
                                .text_size(px(12.0))
                                .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
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

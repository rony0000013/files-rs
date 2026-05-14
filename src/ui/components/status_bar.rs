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
    pub fn sync(&mut self, item_count: usize) {
        self.total_count = item_count;
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
    pub fn build(state: &StatusBarState, _app: &AppState) -> impl IntoElement {
        // Build LEFT section: item count + selection info
        let left_content = div()
            .flex()
            .items_center()
            .gap(px(UiConstants::PADDING_SMALL_RAW as f32))
            .child(
                div()
                    .text_size(AbsoluteLength::Pixels(px(
                        UiConstants::TEXT_SIZE_STATUS_BAR as f32,
                    )))
                    .font_family(UiConstants::FONT_MAIN)
                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                    .child(format!(
                        "{} item{}",
                        state.total_count,
                        if state.total_count == 1 { "" } else { "s" }
                    )),
            );

        // Selection info if items are selected
        let selection_info = if state.selected_count > 0 {
            div()
                .flex()
                .items_center()
                .gap(px(UiConstants::PADDING_SMALL_RAW as f32))
                .child(
                    div()
                        .w_px()
                        .h(px(18.0))
                        .mx(px(UiConstants::PADDING_SMALL_RAW as f32))
                        .bg(rgba(UiConstants::DIVIDER_RGBA)),
                )
                .child(
                    div()
                        .text_size(AbsoluteLength::Pixels(px(
                            UiConstants::TEXT_SIZE_STATUS_BAR as f32,
                        )))
                        .text_color(rgba(UiConstants::TEXT_SECONDARY_RGB))
                        .child(format!("{} selected", state.selected_count)),
                )
                .child(
                    div()
                        .text_size(AbsoluteLength::Pixels(px(
                            UiConstants::TEXT_SIZE_STATUS_BAR as f32,
                        )))
                        .text_color(rgba(UiConstants::TEXT_SECONDARY_RGB))
                        .child(format!(", {}", format_size(state.selected_size_bytes))),
                )
        } else {
            div()
        };

        // RIGHT section: drive space (if available)
        let right_content = if state.total_bytes > 0 {
            let free_str = format_size(state.free_bytes);
            let total_str = format_size(state.total_bytes);
            let usage_ratio = 1.0 - (state.free_bytes as f64 / state.total_bytes as f64);
            let bar_color = if usage_ratio > 0.9 {
                rgb(UiConstants::SYSTEM_FILL_COLOR_CRITICAL_RGB)
            } else {
                rgb(UiConstants::ACCENT_COLOR_RGB)
            };

            div()
                .flex()
                .items_center()
                .gap(px(UiConstants::PADDING_SMALL_RAW as f32))
                .child(
                    div()
                        .text_size(AbsoluteLength::Pixels(px(
                            UiConstants::TEXT_SIZE_STATUS_BAR as f32,
                        )))
                        .text_color(rgba(UiConstants::TEXT_SECONDARY_RGB))
                        .child(format!("{} free of {}", free_str, total_str)),
                )
                .child(
                    div()
                        .w(px(50.0))
                        .h(px(4.0))
                        .rounded(px(2.0))
                        .bg(rgba(UiConstants::DIVIDER_RGBA))
                        .child(
                            div()
                                .w(px((50.0 * usage_ratio.max(0.0).min(1.0)) as f32))
                                .h_px()
                                .rounded(px(2.0))
                                .bg(bar_color),
                        ),
                )
        } else {
            div()
        };

        // Git branch indicator (shown when in a repo)
        let git_indicator = if state.show_git_actions {
            div()
                .flex()
                .items_center()
                .gap(px(UiConstants::PADDING_SMALL_RAW as f32))
                .child(
                    div()
                        .w_px()
                        .h(px(18.0))
                        .mx(px(UiConstants::PADDING_SMALL_RAW as f32))
                        .bg(rgba(UiConstants::DIVIDER_RGBA)),
                )
                .child(
                    div()
                        .text_size(AbsoluteLength::Pixels(px(
                            UiConstants::TEXT_SIZE_STATUS_BAR as f32,
                        )))
                        .text_color(rgba(UiConstants::TEXT_SECONDARY_RGB))
                        .child(format!(
                            "\u{E8EC} {}",
                            state.git_branch.as_deref().unwrap_or("main")
                        )),
                )
        } else {
            div()
        };

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
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.0))
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                            .child(format!(
                                "{} item{}",
                                state.total_count,
                                if state.total_count == 1 { "" } else { "s" }
                            )),
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
                                        "{} selected  {}",
                                        state.selected_count,
                                        format_size(state.selected_size_bytes)
                                    ))
                            )
                        )
                    } else {
                        None
                    })
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(16.0))
                    .children(if state.show_git_actions {
                        Some(div()
                            .flex()
                            .items_center()
                            .gap(px(6.0))
                            .child(
                                div()
                                    .text_size(px(14.0))
                                    .font_family(UiConstants::FONT_ICONS)
                                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                    .child("\u{E8EC}")
                            )
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                    .child(state.git_branch.clone().unwrap_or_else(|| "main".to_string()))
                            )
                        )
                    } else {
                        None
                    })
                    .children(if state.total_bytes > 0 {
                        let usage_ratio = 1.0 - (state.free_bytes as f64 / state.total_bytes as f64);
                        Some(div()
                            .flex()
                            .items_center()
                            .gap(px(12.0))
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                    .child(format!("{} free of {}", format_size(state.free_bytes), format_size(state.total_bytes)))
                            )
                            .child(
                                div()
                                    .w(px(60.0))
                                    .h(px(4.0))
                                    .rounded(px(2.0))
                                    .bg(rgba(UiConstants::BG_BADGE_RGBA))
                                    .child(
                                        div()
                                            .h_full()
                                            .w(px((60.0 * usage_ratio) as f32))
                                            .rounded(px(2.0))
                                            .bg(rgba(if usage_ratio > 0.9 { UiConstants::DANGER_RGBA } else { UiConstants::ACCENT_RGBA }))
                                    )
                            )
                        )
                    } else {
                        None
                    })
            )
    }
}

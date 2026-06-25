//! Home page rendering for MainView.
//!
//! Extracted from main_view.rs for maintainability:
//! - Home page layout with Quick Access, Drives, Recent Files, Tags, Network sections
//! - Widget builders for each home section

use gpui::*;

use crate::ui::constants::UiConstants;

use super::main_view::{MainView, MainViewState};

impl MainView {
    pub(crate) fn render_home_view(
        state: &MainViewState,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let tags = state.home_tags_cache.clone();
        div()
            .flex_col()
            .flex_grow()
            .w_full()
            // NO h_full() - flex_grow takes remaining space
            .p(px(24.0))
            .gap(px(24.0))
            .id("home-scroll")
            .overflow_y_scroll()
            .track_scroll(&state.scroll_handle)
            .child(
                div()
                    .flex_col()
                    .gap(px(8.0))
                    .child(
                        div()
                            .font_weight(FontWeight(600.0))
                            .text_size(px(18.0))
                            .font_family(UiConstants::FONT_MAIN)
                            .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                            .child("Quick Access"),
                    )
                    .child(
                        div()
                            .flex_row()
                            .gap(px(12.0))
                            .child(Self::build_home_widget("Desktop", "\u{E8B7}", view.clone()))
                            .child(Self::build_network_widget(
                                "Network".to_string(),
                                "\u{E968}".to_string(),
                                "Connected".to_string(),
                            ))
                            .child(Self::build_network_widget(
                                "Server".to_string(),
                                "\u{E839}".to_string(),
                                "Available".to_string(),
                            )),
                    ),
            )
            .child(
                div()
                    .flex_col()
                    .gap(px(8.0))
                    .child(
                        div()
                            .font_weight(FontWeight(600.0))
                            .text_size(px(18.0))
                            .font_family(UiConstants::FONT_MAIN)
                            .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                            .child("Drives"),
                    )
                    .child(
                        div()
                            .flex_row()
                            .gap(px(12.0))
                            .child(Self::build_drive_widget(
                                "Local Disk (C:)",
                                0.65,
                                view.clone(),
                            ))
                            .child(Self::build_drive_widget("Data (D:)", 0.2, view.clone())),
                    ),
            )
            .child(Self::build_recent_files_section(view.clone()))
            .child(Self::build_file_tags_section(tags, view.clone()))
            .child(Self::build_network_section(view.clone()))
    }

    pub(crate) fn build_home_widget(
        label: &str,
        glyph: &str,
        _view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        div()
            .w(px(120.0))
            .h(px(100.0))
            .bg(rgba(0xFFFFFF10u32))
            .rounded(px(8.0))
            .border(px(1.0))
            .border_color(rgba(UiConstants::DIVIDER_RGBA))
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(8.0))
            .hover(|s| s.bg(rgba(0xFFFFFF15u32)))
            .cursor_pointer()
            .child(
                div()
                    .text_size(px(32.0))
                    .font_family(UiConstants::FONT_MDL2)
                    .text_color(rgba(UiConstants::ACCENT_RGBA))
                    .child(glyph.to_string()),
            )
            .child(
                div()
                    .text_size(px(13.0))
                    .font_family(UiConstants::FONT_MAIN)
                    .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                    .child(label.to_string()),
            )
    }

    pub(crate) fn build_drive_widget(
        label: &str,
        usage: f32,
        _view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        div()
            .w(px(200.0))
            .p(px(12.0))
            .bg(rgba(0xFFFFFF10u32))
            .rounded(px(8.0))
            .border(px(1.0))
            .border_color(rgba(UiConstants::DIVIDER_RGBA))
            .flex_col()
            .gap(px(8.0))
            .hover(|s| s.bg(rgba(0xFFFFFF15u32)))
            .cursor_pointer()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        div()
                            .text_size(px(24.0))
                            .font_family(UiConstants::FONT_MDL2)
                            .text_color(rgba(UiConstants::ACCENT_RGBA))
                            .child("\u{E770}"),
                    )
                    .child(
                        div()
                            .text_size(px(14.0))
                            .font_weight(FontWeight(600.0))
                            .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                            .child(label.to_string()),
                    ),
            )
            .child(
                div()
                    .flex_col()
                    .gap(px(4.0))
                    .child(
                        div()
                            .w_full()
                            .h(px(6.0))
                            .bg(rgba(0xFFFFFF20u32))
                            .rounded(px(3.0))
                            .child(
                                div()
                                    .h_full()
                                    .w(px(100.0 * usage))
                                    .bg(rgb(UiConstants::ACCENT_COLOR_RGB))
                                    .rounded(px(3.0)),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(rgb(UiConstants::TEXT_MUTED_RGB))
                            .child(format!("{:.0}% used", usage * 100.0)),
                    ),
            )
    }

    /// Build the Recent Files section for Home page
    /// Shows a table list: icon, name, path, date accessed
    /// Source: Windows Shell recent items via SHGetKnownFolderPath(FOLDERID_Recent)
    pub(crate) fn build_recent_files_section(
        _view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        div()
            .flex_col()
            .gap(px(8.0))
            .child(
                div()
                    .font_weight(FontWeight(600.0))
                    .text_size(px(18.0))
                    .font_family(UiConstants::FONT_MAIN)
                    .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                    .child("Recent Files"),
            )
            .child(
                div()
                    .flex_col()
                    .bg(rgba(0xFFFFFF10u32))
                    .rounded(px(8.0))
                    .border(px(1.0))
                    .border_color(rgba(UiConstants::DIVIDER_RGBA))
                    .child(
                        // Header row
                        div()
                            .flex()
                            .flex_row()
                            .h(px(36.0))
                            .px(px(12.0))
                            .items_center()
                            .border_b(px(1.0))
                            .border_color(rgba(UiConstants::DIVIDER_RGBA))
                            .child(
                                div()
                                    .flex_grow()
                                    .text_size(px(12.0))
                                    .font_weight(FontWeight(600.0))
                                    .text_color(rgb(UiConstants::TEXT_SECONDARY_RGB))
                                    .child("Name"),
                            )
                            .child(
                                div()
                                    .w(px(200.0))
                                    .text_size(px(12.0))
                                    .font_weight(FontWeight(600.0))
                                    .text_color(rgb(UiConstants::TEXT_SECONDARY_RGB))
                                    .child("Location"),
                            )
                            .child(
                                div()
                                    .w(px(120.0))
                                    .text_size(px(12.0))
                                    .font_weight(FontWeight(600.0))
                                    .text_color(rgb(UiConstants::TEXT_SECONDARY_RGB))
                                    .child("Date Modified"),
                            ),
                    )
                    .child(
                        // Sample recent files (placeholder data)
                        Self::build_recent_file_row(
                            "document.pdf".to_string(),
                            "Documents".to_string(),
                            "Today".to_string(),
                        ),
                    )
                    .child(
                        div()
                            .border_b(px(1.0))
                            .border_color(rgba(0xFFFFFF0Fu32))
                            .child(Self::build_recent_file_row(
                                "image.png".to_string(),
                                "Pictures".to_string(),
                                "Yesterday".to_string(),
                            )),
                    )
                    .child(
                        div()
                            .border_b(px(1.0))
                            .border_color(rgba(0xFFFFFF0Fu32))
                            .child(Self::build_recent_file_row(
                                "report.xlsx".to_string(),
                                "Documents".to_string(),
                                "2 days ago".to_string(),
                            )),
                    ),
            )
    }

    pub(crate) fn build_recent_file_row(name: String, location: String, date: String) -> Div {
        div()
            .flex()
            .flex_row()
            .h(px(36.0))
            .px(px(12.0))
            .items_center()
            .hover(|s| s.bg(rgba(0xFFFFFF08u32)))
            .cursor_pointer()
            .child(
                div()
                    .flex_grow()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        div()
                            .text_size(px(16.0))
                            .font_family(UiConstants::FONT_MDL2)
                            .text_color(rgba(UiConstants::ACCENT_RGBA))
                            .child("\u{E8A5}"), // Document icon
                    )
                    .child(
                        div()
                            .text_size(px(13.0))
                            .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                            .child(name.clone()),
                    ),
            )
            .child(
                div()
                    .w(px(200.0))
                    .text_size(px(12.0))
                    .text_color(rgb(UiConstants::TEXT_SECONDARY_RGB))
                    .child(location.clone()),
            )
            .child(
                div()
                    .w(px(120.0))
                    .text_size(px(12.0))
                    .text_color(rgb(UiConstants::TEXT_MUTED_RGB))
                    .child(date.clone()),
            )
    }

    /// Build the File Tags section for Home page
    /// Shows tag chips grouped by color with file count
    pub(crate) fn build_file_tags_section(
        tags: Vec<(String, String, i64)>,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let mut tag_chips = div().flex_row().gap(px(8.0)).flex_wrap();

        if tags.is_empty() {
            tag_chips = tag_chips.child(
                div()
                    .text_size(px(13.0))
                    .text_color(rgb(UiConstants::TEXT_SECONDARY_RGB))
                    .child("No tags yet. Right-click a file to add tags."),
            );
        } else {
            for (name, color, count) in tags.clone() {
                let view_clone = view.clone();
                tag_chips = tag_chips.child(
                    Self::build_tag_chip(name.clone(), color, count).on_mouse_down(
                        MouseButton::Left,
                        move |_, _, cx| {
                            view_clone.update(cx, |this, cx| {
                                this.set_filter_text(name.clone(), cx);
                            });
                        },
                    ),
                );
            }
        }

        div()
            .flex_col()
            .gap(px(8.0))
            .child(
                div()
                    .font_weight(FontWeight(600.0))
                    .text_size(px(18.0))
                    .font_family(UiConstants::FONT_MAIN)
                    .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                    .child("File Tags"),
            )
            .child(tag_chips)
    }

    pub(crate) fn build_tag_chip(name: String, color: String, count: i64) -> Div {
        // Parse hex color for display - convert "#4285F4" to 0x004285F4 for rgb()
        let hex_color = color.trim_start_matches('#');
        let r = u8::from_str_radix(&hex_color[0..2], 16).unwrap_or(128);
        let g = u8::from_str_radix(&hex_color[2..4], 16).unwrap_or(128);
        let b = u8::from_str_radix(&hex_color[4..6], 16).unwrap_or(128);
        let rgb_value = ((r as u32) << 16) | ((g as u32) << 8) | (b as u32);

        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.0))
            .px(px(12.0))
            .py(px(6.0))
            .bg(rgba(0xFFFFFF10u32))
            .rounded(px(16.0))
            .border(px(1.0))
            .border_color(rgba(UiConstants::DIVIDER_RGBA))
            .hover(|s| s.bg(rgba(0xFFFFFF15u32)))
            .cursor_pointer()
            .child(
                div()
                    .w(px(8.0))
                    .h(px(8.0))
                    .rounded(px(4.0))
                    .bg(rgb(rgb_value)),
            )
            .child(
                div()
                    .text_size(px(13.0))
                    .font_family(UiConstants::FONT_MAIN)
                    .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                    .child(name.clone()),
            )
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(rgb(UiConstants::TEXT_MUTED_RGB))
                    .child(format!("({})", count)),
            )
    }

    /// Build the Network Locations section for Home page
    /// Shows SMB shares and network drives
    pub(crate) fn build_network_section(_view: Entity<crate::app::AppWindow>) -> impl IntoElement {
        div()
            .flex_col()
            .gap(px(8.0))
            .child(
                div()
                    .font_weight(FontWeight(600.0))
                    .text_size(px(18.0))
                    .font_family(UiConstants::FONT_MAIN)
                    .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                    .child("Network Locations"),
            )
            .child(
                div()
                    .flex_row()
                    .gap(px(12.0))
                    .child(Self::build_network_widget(
                        "Network".to_string(),
                        "\u{E968}".to_string(),
                        "Connected".to_string(),
                    ))
                    .child(Self::build_network_widget(
                        "Server".to_string(),
                        "\u{E839}".to_string(),
                        "Available".to_string(),
                    )),
            )
    }

    pub(crate) fn build_network_widget(label: String, glyph: String, status: String) -> Div {
        div()
            .w(px(180.0))
            .p(px(12.0))
            .bg(rgba(0xFFFFFF10u32))
            .rounded(px(8.0))
            .border(px(1.0))
            .border_color(rgba(UiConstants::DIVIDER_RGBA))
            .flex_col()
            .gap(px(8.0))
            .hover(|s| s.bg(rgba(0xFFFFFF15u32)))
            .cursor_pointer()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        div()
                            .text_size(px(24.0))
                            .font_family(UiConstants::FONT_MDL2)
                            .text_color(rgba(UiConstants::ACCENT_RGBA))
                            .child(glyph.clone()),
                    )
                    .child(
                        div()
                            .text_size(px(14.0))
                            .font_weight(FontWeight(600.0))
                            .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                            .child(label.clone()),
                    ),
            )
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(rgb(UiConstants::TEXT_MUTED_RGB))
                    .child(status.clone()),
            )
    }
}

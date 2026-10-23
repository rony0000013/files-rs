//! Details & Preview Pane (`InfoPane.xaml` parity) rendering methods for `MainView`.
//!
//! Implements both tabs of the `Files-community` `InfoPane`:
//! - **Details Tab**:
//!   - 0-selection state: Current Directory & Drive capacity bar summary
//!   - Multi-selection state: Combined item count, total size, and batch Tag assignment pills
//!   - Single-item state: Large thumbnail/icon hero, title & type badge, interactive File Tags
//!     pills (click to add/remove tag), General properties (`FileDetailsRepeater`), EXIF/Media
//!     metadata, Git commit info, and `Open Properties (Alt+Enter)` button.
//! - **Preview Tab**:
//!   - Full rich previews for Images, Syntax-Highlighted Code (with language header & line numbers),
//!     Markdown, PDFs, Archives, Audio/Video Media, HTML, RTF, Plain Text, and Folder 12-item grid.

use crate::domain::metadata::FileItem;
use crate::platform::windows::drive_service::{enumerate_drives, DriveInfo};
use crate::storage::storage::FileType;
use crate::ui::constants::UiConstants;
use gpui::*;

use super::helpers::*;
use super::main_view::MainView;
use super::types::*;

std::thread_local! {
    /// Drive list cache: `enumerate_drives()` hits Win32 volume APIs per
    /// call, so renders (and especially resize-drag frames) share a
    /// 30-second snapshot instead of re-querying every frame.
    static DRIVE_LIST_CACHE: std::cell::RefCell<(Option<std::time::Instant>, Vec<DriveInfo>)> =
        std::cell::RefCell::new((None, Vec::new()));
}

/// Cached `enumerate_drives()` with a 30s TTL for per-frame render paths.
fn cached_drives() -> Vec<DriveInfo> {
    DRIVE_LIST_CACHE.with(|cell| {
        let mut guard = cell.borrow_mut();
        let now = std::time::Instant::now();
        let stale = guard
            .0
            .map(|t| now.duration_since(t).as_secs() >= 30)
            .unwrap_or(true);
        if stale {
            guard.1 = enumerate_drives();
            guard.0 = Some(now);
        }
        guard.1.clone()
    })
}

impl MainView {
    /// Render the `InfoPane` content when **no item is selected** (shows current folder & drive details, `Files-community` parity).
    pub(crate) fn render_empty_selection_details(
        state: &MainViewState,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let path_str = state.primary_path.to_string_lossy().to_string();
        let folder_name = state
            .primary_path
            .as_path()
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| {
                if path_str.is_empty() {
                    "Home".to_string()
                } else {
                    path_str.clone()
                }
            });

        let item_count = state.items.len();
        let dir_count = state
            .items
            .iter()
            .filter(|i| matches!(i.file_type, FileType::Directory))
            .count();
        let file_count = item_count.saturating_sub(dir_count);
        let total_size: u64 = state
            .items
            .iter()
            .filter(|i| !matches!(i.file_type, FileType::Directory))
            .map(|i| i.size)
            .sum();

        // Check if current path belongs to a known drive so we can show Drive Storage Capacity Bar.
        // Uses a 30s TTL cache: raw enumerate_drives() hits Win32 volume APIs
        // and must not run on every render frame (e.g. splitter drags).
        let matched_drive = cached_drives().into_iter().find(|d| {
            path_str
                .to_ascii_lowercase()
                .starts_with(&d.root_path().to_ascii_lowercase())
        });

        let mut drive_card = div();
        if let Some(drive) = matched_drive {
            let usage_pct = drive.usage_percent();
            let ratio = (usage_pct / 100.0).clamp(0.02, 1.0);
            let bar_col = if drive.is_low_space() || usage_pct >= 90.0 {
                0xE81123u32
            } else if usage_pct >= 78.0 {
                0xF7931Eu32
            } else {
                0x60CDFFu32
            };
            drive_card = div()
                .w_full()
                .p(px(12.0))
                .rounded(px(8.0))
                .bg(rgba(0xFFFFFF07u32))
                .border(px(1.0))
                .border_color(rgba(UiConstants::CARD_STROKE_RGBA))
                .flex()
                .flex_col()
                .gap(px(6.0))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .text_size(px(12.0))
                                .font_weight(FontWeight(600.0))
                                .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                                .child(drive.display_name()),
                        )
                        .child(
                            div()
                                .text_size(px(11.0))
                                .text_color(rgba(UiConstants::TEXT_MUTED_RGBA))
                                .child(drive.file_system.clone()),
                        ),
                )
                .child(
                    div()
                        .w_full()
                        .h(px(6.0))
                        .rounded(px(3.0))
                        .bg(rgba(0xFFFFFF18u32))
                        .overflow_hidden()
                        .child(
                            div()
                                .h_full()
                                .w(px((220.0 * ratio) as f32))
                                .rounded(px(3.0))
                                .bg(rgb(bar_col)),
                        ),
                )
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                        .child(drive.capacity_summary()),
                );
        }

        let v_props = view;
        div()
            .w_full()
            .flex()
            .flex_col()
            .gap(px(12.0))
            .child(
                // Folder Hero Card
                div()
                    .w_full()
                    .h(px(128.0))
                    .rounded(px(8.0))
                    .bg(rgba(0xFFFFFF07u32))
                    .border(px(1.0))
                    .border_color(rgba(UiConstants::CARD_STROKE_RGBA))
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap(px(8.0))
                    .child(
                        div()
                            .w(px(52.0))
                            .h(px(52.0))
                            .rounded(px(10.0))
                            .bg(rgba(0xFCCB4520u32))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                div()
                                    .text_size(px(28.0))
                                    .font_family(UiConstants::FONT_ICONS)
                                    .text_color(rgb(0xFCCB45))
                                    .child("\u{E8B7}"),
                            ),
                    )
                    .child(
                        div()
                            .px(px(12.0))
                            .text_size(px(14.0))
                            .font_weight(FontWeight(600.0))
                            .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                            .text_center()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .child(folder_name),
                    ),
            )
            .child(
                div()
                    .w_full()
                    .p(px(12.0))
                    .rounded(px(8.0))
                    .bg(rgba(0xFFFFFF07u32))
                    .border(px(1.0))
                    .border_color(rgba(UiConstants::CARD_STROKE_RGBA))
                    .flex()
                    .flex_col()
                    .gap(px(6.0))
                    .child(section_header("Folder Summary"))
                    .child(info_row("Total items", &format!("{}", item_count)))
                    .child(info_row(
                        "Breakdown",
                        &format!("{} folders, {} files", dir_count, file_count),
                    ))
                    .child(info_row("Files size", &format_size(total_size)))
                    .child(info_row(
                        "Path",
                        if path_str.is_empty() { "Home" } else { &path_str },
                    )),
            )
            .child(drive_card)
            .child(
                div()
                    .w_full()
                    .py(px(7.0))
                    .rounded(px(6.0))
                    .bg(rgba(0xFFFFFF0Eu32))
                    .border(px(1.0))
                    .border_color(rgba(UiConstants::CARD_STROKE_RGBA))
                    .hover(|s| s.bg(rgba(0xFFFFFF18u32)))
                    .cursor_pointer()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_center()
                    .gap(px(6.0))
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        v_props.update(cx, |this, cx| {
                            this.open_properties_for_selected(cx);
                        });
                    })
                    .child(
                        div()
                            .text_size(px(12.5))
                            .font_family(UiConstants::FONT_ICONS)
                            .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                            .child("\u{E946}"),
                    )
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                            .child("Folder Properties"),
                    ),
            )
    }

    /// Preview-tab body for multi-selection: a compact per-item preview list
    /// (icon + name + size) so Preview never echoes the Details tab.
    pub(crate) fn render_multi_selection_preview(state: &MainViewState) -> impl IntoElement {
        let mut rows: Vec<AnyElement> = Vec::new();
        let mut indices: Vec<usize> = state.selected_indices.iter().copied().collect();
        indices.sort_unstable();
        for &idx in indices.iter().take(50) {
            if let Some(item) = state.items.get(idx) {
                let is_dir = matches!(item.file_type, FileType::Directory);
                rows.push(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(8.0))
                        .py(px(4.0))
                        .child(
                            div()
                                .text_size(px(16.0))
                                .font_family(UiConstants::FONT_ICONS)
                                .text_color(rgb(0x60CDFF))
                                .child(if is_dir { "\u{E8B7}" } else { "\u{E8A5}" }),
                        )
                        .child(
                            div()
                                .flex_1()
                                .overflow_hidden()
                                .text_size(px(12.0))
                                .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                                .child(item.name.clone()),
                        )
                        .child(
                            div()
                                .text_size(px(11.0))
                                .text_color(rgba(UiConstants::TEXT_MUTED_RGBA))
                                .child(if is_dir {
                                    "Folder".to_string()
                                } else {
                                    format_size(item.size)
                                }),
                        )
                        .into_any_element(),
                );
            }
        }
        let header = format!(
            "{} items selected",
            state.selected_indices.len()
        );
        div()
            .flex_col()
            .gap(px(8.0))
            .w_full()
            .child(
                div()
                    .text_size(px(13.0))
                    .font_weight(FontWeight(600.0))
                    .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                    .child(header),
            )
            .children(rows)
            .into_any_element()
    }

    /// Render the `InfoPane` content when **multiple items are selected** (`Files-community` multi-select summary parity).
    pub(crate) fn render_multi_selection_details(
        state: &MainViewState,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let count = state.selected_indices.len();
        let mut total_size = 0u64;
        let mut folders = 0usize;
        let mut files = 0usize;
        for &idx in &state.selected_indices {
            if let Some(it) = state.items.get(idx) {
                if matches!(it.file_type, FileType::Directory) {
                    folders += 1;
                } else {
                    files += 1;
                    total_size += it.size;
                }
            }
        }

        let quick_tags = [
            ("Work", 0x0078D4u32),
            ("Personal", 0x107C10u32),
            ("Important", 0xE81123u32),
            ("Urgent", 0xF7630Cu32),
            ("Archive", 0x7A7574u32),
        ];

        let mut tag_row = div().flex().flex_row().flex_wrap().gap(px(6.0));
        for (tag_name, color_hex) in quick_tags {
            let v_tag = view.clone();
            let t_str = tag_name.to_string();
            tag_row = tag_row.child(
                div()
                    .px(px(8.0))
                    .py(px(3.5))
                    .rounded(px(12.0))
                    .bg(rgba(0xFFFFFF0Du32))
                    .border(px(1.0))
                    .border_color(rgba(UiConstants::CARD_STROKE_RGBA))
                    .hover(|s| s.bg(rgba(0xFFFFFF1Au32)))
                    .cursor_pointer()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(5.0))
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        let t = t_str.clone();
                        v_tag.update(cx, |this, cx| {
                            this.add_tag_to_selected(t, cx);
                        });
                    })
                    .child(
                        div()
                            .w(px(7.0))
                            .h(px(7.0))
                            .rounded(px(3.5))
                            .bg(rgb(color_hex)),
                    )
                    .child(
                        div()
                            .text_size(px(11.5))
                            .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                            .child(format!("+ {}", tag_name)),
                    ),
            );
        }

        let v_props = view;
        div()
            .w_full()
            .flex()
            .flex_col()
            .gap(px(12.0))
            .child(
                div()
                    .w_full()
                    .h(px(124.0))
                    .rounded(px(8.0))
                    .bg(rgba(0xFFFFFF07u32))
                    .border(px(1.0))
                    .border_color(rgba(UiConstants::CARD_STROKE_RGBA))
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap(px(8.0))
                    .child(
                        div()
                            .w(px(52.0))
                            .h(px(52.0))
                            .rounded(px(10.0))
                            .bg(rgba(0x60CDFF22u32))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                div()
                                    .text_size(px(26.0))
                                    .font_family(UiConstants::FONT_ICONS)
                                    .text_color(rgb(0x60CDFF))
                                    .child("\u{E762}"),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(14.0))
                            .font_weight(FontWeight(600.0))
                            .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                            .child(format!("{} items selected", count)),
                    ),
            )
            .child(
                div()
                    .w_full()
                    .p(px(12.0))
                    .rounded(px(8.0))
                    .bg(rgba(0xFFFFFF07u32))
                    .border(px(1.0))
                    .border_color(rgba(UiConstants::CARD_STROKE_RGBA))
                    .flex()
                    .flex_col()
                    .gap(px(6.0))
                    .child(section_header("Selection Summary"))
                    .child(info_row("Selected items", &format!("{}", count)))
                    .child(info_row(
                        "Composition",
                        &format!("{} folders, {} files", folders, files),
                    ))
                    .child(info_row("Combined size", &format_size(total_size))),
            )
            .child(
                div()
                    .w_full()
                    .p(px(12.0))
                    .rounded(px(8.0))
                    .bg(rgba(0xFFFFFF07u32))
                    .border(px(1.0))
                    .border_color(rgba(UiConstants::CARD_STROKE_RGBA))
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .child(section_header("Batch Assign File Tag"))
                    .child(tag_row),
            )
            .child(
                div()
                    .w_full()
                    .py(px(7.0))
                    .rounded(px(6.0))
                    .bg(rgba(0xFFFFFF0Eu32))
                    .border(px(1.0))
                    .border_color(rgba(UiConstants::CARD_STROKE_RGBA))
                    .hover(|s| s.bg(rgba(0xFFFFFF18u32)))
                    .cursor_pointer()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_center()
                    .gap(px(6.0))
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        v_props.update(cx, |this, cx| {
                            this.open_properties_for_selected(cx);
                        });
                    })
                    .child(
                        div()
                            .text_size(px(12.5))
                            .font_family(UiConstants::FONT_ICONS)
                            .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                            .child("\u{E946}"),
                    )
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                            .child("Properties (Alt+Enter)"),
                    ),
            )
    }

    pub(crate) fn render_preview_content(
        state: &MainViewState,
        item: &FileItem,
    ) -> impl IntoElement {
        match &state.preview_content {
            PreviewContent::Markdown => {
                if let Some(text) = &state.preview_text {
                    Self::render_markdown_preview(text).into_any_element()
                } else {
                    Self::render_default_preview(item, state).into_any_element()
                }
            }
            PreviewContent::Code { language } => {
                if state.preview_text.is_some() {
                    let (lines, total, truncated) = Self::code_tokens_for_state(state);
                    Self::render_code_preview(&lines, language, total, truncated)
                        .into_any_element()
                } else {
                    Self::render_default_preview(item, state).into_any_element()
                }
            }
            PreviewContent::Html => {
                if let Some(text) = &state.preview_text {
                    Self::render_html_preview(text).into_any_element()
                } else {
                    Self::render_default_preview(item, state).into_any_element()
                }
            }
            PreviewContent::RichText => {
                if let Some(text) = &state.preview_text {
                    Self::render_rtf_preview(text).into_any_element()
                } else {
                    Self::render_default_preview(item, state).into_any_element()
                }
            }
            PreviewContent::Audio(info) | PreviewContent::Video(info) => {
                let type_str = if matches!(&state.preview_content, PreviewContent::Audio(_)) {
                    "Audio"
                } else {
                    "Video"
                };
                let text = state
                    .preview_text
                    .as_deref()
                    .unwrap_or("No metadata available");
                Self::render_media_preview(info, type_str, text, item).into_any_element()
            }
            PreviewContent::Folder(items) => Self::render_folder_preview(items).into_any_element(),
            PreviewContent::Pdf(info) => {
                let text = state
                    .preview_text
                    .as_deref()
                    .unwrap_or("Loading PDF info...");
                Self::render_pdf_preview(info, text).into_any_element()
            }
            PreviewContent::Archive(info) => {
                let text = state
                    .preview_text
                    .as_deref()
                    .unwrap_or("Loading archive...");
                Self::render_archive_preview(info, text).into_any_element()
            }
            PreviewContent::Text(_) | PreviewContent::None => {
                Self::render_default_preview(item, state).into_any_element()
            }
            PreviewContent::ThirdParty(info) => {
                if let Some(text) = &state.preview_text {
                    div()
                        .flex_col()
                        .w_full()
                        .max_h(px(440.0))
                        .child(
                            div()
                                .child(
                                    div()
                                        .text_size(px(13.0))
                                        .font_weight(FontWeight(600.0))
                                        .mb(px(6.0))
                                        .child(format!("Shell Preview ({:?})", info.handler)),
                                )
                                .child(
                                    div()
                                        .bg(rgba(0xFFFFFF07u32))
                                        .border(px(1.0))
                                        .border_color(rgba(UiConstants::CARD_STROKE_RGBA))
                                        .rounded(px(6.0))
                                        .p(px(10.0))
                                        .child(
                                            div()
                                                .text_size(px(11.0))
                                                .font_family("ui-monospace, SFMono-Regular, 'Cascadia Code', Consolas, monospace")
                                                .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                                                .child(text.clone()),
                                        ),
                                ),
                        )
                        .into_any_element()
                } else {
                    Self::render_default_preview(item, state).into_any_element()
                }
            }
        }
    }

    pub(crate) fn render_info_content(item: &FileItem) -> impl IntoElement {
        let type_label = if matches!(item.file_type, FileType::Directory) {
            "File folder"
        } else {
            "File"
        };
        let size_str = format_size(item.size);

        div()
            .flex_col()
            .gap(px(8.0))
            .w_full()
            .child(info_row("Name", &item.name))
            .child(info_row("Type", type_label))
            .child(info_row("Size", &size_str))
            .child(info_row("Path", &item.path.to_string_lossy()))
    }

    /// Rich `Files-community` Details Tab (`InfoPane.xaml` `FileDetailsRepeater` + `FileTags` parity).
    pub(crate) fn render_details_pane(
        state: &MainViewState,
        item: &FileItem,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let is_dir = matches!(item.file_type, FileType::Directory);
        let ext = if is_dir {
            String::new()
        } else {
            item.name
                .rsplit('.')
                .next()
                .filter(|e| *e != item.name)
                .unwrap_or("")
                .to_uppercase()
        };
        let type_label = if is_dir {
            "File folder".to_string()
        } else if ext.is_empty() {
            "File".to_string()
        } else {
            format!("{} File", ext)
        };
        let size_str = if is_dir {
            "—".to_string()
        } else {
            format_size(item.size)
        };
        let is_image = matches!(
            ext.as_str(),
            "JPG" | "JPEG" | "PNG" | "WEBP" | "BMP" | "GIF" | "TIFF"
        );
        let cached_icon = state.icon_paths.get(&item.path).cloned();

        let modified_str = item
            .modified
            .map(|dt| {
                chrono::DateTime::<chrono::Local>::from(dt)
                    .format("%Y-%m-%d %H:%M")
                    .to_string()
            })
            .unwrap_or_else(|| "—".to_string());
        let created_str = item
            .created
            .map(|dt| {
                chrono::DateTime::<chrono::Local>::from(dt)
                    .format("%Y-%m-%d %H:%M")
                    .to_string()
            })
            .unwrap_or_else(|| "—".to_string());
        let parent_str = item
            .path
            .as_path()
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| item.path.to_string_lossy().to_string());

        // Interactive File Tags section
        let assigned_tags = state.details_cache.tags.clone();
        let quick_palette = [
            ("Work", 0x0078D4u32),
            ("Personal", 0x107C10u32),
            ("Important", 0xE81123u32),
            ("Urgent", 0xF7630Cu32),
            ("Archive", 0x7A7574u32),
        ];

        let mut tags_wrap = div().flex().flex_row().flex_wrap().gap(px(6.0));
        for (t_name, t_col) in quick_palette {
            let is_active = assigned_tags
                .iter()
                .any(|t| t.eq_ignore_ascii_case(t_name));
            let v_tag = view.clone();
            let tag_str = t_name.to_string();
            let file_path = item.path.clone();

            tags_wrap = tags_wrap.child(
                div()
                    .px(px(9.0))
                    .py(px(3.5))
                    .rounded(px(12.0))
                    .bg(if is_active {
                        rgba((t_col << 8) | 0x33)
                    } else {
                        rgba(0xFFFFFF0Au32)
                    })
                    .border(px(1.0))
                    .border_color(if is_active {
                        rgba((t_col << 8) | 0xAA)
                    } else {
                        rgba(UiConstants::CARD_STROKE_RGBA)
                    })
                    .hover(|s| s.bg(rgba(0xFFFFFF18u32)))
                    .cursor_pointer()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(5.0))
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        let t = tag_str.clone();
                        let fp = file_path.clone();
                        v_tag.update(cx, |this, cx| {
                            if is_active {
                                this.remove_tag_from_selected(&t, cx);
                                this.state
                                    .main_view
                                    .details_cache
                                    .tags
                                    .retain(|x| !x.eq_ignore_ascii_case(&t));
                            } else {
                                this.add_tag_to_file(&fp, t.clone(), cx);
                                this.state.main_view.details_cache.tags.push(t);
                            }
                            cx.notify();
                        });
                    })
                    .child(
                        div()
                            .w(px(7.0))
                            .h(px(7.0))
                            .rounded(px(3.5))
                            .bg(rgb(t_col)),
                    )
                    .child(
                        div()
                            .text_size(px(11.5))
                            .font_weight(if is_active {
                                FontWeight(600.0)
                            } else {
                                FontWeight(400.0)
                            })
                            .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                            .child(if is_active {
                                format!("✓ {}", t_name)
                            } else {
                                t_name.to_string()
                            }),
                    ),
            );
        }

        let v_props = view;

        div()
            .flex()
            .flex_col()
            .gap(px(10.0))
            .w_full()
            // 1. Hero Thumbnail / Icon Card
            .child(
                div()
                    .w_full()
                    .h(px(144.0))
                    .p(px(12.0))
                    .bg(rgba(0xFFFFFF07u32))
                    .border(px(1.0))
                    .border_color(rgba(UiConstants::CARD_STROKE_RGBA))
                    .rounded(px(8.0))
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap(px(8.0))
                    .child(if is_image {
                        img(item.path.as_path().to_path_buf())
                            .max_w(px(220.0))
                            .h(px(92.0))
                            .rounded(px(6.0))
                            .into_any_element()
                    } else if let Some(icon_pb) = cached_icon {
                        img(icon_pb)
                            .w(px(52.0))
                            .h(px(52.0))
                            .into_any_element()
                    } else {
                        div()
                            .w(px(52.0))
                            .h(px(52.0))
                            .rounded(px(10.0))
                            .bg(if is_dir {
                                rgba(0xFCCB4522u32)
                            } else {
                                rgba(0x60CDFF20u32)
                            })
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                div()
                                    .text_size(px(28.0))
                                    .font_family(UiConstants::FONT_ICONS)
                                    .text_color(if is_dir {
                                        rgb(0xFCCB45)
                                    } else {
                                        rgb(0x60CDFF)
                                    })
                                    .child(if is_dir { "\u{E8B7}" } else { "\u{E8A5}" }),
                            )
                            .into_any_element()
                    })
                    .child(
                        div()
                            .w_full()
                            .text_size(px(13.5))
                            .font_weight(FontWeight(600.0))
                            .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                            .text_center()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .child(item.name.clone()),
                    )
                    .child(
                        div()
                            .text_size(px(11.5))
                            .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                            .child(if is_dir {
                                type_label.clone()
                            } else {
                                format!("{} • {}", type_label, size_str)
                            }),
                    ),
            )
            // 2. Interactive File Tags Card (`InfoPane.xaml` FileTags parity)
            .child(
                div()
                    .w_full()
                    .p(px(12.0))
                    .rounded(px(8.0))
                    .bg(rgba(0xFFFFFF07u32))
                    .border(px(1.0))
                    .border_color(rgba(UiConstants::CARD_STROKE_RGBA))
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .child(section_header("File Tags"))
                    .child(tags_wrap),
            )
            // 3. General Properties Card (`FileDetailsRepeater`)
            .child(
                div()
                    .w_full()
                    .p(px(12.0))
                    .rounded(px(8.0))
                    .bg(rgba(0xFFFFFF07u32))
                    .border(px(1.0))
                    .border_color(rgba(UiConstants::CARD_STROKE_RGBA))
                    .flex()
                    .flex_col()
                    .gap(px(6.0))
                    .child(section_header("General"))
                    .child(info_row("Item type", &type_label))
                    .child(info_row("Size", &size_str))
                    .child(info_row("Date modified", &modified_str))
                    .child(info_row("Date created", &created_str))
                    .child(info_row("Location", &parent_str)),
            )
            // 4. EXIF / Media Metadata Card (when available)
            .children(
                if state.details_cache.exif_dimensions.is_some()
                    || state.details_cache.audio_duration.is_some()
                    || state.details_cache.video_resolution.is_some()
                {
                    let mut media_rows: Vec<AnyElement> = vec![section_header("Media & EXIF").into_any_element()];
                    if let Some(ref dims) = state.details_cache.exif_dimensions {
                        media_rows.push(info_row("Dimensions", dims).into_any_element());
                    }
                    if let Some(ref camera) = state.details_cache.exif_camera {
                        media_rows.push(info_row("Camera", camera).into_any_element());
                    }
                    if let Some(ref date) = state.details_cache.exif_date {
                        media_rows.push(info_row("Date taken", date).into_any_element());
                    }
                    if let Some(ref duration) = state.details_cache.audio_duration {
                        media_rows.push(info_row("Duration", duration).into_any_element());
                    }
                    if let Some(ref artist) = state.details_cache.audio_artist {
                        media_rows.push(info_row("Artist", artist).into_any_element());
                    }
                    if let Some(ref album) = state.details_cache.audio_album {
                        media_rows.push(info_row("Album", album).into_any_element());
                    }
                    if let Some(ref res) = state.details_cache.video_resolution {
                        media_rows.push(info_row("Resolution", res).into_any_element());
                    }
                    vec![div()
                        .w_full()
                        .p(px(12.0))
                        .rounded(px(8.0))
                        .bg(rgba(0xFFFFFF07u32))
                        .border(px(1.0))
                        .border_color(rgba(UiConstants::CARD_STROKE_RGBA))
                        .flex()
                        .flex_col()
                        .gap(px(6.0))
                        .children(media_rows)
                        .into_any_element()]
                } else {
                    vec![]
                },
            )
            // 5. Git Repository Card
            .children({
                if let Some(info) = crate::domain::git_service::GitService::get_last_commit(&item.path) {
                    vec![div()
                        .w_full()
                        .p(px(12.0))
                        .rounded(px(8.0))
                        .bg(rgba(0xFFFFFF07u32))
                        .border(px(1.0))
                        .border_color(rgba(UiConstants::CARD_STROKE_RGBA))
                        .flex()
                        .flex_col()
                        .gap(px(6.0))
                        .child(section_header("Git Last Commit"))
                        .child(info_row("Author", &info.author))
                        .child(info_row("Date", &info.date))
                        .child(info_row("Commit", &info.message))
                        .into_any_element()]
                } else {
                    vec![]
                }
            })
            // 6. Bottom Open Properties Action Button
            .child(
                div()
                    .w_full()
                    .py(px(7.0))
                    .rounded(px(6.0))
                    .bg(rgba(0xFFFFFF0Eu32))
                    .border(px(1.0))
                    .border_color(rgba(UiConstants::CARD_STROKE_RGBA))
                    .hover(|s| s.bg(rgba(0xFFFFFF18u32)))
                    .cursor_pointer()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_center()
                    .gap(px(6.0))
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        v_props.update(cx, |this, cx| {
                            this.open_properties_for_selected(cx);
                        });
                    })
                    .child(
                        div()
                            .text_size(px(12.5))
                            .font_family(UiConstants::FONT_ICONS)
                            .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                            .child("\u{E946}"),
                    )
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                            .child("Open Properties (Alt+Enter)"),
                    ),
            )
    }

    fn render_default_preview(item: &FileItem, state: &MainViewState) -> impl IntoElement {
        let ext = get_extension(&item.name);
        let is_image = matches!(
            ext.as_str(),
            "jpg" | "jpeg" | "png" | "gif" | "bmp" | "webp" | "ico" | "tiff" | "tif" | "svg"
        );

        div()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(12.0))
            .w_full()
            .child(if is_image {
                div()
                    .w_full()
                    .h(px(220.0))
                    .p(px(10.0))
                    .rounded(px(8.0))
                    .bg(rgba(0xFFFFFF07u32))
                    .border(px(1.0))
                    .border_color(rgba(UiConstants::CARD_STROKE_RGBA))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        img(item.path.as_path().to_path_buf())
                            .max_w_full()
                            .max_h(px(196.0))
                            .rounded(px(6.0)),
                    )
                    .into_any_element()
            } else {
                div()
                    .w_full()
                    .h(px(120.0))
                    .rounded(px(8.0))
                    .bg(rgba(0xFFFFFF07u32))
                    .border(px(1.0))
                    .border_color(rgba(UiConstants::CARD_STROKE_RGBA))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .text_size(px(36.0))
                            .font_family(UiConstants::FONT_ICONS)
                            .text_color(rgb(0x60CDFF))
                            .child(if matches!(item.file_type, FileType::Directory) {
                                "\u{E8B7}"
                            } else {
                                "\u{E8A5}"
                            }),
                    )
                    .into_any_element()
            })
            .child(
                div()
                    .text_size(px(13.5))
                    .font_weight(FontWeight(600.0))
                    .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                    .text_center()
                    .overflow_hidden()
                    .max_w(px(260.0))
                    .child(item.name.clone()),
            )
            .child(if let Some(text) = &state.preview_text {
                div()
                    .w_full()
                    .p(px(10.0))
                    .max_h(px(360.0))
                    .bg(rgba(0xFFFFFF07u32))
                    .border(px(1.0))
                    .border_color(rgba(UiConstants::CARD_STROKE_RGBA))
                    .rounded(px(8.0))
                    .overflow_hidden()
                    .child(
                        div()
                            .text_size(px(11.5))
                            .font_family("ui-monospace, SFMono-Regular, 'Cascadia Code', Consolas, monospace")
                            .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                            .child(text.clone()),
                    )
                    .into_any_element()
            } else {
                div()
                    .text_size(px(12.0))
                    .text_color(rgba(UiConstants::TEXT_MUTED_RGBA))
                    .child("No preview available for this file type")
                    .into_any_element()
            })
    }

    fn render_markdown_preview(text: &str) -> impl IntoElement {
        let mut children: Vec<AnyElement> = Vec::new();
        let mut in_code_block = false;
        let mut code_block_text = String::new();

        for line in text.lines() {
            if line.starts_with("```") {
                if in_code_block {
                    children.push(
                        div()
                            .font_family("ui-monospace, SFMono-Regular, 'Cascadia Code', Consolas, monospace")
                            .text_size(px(11.0))
                            .text_color(rgba(0x00FFD700u32))
                            .bg(rgba(0xFFFFFF10u32))
                            .p(px(6.0))
                            .rounded(px(4.0))
                            .w_full()
                            .child(code_block_text.clone())
                            .into_any_element(),
                    );
                    code_block_text.clear();
                    in_code_block = false;
                } else {
                    in_code_block = true;
                }
                continue;
            }

            if in_code_block {
                if !code_block_text.is_empty() {
                    code_block_text.push('\n');
                }
                code_block_text.push_str(line);
                continue;
            }

            let trimmed = line.trim();
            if let Some(content) = trimmed.strip_prefix("### ") {
                children.push(
                    div()
                        .font_weight(FontWeight(600.0))
                        .text_size(px(13.0))
                        .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                        .mt(px(6.0))
                        .mb(px(2.0))
                        .child(content.to_string())
                        .into_any_element(),
                );
            } else if let Some(content) = trimmed.strip_prefix("## ") {
                children.push(
                    div()
                        .font_weight(FontWeight(700.0))
                        .text_size(px(15.0))
                        .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                        .mt(px(8.0))
                        .mb(px(2.0))
                        .child(content.to_string())
                        .into_any_element(),
                );
            } else if let Some(content) = trimmed.strip_prefix("# ") {
                children.push(
                    div()
                        .font_weight(FontWeight(700.0))
                        .text_size(px(17.0))
                        .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                        .mt(px(10.0))
                        .mb(px(4.0))
                        .child(content.to_string())
                        .into_any_element(),
                );
            } else if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
                let content = &trimmed[2..];
                children.push(
                    div()
                        .flex()
                        .flex_row()
                        .gap(px(4.0))
                        .px(px(8.0))
                        .text_size(px(11.5))
                        .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                        .child(div().child("\u{2022} "))
                        .child(div().child(Self::parse_inline_markdown(content)))
                        .into_any_element(),
                );
            } else if trimmed.is_empty() {
                children.push(div().h(px(6.0)).into_any_element());
            } else {
                children.push(
                    div()
                        .text_size(px(11.5))
                        .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                        .py(px(1.0))
                        .child(Self::parse_inline_markdown(line))
                        .into_any_element(),
                );
            }
        }

        if in_code_block && !code_block_text.is_empty() {
            children.push(
                div()
                    .font_family(
                        "ui-monospace, SFMono-Regular, 'Cascadia Code', Consolas, monospace",
                    )
                    .text_size(px(11.0))
                    .text_color(rgba(0x00FFD700u32))
                    .bg(rgba(0xFFFFFF10u32))
                    .p(px(6.0))
                    .rounded(px(4.0))
                    .w_full()
                    .child(code_block_text)
                    .into_any_element(),
            );
        }

        div()
            .flex_col()
            .w_full()
            .p(px(12.0))
            .rounded(px(8.0))
            .bg(rgba(0xFFFFFF07u32))
            .border(px(1.0))
            .border_color(rgba(UiConstants::CARD_STROKE_RGBA))
            .children(children)
    }

    fn parse_inline_markdown(text: &str) -> impl IntoElement {
        let mut children: Vec<AnyElement> = Vec::new();
        let mut remaining = text;

        while !remaining.is_empty() {
            if let Some(start) = remaining.find("**") {
                if start > 0 {
                    children.push(
                        div()
                            .child(remaining[..start].to_string())
                            .into_any_element(),
                    );
                }
                let after = &remaining[start + 2..];
                if let Some(end) = after.find("**") {
                    children.push(
                        div()
                            .font_weight(FontWeight(700.0))
                            .child(after[..end].to_string())
                            .into_any_element(),
                    );
                    remaining = &after[end + 2..];
                    continue;
                }
            }

            if let Some(start) = remaining.find('`') {
                if start > 0 {
                    children.push(
                        div()
                            .child(remaining[..start].to_string())
                            .into_any_element(),
                    );
                }
                let after = &remaining[start + 1..];
                if let Some(end) = after.find('`') {
                    children.push(
                        div()
                            .font_family("ui-monospace, SFMono-Regular, 'Cascadia Code', Consolas, monospace")
                            .bg(rgba(0xFFFFFF15u32))
                            .px(px(3.0))
                            .rounded(px(2.0))
                            .child(after[..end].to_string())
                            .into_any_element(),
                    );
                    remaining = &after[end + 1..];
                    continue;
                }
            }

            children.push(div().child(remaining.to_string()).into_any_element());
            break;
        }

        div().flex().flex_row().flex_wrap().children(children)
    }

    /// Max lines rendered in the code preview; the full text stays in state.
    const CODE_PREVIEW_MAX_LINES: usize = 400;

    /// Resolve highlighted lines for the current preview text, preferring the
    /// load-time cache so resize drags never re-tokenize. Returns
    /// (lines_to_render, total_lines, was_truncated).
    pub(crate) fn code_tokens_for_state(
        state: &MainViewState,
    ) -> (Vec<Vec<(String, Option<u32>)>>, usize, bool) {
        const MAX: usize = MainView::CODE_PREVIEW_MAX_LINES;
        if let Some(text) = &state.preview_text {
            if !text.is_empty() && state.preview_highlighted_src == *text {
                let total = state.preview_highlighted.len();
                let shown = total.min(MAX);
                return (
                    state.preview_highlighted[..shown].to_vec(),
                    total,
                    total > MAX,
                );
            }
            let all = Self::highlight_code(text);
            let total = all.len();
            let shown = total.min(MAX);
            (all[..shown].to_vec(), total, total > MAX)
        } else {
            (Vec::new(), 0, false)
        }
    }

    fn render_code_preview(
        lines: &[Vec<(String, Option<u32>)>],
        language: &str,
        total_lines: usize,
        truncated: bool,
    ) -> impl IntoElement {

        div()
            .flex_col()
            .w_full()
            .bg(rgba(0xFFFFFF07u32))
            .border(px(1.0))
            .border_color(rgba(UiConstants::CARD_STROKE_RGBA))
            .rounded(px(8.0))
            .overflow_hidden()
            .child(
                div()
                    .w_full()
                    .h(px(28.0))
                    .px(px(10.0))
                    .bg(rgba(0xFFFFFF08u32))
                    .border_b(px(1.0))
                    .border_color(rgba(UiConstants::CARD_STROKE_RGBA))
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(px(11.0))
                            .font_weight(FontWeight(600.0))
                            .text_color(rgb(0x60CDFF))
                            .child(language.to_uppercase()),
                    )
                    .child(
                        div()
                            .text_size(px(10.5))
                            .text_color(rgba(UiConstants::TEXT_MUTED_RGBA))
                            .child(if truncated {
                                format!("{} lines (showing first {})", total_lines, lines.len())
                            } else {
                                format!("{} lines", total_lines)
                            }),
                    ),
            )
            .child(
                div()
                    .p(px(8.0))
                    .flex_col()
                    .font_family(
                        "ui-monospace, SFMono-Regular, 'Cascadia Code', Consolas, monospace",
                    )
                    .text_size(px(11.0))
                    .children(lines.iter().enumerate().map(|(i, tokens)| {
                        div()
                            .flex()
                            .flex_row()
                            .gap(px(8.0))
                            .w_full()
                            .child(
                                div()
                                    .text_color(rgba(UiConstants::TEXT_MUTED_RGBA))
                                    .min_w(px(24.0))
                                    .text_right()
                                    .child(format!("{:>3}", i + 1)),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .flex_wrap()
                                    .children(tokens.iter().map(|(token, color_opt)| {
                                        let mut el = div().child(token.clone());
                                        if let Some(color) = color_opt {
                                            el = el.text_color(rgba(*color));
                                        } else {
                                            el = el.text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA));
                                        }
                                        el.into_any_element()
                                    })),
                            )
                            .into_any_element()
                    })),
            )
    }

    pub(crate) fn highlight_code(text: &str) -> Vec<Vec<(String, Option<u32>)>> {
        let keywords: &[(&str, u32)] = &[
            ("fn", 0x569CD6FF),
            ("let", 0x569CD6FF),
            ("mut", 0x569CD6FF),
            ("const", 0x569CD6FF),
            ("pub", 0x569CD6FF),
            ("use", 0x569CD6FF),
            ("mod", 0x569CD6FF),
            ("struct", 0x569CD6FF),
            ("enum", 0x569CD6FF),
            ("impl", 0x569CD6FF),
            ("trait", 0x569CD6FF),
            ("type", 0x569CD6FF),
            ("return", 0xC586C0FF),
            ("if", 0xC586C0FF),
            ("else", 0xC586C0FF),
            ("for", 0xC586C0FF),
            ("while", 0xC586C0FF),
            ("loop", 0xC586C0FF),
            ("match", 0xC586C0FF),
            ("break", 0xC586C0FF),
            ("continue", 0xC586C0FF),
            ("self", 0x569CD6FF),
            ("Self", 0x4EC9B0FF),
            ("super", 0x569CD6FF),
            ("where", 0x569CD6FF),
            ("async", 0x569CD6FF),
            ("await", 0xC586C0FF),
            ("unsafe", 0x569CD6FF),
            ("ref", 0x569CD6FF),
            ("static", 0x569CD6FF),
            ("dyn", 0x569CD6FF),
            ("move", 0x569CD6FF),
            ("true", 0x569CD6FF),
            ("false", 0x569CD6FF),
            ("Some", 0x4EC9B0FF),
            ("None", 0x4EC9B0FF),
            ("Ok", 0x4EC9B0FF),
            ("Err", 0x4EC9B0FF),
            ("as", 0x569CD6FF),
            ("in", 0x569CD6FF),
            ("def", 0x569CD6FF),
            ("class", 0x569CD6FF),
            ("import", 0xC586C0FF),
            ("from", 0xC586C0FF),
            ("function", 0x569CD6FF),
            ("export", 0xC586C0FF),
            ("interface", 0x569CD6FF),
        ];

        let type_keywords: &[(&str, u32)] = &[
            ("String", 0x4EC9B0FF),
            ("str", 0x4EC9B0FF),
            ("i32", 0x4EC9B0FF),
            ("i64", 0x4EC9B0FF),
            ("u32", 0x4EC9B0FF),
            ("u64", 0x4EC9B0FF),
            ("f32", 0x4EC9B0FF),
            ("f64", 0x4EC9B0FF),
            ("bool", 0x4EC9B0FF),
            ("usize", 0x4EC9B0FF),
            ("Vec", 0x4EC9B0FF),
            ("Option", 0x4EC9B0FF),
            ("Result", 0x4EC9B0FF),
            ("HashMap", 0x4EC9B0FF),
            ("HashSet", 0x4EC9B0FF),
            ("Arc", 0x4EC9B0FF),
            ("Mutex", 0x4EC9B0FF),
        ];

        let mut result = Vec::new();
        for line in text.lines() {
            let mut tokens: Vec<(String, Option<u32>)> = Vec::new();
            let mut remaining = line;

            while !remaining.is_empty() {
                if remaining.starts_with('"') || remaining.starts_with('\'') {
                    let quote = remaining.chars().next().unwrap();
                    let end = remaining[1..]
                        .find(quote)
                        .map(|i| i + 2)
                        .unwrap_or(remaining.len());
                    tokens.push((remaining[..end].to_string(), Some(0xCE9178FFu32)));
                    remaining = &remaining[end..];
                    continue;
                }
                if remaining.starts_with("//") || remaining.starts_with('#') {
                    tokens.push((remaining.to_string(), Some(0x6A9955FFu32)));
                    break;
                }
                if remaining.starts_with(|c: char| c.is_ascii_digit()) {
                    let end = remaining
                        .find(|c: char| !c.is_ascii_digit() && c != '.' && c != 'x')
                        .unwrap_or(remaining.len());
                    tokens.push((remaining[..end].to_string(), Some(0xB5CEA8FFu32)));
                    remaining = &remaining[end..];
                    continue;
                }

                let mut matched = false;
                for &(kw, color) in keywords.iter().chain(type_keywords.iter()) {
                    if remaining.starts_with(kw) {
                        let after = &remaining[kw.len()..];
                        let is_word_boundary = after.is_empty()
                            || !after.starts_with(|c: char| c.is_alphanumeric() || c == '_');
                        if is_word_boundary {
                            tokens.push((kw.to_string(), Some(color)));
                            remaining = after;
                            matched = true;
                            break;
                        }
                    }
                }
                if matched {
                    continue;
                }

                let end = remaining
                    .find(|c: char| !c.is_alphanumeric() && c != '_')
                    .unwrap_or(remaining.len());
                if end > 0 {
                    tokens.push((remaining[..end].to_string(), None));
                    remaining = &remaining[end..];
                } else {
                    let step = remaining.chars().next().map(|c| c.len_utf8()).unwrap_or(1);
                    tokens.push((remaining[..step].to_string(), None));
                    remaining = &remaining[step..];
                }
            }
            result.push(tokens);
        }
        result
    }

    fn render_html_preview(text: &str) -> impl IntoElement {
        let stripped = Self::strip_html_tags(text);
        div().flex_col().w_full().child(
            div()
                .bg(rgba(0xFFFFFF07u32))
                .border(px(1.0))
                .border_color(rgba(UiConstants::CARD_STROKE_RGBA))
                .rounded(px(8.0))
                .p(px(10.0))
                .child(
                    div()
                        .text_size(px(11.5))
                        .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                        .child(stripped),
                ),
        )
    }

    fn strip_html_tags(html: &str) -> String {
        let mut result = String::new();
        let mut in_tag = false;
        for c in html.chars() {
            match c {
                '<' => in_tag = true,
                '>' => in_tag = false,
                _ if !in_tag => result.push(c),
                _ => {}
            }
        }
        result.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    fn render_rtf_preview(text: &str) -> impl IntoElement {
        let stripped = Self::strip_rtf(text);
        div()
            .flex_col()
            .w_full()
            .bg(rgba(0xFFFFFF07u32))
            .border(px(1.0))
            .border_color(rgba(UiConstants::CARD_STROKE_RGBA))
            .rounded(px(8.0))
            .p(px(10.0))
            .child(
                div()
                    .text_size(px(11.5))
                    .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                    .child(stripped),
            )
    }

    fn strip_rtf(rtf: &str) -> String {
        let mut result = String::new();
        let mut in_control = false;
        let mut brace_depth = 0u32;
        let mut chars = rtf.chars().peekable();

        while let Some(c) = chars.next() {
            match c {
                '{' => brace_depth += 1,
                '}' => brace_depth = brace_depth.saturating_sub(1),
                '\\' => in_control = true,
                '\r' | '\n' => {
                    if brace_depth == 0 {
                        result.push(c);
                    }
                }
                _ => {
                    if in_control {
                        if c.is_ascii_alphabetic() {
                            while let Some(&next) = chars.peek() {
                                if next.is_ascii_alphabetic() {
                                    chars.next();
                                } else {
                                    break;
                                }
                            }
                            if let Some(&' ') = chars.peek() {
                                chars.next();
                            }
                        }
                        in_control = false;
                    } else if brace_depth == 0 {
                        result.push(c);
                    }
                }
            }
        }
        result.trim().to_string()
    }

    fn render_media_preview(
        info: &MediaPreviewInfo,
        media_type: &str,
        text: &str,
        item: &FileItem,
    ) -> impl IntoElement {
        let icon = if media_type == "Audio" {
            "\u{EC4F}"
        } else {
            "\u{E714}"
        };
        let format_line = info.format.as_deref().unwrap_or("Unknown");
        let duration_str = info
            .duration_secs
            .map(|d| {
                let mins = (d / 60.0) as u64;
                let secs = (d % 60.0) as u64;
                format!("{}:{:02}", mins, secs)
            })
            .unwrap_or_else(|| "0:00".to_string());

        let codec_str = info.codec.as_deref().unwrap_or("Standard");
        let bitrate_str = info
            .bitrate_kbps
            .map(|b| format!("{} kbps", b))
            .unwrap_or_else(|| "Variable".to_string());

        let mut meta_children: Vec<AnyElement> = vec![
            info_row("Format", format_line).into_any_element(),
            info_row("Duration", &duration_str).into_any_element(),
            info_row("Codec", codec_str).into_any_element(),
            info_row("Bitrate", &bitrate_str).into_any_element(),
        ];

        if let Some(sr) = info.sample_rate {
            meta_children.push(info_row("Sample Rate", &format!("{} Hz", sr)).into_any_element());
        }
        if let Some(ch) = info.channels {
            meta_children.push(info_row("Channels", &format!("{}", ch)).into_any_element());
        }
        if let Some(w) = info.width {
            if let Some(h) = info.height {
                meta_children
                    .push(info_row("Resolution", &format!("{} × {}", w, h)).into_any_element());
            }
        }

        div()
            .flex_col()
            .gap(px(10.0))
            .w_full()
            .child(
                div()
                    .w_full()
                    .p(px(14.0))
                    .rounded(px(8.0))
                    .bg(rgba(0xFFFFFF07u32))
                    .border(px(1.0))
                    .border_color(rgba(UiConstants::CARD_STROKE_RGBA))
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(10.0))
                    .child(
                        div()
                            .w(px(56.0))
                            .h(px(56.0))
                            .rounded(px(12.0))
                            .bg(rgba(0x60CDFF22u32))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                div()
                                    .text_size(px(28.0))
                                    .font_family(UiConstants::FONT_ICONS)
                                    .text_color(rgb(0x60CDFF))
                                    .child(icon),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(13.0))
                            .font_weight(FontWeight(600.0))
                            .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                            .text_center()
                            .child(item.name.clone()),
                    )
                    .child(
                        // Media Transport Bar
                        div()
                            .w_full()
                            .px(px(10.0))
                            .py(px(8.0))
                            .rounded(px(6.0))
                            .bg(rgba(0x00000033u32))
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(8.0))
                            .child(
                                div()
                                    .w(px(26.0))
                                    .h(px(26.0))
                                    .rounded(px(13.0))
                                    .bg(rgba(UiConstants::ACCENT_RGBA))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(
                                        div()
                                            .text_size(px(11.0))
                                            .font_family(UiConstants::FONT_ICONS)
                                            .text_color(rgb(0xFFFFFF))
                                            .child("\u{E768}"),
                                    ),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .h(px(4.0))
                                    .rounded(px(2.0))
                                    .bg(rgba(0xFFFFFF22u32))
                                    .child(
                                        div()
                                            .w(px(48.0))
                                            .h_full()
                                            .rounded(px(2.0))
                                            .bg(rgb(0x60CDFF)),
                                    ),
                            )
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                    .child(duration_str),
                            ),
                    ),
            )
            .child(
                div()
                    .w_full()
                    .p(px(12.0))
                    .rounded(px(8.0))
                    .bg(rgba(0xFFFFFF07u32))
                    .border(px(1.0))
                    .border_color(rgba(UiConstants::CARD_STROKE_RGBA))
                    .flex()
                    .flex_col()
                    .gap(px(6.0))
                    .child(section_header(&format!("{} Stream Info", media_type)))
                    .children(meta_children),
            )
            .child(
                div()
                    .p(px(10.0))
                    .w_full()
                    .bg(rgba(0xFFFFFF05u32))
                    .rounded(px(6.0))
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                            .child(text.to_string()),
                    ),
            )
    }

    fn render_pdf_preview(_info: &PdfPreviewInfo, text: &str) -> impl IntoElement {
        div()
            .flex_col()
            .items_center()
            .gap(px(10.0))
            .w_full()
            .child(
                div()
                    .w(px(64.0))
                    .h(px(64.0))
                    .bg(rgba(0xE8112322u32))
                    .rounded(px(10.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .text_size(px(30.0))
                            .font_family(UiConstants::FONT_ICONS)
                            .text_color(rgb(0xFF8080))
                            .child("\u{EA90}"),
                    ),
            )
            .child(
                div()
                    .p(px(12.0))
                    .w_full()
                    .bg(rgba(0xFFFFFF07u32))
                    .border(px(1.0))
                    .border_color(rgba(UiConstants::CARD_STROKE_RGBA))
                    .rounded(px(8.0))
                    .child(
                        div()
                            .text_size(px(11.5))
                            .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                            .child(text.to_string()),
                    ),
            )
    }

    fn render_archive_preview(_info: &ArchivePreviewInfo, text: &str) -> impl IntoElement {
        div()
            .flex_col()
            .items_center()
            .gap(px(10.0))
            .w_full()
            .child(
                div()
                    .w(px(64.0))
                    .h(px(64.0))
                    .bg(rgba(0xFCCB4522u32))
                    .rounded(px(10.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .text_size(px(30.0))
                            .font_family(UiConstants::FONT_ICONS)
                            .text_color(rgb(0xFCCB45))
                            .child("\u{F012}"),
                    ),
            )
            .child(
                div()
                    .p(px(10.0))
                    .w_full()
                    .max_h(px(340.0))
                    .bg(rgba(0xFFFFFF07u32))
                    .border(px(1.0))
                    .border_color(rgba(UiConstants::CARD_STROKE_RGBA))
                    .rounded(px(8.0))
                    .child(
                        div()
                            .text_size(px(11.0))
                            .font_family("ui-monospace, SFMono-Regular, 'Cascadia Code', Consolas, monospace")
                            .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                            .child(text.to_string()),
                    ),
            )
    }

    fn render_folder_preview(items: &[FolderPreviewItem]) -> impl IntoElement {
        let display_items = if items.len() > 12 {
            &items[..12]
        } else {
            items
        };

        div()
            .flex_col()
            .items_center()
            .gap(px(10.0))
            .w_full()
            .child(
                div()
                    .w(px(56.0))
                    .h(px(56.0))
                    .bg(rgba(0xFCCB4522u32))
                    .rounded(px(10.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .text_size(px(28.0))
                            .font_family(UiConstants::FONT_ICONS)
                            .text_color(rgb(0xFCCB45))
                            .child("\u{E8B7}"),
                    ),
            )
            .child(
                div()
                    .text_size(px(13.0))
                    .font_weight(FontWeight(600.0))
                    .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                    .child(format!("Folder Preview ({} items)", items.len())),
            )
            .child(
                div()
                    .w_full()
                    .p(px(10.0))
                    .rounded(px(8.0))
                    .bg(rgba(0xFFFFFF07u32))
                    .border(px(1.0))
                    .border_color(rgba(UiConstants::CARD_STROKE_RGBA))
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .gap(px(8.0))
                    .children(display_items.iter().map(|entry| {
                        let icon = if entry.is_dir { "\u{E8B7}" } else { "\u{E8A5}" };
                        div()
                            .flex_col()
                            .items_center()
                            .gap(px(3.0))
                            .w(px(64.0))
                            .child(
                                div()
                                    .w(px(40.0))
                                    .h(px(40.0))
                                    .bg(rgba(0xFFFFFF0Eu32))
                                    .rounded(px(6.0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(
                                        div()
                                            .text_size(px(18.0))
                                            .font_family(UiConstants::FONT_ICONS)
                                            .text_color(if entry.is_dir {
                                                rgb(0xFCCB45)
                                            } else {
                                                rgb(0x60CDFF)
                                            })
                                            .child(icon),
                                    ),
                            )
                            .child(
                                div()
                                    .text_size(px(10.0))
                                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                    .overflow_hidden()
                                    .whitespace_nowrap()
                                    .text_ellipsis()
                                    .max_w(px(62.0))
                                    .text_center()
                                    .child(entry.name.clone()),
                            )
                            .child(if entry.is_dir {
                                div()
                                    .text_size(px(10.0))
                                    .text_color(rgba(UiConstants::TEXT_MUTED_RGBA))
                                    .text_center()
                                    .child(match entry.child_count {
                                        Some(n) if n > 100 => "100+ items".to_string(),
                                        Some(n) => format!(
                                            "{} {}",
                                            n,
                                            if n == 1 { "item" } else { "items" }
                                        ),
                                        None => "Folder".to_string(),
                                    })
                                    .into_any_element()
                            } else {
                                div().into_any_element()
                            })
                            .into_any_element()
                    })),
            )
    }
}

#[cfg(test)]
mod code_preview_tests {
    use super::MainView;

    fn plain_text(lines: &[Vec<(String, Option<u32>)>]) -> Vec<String> {
        lines
            .iter()
            .map(|tokens| {
                tokens
                    .iter()
                    .map(|(t, _)| t.as_str())
                    .collect::<String>()
            })
            .collect()
    }

    #[test]
    fn identifiers_stay_whole_words() {
        let lines = MainView::highlight_code("import os\nimport random\n");
        assert_eq!(plain_text(&lines), vec!["import os", "import random"]);
        // "os" / "random" must each be a single token, not per-char splits.
        let first: Vec<&str> = lines[0].iter().map(|(t, _)| t.as_str()).collect();
        assert!(first.contains(&"os"), "tokens were: {:?}", first);
        // "import", " ", "random" — "random" must be one piece, not chars.
        assert!(lines[1].iter().any(|(t, _)| t == "random"));
    }

    #[test]
    fn keywords_keep_their_color() {
        let lines = MainView::highlight_code("import os\n");
        let import_tok = lines[0]
            .iter()
            .find(|(t, _)| t == "import")
            .expect("import token missing");
        assert!(import_tok.1.is_some(), "keyword lost its color");
        let os_tok = lines[0]
            .iter()
            .find(|(t, _)| t == "os")
            .expect("os token missing");
        assert!(os_tok.1.is_none(), "plain identifier wrongly colored");
    }

    #[test]
    fn roundtrip_preserves_source() {
        let src = "import torch as t\nx = 42  # comment\n";
        let lines = MainView::highlight_code(src);
        let joined = plain_text(&lines).join("\n");
        assert_eq!(joined, src.trim_end_matches('\n'));
    }
}

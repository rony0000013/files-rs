// Preview rendering methods for MainView
// Extracted from main_view.rs to reduce file size

use crate::domain::metadata::FileItem;
use crate::storage::storage::FileType;
use crate::ui::constants::UiConstants;
use gpui::*;

use super::helpers::*;
use super::main_view::MainView;
use super::types::*;

impl MainView {
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
                if let Some(text) = &state.preview_text {
                    Self::render_code_preview(text, language).into_any_element()
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
                    // Show shell preview text for text-based files
                    div()
                        .flex_col()
                        .w_full()
                        .max_h(px(400.0))
                        .child(
                            div()
                                .child(div().text_size(px(13.0)).font_weight(FontWeight(600.0)).mb(px(6.0))
                                    .child(format!("Shell Preview ({:?})", info.handler)))
                                .child(
                                    div()
                                        .bg(rgba(0xFFFFFF05u32))
                                        .rounded(px(4.0))
                                        .p(px(8.0))
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
            "Folder"
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

    /// Rich Details pane — shows extended metadata, git status stub, tags

    pub(crate) fn render_details_pane(state: &MainViewState, item: &FileItem) -> impl IntoElement {
        let type_label = if matches!(item.file_type, FileType::Directory) {
            "Folder"
        } else {
            "File"
        };
        let ext = item.name.rsplit('.').next().unwrap_or("").to_uppercase();
        let size_str = format_size(item.size);
        let is_image = matches!(
            ext.as_str(),
            "JPG" | "JPEG" | "PNG" | "WEBP" | "BMP" | "GIF" | "TIFF"
        );

        div()
            .flex_col()
            .gap(px(10.0))
            .w_full()
            // Thumbnail / large icon
            .child(
                div()
                    .w_full()
                    .h(px(120.0))
                    .bg(rgba(0xFFFFFF08u32))
                    .rounded(px(6.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(if is_image {
                        img(item.path.as_path().to_path_buf())
                            .w_full()
                            .h_full()
                            .into_any_element()
                    } else {
                        div()
                            .text_size(px(40.0))
                            .font_family(UiConstants::FONT_MDL2)
                            .text_color(rgba(UiConstants::ACCENT_RGBA))
                            .child(if matches!(item.file_type, FileType::Directory) {
                                "\u{E8B7}"
                            } else {
                                "\u{E8A5}"
                            })
                            .into_any_element()
                    }),
            )
            // File name
            .child(
                div()
                    .text_size(px(14.0))
                    .font_weight(FontWeight(600.0))
                    .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                    .text_center()
                    .child(item.name.clone()),
            )
            // Separator
            .child(
                div()
                    .w_full()
                    .h(px(1.0))
                    .bg(rgba(UiConstants::DIVIDER_RGBA)),
            )
            // Properties section
            .child(
                div()
                    .flex_col()
                    .gap(px(6.0))
                    .child(section_header("Properties"))
                    .child(info_row("Type", type_label))
                    .child(info_row("Size", &size_str))
                    .child(info_row("Format", if ext.is_empty() { "—" } else { &ext })),
            )
            // Shortcut section
            .children(if ext == "LNK" {
                vec![
                    div()
                        .w_full()
                        .h(px(1.0))
                        .bg(rgba(UiConstants::DIVIDER_RGBA))
                        .into_any_element(),
                    div()
                        .flex_col()
                        .gap(px(6.0))
                        .child(section_header("Shortcut"))
                        .child(info_row("Target", "C:\\Program Files\\..."))
                        .child(info_row("Arguments", "--preview"))
                        .into_any_element(),
                ]
            } else {
                vec![]
            })
            // Tags section
            .child(
                div()
                    .w_full()
                    .h(px(1.0))
                    .bg(rgba(UiConstants::DIVIDER_RGBA)),
            )
            .child(
                div()
                    .flex_col()
                    .gap(px(6.0))
                    .child(section_header("Tags"))
                    .children(if state.details_cache.tags.is_empty() {
                        vec![div()
                            .text_size(px(12.0))
                            .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                            .child("No tags — right-click to add")
                            .into_any_element()]
                    } else {
                        state
                            .details_cache
                            .tags
                            .iter()
                            .map(|tag| {
                                div()
                                    .flex()
                                    .px(px(6.0))
                                    .py(px(2.0))
                                    .bg(rgba(0xFFFFFF1Au32))
                                    .rounded(px(4.0))
                                    .text_size(px(11.0))
                                    .text_color(rgba(UiConstants::ACCENT_RGBA))
                                    .child(tag.clone())
                                    .into_any_element()
                            })
                            .collect()
                    }),
            )
            // EXIF/Metadata section (for images)
            .children(
                if !state.details_cache.exif_dimensions.is_none()
                    || state.details_cache.audio_duration.is_some()
                {
                    let mut children = vec![
                        div()
                            .w_full()
                            .h(px(1.0))
                            .bg(rgba(UiConstants::DIVIDER_RGBA))
                            .into_any_element(),
                        div()
                            .flex_col()
                            .gap(px(6.0))
                            .child(section_header("Media Info"))
                            .into_any_element(),
                    ];

                    if let Some(ref dims) = state.details_cache.exif_dimensions {
                        children.push(info_row("Dimensions", dims).into_any_element());
                    }
                    if let Some(ref camera) = state.details_cache.exif_camera {
                        children.push(info_row("Camera", camera).into_any_element());
                    }
                    if let Some(ref date) = state.details_cache.exif_date {
                        children.push(info_row("Date Taken", date).into_any_element());
                    }
                    if let Some(ref duration) = state.details_cache.audio_duration {
                        children.push(info_row("Duration", duration).into_any_element());
                    }
                    if let Some(ref artist) = state.details_cache.audio_artist {
                        children.push(info_row("Artist", artist).into_any_element());
                    }
                    if let Some(ref album) = state.details_cache.audio_album {
                        children.push(info_row("Album", album).into_any_element());
                    }
                    if let Some(ref res) = state.details_cache.video_resolution {
                        children.push(info_row("Resolution", res).into_any_element());
                    }

                    children
                } else {
                    vec![]
                },
            )
            // Git section — shows last commit info and file status
            .child(
                div()
                    .w_full()
                    .h(px(1.0))
                    .bg(rgba(UiConstants::DIVIDER_RGBA)),
            )
            .children({
                let mut git_children: Vec<gpui::AnyElement> = Vec::new();
                git_children.push(
                    div()
                        .flex_col()
                        .gap(px(6.0))
                        .child(section_header("Git"))
                        .into_any_element(),
                );

                // Try to get last commit info for this file
                let commit_info =
                    crate::domain::git_service::GitService::get_last_commit(&item.path);
                if let Some(info) = commit_info {
                    git_children.push(info_row("Author", &info.author).into_any_element());
                    git_children.push(info_row("Date", &info.date).into_any_element());
                    git_children.push(info_row("Message", &info.message).into_any_element());
                } else {
                    git_children.push(
                        div()
                            .text_size(px(12.0))
                            .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                            .child(if state.side_pane != SidePaneMode::None {
                                "Not in a repository"
                            } else {
                                ""
                            })
                            .into_any_element(),
                    );
                }

                git_children
            })
            // Text preview section
            .children(if let Some(text) = &state.preview_text {
                vec![
                    div()
                        .w_full()
                        .h(px(1.0))
                        .bg(rgba(UiConstants::DIVIDER_RGBA))
                        .into_any_element(),
                    div()
                        .flex_col()
                        .gap(px(6.0))
                        .child(section_header("Preview"))
                        .child(
                            div()
                                .bg(rgba(0xFFFFFF05u32))
                                .p(px(8.0))
                                .rounded(px(4.0))
                                .child(
                                    div()
                                        .text_size(px(11.0))
                                        .font_family(UiConstants::FONT_MAIN)
                                        .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                                        .child(text.clone()),
                                ),
                        )
                        .into_any_element(),
                ]
            } else {
                vec![]
            })
    }

    /// Render the shelf pane (staging area for bulk operations)

    fn render_default_preview(item: &FileItem, state: &MainViewState) -> impl IntoElement {
        let ext = get_extension(&item.name);
        let is_image = matches!(
            ext.as_str(),
            "jpg" | "jpeg" | "png" | "gif" | "bmp" | "webp" | "ico" | "tiff" | "tif" | "svg"
        );

        div()
            .flex_col()
            .items_center()
            .gap(px(12.0))
            .w_full()
            .child(
                div()
                    .w(px(64.0))
                    .h(px(64.0))
                    .bg(rgba(UiConstants::BG_BADGE_RGBA))
                    .rounded(px(8.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(if is_image {
                        img(item.path.as_path().to_path_buf())
                            .w(px(64.0))
                            .h(px(64.0))
                            .rounded(px(8.0))
                            .into_any_element()
                    } else {
                        div()
                            .text_size(px(32.0))
                            .font_family(UiConstants::FONT_MDL2)
                            .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                            .child(if matches!(item.file_type, FileType::Directory) {
                                "\u{E8B7}"
                            } else {
                                "\u{E7C3}"
                            })
                            .into_any_element()
                    }),
            )
            .child(
                div()
                    .text_size(px(13.0))
                    .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                    .text_center()
                    .overflow_hidden()
                    .max_w(px(250.0))
                    .child(item.name.clone()),
            )
            .child(if let Some(text) = &state.preview_text {
                div()
                    .mt(px(8.0))
                    .p(px(8.0))
                    .w_full()
                    .max_h(px(300.0))
                    .bg(rgba(0xFFFFFF05u32))
                    .rounded(px(4.0))
                    .overflow_hidden()
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                            .child(text.clone()),
                    )
                    .into_any_element()
            } else {
                div().into_any_element()
            })
    }

    /// Render a Markdown preview with basic formatting

    fn render_markdown_preview(text: &str) -> impl IntoElement {
        let mut children: Vec<AnyElement> = Vec::new();
        let mut in_code_block = false;
        let mut code_block_text = String::new();

        for line in text.lines() {
            if line.starts_with("```") {
                if in_code_block {
                    // End code block
                    children.push(
                        div()
                            .font_family("ui-monospace, SFMono-Regular, 'Cascadia Code', Consolas, monospace")
                            .text_size(px(11.0))
                            .text_color(rgba(0x00FFD700u32)) // gold-ish
                            .bg(rgba(0xFFFFFF10u32))
                            .p(px(6.0))
                            .rounded(px(4.0))
                            .w_full()
                            .child(code_block_text.clone())
                            .into_any_element()
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

            // Headers
            if trimmed.starts_with("### ") {
                let content = &trimmed[4..];
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
            } else if trimmed.starts_with("## ") {
                let content = &trimmed[3..];
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
            } else if trimmed.starts_with("# ") {
                let content = &trimmed[2..];
                children.push(
                    div()
                        .font_weight(FontWeight(700.0))
                        .text_size(px(18.0))
                        .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                        .mt(px(10.0))
                        .mb(px(4.0))
                        .child(content.to_string())
                        .into_any_element(),
                );
            } else if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
                // List items
                let content = &trimmed[2..];
                children.push(
                    div()
                        .flex()
                        .flex_row()
                        .gap(px(4.0))
                        .px(px(8.0))
                        .text_size(px(11.0))
                        .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                        .child(div().child("\u{2022} ")) // bullet
                        .child(div().child(Self::parse_inline_markdown(content)))
                        .into_any_element(),
                );
            } else if trimmed.is_empty() {
                children.push(div().h(px(6.0)).into_any_element());
            } else {
                // Regular paragraph with inline formatting
                children.push(
                    div()
                        .text_size(px(11.0))
                        .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                        .py(px(1.0))
                        .child(Self::parse_inline_markdown(line))
                        .into_any_element(),
                );
            }
        }

        // Flush remaining code block
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
            .max_h(px(400.0))
            .children(children)
    }

    /// Parse inline markdown formatting (bold, italic, code, links)

    fn parse_inline_markdown(text: &str) -> impl IntoElement {
        // For simplicity, we return a div with basic inline formatting detection.
        // Handle **bold**, *italic*, `inline code`, [links](url)
        let mut children: Vec<AnyElement> = Vec::new();
        let mut remaining = text;

        while !remaining.is_empty() {
            // Bold: **text**
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

            // Inline code: `code`
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
                            .into_any_element()
                    );
                    remaining = &after[end + 1..];
                    continue;
                }
            }

            // No more formatting - add remaining text
            children.push(div().child(remaining.to_string()).into_any_element());
            break;
        }

        div().flex_row().flex_wrap().children(children)
    }

    /// Render code with syntax highlighting

    fn render_code_preview(text: &str, _language: &str) -> impl IntoElement {
        let highlighted = Self::highlight_code(text);
        let line_count = text.lines().count();

        div()
            .flex_col()
            .w_full()
            .max_h(px(400.0))
            .bg(rgba(0xFFFFFF08u32))
            .rounded(px(4.0))
            .p(px(8.0))
            .child(
                div()
                    .flex_col()
                    .font_family(
                        "ui-monospace, SFMono-Regular, 'Cascadia Code', Consolas, monospace",
                    )
                    .text_size(px(11.0))
                    .children(highlighted.into_iter().enumerate().map(|(i, tokens)| {
                        // Line number + highlighted tokens
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
                                    .flex_row()
                                    .flex_wrap()
                                    .children(tokens.into_iter().map(|(token, color_opt)| {
                                        let mut el = div().child(token);
                                        if let Some(color) = color_opt {
                                            el = el.text_color(color);
                                        } else {
                                            el =
                                                el.text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA));
                                        }
                                        el.into_any_element()
                                    })),
                            )
                            .into_any_element()
                    })),
            )
    }

    /// Simple keyword-based syntax highlighter
    /// Returns a list of lines, each containing (token, optional_color) pairs

    fn highlight_code(text: &str) -> Vec<Vec<(String, Option<gpui::Rgba>)>> {
        // Language-agnostic keyword highlighting based on common patterns
        let keywords: &[(&str, u32)] = &[
            // Rust, C, C++, Java, JavaScript, TypeScript, etc.
            ("fn", 0x005695FF), // blue
            ("let", 0x005695FF),
            ("mut", 0x005695FF),
            ("const", 0x005695FF),
            ("pub", 0x005695FF),
            ("use", 0x005695FF),
            ("mod", 0x005695FF),
            ("struct", 0x005695FF),
            ("enum", 0x005695FF),
            ("impl", 0x005695FF),
            ("trait", 0x005695FF),
            ("type", 0x005695FF),
            ("fn ", 0x005695FF),
            ("return", 0x005695FF),
            ("if", 0x005695FF),
            ("else", 0x005695FF),
            ("for", 0x005695FF),
            ("while", 0x005695FF),
            ("loop", 0x005695FF),
            ("match", 0x005695FF),
            ("break", 0x005695FF),
            ("continue", 0x005695FF),
            ("self", 0x005695FF),
            ("Self", 0x005695FF),
            ("super", 0x005695FF),
            ("where", 0x005695FF),
            ("async", 0x005695FF),
            ("await", 0x005695FF),
            ("unsafe", 0x005695FF),
            ("ref", 0x005695FF),
            ("static", 0x005695FF),
            ("dyn", 0x005695FF),
            ("move", 0x005695FF),
            ("true", 0x005695FF),
            ("false", 0x005695FF),
            ("Some", 0x005695FF),
            ("None", 0x005695FF),
            ("Ok", 0x005695FF),
            ("Err", 0x005695FF),
            ("as", 0x005695FF),
            ("in", 0x005695FF),
            ("is", 0x005695FF),
            // Python
            ("def", 0x005695FF),
            ("class", 0x005695FF),
            ("import", 0x005695FF),
            ("from", 0x005695FF),
            ("with", 0x005695FF),
            ("as ", 0x005695FF),
            ("pass", 0x005695FF),
            ("yield", 0x005695FF),
            ("lambda", 0x005695FF),
            ("print", 0x005695FF),
            ("range", 0x005695FF),
            ("len", 0x005695FF),
            ("not", 0x005695FF),
            ("and", 0x005695FF),
            ("or", 0x005695FF),
            ("try", 0x005695FF),
            ("except", 0x005695FF),
            ("finally", 0x005695FF),
            ("raise", 0x005695FF),
            // JS/TS
            ("function", 0x005695FF),
            ("var", 0x005695FF),
            ("typeof", 0x005695FF),
            ("new", 0x005695FF),
            ("this", 0x005695FF),
            ("null", 0x005695FF),
            ("undefined", 0x005695FF),
            ("export", 0x005695FF),
            ("import ", 0x005695FF),
            ("from ", 0x005695FF),
            ("default", 0x005695FF),
            ("extends", 0x005695FF),
            ("implements", 0x005695FF),
            ("interface", 0x005695FF),
            ("typeof", 0x005695FF),
            ("instanceof", 0x005695FF),
            ("throw", 0x005695FF),
            ("catch", 0x005695FF),
            // String/char literals
        ];

        let type_keywords: &[(&str, u32)] = &[
            ("String", 0x004EC9B0FF), // teal
            ("str", 0x004EC9B0FF),
            ("i32", 0x004EC9B0FF),
            ("i64", 0x004EC9B0FF),
            ("u32", 0x004EC9B0FF),
            ("u64", 0x004EC9B0FF),
            ("f32", 0x004EC9B0FF),
            ("f64", 0x004EC9B0FF),
            ("bool", 0x004EC9B0FF),
            ("char", 0x004EC9B0FF),
            ("usize", 0x004EC9B0FF),
            ("Vec", 0x004EC9B0FF),
            ("Option", 0x004EC9B0FF),
            ("Result", 0x004EC9B0FF),
            ("HashMap", 0x004EC9B0FF),
            ("HashSet", 0x004EC9B0FF),
            ("Box", 0x004EC9B0FF),
            ("Arc", 0x004EC9B0FF),
            ("Mutex", 0x004EC9B0FF),
            ("Rc", 0x004EC9B0FF),
            ("Cell", 0x004EC9B0FF),
            ("RefCell", 0x004EC9B0FF),
            ("u8", 0x004EC9B0FF),
            ("u16", 0x004EC9B0FF),
            ("i8", 0x004EC9B0FF),
            ("i16", 0x004EC9B0FF),
            ("int", 0x004EC9B0FF),
            ("float", 0x004EC9B0FF),
            ("double", 0x004EC9B0FF),
            ("char", 0x004EC9B0FF),
            ("void", 0x004EC9B0FF),
            ("boolean", 0x004EC9B0FF),
            ("number", 0x004EC9B0FF),
            ("string", 0x004EC9B0FF),
            ("Promise", 0x004EC9B0FF),
            ("Array", 0x004EC9B0FF),
            ("Map", 0x004EC9B0FF),
            ("Set", 0x004EC9B0FF),
            ("Error", 0x004EC9B0FF),
            ("Date", 0x004EC9B0FF),
            ("RegExp", 0x004EC9B0FF),
        ];

        let mut result = Vec::new();

        for line in text.lines() {
            let mut tokens: Vec<(String, Option<gpui::Rgba>)> = Vec::new();
            let mut remaining = line;

            while !remaining.is_empty() {
                // Try to match a string literal first (single or double quoted)
                if remaining.starts_with('"') || remaining.starts_with('\'') {
                    let quote = remaining.chars().next().unwrap();
                    let end = remaining[1..]
                        .find(quote)
                        .map(|i| i + 2)
                        .unwrap_or(remaining.len());
                    tokens.push((remaining[..end].to_string(), Some(rgba(0x00CE9178u32)))); // green
                    remaining = &remaining[end..];
                    continue;
                }

                // Try to match a comment
                if remaining.starts_with("//") || remaining.starts_with('#') {
                    tokens.push((remaining.to_string(), Some(rgba(0x006A9955u32)))); // dim green
                    break;
                }

                // Try to match a number
                if remaining.starts_with(|c: char| c.is_ascii_digit()) {
                    let end = remaining
                        .find(|c: char| {
                            !c.is_ascii_digit()
                                && c != '.'
                                && c != 'x'
                                && !(c >= 'A' && c <= 'F')
                                && !(c >= 'a' && c <= 'f')
                        })
                        .unwrap_or(remaining.len());
                    tokens.push((remaining[..end].to_string(), Some(rgba(0x00B5CEA8u32)))); // light green
                    remaining = &remaining[end..];
                    continue;
                }

                // Check for multi-char tokens (keywords, types)
                let mut matched = false;

                // Sort keywords by length descending to match longer tokens first
                for &(kw, color) in keywords.iter().chain(type_keywords.iter()) {
                    if remaining.starts_with(kw) {
                        // Ensure word boundary
                        let after = &remaining[kw.len()..];
                        let is_word_boundary = after.is_empty()
                            || !after.starts_with(|c: char| c.is_alphanumeric() || c == '_');
                        if is_word_boundary {
                            tokens.push((kw.to_string(), Some(rgba(color))));
                            remaining = after;
                            matched = true;
                            break;
                        }
                    }
                }

                if matched {
                    continue;
                }

                // Single character or non-matching token
                let end = remaining
                    .find(|c: char| !c.is_alphanumeric() && c != '_')
                    .unwrap_or(1);
                if end > 0 {
                    tokens.push((remaining[..end].to_string(), None));
                    remaining = &remaining[end..];
                } else {
                    // Just take the next character (punctuation, whitespace)
                    let end = remaining.len().min(1);
                    tokens.push((remaining[..end].to_string(), None));
                    remaining = &remaining[end..];
                }
            }

            result.push(tokens);
        }

        result
    }

    /// Render HTML preview — strip tags and show readable text, or show highlighted raw HTML

    fn render_html_preview(text: &str) -> impl IntoElement {
        // Strip HTML tags for readability
        let stripped = Self::strip_html_tags(text);

        div().flex_col().w_full().max_h(px(400.0)).child(
            div()
                .bg(rgba(0xFFFFFF05u32))
                .rounded(px(4.0))
                .p(px(8.0))
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                        .font_family(
                            "ui-monospace, SFMono-Regular, 'Cascadia Code', Consolas, monospace",
                        )
                        .child(stripped),
                ),
        )
    }

    /// Simple HTML tag stripper — removes <tags> and decodes common entities

    fn strip_html_tags(html: &str) -> String {
        let mut result = String::new();
        let mut in_tag = false;
        let mut in_entity = false;
        let mut entity_buf = String::new();

        for c in html.chars() {
            match c {
                '<' => {
                    if !in_tag {
                        // Flush any pending entity
                        if in_entity {
                            result.push_str(&Self::decode_html_entity(&entity_buf));
                            entity_buf.clear();
                            in_entity = false;
                        }
                        in_tag = true;
                    } else {
                        result.push('<');
                    }
                }
                '>' => {
                    if in_tag {
                        in_tag = false;
                    } else {
                        result.push('>');
                    }
                }
                '&' if !in_tag => {
                    if in_entity {
                        result.push_str(&Self::decode_html_entity(&entity_buf));
                        entity_buf.clear();
                    }
                    in_entity = true;
                    entity_buf.clear();
                }
                ';' if in_entity => {
                    result.push_str(&Self::decode_html_entity(&entity_buf));
                    entity_buf.clear();
                    in_entity = false;
                }
                _ => {
                    if in_tag {
                        // Skip tag content
                    } else if in_entity {
                        entity_buf.push(c);
                    } else {
                        result.push(c);
                    }
                }
            }
        }

        // Flush any remaining entity
        if in_entity && !entity_buf.is_empty() {
            result.push_str(&Self::decode_html_entity(&entity_buf));
        }

        // Collapse multiple whitespace
        let mut collapsed = String::new();
        let mut prev_space = false;
        for c in result.chars() {
            if c.is_whitespace() {
                if !prev_space {
                    collapsed.push(' ');
                    prev_space = true;
                }
            } else {
                collapsed.push(c);
                prev_space = false;
            }
        }

        collapsed.trim().to_string()
    }

    /// Decode common HTML entities

    fn decode_html_entity(entity: &str) -> String {
        match entity {
            "amp" => "&".to_string(),
            "lt" => "<".to_string(),
            "gt" => ">".to_string(),
            "quot" => "\"".to_string(),
            "apos" => "'".to_string(),
            "nbsp" => " ".to_string(),
            "copy" => "©".to_string(),
            "reg" => "®".to_string(),
            "trade" => "™".to_string(),
            "mdash" => "—".to_string(),
            "ndash" => "–".to_string(),
            "laquo" => "«".to_string(),
            "raquo" => "»".to_string(),
            _ => {
                // Numeric entities
                if let Some(num_str) = entity.strip_prefix('#') {
                    if let Some(hex_str) = num_str.strip_prefix('x') {
                        if let Ok(code) = u32::from_str_radix(hex_str, 16) {
                            if let Some(ch) = char::from_u32(code) {
                                return ch.to_string();
                            }
                        }
                    } else if let Ok(code) = num_str.parse::<u32>() {
                        if let Some(ch) = char::from_u32(code) {
                            return ch.to_string();
                        }
                    }
                }
                format!("&{};", entity)
            }
        }
    }

    /// Render RTF preview — strips RTF control words and shows plain text

    fn render_rtf_preview(text: &str) -> impl IntoElement {
        let stripped = Self::strip_rtf(text);

        div()
            .flex_col()
            .w_full()
            .max_h(px(400.0))
            .child(
                div()
                    .child(div().text_size(px(13.0)).font_weight(FontWeight(600.0)).mb(px(6.0)).child("RTF Document"))
                    .child(
                        div()
                            .bg(rgba(0xFFFFFF05u32))
                            .rounded(px(4.0))
                            .p(px(8.0))
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .font_family("ui-monospace, SFMono-Regular, 'Cascadia Code', Consolas, monospace")
                                    .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                                    .child(stripped),
                            ),
                    ),
            )
    }

    /// Simple RTF stripper — removes \control words, {braces} groups, and keeps plain text

    fn strip_rtf(rtf: &str) -> String {
        let mut result = String::new();
        let mut in_control = false;
        let mut brace_depth = 0u32;
        let mut chars = rtf.chars().peekable();

        while let Some(c) = chars.next() {
            match c {
                '{' => {
                    brace_depth += 1;
                    // Skip group open
                }
                '}' => {
                    if brace_depth > 0 {
                        brace_depth -= 1;
                    }
                    // Skip group close
                }
                '\\' => {
                    // Start of a control word or symbol
                    in_control = true;
                    // Skip the backslash
                }
                '\r' | '\n' => {
                    if brace_depth == 0 {
                        result.push(c);
                    }
                }
                _ => {
                    if in_control {
                        if c.is_ascii_alphabetic() {
                            // Skip control word characters
                            while let Some(&next) = chars.peek() {
                                if next.is_ascii_alphabetic() {
                                    chars.next();
                                } else {
                                    break;
                                }
                            }
                            // Skip optional space delimiter after control word
                            if let Some(&' ') = chars.peek() {
                                chars.next();
                            }
                        }
                        // Skip control symbol (single non-alpha char after backslash)
                        in_control = false;
                    } else if brace_depth == 0 {
                        // Only keep text at the top level (not inside braces)
                        // Also handle common RTF escape sequences
                        match c {
                            '\\' => result.push('\\'),
                            _ => result.push(c),
                        }
                    }
                }
            }
        }

        // Collapse whitespace
        let mut collapsed = String::new();
        let mut prev_space = false;
        for c in result.chars() {
            if c.is_whitespace() && c != '\n' {
                if !prev_space {
                    collapsed.push(' ');
                    prev_space = true;
                }
            } else {
                collapsed.push(c);
                prev_space = false;
            }
        }

        collapsed.trim().to_string()
    }

    /// Render media preview (audio/video) with metadata and player placeholder

    fn render_media_preview(
        info: &MediaPreviewInfo,
        media_type: &str,
        text: &str,
        item: &FileItem,
    ) -> impl IntoElement {
        let icon = match media_type {
            "Audio" => "\u{E8D1}",
            _ => "\u{E8D1}",
        };

        let format_line = info.format.as_deref().unwrap_or("Unknown");
        let duration_str = info
            .duration_secs
            .map(|d| {
                let mins = (d / 60.0) as u64;
                let secs = (d % 60.0) as u64;
                format!("{}:{:02}", mins, secs)
            })
            .unwrap_or_else(|| "Unknown".to_string());

        let codec_str = info.codec.as_deref().unwrap_or("Unknown");
        let bitrate_str = info
            .bitrate_kbps
            .map(|b| format!("{} kbps", b))
            .unwrap_or_else(|| "Unknown".to_string());

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
                    .push(info_row("Resolution", &format!("{}x{}", w, h)).into_any_element());
            }
        }
        if let Some(fr) = info.frame_rate {
            meta_children
                .push(info_row("Frame Rate", &format!("{:.1} fps", fr)).into_any_element());
        }

        div()
            .flex_col()
            .items_center()
            .gap(px(12.0))
            .w_full()
            // Large icon
            .child(
                div()
                    .w(px(64.0))
                    .h(px(64.0))
                    .bg(rgba(UiConstants::BG_BADGE_RGBA))
                    .rounded(px(8.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .text_size(px(32.0))
                            .font_family(UiConstants::FONT_MDL2)
                            .text_color(rgba(UiConstants::ACCENT_RGBA))
                            .child(icon),
                    ),
            )
            // File name
            .child(
                div()
                    .text_size(px(14.0))
                    .font_weight(FontWeight(600.0))
                    .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                    .text_center()
                    .child(item.name.clone()),
            )
            // Player placeholder
            .child(
                div()
                    .w_full()
                    .h(px(60.0))
                    .bg(rgba(0xFFFFFF10u32))
                    .rounded(px(6.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .gap(px(8.0))
                    .child(
                        div()
                            .text_size(px(18.0))
                            .font_family(UiConstants::FONT_MDL2)
                            .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                            .child("\u{E768}"), // Play icon
                    )
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                            .child("Player placeholder"),
                    ),
            )
            // Separator
            .child(
                div()
                    .w_full()
                    .h(px(1.0))
                    .bg(rgba(UiConstants::DIVIDER_RGBA)),
            )
            // Metadata section
            .child(
                div()
                    .flex_col()
                    .gap(px(6.0))
                    .w_full()
                    .child(section_header(&format!("{} Info", media_type)))
                    .children(meta_children),
            )
            // Raw text info (preview_text)
            .child(
                div()
                    .mt(px(8.0))
                    .p(px(8.0))
                    .w_full()
                    .bg(rgba(0xFFFFFF05u32))
                    .rounded(px(4.0))
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                            .child(text.to_string()),
                    ),
            )
    }

    /// Render PDF preview with metadata

    fn render_pdf_preview(_info: &PdfPreviewInfo, text: &str) -> impl IntoElement {
        div()
            .flex_col()
            .items_center()
            .gap(px(8.0))
            .w_full()
            .child(
                div()
                    .w(px(64.0))
                    .h(px(64.0))
                    .bg(rgba(UiConstants::BG_BADGE_RGBA))
                    .rounded(px(8.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .text_size(px(32.0))
                            .font_family(UiConstants::FONT_MDL2)
                            .text_color(rgba(0xFFFF0000u32)) // Red for PDF
                            .child("\u{E8A5}"),
                    ),
            )
            .child(
                div()
                    .mt(px(8.0))
                    .p(px(8.0))
                    .w_full()
                    .bg(rgba(0xFFFFFF05u32))
                    .rounded(px(4.0))
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                            .child(text.to_string()),
                    ),
            )
    }

    /// Render archive preview with file listing

    fn render_archive_preview(_info: &ArchivePreviewInfo, text: &str) -> impl IntoElement {
        div()
            .flex_col()
            .items_center()
            .gap(px(8.0))
            .w_full()
            .child(
                div()
                    .w(px(64.0))
                    .h(px(64.0))
                    .bg(rgba(UiConstants::BG_BADGE_RGBA))
                    .rounded(px(8.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .text_size(px(32.0))
                            .font_family(UiConstants::FONT_MDL2)
                            .text_color(rgba(UiConstants::ACCENT_RGBA))
                            .child("\u{E7A3}"),
                    ),
            )
            .child(
                div()
                    .mt(px(8.0))
                    .p(px(8.0))
                    .w_full()
                    .max_h(px(300.0))
                    .bg(rgba(0xFFFFFF05u32))
                    .rounded(px(4.0))
                    .child(
                        div()
                            .text_size(px(11.0))
                            .font_family("ui-monospace, SFMono-Regular, 'Cascadia Code', Consolas, monospace")
                            .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                            .child(text.to_string()),
                    ),
            )
    }

    /// Render folder preview as a thumbnail grid of up to 12 items

    fn render_folder_preview(items: &[FolderPreviewItem]) -> impl IntoElement {
        let display_items = if items.len() > 12 {
            &items[..12]
        } else {
            items
        };
        let cols = 4.min(display_items.len().max(1));
        let width_pct = 100.0 / cols as f32;

        div()
            .flex_col()
            .items_center()
            .gap(px(12.0))
            .w_full()
            .child(
                div()
                    .w(px(64.0))
                    .h(px(64.0))
                    .bg(rgba(UiConstants::BG_BADGE_RGBA))
                    .rounded(px(8.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .text_size(px(32.0))
                            .font_family(UiConstants::FONT_MDL2)
                            .text_color(rgba(UiConstants::ACCENT_RGBA))
                            .child("\u{E8B7}"),
                    ),
            )
            .child(
                div()
                    .text_size(px(14.0))
                    .font_weight(FontWeight(600.0))
                    .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                    .child(format!("Folder Contents ({} items)", items.len())),
            )
            .child(
                div()
                    .w_full()
                    .h(px(1.0))
                    .bg(rgba(UiConstants::DIVIDER_RGBA)),
            )
            // Thumbnail grid
            .child(div().flex_row().flex_wrap().w_full().gap(px(6.0)).children(
                display_items.iter().map(|entry| {
                    let icon = if entry.is_dir { "\u{E8B7}" } else { "\u{E8A5}" };

                    div()
                        .flex_col()
                        .items_center()
                        .gap(px(3.0))
                        .w(px(60.0))
                        .child(
                            div()
                                .w(px(40.0))
                                .h(px(40.0))
                                .bg(rgba(0xFFFFFF10u32))
                                .rounded(px(6.0))
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(
                                    div()
                                        .text_size(px(20.0))
                                        .font_family(UiConstants::FONT_MDL2)
                                        .text_color(if entry.is_dir {
                                            rgba(UiConstants::ACCENT_RGBA)
                                        } else {
                                            rgba(UiConstants::TEXT_SECONDARY_RGBA)
                                        })
                                        .child(icon),
                                ),
                        )
                        .child(
                            div()
                                .text_size(px(9.0))
                                .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                .overflow_hidden()
                                .max_w(px(60.0))
                                .text_center()
                                .child(entry.name.clone()),
                        )
                        .into_any_element()
                }),
            ))
            .children(if items.len() > 12 {
                vec![div()
                    .text_size(px(10.0))
                    .text_color(rgba(UiConstants::TEXT_MUTED_RGBA))
                    .child(format!("+ {} more items", items.len() - 12))
                    .into_any_element()]
            } else {
                vec![]
            })
    }
}

use crate::config::config::ViewMode;
use crate::ui::components::text_input::{
    render_text_input, TextInputMode, TextInputState, TextInputStyle,
};
use crate::ui::constants::UiConstants;
use gpui::*;
use std::cmp::Ordering;
use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) use crate::ui::components::status_bar::format_size;

fn active_rename_input_mut(app: &mut crate::app::AppWindow) -> Option<&mut TextInputState> {
    let active_focus = app.state.active_pane_focus();
    if app.state.pane_view(active_focus).renaming_index.is_some() {
        Some(&mut app.state.pane_view_mut(active_focus).rename_input)
    } else if app.state.main_view.renaming_index.is_some() {
        Some(&mut app.state.main_view.rename_input)
    } else if app.state.secondary_view.renaming_index.is_some() {
        Some(&mut app.state.secondary_view.rename_input)
    } else {
        None
    }
}

fn focus_main_on_rename_click(
    app: &mut crate::app::AppWindow,
    win: &mut Window,
    _cx: &mut Context<crate::app::AppWindow>,
) {
    if let Some(ref focus) = app.state.main_focus {
        win.focus(focus);
    }
}

/// Render unified single-line inline rename input (Details, List, Columns, Tree views).
pub fn render_inline_rename_single(
    state: &TextInputState,
    view: Entity<crate::app::AppWindow>,
) -> AnyElement {
    div()
        .flex_1()
        .min_w(px(0.0))
        .child(render_text_input(
            "inline-rename-single-input",
            state,
            "",
            true,
            false,
            TextInputMode::SingleLine,
            TextInputStyle::inline_rename_single(),
            view,
            Rc::new(active_rename_input_mut),
            Some(Rc::new(focus_main_on_rename_click)),
        ))
        .into_any_element()
}

/// Render unified 3-line centered inline rename input (Grid view tiles).
pub fn render_inline_rename_grid(
    state: &TextInputState,
    box_height: f32,
    font_size: f32,
    line_height: f32,
    box_width: f32,
    view: Entity<crate::app::AppWindow>,
) -> AnyElement {
    div()
        .w(px(box_width.max(48.0)))
        .max_w_full()
        .mt(px(4.0))
        .child(render_text_input(
            "inline-rename-grid-input",
            state,
            "",
            true,
            false,
            TextInputMode::MultiLine {
                max_lines: Some(3),
                allow_newlines: false,
                text_align_center: true,
            },
            TextInputStyle::inline_rename_grid(font_size, line_height, box_height.max(line_height + 6.0)),
            view,
            Rc::new(active_rename_input_mut),
            Some(Rc::new(focus_main_on_rename_click)),
        ))
        .into_any_element()
}


/// Compute an adaptive 3-line filename label, effective font size, and line height.
///
/// Behavior:
/// 1. Keeps the full filename at `base_font_size` as long as it fits within **3 lines**
///    at `avail_width_px`.
/// 2. Only when the filename exceeds 3 lines at `base_font_size` does it reduce the font
///    size toward `min_font_size` so more characters fit across the 3 lines.
/// 3. If the filename still exceeds 3 lines at `min_font_size`, applies middle-ellipsis
///    shortening (`shorten_filename_middle`) at the 3-line capacity so the prefix and
///    file extension remain visible without overflowing the 3-line box.
pub fn adaptive_three_line_label(
    name: &str,
    avail_width_px: f32,
    base_font_size: f32,
    min_font_size: f32,
) -> (String, f32, f32) {
    let safe_w = avail_width_px.max(48.0);
    let char_count = name.chars().count();

    let base_cpl = ((safe_w / (base_font_size * 0.56)).floor() as usize).max(8);
    let base_cap = base_cpl * 3;
    if char_count <= base_cap {
        let line_h = (base_font_size * 1.24).ceil();
        return (name.to_string(), base_font_size, line_h);
    }

    let mid_font = ((base_font_size + min_font_size) * 0.5).max(min_font_size);
    let mid_cpl = ((safe_w / (mid_font * 0.55)).floor() as usize).max(9);
    let mid_cap = mid_cpl * 3;
    if char_count <= mid_cap {
        let line_h = (mid_font * 1.22).ceil();
        return (name.to_string(), mid_font, line_h);
    }

    let small_font = min_font_size.max(8.5);
    let small_cpl = ((safe_w / (small_font * 0.54)).floor() as usize).max(10);
    let small_cap = small_cpl * 3;
    let line_h = (small_font * 1.22).ceil();
    if char_count <= small_cap {
        (name.to_string(), small_font, line_h)
    } else {
        (
            crate::ui::views::grid_view::shorten_filename_middle(name, small_cap),
            small_font,
            line_h,
        )
    }
}

/// Format a `SystemTime` into `YYYY-MM-DD HH:MM` using O(1) civil-from-days arithmetic
/// (zero loops, zero intermediate heap allocations). Returns `"—"` if `None`.
pub fn format_system_time(time: Option<SystemTime>) -> String {
    let Some(t) = time else {
        return "\u{2014}".to_string();
    };
    let Ok(dur) = t.duration_since(UNIX_EPOCH) else {
        return "\u{2014}".to_string();
    };
    let total_secs = dur.as_secs();
    let days = (total_secs / 86_400) as i64 + 719_468;
    let rem_secs = total_secs % 86_400;
    let hours = rem_secs / 3600;
    let minutes = (rem_secs % 3600) / 60;

    let era = if days >= 0 { days } else { days - 146_096 } / 146_097;
    let doe = (days - era * 146_097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_095) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = y + if m <= 2 { 1 } else { 0 };

    format!("{:04}-{:02}-{:02} {:02}:{:02}", year, m, d, hours, minutes)
}

/// Zero-allocation, case-insensitive natural alphanumeric comparator
/// (equivalent to Windows `StrCmpLogicalW`, cross-platform).
/// Ensures `"file2.txt" < "file10.txt"` without allocating lowercase `String`s inside `sort_by`.
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    let mut ia = a.as_bytes().iter().copied().peekable();
    let mut ib = b.as_bytes().iter().copied().peekable();

    loop {
        match (ia.peek(), ib.peek()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(&ca), Some(&cb)) => {
                if ca.is_ascii_digit() && cb.is_ascii_digit() {
                    // Skip leading zeros while tracking count for tie-breaking
                    let mut zeros_a = 0usize;
                    while ia.peek() == Some(&b'0') {
                        ia.next();
                        zeros_a += 1;
                    }
                    let mut zeros_b = 0usize;
                    while ib.peek() == Some(&b'0') {
                        ib.next();
                        zeros_b += 1;
                    }

                    // Compare numeric run length and first differing digit in one pass
                    let mut len_a = 0usize;
                    let mut len_b = 0usize;
                    let mut first_diff = Ordering::Equal;

                    loop {
                        let da = ia.peek().copied().filter(|c| c.is_ascii_digit());
                        let db = ib.peek().copied().filter(|c| c.is_ascii_digit());
                        match (da, db) {
                            (Some(x), Some(y)) => {
                                ia.next();
                                ib.next();
                                len_a += 1;
                                len_b += 1;
                                if first_diff == Ordering::Equal && x != y {
                                    first_diff = x.cmp(&y);
                                }
                            }
                            (Some(_), None) => {
                                ia.next();
                                len_a += 1;
                            }
                            (None, Some(_)) => {
                                ib.next();
                                len_b += 1;
                            }
                            (None, None) => break,
                        }
                    }

                    match len_a.cmp(&len_b) {
                        Ordering::Equal => {
                            if first_diff != Ordering::Equal {
                                return first_diff;
                            }
                            if zeros_a != zeros_b {
                                // Fewer leading zeros comes first (e.g. "2" < "02")
                                return zeros_a.cmp(&zeros_b);
                            }
                        }
                        non_eq => return non_eq,
                    }
                } else {
                    ia.next();
                    ib.next();
                    let la = ca.to_ascii_lowercase();
                    let lb = cb.to_ascii_lowercase();
                    if la != lb {
                        return la.cmp(&lb);
                    }
                }
            }
        }
    }
}

/// Zero-allocation case-insensitive substring check for ASCII/UTF-8 filenames.
pub fn contains_ignore_case(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }
    if needle.len() > haystack.len() {
        return false;
    }
    if haystack.is_ascii() && needle.is_ascii() {
        let h = haystack.as_bytes();
        let n = needle.as_bytes();
        return h.windows(n.len()).any(|w| w.eq_ignore_ascii_case(n));
    }
    haystack.to_lowercase().contains(&needle.to_lowercase())
}

/// Zero-allocation case-insensitive file extension comparator for `SortBy::Type`.
pub fn ext_cmp(a: &str, b: &str) -> Ordering {
    let ext_a = a.rsplit_once('.').map(|(_, e)| e).unwrap_or("");
    let ext_b = b.rsplit_once('.').map(|(_, e)| e).unwrap_or("");
    match natural_cmp(ext_a, ext_b) {
        Ordering::Equal => natural_cmp(a, b),
        ord => ord,
    }
}

pub(crate) fn info_row(label: &str, value: &str) -> impl IntoElement {
    div()
        .flex_col()
        .gap(px(2.0))
        .w_full()
        .child(
            div()
                .text_size(AbsoluteLength::Pixels(px(11.0)))
                .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                .font_weight(FontWeight(600.0))
                .child(label.to_string()),
        )
        .child(
            div()
                .text_size(AbsoluteLength::Pixels(px(13.0)))
                .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                .overflow_hidden()
                .child(value.to_string()),
        )
}

/// Render a small bold section heading (used in details pane)
pub(crate) fn section_header(title: &str) -> impl IntoElement {
    div()
        .text_size(px(10.0))
        .font_weight(FontWeight(700.0))
        .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
        .py(px(2.0))
        .child(title.to_uppercase())
}

/// Get file extension from a filename (lowercase)
pub(crate) fn get_extension(filename: &str) -> String {
    filename
        .rsplit('.')
        .next()
        .map(|s| s.to_lowercase())
        .unwrap_or_default()
}

/// Get the MDL2/Fluent glyph for a file based on its extension
pub(crate) fn get_file_type_icon(extension: &str) -> &'static str {
    match extension.to_lowercase().as_str() {
        // Documents
        "pdf" => "\u{E8A5}",
        "doc" | "docx" | "odt" | "rtf" | "epub" | "mobi" | "azw" | "azw3" | "txt" | "md"
        | "markdown" | "rst" | "log" | "ini" | "cfg" | "yaml" | "yml" | "toml" | "json" | "xml"
        | "nfo" => "\u{E8A5}",
        // Spreadsheets
        "xls" | "xlsx" | "csv" | "ods" => "\u{E8A2}",
        // Presentations
        "ppt" | "pptx" | "odp" => "\u{E8A1}",
        // Images
        "jpg" | "jpeg" | "png" | "gif" | "bmp" | "webp" | "ico" | "tiff" | "tif" | "svg"
        | "raw" | "psd" | "ai" | "eps" | "tga" | "exr" | "hdr" | "heic" | "heif" => "\u{E8B9}",
        // Audio
        "mp3" | "wav" | "flac" | "ogg" | "m4a" | "aac" | "wma" | "ape" | "alac" | "aiff" => {
            "\u{E8D1}"
        }
        // Video
        "mp4" | "mkv" | "avi" | "mov" | "wmv" | "webm" | "m4v" | "mpg" | "mpeg" | "3gp" | "flv"
        | "vob" => "\u{E8D1}",
        // Archives
        "zip" | "rar" | "7z" | "tar" | "gz" | "bz2" | "xz" | "lz" | "lzma" | "cab" | "iso"
        | "dmg" => "\u{E7A3}",
        // Code / Dev files
        "rs" | "js" | "ts" | "jsx" | "tsx" | "py" | "java" | "c" | "cpp" | "h" | "hpp" | "cs"
        | "go" | "rb" | "php" | "swift" | "kt" | "scala" | "r" | "lua" | "perl" | "sh" | "bash"
        | "zsh" | "fish" | "ps1" | "bat" | "cmd" | "dockerfile" | "makefile" | "css" | "scss"
        | "sass" | "less" | "stylus" | "html" | "htm" | "xhtml" | "ejs" | "hbs" | "handlebars"
        | "sql" | "db" | "sqlite" | "mdb" | "accdb" | "gitignore" | "gitattributes"
        | "editorconfig" | "prettierrc" | "eslintrc" | "npmrc" | "yarnrc" | "env"
        | "properties" | "plist" => "\u{E8F1}",
        // Executables
        "exe" | "msi" | "jar" | "app" | "pkg" | "deb" | "rpm" | "apk" | "xap" => "\u{E8F2}",
        // Fonts
        "ttf" | "otf" | "woff" | "woff2" | "eot" => "\u{E8D0}",
        // 3D / CAD
        "obj" | "fbx" | "dae" | "stl" | "blend" | "3ds" | "max" | "skp" | "dwg" | "dxf"
        | "step" | "stp" | "iges" | "igs" => "\u{E8D0}",
        // Security / Certs
        "pem" | "key" | "crt" | "cer" | "pfx" | "p12" | "jks" | "keystore" => "\u{E8BB}",
        // Links / Shortcuts
        "lnk" | "shortcut" | "url" => "\u{E71B}",
        // Torrents
        "torrent" => "\u{E896}",
        // Fallback - generic file
        _ => "\u{E7C3}",
    }
}

/// Get both the MDL2/Fluent icon glyph and semantic color (RGB hex) for a file extension.
/// Used across Details, List, Cards, Grid, and Columns views for O(1) zero-allocation icon rendering.
pub fn get_file_type_icon_and_color(extension: &str) -> (&'static str, u32) {
    match extension {
        "pdf" => ("\u{EA90}", 0xFF6B6B),
        "doc" | "docx" | "odt" | "rtf" => ("\u{E8A5}", 0x4B9EFF),
        "xls" | "xlsx" | "xlsm" | "csv" | "tsv" | "ods" => ("\u{E8A2}", 0x4CCB5E),
        "ppt" | "pptx" | "odp" => ("\u{E8A1}", 0xFF8C42),
        "jpg" | "jpeg" | "png" | "gif" | "bmp" | "webp" | "ico" | "tiff" | "tif" | "svg"
        | "avif" | "heic" | "heif" | "raw" | "psd" | "tga" | "hdr" => ("\u{EB9F}", 0xC586C0),
        "mp4" | "mkv" | "avi" | "mov" | "wmv" | "webm" | "m4v" | "flv" | "ts" => ("\u{E714}", 0xFF79C6),
        "mp3" | "wav" | "flac" | "ogg" | "m4a" | "aac" | "opus" | "wma" => ("\u{EC4F}", 0x50FA7B),
        "zip" | "rar" | "7z" | "tar" | "gz" | "bz2" | "xz" | "tgz" | "zst" | "iso" => ("\u{F012}", 0xFCCB45),
        "rs" | "py" | "js" | "ts" | "jsx" | "tsx" | "c" | "cpp" | "h" | "hpp" | "cs" | "go"
        | "java" | "kt" | "swift" | "rb" | "php" | "sh" | "ps1" | "sql" | "html" | "css"
        | "json" | "toml" | "yaml" | "yml" | "xml" | "md" => ("\u{E943}", 0x60CDFF),
        "exe" | "msi" | "dll" | "sys" | "so" | "dylib" | "wasm" | "bat" | "cmd" => ("\u{E756}", 0xFFB86C),
        _ => ("\u{E8A5}", 0x9AA4B4),
    }
}

/// Map a ViewMode to its default zoom level (0.0–1.0).
pub fn zoom_for_view_mode(mode: &ViewMode) -> f32 {
    match mode {
        ViewMode::Details | ViewMode::Tree => 0.15,
        ViewMode::List => 0.25,
        ViewMode::Cards => 0.42,
        ViewMode::Columns => 0.35,
        ViewMode::Grid => 0.55,
    }
}

/// Map a zoom level (0.0–1.0) to the appropriate ViewMode.
pub fn view_mode_for_zoom(zoom: f32) -> ViewMode {
    if zoom <= 0.14 {
        ViewMode::Details
    } else if zoom <= 0.26 {
        ViewMode::List
    } else if zoom <= 0.36 {
        ViewMode::Columns
    } else if zoom <= 0.46 {
        ViewMode::Cards
    } else {
        ViewMode::Grid
    }
}

/// Label + icon for the current zoom mode (used in status bar and toolbar).
pub fn zoom_mode_label(mode: &ViewMode) -> (&'static str, &'static str) {
    match mode {
        ViewMode::Details => ("\u{E8FD}", "Details"),
        ViewMode::List => ("\u{EA37}", "List"),
        ViewMode::Cards => ("\u{E8A9}", "Cards"),
        ViewMode::Columns => ("\u{E8F1}", "Columns"),
        ViewMode::Grid => ("\u{E80A}", "Grid"),
        ViewMode::Tree => ("\u{F0E2}", "Tree"),
    }
}

/// ViewMode icon glyphs for the slider labels.
pub fn zoom_mode_icon(mode: &ViewMode) -> &'static str {
    zoom_mode_label(mode).0
}

/// ViewMode display names.
pub fn zoom_mode_name(mode: &ViewMode) -> &'static str {
    zoom_mode_label(mode).1
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    use std::time::Duration;

    #[test]
    fn test_natural_cmp_numeric_ordering() {
        let mut names = vec![
            "file10.txt",
            "file2.txt",
            "file1.txt",
            "file20.txt",
            "File3.txt",
        ];
        names.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(
            names,
            vec![
                "file1.txt",
                "file2.txt",
                "File3.txt",
                "file10.txt",
                "file20.txt"
            ]
        );
    }

    #[test]
    fn test_natural_cmp_leading_zeros() {
        assert_eq!(natural_cmp("img2.png", "img02.png"), Ordering::Less);
        assert_eq!(natural_cmp("img02.png", "img10.png"), Ordering::Less);
    }

    #[test]
    fn test_format_system_time_epoch_and_none() {
        assert_eq!(format_system_time(None), "\u{2014}");
        assert_eq!(format_system_time(Some(UNIX_EPOCH)), "1970-01-01 00:00");
        // 2026-06-25 12:30:00 UTC = 1782390600 secs
        let t = UNIX_EPOCH + Duration::from_secs(1_782_390_600);
        assert_eq!(format_system_time(Some(t)), "2026-06-25 12:30");
    }

    #[test]
    fn bench_natural_sort_50k_filenames_under_budget() {
        let mut names: Vec<String> = (0..50_000)
            .map(|i| format!("Project_Asset_{}_v{}.png", (i * 7919) % 50_000, i % 20))
            .collect();
        let start = std::time::Instant::now();
        names.sort_unstable_by(|a, b| natural_cmp(a, b));
        let elapsed = start.elapsed();
        assert!(
            elapsed.as_millis() < 250,
            "Sorting 50,000 filenames took {:?} (expected < 250ms in debug / < 15ms in release)",
            elapsed
        );
    }
}

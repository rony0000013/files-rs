use crate::config::config::ViewMode;
use crate::ui::constants::UiConstants;
use gpui::*;

pub(crate) use crate::ui::components::status_bar::format_size;

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

/// Get the MDL2 glyph for a file based on its extension
/// Uses Segoe MDL2 Assets glyphs for file type icons
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

/// Map a ViewMode to its center zoom level (0.0–1.0).
pub fn zoom_for_view_mode(mode: &ViewMode) -> f32 {
    match mode {
        ViewMode::Details => 0.05,
        ViewMode::List => 0.18,
        ViewMode::Columns => 0.33,
        ViewMode::Grid => 0.50,
    }
}

/// Map a zoom level (0.0–1.0) to the appropriate ViewMode.
pub fn view_mode_for_zoom(zoom: f32) -> ViewMode {
    if zoom <= 0.10 {
        ViewMode::Details
    } else if zoom <= 0.25 {
        ViewMode::List
    } else if zoom <= 0.40 {
        ViewMode::Columns
    } else {
        ViewMode::Grid
    }
}

/// Label + icon for the current zoom mode (used in status bar).
pub fn zoom_mode_label(mode: &ViewMode) -> (&'static str, &'static str) {
    match mode {
        ViewMode::Details => ("\u{E783}", "Details"),
        ViewMode::List => ("\u{E77B}", "List"),
        ViewMode::Columns => ("\u{E7A7}", "Columns"),
        ViewMode::Grid => ("\u{E7A5}", "Grid"),
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

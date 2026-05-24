use lucide_rs::Lucide;
use once_cell::sync::Lazy;
use resvg::tiny_skia::Pixmap;
use std::collections::HashMap;
use std::sync::Mutex;

/// Cache for rendered icon textures
static ICON_CACHE: Lazy<Mutex<HashMap<String, Vec<u8>>>> = Lazy::new(|| Mutex::new(HashMap::new()));

// ─────────────────────────────────────────────────────────────────────────────
// ThumbnailRenderer
// ─────────────────────────────────────────────────────────────────────────────

/// Image file extensions that support thumbnail generation
const THUMBNAIL_IMAGE_EXTENSIONS: &[&str] = &[
    "jpg", "jpeg", "png", "gif", "bmp", "webp", "ico", "tiff", "tif", "heic", "heif",
];

/// Check if a file extension is a supported image type for thumbnails
pub fn is_thumbnail_image(extension: &str) -> bool {
    THUMBNAIL_IMAGE_EXTENSIONS.contains(&extension.to_lowercase().as_str())
}

/// Get the thumbnail size for grid view (64px as per DESIGN_SPEC)
pub const THUMBNAIL_SIZE: u32 = 64;

/// ThumbnailRenderer provides thumbnail loading and caching for image files
pub struct ThumbnailRenderer;

impl ThumbnailRenderer {
    /// Check if a file path points to a supported thumbnail image
    pub fn is_supported(path: &std::path::Path) -> bool {
        path.extension()
            .and_then(|ext| ext.to_str())
            .map(is_thumbnail_image)
            .unwrap_or(false)
    }

    /// Load and resize an image to thumbnail size, returning RGBA pixel data
    /// Returns None if the file cannot be loaded or is not an image
    pub fn load_thumbnail(path: &std::path::Path) -> Option<Vec<u8>> {
        if !Self::is_supported(path) {
            return None;
        }

        // Load the image
        let img = match image::open(path) {
            Ok(img) => img,
            Err(_) => return None,
        };

        // Resize to thumbnail dimensions (maintaining aspect ratio)
        let thumbnail = img.thumbnail(THUMBNAIL_SIZE, THUMBNAIL_SIZE);

        // Convert to RGBA if necessary
        let rgba = thumbnail.to_rgba8();

        // Return raw RGBA bytes
        Some(rgba.into_raw())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// IconRenderer
// ─────────────────────────────────────────────────────────────────────────────

#[allow(dead_code)]
pub struct IconRenderer;

#[allow(dead_code)]
impl IconRenderer {
    /// Render an icon to raw RGBA pixel data
    pub fn render(icon: Lucide, size: u32) -> Vec<u8> {
        let key = format!("{:?}_{}", icon, size);

        // Check cache first
        if let Ok(cache) = ICON_CACHE.lock() {
            if let Some(data) = cache.get(&key) {
                return data.clone();
            }
        }

        // Get SVG data from lucide-rs
        let svg_data = icon.to_string();

        // Parse SVG
        let tree =
            resvg::usvg::Tree::from_data(svg_data.as_bytes(), &resvg::usvg::Options::default())
                .map_err(|e| format!("Failed to parse SVG: {}", e))
                .unwrap();

        // Create pixmap
        let mut pixmap = Pixmap::new(size, size).unwrap();

        // Render using the high-level resvg::render function
        let transform = resvg::tiny_skia::Transform::from_scale(
            size as f32 / tree.size().width() as f32,
            size as f32 / tree.size().height() as f32,
        );
        resvg::render(&tree, transform, &mut pixmap.as_mut());

        let pixel_data = pixmap.data().to_vec();

        // Cache it
        if let Ok(mut cache) = ICON_CACHE.lock() {
            cache.insert(key, pixel_data.clone());
        }

        pixel_data
    }

    /// Get icon for common file types
    pub fn for_file_type(extension: &str) -> Lucide {
        match extension.to_lowercase().as_str() {
            // Documents
            "pdf" => Lucide::FileText,
            "doc" | "docx" | "odt" | "rtf" => Lucide::FileText,
            "xls" | "xlsx" | "csv" | "ods" => Lucide::Table,
            "ppt" | "pptx" | "odp" => Lucide::Presentation,
            "txt" | "md" | "log" | "ini" | "cfg" | "yaml" | "yml" | "toml" | "json" | "xml" => {
                Lucide::FileText
            }
            "epub" | "mobi" | "azw" | "azw3" => Lucide::BookOpen,

            // Images
            "jpg" | "jpeg" | "png" | "gif" | "bmp" | "webp" | "ico" | "tiff" | "tif" | "svg"
            | "raw" | "psd" | "ai" | "eps" | "tga" | "exr" | "hdr" => Lucide::Image,
            "heic" | "heif" => Lucide::Image,

            // Audio
            "mp3" | "wav" | "flac" | "ogg" | "m4a" | "aac" | "wma" | "ape" | "alac" | "aiff" => {
                Lucide::Music
            }

            // Video
            "mp4" | "mkv" | "avi" | "mov" | "wmv" | "webm" | "m4v" | "mpg" | "mpeg" | "3gp"
            | "flv" | "vob" => Lucide::Film,

            // Archives
            "zip" | "rar" | "7z" | "tar" | "gz" | "bz2" | "xz" | "lz" | "lzma" | "cab" | "iso"
            | "dmg" => Lucide::Archive,

            // Code / Dev
            "rs" | "js" | "ts" | "jsx" | "tsx" | "py" | "java" | "c" | "cpp" | "h" | "hpp"
            | "cs" | "go" | "rb" | "php" | "swift" | "kt" | "scala" | "r" | "lua" | "perl"
            | "sh" | "bash" | "zsh" | "fish" | "ps1" | "bat" | "cmd" | "dockerfile"
            | "makefile" => Lucide::Code,
            "css" | "scss" | "sass" | "less" | "stylus" => Lucide::Palette,
            "html" | "htm" | "xhtml" | "ejs" | "hbs" | "handlebars" => Lucide::FileCode,
            "sql" | "db" | "sqlite" | "mdb" | "accdb" => Lucide::Database,
            "markdown" | "rst" => Lucide::FileText,
            "gitignore" | "gitattributes" | "editorconfig" | "prettierrc" | "eslintrc"
            | "npmrc" | "yarnrc" => Lucide::Settings,

            // Executables
            "exe" | "msi" | "jar" | "app" | "pkg" | "deb" | "rpm" | "apk" | "xap" => {
                Lucide::AppWindow
            }

            // Fonts
            "ttf" | "otf" | "woff" | "woff2" | "eot" => Lucide::Type,

            // 3D / CAD
            "obj" | "fbx" | "dae" | "blend" | "3ds" | "max" | "skp" | "dwg" | "dxf" => Lucide::Box,
            "step" | "stp" | "iges" | "igs" => Lucide::Box,

            // Data / Config
            "env" | "properties" | "plist" | "registry" => Lucide::Settings,
            "pem" | "key" | "crt" | "cer" | "pfx" | "p12" | "jks" | "keystore" => Lucide::Lock,

            // Other
            "lnk" | "shortcut" => Lucide::Link,
            "url" => Lucide::Link,
            "torrent" => Lucide::Download,
            "nfo" => Lucide::Info,

            _ => Lucide::File,
        }
    }

    /// Get icon for folders
    pub fn for_folder() -> Lucide {
        Lucide::Folder
    }

    /// Get icon for drives
    pub fn for_drive() -> Lucide {
        Lucide::HardDrive
    }
}

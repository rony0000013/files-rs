//! Rich Content Previews — Preview panels for Code, Markdown, PDF, and Media files.
//! Per DESIGN_SPEC.md section 6 / TODO.md Section 6.
//!
//! This module provides preview components for various file types:
//! - Code with syntax highlighting
//! - Markdown rendering
//! - PDF viewing
//! - Media playback (images, audio, video)

use std::path::PathBuf;

/// Supported preview types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewType {
    Code,
    Markdown,
    Pdf,
    Image,
    Audio,
    Video,
    Unknown,
}

impl PreviewType {
    /// Determines the preview type based on file extension.
    pub fn from_extension(ext: &str) -> Self {
        match ext.to_lowercase().as_str() {
            // Code languages
            "rs" | "js" | "ts" | "jsx" | "tsx" | "py" | "java" | "c" | "cpp" | "h" | "hpp"
            | "cs" | "go" | "rb" | "php" | "swift" | "kt" | "scala" | "r" | "lua"
            | "perl" | "sh" | "bash" | "zsh" | "fish" | "ps1" | "bat" | "cmd"
            | "dockerfile" | "makefile" | "css" | "scss" | "sass" | "less" | "html"
            | "htm" | "xml" | "json" | "yaml" | "yml" | "toml" | "ini" | "cfg" | "txt" => {
                PreviewType::Code
            }
            // Markdown
            "md" | "markdown" | "rst" => PreviewType::Markdown,
            // PDF
            "pdf" => PreviewType::Pdf,
            // Images
            "jpg" | "jpeg" | "png" | "gif" | "bmp" | "webp" | "ico" | "tiff" | "tif"
            | "svg" | "raw" | "psd" | "ai" | "eps" | "heic" | "heif" => PreviewType::Image,
            // Audio
            "mp3" | "wav" | "flac" | "ogg" | "m4a" | "aac" | "wma" | "ape" | "alac"
            | "aiff" => PreviewType::Audio,
            // Video
            "mp4" | "mkv" | "avi" | "mov" | "wmv" | "webm" | "m4v" | "mpg" | "mpeg"
            | "3gp" | "flv" | "vob" => PreviewType::Video,
            _ => PreviewType::Unknown,
        }
    }

    /// Checks if this preview type requires syntax highlighting.
    pub fn is_code(&self) -> bool {
        matches!(self, PreviewType::Code)
    }

    /// Checks if this preview type supports text rendering.
    pub fn is_text(&self) -> bool {
        matches!(self, PreviewType::Code | PreviewType::Markdown)
    }
}

/// Language for syntax highlighting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodeLanguage {
    Rust,
    JavaScript,
    TypeScript,
    Python,
    Java,
    C,
    Cpp,
    Go,
    Ruby,
    Php,
    Swift,
    Kotlin,
    Html,
    Css,
    Json,
    Yaml,
    Toml,
    Markdown,
    PlainText,
    Unknown,
}

impl CodeLanguage {
    /// Determines the language based on file extension.
    pub fn from_extension(ext: &str) -> Self {
        match ext.to_lowercase().as_str() {
            "rs" => CodeLanguage::Rust,
            "js" | "jsx" => CodeLanguage::JavaScript,
            "ts" | "tsx" => CodeLanguage::TypeScript,
            "py" => CodeLanguage::Python,
            "java" => CodeLanguage::Java,
            "c" | "h" => CodeLanguage::C,
            "cpp" | "hpp" | "cc" => CodeLanguage::Cpp,
            "go" => CodeLanguage::Go,
            "rb" => CodeLanguage::Ruby,
            "php" => CodeLanguage::Php,
            "swift" => CodeLanguage::Swift,
            "kt" => CodeLanguage::Kotlin,
            "html" | "htm" => CodeLanguage::Html,
            "css" | "scss" | "sass" | "less" => CodeLanguage::Css,
            "json" => CodeLanguage::Json,
            "yaml" | "yml" => CodeLanguage::Yaml,
            "toml" => CodeLanguage::Toml,
            "md" | "markdown" => CodeLanguage::Markdown,
            "txt" | "log" | "ini" | "cfg" => CodeLanguage::PlainText,
            _ => CodeLanguage::Unknown,
        }
    }

    /// Returns the default tab size for this language.
    pub fn default_tab_size(&self) -> usize {
        match self {
            CodeLanguage::Python => 4, // Python uses 4 spaces
            _ => 2,
        }
    }
}

/// Theme for syntax highlighting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HighlightTheme {
    Dark,
    Light,
}

impl Default for HighlightTheme {
    fn default() -> Self {
        HighlightTheme::Dark
    }
}

/// Preview configuration options.
#[derive(Debug, Clone, PartialEq)]
pub struct PreviewConfig {
    /// Whether to show line numbers.
    pub show_line_numbers: bool,
    /// Whether to enable syntax highlighting.
    pub syntax_highlighting: bool,
    /// Whether to wrap long lines.
    pub word_wrap: bool,
    /// Tab size for code preview.
    pub tab_size: usize,
    /// Theme for code preview.
    pub theme: HighlightTheme,
    /// Font size in pixels.
    pub font_size: f64,
    /// Maximum image preview size in pixels.
    pub max_image_size: u32,
    /// Whether to auto-play media.
    pub autoplay_media: bool,
}

impl Eq for PreviewConfig {}

impl Default for PreviewConfig {
    fn default() -> Self {
        Self {
            show_line_numbers: true,
            syntax_highlighting: true,
            word_wrap: false,
            tab_size: 2,
            theme: HighlightTheme::Dark,
            font_size: 14.0,
            max_image_size: 4096,
            autoplay_media: false,
        }
    }
}

/// State for a preview panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviewState {
    /// Current preview type.
    pub preview_type: PreviewType,
    /// File path being previewed.
    pub file_path: Option<PathBuf>,
    /// Whether the preview is loading.
    pub is_loading: bool,
    /// Error message if preview failed.
    pub error: Option<String>,
    /// Content hash for change detection.
    pub content_hash: Option<u64>,
}

impl PreviewState {
    pub fn new() -> Self {
        Self {
            preview_type: PreviewType::Unknown,
            file_path: None,
            is_loading: false,
            error: None,
            content_hash: None,
        }
    }

    /// Returns whether this preview can be edited.
    pub fn can_edit(&self) -> bool {
        matches!(self.preview_type, PreviewType::Code | PreviewType::Markdown)
    }

    /// Returns whether this preview is a media type.
    pub fn is_media(&self) -> bool {
        matches!(
            self.preview_type,
            PreviewType::Image | PreviewType::Audio | PreviewType::Video
        )
    }
}

impl Default for PreviewState {
    fn default() -> Self {
        Self::new()
    }
}

/// Service for managing previews.
#[derive(Debug, Clone, Default)]
pub struct PreviewService;

impl PreviewService {
    pub fn new() -> Self {
        Self
    }

    /// Detects the preview type for a given file path.
    pub fn detect_preview_type(&self, path: &PathBuf) -> PreviewType {
        path.extension()
            .and_then(|e| e.to_str())
            .map(PreviewType::from_extension)
            .unwrap_or(PreviewType::Unknown)
    }

    /// Checks if a file type is supported for preview.
    pub fn is_supported(&self, path: &PathBuf) -> bool {
        self.detect_preview_type(path) != PreviewType::Unknown
    }

    /// Gets suggested config for a preview type.
    pub fn get_default_config(&self, preview_type: PreviewType) -> PreviewConfig {
        let mut config = PreviewConfig::default();
        match preview_type {
            PreviewType::Code | PreviewType::Markdown => {
                config.show_line_numbers = true;
                config.syntax_highlighting = true;
            }
            PreviewType::Image => {
                config.word_wrap = true;
            }
            PreviewType::Audio | PreviewType::Video => {
                config.autoplay_media = false;
            }
            _ => {}
        }
        config
    }

    /// Calculates a simple hash for content change detection.
    pub fn calculate_hash(&self, content: &[u8]) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        content.hash(&mut hasher);
        hasher.finish()
    }

    /// Determines if file is too large for text preview.
    pub fn is_file_too_large(&self, size_bytes: u64, preview_type: PreviewType) -> bool {
        match preview_type {
            PreviewType::Code | PreviewType::Markdown => size_bytes > 10 * 1024 * 1024, // 10MB
            PreviewType::Image => size_bytes > 50 * 1024 * 1024,                      // 50MB
            _ => size_bytes > 100 * 1024 * 1024,                                      // 100MB
        }
    }

    /// Gets the language for syntax highlighting.
    pub fn get_language(&self, path: &PathBuf) -> CodeLanguage {
        path.extension()
            .and_then(|e| e.to_str())
            .map(CodeLanguage::from_extension)
            .unwrap_or(CodeLanguage::Unknown)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preview_type_from_extension_code() {
        assert_eq!(PreviewType::from_extension("rs"), PreviewType::Code);
        assert_eq!(PreviewType::from_extension("py"), PreviewType::Code);
        assert_eq!(PreviewType::from_extension("js"), PreviewType::Code);
        assert_eq!(PreviewType::from_extension("RS"), PreviewType::Code);
    }

    #[test]
    fn test_preview_type_from_extension_markdown() {
        assert_eq!(PreviewType::from_extension("md"), PreviewType::Markdown);
        assert_eq!(PreviewType::from_extension("markdown"), PreviewType::Markdown);
    }

    #[test]
    fn test_preview_type_from_extension_pdf() {
        assert_eq!(PreviewType::from_extension("pdf"), PreviewType::Pdf);
    }

    #[test]
    fn test_preview_type_from_extension_image() {
        assert_eq!(PreviewType::from_extension("png"), PreviewType::Image);
        assert_eq!(PreviewType::from_extension("jpg"), PreviewType::Image);
        assert_eq!(PreviewType::from_extension("gif"), PreviewType::Image);
    }

    #[test]
    fn test_preview_type_from_extension_audio() {
        assert_eq!(PreviewType::from_extension("mp3"), PreviewType::Audio);
        assert_eq!(PreviewType::from_extension("wav"), PreviewType::Audio);
    }

    #[test]
    fn test_preview_type_from_extension_video() {
        assert_eq!(PreviewType::from_extension("mp4"), PreviewType::Video);
        assert_eq!(PreviewType::from_extension("mkv"), PreviewType::Video);
    }

    #[test]
    fn test_preview_type_is_code() {
        assert!(PreviewType::Code.is_code());
        assert!(!PreviewType::Markdown.is_code());
        assert!(!PreviewType::Image.is_code());
    }

    #[test]
    fn test_preview_type_is_text() {
        assert!(PreviewType::Code.is_text());
        assert!(PreviewType::Markdown.is_text());
        assert!(!PreviewType::Pdf.is_text());
        assert!(!PreviewType::Image.is_text());
    }

    #[test]
    fn test_code_language_from_extension() {
        assert_eq!(CodeLanguage::from_extension("rs"), CodeLanguage::Rust);
        assert_eq!(CodeLanguage::from_extension("py"), CodeLanguage::Python);
        assert_eq!(CodeLanguage::from_extension("js"), CodeLanguage::JavaScript);
    }

    #[test]
    fn test_code_language_default_tab_size() {
        assert_eq!(CodeLanguage::Rust.default_tab_size(), 2);
        assert_eq!(CodeLanguage::Python.default_tab_size(), 4);
        assert_eq!(CodeLanguage::Html.default_tab_size(), 2);
    }

    #[test]
    fn test_preview_state_new() {
        let state = PreviewState::new();
        assert_eq!(state.preview_type, PreviewType::Unknown);
        assert!(state.file_path.is_none());
        assert!(!state.is_loading);
        assert!(state.error.is_none());
    }

    #[test]
    fn test_preview_state_can_edit() {
        let mut state = PreviewState::new();
        assert!(!state.can_edit());

        state.preview_type = PreviewType::Code;
        assert!(state.can_edit());

        state.preview_type = PreviewType::Markdown;
        assert!(state.can_edit());

        state.preview_type = PreviewType::Image;
        assert!(!state.can_edit());
    }

    #[test]
    fn test_preview_state_is_media() {
        let mut state = PreviewState::new();
        assert!(!state.is_media());

        state.preview_type = PreviewType::Image;
        assert!(state.is_media());

        state.preview_type = PreviewType::Audio;
        assert!(state.is_media());

        state.preview_type = PreviewType::Video;
        assert!(state.is_media());
    }

    #[test]
    fn test_preview_service_new() {
        let service = PreviewService::new();
        drop(service);
    }

    #[test]
    fn test_preview_service_detect_type() {
        let service = PreviewService::new();
        assert_eq!(
            service.detect_preview_type(&PathBuf::from("test.rs")),
            PreviewType::Code
        );
        assert_eq!(
            service.detect_preview_type(&PathBuf::from("test.md")),
            PreviewType::Markdown
        );
    }

    #[test]
    fn test_preview_service_is_supported() {
        let service = PreviewService::new();
        assert!(service.is_supported(&PathBuf::from("test.rs")));
        assert!(service.is_supported(&PathBuf::from("test.png")));
        assert!(service.is_supported(&PathBuf::from("test.mp3")));
        assert!(!service.is_supported(&PathBuf::from("test.unknown")));
    }

    #[test]
    fn test_preview_service_get_language() {
        let service = PreviewService::new();
        assert_eq!(
            service.get_language(&PathBuf::from("test.rs")),
            CodeLanguage::Rust
        );
        assert_eq!(
            service.get_language(&PathBuf::from("test.py")),
            CodeLanguage::Python
        );
    }

    #[test]
    fn test_preview_service_calculate_hash() {
        let service = PreviewService::new();
        let hash1 = service.calculate_hash(b"hello");
        let hash2 = service.calculate_hash(b"hello");
        let hash3 = service.calculate_hash(b"world");
        assert_eq!(hash1, hash2);
        assert_ne!(hash1, hash3);
    }

    #[test]
    fn test_preview_service_is_file_too_large() {
        let service = PreviewService::new();
        assert!(service.is_file_too_large(15 * 1024 * 1024, PreviewType::Code));
        assert!(!service.is_file_too_large(5 * 1024 * 1024, PreviewType::Code));
        assert!(service.is_file_too_large(60 * 1024 * 1024, PreviewType::Image));
        assert!(!service.is_file_too_large(40 * 1024 * 1024, PreviewType::Image));
    }

    #[test]
    fn test_preview_config_default() {
        let config = PreviewConfig::default();
        assert!(config.show_line_numbers);
        assert!(config.syntax_highlighting);
        assert!(!config.word_wrap);
        assert_eq!(config.tab_size, 2);
        assert_eq!(config.font_size, 14.0);
    }

    #[test]
    fn test_highlight_theme_default() {
        assert_eq!(HighlightTheme::default(), HighlightTheme::Dark);
    }
}
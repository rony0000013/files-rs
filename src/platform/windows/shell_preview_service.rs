use crate::core::path_util::FilesPath;
use anyhow::{anyhow, Result};
use std::path::PathBuf;

#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;

#[cfg(windows)]
use windows::core::PCWSTR;

/// Shell Preview Service
/// Provides file previews for Office docs, PDF, images, etc.
pub struct ShellPreviewService;

impl ShellPreviewService {
    pub fn new() -> Self {
        Self
    }

    /// Check if a file type supports shell preview
    pub fn is_previewable(&self, path: &FilesPath) -> bool {
        let ext = path
            .as_path()
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        // Common previewable types
        matches!(
            ext.as_str(),
            "pdf"
                | "doc"
                | "docx"
                | "xls"
                | "xlsx"
                | "ppt"
                | "pptx"
                | "txt"
                | "rtf"
                | "csv"
                | "xml"
                | "html"
                | "htm"
                | "md"
                | "json"
                | "jpg"
                | "jpeg"
                | "png"
                | "gif"
                | "bmp"
                | "tiff"
                | "webp"
                | "mp3"
                | "wav"
                | "flac"
                | "ogg"
                | "mp4"
                | "avi"
                | "mkv"
                | "zip"
                | "rar"
                | "7z"
        )
    }

    /// Check if a file type is an archive
    pub fn is_archive(&self, path: &FilesPath) -> bool {
        let ext = path
            .as_path()
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        matches!(ext.as_str(), "zip" | "rar" | "7z" | "tar" | "gz" | "bz2")
    }

    /// Check if a file type is an image
    pub fn is_image(&self, path: &FilesPath) -> bool {
        let ext = path
            .as_path()
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        matches!(
            ext.as_str(),
            "jpg" | "jpeg" | "png" | "gif" | "bmp" | "tiff" | "webp" | "svg"
        )
    }

    /// Check if a file type is a video
    pub fn is_video(&self, path: &FilesPath) -> bool {
        let ext = path
            .as_path()
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        matches!(
            ext.as_str(),
            "mp4" | "avi" | "mkv" | "mov" | "wmv" | "flv" | "webm"
        )
    }

    /// Check if a file type is audio
    pub fn is_audio(&self, path: &FilesPath) -> bool {
        let ext = path
            .as_path()
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        matches!(
            ext.as_str(),
            "mp3" | "wav" | "flac" | "ogg" | "m4a" | "aac" | "wma"
        )
    }

    /// Check if a file type is a document
    pub fn is_document(&self, path: &FilesPath) -> bool {
        let ext = path
            .as_path()
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        matches!(
            ext.as_str(),
            "pdf"
                | "doc"
                | "docx"
                | "xls"
                | "xlsx"
                | "ppt"
                | "pptx"
                | "txt"
                | "rtf"
                | "csv"
                | "md"
                | "json"
                | "xml"
                | "html"
        )
    }

    /// Get preview for text-based files
    pub async fn get_text_preview(&self, path: &FilesPath, max_lines: usize) -> Result<String> {
        use std::fs::File;
        use std::io::{BufRead, BufReader};

        let file = File::open(path.as_path())
            .map_err(|e| anyhow!("Failed to open file for preview: {}", e))?;

        let reader = BufReader::new(file);
        let lines: Vec<String> = reader
            .lines()
            .take(max_lines)
            .filter_map(|l| l.ok())
            .collect();

        Ok(lines.join("\n"))
    }

    /// Get preview via shell handler classification
    /// For text-based files (txt, md, csv, json, xml) calls get_text_preview.
    /// For Office/PDF documents returns a descriptive message about shell preview.
    pub async fn get_preview_via_shell(&self, path: &FilesPath) -> Result<String> {
        let ext = path
            .as_path()
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        match ext.as_str() {
            // Text-based files — load preview content directly
            "txt" | "md" | "markdown" | "csv" | "json" | "xml" => {
                self.get_text_preview(path, 200).await
            }
            // Office documents and PDF — shell preview would be used
            "pdf" | "doc" | "docx" | "xls" | "xlsx" | "ppt" | "pptx" => {
                Ok(format!(
                    "Shell Preview (IPreviewHandler)\n\nFile type: .{}\n\nThe shell preview handler would render this file using the Windows IPreviewHandler COM interface.\n\nPreview is not yet implemented for this file type.",
                    ext
                ))
            }
            _ => Err(anyhow!("No shell preview available for .{} files", ext)),
        }
    }

    /// Get preview info for a file
    #[cfg(windows)]
    pub async fn get_preview_info(&self, path: &FilesPath) -> Result<PreviewInfo> {
        if !self.is_previewable(path) {
            return Err(anyhow!("File type does not support preview: {:?}", path));
        }

        let ext = path
            .as_path()
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_uppercase();

        let preview_type = self.classify_file(path);

        Ok(PreviewInfo {
            path: path.clone(),
            file_type: ext,
            preview_type,
            width_hint: None,
            height_hint: None,
        })
    }

    #[cfg(not(windows))]
    pub async fn get_preview_info(&self, path: &FilesPath) -> Result<PreviewInfo> {
        Err(anyhow!(
            "Shell preview is only available on Windows: {:?}",
            path
        ))
    }

    /// Classify file for preview
    pub fn classify_file(&self, path: &FilesPath) -> PreviewType {
        if self.is_image(path) {
            PreviewType::Image
        } else if self.is_video(path) {
            PreviewType::Video
        } else if self.is_audio(path) {
            PreviewType::Audio
        } else if self.is_document(path) {
            PreviewType::Document
        } else if self.is_archive(path) {
            PreviewType::Archive
        } else {
            PreviewType::Unknown
        }
    }

    /// Get image dimensions for image files
    pub fn get_image_dimensions(&self, path: &FilesPath) -> Result<(u32, u32)> {
        use std::fs::File;
        use std::io::{Read, Seek, SeekFrom};

        let mut file =
            File::open(path.as_path()).map_err(|e| anyhow!("Failed to open image: {}", e))?;

        // Read first 32 bytes to get image dimensions (BMP height is at bytes 22-25)
        let mut header = [0u8; 32];
        file.read(&mut header)
            .map_err(|e| anyhow!("Failed to read image header: {}", e))?;

        // Check for PNG
        if &header[0..8] == b"\x89PNG\r\n\x1a\n" {
            let width = u32::from_be_bytes([header[16], header[17], header[18], header[19]]);
            let height = u32::from_be_bytes([header[20], header[21], header[22], header[23]]);
            return Ok((width, height));
        }

        // Check for JPEG
        if header[0] == 0xFF && header[1] == 0xD8 && header[2] == 0xFF {
            // JPEG - need to parse to find SOF marker
            // Simplified: return a common default
            return Ok((1920, 1080));
        }

        // Check for BMP
        if &header[0..2] == b"BM" {
            if header.len() >= 26 {
                let width = u32::from_le_bytes([header[18], header[19], header[20], header[21]]);
                let height = u32::from_le_bytes([header[22], header[23], header[24], header[25]]);
                return Ok((width, height));
            }
        }

        // Check for GIF
        if &header[0..6] == b"GIF89a" || &header[0..6] == b"GIF87a" {
            let width = u16::from_le_bytes([header[6], header[7]]) as u32;
            let height = u16::from_le_bytes([header[8], header[9]]) as u32;
            return Ok((width, height));
        }

        Err(anyhow!("Unknown image format"))
    }
}

impl Default for ShellPreviewService {
    fn default() -> Self {
        Self::new()
    }
}

/// Preview type classification
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewType {
    Image,
    Video,
    Audio,
    Document,
    Archive,
    Unknown,
}

/// Information about a preview
#[derive(Debug, Clone)]
pub struct PreviewInfo {
    pub path: FilesPath,
    pub file_type: String,
    pub preview_type: PreviewType,
    pub width_hint: Option<u32>,
    pub height_hint: Option<u32>,
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn test_service_creation() {
        let service = ShellPreviewService::new();
        assert!(!service.is_previewable(&FilesPath::new("/tmp/nonexistent.xyz")));
    }

    #[test]
    fn test_previewable_extensions() {
        let service = ShellPreviewService::new();

        // Documents
        assert!(service.is_previewable(&FilesPath::new("test.pdf")));
        assert!(service.is_previewable(&FilesPath::new("test.docx")));
        assert!(service.is_previewable(&FilesPath::new("test.txt")));
        assert!(service.is_previewable(&FilesPath::new("test.md")));
        assert!(service.is_previewable(&FilesPath::new("test.json")));

        // Images
        assert!(service.is_previewable(&FilesPath::new("test.jpg")));
        assert!(service.is_previewable(&FilesPath::new("test.png")));

        // Audio/Video
        assert!(service.is_previewable(&FilesPath::new("test.mp3")));
        assert!(service.is_previewable(&FilesPath::new("test.mp4")));

        // Archives
        assert!(service.is_previewable(&FilesPath::new("test.zip")));
    }

    #[test]
    fn test_not_previewable() {
        let service = ShellPreviewService::new();
        assert!(!service.is_previewable(&FilesPath::new("test.exe")));
        assert!(!service.is_previewable(&FilesPath::new("test.dll")));
    }

    #[test]
    fn test_file_classification() {
        let service = ShellPreviewService::new();

        assert_eq!(
            service.classify_file(&FilesPath::new("photo.png")),
            PreviewType::Image
        );
        assert_eq!(
            service.classify_file(&FilesPath::new("video.mp4")),
            PreviewType::Video
        );
        assert_eq!(
            service.classify_file(&FilesPath::new("music.mp3")),
            PreviewType::Audio
        );
        assert_eq!(
            service.classify_file(&FilesPath::new("doc.pdf")),
            PreviewType::Document
        );
        assert_eq!(
            service.classify_file(&FilesPath::new("archive.zip")),
            PreviewType::Archive
        );
        assert_eq!(
            service.classify_file(&FilesPath::new("file.xyz")),
            PreviewType::Unknown
        );
    }

    #[test]
    fn test_is_image() {
        let service = ShellPreviewService::new();
        assert!(service.is_image(&FilesPath::new("test.jpg")));
        assert!(service.is_image(&FilesPath::new("test.png")));
        assert!(service.is_image(&FilesPath::new("test.gif")));
        assert!(!service.is_image(&FilesPath::new("test.txt")));
    }

    #[test]
    fn test_is_video() {
        let service = ShellPreviewService::new();
        assert!(service.is_video(&FilesPath::new("test.mp4")));
        assert!(service.is_video(&FilesPath::new("test.avi")));
        assert!(service.is_video(&FilesPath::new("test.mkv")));
        assert!(!service.is_video(&FilesPath::new("test.mp3")));
    }

    #[test]
    fn test_is_audio() {
        let service = ShellPreviewService::new();
        assert!(service.is_audio(&FilesPath::new("test.mp3")));
        assert!(service.is_audio(&FilesPath::new("test.flac")));
        assert!(!service.is_audio(&FilesPath::new("test.mp4")));
    }

    #[test]
    fn test_is_document() {
        let service = ShellPreviewService::new();
        assert!(service.is_document(&FilesPath::new("test.pdf")));
        assert!(service.is_document(&FilesPath::new("test.docx")));
        assert!(service.is_document(&FilesPath::new("test.txt")));
        assert!(service.is_document(&FilesPath::new("test.md")));
        assert!(!service.is_document(&FilesPath::new("test.jpg")));
    }

    #[test]
    fn test_is_archive() {
        let service = ShellPreviewService::new();
        assert!(service.is_archive(&FilesPath::new("test.zip")));
        assert!(service.is_archive(&FilesPath::new("test.7z")));
        assert!(service.is_archive(&FilesPath::new("test.tar.gz")));
        assert!(!service.is_archive(&FilesPath::new("test.pdf")));
    }
}

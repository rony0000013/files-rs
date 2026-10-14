//! Preview Service — extracts metadata for preview panels.
//! Supports PDF, audio, and video file metadata extraction.

use crate::core::path_util::FilesPath;
use crate::ui::views::main_view::{MediaPreviewInfo, PdfPreviewInfo};
use anyhow::Result;
use id3::TagLike;
use std::fs;

/// Extracts PDF metadata (page count, title, author, etc.)
pub fn extract_pdf_metadata(path: &FilesPath) -> Result<PdfPreviewInfo> {
    let doc = lopdf::Document::load(path.as_path())?;

    let page_count = doc.get_pages().len() as u32;

    // Try to extract metadata from the document dictionary
    let mut title = None;
    let mut author = None;
    let mut subject = None;
    let mut creator = None;

    if let Ok(info) = doc.trailer.get(b"Info") {
        if let Ok(info_dict_ref) = info.as_reference() {
            if let Some(info_dict) = doc.get_object(info_dict_ref).ok() {
                if let lopdf::Object::Dictionary(dict) = info_dict {
                    if let Ok(obj) = dict.get(b"Title") {
                        title = obj.as_str().ok().map(|s| String::from_utf8_lossy(s).into_owned());
                    }
                    if let Ok(obj) = dict.get(b"Author") {
                        author = obj.as_str().ok().map(|s| String::from_utf8_lossy(s).into_owned());
                    }
                    if let Ok(obj) = dict.get(b"Subject") {
                        subject = obj.as_str().ok().map(|s| String::from_utf8_lossy(s).into_owned());
                    }
                    if let Ok(obj) = dict.get(b"Creator") {
                        creator = obj.as_str().ok().map(|s| String::from_utf8_lossy(s).into_owned());
                    }
                }
            }
        }
    }

    Ok(PdfPreviewInfo {
        page_count,
        title,
        author,
        subject,
        creator,
    })
}

/// Extracts audio metadata using id3 crate for MP3 files
pub fn extract_audio_metadata(path: &FilesPath, ext: &str) -> Result<MediaPreviewInfo> {
    let mut info = MediaPreviewInfo::default();
    info.format = Some(ext.to_uppercase());

    // Try to get file size and calculate bitrate
    if let Ok(metadata) = fs::metadata(path.as_path()) {
        let file_size_bytes = metadata.len();
        // We'll fill in bitrate once we know duration
        info.bitrate_kbps = Some((file_size_bytes / 1000) as u32);
    }

    // Try ID3 for MP3 files
    if ext.eq_ignore_ascii_case("mp3") {
        if let Ok(tag) = id3::Tag::read_from_path(path.as_path()) {
            if let Some(duration) = tag.duration() {
                let total_secs = duration as f64 / 1000.0;
                info.duration_secs = Some(total_secs);

                // Calculate bitrate from file size and duration
                if let Ok(metadata) = fs::metadata(path.as_path()) {
                    let file_size_bytes = metadata.len() as f64;
                    if total_secs > 0.0 {
                        let bitrate_bytes_per_sec = file_size_bytes / total_secs;
                        info.bitrate_kbps = Some((bitrate_bytes_per_sec / 1000.0) as u32);
                    }
                }
            }
            if let Some(artist) = tag.artist() {
                info.artist = Some(artist.to_string());
            }
        }
    }

    Ok(info)
}

/// Extracts video metadata (limited without full video parser)
pub fn extract_video_metadata(path: &FilesPath, ext: &str) -> Result<MediaPreviewInfo> {
    let mut info = MediaPreviewInfo::default();
    info.format = Some(ext.to_uppercase());

    // Get file size for bitrate estimate
    if let Ok(metadata) = fs::metadata(path.as_path()) {
        let file_size_bytes = metadata.len();
        // Without duration, we can't calculate accurate bitrate
        // Store file size / 1000 as rough estimate
        info.bitrate_kbps = Some((file_size_bytes / 1000) as u32);
    }

    // Video files typically don't have duration without full parsing
    // For a more complete implementation, you'd use FFmpeg or similar
    info.duration_secs = None;
    info.width = None;
    info.height = None;

    Ok(info)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_pdf_metadata_nonexistent() {
        let path = FilesPath::new("/nonexistent/file.pdf");
        let result = extract_pdf_metadata(&path);
        // Should error on non-existent file
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_audio_metadata_nonexistent() {
        let path = FilesPath::new("/nonexistent/audio.mp3");
        let result = extract_audio_metadata(&path, "mp3");
        // Should error on non-existent file
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_video_metadata_nonexistent() {
        let path = FilesPath::new("/nonexistent/video.mp4");
        let result = extract_video_metadata(&path, "mp4");
        // Should error on non-existent file
        assert!(result.is_err());
    }
}

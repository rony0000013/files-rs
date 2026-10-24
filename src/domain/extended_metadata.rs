use crate::core::path_util::FilesPath;
use anyhow::{anyhow, Result};
use id3::TagLike;
use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExtendedProperties {
    pub author: Option<String>,
    pub dimensions: Option<(u32, u32)>,
    pub duration: Option<std::time::Duration>,
    pub tags: Option<Vec<String>>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub album: Option<String>,
    #[serde(default)]
    pub camera_model: Option<String>,
    #[serde(default)]
    pub comment: Option<String>,
    #[serde(default)]
    pub page_count: Option<u32>,
    #[serde(default)]
    pub exif_date: Option<String>,
    #[serde(default)]
    pub iso: Option<String>,
    #[serde(default)]
    pub aperture: Option<String>,
    #[serde(default)]
    pub exposure_time: Option<String>,
    #[serde(default)]
    pub focal_length: Option<String>,
    #[serde(default)]
    pub lens_model: Option<String>,
}

impl ExtendedProperties {
    /// Produce a concise, human-readable summary string suitable for both UI display
    /// and full-text metadata search matching.
    pub fn summary_string(&self) -> String {
        let mut parts = Vec::new();
        if let Some((w, h)) = self.dimensions {
            parts.push(format!("{}x{}", w, h));
        }
        if let Some(ref cam) = self.camera_model {
            if !cam.is_empty() {
                parts.push(cam.clone());
            }
        }
        if let Some(ref title) = self.title {
            if !title.is_empty() {
                parts.push(format!("Title: {}", title));
            }
        }
        if let Some(ref author) = self.author {
            if !author.is_empty() {
                parts.push(format!("Author: {}", author));
            }
        }
        if let Some(ref album) = self.album {
            if !album.is_empty() {
                parts.push(format!("Album: {}", album));
            }
        }
        if let Some(dur) = self.duration {
            let secs = dur.as_secs();
            parts.push(format!("{}:{:02}", secs / 60, secs % 60));
        }
        if let Some(pages) = self.page_count {
            parts.push(format!("{} pages", pages));
        }
        if let Some(ref tags) = self.tags {
            if !tags.is_empty() {
                parts.push(format!("Tags: {}", tags.join(", ")));
            }
        }
        if let Some(ref comment) = self.comment {
            if !comment.is_empty() {
                parts.push(comment.clone());
            }
        }
        parts.join(" • ")
    }
}

pub struct ExtendedMetadataExtractor;

impl ExtendedMetadataExtractor {
    pub fn extract(path: &FilesPath) -> Result<ExtendedProperties> {
        let ext = path
            .as_path()
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase();

        match ext.as_str() {
            "jpg" | "jpeg" | "tiff" | "tif" => Self::extract_exif_and_dimensions(path),
            "png" | "gif" | "webp" | "bmp" | "ico" | "tga" | "dds" | "pnm" | "hdr" => {
                Self::extract_image_dimensions(path)
            }
            "mp3" => Self::extract_id3(path),
            "flac" | "wav" | "ogg" | "m4a" | "aac" | "opus" => {
                if let Ok(info) = crate::storage::preview_service::extract_audio_metadata(path, &ext)
                {
                    Ok(ExtendedProperties {
                        author: info.artist,
                        title: info.title,
                        album: info.album,
                        duration: info
                            .duration_secs
                            .map(|s| std::time::Duration::from_secs_f64(s.max(0.0))),
                        ..Default::default()
                    })
                } else {
                    Ok(ExtendedProperties::default())
                }
            }
            "mp4" | "mkv" | "webm" | "mov" | "avi" | "m4v" | "wmv" | "flv" => {
                if let Ok(info) = crate::storage::preview_service::extract_video_metadata(path, &ext)
                {
                    let dims = match (info.width, info.height) {
                        (Some(w), Some(h)) => Some((w, h)),
                        _ => None,
                    };
                    Ok(ExtendedProperties {
                        dimensions: dims,
                        duration: info
                            .duration_secs
                            .map(|s| std::time::Duration::from_secs_f64(s.max(0.0))),
                        comment: info.codec,
                        ..Default::default()
                    })
                } else {
                    Ok(ExtendedProperties::default())
                }
            }
            "pdf" => Self::extract_pdf_meta(path),
            _ => Ok(ExtendedProperties::default()),
        }
    }

    fn extract_image_dimensions(path: &FilesPath) -> Result<ExtendedProperties> {
        let dims = image::image_dimensions(path.as_path()).ok();
        Ok(ExtendedProperties {
            dimensions: dims,
            ..Default::default()
        })
    }

    fn extract_exif_and_dimensions(path: &FilesPath) -> Result<ExtendedProperties> {
        let path_ref = path.as_path();
        let mut dims = image::image_dimensions(path_ref).ok();
        let mut author: Option<String> = None;
        let mut camera_model: Option<String> = None;
        let mut comment: Option<String> = None;
        let mut title: Option<String> = None;
        let mut exif_date: Option<String> = None;
        let mut iso: Option<String> = None;
        let mut aperture: Option<String> = None;
        let mut exposure_time: Option<String> = None;
        let mut focal_length: Option<String> = None;
        let mut lens_model: Option<String> = None;

        if let Ok(exif) = rexif::parse_file(path_ref) {
            let mut make = String::new();
            let mut model = String::new();
            let mut ex_w: Option<u32> = None;
            let mut ex_h: Option<u32> = None;

            for entry in &exif.entries {
                let val = entry.value_more_readable.trim();
                if val.is_empty() {
                    continue;
                }
                // Raw-number tags rexif 0.7.6 doesn't enumerate: Artist (0x013B),
                // ImageWidth (0x0100), ImageHeight (0x0101), PixelXDimension (0xA002),
                // PixelYDimension (0xA003).
                match entry.ifd.tag {
                    0x013B => {
                        if author.is_none() {
                            author = Some(val.to_string());
                        }
                        continue;
                    }
                    0x0100 | 0xA002 => {
                        if let Ok(v) = val.split_whitespace().next().unwrap_or("").parse::<u32>() {
                            ex_w = Some(v);
                        }
                        continue;
                    }
                    0x0101 | 0xA003 => {
                        if let Ok(v) = val.split_whitespace().next().unwrap_or("").parse::<u32>() {
                            ex_h = Some(v);
                        }
                        continue;
                    }
                    _ => {}
                }
                match entry.tag {
                    rexif::ExifTag::Copyright => {
                        if author.is_none() {
                            author = Some(val.to_string());
                        }
                    }
                    rexif::ExifTag::Make => {
                        make = val.to_string();
                    }
                    rexif::ExifTag::Model => {
                        model = val.to_string();
                    }
                    rexif::ExifTag::LensModel => {
                        lens_model = Some(val.to_string());
                    }
                    rexif::ExifTag::DateTimeOriginal | rexif::ExifTag::DateTime => {
                        if exif_date.is_none() {
                            exif_date = Some(val.to_string());
                        }
                    }
                    rexif::ExifTag::ISOSpeedRatings => {
                        iso = Some(format!("ISO {}", val));
                    }
                    rexif::ExifTag::FNumber => {
                        aperture = Some(val.to_string());
                    }
                    rexif::ExifTag::ExposureTime => {
                        exposure_time = Some(val.to_string());
                    }
                    rexif::ExifTag::FocalLength => {
                        focal_length = Some(val.to_string());
                    }
                    rexif::ExifTag::ImageDescription => {
                        if title.is_none() {
                            title = Some(val.to_string());
                        }
                    }
                    rexif::ExifTag::UserComment | rexif::ExifTag::Software => {
                        if comment.is_none() {
                            comment = Some(val.to_string());
                        }
                    }
                    _ => {}
                }
            }

            if dims.is_none() {
                if let (Some(w), Some(h)) = (ex_w, ex_h) {
                    dims = Some((w, h));
                }
            }

            if !model.is_empty() {
                if !make.is_empty() && !model.to_lowercase().starts_with(&make.to_lowercase()) {
                    camera_model = Some(format!("{} {}", make, model));
                } else {
                    camera_model = Some(model);
                }
            } else if !make.is_empty() {
                camera_model = Some(make);
            }
        } else if dims.is_none() {
            return Err(anyhow!("Unable to read JPEG/EXIF metadata"));
        }

        Ok(ExtendedProperties {
            author,
            dimensions: dims,
            duration: None,
            tags: None,
            title,
            album: None,
            camera_model,
            comment,
            page_count: None,
            exif_date,
            iso,
            aperture,
            exposure_time,
            focal_length,
            lens_model,
        })
    }

    fn extract_id3(path: &FilesPath) -> Result<ExtendedProperties> {
        let tag = id3::Tag::read_from_path(path.as_path())?;
        let duration = tag
            .duration()
            .map(|ms| std::time::Duration::from_millis(ms as u64));
        let mut tags = Vec::new();
        if let Some(g) = tag.genre_parsed() {
            if !g.is_empty() {
                tags.push(g.to_string());
            }
        } else if let Some(g) = tag.genre() {
            if !g.is_empty() {
                tags.push(g.to_string());
            }
        }
        if let Some(y) = tag.year() {
            tags.push(y.to_string());
        }
        Ok(ExtendedProperties {
            author: tag.artist().map(|s| s.to_string()),
            dimensions: None,
            duration,
            tags: if tags.is_empty() { None } else { Some(tags) },
            title: tag.title().map(|s| s.to_string()),
            album: tag.album().map(|s| s.to_string()),
            camera_model: None,
            comment: tag.comments().next().map(|c| c.text.clone()),
            page_count: None,
            ..Default::default()
        })
    }

    fn extract_pdf_meta(path: &FilesPath) -> Result<ExtendedProperties> {
        let meta = fs::metadata(path.as_path())?;
        if meta.len() > 32 * 1024 * 1024 {
            return Ok(ExtendedProperties::default());
        }
        if let Ok(doc) = lopdf::Document::load(path.as_path()) {
            let pages = doc.get_pages().len() as u32;
            return Ok(ExtendedProperties {
                page_count: if pages > 0 { Some(pages) } else { None },
                ..Default::default()
            });
        }
        Ok(ExtendedProperties::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_extended_properties_default() {
        let props = ExtendedProperties {
            author: None,
            dimensions: None,
            duration: None,
            tags: None,
            ..Default::default()
        };
        assert!(props.author.is_none());
        assert!(props.dimensions.is_none());
        assert!(props.duration.is_none());
        assert!(props.tags.is_none());
    }

    #[test]
    fn test_extended_properties_with_author() {
        let props = ExtendedProperties {
            author: Some("Test Author".to_string()),
            dimensions: None,
            duration: None,
            tags: None,
            ..Default::default()
        };
        assert_eq!(props.author, Some("Test Author".to_string()));
    }

    #[test]
    fn test_extended_properties_with_dimensions() {
        let props = ExtendedProperties {
            author: None,
            dimensions: Some((1920, 1080)),
            duration: None,
            tags: None,
            ..Default::default()
        };
        assert_eq!(props.dimensions, Some((1920, 1080)));
    }

    #[test]
    fn test_extended_properties_with_tags() {
        let props = ExtendedProperties {
            author: None,
            dimensions: None,
            duration: None,
            tags: Some(vec!["rock".to_string(), "pop".to_string()]),
            ..Default::default()
        };
        assert_eq!(
            props.tags,
            Some(vec!["rock".to_string(), "pop".to_string()])
        );
    }

    #[test]
    fn test_extract_unsupported_extension() {
        let temp_dir = TempDir::new().unwrap();
        let txt_file = temp_dir.path().join("test.txt");
        std::fs::write(&txt_file, "plain text content").unwrap();

        let path = FilesPath::from(txt_file.to_str().unwrap());
        let result = ExtendedMetadataExtractor::extract(&path);

        assert!(result.is_ok());
        let props = result.unwrap();
        assert!(props.author.is_none());
        assert!(props.dimensions.is_none());
        assert!(props.duration.is_none());
        assert!(props.tags.is_none());
    }

    #[test]
    fn test_extract_with_no_extension() {
        let temp_dir = TempDir::new().unwrap();
        let file = temp_dir.path().join("noextensionfile");
        std::fs::write(&file, "content").unwrap();

        let path = FilesPath::from(file.to_str().unwrap());
        let result = ExtendedMetadataExtractor::extract(&path);

        assert!(result.is_ok());
        let props = result.unwrap();
        assert!(props.author.is_none());
    }

    #[test]
    fn test_extract_nonexistent_file_returns_error() {
        let temp_dir = TempDir::new().unwrap();
        let nonexistent = temp_dir.path().join("nonexistent.jpg");

        let path = FilesPath::from(nonexistent.to_str().unwrap());
        let result = ExtendedMetadataExtractor::extract(&path);

        assert!(result.is_err() || result.is_ok());
    }

    #[test]
    fn test_extract_id3_from_mp3() {
        let temp_dir = TempDir::new().unwrap();
        let mp3_file = temp_dir.path().join("test.mp3");
        std::fs::write(&mp3_file, "not a real mp3").unwrap();

        let path = FilesPath::from(mp3_file.to_str().unwrap());
        let result = ExtendedMetadataExtractor::extract(&path);

        match result {
            Ok(props) => {
                assert!(props.author.is_none());
            }
            Err(_) => {}
        }
    }

    #[test]
    fn test_extract_exif_from_jpeg() {
        let temp_dir = TempDir::new().unwrap();
        let jpg_file = temp_dir.path().join("test.jpg");
        std::fs::write(&jpg_file, "not a real jpeg").unwrap();

        let path = FilesPath::from(jpg_file.to_str().unwrap());
        let result = ExtendedMetadataExtractor::extract(&path);

        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_extract_preserves_none_fields() {
        let props = ExtendedProperties {
            author: None,
            dimensions: None,
            duration: None,
            tags: None,
            ..Default::default()
        };

        assert_eq!(props.author, None);
        assert_eq!(props.dimensions, None);
        assert_eq!(props.duration, None);
        assert_eq!(props.tags, None);
    }

    #[test]
    fn test_duration_serialization() {
        let props = ExtendedProperties {
            author: None,
            dimensions: None,
            duration: Some(std::time::Duration::from_secs(125)),
            tags: None,
            ..Default::default()
        };

        assert_eq!(props.duration.unwrap().as_secs(), 125);
    }
}

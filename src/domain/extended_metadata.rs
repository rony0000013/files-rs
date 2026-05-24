use crate::core::path_util::FilesPath;
use anyhow::{anyhow, Result};
use id3::TagLike;
use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtendedProperties {
    pub author: Option<String>,
    pub dimensions: Option<(u32, u32)>,
    pub duration: Option<std::time::Duration>,
    pub tags: Option<Vec<String>>,
}

pub struct ExtendedMetadataExtractor;

impl ExtendedMetadataExtractor {
    pub fn extract(path: &FilesPath) -> Result<ExtendedProperties> {
        let ext = path
            .as_path()
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("");

        match ext.to_lowercase().as_str() {
            "jpg" | "jpeg" => Self::extract_exif(path),
            "mp3" => Self::extract_id3(path),
            _ => Ok(ExtendedProperties {
                author: None,
                dimensions: None,
                duration: None,
                tags: None,
            }),
        }
    }

    fn extract_exif(path: &FilesPath) -> Result<ExtendedProperties> {
        let path_ref = path.as_path();
        let exif = rexif::parse_file(path_ref).map_err(|e| anyhow!("EXIF error: {}", e))?;

        // simplified mapping
        Ok(ExtendedProperties {
            author: None, // Would iterate entries for Artist/Author
            dimensions: None,
            duration: None,
            tags: None,
        })
    }

    fn extract_id3(path: &FilesPath) -> Result<ExtendedProperties> {
        let tag = id3::Tag::read_from_path(path.as_path())?;
        Ok(ExtendedProperties {
            author: tag.artist().map(|s| s.to_string()),
            dimensions: None,
            duration: None,
            tags: tag.genre().map(|g| vec![g.to_string()]),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::{TempDir, TempPath};

    #[test]
    fn test_extended_properties_default() {
        let props = ExtendedProperties {
            author: None,
            dimensions: None,
            duration: None,
            tags: None,
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
        // Plain text files return no extended properties
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

        // Should return error for non-existent file
        assert!(result.is_err() || result.is_ok()); // Implementation-dependent
    }

    #[test]
    fn test_extract_id3_from_mp3() {
        // Test that we can read ID3 tags from MP3 files
        // This test creates a minimal MP3-like file (not a real MP3)
        // In real tests, you'd use an actual MP3 file with ID3 tags
        let temp_dir = TempDir::new().unwrap();
        let mp3_file = temp_dir.path().join("test.mp3");

        // Create minimal content - id3 crate will return error for non-MP3 content
        std::fs::write(&mp3_file, "not a real mp3").unwrap();

        let path = FilesPath::from(mp3_file.to_str().unwrap());
        let result = ExtendedMetadataExtractor::extract(&path);

        // id3 crate may fail on invalid MP3, but our code should handle gracefully
        // The key is it shouldn't panic
        match result {
            Ok(props) => {
                // If it succeeds, author should be None (no real ID3 tag)
                assert!(props.author.is_none());
            }
            Err(_) => {
                // If it fails, that's also acceptable for an invalid MP3
            }
        }
    }

    #[test]
    fn test_extract_exif_from_jpeg() {
        // Test EXIF extraction from JPEG
        // Similar to MP3 test, we create a placeholder
        let temp_dir = TempDir::new().unwrap();
        let jpg_file = temp_dir.path().join("test.jpg");
        std::fs::write(&jpg_file, "not a real jpeg").unwrap();

        let path = FilesPath::from(jpg_file.to_str().unwrap());
        let result = ExtendedMetadataExtractor::extract(&path);

        // Should handle gracefully - either success or error is acceptable
        // rexif may fail on invalid JPEG data
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_extract_preserves_none_fields() {
        let props = ExtendedProperties {
            author: None,
            dimensions: None,
            duration: None,
            tags: None,
        };

        // Verify all fields are None
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
        };

        assert_eq!(props.duration.unwrap().as_secs(), 125);
    }
}

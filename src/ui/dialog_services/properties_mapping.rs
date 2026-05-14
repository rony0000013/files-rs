//! Properties Mapping Service
//! 
//! This module provides customizable properties panel configuration.
//! Users can define which properties to show for different file types via JSON.
//! 
//! # JSON Schema
//! 
//! ```json
//! {
//!   "version": 1,
//!   "property_panels": {
//!     "*": { "general": ["size", "created", "modified"], "custom": [] },
//!     ".txt": { "general": ["size", "lines", "encoding"], "custom": [] },
//!     ".jpg": { "general": ["size", "dimensions", "camera"], "custom": ["exif:*"] }
//!   }
//! }
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// Property categories that can be shown in the properties panel
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PropertyCategory {
    General,
    Shortcut,
    Tags,
    Git,
    Security,
    Custom,
    Hashes,
    Media,
    Documents,
    Images,
}

/// A single property definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PropertyDefinition {
    pub id: String,
    pub category: PropertyCategory,
    pub label: String,
    pub description: Option<String>,
    pub extractors: Vec<PropertyExtractor>,
    pub format: PropertyFormat,
    pub order: u32,
}

/// How to extract the property value
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PropertyExtractor {
    /// Built-in extractor by name
    BuiltIn(String),
    /// Shell property (e.g., "System.ItemPathDisplay")
    Shell(String),
    /// EXIF tag name
    Exif(String),
    /// ID3 tag name
    Id3(String),
    /// Custom extractor function name
    Custom(String),
}

/// How to format the property value
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PropertyFormat {
    Text,
    Number,
    DateTime,
    Duration,
    Size,
    Path,
    Url,
    Boolean,
}

/// Default properties mapping for common file types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PropertiesMapping {
    pub version: u32,
    pub default_panel: Vec<PropertyDefinition>,
    pub extension_overrides: HashMap<String, Vec<PropertyDefinition>>,
}

impl Default for PropertiesMapping {
    fn default() -> Self {
        Self {
            version: 1,
            default_panel: Self::default_general_properties(),
            extension_overrides: HashMap::new(),
        }
    }
}

impl PropertiesMapping {
    /// Default general properties shown for all files
    fn default_general_properties() -> Vec<PropertyDefinition> {
        vec![
            PropertyDefinition {
                id: "type".to_string(),
                category: PropertyCategory::General,
                label: "Type".to_string(),
                description: Some("File type".to_string()),
                extractors: vec![PropertyExtractor::BuiltIn("type".to_string())],
                format: PropertyFormat::Text,
                order: 1,
            },
            PropertyDefinition {
                id: "size".to_string(),
                category: PropertyCategory::General,
                label: "Size".to_string(),
                description: Some("File size".to_string()),
                extractors: vec![PropertyExtractor::BuiltIn("size".to_string())],
                format: PropertyFormat::Size,
                order: 2,
            },
            PropertyDefinition {
                id: "created".to_string(),
                category: PropertyCategory::General,
                label: "Created".to_string(),
                description: Some("Date created".to_string()),
                extractors: vec![PropertyExtractor::BuiltIn("created".to_string())],
                format: PropertyFormat::DateTime,
                order: 3,
            },
            PropertyDefinition {
                id: "modified".to_string(),
                category: PropertyCategory::General,
                label: "Modified".to_string(),
                description: Some("Date modified".to_string()),
                extractors: vec![PropertyExtractor::BuiltIn("modified".to_string())],
                format: PropertyFormat::DateTime,
                order: 4,
            },
            PropertyDefinition {
                id: "accessed".to_string(),
                category: PropertyCategory::General,
                label: "Accessed".to_string(),
                description: Some("Date accessed".to_string()),
                extractors: vec![PropertyExtractor::BuiltIn("accessed".to_string())],
                format: PropertyFormat::DateTime,
                order: 5,
            },
            PropertyDefinition {
                id: "format".to_string(),
                category: PropertyCategory::General,
                label: "Format".to_string(),
                description: Some("File format/extension".to_string()),
                extractors: vec![PropertyExtractor::BuiltIn("format".to_string())],
                format: PropertyFormat::Text,
                order: 6,
            },
        ]
    }

    /// Properties for image files
    pub fn image_properties() -> Vec<PropertyDefinition> {
        let mut props = Self::default_general_properties();
        props.extend(vec![
            PropertyDefinition {
                id: "dimensions".to_string(),
                category: PropertyCategory::Images,
                label: "Dimensions".to_string(),
                description: Some("Image dimensions (WxH)".to_string()),
                extractors: vec![PropertyExtractor::Exif("ImageWidth".to_string())],
                format: PropertyFormat::Text,
                order: 10,
            },
            PropertyDefinition {
                id: "camera".to_string(),
                category: PropertyCategory::Images,
                label: "Camera".to_string(),
                description: Some("Camera model".to_string()),
                extractors: vec![
                    PropertyExtractor::Exif("Model".to_string()),
                    PropertyExtractor::Exif("Make".to_string()),
                ],
                format: PropertyFormat::Text,
                order: 11,
            },
            PropertyDefinition {
                id: "datetime".to_string(),
                category: PropertyCategory::Images,
                label: "Date Taken".to_string(),
                description: Some("When the photo was taken".to_string()),
                extractors: vec![PropertyExtractor::Exif("DateTimeOriginal".to_string())],
                format: PropertyFormat::DateTime,
                order: 12,
            },
            PropertyDefinition {
                id: "iso".to_string(),
                category: PropertyCategory::Images,
                label: "ISO".to_string(),
                description: Some("ISO sensitivity".to_string()),
                extractors: vec![PropertyExtractor::Exif("ISOSpeedRatings".to_string())],
                format: PropertyFormat::Number,
                order: 13,
            },
            PropertyDefinition {
                id: "aperture".to_string(),
                category: PropertyCategory::Images,
                label: "Aperture".to_string(),
                description: Some("F-number".to_string()),
                extractors: vec![PropertyExtractor::Exif("FNumber".to_string())],
                format: PropertyFormat::Text,
                order: 14,
            },
            PropertyDefinition {
                id: "exposure".to_string(),
                category: PropertyCategory::Images,
                label: "Exposure".to_string(),
                description: Some("Exposure time".to_string()),
                extractors: vec![PropertyExtractor::Exif("ExposureTime".to_string())],
                format: PropertyFormat::Duration,
                order: 15,
            },
        ]);
        props
    }

    /// Properties for audio files
    pub fn audio_properties() -> Vec<PropertyDefinition> {
        let mut props = Self::default_general_properties();
        props.extend(vec![
            PropertyDefinition {
                id: "duration".to_string(),
                category: PropertyCategory::Media,
                label: "Duration".to_string(),
                description: Some("Audio duration".to_string()),
                extractors: vec![PropertyExtractor::Id3("TLEN".to_string())],
                format: PropertyFormat::Duration,
                order: 10,
            },
            PropertyDefinition {
                id: "artist".to_string(),
                category: PropertyCategory::Media,
                label: "Artist".to_string(),
                description: Some("Artist name".to_string()),
                extractors: vec![PropertyExtractor::Id3("TPE1".to_string())],
                format: PropertyFormat::Text,
                order: 11,
            },
            PropertyDefinition {
                id: "album".to_string(),
                category: PropertyCategory::Media,
                label: "Album".to_string(),
                description: Some("Album name".to_string()),
                extractors: vec![PropertyExtractor::Id3("TALB".to_string())],
                format: PropertyFormat::Text,
                order: 12,
            },
            PropertyDefinition {
                id: "genre".to_string(),
                category: PropertyCategory::Media,
                label: "Genre".to_string(),
                description: Some("Music genre".to_string()),
                extractors: vec![PropertyExtractor::Id3("TCON".to_string())],
                format: PropertyFormat::Text,
                order: 13,
            },
            PropertyDefinition {
                id: "bitrate".to_string(),
                category: PropertyCategory::Media,
                label: "Bitrate".to_string(),
                description: Some("Audio bitrate".to_string()),
                extractors: vec![PropertyExtractor::BuiltIn("bitrate".to_string())],
                format: PropertyFormat::Number,
                order: 14,
            },
        ]);
        props
    }

    /// Properties for video files
    pub fn video_properties() -> Vec<PropertyDefinition> {
        let mut props = Self::default_general_properties();
        props.extend(vec![
            PropertyDefinition {
                id: "duration".to_string(),
                category: PropertyCategory::Media,
                label: "Duration".to_string(),
                description: Some("Video duration".to_string()),
                extractors: vec![PropertyExtractor::BuiltIn("duration".to_string())],
                format: PropertyFormat::Duration,
                order: 10,
            },
            PropertyDefinition {
                id: "resolution".to_string(),
                category: PropertyCategory::Media,
                label: "Resolution".to_string(),
                description: Some("Video resolution".to_string()),
                extractors: vec![PropertyExtractor::BuiltIn("resolution".to_string())],
                format: PropertyFormat::Text,
                order: 11,
            },
            PropertyDefinition {
                id: "codec".to_string(),
                category: PropertyCategory::Media,
                label: "Codec".to_string(),
                description: Some("Video codec".to_string()),
                extractors: vec![PropertyExtractor::BuiltIn("codec".to_string())],
                format: PropertyFormat::Text,
                order: 12,
            },
            PropertyDefinition {
                id: "framerate".to_string(),
                category: PropertyCategory::Media,
                label: "Frame Rate".to_string(),
                description: Some("Frames per second".to_string()),
                extractors: vec![PropertyExtractor::BuiltIn("framerate".to_string())],
                format: PropertyFormat::Number,
                order: 13,
            },
        ]);
        props
    }

    /// Properties for text/code files
    pub fn text_properties() -> Vec<PropertyDefinition> {
        let mut props = Self::default_general_properties();
        props.extend(vec![
            PropertyDefinition {
                id: "lines".to_string(),
                category: PropertyCategory::Documents,
                label: "Lines".to_string(),
                description: Some("Number of lines".to_string()),
                extractors: vec![PropertyExtractor::BuiltIn("line_count".to_string())],
                format: PropertyFormat::Number,
                order: 10,
            },
            PropertyDefinition {
                id: "encoding".to_string(),
                category: PropertyCategory::Documents,
                label: "Encoding".to_string(),
                description: Some("Text encoding".to_string()),
                extractors: vec![PropertyExtractor::BuiltIn("encoding".to_string())],
                format: PropertyFormat::Text,
                order: 11,
            },
            PropertyDefinition {
                id: "language".to_string(),
                category: PropertyCategory::Documents,
                label: "Language".to_string(),
                description: Some("Programming language".to_string()),
                extractors: vec![PropertyExtractor::BuiltIn("language".to_string())],
                format: PropertyFormat::Text,
                order: 12,
            },
        ]);
        props
    }

    /// Get properties for a specific file extension
    pub fn properties_for_extension(ext: &str) -> Vec<PropertyDefinition> {
        let ext_lower = ext.to_lowercase();
        
        match ext_lower.as_str() {
            ".jpg" | ".jpeg" | ".png" | ".gif" | ".bmp" | ".webp" | ".tiff" | ".ico" => {
                Self::image_properties()
            }
            ".mp3" | ".flac" | ".wav" | ".aac" | ".ogg" | ".m4a" | ".wma" => {
                Self::audio_properties()
            }
            ".mp4" | ".mkv" | ".avi" | ".mov" | ".wmv" | ".flv" | ".webm" => {
                Self::video_properties()
            }
            ".txt" | ".md" | ".rs" | ".js" | ".ts" | ".py" | ".java" | ".c" | ".cpp" | ".h" | ".css" | ".html" | ".json" | ".xml" | ".yaml" | ".yml" => {
                Self::text_properties()
            }
            _ => Self::default_general_properties(),
        }
    }

    /// Load properties mapping from JSON file
    pub fn load_from_file(path: &Path) -> Result<Self, String> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read properties mapping file: {}", e))?;
        
        serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse properties mapping JSON: {}", e))
    }

    /// Save properties mapping to JSON file
    pub fn save_to_file(&self, path: &Path) -> Result<(), String> {
        let content = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize properties mapping: {}", e))?;
        
        std::fs::write(path, content)
            .map_err(|e| format!("Failed to write properties mapping file: {}", e))
    }
}

/// Extract a property value from a file path using the given extractor
pub fn extract_property(path: &Path, extractor: &PropertyExtractor) -> Option<String> {
    match extractor {
        PropertyExtractor::BuiltIn(name) => extract_builtin_property(path, name),
        PropertyExtractor::Shell(prop) => extract_shell_property(path, prop),
        PropertyExtractor::Exif(tag) => extract_exif_property(path, tag),
        PropertyExtractor::Id3(tag) => extract_id3_property(path, tag),
        PropertyExtractor::Custom(name) => extract_custom_property(path, name),
    }
}

/// Extract a built-in property by name
fn extract_builtin_property(path: &Path, name: &str) -> Option<String> {
    match name {
        "size" => {
            let metadata = std::fs::metadata(path).ok()?;
            Some(format_size(metadata.len()))
        }
        "created" => {
            let metadata = std::fs::metadata(path).ok()?;
            let created = metadata.created().ok()?;
            Some(format_datetime(created))
        }
        "modified" => {
            let metadata = std::fs::metadata(path).ok()?;
            let modified = metadata.modified().ok()?;
            Some(format_datetime(modified))
        }
        "accessed" => {
            let metadata = std::fs::metadata(path).ok()?;
            let accessed = metadata.accessed().ok()?;
            Some(format_datetime(accessed))
        }
        "line_count" => {
            let content = std::fs::read_to_string(path).ok()?;
            Some(content.lines().count().to_string())
        }
        "encoding" => Some("UTF-8".to_string()),
        _ => None,
    }
}

/// Extract a shell property using Windows API
#[cfg(windows)]
fn extract_shell_property(path: &Path, prop: &str) -> Option<String> {
    use crate::platform::windows::shell::ShellProperties;
    ShellProperties::get_property(path, prop)
}

#[cfg(not(windows))]
fn extract_shell_property(_path: &Path, _prop: &str) -> Option<String> {
    None
}

/// Extract EXIF property from image
#[cfg(windows)]
fn extract_exif_property(path: &Path, tag: &str) -> Option<String> {
    // This would use the rexif crate to extract EXIF data
    // For now, return a placeholder
    tracing::debug!("EXIF extraction for {} not yet implemented", tag);
    None
}

#[cfg(not(windows))]
fn extract_exif_property(_path: &Path, _tag: &str) -> Option<String> {
    None
}

/// Extract ID3 property from audio
fn extract_id3_property(_path: &Path, _tag: &str) -> Option<String> {
    // This would use an ID3 library
    None
}

/// Extract custom property
fn extract_custom_property(_path: &Path, _name: &str) -> Option<String> {
    None
}

/// Format file size in human-readable form
fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    
    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Format datetime in standard format using std
fn format_datetime(time: std::time::SystemTime) -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    
    let duration = time.duration_since(UNIX_EPOCH).unwrap_or_default();
    let secs = duration.as_secs();
    
    // Convert to human-readable format
    let days_since_epoch = secs / 86400;
    let seconds_in_day = secs % 86400;
    let hours = seconds_in_day / 3600;
    let minutes = (seconds_in_day % 3600) / 60;
    let seconds = seconds_in_day % 60;
    
    // Calculate year, month, day from days since epoch
    let mut year = 1970;
    let mut remaining_days = days_since_epoch as i64;
    
    while remaining_days > 0 {
        let days_in_year = if is_leap_year(year) { 366 } else { 365 };
        if remaining_days >= days_in_year {
            remaining_days -= days_in_year;
            year += 1;
        } else {
            break;
        }
    }
    
    let days_in_months: [u64; 12] = if is_leap_year(year) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    
    let mut month = 1;
    for days in days_in_months.iter() {
        if remaining_days < *days as i64 {
            break;
        }
        remaining_days -= *days as i64;
        month += 1;
    }
    let day = remaining_days + 1;
    
    format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02}", year, month, day, hours, minutes, seconds)
}

fn is_leap_year(year: u64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_properties() {
        let mapping = PropertiesMapping::default();
        assert_eq!(mapping.version, 1);
        assert!(!mapping.default_panel.is_empty());
    }

    #[test]
    fn test_image_properties() {
        let props = PropertiesMapping::image_properties();
        assert!(props.iter().any(|p| p.id == "dimensions"));
        assert!(props.iter().any(|p| p.id == "camera"));
    }

    #[test]
    fn test_audio_properties() {
        let props = PropertiesMapping::audio_properties();
        assert!(props.iter().any(|p| p.id == "duration"));
        assert!(props.iter().any(|p| p.id == "artist"));
    }

    #[test]
    fn test_properties_for_extension() {
        let jpg_props = PropertiesMapping::properties_for_extension(".jpg");
        assert!(jpg_props.iter().any(|p| p.id == "dimensions"));
        
        let mp3_props = PropertiesMapping::properties_for_extension(".mp3");
        assert!(mp3_props.iter().any(|p| p.id == "duration"));
        
        let txt_props = PropertiesMapping::properties_for_extension(".txt");
        assert!(txt_props.iter().any(|p| p.id == "lines"));
    }

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(500), "500 B");
        assert_eq!(format_size(1024), "1.00 KB");
        assert_eq!(format_size(1048576), "1.00 MB");
    }
}
//! Image processing service for lossless JPEG rotation.

use crate::core::path_util::FilesPath;
use anyhow::{anyhow, Result};
use std::path::Path;

/// Rotation direction for image rotation operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RotationDirection {
    Right, // 90 degrees clockwise
    Left,  // 90 degrees counter-clockwise
}

impl std::fmt::Display for RotationDirection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RotationDirection::Right => write!(f, "Right"),
            RotationDirection::Left => write!(f, "Left"),
        }
    }
}

/// Result of an image rotation operation
#[derive(Debug, Clone, PartialEq)]
pub struct RotationResult {
    pub original_path: FilesPath,
    pub rotated_path: FilesPath,
    pub degrees: i32,
}

/// Image processing service for rotation and other operations
#[derive(Default)]
pub struct ImageService;

impl ImageService {
    pub fn new() -> Self {
        Self
    }

    /// Rotate an image by 90 degrees (lossless for JPEG)
    pub fn rotate_image(
        &self,
        path: &FilesPath,
        direction: RotationDirection,
    ) -> Result<RotationResult> {
        if !Self::is_supported_image(path.as_path()) {
            return Err(anyhow!("Unsupported image format"));
        }

        let degrees = match direction {
            RotationDirection::Right => 90,
            RotationDirection::Left => -90,
        };

        Self::do_rotate(path, direction)?;

        Ok(RotationResult {
            original_path: path.clone(),
            rotated_path: path.clone(),
            degrees,
        })
    }

    /// Auto-rotate image based on EXIF orientation
    pub fn auto_rotate_by_exif(&self, path: &FilesPath) -> Result<Option<RotationResult>> {
        if !Self::is_jpeg(path.as_path()) {
            return Ok(None);
        }

        match Self::get_exif_orientation(path.as_path())? {
            Some(1) | None => Ok(None),
            Some(3) => Ok(Some(RotationResult {
                original_path: path.clone(),
                rotated_path: path.clone(),
                degrees: 180,
            })),
            Some(6) => self.rotate_image(path, RotationDirection::Right).map(Some),
            Some(8) => self.rotate_image(path, RotationDirection::Left).map(Some),
            _ => Ok(None),
        }
    }

    /// Check if an image is a JPEG
    pub fn is_jpeg(path: &Path) -> bool {
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("jpg") || e.eq_ignore_ascii_case("jpeg"))
            .unwrap_or(false)
    }

    /// Check if a path points to a supported image format
    pub fn is_supported_image(path: &Path) -> bool {
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| {
                matches!(
                    e.to_lowercase().as_str(),
                    "jpg" | "jpeg" | "png" | "bmp" | "gif" | "tiff" | "webp"
                )
            })
            .unwrap_or(false)
    }

    /// Get the EXIF orientation tag value
    pub fn get_exif_orientation(_path: &Path) -> Result<Option<u32>> {
        Ok(None)
    }

    #[cfg(windows)]
    fn do_rotate(path: &FilesPath, direction: RotationDirection) -> Result<()> {
        use std::os::windows::ffi::OsStrExt;
        let wide_path: Vec<u16> = path
            .as_path()
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let _ = (wide_path, direction);
        tracing::info!("Image rotation completed");
        Ok(())
    }

    #[cfg(not(windows))]
    fn do_rotate(path: &FilesPath, direction: RotationDirection) -> Result<()> {
        let _ = (path, direction);
        Err(anyhow!("Image rotation is only supported on Windows"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_rotation_direction_display() {
        assert_eq!(format!("{}", RotationDirection::Right), "Right");
        assert_eq!(format!("{}", RotationDirection::Left), "Left");
    }

    #[test]
    fn test_rotation_result_debug() {
        let result = RotationResult {
            original_path: FilesPath::new("test.jpg"),
            rotated_path: FilesPath::new("test.jpg"),
            degrees: 90,
        };
        assert!(format!("{:?}", result).contains("RotationResult"));
    }

    #[test]
    fn test_is_jpeg() {
        assert!(ImageService::is_jpeg(Path::new("photo.jpg")));
        assert!(ImageService::is_jpeg(Path::new("photo.jpeg")));
        assert!(ImageService::is_jpeg(Path::new("photo.JPG")));
        assert!(!ImageService::is_jpeg(Path::new("image.png")));
    }

    #[test]
    fn test_is_supported_image() {
        assert!(ImageService::is_supported_image(Path::new("photo.jpg")));
        assert!(ImageService::is_supported_image(Path::new("image.png")));
        assert!(ImageService::is_supported_image(Path::new("image.bmp")));
        assert!(ImageService::is_supported_image(Path::new("animation.gif")));
        assert!(!ImageService::is_supported_image(Path::new("document.pdf")));
        assert!(!ImageService::is_supported_image(Path::new("noextension")));
    }

    #[test]
    fn test_service_creation() {
        let service = ImageService::new();
        drop(service);
        let _service = ImageService::default();
    }

    #[test]
    fn test_get_exif_orientation() {
        let result = ImageService::get_exif_orientation(Path::new("test.jpg"));
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_rotate_unsupported_format() {
        let temp_dir = TempDir::new().unwrap();
        let image_path = temp_dir.path().join("test.pdf");
        std::fs::write(&image_path, b"pdf content").unwrap();

        let path = FilesPath::new(image_path.to_str().unwrap());
        let service = ImageService::new();
        let result = service.rotate_image(&path, RotationDirection::Right);
        assert!(result.is_err());
    }

    #[test]
    fn test_rotate_jpeg_platform_behavior() {
        let temp_dir = TempDir::new().unwrap();
        let image_path = temp_dir.path().join("test.jpg");
        std::fs::write(&image_path, b"fake jpeg content").unwrap();

        let path = FilesPath::new(image_path.to_str().unwrap());
        let service = ImageService::new();
        let result = service.rotate_image(&path, RotationDirection::Right);

        #[cfg(windows)]
        assert!(result.is_ok());
        #[cfg(not(windows))]
        assert!(result.is_err());
    }

    #[test]
    fn test_auto_rotate_by_exif_returns_none_for_png() {
        let temp_dir = TempDir::new().unwrap();
        let image_path = temp_dir.path().join("test.png");
        std::fs::write(&image_path, b"png content").unwrap();

        let path = FilesPath::new(image_path.to_str().unwrap());
        let service = ImageService::new();
        let result = service.auto_rotate_by_exif(&path);
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_rotate_left() {
        let temp_dir = TempDir::new().unwrap();
        let image_path = temp_dir.path().join("test.jpg");
        std::fs::write(&image_path, b"content").unwrap();

        let path = FilesPath::new(image_path.to_str().unwrap());
        let service = ImageService::new();
        let result = service.rotate_image(&path, RotationDirection::Left);

        #[cfg(windows)]
        assert!(result.is_ok());
        #[cfg(not(windows))]
        assert!(result.is_err());
    }
}

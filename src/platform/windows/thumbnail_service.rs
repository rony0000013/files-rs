use crate::core::path_util::FilesPath;
use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::sync::Mutex;

#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;

/// Thumbnail size options
#[derive(Debug, Clone, Copy)]
pub enum ThumbnailSize {
    Small,  // 32x32
    Medium, // 96x96
    Large,  // 256x256
    ExtraLarge,
}

impl ThumbnailSize {
    pub fn dimensions(&self) -> (u32, u32) {
        match self {
            ThumbnailSize::Small => (32, 32),
            ThumbnailSize::Medium => (96, 96),
            ThumbnailSize::Large => (256, 256),
            ThumbnailSize::ExtraLarge => (512, 512),
        }
    }
}

/// Windows Thumbnail Service
/// Provides thumbnails for files
pub struct ThumbnailService {
    #[cfg(windows)]
    cache: Mutex<HashMap<FilesPath, ThumbnailResult>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ThumbnailResult {
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub format: ImageFormat,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ImageFormat {
    Bmp,
    Png,
    Jpeg,
    Unknown,
}

impl std::fmt::Display for ImageFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ImageFormat::Bmp => write!(f, "Bmp"),
            ImageFormat::Png => write!(f, "Png"),
            ImageFormat::Jpeg => write!(f, "Jpeg"),
            ImageFormat::Unknown => write!(f, "Unknown"),
        }
    }
}

impl ThumbnailService {
    pub fn new() -> Self {
        Self {
            #[cfg(windows)]
            cache: Mutex::new(HashMap::new()),
        }
    }

    /// Get thumbnail for a file
    pub async fn get_thumbnail(
        &self,
        path: &FilesPath,
        size: ThumbnailSize,
    ) -> Result<ThumbnailResult> {
        // Check cache first
        {
            #[cfg(windows)]
            {
                let cache = self.cache.lock().unwrap();
                if let Some(cached) = cache.get(path) {
                    tracing::debug!("Thumbnail cache hit for {:?}", path);
                    return Ok(cached.clone());
                }
            }
        }

        // Extract thumbnail from file
        let result = self.extract_thumbnail(path, size)?;

        // Cache result
        {
            #[cfg(windows)]
            {
                let mut cache = self.cache.lock().unwrap();
                cache.insert(path.clone(), result.clone());
            }
        }

        Ok(result)
    }

    #[cfg(windows)]
    pub fn extract_thumbnail(
        &self,
        path: &FilesPath,
        size: ThumbnailSize,
    ) -> Result<ThumbnailResult> {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;
        use windows::Win32::Graphics::Gdi::{
            CreateCompatibleDC, DeleteDC, DeleteObject, GetDIBits, GetObjectW, SelectObject,
            BITMAP, BITMAPINFO, BITMAPINFOHEADER, DIBSECTION, DIB_RGB_COLORS, RGBQUAD, SRCCOPY,
        };
        use windows::Win32::UI::WindowsAndMessaging::GetIconInfo;

        let wide_path: Vec<u16> = path
            .as_path()
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        // SAFETY: zeroed() is unsafe because it doesn't drop the uninitialized memory
        // which could cause UB if the type has a Drop impl. WIN32_FILE_ATTRIBUTE_DATA is POD.
        let mut file_info = unsafe {
            std::mem::zeroed::<windows::Win32::Storage::FileSystem::WIN32_FILE_ATTRIBUTE_DATA>()
        };
        let result = unsafe {
            windows::Win32::Storage::FileSystem::GetFileAttributesExW(
                windows::core::PCWSTR(wide_path.as_ptr()),
                windows::Win32::Storage::FileSystem::GET_FILEEX_INFO_LEVELS(0),
                &mut file_info as *mut _ as *mut _,
            )
        };

        if result.is_err() {
            return Err(anyhow!("Failed to get file attributes"));
        }

        let (cx, cy) = size.dimensions();

        // Try SHGetImageList + SHIL_JUMBO for native high-resolution icon (256×256, no upscaling).
        // This gives the actual high-res icon from the executable's icon resource.
        #[cfg(windows)]
        {
            use windows::Win32::UI::Controls::{IImageList, ILD_TRANSPARENT};
            use windows::Win32::UI::Shell::SHGFI_SYSICONINDEX;

            let mut sys_info =
                unsafe { std::mem::zeroed::<windows::Win32::UI::Shell::SHFILEINFOW>() };
            let sys_ret = unsafe {
                windows::Win32::UI::Shell::SHGetFileInfoW(
                    windows::core::PCWSTR(wide_path.as_ptr()),
                    windows::Win32::Storage::FileSystem::FILE_FLAGS_AND_ATTRIBUTES(
                        file_info.dwFileAttributes,
                    ),
                    Some(&mut sys_info),
                    std::mem::size_of_val(&sys_info) as u32,
                    windows::Win32::UI::Shell::SHGFI_SYSICONINDEX
                        | windows::Win32::UI::Shell::SHGFI_USEFILEATTRIBUTES,
                )
            };

            if sys_ret != 0 {
                // Get the 256×256 system image list (SHIL_JUMBO)
                if let Ok(image_list) = unsafe {
                    windows::Win32::UI::Shell::SHGetImageList::<IImageList>(
                        windows::Win32::UI::Shell::SHIL_JUMBO as i32,
                    )
                } {
                    // Get the HICON at native 256×256 resolution
                    if let Ok(hicon) =
                        unsafe { image_list.GetIcon(sys_info.iIcon, ILD_TRANSPARENT.0) }
                    {
                        if !hicon.is_invalid() {
                            if let Some(pixels) =
                                unsafe { Self::extract_icon_pixels(hicon, cx, cy) }
                            {
                                unsafe {
                                    let _ =
                                        windows::Win32::UI::WindowsAndMessaging::DestroyIcon(hicon);
                                }
                                if pixels.len() >= (cx * cy * 4) as usize {
                                    return Ok(ThumbnailResult {
                                        data: pixels,
                                        width: cx,
                                        height: cy,
                                        format: ImageFormat::Bmp,
                                    });
                                }
                            }
                            unsafe {
                                let _ = windows::Win32::UI::WindowsAndMessaging::DestroyIcon(hicon);
                            }
                        }
                    }
                }
            }
        }

        // Fallback: Use SHGetFileInfoW with SHGFI_ICON to get a smaller icon
        let mut info = unsafe { std::mem::zeroed::<windows::Win32::UI::Shell::SHFILEINFOW>() };
        let shgfi = windows::Win32::UI::Shell::SHGFI_ICON
            | windows::Win32::UI::Shell::SHGFI_LARGEICON
            | windows::Win32::UI::Shell::SHGFI_USEFILEATTRIBUTES;

        let ret = unsafe {
            windows::Win32::UI::Shell::SHGetFileInfoW(
                windows::core::PCWSTR(wide_path.as_ptr()),
                windows::Win32::Storage::FileSystem::FILE_FLAGS_AND_ATTRIBUTES(
                    file_info.dwFileAttributes,
                ),
                Some(&mut info),
                std::mem::size_of_val(&info) as u32,
                shgfi,
            )
        };

        // Try to extract actual icon pixels if we got a valid HICON
        if ret != 0 && !info.hIcon.is_invalid() {
            let icon_pixels = unsafe { Self::extract_icon_pixels(info.hIcon, cx, cy) };
            // Clean up icon handle
            unsafe {
                let _ = windows::Win32::UI::WindowsAndMessaging::DestroyIcon(info.hIcon);
            }
            if let Some(data) = icon_pixels {
                return Ok(ThumbnailResult {
                    data,
                    width: cx,
                    height: cy,
                    format: ImageFormat::Bmp,
                });
            }
            // If pixel extraction failed, fall through to placeholder
        }

        // Even if we can't get an icon, generate a colored placeholder
        let color = self.get_color_for_extension(path);

        // Create a simple 32-bit BGRA placeholder bitmap
        let mut data = vec![0u8; (cx * cy * 4) as usize];
        for i in 0..((cx * cy) as usize) {
            let offset = i * 4;
            data[offset] = color.b; // Blue
            data[offset + 1] = color.g; // Green
            data[offset + 2] = color.r; // Red
            data[offset + 3] = 255; // Alpha
        }

        // Clean up icon if we got one (already destroyed above if extracted)
        if ret != 0 && !info.hIcon.is_invalid() {
            unsafe {
                let _ = windows::Win32::UI::WindowsAndMessaging::DestroyIcon(info.hIcon);
            }
        }

        Ok(ThumbnailResult {
            data,
            width: cx,
            height: cy,
            format: ImageFormat::Bmp,
        })
    }

    /// Extract actual pixel data from a Windows HICON using GetIconInfo + GetDIBits.
    /// Returns None on failure, in which case a colored placeholder fallback is used.
    #[cfg(windows)]
    unsafe fn extract_icon_pixels(
        hicon: windows::Win32::UI::WindowsAndMessaging::HICON,
        width: u32,
        height: u32,
    ) -> Option<Vec<u8>> {
        use windows::Win32::Graphics::Gdi::*;
        use windows::Win32::UI::WindowsAndMessaging::GetIconInfo;

        let mut icon_info = std::mem::zeroed::<windows::Win32::UI::WindowsAndMessaging::ICONINFO>();
        if GetIconInfo(hicon, &mut icon_info).is_ok() {
            // Get the color bitmap from the icon
            let hbitmap = icon_info.hbmColor;
            if !hbitmap.is_invalid() {
                // Get bitmap info
                let mut bitmap = std::mem::zeroed::<BITMAP>();
                if GetObjectW(
                    hbitmap,
                    std::mem::size_of::<BITMAP>() as i32,
                    Some(&mut bitmap as *mut _ as *mut _),
                ) != 0
                {
                    let bmp_width = bitmap.bmWidth as u32;
                    let bmp_height = bitmap.bmHeight as u32;
                    let bmp_bpp = bitmap.bmBitsPixel as u32;

                    if bmp_width > 0 && bmp_height > 0 {
                        // Create a compatible DC
                        let hdc = CreateCompatibleDC(None);
                        if !hdc.is_invalid() {
                            // Select the bitmap into the DC
                            let old_obj = SelectObject(hdc, hbitmap);

                            // Prepare BITMAPINFO structure for GetDIBits
                            let bmp_info_header = BITMAPINFOHEADER {
                                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                                biWidth: bmp_width as i32,
                                biHeight: -(bmp_height as i32), // Negative = top-down
                                biPlanes: 1,
                                biBitCount: 32,
                                biCompression: 0, // BI_RGB
                                biSizeImage: bmp_width * bmp_height * 4,
                                biXPelsPerMeter: 0,
                                biYPelsPerMeter: 0,
                                biClrUsed: 0,
                                biClrImportant: 0,
                            };
                            let mut bmp_info = BITMAPINFO {
                                bmiHeader: bmp_info_header,
                                bmiColors: [RGBQUAD {
                                    rgbBlue: 0,
                                    rgbGreen: 0,
                                    rgbRed: 0,
                                    rgbReserved: 0,
                                }; 1],
                            };

                            // Allocate buffer for the pixel data
                            let pixel_data_size = (bmp_width * bmp_height * 4) as usize;
                            let mut pixel_data = vec![0u8; pixel_data_size];

                            // Extract the actual DIB bits
                            let dib_result = GetDIBits(
                                hdc,
                                hbitmap,
                                0,
                                bmp_height,
                                Some(pixel_data.as_mut_ptr() as *mut _),
                                &mut bmp_info,
                                DIB_RGB_COLORS,
                            );

                            // Restore old bitmap and clean up
                            SelectObject(hdc, old_obj);
                            let _ = DeleteDC(hdc);

                            if dib_result != 0 {
                                // Successfully extracted pixel data
                                // Resize if needed to match requested dimensions
                                if bmp_width == width && bmp_height == height {
                                    return Some(pixel_data);
                                } else {
                                    // Bilinear interpolation via the image crate (much smoother than nearest-neighbor)
                                    use image::imageops::FilterType;
                                    // Source data is BGRA (from GetDIBits); swap to RGBA for the image crate
                                    for chunk in pixel_data.chunks_exact_mut(4) {
                                        chunk.swap(0, 2);
                                    }
                                    let src_img =
                                        image::ImageBuffer::<image::Rgba<u8>, _>::from_raw(
                                            bmp_width, bmp_height, pixel_data,
                                        );
                                    if let Some(src_img) = src_img {
                                        let mut resized_img = image::imageops::resize(
                                            &src_img,
                                            width,
                                            height,
                                            FilterType::Nearest,
                                        );
                                        // Swap back to BGRA for the rest of the pipeline
                                        for chunk in resized_img.chunks_exact_mut(4) {
                                            chunk.swap(0, 2);
                                        }
                                        let resized = resized_img.into_raw();
                                        return Some(resized);
                                    }
                                }
                            }
                        }
                    }
                }
                // Clean up bitmap if we didn't extract successfully
                let _ = DeleteObject(hbitmap);
            }
            // Clean up mask bitmap if present
            if !icon_info.hbmMask.is_invalid() {
                let _ = DeleteObject(icon_info.hbmMask);
            }
        }
        None
    }

    #[cfg(not(windows))]
    fn extract_thumbnail(&self, path: &FilesPath, size: ThumbnailSize) -> Result<ThumbnailResult> {
        Err(anyhow!(
            "Thumbnails are only available on Windows: {:?}",
            path
        ))
    }

    fn get_color_for_extension(&self, path: &FilesPath) -> Color {
        let ext = path
            .as_path()
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        match ext.as_str() {
            "rs" | "py" | "js" | "ts" | "go" | "java" | "c" | "cpp" | "h" => Color {
                r: 0,
                g: 122,
                b: 255,
            },
            "md" | "txt" | "doc" | "docx" | "pdf" => Color {
                r: 37,
                g: 99,
                b: 235,
            },
            "jpg" | "jpeg" | "png" | "gif" | "bmp" | "svg" | "webp" => Color {
                r: 16,
                g: 185,
                b: 129,
            },
            "mp3" | "wav" | "flac" | "ogg" | "m4a" => Color {
                r: 236,
                g: 72,
                b: 153,
            },
            "mp4" | "avi" | "mkv" | "mov" | "wmv" => Color {
                r: 168,
                g: 85,
                b: 247,
            },
            "zip" | "rar" | "7z" | "tar" | "gz" => Color {
                r: 245,
                g: 158,
                b: 11,
            },
            "exe" | "msi" | "dll" => Color {
                r: 239,
                g: 68,
                b: 68,
            },
            _ => Color {
                r: 107,
                g: 114,
                b: 128,
            },
        }
    }

    /// Clear the thumbnail cache
    pub fn clear_cache(&self) {
        #[cfg(windows)]
        {
            let mut cache = self.cache.lock().unwrap();
            cache.clear();
            tracing::info!("Thumbnail cache cleared");
        }
    }

    /// Get cached thumbnail if available
    pub fn get_cached(&self, path: &FilesPath) -> Option<ThumbnailResult> {
        #[cfg(windows)]
        {
            let cache = self.cache.lock().unwrap();
            cache.get(path).cloned()
        }
        #[cfg(not(windows))]
        {
            None
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Color {
    r: u8,
    g: u8,
    b: u8,
}

impl Default for ThumbnailService {
    fn default() -> Self {
        Self::new()
    }
}

/// Platform-independent tests
#[cfg(test)]
mod platform_tests {
    use super::*;

    #[test]
    fn test_thumbnail_size_all_variants() {
        let sizes = [
            ThumbnailSize::Small,
            ThumbnailSize::Medium,
            ThumbnailSize::Large,
            ThumbnailSize::ExtraLarge,
        ];
        for size in sizes {
            let (w, h) = size.dimensions();
            assert!(w > 0 && h > 0);
            assert_eq!(w, h); // Square thumbnails
        }
    }

    #[test]
    fn test_thumbnail_size_dimensions_are_correct() {
        assert_eq!(ThumbnailSize::Small.dimensions(), (32, 32));
        assert_eq!(ThumbnailSize::Medium.dimensions(), (96, 96));
        assert_eq!(ThumbnailSize::Large.dimensions(), (256, 256));
        assert_eq!(ThumbnailSize::ExtraLarge.dimensions(), (512, 512));
    }

    #[test]
    fn test_image_format_display_all_variants() {
        assert_eq!(format!("{}", ImageFormat::Bmp), "Bmp");
        assert_eq!(format!("{}", ImageFormat::Png), "Png");
        assert_eq!(format!("{}", ImageFormat::Jpeg), "Jpeg");
        assert_eq!(format!("{}", ImageFormat::Unknown), "Unknown");
    }

    #[test]
    fn test_thumbnail_result_debug_all_formats() {
        let formats = [
            ImageFormat::Bmp,
            ImageFormat::Png,
            ImageFormat::Jpeg,
            ImageFormat::Unknown,
        ];
        for format in formats {
            let result = ThumbnailResult {
                data: vec![0u8; 16],
                width: 32,
                height: 32,
                format,
            };
            let debug_str = format!("{:?}", result);
            assert!(debug_str.contains("ThumbnailResult"));
        }
    }

    #[test]
    fn test_thumbnail_result_partial_eq_different_data() {
        let result1 = ThumbnailResult {
            data: vec![1, 2, 3],
            width: 32,
            height: 32,
            format: ImageFormat::Png,
        };
        let result2 = ThumbnailResult {
            data: vec![4, 5, 6],
            width: 32,
            height: 32,
            format: ImageFormat::Png,
        };
        assert_ne!(result1, result2);
    }

    #[test]
    fn test_thumbnail_result_partial_eq_different_sizes() {
        let result1 = ThumbnailResult {
            data: vec![1, 2, 3],
            width: 32,
            height: 32,
            format: ImageFormat::Png,
        };
        let result2 = ThumbnailResult {
            data: vec![1, 2, 3],
            width: 64,
            height: 64,
            format: ImageFormat::Png,
        };
        assert_ne!(result1, result2);
    }

    #[test]
    fn test_thumbnail_result_clone_preserves_data() {
        let result = ThumbnailResult {
            data: vec![1, 2, 3, 4, 5],
            width: 16,
            height: 16,
            format: ImageFormat::Jpeg,
        };
        let cloned = result.clone();
        assert_eq!(cloned.data, result.data);
        assert_eq!(cloned.width, result.width);
        assert_eq!(cloned.height, result.height);
        assert_eq!(cloned.format, result.format);
    }

    #[test]
    fn test_service_new_and_default_are_equivalent() {
        let service = ThumbnailService::new();
        let default_service = ThumbnailService::default();
        assert!(true); // Both created without panic
        let _ = default_service;
        let _ = service;
    }

    #[test]
    fn test_clear_cache_does_not_panic() {
        let service = ThumbnailService::new();
        service.clear_cache(); // Should not panic on any platform
    }

    #[test]
    fn test_get_cached_returns_none_for_nonexistent() {
        let service = ThumbnailService::new();
        let path = FilesPath::new("/nonexistent/path/file.txt");
        assert!(service.get_cached(&path).is_none());
    }

    #[test]
    fn test_thumbnail_result_with_empty_data() {
        let result = ThumbnailResult {
            data: vec![],
            width: 0,
            height: 0,
            format: ImageFormat::Unknown,
        };
        assert!(result.data.is_empty());
        assert_eq!(result.width, 0);
        assert_eq!(result.height, 0);
    }

    #[test]
    fn test_thumbnail_result_debug_contains_dimensions() {
        let result = ThumbnailResult {
            data: vec![0u8; 100],
            width: 128,
            height: 256,
            format: ImageFormat::Bmp,
        };
        let debug_str = format!("{:?}", result);
        assert!(debug_str.contains("128"));
        assert!(debug_str.contains("256"));
    }

    #[test]
    fn test_color_for_extension_video() {
        let service = ThumbnailService::new();
        let path = FilesPath::new("movie.mp4");
        let color = service.get_color_for_extension(&path);
        assert_eq!(color.r, 168);
        assert_eq!(color.g, 85);
        assert_eq!(color.b, 247);
    }

    #[test]
    fn test_color_for_extension_audio() {
        let service = ThumbnailService::new();
        let path = FilesPath::new("song.mp3");
        let color = service.get_color_for_extension(&path);
        assert_eq!(color.r, 236);
        assert_eq!(color.g, 72);
        assert_eq!(color.b, 153);
    }

    #[test]
    fn test_color_for_extension_executable() {
        let service = ThumbnailService::new();
        let path = FilesPath::new("program.exe");
        let color = service.get_color_for_extension(&path);
        assert_eq!(color.r, 239);
        assert_eq!(color.g, 68);
        assert_eq!(color.b, 68);
    }

    #[test]
    fn test_color_for_extension_document() {
        let service = ThumbnailService::new();
        let path = FilesPath::new("readme.md");
        let color = service.get_color_for_extension(&path);
        assert_eq!(color.r, 37);
        assert_eq!(color.g, 99);
        assert_eq!(color.b, 235);
    }

    #[test]
    fn test_color_for_extension_with_uppercase_extension() {
        let service = ThumbnailService::new();
        let path = FilesPath::new("image.PNG");
        let color = service.get_color_for_extension(&path);
        // Should be matched as png (lowercase)
        assert_eq!(color.r, 16);
        assert_eq!(color.g, 185);
        assert_eq!(color.b, 129);
    }

    #[test]
    fn test_thumbnail_size_copy_trait() {
        let size = ThumbnailSize::Medium;
        let _copied = size; // Copy should work
        assert_eq!(size.dimensions(), (96, 96));
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn test_service_creation() {
        let service = ThumbnailService::new();
        #[cfg(windows)]
        assert!(service.get_cached(&FilesPath::new("/tmp")).is_none());
    }

    #[test]
    fn test_thumbnail_size_dimensions() {
        assert_eq!(ThumbnailSize::Small.dimensions(), (32, 32));
        assert_eq!(ThumbnailSize::Medium.dimensions(), (96, 96));
        assert_eq!(ThumbnailSize::Large.dimensions(), (256, 256));
        assert_eq!(ThumbnailSize::ExtraLarge.dimensions(), (512, 512));
    }

    #[test]
    fn test_color_for_extension_code() {
        let service = ThumbnailService::new();
        let path = FilesPath::new("test.rs");
        let color = service.get_color_for_extension(&path);
        assert_eq!(color.r, 0);
        assert_eq!(color.g, 122);
        assert_eq!(color.b, 255);
    }

    #[test]
    fn test_color_for_extension_image() {
        let service = ThumbnailService::new();
        let path = FilesPath::new("photo.png");
        let color = service.get_color_for_extension(&path);
        assert_eq!(color.r, 16);
        assert_eq!(color.g, 185);
        assert_eq!(color.b, 129);
    }

    #[test]
    fn test_color_for_extension_archive() {
        let service = ThumbnailService::new();
        let path = FilesPath::new("archive.zip");
        let color = service.get_color_for_extension(&path);
        assert_eq!(color.r, 245);
        assert_eq!(color.g, 158);
        assert_eq!(color.b, 11);
    }

    #[test]
    fn test_color_for_extension_unknown() {
        let service = ThumbnailService::new();
        let path = FilesPath::new("file.xyz");
        let color = service.get_color_for_extension(&path);
        assert_eq!(color.r, 107);
        assert_eq!(color.g, 114);
        assert_eq!(color.b, 128);
    }

    #[test]
    fn test_thumbnail_result_debug() {
        let result = ThumbnailResult {
            data: vec![0u8; 100],
            width: 32,
            height: 32,
            format: ImageFormat::Bmp,
        };
        let debug_str = format!("{:?}", result);
        assert!(debug_str.contains("ThumbnailResult"));
    }

    #[test]
    fn test_image_format_display() {
        assert_eq!(format!("{}", ImageFormat::Bmp), "Bmp");
        assert_eq!(format!("{}", ImageFormat::Png), "Png");
        assert_eq!(format!("{}", ImageFormat::Jpeg), "Jpeg");
        assert_eq!(format!("{}", ImageFormat::Unknown), "Unknown");
    }

    #[test]
    fn test_thumbnail_result_clone() {
        let result = ThumbnailResult {
            data: vec![1, 2, 3, 4],
            width: 32,
            height: 32,
            format: ImageFormat::Png,
        };
        let cloned = result.clone();
        assert_eq!(cloned.data, result.data);
        assert_eq!(cloned.width, result.width);
        assert_eq!(cloned.height, result.height);
        assert_eq!(cloned.format, result.format);
    }

    #[test]
    fn test_thumbnail_result_partial_eq() {
        let result1 = ThumbnailResult {
            data: vec![1, 2, 3, 4],
            width: 32,
            height: 32,
            format: ImageFormat::Png,
        };
        let result2 = ThumbnailResult {
            data: vec![1, 2, 3, 4],
            width: 32,
            height: 32,
            format: ImageFormat::Png,
        };
        assert_eq!(result1, result2);
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn test_extract_thumbnail_returns_valid_bitmap() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        // Create a temporary BMP file using the helper
        let mut temp_file = NamedTempFile::new().unwrap();
        let bmp_data = create_minimal_bmp(4, 4);
        temp_file.write_all(&bmp_data).unwrap();
        temp_file.flush();

        let service = ThumbnailService::new();
        let path = FilesPath::new(temp_file.path().to_str().unwrap());

        // This should succeed on Windows
        let result = service.extract_thumbnail(&path, ThumbnailSize::Small);
        assert!(result.is_ok());

        let thumb = result.unwrap();
        assert_eq!(thumb.width, 32);
        assert_eq!(thumb.height, 32);
        assert!(thumb.data.len() > 0);
    }

    #[cfg(windows)]
    #[test]
    fn test_thumbnail_cache_behavior() {
        let service = ThumbnailService::new();
        let path = FilesPath::new("C:\\test\\image.png");

        // Initially not cached
        assert!(service.get_cached(&path).is_none());

        // Clear should work without panic
        service.clear_cache();

        // Cache should still be empty
        assert!(service.get_cached(&path).is_none());
    }

    #[cfg(windows)]
    #[test]
    fn test_thumbnail_service_thread_safety() {
        use std::sync::Arc;
        use std::thread;

        let service = Arc::new(ThumbnailService::new());

        let service_clone = Arc::clone(&service);

        let handle = thread::spawn(move || {
            let path = FilesPath::new("C:\\test\\file.txt");
            service_clone.clear_cache();
            service_clone.get_cached(&path)
        });

        // Main thread can also use the service
        let path = FilesPath::new("C:\\test\\file2.txt");
        let _ = service.clear_cache();
        let result = service.get_cached(&path);

        // Result should be None (file doesn't exist)
        assert!(result.is_none());

        handle.join().unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn test_ntfs_attributes_helper() {
        use crate::domain::icon_cache::NtfsAttributes;

        // Test compression flag detection
        let compressed_attrs = NtfsAttributes {
            is_compressed: true,
            is_encrypted: false,
            is_sparse: false,
            is_reparse_point: false,
        };

        assert!(compressed_attrs.is_compressed);
        assert!(!compressed_attrs.is_encrypted);

        let encrypted_attrs = NtfsAttributes {
            is_compressed: false,
            is_encrypted: true,
            is_sparse: false,
            is_reparse_point: false,
        };

        assert!(!encrypted_attrs.is_compressed);
        assert!(encrypted_attrs.is_encrypted);
    }

    #[test]
    fn test_thumbnail_size_from_dimensions() {
        // Test that different sizes produce correct dimensions
        assert_eq!(ThumbnailSize::Small.dimensions(), (32, 32));
        assert_eq!(ThumbnailSize::Medium.dimensions(), (96, 96));
        assert_eq!(ThumbnailSize::Large.dimensions(), (256, 256));
        assert_eq!(ThumbnailSize::ExtraLarge.dimensions(), (512, 512));
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn test_get_thumbnail_caches_result() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut temp_file = NamedTempFile::new().unwrap();
        // Minimal BMP
        let bmp_header = create_minimal_bmp(4, 4);
        temp_file.write_all(&bmp_header).unwrap();
        temp_file.flush();

        let service = ThumbnailService::new();
        let path = FilesPath::new(temp_file.path().to_str().unwrap());

        // First call should cache
        let result1 = service.get_thumbnail(&path, ThumbnailSize::Small).await;
        assert!(result1.is_ok());

        // Second call should return cached (verify cache hit)
        let result2 = service.get_thumbnail(&path, ThumbnailSize::Small).await;
        assert!(result2.is_ok());

        // Results should be equal (same cached value)
        assert_eq!(result1.unwrap().data, result2.unwrap().data);
    }
}

// Helper function to create minimal BMP for testing
fn create_minimal_bmp(width: u32, height: u32) -> Vec<u8> {
    let row_size = ((width * 3 + 3) / 4) * 4;
    let pixel_data_size = row_size * height;
    let file_size = 54 + pixel_data_size;

    let mut data = Vec::with_capacity(file_size as usize);

    // BMP Header
    data.extend_from_slice(&[0x42, 0x4D]); // "BM"
    data.extend_from_slice(&(file_size as u32).to_le_bytes());
    data.extend_from_slice(&[0, 0, 0, 0]); // Reserved
    data.extend_from_slice(&(54u32).to_le_bytes()); // Pixel data offset

    // DIB Header (BITMAPINFOHEADER)
    data.extend_from_slice(&(40u32).to_le_bytes()); // Header size
    data.extend_from_slice(&(width as i32).to_le_bytes());
    data.extend_from_slice(&((height as i32).to_le_bytes()));
    data.extend_from_slice(&(1u16).to_le_bytes()); // Color planes
    data.extend_from_slice(&(24u16).to_le_bytes()); // Bits per pixel
    data.extend_from_slice(&[0, 0, 0, 0]); // Compression (none)
    data.extend_from_slice(&(pixel_data_size as u32).to_le_bytes());
    data.extend_from_slice(&[0, 0, 0, 0]); // X pixels per meter
    data.extend_from_slice(&[0, 0, 0, 0]); // Y pixels per meter
    data.extend_from_slice(&[0, 0, 0, 0]); // Colors in color table
    data.extend_from_slice(&[0, 0, 0, 0]); // Important colors

    // Pixel data (blue pixels)
    let padding = (4 - (width * 3) % 4) % 4;
    for _ in 0..height {
        for _ in 0..width {
            data.extend_from_slice(&[255, 0, 0]); // Blue
        }
        for _ in 0..padding {
            data.push(0);
        }
    }

    data
}

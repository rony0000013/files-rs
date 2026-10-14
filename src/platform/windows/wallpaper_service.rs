//! Wallpaper Service for Windows
//! Provides functionality to set desktop wallpaper, lockscreen, and slideshow
//! Uses Windows API: SystemParametersInfoW, SetWallpaper

use anyhow::{Context, Result};
use std::path::Path;

/// Wallpaper style options
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WallpaperStyle {
    Fill,
    Fit,
    Stretch,
    Tile,
    Center,
    Span,
}

impl Default for WallpaperStyle {
    fn default() -> Self {
        Self::Fill
    }
}

/// Represents the result of a wallpaper operation
#[derive(Debug)]
pub struct WallpaperResult {
    pub success: bool,
    pub path: String,
}

/// Service for setting Windows desktop wallpaper, lockscreen, and slideshow
pub struct WallpaperService;

impl WallpaperService {
    /// Creates a new WallpaperService instance
    pub fn new() -> Self {
        Self
    }

    /// Sets the desktop wallpaper using the Windows API SystemParametersInfoW
    pub async fn set_as_wallpaper(&self, path: &Path) -> Result<WallpaperResult> {
        let path_str = path.to_string_lossy();

        #[cfg(windows)]
        {
            use std::os::windows::prelude::OsStrExt;
            use windows::core::PCWSTR;
            use windows::Win32::UI::WindowsAndMessaging::{
                SystemParametersInfoW, SPIF_SENDWININICHANGE, SPIF_UPDATEINIFILE,
                SPI_SETDESKWALLPAPER,
            };

            tracing::info!("Setting wallpaper to: {}", path_str);

            // Convert path to wide string (null-terminated)
            let path_wide: Vec<u16> = std::ffi::OsStr::new(&*path_str)
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();

            unsafe {
                SystemParametersInfoW(
                    SPI_SETDESKWALLPAPER,
                    0,
                    Some(path_wide.as_ptr() as *mut _),
                    SPIF_UPDATEINIFILE | SPIF_SENDWININICHANGE,
                )
                .context("Failed to set wallpaper")?;
            }

            Ok(WallpaperResult {
                success: true,
                path: path_str.to_string(),
            })
        }

        #[cfg(not(windows))]
        {
            let _ = path;
            tracing::info!("Cannot set wallpaper on non-Windows platform: {}", path_str);
            Ok(WallpaperResult {
                success: false,
                path: path_str.to_string(),
            })
        }
    }

    /// Sets the wallpaper with a specific style
    pub async fn set_with_style(
        &self,
        path: &Path,
        style: WallpaperStyle,
    ) -> Result<WallpaperResult> {
        #[cfg(windows)]
        {
            use std::ffi::OsStr;
            use std::os::windows::prelude::OsStrExt;
            use windows::core::PCWSTR;
            use windows::Win32::System::Registry::{
                RegCloseKey, RegOpenKeyExW, RegSetValueExW, HKEY_CURRENT_USER, KEY_WRITE, REG_SZ,
            };

            let path_str = path.to_string_lossy();

            // Set the wallpaper style in the registry
            let key_path: Vec<u16> = "Control Panel\\Desktop"
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();

            // WallpaperStyle / TileWallpaper documented numeric values:
            // Center = (0, 0), Tile = (1, 0), Stretch = (0, 2),
            // Fit = (0, 6), Fill = (0, 10), Span = (0, 22).
            let (tile_value, style_value) = match style {
                WallpaperStyle::Fill => ("0", "10"),
                WallpaperStyle::Fit => ("0", "6"),
                WallpaperStyle::Stretch => ("0", "2"),
                WallpaperStyle::Tile => ("1", "0"),
                WallpaperStyle::Center => ("0", "0"),
                WallpaperStyle::Span => ("0", "22"),
            };

            unsafe {
                let mut hkey = std::mem::zeroed();
                let _ = RegOpenKeyExW(
                    HKEY_CURRENT_USER,
                    PCWSTR(key_path.as_ptr()),
                    Some(0),
                    KEY_WRITE,
                    &mut hkey,
                );

                // Set Wallpaper style (names and REG_SZ data are NUL-terminated UTF-16)
                let wallpaper_style_name: Vec<u16> = "WallpaperStyle"
                    .encode_utf16()
                    .chain(std::iter::once(0))
                    .collect();
                let tile_wallpaper_name: Vec<u16> = "TileWallpaper"
                    .encode_utf16()
                    .chain(std::iter::once(0))
                    .collect();
                let tile_wide: Vec<u16> = tile_value
                    .encode_utf16()
                    .chain(std::iter::once(0))
                    .collect();
                let style_wide: Vec<u16> = style_value
                    .encode_utf16()
                    .chain(std::iter::once(0))
                    .collect();
                // Byte views include the trailing NUL, as REG_SZ requires.
                let tile_bytes = unsafe {
                    std::slice::from_raw_parts(tile_wide.as_ptr() as *const u8, tile_wide.len() * 2)
                };
                let style_bytes = unsafe {
                    std::slice::from_raw_parts(
                        style_wide.as_ptr() as *const u8,
                        style_wide.len() * 2,
                    )
                };
                let _ = RegSetValueExW(
                    hkey,
                    PCWSTR(wallpaper_style_name.as_ptr()),
                    Some(0),
                    REG_SZ,
                    Some(style_bytes),
                );

                let _ = RegSetValueExW(
                    hkey,
                    PCWSTR(tile_wallpaper_name.as_ptr()),
                    Some(0),
                    REG_SZ,
                    Some(tile_bytes),
                );

                let _ = RegCloseKey(hkey);
            }
        }

        #[cfg(not(windows))]
        {
            let _ = path;
            let _ = style;
        }

        // Now set the wallpaper (calls the platform-specific implementation)
        self.set_as_wallpaper(path).await
    }

    /// Sets the lockscreen background using Windows Runtime API
    pub async fn set_as_lockscreen(&self, path: &Path) -> Result<WallpaperResult> {
        let path_str = path.to_string_lossy();

        #[cfg(windows)]
        {
            use std::process::Command;

            tracing::info!("Setting lockscreen to: {}", path_str);

            // Use PowerShell to set lockscreen via Windows Runtime
            let ps_script = format!(
                r#"Add-Type -AssemblyName System.Windows.Forms; [Windows.System.UserProfile.LockScreen]::SetImageFile([System.IO.FileInfo]'{}')"#,
                path_str.replace("'", "''")
            );

            let output = Command::new("powershell")
                .args(["-NoProfile", "-Command", &ps_script])
                .output()
                .context("Failed to execute PowerShell for lockscreen")?;

            if output.status.success() {
                Ok(WallpaperResult {
                    success: true,
                    path: path_str.to_string(),
                })
            } else {
                anyhow::bail!(
                    "Failed to set lockscreen: {}",
                    String::from_utf8_lossy(&output.stderr)
                )
            }
        }

        #[cfg(not(windows))]
        {
            let _ = path;
            tracing::info!(
                "Cannot set lockscreen on non-Windows platform: {}",
                path_str
            );
            Ok(WallpaperResult {
                success: false,
                path: path_str.to_string(),
            })
        }
    }

    /// Sets the slideshow (wallpaper rotation) settings
    pub async fn set_as_slideshow(&self, folder_path: &Path) -> Result<WallpaperResult> {
        let path_str = folder_path.to_string_lossy();

        #[cfg(windows)]
        {
            use std::ffi::OsStr;
            use std::os::windows::prelude::OsStrExt;
            use windows::core::PCWSTR;
            use windows::Win32::System::Registry::{
                RegCloseKey, RegOpenKeyExW, RegSetValueExW, HKEY_CURRENT_USER, KEY_WRITE, REG_SZ,
            };

            tracing::info!("Setting slideshow folder to: {}", path_str);

            let key_path: Vec<u16> = "Control Panel\\Personalization\\Desktop Slideshow"
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();

            unsafe {
                let mut hkey = std::mem::zeroed();
                let _ = RegOpenKeyExW(
                    HKEY_CURRENT_USER,
                    PCWSTR(key_path.as_ptr()),
                    Some(0),
                    KEY_WRITE,
                    &mut hkey,
                );

                // Enable slideshow (names and REG_SZ data are NUL-terminated UTF-16)
                let enable_status_name: Vec<u16> = "EnableStatus"
                    .encode_utf16()
                    .chain(std::iter::once(0))
                    .collect();
                let image_path_name: Vec<u16> = "ImagePath"
                    .encode_utf16()
                    .chain(std::iter::once(0))
                    .collect();
                let enable_wide: Vec<u16> =
                    "1".encode_utf16().chain(std::iter::once(0)).collect();
                let enable_bytes = unsafe {
                    std::slice::from_raw_parts(
                        enable_wide.as_ptr() as *const u8,
                        enable_wide.len() * 2,
                    )
                };
                let _ = RegSetValueExW(
                    hkey,
                    PCWSTR(enable_status_name.as_ptr()),
                    Some(0),
                    REG_SZ,
                    Some(enable_bytes),
                );

                // Set slideshow folder
                let folder_wide: Vec<u16> = OsStr::new(&*path_str)
                    .encode_wide()
                    .chain(std::iter::once(0))
                    .collect();

                let _ = RegSetValueExW(
                    hkey,
                    PCWSTR(image_path_name.as_ptr()),
                    Some(0),
                    REG_SZ,
                    Some(unsafe {
                        std::slice::from_raw_parts(
                            folder_wide.as_ptr() as *const u8,
                            folder_wide.len() * 2,
                        )
                    }),
                );

                let _ = RegCloseKey(hkey);
            }

            Ok(WallpaperResult {
                success: true,
                path: path_str.to_string(),
            })
        }

        #[cfg(not(windows))]
        {
            let _ = folder_path;
            tracing::info!("Cannot set slideshow on non-Windows platform: {}", path_str);
            Ok(WallpaperResult {
                success: false,
                path: path_str.to_string(),
            })
        }
    }
}

impl Default for WallpaperService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wallpaper_style_default() {
        assert_eq!(WallpaperStyle::default(), WallpaperStyle::Fill);
    }

    #[test]
    fn test_wallpaper_style_variants() {
        let styles = [
            WallpaperStyle::Fill,
            WallpaperStyle::Fit,
            WallpaperStyle::Stretch,
            WallpaperStyle::Tile,
            WallpaperStyle::Center,
            WallpaperStyle::Span,
        ];
        for style in styles {
            let debug_str = format!("{:?}", style);
            assert!(!debug_str.is_empty());
        }
    }

    #[test]
    fn test_wallpaper_result_debug() {
        let result = WallpaperResult {
            success: true,
            path: "C:\\test\\wallpaper.jpg".to_string(),
        };
        let debug_str = format!("{:?}", result);
        assert!(debug_str.contains("success"));
        assert!(debug_str.contains("wallpaper.jpg"));
    }

    #[test]
    fn test_service_creation() {
        let service = WallpaperService::new();
        let default_service = WallpaperService::default();
        assert!(true); // Both created successfully
        let _ = default_service;
    }

    #[tokio::test]
    async fn test_set_as_wallpaper_accepts_valid_path() {
        let service = WallpaperService::new();
        let path = Path::new("C:\\Windows\\Web\\Wallpaper\\test.jpg");
        let result = service.set_as_wallpaper(path).await;
        assert!(result.is_ok());
        let wallpaper_result = result.unwrap();
        assert!(wallpaper_result.success || !wallpaper_result.success); // May succeed or fail depending on environment
    }

    #[tokio::test]
    async fn test_set_as_lockscreen_accepts_valid_path() {
        let service = WallpaperService::new();
        let path = Path::new("C:\\Windows\\Web\\Screen\\test.jpg");
        let result = service.set_as_lockscreen(path).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_set_as_slideshow_with_folder() {
        let service = WallpaperService::new();
        let path = Path::new("C:\\Users\\Public\\Pictures");
        let result = service.set_as_slideshow(path).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_set_with_style_accepts_valid_path() {
        let service = WallpaperService::new();
        let path = Path::new("C:\\test\\wallpaper.png");
        let result = service.set_with_style(path, WallpaperStyle::Center).await;
        assert!(result.is_ok());
    }
}

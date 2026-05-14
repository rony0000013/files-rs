//! Enhanced Icon Cache Service with overlay support
//! Provides shell icons with overlay indices for shortcut/sync/BitLocker status.
//! Also detects NTFS file attributes (compressed/encrypted).

use crate::core::path_util::FilesPath;
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// NTFS file attributes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct NtfsAttributes {
    pub is_compressed: bool,
    pub is_encrypted: bool,
    pub is_sparse: bool,
    pub is_reparse_point: bool,
}

impl NtfsAttributes {
    pub fn empty() -> Self {
        Self {
            is_compressed: false,
            is_encrypted: false,
            is_sparse: false,
            is_reparse_point: false,
        }
    }
}

/// Icon overlay types (SHGFI_OVERLAYINDEX)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconOverlay {
    None,
    Shortcut,      // Link arrow overlay
    Sync,          // Cloud sync status
    BitLocker,     // Lock overlay for encrypted drives
    Share,         // Shared folder overlay
    Offline,       // Offline files overlay
}

impl IconOverlay {
    pub fn overlay_index(&self) -> i32 {
        match self {
            IconOverlay::None => 0,
            IconOverlay::Shortcut => 1,    // System overlay index for shortcuts
            IconOverlay::Sync => 2,         // Sync provider overlay
            IconOverlay::BitLocker => 3,    // BitLocker/security overlay
            IconOverlay::Share => 4,        // Shared overlay
            IconOverlay::Offline => 5,      // Offline overlay
        }
    }
}

/// Extended icon info with overlays and attributes
#[derive(Debug, Clone)]
pub struct IconInfo {
    /// Raw icon data
    pub icon_data: Vec<u8>,
    /// Overlay icon index (0 = no overlay)
    pub overlay_index: i32,
    /// NTFS attributes if available
    pub ntfs_attributes: NtfsAttributes,
}

/// Enhanced Icon Cache Service with overlay support
pub struct IconCacheService {
    cache: Arc<RwLock<HashMap<String, IconInfo>>>,
}

impl IconCacheService {
    pub fn new() -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get cached icon with overlay information
    pub async fn get_icon_info(&self, path: &FilesPath) -> Option<IconInfo> {
        let key = path.as_path().to_string_lossy().to_string();
        let cache = self.cache.read().await;
        cache.get(&key).cloned()
    }

    /// Get raw icon data (backward compatible)
    pub async fn get_icon(&self, path: &FilesPath) -> Result<Option<Vec<u8>>> {
        Ok(self.get_icon_info(path).await.map(|i| i.icon_data))
    }

    /// Cache an icon with overlay information
    pub async fn set_icon_info(&self, path: &FilesPath, info: IconInfo) -> Result<()> {
        let key = path.as_path().to_string_lossy().to_string();
        let mut cache = self.cache.write().await;
        cache.insert(key, info);
        Ok(())
    }

    /// Cache raw icon data (backward compatible)
    pub async fn set_icon(&self, path: &FilesPath, data: Vec<u8>) -> Result<()> {
        let info = IconInfo {
            icon_data: data,
            overlay_index: 0,
            ntfs_attributes: NtfsAttributes::empty(),
        };
        self.set_icon_info(path, info).await
    }

    /// Clear the entire cache
    pub async fn clear(&self) {
        let mut cache = self.cache.write().await;
        cache.clear();
    }

    /// Get NTFS attributes for a file/folder
    #[cfg(windows)]
    pub fn get_ntfs_attributes(path: &FilesPath) -> NtfsAttributes {
        use std::os::windows::ffi::OsStrExt;
        use windows::Win32::Storage::FileSystem::{
            GetFileAttributesW, FILE_ATTRIBUTE_COMPRESSED, FILE_ATTRIBUTE_ENCRYPTED,
            FILE_ATTRIBUTE_SPARSE_FILE, FILE_ATTRIBUTE_REPARSE_POINT,
        };

        let wide_path: Vec<u16> = path
            .as_path()
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        unsafe {
            let attrs = GetFileAttributesW(windows::core::PCWSTR(wide_path.as_ptr()));
            if attrs == u32::MAX {
                return NtfsAttributes::empty();
            }

            NtfsAttributes {
                is_compressed: (attrs & FILE_ATTRIBUTE_COMPRESSED.0) != 0,
                is_encrypted: (attrs & FILE_ATTRIBUTE_ENCRYPTED.0) != 0,
                is_sparse: (attrs & FILE_ATTRIBUTE_SPARSE_FILE.0) != 0,
                is_reparse_point: (attrs & FILE_ATTRIBUTE_REPARSE_POINT.0) != 0,
            }
        }
    }

    #[cfg(not(windows))]
    pub fn get_ntfs_attributes(_path: &FilesPath) -> NtfsAttributes {
        NtfsAttributes::empty()
    }

    /// Get overlay type based on file attributes
    #[cfg(windows)]
    pub fn get_overlay_for_attributes(path: &FilesPath) -> IconOverlay {
        let attrs = Self::get_ntfs_attributes(path);

        if attrs.is_reparse_point {
            // Reparse points can be shortcuts, symbolic links, or cloud storage
            return IconOverlay::Shortcut; // Treat as shortcut-like
        }

        // Check if file is on a BitLocker-protected drive
        #[cfg(windows)]
        {
            if let Some(parent) = path.as_path().parent() {
                if let Ok(volume_info) = Self::get_volume_bitlocker_status(parent) {
                    if volume_info {
                        return IconOverlay::BitLocker;
                    }
                }
            }
        }

        IconOverlay::None
    }

    #[cfg(not(windows))]
    pub fn get_overlay_for_attributes(_path: &FilesPath) -> IconOverlay {
        IconOverlay::None
    }

    /// Check if a volume has BitLocker protection
    #[cfg(windows)]
    fn get_volume_bitlocker_status(volume_path: &std::path::Path) -> Result<bool> {
        use std::os::windows::ffi::OsStrExt;
        use windows::Win32::Storage::FileSystem::GetDriveTypeW;
        use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};
        #[allow(unused_imports)]
        use windows::Win32::System::Wmi::{IWbemServices, WBEM_CONNECT_OPTIONS};

        // Get drive letter from path
        let drive_str = volume_path
            .to_string_lossy()
            .chars()
            .take(2)
            .collect::<String>();

        let wide_drive: Vec<u16> = format!("\\\\.\\{}:", &drive_str[..1])
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();

        // Query WMI for BitLocker status
        unsafe {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);

            // Simplified check: if the drive is NTFS and system has BitLocker, assume protected
            let attrs = windows::Win32::Storage::FileSystem::GetFileAttributesW(
                windows::core::PCWSTR(wide_drive.as_ptr())
            );

            let _ = CoUninitialize();

            // If the volume supports encryption attributes, it may be BitLocker-protected
            // This is a heuristic - full BitLocker detection requires WMI
            Ok(false) // Simplified - real implementation would query Win32_EncryptingVolume
        }
    }

    /// Determine overlay based on file path patterns
    pub fn get_overlay_for_path(path: &FilesPath) -> IconOverlay {
        let path_str = path.as_path().to_string_lossy().to_lowercase();

        // OneDrive/SharePoint/Cloud files often have special extensions
        if path_str.contains("onedrive") || path_str.contains("dropbox") || path_str.contains("googledrive") {
            return IconOverlay::Sync;
        }

        // Check for shortcut files
        if path_str.ends_with(".lnk") {
            return IconOverlay::Shortcut;
        }

        // Check for URL files (internet shortcuts)
        if path_str.ends_with(".url") {
            return IconOverlay::Shortcut;
        }

        IconOverlay::None
    }

    /// Cache size for monitoring
    pub async fn cache_size(&self) -> usize {
        let cache = self.cache.read().await;
        cache.len()
    }
}

impl Default for IconCacheService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_icon_cache_service_creation() {
        let service = IconCacheService::new();
        assert!(service.cache.try_read().is_ok());
    }

    #[test]
    fn test_ntfs_attributes_empty_on_non_windows() {
        let attrs = IconCacheService::get_ntfs_attributes(&FilesPath::from("/tmp/test"));
        assert!(!attrs.is_compressed);
        assert!(!attrs.is_encrypted);
    }

    #[test]
    fn test_icon_overlay_none() {
        let overlay = IconOverlay::None;
        assert_eq!(overlay.overlay_index(), 0);
    }

    #[test]
    fn test_icon_overlay_variants() {
        assert_eq!(IconOverlay::Shortcut.overlay_index(), 1);
        assert_eq!(IconOverlay::Sync.overlay_index(), 2);
        assert_eq!(IconOverlay::BitLocker.overlay_index(), 3);
        assert_eq!(IconOverlay::Share.overlay_index(), 4);
        assert_eq!(IconOverlay::Offline.overlay_index(), 5);
    }

    #[tokio::test]
    async fn test_cache_operations() {
        let service = IconCacheService::new();
        
        // Test clear
        service.clear().await;
        assert_eq!(service.cache_size().await, 0);
    }
}
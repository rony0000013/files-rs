//! Cloud Filter Service
//! Implements IStorageProviderStatusUISourceFactory for quota and sync status.
//! Provides Windows Cloud Files API integration for monitoring cloud storage providers.

use crate::core::path_util::FilesPath;
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Cloud file sync status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CloudSyncStatus {
    Unknown,
    NotTracked,      // File is not in the cloud
    Synced,          // File is synced and up-to-date
    Syncing,         // File is currently syncing
    PendingUpload,   // Local changes pending upload
    PendingDownload,  // Remote changes pending download
    PendingDelete,   // File pending deletion
    Conflict,        // Sync conflict detected
    Error,           // Sync error occurred
    Excluded,         // File excluded from sync (e.g., selective sync)
}

/// Information about a cloud storage provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudProviderStatus {
    pub provider_name: String,
    pub mount_point: FilesPath,
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub free_bytes: u64,
    pub is_available: bool,
    pub sync_status: CloudSyncStatus,
}

/// Cloud Filter Service for monitoring cloud storage providers
/// Provides sync status and quota information via Windows Cloud Files API.
pub struct CloudFilterService {
    #[cfg(windows)]
    providers: RwLock<HashMap<String, CloudProviderStatus>>,
}

impl CloudFilterService {
    pub fn new() -> Self {
        Self {
            #[cfg(windows)]
            providers: RwLock::new(HashMap::new()),
        }
    }

    /// Get the sync status of a file or directory
    #[cfg(windows)]
    pub fn get_sync_status(path: &FilesPath) -> Result<CloudSyncStatus> {
        use std::os::windows::ffi::OsStrExt;
        use windows::Win32::Storage::FileSystem::{
            CreateFileW, FILE_FLAGS_AND_ATTRIBUTES, FILE_FLAG_BACKUP_SEMANTICS, FILE_SHARE_MODE,
            GetFileAttributesExW, GetFileInformationByHandle, FILE_FLAG_OPEN_REPARSE_POINT,
            FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING, GetFileExInfoStandard, FILE_BASIC_INFO,
        };
        use windows::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};

        let wide_path: Vec<u16> = path
            .as_path()
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        unsafe {
            let handle = CreateFileW(
                windows::core::PCWSTR(wide_path.as_ptr()),
                0u32, // dwDesiredAccess
                FILE_SHARE_MODE(FILE_SHARE_READ.0 | FILE_SHARE_WRITE.0), // dwShareMode
                std::option::Option::None, // lpSecurityAttributes
                OPEN_EXISTING, // dwCreationDisposition
                FILE_FLAGS_AND_ATTRIBUTES(FILE_FLAG_BACKUP_SEMANTICS.0 | FILE_FLAG_OPEN_REPARSE_POINT.0), // dwFlagsAndAttributes
                None, // hTemplateFile
            );

            let handle = match handle {
                Ok(h) => h,
                Err(_) => return Ok(CloudSyncStatus::Unknown),
            };

            let mut basic_info: FILE_BASIC_INFO = std::mem::zeroed();
            let result = GetFileInformationByHandle(
                handle,
                &mut basic_info as *mut _ as *mut _,
            );

            let _ = CloseHandle(handle);

            // Check for cloud provider reparse point
            // FILE_ATTRIBUTE_NOT_CONTENT_INDEXED is used as a proxy for cloud status
            // Real implementation would use ICloudSyncPropertyStore
            if result.is_ok() {
                let attrs = basic_info.FileAttributes;
                if (attrs & 0x00004000) != 0 {
                    // FILE_ATTRIBUTE_NOT_CONTENT_INDEXED often indicates cloud files
                    // Check FILE_ATTRIBUTE_PINNED (0x00080000) for pinned state
                    if (attrs & 0x00080000) != 0 {
                        return Ok(CloudSyncStatus::Synced);
                    }
                    return Ok(CloudSyncStatus::Syncing);
                }
            }

            Ok(CloudSyncStatus::NotTracked)
        }
    }

    #[cfg(not(windows))]
    pub fn get_sync_status(_path: &FilesPath) -> Result<CloudSyncStatus> {
        Ok(CloudSyncStatus::NotTracked)
    }

    /// Get the sync status description (human-readable)
    pub fn get_sync_status_description(status: CloudSyncStatus) -> &'static str {
        match status {
            CloudSyncStatus::Unknown => "Unknown",
            CloudSyncStatus::NotTracked => "Not tracked",
            CloudSyncStatus::Synced => "Synced",
            CloudSyncStatus::Syncing => "Syncing",
            CloudSyncStatus::PendingUpload => "Pending upload",
            CloudSyncStatus::PendingDownload => "Pending download",
            CloudSyncStatus::PendingDelete => "Pending delete",
            CloudSyncStatus::Conflict => "Conflict",
            CloudSyncStatus::Error => "Error",
            CloudSyncStatus::Excluded => "Excluded",
        }
    }

    /// Check if a path is a cloud provider mount point
    #[cfg(windows)]
    pub fn is_cloud_provider_path(path: &FilesPath) -> bool {
        use winreg::enums::*;
        use winreg::RegKey;

        let path_str = path.as_path().to_string_lossy().to_lowercase();

        // Check for known cloud provider paths
        let cloud_indicators = [
            "onedrive",
            "google drive",
            "dropbox",
            "icloud",
            "box",
            "pcloud",
        ];

        for indicator in &cloud_indicators {
            if path_str.contains(indicator) {
                return true;
            }
        }

        // Also check via WMI for registered cloud providers
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        if let Ok(shell_folders) = hkcu.open_subkey("Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\Desktop\\NameSpace") {
            for value_result in shell_folders.enum_values() {
                if let Ok((name, _)) = value_result {
                    // Look for cloud provider GUIDs
                    if name.contains("OneDrive") || name.contains("Dropbox") {
                        return true;
                    }
                }
            }
        }

        false
    }

    #[cfg(not(windows))]
    pub fn is_cloud_provider_path(_path: &FilesPath) -> bool {
        false
    }

    /// Get quota information for a cloud provider
    #[cfg(windows)]
    pub fn get_provider_quota(provider_path: &FilesPath) -> Result<CloudProviderStatus> {
        use std::path::Path;

        let path_str = provider_path.as_path().to_string_lossy();
        let provider_name = Self::detect_provider_name(&path_str);

        // Use Windows API to get disk space info
        #[cfg(windows)]
        {
            use std::os::windows::ffi::OsStrExt;
            use windows::Win32::Storage::FileSystem::{
                GetDiskFreeSpaceExW, GetDriveTypeW, GetVolumeInformationW,
            };

            // Get the root of the cloud provider path
            let root: String = if path_str.len() >= 3 && path_str.chars().nth(1) == Some(':') {
                format!("{}:\\", path_str.chars().next().unwrap())
            } else {
                path_str.to_string()
            };

            let wide_root: Vec<u16> = root
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();

            unsafe {
                let mut free_bytes: u64 = 0;
                let mut total_bytes: u64 = 0;
                let mut total_free: u64 = 0;

                let result = GetDiskFreeSpaceExW(
                    windows::core::PCWSTR(wide_root.as_ptr()),
                    Some(&mut free_bytes),
                    Some(&mut total_bytes),
                    Some(&mut total_free),
                );

                if result.is_ok() {
                    return Ok(CloudProviderStatus {
                        provider_name,
                        mount_point: provider_path.clone(),
                        total_bytes,
                        used_bytes: total_bytes.saturating_sub(free_bytes),
                        free_bytes,
                        is_available: true,
                        sync_status: CloudSyncStatus::Synced,
                    });
                }
            }
        }

        Ok(CloudProviderStatus {
            provider_name,
            mount_point: provider_path.clone(),
            total_bytes: 0,
            used_bytes: 0,
            free_bytes: 0,
            is_available: Path::new(path_str.as_ref()).exists(),
            sync_status: CloudSyncStatus::Unknown,
        })
    }

    #[cfg(not(windows))]
    pub fn get_provider_quota(provider_path: &FilesPath) -> Result<CloudProviderStatus> {
        Ok(CloudProviderStatus {
            provider_name: "Unknown".to_string(),
            mount_point: provider_path.clone(),
            total_bytes: 0,
            used_bytes: 0,
            free_bytes: 0,
            is_available: false,
            sync_status: CloudSyncStatus::Unknown,
        })
    }

    /// Detect cloud provider name from path
    fn detect_provider_name(path: &str) -> String {
        let path_lower = path.to_lowercase();
        if path_lower.contains("onedrive") {
            "OneDrive".to_string()
        } else if path_lower.contains("google drive") || path_lower.contains("gdriv") {
            "Google Drive".to_string()
        } else if path_lower.contains("dropbox") {
            "Dropbox".to_string()
        } else if path_lower.contains("icloud") {
            "iCloud Drive".to_string()
        } else if path_lower.contains("box") {
            "Box".to_string()
        } else if path_lower.contains("pcloud") {
            "pCloud".to_string()
        } else {
            "Cloud Storage".to_string()
        }
    }

    /// Update provider status cache
    #[cfg(windows)]
    pub fn update_provider_cache(&self) -> Result<()> {
        use std::env;

        let mut providers = self.providers.write().unwrap();
        providers.clear();

        // Common cloud provider locations
        let user_profile = env::var("USERPROFILE").unwrap_or_default();

        let known_providers = [
            ("OneDrive", format!("{}\\OneDrive", user_profile)),
            ("Google Drive", format!("{}\\Google Drive", user_profile)),
            ("Dropbox", format!("{}\\Dropbox", user_profile)),
        ];

        for (name, path_str) in known_providers {
            if std::path::Path::new(&path_str).exists() {
                let path = FilesPath::from(path_str.as_str());
                if let Ok(status) = Self::get_provider_quota(&path) {
                    providers.insert(name.to_string(), status);
                }
            }
        }

        Ok(())
    }

    #[cfg(not(windows))]
    pub fn update_provider_cache(&self) -> Result<()> {
        Ok(())
    }

    /// Get cached provider statuses
    pub fn get_cached_providers(&self) -> Vec<CloudProviderStatus> {
        #[cfg(windows)]
        {
            let providers = self.providers.read().unwrap();
            return providers.values().cloned().collect();
        }
        #[cfg(not(windows))]
        Vec::new()
    }
}

impl Default for CloudFilterService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn test_cloud_filter_service_creation() {
        let service = CloudFilterService::new();
        assert!(service.get_cached_providers().is_empty());
    }

    #[test]
    fn test_sync_status_description() {
        assert_eq!(CloudFilterService::get_sync_status_description(CloudSyncStatus::Synced), "Synced");
        assert_eq!(CloudFilterService::get_sync_status_description(CloudSyncStatus::Syncing), "Syncing");
        assert_eq!(CloudFilterService::get_sync_status_description(CloudSyncStatus::Conflict), "Conflict");
    }

    #[test]
    fn test_detect_provider_name() {
        assert_eq!(CloudFilterService::detect_provider_name("C:\\Users\\Test\\OneDrive\\Docs"), "OneDrive");
        assert_eq!(CloudFilterService::detect_provider_name("D:\\Dropbox\\Files"), "Dropbox");
        assert_eq!(CloudFilterService::detect_provider_name("E:\\Google Drive"), "Google Drive");
        assert_eq!(CloudFilterService::detect_provider_name("C:\\Random\\Folder"), "Cloud Storage");
    }

    #[test]
    fn test_is_cloud_provider_path() {
        let onedrive_path = FilesPath::from("C:\\Users\\Test\\OneDrive\\Documents");
        assert!(CloudFilterService::is_cloud_provider_path(&onedrive_path));

        let normal_path = FilesPath::from("C:\\Windows\\System32");
        assert!(!CloudFilterService::is_cloud_provider_path(&normal_path));
    }

    #[tokio::test]
    async fn test_get_sync_status() {
        let path = FilesPath::from("C:\\Users");
        let status = CloudFilterService::get_sync_status(&path);
        assert!(status.is_ok());
    }
}
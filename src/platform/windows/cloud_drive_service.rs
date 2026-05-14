//! Cloud drive detection via Windows registry.
//! Detects: OneDrive, Google Drive, Dropbox, iCloud.

#[cfg(windows)]
use winreg::enums::*;
#[cfg(windows)]
use winreg::RegKey;

use crate::core::path_util::FilesPath;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CloudProvider {
    OneDrive,
    GoogleDrive,
    Dropbox,
    iCloud,
}

impl CloudProvider {
    pub fn name(&self) -> &'static str {
        match self {
            CloudProvider::OneDrive => "OneDrive",
            CloudProvider::GoogleDrive => "Google Drive",
            CloudProvider::Dropbox => "Dropbox",
            CloudProvider::iCloud => "iCloud Drive",
        }
    }
    pub fn icon_index(&self) -> u32 {
        184
    } // generic cloud
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudDriveInfo {
    pub provider: CloudProvider,
    pub root: FilesPath,
    pub display_name: String,
    pub is_connected: bool,
}

#[cfg(windows)]
fn detect_onedrive() -> Option<CloudDriveInfo> {
    // OneDrive Personal: HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\Desktop\NameSpace\{018D5C66-4533-4301-A90E-FC5B2F7C3E56}
    let namespace_guids = [
        "{018D5C66-4533-4301-A90E-FC5B2F7C3E56}", // OneDrive
    ];
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);

    for guid in namespace_guids {
        let key_path = format!(
            "Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\Desktop\\NameSpace\\{}",
            guid
        );
        if let Ok(_key) = hkcu.open_subkey(&key_path) {
            let user_path = std::env::var("OneDrive")
                .ok()
                .or_else(|| std::env::var("OneDriveConsumer").ok())
                .unwrap_or_else(|| {
                    format!(
                        "{}\\OneDrive",
                        std::env::var("USERPROFILE").unwrap_or_default()
                    )
                });
            let path = FilesPath::from(user_path.as_str());
            return Some(CloudDriveInfo {
                provider: CloudProvider::OneDrive,
                root: path,
                display_name: "OneDrive".to_string(),
                is_connected: std::path::Path::new(&user_path).exists(),
            });
        }
    }
    None
}

#[cfg(windows)]
fn detect_dropbox() -> Option<CloudDriveInfo> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    if let Ok(key) = hkcu.open_subkey("Software\\Dropbox") {
        if let Ok(install_path) = key.get_value::<String, _>("InstallPath") {
            let db_path = format!("{}\\Dropbox", install_path);
            let path = FilesPath::from(db_path.as_str());
            return Some(CloudDriveInfo {
                provider: CloudProvider::Dropbox,
                root: path,
                display_name: "Dropbox".to_string(),
                is_connected: std::path::Path::new(&db_path).exists(),
            });
        }
    }
    // Also check namespace
    if let Ok(ns_key) = hkcu.open_subkey("Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\Desktop\\NameSpace\\{FB314ED9-A251-47B7-93E1-CDD82E34AF8B}") {
        let _ = ns_key;
        let user_profile = std::env::var("USERPROFILE").unwrap_or_default();
        let db_path = format!("{}\\Dropbox", user_profile);
        let path = FilesPath::from(db_path.as_str());
        return Some(CloudDriveInfo {
            provider: CloudProvider::Dropbox,
            root: path,
            display_name: "Dropbox".to_string(),
            is_connected: std::path::Path::new(&db_path).exists(),
        });
    }
    None
}

#[cfg(windows)]
fn detect_google_drive() -> Option<CloudDriveInfo> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    // Check both user and team drive locations
    let possible_paths = [
        std::env::var("USERPROFILE")
            .map(|p| format!("{}\\Google Drive", p))
            .ok(),
        Some("C:\\Program Files\\Google\\Drive File Stream".to_string()),
        Some("C:\\Program Files (x86)\\Google\\Drive File Stream".to_string()),
    ];
    for user_path in possible_paths.into_iter().flatten() {
        if std::path::Path::new(&user_path).exists() {
            let path = FilesPath::from(user_path.as_str());
            return Some(CloudDriveInfo {
                provider: CloudProvider::GoogleDrive,
                root: path,
                display_name: "Google Drive".to_string(),
                is_connected: true,
            });
        }
    }
    // Also check namespace
    if hkcu.open_subkey("Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\Desktop\\NameSpace\\{FB314ED9-A251-47B7-93E1-CDD82E34AF8B}").is_ok() {
        let user_profile = std::env::var("USERPROFILE").unwrap_or_default();
        let path = FilesPath::from(format!("{}\\Google Drive", user_profile).as_str());
        return Some(CloudDriveInfo {
            provider: CloudProvider::GoogleDrive,
            root: path,
            display_name: "Google Drive".to_string(),
            is_connected: true,
        });
    }
    None
}

#[cfg(windows)]
fn detect_icloud() -> Option<CloudDriveInfo> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    if let Ok(key) = hkcu.open_subkey("Software\\Apple Inc\\iCloud") {
        let _ = key;
        let user_profile = std::env::var("USERPROFILE").unwrap_or_default();
        let icloud_path = format!("{}\\Apple Cloud", user_profile);
        let path = FilesPath::from(icloud_path.as_str());
        return Some(CloudDriveInfo {
            provider: CloudProvider::iCloud,
            root: path,
            display_name: "iCloud Drive".to_string(),
            is_connected: std::path::Path::new(&icloud_path).exists(),
        });
    }
    None
}

#[cfg(windows)]
pub fn enumerate_cloud_drives() -> Vec<CloudDriveInfo> {
    let mut drives = Vec::new();
    if let Some(d) = detect_onedrive() {
        drives.push(d);
    }
    if let Some(d) = detect_dropbox() {
        drives.push(d);
    }
    if let Some(d) = detect_google_drive() {
        drives.push(d);
    }
    if let Some(d) = detect_icloud() {
        drives.push(d);
    }
    drives
}

#[cfg(not(windows))]
pub fn enumerate_cloud_drives() -> Vec<CloudDriveInfo> {
    Vec::new()
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn test_cloud_provider_names() {
        assert_eq!(CloudProvider::OneDrive.name(), "OneDrive");
        assert_eq!(CloudProvider::GoogleDrive.name(), "Google Drive");
        assert_eq!(CloudProvider::Dropbox.name(), "Dropbox");
        assert_eq!(CloudProvider::iCloud.name(), "iCloud Drive");
    }
}

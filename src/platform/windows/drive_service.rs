//! Drive detection service using Win32 APIs.
//! Enumerates all mounted volumes with type, capacity, and icon info.
//! Stub implementations for non-Windows platforms.

use crate::core::path_util::FilesPath;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DriveType {
    Unknown = 0,
    Removable = 2,
    Fixed = 3,
    Network = 4,
    CdRom = 5,
    RamDisk = 6,
}

impl DriveType {
    pub fn from_u32(v: u32) -> Self {
        match v {
            2 => DriveType::Removable,
            3 => DriveType::Fixed,
            4 => DriveType::Network,
            5 => DriveType::CdRom,
            6 => DriveType::RamDisk,
            _ => DriveType::Unknown,
        }
    }
    pub fn label(&self) -> &'static str {
        match self {
            DriveType::Unknown => "Unknown",
            DriveType::Removable => "Removable",
            DriveType::Fixed => "Local Disk",
            DriveType::Network => "Network Drive",
            DriveType::CdRom => "CD/DVD Drive",
            DriveType::RamDisk => "RAM Disk",
        }
    }
    pub fn icon_index(&self) -> u32 {
        34
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriveInfo {
    pub root: FilesPath,
    pub label: String,
    pub drive_type: DriveType,
    pub total_bytes: u64,
    pub free_bytes: u64,
    pub file_system: String,
    pub is_ready: bool,
}

impl DriveInfo {
    pub fn usage_ratio(&self) -> f64 {
        if self.total_bytes == 0 {
            0.0
        } else {
            1.0 - (self.free_bytes as f64 / self.total_bytes as f64)
        }
    }
    pub fn is_low_storage(&self) -> bool {
        self.usage_ratio() > 0.90
    }
    pub fn total_formatted(&self) -> String {
        humanize_bytes(self.total_bytes)
    }
    pub fn free_formatted(&self) -> String {
        humanize_bytes(self.free_bytes)
    }
}

#[cfg(windows)]
pub fn enumerate_drives() -> Vec<DriveInfo> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    let mut drives = Vec::new();
    let mut buffer: [u16; 256] = [0; 256];
    let len =
        unsafe { windows::Win32::Storage::FileSystem::GetLogicalDriveStringsW(Some(&mut buffer)) };
    if len == 0 {
        tracing::warn!("GetLogicalDriveStringsW returned no drives");
        return drives;
    }

    let mut pos = 0usize;
    while pos < buffer.len() {
        let c = buffer[pos];
        if c == 0 {
            break;
        }
        let start = pos;
        let mut end = start;
        while end < buffer.len() && buffer[end] != 0 {
            end += 1;
        }
        let root_wide: Vec<u16> = buffer[start..end].to_vec();
        let root_str = String::from_utf16_lossy(&root_wide);
        let root_path = FilesPath::from(root_str.as_str());
        if let Some(info) = get_drive_info_impl(&root_path) {
            drives.push(info);
        }
        pos = end + 1;
    }
    drives
}

#[cfg(windows)]
fn get_drive_info_impl(root: &FilesPath) -> Option<DriveInfo> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    let root_str = root.as_path().to_string_lossy();
    if !root_str.ends_with('\\') {
        return None;
    }

    let root_wide: Vec<u16> = root_str.encode_utf16().chain(std::iter::once(0)).collect();

    let drive_type_val = unsafe {
        windows::Win32::Storage::FileSystem::GetDriveTypeW(windows::core::PCWSTR(
            root_wide.as_ptr(),
        ))
    };
    let drive_type = DriveType::from_u32(drive_type_val);

    let mut free_bytes_available: u64 = 0;
    let mut total_bytes: u64 = 0;
    let mut total_free_bytes: u64 = 0;
    let ok = unsafe {
        windows::Win32::Storage::FileSystem::GetDiskFreeSpaceExW(
            windows::core::PCWSTR(root_wide.as_ptr()),
            Some(&mut free_bytes_available),
            Some(&mut total_bytes),
            Some(&mut total_free_bytes),
        )
    };
    let (total, free) = if ok.is_ok() {
        (total_bytes, free_bytes_available)
    } else {
        return Some(DriveInfo {
            root: root.clone(),
            label: format!("{:?}", drive_type),
            drive_type,
            total_bytes: 0,
            free_bytes: 0,
            file_system: String::new(),
            is_ready: false,
        });
    };

    let mut vol_name_buf: [u16; 261] = [0; 261];
    let mut fs_name_buf: [u16; 261] = [0; 261];
    let vol_ok = unsafe {
        windows::Win32::Storage::FileSystem::GetVolumeInformationW(
            windows::core::PCWSTR(root_wide.as_ptr()),
            Some(&mut vol_name_buf),
            None,
            None,
            None,
            Some(&mut fs_name_buf),
        )
    };

    let (label, file_system) = if vol_ok.is_ok() {
        let vol_label = String::from_utf16_lossy(&vol_name_buf)
            .trim_end_matches('\0')
            .to_string();
        let fs = String::from_utf16_lossy(&fs_name_buf)
            .trim_end_matches('\0')
            .to_string();
        (vol_label, fs)
    } else {
        (String::new(), String::new())
    };

    let display_label = if label.is_empty() {
        drive_type.label().to_string()
    } else {
        label
    };

    Some(DriveInfo {
        root: root.clone(),
        label: display_label,
        drive_type,
        total_bytes: total,
        free_bytes: free,
        file_system,
        is_ready: true,
    })
}

#[cfg(not(windows))]
pub fn enumerate_drives() -> Vec<DriveInfo> {
    Vec::new()
}

fn humanize_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    const TB: u64 = GB * 1024;
    if bytes >= TB {
        format!("{:.1} TB", bytes as f64 / TB as f64)
    } else if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn test_drive_type_from_u32() {
        assert_eq!(DriveType::from_u32(0), DriveType::Unknown);
        assert_eq!(DriveType::from_u32(2), DriveType::Removable);
        assert_eq!(DriveType::from_u32(3), DriveType::Fixed);
        assert_eq!(DriveType::from_u32(4), DriveType::Network);
        assert_eq!(DriveType::from_u32(5), DriveType::CdRom);
        assert_eq!(DriveType::from_u32(6), DriveType::RamDisk);
        assert_eq!(DriveType::from_u32(99), DriveType::Unknown); // Invalid
    }

    #[test]
    fn test_drive_type_label() {
        assert_eq!(DriveType::Unknown.label(), "Unknown");
        assert_eq!(DriveType::Removable.label(), "Removable");
        assert_eq!(DriveType::Fixed.label(), "Local Disk");
        assert_eq!(DriveType::Network.label(), "Network Drive");
        assert_eq!(DriveType::CdRom.label(), "CD/DVD Drive");
        assert_eq!(DriveType::RamDisk.label(), "RAM Disk");
    }

    #[test]
    fn test_drive_type_icon_index() {
        assert_eq!(DriveType::Unknown.icon_index(), 34);
        assert_eq!(DriveType::Removable.icon_index(), 34);
        assert_eq!(DriveType::Fixed.icon_index(), 34);
        assert_eq!(DriveType::Network.icon_index(), 34);
        assert_eq!(DriveType::CdRom.icon_index(), 34);
        assert_eq!(DriveType::RamDisk.icon_index(), 34);
    }

    #[test]
    fn test_drive_type_equality() {
        assert_eq!(DriveType::Fixed, DriveType::Fixed);
        assert_ne!(DriveType::Fixed, DriveType::Removable);
    }

    #[test]
    fn test_drive_info_debug() {
        let info = DriveInfo {
            root: FilesPath::new("C:\\"),
            label: "System".to_string(),
            drive_type: DriveType::Fixed,
            total_bytes: 500_000_000_000,
            free_bytes: 100_000_000_000,
            file_system: "NTFS".to_string(),
            is_ready: true,
        };
        let debug_str = format!("{:?}", info);
        assert!(debug_str.contains("DriveInfo"));
        assert!(debug_str.contains("System"));
    }

    #[test]
    fn test_humanize_bytes() {
        assert_eq!(humanize_bytes(0), "0 B");
        assert_eq!(humanize_bytes(512), "512 B");
        assert_eq!(humanize_bytes(1024), "1.0 KB");
        assert_eq!(humanize_bytes(1536), "1.5 KB");
        assert_eq!(humanize_bytes(1048576), "1.0 MB");
        assert_eq!(humanize_bytes(1073741824), "1.0 GB");
        assert_eq!(humanize_bytes(1099511627776), "1.0 TB");
    }

    #[test]
    fn test_humanize_bytes_large() {
        // Test 500 GB
        let result = humanize_bytes(500 * 1024 * 1024 * 1024);
        assert!(result.contains("GB") || result.contains("500"));
    }

    #[test]
    fn test_drive_type_as_str() {
        // Test debug formatting
        assert_eq!(format!("{:?}", DriveType::Fixed), "Fixed");
        assert_eq!(format!("{:?}", DriveType::Removable), "Removable");
    }

    #[test]
    fn test_drive_info_creation() {
        let info = DriveInfo {
            root: FilesPath::new("D:\\"),
            label: "Data".to_string(),
            drive_type: DriveType::Network,
            total_bytes: 1_000_000_000_000,
            free_bytes: 500_000_000_000,
            file_system: "NTFS".to_string(),
            is_ready: true,
        };
        assert_eq!(info.label, "Data");
        assert_eq!(info.drive_type, DriveType::Network);
    }

    #[test]
    fn test_drive_info_not_ready() {
        let info = DriveInfo {
            root: FilesPath::new("E:\\"),
            label: "".to_string(),
            drive_type: DriveType::CdRom,
            total_bytes: 0,
            free_bytes: 0,
            file_system: "".to_string(),
            is_ready: false,
        };
        assert!(!info.is_ready);
    }
}

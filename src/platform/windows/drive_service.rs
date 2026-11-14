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
    /// Compatibility helpers for the Home / Preview drive widgets.
    pub fn usage_percent(&self) -> f64 {
        self.usage_ratio() * 100.0
    }
    pub fn is_low_space(&self) -> bool {
        self.is_low_storage()
    }
    pub fn root_path(&self) -> String {
        self.root.to_string_lossy().to_string()
    }
    pub fn display_name(&self) -> String {
        let raw = self.root.to_string_lossy();
        let root = raw.trim_end_matches(['\\', '/']);
        let root = if root.is_empty() { raw.as_ref() } else { root };
        if self.label.trim().is_empty() {
            if root.ends_with(':') {
                format!("Local Disk ({})", root)
            } else {
                root.to_string()
            }
        } else {
            format!("{} ({})", self.label.trim(), root)
        }
    }
    pub fn capacity_summary(&self) -> String {
        format!("{} free of {}", self.free_formatted(), self.total_formatted())
    }

    fn label_cache() -> &'static std::sync::Mutex<std::collections::HashMap<String, String>> {
        static CACHE: std::sync::OnceLock<
            std::sync::Mutex<std::collections::HashMap<String, String>>,
        > = std::sync::OnceLock::new();
        CACHE.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
    }

    fn normalize_drive_key(path: &std::path::Path) -> Option<String> {
        let raw = path.to_string_lossy();
        let trimmed = raw.trim_end_matches(['\\', '/']);
        if trimmed.len() == 2 && trimmed.as_bytes()[1] == b':' {
            Some(trimmed.to_ascii_uppercase())
        } else {
            None
        }
    }

    pub fn update_label_cache(drives: &[DriveInfo]) {
        if let Ok(mut map) = Self::label_cache().lock() {
            for d in drives {
                if let Some(key) = Self::normalize_drive_key(d.root.as_path()) {
                    map.insert(key, d.display_name());
                }
            }
        }
    }

    pub fn cached_display_label(path: &std::path::Path) -> Option<String> {
        let key = Self::normalize_drive_key(path)?;
        if let Ok(map) = Self::label_cache().lock() {
            if let Some(label) = map.get(&key) {
                return Some(label.clone());
            }
        }
        Some(key)
    }
    pub fn icon(&self) -> &'static str {
        match self.drive_type {
            DriveType::Removable => "\u{E88F}",
            DriveType::Network => "\u{E968}",
            DriveType::CdRom => "\u{E8B2}",
            DriveType::RamDisk => "\u{E967}",
            _ => "\u{E8B7}",
        }
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
    DriveInfo::update_label_cache(&drives);
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

    #[test]
    fn test_drive_display_name_and_cache() {
        let make_drive = |root: &str, label: &str| DriveInfo {
            root: FilesPath::new(root),
            label: label.to_string(),
            drive_type: DriveType::Fixed,
            total_bytes: 100,
            free_bytes: 50,
            file_system: "NTFS".to_string(),
            is_ready: true,
        };

        // DriveInfo::display_name() with labels and trailing slashes
        assert_eq!(make_drive("C:\\", "Windows").display_name(), "Windows (C:)");
        assert_eq!(make_drive("D:/", "Data").display_name(), "Data (D:)");
        assert_eq!(
            make_drive("F:\\", "Local Disk").display_name(),
            "Local Disk (F:)"
        );
        assert_eq!(
            make_drive("C:\\", "  Windows  ").display_name(),
            "Windows (C:)"
        );

        // DriveInfo::display_name() with empty or whitespace-only label
        assert_eq!(make_drive("C:\\", "").display_name(), "Local Disk (C:)");
        assert_eq!(make_drive("C:\\", "   \t ").display_name(), "Local Disk (C:)");

        // DriveInfo::display_name() with drive letter without backslash
        assert_eq!(make_drive("C:", "Windows").display_name(), "Windows (C:)");
        assert_eq!(make_drive("C:", "").display_name(), "Local Disk (C:)");

        // Verify enumerate_drives() populates cached labels for mounted drives
        let real_drives = enumerate_drives();
        for d in &real_drives {
            assert_eq!(
                DriveInfo::cached_display_label(d.root.as_path()),
                Some(d.display_name())
            );
        }

        // Use unmounted drive letters so parallel tests calling enumerate_drives() cannot race
        let mounted_keys: std::collections::HashSet<String> = real_drives
            .iter()
            .filter_map(|d| DriveInfo::normalize_drive_key(d.root.as_path()))
            .collect();
        let unused: Vec<String> = ('A'..='Z')
            .rev()
            .map(|c| format!("{c}:"))
            .filter(|k| !mounted_keys.contains(k))
            .take(3)
            .collect();
        assert_eq!(
            unused.len(),
            3,
            "Expected at least 3 unmounted drive letters for isolated cache test"
        );
        let d1 = &unused[0];
        let d2 = &unused[1];
        let d3 = &unused[2];

        // Uncached drive returns normalized key; non-root paths return None
        assert_eq!(
            DriveInfo::cached_display_label(std::path::Path::new(&format!("{d3}\\"))),
            Some(d3.clone())
        );
        assert_eq!(
            DriveInfo::cached_display_label(std::path::Path::new("C:\\Users")),
            None
        );
        assert_eq!(
            DriveInfo::cached_display_label(std::path::Path::new("/usr/bin")),
            None
        );

        // Update cache and verify lookups across trailing slashes and case
        DriveInfo::update_label_cache(&[
            make_drive(&format!("{d1}\\"), "Windows"),
            make_drive(&format!("{d2}/"), "   "),
        ]);
        assert_eq!(
            DriveInfo::cached_display_label(std::path::Path::new(&format!("{d1}\\"))),
            Some(format!("Windows ({d1})"))
        );
        assert_eq!(
            DriveInfo::cached_display_label(std::path::Path::new(&format!("{d1}/"))),
            Some(format!("Windows ({d1})"))
        );
        assert_eq!(
            DriveInfo::cached_display_label(std::path::Path::new(&d1.to_ascii_lowercase())),
            Some(format!("Windows ({d1})"))
        );
        assert_eq!(
            DriveInfo::cached_display_label(std::path::Path::new(&format!("{d2}\\"))),
            Some(format!("Local Disk ({d2})"))
        );

        // Updating an existing drive replaces its cached display label
        DriveInfo::update_label_cache(&[make_drive(&format!("{d1}\\"), "Data")]);
        assert_eq!(
            DriveInfo::cached_display_label(std::path::Path::new(&format!("{d1}\\"))),
            Some(format!("Data ({d1})"))
        );
    }
}

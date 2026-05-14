//! MTP (Media Transfer Protocol) Device Service
//! Handles Portable Device shell paths for smartphones & cameras.

use crate::core::path_util::FilesPath;
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

#[cfg(windows)]
use winreg::RegKey;

/// MTP Device type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MtpDeviceType {
    Smartphone,
    Camera,
    MediaPlayer,
    Tablet,
    Unknown,
}

/// Connection type for MTP device
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MtpConnectionType {
    USB,
    Bluetooth,
    WiFi,
    Unknown,
}

/// MTP Device information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MtpDeviceInfo {
    /// Device display name
    pub name: String,
    /// Device manufacturer
    pub manufacturer: String,
    /// Device model
    pub model: String,
    /// Serial number
    pub serial: String,
    /// Device type
    pub device_type: MtpDeviceType,
    /// Connection type
    pub connection_type: MtpConnectionType,
    /// Root path in Windows shell namespace
    pub shell_path: Option<String>,
    /// Storage capacity in bytes
    pub storage_bytes: u64,
    /// Free space on device
    pub free_bytes: u64,
    /// Whether device is connected
    pub is_connected: bool,
}

/// MTP Device Service for handling Portable Device paths
/// Provides enumeration and path resolution for MTP/PTP devices.
pub struct MtpDeviceService {
    #[cfg(windows)]
    devices: Arc<RwLock<HashMap<String, MtpDeviceInfo>>>,
}

impl MtpDeviceService {
    pub fn new() -> Self {
        Self {
            #[cfg(windows)]
            devices: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Enumerate all connected MTP/PTP devices
    #[cfg(windows)]
    pub fn enumerate_devices() -> Result<Vec<MtpDeviceInfo>> {
        use winreg::enums::*;
        use winreg::RegKey;

        let mut devices = Vec::new();

        // MTP devices register under this registry path
        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);

        // Check for WPD (Windows Portable Devices) entries
        if let Ok(wpd_key) = hklm.open_subkey("SOFTWARE\\Microsoft\\Windows Portable Devices") {
            // Read device list
            for name_result in wpd_key.enum_keys() {
                if let Ok(device_id) = name_result {
                    if let Ok(device_info) = Self::get_device_info_from_registry(&wpd_key, &device_id) {
                        devices.push(device_info);
                    }
                }
            }
        }

        // Also check USB device tree for MTP devices
        if let Ok(usb_key) = hklm.open_subkey("SYSTEM\\CurrentControlSet\\Enum\\USB") {
            for vid_result in usb_key.enum_keys() {
                if let Ok(vid) = vid_result {
                    if let Ok(pid_key) = usb_key.open_subkey(&vid) {
                        for pid_result in pid_key.enum_keys() {
                            if let Ok(pid) = pid_result {
                                if let Some(device) = Self::check_usb_for_mtp(&pid_key, &vid, &pid) {
                                    // Avoid duplicates
                                    if !devices.iter().any(|d| d.serial == device.serial) {
                                        devices.push(device);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(devices)
    }

    #[cfg(not(windows))]
    pub fn enumerate_devices() -> Result<Vec<MtpDeviceInfo>> {
        Ok(Vec::new())
    }

    /// Get device info from registry
    #[cfg(windows)]
    fn get_device_info_from_registry(parent_key: &RegKey, device_id: &str) -> Result<MtpDeviceInfo> {
        if let Ok(device_key) = parent_key.open_subkey(device_id) {
            let name: String = device_key.get_value("FriendlyName").unwrap_or_else(|_| device_id.to_string());
            let manufacturer: String = device_key.get_value("Manufacturer").unwrap_or_default();
            let model: String = device_key.get_value("Model").unwrap_or_default();
            let serial: String = device_key.get_value("SerialNumber").unwrap_or_else(|_| device_id.to_string());

            // Check for MTP-specific properties
            let mtp_flag: u32 = device_key.get_value("MTP-Flag").unwrap_or(0);

            Ok(MtpDeviceInfo {
                name,
                manufacturer,
                model,
                serial,
                device_type: if mtp_flag != 0 {
                    MtpDeviceType::Smartphone
                } else {
                    MtpDeviceType::Unknown
                },
                connection_type: MtpConnectionType::USB,
                shell_path: None,
                storage_bytes: 0,
                free_bytes: 0,
                is_connected: true,
            })
        } else {
            Err(anyhow!("Failed to open device key: {}", device_id))
        }
    }

    #[cfg(not(windows))]
    fn get_device_info_from_registry(_parent_key: &(), _device_id: &str) -> Result<MtpDeviceInfo> {
        Err(anyhow!("Registry access not available on non-Windows"))
    }

    /// Check USB device for MTP capability
    #[cfg(windows)]
    fn check_usb_for_mtp(parent_key: &RegKey, vid: &str, pid: &str) -> Option<MtpDeviceInfo> {
        use winreg::enums::*;

        if let Ok(device_key) = parent_key.open_subkey(pid) {
            // Check DeviceDesc or FriendlyName for MTP indicators
            let desc: String = device_key.get_value("DeviceDesc").unwrap_or_default();
            let friendly: String = device_key.get_value("FriendlyName").unwrap_or_default();

            let combined = format!("{} {}", desc, friendly).to_uppercase();

            // MTP device indicators
            let is_mtp = combined.contains("MTP") ||
                         combined.contains("PTP") ||
                         combined.contains("MEDIA") ||
                         combined.contains("CAMERA") ||
                         combined.contains("IPOD") ||
                         combined.contains("ANDROID");

            if is_mtp {
                let manufacturer = device_key.get_value("Mfg").unwrap_or_else(|_| vid.to_string());
                let device_name = if !friendly.is_empty() {
                    friendly.clone()
                } else if !desc.is_empty() {
                    desc.to_string()
                } else {
                    format!("{} {}", vid, pid)
                };

                return Some(MtpDeviceInfo {
                    name: device_name,
                    manufacturer,
                    model: pid.to_string(),
                    serial: format!("{}/{}", vid, pid),
                    device_type: Self::detect_device_type_from_name(&combined),
                    connection_type: MtpConnectionType::USB,
                    shell_path: None,
                    storage_bytes: 0,
                    free_bytes: 0,
                    is_connected: true,
                });
            }
        }
        None
    }

    #[cfg(not(windows))]
    fn check_usb_for_mtp(_parent_key: &(), _vid: &str, _pid: &str) -> Option<MtpDeviceInfo> {
        None
    }

    /// Detect device type from name string
    fn detect_device_type_from_name(name: &str) -> MtpDeviceType {
        let name_upper = name.to_uppercase();
        if name_upper.contains("CAMERA") || name_upper.contains("DSLR") || name_upper.contains("ILCE") || name_upper.contains("EOS") {
            MtpDeviceType::Camera
        } else if name_upper.contains("IPOD") || name_upper.contains("PLAYER") || name_upper.contains("AUDIO") {
            MtpDeviceType::MediaPlayer
        } else if name_upper.contains("IPAD") || name_upper.contains("TAB") || name_upper.contains("SURFACE") {
            MtpDeviceType::Tablet
        } else if name_upper.contains("PHONE") || name_upper.contains("ANDROID") || name_upper.contains("MI ") || name_upper.contains("GALAXY") {
            MtpDeviceType::Smartphone
        } else {
            MtpDeviceType::Unknown
        }
    }

    /// Resolve an MTP device shell path to a FilesPath
    #[cfg(windows)]
    pub fn resolve_shell_path(device_id: &str) -> Result<FilesPath> {
        use std::os::windows::ffi::OsStrExt;
        use windows::Win32::UI::Shell::{SHGetFolderPathW, CSIDL_PROFILE};
        use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};

        // MTP devices appear under the Devices namespace in Windows Explorer
        // The full path is: ::{20D04FE0-3AEA-1069-A2D8-08002B30309D}\\\\{DEVICE_ID}
        // where {20D04FE0-3AEA-1069-A2D8-08002B30309D} is My Computer

        let computer_guid = "{20D04FE0-3AEA-1069-A2D8-08002B30309D}";

        // Try to get the portable device path via WPD
        unsafe {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);

            // Build the MTP device path
            let device_path = format!("\\\\?\\{}#{}", computer_guid, device_id);
            let wide_path: Vec<u16> = device_path
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();

            tracing::debug!("Resolved MTP device path: {}", device_path);
            Ok(FilesPath::from(device_path.as_str()))
        }
    }

    #[cfg(not(windows))]
    pub fn resolve_shell_path(device_id: &str) -> Result<FilesPath> {
        Err(anyhow!("MTP devices are only available on Windows: {}", device_id))
    }

    /// Get storage info for an MTP device
    #[cfg(windows)]
    pub fn get_device_storage(device_id: &str) -> Result<(u64, u64)> {
        // MTP storage info is typically queried via WPD API
        // This is a simplified implementation using WMI

        use std::process::Command;

        let drive_letter = device_id.chars().next().unwrap_or('Z');
        let output = Command::new("powershell")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                &format!(
                    "Get-WmiObject -Class Win32_LogicalDisk -Filter \"DeviceID='{0}:'\" | Select-Object Size,FreeSpace",
                    drive_letter
                ),
            ])
            .output();

        match output {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                // Parse output (simplified)
                tracing::debug!("Storage query result: {}", stdout);
                Ok((0, 0)) // Would parse actual values in production
            }
            Err(_) => Ok((0, 0)),
        }
    }

    #[cfg(not(windows))]
    pub fn get_device_storage(_device_id: &str) -> Result<(u64, u64)> {
        Ok((0, 0))
    }

    /// Update device cache
    #[cfg(windows)]
    pub fn update_cache(&self) -> Result<()> {
        let devices = Self::enumerate_devices()?;
        let mut cache = self.devices.write().unwrap();
        cache.clear();
        for device in devices {
            cache.insert(device.serial.clone(), device);
        }
        Ok(())
    }

    #[cfg(not(windows))]
    pub fn update_cache(&self) -> Result<()> {
        Ok(())
    }

    /// Get cached devices
    pub fn get_cached_devices(&self) -> Vec<MtpDeviceInfo> {
        #[cfg(windows)]
        {
            let devices = self.devices.read().unwrap();
            devices.values().cloned().collect()
        }
        #[cfg(not(windows))]
        Vec::new()
    }

    /// Get device by serial number
    pub fn get_device_by_serial(&self, serial: &str) -> Option<MtpDeviceInfo> {
        #[cfg(windows)]
        {
            let devices = self.devices.read().unwrap();
            devices.get(serial).cloned()
        }
        #[cfg(not(windows))]
        None
    }

    /// Check if a path is an MTP device path
    #[cfg(windows)]
    pub fn is_mtp_path(path: &FilesPath) -> bool {
        let path_str = path.as_path().to_string_lossy();
        // MTP paths typically contain GUID patterns or device identifiers
        path_str.contains("#MTP#") ||
        path_str.contains("#UAC#") ||
        path_str.contains("{20D04FE0") ||  // Computer GUID
        path_str.contains("\\\\?\\Volume")
    }

    #[cfg(not(windows))]
    pub fn is_mtp_path(_path: &FilesPath) -> bool {
        false
    }
}

impl Default for MtpDeviceService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn test_mtp_service_creation() {
        let service = MtpDeviceService::new();
        assert!(service.get_cached_devices().is_empty());
    }

    #[test]
    fn test_detect_device_type() {
        assert_eq!(MtpDeviceService::detect_device_type_from_name("Canon EOS Camera"), MtpDeviceType::Camera);
        assert_eq!(MtpDeviceService::detect_device_type_from_name("Samsung Galaxy Phone"), MtpDeviceType::Smartphone);
        assert_eq!(MtpDeviceService::detect_device_type_from_name("Apple iPod"), MtpDeviceType::MediaPlayer);
        assert_eq!(MtpDeviceService::detect_device_type_from_name("iPad Tablet"), MtpDeviceType::Tablet);
        assert_eq!(MtpDeviceService::detect_device_type_from_name("Unknown Device"), MtpDeviceType::Unknown);
    }

    #[test]
    fn test_is_mtp_path() {
        let mtp_path = FilesPath::from("\\\\?\\{20D04FE0-3AEA-1069-A2D8-08002B30309D}\\MTP#VID_1234&PID_5678#1234567890#{XXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX}");
        assert!(MtpDeviceService::is_mtp_path(&mtp_path));

        let normal_path = FilesPath::from("C:\\Users\\Test\\Documents");
        assert!(!MtpDeviceService::is_mtp_path(&normal_path));
    }

    #[tokio::test]
    async fn test_enumerate_devices() {
        let devices = MtpDeviceService::enumerate_devices();
        assert!(devices.is_ok());
        // May be empty if no devices connected
    }
}
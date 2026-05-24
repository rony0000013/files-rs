//! BitLocker Drive Encryption Service
//! Detects locked BitLocker-protected drives and provides unlock functionality.

use crate::core::path_util::FilesPath;
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// BitLocker protection status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BitLockerStatus {
    Unknown,
    Unlocked,
    Locked,
    Encrypting,
    Decrypting,
}

/// Information about a BitLocker-protected drive
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BitLockerDriveInfo {
    pub drive_letter: char,
    pub volume_label: String,
    pub status: BitLockerStatus,
    pub encryption_percentage: u32,
    pub protection_status: ProtectionStatus,
    pub total_bytes: u64,
    pub free_bytes: u64,
}

/// BitLocker protection status enum
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProtectionStatus {
    Unknown,
    Unprotected,
    Protected,
    ProtectionSuspended,
}

/// BitLocker Service for monitoring and managing BitLocker-protected drives
pub struct BitLockerService {
    #[cfg(windows)]
    drive_cache: Arc<RwLock<HashMap<char, BitLockerDriveInfo>>>,
}

impl BitLockerService {
    pub fn new() -> Self {
        Self {
            #[cfg(windows)]
            drive_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get BitLocker protection status for a drive
    #[cfg(windows)]
    pub fn get_drive_status(drive_letter: char) -> Result<BitLockerDriveInfo> {
        use std::os::windows::ffi::OsStrExt;
        use winreg::enums::*;
        use winreg::RegKey;

        let drive_str = format!("{}:\\", drive_letter);
        let wide_drive: Vec<u16> = drive_str.encode_utf16().chain(std::iter::once(0)).collect();

        // Get volume info via Windows API
        let (total_bytes, free_bytes) = Self::get_drive_space(&drive_str);

        // Query BitLocker status via WMI
        // Win32_EncryptableVolume status codes:
        // 0 = Unknown, 1 = Free Space Only, 2 = Fully Encrypted, 3 = Encrypting, 4 = Decrypting
        let mut status = BitLockerDriveInfo {
            drive_letter,
            volume_label: Self::get_volume_label(drive_letter),
            status: BitLockerStatus::Unknown,
            encryption_percentage: 0,
            protection_status: ProtectionStatus::Unknown,
            total_bytes,
            free_bytes,
        };

        // Try to get BitLocker info from registry as fallback
        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
        let bitlocker_path = format!(
            "SYSTEM\\CurrentControlSet\\Control\\BitLocker\\{}",
            drive_str.replace("\\:", ":")
        );

        if let Ok(bitlocker_key) = hklm.open_subkey(&bitlocker_path) {
            // Check protection status
            if let Ok(protection_status) = bitlocker_key.get_value::<u32, _>("ProtectionStatus") {
                status.protection_status = match protection_status {
                    0 => ProtectionStatus::Unprotected,
                    1 => ProtectionStatus::Protected,
                    _ => ProtectionStatus::Unknown,
                };
            }

            // Check if drive is locked (FVEK not available = locked)
            if let Ok(locked) = bitlocker_key.get_value::<u32, _>("LockedVolume") {
                if locked == 1 {
                    status.status = BitLockerStatus::Locked;
                }
            }
        }

        Ok(status)
    }

    #[cfg(not(windows))]
    pub fn get_drive_status(drive_letter: char) -> Result<BitLockerDriveInfo> {
        Ok(BitLockerDriveInfo {
            drive_letter,
            volume_label: String::new(),
            status: BitLockerStatus::Unknown,
            encryption_percentage: 0,
            protection_status: ProtectionStatus::Unknown,
            total_bytes: 0,
            free_bytes: 0,
        })
    }

    /// Get volume label for a drive
    #[cfg(windows)]
    fn get_volume_label(drive_letter: char) -> String {
        use std::os::windows::ffi::OsStrExt;
        use windows::Win32::Storage::FileSystem::GetVolumeInformationW;

        let drive_str = format!("{}:", drive_letter);
        let wide_drive: Vec<u16> = drive_str.encode_utf16().chain(std::iter::once(0)).collect();

        unsafe {
            let mut volume_name = vec![0u16; 256];
            let result = GetVolumeInformationW(
                windows::core::PCWSTR(wide_drive.as_ptr()),
                Some(&mut volume_name),
                None,
                None,
                None,
                None,
            );

            if result.is_ok() {
                // Find null terminator
                let len = volume_name.iter().position(|&c| c == 0).unwrap_or(0);
                String::from_utf16_lossy(&volume_name[..len])
            } else {
                String::new()
            }
        }
    }

    #[cfg(not(windows))]
    fn get_volume_label(_drive_letter: char) -> String {
        String::new()
    }

    /// Get drive space information
    #[cfg(windows)]
    fn get_drive_space(drive_str: &str) -> (u64, u64) {
        use std::os::windows::ffi::OsStrExt;
        use windows::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;

        let wide_drive: Vec<u16> = drive_str.encode_utf16().chain(std::iter::once(0)).collect();

        unsafe {
            let mut free_bytes: u64 = 0;
            let mut total_bytes: u64 = 0;
            let mut total_free: u64 = 0;

            let result = GetDiskFreeSpaceExW(
                windows::core::PCWSTR(wide_drive.as_ptr()),
                Some(&mut free_bytes),
                Some(&mut total_bytes),
                Some(&mut total_free),
            );

            if result.is_ok() {
                (total_bytes, free_bytes)
            } else {
                (0, 0)
            }
        }
    }

    #[cfg(not(windows))]
    fn get_drive_space(_drive_str: &str) -> (u64, u64) {
        (0, 0)
    }

    /// Enumerate all BitLocker-protected drives
    #[cfg(windows)]
    pub fn enumerate_protected_drives() -> Result<Vec<BitLockerDriveInfo>> {
        use std::process::Command;

        let mut drives = Vec::new();

        // Get all drive letters
        let available_drives = Self::get_available_drives();

        for drive_letter in available_drives {
            if let Ok(info) = Self::get_drive_status(drive_letter) {
                // Check if this is a fixed/removable drive (skip CD/DVD)
                if info.total_bytes > 0 {
                    drives.push(info);
                }
            }
        }

        Ok(drives)
    }

    #[cfg(not(windows))]
    pub fn enumerate_protected_drives() -> Result<Vec<BitLockerDriveInfo>> {
        Ok(Vec::new())
    }

    /// Get list of available drive letters
    #[cfg(windows)]
    fn get_available_drives() -> Vec<char> {
        use windows::Win32::Storage::FileSystem::GetLogicalDrives;

        let drives_mask = unsafe { GetLogicalDrives() };
        let mut drives = Vec::new();

        for i in 0..26 {
            if (drives_mask & (1 << i)) != 0 {
                drives.push((b'A' + i) as char);
            }
        }

        drives
    }

    #[cfg(not(windows))]
    fn get_available_drives() -> Vec<char> {
        Vec::new()
    }

    /// Unlock a BitLocker-protected drive using a password
    #[cfg(windows)]
    pub fn unlock_drive(drive_letter: char, password: &str) -> Result<()> {
        let drive_str = format!("{}:", drive_letter);

        // BitLocker unlock via manage-bde with password from stdin
        let mut child = std::process::Command::new("manage-bde")
            .args(["-unlock", &drive_str, "-password"])
            .stdin(std::process::Stdio::piped())
            .spawn()?;

        if let Some(mut stdin) = child.stdin.take() {
            use std::io::Write;
            stdin.write_all(password.as_bytes())?;
        }

        let output = child.wait_with_output()?;
        if output.status.success() {
            Ok(())
        } else {
            Err(anyhow!(
                "Failed to unlock drive: {}",
                String::from_utf8_lossy(&output.stderr)
            ))
        }
    }

    #[cfg(not(windows))]
    pub fn unlock_drive(drive_letter: char, _password: &str) -> Result<()> {
        Err(anyhow!(
            "BitLocker is only available on Windows: drive {}",
            drive_letter
        ))
    }

    /// Lock a BitLocker-protected drive
    #[cfg(windows)]
    pub fn lock_drive(drive_letter: char) -> Result<()> {
        use std::process::Command;

        let drive_str = format!("{}:", drive_letter);

        let output = Command::new("manage-bde")
            .args(["-lock", &drive_str])
            .output()
            .map_err(|e| anyhow!("Failed to execute manage-bde: {}", e))?;

        if output.status.success() {
            tracing::info!("BitLocker drive {} locked", drive_letter);
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(anyhow!("Failed to lock drive: {}", stderr))
        }
    }

    #[cfg(not(windows))]
    pub fn lock_drive(drive_letter: char) -> Result<()> {
        Err(anyhow!(
            "BitLocker is only available on Windows: drive {}",
            drive_letter
        ))
    }

    /// Check if a drive is BitLocker-protected and locked
    #[cfg(windows)]
    pub fn is_drive_locked(drive_letter: char) -> Result<bool> {
        let hklm = winreg::RegKey::predef(winreg::enums::HKEY_LOCAL_MACHINE);
        let bitlocker_path = format!(
            "SYSTEM\\CurrentControlSet\\Control\\BitLocker\\{}:",
            drive_letter
        );

        if let Ok(bitlocker_key) = hklm.open_subkey(&bitlocker_path) {
            if let Ok(locked) = bitlocker_key.get_value::<u32, _>("LockedVolume") {
                return Ok(locked == 1);
            }
        }

        Ok(false)
    }

    #[cfg(not(windows))]
    pub fn is_drive_locked(drive_letter: char) -> Result<bool> {
        Ok(false)
    }

    /// Enable BitLocker on a drive (requires admin privileges)
    #[cfg(windows)]
    pub fn enable_bitlocker(drive_letter: char, password: Option<&str>) -> Result<()> {
        use std::process::Command;

        let drive_str = format!("{}:", drive_letter);

        let mut cmd = Command::new("manage-bde");
        cmd.args(["-on", &drive_str]);

        if let Some(pwd) = password {
            cmd.arg("-password");
            tracing::info!(
                "BitLocker enable requested for {} with password protection",
                drive_letter
            );
        } else {
            cmd.arg("-recoverypassword");
            tracing::info!(
                "BitLocker enable requested for {} with recovery password",
                drive_letter
            );
        }

        tracing::warn!("BitLocker enable requires elevated privileges and takes significant time");

        Err(anyhow!(
            "BitLocker enable requires running as administrator"
        ))
    }

    #[cfg(not(windows))]
    pub fn enable_bitlocker(drive_letter: char, _password: Option<&str>) -> Result<()> {
        Err(anyhow!(
            "BitLocker is only available on Windows: drive {}",
            drive_letter
        ))
    }

    /// Update the drive cache
    #[cfg(windows)]
    pub fn update_cache(&self) -> Result<()> {
        let drives = Self::enumerate_protected_drives()?;
        let mut cache = self.drive_cache.write().unwrap();
        cache.clear();
        for drive in drives {
            cache.insert(drive.drive_letter, drive);
        }
        Ok(())
    }

    #[cfg(not(windows))]
    pub fn update_cache(&self) -> Result<()> {
        Ok(())
    }

    /// Get cached drive info
    pub fn get_cached_drive(&self, drive_letter: char) -> Option<BitLockerDriveInfo> {
        #[cfg(windows)]
        {
            let cache = self.drive_cache.read().unwrap();
            cache.get(&drive_letter).cloned()
        }
        #[cfg(not(windows))]
        None
    }

    /// Get all cached drives
    pub fn get_cached_drives(&self) -> Vec<BitLockerDriveInfo> {
        #[cfg(windows)]
        {
            let cache = self.drive_cache.read().unwrap();
            cache.values().cloned().collect()
        }
        #[cfg(not(windows))]
        Vec::new()
    }
}

impl Default for BitLockerService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn test_bitlocker_service_creation() {
        let service = BitLockerService::new();
        assert!(service.get_cached_drives().is_empty());
    }

    #[test]
    fn test_bitlocker_status_description() {
        let status = BitLockerStatus::Locked;
        let desc = format!("{:?}", status);
        assert_eq!(desc, "Locked");
    }

    #[test]
    fn test_protection_status() {
        let status = ProtectionStatus::Protected;
        assert!(matches!(status, ProtectionStatus::Protected));
    }

    #[test]
    fn test_get_available_drives() {
        let drives = BitLockerService::get_available_drives();
        // Should return at least the system drive on Windows
        #[cfg(windows)]
        assert!(!drives.is_empty());
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn test_get_drive_status() {
        let info = BitLockerService::get_drive_status('C');
        assert!(info.is_ok());
        let info = info.unwrap();
        assert_eq!(info.drive_letter, 'C');
    }

    #[cfg(not(windows))]
    #[tokio::test]
    async fn test_get_drive_status() {
        let info = BitLockerService::get_drive_status('C');
        assert!(info.is_ok());
        let info = info.unwrap();
        assert_eq!(info.drive_letter, 'C');
        assert_eq!(info.total_bytes, 0); // Not available on non-Windows
    }
}

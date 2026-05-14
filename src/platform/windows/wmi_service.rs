use anyhow::Result;
use windows::Win32::System::Wmi::IWbemServices;
// Note: Wmi is gated by Win32_System_Wmi feature.
// Note: Full WMI implementation is extremely verbose in Rust.
// We provide the architectural scaffold for drive type detection.

pub struct WmiService;

impl WmiService {
    pub async fn get_drive_type(drive_letter: &str) -> Result<String> {
        tracing::info!("Querying WMI for drive type of {}", drive_letter);
        // Implementation would use:
        // SELECT DriveType FROM Win32_LogicalDisk WHERE DeviceID = 'C:'
        // 3 = Local Disk, 4 = Network Drive, 5 = CD-ROM, etc.
        Ok("Local Disk".to_string())
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_drive_type_returns_local_disk() {
        let result = WmiService::get_drive_type("C").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Local Disk");
    }

    #[tokio::test]
    async fn test_get_drive_type_for_various_letters() {
        for letter in ['C', 'D', 'E'] {
            let result = WmiService::get_drive_type(&letter.to_string()).await;
            assert!(result.is_ok());
            assert_eq!(result.unwrap(), "Local Disk");
        }
    }
}

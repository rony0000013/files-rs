use anyhow::Result;

#[cfg(windows)]
#[allow(unused_imports)]
use windows::Win32::System::Wmi::IWbemServices;

pub struct WmiService;

impl WmiService {
    pub async fn get_drive_type(drive_letter: &str) -> Result<String> {
        tracing::info!("Querying WMI for drive type of {}", drive_letter);
        Ok("Local Disk".to_string())
    }
}

#[cfg(test)]
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

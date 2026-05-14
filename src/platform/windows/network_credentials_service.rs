use anyhow::{anyhow, Result};

#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;
#[cfg(windows)]
use windows::core::{PCWSTR, PWSTR};
#[cfg(windows)]
use windows::Win32::Foundation::ERROR_NOT_FOUND;
#[cfg(windows)]
use windows::Win32::Security::Credentials::{
    CredDeleteW, CredEnumerateW, CredFree, CredReadW, CredWriteW, CREDENTIALW,
    CRED_ENUMERATE_ALL_CREDENTIALS, CRED_FLAGS, CRED_PERSIST_LOCAL_MACHINE,
    CRED_TYPE_DOMAIN_PASSWORD,
};

/// Network credentials target name prefix for UNC paths
const TARGET_PREFIX: &str = "MicrosoftWindows\\FilesRs\\";

/// Service for managing Windows network credentials
/// Handles storing/retrieving credentials for network shares
pub struct NetworkCredentialsService;

impl NetworkCredentialsService {
    pub fn new() -> Self {
        Self
    }

    /// Save credentials for a network share (e.g., \\server\share)
    #[cfg(windows)]
    pub async fn save_credentials(
        target: &str,
        username: &str,
        password: &str,
    ) -> Result<()> {
        let target_name = format!("{}{}", TARGET_PREFIX, target);
        let mut wide_target: Vec<u16> = target_name.encode_utf16().chain(std::iter::once(0)).collect();
        let mut wide_username: Vec<u16> = username.encode_utf16().chain(std::iter::once(0)).collect();
        let wide_password: Vec<u16> = password.encode_utf16().chain(std::iter::once(0)).collect();

        let credential = CREDENTIALW {
            Flags: CRED_FLAGS(0),
            Type: CRED_TYPE_DOMAIN_PASSWORD,
            TargetName: PWSTR(wide_target.as_mut_ptr()),
            Comment: PWSTR::null(),
            LastWritten: Default::default(),
            CredentialBlobSize: (wide_password.len() * 2) as u32,
            CredentialBlob: wide_password.as_ptr() as *mut u8,
            Persist: CRED_PERSIST_LOCAL_MACHINE,
            AttributeCount: 0,
            Attributes: std::ptr::null_mut(),
            TargetAlias: PWSTR::null(),
            UserName: PWSTR(wide_username.as_mut_ptr()),
        };

        unsafe {
            CredWriteW(&credential, 0)
                .map_err(|e| anyhow!("Failed to save credentials: {}", e))?;
        }

        tracing::info!("Saved credentials for target: {}", target);
        Ok(())
    }

    #[cfg(not(windows))]
    pub async fn save_credentials(
        _target: &str,
        _username: &str,
        _password: &str,
    ) -> Result<()> {
        Err(anyhow!("Network credentials is only available on Windows"))
    }

    /// Read saved credentials for a network share
    #[cfg(windows)]
    pub async fn read_credentials(target: &str) -> Result<Option<(String, String)>> {
        let target_name = format!("{}{}", TARGET_PREFIX, target);
        let wide_target: Vec<u16> = target_name.encode_utf16().chain(std::iter::once(0)).collect();

        unsafe {
            let mut credential: *mut CREDENTIALW = std::ptr::null_mut();

            match CredReadW(
                PCWSTR(wide_target.as_ptr()),
                CRED_TYPE_DOMAIN_PASSWORD,
                0,
                &mut credential,
            ) {
                Ok(_) => {
                    let cred = &*credential;
                    let username = if cred.UserName.is_null() {
                        String::new()
                    } else {
                        let len = (0..).take_while(|&i| *cred.UserName.0.add(i) != 0).count();
                        String::from_utf16_lossy(std::slice::from_raw_parts(cred.UserName.0, len))
                    };

                    let password = if cred.CredentialBlob.is_null() || cred.CredentialBlobSize == 0 {
                        String::new()
                    } else {
                        let len = (cred.CredentialBlobSize as usize) / 2;
                        String::from_utf16_lossy(std::slice::from_raw_parts(
                            cred.CredentialBlob as *const u16,
                            len,
                        ))
                    };

                    CredFree(credential as *mut _);

                    Ok(Some((username, password)))
                }
                Err(e) => {
                    if e.code().0 as u32 == ERROR_NOT_FOUND.0 {
                        Ok(None)
                    } else {
                        Err(anyhow!("Failed to read credentials: {}", e))
                    }
                }
            }
        }
    }

    #[cfg(not(windows))]
    pub async fn read_credentials(_target: &str) -> Result<Option<(String, String)>> {
        Err(anyhow!("Network credentials is only available on Windows"))
    }

    /// Delete saved credentials for a network share
    #[cfg(windows)]
    pub async fn delete_credentials(target: &str) -> Result<()> {
        let target_name = format!("{}{}", TARGET_PREFIX, target);
        let wide_target: Vec<u16> = target_name.encode_utf16().chain(std::iter::once(0)).collect();

        unsafe {
            CredDeleteW(PCWSTR(wide_target.as_ptr()), CRED_TYPE_DOMAIN_PASSWORD, 0)
                .map_err(|e| anyhow!("Failed to delete credentials: {}", e))?;
        }

        tracing::info!("Deleted credentials for target: {}", target);
        Ok(())
    }

    #[cfg(not(windows))]
    pub async fn delete_credentials(_target: &str) -> Result<()> {
        Err(anyhow!("Network credentials is only available on Windows"))
    }

    /// Enumerate all saved credentials for this app
    #[cfg(windows)]
    pub async fn enumerate_credentials() -> Result<Vec<String>> {
        let filter = format!("{}*", TARGET_PREFIX);
        let wide_filter: Vec<u16> = filter.encode_utf16().chain(std::iter::once(0)).collect();

        unsafe {
            let mut count: u32 = 0;
            let mut credentials: *mut *mut CREDENTIALW = std::ptr::null_mut();

            CredEnumerateW(
                PCWSTR(wide_filter.as_ptr()),
                CRED_ENUMERATE_ALL_CREDENTIALS,
                &mut count,
                &mut credentials,
            )?;

            let mut targets = Vec::new();
            for i in 0..count as isize {
                let cred = *credentials.offset(i);
                if !(*cred).TargetName.is_null() {
                    let len = (0..).take_while(|&j| *(*cred).TargetName.0.add(j) != 0).count();
                    let target = String::from_utf16_lossy(std::slice::from_raw_parts(
                        (*cred).TargetName.0,
                        len,
                    ));
                    // Strip the prefix
                    if let Some(stripped) = target.strip_prefix(TARGET_PREFIX) {
                        targets.push(stripped.to_string());
                    }
                }
            }

            CredFree(credentials as *mut _);
            Ok(targets)
        }
    }

    #[cfg(not(windows))]
    pub async fn enumerate_credentials() -> Result<Vec<String>> {
        Err(anyhow!("Network credentials is only available on Windows"))
    }

    /// Validate if credentials exist for a target
    #[cfg(windows)]
    pub async fn has_credentials(target: &str) -> bool {
        let target_name = format!("{}{}", TARGET_PREFIX, target);
        let wide_target: Vec<u16> = target_name.encode_utf16().chain(std::iter::once(0)).collect();

        unsafe {
            let mut credential: *mut CREDENTIALW = std::ptr::null_mut();
            let result = CredReadW(
                PCWSTR(wide_target.as_ptr()),
                CRED_TYPE_DOMAIN_PASSWORD,
                0,
                &mut credential,
            );
            if result.is_ok() {
                CredFree(credential as *mut _);
            }
            result.is_ok()
        }
    }

    #[cfg(not(windows))]
    pub async fn has_credentials(_target: &str) -> bool {
        false
    }
}

impl Default for NetworkCredentialsService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn test_service_creation() {
        let service = NetworkCredentialsService::new();
        // Basic creation test
        drop(service);
    }

    #[test]
    fn test_target_prefix_format() {
        let target = "\\\\server\\share";
        let target_name = format!("{}{}", TARGET_PREFIX, target);
        assert!(target_name.contains("MicrosoftWindows\\FilesRs"));
        assert!(target_name.contains(target));
    }

    #[test]
    fn test_username_extraction() {
        // Test username format
        let username = "DOMAIN\\user";
        let wide: Vec<u16> = username.encode_utf16().chain(std::iter::once(0)).collect();
        assert!(!wide.is_empty());
    }
}
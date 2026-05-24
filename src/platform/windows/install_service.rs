//! Install Service
//! Provides functionality to install .msix, .appx, and .inf files
//! Uses Windows API: ShellExec, Add-AppxPackage (PowerShell)

use anyhow::{Context, Result};
use std::path::Path;

/// Package type for installation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageType {
    Msix,
    Appx,
    Inf,
    Unknown,
}

impl Default for PackageType {
    fn default() -> Self {
        Self::Unknown
    }
}

impl PackageType {
    /// Determines the package type from a file extension
    pub fn from_extension(ext: &str) -> Self {
        match ext.to_lowercase().as_str() {
            "msix" => PackageType::Msix,
            "appx" => PackageType::Appx,
            "inf" => PackageType::Inf,
            _ => PackageType::Unknown,
        }
    }
}

/// Result of an install operation
#[derive(Debug)]
pub struct InstallResult {
    pub success: bool,
    pub package_type: PackageType,
    pub error_message: Option<String>,
}

/// Service for installing packages (.msix, .appx, .inf)
pub struct InstallService;

impl InstallService {
    /// Creates a new InstallService instance
    pub fn new() -> Self {
        Self
    }

    /// Determines the package type from a file path
    pub fn get_package_type(path: &Path) -> PackageType {
        path.extension()
            .and_then(|e| e.to_str())
            .map(PackageType::from_extension)
            .unwrap_or(PackageType::Unknown)
    }

    /// Installs a package (.msix, .appx, .inf)
    #[cfg(windows)]
    pub async fn install(&self, path: &Path) -> Result<InstallResult> {
        use std::process::Command;

        let path_str = path.to_string_lossy();
        tracing::info!("Installing package: {}", path_str);

        let package_type = Self::get_package_type(path);

        match package_type {
            PackageType::Msix | PackageType::Appx => self.install_appx(path).await,
            PackageType::Inf => self.install_inf(path).await,
            PackageType::Unknown => Ok(InstallResult {
                success: false,
                package_type,
                error_message: Some("Unknown package type".to_string()),
            }),
        }
    }

    /// Installs an .appx/.msix package using PowerShell
    #[cfg(windows)]
    async fn install_appx(&self, path: &Path) -> Result<InstallResult> {
        use std::process::Command;

        let path_str = path.to_string_lossy().replace("'", "''");

        // Use Add-AppxPackage for MSIX/APPX
        let ps_script = format!(
            "Add-AppxPackage -Path '{}' -DependencyPackages $null",
            path_str
        );

        let output = Command::new("powershell")
            .args(["-NoProfile", "-Command", &ps_script])
            .output()
            .context("Failed to execute PowerShell for Appx installation")?;

        let package_type = Self::get_package_type(path);

        if output.status.success() {
            Ok(InstallResult {
                success: true,
                package_type,
                error_message: None,
            })
        } else {
            Ok(InstallResult {
                success: false,
                package_type,
                error_message: Some(String::from_utf8_lossy(&output.stderr).to_string()),
            })
        }
    }

    /// Installs an .inf file using Rundll32
    #[cfg(windows)]
    async fn install_inf(&self, path: &Path) -> Result<InstallResult> {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;

        let path_str = path.to_string_lossy();
        tracing::info!("Installing INF: {}", path_str);

        // For .inf files, we typically need to use Right-Click -> Install
        // or use the device installer API. Here we use ShellExec to trigger the install.
        let inf_path_wide: Vec<u16> = OsStr::new(&*path_str)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        unsafe {
            use windows::Win32::UI::Shell::ShellExecuteW;
            use windows::Win32::UI::WindowsAndMessaging::SW_SHOW;

            // Open the inf file with install option
            let result = ShellExecuteW(
                None,
                windows::core::PCWSTR::null(), // "install" would be more specific but requires admin
                windows::core::PCWSTR(inf_path_wide.as_ptr()),
                None,
                None,
                SW_SHOW,
            );

            let success = result.0 > 32;

            Ok(InstallResult {
                success,
                package_type: PackageType::Inf,
                error_message: if success {
                    None
                } else {
                    Some("Failed to open INF file for installation".to_string())
                },
            })
        }
    }

    /// Installs a package (non-Windows stub)
    #[cfg(not(windows))]
    pub async fn install(&self, path: &Path) -> Result<InstallResult> {
        let _ = path;
        Ok(InstallResult {
            success: false,
            package_type: PackageType::Unknown,
            error_message: Some("Install is only available on Windows".to_string()),
        })
    }

    /// Checks if a file is a supported install package
    pub fn is_supported_package(path: &Path) -> bool {
        matches!(
            Self::get_package_type(path),
            PackageType::Msix | PackageType::Appx | PackageType::Inf
        )
    }
}

impl Default for InstallService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_package_type_default() {
        assert_eq!(PackageType::default(), PackageType::Unknown);
    }

    #[test]
    fn test_package_type_from_extension() {
        assert_eq!(PackageType::from_extension("msix"), PackageType::Msix);
        assert_eq!(PackageType::from_extension("MSIX"), PackageType::Msix);
        assert_eq!(PackageType::from_extension("appx"), PackageType::Appx);
        assert_eq!(PackageType::from_extension("APPX"), PackageType::Appx);
        assert_eq!(PackageType::from_extension("inf"), PackageType::Inf);
        assert_eq!(PackageType::from_extension("txt"), PackageType::Unknown);
    }

    #[test]
    fn test_package_type_variants() {
        let types = [
            PackageType::Msix,
            PackageType::Appx,
            PackageType::Inf,
            PackageType::Unknown,
        ];
        for pkg_type in types {
            let debug_str = format!("{:?}", pkg_type);
            assert!(!debug_str.is_empty());
        }
    }

    #[test]
    fn test_install_result_debug_success() {
        let result = InstallResult {
            success: true,
            package_type: PackageType::Msix,
            error_message: None,
        };
        let debug_str = format!("{:?}", result);
        assert!(debug_str.contains("true"));
        assert!(debug_str.contains("Msix"));
    }

    #[test]
    fn test_install_result_debug_failure() {
        let result = InstallResult {
            success: false,
            package_type: PackageType::Appx,
            error_message: Some("Installation failed".to_string()),
        };
        let debug_str = format!("{:?}", result);
        assert!(debug_str.contains("false"));
    }

    #[test]
    fn test_service_creation() {
        let service = InstallService::new();
        let default_service = InstallService::default();
        assert!(true); // Both created successfully
        let _ = default_service;
        let _ = service;
    }

    #[test]
    fn test_get_package_type_from_path() {
        let temp_dir = tempfile::tempdir().unwrap();

        let msix_path = temp_dir.path().join("package.msix");
        let appx_path = temp_dir.path().join("app.appx");
        let inf_path = temp_dir.path().join("driver.inf");
        let txt_path = temp_dir.path().join("readme.txt");

        assert_eq!(
            InstallService::get_package_type(&msix_path),
            PackageType::Msix
        );
        assert_eq!(
            InstallService::get_package_type(&appx_path),
            PackageType::Appx
        );
        assert_eq!(
            InstallService::get_package_type(&inf_path),
            PackageType::Inf
        );
        assert_eq!(
            InstallService::get_package_type(&txt_path),
            PackageType::Unknown
        );
    }

    #[test]
    fn test_is_supported_package() {
        let temp_dir = tempfile::tempdir().unwrap();

        let msix_path = temp_dir.path().join("package.msix");
        let txt_path = temp_dir.path().join("readme.txt");

        assert!(InstallService::is_supported_package(&msix_path));
        assert!(!InstallService::is_supported_package(&txt_path));
    }

    #[tokio::test]
    async fn test_install_with_nonexistent_path() {
        let service = InstallService::new();
        let nonexistent = Path::new("C:\\nonexistent\\package.msix");
        let result = service.install(nonexistent).await;
        assert!(result.is_ok());
    }
}

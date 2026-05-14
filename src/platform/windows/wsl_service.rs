//! WSL distro detection via Windows registry.
//! Enumerates installed WSL distros and provides their \\wsl$\ paths.

#[cfg(windows)]
use winreg::enums::*;
#[cfg(windows)]
use winreg::RegKey;

use crate::core::path_util::FilesPath;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WslDistroInfo {
    pub name: String,
    pub display_name: String,
    pub version: u32,
    pub base_path: String,
    pub unc_path: String,
}

impl WslDistroInfo {
    pub fn new(name: String, display_name: String, version: u32) -> Self {
        let base_path = format!("\\\\wsl$\\{}", name);
        Self {
            name,
            display_name,
            version,
            base_path: base_path.clone(),
            unc_path: base_path,
        }
    }
    pub fn root(&self) -> FilesPath {
        FilesPath::new(self.unc_path.as_str())
    }
}

#[cfg(windows)]
pub fn enumerate_wsl_distros() -> Vec<WslDistroInfo> {
    let mut distros = Vec::new();
    let hklm = RegKey::predef(HKEY_CURRENT_USER);
    // WSL distros are listed under HKCU\Software\Microsoft\Windows\CurrentVersion\Lxss
    if let Ok(lxss_key) = hklm.open_subkey("Software\\Microsoft\\Windows\\CurrentVersion\\Lxss") {
        for (subkey_name, subkey_result) in lxss_key.enum_values().filter_map(|r| r.ok()) {
            if let Ok(distros_key) = lxss_key.open_subkey(&subkey_name) {
                let name: String = distros_key
                    .get_value("DistributionName")
                    .ok()
                    .unwrap_or_default();
                let version: u32 = distros_key.get_value::<u32, _>("Version").unwrap_or(0);
                let base_path: String = distros_key.get_value("BasePath").ok().unwrap_or_default();
                if !name.is_empty() {
                    let display = name.clone();
                    let dist = WslDistroInfo::new(name, display, version);
                    distros.push(dist);
                }
            }
        }
    }
    distros
}

#[cfg(not(windows))]
pub fn enumerate_wsl_distros() -> Vec<WslDistroInfo> {
    Vec::new()
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn test_wsl_distro_unc_path() {
        let info = WslDistroInfo::new("Ubuntu".to_string(), "Ubuntu".to_string(), 2);
        assert_eq!(info.unc_path, "\\\\wsl$\\Ubuntu");
        assert_eq!(info.root().as_path().to_string_lossy(), "\\\\wsl$\\Ubuntu");
    }
}

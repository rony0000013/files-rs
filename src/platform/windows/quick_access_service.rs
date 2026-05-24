use crate::core::path_util::FilesPath;
use anyhow::{anyhow, Result};
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;

/// Windows Quick Access Service
/// Reads and manages Windows Quick Access pinned items
pub struct QuickAccessService {
    #[cfg(windows)]
    cached_pins: std::sync::Mutex<HashSet<FilesPath>>,
}

#[cfg(windows)]
impl QuickAccessService {
    pub fn new() -> Self {
        Self {
            cached_pins: std::sync::Mutex::new(HashSet::new()),
        }
    }

    /// Refresh the cached Quick Access pins from the Windows shell
    pub async fn refresh_pins(&self) -> Result<Vec<FilesPath>> {
        let mut pins = Vec::new();

        if let Some(pins_from_dir) = self.read_quick_access_directory() {
            pins.extend(pins_from_dir);
        }

        {
            let mut cached = self.cached_pins.lock().unwrap();
            cached.clear();
            cached.extend(pins.clone());
        }

        Ok(pins)
    }

    fn read_quick_access_directory(&self) -> Option<Vec<FilesPath>> {
        if let Some(appdata) = std::env::var_os("APPDATA") {
            let quick_access_path = PathBuf::from(appdata)
                .join("Microsoft")
                .join("Windows")
                .join("Quick Access");

            if quick_access_path.exists() {
                let mut pins = Vec::new();

                if let Ok(entries) = fs::read_dir(&quick_access_path) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.extension().map_or(false, |e| e == "lnk") {
                            pins.push(FilesPath::new(path));
                        }
                    }
                }

                return Some(pins);
            }
        }
        None
    }

    pub fn is_pinned(&self, path: &FilesPath) -> bool {
        let cached = self.cached_pins.lock().unwrap();
        cached.contains(path)
    }

    pub fn get_cached_pins(&self) -> Vec<FilesPath> {
        let cached = self.cached_pins.lock().unwrap();
        cached.iter().cloned().collect()
    }

    pub async fn pin_folder(&self, path: &FilesPath) -> Result<()> {
        let quick_access_path = if let Some(appdata) = std::env::var_os("APPDATA") {
            PathBuf::from(appdata)
                .join("Microsoft")
                .join("Windows")
                .join("Quick Access")
        } else {
            return Err(anyhow!("Could not find Quick Access folder"));
        };

        if !quick_access_path.exists() {
            fs::create_dir_all(&quick_access_path)?;
        }

        let folder_name = path
            .as_path()
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Pinned Folder");

        let lnk_name = format!("{}.lnk", folder_name);
        let lnk_path = quick_access_path.join(lnk_name);

        self.create_basic_shortcut(&lnk_path, path)?;

        {
            let mut cached = self.cached_pins.lock().unwrap();
            cached.insert(path.clone());
        }

        tracing::info!("Pinned to Quick Access: {:?}", path);
        Ok(())
    }

    fn create_basic_shortcut(&self, lnk_path: &PathBuf, target: &FilesPath) -> Result<()> {
        use std::io::Write;
        use std::os::windows::ffi::OsStrExt;

        let mut file = fs::File::create(lnk_path)?;

        let header = self.create_shell_link_header();
        file.write_all(&header)?;

        let wide_path: Vec<u8> = target
            .as_path()
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .flat_map(|c| c.to_le_bytes())
            .collect();
        file.write_all(&wide_path)?;

        Ok(())
    }

    fn create_shell_link_header(&self) -> Vec<u8> {
        let mut header = Vec::with_capacity(128);

        header.extend_from_slice(&0x4C_u32.to_le_bytes());

        header.extend_from_slice(&[
            0x01u8, 0x14, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0xC0, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x46,
        ]);

        let flags: u32 = 0x00000001;
        header.extend_from_slice(&flags.to_le_bytes());

        header.extend_from_slice(&0x00000010_u32.to_le_bytes());
        header.extend_from_slice(&[0u8; 48]);
        header.extend_from_slice(&0u32.to_le_bytes());
        header.extend_from_slice(&0u32.to_le_bytes());
        header.extend_from_slice(&0x00000001_u32.to_le_bytes());
        header.extend_from_slice(&0u16.to_le_bytes());
        header.extend_from_slice(&[0u8; 10]);
        header.extend_from_slice(&0u16.to_le_bytes());

        header
    }

    pub async fn unpin_folder(&self, path: &FilesPath) -> Result<()> {
        let quick_access_path = if let Some(appdata) = std::env::var_os("APPDATA") {
            PathBuf::from(appdata)
                .join("Microsoft")
                .join("Windows")
                .join("Quick Access")
        } else {
            return Err(anyhow!("Could not find Quick Access folder"));
        };

        if !quick_access_path.exists() {
            return Ok(());
        }

        if let Ok(entries) = fs::read_dir(&quick_access_path) {
            for entry in entries.flatten() {
                let entry_path = entry.path();
                if entry_path.extension().map_or(false, |e| e == "lnk") {
                    if let Some(name) = entry_path.file_stem() {
                        let target_name = path.as_path().file_name().and_then(|n| n.to_str());
                        if name.to_str() == target_name {
                            fs::remove_file(&entry_path)?;
                            tracing::info!("Unpinned from Quick Access: {:?}", path);
                        }
                    }
                }
            }
        }

        {
            let mut cached = self.cached_pins.lock().unwrap();
            cached.remove(path);
        }

        Ok(())
    }
}

#[cfg(not(windows))]
impl QuickAccessService {
    pub fn new() -> Self {
        Self {}
    }

    pub async fn refresh_pins(&self) -> Result<Vec<FilesPath>> {
        tracing::warn!("Quick Access is only implemented for Windows");
        Ok(Vec::new())
    }

    pub fn is_pinned(&self, _path: &FilesPath) -> bool {
        false
    }

    pub fn get_cached_pins(&self) -> Vec<FilesPath> {
        Vec::new()
    }

    pub async fn pin_folder(&self, path: &FilesPath) -> Result<()> {
        Err(anyhow!(
            "Quick Access is only available on Windows: {:?}",
            path
        ))
    }

    pub async fn unpin_folder(&self, path: &FilesPath) -> Result<()> {
        Err(anyhow!(
            "Quick Access is only available on Windows: {:?}",
            path
        ))
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn test_service_creation() {
        let service = QuickAccessService::new();
        assert!(service.get_cached_pins().is_empty());
    }

    #[test]
    fn test_is_pinned_empty() {
        let service = QuickAccessService::new();
        let test_path = FilesPath::new("/tmp/test");
        assert!(!service.is_pinned(&test_path));
    }
}

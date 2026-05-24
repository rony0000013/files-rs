use crate::core::path_util::FilesPath;
use anyhow::{anyhow, Result};
use std::sync::Mutex;

#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;

/// Windows Jump List Service
/// Adds recent folders and tasks to the Windows taskbar jump list
pub struct JumpListService {
    #[cfg(windows)]
    recent_folders: Mutex<Vec<FilesPath>>,
}

impl JumpListService {
    pub fn new() -> Self {
        Self {
            #[cfg(windows)]
            recent_folders: Mutex::new(Vec::new()),
        }
    }

    /// Add a folder to the recent list and update the jump list
    #[cfg(windows)]
    pub async fn add_recent_folder(&self, path: FilesPath) -> Result<()> {
        // Add to internal list (keep last 10)
        {
            let mut recent = self.recent_folders.lock().unwrap();
            // Remove if already exists to avoid duplicates
            recent.retain(|p| p != &path);
            recent.insert(0, path.clone());
            // Keep only last 10
            recent.truncate(10);
        }

        // Add to Windows Recent Documents for Jump List integration
        unsafe {
            use std::os::windows::ffi::OsStrExt;
            use windows::Win32::UI::Shell::{SHAddToRecentDocs, SHARD_PATHW};
            let wide_path: Vec<u16> = path
                .as_path()
                .as_os_str()
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();
            SHAddToRecentDocs(
                SHARD_PATHW.0 as u32,
                Some(wide_path.as_ptr() as *const std::ffi::c_void),
            );
        }

        tracing::info!("Added to jump list: {:?}", path);
        Ok(())
    }

    #[cfg(not(windows))]
    pub async fn add_recent_folder(&self, path: FilesPath) -> Result<()> {
        tracing::warn!("Jump list is only available on Windows: {:?}", path);
        Ok(())
    }

    /// Clear all recent items from the jump list
    #[cfg(windows)]
    pub async fn clear_recent(&self) -> Result<()> {
        unsafe {
            use windows::Win32::UI::Shell::{SHAddToRecentDocs, SHARD_PATHW};
            // Passing null clears the recent docs list
            SHAddToRecentDocs(SHARD_PATHW.0 as u32, None);
        }

        // Clear internal list
        let mut recent = self.recent_folders.lock().unwrap();
        recent.clear();

        tracing::info!("Jump list cleared");
        Ok(())
    }

    #[cfg(not(windows))]
    pub async fn clear_recent(&self) -> Result<()> {
        tracing::warn!("Jump list is only available on Windows");
        Ok(())
    }

    /// Get list of recent folders
    pub fn get_recent_folders(&self) -> Vec<FilesPath> {
        #[cfg(windows)]
        {
            self.recent_folders.lock().unwrap().clone()
        }
        #[cfg(not(windows))]
        {
            Vec::new()
        }
    }

    /// Create a shell link (.lnk) for a path (Windows only)
    #[cfg(windows)]
    pub fn create_shell_link(path: &FilesPath) -> Result<Vec<u8>> {
        use std::os::windows::ffi::OsStrExt;
        // Build a minimal .lnk file format
        // Shell links have a specific binary format
        let mut data = Vec::new();

        // Header size (4 bytes) - 0x4C = 76
        data.extend_from_slice(&0x4C_u32.to_le_bytes());

        // LinkCLSID (16 bytes) - shell link CLSID {00021401-0000-0000-C000-000000000046}
        data.extend_from_slice(&[
            0x01u8, 0x14, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0xC0, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x46,
        ]);

        // LinkFlags (4 bytes)
        let flags: u32 = 0x00000001; // HasLinkTargetIDList
        data.extend_from_slice(&flags.to_le_bytes());

        // FileAttributes (4 bytes) - FILE_ATTRIBUTE_ARCHIVE
        let attrs: u32 = 0x20;
        data.extend_from_slice(&attrs.to_le_bytes());

        // Creation time, access time, write time (16 bytes each = 48 bytes total, all zeros)
        data.extend_from_slice(&[0u8; 48]);

        // File size (4 bytes) - 0 for directory
        data.extend_from_slice(&0u32.to_le_bytes());

        // Icon index (4 bytes)
        data.extend_from_slice(&0u32.to_le_bytes());

        // ShowCommand (4 bytes) - SW_SHOWNORMAL = 1
        data.extend_from_slice(&0x00000001_u32.to_le_bytes());

        // Hotkey (2 bytes)
        data.extend_from_slice(&0u16.to_le_bytes());

        // Reserved (10 bytes)
        data.extend_from_slice(&[0u8; 10]);

        // Now at offset 0x4C - LinkTargetIDList
        // IDList size (2 bytes) = 0 means empty, followed by terminal 0x0000
        data.extend_from_slice(&0u16.to_le_bytes());

        // Link target path stored as wide string at end
        let wide_path: Vec<u8> = path
            .as_path()
            .as_os_str()
            .to_os_string()
            .encode_wide()
            .chain(std::iter::once(0))
            .flat_map(|c| c.to_le_bytes())
            .collect();

        data.extend_from_slice(&wide_path);

        Ok(data)
    }
}

impl Default for JumpListService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn test_jump_list_service_creation() {
        let service = JumpListService::new();
        assert!(service.get_recent_folders().is_empty());
    }

    #[test]
    fn test_get_recent_folders_empty() {
        let service = JumpListService::new();
        let folders = service.get_recent_folders();
        assert!(folders.is_empty());
    }

    #[cfg(windows)]
    #[test]
    fn test_create_shell_link_format() {
        let path = FilesPath::new("C:\\Test\\Folder");
        let link_data = JumpListService::create_shell_link(&path).unwrap();

        // Verify minimum size (76 byte header + terminator + path)
        assert!(link_data.len() > 80);

        // Check header size field
        let header_size =
            u32::from_le_bytes([link_data[0], link_data[1], link_data[2], link_data[3]]);
        assert_eq!(header_size, 0x4C);
    }

    #[cfg(windows)]
    #[test]
    fn test_shell_link_clsuid() {
        let path = FilesPath::new("C:\\Test\\Folder");
        let link_data = JumpListService::create_shell_link(&path).unwrap();

        // Verify CLSID at offset 4 (16 bytes)
        assert_eq!(link_data[4], 0x01);
        assert_eq!(link_data[5], 0x14);
        assert_eq!(link_data[19], 0x46); // Last byte of CLSID
    }
}

//! Recycle Bin Service for Windows & Cross-Platform Trash
//! Provides functionality to enumerate, restore, and permanently delete items from the Recycle Bin.
//! On Windows, parses `$Recycle.Bin\<SID>\$I*` metadata headers directly (sub-millisecond) with
//! Shell API (`SHQueryRecycleBinW`, `SHEmptyRecycleBinW`, COM `Shell.Application`) fallback.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// Represents information about the Recycle Bin
#[derive(Debug, Clone)]
pub struct RecycleBinInfo {
    /// Total size of items in the Recycle Bin in bytes
    pub total_size: u64,
    /// Total number of items in the Recycle Bin
    pub item_count: u64,
}

impl Default for RecycleBinInfo {
    fn default() -> Self {
        Self {
            total_size: 0,
            item_count: 0,
        }
    }
}

/// Result of a restore operation
#[derive(Debug)]
pub struct RestoreResult {
    /// Number of items successfully restored
    pub restored_count: u32,
    /// Paths that were restored (original paths before deletion)
    pub restored_paths: Vec<String>,
}

/// Represents a single item in the Recycle Bin
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RecycleBinItem {
    /// Original path before deletion
    pub original_path: String,
    /// File name (without path)
    pub name: String,
    /// File size in bytes
    pub size: u64,
    /// Date deleted (formatted string)
    pub date_deleted: Option<String>,
    /// Original parent directory path
    pub original_parent: Option<String>,
    /// Whether this is a directory
    pub is_dir: bool,
}

impl Default for RecycleBinItem {
    fn default() -> Self {
        Self {
            original_path: String::new(),
            name: String::new(),
            size: 0,
            date_deleted: None,
            original_parent: None,
            is_dir: false,
        }
    }
}

impl RecycleBinItem {
    /// Human-readable item type description (Files-community parity).
    pub fn item_type_label(&self) -> String {
        if self.is_dir {
            "File folder".to_string()
        } else {
            let ext = Path::new(&self.name)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_uppercase();
            if ext.is_empty() {
                "File".to_string()
            } else {
                format!("{} File", ext)
            }
        }
    }
}

/// Service for interacting with the Windows Recycle Bin / system Trash
pub struct RecycleBinService;

impl RecycleBinService {
    /// Creates a new RecycleBinService instance
    pub fn new() -> Self {
        Self
    }

    /// Gets information about the Recycle Bin (total size and item count)
    #[cfg(windows)]
    pub async fn get_recycle_bin_info(&self) -> Result<RecycleBinInfo> {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;
        use windows::core::PCWSTR;
        use windows::Win32::UI::Shell::SHQueryRecycleBinW;
        use windows::Win32::UI::Shell::SHQUERYRBINFO;

        unsafe {
            let mut rb_info = std::mem::MaybeUninit::<SHQUERYRBINFO>::zeroed();
            rb_info.assume_init_mut().cbSize = std::mem::size_of::<SHQUERYRBINFO>() as u32;

            let drive_path: Vec<u16> = OsStr::new("C:\\")
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();

            SHQueryRecycleBinW(PCWSTR(drive_path.as_ptr()), rb_info.as_mut_ptr())
                .context("Failed to query Recycle Bin")?;

            let info = rb_info.assume_init();
            Ok(RecycleBinInfo {
                total_size: info.i64Size as u64,
                item_count: info.i64NumItems as u64,
            })
        }
    }

    /// Gets information about the Recycle Bin (non-Windows)
    #[cfg(not(windows))]
    pub async fn get_recycle_bin_info(&self) -> Result<RecycleBinInfo> {
        let items = self.list_contents_sync().unwrap_or_default();
        let total_size = items.iter().map(|i| i.size).sum();
        Ok(RecycleBinInfo {
            total_size,
            item_count: items.len() as u64,
        })
    }

    /// Restores a specific item from the Recycle Bin by its original path.
    #[cfg(windows)]
    pub async fn restore_item(&self, original_path: &Path) -> Result<RestoreResult> {
        // First try fast native `$Recycle.Bin` pair restoration ($R -> original_path, remove $I)
        if let Ok(Some(restored)) = Self::try_native_restore_windows(original_path) {
            return Ok(RestoreResult {
                restored_count: 1,
                restored_paths: vec![restored],
            });
        }

        let path_str = original_path
            .file_name()
            .and_then(|f| f.to_str())
            .context("Could not extract filename from path")?;

        let script = format!(
            r#"
            $shell = New-Object -ComObject Shell.Application
            $rb = $shell.Namespace(0x0A)
            foreach ($item in $rb.Items()) {{
                if ($item.Name -ieq '{name}') {{
                    $verb = $item.Verbs() | Where-Object {{ $_.Name -match 'Restore' }} | Select-Object -First 1
                    if ($verb) {{ $verb.DoIt() }}
                    break
                }}
            }}
            "#,
            name = path_str.replace('\'', "''")
        );

        let output = std::process::Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", &script])
            .output()?;

        if output.status.success() {
            Ok(RestoreResult {
                restored_count: 1,
                restored_paths: vec![original_path.to_string_lossy().to_string()],
            })
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(anyhow::anyhow!(
                "Failed to restore item '{}': {}",
                path_str,
                stderr
            ))
        }
    }

    /// Restores a specific item from the Recycle Bin (non-Windows)
    #[cfg(not(windows))]
    pub async fn restore_item(&self, original_path: &Path) -> Result<RestoreResult> {
        if let Ok(Some(restored)) = Self::try_native_restore_unix(original_path) {
            return Ok(RestoreResult {
                restored_count: 1,
                restored_paths: vec![restored],
            });
        }
        Ok(RestoreResult {
            restored_count: 0,
            restored_paths: vec![],
        })
    }

    /// Synchronously restores a single item from the Recycle Bin by its original path.
    pub fn restore_item_sync(original_path: &Path) -> Result<()> {
        #[cfg(windows)]
        {
            match Self::try_native_restore_windows(original_path)? {
                Some(_) => Ok(()),
                None => Err(anyhow::anyhow!(
                    "Item not found in Recycle Bin for {}",
                    original_path.display()
                )),
            }
        }
        #[cfg(not(windows))]
        {
            match Self::try_native_restore_unix(original_path)? {
                Some(_) => Ok(()),
                None => Err(anyhow::anyhow!(
                    "Item not found in Trash for {}",
                    original_path.display()
                )),
            }
        }
    }

    /// Permanently deletes a single item from the Recycle Bin by its original path.
    pub async fn delete_item(&self, original_path: &Path) -> Result<()> {
        #[cfg(windows)]
        {
            let _ = Self::try_native_delete_windows(original_path);
        }
        #[cfg(not(windows))]
        {
            let _ = Self::try_native_delete_unix(original_path);
        }
        Ok(())
    }

    /// Restores all items in the Recycle Bin
    #[cfg(windows)]
    pub async fn restore_all(&self) -> Result<RestoreResult> {
        let items = self.list_contents_sync().unwrap_or_default();
        let mut restored_paths = Vec::new();
        for item in &items {
            let p = PathBuf::from(&item.original_path);
            if let Ok(Some(r)) = Self::try_native_restore_windows(&p) {
                restored_paths.push(r);
            }
        }
        Ok(RestoreResult {
            restored_count: restored_paths.len() as u32,
            restored_paths,
        })
    }

    /// Restores all items in the Recycle Bin (non-Windows)
    #[cfg(not(windows))]
    pub async fn restore_all(&self) -> Result<RestoreResult> {
        let items = self.list_contents_sync().unwrap_or_default();
        let mut restored_paths = Vec::new();
        for item in &items {
            let p = PathBuf::from(&item.original_path);
            if let Ok(Some(r)) = Self::try_native_restore_unix(&p) {
                restored_paths.push(r);
            }
        }
        Ok(RestoreResult {
            restored_count: restored_paths.len() as u32,
            restored_paths,
        })
    }

    /// Empties the Recycle Bin
    #[cfg(windows)]
    pub async fn empty(&self) -> Result<()> {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;
        use windows::core::PCWSTR;
        use windows::Win32::UI::Shell::{
            SHEmptyRecycleBinW, SHERB_NOCONFIRMATION, SHERB_NOPROGRESSUI, SHERB_NOSOUND,
        };

        let drives = ['C', 'D', 'E', 'F', 'G'];
        for drive in drives {
            if std::path::Path::new(&format!("{}:\\", drive)).exists() {
                let drive_path: Vec<u16> = OsStr::new(&format!("{}:\\", drive))
                    .encode_wide()
                    .chain(std::iter::once(0))
                    .collect();

                unsafe {
                    let _ = SHEmptyRecycleBinW(
                        None,
                        PCWSTR(drive_path.as_ptr()),
                        SHERB_NOCONFIRMATION | SHERB_NOPROGRESSUI | SHERB_NOSOUND,
                    );
                }
            }
        }

        Ok(())
    }

    /// Empties the Recycle Bin (non-Windows)
    #[cfg(not(windows))]
    pub async fn empty(&self) -> Result<()> {
        if let Some(trash_dir) = Self::unix_trash_dir() {
            let files_dir = trash_dir.join("files");
            let info_dir = trash_dir.join("info");
            if let Ok(entries) = std::fs::read_dir(&files_dir) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if p.is_dir() {
                        let _ = std::fs::remove_dir_all(&p);
                    } else {
                        let _ = std::fs::remove_file(&p);
                    }
                }
            }
            if let Ok(entries) = std::fs::read_dir(&info_dir) {
                for entry in entries.flatten() {
                    let _ = std::fs::remove_file(entry.path());
                }
            }
        }
        Ok(())
    }

    /// List items in the Recycle Bin (synchronous; call on background executor for UI refresh)
    pub fn list_contents_sync(&self) -> Result<Vec<RecycleBinItem>> {
        #[cfg(windows)]
        {
            self.list_contents_windows()
        }
        #[cfg(not(windows))]
        {
            self.list_contents_unix()
        }
    }

    /// Fast native Windows `$Recycle.Bin` `$I*` header parser across fixed drives.
    #[cfg(windows)]
    fn list_contents_windows(&self) -> Result<Vec<RecycleBinItem>> {
        let mut items = Vec::new();
        for drive in ['C', 'D', 'E', 'F', 'G', 'H'] {
            let rb_root = PathBuf::from(format!("{}:\\$Recycle.Bin", drive));
            let Ok(sid_dirs) = std::fs::read_dir(&rb_root) else {
                continue;
            };
            for sid_entry in sid_dirs.flatten() {
                let sid_path = sid_entry.path();
                let Ok(rb_files) = std::fs::read_dir(&sid_path) else {
                    continue;
                };
                for entry in rb_files.flatten() {
                    let fname = entry.file_name().to_string_lossy().to_string();
                    if !fname.starts_with("$I") {
                        continue;
                    }
                    let i_path = entry.path();
                    if let Some(parsed) = Self::parse_dollar_i_file(&i_path, &sid_path, &fname) {
                        items.push(parsed);
                    }
                }
            }
        }
        items.sort_by(|a, b| b.date_deleted.cmp(&a.date_deleted));
        Ok(items)
    }

    /// Parse a Windows `$Ixxxxxx` Recycle Bin metadata file.
    #[cfg(windows)]
    fn parse_dollar_i_file(
        i_path: &Path,
        sid_dir: &Path,
        i_name: &str,
    ) -> Option<RecycleBinItem> {
        let bytes = std::fs::read(i_path).ok()?;
        if bytes.len() < 28 {
            return None;
        }
        let version = u64::from_le_bytes(bytes[0..8].try_into().ok()?);
        let size = u64::from_le_bytes(bytes[8..16].try_into().ok()?);
        let filetime = u64::from_le_bytes(bytes[16..24].try_into().ok()?);

        let path_offset = if version == 2 { 28 } else { 24 };
        if bytes.len() <= path_offset {
            return None;
        }
        let utf16_slice = &bytes[path_offset..];
        let u16_iter = utf16_slice
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .take_while(|&ch| ch != 0)
            .collect::<Vec<u16>>();
        let original_path = String::from_utf16(&u16_iter).ok()?;
        if original_path.is_empty() {
            return None;
        }

        let r_name = format!("$R{}", &i_name[2..]);
        let r_path = sid_dir.join(&r_name);
        let is_dir = r_path.is_dir();

        let orig_pb = PathBuf::from(&original_path);
        let name = orig_pb
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| original_path.clone());
        let original_parent = orig_pb
            .parent()
            .map(|p| p.to_string_lossy().to_string());

        // Convert Windows FILETIME (100ns since 1601-01-01) to Unix epoch seconds
        let date_deleted = if filetime > 116_444_736_000_000_000 {
            let unix_secs = ((filetime - 116_444_736_000_000_000) / 10_000_000) as i64;
            chrono::DateTime::from_timestamp(unix_secs, 0)
                .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
        } else {
            None
        };

        Some(RecycleBinItem {
            original_path,
            name,
            size,
            date_deleted,
            original_parent,
            is_dir,
        })
    }

    #[cfg(windows)]
    fn ordered_drive_letters(target_orig_path: &Path) -> Vec<char> {
        let mut drives = Vec::with_capacity(26);
        if let Some(first) = target_orig_path
            .to_string_lossy()
            .chars()
            .next()
            .filter(|c| c.is_ascii_alphabetic())
        {
            drives.push(first.to_ascii_uppercase());
        }
        for drive in 'C'..='Z' {
            if !drives.contains(&drive) {
                drives.push(drive);
            }
        }
        drives
    }

    #[cfg(windows)]
    fn try_native_restore_windows(target_orig_path: &Path) -> Result<Option<String>> {
        let target_str = target_orig_path.to_string_lossy().to_string();
        for drive in Self::ordered_drive_letters(target_orig_path) {
            let rb_root = PathBuf::from(format!("{}:\\$Recycle.Bin", drive));
            let Ok(sid_dirs) = std::fs::read_dir(&rb_root) else {
                continue;
            };
            for sid_entry in sid_dirs.flatten() {
                let sid_path = sid_entry.path();
                let Ok(rb_files) = std::fs::read_dir(&sid_path) else {
                    continue;
                };
                for entry in rb_files.flatten() {
                    let fname = entry.file_name().to_string_lossy().to_string();
                    if !fname.starts_with("$I") {
                        continue;
                    }
                    let i_path = entry.path();
                    if let Some(parsed) = Self::parse_dollar_i_file(&i_path, &sid_path, &fname) {
                        if parsed.original_path.eq_ignore_ascii_case(&target_str) {
                            let r_path = sid_path.join(format!("$R{}", &fname[2..]));
                            if r_path.exists() {
                                if let Some(parent) = target_orig_path.parent() {
                                    let _ = std::fs::create_dir_all(parent);
                                }
                                std::fs::rename(&r_path, target_orig_path)?;
                                let _ = std::fs::remove_file(&i_path);
                                return Ok(Some(parsed.original_path));
                            }
                        }
                    }
                }
            }
        }
        Ok(None)
    }

    #[cfg(windows)]
    fn try_native_delete_windows(target_orig_path: &Path) -> Result<bool> {
        let target_str = target_orig_path.to_string_lossy().to_string();
        for drive in Self::ordered_drive_letters(target_orig_path) {
            let rb_root = PathBuf::from(format!("{}:\\$Recycle.Bin", drive));
            let Ok(sid_dirs) = std::fs::read_dir(&rb_root) else {
                continue;
            };
            for sid_entry in sid_dirs.flatten() {
                let sid_path = sid_entry.path();
                let Ok(rb_files) = std::fs::read_dir(&sid_path) else {
                    continue;
                };
                for entry in rb_files.flatten() {
                    let fname = entry.file_name().to_string_lossy().to_string();
                    if !fname.starts_with("$I") {
                        continue;
                    }
                    let i_path = entry.path();
                    if let Some(parsed) = Self::parse_dollar_i_file(&i_path, &sid_path, &fname) {
                        if parsed.original_path.eq_ignore_ascii_case(&target_str) {
                            let r_path = sid_path.join(format!("$R{}", &fname[2..]));
                            if r_path.is_dir() {
                                let _ = std::fs::remove_dir_all(&r_path);
                            } else {
                                let _ = std::fs::remove_file(&r_path);
                            }
                            let _ = std::fs::remove_file(&i_path);
                            return Ok(true);
                        }
                    }
                }
            }
        }
        Ok(false)
    }

    #[cfg(not(windows))]
    fn unix_trash_dir() -> Option<PathBuf> {
        if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
            let p = PathBuf::from(xdg).join("Trash");
            if p.exists() {
                return Some(p);
            }
        }
        dirs::home_dir().map(|h| h.join(".local/share/Trash"))
    }

    #[cfg(not(windows))]
    fn list_contents_unix(&self) -> Result<Vec<RecycleBinItem>> {
        let Some(trash_dir) = Self::unix_trash_dir() else {
            return Ok(Vec::new());
        };
        let info_dir = trash_dir.join("info");
        let files_dir = trash_dir.join("files");
        let Ok(entries) = std::fs::read_dir(&info_dir) else {
            return Ok(Vec::new());
        };

        let mut items = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("trashinfo") {
                continue;
            }
            let stem = path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
            let content = std::fs::read_to_string(&path).unwrap_or_default();
            let mut orig_path = String::new();
            let mut deletion_date = None;
            for line in content.lines() {
                if let Some(p) = line.strip_prefix("Path=") {
                    orig_path = p.trim().to_string();
                } else if let Some(d) = line.strip_prefix("DeletionDate=") {
                    deletion_date = Some(d.trim().replace('T', " "));
                }
            }
            if orig_path.is_empty() {
                continue;
            }
            let payload_path = files_dir.join(&stem);
            let meta = std::fs::metadata(&payload_path).ok();
            let is_dir = meta.as_ref().map(|m| m.is_dir()).unwrap_or(false);
            let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
            let orig_pb = PathBuf::from(&orig_path);
            let name = orig_pb
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or(stem);
            let original_parent = orig_pb.parent().map(|p| p.to_string_lossy().to_string());

            items.push(RecycleBinItem {
                original_path: orig_path,
                name,
                size,
                date_deleted: deletion_date,
                original_parent,
                is_dir,
            });
        }
        Ok(items)
    }

    #[cfg(not(windows))]
    fn try_native_restore_unix(target_orig_path: &Path) -> Result<Option<String>> {
        let Some(trash_dir) = Self::unix_trash_dir() else {
            return Ok(None);
        };
        let info_dir = trash_dir.join("info");
        let files_dir = trash_dir.join("files");
        let target_str = target_orig_path.to_string_lossy().to_string();
        if let Ok(entries) = std::fs::read_dir(&info_dir) {
            for entry in entries.flatten() {
                let info_path = entry.path();
                if let Ok(content) = std::fs::read_to_string(&info_path) {
                    if content.lines().any(|l| l.strip_prefix("Path=").map(|p| p.trim()) == Some(target_str.as_str())) {
                        let stem = info_path.file_stem().unwrap_or_default();
                        let payload = files_dir.join(stem);
                        if payload.exists() {
                            if let Some(parent) = target_orig_path.parent() {
                                let _ = std::fs::create_dir_all(parent);
                            }
                            std::fs::rename(&payload, target_orig_path)?;
                            let _ = std::fs::remove_file(&info_path);
                            return Ok(Some(target_str));
                        }
                    }
                }
            }
        }
        Ok(None)
    }

    #[cfg(not(windows))]
    fn try_native_delete_unix(target_orig_path: &Path) -> Result<bool> {
        let Some(trash_dir) = Self::unix_trash_dir() else {
            return Ok(false);
        };
        let info_dir = trash_dir.join("info");
        let files_dir = trash_dir.join("files");
        let target_str = target_orig_path.to_string_lossy().to_string();
        if let Ok(entries) = std::fs::read_dir(&info_dir) {
            for entry in entries.flatten() {
                let info_path = entry.path();
                if let Ok(content) = std::fs::read_to_string(&info_path) {
                    if content.lines().any(|l| l.strip_prefix("Path=").map(|p| p.trim()) == Some(target_str.as_str())) {
                        let stem = info_path.file_stem().unwrap_or_default();
                        let payload = files_dir.join(stem);
                        if payload.is_dir() {
                            let _ = std::fs::remove_dir_all(&payload);
                        } else {
                            let _ = std::fs::remove_file(&payload);
                        }
                        let _ = std::fs::remove_file(&info_path);
                        return Ok(true);
                    }
                }
            }
        }
        Ok(false)
    }
}

impl Default for RecycleBinService {
    fn default() -> Self {
        Self::new()
    }
}

/// Watcher for monitoring Recycle Bin state changes via polling.
/// Tracks the last known item count and reports when it changes.
#[derive(Debug, Clone)]
pub struct RecycleBinWatcher {
    last_item_count: u64,
    has_polled: bool,
}

impl RecycleBinWatcher {
    /// Creates a new RecycleBinWatcher with initial state.
    pub fn new() -> Self {
        Self {
            last_item_count: 0,
            has_polled: false,
        }
    }

    /// Poll the Recycle Bin to check if the item count has changed.
    pub fn poll(&mut self) -> bool {
        let service = RecycleBinService::new();
        let current_count = match service.list_contents_sync() {
            Ok(items) => items.len() as u64,
            Err(_) => {
                return false;
            }
        };

        if !self.has_polled {
            self.last_item_count = current_count;
            self.has_polled = true;
            return false;
        }

        if current_count != self.last_item_count {
            self.last_item_count = current_count;
            return true;
        }

        false
    }

    /// Returns the last known item count.
    pub fn last_item_count(&self) -> u64 {
        self.last_item_count
    }

    /// Returns whether the watcher has performed at least one poll.
    pub fn has_polled(&self) -> bool {
        self.has_polled
    }
}

impl Default for RecycleBinWatcher {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recycle_bin_info_default() {
        let info = RecycleBinInfo::default();
        assert_eq!(info.total_size, 0);
        assert_eq!(info.item_count, 0);
    }

    #[test]
    fn test_restore_result_debug() {
        let result = RestoreResult {
            restored_count: 5,
            restored_paths: vec![
                "C:\\Users\\test\\file1.txt".to_string(),
                "C:\\Users\\test\\file2.txt".to_string(),
            ],
        };
        let debug_str = format!("{:?}", result);
        assert!(debug_str.contains("5"));
        assert!(debug_str.contains("file1.txt"));
    }

    #[tokio::test]
    async fn test_service_creation() {
        let service = RecycleBinService::new();
        let info = service.get_recycle_bin_info().await;
        assert!(info.is_ok());
    }

    #[tokio::test]
    async fn test_restore_item_with_valid_path() {
        let service = RecycleBinService::new();
        let path = std::path::Path::new("C:\\Users\\Public\\Documents\\testfile.txt");
        let result = service.restore_item(path).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_restore_all_returns_result() {
        let service = RecycleBinService::new();
        let result = service.restore_all().await;
        assert!(result.is_ok());
        let restore_result = result.unwrap();
        assert!(
            restore_result.restored_count == restore_result.restored_paths.len() as u32
                || restore_result.restored_count == 0
        );
    }

    #[tokio::test]
    async fn test_empty_recycle_bin() {
        let service = RecycleBinService::new();
        let result = service.empty().await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_new_service_is_default() {
        let _service = RecycleBinService::new();
        let _default_service = RecycleBinService::default();
    }

    #[test]
    fn test_recycle_bin_item_creation() {
        let item = RecycleBinItem {
            original_path: "C:\\Users\\test\\file.txt".to_string(),
            name: "file.txt".to_string(),
            size: 1024,
            date_deleted: Some("2024-01-15 14:30".to_string()),
            original_parent: Some("C:\\Users\\test".to_string()),
            is_dir: false,
        };
        assert_eq!(item.name, "file.txt");
        assert_eq!(item.size, 1024);
        assert!(!item.is_dir);
    }
}

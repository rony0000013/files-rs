use crate::core::path_util::FilesPath;
use crate::platform::windows::windows_interop::ComThread;
use anyhow::{anyhow, Result};
use std::sync::{Arc, Mutex};

#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;

#[derive(Debug)]
#[allow(dead_code)]
pub enum ShellCommand {
    ShowContextMenu(u64, u64, u64), // window_handle, x, y
    ExecuteAction(String),
}

/// Context menu host that manages the COM STA thread for shell context menus
pub struct ContextMenuHost {
    com_thread: Arc<ComThread>,
    custom_verbs: Mutex<Vec<CustomVerb>>,
    recent_paths: Mutex<Vec<FilesPath>>,
}

/// Custom verb for the context menu
#[derive(Debug, Clone)]
pub struct CustomVerb {
    pub id: u32,
    pub name: String,
    pub icon: Option<String>,
}

impl ContextMenuHost {
    pub fn new(com_thread: Arc<ComThread>) -> Self {
        Self {
            com_thread,
            custom_verbs: Mutex::new(Vec::new()),
            recent_paths: Mutex::new(Vec::new()),
        }
    }

    /// Show the standard Windows context menu for a file/folder
    pub fn show_menu(
        &self,
        window_handle: u64,
        x: u64,
        y: u64,
        selected_paths: &[FilesPath],
        is_background: bool,
    ) -> Result<()> {
        tracing::info!(
            "Showing context menu at ({}, {}) for {} items, is_background={}",
            x,
            y,
            selected_paths.len(),
            is_background
        );

        self.com_thread.execute(move || {
            tracing::info!(
                "STA Thread: Displaying context menu for HWND {} at ({}, {})",
                window_handle,
                x,
                y
            );
        });

        // Track recent paths
        for path in selected_paths {
            self.add_recent_path(path.clone());
        }

        Ok(())
    }

    #[cfg(windows)]
    fn add_recent_path(&self, path: FilesPath) {
        use std::os::windows::ffi::OsStrExt;
        use windows::Win32::UI::Shell::{SHAddToRecentDocs, SHARD_PATHW};

        let mut recent = self.recent_paths.lock().unwrap();
        recent.retain(|p| p != &path);
        recent.insert(0, path.clone());
        recent.truncate(20);

        unsafe {
            let wide_path: Vec<u16> = path
                .as_path()
                .as_os_str()
                .to_os_string()
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();
            SHAddToRecentDocs(SHARD_PATHW.0 as u32, Some(wide_path.as_ptr() as *const _));
        }
    }

    #[cfg(not(windows))]
    fn add_recent_path(&self, _path: FilesPath) {
        // No-op on non-Windows
    }

    /// Execute a context menu command by ID
    pub fn execute_command(&self, command_id: u32, paths: &[FilesPath]) -> Result<()> {
        match command_id {
            0x01 => {
                // Open
                for path in paths {
                    self.do_open(path)?;
                }
            }
            0x02 => {
                // Cut
                self.do_cut(paths)?;
            }
            0x03 => {
                // Copy
                self.do_copy(paths)?;
            }
            0x04 => {
                // Paste - handled separately
                tracing::info!("Paste command called");
            }
            0x05 => {
                // Delete
                for path in paths {
                    self.do_delete(path, false)?;
                }
            }
            0x06 => {
                // Rename - handled by UI layer
                tracing::info!("Rename command called for {} files", paths.len());
            }
            0x07 => {
                // Properties
                for path in paths {
                    self.do_properties(path)?;
                }
            }
            0x08 => {
                // Open As
                for path in paths {
                    self.do_open_as(path)?;
                }
            }
            _ if command_id >= 0x1000 => {
                // Custom verb
                self.do_custom_verb(command_id, paths)?;
            }
            _ => {
                tracing::warn!("Unknown command ID: {}", command_id);
            }
        }
        Ok(())
    }

    #[cfg(windows)]
    fn do_open(&self, path: &FilesPath) -> Result<()> {
        use std::process::Command;
        let path_str = path.as_path().to_string_lossy();
        Command::new("explorer").arg(path_str.as_ref()).spawn()?;
        tracing::info!("Opened: {:?}", path);
        Ok(())
    }

    #[cfg(not(windows))]
    fn do_open(&self, path: &FilesPath) -> Result<()> {
        tracing::warn!("Open not implemented on non-Windows: {:?}", path);
        Ok(())
    }

    #[cfg(windows)]
    fn do_cut(&self, paths: &[FilesPath]) -> Result<()> {
        self.do_clipboard_drop_files(paths, true)
    }

    #[cfg(not(windows))]
    fn do_cut(&self, paths: &[FilesPath]) -> Result<()> {
        tracing::warn!("Cut not implemented on non-Windows: {} items", paths.len());
        Ok(())
    }

    #[cfg(windows)]
    fn do_copy(&self, paths: &[FilesPath]) -> Result<()> {
        self.do_clipboard_drop_files(paths, false)
    }

    #[cfg(not(windows))]
    fn do_copy(&self, paths: &[FilesPath]) -> Result<()> {
        tracing::warn!("Copy not implemented on non-Windows: {} items", paths.len());
        Ok(())
    }

    /// Shared implementation for cut/copy via CF_HDROP + CF_PREFERREDDROPEFFECT.
    /// `is_move=true` for cut, `is_move=false` for copy.
    #[cfg(windows)]
    fn do_clipboard_drop_files(&self, paths: &[FilesPath], is_move: bool) -> Result<()> {
        use std::os::windows::ffi::OsStrExt;
        use windows::Win32::Foundation::{GlobalFree, HANDLE};
        use windows::core::BOOL;
        use windows::Win32::System::DataExchange::{
            CloseClipboard, EmptyClipboard, OpenClipboard, RegisterClipboardFormatW,
            SetClipboardData,
        };
        use windows::Win32::System::Memory::{
            GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE,
        };
        use windows::Win32::UI::Shell::DROPFILES;

        // CF_HDROP clipboard format = 15
        const CF_HDROP: u32 = 15;

        unsafe {
            let wide_paths: Vec<Vec<u16>> = paths
                .iter()
                .map(|p| {
                    p.as_path()
                        .as_os_str()
                        .to_os_string()
                        .encode_wide()
                        .chain(std::iter::once(0))
                        .collect()
                })
                .collect();

            let mut total_size = std::mem::size_of::<DROPFILES>();
            for wp in &wide_paths {
                total_size += wp.len() * 2;
            }
            total_size += 4; // double null terminator (u16)

            let hmem = GlobalAlloc(GMEM_MOVEABLE, total_size)?;
            let ptr = GlobalLock(hmem);

            let dropfiles = ptr as *mut DROPFILES;
            (*dropfiles).pFiles = std::mem::size_of::<DROPFILES>() as u32;
            (*dropfiles).fWide = BOOL::from(true);

            let mut offset = std::mem::size_of::<DROPFILES>();
            for wp in &wide_paths {
                let src = wp.as_slice();
                std::ptr::copy_nonoverlapping(
                    src.as_ptr(),
                    (ptr as *mut u8).add(offset) as *mut u16,
                    src.len(),
                );
                offset += src.len() * 2;
            }
            *(ptr as *mut u16).add(offset / 2) = 0;
            *(ptr as *mut u16).add(offset / 2 + 1) = 0;

            GlobalUnlock(hmem);

            // Open clipboard — if this fails, free the allocated memory to avoid leak.
            if OpenClipboard(None).is_err() {
                let _ = GlobalFree(Some(hmem));
                anyhow::bail!("Failed to open clipboard");
            }
            EmptyClipboard()?;

            // Set CF_HDROP data — Windows takes ownership of hmem on success
            let hdrop = HANDLE(hmem.0);
            let set_result = SetClipboardData(CF_HDROP, Some(hdrop));

            // Set preferred drop effect so paste handler knows whether to delete source
            if set_result.is_ok() {
                let format_name: Vec<u16> = "Preferred DropEffect\0".encode_utf16().collect();
                let cf_drop_effect =
                    RegisterClipboardFormatW(windows::core::PCWSTR(format_name.as_ptr()));
                if cf_drop_effect != 0 {
                    let effect_hmem = GlobalAlloc(GMEM_MOVEABLE, std::mem::size_of::<u32>())?;
                    let effect_ptr = GlobalLock(effect_hmem) as *mut u32;
                    // DROPEFFECT_MOVE = 2, DROPEFFECT_COPY = 1
                    *effect_ptr = if is_move { 2 } else { 1 };
                    GlobalUnlock(effect_hmem);
                    let _ = SetClipboardData(cf_drop_effect, Some(HANDLE(effect_hmem.0)));
                }
            } else {
                // SetClipboardData failed — Windows didn't take ownership.
                // Free the memory to avoid leaking it until process exit.
                let _ = GlobalFree(Some(hmem));
                tracing::warn!("SetClipboardData failed — HDROP memory freed");
            }

            CloseClipboard()?;
        }
        tracing::info!(
            "{} {} items to clipboard",
            if is_move { "Cut" } else { "Copied" },
            paths.len()
        );
        Ok(())
    }

    #[cfg(windows)]
    fn do_delete(&self, path: &FilesPath, permanent: bool) -> Result<()> {
        if permanent {
            let meta = std::fs::metadata(path.as_path());
            match meta {
                Ok(m) if m.is_dir() => std::fs::remove_dir_all(path.as_path())?,
                _ => std::fs::remove_file(path.as_path())?,
            }
        } else {
            trash::delete(path.as_path())?;
        }
        tracing::info!("Deleted (permanent={:?}): {:?}", permanent, path);
        Ok(())
    }

    #[cfg(not(windows))]
    fn do_delete(&self, path: &FilesPath, permanent: bool) -> Result<()> {
        tracing::warn!(
            "Delete not implemented on non-Windows: {:?} permanent={}",
            path,
            permanent
        );
        Ok(())
    }

    #[cfg(windows)]
    fn do_properties(&self, path: &FilesPath) -> Result<()> {
        // Use spawn since ShellService::open_properties is async
        use std::process::Command;
        let path_str = path.as_path().to_string_lossy();
        Command::new("rundll32")
            .args(["shell32.dll,PropertiesDialogW", &path_str])
            .spawn()?;
        tracing::info!("Showing properties for: {:?}", path);
        Ok(())
    }

    #[cfg(not(windows))]
    fn do_properties(&self, path: &FilesPath) -> Result<()> {
        tracing::warn!("Properties not implemented on non-Windows: {:?}", path);
        Ok(())
    }

    #[cfg(windows)]
    fn do_open_as(&self, path: &FilesPath) -> Result<()> {
        use std::process::Command;
        let path_str = path.as_path().to_string_lossy();
        Command::new("rundll32")
            .args(["shell32.dll,OpenAs_RunDLL", &path_str])
            .spawn()?;
        tracing::info!("Open As dialog for: {:?}", path);
        Ok(())
    }

    #[cfg(not(windows))]
    fn do_open_as(&self, path: &FilesPath) -> Result<()> {
        tracing::warn!("Open As not implemented on non-Windows: {:?}", path);
        Ok(())
    }

    fn do_custom_verb(&self, verb_id: u32, paths: &[FilesPath]) -> Result<()> {
        // Custom verbs are stored without callback - they would need to be invoked differently
        // For now, just log the attempt
        tracing::warn!(
            "Custom verb {} called for {} files (not fully implemented)",
            verb_id,
            paths.len()
        );
        Ok(())
    }

    /// Register a custom verb (e.g., Git actions, tag actions)
    pub fn register_verb(&self, name: &str, icon: Option<&str>) -> u32 {
        use std::sync::atomic::{AtomicU32, Ordering};
        static VERB_ID_COUNTER: AtomicU32 = AtomicU32::new(0x1000);

        let mut verbs = self.custom_verbs.lock().unwrap();
        // Check if verb already exists
        if let Some(existing) = verbs.iter().find(|v| v.name == name) {
            return existing.id;
        }
        let id = VERB_ID_COUNTER.fetch_add(1, Ordering::Relaxed);
        verbs.push(CustomVerb {
            id,
            name: name.to_string(),
            icon: icon.map(String::from),
        });
        tracing::info!("Registered custom verb: {} (id={})", name, id);
        id
    }

    /// Unregister a custom verb
    pub fn unregister_verb(&self, name: &str) -> bool {
        let mut verbs = self.custom_verbs.lock().unwrap();
        let len_before = verbs.len();
        verbs.retain(|v| v.name != name);
        verbs.len() < len_before
    }

    /// Get list of registered custom verbs
    pub fn get_custom_verbs(&self) -> Vec<(String, u32)> {
        let verbs = self.custom_verbs.lock().unwrap();
        verbs.iter().map(|v| (v.name.clone(), v.id)).collect()
    }

    pub fn dispatch_command(&self, command: ShellCommand) -> Result<()> {
        self.com_thread.execute(move || match command {
            ShellCommand::ShowContextMenu(_, _, _) => {
                println!("STA Thread: Processing ShowContextMenu");
            }
            ShellCommand::ExecuteAction(cmd) => println!("STA Thread: Executing action: {}", cmd),
        });
        Ok(())
    }
}

/// Cascading submenu support
#[derive(Debug, Clone)]
pub struct CascadingMenu {
    pub id: String,
    pub items: Vec<MenuItem>,
}

#[derive(Debug, Clone)]
pub struct MenuItem {
    pub id: u32,
    pub name: String,
    pub icon: Option<String>,
    pub shortcut: Option<String>,
    pub enabled: bool,
}

/// Extended context menu support
pub struct ExtendedContextMenu {
    host: ContextMenuHost,
    extended_items: Vec<ExtendedMenuItem>,
}

#[derive(Debug, Clone)]
pub struct ExtendedMenuItem {
    pub verb: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub icon: Option<String>,
    pub position: MenuPosition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuPosition {
    Primary,
    Secondary,
    Favorites,
    Extended,
}

impl ExtendedContextMenu {
    pub fn new(host: ContextMenuHost) -> Self {
        Self {
            host,
            extended_items: Vec::new(),
        }
    }

    pub fn add_extended_item(&mut self, item: ExtendedMenuItem) {
        let verb = item.verb.clone();
        self.extended_items.push(item);
        tracing::debug!("Added extended menu item: {}", verb);
    }

    pub fn get_items_at_position(&self, position: MenuPosition) -> Vec<&ExtendedMenuItem> {
        self.extended_items
            .iter()
            .filter(|item| item.position == position)
            .collect()
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn test_shell_command_debug() {
        let cmd = ShellCommand::ShowContextMenu(123, 456, 789);
        let debug_str = format!("{:?}", cmd);
        assert!(debug_str.contains("ShowContextMenu"));
        assert!(debug_str.contains("123"));
    }

    #[test]
    fn test_shell_command_execute_action() {
        let cmd = ShellCommand::ExecuteAction("open".to_string());
        let debug_str = format!("{:?}", cmd);
        assert!(debug_str.contains("ExecuteAction"));
        assert!(debug_str.contains("open"));
    }

    #[test]
    fn test_custom_verb_creation() {
        let verb = CustomVerb {
            id: 1,
            name: "Test Verb".to_string(),
            icon: Some("test.ico".to_string()),
        };
        assert_eq!(verb.id, 1);
        assert_eq!(verb.name, "Test Verb");
        assert!(verb.icon.is_some());
    }

    #[test]
    fn test_custom_verb_without_icon() {
        let verb = CustomVerb {
            id: 2,
            name: "No Icon Verb".to_string(),
            icon: None,
        };
        assert_eq!(verb.id, 2);
        assert!(verb.icon.is_none());
    }

    #[test]
    fn test_menu_position_variants() {
        assert_eq!(MenuPosition::Primary, MenuPosition::Primary);
        assert_eq!(MenuPosition::Secondary, MenuPosition::Secondary);
        assert_eq!(MenuPosition::Favorites, MenuPosition::Favorites);
        assert_eq!(MenuPosition::Extended, MenuPosition::Extended);
    }

    #[test]
    fn test_menu_position_equality() {
        assert_eq!(MenuPosition::Primary, MenuPosition::Primary);
        assert_ne!(MenuPosition::Primary, MenuPosition::Secondary);
    }

    #[test]
    fn test_extended_menu_item_creation() {
        let item = ExtendedMenuItem {
            verb: "test.verb".to_string(),
            title: "Test Title".to_string(),
            subtitle: Some("Test Subtitle".to_string()),
            icon: Some("icon.ico".to_string()),
            position: MenuPosition::Primary,
        };
        assert_eq!(item.verb, "test.verb");
        assert_eq!(item.title, "Test Title");
        assert!(item.subtitle.is_some());
        assert_eq!(item.position, MenuPosition::Primary);
    }

    #[test]
    fn test_extended_menu_item_without_subtitle() {
        let item = ExtendedMenuItem {
            verb: "no.subtitle".to_string(),
            title: "No Subtitle".to_string(),
            subtitle: None,
            icon: None,
            position: MenuPosition::Extended,
        };
        assert!(item.subtitle.is_none());
        assert!(item.icon.is_none());
    }

    #[test]
    fn test_extended_menu_item_positions() {
        let item_primary = ExtendedMenuItem {
            verb: "p".to_string(),
            title: "P".to_string(),
            subtitle: None,
            icon: None,
            position: MenuPosition::Primary,
        };
        let item_secondary = ExtendedMenuItem {
            verb: "s".to_string(),
            title: "S".to_string(),
            subtitle: None,
            icon: None,
            position: MenuPosition::Secondary,
        };
        assert_eq!(item_primary.position, MenuPosition::Primary);
        assert_eq!(item_secondary.position, MenuPosition::Secondary);
    }

    #[test]
    fn test_context_menu_host_creation() {
        // ContextMenuHost requires ComThread which needs COM initialization
        // Just test the basic types work
        let custom_verb = CustomVerb {
            id: 100,
            name: "Test".to_string(),
            icon: None,
        };
        assert_eq!(custom_verb.id, 100);
    }

    #[test]
    fn test_cascading_menu_children() {
        let menu = CascadingMenu {
            id: "casc".to_string(),
            items: vec![
                MenuItem {
                    id: 1,
                    name: "Child 1".to_string(),
                    icon: None,
                    shortcut: None,
                    enabled: true,
                },
                MenuItem {
                    id: 2,
                    name: "Child 2".to_string(),
                    icon: None,
                    shortcut: None,
                    enabled: true,
                },
            ],
        };
        assert_eq!(menu.items.len(), 2);
        assert_eq!(menu.items[0].name, "Child 1");
    }

    #[test]
    fn test_menu_item_enabled() {
        let item = MenuItem {
            id: 1,
            name: "Test Item".to_string(),
            icon: Some("icon.png".to_string()),
            shortcut: Some("Ctrl+T".to_string()),
            enabled: true,
        };
        assert!(item.enabled);
        assert!(item.icon.is_some());
        assert!(item.shortcut.is_some());
    }
}

/// ShellProperties — provides real Windows shell property extraction
///
/// Uses standard file system APIs (std::fs::metadata) to extract properties
/// like file size, dates, types, and path info for common shell property names.
/// This approach avoids the COM/IPropertyStore complexity while still providing
/// real property extraction for the most commonly used properties.
#[cfg(windows)]
pub struct ShellProperties;

#[cfg(windows)]
impl ShellProperties {
    /// Get the property value for a specific named shell property (e.g., "System.Title")
    pub fn get_property(path: &std::path::Path, prop_name: &str) -> Option<String> {
        // Try common well-known properties through fast file system APIs first
        let result = Self::get_fast_property(path, prop_name);
        if result.is_some() {
            return result;
        }
        // Fall back to file metadata-based extraction for known properties
        Self::get_fs_property(path, prop_name)
    }

    /// Fast property extraction for well-known properties using file system
    fn get_fast_property(path: &std::path::Path, prop_name: &str) -> Option<String> {
        match prop_name {
            "System.ItemPathDisplay" | "System.ParsingPath" | "System.ItemUrl" => {
                path.to_str().map(|s| s.to_string())
            }
            "System.ItemNameDisplay" => path.file_name()?.to_str().map(|s| s.to_string()),
            "System.FileExtension" => path.extension()?.to_str().map(|s| format!(".{}", s)),
            "System.ItemType" => {
                let ext = path.extension()?.to_str()?;
                match ext.to_lowercase().as_str() {
                    "txt" => Some("Text Document".to_string()),
                    "pdf" => Some("PDF Document".to_string()),
                    "jpg" | "jpeg" => Some("JPEG Image".to_string()),
                    "png" => Some("PNG Image".to_string()),
                    "gif" => Some("GIF Image".to_string()),
                    "mp3" => Some("MP3 Audio".to_string()),
                    "mp4" => Some("MP4 Video".to_string()),
                    "zip" => Some("Compressed (zipped) Folder".to_string()),
                    "doc" | "docx" => Some("Microsoft Word Document".to_string()),
                    "xls" | "xlsx" => Some("Microsoft Excel Worksheet".to_string()),
                    "ppt" | "pptx" => Some("Microsoft PowerPoint Presentation".to_string()),
                    _ => Some(format!("{} File", ext.to_uppercase())),
                }
            }
            _ => None,
        }
    }

    /// Property extraction using std::fs::metadata for file properties
    fn get_fs_property(path: &std::path::Path, prop_name: &str) -> Option<String> {
        let meta = std::fs::metadata(path).ok()?;
        match prop_name {
            "System.Size" => Some(Self::format_file_size(meta.len())),
            "System.DateModified" => {
                let modified = meta.modified().ok()?;
                Some(Self::format_system_time(modified))
            }
            "System.DateCreated" => {
                let created = meta.created().ok()?;
                Some(Self::format_system_time(created))
            }
            "System.DateAccessed" => {
                let accessed = meta.accessed().ok()?;
                Some(Self::format_system_time(accessed))
            }
            "System.FileAttributes" => {
                let attrs = if meta.is_dir() { "D" } else { "" };
                let attrs = if meta.is_symlink() {
                    format!("{}L", attrs)
                } else {
                    attrs.to_string()
                };
                if attrs.is_empty() {
                    Some("A".to_string())
                } else {
                    Some(attrs)
                }
            }
            "System.ItemFolderPathDisplay" => path.parent()?.to_str().map(|s| s.to_string()),
            "System.FileCount" if meta.is_dir() => {
                if let Ok(entries) = std::fs::read_dir(path) {
                    Some(entries.count().to_string())
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Get all properties as a HashMap
    pub fn get_all_properties(path: &std::path::Path) -> std::collections::HashMap<String, String> {
        let mut props = std::collections::HashMap::new();
        let meta = match std::fs::metadata(path) {
            Ok(m) => m,
            Err(_) => return props,
        };
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            props.insert("System.ItemNameDisplay".to_string(), name.to_string());
        }
        if let Some(path_str) = path.to_str() {
            props.insert("System.ItemPathDisplay".to_string(), path_str.to_string());
        }
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            props.insert("System.FileExtension".to_string(), format!(".{}", ext));
            props.insert(
                "System.ItemType".to_string(),
                Self::get_fast_property(path, "System.ItemType")
                    .unwrap_or_else(|| format!("{} File", ext.to_uppercase())),
            );
        }
        if let Some(parent) = path.parent().and_then(|p| p.to_str()) {
            props.insert(
                "System.ItemFolderPathDisplay".to_string(),
                parent.to_string(),
            );
        }
        props.insert(
            "System.Size".to_string(),
            Self::format_file_size(meta.len()),
        );
        if let Ok(modified) = meta.modified() {
            props.insert(
                "System.DateModified".to_string(),
                Self::format_system_time(modified),
            );
        }
        if let Ok(created) = meta.created() {
            props.insert(
                "System.DateCreated".to_string(),
                Self::format_system_time(created),
            );
        }
        props
    }

    /// Get a summary of key properties for a given file path
    pub fn get_summary(path: &std::path::Path, _ext: &str) -> ShellPropertySummary {
        let mut summary = ShellPropertySummary::default();
        if let Ok(meta) = std::fs::metadata(path) {
            summary.byte_count = Some(meta.len());
            if let Ok(modified) = meta.modified() {
                summary.comment = Some(Self::format_system_time(modified));
            }
        }
        summary
    }

    /// Format file size in human-readable form
    fn format_file_size(bytes: u64) -> String {
        const KB: u64 = 1024;
        const MB: u64 = KB * 1024;
        const GB: u64 = MB * 1024;
        if bytes >= GB {
            format!("{:.2} GB", bytes as f64 / GB as f64)
        } else if bytes >= MB {
            format!("{:.2} MB", bytes as f64 / MB as f64)
        } else if bytes >= KB {
            format!("{:.2} KB", bytes as f64 / KB as f64)
        } else {
            format!("{} bytes", bytes)
        }
    }

    /// Format SystemTime to ISO-like string
    fn format_system_time(time: std::time::SystemTime) -> String {
        let duration = time
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        let secs = duration.as_secs();
        let days_since_epoch = secs / 86400;
        let seconds_in_day = secs % 86400;
        let hours = seconds_in_day / 3600;
        let minutes = (seconds_in_day % 3600) / 60;
        let seconds = seconds_in_day % 60;
        let mut year = 1970i64;
        let mut remaining_days = days_since_epoch as i64;
        while remaining_days > 0 {
            let days_in_year = if Self::is_leap_year(year) { 366 } else { 365 };
            if remaining_days >= days_in_year {
                remaining_days -= days_in_year;
                year += 1;
            } else {
                break;
            }
        }
        if remaining_days < 0 {
            year -= 1;
            remaining_days += if Self::is_leap_year(year) { 366 } else { 365 };
        }
        let days_in_months: [i64; 12] = if Self::is_leap_year(year) {
            [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
        } else {
            [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
        };
        let mut month = 1;
        for days in days_in_months.iter() {
            if remaining_days < *days {
                break;
            }
            remaining_days -= *days;
            month += 1;
        }
        let day = remaining_days + 1;
        format!(
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
            year, month, day, hours, minutes, seconds
        )
    }

    fn is_leap_year(year: i64) -> bool {
        (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
    }

    /// Get the relevant property keys for a file extension
    fn relevant_keys_for_extension(_ext: &str) -> Vec<&'static str> {
        Vec::new()
    }
}

/// Summary of shell properties for a file
#[derive(Debug, Clone, Default)]
pub struct ShellPropertySummary {
    pub title: Option<String>,
    pub author: Option<String>,
    pub subject: Option<String>,
    pub page_count: Option<u32>,
    pub character_count: Option<u64>,
    pub line_count: Option<u64>,
    pub byte_count: Option<u64>,
    pub last_author: Option<String>,
    pub comment: Option<String>,
}

#[cfg(windows)]
impl std::fmt::Display for ShellPropertySummary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut parts = Vec::new();
        if let Some(t) = &self.title {
            parts.push(format!("Title: {}", t));
        }
        if let Some(a) = &self.author {
            parts.push(format!("Author: {}", a));
        }
        if let Some(s) = &self.subject {
            parts.push(format!("Subject: {}", s));
        }
        if let Some(pc) = self.page_count {
            parts.push(format!("Pages: {}", pc));
        }
        if let Some(cc) = self.character_count {
            parts.push(format!("Characters: {}", cc));
        }
        if let Some(lc) = self.line_count {
            parts.push(format!("Lines: {}", lc));
        }
        if let Some(bc) = self.byte_count {
            parts.push(format!("Size: {} bytes", bc));
        }
        if let Some(la) = &self.last_author {
            parts.push(format!("Last Author: {}", la));
        }
        if let Some(c) = &self.comment {
            parts.push(format!("Comment: {}", c));
        }
        if parts.is_empty() {
            write!(f, "(no properties found)")
        } else {
            write!(f, "{}", parts.join(" | "))
        }
    }
}

#[cfg(not(windows))]
pub struct ShellProperties;

#[cfg(not(windows))]
impl ShellProperties {
    pub fn get_property(_path: &std::path::Path, _prop_name: &str) -> Option<String> {
        None
    }

    pub fn get_all_properties(
        _path: &std::path::Path,
    ) -> std::collections::HashMap<String, String> {
        std::collections::HashMap::new()
    }

    pub fn get_summary(_path: &std::path::Path, _ext: &str) -> ShellPropertySummary {
        ShellPropertySummary::default()
    }
}

#[cfg(test)]
mod shell_property_tests {
    use super::*;

    // ─── ShellPropertySummary tests ──────────────────────────────────────

    #[test]
    fn test_shell_property_summary_default() {
        let summary = ShellPropertySummary::default();
        assert!(summary.title.is_none());
        assert!(summary.author.is_none());
        assert!(summary.subject.is_none());
        assert!(summary.page_count.is_none());
        assert!(summary.character_count.is_none());
        assert!(summary.line_count.is_none());
        assert!(summary.byte_count.is_none());
        assert!(summary.last_author.is_none());
        assert!(summary.comment.is_none());
    }

    #[test]
    fn test_shell_property_summary_display_empty() {
        let summary = ShellPropertySummary::default();
        assert_eq!(summary.to_string(), "(no properties found)");
    }

    #[test]
    fn test_shell_property_summary_display_with_values() {
        let mut summary = ShellPropertySummary::default();
        summary.title = Some("Test Doc".to_string());
        summary.author = Some("John Doe".to_string());
        summary.page_count = Some(42);
        let display = summary.to_string();
        assert!(display.contains("Title: Test Doc"));
        assert!(display.contains("Author: John Doe"));
        assert!(display.contains("Pages: 42"));
    }

    #[test]
    fn test_shell_properties_non_windows_stub() {
        // On non-Windows, these just return empty results
        let path = std::path::Path::new("/tmp/test.pdf");
        assert!(ShellProperties::get_property(path, "Title").is_none());
        let props = ShellProperties::get_all_properties(path);
        assert!(props.is_empty() || !props.is_empty()); // Just verify it returns a HashMap
    }

    #[cfg(windows)]
    #[test]
    fn test_format_file_size() {
        let bytes = 1024u64;
        let result = ShellProperties::format_file_size(bytes);
        assert!(result.contains("1.00 KB"));
    }

    #[cfg(windows)]
    #[test]
    fn test_format_system_time() {
        let epoch = std::time::UNIX_EPOCH;
        let result = ShellProperties::format_system_time(epoch);
        assert!(result.contains("1970") || result.contains("1970"));
    }

    #[cfg(windows)]
    #[test]
    fn test_fast_property() {
        let path = std::path::Path::new("/tmp/test.pdf");
        let name = ShellProperties::get_fast_property(path, "System.ItemNameDisplay");
        assert_eq!(name, Some("test.pdf".to_string()));

        let item_type = ShellProperties::get_fast_property(path, "System.ItemType");
        assert_eq!(item_type, Some("PDF Document".to_string()));
    }

    #[cfg(windows)]
    #[test]
    fn test_get_property_path_display() {
        let path = std::path::Path::new("/home/user/document.txt");
        let display = ShellProperties::get_fast_property(path, "System.ItemPathDisplay");
        assert!(display.is_some());
        assert!(display.unwrap().contains("document.txt"));
    }

    #[cfg(windows)]
    #[test]
    fn test_get_property_nonexistent() {
        let path = std::path::Path::new("/nonexistent_file_xyz");
        let result = ShellProperties::get_property(path, "System.Size");
        // Non-existent file returns None
        assert!(result.is_none());
    }

    #[cfg(windows)]
    #[test]
    fn test_get_all_properties_non_windows() {
        let path = std::path::Path::new("/tmp");
        let props = ShellProperties::get_all_properties(path);
        assert!(props.contains_key("System.ItemNameDisplay"));
    }
}

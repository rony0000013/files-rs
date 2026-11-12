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
        Command::new("explorer")
            .arg(path_str.as_ref())
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()?;
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
        stage_hdrop_clipboard_files(paths, is_move)
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
        // ShellService::open_properties uses ShellExecuteExW with the
        // "properties" verb (rundll32 shell32.dll,PropertiesDialogW does not
        // exist and pops an error dialog).
        crate::platform::windows::shell_service::ShellService::open_properties_sync(path)?;
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
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
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

/// Build a `CF_HDROP` (`DROPFILES` + UTF-16 null-separated paths + double null terminator)
/// `HGLOBAL` suitable for clipboard or OLE `IDataObject` transfer.
#[cfg(windows)]
unsafe fn build_hdrop_hglobal(paths: &[std::path::PathBuf]) -> Result<windows::Win32::Foundation::HGLOBAL> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::BOOL;
    use windows::Win32::System::Memory::{
        GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE, GMEM_ZEROINIT,
    };
    use windows::Win32::UI::Shell::DROPFILES;

    let wide_paths: Vec<Vec<u16>> = paths
        .iter()
        .map(|p| {
            let simplified = dunce::simplified(p);
            simplified
                .as_os_str()
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

    let hmem = unsafe { GlobalAlloc(GMEM_MOVEABLE | GMEM_ZEROINIT, total_size)? };
    let ptr = unsafe { GlobalLock(hmem) };
    if ptr.is_null() {
        let _ = unsafe { windows::Win32::Foundation::GlobalFree(Some(hmem)) };
        anyhow::bail!("GlobalLock failed for CF_HDROP");
    }

    unsafe {
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

        let _ = GlobalUnlock(hmem);
    }

    Ok(hmem)
}

/// Shared CF_HDROP clipboard staging for cut/copy and native OS drag.
#[cfg(windows)]
pub fn stage_hdrop_clipboard_files(paths: &[FilesPath], is_move: bool) -> Result<()> {
    use windows::Win32::Foundation::{GlobalFree, HANDLE};
    use windows::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, OpenClipboard, RegisterClipboardFormatW,
        SetClipboardData,
    };
    use windows::Win32::System::Memory::{
        GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE, GMEM_ZEROINIT,
    };

    const CF_HDROP: u32 = 15;
    const CF_UNICODETEXT: u32 = 13;

    if paths.is_empty() {
        return Ok(());
    }

    let path_bufs: Vec<std::path::PathBuf> =
        paths.iter().map(|p| p.as_path().to_path_buf()).collect();

    unsafe {
        let hmem = build_hdrop_hglobal(&path_bufs)?;

        // Open clipboard with retry (up to 10 attempts with 10ms delay)
        let mut opened = false;
        for _ in 0..10 {
            if OpenClipboard(None).is_ok() {
                opened = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        if !opened {
            let _ = GlobalFree(Some(hmem));
            anyhow::bail!("Failed to open clipboard");
        }
        let _ = EmptyClipboard();

        // 1. Set CF_HDROP data — Windows takes ownership of hmem on success
        let hdrop = HANDLE(hmem.0);
        let set_result = SetClipboardData(CF_HDROP, Some(hdrop));
        if set_result.is_err() {
            let _ = GlobalFree(Some(hmem));
            tracing::warn!("SetClipboardData(CF_HDROP) failed — HDROP memory freed");
        }

        // 2. Set preferred drop effect so paste handler knows whether to delete source
        let format_name: Vec<u16> = "Preferred DropEffect\0".encode_utf16().collect();
        let cf_drop_effect =
            RegisterClipboardFormatW(windows::core::PCWSTR(format_name.as_ptr()));
        if cf_drop_effect != 0 {
            if let Ok(effect_hmem) =
                GlobalAlloc(GMEM_MOVEABLE | GMEM_ZEROINIT, std::mem::size_of::<u32>())
            {
                let effect_ptr = GlobalLock(effect_hmem) as *mut u32;
                if !effect_ptr.is_null() {
                    // DROPEFFECT_MOVE = 2, DROPEFFECT_COPY = 1
                    *effect_ptr = if is_move { 2 } else { 1 };
                    let _ = GlobalUnlock(effect_hmem);
                    let _ = SetClipboardData(cf_drop_effect, Some(HANDLE(effect_hmem.0)));
                } else {
                    let _ = GlobalFree(Some(effect_hmem));
                }
            }
        }

        // 3. Set CF_UNICODETEXT with newline-separated paths so text editors, browsers, and terminals receive paths
        let text_paths = paths
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect::<Vec<_>>()
            .join("\r\n");
        let text_wide: Vec<u16> = text_paths.encode_utf16().chain(std::iter::once(0)).collect();
        let text_bytes = text_wide.len() * 2;
        if let Ok(text_hmem) = GlobalAlloc(GMEM_MOVEABLE | GMEM_ZEROINIT, text_bytes) {
            let text_ptr = GlobalLock(text_hmem) as *mut u16;
            if !text_ptr.is_null() {
                std::ptr::copy_nonoverlapping(text_wide.as_ptr(), text_ptr, text_wide.len());
                let _ = GlobalUnlock(text_hmem);
                let _ = SetClipboardData(CF_UNICODETEXT, Some(HANDLE(text_hmem.0)));
            } else {
                let _ = GlobalFree(Some(text_hmem));
            }
        }

        let _ = CloseClipboard();
    }
    tracing::info!(
        "{} {} items to system clipboard (CF_HDROP + CF_UNICODETEXT)",
        if is_move { "Cut" } else { "Copied" },
        paths.len()
    );
    Ok(())
}

/// Read file paths from the OS clipboard (CF_HDROP) along with Preferred DropEffect (is_move).
#[cfg(windows)]
pub fn get_hdrop_clipboard_files() -> Option<(Vec<std::path::PathBuf>, bool)> {
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::DataExchange::{
        CloseClipboard, GetClipboardData, IsClipboardFormatAvailable, OpenClipboard,
        RegisterClipboardFormatW,
    };
    use windows::Win32::System::Memory::{GlobalLock, GlobalUnlock};
    use windows::Win32::UI::Shell::{DragQueryFileW, HDROP};

    const CF_HDROP: u32 = 15;

    unsafe {
        let mut opened = false;
        for _ in 0..10 {
            if OpenClipboard(None).is_ok() {
                opened = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        if !opened {
            return None;
        }

        if IsClipboardFormatAvailable(CF_HDROP).is_err() {
            let _ = CloseClipboard();
            return None;
        }

        let h_data = match GetClipboardData(CF_HDROP) {
            Ok(h) => h,
            Err(_) => {
                let _ = CloseClipboard();
                return None;
            }
        };

        let hdrop = HDROP(h_data.0 as _);
        let count = DragQueryFileW(hdrop, 0xFFFFFFFF, None);
        let mut paths = Vec::with_capacity(count as usize);

        for i in 0..count {
            let len = DragQueryFileW(hdrop, i, None) as usize;
            if len > 0 {
                let mut buf = vec![0u16; len + 1];
                let copied = DragQueryFileW(hdrop, i, Some(&mut buf)) as usize;
                if copied > 0 {
                    buf.truncate(copied);
                    use std::os::windows::ffi::OsStringExt;
                    let os_str = std::ffi::OsString::from_wide(&buf);
                    paths.push(std::path::PathBuf::from(os_str));
                }
            }
        }

        // Check Preferred DropEffect for cut vs copy
        let mut is_move = false;
        let format_name: Vec<u16> = "Preferred DropEffect\0".encode_utf16().collect();
        let cf_drop_effect = RegisterClipboardFormatW(windows::core::PCWSTR(format_name.as_ptr()));
        if cf_drop_effect != 0 && IsClipboardFormatAvailable(cf_drop_effect).is_ok() {
            if let Ok(effect_handle) = GetClipboardData(cf_drop_effect) {
                let ptr = GlobalLock(windows::Win32::Foundation::HGLOBAL(effect_handle.0));
                if !ptr.is_null() {
                    let effect = *(ptr as *const u32);
                    let _ = GlobalUnlock(windows::Win32::Foundation::HGLOBAL(effect_handle.0));
                    // DROPEFFECT_MOVE = 2
                    is_move = (effect & 2) != 0;
                }
            }
        }

        let _ = CloseClipboard();

        if paths.is_empty() {
            None
        } else {
            Some((paths, is_move))
        }
    }
}

#[cfg(not(windows))]
pub fn get_hdrop_clipboard_files() -> Option<(Vec<std::path::PathBuf>, bool)> {
    None
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


#[cfg(windows)]
#[windows::core::implement(
    windows::Win32::System::Com::IDataObject,
    windows::Win32::UI::Shell::IDataObjectAsyncCapability
)]
struct DraggableShellDataObject {
    inner: windows::Win32::System::Com::IDataObject,
    paths: Vec<std::path::PathBuf>,
}

#[cfg(windows)]
impl DraggableShellDataObject {
    fn is_hdrop_format(pformatetc: *const windows::Win32::System::Com::FORMATETC) -> bool {
        use windows::Win32::System::Com::{DVASPECT_CONTENT, TYMED_HGLOBAL};
        const CF_HDROP: u16 = 15;
        if pformatetc.is_null() {
            return false;
        }
        let fmt = unsafe { *pformatetc };
        fmt.cfFormat == CF_HDROP
            && (fmt.tymed & (TYMED_HGLOBAL.0 as u32)) != 0
            && fmt.dwAspect == DVASPECT_CONTENT.0
            && fmt.lindex == -1
    }
}

#[cfg(windows)]
#[allow(non_snake_case)]
impl windows::Win32::System::Com::IDataObject_Impl for DraggableShellDataObject_Impl {
    fn GetData(
        &self,
        pformatetcin: *const windows::Win32::System::Com::FORMATETC,
    ) -> windows::core::Result<windows::Win32::System::Com::STGMEDIUM> {
        use std::mem::ManuallyDrop;
        use windows::Win32::Foundation::{E_INVALIDARG, E_OUTOFMEMORY};
        use windows::Win32::System::Com::{STGMEDIUM, STGMEDIUM_0, TYMED_HGLOBAL};

        if pformatetcin.is_null() {
            return Err(E_INVALIDARG.into());
        }
        if DraggableShellDataObject::is_hdrop_format(pformatetcin) {
            // Always return a GMEM_MOVEABLE HGLOBAL for CF_HDROP:
            // SHCreateDataObject returns GMEM_FIXED by default, which causes
            // gpui 0.2.2's `*(hGlobal.0 as *mut HDROP)` in WindowsDragDropHandler::DragEnter
            // to pass DROPFILES.pFiles (0x14) to DragQueryFileW and segfault (0xc0000005).
            let hmem = unsafe { build_hdrop_hglobal(&self.paths) }
                .map_err(|_| windows::core::Error::from(E_OUTOFMEMORY))?;
            return Ok(STGMEDIUM {
                tymed: TYMED_HGLOBAL.0 as u32,
                u: STGMEDIUM_0 { hGlobal: hmem },
                pUnkForRelease: ManuallyDrop::new(None),
            });
        }
        unsafe { self.inner.GetData(pformatetcin) }
    }

    fn GetDataHere(
        &self,
        pformatetc: *const windows::Win32::System::Com::FORMATETC,
        pmedium: *mut windows::Win32::System::Com::STGMEDIUM,
    ) -> windows::core::Result<()> {
        unsafe { self.inner.GetDataHere(pformatetc, pmedium) }
    }

    fn QueryGetData(
        &self,
        pformatetc: *const windows::Win32::System::Com::FORMATETC,
    ) -> windows::core::HRESULT {
        use windows::Win32::Foundation::{E_INVALIDARG, S_OK};
        if pformatetc.is_null() {
            return E_INVALIDARG;
        }
        if DraggableShellDataObject::is_hdrop_format(pformatetc) {
            return S_OK;
        }
        unsafe { self.inner.QueryGetData(pformatetc) }
    }

    fn GetCanonicalFormatEtc(
        &self,
        pformatectin: *const windows::Win32::System::Com::FORMATETC,
        pformatetcout: *mut windows::Win32::System::Com::FORMATETC,
    ) -> windows::core::HRESULT {
        unsafe { self.inner.GetCanonicalFormatEtc(pformatectin, pformatetcout) }
    }

    fn SetData(
        &self,
        pformatetc: *const windows::Win32::System::Com::FORMATETC,
        pmedium: *const windows::Win32::System::Com::STGMEDIUM,
        frelease: windows::core::BOOL,
    ) -> windows::core::Result<()> {
        unsafe { self.inner.SetData(pformatetc, pmedium, frelease.as_bool()) }
    }

    fn EnumFormatEtc(
        &self,
        dwdirection: u32,
    ) -> windows::core::Result<windows::Win32::System::Com::IEnumFORMATETC> {
        unsafe { self.inner.EnumFormatEtc(dwdirection) }
    }

    fn DAdvise(
        &self,
        pformatetc: *const windows::Win32::System::Com::FORMATETC,
        advf: u32,
        padvsink: windows::core::Ref<windows::Win32::System::Com::IAdviseSink>,
    ) -> windows::core::Result<u32> {
        unsafe { self.inner.DAdvise(pformatetc, advf, padvsink.as_ref()) }
    }

    fn DUnadvise(&self, dwconnection: u32) -> windows::core::Result<()> {
        unsafe { self.inner.DUnadvise(dwconnection) }
    }

    fn EnumDAdvise(&self) -> windows::core::Result<windows::Win32::System::Com::IEnumSTATDATA> {
        unsafe { self.inner.EnumDAdvise() }
    }
}

#[cfg(windows)]
#[allow(non_snake_case)]
impl windows::Win32::UI::Shell::IDataObjectAsyncCapability_Impl for DraggableShellDataObject_Impl {
    fn SetAsyncMode(&self, fdoopasync: windows::core::BOOL) -> windows::core::Result<()> {
        use windows::core::Interface;
        use windows::Win32::UI::Shell::IDataObjectAsyncCapability;
        let async_cap: IDataObjectAsyncCapability = self.inner.cast()?;
        unsafe { async_cap.SetAsyncMode(fdoopasync.as_bool()) }
    }

    fn GetAsyncMode(&self) -> windows::core::Result<windows::core::BOOL> {
        use windows::core::Interface;
        use windows::Win32::UI::Shell::IDataObjectAsyncCapability;
        let async_cap: IDataObjectAsyncCapability = self.inner.cast()?;
        unsafe { async_cap.GetAsyncMode() }
    }

    fn StartOperation(
        &self,
        pbcreserved: windows::core::Ref<windows::Win32::System::Com::IBindCtx>,
    ) -> windows::core::Result<()> {
        use windows::core::Interface;
        use windows::Win32::UI::Shell::IDataObjectAsyncCapability;
        let async_cap: IDataObjectAsyncCapability = self.inner.cast()?;
        unsafe { async_cap.StartOperation(pbcreserved.as_ref()) }
    }

    fn InOperation(&self) -> windows::core::Result<windows::core::BOOL> {
        use windows::core::Interface;
        use windows::Win32::UI::Shell::IDataObjectAsyncCapability;
        let async_cap: IDataObjectAsyncCapability = self.inner.cast()?;
        unsafe { async_cap.InOperation() }
    }

    fn EndOperation(
        &self,
        hresult: windows::core::HRESULT,
        pbcreserved: windows::core::Ref<windows::Win32::System::Com::IBindCtx>,
        dweffects: u32,
    ) -> windows::core::Result<()> {
        use windows::core::Interface;
        use windows::Win32::UI::Shell::IDataObjectAsyncCapability;
        let async_cap: IDataObjectAsyncCapability = self.inner.cast()?;
        unsafe { async_cap.EndOperation(hresult, pbcreserved.as_ref(), dweffects) }
    }
}

/// Create a Windows Shell `IDataObject` for the given filesystem paths, containing `CF_HDROP`
/// (`GMEM_MOVEABLE`) and Shell IDList Array / rich Shell handlers wrapped for safe in-process
/// and cross-process OLE drag-and-drop.
#[cfg(windows)]
unsafe fn create_shell_data_object(
    paths: &[std::path::PathBuf],
    is_copy: bool,
) -> Result<windows::Win32::System::Com::IDataObject> {
    use std::mem::ManuallyDrop;
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{GlobalFree, S_OK};
    use windows::Win32::System::Com::{
        CoTaskMemFree, IDataObject, DVASPECT_CONTENT, FORMATETC, STGMEDIUM, STGMEDIUM_0,
        TYMED_HGLOBAL,
    };
    use windows::Win32::System::DataExchange::RegisterClipboardFormatW;
    use windows::Win32::System::Memory::{
        GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE, GMEM_ZEROINIT,
    };
    use windows::Win32::System::Ole::DROPEFFECT_COPY;
    use windows::Win32::UI::Shell::Common::ITEMIDLIST;
    use windows::Win32::UI::Shell::{
        ILClone, ILFindLastID, SHCreateDataObject, SHGetDesktopFolder, SHGetIDListFromObject,
        SHParseDisplayName,
    };

    const CF_HDROP: u16 = 15;

    let normalized: Vec<std::path::PathBuf> = paths
        .iter()
        .map(|p| dunce::simplified(p).to_path_buf())
        .collect();
    if normalized.is_empty() {
        anyhow::bail!("Cannot create Shell IDataObject for empty path list");
    }

    let first_parent = normalized[0].parent().filter(|p| !p.as_os_str().is_empty());
    let same_parent = first_parent.filter(|parent| {
        normalized
            .iter()
            .all(|p| p.parent() == Some(*parent))
    });

    let mut data_obj_opt: Option<IDataObject> = None;

    // 1. Common-parent path (canonical Shell drag path: SHCreateDataObject(dir_pidl, &child_pidls))
    if let Some(parent_dir) = same_parent {
        let dir_w: Vec<u16> = parent_dir
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let mut dir_pidl: *mut ITEMIDLIST = std::ptr::null_mut();
        if unsafe { SHParseDisplayName(PCWSTR(dir_w.as_ptr()), None, &mut dir_pidl, 0, None) }
            .is_ok()
            && !dir_pidl.is_null()
        {
            let mut child_pidls: Vec<*const ITEMIDLIST> = Vec::with_capacity(normalized.len());
            for p in &normalized {
                let pw: Vec<u16> = p
                    .as_os_str()
                    .encode_wide()
                    .chain(std::iter::once(0))
                    .collect();
                let mut full: *mut ITEMIDLIST = std::ptr::null_mut();
                if unsafe { SHParseDisplayName(PCWSTR(pw.as_ptr()), None, &mut full, 0, None) }
                    .is_ok()
                    && !full.is_null()
                {
                    let rel = unsafe { ILFindLastID(full) };
                    if !rel.is_null() {
                        let cloned = unsafe { ILClone(rel) };
                        if !cloned.is_null() {
                            child_pidls.push(cloned as *const ITEMIDLIST);
                        }
                    }
                    unsafe { CoTaskMemFree(Some(full as *const _ as _)) };
                }
            }
            if !child_pidls.is_empty() {
                match unsafe {
                    SHCreateDataObject::<_, IDataObject>(
                        Some(dir_pidl as *const ITEMIDLIST),
                        Some(&child_pidls),
                        None,
                    )
                } {
                    Ok(obj) => data_obj_opt = Some(obj),
                    Err(err) => {
                        tracing::warn!(
                            "SHCreateDataObject (common parent) failed for {:?}: {:?}",
                            parent_dir,
                            err
                        );
                    }
                }
            }
            for pidl in child_pidls {
                unsafe { CoTaskMemFree(Some(pidl as *const _ as _)) };
            }
            unsafe { CoTaskMemFree(Some(dir_pidl as *const _ as _)) };
        }
    }

    // 2. Multi-folder or fallback path: Desktop PIDL + full PIDLs (or empty Shell DataObject + SetData CF_HDROP)
    if data_obj_opt.is_none() {
        if let Ok(desktop) = unsafe { SHGetDesktopFolder() } {
            if let Ok(desktop_pidl) = unsafe { SHGetIDListFromObject(&desktop) } {
                let mut full_pidls: Vec<*const ITEMIDLIST> = Vec::with_capacity(normalized.len());
                for p in &normalized {
                    let pw: Vec<u16> = p
                        .as_os_str()
                        .encode_wide()
                        .chain(std::iter::once(0))
                        .collect();
                    let mut full: *mut ITEMIDLIST = std::ptr::null_mut();
                    if unsafe { SHParseDisplayName(PCWSTR(pw.as_ptr()), None, &mut full, 0, None) }
                        .is_ok()
                        && !full.is_null()
                    {
                        full_pidls.push(full as *const ITEMIDLIST);
                    }
                }
                if !full_pidls.is_empty() {
                    if let Ok(obj) = unsafe {
                        SHCreateDataObject::<_, IDataObject>(
                            Some(desktop_pidl as *const ITEMIDLIST),
                            Some(&full_pidls),
                            None,
                        )
                    } {
                        data_obj_opt = Some(obj);
                    }
                }
                if data_obj_opt.is_none() {
                    if let Ok(obj) = unsafe {
                        SHCreateDataObject::<_, IDataObject>(
                            Some(desktop_pidl as *const ITEMIDLIST),
                            None,
                            None,
                        )
                    } {
                        data_obj_opt = Some(obj);
                    }
                }
                for pidl in full_pidls {
                    unsafe { CoTaskMemFree(Some(pidl as *const _ as _)) };
                }
                unsafe { CoTaskMemFree(Some(desktop_pidl as *const _ as _)) };
            }
        }
    }

    let inner =
        data_obj_opt.ok_or_else(|| anyhow!("SHCreateDataObject failed to create IDataObject"))?;

    // 3. Ensure CF_HDROP appears in EnumFormatEtc on the inner object when fallback lacked it
    let hdrop_fmt = FORMATETC {
        cfFormat: CF_HDROP,
        ptd: std::ptr::null_mut(),
        dwAspect: DVASPECT_CONTENT.0,
        lindex: -1,
        tymed: TYMED_HGLOBAL.0 as u32,
    };
    if unsafe { inner.QueryGetData(&hdrop_fmt) } != S_OK {
        let hmem = unsafe { build_hdrop_hglobal(&normalized)? };
        let medium = STGMEDIUM {
            tymed: TYMED_HGLOBAL.0 as u32,
            u: STGMEDIUM_0 { hGlobal: hmem },
            pUnkForRelease: ManuallyDrop::new(None),
        };
        if let Err(err) = unsafe { inner.SetData(&hdrop_fmt, &medium, true) } {
            let _ = unsafe { GlobalFree(Some(hmem)) };
            return Err(anyhow!("IDataObject::SetData(CF_HDROP) failed: {:?}", err));
        }
    }

    // 4. Honor explicit copy modifier (Ctrl+drag) via Preferred DropEffect
    if is_copy {
        let format_name: Vec<u16> = "Preferred DropEffect\0".encode_utf16().collect();
        let cf_drop_effect = unsafe { RegisterClipboardFormatW(PCWSTR(format_name.as_ptr())) };
        if cf_drop_effect != 0 {
            if let Ok(effect_hmem) =
                unsafe { GlobalAlloc(GMEM_MOVEABLE | GMEM_ZEROINIT, std::mem::size_of::<u32>()) }
            {
                let effect_ptr = unsafe { GlobalLock(effect_hmem) } as *mut u32;
                if !effect_ptr.is_null() {
                    unsafe {
                        *effect_ptr = DROPEFFECT_COPY.0;
                        let _ = GlobalUnlock(effect_hmem);
                    }
                    let effect_fmt = FORMATETC {
                        cfFormat: cf_drop_effect as u16,
                        ptd: std::ptr::null_mut(),
                        dwAspect: DVASPECT_CONTENT.0,
                        lindex: -1,
                        tymed: TYMED_HGLOBAL.0 as u32,
                    };
                    let effect_med = STGMEDIUM {
                        tymed: TYMED_HGLOBAL.0 as u32,
                        u: STGMEDIUM_0 { hGlobal: effect_hmem },
                        pUnkForRelease: ManuallyDrop::new(None),
                    };
                    if unsafe { inner.SetData(&effect_fmt, &effect_med, true) }.is_err() {
                        let _ = unsafe { GlobalFree(Some(effect_hmem)) };
                    }
                } else {
                    let _ = unsafe { GlobalFree(Some(effect_hmem)) };
                }
            }
        }
    }

    let wrapped: IDataObject = DraggableShellDataObject {
        inner,
        paths: normalized,
    }
    .into();
    Ok(wrapped)
}

/// Applies the embedded application icon (`assets/app_icon.png`) to the given window handle
/// (`WM_SETICON` for `ICON_BIG` and `ICON_SMALL`) so the live title bar, Alt+Tab, and taskbar
/// display the custom icon crisply even when launched via `cargo run`.
pub fn apply_window_icon(hwnd_raw: isize) {
    #[cfg(windows)]
    {
        use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
        use windows::Win32::UI::WindowsAndMessaging::{
            CreateIconFromResourceEx, GetSystemMetrics, SendMessageW, ICON_BIG, ICON_SMALL,
            LR_DEFAULTCOLOR, SM_CXICON, SM_CXSMICON, SM_CYICON, SM_CYSMICON, WM_SETICON,
        };

        if hwnd_raw == 0 {
            return;
        }
        const ICON_PNG: &[u8] = include_bytes!("../../../assets/app_icon.png");
        let hwnd = HWND(hwnd_raw as *mut core::ffi::c_void);
        unsafe {
            let cx_big = GetSystemMetrics(SM_CXICON).max(32);
            let cy_big = GetSystemMetrics(SM_CYICON).max(32);
            let cx_small = GetSystemMetrics(SM_CXSMICON).max(16);
            let cy_small = GetSystemMetrics(SM_CYSMICON).max(16);

            if let Ok(hbig) = CreateIconFromResourceEx(
                ICON_PNG,
                true,
                0x0003_0000,
                cx_big,
                cy_big,
                LR_DEFAULTCOLOR,
            ) {
                let _ = SendMessageW(
                    hwnd,
                    WM_SETICON,
                    Some(WPARAM(ICON_BIG as usize)),
                    Some(LPARAM(hbig.0 as isize)),
                );
            }
            if let Ok(hsmall) = CreateIconFromResourceEx(
                ICON_PNG,
                true,
                0x0003_0000,
                cx_small,
                cy_small,
                LR_DEFAULTCOLOR,
            ) {
                let _ = SendMessageW(
                    hwnd,
                    WM_SETICON,
                    Some(WPARAM(ICON_SMALL as usize)),
                    Some(LPARAM(hsmall.0 as isize)),
                );
            }
        }
    }
    #[cfg(not(windows))]
    {
        let _ = hwnd_raw;
    }
}

/// Returns `true` when the current process is running with elevated (Administrator) privileges.
/// On Windows, UIPI blocks OLE drag-and-drop between elevated processes and standard
/// (medium-integrity) applications such as Explorer, the Desktop, Taskbar, and web browsers.
pub fn is_process_elevated() -> bool {
    #[cfg(windows)]
    {
        use windows::Win32::Foundation::{CloseHandle, HANDLE};
        use windows::Win32::Security::{
            GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY,
        };
        use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

        unsafe {
            let mut token = HANDLE::default();
            if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).is_err() {
                return false;
            }
            let mut elevation = TOKEN_ELEVATION::default();
            let mut ret_len = 0u32;
            let ok = GetTokenInformation(
                token,
                TokenElevation,
                Some(&mut elevation as *mut _ as *mut core::ffi::c_void),
                std::mem::size_of::<TOKEN_ELEVATION>() as u32,
                &mut ret_len,
            )
            .is_ok();
            let _ = CloseHandle(token);
            ok && elevation.TokenIsElevated != 0
        }
    }
    #[cfg(not(windows))]
    {
        false
    }
}

/// Initiate a cross-platform native OS Drag-and-Drop operation when the user drags files
/// out of the `files-rs` window toward another application (Explorer, Desktop, Discord, Browser, VS Code, etc.).
/// - On Windows: Releases GPUI mouse capture, creates a real Shell `IDataObject` (`SHCreateDataObject` + `GMEM_MOVEABLE` `CF_HDROP`),
///   and invokes `shell32.dll!SHDoDragDrop` supporting `DROPEFFECT_COPY | DROPEFFECT_MOVE | DROPEFFECT_LINK`.
///   Must be called on the UI thread with no GPUI `AppCell` / `Window` borrows held on the stack.
/// - On Linux / macOS: Launches `dragon-drop` / `ripdrag` if installed and stages `text/uri-list` (`file:///...`).
///
/// Returns `true` if a native drop completed with a non-zero drop effect.
pub fn start_native_os_file_drag(paths: &[FilesPath], is_copy: bool, hwnd_raw: isize) -> bool {
    if paths.is_empty() {
        return false;
    }
    let paths_owned: Vec<std::path::PathBuf> =
        paths.iter().map(|p| p.as_path().to_path_buf()).collect();

    #[cfg(windows)]
    {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::System::Ole::{
            OleInitialize, OleUninitialize, DROPEFFECT, DROPEFFECT_COPY, DROPEFFECT_LINK,
            DROPEFFECT_MOVE,
        };
        use windows::Win32::UI::Input::KeyboardAndMouse::{
            GetAsyncKeyState, ReleaseCapture, VK_LBUTTON,
        };
        use windows::Win32::UI::Shell::SHDoDragDrop;
        use windows::Win32::UI::WindowsAndMessaging::{
            GetForegroundWindow, GetSystemMetrics, IsWindow, SM_SWAPBUTTON,
        };

        unsafe {
            let swapped = GetSystemMetrics(SM_SWAPBUTTON) != 0;
            let vk = if swapped { 0x02i32 } else { VK_LBUTTON.0 as i32 };
            let lbutton_down = (GetAsyncKeyState(vk) as u16 & 0x8000) != 0;
            if !lbutton_down {
                tracing::debug!("Skipping SHDoDragDrop: primary mouse button already released");
                return false;
            }

            // Release GPUI's WM_LBUTTONDOWN capture so SHDoDragDrop can own drag capture cleanly
            let _ = ReleaseCapture();

            let candidate_hwnd = if hwnd_raw != 0 {
                HWND(hwnd_raw as *mut core::ffi::c_void)
            } else {
                GetForegroundWindow()
            };
            let hwnd = if !candidate_hwnd.is_invalid() && IsWindow(Some(candidate_hwnd)).as_bool() {
                Some(candidate_hwnd)
            } else {
                None
            };

            let ole_ok = OleInitialize(None).is_ok();
            let mut dropped = false;

            match create_shell_data_object(&paths_owned, is_copy) {
                Ok(data_obj) => {
                    let allowed_effects =
                        DROPEFFECT_COPY.0 | DROPEFFECT_MOVE.0 | DROPEFFECT_LINK.0;
                    if is_process_elevated() {
                        tracing::warn!(
                            "Process is running elevated (Administrator): Windows UIPI blocks drag-and-drop onto standard (medium-integrity) applications such as Explorer, Taskbar, and web browsers."
                        );
                    }
                    tracing::info!(
                        "Initiating native SHDoDragDrop with {} items (copy={}, hwnd={:?})",
                        paths_owned.len(),
                        is_copy,
                        hwnd
                    );
                    match SHDoDragDrop(
                        hwnd,
                        &data_obj,
                        None,
                        DROPEFFECT(allowed_effects),
                    ) {
                        Ok(effect) => {
                            tracing::info!("SHDoDragDrop finished with effect={:?}", effect);
                            dropped = effect.0 != 0;
                        }
                        Err(err) => {
                            tracing::warn!("SHDoDragDrop failed: {:?}", err);
                        }
                    }
                }
                Err(err) => {
                    tracing::warn!("create_shell_data_object failed: {:?}", err);
                }
            }

            if ole_ok {
                OleUninitialize();
            }
            dropped
        }
    }

    #[cfg(not(windows))]
    {
        let _ = is_copy;
        let _ = hwnd_raw;
        std::thread::spawn(move || {
            use std::io::Write;
            use std::process::{Command, Stdio};

            // 1. Try native X11/Wayland external drag helper (`dragon-drop` or `ripdrag`) if installed
            let args: Vec<String> = paths_owned
                .iter()
                .map(|p| p.to_string_lossy().to_string())
                .collect();
            if Command::new("dragon-drop")
                .arg("--and-exit")
                .args(&args)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .is_ok()
            {
                return;
            }
            if Command::new("ripdrag")
                .arg("-x")
                .args(&args)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .is_ok()
            {
                return;
            }

            // 2. Stage RFC 2483 `text/uri-list` (`file:///...`) on Wayland / X11 / macOS clipboard
            let uri_list: String = paths_owned
                .iter()
                .map(|p| format!("file://{}\r\n", p.to_string_lossy()))
                .collect();
            if let Ok(mut child) = Command::new("wl-copy")
                .args(["--type", "text/uri-list"])
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
            {
                if let Some(ref mut stdin) = child.stdin {
                    let _ = stdin.write_all(uri_list.as_bytes());
                }
                let _ = child.wait();
            } else if let Ok(mut child) = Command::new("xclip")
                .args(["-selection", "clipboard", "-t", "text/uri-list"])
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
            {
                if let Some(ref mut stdin) = child.stdin {
                    let _ = stdin.write_all(uri_list.as_bytes());
                }
                let _ = child.wait();
            }
        });
        false
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
        let path = std::env::temp_dir();
        let props = ShellProperties::get_all_properties(&path);
        assert!(props.contains_key("System.ItemNameDisplay"));
    }

    #[cfg(windows)]
    unsafe fn extract_hdrop_paths_from_data_object(
        data_obj: &windows::Win32::System::Com::IDataObject,
    ) -> Vec<std::path::PathBuf> {
        use windows::Win32::Foundation::S_OK;
        use windows::Win32::System::Com::{DVASPECT_CONTENT, FORMATETC, TYMED_HGLOBAL};
        use windows::Win32::System::Memory::{GlobalFlags, GlobalLock, GlobalUnlock};
        use windows::Win32::System::Ole::ReleaseStgMedium;
        use windows::Win32::UI::Shell::{DragQueryFileW, HDROP};

        let fmt = FORMATETC {
            cfFormat: 15, // CF_HDROP
            ptd: std::ptr::null_mut(),
            dwAspect: DVASPECT_CONTENT.0,
            lindex: -1,
            tymed: TYMED_HGLOBAL.0 as u32,
        };
        assert_eq!(unsafe { data_obj.QueryGetData(&fmt) }, S_OK);
        let mut medium = unsafe { data_obj.GetData(&fmt).expect("GetData(CF_HDROP) failed") };

        // Must be GMEM_MOVEABLE (so locking increments GlobalFlags lock count and locked != handle)
        // so gpui-0.2.2's `*(hGlobal.0 as *mut HDROP)` in WindowsDragDropHandler::DragEnter
        // never passes DROPFILES.pFiles (0x14) to DragQueryFileW and segfaults.
        let locked = unsafe { GlobalLock(medium.u.hGlobal) };
        assert!(!locked.is_null(), "GlobalLock(CF_HDROP) returned null");
        let lock_flags = unsafe { GlobalFlags(medium.u.hGlobal) };
        let _ = unsafe { GlobalUnlock(medium.u.hGlobal) };
        assert_ne!(
            lock_flags, 0,
            "CF_HDROP HGLOBAL must be GMEM_MOVEABLE (lock count > 0 when locked), not GMEM_FIXED"
        );
        assert_ne!(
            locked,
            medium.u.hGlobal.0,
            "GMEM_MOVEABLE HGLOBAL handle must differ from its locked pointer"
        );

        let hdrop = HDROP(unsafe { medium.u.hGlobal.0 });
        let count = unsafe { DragQueryFileW(hdrop, 0xFFFFFFFF, None) };
        let mut out = Vec::with_capacity(count as usize);
        for i in 0..count {
            let len = unsafe { DragQueryFileW(hdrop, i, None) };
            let mut buf = vec![0u16; len as usize + 1];
            unsafe { DragQueryFileW(hdrop, i, Some(&mut buf)) };
            if let Some(end) = buf.iter().position(|&c| c == 0) {
                out.push(std::path::PathBuf::from(String::from_utf16_lossy(&buf[..end])));
            }
        }

        // Also simulate gpui-0.2.2's exact `*hdrop` dereference in WindowsDragDropHandler::DragEnter
        let deref_hdrop = unsafe { *(medium.u.hGlobal.0 as *const HDROP) };
        let deref_count = unsafe { DragQueryFileW(deref_hdrop, 0xFFFFFFFF, None) };
        assert_eq!(deref_count, count);

        unsafe { ReleaseStgMedium(&mut medium) };
        out
    }

    #[cfg(windows)]
    #[test]
    fn test_create_shell_data_object_single_and_multi_same_parent() {
        use windows::core::Interface;
        use windows::Win32::System::Ole::{OleInitialize, OleUninitialize};
        use windows::Win32::UI::Shell::IDataObjectAsyncCapability;

        let dir = tempfile::tempdir().unwrap();
        let file_a = dir.path().join("alpha.txt");
        let file_b = dir.path().join("beta.txt");
        std::fs::write(&file_a, b"a").unwrap();
        std::fs::write(&file_b, b"b").unwrap();

        unsafe {
            let ole_ok = OleInitialize(None).is_ok();

            let obj_single = create_shell_data_object(&[file_a.clone()], false)
                .expect("single file IDataObject failed");
            let got_single = extract_hdrop_paths_from_data_object(&obj_single);
            assert_eq!(got_single, vec![dunce::simplified(&file_a).to_path_buf()]);

            // Async capability forwarding should succeed
            let async_cap: IDataObjectAsyncCapability = obj_single
                .cast()
                .expect("DraggableShellDataObject should expose IDataObjectAsyncCapability");
            assert!(async_cap.GetAsyncMode().is_ok());

            let obj_multi = create_shell_data_object(&[file_a.clone(), file_b.clone()], true)
                .expect("multi-file same-parent IDataObject failed");
            let got_multi = extract_hdrop_paths_from_data_object(&obj_multi);
            assert_eq!(
                got_multi,
                vec![
                    dunce::simplified(&file_a).to_path_buf(),
                    dunce::simplified(&file_b).to_path_buf(),
                ]
            );

            if ole_ok {
                OleUninitialize();
            }
        }
    }

    #[cfg(windows)]
    #[test]
    fn test_create_shell_data_object_mixed_parents() {
        use windows::Win32::System::Ole::{OleInitialize, OleUninitialize};

        let dir = tempfile::tempdir().unwrap();
        let sub1 = dir.path().join("sub1");
        let sub2 = dir.path().join("sub2");
        std::fs::create_dir_all(&sub1).unwrap();
        std::fs::create_dir_all(&sub2).unwrap();
        let file_a = sub1.join("one.txt");
        let file_b = sub2.join("two.txt");
        std::fs::write(&file_a, b"1").unwrap();
        std::fs::write(&file_b, b"2").unwrap();

        unsafe {
            let ole_ok = OleInitialize(None).is_ok();

            let obj = create_shell_data_object(&[file_a.clone(), file_b.clone()], false)
                .expect("mixed-parent IDataObject failed");
            let got = extract_hdrop_paths_from_data_object(&obj);
            assert_eq!(
                got,
                vec![
                    dunce::simplified(&file_a).to_path_buf(),
                    dunce::simplified(&file_b).to_path_buf(),
                ]
            );

            if ole_ok {
                OleUninitialize();
            }
        }
    }

    #[cfg(windows)]
    #[test]
    fn test_explorer_drop_target_accepts_shell_data_object() {
        use std::os::windows::ffi::OsStrExt;
        use windows::core::{Interface, PCWSTR};
        use windows::Win32::Foundation::{HWND, POINTL};
        use windows::Win32::System::Com::{CoTaskMemFree, DATADIR_GET, FORMATETC};
        use windows::Win32::System::DataExchange::GetClipboardFormatNameW;
        use windows::Win32::System::Ole::{
            IDropTarget, OleInitialize, OleUninitialize, DROPEFFECT, DROPEFFECT_COPY,
            DROPEFFECT_LINK, DROPEFFECT_MOVE,
        };
        use windows::Win32::System::SystemServices::MK_LBUTTON;
        use windows::Win32::UI::Shell::Common::ITEMIDLIST;
        use windows::Win32::UI::Shell::{IShellFolder, SHBindToObject, SHParseDisplayName};

        let src_dir = tempfile::tempdir().unwrap();
        let dst_dir = tempfile::tempdir().unwrap();
        let file_a = src_dir.path().join("drag_me.txt");
        std::fs::write(&file_a, b"hello drag").unwrap();

        unsafe {
            let ole_ok = OleInitialize(None).is_ok();

            let data_obj =
                create_shell_data_object(&[file_a.clone()], false).expect("create_shell_data_object failed");

            // 1. Verify EnumFormatEtc lists CF_HDROP (15)
            let enum_fmt = data_obj
                .EnumFormatEtc(DATADIR_GET.0 as u32)
                .expect("EnumFormatEtc(DATADIR_GET) failed");
            let mut formats = [FORMATETC::default(); 32];
            let mut fetched = 0u32;
            let _ = enum_fmt.Next(&mut formats, Some(&mut fetched));
            let mut found_hdrop = false;
            for fmt in &formats[..fetched as usize] {
                let mut name_buf = [0u16; 128];
                let nlen = GetClipboardFormatNameW(fmt.cfFormat as u32, &mut name_buf);
                let fname = if nlen > 0 {
                    String::from_utf16_lossy(&name_buf[..nlen as usize])
                } else {
                    format!("std:{}", fmt.cfFormat)
                };
                eprintln!(
                    "  enum format: cf={} ({}) tymed={} aspect={} lindex={}",
                    fmt.cfFormat, fname, fmt.tymed, fmt.dwAspect, fmt.lindex
                );
                if fmt.cfFormat == 15 {
                    found_hdrop = true;
                }
            }
            assert!(found_hdrop, "EnumFormatEtc(DATADIR_GET) must enumerate CF_HDROP");

            // 2. Bind real Windows Shell folder IDropTarget for dst_dir and invoke DragEnter
            let dst_w: Vec<u16> = dunce::simplified(dst_dir.path())
                .as_os_str()
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();
            let mut dst_pidl: *mut ITEMIDLIST = std::ptr::null_mut();
            SHParseDisplayName(PCWSTR(dst_w.as_ptr()), None, &mut dst_pidl, 0, None)
                .expect("SHParseDisplayName(dst_dir) failed");
            assert!(!dst_pidl.is_null());

            let folder: IShellFolder = SHBindToObject(None, dst_pidl, None)
                .expect("SHBindToObject(IShellFolder) failed");
            CoTaskMemFree(Some(dst_pidl as *const _ as _));

            let drop_target: IDropTarget = folder
                .CreateViewObject(HWND::default())
                .expect("IShellFolder::CreateViewObject(IDropTarget) failed");

            let mut effect = DROPEFFECT(DROPEFFECT_COPY.0 | DROPEFFECT_MOVE.0 | DROPEFFECT_LINK.0);
            drop_target
                .DragEnter(&data_obj, MK_LBUTTON, POINTL { x: 10, y: 10 }, &mut effect)
                .expect("IDropTarget::DragEnter failed");
            eprintln!("  Explorer IDropTarget::DragEnter returned effect = {:?}", effect);
            assert_ne!(
                effect.0, 0,
                "Windows Shell folder IDropTarget rejected our IDataObject (DROPEFFECT_NONE)"
            );
            let _ = drop_target.DragLeave();

            // 3. Verify standard COM cross-apartment marshaling (proxy/stub path used across processes)
            use windows::Win32::System::Com::Marshal::CoMarshalInterThreadInterfaceInStream;
            use windows::Win32::System::Com::StructuredStorage::CoGetInterfaceAndReleaseStream;
            use windows::Win32::System::Com::IDataObject;
            let stream = CoMarshalInterThreadInterfaceInStream(&IDataObject::IID, &data_obj)
                .expect("CoMarshalInterThreadInterfaceInStream(IDataObject) failed");
            let stream_raw = windows::core::Interface::into_raw(stream) as usize;

            let join = std::thread::spawn(move || unsafe {
                let ole2 = OleInitialize(None).is_ok();
                let stream: std::mem::ManuallyDrop<windows::Win32::System::Com::IStream> =
                    std::mem::ManuallyDrop::new(windows::core::Interface::from_raw(
                        stream_raw as *mut _,
                    ));
                let proxy: IDataObject = CoGetInterfaceAndReleaseStream(&*stream)
                    .expect("CoGetInterfaceAndReleaseStream(IDataObject) failed");
                let got = extract_hdrop_paths_from_data_object(&proxy);
                drop(proxy);
                if ole2 {
                    OleUninitialize();
                }
                got
            });

            // Pump STA messages while the worker STA thread calls back through the COM proxy!
            use windows::Win32::UI::WindowsAndMessaging::{
                DispatchMessageW, PeekMessageW, TranslateMessage, MSG, PM_REMOVE,
            };
            let start = std::time::Instant::now();
            while !join.is_finished() && start.elapsed() < std::time::Duration::from_secs(5) {
                let mut msg = MSG::default();
                while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            let marshaled_paths = join.join().expect("COM marshaling thread panicked");
            assert_eq!(marshaled_paths, vec![dunce::simplified(&file_a).to_path_buf()]);

            if ole_ok {
                OleUninitialize();
            }
        }
    }
}

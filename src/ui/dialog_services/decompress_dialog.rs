//! Decompress Dialog Service
//! State machine for DecompressArchiveDialog.

use crate::ui::components::text_input::TextInputState;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

/// Wrapper around `gpui::ScrollHandle` so `DecompressState` keeps `Clone`, `Debug`, and `PartialEq`.
#[derive(Clone, Default)]
pub struct ArchiveTreeScrollHandle(pub gpui::ScrollHandle);

impl std::fmt::Debug for ArchiveTreeScrollHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ArchiveTreeScrollHandle").finish()
    }
}

impl PartialEq for ArchiveTreeScrollHandle {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

/// Tri-state checkbox state for a folder node in the archive tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FolderCheckState {
    Unchecked,
    Partial,
    Checked,
}

/// Flattened visible node produced by `DecompressState::visible_tree_rows()`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveTreeRow {
    pub name: String,
    pub folder_key: String,
    pub depth: usize,
    pub is_dir: bool,
    pub expanded: bool,
    pub entry_index: Option<usize>,
    pub descendant_indices: Vec<usize>,
    pub total_size: u64,
    pub child_count: usize,
}

/// Focused text input field inside `DecompressArchiveDialog`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DecompressInputField {
    Destination,
    Password,
}

/// State for DecompressArchiveDialog
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DecompressState {
    /// Archive path
    pub archive_path: PathBuf,
    /// List of archive contents (for preview)
    pub contents: Vec<ArchiveEntry>,
    /// Selected entries to extract (empty = all)
    pub selected_entries: Vec<usize>,
    /// Destination folder
    pub destination: PathBuf,
    /// Editable destination text buffer synced with `destination`
    #[serde(default)]
    pub destination_text: String,
    /// Unified text input state for Destination field
    #[serde(skip)]
    pub destination_input: TextInputState,
    /// Detected archive format badge (e.g. "ZIP", "TAR.GZ", "7Z")
    #[serde(default)]
    pub format_badge: String,
    /// Archive compressed size on disk (bytes)
    #[serde(default)]
    pub compressed_size: u64,
    /// Extract with full paths
    pub full_paths: bool,
    /// Extract into a subfolder named after the archive
    #[serde(default)]
    pub create_subfolder: bool,
    /// Open / navigate to destination folder after extraction finishes
    #[serde(default)]
    pub open_destination: bool,
    /// Overwrite existing files
    pub overwrite: bool,
    /// Password for encrypted archives
    pub password: Option<String>,
    /// Unified text input state for Password field
    #[serde(skip)]
    pub password_input: TextInputState,
    /// Show password field / reveal password characters
    pub show_password: bool,
    /// Currently focused input field for keyboard typing
    #[serde(default)]
    pub focused_field: Option<DecompressInputField>,
    /// True while background archive inventory scan is running
    #[serde(default)]
    pub loading: bool,
    /// Error message
    pub error: Option<String>,
    /// Is the archive password protected
    pub is_encrypted: bool,
    /// Folders currently collapsed in the Extract Archive tree view
    #[serde(default)]
    pub collapsed_folders: HashSet<String>,
    /// Scroll handle for the archive tree scroll viewport
    #[serde(skip)]
    pub tree_scroll_handle: ArchiveTreeScrollHandle,
}

/// An entry within an archive
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArchiveEntry {
    /// Index in archive
    pub index: usize,
    /// Path within archive
    pub path: String,
    /// Is this a directory
    pub is_dir: bool,
    /// Uncompressed size (bytes)
    pub size: u64,
}

impl Default for DecompressState {
    fn default() -> Self {
        Self {
            archive_path: PathBuf::new(),
            contents: Vec::new(),
            selected_entries: Vec::new(),
            destination: PathBuf::new(),
            destination_text: String::new(),
            destination_input: TextInputState::default(),
            format_badge: String::new(),
            compressed_size: 0,
            full_paths: false,
            create_subfolder: false,
            open_destination: false,
            overwrite: false,
            password: None,
            password_input: TextInputState::default(),
            show_password: false,
            focused_field: None,
            loading: false,
            error: None,
            is_encrypted: false,
            collapsed_folders: HashSet::new(),
            tree_scroll_handle: ArchiveTreeScrollHandle::default(),
        }
    }
}

impl DecompressState {
    /// Create a new decompress state
    pub fn new(archive_path: PathBuf, destination: PathBuf) -> Self {
        let dest_text = destination.to_string_lossy().to_string();
        let format_badge = crate::storage::archive_service::ArchiveFormat::detect(&archive_path)
            .map(|f| f.short_badge().to_string())
            .unwrap_or_else(|| "ARCHIVE".to_string());
        Self {
            archive_path,
            destination,
            destination_text: dest_text.clone(),
            destination_input: TextInputState::with_text(dest_text, false),
            format_badge,
            full_paths: true,
            overwrite: true,
            create_subfolder: true,
            open_destination: true,
            focused_field: Some(DecompressInputField::Destination),
            ..Default::default()
        }
    }

    pub fn sync_destination_from_text(&mut self) {
        self.destination_text = self.destination_input.text.clone();
        let trimmed = self.destination_text.trim();
        self.destination = if trimmed.is_empty() {
            PathBuf::new()
        } else {
            PathBuf::from(trimmed)
        };
    }

    pub fn sync_password_from_input(&mut self) {
        self.password = if self.password_input.text.is_empty() {
            None
        } else {
            Some(self.password_input.text.clone())
        };
    }

    pub fn set_destination_path(&mut self, dest: PathBuf) {
        self.destination_text = dest.to_string_lossy().to_string();
        self.destination_input.set_text_and_cursor_end(self.destination_text.clone());
        self.destination = dest;
    }

    pub fn effective_destination_preview(&self) -> PathBuf {
        let base = if self.destination_text.trim().is_empty() {
            self.destination.clone()
        } else {
            PathBuf::from(self.destination_text.trim())
        };
        if self.create_subfolder {
            let sub = crate::storage::archive_service::ArchiveFormat::suggested_folder_name(
                &self.archive_path,
            );
            base.join(sub)
        } else {
            base
        }
    }

    /// Set archive contents
    pub fn set_contents(&mut self, contents: Vec<ArchiveEntry>) {
        let count = contents.len();
        self.contents = contents;
        // By default, select all entries
        self.selected_entries = (0..count).collect();
        self.collapsed_folders.clear();
        // On huge archives (> 250 entries), auto-collapse deeper folders for responsive browsing
        if count > 250 {
            let mut seen_folders = HashSet::new();
            for e in &self.contents {
                let norm = e.path.trim_matches(['/', '\\']);
                let mut cur = String::new();
                let parts: Vec<&str> = norm.split(['/', '\\']).filter(|s| !s.is_empty()).collect();
                let limit = if e.is_dir { parts.len() } else { parts.len().saturating_sub(1) };
                for (depth, seg) in parts.iter().take(limit).enumerate() {
                    if !cur.is_empty() {
                        cur.push('/');
                    }
                    cur.push_str(seg);
                    if depth >= 1 && seen_folders.insert(cur.clone()) {
                        self.collapsed_folders.insert(cur.clone());
                    }
                }
            }
        }
    }

    pub fn toggle_folder_expand(&mut self, folder_key: &str) {
        if !self.collapsed_folders.remove(folder_key) {
            self.collapsed_folders.insert(folder_key.to_string());
        }
    }

    pub fn expand_all_folders(&mut self) {
        self.collapsed_folders.clear();
    }

    pub fn collapse_all_folders(&mut self) {
        let mut all = HashSet::new();
        for e in &self.contents {
            let norm = e.path.trim_matches(['/', '\\']);
            let parts: Vec<&str> = norm.split(['/', '\\']).filter(|s| !s.is_empty()).collect();
            let limit = if e.is_dir { parts.len() } else { parts.len().saturating_sub(1) };
            let mut cur = String::new();
            for seg in parts.iter().take(limit) {
                if !cur.is_empty() {
                    cur.push('/');
                }
                cur.push_str(seg);
                all.insert(cur.clone());
            }
        }
        self.collapsed_folders = all;
    }

    pub fn folder_check_state(&self, descendant_indices: &[usize]) -> FolderCheckState {
        if descendant_indices.is_empty() {
            return FolderCheckState::Unchecked;
        }
        let sel: HashSet<usize> = self.selected_entries.iter().copied().collect();
        let selected_count = descendant_indices.iter().filter(|i| sel.contains(i)).count();
        if selected_count == 0 {
            FolderCheckState::Unchecked
        } else if selected_count == descendant_indices.len() {
            FolderCheckState::Checked
        } else {
            FolderCheckState::Partial
        }
    }

    pub fn toggle_folder_selection(&mut self, descendant_indices: &[usize]) {
        if descendant_indices.is_empty() {
            return;
        }
        let state = self.folder_check_state(descendant_indices);
        let targets: HashSet<usize> = descendant_indices.iter().copied().collect();
        if state == FolderCheckState::Checked {
            self.selected_entries.retain(|i| !targets.contains(i));
        } else {
            let existing: HashSet<usize> = self.selected_entries.iter().copied().collect();
            for &idx in descendant_indices {
                if !existing.contains(&idx) {
                    self.selected_entries.push(idx);
                }
            }
            self.selected_entries.sort_unstable();
        }
    }

    /// Build a hierarchical tree view (folders first, sorted naturally at each level) flattened for display.
    pub fn visible_tree_rows(&self) -> Vec<ArchiveTreeRow> {
        #[derive(Default)]
        struct BuildNode {
            name: String,
            folder_key: String,
            is_dir: bool,
            entry_index: Option<usize>,
            own_size: u64,
            children: BTreeMap<(u8, String, String), BuildNode>,
        }

        fn insert_path(
            root: &mut BuildNode,
            parts: &[&str],
            depth_idx: usize,
            prefix: &str,
            entry: &ArchiveEntry,
        ) {
            if depth_idx >= parts.len() {
                return;
            }
            let seg = parts[depth_idx];
            let is_last = depth_idx + 1 == parts.len();
            let node_is_dir = !is_last || entry.is_dir;
            let full_key = if prefix.is_empty() {
                seg.to_string()
            } else {
                format!("{}/{}", prefix, seg)
            };
            let order_group = if node_is_dir { 0u8 } else { 1u8 };
            let sort_key = (order_group, seg.to_lowercase(), full_key.clone());
            let child = root.children.entry(sort_key).or_insert_with(|| BuildNode {
                name: seg.to_string(),
                folder_key: full_key.clone(),
                is_dir: node_is_dir,
                entry_index: None,
                own_size: 0,
                children: BTreeMap::new(),
            });
            if node_is_dir {
                child.is_dir = true;
            }
            if is_last {
                child.entry_index = Some(entry.index);
                child.own_size = if entry.is_dir { 0 } else { entry.size };
            } else {
                insert_path(child, parts, depth_idx + 1, &full_key, entry);
            }
        }

        fn summarize_and_flatten(
            node: &BuildNode,
            depth: usize,
            collapsed: &HashSet<String>,
            out: &mut Vec<ArchiveTreeRow>,
        ) -> (Vec<usize>, u64, usize) {
            let mut desc = Vec::new();
            let mut total_size = node.own_size;
            let mut file_count = usize::from(!node.is_dir);
            if let Some(idx) = node.entry_index {
                desc.push(idx);
            }
            let expanded = !node.is_dir || !collapsed.contains(&node.folder_key);
            let row_pos = out.len();
            if node.is_dir {
                out.push(ArchiveTreeRow {
                    name: node.name.clone(),
                    folder_key: node.folder_key.clone(),
                    depth,
                    is_dir: true,
                    expanded,
                    entry_index: node.entry_index,
                    descendant_indices: Vec::new(),
                    total_size: 0,
                    child_count: node.children.len(),
                });
                let mut tmp_desc_only = Vec::new();
                for child in node.children.values() {
                    if expanded {
                        let (cd, cs, cf) = summarize_and_flatten(child, depth + 1, collapsed, out);
                        desc.extend(cd);
                        total_size = total_size.saturating_add(cs);
                        file_count += cf;
                    } else {
                        let (cd, cs, cf) =
                            summarize_and_flatten(child, depth + 1, collapsed, &mut tmp_desc_only);
                        desc.extend(cd);
                        total_size = total_size.saturating_add(cs);
                        file_count += cf;
                    }
                }
                if let Some(row) = out.get_mut(row_pos) {
                    row.descendant_indices = desc.clone();
                    row.total_size = total_size;
                }
            } else {
                out.push(ArchiveTreeRow {
                    name: node.name.clone(),
                    folder_key: node.folder_key.clone(),
                    depth,
                    is_dir: false,
                    expanded: false,
                    entry_index: node.entry_index,
                    descendant_indices: desc.clone(),
                    total_size,
                    child_count: 0,
                });
            }
            (desc, total_size, file_count)
        }

        let mut root = BuildNode::default();
        for entry in &self.contents {
            let norm = entry.path.trim_matches(['/', '\\']);
            let parts: Vec<&str> = norm.split(['/', '\\']).filter(|s| !s.is_empty()).collect();
            if parts.is_empty() {
                continue;
            }
            insert_path(&mut root, &parts, 0, "", entry);
        }

        let mut out = Vec::new();
        for child in root.children.values() {
            summarize_and_flatten(child, 0, &self.collapsed_folders, &mut out);
        }
        out
    }

    /// Toggle selection of an entry
    pub fn toggle_entry(&mut self, index: usize) {
        if self.selected_entries.contains(&index) {
            self.selected_entries.retain(|&i| i != index);
        } else {
            self.selected_entries.push(index);
        }
    }

    /// Select all entries
    pub fn select_all(&mut self) {
        self.selected_entries = (0..self.contents.len()).collect();
    }

    /// Deselect all entries
    pub fn deselect_all(&mut self) {
        self.selected_entries.clear();
    }

    /// Set password
    pub fn set_password(&mut self, password: Option<String>) {
        self.password = password;
    }

    /// Toggle password visibility
    pub fn toggle_password(&mut self) {
        self.show_password = !self.show_password;
    }

    /// Toggle overwrite mode
    pub fn toggle_overwrite(&mut self) {
        self.overwrite = !self.overwrite;
    }

    /// Toggle full paths extraction
    pub fn toggle_full_paths(&mut self) {
        self.full_paths = !self.full_paths;
    }

    /// Get total size of selected entries
    pub fn selected_size(&self) -> u64 {
        self.selected_entries
            .iter()
            .filter_map(|&i| self.contents.get(i))
            .filter(|e| !e.is_dir)
            .map(|e| e.size)
            .sum()
    }

    /// Get count of selected entries
    pub fn selected_count(&self) -> usize {
        self.selected_entries.len()
    }

    /// Get count of directories in selection
    pub fn selected_dir_count(&self) -> usize {
        self.selected_entries
            .iter()
            .filter_map(|&i| self.contents.get(i))
            .filter(|e| e.is_dir)
            .count()
    }

    /// Validate the decompress settings
    pub fn validate(&self) -> Result<(), String> {
        // Check destination first (user can fix this easily)
        if self.destination.to_str().map_or(true, |s| s.is_empty()) {
            return Err("No destination folder specified".to_string());
        }
        // Check selection
        if self.selected_entries.is_empty() {
            return Err("No files selected for extraction".to_string());
        }
        // Check archive path is specified (existence is checked by archive service)
        if self.archive_path.to_str().map_or(true, |s| s.is_empty()) {
            return Err("No archive file specified".to_string());
        }
        Ok(())
    }

    /// Check if archive is encrypted (call after reading contents)
    pub fn check_encrypted(&self) -> bool {
        // Archives with encrypted entries typically have a specific flag
        // For now, we check if password was needed
        self.is_encrypted
    }
}

/// Opens a native OS folder picker dialog (`IFileOpenDialog` with `FOS_PICKFOLDERS` on Windows).
#[cfg(windows)]
pub fn pick_folder_native(initial_dir: Option<&Path>) -> Option<PathBuf> {
    use std::ffi::OsString;
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    use windows::core::PCWSTR;
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, CLSCTX_ALL,
        COINIT_APARTMENTTHREADED,
    };
    use windows::Win32::UI::Shell::{
        FileOpenDialog, IFileOpenDialog, IShellItem, SHCreateItemFromParsingName,
        FOS_FORCEFILESYSTEM, FOS_PICKFOLDERS, SIGDN_FILESYSPATH,
    };

    unsafe {
        let hr_init = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let did_init = hr_init.is_ok();

        let result = (|| -> Option<PathBuf> {
            let dialog: IFileOpenDialog =
                CoCreateInstance(&FileOpenDialog, None, CLSCTX_ALL).ok()?;
            if let Ok(mut opts) = dialog.GetOptions() {
                opts |= FOS_PICKFOLDERS | FOS_FORCEFILESYSTEM;
                let _ = dialog.SetOptions(opts);
            }
            let title: Vec<u16> = "Select Extraction Folder\0".encode_utf16().collect();
            let _ = dialog.SetTitle(PCWSTR(title.as_ptr()));
            if let Some(dir) = initial_dir.filter(|p| p.exists()) {
                let wide: Vec<u16> = dir
                    .as_os_str()
                    .encode_wide()
                    .chain(std::iter::once(0))
                    .collect();
                if let Ok(item) =
                    SHCreateItemFromParsingName::<_, _, IShellItem>(PCWSTR(wide.as_ptr()), None)
                {
                    let _ = dialog.SetFolder(&item);
                }
            }
            if dialog.Show(None).is_err() {
                return None;
            }
            let item = dialog.GetResult().ok()?;
            let pwstr = item.GetDisplayName(SIGDN_FILESYSPATH).ok()?;
            let mut len = 0usize;
            while *pwstr.0.add(len) != 0 {
                len += 1;
            }
            let slice = std::slice::from_raw_parts(pwstr.0, len);
            let path = PathBuf::from(OsString::from_wide(slice));
            CoTaskMemFree(Some(pwstr.0 as *const _));
            Some(path)
        })();

        if did_init {
            CoUninitialize();
        }
        result
    }
}

#[cfg(not(windows))]
pub fn pick_folder_native(initial_dir: Option<&Path>) -> Option<PathBuf> {
    initial_dir.map(|p| p.to_path_buf())
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_entry(index: usize, path: &str, is_dir: bool, size: u64) -> ArchiveEntry {
        ArchiveEntry {
            index,
            path: path.to_string(),
            is_dir,
            size,
        }
    }

    #[test]
    fn test_decompress_state_default() {
        let state = DecompressState::default();
        assert!(state.archive_path.as_os_str().is_empty());
        assert!(state.contents.is_empty());
        assert!(state.selected_entries.is_empty());
        assert!(!state.full_paths);
        assert!(!state.overwrite);
        assert!(state.password.is_none());
        assert!(!state.show_password);
        assert!(state.error.is_none());
        assert!(!state.is_encrypted);
    }

    #[test]
    fn test_decompress_state_new() {
        let archive = PathBuf::from("C:\\test\\archive.zip");
        let dest = PathBuf::from("C:\\test\\extracted");
        let state = DecompressState::new(archive.clone(), dest.clone());
        assert_eq!(state.archive_path, archive);
        assert_eq!(state.destination, dest);
    }

    #[test]
    fn test_decompress_state_set_contents() {
        let mut state = DecompressState::default();
        let entries = vec![
            create_test_entry(0, "folder/", true, 0),
            create_test_entry(1, "folder/file1.txt", false, 1024),
            create_test_entry(2, "folder/file2.txt", false, 2048),
        ];
        state.set_contents(entries.clone());
        assert_eq!(state.contents.len(), 3);
        assert_eq!(state.selected_entries.len(), 3); // All selected by default
    }

    #[test]
    fn test_decompress_state_toggle_entry() {
        let mut state = DecompressState::default();
        state.contents = vec![
            create_test_entry(0, "file1.txt", false, 1024),
            create_test_entry(1, "file2.txt", false, 2048),
        ];
        state.selected_entries = vec![0]; // Select first only

        state.toggle_entry(1); // Toggle second
        assert_eq!(state.selected_entries, vec![0, 1]);

        state.toggle_entry(0); // Toggle first
        assert_eq!(state.selected_entries, vec![1]);

        state.toggle_entry(1); // Toggle second again
        assert_eq!(state.selected_entries, vec![] as Vec<usize>);
    }

    #[test]
    fn test_decompress_state_select_all_deselect_all() {
        let mut state = DecompressState::default();
        state.contents = vec![
            create_test_entry(0, "file1.txt", false, 1024),
            create_test_entry(1, "file2.txt", false, 2048),
            create_test_entry(2, "file3.txt", false, 3072),
        ];

        state.select_all();
        assert_eq!(state.selected_entries.len(), 3);

        state.deselect_all();
        assert!(state.selected_entries.is_empty());
    }

    #[test]
    fn test_decompress_state_selected_size() {
        let mut state = DecompressState::default();
        state.contents = vec![
            create_test_entry(0, "folder/", true, 0),
            create_test_entry(1, "file1.txt", false, 1024),
            create_test_entry(2, "file2.txt", false, 2048),
            create_test_entry(3, "file3.txt", false, 3072),
        ];
        state.selected_entries = vec![0, 1, 2, 3]; // All

        assert_eq!(state.selected_size(), 1024 + 2048 + 3072); // Excludes dirs

        state.selected_entries = vec![1, 2];
        assert_eq!(state.selected_size(), 1024 + 2048);
    }

    #[test]
    fn test_decompress_state_selected_count() {
        let mut state = DecompressState::default();
        state.contents = vec![
            create_test_entry(0, "folder/", true, 0),
            create_test_entry(1, "file1.txt", false, 1024),
            create_test_entry(2, "file2.txt", false, 2048),
        ];
        state.selected_entries = vec![0, 1];
        assert_eq!(state.selected_count(), 2);
    }

    #[test]
    fn test_decompress_state_selected_dir_count() {
        let mut state = DecompressState::default();
        state.contents = vec![
            create_test_entry(0, "folder1/", true, 0),
            create_test_entry(1, "folder2/", true, 0),
            create_test_entry(2, "file1.txt", false, 1024),
        ];
        state.selected_entries = vec![0, 1, 2];
        assert_eq!(state.selected_dir_count(), 2);
    }

    #[test]
    fn test_decompress_state_toggle_password() {
        let mut state = DecompressState::default();
        assert!(!state.show_password);
        state.toggle_password();
        assert!(state.show_password);
        state.toggle_password();
        assert!(!state.show_password);
    }

    #[test]
    fn test_decompress_state_toggle_overwrite() {
        let mut state = DecompressState::default();
        assert!(!state.overwrite);
        state.toggle_overwrite();
        assert!(state.overwrite);
        state.toggle_overwrite();
        assert!(!state.overwrite);
    }

    #[test]
    fn test_decompress_state_toggle_full_paths() {
        let mut state = DecompressState::default();
        assert!(!state.full_paths); // Default false
        state.toggle_full_paths();
        assert!(state.full_paths);
    }

    #[test]
    fn test_decompress_state_validate_empty_archive() {
        let mut state = DecompressState::default();
        state.destination = PathBuf::from("/tmp/extracted");
        state.contents = vec![create_test_entry(0, "file.txt", false, 1024)];
        state.selected_entries = vec![0]; // Select at least one entry
        let result = state.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("No archive file"));
    }

    #[test]
    fn test_decompress_state_validate_no_selection() {
        let mut state = DecompressState::default();
        state.archive_path = PathBuf::from("C:\\test\\archive.zip");
        state.destination = PathBuf::from("C:\\test\\extracted");
        state.contents = vec![create_test_entry(0, "file.txt", false, 1024)];
        state.selected_entries = vec![]; // Nothing selected
        let result = state.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("No files selected"));
    }

    #[test]
    fn test_decompress_state_validate_success() {
        let mut state = DecompressState::default();
        state.archive_path = PathBuf::from("C:\\test\\archive.zip");
        state.destination = PathBuf::from("C:\\test\\extracted");
        state.contents = vec![create_test_entry(0, "file.txt", false, 1024)];
        state.selected_entries = vec![0];
        let result = state.validate();
        assert!(result.is_ok());
    }

    #[test]
    fn test_decompress_tree_hierarchy_collapse_and_tristate() {
        let mut state = DecompressState::default();
        // Implicit parent folder "src/sub" should be synthesized cleanly alongside files
        let entries = vec![
            create_test_entry(0, "src/main.rs", false, 500),
            create_test_entry(1, "src/sub/lib.rs", false, 300),
            create_test_entry(2, "README.md", false, 200),
        ];
        state.set_contents(entries);

        let rows = state.visible_tree_rows();
        assert_eq!(rows.len(), 5);
        assert_eq!(rows[0].name, "src");
        assert!(rows[0].is_dir);
        assert_eq!(rows[0].depth, 0);
        assert_eq!(rows[1].name, "sub");
        assert!(rows[1].is_dir);
        assert_eq!(rows[1].depth, 1);
        assert_eq!(rows[2].name, "lib.rs");
        assert_eq!(rows[2].depth, 2);
        assert_eq!(rows[3].name, "main.rs");
        assert_eq!(rows[3].depth, 1);
        assert_eq!(rows[4].name, "README.md");
        assert_eq!(rows[4].depth, 0);

        let src_desc = rows[0].descendant_indices.clone();
        let sub_desc = rows[1].descendant_indices.clone();

        // Checked state for folders initially All Checked
        assert_eq!(state.folder_check_state(&src_desc), FolderCheckState::Checked);
        assert_eq!(state.folder_check_state(&sub_desc), FolderCheckState::Checked);

        // Deselecting one child in "src" makes "src" Partial
        state.toggle_entry(0);
        assert_eq!(state.folder_check_state(&src_desc), FolderCheckState::Partial);
        assert_eq!(state.folder_check_state(&sub_desc), FolderCheckState::Checked);

        // Clicking a Partial folder checks all descendants
        state.toggle_folder_selection(&src_desc);
        assert_eq!(state.folder_check_state(&src_desc), FolderCheckState::Checked);

        // Clicking a Checked folder unchecks all descendants
        state.toggle_folder_selection(&src_desc);
        assert_eq!(state.folder_check_state(&src_desc), FolderCheckState::Unchecked);
        assert!(!state.selected_entries.contains(&0));
        assert!(!state.selected_entries.contains(&1));
        assert!(state.selected_entries.contains(&2));

        // Collapsing "src" hides its descendants but preserves README.md
        state.toggle_folder_expand("src");
        let collapsed_rows = state.visible_tree_rows();
        assert_eq!(collapsed_rows.len(), 2);
        assert_eq!(collapsed_rows[0].name, "src");
        assert!(!collapsed_rows[0].expanded);
        assert_eq!(collapsed_rows[1].name, "README.md");

        state.expand_all_folders();
        assert_eq!(state.visible_tree_rows().len(), 5);
        state.collapse_all_folders();
        assert_eq!(state.visible_tree_rows().len(), 2);
    }
}

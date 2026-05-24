//! Decompress Dialog Service
//! State machine for DecompressArchiveDialog.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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
    /// Extract with full paths
    pub full_paths: bool,
    /// Overwrite existing files
    pub overwrite: bool,
    /// Password for encrypted archives
    pub password: Option<String>,
    /// Show password field
    pub show_password: bool,
    /// Error message
    pub error: Option<String>,
    /// Is the archive password protected
    pub is_encrypted: bool,
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
            full_paths: false,
            overwrite: false,
            password: None,
            show_password: false,
            error: None,
            is_encrypted: false,
        }
    }
}

impl DecompressState {
    /// Create a new decompress state
    pub fn new(archive_path: PathBuf, destination: PathBuf) -> Self {
        Self {
            archive_path,
            destination,
            ..Default::default()
        }
    }

    /// Set archive contents
    pub fn set_contents(&mut self, contents: Vec<ArchiveEntry>) {
        let count = contents.len();
        self.contents = contents;
        // By default, select all entries
        self.selected_entries = (0..count).collect();
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
}

//! Conflict Resolution Dialog Service
//! State machine for FilesystemOperationDialog conflict resolution.
//! Provides data structures and helpers for displaying source vs. destination file info
//! and choosing a resolution action.

use serde::{Deserialize, Serialize};

/// Resolution action for a file conflict
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConflictResolution {
    /// Replace the destination with the source
    Replace,
    /// Replace this and all remaining conflicts
    ReplaceAll,
    /// Skip this file (keep destination as-is)
    Skip,
    /// Skip this and all remaining conflicts
    SkipAll,
    /// Rename the source file before copying
    Rename,
}

impl ConflictResolution {
    /// Get a display label for this resolution
    pub fn display_name(&self) -> &'static str {
        match self {
            ConflictResolution::Replace => "Replace",
            ConflictResolution::ReplaceAll => "Replace All",
            ConflictResolution::Skip => "Skip",
            ConflictResolution::SkipAll => "Skip All",
            ConflictResolution::Rename => "Rename",
        }
    }

    /// Whether this resolution applies to all remaining conflicts
    pub fn applies_to_all(&self) -> bool {
        matches!(self, ConflictResolution::ReplaceAll | ConflictResolution::SkipAll)
    }
}

/// State for the conflict resolution dialog
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConflictDialogState {
    /// Source file path
    pub source_path: String,
    /// Destination file path
    pub dest_path: String,
    /// Source file size in bytes
    pub source_size: u64,
    /// Destination file size in bytes
    pub dest_size: u64,
    /// Source file last modified date string
    pub source_modified: String,
    /// Destination file last modified date string
    pub dest_modified: String,
    /// Source file creation date string
    pub source_created: Option<String>,
    /// Destination file creation date string
    pub dest_created: Option<String>,
    /// Whether "Apply to all" checkbox is checked
    pub apply_to_all: bool,
    /// The chosen resolution (set when user clicks a button)
    pub chosen_resolution: Option<ConflictResolution>,
}

impl Default for ConflictDialogState {
    fn default() -> Self {
        Self {
            source_path: String::new(),
            dest_path: String::new(),
            source_size: 0,
            dest_size: 0,
            source_modified: String::new(),
            dest_modified: String::new(),
            source_created: None,
            dest_created: None,
            apply_to_all: false,
            chosen_resolution: None,
        }
    }
}

impl ConflictDialogState {
    /// Create a new conflict dialog state for a source/dest pair
    pub fn new(
        source_path: String,
        dest_path: String,
        source_size: u64,
        dest_size: u64,
        source_modified: String,
        dest_modified: String,
    ) -> Self {
        Self {
            source_path,
            dest_path,
            source_size,
            dest_size,
            source_modified,
            dest_modified,
            ..Default::default()
        }
    }

    /// Set the apply-to-all flag
    pub fn set_apply_to_all(&mut self, apply: bool) {
        self.apply_to_all = apply;
    }

    /// Toggle the apply-to-all flag
    pub fn toggle_apply_to_all(&mut self) {
        self.apply_to_all = !self.apply_to_all;
    }

    /// Choose a resolution and mark dialog as ready to return
    pub fn choose(&mut self, resolution: ConflictResolution) {
        self.chosen_resolution = Some(resolution);
    }

    /// Get the formatted source file name (last component of path)
    pub fn source_filename(&self) -> &str {
        self.source_path
            .rsplit('/')
            .next()
            .unwrap_or(&self.source_path)
    }

    /// Get the formatted destination file name (last component of path)
    pub fn dest_filename(&self) -> &str {
        self.dest_path
            .rsplit('/')
            .next()
            .unwrap_or(&self.dest_path)
    }

    /// Format a file size in human-readable form
    pub fn format_size(size: u64) -> String {
        if size >= 1_000_000_000 {
            format!("{:.2} GB", size as f64 / 1_000_000_000.0)
        } else if size >= 1_000_000 {
            format!("{:.2} MB", size as f64 / 1_000_000.0)
        } else if size >= 1_000 {
            format!("{:.2} KB", size as f64 / 1_000.0)
        } else {
            format!("{} bytes", size)
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_conflict_resolution_display_name() {
        assert_eq!(ConflictResolution::Replace.display_name(), "Replace");
        assert_eq!(ConflictResolution::ReplaceAll.display_name(), "Replace All");
        assert_eq!(ConflictResolution::Skip.display_name(), "Skip");
        assert_eq!(ConflictResolution::SkipAll.display_name(), "Skip All");
        assert_eq!(ConflictResolution::Rename.display_name(), "Rename");
    }

    #[test]
    fn test_conflict_resolution_applies_to_all() {
        assert!(ConflictResolution::ReplaceAll.applies_to_all());
        assert!(ConflictResolution::SkipAll.applies_to_all());
        assert!(!ConflictResolution::Replace.applies_to_all());
        assert!(!ConflictResolution::Skip.applies_to_all());
        assert!(!ConflictResolution::Rename.applies_to_all());
    }

    #[test]
    fn test_conflict_dialog_state_default() {
        let state = ConflictDialogState::default();
        assert!(state.source_path.is_empty());
        assert!(state.dest_path.is_empty());
        assert_eq!(state.source_size, 0);
        assert_eq!(state.dest_size, 0);
        assert!(!state.apply_to_all);
        assert!(state.chosen_resolution.is_none());
    }

    #[test]
    fn test_conflict_dialog_state_new() {
        let state = ConflictDialogState::new(
            "/src/file.txt".to_string(),
            "/dest/file.txt".to_string(),
            1024,
            2048,
            "2024-01-01".to_string(),
            "2024-01-02".to_string(),
        );
        assert_eq!(state.source_path, "/src/file.txt");
        assert_eq!(state.dest_path, "/dest/file.txt");
        assert_eq!(state.source_size, 1024);
        assert_eq!(state.dest_size, 2048);
    }

    #[test]
    fn test_set_apply_to_all() {
        let mut state = ConflictDialogState::default();
        assert!(!state.apply_to_all);
        state.set_apply_to_all(true);
        assert!(state.apply_to_all);
        state.set_apply_to_all(false);
        assert!(!state.apply_to_all);
    }

    #[test]
    fn test_toggle_apply_to_all() {
        let mut state = ConflictDialogState::default();
        state.toggle_apply_to_all();
        assert!(state.apply_to_all);
        state.toggle_apply_to_all();
        assert!(!state.apply_to_all);
    }

    #[test]
    fn test_choose() {
        let mut state = ConflictDialogState::default();
        assert!(state.chosen_resolution.is_none());
        state.choose(ConflictResolution::Replace);
        assert_eq!(state.chosen_resolution, Some(ConflictResolution::Replace));
    }

    #[test]
    fn test_source_filename() {
        let state = ConflictDialogState::new(
            "/home/user/documents/file.txt".to_string(),
            "/backup/file.txt".to_string(),
            0, 0, String::new(), String::new(),
        );
        assert_eq!(state.source_filename(), "file.txt");
    }

    #[test]
    fn test_dest_filename() {
        let state = ConflictDialogState::new(
            "/src/file.txt".to_string(),
            "/backup/archive/file.txt".to_string(),
            0, 0, String::new(), String::new(),
        );
        assert_eq!(state.dest_filename(), "file.txt");
    }

    #[test]
    fn test_format_size() {
        assert_eq!(ConflictDialogState::format_size(500), "500 bytes");
        assert_eq!(ConflictDialogState::format_size(1_500), "1.50 KB");
        assert_eq!(ConflictDialogState::format_size(1_500_000), "1.50 MB");
        assert_eq!(ConflictDialogState::format_size(1_500_000_000), "1.50 GB");
    }
}

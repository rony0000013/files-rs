//! Quick Jump Dialog — keyboard-driven path navigation (Ctrl+G style).
//! Per TODO.md Section 6.11 - QuickJumpDialog for fast path navigation.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Represents the state of the quick jump dialog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuickJumpState {
    /// Dialog is hidden.
    Closed,
    /// Dialog is open and ready for input.
    Open {
        /// Current input text.
        input: String,
        /// Whether the input is valid.
        is_valid: bool,
    },
}

/// Configuration for the quick jump dialog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuickJumpConfig {
    /// Maximum number of history entries to keep.
    pub max_history: usize,
    /// Whether to validate paths as they are typed.
    pub validate_on_input: bool,
    /// Whether to auto-complete matching paths.
    pub auto_complete: bool,
    /// Whether to show hidden files in suggestions.
    pub show_hidden: bool,
}

impl Default for QuickJumpConfig {
    fn default() -> Self {
        Self {
            max_history: 20,
            validate_on_input: true,
            auto_complete: true,
            show_hidden: false,
        }
    }
}

/// Represents a history entry for quick jump.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuickJumpHistoryEntry {
    /// The path that was jumped to.
    pub path: PathBuf,
    /// Timestamp when the jump occurred.
    pub timestamp: Instant,
    /// Number of times this entry has been used.
    pub use_count: usize,
}

impl QuickJumpHistoryEntry {
    /// Create a new history entry.
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            timestamp: Instant::now(),
            use_count: 1,
        }
    }

    /// Record another use of this entry.
    pub fn increment_use(&mut self) {
        self.use_count += 1;
    }

    /// Get the age of this entry.
    pub fn age(&self) -> Duration {
        self.timestamp.elapsed()
    }
}

/// Service for managing the quick jump dialog state and history.
#[derive(Debug, Clone)]
pub struct QuickJumpDialog {
    /// Current dialog state.
    state: QuickJumpState,
    /// Configuration.
    config: QuickJumpConfig,
    /// Jump history.
    history: Vec<QuickJumpHistoryEntry>,
    /// Whether the dialog was just closed (for focus management).
    was_just_closed: bool,
}

impl QuickJumpDialog {
    /// Create a new quick jump dialog with default config.
    pub fn new() -> Self {
        Self {
            state: QuickJumpState::Closed,
            config: QuickJumpConfig::default(),
            history: Vec::new(),
            was_just_closed: false,
        }
    }

    /// Create with custom configuration.
    pub fn with_config(config: QuickJumpConfig) -> Self {
        Self {
            state: QuickJumpState::Closed,
            config,
            history: Vec::new(),
            was_just_closed: false,
        }
    }

    /// Get the current state.
    pub fn state(&self) -> &QuickJumpState {
        &self.state
    }

    /// Check if dialog is open.
    pub fn is_open(&self) -> bool {
        matches!(self.state, QuickJumpState::Open { .. })
    }

    /// Get the current input text if open.
    pub fn input(&self) -> Option<&str> {
        match &self.state {
            QuickJumpState::Closed => None,
            QuickJumpState::Open { input, .. } => Some(input),
        }
    }

    /// Get the validation status if open.
    pub fn is_valid(&self) -> Option<bool> {
        match &self.state {
            QuickJumpState::Closed => None,
            QuickJumpState::Open { is_valid, .. } => Some(*is_valid),
        }
    }

    /// Open the dialog with optional initial input.
    pub fn open(&mut self, initial_input: Option<String>) {
        let input = initial_input.unwrap_or_default();
        self.state = QuickJumpState::Open {
            input,
            is_valid: false,
        };
        self.was_just_closed = false;
    }

    /// Close the dialog.
    pub fn close(&mut self) {
        self.state = QuickJumpState::Closed;
        self.was_just_closed = true;
    }

    /// Check if dialog was just closed (for focus management).
    pub fn was_just_closed(&self) -> bool {
        self.was_just_closed
    }

    /// Clear the "just closed" flag.
    pub fn clear_just_closed_flag(&mut self) {
        self.was_just_closed = false;
    }

    /// Update the input text.
    pub fn set_input(&mut self, input: String) {
        match &mut self.state {
            QuickJumpState::Closed => {}
            QuickJumpState::Open {
                input: current_input,
                is_valid,
            } => {
                *current_input = input;
                // Validation will be done by caller via validate_input
                *is_valid = false;
            }
        }
    }

    /// Validate the current input and update state.
    pub fn validate_input(&mut self) {
        match &mut self.state {
            QuickJumpState::Closed => {}
            QuickJumpState::Open { input, is_valid } => {
                let path_str = input.clone();
                *is_valid = Self::is_valid_path(&path_str);
            }
        }
    }

    /// Check if a path is valid (exists or can be created).
    fn is_valid_path(path: &str) -> bool {
        if path.is_empty() {
            return false;
        }

        let path_buf = PathBuf::from(path);

        // Check if path exists
        if path_buf.exists() {
            return true;
        }

        // Check if parent exists (for new paths)
        if let Some(parent) = path_buf.parent() {
            return parent.exists();
        }

        false
    }

    /// Execute the jump to the current input.
    /// Returns the path if valid and jump was executed.
    pub fn execute_jump(&mut self) -> Option<PathBuf> {
        match &self.state {
            QuickJumpState::Closed => None,
            QuickJumpState::Open { input, is_valid } => {
                if !is_valid {
                    return None;
                }

                let path = PathBuf::from(input);
                if path.exists()
                    || PathBuf::from(input)
                        .parent()
                        .map(|p| p.exists())
                        .unwrap_or(false)
                {
                    // Add to history
                    self.add_to_history(path.clone());

                    // Close dialog
                    self.close();

                    Some(path)
                } else {
                    None
                }
            }
        }
    }

    /// Add a path to history.
    pub fn add_to_history(&mut self, path: PathBuf) {
        // Check if path already exists in history
        if let Some(existing) = self.history.iter_mut().find(|e| e.path == path) {
            existing.increment_use();
            existing.timestamp = Instant::now();
            return;
        }

        // Add new entry
        let entry = QuickJumpHistoryEntry::new(path);
        self.history.push(entry);

        // Trim history if needed
        while self.history.len() > self.config.max_history {
            self.history.remove(0);
        }
    }

    /// Get the jump history.
    pub fn history(&self) -> &[QuickJumpHistoryEntry] {
        &self.history
    }

    /// Get the most recently used entries.
    pub fn recent_entries(&self, count: usize) -> Vec<&QuickJumpHistoryEntry> {
        let mut entries: Vec<&QuickJumpHistoryEntry> = self.history.iter().collect();
        entries.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        entries.truncate(count);
        entries
    }

    /// Get the most frequently used entries.
    pub fn frequent_entries(&self, count: usize) -> Vec<&QuickJumpHistoryEntry> {
        let mut entries: Vec<&QuickJumpHistoryEntry> = self.history.iter().collect();
        entries.sort_by(|a, b| b.use_count.cmp(&a.use_count));
        entries.truncate(count);
        entries
    }

    /// Clear the jump history.
    pub fn clear_history(&mut self) {
        self.history.clear();
    }

    /// Remove a specific entry from history.
    pub fn remove_from_history(&mut self, path: &Path) {
        self.history.retain(|e| &e.path != path);
    }

    /// Navigate up one directory level.
    pub fn navigate_up(&mut self) {
        match &mut self.state {
            QuickJumpState::Closed => {}
            QuickJumpState::Open { input, .. } => {
                let input_str: &str = input.as_str();
                let path = PathBuf::from(input_str);
                if let Some(parent) = path.parent() {
                    *input = parent.to_string_lossy().to_string();
                }
            }
        }
    }

    /// Navigate to the parent of current path.
    pub fn navigate_to_parent(&mut self) {
        self.navigate_up();
    }

    /// Toggle between showing hidden files.
    pub fn toggle_show_hidden(&mut self) {
        self.config.show_hidden = !self.config.show_hidden;
    }

    /// Get the configuration.
    pub fn config(&self) -> &QuickJumpConfig {
        &self.config
    }

    /// Update configuration.
    pub fn set_config(&mut self, config: QuickJumpConfig) {
        self.config = config;

        // Trim history if needed
        while self.history.len() > self.config.max_history {
            self.history.remove(0);
        }
    }
}

impl Default for QuickJumpDialog {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quick_jump_dialog_new() {
        let dialog = QuickJumpDialog::new();
        assert!(!dialog.is_open());
        assert!(dialog.state().is_closed());
        assert_eq!(dialog.input(), None);
    }

    #[test]
    fn test_quick_jump_dialog_open() {
        let mut dialog = QuickJumpDialog::new();
        dialog.open(None);

        assert!(dialog.is_open());
        assert_eq!(dialog.input(), Some(""));
        assert_eq!(dialog.is_valid(), Some(false));
    }

    #[test]
    fn test_quick_jump_dialog_open_with_initial() {
        let mut dialog = QuickJumpDialog::new();
        dialog.open(Some("C:\\Users".to_string()));

        assert!(dialog.is_open());
        assert_eq!(dialog.input(), Some("C:\\Users"));
    }

    #[test]
    fn test_quick_jump_dialog_close() {
        let mut dialog = QuickJumpDialog::new();
        dialog.open(None);
        assert!(dialog.is_open());

        dialog.close();
        assert!(!dialog.is_open());
        assert!(dialog.was_just_closed());
    }

    #[test]
    fn test_quick_jump_dialog_clear_just_closed() {
        let mut dialog = QuickJumpDialog::new();
        dialog.open(None);
        dialog.close();
        assert!(dialog.was_just_closed());

        dialog.clear_just_closed_flag();
        assert!(!dialog.was_just_closed());
    }

    #[test]
    fn test_quick_jump_dialog_set_input() {
        let mut dialog = QuickJumpDialog::new();
        dialog.open(None);

        dialog.set_input("C:\\Test".to_string());
        assert_eq!(dialog.input(), Some("C:\\Test"));
    }

    #[test]
    fn test_quick_jump_config_default() {
        let config = QuickJumpConfig::default();
        assert_eq!(config.max_history, 20);
        assert!(config.validate_on_input);
        assert!(config.auto_complete);
        assert!(!config.show_hidden);
    }

    #[test]
    fn test_quick_jump_state_variants() {
        let closed = QuickJumpState::Closed;
        assert!(closed.is_closed());

        let open = QuickJumpState::Open {
            input: "test".to_string(),
            is_valid: true,
        };
        assert!(!open.is_closed());
    }

    #[test]
    fn test_quick_jump_history_entry_new() {
        let entry = QuickJumpHistoryEntry::new(PathBuf::from("C:\\test"));
        assert_eq!(entry.path, PathBuf::from("C:\\test"));
        assert_eq!(entry.use_count, 1);
    }

    #[test]
    fn test_quick_jump_history_entry_increment_use() {
        let mut entry = QuickJumpHistoryEntry::new(PathBuf::from("C:\\test"));
        entry.increment_use();
        assert_eq!(entry.use_count, 2);
        entry.increment_use();
        assert_eq!(entry.use_count, 3);
    }

    #[test]
    fn test_quick_jump_history_entry_age() {
        let entry = QuickJumpHistoryEntry::new(PathBuf::from("C:\\test"));
        let age = entry.age();
        // Age should be very small (close to 0) since we just created it
        assert!(age.as_secs() == 0 || age.as_millis() < 100);
    }

    #[test]
    fn test_quick_jump_dialog_add_to_history() {
        let mut dialog = QuickJumpDialog::new();
        dialog.add_to_history(PathBuf::from("C:\\test1"));
        dialog.add_to_history(PathBuf::from("C:\\test2"));

        assert_eq!(dialog.history().len(), 2);
    }

    #[test]
    fn test_quick_jump_dialog_add_duplicate_to_history() {
        let mut dialog = QuickJumpDialog::new();
        dialog.add_to_history(PathBuf::from("C:\\test"));
        dialog.add_to_history(PathBuf::from("C:\\test"));

        // Should only have one entry
        assert_eq!(dialog.history().len(), 1);
        // But use count should be 2
        assert_eq!(dialog.history()[0].use_count, 2);
    }

    #[test]
    fn test_quick_jump_dialog_history_limit() {
        let config = QuickJumpConfig {
            max_history: 3,
            ..Default::default()
        };
        let mut dialog = QuickJumpDialog::with_config(config);

        dialog.add_to_history(PathBuf::from("C:\\test1"));
        dialog.add_to_history(PathBuf::from("C:\\test2"));
        dialog.add_to_history(PathBuf::from("C:\\test3"));
        dialog.add_to_history(PathBuf::from("C:\\test4"));

        // Should only have 3 entries
        assert_eq!(dialog.history().len(), 3);
        // First entry should have been removed
        assert!(!dialog
            .history()
            .iter()
            .any(|e| e.path == PathBuf::from("C:\\test1")));
    }

    #[test]
    fn test_quick_jump_dialog_recent_entries() {
        let mut dialog = QuickJumpDialog::new();
        dialog.add_to_history(PathBuf::from("C:\\test1"));
        dialog.add_to_history(PathBuf::from("C:\\test2"));
        dialog.add_to_history(PathBuf::from("C:\\test3"));

        let recent = dialog.recent_entries(2);
        assert_eq!(recent.len(), 2);
        // Most recent should be first
        assert_eq!(recent[0].path, PathBuf::from("C:\\test3"));
    }

    #[test]
    fn test_quick_jump_dialog_frequent_entries() {
        let mut dialog = QuickJumpDialog::new();
        dialog.add_to_history(PathBuf::from("C:\\test1"));
        dialog.add_to_history(PathBuf::from("C:\\test2"));
        dialog.add_to_history(PathBuf::from("C:\\test1")); // test1 used twice

        let frequent = dialog.frequent_entries(2);
        assert_eq!(frequent.len(), 2);
        // Most frequent should be first
        assert_eq!(frequent[0].path, PathBuf::from("C:\\test1"));
    }

    #[test]
    fn test_quick_jump_dialog_clear_history() {
        let mut dialog = QuickJumpDialog::new();
        dialog.add_to_history(PathBuf::from("C:\\test1"));
        dialog.add_to_history(PathBuf::from("C:\\test2"));

        assert_eq!(dialog.history().len(), 2);

        dialog.clear_history();
        assert_eq!(dialog.history().len(), 0);
    }

    #[test]
    fn test_quick_jump_dialog_remove_from_history() {
        let mut dialog = QuickJumpDialog::new();
        dialog.add_to_history(PathBuf::from("C:\\test1"));
        dialog.add_to_history(PathBuf::from("C:\\test2"));

        dialog.remove_from_history(Path::new("C:\\test1"));

        assert_eq!(dialog.history().len(), 1);
        assert!(!dialog
            .history()
            .iter()
            .any(|e| e.path == PathBuf::from("C:\\test1")));
    }

    #[test]
    fn test_quick_jump_dialog_navigate_up() {
        let mut dialog = QuickJumpDialog::new();
        // Use forward slashes for cross-platform compatibility
        dialog.open(Some("/home/user/test".to_string()));
        dialog.set_input("/home/user/test".to_string());

        dialog.navigate_up();
        assert_eq!(dialog.input(), Some("/home/user"));
    }

    #[test]
    fn test_quick_jump_dialog_toggle_show_hidden() {
        let mut dialog = QuickJumpDialog::new();
        assert!(!dialog.config.show_hidden);

        dialog.toggle_show_hidden();
        assert!(dialog.config.show_hidden);

        dialog.toggle_show_hidden();
        assert!(!dialog.config.show_hidden);
    }

    #[test]
    fn test_quick_jump_dialog_set_config() {
        let mut dialog = QuickJumpDialog::new();
        dialog.add_to_history(PathBuf::from("C:\\test1"));

        let new_config = QuickJumpConfig {
            max_history: 1,
            ..Default::default()
        };
        dialog.set_config(new_config);

        assert_eq!(dialog.config().max_history, 1);
        // History should be trimmed
        assert_eq!(dialog.history().len(), 1);
    }

    #[test]
    fn test_quick_jump_dialog_validate_input_empty() {
        let mut dialog = QuickJumpDialog::new();
        dialog.open(None);
        dialog.set_input("".to_string());
        dialog.validate_input();

        // Empty input should not be valid
        assert!(!dialog.is_valid().unwrap());
    }
}

// Helper trait for QuickJumpState
trait QuickJumpStateExt {
    fn is_closed(&self) -> bool;
}

impl QuickJumpStateExt for QuickJumpState {
    fn is_closed(&self) -> bool {
        matches!(self, QuickJumpState::Closed)
    }
}

//! File Too Large Dialog Service
//! State machine for FileTooLargeDialog.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// FileTooLargeDialog options
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileTooLargeAction {
    OpenAnyway,
    Cancel,
}

impl Default for FileTooLargeAction {
    fn default() -> Self {
        Self::Cancel
    }
}

/// State for FileTooLargeDialog
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileTooLargeState {
    /// File path
    pub path: PathBuf,
    /// File size in bytes
    pub file_size: u64,
    /// Size limit in bytes (e.g., 10MB = 10 * 1024 * 1024)
    pub size_limit: u64,
    /// Selected action
    pub action: FileTooLargeAction,
    /// Don't ask again for this session
    pub dont_ask_again: bool,
}

impl Default for FileTooLargeState {
    fn default() -> Self {
        Self {
            path: PathBuf::new(),
            file_size: 0,
            size_limit: 10 * 1024 * 1024, // Default 10MB
            action: FileTooLargeAction::default(),
            dont_ask_again: false,
        }
    }
}

impl FileTooLargeState {
    /// Create a new state
    pub fn new(path: PathBuf, file_size: u64, limit: u64) -> Self {
        Self {
            path,
            file_size,
            size_limit: limit,
            action: FileTooLargeAction::default(),
            dont_ask_again: false,
        }
    }

    /// Set the action
    pub fn set_action(&mut self, action: FileTooLargeAction) {
        self.action = action;
    }

    /// Toggle dont ask again
    pub fn toggle_dont_ask_again(&mut self) {
        self.dont_ask_again = !self.dont_ask_again;
    }

    /// Get file size formatted
    pub fn formatted_file_size(&self) -> String {
        bytesize::ByteSize(self.file_size).to_string()
    }

    /// Get size limit formatted
    pub fn formatted_limit(&self) -> String {
        bytesize::ByteSize(self.size_limit).to_string()
    }

    /// Check if the action is to open anyway
    pub fn should_open(&self) -> bool {
        self.action == FileTooLargeAction::OpenAnyway
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_too_large_action_default() {
        assert_eq!(FileTooLargeAction::default(), FileTooLargeAction::Cancel);
    }

    #[test]
    fn test_file_too_large_state_default() {
        let state = FileTooLargeState::default();
        assert!(state.path.as_os_str().is_empty());
        assert_eq!(state.file_size, 0);
        assert_eq!(state.size_limit, 10 * 1024 * 1024);
        assert_eq!(state.action, FileTooLargeAction::Cancel);
        assert!(!state.dont_ask_again);
    }

    #[test]
    fn test_file_too_large_state_new() {
        let path = PathBuf::from("C:\\test\\large_file.bin");
        let state = FileTooLargeState::new(path.clone(), 100_000_000, 10_000_000);
        assert_eq!(state.path, path);
        assert_eq!(state.file_size, 100_000_000);
        assert_eq!(state.size_limit, 10_000_000);
    }

    #[test]
    fn test_file_too_large_state_set_action() {
        let mut state = FileTooLargeState::default();
        state.set_action(FileTooLargeAction::OpenAnyway);
        assert_eq!(state.action, FileTooLargeAction::OpenAnyway);
        state.set_action(FileTooLargeAction::Cancel);
        assert_eq!(state.action, FileTooLargeAction::Cancel);
    }

    #[test]
    fn test_file_too_large_state_toggle_dont_ask_again() {
        let mut state = FileTooLargeState::default();
        assert!(!state.dont_ask_again);
        state.toggle_dont_ask_again();
        assert!(state.dont_ask_again);
        state.toggle_dont_ask_again();
        assert!(!state.dont_ask_again);
    }

    #[test]
    fn test_file_too_large_state_formatted_file_size() {
        let mut state = FileTooLargeState::default();
        state.file_size = 1024 * 1024;
        assert_eq!(state.formatted_file_size(), "1.0 MiB");
        state.file_size = 1024 * 1024 * 1024;
        assert_eq!(state.formatted_file_size(), "1.0 GiB");
    }

    #[test]
    fn test_file_too_large_state_formatted_limit() {
        let mut state = FileTooLargeState::default();
        state.size_limit = 10 * 1024 * 1024;
        assert_eq!(state.formatted_limit(), "10.0 MiB");
    }

    #[test]
    fn test_file_too_large_state_should_open() {
        let mut state = FileTooLargeState::default();
        assert!(!state.should_open());

        state.set_action(FileTooLargeAction::OpenAnyway);
        assert!(state.should_open());

        state.set_action(FileTooLargeAction::Cancel);
        assert!(!state.should_open());
    }
}

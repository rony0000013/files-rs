//! Clipboard Manager — state management for cut/copy/paste operations.
//! Per DESIGN_SPEC.md ActionContext.clipboard usage and TODO.md Section 6.10.

use std::path::PathBuf;
use std::time::{Duration, Instant};

/// Represents the type of clipboard operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipboardOperation {
    /// Items were cut (will be moved on paste).
    Cut,
    /// Items were copied (will be duplicated on paste).
    Copy,
}

/// Represents an item stored in the clipboard.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipboardItem {
    /// The path to the item.
    pub path: PathBuf,
    /// Original path for cut operations (used for move semantics).
    pub original_path: PathBuf,
}

impl ClipboardItem {
    /// Create a new clipboard item.
    pub fn new(path: PathBuf) -> Self {
        Self {
            original_path: path.clone(),
            path,
        }
    }
}

/// Represents the state of the clipboard.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClipboardState {
    /// Clipboard is empty.
    Empty,
    /// Clipboard contains items from a cut operation.
    Cut(Vec<ClipboardItem>),
    /// Clipboard contains items from a copy operation.
    Copy(Vec<ClipboardItem>),
}

impl Default for ClipboardState {
    fn default() -> Self {
        Self::Empty
    }
}

/// Configuration for the clipboard manager.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipboardConfig {
    /// Maximum number of items allowed in clipboard.
    pub max_items: usize,
    /// Whether to support cut operation.
    pub allow_cut: bool,
    /// Whether to support copy operation.
    pub allow_copy: bool,
    /// Whether to support paste operation.
    pub allow_paste: bool,
}

impl Default for ClipboardConfig {
    fn default() -> Self {
        Self {
            max_items: 100,
            allow_cut: true,
            allow_copy: true,
            allow_paste: true,
        }
    }
}

/// Service for managing clipboard operations in the file manager.
/// Tracks cut/copy items and provides paste functionality.
#[derive(Debug, Clone)]
pub struct ClipboardManager {
    /// Current clipboard state.
    state: ClipboardState,
    /// Configuration.
    config: ClipboardConfig,
    /// Timestamp when items were added.
    timestamp: Option<Instant>,
}

impl ClipboardManager {
    /// Create a new clipboard manager with default config.
    pub fn new() -> Self {
        Self {
            state: ClipboardState::Empty,
            config: ClipboardConfig::default(),
            timestamp: None,
        }
    }

    /// Create with custom configuration.
    pub fn with_config(config: ClipboardConfig) -> Self {
        Self {
            state: ClipboardState::Empty,
            config,
            timestamp: None,
        }
    }

    /// Get the current clipboard state.
    pub fn state(&self) -> &ClipboardState {
        &self.state
    }

    /// Get the operation type if clipboard has items.
    pub fn operation(&self) -> Option<ClipboardOperation> {
        match &self.state {
            ClipboardState::Empty => None,
            ClipboardState::Cut(_) => Some(ClipboardOperation::Cut),
            ClipboardState::Copy(_) => Some(ClipboardOperation::Copy),
        }
    }

    /// Check if clipboard is empty.
    pub fn is_empty(&self) -> bool {
        matches!(self.state, ClipboardState::Empty)
    }

    /// Get the number of items in clipboard.
    pub fn len(&self) -> usize {
        match &self.state {
            ClipboardState::Empty => 0,
            ClipboardState::Cut(items) | ClipboardState::Copy(items) => items.len(),
        }
    }

    /// Check if clipboard has items.
    pub fn has_items(&self) -> bool {
        !self.is_empty()
    }

    /// Add items to clipboard with a cut operation.
    pub fn cut(&mut self, paths: Vec<PathBuf>) {
        if !self.config.allow_cut {
            return;
        }

        let items: Vec<ClipboardItem> = paths
            .into_iter()
            .take(self.config.max_items)
            .map(ClipboardItem::new)
            .collect();

        self.state = ClipboardState::Cut(items);
        self.timestamp = Some(Instant::now());
    }

    /// Add items to clipboard with a copy operation.
    pub fn copy(&mut self, paths: Vec<PathBuf>) {
        if !self.config.allow_copy {
            return;
        }

        let items: Vec<ClipboardItem> = paths
            .into_iter()
            .take(self.config.max_items)
            .map(ClipboardItem::new)
            .collect();

        self.state = ClipboardState::Copy(items);
        self.timestamp = Some(Instant::now());
    }

    /// Get the items from clipboard for paste operation.
    /// Returns None if clipboard is empty.
    pub fn get_items(&self) -> Option<Vec<ClipboardItem>> {
        match &self.state {
            ClipboardState::Empty => None,
            ClipboardState::Cut(items) => Some(items.clone()),
            ClipboardState::Copy(items) => Some(items.clone()),
        }
    }

    /// Get clipboard items as paths.
    pub fn get_paths(&self) -> Vec<PathBuf> {
        self.get_items()
            .map(|items| items.into_iter().map(|i| i.path).collect())
            .unwrap_or_default()
    }

    /// Clear the clipboard.
    pub fn clear(&mut self) {
        self.state = ClipboardState::Empty;
        self.timestamp = None;
    }

    /// Get the age of clipboard items.
    pub fn age(&self) -> Option<Duration> {
        self.timestamp.map(|t| t.elapsed())
    }

    /// Check if clipboard has items from a cut operation.
    pub fn is_cut(&self) -> bool {
        matches!(self.state, ClipboardState::Cut(_))
    }

    /// Check if clipboard has items from a copy operation.
    pub fn is_copy(&self) -> bool {
        matches!(self.state, ClipboardState::Copy(_))
    }

    /// Remove specific items from clipboard.
    pub fn remove_items(&mut self, paths: &[PathBuf]) {
        match &mut self.state {
            ClipboardState::Empty => {}
            ClipboardState::Cut(items) | ClipboardState::Copy(items) => {
                items.retain(|item| !paths.contains(&item.path));
                if items.is_empty() {
                    self.state = ClipboardState::Empty;
                    self.timestamp = None;
                }
            }
        }
    }

    /// Check if a specific path is in the clipboard.
    pub fn contains(&self, path: &PathBuf) -> bool {
        match &self.state {
            ClipboardState::Empty => false,
            ClipboardState::Cut(items) | ClipboardState::Copy(items) => {
                items.iter().any(|item| &item.path == path)
            }
        }
    }
}

impl Default for ClipboardManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clipboard_manager_new() {
        let manager = ClipboardManager::new();
        assert!(manager.is_empty());
        assert_eq!(manager.len(), 0);
        assert!(manager.operation().is_none());
    }

    #[test]
    fn test_clipboard_manager_cut() {
        let mut manager = ClipboardManager::new();
        let paths = vec![
            PathBuf::from("C:\\test\\file1.txt"),
            PathBuf::from("C:\\test\\file2.txt"),
        ];

        manager.cut(paths.clone());

        assert!(manager.has_items());
        assert_eq!(manager.len(), 2);
        assert!(manager.is_cut());
        assert!(!manager.is_copy());
        assert_eq!(manager.operation(), Some(ClipboardOperation::Cut));
    }

    #[test]
    fn test_clipboard_manager_copy() {
        let mut manager = ClipboardManager::new();
        let paths = vec![
            PathBuf::from("C:\\test\\file1.txt"),
            PathBuf::from("C:\\test\\file2.txt"),
        ];

        manager.copy(paths.clone());

        assert!(manager.has_items());
        assert_eq!(manager.len(), 2);
        assert!(!manager.is_cut());
        assert!(manager.is_copy());
        assert_eq!(manager.operation(), Some(ClipboardOperation::Copy));
    }

    #[test]
    fn test_clipboard_manager_clear() {
        let mut manager = ClipboardManager::new();
        let paths = vec![PathBuf::from("C:\\test\\file1.txt")];

        manager.copy(paths);
        assert!(manager.has_items());

        manager.clear();

        assert!(manager.is_empty());
        assert!(manager.operation().is_none());
    }

    #[test]
    fn test_clipboard_manager_get_items() {
        let mut manager = ClipboardManager::new();
        let paths = vec![
            PathBuf::from("C:\\test\\file1.txt"),
            PathBuf::from("C:\\test\\file2.txt"),
        ];

        manager.copy(paths.clone());

        let items = manager.get_items().unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].path, paths[0]);
        assert_eq!(items[1].path, paths[1]);
    }

    #[test]
    fn test_clipboard_manager_get_items_empty() {
        let manager = ClipboardManager::new();
        assert!(manager.get_items().is_none());
    }

    #[test]
    fn test_clipboard_manager_get_paths() {
        let mut manager = ClipboardManager::new();
        let paths = vec![
            PathBuf::from("C:\\test\\file1.txt"),
            PathBuf::from("C:\\test\\file2.txt"),
        ];

        manager.copy(paths.clone());

        let result = manager.get_paths();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], paths[0]);
        assert_eq!(result[1], paths[1]);
    }

    #[test]
    fn test_clipboard_manager_remove_items() {
        let mut manager = ClipboardManager::new();
        let paths = vec![
            PathBuf::from("C:\\test\\file1.txt"),
            PathBuf::from("C:\\test\\file2.txt"),
            PathBuf::from("C:\\test\\file3.txt"),
        ];

        manager.copy(paths.clone());
        assert_eq!(manager.len(), 3);

        manager.remove_items(&[paths[1].clone()]);
        assert_eq!(manager.len(), 2);

        let remaining = manager.get_paths();
        assert!(!remaining.contains(&paths[1]));
        assert!(remaining.contains(&paths[0]));
        assert!(remaining.contains(&paths[2]));
    }

    #[test]
    fn test_clipboard_manager_remove_all() {
        let mut manager = ClipboardManager::new();
        let paths = vec![PathBuf::from("C:\\test\\file1.txt")];

        manager.copy(paths);
        assert!(manager.has_items());

        manager.remove_items(&[PathBuf::from("C:\\test\\file1.txt")]);
        assert!(manager.is_empty());
    }

    #[test]
    fn test_clipboard_manager_contains() {
        let mut manager = ClipboardManager::new();
        let path1 = PathBuf::from("C:\\test\\file1.txt");
        let path2 = PathBuf::from("C:\\test\\file2.txt");

        manager.copy(vec![path1.clone()]);

        assert!(manager.contains(&path1));
        assert!(!manager.contains(&path2));
    }

    #[test]
    fn test_clipboard_manager_age() {
        let mut manager = ClipboardManager::new();
        assert!(manager.age().is_none());

        manager.copy(vec![PathBuf::from("C:\\test\\file1.txt")]);
        assert!(manager.age().is_some());

        // Age should be at least 0 (may be slightly more due to time)
        let age = manager.age().unwrap();
        assert!(age.as_secs() == 0 || age.as_millis() >= 0);
    }

    #[test]
    fn test_clipboard_config_default() {
        let config = ClipboardConfig::default();
        assert!(config.allow_cut);
        assert!(config.allow_copy);
        assert!(config.allow_paste);
        assert_eq!(config.max_items, 100);
    }

    #[test]
    fn test_clipboard_config_max_items() {
        let config = ClipboardConfig {
            max_items: 5,
            ..Default::default()
        };
        let mut manager = ClipboardManager::with_config(config);

        let paths: Vec<PathBuf> = (0..10)
            .map(|i| PathBuf::from(format!("C:\\test\\file{}.txt", i)))
            .collect();

        manager.copy(paths);
        assert_eq!(manager.len(), 5);
    }

    #[test]
    fn test_clipboard_config_disable_cut() {
        let config = ClipboardConfig {
            allow_cut: false,
            ..Default::default()
        };
        let mut manager = ClipboardManager::with_config(config);

        manager.cut(vec![PathBuf::from("C:\\test\\file1.txt")]);
        assert!(manager.is_empty());

        // Copy should still work
        manager.copy(vec![PathBuf::from("C:\\test\\file2.txt")]);
        assert!(manager.has_items());
    }

    #[test]
    fn test_clipboard_config_disable_copy() {
        let config = ClipboardConfig {
            allow_copy: false,
            ..Default::default()
        };
        let mut manager = ClipboardManager::with_config(config);

        manager.copy(vec![PathBuf::from("C:\\test\\file1.txt")]);
        assert!(manager.is_empty());

        // Cut should still work
        manager.cut(vec![PathBuf::from("C:\\test\\file2.txt")]);
        assert!(manager.has_items());
    }

    #[test]
    fn test_clipboard_item_new() {
        let path = PathBuf::from("C:\\test\\file.txt");
        let item = ClipboardItem::new(path.clone());

        assert_eq!(item.path, path);
        assert_eq!(item.original_path, path);
    }

    #[test]
    fn test_clipboard_state_default() {
        let state = ClipboardState::default();
        assert!(matches!(state, ClipboardState::Empty));
    }

    #[test]
    fn test_clipboard_operation_variants() {
        assert!(matches!(
            ClipboardOperation::Cut,
            ClipboardOperation::Cut
        ));
        assert!(matches!(
            ClipboardOperation::Copy,
            ClipboardOperation::Copy
        ));
    }

    #[test]
    fn test_clipboard_manager_multiple_operations() {
        let mut manager = ClipboardManager::new();

        manager.copy(vec![PathBuf::from("C:\\test\\file1.txt")]);
        assert!(manager.is_copy());

        // Another copy replaces the clipboard
        manager.copy(vec![PathBuf::from("C:\\test\\file2.txt")]);
        assert_eq!(manager.len(), 1);
        assert!(manager.is_copy());

        // Cut replaces the clipboard
        manager.cut(vec![PathBuf::from("C:\\test\\file3.txt")]);
        assert_eq!(manager.len(), 1);
        assert!(manager.is_cut());
    }
}
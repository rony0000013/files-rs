//! BreadcrumbBar Component — breadcrumb path display with drag/drop, overflow handling, and navigation memory.
//! Per TODO.md Section 6 BreadcrumbBar features.

use std::path::PathBuf;
use std::time::{Duration, Instant};

/// Represents a single breadcrumb segment in the path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BreadcrumbSegment {
    /// The full path this segment represents.
    pub path: PathBuf,
    /// Display label (folder name).
    pub label: String,
    /// Whether this segment is draggable.
    pub is_draggable: bool,
    /// Whether this is the last (current) segment.
    pub is_current: bool,
}

/// Configuration for breadcrumb overflow behavior.
#[derive(Debug, Clone, PartialEq)]
pub struct BreadcrumbOverflowConfig {
    /// Maximum width in pixels before truncation.
    pub max_width: f32,
    /// Minimum segments to always show (including root and current).
    pub min_visible_segments: usize,
    /// Whether to show ellipsis for hidden segments.
    pub show_ellipsis: bool,
}

impl Default for BreadcrumbOverflowConfig {
    fn default() -> Self {
        Self {
            max_width: 600.0,
            min_visible_segments: 2,
            show_ellipsis: true,
        }
    }
}

/// Navigation memory entry — tracks the last position in a directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavigationMemoryEntry {
    /// The directory path.
    pub path: PathBuf,
    /// The item index that was selected/focused when leaving.
    pub last_item_index: usize,
    /// The item name that was focused.
    pub last_item_name: String,
    /// Timestamp when this entry was recorded.
    pub timestamp: Instant,
}

impl NavigationMemoryEntry {
    pub fn new(path: PathBuf, item_index: usize, item_name: String) -> Self {
        Self {
            path,
            last_item_index: item_index,
            last_item_name: item_name,
            timestamp: Instant::now(),
        }
    }

    /// Get the age of this memory entry.
    pub fn age(&self) -> Duration {
        self.timestamp.elapsed()
    }
}

/// State for tracking navigation history to enable "go up and highlight" feature.
#[derive(Debug, Clone)]
pub struct NavigationMemory {
    /// Stack of previous navigation entries.
    entries: Vec<NavigationMemoryEntry>,
    /// Maximum entries to keep.
    max_entries: usize,
}

impl NavigationMemory {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            max_entries: 50,
        }
    }

    /// Record current position before navigating to a new path.
    pub fn record_position(&mut self, path: PathBuf, item_index: usize, item_name: String) {
        // Avoid duplicate consecutive entries
        if let Some(last) = self.entries.last() {
            if last.path == path && last.last_item_index == item_index {
                return;
            }
        }

        self.entries.push(NavigationMemoryEntry::new(path, item_index, item_name));

        // Trim if needed
        while self.entries.len() > self.max_entries {
            self.entries.remove(0);
        }
    }

    /// Get the last position before navigating to the given parent path.
    /// Returns the item index and name to highlight.
    pub fn get_last_position(&self, parent_path: &PathBuf) -> Option<(usize, String)> {
        self.entries
            .iter()
            .rev()
            .find(|e| &e.path == parent_path)
            .map(|e| (e.last_item_index, e.last_item_name.clone()))
    }

    /// Remove a specific path from memory.
    pub fn forget(&mut self, path: &PathBuf) {
        self.entries.retain(|e| &e.path != path);
    }

    /// Clear all navigation memory.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Get the number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Default for NavigationMemory {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of processing breadcrumb segments for display.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BreadcrumbDisplayResult {
    /// Segments to display (may be truncated).
    pub visible_segments: Vec<BreadcrumbSegment>,
    /// Whether there are hidden segments before the first visible one.
    pub has_hidden_before: bool,
    /// Whether there are hidden segments after the last visible one.
    pub has_hidden_after: bool,
}

/// Service for managing breadcrumb display and interactions.
#[derive(Debug, Clone)]
pub struct BreadcrumbBarService {
    pub config: BreadcrumbOverflowConfig,
}

impl BreadcrumbBarService {
    pub fn new() -> Self {
        Self {
            config: BreadcrumbOverflowConfig::default(),
        }
    }

    pub fn with_config(config: BreadcrumbOverflowConfig) -> Self {
        Self { config }
    }

    /// Build breadcrumb segments from a path.
    pub fn build_segments(&self, path: &PathBuf, _current_index: usize) -> Vec<BreadcrumbSegment> {
        let mut segments = Vec::new();

        // For root path, just return one segment
        if path == &PathBuf::from("/") || path.to_string_lossy() == "/" {
            return vec![BreadcrumbSegment {
                path: PathBuf::from("/"),
                label: "/".to_string(),
                is_draggable: false,
                is_current: true,
            }];
        }

        // Build ancestors (root to current)
        let mut ancestor = path.parent().map(|p| p.to_path_buf());
        while let Some(parent) = ancestor {
            // Stop at root
            if parent == PathBuf::from("/") {
                break;
            }
            
            let label = parent
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "/".to_string());

            segments.insert(0, BreadcrumbSegment {
                path: parent.clone(),
                label,
                is_draggable: true,
                is_current: false,
            });

            ancestor = parent.parent().map(|p| p.to_path_buf());
        }

        // Add root if not already included
        if segments.is_empty() || segments[0].path != PathBuf::from("/") {
            segments.insert(0, BreadcrumbSegment {
                path: PathBuf::from("/"),
                label: "/".to_string(),
                is_draggable: true,
                is_current: false,
            });
        }

        // Add the current segment
        let current_label = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path.to_string_lossy().to_string());

        segments.push(BreadcrumbSegment {
            path: path.clone(),
            label: current_label,
            is_draggable: false,
            is_current: true,
        });

        segments
    }

    /// Process segments for display with overflow handling.
    pub fn process_for_display(
        &self,
        segments: &[BreadcrumbSegment],
        available_width: f32,
    ) -> BreadcrumbDisplayResult {
        let config = &self.config;
        
        // If we have enough space, show all segments
        if available_width >= config.max_width || segments.len() <= config.min_visible_segments {
            return BreadcrumbDisplayResult {
                visible_segments: segments.to_vec(),
                has_hidden_before: false,
                has_hidden_after: false,
            };
        }

        // Calculate how many segments we can fit
        // Estimate ~100px per segment + chevron
        let estimated_per_segment = 100.0;
        let segments_to_show = (available_width / estimated_per_segment) as usize;
        let segments_to_show = segments_to_show.max(config.min_visible_segments).min(segments.len());
        
        // Always show first (root) and last (current) segments
        let mut visible: Vec<BreadcrumbSegment> = Vec::new();
        
        if segments_to_show >= 3 {
            // Show: first, ellipsis placeholder, last few
            visible.push(segments[0].clone());
            visible.push(BreadcrumbSegment {
                path: PathBuf::new(),
                label: "...".to_string(),
                is_draggable: false,
                is_current: false,
            });
            
            // Add remaining from the end
            let start_idx = segments.len() - (segments_to_show - 2);
            for i in start_idx..segments.len() {
                visible.push(segments[i].clone());
            }
            
            BreadcrumbDisplayResult {
                visible_segments: visible,
                has_hidden_before: true,
                has_hidden_after: false,
            }
        } else {
            // Show first and last only
            visible.push(segments[0].clone());
            visible.push(segments.last().unwrap().clone());
            
            BreadcrumbDisplayResult {
                visible_segments: visible,
                has_hidden_before: true,
                has_hidden_after: true,
            }
        }
    }

    /// Check if a path is a valid drop target for drag and drop.
    pub fn is_valid_drop_target(&self, path: &PathBuf, dropped_path: &PathBuf) -> bool {
        // Can't drop a folder into itself
        if dropped_path == path {
            return false;
        }
        // Can't drop parent INTO child (dropping /tmp into /tmp/subfolder)
        if path.starts_with(dropped_path) {
            return false;
        }
        // For drag and drop, we allow dropping into any directory path
        // that exists or whose parent exists (so it can be created)
        if path.exists() {
            return path.is_dir();
        }
        // Check if parent exists (for creating new folders)
        path.parent()
            .map(|p| p.exists() && p.is_dir())
            .unwrap_or(false)
    }

    /// Get the drop position indicator.
    pub fn get_drop_indicator(&self, target_path: &PathBuf) -> DropIndicator {
        DropIndicator::Below
    }
}

/// Indicator for where a drop would occur relative to a target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropIndicator {
    Above,
    Below,
    Onto,
}

/// Drag context for breadcrumb segments.
#[derive(Debug, Clone)]
pub struct BreadcrumbDragContext {
    /// The path being dragged.
    pub dragged_path: PathBuf,
    /// The segment index where drag started.
    pub source_index: usize,
    /// Current hover target (if any).
    pub hover_target: Option<PathBuf>,
    /// Whether drag is active.
    pub is_dragging: bool,
}

impl BreadcrumbDragContext {
    pub fn new(dragged_path: PathBuf, source_index: usize) -> Self {
        Self {
            dragged_path,
            source_index,
            hover_target: None,
            is_dragging: true,
        }
    }

    pub fn end(&mut self) {
        self.is_dragging = false;
        self.hover_target = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_breadcrumb_overflow_config_default() {
        let config = BreadcrumbOverflowConfig::default();
        assert_eq!(config.max_width, 600.0);
        assert_eq!(config.min_visible_segments, 2);
        assert!(config.show_ellipsis);
    }

    #[test]
    fn test_navigation_memory_new() {
        let memory = NavigationMemory::new();
        assert!(memory.is_empty());
        assert_eq!(memory.len(), 0);
    }

    #[test]
    fn test_navigation_memory_record_position() {
        let mut memory = NavigationMemory::new();
        memory.record_position(
            PathBuf::from("/home/user"),
            5,
            "Documents".to_string(),
        );
        
        assert_eq!(memory.len(), 1);
        let (idx, name) = memory.get_last_position(&PathBuf::from("/home/user")).unwrap();
        assert_eq!(idx, 5);
        assert_eq!(name, "Documents");
    }

    #[test]
    fn test_navigation_memory_no_duplicate_consecutive() {
        let mut memory = NavigationMemory::new();
        memory.record_position(
            PathBuf::from("/home/user"),
            5,
            "Documents".to_string(),
        );
        memory.record_position(
            PathBuf::from("/home/user"),
            5,
            "Documents".to_string(),
        );
        
        // Should only have one entry
        assert_eq!(memory.len(), 1);
    }

    #[test]
    fn test_navigation_memory_get_last_position_not_found() {
        let memory = NavigationMemory::new();
        let result = memory.get_last_position(&PathBuf::from("/nonexistent"));
        assert!(result.is_none());
    }

    #[test]
    fn test_navigation_memory_forget() {
        let mut memory = NavigationMemory::new();
        memory.record_position(
            PathBuf::from("/home/user"),
            5,
            "Documents".to_string(),
        );
        memory.forget(&PathBuf::from("/home/user"));
        
        assert!(memory.is_empty());
    }

    #[test]
    fn test_navigation_memory_clear() {
        let mut memory = NavigationMemory::new();
        memory.record_position(
            PathBuf::from("/home/user"),
            5,
            "Documents".to_string(),
        );
        memory.record_position(
            PathBuf::from("/home"),
            3,
            "user".to_string(),
        );
        
        memory.clear();
        assert!(memory.is_empty());
    }

    #[test]
    fn test_navigation_memory_entry_age() {
        let entry = NavigationMemoryEntry::new(
            PathBuf::from("/home/user"),
            5,
            "Documents".to_string(),
        );
        let age = entry.age();
        // Should be very small since we just created it
        assert!(age.as_secs() == 0 || age.as_millis() < 100);
    }

    #[test]
    fn test_breadcrumb_bar_service_new() {
        let service = BreadcrumbBarService::new();
        assert_eq!(service.config.max_width, 600.0);
    }

    #[test]
    fn test_breadcrumb_bar_service_build_segments() {
        let service = BreadcrumbBarService::new();
        let path = PathBuf::from("/home/user/Documents");
        let segments = service.build_segments(&path, 0);
        
        assert!(segments.len() >= 3);
        assert_eq!(segments.last().unwrap().label, "Documents");
        assert!(segments.last().unwrap().is_current);
    }

    #[test]
    fn test_breadcrumb_bar_service_build_segments_root() {
        let service = BreadcrumbBarService::new();
        let path = PathBuf::from("/");
        let segments = service.build_segments(&path, 0);
        
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].label, "/");
        assert!(segments[0].is_current);
    }

    #[test]
    fn test_breadcrumb_bar_service_process_for_display_full() {
        let service = BreadcrumbBarService::new();
        let segments = vec![
            BreadcrumbSegment {
                path: PathBuf::from("/"),
                label: "/".to_string(),
                is_draggable: true,
                is_current: false,
            },
            BreadcrumbSegment {
                path: PathBuf::from("/home"),
                label: "home".to_string(),
                is_draggable: true,
                is_current: false,
            },
            BreadcrumbSegment {
                path: PathBuf::from("/home/user"),
                label: "user".to_string(),
                is_draggable: true,
                is_current: true,
            },
        ];
        
        let result = service.process_for_display(&segments, 800.0);
        
        assert_eq!(result.visible_segments.len(), 3);
        assert!(!result.has_hidden_before);
        assert!(!result.has_hidden_after);
    }

    #[test]
    fn test_breadcrumb_bar_service_process_for_display_truncated() {
        let service = BreadcrumbBarService::new();
        let segments = vec![
            BreadcrumbSegment {
                path: PathBuf::from("/"),
                label: "/".to_string(),
                is_draggable: true,
                is_current: false,
            },
            BreadcrumbSegment {
                path: PathBuf::from("/home"),
                label: "home".to_string(),
                is_draggable: true,
                is_current: false,
            },
            BreadcrumbSegment {
                path: PathBuf::from("/home/user"),
                label: "user".to_string(),
                is_draggable: true,
                is_current: false,
            },
            BreadcrumbSegment {
                path: PathBuf::from("/home/user/Documents"),
                label: "Documents".to_string(),
                is_draggable: false,
                is_current: true,
            },
        ];
        
        // With small width, should truncate
        let result = service.process_for_display(&segments, 100.0);
        
        assert!(result.has_hidden_before);
    }

    #[test]
    fn test_breadcrumb_bar_service_is_valid_drop_target() {
        let service = BreadcrumbBarService::new();

        // Valid: dropping into different directory (use /tmp which exists on Linux)
        assert!(service.is_valid_drop_target(
            &PathBuf::from("/tmp"),
            &PathBuf::from("/tmp/old_folder"),
        ));

        // Invalid: dropping folder into itself
        assert!(!service.is_valid_drop_target(
            &PathBuf::from("/tmp"),
            &PathBuf::from("/tmp"),
        ));

        // Invalid: dropping parent into child
        assert!(!service.is_valid_drop_target(
            &PathBuf::from("/tmp/test"),
            &PathBuf::from("/tmp"),
        ));
    }

    #[test]
    fn test_breadcrumb_drag_context_new() {
        let ctx = BreadcrumbDragContext::new(PathBuf::from("/home/user"), 2);
        assert!(ctx.is_dragging);
        assert_eq!(ctx.source_index, 2);
        assert_eq!(ctx.dragged_path, PathBuf::from("/home/user"));
        assert!(ctx.hover_target.is_none());
    }

    #[test]
    fn test_breadcrumb_drag_context_end() {
        let mut ctx = BreadcrumbDragContext::new(PathBuf::from("/home/user"), 2);
        ctx.hover_target = Some(PathBuf::from("/home"));
        ctx.end();
        
        assert!(!ctx.is_dragging);
        assert!(ctx.hover_target.is_none());
    }

    #[test]
    fn test_breadcrumb_segment_properties() {
        let segment = BreadcrumbSegment {
            path: PathBuf::from("/home/user"),
            label: "user".to_string(),
            is_draggable: true,
            is_current: false,
        };
        
        assert!(segment.is_draggable);
        assert!(!segment.is_current);
    }

    #[test]
    fn test_drop_indicator_variants() {
        assert!(matches!(DropIndicator::Above, DropIndicator::Above));
        assert!(matches!(DropIndicator::Below, DropIndicator::Below));
        assert!(matches!(DropIndicator::Onto, DropIndicator::Onto));
    }
}
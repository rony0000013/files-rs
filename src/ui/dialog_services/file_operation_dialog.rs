// FilesystemOperationDialog - progress and conflict resolution for file operations
// Handles: Copy, Move, Delete, Rename operations with progress tracking and conflict resolution

use crate::ui::constants::UiConstants;
use gpui::*;

#[derive(Debug, Clone, PartialEq)]
pub enum OperationType {
    Copy,
    Move,
    Delete,
    Rename,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConflictResolution {
    Replace,
    Skip,
    RenameNew,
    RenameExisting,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConflictItem {
    pub source_path: String,
    pub dest_path: String,
    pub source_size: u64,
    pub dest_size: u64,
    pub source_modified: String,
    pub dest_modified: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FileOperationState {
    pub operation: OperationType,
    pub source_items: Vec<String>,
    pub dest_path: String,
    pub current_file: Option<String>,
    pub progress: Option<f32>,
    pub total_bytes: u64,
    pub processed_bytes: u64,
    pub speed_bps: u64,
    pub eta_seconds: u64,
    pub conflicts: Vec<ConflictItem>,
    pub current_conflict_index: usize,
    pub is_paused: bool,
    pub is_complete: bool,
    pub error_message: Option<String>,
}

impl Default for FileOperationState {
    fn default() -> Self {
        Self {
            operation: OperationType::Copy,
            source_items: Vec::new(),
            dest_path: String::new(),
            current_file: None,
            progress: None,
            total_bytes: 0,
            processed_bytes: 0,
            speed_bps: 0,
            eta_seconds: 0,
            conflicts: Vec::new(),
            current_conflict_index: 0,
            is_paused: false,
            is_complete: false,
            error_message: None,
        }
    }
}

impl FileOperationState {
    pub fn new(operation: OperationType, source_items: Vec<String>, dest_path: String) -> Self {
        Self {
            operation,
            source_items,
            dest_path,
            ..Default::default()
        }
    }

    pub fn update_progress(&mut self, current_file: String, processed: u64, total: u64) {
        self.current_file = Some(current_file);
        self.processed_bytes = processed;
        self.total_bytes = total;
        self.progress = if total > 0 {
            Some((processed as f32) / (total as f32) * 100.0)
        } else {
            None
        };
    }

    pub fn update_speed(&mut self, speed_bps: u64, eta_seconds: u64) {
        self.speed_bps = speed_bps;
        self.eta_seconds = eta_seconds;
    }

    pub fn add_conflict(&mut self, conflict: ConflictItem) {
        self.conflicts.push(conflict);
    }

    pub fn resolve_conflict(&mut self, resolution: ConflictResolution) {
        if self.current_conflict_index < self.conflicts.len() {
            // Mark conflict as resolved (in real impl, would track resolution)
            self.current_conflict_index += 1;
        }
    }

    pub fn current_conflict(&self) -> Option<&ConflictItem> {
        self.conflicts.get(self.current_conflict_index)
    }

    pub fn has_pending_conflicts(&self) -> bool {
        self.current_conflict_index < self.conflicts.len()
    }

    pub fn pause(&mut self) {
        self.is_paused = true;
    }

    pub fn resume(&mut self) {
        self.is_paused = false;
    }

    pub fn complete(&mut self) {
        self.is_complete = true;
        self.progress = Some(100.0);
    }

    pub fn set_error(&mut self, message: String) {
        self.error_message = Some(message);
    }

    pub fn formatted_speed(&self) -> String {
        if self.speed_bps > 1_000_000_000 {
            format!("{:.1} GB/s", self.speed_bps as f64 / 1_000_000_000.0)
        } else if self.speed_bps > 1_000_000 {
            format!("{:.1} MB/s", self.speed_bps as f64 / 1_000_000.0)
        } else if self.speed_bps > 1_000 {
            format!("{:.1} KB/s", self.speed_bps as f64 / 1_000.0)
        } else {
            format!("{} B/s", self.speed_bps)
        }
    }

    pub fn formatted_eta(&self) -> String {
        if self.eta_seconds == 0 {
            return "calculating...".to_string();
        }
        let minutes = self.eta_seconds / 60;
        let seconds = self.eta_seconds % 60;
        if minutes > 0 {
            format!("{} min {} sec remaining", minutes, seconds)
        } else {
            format!("{} sec remaining", seconds)
        }
    }

    pub fn progress_percentage(&self) -> f32 {
        self.progress.unwrap_or(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_operation_type_variants() {
        assert_eq!(OperationType::Copy, OperationType::Copy);
        assert_eq!(OperationType::Move, OperationType::Move);
        assert_eq!(OperationType::Delete, OperationType::Delete);
        assert_eq!(OperationType::Rename, OperationType::Rename);
    }

    #[test]
    fn test_conflict_resolution_variants() {
        assert_eq!(ConflictResolution::Replace, ConflictResolution::Replace);
        assert_eq!(ConflictResolution::Skip, ConflictResolution::Skip);
        assert_eq!(ConflictResolution::RenameNew, ConflictResolution::RenameNew);
        assert_eq!(ConflictResolution::RenameExisting, ConflictResolution::RenameExisting);
    }

    #[test]
    fn test_file_operation_state_new() {
        let state = FileOperationState::new(
            OperationType::Copy,
            vec!["file1.txt".to_string(), "file2.txt".to_string()],
            "/dest".to_string(),
        );
        assert_eq!(state.operation, OperationType::Copy);
        assert_eq!(state.source_items.len(), 2);
        assert_eq!(state.dest_path, "/dest");
        assert!(!state.is_paused);
        assert!(!state.is_complete);
    }

    #[test]
    fn test_file_operation_state_default() {
        let state = FileOperationState::default();
        assert_eq!(state.operation, OperationType::Copy);
        assert!(state.source_items.is_empty());
        assert!(state.current_file.is_none());
        assert!(state.progress.is_none());
    }

    #[test]
    fn test_update_progress() {
        let mut state = FileOperationState::default();
        state.update_progress("file.txt".to_string(), 500, 1000);
        assert_eq!(state.current_file, Some("file.txt".to_string()));
        assert_eq!(state.processed_bytes, 500);
        assert_eq!(state.total_bytes, 1000);
        assert_eq!(state.progress, Some(50.0));
    }

    #[test]
    fn test_update_speed() {
        let mut state = FileOperationState::default();
        state.update_speed(1_500_000, 120);
        assert_eq!(state.speed_bps, 1_500_000);
        assert_eq!(state.eta_seconds, 120);
    }

    #[test]
    fn test_add_conflict() {
        let mut state = FileOperationState::default();
        let conflict = ConflictItem {
            source_path: "/src/file.txt".to_string(),
            dest_path: "/dest/file.txt".to_string(),
            source_size: 1000,
            dest_size: 500,
            source_modified: "2024-01-01".to_string(),
            dest_modified: "2024-01-02".to_string(),
        };
        state.add_conflict(conflict.clone());
        assert_eq!(state.conflicts.len(), 1);
        assert_eq!(state.conflicts[0].source_path, "/src/file.txt");
    }

    #[test]
    fn test_resolve_conflict() {
        let mut state = FileOperationState::default();
        state.add_conflict(ConflictItem {
            source_path: "/src/file.txt".to_string(),
            dest_path: "/dest/file.txt".to_string(),
            source_size: 1000,
            dest_size: 500,
            source_modified: "2024-01-01".to_string(),
            dest_modified: "2024-01-02".to_string(),
        });
        state.add_conflict(ConflictItem {
            source_path: "/src/file2.txt".to_string(),
            dest_path: "/dest/file2.txt".to_string(),
            source_size: 2000,
            dest_size: 1000,
            source_modified: "2024-01-01".to_string(),
            dest_modified: "2024-01-02".to_string(),
        });
        assert_eq!(state.current_conflict_index, 0);
        state.resolve_conflict(ConflictResolution::Replace);
        assert_eq!(state.current_conflict_index, 1);
    }

    #[test]
    fn test_current_conflict() {
        let mut state = FileOperationState::default();
        assert!(state.current_conflict().is_none());
        state.add_conflict(ConflictItem {
            source_path: "/src/file.txt".to_string(),
            dest_path: "/dest/file.txt".to_string(),
            source_size: 1000,
            dest_size: 500,
            source_modified: "2024-01-01".to_string(),
            dest_modified: "2024-01-02".to_string(),
        });
        assert!(state.current_conflict().is_some());
        assert_eq!(state.current_conflict().unwrap().source_path, "/src/file.txt");
    }

    #[test]
    fn test_has_pending_conflicts() {
        let mut state = FileOperationState::default();
        assert!(!state.has_pending_conflicts());
        state.add_conflict(ConflictItem {
            source_path: "/src/file.txt".to_string(),
            dest_path: "/dest/file.txt".to_string(),
            source_size: 1000,
            dest_size: 500,
            source_modified: "2024-01-01".to_string(),
            dest_modified: "2024-01-02".to_string(),
        });
        assert!(state.has_pending_conflicts());
    }

    #[test]
    fn test_pause_resume() {
        let mut state = FileOperationState::default();
        assert!(!state.is_paused);
        state.pause();
        assert!(state.is_paused);
        state.resume();
        assert!(!state.is_paused);
    }

    #[test]
    fn test_complete() {
        let mut state = FileOperationState::default();
        state.complete();
        assert!(state.is_complete);
        assert_eq!(state.progress, Some(100.0));
    }

    #[test]
    fn test_set_error() {
        let mut state = FileOperationState::default();
        state.set_error("Access denied".to_string());
        assert_eq!(state.error_message, Some("Access denied".to_string()));
    }

    #[test]
    fn test_formatted_speed() {
        let mut state = FileOperationState::default();
        state.speed_bps = 0;
        assert_eq!(state.formatted_speed(), "0 B/s");
        state.speed_bps = 500;
        assert_eq!(state.formatted_speed(), "500 B/s");
        state.speed_bps = 1_500;
        assert_eq!(state.formatted_speed(), "1.5 KB/s");
        state.speed_bps = 1_500_000;
        assert_eq!(state.formatted_speed(), "1.5 MB/s");
        state.speed_bps = 1_500_000_000;
        assert_eq!(state.formatted_speed(), "1.5 GB/s");
    }

    #[test]
    fn test_formatted_eta() {
        let mut state = FileOperationState::default();
        assert_eq!(state.formatted_eta(), "calculating...");
        state.eta_seconds = 30;
        assert_eq!(state.formatted_eta(), "30 sec remaining");
        state.eta_seconds = 90;
        assert_eq!(state.formatted_eta(), "1 min 30 sec remaining");
        state.eta_seconds = 3665;
        assert_eq!(state.formatted_eta(), "61 min 5 sec remaining");
    }

    #[test]
    fn test_progress_percentage() {
        let state = FileOperationState::default();
        assert_eq!(state.progress_percentage(), 0.0);
        let mut state = FileOperationState::default();
        state.progress = Some(75.5);
        assert_eq!(state.progress_percentage(), 75.5);
    }

    #[test]
    fn test_conflict_item_clone() {
        let conflict = ConflictItem {
            source_path: "/src/file.txt".to_string(),
            dest_path: "/dest/file.txt".to_string(),
            source_size: 1000,
            dest_size: 500,
            source_modified: "2024-01-01".to_string(),
            dest_modified: "2024-01-02".to_string(),
        };
        let cloned = conflict.clone();
        assert_eq!(cloned.source_path, conflict.source_path);
        assert_eq!(cloned.dest_path, conflict.dest_path);
    }

    #[test]
    fn test_operation_type_debug() {
        assert_eq!(format!("{:?}", OperationType::Copy), "Copy");
        assert_eq!(format!("{:?}", OperationType::Move), "Move");
        assert_eq!(format!("{:?}", OperationType::Delete), "Delete");
        assert_eq!(format!("{:?}", OperationType::Rename), "Rename");
    }

    #[test]
    fn test_conflict_resolution_debug() {
        assert_eq!(format!("{:?}", ConflictResolution::Replace), "Replace");
        assert_eq!(format!("{:?}", ConflictResolution::Skip), "Skip");
        assert_eq!(format!("{:?}", ConflictResolution::RenameNew), "RenameNew");
        assert_eq!(format!("{:?}", ConflictResolution::RenameExisting), "RenameExisting");
    }
}
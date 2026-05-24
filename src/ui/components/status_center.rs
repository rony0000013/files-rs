//! Status Center Flyout — dedicated panel for monitoring background tasks, transfers, and resolving conflicts.
//! Per DESIGN_SPEC.md section 5.3 / TODO.md Section 6.
//!
//! This module provides a status center that:
//! - Shows active file operations (copy, move, delete, etc.)
//! - Displays transfer progress with speed and ETA
//! - Shows conflict resolution requests
//! - Provides quick access to cancel/pause operations

use crate::core::path_util::FilesPath;
use crate::core::progress::{FileOperationJob, OperationStatus};
use std::time::{Duration, Instant, SystemTime};

/// Represents a conflict that needs user resolution during file operations.
#[derive(Debug, Clone, PartialEq)]
pub struct ConflictInfo {
    /// Unique identifier for this conflict
    pub id: uuid::Uuid,
    /// Source path involved in the conflict
    pub source_path: FilesPath,
    /// Destination path involved in the conflict
    pub dest_path: FilesPath,
    /// Type of conflict
    pub conflict_type: ConflictType,
    /// Suggested resolutions available
    pub available_resolutions: Vec<ConflictResolution>,
    /// Time when the conflict was created
    pub created_at: Instant,
}

/// Type of conflict detected during file operations.
#[derive(Debug, Clone, PartialEq)]
pub enum ConflictType {
    /// Destination file already exists
    FileExists,
    /// Source and destination have different content (size mismatch)
    SizeMismatch,
    /// Source was modified after the operation started
    SourceModified,
    /// Destination is locked by another process
    DestinationLocked,
    /// Permission denied
    PermissionDenied,
    /// Network connection was lost
    NetworkError,
    /// Other/unknown conflict
    Other(String),
}

/// Resolution options for a conflict.
#[derive(Debug, Clone, PartialEq)]
pub enum ConflictResolution {
    /// Replace the destination file
    Replace,
    /// Skip this file/folder entirely
    Skip,
    /// Rename the source to avoid conflict
    Rename,
    /// Merge source and destination (for folders)
    Merge,
    /// Keep both files (create unique name)
    KeepBoth,
    /// Retry the operation
    Retry,
    /// Cancel the entire operation
    Cancel,
}

/// State for the Status Center Flyout.
#[derive(Debug, Clone, PartialEq)]
pub struct StatusCenterState {
    /// All active jobs
    pub jobs: Vec<FileOperationJob>,
    /// All pending conflicts requiring resolution
    pub conflicts: Vec<ConflictInfo>,
    /// Whether the flyout is currently open
    pub is_open: bool,
    /// Timestamp of the oldest active job
    pub oldest_active_time: Option<Instant>,
}

impl Default for StatusCenterState {
    fn default() -> Self {
        Self::new()
    }
}

impl StatusCenterState {
    pub fn new() -> Self {
        Self {
            jobs: Vec::new(),
            conflicts: Vec::new(),
            is_open: false,
            oldest_active_time: None,
        }
    }

    /// Get count of active (in-progress) jobs.
    pub fn active_job_count(&self) -> usize {
        self.jobs
            .iter()
            .filter(|j| matches!(j.status, OperationStatus::InProgress { .. }))
            .count()
    }

    pub fn update_job(&mut self, id: uuid::Uuid, status: OperationStatus) {
        if let Some(job) = self.jobs.iter_mut().find(|j| j.id == id) {
            job.status = status;
        }
    }

    pub fn complete_job(&mut self, id: uuid::Uuid) {
        if let Some(job) = self.jobs.iter_mut().find(|j| j.id == id) {
            job.status = OperationStatus::Completed;
        }
    }

    pub fn fail_job(&mut self, id: uuid::Uuid, reason: String) {
        if let Some(job) = self.jobs.iter_mut().find(|j| j.id == id) {
            job.status = OperationStatus::Failed(reason);
        }
    }

    pub fn add_job(&mut self, job: FileOperationJob) {
        self.jobs.push(job);
    }

    /// Get count of jobs pending resolution (conflicts).
    pub fn conflict_count(&self) -> usize {
        self.conflicts.len()
    }

    /// Get total progress across all active jobs (0.0 to 1.0).
    /// Returns None if there are no active jobs with known total bytes.
    pub fn aggregate_progress(&self) -> Option<f32> {
        let active_jobs: Vec<&FileOperationJob> = self
            .jobs
            .iter()
            .filter(|j| matches!(j.status, OperationStatus::InProgress { .. }))
            .collect();

        if active_jobs.is_empty() {
            return None;
        }

        let mut total_bytes: u64 = 0;
        let mut processed_bytes: u64 = 0;

        for job in &active_jobs {
            if let OperationStatus::InProgress {
                bytes_processed,
                total_bytes: total,
                ..
            } = &job.status
            {
                total_bytes += total;
                processed_bytes += bytes_processed;
            }
        }

        if total_bytes == 0 {
            return None;
        }

        Some(processed_bytes as f32 / total_bytes as f32)
    }

    /// Check if there are any conflicts that need resolution.
    pub fn has_pending_conflicts(&self) -> bool {
        !self.conflicts.is_empty()
    }

    /// Get the total count of items (jobs + conflicts).
    pub fn total_items(&self) -> usize {
        self.jobs.len() + self.conflicts.len()
    }

    /// Check if the status center has any content to display.
    pub fn has_content(&self) -> bool {
        !self.jobs.is_empty() || !self.conflicts.is_empty()
    }

    /// Find a conflict by its ID.
    pub fn find_conflict(&self, id: uuid::Uuid) -> Option<&ConflictInfo> {
        self.conflicts.iter().find(|c| c.id == id)
    }

    /// Find a conflict by ID and return mutable reference.
    pub fn find_conflict_mut(&mut self, id: uuid::Uuid) -> Option<&mut ConflictInfo> {
        self.conflicts.iter_mut().find(|c| c.id == id)
    }

    /// Add a new conflict to the queue.
    pub fn add_conflict(&mut self, conflict: ConflictInfo) {
        self.conflicts.push(conflict);
    }

    /// Remove a resolved conflict from the queue.
    pub fn remove_conflict(&mut self, id: uuid::Uuid) {
        self.conflicts.retain(|c| c.id != id);
    }

    /// Update jobs from the progress engine.
    pub fn sync_jobs(&mut self, jobs: Vec<FileOperationJob>) {
        self.jobs = jobs;
        self.update_oldest_time();
    }

    /// Update the oldest active time based on current jobs.
    fn update_oldest_time(&mut self) {
        let active_jobs: Vec<&FileOperationJob> = self
            .jobs
            .iter()
            .filter(|j| matches!(j.status, OperationStatus::InProgress { .. }))
            .collect();

        if active_jobs.is_empty() {
            self.oldest_active_time = None;
        } else {
            // Find the job that started earliest
            let oldest: Option<Instant> = active_jobs.iter().map(|j| j.created_at).min();
            self.oldest_active_time = oldest;
        }
    }
}

/// Actions that can be performed in the Status Center.
#[derive(Debug, Clone, PartialEq)]
pub enum StatusCenterAction {
    /// Cancel a specific job
    CancelJob(uuid::Uuid),
    /// Pause a specific job
    PauseJob(uuid::Uuid),
    /// Resume a paused job
    ResumeJob(uuid::Uuid),
    /// Retry a failed job
    RetryJob(uuid::Uuid),
    /// Clear completed jobs from the list
    ClearCompleted,
    /// Clear all jobs (including in-progress, with confirmation)
    ClearAll,
    /// Resolve a conflict with the given resolution
    ResolveConflict {
        conflict_id: uuid::Uuid,
        resolution: ConflictResolution,
    },
    /// Open the status center flyout
    Open,
    /// Close the status center flyout
    Close,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_center_state_new() {
        let state = StatusCenterState::new();
        assert!(state.jobs.is_empty());
        assert!(state.conflicts.is_empty());
        assert!(!state.is_open);
        assert!(state.oldest_active_time.is_none());
    }

    #[test]
    fn test_status_center_state_active_job_count() {
        let mut state = StatusCenterState::new();

        // No jobs
        assert_eq!(state.active_job_count(), 0);

        // Add a pending job (not in progress)
        state.jobs.push(FileOperationJob {
            id: uuid::Uuid::new_v4(),
            name: "Pending job".to_string(),
            status: OperationStatus::Pending,
            created_at: Instant::now(),
        });
        assert_eq!(state.active_job_count(), 0);

        // Add an in-progress job
        state.jobs.push(FileOperationJob {
            id: uuid::Uuid::new_v4(),
            name: "In progress job".to_string(),
            status: OperationStatus::InProgress {
                progress: 0.5,
                bytes_processed: 50,
                total_bytes: 100,
                speed: 10.0,
                eta: Some(Duration::from_secs(5)),
            },
            created_at: Instant::now(),
        });
        assert_eq!(state.active_job_count(), 1);
    }

    #[test]
    fn test_status_center_state_conflict_count() {
        let mut state = StatusCenterState::new();
        assert_eq!(state.conflict_count(), 0);

        state.add_conflict(ConflictInfo {
            id: uuid::Uuid::new_v4(),
            source_path: FilesPath::from(std::path::PathBuf::from("/source")),
            dest_path: FilesPath::from(std::path::PathBuf::from("/dest")),
            conflict_type: ConflictType::FileExists,
            available_resolutions: vec![ConflictResolution::Replace],
            created_at: Instant::now(),
        });
        assert_eq!(state.conflict_count(), 1);
    }

    #[test]
    fn test_status_center_state_aggregate_progress() {
        let mut state = StatusCenterState::new();

        // No jobs
        assert!(state.aggregate_progress().is_none());

        // Add a completed job
        state.jobs.push(FileOperationJob {
            id: uuid::Uuid::new_v4(),
            name: "Completed".to_string(),
            status: OperationStatus::Completed,
            created_at: Instant::now(),
        });
        assert!(state.aggregate_progress().is_none());

        // Add an in-progress job with 50% progress
        state.jobs.push(FileOperationJob {
            id: uuid::Uuid::new_v4(),
            name: "In progress".to_string(),
            status: OperationStatus::InProgress {
                progress: 0.5,
                bytes_processed: 50,
                total_bytes: 100,
                speed: 10.0,
                eta: Some(Duration::from_secs(5)),
            },
            created_at: Instant::now(),
        });
        assert_eq!(state.aggregate_progress(), Some(0.5));
    }

    #[test]
    fn test_status_center_state_has_pending_conflicts() {
        let mut state = StatusCenterState::new();
        assert!(!state.has_pending_conflicts());

        state.add_conflict(ConflictInfo {
            id: uuid::Uuid::new_v4(),
            source_path: FilesPath::from(std::path::PathBuf::from("/source")),
            dest_path: FilesPath::from(std::path::PathBuf::from("/dest")),
            conflict_type: ConflictType::FileExists,
            available_resolutions: vec![ConflictResolution::Replace],
            created_at: Instant::now(),
        });
        assert!(state.has_pending_conflicts());
    }

    #[test]
    fn test_status_center_state_total_items() {
        let mut state = StatusCenterState::new();
        assert_eq!(state.total_items(), 0);

        state.jobs.push(FileOperationJob {
            id: uuid::Uuid::new_v4(),
            name: "Job".to_string(),
            status: OperationStatus::Pending,
            created_at: Instant::now(),
        });
        assert_eq!(state.total_items(), 1);

        state.add_conflict(ConflictInfo {
            id: uuid::Uuid::new_v4(),
            source_path: FilesPath::from(std::path::PathBuf::from("/source")),
            dest_path: FilesPath::from(std::path::PathBuf::from("/dest")),
            conflict_type: ConflictType::FileExists,
            available_resolutions: vec![ConflictResolution::Replace],
            created_at: Instant::now(),
        });
        assert_eq!(state.total_items(), 2);
    }

    #[test]
    fn test_status_center_state_has_content() {
        let state = StatusCenterState::new();
        assert!(!state.has_content());

        let mut state_with_jobs = StatusCenterState::new();
        state_with_jobs.jobs.push(FileOperationJob {
            id: uuid::Uuid::new_v4(),
            name: "Job".to_string(),
            status: OperationStatus::Pending,
            created_at: Instant::now(),
        });
        assert!(state_with_jobs.has_content());

        let mut state_with_conflicts = StatusCenterState::new();
        state_with_conflicts.add_conflict(ConflictInfo {
            id: uuid::Uuid::new_v4(),
            source_path: FilesPath::from(std::path::PathBuf::from("/source")),
            dest_path: FilesPath::from(std::path::PathBuf::from("/dest")),
            conflict_type: ConflictType::FileExists,
            available_resolutions: vec![ConflictResolution::Replace],
            created_at: Instant::now(),
        });
        assert!(state_with_conflicts.has_content());
    }

    #[test]
    fn test_find_conflict() {
        let mut state = StatusCenterState::new();
        let conflict_id = uuid::Uuid::new_v4();

        state.add_conflict(ConflictInfo {
            id: conflict_id,
            source_path: FilesPath::from(std::path::PathBuf::from("/source")),
            dest_path: FilesPath::from(std::path::PathBuf::from("/dest")),
            conflict_type: ConflictType::FileExists,
            available_resolutions: vec![ConflictResolution::Replace],
            created_at: Instant::now(),
        });

        let found = state.find_conflict(conflict_id);
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, conflict_id);

        let not_found = state.find_conflict(uuid::Uuid::nil());
        assert!(not_found.is_none());
    }

    #[test]
    fn test_remove_conflict() {
        let mut state = StatusCenterState::new();
        let conflict_id = uuid::Uuid::new_v4();

        state.add_conflict(ConflictInfo {
            id: conflict_id,
            source_path: FilesPath::from(std::path::PathBuf::from("/source")),
            dest_path: FilesPath::from(std::path::PathBuf::from("/dest")),
            conflict_type: ConflictType::FileExists,
            available_resolutions: vec![ConflictResolution::Replace],
            created_at: Instant::now(),
        });

        assert_eq!(state.conflict_count(), 1);
        state.remove_conflict(conflict_id);
        assert_eq!(state.conflict_count(), 0);
    }

    #[test]
    fn test_conflict_type_variants() {
        let variants = vec![
            ConflictType::FileExists,
            ConflictType::SizeMismatch,
            ConflictType::SourceModified,
            ConflictType::DestinationLocked,
            ConflictType::PermissionDenied,
            ConflictType::NetworkError,
            ConflictType::Other("custom".to_string()),
        ];

        assert_eq!(variants.len(), 7);
    }

    #[test]
    fn test_conflict_resolution_variants() {
        let variants = vec![
            ConflictResolution::Replace,
            ConflictResolution::Skip,
            ConflictResolution::Rename,
            ConflictResolution::Merge,
            ConflictResolution::KeepBoth,
            ConflictResolution::Retry,
            ConflictResolution::Cancel,
        ];

        assert_eq!(variants.len(), 7);
    }

    #[test]
    fn test_status_center_action_variants() {
        let job_id = uuid::Uuid::new_v4();
        let conflict_id = uuid::Uuid::new_v4();

        let actions = vec![
            StatusCenterAction::CancelJob(job_id),
            StatusCenterAction::PauseJob(job_id),
            StatusCenterAction::ResumeJob(job_id),
            StatusCenterAction::RetryJob(job_id),
            StatusCenterAction::ClearCompleted,
            StatusCenterAction::ClearAll,
            StatusCenterAction::ResolveConflict {
                conflict_id,
                resolution: ConflictResolution::Replace,
            },
            StatusCenterAction::Open,
            StatusCenterAction::Close,
        ];

        assert_eq!(actions.len(), 9);
    }

    #[test]
    fn test_sync_jobs() {
        let mut state = StatusCenterState::new();

        let jobs = vec![
            FileOperationJob {
                id: uuid::Uuid::new_v4(),
                name: "Job 1".to_string(),
                status: OperationStatus::Pending,
                created_at: Instant::now(),
            },
            FileOperationJob {
                id: uuid::Uuid::new_v4(),
                name: "Job 2".to_string(),
                status: OperationStatus::Completed,
                created_at: Instant::now(),
            },
        ];

        state.sync_jobs(jobs);
        assert_eq!(state.jobs.len(), 2);
    }

    #[test]
    fn test_oldest_active_time_tracking() {
        let mut state = StatusCenterState::new();
        assert!(state.oldest_active_time.is_none());

        // Add in-progress jobs
        state.jobs.push(FileOperationJob {
            id: uuid::Uuid::new_v4(),
            name: "Job 1".to_string(),
            status: OperationStatus::InProgress {
                progress: 0.5,
                bytes_processed: 50,
                total_bytes: 100,
                speed: 10.0,
                eta: Some(Duration::from_secs(5)),
            },
            created_at: Instant::now(),
        });

        state.sync_jobs(state.jobs.clone());
        // After sync, oldest_active_time should be updated
    }
}

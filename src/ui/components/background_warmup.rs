//! Background Warmup — Pre-load metadata and context menu data to improve performance.
//! Per DESIGN_SPEC.md section 6 / TODO.md Section 6.
//!
//! This module provides warmup functionality for:
//! - Pre-loading context menu metadata
//! - Caching file/directory information
//! - Background prefetching of commonly accessed paths

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Priority level for warmup tasks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum WarmupPriority {
    Low = 0,
    Medium = 1,
    High = 2,
}

/// Status of a warmup operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WarmupStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
    Cancelled,
}

/// A single warmup task entry.
#[derive(Debug, Clone)]
pub struct WarmupTask {
    /// Unique identifier for this task.
    pub id: u64,
    /// Path being warmed up.
    pub path: PathBuf,
    /// Priority level.
    pub priority: WarmupPriority,
    /// Current status.
    pub status: WarmupStatus,
    /// When the task was created.
    pub created_at: Instant,
    /// When the task started processing.
    pub started_at: Option<Instant>,
    /// When the task completed (or failed).
    pub completed_at: Option<Instant>,
    /// Optional error message if failed.
    pub error: Option<String>,
}

impl WarmupTask {
    pub fn new(id: u64, path: PathBuf, priority: WarmupPriority) -> Self {
        Self {
            id,
            path,
            priority,
            status: WarmupStatus::Pending,
            created_at: Instant::now(),
            started_at: None,
            completed_at: None,
            error: None,
        }
    }

    /// Returns the duration since the task was created.
    pub fn age(&self) -> Duration {
        self.created_at.elapsed()
    }

    /// Returns the processing duration if completed.
    pub fn processing_duration(&self) -> Option<Duration> {
        self.started_at.and_then(|started| {
            self.completed_at.map(|completed| completed.duration_since(started))
        })
    }

    /// Checks if the task is in a terminal state.
    pub fn is_done(&self) -> bool {
        matches!(
            self.status,
            WarmupStatus::Completed | WarmupStatus::Failed | WarmupStatus::Cancelled
        )
    }
}

/// Statistics for the warmup service.
#[derive(Debug, Clone, Default)]
pub struct WarmupStats {
    /// Total tasks submitted.
    pub total_submitted: u64,
    /// Tasks completed successfully.
    pub total_completed: u64,
    /// Tasks that failed.
    pub total_failed: u64,
    /// Tasks that were cancelled.
    pub total_cancelled: u64,
    /// Total bytes processed (if applicable).
    pub bytes_processed: u64,
    /// Total items processed.
    pub items_processed: u64,
}

impl WarmupStats {
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the number of tasks that are not yet complete.
    pub fn pending_count(&self) -> u64 {
        self.total_submitted - self.total_completed - self.total_failed - self.total_cancelled
    }

    /// Returns the success rate as a percentage (0.0 to 100.0).
    pub fn success_rate(&self) -> f64 {
        if self.total_submitted == 0 {
            100.0
        } else {
            (self.total_completed as f64 / self.total_submitted as f64) * 100.0
        }
    }
}

/// Service for managing background warmup operations.
#[derive(Debug, Clone, Default)]
pub struct BackgroundWarmupService {
    /// Active warmup tasks.
    tasks: Arc<Vec<WarmupTask>>,
    /// Statistics.
    stats: Arc<WarmupStats>,
    /// Whether the service is enabled.
    enabled: bool,
    /// Maximum concurrent warmup operations.
    max_concurrent: usize,
}

impl BackgroundWarmupService {
    pub fn new() -> Self {
        Self {
            tasks: Arc::new(Vec::new()),
            stats: Arc::new(WarmupStats::new()),
            enabled: true,
            max_concurrent: 4,
        }
    }

    /// Gets whether warmup is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Enables or disables warmup.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Gets the maximum concurrent warmup operations.
    pub fn max_concurrent(&self) -> usize {
        self.max_concurrent
    }

    /// Sets the maximum concurrent warmup operations.
    pub fn set_max_concurrent(&mut self, max: usize) {
        self.max_concurrent = max;
    }

    /// Gets a snapshot of current stats.
    pub fn get_stats(&self) -> WarmupStats {
        (*self.stats).clone()
    }

    /// Gets the number of pending tasks.
    pub fn pending_count(&self) -> usize {
        self.tasks.iter().filter(|t| !t.is_done()).count()
    }

    /// Checks if warmup should run (based on system load, etc.).
    pub fn should_warmup(&self, current_load: f64) -> bool {
        if !self.enabled {
            return false;
        }
        // Don't start new warmup if system load is too high (> 80%)
        current_load < 0.8 && self.pending_count() < self.max_concurrent
    }

    /// Gets suggested paths to warm up based on common access patterns.
    pub fn get_suggested_paths(&self, recent_paths: &[PathBuf]) -> Vec<PathBuf> {
        // Return unique paths, prioritizing newer entries
        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();

        for path in recent_paths.iter().rev() {
            if seen.insert(path.clone()) {
                result.push(path.clone());
                if result.len() >= 10 {
                    break;
                }
            }
        }

        result
    }

    /// Calculates priority score for a path (higher = more important to warm up).
    pub fn calculate_priority(&self, path: &PathBuf, access_count: u32) -> WarmupPriority {
        let extension_priority = if let Some(ext) = path.extension() {
            match ext.to_string_lossy().as_ref() {
                "exe" | "msi" | "dll" => WarmupPriority::High,
                "rs" | "js" | "ts" | "py" | "cpp" => WarmupPriority::High,
                "docx" | "xlsx" | "pptx" => WarmupPriority::Medium,
                _ => WarmupPriority::Low,
            }
        } else {
            WarmupPriority::Low
        };

        let access_priority = if access_count > 10 {
            WarmupPriority::High
        } else if access_count > 3 {
            WarmupPriority::Medium
        } else {
            WarmupPriority::Low
        };

        // Return the higher priority
        if extension_priority > access_priority {
            extension_priority
        } else {
            access_priority
        }
    }
}

/// Cache for warmup metadata.
#[derive(Debug, Clone, Default)]
pub struct WarmupCache {
    /// Cache entries keyed by path.
    entries: HashMap<PathBuf, CacheEntry>,
    /// Maximum cache size in bytes.
    max_size: usize,
    /// Current cache size in bytes.
    current_size: usize,
}

/// A single cache entry.
#[derive(Debug, Clone)]
pub struct CacheEntry {
    /// The path this entry is for.
    pub path: PathBuf,
    /// Cached metadata (JSON serialized or similar).
    pub metadata: Vec<u8>,
    /// When this entry was created.
    pub created_at: Instant,
    /// When this entry was last accessed.
    pub accessed_at: Instant,
    /// Size in bytes.
    pub size: usize,
}

impl WarmupCache {
    pub fn new(max_size: usize) -> Self {
        Self {
            entries: HashMap::new(),
            max_size,
            current_size: 0,
        }
    }

    /// Gets an entry from the cache.
    pub fn get(&mut self, path: &PathBuf) -> Option<&CacheEntry> {
        if let Some(entry) = self.entries.get_mut(path) {
            entry.accessed_at = Instant::now();
            Some(entry)
        } else {
            None
        }
    }

    /// Inserts an entry into the cache.
    pub fn insert(&mut self, path: PathBuf, metadata: Vec<u8>) {
        let size = metadata.len();
        let entry = CacheEntry {
            path: path.clone(),
            metadata,
            created_at: Instant::now(),
            accessed_at: Instant::now(),
            size,
        };

        // Evict if necessary
        while self.current_size + size > self.max_size && !self.entries.is_empty() {
            self.evict_oldest();
        }

        // Remove existing entry if present
        if let Some(old) = self.entries.remove(&path) {
            self.current_size -= old.size;
        }

        self.current_size += size;
        self.entries.insert(path, entry);
    }

    /// Evicts the oldest entry from the cache.
    fn evict_oldest(&mut self) {
        if let Some(oldest) = self
            .entries
            .iter()
            .min_by_key(|e| e.1.accessed_at)
            .map(|(k, v)| (k.clone(), v.size))
        {
            if let Some(entry) = self.entries.remove(&oldest.0) {
                self.current_size -= entry.size;
            }
        }
    }

    /// Clears the cache.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.current_size = 0;
    }

    /// Returns the number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns whether the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_warmup_priority_ordering() {
        assert!(WarmupPriority::High > WarmupPriority::Medium);
        assert!(WarmupPriority::Medium > WarmupPriority::Low);
    }

    #[test]
    fn test_warmup_task_new() {
        let task = WarmupTask::new(1, PathBuf::from("/test"), WarmupPriority::Medium);
        assert_eq!(task.id, 1);
        assert_eq!(task.path, PathBuf::from("/test"));
        assert_eq!(task.priority, WarmupPriority::Medium);
        assert_eq!(task.status, WarmupStatus::Pending);
        assert!(task.error.is_none());
    }

    #[test]
    fn test_warmup_task_age() {
        let task = WarmupTask::new(1, PathBuf::from("/test"), WarmupPriority::Low);
        std::thread::sleep(Duration::from_millis(10));
        assert!(task.age() >= Duration::from_millis(10));
    }

    #[test]
    fn test_warmup_task_is_done() {
        let mut task = WarmupTask::new(1, PathBuf::from("/test"), WarmupPriority::Low);
        assert!(!task.is_done());

        task.status = WarmupStatus::Completed;
        assert!(task.is_done());

        task.status = WarmupStatus::Failed;
        assert!(task.is_done());
    }

    #[test]
    fn test_warmup_stats_new() {
        let stats = WarmupStats::new();
        assert_eq!(stats.total_submitted, 0);
        assert_eq!(stats.total_completed, 0);
    }

    #[test]
    fn test_warmup_stats_pending_count() {
        let mut stats = WarmupStats::new();
        stats.total_submitted = 10;
        stats.total_completed = 3;
        stats.total_failed = 1;
        assert_eq!(stats.pending_count(), 6);
    }

    #[test]
    fn test_warmup_stats_success_rate() {
        let mut stats = WarmupStats::new();
        stats.total_submitted = 10;
        stats.total_completed = 7;
        stats.total_failed = 3;
        assert_eq!(stats.success_rate(), 70.0);

        // Edge case: no submissions
        let stats_empty = WarmupStats::new();
        assert_eq!(stats_empty.success_rate(), 100.0);
    }

    #[test]
    fn test_background_warmup_service_new() {
        let service = BackgroundWarmupService::new();
        assert!(service.is_enabled());
    }

    #[test]
    fn test_background_warmup_service_enable_disable() {
        let mut service = BackgroundWarmupService::new();
        service.set_enabled(false);
        assert!(!service.is_enabled());
        service.set_enabled(true);
        assert!(service.is_enabled());
    }

    #[test]
    fn test_background_warmup_service_max_concurrent() {
        let mut service = BackgroundWarmupService::new();
        assert_eq!(service.max_concurrent(), 4);
        service.set_max_concurrent(8);
        assert_eq!(service.max_concurrent(), 8);
    }

    #[test]
    fn test_background_warmup_service_should_warmup() {
        let service = BackgroundWarmupService::new();
        // Should warmup when enabled and load is low
        assert!(service.should_warmup(0.3));
        // Should not warmup when load is high
        assert!(!service.should_warmup(0.9));
    }

    #[test]
    fn test_background_warmup_service_disabled() {
        let mut service = BackgroundWarmupService::new();
        service.set_enabled(false);
        assert!(!service.should_warmup(0.1));
    }

    #[test]
    fn test_background_warmup_service_get_stats() {
        let service = BackgroundWarmupService::new();
        let stats = service.get_stats();
        assert_eq!(stats.total_submitted, 0);
    }

    #[test]
    fn test_background_warmup_service_get_suggested_paths() {
        let service = BackgroundWarmupService::new();
        let recent = vec![
            PathBuf::from("/a"),
            PathBuf::from("/b"),
            PathBuf::from("/a"), // duplicate
            PathBuf::from("/c"),
        ];
        let suggested = service.get_suggested_paths(&recent);
        // Should return unique paths in reverse order, max 10
        assert_eq!(suggested.len(), 3);
        assert_eq!(suggested[0], PathBuf::from("/c"));
    }

    #[test]
    fn test_background_warmup_service_calculate_priority() {
        let service = BackgroundWarmupService::new();

        // High priority: executable
        let exe_path = PathBuf::from("/test.exe");
        assert_eq!(
            service.calculate_priority(&exe_path, 1),
            WarmupPriority::High
        );

        // Medium priority: document
        let doc_path = PathBuf::from("/test.docx");
        assert_eq!(
            service.calculate_priority(&doc_path, 1),
            WarmupPriority::Medium
        );

        // High priority: frequently accessed
        let freq_path = PathBuf::from("/test.txt");
        assert_eq!(
            service.calculate_priority(&freq_path, 15),
            WarmupPriority::High
        );
    }

    #[test]
    fn test_warmup_cache_new() {
        let cache = WarmupCache::new(1000);
        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn test_warmup_cache_insert_get() {
        let mut cache = WarmupCache::new(100);
        cache.insert(PathBuf::from("/test"), vec![1, 2, 3]);
        assert_eq!(cache.len(), 1);

        let entry = cache.get(&PathBuf::from("/test"));
        assert!(entry.is_some());
        assert_eq!(entry.unwrap().metadata, vec![1, 2, 3]);
    }

    #[test]
    fn test_warmup_cache_clear() {
        let mut cache = WarmupCache::new(100);
        cache.insert(PathBuf::from("/test1"), vec![1, 2, 3]);
        cache.insert(PathBuf::from("/test2"), vec![4, 5, 6]);
        assert_eq!(cache.len(), 2);

        cache.clear();
        assert!(cache.is_empty());
    }

    #[test]
    fn test_warmup_cache_eviction() {
        let mut cache = WarmupCache::new(10); // Small cache
        cache.insert(PathBuf::from("/test1"), vec![1, 2, 3, 4, 5]); // 5 bytes
        cache.insert(PathBuf::from("/test2"), vec![6, 7, 8, 9, 10]); // 5 bytes
        assert_eq!(cache.len(), 2);

        // Adding this should trigger eviction since 5+5+5 > 10
        cache.insert(PathBuf::from("/test3"), vec![1, 2, 3, 4, 5]);
        // One entry should have been evicted
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn test_warmup_cache_update_existing() {
        let mut cache = WarmupCache::new(100);
        cache.insert(PathBuf::from("/test"), vec![1, 2, 3]);
        cache.insert(PathBuf::from("/test"), vec![1, 2, 3, 4, 5]);

        let entry = cache.get(&PathBuf::from("/test")).unwrap();
        assert_eq!(entry.metadata, vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_warmup_status_variants() {
        let statuses = vec![
            WarmupStatus::Pending,
            WarmupStatus::InProgress,
            WarmupStatus::Completed,
            WarmupStatus::Failed,
            WarmupStatus::Cancelled,
        ];
        assert_eq!(statuses.len(), 5);
    }
}
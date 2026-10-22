use crate::core::path_util::FilesPath;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

#[derive(Debug, Clone, PartialEq)]
pub enum OperationStatus {
    Pending,
    InProgress {
        progress: f32, // 0.0 to 1.0
        bytes_processed: u64,
        total_bytes: u64,
        speed: f64, // bytes per second
        eta: Option<Duration>,
    },
    Paused {
        progress: f32,
        bytes_processed: u64,
        total_bytes: u64,
    },
    Completed,
    Failed(String),
    Cancelled,
}

/// Kind of file operation (drives the StatusCenter icon + label, matching Files-community `StatusCenterItemIconKind`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpKind {
    Copy,
    Move,
    Delete,
    Extract,
    Compress,
    Git,
    BulkRename,
    Other,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FileOperationJob {
    pub id: uuid::Uuid,
    pub name: String,
    pub kind: OpKind,
    pub status: OperationStatus,
    pub created_at: Instant,
    /// File currently being transferred (for the StatusCenter message line).
    pub current_file: String,
    /// Recent speed samples in bytes/sec (cap 40) for the sparkline graph.
    pub speed_history: Vec<f32>,
    /// Number of items processed so far.
    pub items_processed: u64,
    /// Total number of items in this batch operation.
    pub total_items: u64,
    /// Optional destination folder path (enables "Open folder" button in Status Center).
    pub destination_path: Option<FilesPath>,
}

impl FileOperationJob {
    pub fn is_running_or_paused(&self) -> bool {
        matches!(
            self.status,
            OperationStatus::Pending
                | OperationStatus::InProgress { .. }
                | OperationStatus::Paused { .. }
        )
    }

    pub fn is_paused(&self) -> bool {
        matches!(self.status, OperationStatus::Paused { .. })
    }

    pub fn progress_fraction(&self) -> f32 {
        match &self.status {
            OperationStatus::InProgress { progress, .. } => progress.clamp(0.0, 1.0),
            OperationStatus::Paused { progress, .. } => progress.clamp(0.0, 1.0),
            OperationStatus::Completed => 1.0,
            _ => 0.0,
        }
    }
}

struct SpeedTracker {
    last_bytes: u64,
    last_time: Instant,
    smoothed_speed: f64,
}

pub struct ProgressEngine {
    jobs: Arc<Mutex<Vec<FileOperationJob>>>,
    subscribers: Arc<Mutex<Vec<mpsc::UnboundedSender<JobUpdate>>>>,
    cancel_flags: Arc<Mutex<HashMap<uuid::Uuid, Arc<AtomicBool>>>>,
    pause_flags: Arc<Mutex<HashMap<uuid::Uuid, Arc<AtomicBool>>>>,
    speed_trackers: Arc<Mutex<HashMap<uuid::Uuid, SpeedTracker>>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum JobUpdate {
    UpdateStatus(uuid::Uuid, OperationStatus),
    UpdateJob(FileOperationJob),
    Complete(uuid::Uuid),
    Fail(uuid::Uuid, String),
    NewJob(FileOperationJob),
}

impl ProgressEngine {
    pub fn new() -> Self {
        Self {
            jobs: Arc::new(Mutex::new(Vec::new())),
            subscribers: Arc::new(Mutex::new(Vec::new())),
            cancel_flags: Arc::new(Mutex::new(HashMap::new())),
            pause_flags: Arc::new(Mutex::new(HashMap::new())),
            speed_trackers: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn subscribe(&self) -> mpsc::UnboundedReceiver<JobUpdate> {
        let (tx, rx) = mpsc::unbounded_channel();
        let mut subs = self.subscribers.lock().unwrap_or_else(|e| e.into_inner());
        subs.push(tx);
        rx
    }

    fn broadcast(&self, update: JobUpdate) {
        let mut subs = self.subscribers.lock().unwrap_or_else(|e| e.into_inner());
        subs.retain(|tx| tx.send(update.clone()).is_ok());
    }

    pub async fn start_job(&self, name: String, kind: OpKind) -> uuid::Uuid {
        self.start_job_with_meta(name, kind, None, 0).await
    }

    pub async fn start_job_with_meta(
        &self,
        name: String,
        kind: OpKind,
        destination_path: Option<FilesPath>,
        total_items: u64,
    ) -> uuid::Uuid {
        let id = uuid::Uuid::new_v4();
        let now = Instant::now();
        let job = FileOperationJob {
            id,
            name,
            kind,
            status: OperationStatus::Pending,
            created_at: now,
            current_file: String::new(),
            speed_history: Vec::new(),
            items_processed: 0,
            total_items,
            destination_path,
        };
        self.jobs
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(job.clone());
        self.cancel_flags
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(id, Arc::new(AtomicBool::new(false)));
        self.pause_flags
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(id, Arc::new(AtomicBool::new(false)));
        self.speed_trackers
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(
                id,
                SpeedTracker {
                    last_bytes: 0,
                    last_time: now,
                    smoothed_speed: 0.0,
                },
            );
        self.broadcast(JobUpdate::NewJob(job));
        id
    }

    /// Fetch the cancellation flag for a job (workers poll this per chunk).
    pub fn cancel_flag(&self, id: uuid::Uuid) -> Arc<AtomicBool> {
        self.cancel_flags
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .entry(id)
            .or_insert_with(|| Arc::new(AtomicBool::new(false)))
            .clone()
    }

    /// Fetch the pause flag for a job (workers poll this per chunk).
    pub fn pause_flag(&self, id: uuid::Uuid) -> Arc<AtomicBool> {
        self.pause_flags
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .entry(id)
            .or_insert_with(|| Arc::new(AtomicBool::new(false)))
            .clone()
    }

    /// Request cancellation of a running job (StatusCenter cancel button).
    pub fn request_cancel(&self, id: uuid::Uuid) {
        if let Some(flag) = self
            .cancel_flags
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&id)
        {
            flag.store(true, Ordering::SeqCst);
        }
        // Also unpause so a paused worker wakes up and exits immediately
        if let Some(pflag) = self
            .pause_flags
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&id)
        {
            pflag.store(false, Ordering::SeqCst);
        }
    }

    /// Toggle pause/resume on a running job. Returns the new paused state (`true` = paused).
    pub fn toggle_pause(&self, id: uuid::Uuid) -> bool {
        let pflag = self.pause_flag(id);
        let prev = pflag.load(Ordering::SeqCst);
        let next = !prev;
        pflag.store(next, Ordering::SeqCst);

        let mut updated_job: Option<FileOperationJob> = None;
        {
            let mut jobs = self.jobs.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(job) = jobs.iter_mut().find(|j| j.id == id) {
                match &job.status {
                    OperationStatus::InProgress {
                        progress,
                        bytes_processed,
                        total_bytes,
                        ..
                    } if next => {
                        job.status = OperationStatus::Paused {
                            progress: *progress,
                            bytes_processed: *bytes_processed,
                            total_bytes: *total_bytes,
                        };
                        updated_job = Some(job.clone());
                    }
                    OperationStatus::Paused {
                        progress,
                        bytes_processed,
                        total_bytes,
                    } if !next => {
                        // Reset speed tracker timestamp so pause duration doesn't skew speed
                        if let Ok(mut trackers) = self.speed_trackers.lock() {
                            if let Some(t) = trackers.get_mut(&id) {
                                t.last_time = Instant::now();
                            }
                        }
                        job.status = OperationStatus::InProgress {
                            progress: *progress,
                            bytes_processed: *bytes_processed,
                            total_bytes: *total_bytes,
                            speed: 0.0,
                            eta: None,
                        };
                        updated_job = Some(job.clone());
                    }
                    _ => {}
                }
            }
        }
        if let Some(j) = updated_job {
            self.broadcast(JobUpdate::UpdateJob(j));
        }
        next
    }

    /// Byte-level progress report with current filename. Computes speed/ETA,
    /// appends a speed sample for the graph, and broadcasts to subscribers.
    pub async fn report(
        &self,
        id: uuid::Uuid,
        bytes_done: u64,
        total_bytes: u64,
        current_file: &str,
    ) {
        self.report_with_items(id, bytes_done, total_bytes, 0, 0, current_file)
            .await;
    }

    /// Detailed progress report including item counts (`items_done` / `total_items`).
    pub async fn report_with_items(
        &self,
        id: uuid::Uuid,
        bytes_done: u64,
        total_bytes: u64,
        items_done: u64,
        total_items: u64,
        current_file: &str,
    ) {
        let progress = if total_bytes > 0 {
            (bytes_done as f32 / total_bytes as f32).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let speed = self.calculate_speed(id, bytes_done).await;
        let eta = if speed > 0.0 && total_bytes > bytes_done {
            Some(Duration::from_secs(
                ((total_bytes - bytes_done) as f64 / speed) as u64,
            ))
        } else {
            None
        };
        let status = OperationStatus::InProgress {
            progress,
            bytes_processed: bytes_done,
            total_bytes,
            speed,
            eta,
        };
        let mut snapshot = None;
        {
            let mut jobs = self.jobs.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(job) = jobs.iter_mut().find(|j| j.id == id) {
                job.status = status.clone();
                if !current_file.is_empty() {
                    job.current_file = current_file.to_string();
                }
                if total_items > 0 {
                    job.items_processed = items_done;
                    job.total_items = total_items;
                }
                job.speed_history.push(speed as f32);
                if job.speed_history.len() > 40 {
                    job.speed_history.remove(0);
                }
                snapshot = Some(job.clone());
            }
        }
        if let Some(job) = snapshot {
            self.broadcast(JobUpdate::UpdateJob(job));
        } else {
            self.broadcast(JobUpdate::UpdateStatus(id, status));
        }
    }

    pub async fn update_progress(&self, id: uuid::Uuid, progress: f32, bytes: u64, total: u64) {
        let speed = self.calculate_speed(id, bytes).await;
        let eta = if speed > 0.0 && total >= bytes {
            Some(Duration::from_secs(((total - bytes) as f64 / speed) as u64))
        } else {
            None
        };

        let status = OperationStatus::InProgress {
            progress,
            bytes_processed: bytes,
            total_bytes: total,
            speed,
            eta,
        };

        let mut jobs = self.jobs.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(job) = jobs.iter_mut().find(|j| j.id == id) {
            job.status = status.clone();
        }
        drop(jobs);

        self.broadcast(JobUpdate::UpdateStatus(id, status));
    }

    pub async fn complete_job(&self, id: uuid::Uuid) {
        let mut jobs = self.jobs.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(job) = jobs.iter_mut().find(|j| j.id == id) {
            job.status = OperationStatus::Completed;
            if job.total_items > 0 {
                job.items_processed = job.total_items;
            }
        }
        drop(jobs);
        self.broadcast(JobUpdate::Complete(id));
    }

    pub async fn fail_job(&self, id: uuid::Uuid, reason: String) {
        let mut jobs = self.jobs.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(job) = jobs.iter_mut().find(|j| j.id == id) {
            job.status = OperationStatus::Failed(reason.clone());
        }
        drop(jobs);
        self.broadcast(JobUpdate::Fail(id, reason));
    }

    pub async fn get_job(&self, id: uuid::Uuid) -> Option<FileOperationJob> {
        let jobs = self.jobs.lock().unwrap_or_else(|e| e.into_inner());
        jobs.iter().find(|j| j.id == id).cloned()
    }

    pub async fn list_jobs(&self) -> Vec<FileOperationJob> {
        self.jobs.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    pub async fn remove_job(&self, id: uuid::Uuid) {
        let mut jobs = self.jobs.lock().unwrap_or_else(|e| e.into_inner());
        jobs.retain(|j| j.id != id);
    }

    async fn calculate_speed(&self, id: uuid::Uuid, bytes: u64) -> f64 {
        let now = Instant::now();
        if let Ok(mut trackers) = self.speed_trackers.lock() {
            if let Some(tracker) = trackers.get_mut(&id) {
                let dt = now.duration_since(tracker.last_time).as_secs_f64();
                if dt >= 0.05 {
                    let delta_bytes = bytes.saturating_sub(tracker.last_bytes);
                    let inst_speed = delta_bytes as f64 / dt;
                    tracker.last_bytes = bytes;
                    tracker.last_time = now;
                    if tracker.smoothed_speed <= 0.0 {
                        tracker.smoothed_speed = inst_speed;
                    } else {
                        tracker.smoothed_speed = tracker.smoothed_speed * 0.65 + inst_speed * 0.35;
                    }
                    return tracker.smoothed_speed;
                } else if tracker.smoothed_speed > 0.0 {
                    return tracker.smoothed_speed;
                }
            }
        }

        let jobs = self.jobs.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(job) = jobs.iter().find(|j| j.id == id) {
            let elapsed = job.created_at.elapsed().as_secs_f64();
            if elapsed > 0.0 {
                return bytes as f64 / elapsed;
            }
        }
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_start_job() {
        let engine = ProgressEngine::new();
        let _rx = engine.subscribe();

        let job_id = engine.start_job("Test job".to_string(), OpKind::Other).await;
        assert!(job_id != uuid::Uuid::nil());

        let job = engine.get_job(job_id).await;
        assert!(job.is_some());
        assert_eq!(job.unwrap().name, "Test job");
    }

    #[tokio::test]
    async fn test_get_nonexistent_job() {
        let engine = ProgressEngine::new();
        let _rx = engine.subscribe();

        let job = engine.get_job(uuid::Uuid::nil()).await;
        assert!(job.is_none());
    }

    #[tokio::test]
    async fn test_list_jobs() {
        let engine = ProgressEngine::new();
        let _rx = engine.subscribe();

        let id1 = engine.start_job("Job 1".to_string(), OpKind::Other).await;
        let id2 = engine.start_job("Job 2".to_string(), OpKind::Other).await;

        let jobs = engine.list_jobs().await;
        assert_eq!(jobs.len(), 2);
        assert!(jobs.iter().any(|j| j.id == id1));
        assert!(jobs.iter().any(|j| j.id == id2));
    }

    #[tokio::test]
    async fn test_remove_job() {
        let engine = ProgressEngine::new();
        let _rx = engine.subscribe();

        let job_id = engine.start_job("Test job".to_string(), OpKind::Other).await;
        assert!(engine.get_job(job_id).await.is_some());

        engine.remove_job(job_id).await;
        assert!(engine.get_job(job_id).await.is_none());
    }

    #[tokio::test]
    async fn test_operation_status() {
        let status = OperationStatus::InProgress {
            progress: 0.5,
            bytes_processed: 50,
            total_bytes: 100,
            speed: 10.0,
            eta: Some(Duration::from_secs(5)),
        };

        assert!(matches!(status, OperationStatus::InProgress { progress, .. } if progress == 0.5));
    }

    #[tokio::test]
    async fn test_complete_job_sends_update() {
        let engine = ProgressEngine::new();
        let _rx = engine.subscribe();

        let job_id = engine.start_job("Test job".to_string(), OpKind::Other).await;
        engine.complete_job(job_id).await;

        let job = engine.get_job(job_id).await;
        assert!(job.is_some());
    }

    #[tokio::test]
    async fn test_fail_job_sends_update() {
        let engine = ProgressEngine::new();
        let _rx = engine.subscribe();

        let job_id = engine.start_job("Test job".to_string(), OpKind::Other).await;
        let error_msg = "Something went wrong".to_string();

        engine.fail_job(job_id, error_msg.clone()).await;

        let job = engine.get_job(job_id).await;
        assert!(job.is_some());
    }

    #[test]
    fn test_operation_status_variants() {
        use OperationStatus::*;

        let pending = Pending;
        assert!(matches!(pending, Pending));

        let completed = Completed;
        assert!(matches!(completed, Completed));

        let failed = Failed("error".to_string());
        assert!(matches!(failed, Failed(msg) if msg == "error"));

        let in_progress = InProgress {
            progress: 0.75,
            bytes_processed: 75,
            total_bytes: 100,
            speed: 50.0,
            eta: None,
        };
        assert!(matches!(in_progress, InProgress { progress, .. } if progress == 0.75));
    }
}

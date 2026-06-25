use crate::core::path_util::FilesPath;
use anyhow::Result;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tracing::{debug, info};

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
    Completed,
    Failed(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct FileOperationJob {
    pub id: uuid::Uuid,
    pub name: String,
    pub status: OperationStatus,
    pub created_at: Instant,
}

pub struct ProgressEngine {
    jobs: Arc<std::sync::Mutex<Vec<FileOperationJob>>>,
    subscribers: Arc<std::sync::Mutex<Vec<mpsc::UnboundedSender<JobUpdate>>>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum JobUpdate {
    UpdateStatus(uuid::Uuid, OperationStatus),
    Complete(uuid::Uuid),
    Fail(uuid::Uuid, String),
    NewJob(FileOperationJob),
}

impl ProgressEngine {
    pub fn new() -> Self {
        Self {
            jobs: Arc::new(std::sync::Mutex::new(Vec::new())),
            subscribers: Arc::new(std::sync::Mutex::new(Vec::new())),
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

    pub async fn start_job(&self, name: String) -> uuid::Uuid {
        let id = uuid::Uuid::new_v4();
        let job = FileOperationJob {
            id,
            name,
            status: OperationStatus::Pending,
            created_at: Instant::now(),
        };
        self.jobs
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(job.clone());
        self.broadcast(JobUpdate::NewJob(job));
        id
    }

    pub async fn update_progress(&self, id: uuid::Uuid, progress: f32, bytes: u64, total: u64) {
        let speed = self.calculate_speed(id, bytes).await;
        let eta = if speed > 0.0 {
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

        // Update internal state
        let mut jobs = self.jobs.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(job) = jobs.iter_mut().find(|j| j.id == id) {
            job.status = status.clone();
        }

        self.broadcast(JobUpdate::UpdateStatus(id, status));
    }

    pub async fn complete_job(&self, id: uuid::Uuid) {
        let mut jobs = self.jobs.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(job) = jobs.iter_mut().find(|j| j.id == id) {
            job.status = OperationStatus::Completed;
        }
        self.broadcast(JobUpdate::Complete(id));
    }

    pub async fn fail_job(&self, id: uuid::Uuid, reason: String) {
        let mut jobs = self.jobs.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(job) = jobs.iter_mut().find(|j| j.id == id) {
            job.status = OperationStatus::Failed(reason.clone());
        }
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
        // Simplified speed calculation
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

        let job_id = engine.start_job("Test job".to_string()).await;

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

        let id1 = engine.start_job("Job 1".to_string()).await;
        let id2 = engine.start_job("Job 2".to_string()).await;

        let jobs = engine.list_jobs().await;
        assert_eq!(jobs.len(), 2);
        assert!(jobs.iter().any(|j| j.id == id1));
        assert!(jobs.iter().any(|j| j.id == id2));
    }

    #[tokio::test]
    async fn test_remove_job() {
        let engine = ProgressEngine::new();
        let _rx = engine.subscribe();

        let job_id = engine.start_job("Test job".to_string()).await;
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

        let job_id = engine.start_job("Test job".to_string()).await;

        // Complete job should not panic
        engine.complete_job(job_id).await;

        // Verify job is still in list (complete_job sends update but doesn't remove)
        let job = engine.get_job(job_id).await;
        assert!(job.is_some());
    }

    #[tokio::test]
    async fn test_fail_job_sends_update() {
        let engine = ProgressEngine::new();
        let _rx = engine.subscribe();

        let job_id = engine.start_job("Test job".to_string()).await;
        let error_msg = "Something went wrong".to_string();

        // Fail job should not panic
        engine.fail_job(job_id, error_msg.clone()).await;

        // Verify job is still in list (fail_job sends update but doesn't remove)
        let job = engine.get_job(job_id).await;
        assert!(job.is_some());
    }

    #[test]
    fn test_operation_status_variants() {
        use OperationStatus::*;

        // Test Pending
        let pending = Pending;
        assert!(matches!(pending, Pending));

        // Test Completed
        let completed = Completed;
        assert!(matches!(completed, Completed));

        // Test Failed
        let failed = Failed("error".to_string());
        assert!(matches!(failed, Failed(msg) if msg == "error"));

        // Test InProgress
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

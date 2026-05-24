use anyhow::Result;
use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::Instant;

/// Batches file system change events with configurable debounce delay.
/// Prevents event flooding by collecting changes and releasing them
/// after a quiet period elapses.
pub struct DebouncedFileWatcher {
    /// Internal watcher for file system events
    watcher: RecommendedWatcher,
    /// The debounce duration
    debounce_duration: Duration,
    /// Handle to the background task
    _processing_task: tokio::task::JoinHandle<()>,
}

impl DebouncedFileWatcher {
    /// Creates a new DebouncedFileWatcher with the specified debounce duration.
    ///
    /// # Arguments
    /// * `debounce_duration` - How long to wait after the last event before
    ///   releasing the batch
    ///
    /// # Returns
    /// A new DebouncedFileWatcher instance and a receiver for batches of events
    pub fn new(debounce_duration: Duration) -> Result<(Self, mpsc::Receiver<Vec<Event>>)> {
        let (batch_sender, batch_receiver) = mpsc::channel(100);
        let (path_sender, mut path_receiver) = mpsc::channel::<PathBuf>(1000);

        let debounce_duration_clone = debounce_duration;

        // Spawn the event processing task
        let processing_task = tokio::spawn(async move {
            let mut pending_paths: HashSet<PathBuf> = HashSet::new();
            let mut debounce_end: Option<Instant> = None;

            loop {
                if pending_paths.is_empty() {
                    // No pending events, wait for next path
                    match path_receiver.recv().await {
                        Some(path) => {
                            pending_paths.insert(path);
                            debounce_end = Some(Instant::now() + debounce_duration_clone);
                        }
                        None => break, // Channel closed
                    }
                } else {
                    // Have pending events, wait for either new paths or timeout
                    let timeout_duration = debounce_end
                        .map(|end| {
                            let now = Instant::now();
                            if end > now {
                                end - now
                            } else {
                                Duration::from_secs(0)
                            }
                        })
                        .unwrap_or(Duration::from_secs(0));

                    tokio::select! {
                        path_opt = path_receiver.recv() => {
                            match path_opt {
                                Some(path) => {
                                    pending_paths.insert(path);
                                    debounce_end = Some(Instant::now() + debounce_duration_clone);
                                }
                                None => break, // Channel closed
                            }
                        }
                        _ = tokio::time::sleep(timeout_duration), if debounce_end.is_some() => {
                            // Timer expired, send all pending events
                            let paths: Vec<PathBuf> = pending_paths.drain().collect();
                            let events: Vec<Event> = paths
                                .into_iter()
                                .map(|path| Event {
                                    paths: vec![path],
                                    kind: notify::EventKind::Any,
                                    attrs: notify::event::EventAttributes::default(),
                                })
                                .collect();

                            if batch_sender.send(events).await.is_err() {
                                // Receiver dropped, exit
                                break;
                            }
                            debounce_end = None;
                        }
                    }
                }
            }
        });

        // Create the notify watcher
        let watcher = RecommendedWatcher::new(
            move |res: Result<Event, notify::Error>| {
                if let Ok(event) = res {
                    for path in &event.paths {
                        let _ = path_sender.blocking_send(path.clone());
                    }
                }
            },
            Config::default(),
        )?;

        Ok((
            DebouncedFileWatcher {
                watcher,
                debounce_duration,
                _processing_task: processing_task,
            },
            batch_receiver,
        ))
    }

    /// Starts watching a path for changes.
    ///
    /// # Arguments
    /// * `path` - The path to watch
    /// * `recursive` - Whether to watch recursively
    pub fn watch(&mut self, path: impl Into<PathBuf>, recursive: bool) -> Result<()> {
        let mode = if recursive {
            RecursiveMode::Recursive
        } else {
            RecursiveMode::NonRecursive
        };
        self.watcher.watch(path.into().as_path(), mode)?;
        Ok(())
    }

    /// Stops watching a path.
    ///
    /// # Arguments
    /// * `path` - The path to stop watching
    pub fn unwatch(&mut self, path: impl Into<PathBuf>) -> Result<()> {
        self.watcher.unwatch(path.into().as_path())?;
        Ok(())
    }

    /// Returns the debounce duration.
    pub fn debounce_duration(&self) -> Duration {
        self.debounce_duration
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use tokio::time::{sleep, timeout, Duration};

    #[tokio::test]
    async fn test_debounced_file_watcher_creation() {
        let temp_dir = TempDir::new().unwrap();
        let (mut watcher, _receiver) =
            DebouncedFileWatcher::new(Duration::from_millis(100)).unwrap();

        watcher
            .watch(temp_dir.path(), false)
            .expect("Failed to watch temp dir");

        assert_eq!(watcher.debounce_duration(), Duration::from_millis(100));
    }

    #[tokio::test]
    async fn test_debounce_batches_multiple_events() {
        let temp_dir = TempDir::new().unwrap();
        let debounce_duration = Duration::from_millis(50);

        let (mut watcher, mut receiver) = DebouncedFileWatcher::new(debounce_duration).unwrap();

        watcher
            .watch(temp_dir.path(), true)
            .expect("Failed to watch temp dir");

        // Create multiple files rapidly
        let temp_path = temp_dir.path();
        tokio::fs::write(temp_path.join("file1.txt"), "content1")
            .await
            .unwrap();
        tokio::fs::write(temp_path.join("file2.txt"), "content2")
            .await
            .unwrap();
        tokio::fs::write(temp_path.join("file3.txt"), "content3")
            .await
            .unwrap();

        // Wait for debounce to elapse
        sleep(Duration::from_millis(150)).await;

        // Should receive a single batch with multiple events
        let result = timeout(Duration::from_secs(2), receiver.recv()).await;
        assert!(result.is_ok(), "Should receive batched events");

        let events = result.unwrap().unwrap();
        // Multiple file changes should be batched together
        assert!(
            events.len() >= 3,
            "Expected at least 3 events batched, got {}",
            events.len()
        );
    }

    #[tokio::test]
    async fn test_debounce_resets_on_new_events() {
        let temp_dir = TempDir::new().unwrap();
        let debounce_duration = Duration::from_millis(100);

        let (mut watcher, mut receiver) = DebouncedFileWatcher::new(debounce_duration).unwrap();

        watcher
            .watch(temp_dir.path(), true)
            .expect("Failed to watch temp dir");

        let temp_path = temp_dir.path();

        // Write first file
        tokio::fs::write(temp_path.join("file1.txt"), "content1")
            .await
            .unwrap();

        // Wait less than debounce, then write another file
        sleep(Duration::from_millis(50)).await;
        tokio::fs::write(temp_path.join("file2.txt"), "content2")
            .await
            .unwrap();

        // Wait less than debounce again, then write another file
        sleep(Duration::from_millis(50)).await;
        tokio::fs::write(temp_path.join("file3.txt"), "content3")
            .await
            .unwrap();

        // Wait for the final debounce to elapse
        sleep(Duration::from_millis(200)).await;

        // Should receive a single batch with all events (debounce was reset)
        let result = timeout(Duration::from_secs(2), receiver.recv()).await;
        assert!(result.is_ok(), "Should receive batched events");

        let events = result.unwrap().unwrap();
        // All 3 events should be batched together
        assert!(
            events.len() >= 3,
            "Expected at least 3 events batched, got {}",
            events.len()
        );
    }

    #[tokio::test]
    async fn test_watch_and_unwatch() {
        let temp_dir = TempDir::new().unwrap();
        let debounce_duration = Duration::from_millis(50);

        let (mut watcher, _receiver) = DebouncedFileWatcher::new(debounce_duration).unwrap();

        // Watch should succeed
        watcher
            .watch(temp_dir.path(), false)
            .expect("Failed to watch temp dir");

        // Unwatch should succeed
        watcher
            .unwatch(temp_dir.path())
            .expect("Failed to unwatch temp dir");
    }

    #[tokio::test]
    async fn test_recursive_watch() {
        let temp_dir = TempDir::new().unwrap();
        let debounce_duration = Duration::from_millis(50);

        let (mut watcher, _receiver) = DebouncedFileWatcher::new(debounce_duration).unwrap();

        watcher
            .watch(temp_dir.path(), true)
            .expect("Failed to watch temp dir recursively");

        // Create a nested directory and file
        let nested_dir = temp_dir.path().join("nested");
        tokio::fs::create_dir_all(&nested_dir).await.unwrap();
        tokio::fs::write(nested_dir.join("nested_file.txt"), "content")
            .await
            .unwrap();

        // Watcher should not error on recursive setup
        assert_eq!(watcher.debounce_duration(), debounce_duration);
    }

    #[tokio::test]
    async fn test_empty_batch_when_no_events() {
        let temp_dir = TempDir::new().unwrap();
        let debounce_duration = Duration::from_millis(50);

        let (_watcher, mut receiver) = DebouncedFileWatcher::new(debounce_duration).unwrap();

        // Don't generate any events, just check that we can receive (should timeout)
        let result = timeout(Duration::from_millis(100), receiver.recv()).await;
        assert!(result.is_err(), "Should timeout with no events");
    }
}

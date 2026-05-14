//! # Session Service
//!
//! Tracks application session state for crash recovery, window persistence,
//! and session restore.
//!
//! ## Features
//! - Session start/end with "running" flag for crash detection
//! - Window position, size, and maximized state persistence
//! - Crash count tracking for RestoreCrashedSessionDialog

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// Information about the current or last session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    /// UTC timestamp of session start
    pub start_time: u64,
    /// Whether the session ended cleanly (false = crash)
    pub clean_shutdown: bool,
    /// Number of consecutive crashes
    pub crash_count: u32,
    /// Last active directory path
    pub last_path: String,
    /// Window size as (width, height) in pixels
    pub window_size: Option<(f32, f32)>,
    /// Window position as (x, y) in pixels
    pub window_position: Option<(f32, f32)>,
    /// Whether the window was maximized
    pub window_maximized: bool,
}

impl Default for SessionInfo {
    fn default() -> Self {
        Self {
            start_time: now_secs(),
            clean_shutdown: true,
            crash_count: 0,
            last_path: String::new(),
            window_size: None,
            window_position: None,
            window_maximized: true,
        }
    }
}

impl SessionInfo {
    /// Create a new session starting now.
    pub fn new_start() -> Self {
        Self {
            start_time: now_secs(),
            clean_shutdown: false,
            crash_count: 0,
            last_path: String::new(),
            window_size: None,
            window_position: None,
            window_maximized: true,
        }
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Service for managing application sessions.
#[derive(Debug, Clone)]
pub struct SessionService {
    /// Path to the session file on disk
    session_path: PathBuf,
    /// In-memory session info
    info: SessionInfo,
}

impl SessionService {
    /// Create a new session service at the given file path.
    pub fn new(session_path: PathBuf) -> Self {
        let mut s = Self {
            session_path,
            info: SessionInfo::default(),
        };
        s.info = s.load_or_default();
        s
    }

    /// Path to the session marker file (used by config dir).
    pub fn default_session_path() -> PathBuf {
        if let Some(proj_dirs) = directories::ProjectDirs::from("com", "files-rs", "Files-Rust") {
            let data_dir = proj_dirs.data_local_dir();
            data_dir.join("session.json")
        } else {
            PathBuf::from("session.json")
        }
    }

    /// Load session from disk, or return default if missing/corrupt.
    fn load_or_default(&self) -> SessionInfo {
        let path = &self.session_path;
        if path.exists() {
            match std::fs::read_to_string(path) {
                Ok(contents) => match serde_json::from_str::<SessionInfo>(&contents) {
                    Ok(info) => return info,
                    Err(e) => {
                        tracing::warn!("Failed to parse session file: {}. Using default.", e);
                    }
                },
                Err(e) => {
                    tracing::warn!("Failed to read session file: {}. Using default.", e);
                }
            }
        }
        SessionInfo::default()
    }

    /// Start a new session. Sets the running flag and records start time.
    pub fn start_session(&mut self) {
        self.info = SessionInfo {
            start_time: now_secs(),
            clean_shutdown: false,
            crash_count: self.info.crash_count,
            last_path: self.info.last_path.clone(),
            window_size: self.info.window_size,
            window_position: self.info.window_position,
            window_maximized: self.info.window_maximized,
        };
        self.save();
    }

    /// End a session cleanly. Sets clean_shutdown = true.
    pub fn end_session(&mut self) {
        self.info.clean_shutdown = true;
        self.info.crash_count = 0;
        self.save();
    }

    /// Detect if the previous session crashed.
    /// Returns true if the last session did not end cleanly.
    pub fn detect_crash(&self) -> bool {
        // If the running flag is still set from last session, it crashed
        if self.info.start_time > 0 && !self.info.clean_shutdown {
            // Increment crash count (but don't save — that happens on start_session)
            return true;
        }
        false
    }

    /// Get the number of consecutive crashes.
    pub fn crash_count(&self) -> u32 {
        if self.detect_crash() {
            self.info.crash_count + 1
        } else {
            self.info.crash_count
        }
    }

    /// Update the last active path.
    pub fn set_last_path(&mut self, path: &str) {
        self.info.last_path = path.to_string();
        self.save();
    }

    /// Get the last active path.
    pub fn last_path(&self) -> &str {
        &self.info.last_path
    }

    /// Update window state (position, size, maximized).
    pub fn set_window_state(
        &mut self,
        size: Option<(f32, f32)>,
        position: Option<(f32, f32)>,
        maximized: bool,
    ) {
        self.info.window_size = size;
        self.info.window_position = position;
        self.info.window_maximized = maximized;
        self.save();
    }

    /// Get window size.
    pub fn window_size(&self) -> Option<(f32, f32)> {
        self.info.window_size
    }

    /// Get window position.
    pub fn window_position(&self) -> Option<(f32, f32)> {
        self.info.window_position
    }

    /// Get whether window was maximized.
    pub fn window_maximized(&self) -> bool {
        self.info.window_maximized
    }

    /// Get a reference to the current session info.
    pub fn info(&self) -> &SessionInfo {
        &self.info
    }

    /// Get mutable reference to modify the session info and then save.
    pub fn info_mut(&mut self) -> &mut SessionInfo {
        &mut self.info
    }

    /// Persist the current session info to disk.
    pub fn save(&self) {
        if let Some(parent) = self.session_path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                tracing::error!("Failed to create session directory: {}", e);
                return;
            }
        }
        match serde_json::to_string_pretty(&self.info) {
            Ok(json) => {
                if let Err(e) = std::fs::write(&self.session_path, json) {
                    tracing::error!("Failed to write session file: {}", e);
                }
            }
            Err(e) => {
                tracing::error!("Failed to serialize session info: {}", e);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_session_info_default() {
        let info = SessionInfo::default();
        assert!(info.clean_shutdown);
        assert_eq!(info.crash_count, 0);
        assert_eq!(info.last_path, "");
        assert!(info.start_time > 0);
    }

    #[test]
    fn test_session_info_new_start() {
        let info = SessionInfo::new_start();
        assert!(!info.clean_shutdown);
        assert_eq!(info.crash_count, 0);
        assert!(info.start_time > 0);
    }

    #[test]
    fn test_session_service_load_or_default_when_file_missing() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("session.json");
        let service = SessionService::new(path);
        assert!(service.info().clean_shutdown);
        assert_eq!(service.info().crash_count, 0);
    }

    #[test]
    fn test_session_service_start_and_end() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("session.json");
        let mut service = SessionService::new(path.clone());

        service.start_session();
        assert!(!service.info().clean_shutdown);
        assert!(!service.detect_crash());

        service.end_session();
        assert!(service.info().clean_shutdown);
        // After end, detect_crash should return false
        assert!(!service.detect_crash());
    }

    #[test]
    fn test_session_service_crash_detection() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("session.json");

        // Simulate: start session, crash (never call end_session)
        {
            let mut service = SessionService::new(path.clone());
            service.start_session();
            // Drop without calling end_session -> crash
        }

        // Next launch should detect crash
        {
            let service = SessionService::new(path.clone());
            assert!(service.detect_crash());
            assert_eq!(service.crash_count(), 1);
        }

        // Start a new session (this resets, but crash_count increments)
        {
            let mut service = SessionService::new(path.clone());
            assert!(service.detect_crash());
            service.start_session();
            // detect_crash should now return false for current session
            // (previous crash detected, new session started)
        }
    }

    #[test]
    fn test_session_service_persists_window_state() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("session.json");
        let mut service = SessionService::new(path.clone());

        service.set_window_state(Some((1920.0, 1080.0)), Some((100.0, 50.0)), true);

        // Create new service loading from same file
        let service2 = SessionService::new(path.clone());
        assert_eq!(service2.window_size(), Some((1920.0, 1080.0)));
        assert_eq!(service2.window_position(), Some((100.0, 50.0)));
        assert!(service2.window_maximized());
    }

    #[test]
    fn test_session_service_persists_last_path() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("session.json");
        let mut service = SessionService::new(path.clone());

        service.set_last_path(r"C:\Users\Test\Documents");

        let service2 = SessionService::new(path.clone());
        assert_eq!(service2.last_path(), r"C:\Users\Test\Documents");
    }

    #[test]
    fn test_session_service_crash_count_increments() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("session.json");

        // First session: start and crash
        {
            let mut service = SessionService::new(path.clone());
            service.start_session();
            // crash - no end_session
        }

        // Second session: detect crash, count should be 1
        {
            let service = SessionService::new(path.clone());
            assert_eq!(service.crash_count(), 1);
        }

        // Start second session and crash again
        {
            let mut service = SessionService::new(path.clone());
            service.start_session();
            // crash again
        }

        // Third session: count should be 2
        {
            let service = SessionService::new(path.clone());
            assert_eq!(service.crash_count(), 2);
        }

        // Clean shutdown resets crash count
        {
            let mut service = SessionService::new(path.clone());
            service.start_session();
            service.end_session();
        }

        // Fourth session: crash count should be 0
        {
            let service = SessionService::new(path.clone());
            assert_eq!(service.crash_count(), 0);
            assert!(!service.detect_crash());
        }
    }

    #[test]
    fn test_session_service_save_persists_to_disk() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("session.json");

        let mut service = SessionService::new(path.clone());
        service.set_last_path("/tmp/test");
        service.set_window_state(Some((800.0, 600.0)), None, false);

        // Read the file directly and verify
        let contents = fs::read_to_string(&path).unwrap();
        let loaded: SessionInfo = serde_json::from_str(&contents).unwrap();
        assert_eq!(loaded.last_path, "/tmp/test");
        assert_eq!(loaded.window_size, Some((800.0, 600.0)));
    }
}

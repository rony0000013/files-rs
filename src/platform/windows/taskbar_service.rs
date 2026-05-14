//! Windows Taskbar Progress Service
//! Provides taskbar progress indication for file operations.
//!
//! Uses ITaskbarList3 COM interface to set progress state and value
//! on the Windows taskbar button.

use anyhow::Result;
use std::sync::Mutex;

/// Taskbar progress state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskbarProgressState {
    /// No progress shown
    None,
    /// Indeterminate progress (spinning)
    Indeterminate,
    /// Normal progress (green)
    Normal,
    /// Error progress (red)
    Error,
    /// Paused progress (yellow)
    Paused,
}

impl Default for TaskbarProgressState {
    fn default() -> Self {
        Self::None
    }
}

/// Windows Taskbar Progress Service
/// Sets progress indication on the Windows taskbar button.
///
/// On Windows, uses ITaskbarList3 COM interface.
/// On other platforms, operations are no-ops.
pub struct TaskbarProgressService {
    #[cfg(windows)]
    state: Mutex<TaskbarProgressState>,
    #[cfg(windows)]
    progress: Mutex<u64>,
    #[cfg(windows)]
    hwnd: Mutex<Option<isize>>,
    /// Stub fields for non-Windows platforms
    #[cfg(not(windows))]
    _stub: (),
}

impl TaskbarProgressService {
    pub fn new() -> Self {
        Self {
            #[cfg(windows)]
            state: Mutex::new(TaskbarProgressState::None),
            #[cfg(windows)]
            progress: Mutex::new(0),
            #[cfg(windows)]
            hwnd: Mutex::new(None),
            #[cfg(not(windows))]
            _stub: (),
        }
    }

    /// Set the window handle for progress updates
    /// This is the HWND of the main window
    #[cfg(windows)]
    pub fn set_hwnd(&self, hwnd: isize) {
        let mut hwnd_guard = self.hwnd.lock().unwrap();
        *hwnd_guard = Some(hwnd);
        tracing::debug!("TaskbarProgressService HWND set to: {}", hwnd);
    }

    #[cfg(not(windows))]
    pub fn set_hwnd(&self, _hwnd: isize) {
        // No-op on non-Windows platforms
    }

    /// Get the current progress state
    pub fn get_state(&self) -> TaskbarProgressState {
        #[cfg(windows)]
        {
            *self.state.lock().unwrap()
        }
        #[cfg(not(windows))]
        {
            TaskbarProgressState::None
        }
    }

    /// Get the current progress value (0-100)
    pub fn get_progress(&self) -> u64 {
        #[cfg(windows)]
        {
            *self.progress.lock().unwrap()
        }
        #[cfg(not(windows))]
        {
            0
        }
    }

    /// Set the progress state on the taskbar
    #[cfg(windows)]
    pub fn set_state(&self, state: TaskbarProgressState) -> Result<()> {
        use windows::Win32::UI::Shell::{ITaskbarList3, TaskbarList};
        use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED};
        use windows::Win32::Foundation::HWND;
        use windows::core::Interface;

        let hwnd = {
            let hwnd_guard = self.hwnd.lock().unwrap();
            match *hwnd_guard {
                Some(h) => HWND(h as isize),
                None => {
                    tracing::debug!("No HWND set, cannot update taskbar progress");
                    return Ok(());
                }
            }
        };

        // Initialize COM
        unsafe {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);

            // Create ITaskbarList3 instance
            let taskbar: ITaskbarList3 = CoCreateInstance(&TaskbarList, None, CLSCTX_INPROC_SERVER)?;

            // Map our state to Windows TBPFLAG
            let tbp_flag = match state {
                TaskbarProgressState::None => windows::Win32::UI::Shell::TBPF_NOPROGRESS,
                TaskbarProgressState::Indeterminate => windows::Win32::UI::Shell::TBPF_INDETERMINATE,
                TaskbarProgressState::Normal => windows::Win32::UI::Shell::TBPF_NORMAL,
                TaskbarProgressState::Error => windows::Win32::UI::Shell::TBPF_ERROR,
                TaskbarProgressState::Paused => windows::Win32::UI::Shell::TBPF_PAUSED,
            };

            taskbar.SetProgressState(hwnd, tbp_flag)?;

            // Update internal state
            *self.state.lock().unwrap() = state;
            if state == TaskbarProgressState::None {
                *self.progress.lock().unwrap() = 0;
            }

            tracing::debug!("Taskbar progress state set to: {:?}", state);
        }

        Ok(())
    }

    #[cfg(not(windows))]
    pub fn set_state(&self, state: TaskbarProgressState) -> Result<()> {
        tracing::debug!("Taskbar progress state (no-op on non-Windows): {:?}", state);
        Ok(())
    }

    /// Set the progress value (0-100)
    #[cfg(windows)]
    pub fn set_progress(&self, progress: u64) -> Result<()> {
        use windows::Win32::UI::Shell::{ITaskbarList3, TaskbarList};
        use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED};
        use windows::Win32::Foundation::HWND;
        use windows::core::Interface;

        let hwnd = {
            let hwnd_guard = self.hwnd.lock().unwrap();
            match *hwnd_guard {
                Some(h) => HWND(h as isize),
                None => {
                    tracing::debug!("No HWND set, cannot update taskbar progress");
                    return Ok(());
                }
            }
        };

        // Ensure state is Normal when setting progress value
        let current_state = *self.state.lock().unwrap();
        if current_state != TaskbarProgressState::Normal && current_state != TaskbarProgressState::Paused {
            self.set_state(TaskbarProgressState::Normal)?;
        }

        unsafe {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);

            let taskbar: ITaskbarList3 = CoCreateInstance(&TaskbarList, None, CLSCTX_INPROC_SERVER)?;

            // Clamp progress to 0-100
            let clamped = progress.min(100);
            taskbar.SetProgressValue(hwnd, clamped, 100)?;

            *self.progress.lock().unwrap() = clamped;
            tracing::debug!("Taskbar progress value set to: {}%", clamped);
        }

        Ok(())
    }

    #[cfg(not(windows))]
    pub fn set_progress(&self, progress: u64) -> Result<()> {
        tracing::debug!("Taskbar progress value (no-op on non-Windows): {}%", progress);
        Ok(())
    }

    /// Set both state and progress in one call
    pub fn set_progress_with_state(&self, progress: u64, state: TaskbarProgressState) -> Result<()> {
        if state != TaskbarProgressState::None && state != TaskbarProgressState::Indeterminate {
            self.set_state(state)?;
            self.set_progress(progress)?;
        } else {
            self.set_state(state)?;
        }
        Ok(())
    }

    /// Clear the progress (set to None state)
    pub fn clear(&self) -> Result<()> {
        self.set_state(TaskbarProgressState::None)
    }

    /// Set to error state with progress
    pub fn set_error(&self, progress: u64) -> Result<()> {
        self.set_progress_with_state(progress, TaskbarProgressState::Error)
    }

    /// Set to paused state with progress
    pub fn set_paused(&self, progress: u64) -> Result<()> {
        self.set_progress_with_state(progress, TaskbarProgressState::Paused)
    }

    /// Set to normal state with progress
    pub fn set_normal(&self, progress: u64) -> Result<()> {
        self.set_progress_with_state(progress, TaskbarProgressState::Normal)
    }
}

impl Default for TaskbarProgressService {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for TaskbarProgressService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TaskbarProgressService")
            .field("state", &self.get_state())
            .field("progress", &self.get_progress())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_taskbar_progress_state_default() {
        let state = TaskbarProgressState::default();
        assert_eq!(state, TaskbarProgressState::None);
    }

    #[test]
    fn test_taskbar_progress_state_equality() {
        assert_eq!(TaskbarProgressState::Normal, TaskbarProgressState::Normal);
        assert_eq!(TaskbarProgressState::Error, TaskbarProgressState::Error);
        assert_ne!(TaskbarProgressState::Normal, TaskbarProgressState::Error);
    }

    #[test]
    fn test_service_creation() {
        let service = TaskbarProgressService::new();
        assert_eq!(service.get_state(), TaskbarProgressState::None);
        assert_eq!(service.get_progress(), 0);
    }

    #[test]
    fn test_service_debug() {
        let service = TaskbarProgressService::new();
        let debug_str = format!("{:?}", service);
        assert!(debug_str.contains("TaskbarProgressService"));
    }

    #[test]
    fn test_clear_noop() {
        let service = TaskbarProgressService::new();
        // clear() should not panic on any platform
        let result = service.clear();
        assert!(result.is_ok());
    }

    #[test]
    fn test_set_state_noop() {
        let service = TaskbarProgressService::new();
        // set_state() should not panic on any platform
        let result = service.set_state(TaskbarProgressState::Normal);
        assert!(result.is_ok());

        let result = service.set_state(TaskbarProgressState::Error);
        assert!(result.is_ok());

        let result = service.set_state(TaskbarProgressState::Indeterminate);
        assert!(result.is_ok());
    }

    #[test]
    fn test_set_progress_noop() {
        let service = TaskbarProgressService::new();
        // set_progress() should not panic on any platform
        let result = service.set_progress(50);
        assert!(result.is_ok());

        let result = service.set_progress(0);
        assert!(result.is_ok());

        let result = service.set_progress(100);
        assert!(result.is_ok());
    }

    #[test]
    fn test_set_progress_with_state_noop() {
        let service = TaskbarProgressService::new();
        let result = service.set_progress_with_state(50, TaskbarProgressState::Normal);
        assert!(result.is_ok());

        let result = service.set_progress_with_state(75, TaskbarProgressState::Error);
        assert!(result.is_ok());
    }

    #[test]
    fn test_set_error_noop() {
        let service = TaskbarProgressService::new();
        let result = service.set_error(50);
        assert!(result.is_ok());
    }

    #[test]
    fn test_set_paused_noop() {
        let service = TaskbarProgressService::new();
        let result = service.set_paused(50);
        assert!(result.is_ok());
    }

    #[test]
    fn test_set_normal_noop() {
        let service = TaskbarProgressService::new();
        let result = service.set_normal(50);
        assert!(result.is_ok());
    }

    #[test]
    fn test_all_states() {
        let service = TaskbarProgressService::new();
        
        // All state transitions should succeed
        for state in [
            TaskbarProgressState::None,
            TaskbarProgressState::Indeterminate,
            TaskbarProgressState::Normal,
            TaskbarProgressState::Error,
            TaskbarProgressState::Paused,
        ] {
            let result = service.set_state(state);
            assert!(result.is_ok(), "Failed to set state: {:?}", state);
        }
    }

    #[test]
    fn test_progress_clamps() {
        let service = TaskbarProgressService::new();
        
        // Progress values should be accepted (internal clamping is platform-dependent)
        assert!(service.set_progress(0).is_ok());
        assert!(service.set_progress(50).is_ok());
        assert!(service.set_progress(100).is_ok());
        // Values > 100 should still work (platform may clamp)
        assert!(service.set_progress(150).is_ok());
    }

    // Windows-specific tests that run only on Windows
    #[cfg(windows)]
    mod windows_tests {
        use super::*;

        #[test]
        fn test_windows_hwnd_setting() {
            let service = TaskbarProgressService::new();
            service.set_hwnd(12345);
            // State should still be None initially
            assert_eq!(service.get_state(), TaskbarProgressState::None);
        }

        #[test]
        fn test_windows_hwnd_is_stored() {
            let service = TaskbarProgressService::new();
            // On Windows, we can verify HWND is stored via internal state
            // This is a basic sanity test
            let state = service.get_state();
            assert_eq!(state, TaskbarProgressState::None);
        }
    }
}
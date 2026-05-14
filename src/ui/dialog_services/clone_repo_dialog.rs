//! Clone Repository Dialog
//! State machine for cloning a git repository.

use std::path::PathBuf;

/// State for the Clone Repository dialog
#[derive(Debug, Clone, PartialEq)]
pub struct CloneRepoState {
    /// The URL of the repository to clone
    pub url: String,
    /// The destination path where the repo will be cloned
    pub destination_path: PathBuf,
    /// Optional branch to checkout after cloning
    pub branch: Option<String>,
    /// Whether a clone operation is in progress
    pub is_cloning: bool,
    /// Clone progress percentage (0.0 - 100.0)
    pub progress: f32,
}

impl CloneRepoState {
    /// Create a new CloneRepoState with default values
    pub fn new() -> Self {
        Self {
            url: String::new(),
            destination_path: PathBuf::new(),
            branch: None,
            is_cloning: false,
            progress: 0.0,
        }
    }

    /// Set the repository URL
    pub fn set_url(&mut self, url: String) {
        self.url = url;
    }

    /// Set the destination path
    pub fn set_destination(&mut self, path: PathBuf) {
        self.destination_path = path;
    }

    /// Validate the URL — must not be empty
    pub fn validate_url(&self) -> Result<(), String> {
        if self.url.trim().is_empty() {
            return Err("Repository URL cannot be empty".to_string());
        }
        if !self.url.starts_with("http://")
            && !self.url.starts_with("https://")
            && !self.url.starts_with("git@")
            && !self.url.starts_with("ssh://")
        {
            return Err("Invalid repository URL format".to_string());
        }
        Ok(())
    }

    /// Validate the destination path — must not be empty
    pub fn validate_destination(&self) -> Result<(), String> {
        if self.destination_path.as_os_str().is_empty() {
            return Err("Destination path cannot be empty".to_string());
        }
        Ok(())
    }

    /// Start the clone operation
    pub fn start_cloning(&mut self) {
        self.is_cloning = true;
        self.progress = 0.0;
    }

    /// Update clone progress
    pub fn update_progress(&mut self, progress: f32) {
        self.progress = progress.clamp(0.0, 100.0);
    }

    /// Mark cloning as complete
    pub fn complete_cloning(&mut self) {
        self.is_cloning = false;
        self.progress = 100.0;
    }

    /// Reset the state
    pub fn reset(&mut self) {
        self.url = String::new();
        self.destination_path = PathBuf::new();
        self.branch = None;
        self.is_cloning = false;
        self.progress = 0.0;
    }
}

impl Default for CloneRepoState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clone_repo_state_new() {
        let state = CloneRepoState::new();
        assert!(state.url.is_empty());
        assert!(state.destination_path.as_os_str().is_empty());
        assert!(state.branch.is_none());
        assert!(!state.is_cloning);
        assert_eq!(state.progress, 0.0);
    }

    #[test]
    fn test_clone_repo_state_validate_empty_url() {
        let state = CloneRepoState::new();
        let result = state.validate_url();
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Repository URL cannot be empty");
    }

    #[test]
    fn test_clone_repo_state_set_url() {
        let mut state = CloneRepoState::new();
        state.set_url("https://github.com/user/repo.git".to_string());
        assert_eq!(state.url, "https://github.com/user/repo.git");
        assert!(state.validate_url().is_ok());
    }

    #[test]
    fn test_clone_repo_state_set_destination() {
        let mut state = CloneRepoState::new();
        state.set_destination(PathBuf::from("/home/user/projects"));
        assert_eq!(state.destination_path, PathBuf::from("/home/user/projects"));
        assert!(state.validate_destination().is_ok());
    }

    #[test]
    fn test_clone_repo_state_validate_invalid_url() {
        let mut state = CloneRepoState::new();
        state.set_url("invalid-url".to_string());
        let result = state.validate_url();
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Invalid repository URL format");
    }

    #[test]
    fn test_clone_repo_state_start_cloning() {
        let mut state = CloneRepoState::new();
        state.set_url("https://github.com/user/repo.git".to_string());
        state.start_cloning();
        assert!(state.is_cloning);
        assert_eq!(state.progress, 0.0);
    }

    #[test]
    fn test_clone_repo_state_update_progress() {
        let mut state = CloneRepoState::new();
        state.update_progress(50.0);
        assert_eq!(state.progress, 50.0);
        state.update_progress(150.0);
        assert_eq!(state.progress, 100.0);
        state.update_progress(-10.0);
        assert_eq!(state.progress, 0.0);
    }

    #[test]
    fn test_clone_repo_state_complete_cloning() {
        let mut state = CloneRepoState::new();
        state.start_cloning();
        state.update_progress(75.0);
        state.complete_cloning();
        assert!(!state.is_cloning);
        assert_eq!(state.progress, 100.0);
    }

    #[test]
    fn test_clone_repo_state_reset() {
        let mut state = CloneRepoState::new();
        state.set_url("https://github.com/user/repo.git".to_string());
        state.set_destination(PathBuf::from("/tmp/clone"));
        state.start_cloning();
        state.update_progress(50.0);
        state.reset();
        assert!(state.url.is_empty());
        assert!(state.destination_path.as_os_str().is_empty());
        assert!(!state.is_cloning);
        assert_eq!(state.progress, 0.0);
    }

    #[test]
    fn test_clone_repo_state_default() {
        let state = CloneRepoState::default();
        assert!(state.url.is_empty());
    }
}

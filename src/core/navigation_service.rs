use crate::core::navigation_manager::{NavigationState, PaneFocus};
use crate::core::path_util::FilesPath;
use anyhow::{anyhow, Result};
use std::path::PathBuf;

pub struct NavigationService;

impl NavigationService {
    pub fn get_parent_path(path: &FilesPath) -> Result<FilesPath> {
        let parent = path
            .as_path()
            .parent()
            .ok_or_else(|| anyhow!("Path has no parent"))?;
        Ok(FilesPath::new(parent.to_path_buf()))
    }

    pub fn get_home_path() -> Result<FilesPath> {
        let user_dirs = directories::UserDirs::new()
            .ok_or_else(|| anyhow!("Could not find user directories"))?;

        let home = user_dirs.home_dir().to_path_buf();

        Ok(FilesPath::new(home))
    }

    pub fn navigate_up(state: &mut NavigationState) -> Option<FilesPath> {
        let current = state.tabs.active_tab().path.clone();
        if let Ok(parent) = Self::get_parent_path(&current) {
            state.tabs.active_tab_mut().navigate(parent.clone());
            Some(parent)
        } else {
            None
        }
    }

    pub fn navigate_home(state: &mut NavigationState) -> Option<FilesPath> {
        if let Ok(home) = Self::get_home_path() {
            state.tabs.active_tab_mut().navigate(home.clone());
            Some(home)
        } else {
            None
        }
    }

    pub fn navigate_back(state: &mut NavigationState) -> Option<FilesPath> {
        let tab = state.tabs.active_tab_mut();
        tab.navigate_back()
    }

    pub fn navigate_forward(state: &mut NavigationState) -> Option<FilesPath> {
        let tab = state.tabs.active_tab_mut();
        tab.navigate_forward()
    }

    pub fn can_go_back(state: &NavigationState) -> bool {
        state.tabs.active_tab().can_go_back()
    }

    pub fn can_go_forward(state: &NavigationState) -> bool {
        state.tabs.active_tab().can_go_forward()
    }

    pub fn back_history(state: &NavigationState) -> Vec<FilesPath> {
        state
            .tabs
            .active_tab()
            .back_history()
            .iter()
            .cloned()
            .collect()
    }

    pub fn forward_history(state: &NavigationState) -> Vec<FilesPath> {
        state
            .tabs
            .active_tab()
            .forward_history()
            .iter()
            .cloned()
            .collect()
    }

    pub fn navigate_to_path(state: &mut NavigationState, path: FilesPath) {
        // Update the correct pane
        match state.active_pane {
            PaneFocus::Primary => {
                state.tabs.active_tab_mut().navigate(path);
            }
            PaneFocus::Secondary => {
                let index = state.tabs.active_tab_index;
                if let Some(tab) = state.tabs.tabs.get_mut(index) {
                    tab.secondary_pane_path = Some(path);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_path(s: &str) -> FilesPath {
        FilesPath::new(s)
    }

    fn make_state(path: &str) -> NavigationState {
        NavigationState::new(make_path(path), crate::config::config::ViewMode::List)
    }

    // === NavigationService Tests ===

    #[cfg(windows)]
    #[test]
    fn test_get_parent_path_valid() {
        let path = make_path("C:\\Users\\Test");
        let parent = NavigationService::get_parent_path(&path).unwrap();
        assert_eq!(parent.as_path(), PathBuf::from("C:\\Users"));
    }

    #[cfg(windows)]
    #[test]
    fn test_get_parent_path_root() {
        let path = make_path("C:\\");
        let result = NavigationService::get_parent_path(&path);
        assert!(result.is_err());
    }

    #[cfg(windows)]
    #[test]
    fn test_get_parent_path_single_segment() {
        let path = make_path("C:\\Users");
        let parent = NavigationService::get_parent_path(&path).unwrap();
        assert_eq!(parent.as_path(), PathBuf::from("C:\\"));
    }

    #[cfg(windows)]
    #[test]
    fn test_navigate_up() {
        let mut state = make_state("C:\\Users\\Test");
        let result = NavigationService::navigate_up(&mut state);

        assert!(result.is_some());
        assert_eq!(result.unwrap().as_path(), PathBuf::from("C:\\Users"));
    }

    #[cfg(windows)]
    #[test]
    fn test_navigate_up_at_root() {
        let mut state = make_state("C:\\");
        let result = NavigationService::navigate_up(&mut state);

        assert!(result.is_none());
    }

    #[test]
    fn test_navigate_up_clears_forward_history() {
        let mut state = make_state("C:\\0");
        state.tabs.active_tab_mut().navigate(make_path("C:\\1"));
        state.tabs.active_tab_mut().navigate_back();

        NavigationService::navigate_up(&mut state);

        assert!(state.tabs.active_tab().history_forward.is_empty());
    }

    #[test]
    fn test_navigate_home() {
        let mut state = make_state("/home/user/downloads");
        let result = NavigationService::navigate_home(&mut state);

        assert!(result.is_some());
    }

    #[test]
    fn test_navigate_back() {
        let mut state = make_state("/home/user");
        state.tabs.active_tab_mut().navigate(make_path("/home"));

        let result = NavigationService::navigate_back(&mut state);

        assert!(result.is_some());
    }

    #[test]
    fn test_navigate_forward() {
        let mut state = make_state("/home/user");
        state.tabs.active_tab_mut().navigate(make_path("/home"));
        state.tabs.active_tab_mut().navigate_back();

        let result = NavigationService::navigate_forward(&mut state);

        assert!(result.is_some());
    }

    #[test]
    fn test_can_go_back_true() {
        let mut state = make_state("/home/user/test");
        state
            .tabs
            .active_tab_mut()
            .navigate(make_path("/home/user"));

        assert!(NavigationService::can_go_back(&state));
    }

    #[test]
    fn test_can_go_back_false() {
        let state = make_state("/home/user/test");
        assert!(!NavigationService::can_go_back(&state));
    }

    #[test]
    fn test_can_go_forward_true() {
        let mut state = make_state("/home/user/test");
        state
            .tabs
            .active_tab_mut()
            .navigate(make_path("/home/user"));
        state.tabs.active_tab_mut().navigate_back();

        assert!(NavigationService::can_go_forward(&state));
    }

    #[test]
    fn test_can_go_forward_false() {
        let state = make_state("/home/user/test");
        assert!(!NavigationService::can_go_forward(&state));
    }

    #[test]
    fn test_back_history() {
        let mut state = make_state("/home/user/test");
        state
            .tabs
            .active_tab_mut()
            .navigate(make_path("/home/user"));
        state.tabs.active_tab_mut().navigate(make_path("/home"));

        let history = NavigationService::back_history(&state);
        assert_eq!(history.len(), 2);
    }

    #[test]
    fn test_forward_history() {
        let mut state = make_state("/home/user/test");
        state
            .tabs
            .active_tab_mut()
            .navigate(make_path("/home/user"));
        state.tabs.active_tab_mut().navigate_back();

        let history = NavigationService::forward_history(&state);
        assert!(!history.is_empty());
    }

    #[test]
    fn test_navigate_to_path_primary_pane() {
        let mut state = make_state("/home/user");
        NavigationService::navigate_to_path(&mut state, make_path("/home/downloads"));

        assert_eq!(
            state.tabs.active_tab().path.as_path(),
            PathBuf::from("/home/downloads")
        );
    }

    #[test]
    fn test_navigate_to_path_secondary_pane() {
        let mut state = make_state("/home/user");
        state.split_pane_horizontal();
        state.focus_other_pane();

        NavigationService::navigate_to_path(&mut state, make_path("/home/downloads"));

        assert_eq!(
            state
                .tabs
                .active_tab()
                .secondary_pane_path
                .as_ref()
                .unwrap()
                .as_path(),
            PathBuf::from("/home/downloads")
        );
    }

    // === Integration Tests ===

    #[cfg(windows)]
    #[test]
    fn test_navigation_up_then_back() {
        let mut state = make_state("C:\\Users\\Test");

        // Navigate up
        let parent = NavigationService::navigate_up(&mut state).unwrap();
        assert_eq!(parent.as_path(), PathBuf::from("C:\\Users"));

        // Can go back
        assert!(NavigationService::can_go_back(&state));

        // Navigate back
        let result = NavigationService::navigate_back(&mut state);
        assert!(result.is_some());
    }

    #[test]
    fn test_navigate_preserves_scroll_offset_reset() {
        let mut state = make_state("/home/user/test");
        state.tabs.active_tab_mut().scroll_offset_px = 500.0;

        NavigationService::navigate_up(&mut state);

        assert_eq!(state.tabs.active_tab().scroll_offset_px, 0.0);
    }
}

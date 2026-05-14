use crate::config::config::{SortBy, ViewMode};
use crate::core::path_util::FilesPath;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SortState {
    pub column: SortBy,
    pub ascending: bool,
    pub folder_order: FolderOrder,
    pub group_by: Option<GroupColumn>,
}

impl Default for SortState {
    fn default() -> Self {
        Self {
            column: SortBy::Name,
            ascending: true,
            folder_order: FolderOrder::FoldersFirst,
            group_by: None,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum FolderOrder {
    FoldersFirst,
    FilesFirst,
    Interleaved,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum GroupColumn {
    Name,
    DateModified,
    DateCreated,
    Size,
    FileType,
    SyncStatus,
    FileTag,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TabState {
    pub path: FilesPath,
    pub view_mode: ViewMode,
    pub scroll_offset_px: f32,
    pub sort: SortState,
    pub history_back: VecDeque<FilesPath>,
    pub history_forward: VecDeque<FilesPath>,
    pub secondary_pane_path: Option<FilesPath>,
    pub selection: Vec<FilesPath>,
}

const MAX_HISTORY: usize = 50;

impl TabState {
    pub fn new(path: FilesPath, view_mode: ViewMode) -> Self {
        Self {
            path: path.clone(),
            view_mode,
            scroll_offset_px: 0.0,
            sort: SortState::default(),
            history_back: VecDeque::with_capacity(MAX_HISTORY),
            history_forward: VecDeque::with_capacity(MAX_HISTORY),
            secondary_pane_path: None,
            selection: Vec::new(),
        }
    }

    pub fn navigate(&mut self, path: FilesPath) {
        if self.path == path {
            return;
        }
        // Push current to back stack, limit size
        if self.history_back.len() >= MAX_HISTORY {
            self.history_back.pop_front();
        }
        self.history_back.push_back(self.path.clone());
        self.history_forward.clear();
        self.path = path;
        self.selection.clear();
        self.scroll_offset_px = 0.0;
    }

    pub fn navigate_back(&mut self) -> Option<FilesPath> {
        if let Some(prev) = self.history_back.pop_back() {
            if self.history_forward.len() >= MAX_HISTORY {
                self.history_forward.pop_front();
            }
            self.history_forward.push_back(self.path.clone());
            self.path = prev.clone();
            self.selection.clear();
            Some(prev)
        } else {
            None
        }
    }

    pub fn navigate_forward(&mut self) -> Option<FilesPath> {
        if let Some(next) = self.history_forward.pop_back() {
            if self.history_back.len() >= MAX_HISTORY {
                self.history_back.pop_front();
            }
            self.history_back.push_back(self.path.clone());
            self.path = next.clone();
            self.selection.clear();
            Some(next)
        } else {
            None
        }
    }

    pub fn can_go_back(&self) -> bool {
        !self.history_back.is_empty()
    }

    pub fn can_go_forward(&self) -> bool {
        !self.history_forward.is_empty()
    }

    pub fn back_history(&self) -> &VecDeque<FilesPath> {
        &self.history_back
    }

    pub fn forward_history(&self) -> &VecDeque<FilesPath> {
        &self.history_forward
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TabManagerState {
    pub tabs: Vec<TabState>,
    pub active_tab_index: usize,
    pub closed_tabs: VecDeque<TabState>,
}

const MAX_CLOSED_TABS: usize = 10;

impl TabManagerState {
    pub fn new(initial_path: FilesPath, default_mode: ViewMode) -> Self {
        Self {
            tabs: vec![TabState::new(initial_path, default_mode)],
            active_tab_index: 0,
            closed_tabs: VecDeque::with_capacity(MAX_CLOSED_TABS),
        }
    }

    pub fn active_tab(&self) -> &TabState {
        &self.tabs[self.active_tab_index]
    }

    pub fn active_tab_mut(&mut self) -> &mut TabState {
        &mut self.tabs[self.active_tab_index]
    }

    pub fn add_tab(&mut self, path: FilesPath, mode: ViewMode) {
        self.tabs.push(TabState::new(path, mode));
        self.active_tab_index = self.tabs.len() - 1;
    }

    pub fn close_tab(&mut self, index: usize) {
        if self.tabs.len() <= 1 {
            return;
        }
        let closed = self.tabs.remove(index);
        self.closed_tabs.push_front(closed);
        if self.closed_tabs.len() > MAX_CLOSED_TABS {
            self.closed_tabs.pop_back();
        }
        if self.active_tab_index >= self.tabs.len() {
            self.active_tab_index = self.tabs.len() - 1;
        }
    }

    pub fn close_all_tabs(&mut self) {
        while self.tabs.len() > 1 {
            let idx = self.tabs.len() - 1;
            self.close_tab(idx);
        }
        // Reset the remaining tab to home
        if let Some(tab) = self.tabs.first_mut() {
            let home = directories::UserDirs::new()
                .map(|d| FilesPath::new(d.home_dir().to_path_buf()))
                .unwrap_or_else(|| FilesPath::from("C:\\"));
            tab.path = home.clone();
            tab.history_back.clear();
            tab.history_forward.clear();
            tab.selection.clear();
            tab.secondary_pane_path = None;
        }
        self.active_tab_index = 0;
    }

    pub fn close_other_tabs(&mut self) {
        let active = self.active_tab_index;
        let mut kept = Vec::new();
        let current = self.tabs.remove(active);
        kept.push(current);

        // Save closed tabs for reopen
        for mut tab in self.tabs.drain(..) {
            tab.secondary_pane_path = None;
            self.closed_tabs.push_front(tab);
        }
        while self.closed_tabs.len() > MAX_CLOSED_TABS {
            self.closed_tabs.pop_back();
        }

        self.tabs = kept;
        self.active_tab_index = 0;
    }

    pub fn close_tabs_to_left(&mut self) {
        let active = self.active_tab_index;
        if active == 0 {
            return;
        }
        for i in (0..active).rev() {
            let mut tab = self.tabs.remove(i);
            tab.secondary_pane_path = None;
            self.closed_tabs.push_front(tab);
        }
        while self.closed_tabs.len() > MAX_CLOSED_TABS {
            self.closed_tabs.pop_back();
        }
        self.active_tab_index = 0;
    }

    pub fn close_tabs_to_right(&mut self) {
        let active = self.active_tab_index;
        while self.tabs.len() > active + 1 {
            let idx = self.tabs.len() - 1;
            let mut tab = self.tabs.remove(idx);
            tab.secondary_pane_path = None;
            self.closed_tabs.push_front(tab);
        }
        while self.closed_tabs.len() > MAX_CLOSED_TABS {
            self.closed_tabs.pop_back();
        }
    }

    pub fn duplicate_tab(&mut self) {
        let current = self.active_tab().clone();
        self.tabs.push(current);
        self.active_tab_index = self.tabs.len() - 1;
    }

    pub fn switch_tab(&mut self, index: usize) {
        if index < self.tabs.len() {
            self.active_tab_index = index;
        }
    }

    pub fn next_tab(&mut self) {
        if self.tabs.is_empty() {
            return;
        }
        self.active_tab_index = (self.active_tab_index + 1) % self.tabs.len();
    }

    pub fn previous_tab(&mut self) {
        if self.tabs.is_empty() {
            return;
        }
        if self.active_tab_index == 0 {
            self.active_tab_index = self.tabs.len() - 1;
        } else {
            self.active_tab_index -= 1;
        }
    }

    pub fn reopen_closed_tab(&mut self) -> bool {
        if let Some(tab) = self.closed_tabs.pop_front() {
            self.tabs.push(tab);
            self.active_tab_index = self.tabs.len() - 1;
            true
        } else {
            false
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PaneLayout {
    Single,
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PaneFocus {
    Primary,
    Secondary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NavigationState {
    pub tabs: TabManagerState,
    pub pane_layout: PaneLayout,
    pub active_pane: PaneFocus,
}

impl NavigationState {
    pub fn new(initial_path: FilesPath, default_mode: ViewMode) -> Self {
        Self {
            tabs: TabManagerState::new(initial_path, default_mode),
            pane_layout: PaneLayout::Single,
            active_pane: PaneFocus::Primary,
        }
    }

    pub fn split_pane_horizontal(&mut self) {
        self.pane_layout = PaneLayout::Horizontal;
        let active = self.tabs.active_tab_mut();
        if active.secondary_pane_path.is_none() {
            active.secondary_pane_path = Some(active.path.clone());
        }
    }

    pub fn split_pane_vertical(&mut self) {
        self.pane_layout = PaneLayout::Vertical;
        let active = self.tabs.active_tab_mut();
        if active.secondary_pane_path.is_none() {
            active.secondary_pane_path = Some(active.path.clone());
        }
    }

    pub fn close_active_pane(&mut self) {
        let active = self.tabs.active_tab_mut();
        active.secondary_pane_path = None;
        self.pane_layout = PaneLayout::Single;
        self.active_pane = PaneFocus::Primary;
    }

    pub fn toggle_dual_pane(&mut self) {
        match self.pane_layout {
            PaneLayout::Single => {
                self.pane_layout = PaneLayout::Horizontal;
                let active = self.tabs.active_tab_mut();
                if active.secondary_pane_path.is_none() {
                    active.secondary_pane_path = Some(active.path.clone());
                }
            }
            _ => {
                self.close_active_pane();
            }
        }
    }

    pub fn focus_other_pane(&mut self) {
        self.active_pane = match self.active_pane {
            PaneFocus::Primary => PaneFocus::Secondary,
            PaneFocus::Secondary => PaneFocus::Primary,
        };
    }

    pub fn active_pane_path(&self) -> FilesPath {
        let tab = self.tabs.active_tab();
        match self.active_pane {
            PaneFocus::Primary => tab.path.clone(),
            PaneFocus::Secondary => tab
                .secondary_pane_path
                .clone()
                .unwrap_or_else(|| tab.path.clone()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_path(s: &str) -> FilesPath {
        FilesPath::new(s)
    }

    fn make_tab(path: &str) -> TabState {
        TabState::new(make_path(path), ViewMode::List)
    }

    // === TabState Tests ===

    #[test]
    fn test_tab_state_new() {
        let tab = make_tab("/home/user");
        assert_eq!(tab.path.as_path(), std::path::Path::new("/home/user"));
        assert_eq!(tab.view_mode, ViewMode::List);
        assert_eq!(tab.scroll_offset_px, 0.0);
        assert!(tab.history_back.is_empty());
        assert!(tab.history_forward.is_empty());
        assert!(tab.selection.is_empty());
        assert!(tab.secondary_pane_path.is_none());
    }

    #[test]
    fn test_tab_state_navigate() {
        let mut tab = make_tab("/home/user");
        tab.navigate(make_path("/home"));

        assert_eq!(tab.path.as_path(), std::path::Path::new("/home"));
        assert!(!tab.history_back.is_empty());
        assert!(tab.history_forward.is_empty());
    }

    #[test]
    fn test_tab_state_navigate_same_path_no_change() {
        let mut tab = make_tab("/home/user");
        tab.navigate(make_path("/home/user"));

        assert!(tab.history_back.is_empty());
        assert!(tab.history_forward.is_empty());
    }

    #[test]
    fn test_tab_state_navigate_clears_selection() {
        let mut tab = make_tab("/home/user");
        tab.selection = vec![make_path("/file.txt")];
        tab.navigate(make_path("/home"));

        assert!(tab.selection.is_empty());
    }

    #[test]
    fn test_tab_state_navigate_back() {
        let mut tab = make_tab("/home/user");
        tab.navigate(make_path("/home"));
        tab.navigate(make_path("/"));

        let prev = tab.navigate_back();
        assert_eq!(
            prev.map(|p| p.as_path().to_string_lossy().to_string()),
            Some("/home".into())
        );
        assert_eq!(tab.path.as_path(), std::path::Path::new("/home"));
        assert!(!tab.history_forward.is_empty());
    }

    #[test]
    fn test_tab_state_navigate_back_empty() {
        let mut tab = make_tab("/home/user");
        assert!(tab.navigate_back().is_none());
    }

    #[test]
    fn test_tab_state_navigate_forward() {
        let mut tab = make_tab("/home/user");
        tab.navigate(make_path("/home"));
        tab.navigate_back();
        tab.navigate_back();

        let next = tab.navigate_forward();
        assert_eq!(
            next.map(|p| p.as_path().to_string_lossy().to_string()),
            Some("/home".into())
        );
        assert_eq!(tab.path.as_path(), std::path::Path::new("/home"));
    }

    #[test]
    fn test_tab_state_navigate_forward_empty() {
        let mut tab = make_tab("/home/user");
        assert!(tab.navigate_forward().is_none());
    }

    #[test]
    fn test_tab_state_can_go_back() {
        let mut tab = make_tab("/home/user");
        assert!(!tab.can_go_back());
        tab.navigate(make_path("/home"));
        assert!(tab.can_go_back());
    }

    #[test]
    fn test_tab_state_can_go_forward() {
        let mut tab = make_tab("/home/user");
        tab.navigate(make_path("/home"));
        tab.navigate_back();
        assert!(tab.can_go_forward());
    }

    #[test]
    fn test_tab_state_back_history() {
        let mut tab = make_tab("/home/user");
        tab.navigate(make_path("/home"));
        tab.navigate(make_path("/"));

        let history = tab.back_history();
        assert_eq!(history.len(), 2);
    }

    #[test]
    fn test_tab_state_forward_history() {
        let mut tab = make_tab("/home/user");
        tab.navigate(make_path("/home"));
        tab.navigate_back();

        let history = tab.forward_history();
        assert!(!history.is_empty());
    }

    #[test]
    fn test_tab_state_history_max_50() {
        let mut tab = make_tab("/0");
        for i in 1..=55 {
            tab.navigate(make_path(&format!("/{}", i)));
        }

        assert!(tab.history_back.len() <= 50);
    }

    // === TabManagerState Tests ===

    #[test]
    fn test_tab_manager_new() {
        let mgr = TabManagerState::new(make_path("/home"), ViewMode::List);
        assert_eq!(mgr.tabs.len(), 1);
        assert_eq!(mgr.active_tab_index, 0);
        assert!(mgr.closed_tabs.is_empty());
    }

    #[test]
    fn test_tab_manager_active_tab() {
        let mgr = TabManagerState::new(make_path("/home"), ViewMode::List);
        let tab = mgr.active_tab();
        assert_eq!(tab.path.as_path(), std::path::Path::new("/home"));
    }

    #[test]
    fn test_tab_manager_add_tab() {
        let mut mgr = TabManagerState::new(make_path("/home"), ViewMode::List);
        mgr.add_tab(make_path("/downloads"), ViewMode::Grid);

        assert_eq!(mgr.tabs.len(), 2);
        assert_eq!(mgr.active_tab_index, 1);
    }

    #[test]
    fn test_tab_manager_close_tab() {
        let mut mgr = TabManagerState::new(make_path("/home"), ViewMode::List);
        mgr.add_tab(make_path("/downloads"), ViewMode::Grid);
        mgr.add_tab(make_path("/documents"), ViewMode::List);

        mgr.close_tab(1);

        assert_eq!(mgr.tabs.len(), 2);
        assert_eq!(mgr.active_tab_index, 1);
        assert!(!mgr.closed_tabs.is_empty());
    }

    #[test]
    fn test_tab_manager_close_tab_does_not_close_last_tab() {
        let mut mgr = TabManagerState::new(make_path("/home"), ViewMode::List);
        mgr.close_tab(0);

        assert_eq!(mgr.tabs.len(), 1);
    }

    #[test]
    fn test_tab_manager_close_tab_adjusts_active_index() {
        let mut mgr = TabManagerState::new(make_path("/home"), ViewMode::List);
        mgr.add_tab(make_path("/downloads"), ViewMode::Grid);
        mgr.add_tab(make_path("/documents"), ViewMode::List);
        // tabs=["/home", "/downloads", "/documents"], active=2

        // Close tab at index 0 (not active)
        mgr.close_tab(0);

        // tabs=["/downloads", "/documents"], active was 2, removed index 0
        // so active_index = min(2, 2-1) = 1 after clamping
        assert_eq!(mgr.active_tab_index, 1);
    }

    #[test]
    fn test_tab_manager_close_other_tabs() {
        let mut mgr = TabManagerState::new(make_path("/home"), ViewMode::List);
        mgr.add_tab(make_path("/downloads"), ViewMode::Grid);
        mgr.add_tab(make_path("/documents"), ViewMode::List);

        mgr.close_other_tabs();

        assert_eq!(mgr.tabs.len(), 1);
    }

    #[test]
    fn test_tab_manager_close_tabs_to_left() {
        let mut mgr = TabManagerState::new(make_path("/home"), ViewMode::List);
        mgr.add_tab(make_path("/downloads"), ViewMode::Grid);
        mgr.add_tab(make_path("/documents"), ViewMode::List);
        mgr.switch_tab(2);

        mgr.close_tabs_to_left();

        assert_eq!(mgr.tabs.len(), 1);
        assert_eq!(mgr.active_tab_index, 0);
    }

    #[test]
    fn test_tab_manager_close_all_tabs() {
        let mut mgr = TabManagerState::new(make_path("/home"), ViewMode::List);
        mgr.add_tab(make_path("/downloads"), ViewMode::Grid);
        mgr.add_tab(make_path("/documents"), ViewMode::List);

        mgr.close_all_tabs();

        assert_eq!(mgr.tabs.len(), 1);
        assert_eq!(mgr.active_tab_index, 0);
    }

    #[test]
    fn test_tab_manager_close_tabs_to_right() {
        let mut mgr = TabManagerState::new(make_path("/home"), ViewMode::List);
        mgr.add_tab(make_path("/downloads"), ViewMode::Grid);
        mgr.add_tab(make_path("/documents"), ViewMode::List);
        // active is 2 (last tab), nothing to close to the right
        // tabs = ["/home", "/downloads", "/documents"]
        mgr.close_tabs_to_right();

        assert_eq!(mgr.tabs.len(), 3); // nothing closed since active is last
    }

    #[test]
    fn test_tab_manager_close_tabs_to_right_from_first() {
        let mut mgr = TabManagerState::new(make_path("/home"), ViewMode::List);
        mgr.add_tab(make_path("/downloads"), ViewMode::Grid);
        mgr.add_tab(make_path("/documents"), ViewMode::List);
        mgr.switch_tab(0); // active = 0

        // Now should close tabs 2 and 1, leaving only tab 0
        mgr.close_tabs_to_right();

        assert_eq!(mgr.tabs.len(), 1);
        assert_eq!(mgr.active_tab_index, 0);
    }

    #[test]
    fn test_tab_manager_duplicate_tab() {
        let mut mgr = TabManagerState::new(make_path("/home"), ViewMode::List);
        let original_path = mgr.active_tab().path.clone();
        mgr.duplicate_tab();

        assert_eq!(mgr.tabs.len(), 2);
        assert_eq!(mgr.active_tab_index, 1);
        assert_eq!(mgr.tabs[1].path.as_path(), original_path.as_path());
    }

    #[test]
    fn test_tab_manager_switch_tab() {
        let mut mgr = TabManagerState::new(make_path("/home"), ViewMode::List);
        mgr.add_tab(make_path("/downloads"), ViewMode::Grid);
        mgr.switch_tab(0);

        assert_eq!(mgr.active_tab_index, 0);
    }

    #[test]
    fn test_tab_manager_next_tab() {
        let mut mgr = TabManagerState::new(make_path("/home"), ViewMode::List);
        mgr.add_tab(make_path("/downloads"), ViewMode::Grid);
        mgr.add_tab(make_path("/documents"), ViewMode::List);
        // After adding 3 tabs, active = 2 (last added)

        mgr.next_tab(); // (2 + 1) % 3 = 0
        assert_eq!(mgr.active_tab_index, 0);

        mgr.next_tab(); // (0 + 1) % 3 = 1
        assert_eq!(mgr.active_tab_index, 1);

        mgr.next_tab(); // (1 + 1) % 3 = 2
        assert_eq!(mgr.active_tab_index, 2);

        mgr.next_tab(); // (2 + 1) % 3 = 0
        assert_eq!(mgr.active_tab_index, 0);
    }

    #[test]
    fn test_tab_manager_previous_tab() {
        let mut mgr = TabManagerState::new(make_path("/home"), ViewMode::List);
        mgr.add_tab(make_path("/downloads"), ViewMode::Grid);
        mgr.add_tab(make_path("/documents"), ViewMode::List);
        // After adding 3 tabs, active = 2 (last added)

        mgr.previous_tab(); // 2 -> 1 (since 2 != 0)
        assert_eq!(mgr.active_tab_index, 1);

        mgr.previous_tab(); // 1 -> 0 (since 1 != 0)
        assert_eq!(mgr.active_tab_index, 0);

        mgr.previous_tab(); // 0 -> wraps to len-1 = 2
        assert_eq!(mgr.active_tab_index, 2);
    }

    #[test]
    fn test_tab_manager_reopen_closed_tab() {
        let mut mgr = TabManagerState::new(make_path("/home"), ViewMode::List);
        mgr.add_tab(make_path("/downloads"), ViewMode::Grid);
        // Now: tabs=["/home", "/downloads"], active=1

        mgr.close_tab(0);
        // tabs=["/downloads"], active=0 (shifted after removing index 0)

        let reopened = mgr.reopen_closed_tab();

        assert!(reopened);
        assert_eq!(mgr.tabs.len(), 2);
        assert_eq!(mgr.active_tab_index, 1);
    }

    #[test]
    fn test_tab_manager_reopen_closed_tab_empty() {
        let mut mgr = TabManagerState::new(make_path("/home"), ViewMode::List);
        let reopened = mgr.reopen_closed_tab();

        assert!(!reopened);
    }

    #[test]
    fn test_closed_tabs_max_10() {
        let mut mgr = TabManagerState::new(make_path("/0"), ViewMode::List);

        // Add 12 tabs to have 12 tabs total
        for i in 1..=11 {
            mgr.add_tab(make_path(&format!("/{}", i)), ViewMode::List);
        }
        // Now we have 12 tabs (0 to 11), active = 11

        // Close all 11 tabs to the left of active (index 11)
        mgr.close_tabs_to_left();

        // Should have 1 tab left (index 0) and 11 in closed_tabs
        assert_eq!(mgr.tabs.len(), 1);
        assert!(mgr.closed_tabs.len() <= 10); // capped at 10
    }

    // === NavigationState Tests ===

    #[test]
    fn test_navigation_state_new() {
        let state = NavigationState::new(make_path("/home"), ViewMode::List);
        assert_eq!(state.tabs.tabs.len(), 1);
        assert_eq!(state.pane_layout, PaneLayout::Single);
        assert_eq!(state.active_pane, PaneFocus::Primary);
    }

    #[test]
    fn test_navigation_state_split_horizontal() {
        let mut state = NavigationState::new(make_path("/home"), ViewMode::List);
        state.split_pane_horizontal();

        assert_eq!(state.pane_layout, PaneLayout::Horizontal);
        assert!(state.tabs.active_tab().secondary_pane_path.is_some());
    }

    #[test]
    fn test_navigation_state_split_vertical() {
        let mut state = NavigationState::new(make_path("/home"), ViewMode::List);
        state.split_pane_vertical();

        assert_eq!(state.pane_layout, PaneLayout::Vertical);
        assert!(state.tabs.active_tab().secondary_pane_path.is_some());
    }

    #[test]
    fn test_navigation_state_close_active_pane() {
        let mut state = NavigationState::new(make_path("/home"), ViewMode::List);
        state.split_pane_horizontal();
        state.close_active_pane();

        assert_eq!(state.pane_layout, PaneLayout::Single);
        assert!(state.tabs.active_tab().secondary_pane_path.is_none());
        assert_eq!(state.active_pane, PaneFocus::Primary);
    }

    #[test]
    fn test_navigation_state_toggle_dual_pane() {
        let mut state = NavigationState::new(make_path("/home"), ViewMode::List);
        state.toggle_dual_pane();

        assert_eq!(state.pane_layout, PaneLayout::Horizontal);

        state.toggle_dual_pane();
        assert_eq!(state.pane_layout, PaneLayout::Single);
    }

    #[test]
    fn test_navigation_state_focus_other_pane() {
        let mut state = NavigationState::new(make_path("/home"), ViewMode::List);
        state.split_pane_horizontal();

        state.focus_other_pane();
        assert_eq!(state.active_pane, PaneFocus::Secondary);

        state.focus_other_pane();
        assert_eq!(state.active_pane, PaneFocus::Primary);
    }

    #[test]
    fn test_navigation_state_active_pane_path_primary() {
        let state = NavigationState::new(make_path("/home"), ViewMode::List);
        let path = state.active_pane_path();

        assert_eq!(path.as_path(), std::path::Path::new("/home"));
    }

    #[test]
    fn test_navigation_state_active_pane_path_secondary() {
        let mut state = NavigationState::new(make_path("/home"), ViewMode::List);
        state.split_pane_horizontal();
        state.focus_other_pane();

        let path = state.active_pane_path();
        assert_eq!(path.as_path(), std::path::Path::new("/home"));
    }

    // === SortState Tests ===

    #[test]
    fn test_sort_state_default() {
        let sort = SortState::default();
        assert_eq!(sort.column, SortBy::Name);
        assert!(sort.ascending);
        assert_eq!(sort.folder_order, FolderOrder::FoldersFirst);
        assert!(sort.group_by.is_none());
    }

    // === FolderOrder Tests ===

    #[test]
    fn test_folder_order_variants() {
        assert_eq!(FolderOrder::FoldersFirst, FolderOrder::FoldersFirst);
        assert_eq!(FolderOrder::FilesFirst, FolderOrder::FilesFirst);
        assert_eq!(FolderOrder::Interleaved, FolderOrder::Interleaved);
    }

    // === GroupColumn Tests ===

    #[test]
    fn test_group_column_variants() {
        assert_eq!(GroupColumn::Name, GroupColumn::Name);
        assert_eq!(GroupColumn::DateModified, GroupColumn::DateModified);
        assert_eq!(GroupColumn::DateCreated, GroupColumn::DateCreated);
        assert_eq!(GroupColumn::Size, GroupColumn::Size);
        assert_eq!(GroupColumn::FileType, GroupColumn::FileType);
        assert_eq!(GroupColumn::SyncStatus, GroupColumn::SyncStatus);
        assert_eq!(GroupColumn::FileTag, GroupColumn::FileTag);
    }

    // === PaneLayout Tests ===

    #[test]
    fn test_pane_layout_variants() {
        assert_eq!(PaneLayout::Single, PaneLayout::Single);
        assert_eq!(PaneLayout::Horizontal, PaneLayout::Horizontal);
        assert_eq!(PaneLayout::Vertical, PaneLayout::Vertical);
    }

    // === PaneFocus Tests ===

    #[test]
    fn test_pane_focus_variants() {
        assert_eq!(PaneFocus::Primary, PaneFocus::Primary);
        assert_eq!(PaneFocus::Secondary, PaneFocus::Secondary);
    }
}


//! Tab Advanced Interactions — Cross-window dragging, hover-to-select, and scroll-to-switch.
//! Per DESIGN_SPEC.md section 6 / TODO.md Section 6.
//!
//! This module provides advanced tab interaction features:
//! - Cross-window tab dragging
//! - Hover-to-select tabs
//! - Scroll-to-switch tabs

use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// Tab drag state for cross-window operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TabDragState {
    /// Not dragging.
    Idle,
    /// Dragging started, contains source tab id.
    Dragging { source_tab_id: u64 },
    /// Dragging over a target window.
    OverTarget { target_window_id: u64, insert_position: InsertPosition },
    /// Drop completed.
    Dropped,
    /// Drag cancelled.
    Cancelled,
}

/// Position for inserting a dragged tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InsertPosition {
    /// Insert before the tab.
    Before,
    /// Insert after the tab.
    After,
}

/// Hover state for a tab.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabHoverState {
    /// Whether the tab is currently hovered.
    pub is_hovered: bool,
    /// When the hover started (for timing-based selection).
    pub hover_start: Option<Instant>,
    /// Hover duration required for auto-selection.
    pub hover_duration: Duration,
}

impl TabHoverState {
    pub fn new() -> Self {
        Self {
            is_hovered: false,
            hover_start: None,
            hover_duration: Duration::from_millis(500), // 500ms hover to select
        }
    }

    /// Start hovering over a tab.
    pub fn start_hover(&mut self) {
        if !self.is_hovered {
            self.is_hovered = true;
            self.hover_start = Some(Instant::now());
        }
    }

    /// Stop hovering over the tab.
    pub fn end_hover(&mut self) {
        self.is_hovered = false;
        self.hover_start = None;
    }

    /// Check if hover has exceeded the auto-select duration.
    pub fn should_auto_select(&self) -> bool {
        if let Some(start) = self.hover_start {
            start.elapsed() >= self.hover_duration
        } else {
            false
        }
    }

    /// Get elapsed hover time.
    pub fn hover_elapsed(&self) -> Option<Duration> {
        self.hover_start.map(|start| start.elapsed())
    }
}

impl Default for TabHoverState {
    fn default() -> Self {
        Self::new()
    }
}

/// Scroll behavior for tab switching.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabScrollBehavior {
    /// No scroll switching.
    Disabled,
    /// Switch on horizontal scroll.
    HorizontalOnly,
    /// Switch on any scroll.
    AllDirections,
}

impl Default for TabScrollBehavior {
    fn default() -> Self {
        Self::HorizontalOnly
    }
}

/// Tab interaction configuration.
#[derive(Debug, Clone, PartialEq)]
pub struct TabInteractionConfig {
    /// Whether drag and drop between windows is enabled.
    pub cross_window_drag: bool,
    /// Whether hover-to-select is enabled.
    pub hover_to_select: bool,
    /// Hover duration for auto-selection in milliseconds.
    pub hover_select_duration_ms: u64,
    /// Whether scroll-to-switch is enabled.
    pub scroll_to_switch: TabScrollBehavior,
    /// Minimum drag distance before drag starts (in pixels).
    pub drag_threshold: f64,
    /// Whether to show preview while dragging.
    pub show_drag_preview: bool,
    /// Whether to switch tabs on middle-click close.
    pub middle_click_switch: bool,
}

impl Eq for TabInteractionConfig {}

impl Default for TabInteractionConfig {
    fn default() -> Self {
        Self {
            cross_window_drag: true,
            hover_to_select: true,
            hover_select_duration_ms: 500,
            scroll_to_switch: TabScrollBehavior::HorizontalOnly,
            drag_threshold: 5.0,
            show_drag_preview: true,
            middle_click_switch: true,
        }
    }
}

/// Tab drag service for managing drag operations.
#[derive(Debug, Clone)]
pub struct TabDragService {
    /// Current drag state.
    state: TabDragState,
    /// Current mouse position during drag.
    mouse_position: Option<(f64, f64)>,
    /// Start position where drag began.
    drag_start_position: Option<(f64, f64)>,
    /// History of recently closed tabs (for undo).
    recent_closed_tabs: VecDeque<u64>,
    /// Maximum number of closed tabs to remember.
    max_closed_tabs_history: usize,
}

impl Default for TabDragService {
    fn default() -> Self {
        Self::new()
    }
}

impl TabDragService {
    pub fn new() -> Self {
        Self {
            state: TabDragState::Idle,
            mouse_position: None,
            drag_start_position: None,
            recent_closed_tabs: VecDeque::new(),
            max_closed_tabs_history: 10,
        }
    }

    /// Start dragging a tab.
    pub fn start_drag(&mut self, tab_id: u64) {
        self.state = TabDragState::Dragging { source_tab_id: tab_id };
    }

    /// Update mouse position during drag.
    pub fn update_position(&mut self, x: f64, y: f64) {
        self.mouse_position = Some((x, y));
    }

    /// Set drag start position.
    pub fn set_drag_start(&mut self, x: f64, y: f64) {
        self.drag_start_position = Some((x, y));
    }

    /// Check if drag distance exceeds threshold.
    pub fn exceeds_threshold(&self, current_x: f64, current_y: f64) -> bool {
        if let Some((start_x, start_y)) = self.drag_start_position {
            let dx = current_x - start_x;
            let dy = current_y - start_y;
            (dx * dx + dy * dy).sqrt() >= 5.0 // drag_threshold default 5.0
        } else {
            false
        }
    }

    /// Enter a drop target (window/tab).
    pub fn enter_target(&mut self, target_window_id: u64, position: InsertPosition) {
        self.state = TabDragState::OverTarget {
            target_window_id,
            insert_position: position,
        };
    }

    /// Complete the drag operation.
    pub fn complete(&mut self) {
        self.state = TabDragState::Dropped;
        self.mouse_position = None;
        self.drag_start_position = None;
    }

    /// Cancel the drag operation.
    pub fn cancel(&mut self) {
        self.state = TabDragState::Cancelled;
        self.mouse_position = None;
        self.drag_start_position = None;
    }

    /// Reset to idle state.
    pub fn reset(&mut self) {
        self.state = TabDragState::Idle;
        self.mouse_position = None;
        self.drag_start_position = None;
    }

    /// Get current drag state.
    pub fn get_state(&self) -> &TabDragState {
        &self.state
    }

    /// Add a closed tab to history (for undo close).
    pub fn add_to_history(&mut self, tab_id: u64) {
        if self.recent_closed_tabs.len() >= self.max_closed_tabs_history {
            self.recent_closed_tabs.pop_back();
        }
        self.recent_closed_tabs.push_front(tab_id);
    }

    /// Pop the most recently closed tab (undo close).
    pub fn pop_recent(&mut self) -> Option<u64> {
        self.recent_closed_tabs.pop_front()
    }

    /// Check if undo is available.
    pub fn can_undo(&self) -> bool {
        !self.recent_closed_tabs.is_empty()
    }

    /// Get undo history size.
    pub fn history_size(&self) -> usize {
        self.recent_closed_tabs.len()
    }
}

/// Tab scroll state for wheel-based switching.
#[derive(Debug, Clone, Default)]
pub struct TabScrollState {
    /// Last scroll time for throttling.
    last_scroll_time: Option<Instant>,
    /// Minimum time between tab switches (ms).
    scroll_throttle_ms: u64,
}

impl TabScrollState {
    pub fn new() -> Self {
        Self {
            last_scroll_time: None,
            scroll_throttle_ms: 150, // 150ms between switches
        }
    }

    /// Check if scroll should trigger tab switch.
    pub fn should_switch(&self) -> bool {
        if let Some(last) = self.last_scroll_time {
            last.elapsed().as_millis() as u64 >= self.scroll_throttle_ms
        } else {
            true
        }
    }

    /// Record a scroll event.
    pub fn record_scroll(&mut self) {
        self.last_scroll_time = Some(Instant::now());
    }

    /// Reset the scroll state.
    pub fn reset(&mut self) {
        self.last_scroll_time = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tab_hover_state_new() {
        let state = TabHoverState::new();
        assert!(!state.is_hovered);
        assert!(state.hover_start.is_none());
    }

    #[test]
    fn test_tab_hover_state_start_hover() {
        let mut state = TabHoverState::new();
        state.start_hover();
        assert!(state.is_hovered);
        assert!(state.hover_start.is_some());
    }

    #[test]
    fn test_tab_hover_state_end_hover() {
        let mut state = TabHoverState::new();
        state.start_hover();
        state.end_hover();
        assert!(!state.is_hovered);
        assert!(state.hover_start.is_none());
    }

    #[test]
    fn test_tab_hover_state_should_auto_select() {
        let mut state = TabHoverState::new();
        state.hover_duration = Duration::from_millis(10);
        assert!(!state.should_auto_select());
        state.start_hover();
        std::thread::sleep(Duration::from_millis(15));
        assert!(state.should_auto_select());
    }

    #[test]
    fn test_tab_hover_state_hover_elapsed() {
        let mut state = TabHoverState::new();
        assert!(state.hover_elapsed().is_none());
        state.start_hover();
        std::thread::sleep(Duration::from_millis(5));
        assert!(state.hover_elapsed().is_some());
    }

    #[test]
    fn test_tab_interaction_config_default() {
        let config = TabInteractionConfig::default();
        assert!(config.cross_window_drag);
        assert!(config.hover_to_select);
        assert_eq!(config.hover_select_duration_ms, 500);
        assert_eq!(config.scroll_to_switch, TabScrollBehavior::HorizontalOnly);
    }

    #[test]
    fn test_tab_scroll_behavior_default() {
        assert_eq!(TabScrollBehavior::default(), TabScrollBehavior::HorizontalOnly);
    }

    #[test]
    fn test_tab_scroll_behavior_variants() {
        let behaviors = vec![
            TabScrollBehavior::Disabled,
            TabScrollBehavior::HorizontalOnly,
            TabScrollBehavior::AllDirections,
        ];
        assert_eq!(behaviors.len(), 3);
    }

    #[test]
    fn test_tab_drag_service_new() {
        let service = TabDragService::new();
        assert!(matches!(service.get_state(), &TabDragState::Idle));
    }

    #[test]
    fn test_tab_drag_service_start_drag() {
        let mut service = TabDragService::new();
        service.start_drag(123);
        assert!(matches!(
            service.get_state(),
            &TabDragState::Dragging { source_tab_id: 123 }
        ));
    }

    #[test]
    fn test_tab_drag_service_update_position() {
        let mut service = TabDragService::new();
        service.update_position(100.0, 200.0);
        assert!(service.mouse_position.is_some());
    }

    #[test]
    fn test_tab_drag_service_exceeds_threshold() {
        let mut service = TabDragService::new();
        service.set_drag_start(0.0, 0.0);
        assert!(!service.exceeds_threshold(2.0, 2.0));
        assert!(service.exceeds_threshold(10.0, 0.0));
    }

    #[test]
    fn test_tab_drag_service_enter_target() {
        let mut service = TabDragService::new();
        service.start_drag(1);
        service.enter_target(2, InsertPosition::Before);
        assert!(matches!(
            service.get_state(),
            &TabDragState::OverTarget {
                target_window_id: 2,
                insert_position: InsertPosition::Before
            }
        ));
    }

    #[test]
    fn test_tab_drag_service_complete() {
        let mut service = TabDragService::new();
        service.start_drag(1);
        service.complete();
        assert!(matches!(service.get_state(), &TabDragState::Dropped));
    }

    #[test]
    fn test_tab_drag_service_cancel() {
        let mut service = TabDragService::new();
        service.start_drag(1);
        service.cancel();
        assert!(matches!(service.get_state(), &TabDragState::Cancelled));
    }

    #[test]
    fn test_tab_drag_service_reset() {
        let mut service = TabDragService::new();
        service.start_drag(1);
        service.reset();
        assert!(matches!(service.get_state(), &TabDragState::Idle));
    }

    #[test]
    fn test_tab_drag_service_add_to_history() {
        let mut service = TabDragService::new();
        service.add_to_history(1);
        service.add_to_history(2);
        assert_eq!(service.history_size(), 2);
        assert!(service.can_undo());
    }

    #[test]
    fn test_tab_drag_service_pop_recent() {
        let mut service = TabDragService::new();
        service.add_to_history(1);
        service.add_to_history(2);
        assert_eq!(service.pop_recent(), Some(2));
        assert_eq!(service.pop_recent(), Some(1));
        assert_eq!(service.pop_recent(), None);
    }

    #[test]
    fn test_tab_drag_service_history_limit() {
        let mut service = TabDragService::new();
        service.max_closed_tabs_history = 3;
        service.add_to_history(1);
        service.add_to_history(2);
        service.add_to_history(3);
        service.add_to_history(4); // Should evict 1
        assert_eq!(service.history_size(), 3);
        assert_eq!(service.pop_recent(), Some(4));
    }

    #[test]
    fn test_tab_scroll_state_new() {
        let state = TabScrollState::new();
        assert!(state.should_switch());
    }

    #[test]
    fn test_tab_scroll_state_should_switch_throttled() {
        let mut state = TabScrollState::new();
        state.scroll_throttle_ms = 1000;
        assert!(state.should_switch());
        state.record_scroll();
        assert!(!state.should_switch());
    }

    #[test]
    fn test_tab_scroll_state_reset() {
        let mut state = TabScrollState::new();
        state.record_scroll();
        state.reset();
        assert!(state.should_switch());
    }

    #[test]
    fn test_insert_position_variants() {
        assert_ne!(InsertPosition::Before, InsertPosition::After);
        assert_eq!(InsertPosition::Before, InsertPosition::Before);
    }

    #[test]
    fn test_tab_drag_state_variants() {
        assert!(matches!(TabDragState::Idle, TabDragState::Idle));
        assert!(matches!(
            TabDragState::Dragging { source_tab_id: 1 },
            TabDragState::Dragging { source_tab_id: 1 }
        ));
        assert!(matches!(
            TabDragState::OverTarget {
                target_window_id: 1,
                insert_position: InsertPosition::Before
            },
            TabDragState::OverTarget { .. }
        ));
    }
}
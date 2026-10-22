//! Tab Advanced Interactions — Cross-window dragging, multi-window tab docking registry,
//! hover-to-select, scroll-to-switch, and smooth tab animations.
//!
//! Features:
//! - `WindowTabRegistry`: Process-wide thread-safe registry of open `AppWindow` instances,
//!   their live screen bounds, tab counts, and `WeakEntity<AppWindow>` handles for cross-window
//!   tab dragging and docking ("bringing a tab from one window to another tab bar").
//! - Win32 global cursor & mouse button polling (`query_os_cursor_and_lbutton`) so dragging a tab
//!   outside a window tracks smoothly across the desktop and across multiple windows at display refresh rate.
//! - `TabHoverState`, `TabScrollState`, `TabDragService`, and `TabInteractionConfig`.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

static NEXT_WINDOW_ID: AtomicU64 = AtomicU64::new(1);

/// Allocate a unique window ID for a newly created `AppWindow`.
pub fn next_window_id() -> u64 {
    NEXT_WINDOW_ID.fetch_add(1, Ordering::Relaxed)
}

/// Metadata for a registered open `AppWindow` in the process-wide `WindowTabRegistry`.
#[derive(Clone)]
pub struct RegisteredWindowInfo {
    pub window_id: u64,
    pub weak_view: gpui::WeakEntity<crate::app::AppWindow>,
    pub hwnd: isize,
    /// Window origin X in screen coordinates (logical pixels)
    pub screen_x: f32,
    /// Window origin Y in screen coordinates (logical pixels)
    pub screen_y: f32,
    /// Window width in logical pixels
    pub width: f32,
    /// Window height in logical pixels
    pub height: f32,
    pub scale_factor: f32,
    pub tab_count: usize,
    pub pinned_count: usize,
    pub scroll_offset: f32,
    pub active_tab_title: String,
    pub last_updated: Instant,
}

/// Result of hit-testing a dragged tab against other open windows' tab bars.
#[derive(Clone)]
pub struct CrossWindowTabTarget {
    pub target_window_id: u64,
    pub target_weak: gpui::WeakEntity<crate::app::AppWindow>,
    pub target_hwnd: isize,
    pub insert_index: usize,
    pub local_x: f32,
    pub target_window_title: String,
}

static WINDOW_REGISTRY: OnceLock<Mutex<Vec<RegisteredWindowInfo>>> = OnceLock::new();

fn registry_slot() -> &'static Mutex<Vec<RegisteredWindowInfo>> {
    WINDOW_REGISTRY.get_or_init(|| Mutex::new(Vec::new()))
}

/// Process-wide registry of open `AppWindow` instances for cross-window tab dragging and docking.
pub struct WindowTabRegistry;

impl WindowTabRegistry {
    /// Register or update a window's live screen geometry and tab strip state.
    pub fn upsert(info: RegisteredWindowInfo) {
        if let Ok(mut list) = registry_slot().lock() {
            // Prune any dead WeakEntity entries
            list.retain(|entry| entry.weak_view.upgrade().is_some() || entry.window_id == info.window_id);
            if let Some(existing) = list.iter_mut().find(|e| e.window_id == info.window_id) {
                *existing = info;
            } else {
                list.push(info);
            }
        }
    }

    /// Unregister a window when it closes.
    pub fn unregister(window_id: u64) {
        if let Ok(mut list) = registry_slot().lock() {
            list.retain(|e| e.window_id != window_id);
        }
    }

    /// Return the number of currently open `AppWindow` instances.
    pub fn open_window_count() -> usize {
        if let Ok(mut list) = registry_slot().lock() {
            list.retain(|e| e.weak_view.upgrade().is_some());
            list.len()
        } else {
            1
        }
    }

    /// Return all other open windows (excluding `exclude_window_id`).
    pub fn other_windows(exclude_window_id: u64) -> Vec<RegisteredWindowInfo> {
        if let Ok(mut list) = registry_slot().lock() {
            list.retain(|e| e.weak_view.upgrade().is_some());
            list.iter()
                .filter(|e| e.window_id != exclude_window_id)
                .cloned()
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Hit-test global screen coordinates `(screen_x, screen_y)` against all other open windows' tab bars.
    /// Returns the target window and the exact tab insertion index if the cursor is over another window's tab bar.
    pub fn hit_test_other_window_tab_bar(
        source_window_id: u64,
        screen_x: f32,
        screen_y: f32,
    ) -> Option<CrossWindowTabTarget> {
        let list = registry_slot().lock().ok()?;
        for win in list.iter() {
            if win.window_id == source_window_id {
                continue;
            }
            // Check if screen_x, screen_y is within the target window's tab bar drop zone
            // (allow a generous vertical band [-12px .. +64px] from the top of the window)
            if screen_x >= win.screen_x
                && screen_x <= win.screen_x + win.width
                && screen_y >= win.screen_y - 12.0
                && screen_y <= win.screen_y + 64.0
            {
                let local_x = (screen_x - win.screen_x).max(0.0);
                let metrics = crate::ui::components::tab_bar::compute_strip_metrics(
                    win.width,
                    win.tab_count.max(1),
                    win.pinned_count,
                );
                let rel_x = (local_x - metrics.strip_start_x + win.scroll_offset).max(0.0);
                let pinned_total_w =
                    win.pinned_count as f32 * crate::ui::components::tab_bar::PINNED_TAB_W;
                let insert_index = if rel_x <= pinned_total_w && win.pinned_count > 0 {
                    ((rel_x / crate::ui::components::tab_bar::PINNED_TAB_W).round() as usize)
                        .min(win.pinned_count)
                } else {
                    let unpinned_x = (rel_x - pinned_total_w).max(0.0);
                    let idx = win.pinned_count
                        + (unpinned_x / metrics.tab_w.max(1.0)).round() as usize;
                    idx.min(win.tab_count)
                };

                return Some(CrossWindowTabTarget {
                    target_window_id: win.window_id,
                    target_weak: win.weak_view.clone(),
                    target_hwnd: win.hwnd,
                    insert_index,
                    local_x,
                    target_window_title: win.active_tab_title.clone(),
                });
            }
        }
        None
    }
}

/// Query the OS cursor position (in both logical screen coordinates and window-local coordinates)
/// and whether the primary mouse button (`VK_LBUTTON`) is currently pressed.
///
/// This enables smooth 120–240Hz tab dragging even when the cursor leaves the source window's
/// client area and moves across the desktop or over another window's tab bar.
#[cfg(windows)]
pub fn query_os_cursor_and_lbutton(
    hwnd_raw: isize,
    scale_factor: f32,
) -> Option<((f32, f32), (f32, f32), bool)> {
    use windows::Win32::Foundation::{HWND, POINT};
    use windows::Win32::Graphics::Gdi::ScreenToClient;
    use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON};
    use windows::Win32::UI::WindowsAndMessaging::{GetCursorPos, GetSystemMetrics, SM_SWAPBUTTON};

    if hwnd_raw == 0 {
        return None;
    }
    let scale = scale_factor.max(0.5);
    unsafe {
        let mut screen_pt = POINT::default();
        if GetCursorPos(&mut screen_pt).is_err() {
            return None;
        }
        let mut client_pt = screen_pt;
        let hwnd = HWND(hwnd_raw as *mut core::ffi::c_void);
        let _ = ScreenToClient(hwnd, &mut client_pt);

        let swapped = GetSystemMetrics(SM_SWAPBUTTON) != 0;
        let vk = if swapped { 0x02i32 } else { VK_LBUTTON.0 as i32 };
        let lbutton_down = (GetAsyncKeyState(vk) as u16 & 0x8000) != 0;

        let screen_logical = (screen_pt.x as f32 / scale, screen_pt.y as f32 / scale);
        let local_logical = (client_pt.x as f32 / scale, client_pt.y as f32 / scale);
        Some((local_logical, screen_logical, lbutton_down))
    }
}

#[cfg(not(windows))]
pub fn query_os_cursor_and_lbutton(
    _hwnd_raw: isize,
    _scale_factor: f32,
) -> Option<((f32, f32), (f32, f32), bool)> {
    None
}

/// Bring a target Win32 window to the foreground when a tab is docked into it.
#[cfg(windows)]
pub fn activate_hwnd(hwnd_raw: isize) {
    if hwnd_raw == 0 {
        return;
    }
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::SetForegroundWindow;
    unsafe {
        let _ = SetForegroundWindow(HWND(hwnd_raw as *mut core::ffi::c_void));
    }
}

#[cfg(not(windows))]
pub fn activate_hwnd(_hwnd_raw: isize) {}

/// Move a Win32 window to the specified logical screen coordinates `(screen_x, screen_y)`.
#[cfg(windows)]
pub fn move_hwnd_to_screen_pos(hwnd_raw: isize, screen_x: f32, screen_y: f32, scale_factor: f32) {
    if hwnd_raw == 0 {
        return;
    }
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        SetWindowPos, SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER,
    };
    let scale = scale_factor.max(0.5);
    let px_x = (screen_x * scale).round() as i32;
    let px_y = (screen_y * scale).round() as i32;
    unsafe {
        let _ = SetWindowPos(
            HWND(hwnd_raw as *mut core::ffi::c_void),
            None,
            px_x,
            px_y,
            0,
            0,
            SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
        );
    }
}

#[cfg(not(windows))]
pub fn move_hwnd_to_screen_pos(_hwnd_raw: isize, _screen_x: f32, _screen_y: f32, _scale_factor: f32) {}

/// Tab drag state for cross-window operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TabDragState {
    /// Not dragging.
    Idle,
    /// Dragging started, contains source tab id.
    Dragging { source_tab_id: u64 },
    /// Dragging over a target window.
    OverTarget {
        target_window_id: u64,
        insert_position: InsertPosition,
    },
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
        self.state = TabDragState::Dragging {
            source_tab_id: tab_id,
        };
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

    /// Add a closed tab to history (for undo).
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
            scroll_throttle_ms: 140, // 140ms between switches
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
        assert_eq!(
            TabScrollBehavior::default(),
            TabScrollBehavior::HorizontalOnly
        );
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

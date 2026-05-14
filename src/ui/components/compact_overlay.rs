//! Compact Overlay — Picture-in-Picture (always-on-top) mini-window mode.
//! Per DESIGN_SPEC.md section 6 / TODO.md Section 6.
//!
//! This module provides the CompactOverlay state and service for:
//! - Managing compact/overlay mode state
//! - Tracking window configuration for PiP mode
//! - Providing toggle and transition logic

/// Window mode for the application.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WindowMode {
    /// Normal windowed mode (default)
    #[default]
    Normal,
    /// Compact overlay mode (small, always-on-top)
    CompactOverlay,
    /// Fullscreen mode
    Fullscreen,
    /// Maximized window
    Maximized,
}

/// Compact overlay appearance configuration.
#[derive(Debug, Clone)]
pub struct CompactOverlayConfig {
    /// Width of the compact overlay window in pixels.
    pub width: f64,
    /// Height of the compact overlay window in pixels.
    pub height: f64,
    /// Whether the window should stay on top of other windows.
    pub always_on_top: bool,
    /// Opacity level (0.0 to 1.0).
    pub opacity: f64,
}

impl PartialEq for CompactOverlayConfig {
    fn eq(&self, other: &Self) -> bool {
        self.width.to_bits() == other.width.to_bits()
            && self.height.to_bits() == other.height.to_bits()
            && self.always_on_top == other.always_on_top
            && self.opacity.to_bits() == other.opacity.to_bits()
    }
}

impl Eq for CompactOverlayConfig {}

impl Default for CompactOverlayConfig {
    fn default() -> Self {
        Self {
            width: 320.0,
            height: 240.0,
            always_on_top: true,
            opacity: 0.95,
        }
    }
}

/// State for managing compact overlay mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompactOverlayState {
    /// Whether compact overlay mode is currently active.
    pub is_active: bool,
    /// The previous window mode before entering compact overlay.
    pub previous_mode: Option<WindowMode>,
    /// Whether the user manually dismissed the overlay.
    pub user_dismissed: bool,
}

impl Default for CompactOverlayState {
    fn default() -> Self {
        Self {
            is_active: false,
            previous_mode: None,
            user_dismissed: false,
        }
    }
}

impl CompactOverlayState {
    /// Creates a new compact overlay state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Enters compact overlay mode, saving the current mode.
    pub fn enter(&mut self, current_mode: WindowMode) {
        if !self.is_active {
            self.previous_mode = Some(current_mode);
            self.is_active = true;
            self.user_dismissed = false;
        }
    }

    /// Exits compact overlay mode, restoring the previous mode.
    pub fn exit(&mut self) -> Option<WindowMode> {
        if self.is_active {
            self.is_active = false;
            let prev = self.previous_mode.take();
            self.previous_mode = None;
            prev
        } else {
            None
        }
    }

    /// Toggles compact overlay mode on/off.
    pub fn toggle(&mut self, current_mode: WindowMode) -> bool {
        if self.is_active {
            self.exit();
            false
        } else {
            self.enter(current_mode);
            true
        }
    }

    /// Marks that the user manually dismissed the overlay.
    pub fn mark_dismissed(&mut self) {
        self.user_dismissed = true;
    }

    /// Checks if the overlay should be shown (active and not dismissed).
    pub fn should_show(&self) -> bool {
        self.is_active && !self.user_dismissed
    }

    /// Gets the previous window mode if available.
    pub fn get_previous_mode(&self) -> Option<WindowMode> {
        self.previous_mode
    }

    /// Checks if compact overlay is currently active.
    pub fn is_compact_overlay_active(&self) -> bool {
        self.is_active
    }
}

/// Service for managing compact overlay configuration and behavior.
#[derive(Debug, Clone, Default)]
pub struct CompactOverlayService;

impl CompactOverlayService {
    pub fn new() -> Self {
        Self
    }

    /// Gets the default configuration for compact overlay.
    pub fn get_default_config(&self) -> CompactOverlayConfig {
        CompactOverlayConfig::default()
    }

    /// Gets a configuration preset based on position.
    pub fn get_config_for_position(&self, position: CompactOverlayPosition) -> CompactOverlayConfig {
        match position {
            CompactOverlayPosition::TopLeft
            | CompactOverlayPosition::TopRight
            | CompactOverlayPosition::BottomLeft
            | CompactOverlayPosition::BottomRight => CompactOverlayConfig {
                width: 320.0,
                height: 240.0,
                always_on_top: true,
                opacity: 0.95,
            },
        }
    }

    /// Validates that the configuration is within acceptable bounds.
    pub fn validate_config(&self, config: &CompactOverlayConfig) -> bool {
        config.width >= 200.0
            && config.width <= 800.0
            && config.height >= 150.0
            && config.height <= 600.0
            && config.opacity >= 0.5
            && config.opacity <= 1.0
    }

    /// Calculates appropriate size based on screen dimensions.
    pub fn calculate_auto_size(&self, screen_width: f64, screen_height: f64) -> (f64, f64) {
        let width = (screen_width * 0.25).min(400.0).max(200.0);
        let height = (screen_height * 0.2).min(300.0).max(150.0);
        (width, height)
    }

    /// Determines if the device supports compact overlay mode.
    pub fn is_supported(&self) -> bool {
        // Most modern Windows versions support always-on-top windows
        // This could be enhanced with actual capability detection
        true
    }
}

/// Position for compact overlay on screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompactOverlayPosition {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl Default for CompactOverlayPosition {
    fn default() -> Self {
        Self::BottomRight
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compact_overlay_state_new() {
        let state = CompactOverlayState::new();
        assert!(!state.is_active);
        assert!(!state.user_dismissed);
    }

    #[test]
    fn test_compact_overlay_state_default() {
        let state = CompactOverlayState::default();
        assert!(!state.is_active);
    }

    #[test]
    fn test_compact_overlay_state_enter() {
        let mut state = CompactOverlayState::new();
        state.enter(WindowMode::Normal);
        assert!(state.is_active);
        assert_eq!(state.previous_mode, Some(WindowMode::Normal));
    }

    #[test]
    fn test_compact_overlay_state_exit() {
        let mut state = CompactOverlayState::new();
        state.enter(WindowMode::Normal);
        let prev = state.exit();
        assert!(!state.is_active);
        assert_eq!(prev, Some(WindowMode::Normal));
    }

    #[test]
    fn test_compact_overlay_state_toggle_on() {
        let mut state = CompactOverlayState::new();
        let result = state.toggle(WindowMode::Maximized);
        assert!(result);
        assert!(state.is_active);
        assert_eq!(state.previous_mode, Some(WindowMode::Maximized));
    }

    #[test]
    fn test_compact_overlay_state_toggle_off() {
        let mut state = CompactOverlayState::new();
        state.enter(WindowMode::Normal);
        let result = state.toggle(WindowMode::Normal);
        assert!(!result);
        assert!(!state.is_active);
    }

    #[test]
    fn test_compact_overlay_state_mark_dismissed() {
        let mut state = CompactOverlayState::new();
        state.enter(WindowMode::Normal);
        state.mark_dismissed();
        assert!(state.user_dismissed);
    }

    #[test]
    fn test_compact_overlay_state_should_show() {
        let mut state = CompactOverlayState::new();
        assert!(!state.should_show());

        state.enter(WindowMode::Normal);
        assert!(state.should_show());

        state.mark_dismissed();
        assert!(!state.should_show());
    }

    #[test]
    fn test_compact_overlay_state_get_previous_mode() {
        let mut state = CompactOverlayState::new();
        assert!(state.get_previous_mode().is_none());

        state.enter(WindowMode::Fullscreen);
        assert_eq!(state.get_previous_mode(), Some(WindowMode::Fullscreen));
    }

    #[test]
    fn test_compact_overlay_state_is_compact_overlay_active() {
        let mut state = CompactOverlayState::new();
        assert!(!state.is_compact_overlay_active());

        state.enter(WindowMode::Normal);
        assert!(state.is_compact_overlay_active());

        state.exit();
        assert!(!state.is_compact_overlay_active());
    }

    #[test]
    fn test_compact_overlay_config_default() {
        let config = CompactOverlayConfig::default();
        assert_eq!(config.width, 320.0);
        assert_eq!(config.height, 240.0);
        assert!(config.always_on_top);
        assert_eq!(config.opacity, 0.95);
    }

    #[test]
    fn test_window_mode_variants() {
        let modes = vec![
            WindowMode::Normal,
            WindowMode::CompactOverlay,
            WindowMode::Fullscreen,
            WindowMode::Maximized,
        ];
        assert_eq!(modes.len(), 4);
    }

    #[test]
    fn test_window_mode_default() {
        let mode = WindowMode::default();
        assert_eq!(mode, WindowMode::Normal);
    }

    #[test]
    fn test_compact_overlay_position_variants() {
        let positions = vec![
            CompactOverlayPosition::TopLeft,
            CompactOverlayPosition::TopRight,
            CompactOverlayPosition::BottomLeft,
            CompactOverlayPosition::BottomRight,
        ];
        assert_eq!(positions.len(), 4);
    }

    #[test]
    fn test_compact_overlay_position_default() {
        let pos = CompactOverlayPosition::default();
        assert_eq!(pos, CompactOverlayPosition::BottomRight);
    }

    #[test]
    fn test_compact_overlay_service_new() {
        let service = CompactOverlayService::new();
        drop(service);
    }

    #[test]
    fn test_compact_overlay_service_default_config() {
        let service = CompactOverlayService::new();
        let config = service.get_default_config();
        assert_eq!(config.width, 320.0);
    }

    #[test]
    fn test_compact_overlay_service_validate_config_valid() {
        let service = CompactOverlayService::new();
        let config = CompactOverlayConfig {
            width: 320.0,
            height: 240.0,
            always_on_top: true,
            opacity: 0.95,
        };
        assert!(service.validate_config(&config));
    }

    #[test]
    fn test_compact_overlay_service_validate_config_invalid_width() {
        let service = CompactOverlayService::new();
        let config = CompactOverlayConfig {
            width: 100.0, // Too small
            height: 240.0,
            always_on_top: true,
            opacity: 0.95,
        };
        assert!(!service.validate_config(&config));
    }

    #[test]
    fn test_compact_overlay_service_validate_config_invalid_opacity() {
        let service = CompactOverlayService::new();
        let config = CompactOverlayConfig {
            width: 320.0,
            height: 240.0,
            always_on_top: true,
            opacity: 1.5, // Invalid
        };
        assert!(!service.validate_config(&config));
    }

    #[test]
    fn test_compact_overlay_service_calculate_auto_size() {
        let service = CompactOverlayService::new();
        let (width, height) = service.calculate_auto_size(1920.0, 1080.0);
        assert!(width >= 200.0 && width <= 400.0);
        assert!(height >= 150.0 && height <= 300.0);
    }

    #[test]
    fn test_compact_overlay_service_is_supported() {
        let service = CompactOverlayService::new();
        assert!(service.is_supported());
    }

    #[test]
    fn test_compact_overlay_service_get_config_for_position() {
        let service = CompactOverlayService::new();
        let config = service.get_config_for_position(CompactOverlayPosition::TopLeft);
        assert_eq!(config.width, 320.0);
        assert_eq!(config.height, 240.0);
    }
}
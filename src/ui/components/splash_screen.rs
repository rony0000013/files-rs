//! Splash Screen component — shown on cold start.
//! Displays "Files-Rust" with version info and auto-dismisses after a configurable duration.

use crate::ui::constants::UiConstants;
use gpui::*;
use std::time::{Duration, Instant};

/// State for the splash screen overlay.
pub struct SplashState {
    pub visible: bool,
    pub start_time: Instant,
    pub duration: Duration,
}

impl SplashState {
    pub fn new() -> Self {
        Self {
            visible: false,
            start_time: Instant::now(),
            duration: Duration::ZERO,
        }
    }

    pub fn elapsed(&self) -> Duration {
        self.start_time.elapsed()
    }

    pub fn should_dismiss(&self) -> bool {
        self.visible && self.elapsed() >= self.duration
    }

    pub fn dismiss(&mut self) {
        self.visible = false;
    }
}

impl Default for SplashState {
    fn default() -> Self {
        Self::new()
    }
}

pub struct SplashScreen;

impl SplashScreen {
    /// Build the splash screen overlay.
    /// Shows a centered dark panel with the app name, version, loading bar, and keyboard hint.
    pub fn build(_state: &SplashState, _view: Entity<crate::app::AppWindow>) -> impl IntoElement {
        div()
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(rgba(0xCC000000))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap(px(12.0))
                    .w(px(320.0))
                    .h(px(360.0))
                    .bg(rgb(0x1E1E1E))
                    .border(px(1.0))
                    .border_color(rgba(UiConstants::DIVIDER_RGBA))
                    .rounded(px(12.0))
                    .shadow_lg()
                    .child(
                        // App name: "Files-Rust"
                        div()
                            .text_size(px(36.0))
                            .font_weight(FontWeight(700.0))
                            .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                            .child("Files-Rust"),
                    )
                    .child(
                        // Version
                        div()
                            .text_size(px(14.0))
                            .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                            .child("v0.1.0"),
                    )
                    .child(
                        // Loading bar (static indeterminate style)
                        div()
                            .mt(px(8.0))
                            .w(px(180.0))
                            .h(px(4.0))
                            .bg(rgba(0x33FFFFFF))
                            .rounded(px(2.0))
                            .overflow_hidden()
                            .child(
                                div()
                                    .h_full()
                                    .w(px(48.0))
                                    .ml(px(20.0))
                                    .bg(rgb(UiConstants::ACCENT_COLOR_RGB))
                                    .rounded(px(2.0)),
                            ),
                    )
                    .child(
                        // Keyboard shortcut hint
                        div()
                            .mt(px(16.0))
                            .text_size(px(12.0))
                            .text_color(rgba(UiConstants::TEXT_MUTED_RGBA))
                            .child("Press Esc to close"),
                    ),
            )
    }
}

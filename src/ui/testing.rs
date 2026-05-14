//! Visual Test Infrastructure for UI Component Testing
//!
//! This module provides scaffolding for headless GPUI UI tests, enabling
//! visual/regression testing of UI components like BreadcrumbBar, Omnibar, etc.
//!
//! ## Usage
//! ```rust,ignore
//! use ui_testing::VisualTestApp;
//!
//! let app = VisualTestApp::new();
//! app.open_window(|window, cx| {
//!     let view = MyComponent::build(cx);
//!     window.set_content(view, cx);
//! });
//! app.screenshot("test_output.png")?;
//! ```

use std::path::PathBuf;

/// Visual test configuration
#[derive(Debug, Clone)]
pub struct VisualTestConfig {
    /// Window dimensions
    pub window_size: (u32, u32),
    /// Theme for the test
    pub theme: TestTheme,
    /// Whether to capture console output
    pub capture_console: bool,
    /// Screenshot output directory
    pub screenshot_dir: Option<PathBuf>,
}

impl Default for VisualTestConfig {
    fn default() -> Self {
        Self {
            window_size: (1280, 720),
            theme: TestTheme::Dark,
            capture_console: true,
            screenshot_dir: None,
        }
    }
}

/// Test themes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TestTheme {
    #[default]
    Dark,
    Light,
}

impl TestTheme {
    pub fn as_str(&self) -> &'static str {
        match self {
            TestTheme::Light => "light",
            TestTheme::Dark => "dark",
        }
    }
}

/// Test assertion helpers
pub mod assert {
    /// Assert two values are approximately equal for floating point
    pub fn approx_eq(actual: f32, expected: f32, epsilon: f32) -> bool {
        (actual - expected).abs() < epsilon
    }

    /// Assert a position is within bounds
    pub fn in_bounds(x: f32, y: f32, width: f32, height: f32) -> bool {
        x >= 0.0 && x <= width && y >= 0.0 && y <= height
    }

    /// Assert colors are similar (within tolerance)
    pub fn colors_similar(r1: u8, g1: u8, b1: u8, r2: u8, g2: u8, b2: u8, tolerance: u8) -> bool {
        (r1 as i32 - r2 as i32).abs() <= tolerance as i32
            && (g1 as i32 - g2 as i32).abs() <= tolerance as i32
            && (b1 as i32 - b2 as i32).abs() <= tolerance as i32
    }
}

/// Helper for generating test component variants
pub mod component_variants {
    /// Generate all theme variants for a component test
    #[derive(Debug, Clone)]
    pub struct ThemeVariants<T> {
        pub light: T,
        pub dark: T,
    }

    impl<T> ThemeVariants<T> {
        pub fn new(light: T, dark: T) -> Self {
            Self { light, dark }
        }

        pub fn get(&self, theme: super::TestTheme) -> &T {
            match theme {
                super::TestTheme::Light => &self.light,
                super::TestTheme::Dark => &self.dark,
            }
        }
    }

    /// Generate size variants for a component test
    #[derive(Debug, Clone)]
    pub struct SizeVariants<T> {
        pub small: T,
        pub medium: T,
        pub large: T,
    }
}

/// Test data generators for common UI patterns
pub mod test_data {
    use std::path::PathBuf;

    /// Generate test file paths
    pub fn test_paths() -> Vec<PathBuf> {
        vec![
            PathBuf::from("/home/user/Documents"),
            PathBuf::from("/home/user/Downloads"),
            PathBuf::from("/home/user/Pictures"),
            PathBuf::from("/home/user/Desktop"),
            PathBuf::from("/mnt/c/Users"),
        ]
    }

    /// Generate test file names
    pub fn test_filenames() -> Vec<String> {
        vec![
            "Document.pdf".to_string(),
            "image.png".to_string(),
            "notes.md".to_string(),
            "archive.zip".to_string(),
            ".hidden".to_string(),
            "file.rs".to_string(),
        ]
    }

    /// Generate test folder names
    pub fn test_folder_names() -> Vec<String> {
        vec![
            "Documents".to_string(),
            "Downloads".to_string(),
            "Pictures".to_string(),
            "Music".to_string(),
            ".config".to_string(),
        ]
    }

    /// Generate long path for overflow testing
    pub fn long_path() -> PathBuf {
        let segments = vec!["home", "user", "Documents", "Projects", "files-rs", "src", "ui", "components"];
        segments.into_iter().collect()
    }
}

/// Screenshot capture utilities
pub mod screenshot {
    use std::path::PathBuf;

    /// Generate a screenshot filename with timestamp
    pub fn generate_filename(component: &str, theme: &str, variant: Option<&str>) -> String {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);

        match variant {
            Some(v) => format!("{}_{}_{}_{}.png", component, theme, v, timestamp),
            None => format!("{}_{}_{}.png", component, theme, timestamp),
        }
    }

    /// Validate screenshot dimensions
    pub fn validate_dimensions(width: u32, height: u32, min_width: u32, min_height: u32) -> bool {
        width >= min_width && height >= min_height
    }
}

/// Interaction simulation helpers for tests
pub mod interactions {
    /// Mouse positions for common interactions
    #[derive(Debug, Clone)]
    pub struct MousePosition {
        pub x: f32,
        pub y: f32,
    }

    impl MousePosition {
        pub fn at(x: f32, y: f32) -> Self {
            Self { x, y }
        }

        /// Center of a rect
        pub fn center(width: f32, height: f32) -> Self {
            Self {
                x: width / 2.0,
                y: height / 2.0,
            }
        }

        /// Offset from current position
        pub fn offset(&self, dx: f32, dy: f32) -> Self {
            Self {
                x: self.x + dx,
                y: self.y + dy,
            }
        }
    }

    /// Drag paths for drag-and-drop testing
    #[derive(Debug, Clone)]
    pub struct DragPath {
        pub start: MousePosition,
        pub end: MousePosition,
        pub steps: usize,
    }

    impl DragPath {
        pub fn new(start: MousePosition, end: MousePosition, steps: usize) -> Self {
            Self { start, end, steps }
        }

        /// Generate intermediate positions along the drag path
        pub fn positions(&self) -> Vec<MousePosition> {
            let dx = (self.end.x - self.start.x) / self.steps as f32;
            let dy = (self.end.y - self.start.y) / self.steps as f32;

            (0..=self.steps)
                .map(|i| MousePosition {
                    x: self.start.x + dx * i as f32,
                    y: self.start.y + dy * i as f32,
                })
                .collect()
        }
    }

    /// Keyboard shortcuts for testing
    #[derive(Debug, Clone)]
    pub enum KeyboardShortcut {
        Ctrl(char),
        Alt(char),
        Shift(char),
        CtrlShift(char),
        F(u8),
        Tab,
        Enter,
        Escape,
        Backspace,
    }
}

/// Visual regression test builder
#[derive(Debug, Default)]
pub struct VisualRegressionTest {
    pub name: String,
    pub component: String,
    pub theme: TestTheme,
    pub viewport: (u32, u32),
}

impl VisualRegressionTest {
    pub fn new(name: &str, component: &str) -> Self {
        Self {
            name: name.to_string(),
            component: component.to_string(),
            theme: TestTheme::Dark,
            viewport: (1280, 720),
        }
    }

    pub fn with_theme(mut self, theme: TestTheme) -> Self {
        self.theme = theme;
        self
    }

    pub fn with_viewport(mut self, width: u32, height: u32) -> Self {
        self.viewport = (width, height);
        self
    }

    /// Get the baseline image path for this test
    pub fn baseline_path(&self, baseline_dir: &PathBuf) -> PathBuf {
        let filename = format!(
            "{}_{}_{}_{}x{}.png",
            self.component,
            self.name,
            self.theme.as_str(),
            self.viewport.0,
            self.viewport.1
        );
        baseline_dir.join(filename)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_visual_regression_test_baseline_path() {
        let test = VisualRegressionTest::new("hover", "breadcrumb_bar")
            .with_theme(TestTheme::Dark)
            .with_viewport(800, 600);

        let baseline_dir = PathBuf::from("/baseline");
        let path = test.baseline_path(&baseline_dir);

        assert!(path.to_string_lossy().contains("breadcrumb_bar"));
        assert!(path.to_string_lossy().contains("hover"));
        assert!(path.to_string_lossy().contains("dark"));
        assert!(path.to_string_lossy().contains("800x600"));
    }

    #[test]
    fn test_drag_path_positions() {
        let drag = super::interactions::DragPath::new(
            super::interactions::MousePosition::at(0.0, 0.0),
            super::interactions::MousePosition::at(100.0, 50.0),
            4,
        );

        let positions = drag.positions();
        assert_eq!(positions.len(), 5); // start + 4 steps + end
    }

    #[test]
    fn test_colors_similar() {
        assert!(assert::colors_similar(255, 0, 0, 255, 0, 0, 0)); // exact match
        assert!(assert::colors_similar(255, 0, 0, 254, 1, 0, 2)); // within tolerance
        assert!(!assert::colors_similar(255, 0, 0, 0, 0, 0, 5)); // outside tolerance
    }

    #[test]
    fn test_approx_eq() {
        assert!(assert::approx_eq(1.0, 1.0, 0.001));
        assert!(assert::approx_eq(1.0, 1.001, 0.01));
        assert!(!assert::approx_eq(1.0, 1.1, 0.01));
    }

    #[test]
    fn test_in_bounds() {
        assert!(assert::in_bounds(50.0, 50.0, 100.0, 100.0));
        assert!(assert::in_bounds(0.0, 0.0, 100.0, 100.0));
        assert!(assert::in_bounds(100.0, 100.0, 100.0, 100.0));
        assert!(!assert::in_bounds(-1.0, 50.0, 100.0, 100.0));
        assert!(!assert::in_bounds(50.0, 101.0, 100.0, 100.0));
    }

    #[test]
    fn test_test_data_paths() {
        let paths = test_data::test_paths();
        assert!(!paths.is_empty());
        assert_eq!(paths[0], PathBuf::from("/home/user/Documents"));
    }

    #[test]
    fn test_mouse_position_center() {
        let center = super::interactions::MousePosition::center(200.0, 100.0);
        assert_eq!(center.x, 100.0);
        assert_eq!(center.y, 50.0);
    }

    #[test]
    fn test_mouse_position_offset() {
        let pos = super::interactions::MousePosition::at(10.0, 20.0);
        let offset = pos.offset(5.0, -10.0);
        assert_eq!(offset.x, 15.0);
        assert_eq!(offset.y, 10.0);
    }

    #[test]
    fn test_screenshot_filename() {
        let filename = screenshot::generate_filename("breadcrumb", "dark", Some("hover"));
        assert!(filename.contains("breadcrumb"));
        assert!(filename.contains("dark"));
        assert!(filename.contains("hover"));
        assert!(filename.ends_with(".png"));

        let filename_no_variant = screenshot::generate_filename("button", "light", None);
        assert!(filename_no_variant.contains("button"));
        assert!(filename_no_variant.contains("light"));
    }

    #[test]
    fn test_theme_variants() {
        let variants = component_variants::ThemeVariants::new("light_val", "dark_val");
        assert_eq!(variants.get(TestTheme::Light), &"light_val");
        assert_eq!(variants.get(TestTheme::Dark), &"dark_val");
    }
}
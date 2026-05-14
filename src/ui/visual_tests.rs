//! Visual Test Pages for UI Components
//!
//! These tests use the visual test infrastructure to verify UI components
//! render correctly in different themes, sizes, and states.
//!
//! Run with: `cargo test --lib visual_tests`

use crate::ui::testing::*;
use std::path::PathBuf;

// ============================================================================
// BreadcrumbBarPage Visual Tests
// ============================================================================

/// Visual tests for BreadcrumbBar component
pub mod breadcrumb_bar_page {
    use super::*;

    /// Test data for breadcrumb paths
    pub fn test_paths() -> Vec<&'static str> {
        vec![
            "/",
            "/home",
            "/home/user",
            "/home/user/Documents",
            "/mnt/c/Users/Test",
        ]
    }

    /// Generate test cases for breadcrumb rendering
    pub fn render_test_cases() -> Vec<VisualRegressionTest> {
        let mut cases = Vec::new();

        let path_labels = vec!["root", "home", "user", "documents", "windows_path"];

        for (i, path) in test_paths().iter().enumerate() {
            let label = path_labels.get(i).unwrap_or(&"path");

            cases.push(
                VisualRegressionTest::new(label, "breadcrumb_bar")
                    .with_viewport(800, 40)
                    .with_theme(TestTheme::Dark),
            );
            cases.push(
                VisualRegressionTest::new(label, "breadcrumb_bar")
                    .with_viewport(800, 40)
                    .with_theme(TestTheme::Light),
            );
        }

        // Overflow case with long path
        cases.push(
            VisualRegressionTest::new("overflow", "breadcrumb_bar")
                .with_viewport(400, 40)
                .with_theme(TestTheme::Dark),
        );

        // Hover state
        cases.push(
            VisualRegressionTest::new("segment_hover", "breadcrumb_bar")
                .with_viewport(800, 40)
                .with_theme(TestTheme::Dark),
        );

        cases
    }

    #[test]
    fn test_breadcrumb_bar_render_cases() {
        let cases = render_test_cases();
        assert!(!cases.is_empty());
        assert_eq!(cases[0].component, "breadcrumb_bar");
    }

    #[test]
    fn test_breadcrumb_paths_for_visual_testing() {
        let paths = test_paths();
        assert!(paths.len() >= 4);
        assert_eq!(paths[0], "/");
    }
}

// ============================================================================
// OmnibarPage Visual Tests
// ============================================================================

/// Visual tests for Omnibar component
pub mod omnibar_page {
    use super::*;

    /// Generate test cases for omnibar rendering
    pub fn render_test_cases() -> Vec<VisualRegressionTest> {
        let mut cases = Vec::new();

        // Different themes
        for theme in &[TestTheme::Dark, TestTheme::Light] {
            cases.push(
                VisualRegressionTest::new("default", "omnibar")
                    .with_viewport(600, 48)
                    .with_theme(*theme),
            );

            // Edit mode
            cases.push(
                VisualRegressionTest::new("edit_mode", "omnibar")
                    .with_viewport(600, 48)
                    .with_theme(*theme),
            );

            // With suggestions dropdown
            cases.push(
                VisualRegressionTest::new("with_suggestions", "omnibar")
                    .with_viewport(600, 200)
                    .with_theme(*theme),
            );
        }

        // Search mode
        cases.push(
            VisualRegressionTest::new("search_mode", "omnibar")
                .with_viewport(600, 48)
                .with_theme(TestTheme::Dark),
        );

        cases
    }

    #[test]
    fn test_omnibar_render_cases() {
        let cases = render_test_cases();
        assert!(cases.len() >= 7);
    }
}

// ============================================================================
// SidebarViewPage Visual Tests
// ============================================================================

/// Visual tests for Sidebar component
pub mod sidebar_view_page {
    use super::*;

    /// Generate test cases for sidebar rendering
    pub fn render_test_cases() -> Vec<VisualRegressionTest> {
        let mut cases = Vec::new();

        // Different widths
        for width in &[180u32, 250, 350, 500] {
            cases.push(
                VisualRegressionTest::new("default", "sidebar")
                    .with_viewport(*width, 720)
                    .with_theme(TestTheme::Dark),
            );
        }

        // Light theme
        cases.push(
            VisualRegressionTest::new("default", "sidebar")
                .with_viewport(250, 720)
                .with_theme(TestTheme::Light),
        );

        // Collapsed state
        cases.push(
            VisualRegressionTest::new("collapsed", "sidebar")
                .with_viewport(60, 720)
                .with_theme(TestTheme::Dark),
        );

        // With selection
        cases.push(
            VisualRegressionTest::new("with_selection", "sidebar")
                .with_viewport(250, 720)
                .with_theme(TestTheme::Dark),
        );

        cases
    }

    #[test]
    fn test_sidebar_render_cases() {
        let cases = render_test_cases();
        assert!(cases.len() >= 6);
    }
}

// ============================================================================
// StorageControlsPage Visual Tests
// ============================================================================

/// Visual tests for StorageControls component
pub mod storage_controls_page {
    use super::*;

    /// Generate test cases for storage controls rendering
    pub fn render_test_cases() -> Vec<VisualRegressionTest> {
        let mut cases = Vec::new();

        // Different storage usage levels
        for usage_percent in &[10u32, 50, 80, 95] {
            cases.push(
                VisualRegressionTest::new(
                    &format!("usage_{}", usage_percent),
                    "storage_controls",
                )
                .with_viewport(250, 120)
                .with_theme(TestTheme::Dark),
            );
        }

        // Low storage warning state
        cases.push(
            VisualRegressionTest::new("low_storage_warning", "storage_controls")
                .with_viewport(250, 120)
                .with_theme(TestTheme::Dark),
        );

        // Light theme
        cases.push(
            VisualRegressionTest::new("default", "storage_controls")
                .with_viewport(250, 120)
                .with_theme(TestTheme::Light),
        );

        cases
    }

    #[test]
    fn test_storage_controls_render_cases() {
        let cases = render_test_cases();
        assert!(cases.len() >= 6);
    }
}

// ============================================================================
// ThemedIconGalleryPage Visual Tests
// ============================================================================

/// Visual tests for ThemedIconGallery component
pub mod themed_icon_gallery_page {
    use super::*;

    /// Icon sizes to test
    pub fn icon_sizes() -> Vec<&'static str> {
        vec!["16x16", "24x24", "32x32", "48x48", "64x64"]
    }

    /// Generate test cases for themed icon gallery rendering
    pub fn render_test_cases() -> Vec<VisualRegressionTest> {
        let mut cases = Vec::new();

        // Different icon sizes
        for size in icon_sizes() {
            let width: u32 = size.split('x').next().unwrap().parse().unwrap();

            cases.push(
                VisualRegressionTest::new(size, "themed_icon_gallery")
                    .with_viewport(width * 10, width * 2)
                    .with_theme(TestTheme::Dark),
            );

            cases.push(
                VisualRegressionTest::new(size, "themed_icon_gallery")
                    .with_viewport(width * 10, width * 2)
                    .with_theme(TestTheme::Light),
            );
        }

        cases
    }

    #[test]
    fn test_themed_icon_gallery_render_cases() {
        let cases = render_test_cases();
        assert!(cases.len() >= 10);
    }

    #[test]
    fn test_icon_sizes() {
        let sizes = icon_sizes();
        assert_eq!(sizes.len(), 5);
    }
}

// ============================================================================
// ThemedIconPage Visual Tests
// ============================================================================

/// Visual tests for individual ThemedIcon component
pub mod themed_icon_page {
    use super::*;

    /// Icon names to test
    pub fn icon_names() -> Vec<&'static str> {
        vec![
            "folder", "file", "document", "image", "video",
            "audio", "archive", "code", "settings", "search",
            "home", "back", "forward", "up", "refresh",
        ]
    }

    /// Generate test cases for themed icon rendering
    pub fn render_test_cases() -> Vec<VisualRegressionTest> {
        let mut cases = Vec::new();

        for icon_name in icon_names() {
            for theme in &[TestTheme::Dark, TestTheme::Light] {
                cases.push(
                    VisualRegressionTest::new(icon_name, "themed_icon")
                        .with_viewport(64, 64)
                        .with_theme(*theme),
                );
            }
        }

        // Different sizes
        for size in &[16u32, 24, 32, 48] {
            cases.push(
                VisualRegressionTest::new("64x64", "themed_icon")
                    .with_viewport(*size, *size)
                    .with_theme(TestTheme::Dark),
            );
        }

        cases
    }

    #[test]
    fn test_themed_icon_render_cases() {
        let cases = render_test_cases();
        // 15 icons × 2 themes + 4 sizes = 34 cases
        assert_eq!(cases.len(), 34);
    }

    #[test]
    fn test_icon_names() {
        let names = icon_names();
        assert!(names.len() >= 15);
    }
}

// ============================================================================
// ToolbarPage Visual Tests
// ============================================================================

/// Visual tests for Toolbar component
pub mod toolbar_page {
    use super::*;

    /// Toolbar button states
    #[derive(Debug, Clone)]
    pub enum ToolbarButtonState {
        Normal,
        Hover,
        Active,
        Disabled,
    }

    /// Generate test cases for toolbar rendering
    pub fn render_test_cases() -> Vec<VisualRegressionTest> {
        let mut cases = Vec::new();

        // Full toolbar width
        for theme in &[TestTheme::Dark, TestTheme::Light] {
            cases.push(
                VisualRegressionTest::new("default", "toolbar")
                    .with_viewport(800, 40)
                    .with_theme(*theme),
            );

            // Compact mode
            cases.push(
                VisualRegressionTest::new("compact", "toolbar")
                    .with_viewport(400, 40)
                    .with_theme(*theme),
            );

            // With overflow
            cases.push(
                VisualRegressionTest::new("overflow", "toolbar")
                    .with_viewport(300, 40)
                    .with_theme(*theme),
            );
        }

        cases
    }

    /// Generate button-specific test cases
    pub fn button_test_cases() -> Vec<(&'static str, ToolbarButtonState)> {
        let mut cases = Vec::new();
        let buttons = vec!["new_folder", "cut", "copy", "paste", "delete", "rename", "sort", "view"];

        for button in buttons {
            cases.push((button, ToolbarButtonState::Normal));
            cases.push((button, ToolbarButtonState::Hover));
            cases.push((button, ToolbarButtonState::Active));
            cases.push((button, ToolbarButtonState::Disabled));
        }

        cases
    }

    #[test]
    fn test_toolbar_render_cases() {
        let cases = render_test_cases();
        assert!(cases.len() >= 6);
    }

    #[test]
    fn test_toolbar_button_cases() {
        let cases = button_test_cases();
        // 8 buttons × 4 states = 32 cases
        assert_eq!(cases.len(), 32);
    }
}

// ============================================================================
// Visual Test Summary
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_visual_test_cases_generated() {
        let all_cases: Vec<VisualRegressionTest> = vec![
            breadcrumb_bar_page::render_test_cases(),
            omnibar_page::render_test_cases(),
            sidebar_view_page::render_test_cases(),
            storage_controls_page::render_test_cases(),
            themed_icon_gallery_page::render_test_cases(),
            themed_icon_page::render_test_cases(),
            toolbar_page::render_test_cases(),
        ]
        .into_iter()
        .flatten()
        .collect();

        // Total visual test cases count
        assert!(all_cases.len() >= 70);
    }

    #[test]
    fn test_dark_theme_default() {
        // Verify dark theme is default
        let test = VisualRegressionTest::new("test", "component");
        assert_eq!(test.theme, TestTheme::Dark);
    }

    #[test]
    fn test_viewport_dimensions() {
        // Verify all viewports have reasonable dimensions
        let cases = breadcrumb_bar_page::render_test_cases();
        for case in cases {
            assert!(case.viewport.0 > 0);
            assert!(case.viewport.1 > 0);
            assert!(case.viewport.0 <= 3840); // Max 4K width
            assert!(case.viewport.1 <= 2160); // Max 4K height
        }
    }

    #[test]
    fn test_baseline_path_uniqueness() {
        let cases: Vec<VisualRegressionTest> = breadcrumb_bar_page::render_test_cases();

        let paths: Vec<String> = cases
            .iter()
            .map(|c| {
                c.baseline_path(&PathBuf::from("/baseline"))
                    .to_string_lossy()
                    .to_string()
            })
            .collect();

        // All paths should be unique
        let mut sorted_paths = paths.clone();
        sorted_paths.sort();
        sorted_paths.dedup();
        assert_eq!(paths.len(), sorted_paths.len());
    }
}
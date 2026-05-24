//! Empty States — SVG empty state illustrations for Shelf and Sidebar panels.
//! Per DESIGN_SPEC.md section 6 / TODO.md Section 6.
//!
//! This module provides empty state data and logic for:
//! - Shelf pane (staging area) when empty
//! - Sidebar when empty
//! - Other panels that may need empty state indicators

use crate::ui::constants::UiConstants;

/// Represents which theme variant to use for empty state icons.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EmptyStateTheme {
    #[default]
    Dark,
    Light,
}

/// Represents the size variant for empty state icons.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmptyStateSize {
    Small,  // 48px
    Medium, // 100px
    Large,  // 200px
}

/// Target panel for empty state display.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EmptyStateTarget {
    /// Empty Shelf (staging area) panel
    Shelf,
    /// Empty Sidebar panel
    Sidebar,
    /// Empty folder in the main view
    Folder,
    /// Empty search results
    SearchResults,
    /// Empty recycle bin
    RecycleBin,
    /// Generic empty state
    Generic,
}

/// Data for rendering an empty state.
#[derive(Debug, Clone)]
pub struct EmptyStateData {
    /// Target panel for this empty state
    pub target: EmptyStateTarget,
    /// Primary message to display
    pub title: String,
    /// Secondary description text
    pub description: String,
    /// Action hint text (e.g., "Drop files here")
    pub action_hint: Option<String>,
    /// Icon code point (Segoe MDL2 Assets)
    pub icon: Option<&'static str>,
    /// Whether to show a drag-and-drop hint
    pub show_drop_hint: bool,
}

/// Service for managing empty state rendering and logic.
#[derive(Debug, Clone, Default)]
pub struct EmptyStateService;

impl EmptyStateService {
    pub fn new() -> Self {
        Self
    }

    /// Get empty state data for a given target and theme.
    pub fn get_empty_state(&self, target: EmptyStateTarget) -> EmptyStateData {
        match target {
            EmptyStateTarget::Shelf => EmptyStateData {
                target: EmptyStateTarget::Shelf,
                title: "No items staged".to_string(),
                description: "Drag files here to stage them for operations".to_string(),
                action_hint: Some("Drop files here".to_string()),
                icon: Some("\u{E8B7}"), // Folder icon
                show_drop_hint: true,
            },
            EmptyStateTarget::Sidebar => EmptyStateData {
                target: EmptyStateTarget::Sidebar,
                title: "Sidebar".to_string(),
                description: "This section is empty".to_string(),
                action_hint: None,
                icon: Some("\u{E700}"), // Hamburger menu icon
                show_drop_hint: false,
            },
            EmptyStateTarget::Folder => EmptyStateData {
                target: EmptyStateTarget::Folder,
                title: "This folder is empty".to_string(),
                description: "Drop files here or create new ones".to_string(),
                action_hint: Some("Create new item".to_string()),
                icon: Some("\u{E8B7}"), // Folder icon
                show_drop_hint: true,
            },
            EmptyStateTarget::SearchResults => EmptyStateData {
                target: EmptyStateTarget::SearchResults,
                title: "No results found".to_string(),
                description: "Try a different search term".to_string(),
                action_hint: None,
                icon: Some("\u{E721}"), // Search icon
                show_drop_hint: false,
            },
            EmptyStateTarget::RecycleBin => EmptyStateData {
                target: EmptyStateTarget::RecycleBin,
                title: "Recycle Bin is empty".to_string(),
                description: "Deleted items will appear here".to_string(),
                action_hint: None,
                icon: Some("\u{E74D}"), // Delete icon
                show_drop_hint: false,
            },
            EmptyStateTarget::Generic => EmptyStateData {
                target: EmptyStateTarget::Generic,
                title: "Nothing here".to_string(),
                description: "This area is empty".to_string(),
                action_hint: None,
                icon: None,
                show_drop_hint: false,
            },
        }
    }

    /// Get the appropriate SVG asset name for a given target, size, and theme.
    pub fn get_asset_name(
        &self,
        target: EmptyStateTarget,
        size: EmptyStateSize,
        theme: EmptyStateTheme,
    ) -> String {
        let size_str = match size {
            EmptyStateSize::Small => "48",
            EmptyStateSize::Medium => "100",
            EmptyStateSize::Large => "200",
        };
        let theme_str = match theme {
            EmptyStateTheme::Dark => "ThemeDark",
            EmptyStateTheme::Light => "ThemeLight",
        };
        let panel_str = match target {
            EmptyStateTarget::Shelf => "EmptyShelf",
            EmptyStateTarget::Sidebar => "EmptySidebar",
            _ => "EmptyShelf", // Default to Shelf for unknown types
        };
        format!("{}_{}_{}.svg", panel_str, size_str, theme_str)
    }

    /// Check if empty state should be shown based on item count.
    pub fn should_show_empty_state(&self, item_count: usize, force_show: bool) -> bool {
        force_show || item_count == 0
    }

    /// Get suggested icon size for empty state display.
    pub fn get_icon_size(&self, size: EmptyStateSize) -> f64 {
        match size {
            EmptyStateSize::Small => 48.0,
            EmptyStateSize::Medium => 100.0,
            EmptyStateSize::Large => 200.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_state_service_new() {
        let service = EmptyStateService::new();
        // Should not panic
        drop(service);
    }

    #[test]
    fn test_empty_state_service_default() {
        let service = EmptyStateService::default();
        drop(service);
    }

    #[test]
    fn test_get_empty_state_shelf() {
        let service = EmptyStateService::new();
        let state = service.get_empty_state(EmptyStateTarget::Shelf);
        assert_eq!(state.title, "No items staged");
        assert!(state.show_drop_hint);
        assert!(state.action_hint.is_some());
    }

    #[test]
    fn test_get_empty_state_folder() {
        let service = EmptyStateService::new();
        let state = service.get_empty_state(EmptyStateTarget::Folder);
        assert_eq!(state.title, "This folder is empty");
        assert!(state.show_drop_hint);
    }

    #[test]
    fn test_get_empty_state_search_results() {
        let service = EmptyStateService::new();
        let state = service.get_empty_state(EmptyStateTarget::SearchResults);
        assert_eq!(state.title, "No results found");
        assert!(!state.show_drop_hint);
    }

    #[test]
    fn test_get_empty_state_recycle_bin() {
        let service = EmptyStateService::new();
        let state = service.get_empty_state(EmptyStateTarget::RecycleBin);
        assert_eq!(state.title, "Recycle Bin is empty");
        assert!(!state.show_drop_hint);
    }

    #[test]
    fn test_get_asset_name_shelf_dark_medium() {
        let service = EmptyStateService::new();
        let name = service.get_asset_name(
            EmptyStateTarget::Shelf,
            EmptyStateSize::Medium,
            EmptyStateTheme::Dark,
        );
        assert_eq!(name, "EmptyShelf_100_ThemeDark.svg");
    }

    #[test]
    fn test_get_asset_name_sidebar_light_large() {
        let service = EmptyStateService::new();
        let name = service.get_asset_name(
            EmptyStateTarget::Sidebar,
            EmptyStateSize::Large,
            EmptyStateTheme::Light,
        );
        assert_eq!(name, "EmptySidebar_200_ThemeLight.svg");
    }

    #[test]
    fn test_get_asset_name_sidebar_dark_small() {
        let service = EmptyStateService::new();
        let name = service.get_asset_name(
            EmptyStateTarget::Sidebar,
            EmptyStateSize::Small,
            EmptyStateTheme::Dark,
        );
        assert_eq!(name, "EmptySidebar_48_ThemeDark.svg");
    }

    #[test]
    fn test_should_show_empty_state_zero_items() {
        let service = EmptyStateService::new();
        assert!(service.should_show_empty_state(0, false));
    }

    #[test]
    fn test_should_show_empty_state_with_items() {
        let service = EmptyStateService::new();
        assert!(!service.should_show_empty_state(5, false));
    }

    #[test]
    fn test_should_show_empty_state_force_show() {
        let service = EmptyStateService::new();
        assert!(service.should_show_empty_state(5, true));
    }

    #[test]
    fn test_get_icon_size_variants() {
        let service = EmptyStateService::new();
        assert_eq!(service.get_icon_size(EmptyStateSize::Small), 48.0);
        assert_eq!(service.get_icon_size(EmptyStateSize::Medium), 100.0);
        assert_eq!(service.get_icon_size(EmptyStateSize::Large), 200.0);
    }

    #[test]
    fn test_empty_state_theme_variants() {
        let theme_dark = EmptyStateTheme::Dark;
        let theme_light = EmptyStateTheme::Light;
        assert_ne!(theme_dark, theme_light);
    }

    #[test]
    fn test_empty_state_size_variants() {
        let size_small = EmptyStateSize::Small;
        let size_medium = EmptyStateSize::Medium;
        let size_large = EmptyStateSize::Large;
        assert_ne!(size_small, size_medium);
        assert_ne!(size_medium, size_large);
    }

    #[test]
    fn test_empty_state_target_variants() {
        let targets = vec![
            EmptyStateTarget::Shelf,
            EmptyStateTarget::Sidebar,
            EmptyStateTarget::Folder,
            EmptyStateTarget::SearchResults,
            EmptyStateTarget::RecycleBin,
            EmptyStateTarget::Generic,
        ];
        assert_eq!(targets.len(), 6);
    }

    #[test]
    fn test_empty_state_data_clone() {
        let data = EmptyStateData {
            target: EmptyStateTarget::Folder,
            title: "Test".to_string(),
            description: "Description".to_string(),
            action_hint: Some("Hint".to_string()),
            icon: Some("\u{E8B7}"),
            show_drop_hint: true,
        };
        let cloned = data.clone();
        assert_eq!(data.title, cloned.title);
        assert_eq!(data.target, cloned.target);
    }
}

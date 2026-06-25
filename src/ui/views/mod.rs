//! Views module - File browser view implementations
//!
//! This module contains separate implementations for each view mode:
//! - List View: Compact single-column file listing
//! - Grid View: Icon/grid/tile display with dynamic zoom levels
//! - Columns View: Finder-style three-pane layout
//! - Details View: Multi-column detailed file listing
//!
//! Each view has both flat and grouped variants for optimal performance
//! and organized file browsing.

pub mod columns_view;
pub mod grid_view;
pub mod helpers;
pub mod list_view;
pub mod types;

// Keep main_view for shared types and state management
pub mod details_view;
pub mod main_view;
pub mod main_view_grouped;
pub mod main_view_home;
pub mod main_view_preview;
pub mod main_view_recycle;
pub mod main_view_selection;
pub mod main_view_settings;

// Explicit re-exports for types used via `crate::ui::views::TypeName`
pub use main_view::{
    ColumnResizeState, ColumnWidths, DetailsPaneCache, FileItems, GroupDragState, MainView,
    MainViewState,
};
pub use types::PaneType;

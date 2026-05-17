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

pub mod types;
pub mod helpers;
pub mod list_view;
pub mod grid_view;
pub mod columns_view;

// Keep main_view for shared types and state management
pub mod main_view;
pub mod details_view;

// Re-exports for convenience
pub use types::*;
pub use helpers::*;
pub use main_view::*;

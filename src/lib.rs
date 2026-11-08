#![recursion_limit = "256"]

pub mod app;
mod app_actions;
mod app_navigation;
mod app_render;
pub mod app_state;
pub mod config;
pub mod core;
pub mod domain;
pub mod localization;
pub mod platform;
pub mod storage;
pub mod ui;
pub mod wallpaper_mica;

// Explicit re-exports for types used via `files_rs::TypeName` in main.rs
// All other access goes through the module paths (e.g., files_rs::core::path_util::FilesPath)

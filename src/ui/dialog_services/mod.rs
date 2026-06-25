//! Dialog Services
//! State machines for various dialog components in Section 6 UI.

pub mod adaptive_view;
pub mod archive_dialog;
pub mod clone_repo_dialog;
pub mod conflict_dialog;
pub mod credential_dialog;
pub mod custom_properties_dialog;
pub mod decompress_dialog;
pub mod file_too_large_dialog;
pub mod properties_mapping;
pub mod shortcut_dialog;

// No glob re-exports — all access goes through specific module paths
// e.g., crate::ui::dialog_services::archive_dialog::ArchiveFormat

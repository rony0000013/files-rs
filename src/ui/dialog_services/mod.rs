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

// Re-export for convenience
pub use adaptive_view::*;
pub use archive_dialog::*;
pub use clone_repo_dialog::*;
pub use conflict_dialog::*;
pub use credential_dialog::*;
pub use custom_properties_dialog::*;
pub use decompress_dialog::*;
pub use file_too_large_dialog::*;
pub use properties_mapping::*;
pub use shortcut_dialog::*;

//! Sidebar state and data models.
//! Per DESIGN_SPEC.md section 6.
//!
//! On non-Windows, this module provides a stub implementation.

#![recursion_limit = "256"]

use crate::core::path_util::FilesPath;
use crate::ui::constants::UiConstants;

#[cfg(windows)]
use crate::config::config::AppConfig;
#[cfg(windows)]
use crate::platform::windows::cloud_drive_service::{enumerate_cloud_drives, CloudProvider};
#[cfg(windows)]
use crate::platform::windows::drive_service::{enumerate_drives, DriveType};
#[cfg(windows)]
use crate::platform::windows::wsl_service::enumerate_wsl_distros;

// ─────────────────────────────────────────────────────────────────────────────
// Data models — shared by sidebar_state and sidebar view
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SidebarItemId {
    Pinned(FilesPath),
    Tag(String),
    Library(String),
    #[cfg(windows)]
    Drive {
        root: FilesPath,
        drive_type: DriveType,
    },
    #[cfg(not(windows))]
    Drive {
        root: FilesPath,
        drive_type: (),
    },
    #[cfg(windows)]
    CloudDrive(CloudProvider),
    #[cfg(not(windows))]
    CloudDrive(()),
    Network,
    WslDistro(String),
    RecycleBin,
}

impl SidebarItemId {
    pub fn get_path(&self) -> Option<FilesPath> {
        match self {
            Self::Pinned(path) => Some(path.clone()),
            Self::Library(name) => {
                match name.as_str() {
                    "Desktop" => Some(FilesPath::from(dirs::desktop_dir()?)),
                    "Documents" => Some(FilesPath::from(dirs::document_dir()?)),
                    "Downloads" => Some(FilesPath::from(dirs::download_dir()?)),
                    "Pictures" => Some(FilesPath::from(dirs::picture_dir()?)),
                    "Music" => Some(FilesPath::from(dirs::audio_dir()?)),
                    "Videos" => Some(FilesPath::from(dirs::video_dir()?)),
                    _ => None,
                }
            }
            #[cfg(windows)]
            Self::Drive { root, .. } => Some(root.clone()),
            Self::RecycleBin => {
                #[cfg(windows)]
                {
                    Some(FilesPath::new("shell:RecycleBinFolder"))
                }
                #[cfg(not(windows))]
                {
                    None
                }
            }
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SidebarItem {
    pub id: SidebarItemId,
    pub label: String,
    pub icon: String,
    pub badge: Option<SidebarBadge>,
}

#[derive(Debug, Clone)]
pub enum SidebarBadge {
    Count(usize),
    Usage(f32),
}

#[derive(Debug, Clone)]
pub struct SidebarSectionData {
    pub id: SectionId,
    pub title: String,
    pub items: Vec<SidebarItem>,
    pub collapsed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SectionId {
    Favorites,
    Tags,
    Libraries,
    ThisPc,
    CloudDrives,
    Network,
    Wsl,
}

// ─────────────────────────────────────────────────────────────────────────────
// State
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Default)]
pub struct SidebarState {
    pub sections: Vec<SidebarSectionData>,
    pub active_item: Option<SidebarItemId>,
    pub collapsed: bool,
    pub width: f32,
}

impl SidebarState {
    pub fn new() -> Self {
        let mut state = Self::default();
        state.width = UiConstants::SIDEBAR_WIDTH_RAW as f32;
        state.reload();
        state
    }

    pub fn reload(&mut self) {
        self.sections = build_all_sections();
    }

    pub fn toggle_section(&mut self, section: SectionId) {
        if let Some(s) = self.sections.iter_mut().find(|s| s.id == section) {
            s.collapsed = !s.collapsed;
        }
    }

    pub fn set_collapsed(&mut self, collapsed: bool) {
        self.collapsed = collapsed;
    }

    /// Mark a path as the active sidebar item.
    #[allow(dead_code)]
    pub fn set_active(&mut self, path: &FilesPath) {
        // Try to find a sidebar item that matches this path
        for section in &self.sections {
            for item in &section.items {
                if let Some(item_path) = item.id.get_path() {
                    if item_path == *path {
                        self.active_item = Some(item.id.clone());
                        return;
                    }
                }
            }
        }
        self.active_item = None;
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Section building — canonical version
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(windows)]
fn build_favorites_section() -> SidebarSectionData {
    let config = AppConfig::load();
    let mut items: Vec<SidebarItem> = config
        .sidebar
        .pinned_locations
        .iter()
        .map(|path_str| {
            let path = FilesPath::new(path_str.as_str());
            let label = path
                .as_path()
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| path_str.clone());
            SidebarItem {
                id: SidebarItemId::Pinned(path.clone()),
                label,
                icon: "\u{E8B7}".to_string(), // Folder glyph
                badge: None,
            }
        })
        .collect();

    // Add Recycle Bin
    items.push(SidebarItem {
        id: SidebarItemId::RecycleBin,
        label: "Recycle Bin".to_string(),
        icon: "\u{E74D}".to_string(),        // Delete/Recycle glyph
        badge: Some(SidebarBadge::Count(0)), // Mock count
    });

    SidebarSectionData {
        id: SectionId::Favorites,
        title: "Favorites".to_string(),
        items,
        collapsed: false,
    }
}

#[cfg(not(windows))]
fn build_favorites_section() -> SidebarSectionData {
    SidebarSectionData {
        id: SectionId::Favorites,
        title: "Favorites".to_string(),
        items: vec![],
        collapsed: false,
    }
}

#[cfg(windows)]
fn build_libraries_section() -> SidebarSectionData {
    let mut items = Vec::new();

    if dirs::home_dir().is_some() {
        items.push(SidebarItem {
            id: SidebarItemId::Library("Desktop".to_string()),
            label: "Desktop".to_string(),
            icon: "\u{E8B7}".to_string(),
            badge: None,
        });
        if dirs::document_dir().is_some() {
            items.push(SidebarItem {
                id: SidebarItemId::Library("Documents".to_string()),
                label: "Documents".to_string(),
                icon: "\u{E8B7}".to_string(),
                badge: None,
            });
        }
        if dirs::download_dir().is_some() {
            items.push(SidebarItem {
                id: SidebarItemId::Library("Downloads".to_string()),
                label: "Downloads".to_string(),
                icon: "\u{E8B7}".to_string(),
                badge: None,
            });
        }
        if dirs::picture_dir().is_some() {
            items.push(SidebarItem {
                id: SidebarItemId::Library("Pictures".to_string()),
                label: "Pictures".to_string(),
                icon: "\u{E8B7}".to_string(),
                badge: None,
            });
        }
        if dirs::audio_dir().is_some() {
            items.push(SidebarItem {
                id: SidebarItemId::Library("Music".to_string()),
                label: "Music".to_string(),
                icon: "\u{E8B7}".to_string(),
                badge: None,
            });
        }
        if dirs::video_dir().is_some() {
            items.push(SidebarItem {
                id: SidebarItemId::Library("Videos".to_string()),
                label: "Videos".to_string(),
                icon: "\u{E714}".to_string(),
                badge: None,
            });
        }
    }

    SidebarSectionData {
        id: SectionId::Libraries,
        title: "Libraries".to_string(),
        items,
        collapsed: false,
    }
}

#[cfg(not(windows))]
fn build_libraries_section() -> SidebarSectionData {
    SidebarSectionData {
        id: SectionId::Libraries,
        title: "Libraries".to_string(),
        items: vec![],
        collapsed: false,
    }
}

#[cfg(windows)]
fn build_drives_section() -> SidebarSectionData {
    let items: Vec<SidebarItem> = enumerate_drives()
        .into_iter()
        .map(|d| {
            let label = if d.label.is_empty() {
                d.root.as_path().to_string_lossy().to_string()
            } else {
                d.label.clone()
            };
            let icon = drive_icon(d.drive_type);
            let badge = if d.total_bytes > 0 {
                Some(SidebarBadge::Usage(d.usage_ratio() as f32))
            } else {
                None
            };
            SidebarItem {
                id: SidebarItemId::Drive {
                    root: d.root.clone(),
                    drive_type: d.drive_type,
                },
                label,
                icon: icon.to_string(),
                badge,
            }
        })
        .collect();
    SidebarSectionData {
        id: SectionId::ThisPc,
        title: "This PC".to_string(),
        items,
        collapsed: false,
    }
}

#[cfg(not(windows))]
fn build_drives_section() -> SidebarSectionData {
    SidebarSectionData {
        id: SectionId::ThisPc,
        title: "This PC".to_string(),
        items: vec![],
        collapsed: false,
    }
}

#[cfg(windows)]
fn build_cloud_drives_section() -> SidebarSectionData {
    let items: Vec<SidebarItem> = enumerate_cloud_drives()
        .into_iter()
        .map(|d| {
            let icon = cloud_icon(d.provider);
            SidebarItem {
                id: SidebarItemId::CloudDrive(d.provider),
                label: d.display_name,
                icon: icon.to_string(),
                badge: None,
            }
        })
        .collect();
    SidebarSectionData {
        id: SectionId::CloudDrives,
        title: "Cloud Drives".to_string(),
        items,
        collapsed: false,
    }
}

#[cfg(not(windows))]
fn build_cloud_drives_section() -> SidebarSectionData {
    SidebarSectionData {
        id: SectionId::CloudDrives,
        title: "Cloud Drives".to_string(),
        items: vec![],
        collapsed: false,
    }
}

fn build_network_section() -> SidebarSectionData {
    SidebarSectionData {
        id: SectionId::Network,
        title: "Network".to_string(),
        items: vec![SidebarItem {
            id: SidebarItemId::Network,
            label: "Network".to_string(),
            icon: "\u{E802}".to_string(),
            badge: None,
        }],
        collapsed: false,
    }
}

#[cfg(windows)]
fn build_wsl_section() -> SidebarSectionData {
    let items: Vec<SidebarItem> = enumerate_wsl_distros()
        .into_iter()
        .map(|d| SidebarItem {
            id: SidebarItemId::WslDistro(d.name.clone()),
            label: d.display_name,
            icon: "\u{E756}".to_string(),
            badge: None,
        })
        .collect();
    SidebarSectionData {
        id: SectionId::Wsl,
        title: "WSL".to_string(),
        items,
        collapsed: false,
    }
}

#[cfg(not(windows))]
fn build_wsl_section() -> SidebarSectionData {
    SidebarSectionData {
        id: SectionId::Wsl,
        title: "WSL".to_string(),
        items: vec![],
        collapsed: false,
    }
}

#[cfg(windows)]
fn drive_icon(dt: DriveType) -> &'static str {
    match dt {
        DriveType::Fixed => "\u{E770}",     // HDD
        DriveType::Removable => "\u{E88E}", // USB
        DriveType::Network => "\u{E710}",   // Network
        DriveType::CdRom => "\u{E958}",     // CD
        _ => "\u{E770}",
    }
}

#[cfg(not(windows))]
fn drive_icon(_dt: ()) -> &'static str {
    "chrome://vector_icons/fluent/drive.svg"
}

#[cfg(windows)]
fn cloud_icon(provider: CloudProvider) -> &'static str {
    match provider {
        CloudProvider::OneDrive => "\u{E753}", // Cloud
        CloudProvider::GoogleDrive => "\u{E753}",
        CloudProvider::Dropbox => "\u{E753}",
        CloudProvider::iCloud => "\u{E753}",
    }
}

#[cfg(not(windows))]
fn cloud_icon(_provider: ()) -> &'static str {
    "chrome://vector_icons/fluent/cloud.svg"
}

pub fn build_all_sections() -> Vec<SidebarSectionData> {
    vec![
        build_favorites_section(),
        SidebarSectionData {
            id: SectionId::Tags,
            title: "Tags".to_string(),
            items: vec![
                SidebarItem {
                    id: SidebarItemId::Tag("Red".to_string()),
                    label: "Red".to_string(),
                    icon: "\u{E8EC}".to_string(),
                    badge: None,
                },
                SidebarItem {
                    id: SidebarItemId::Tag("Blue".to_string()),
                    label: "Blue".to_string(),
                    icon: "\u{E8EC}".to_string(), // Tag glyph
                    badge: None,
                },
            ],
            collapsed: false,
        },
        build_libraries_section(),
        build_drives_section(),
        build_cloud_drives_section(),
        build_network_section(),
        build_wsl_section(),
    ]
}

// ─────────────────────────────────────────────────────────────────────────────
// DriveWatcher — Hot-Swap Detection for sidebar drives
// ─────────────────────────────────────────────────────────────────────────────

/// Polls for drive changes (hot-plug/hot-unplug) and signals when the
/// sidebar should refresh. On non-Windows, polls `/mnt/` entries.
/// On Windows, checks drive letters `A:` through `Z:`.
#[derive(Clone)]
pub struct DriveWatcher {
    /// The set of known mount points / drive roots from the last poll.
    known_drives: std::collections::HashSet<String>,
    /// Whether a change has been detected since the last clear.
    changed: bool,
    /// Interval between polls (default: 5 seconds).
    poll_interval_secs: u64,
}

impl Default for DriveWatcher {
    fn default() -> Self {
        Self {
            known_drives: Self::snapshot_drives(),
            changed: false,
            poll_interval_secs: 5,
        }
    }
}

impl DriveWatcher {
    /// Create a new DriveWatcher and take the initial drive snapshot.
    pub fn new() -> Self {
        Self::default()
    }

    /// Check for drive changes and update internal state.
    /// Returns `true` if the set of drives has changed.
    pub fn poll(&mut self) -> bool {
        let current = Self::snapshot_drives();
        if current != self.known_drives {
            self.known_drives = current;
            self.changed = true;
            true
        } else {
            false
        }
    }

    /// Whether a change has been detected.
    pub fn has_changed(&self) -> bool {
        self.changed
    }

    /// Reset the changed flag (e.g., after handling the change).
    pub fn clear_changed(&mut self) {
        self.changed = false;
    }

    /// Get the poll interval in seconds.
    pub fn poll_interval_secs(&self) -> u64 {
        self.poll_interval_secs
    }

    /// Set a custom poll interval.
    pub fn set_poll_interval_secs(&mut self, interval: u64) {
        self.poll_interval_secs = interval.max(1);
    }

    /// Take a snapshot of currently available drives/mounts.
    fn snapshot_drives() -> std::collections::HashSet<String> {
        let mut drives = std::collections::HashSet::new();

        #[cfg(windows)]
        {
            // On Windows, check drive letters A: through Z:
            for letter in 'A'..='Z' {
                let root = format!("{}:\\", letter);
                let path = std::path::Path::new(&root);
                if path.exists() {
                    drives.insert(root);
                }
            }
        }

        #[cfg(not(windows))]
        {
            // On Linux/macOS, read /mnt/ directory entries
            if let Ok(entries) = std::fs::read_dir("/mnt/") {
                for entry in entries.flatten() {
                    if let Ok(file_type) = entry.file_type() {
                        if file_type.is_dir() || file_type.is_symlink() {
                            if let Ok(name) = entry.file_name().into_string() {
                                drives.insert(format!("/mnt/{}", name));
                            }
                        }
                    }
                }
            }
        }

        drives
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ─────────────────────────────────────────────────────────────────────────
    // SidebarItemId tests
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_sidebar_item_id_pinned_get_path() {
        let path = FilesPath::from(std::path::PathBuf::from("/test/path"));
        let item_id = SidebarItemId::Pinned(path.clone());
        assert_eq!(item_id.get_path(), Some(path));
    }

    #[test]
    fn test_sidebar_item_id_library_desktop_get_path() {
        let item_id = SidebarItemId::Library("Desktop".to_string());
        let result = item_id.get_path();
        // Result depends on whether we have a home dir
        if let Some(path) = result {
            assert!(path.as_path().to_string_lossy().ends_with("Desktop"));
        }
    }

    #[test]
    fn test_sidebar_item_id_library_unknown_returns_none() {
        let item_id = SidebarItemId::Library("UnknownLibrary".to_string());
        assert_eq!(item_id.get_path(), None);
    }

    #[test]
    fn test_sidebar_item_id_tag_returns_none() {
        let item_id = SidebarItemId::Tag("Red".to_string());
        assert_eq!(item_id.get_path(), None);
    }

    #[test]
    fn test_sidebar_item_id_network_returns_none() {
        let item_id = SidebarItemId::Network;
        assert_eq!(item_id.get_path(), None);
    }

    #[test]
    fn test_sidebar_item_id_wsl_distro_returns_none() {
        let item_id = SidebarItemId::WslDistro("Ubuntu".to_string());
        assert_eq!(item_id.get_path(), None);
    }

    #[test]
    fn test_sidebar_item_id_recycle_bin_get_path() {
        let item_id = SidebarItemId::RecycleBin;
        #[cfg(windows)]
        assert!(item_id.get_path().is_some());
        #[cfg(not(windows))]
        assert_eq!(item_id.get_path(), None);
    }

    #[test]
    fn test_sidebar_item_id_partial_eq() {
        let path1 = FilesPath::from(std::path::PathBuf::from("/test/path"));
        let path2 = FilesPath::from(std::path::PathBuf::from("/test/path"));
        let item1 = SidebarItemId::Pinned(path1);
        let item2 = SidebarItemId::Pinned(path2);
        assert_eq!(item1, item2);
    }

    #[test]
    fn test_sidebar_item_id_clone() {
        let path = FilesPath::from(std::path::PathBuf::from("/test/path"));
        let item_id = SidebarItemId::Pinned(path);
        let cloned = item_id.clone();
        assert_eq!(item_id.get_path(), cloned.get_path());
    }

    // ─────────────────────────────────────────────────────────────────────────
    // SidebarBadge tests
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_sidebar_badge_count() {
        let badge = SidebarBadge::Count(42);
        match badge {
            SidebarBadge::Count(n) => assert_eq!(n, 42),
            _ => panic!("Expected Count variant"),
        }
    }

    #[test]
    fn test_sidebar_badge_usage() {
        let badge = SidebarBadge::Usage(0.75);
        match badge {
            SidebarBadge::Usage(u) => assert!((u - 0.75).abs() < f32::EPSILON),
            _ => panic!("Expected Usage variant"),
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // SectionId tests
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_section_id_all_variants() {
        // Ensure all section IDs can be created and compared
        let ids = vec![
            SectionId::Favorites,
            SectionId::Tags,
            SectionId::Libraries,
            SectionId::ThisPc,
            SectionId::CloudDrives,
            SectionId::Network,
            SectionId::Wsl,
        ];
        assert_eq!(ids.len(), 7);
    }

    #[test]
    fn test_section_id_partial_eq() {
        assert_eq!(SectionId::Favorites, SectionId::Favorites);
        assert_ne!(SectionId::Favorites, SectionId::Tags);
    }

    #[test]
    fn test_section_id_copy() {
        let id1 = SectionId::Favorites;
        let id2 = id1;
        assert_eq!(id1, id2);
    }

    #[test]
    fn test_section_id_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(SectionId::Favorites);
        set.insert(SectionId::Tags);
        set.insert(SectionId::Libraries);
        assert_eq!(set.len(), 3);
    }

    // ─────────────────────────────────────────────────────────────────────────
    // SidebarItem tests
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_sidebar_item_creation() {
        let item = SidebarItem {
            id: SidebarItemId::Network,
            label: "Network".to_string(),
            icon: "\u{E802}".to_string(),
            badge: None,
        };
        assert_eq!(item.label, "Network");
        assert_eq!(item.icon, "\u{E802}");
        assert!(item.badge.is_none());
    }

    #[test]
    fn test_sidebar_item_with_badge() {
        let item = SidebarItem {
            id: SidebarItemId::RecycleBin,
            label: "Recycle Bin".to_string(),
            icon: "\u{E74D}".to_string(),
            badge: Some(SidebarBadge::Count(5)),
        };
        assert!(item.badge.is_some());
    }

    #[test]
    fn test_sidebar_item_clone() {
        let item = SidebarItem {
            id: SidebarItemId::Network,
            label: "Network".to_string(),
            icon: "\u{E802}".to_string(),
            badge: None,
        };
        let cloned = item.clone();
        assert_eq!(item.label, cloned.label);
        assert_eq!(item.id, cloned.id);
    }

    // ─────────────────────────────────────────────────────────────────────────
    // SidebarSectionData tests
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_sidebar_section_data_creation() {
        let section = SidebarSectionData {
            id: SectionId::Network,
            title: "Network".to_string(),
            items: vec![],
            collapsed: false,
        };
        assert_eq!(section.title, "Network");
        assert!(!section.collapsed);
        assert!(section.items.is_empty());
    }

    #[test]
    fn test_sidebar_section_data_with_items() {
        let items = vec![
            SidebarItem {
                id: SidebarItemId::Network,
                label: "Network".to_string(),
                icon: "\u{E802}".to_string(),
                badge: None,
            },
        ];
        let section = SidebarSectionData {
            id: SectionId::Network,
            title: "Network".to_string(),
            items,
            collapsed: false,
        };
        assert_eq!(section.items.len(), 1);
    }

    // ─────────────────────────────────────────────────────────────────────────
    // SidebarState tests
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_sidebar_state_default() {
        let state = SidebarState::default();
        assert!(state.sections.is_empty());
        assert!(state.active_item.is_none());
        assert!(!state.collapsed);
        assert_eq!(state.width, 0.0);
    }

    #[test]
    fn test_sidebar_state_set_collapsed() {
        let mut state = SidebarState::default();
        state.set_collapsed(true);
        assert!(state.collapsed);
        state.set_collapsed(false);
        assert!(!state.collapsed);
    }

    #[test]
    fn test_sidebar_state_toggle_section() {
        let mut state = SidebarState::default();
        // Can't toggle non-existent section without adding sections first
        // But we can verify it doesn't panic
        state.toggle_section(SectionId::Favorites);
    }

    #[test]
    fn test_sidebar_state_set_active() {
        let mut state = SidebarState::default();
        let test_path = FilesPath::from(std::path::PathBuf::from("/nonexistent/path"));
        state.set_active(&test_path);
        // Should set to None since the path doesn't match any sidebar item
        assert!(state.active_item.is_none());
    }

    #[test]
    fn test_sidebar_state_with_manual_sections() {
        let sections = vec![
            SidebarSectionData {
                id: SectionId::Favorites,
                title: "Favorites".to_string(),
                items: vec![
                    SidebarItem {
                        id: SidebarItemId::RecycleBin,
                        label: "Recycle Bin".to_string(),
                        icon: "\u{E74D}".to_string(),
                        badge: None,
                    },
                ],
                collapsed: false,
            },
        ];
        let mut state = SidebarState {
            sections,
            active_item: None,
            collapsed: false,
            width: 250.0,
        };

        // Test toggle section
        state.toggle_section(SectionId::Favorites);
        assert!(state.sections[0].collapsed);

        // Now add a path for RecycleBin and test set_active
        if let Some(recycle_path) = SidebarItemId::RecycleBin.get_path() {
            state.set_active(&recycle_path);
            match &state.active_item {
                Some(SidebarItemId::RecycleBin) => (),
                other => panic!("Expected RecycleBin, got {:?}", other),
            }
        }
    }
}

//! Sidebar state and data models.
//! Per DESIGN_SPEC.md section 6.
//!
//! On non-Windows, this module provides a stub implementation.

use crate::core::path_util::FilesPath;
use crate::ui::components::bouncy_scroll::BouncyScrollState;
use crate::ui::constants::UiConstants;

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
            Self::Library(name) => match name.as_str() {
                "Desktop" => Self::library_path(dirs::desktop_dir(), "Desktop"),
                "Documents" => Self::library_path(dirs::document_dir(), "Documents"),
                "Downloads" => Self::library_path(dirs::download_dir(), "Downloads"),
                "Pictures" => Self::library_path(dirs::picture_dir(), "Pictures"),
                "Music" => Self::library_path(dirs::audio_dir(), "Music"),
                "Videos" => Self::library_path(dirs::video_dir(), "Videos"),
                _ => None,
            },
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

    /// Resolve a shell library folder, tolerating broken known-folder
    /// registration (e.g. OneDrive desync): prefer the OS-reported path when it
    /// exists, else fall back to `%USERPROFILE%\OneDrive\<name>` or
    /// `%USERPROFILE%\<name>`.
    fn library_path(primary: Option<std::path::PathBuf>, name: &str) -> Option<FilesPath> {
        if let Some(p) = primary {
            if p.is_dir() {
                return Some(FilesPath::from(p));
            }
        }
        if let Some(home) = dirs::home_dir() {
            for candidate in [
                home.join("OneDrive").join(name),
                home.join(name),
            ] {
                if candidate.is_dir() {
                    return Some(FilesPath::from(candidate));
                }
            }
        }
        None
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
    /// Display order of sections by SectionId
    pub section_display_order: Vec<SectionId>,

    // Resize drag tracking
    pub resize_start_x: Option<f32>,
    pub resize_start_width: Option<f32>,
    /// Last width that triggered a notification (avoids per-pixel re-render)
    pub last_notified_width: f32,

    /// Width animation for smooth collapse/expand
    pub anim_start_time: Option<std::time::Instant>,
    pub anim_from_width: f32,
    pub anim_to_width: f32,

    // Section reorder drag tracking
    pub drag_section_idx: Option<usize>,
    pub drag_section_current_y: f32,
    pub drop_target_section_idx: Option<usize>,
    pub saved_order: Vec<SectionId>,

    /// Which section's tooltip/flyout is shown in collapsed mode
    pub tooltip_section: Option<SectionId>,

    /// Animation version counter — incremented on collapse toggle to restart animation
    pub anim_version: u64,
    /// Width before collapsing (so we can animate back to it on expand)
    pub previous_width: f32,
    /// Enable reorder mode for drag-and-drop section reordering
    pub reorder_mode: bool,
    /// Hidden sections (by SectionId) — sections that are not shown in the sidebar
    pub hidden_sections: Vec<SectionId>,
    /// Item reorder drag tracking — which item (section, index) is being dragged
    pub drag_item: Option<(SectionId, usize)>,
    /// Backup of section collapsed states before entering reorder mode
    pub reorder_collapsed_backup: Option<Vec<(SectionId, bool)>>,
    /// Bouncy (spring-loaded) scroll state for rubber-band overscroll
    pub bouncy_scroll: BouncyScrollState,
}

impl SidebarState {
    pub fn new() -> Self {
        let config = AppConfig::load();
        let mut state = Self::default();
        state.width = config
            .sidebar
            .sidebar_width
            .unwrap_or(UiConstants::SIDEBAR_WIDTH_RAW as f32)
            .max(UiConstants::SIDEBAR_WIDTH_MIN_RAW as f32)
            .min(UiConstants::SIDEBAR_WIDTH_MAX_RAW as f32);
        state.reload(&config);
        state
    }

    pub fn reload(&mut self, config: &AppConfig) {
        self.sections = build_all_sections();

        // Apply saved section order if available
        if let Some(order) = &config.sidebar.section_order {
            let name_to_id: std::collections::HashMap<&str, SectionId> = [
                ("Favorites", SectionId::Favorites),
                ("Tags", SectionId::Tags),
                ("Libraries", SectionId::Libraries),
                ("ThisPc", SectionId::ThisPc),
                ("CloudDrives", SectionId::CloudDrives),
                ("Network", SectionId::Network),
                ("Wsl", SectionId::Wsl),
            ]
            .iter()
            .cloned()
            .collect();

            // Build sections in the configured order
            let mut reordered: Vec<SidebarSectionData> = Vec::new();
            let mut remaining: std::collections::HashSet<SectionId> =
                self.sections.iter().map(|s| s.id).collect();

            for name in order {
                if let Some(&sid) = name_to_id.get(name.as_str()) {
                    if let Some(idx) = self.sections.iter().position(|s| s.id == sid) {
                        reordered.push(self.sections.remove(idx));
                        remaining.remove(&sid);
                    }
                }
            }

            // Add any sections not in the saved order (e.g., new sections added in an update)
            for section in self.sections.drain(..) {
                if remaining.contains(&section.id) {
                    reordered.push(section);
                }
            }

            self.sections = reordered;
        }

        // Apply saved hidden sections
        if let Some(hidden) = &config.sidebar.hidden_sections {
            let name_to_id: std::collections::HashMap<&str, SectionId> = [
                ("Favorites", SectionId::Favorites),
                ("Tags", SectionId::Tags),
                ("Libraries", SectionId::Libraries),
                ("ThisPc", SectionId::ThisPc),
                ("CloudDrives", SectionId::CloudDrives),
                ("Network", SectionId::Network),
                ("Wsl", SectionId::Wsl),
            ]
            .iter()
            .cloned()
            .collect();
            self.hidden_sections.clear();
            for name in hidden {
                if let Some(&sid) = name_to_id.get(name.as_str()) {
                    self.hidden_sections.push(sid);
                }
            }
        }

        // Apply saved collapsed state of sections
        if let Some(collapsed_map) = &config.sidebar.section_collapsed {
            for section in &mut self.sections {
                let key = format!("{:?}", section.id);
                if let Some(&collapsed) = collapsed_map.get(&key) {
                    section.collapsed = collapsed;
                }
            }
        }

        // Update display order to match current section order
        self.section_display_order = self.sections.iter().map(|s| s.id).collect();
    }

    pub fn save_width(&self) {
        let mut config = AppConfig::load();
        config.sidebar.sidebar_width = Some(self.width);
        let _ = config.save();
    }

    pub fn save_section_order(&self) {
        let mut config = AppConfig::load();
        config.sidebar.section_order = Some(
            self.section_display_order
                .iter()
                .map(|id| format!("{:?}", id))
                .collect(),
        );
        let _ = config.save();
    }

    pub fn toggle_section(&mut self, section: SectionId) {
        if let Some(s) = self.sections.iter_mut().find(|s| s.id == section) {
            s.collapsed = !s.collapsed;
            self.save_section_collapsed();
        }
    }

    /// Save the collapsed state of a section to config persistence.
    pub fn save_section_collapsed(&self) {
        let mut config = AppConfig::load();
        let mut map = std::collections::HashMap::new();
        for section in &self.sections {
            map.insert(format!("{:?}", section.id), section.collapsed);
        }
        config.sidebar.section_collapsed = Some(map);
        let _ = config.save();
    }

    pub fn set_collapsed(&mut self, collapsed: bool) {
        self.collapsed = collapsed;
    }

    pub fn toggle_collapsed(&mut self) {
        let from = self.width;
        if !self.collapsed {
            self.previous_width = self.width;
            self.collapsed = true;
            self.width = UiConstants::SIDEBAR_WIDTH_COLLAPSED_RAW as f32;
        } else {
            let restore = self
                .previous_width
                .max(UiConstants::SIDEBAR_WIDTH_MIN_RAW as f32);
            self.width = restore;
            self.collapsed = false;
        }
        self.anim_version += 1;
        self.anim_start_time = Some(std::time::Instant::now());
        self.anim_from_width = from;
        self.anim_to_width = self.width;
        self.save_width();
    }

    /// Get the current display width, interpolated during animation
    pub fn get_display_width(&self) -> f32 {
        if let Some(start) = self.anim_start_time {
            let elapsed = start.elapsed();
            let dur = std::time::Duration::from_millis(200);
            if elapsed < dur {
                let t = elapsed.as_secs_f32() / dur.as_secs_f32();
                let eased = if t < 0.5 {
                    2.0 * t * t
                } else {
                    let x = -2.0 * t + 2.0;
                    1.0 - x * x / 2.0
                };
                return self.anim_from_width + (self.anim_to_width - self.anim_from_width) * eased;
            }
        }
        self.width
    }
    /// Show/hide the section tooltip in collapsed mode
    pub fn set_tooltip(&mut self, section: Option<SectionId>) {
        if self.collapsed {
            self.tooltip_section = section;
        }
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

    /// Start dragging a section header for reordering
    pub fn start_section_drag(&mut self, section_idx: usize) {
        self.drag_section_idx = Some(section_idx);
        self.saved_order = self.section_display_order.clone();
    }

    /// Update section drag position and compute drop target
    pub fn update_section_drag(&mut self, current_y: f32) {
        self.drag_section_current_y = current_y;
        // Drop target is computed during render based on Y position
    }

    /// End section drag and apply the reorder
    pub fn end_section_drag(&mut self) {
        self.drag_section_idx = None;
        self.drag_section_current_y = 0.0;
        self.drop_target_section_idx = None;
        self.saved_order.clear();
    }

    /// Cancel section drag and restore original order
    pub fn cancel_section_drag(&mut self) {
        self.section_display_order = self.saved_order.clone();
        self.reorder_sections();
        self.end_section_drag();
    }

    /// Reorder sections based on current section_display_order
    pub fn reorder_sections(&mut self) {
        let mut reordered: Vec<SidebarSectionData> = Vec::new();
        for &sid in &self.section_display_order {
            if let Some(idx) = self.sections.iter().position(|s| s.id == sid) {
                reordered.push(self.sections.remove(idx));
            }
        }
        // Add any remaining sections not in the order list
        reordered.extend(self.sections.drain(..));
        self.sections = reordered;
    }

    /// Swap two sections in the display order
    pub fn swap_sections(&mut self, from: usize, to: usize) {
        if from < self.section_display_order.len() && to < self.section_display_order.len() {
            self.section_display_order.swap(from, to);
            self.reorder_sections();
            self.save_section_order();
        }
    }

    /// Start sidebar resize drag. If currently collapsed, un-collapse so dragging expands.
    pub fn start_resize_drag(&mut self, start_x: f32) {
        self.resize_start_x = Some(start_x);
        self.resize_start_width = Some(if self.collapsed {
            UiConstants::SIDEBAR_WIDTH_COLLAPSED_RAW as f32
        } else {
            self.width
        });
        self.last_notified_width = if self.collapsed {
            UiConstants::SIDEBAR_WIDTH_COLLAPSED_RAW as f32
        } else {
            self.width
        };
        // If collapsed, immediately exit collapsed mode so drag can resize freely
        if self.collapsed {
            self.collapsed = false;
            self.width = UiConstants::SIDEBAR_WIDTH_RAW as f32;
        }
    }

    /// Update sidebar width during resize drag (throttled: only changes when width meaningfully differs)
    /// Auto-collapses when dragged below the collapse threshold.
    pub fn update_resize_drag(&mut self, current_x: f32) -> bool {
        if let (Some(start_x), Some(start_width)) = (self.resize_start_x, self.resize_start_width) {
            let delta = current_x - start_x;
            let raw_width = start_width + delta;

            // Auto-collapse when dragged below threshold (check BEFORE clamping to min)
            let collapse_threshold = UiConstants::SIDEBAR_WIDTH_COLLAPSED_RAW as f32 + 30.0;
            if raw_width < collapse_threshold && !self.collapsed {
                self.collapsed = true;
                self.width = UiConstants::SIDEBAR_WIDTH_COLLAPSED_RAW as f32;
                self.last_notified_width = self.width;
                // Reset start width so subsequent mouse moves use collapsed reference
                self.resize_start_width = Some(self.width);
                return true;
            }

            // Auto-expand when dragged above threshold
            let expand_threshold = collapse_threshold + 10.0;
            if raw_width > expand_threshold && self.collapsed {
                self.collapsed = false;
                self.width = raw_width.min(UiConstants::SIDEBAR_WIDTH_MAX_RAW as f32);
                self.resize_start_width = Some(self.width);
            }

            // Clamp to normal range
            let new_width = raw_width
                .max(UiConstants::SIDEBAR_WIDTH_MIN_RAW as f32)
                .min(UiConstants::SIDEBAR_WIDTH_MAX_RAW as f32);

            // Throttle: only accept if width changed by at least 2px from last notified value
            if (new_width - self.last_notified_width).abs() >= 2.0 {
                self.width = new_width;
                self.last_notified_width = new_width;
                return true;
            }
        }
        false
    }

    /// End resize drag and persist width
    pub fn end_resize_drag(&mut self) {
        self.resize_start_x = None;
        self.resize_start_width = None;
        self.save_width();
    }
    /// Enter reorder mode for drag-and-drop section reordering.
    /// Enables visual indication and interacts with UI to allow reordering.
    /// Collapses all non-draggable sections, keeps Favorites and Tags expanded for item reordering.
    pub fn enter_reorder_mode(&mut self) {
        self.reorder_mode = true;
        // Backup current collapsed states so we can restore on exit
        let mut backup = Vec::new();
        for section in &self.sections {
            backup.push((section.id, section.collapsed));
        }
        self.reorder_collapsed_backup = Some(backup);
        // Collapse all sections that don't have draggable items (Favorites & Tags are draggable)
        for section in &mut self.sections {
            match section.id {
                SectionId::Favorites | SectionId::Tags => {
                    section.collapsed = false; // Keep expanded for item reordering
                }
                _ => {
                    section.collapsed = true; // Collapse non-draggable sections
                }
            }
        }
    }
    /// Exit reorder mode.
    /// Disables reordering and resets any temporary state.
    /// Restores section collapsed states from backup.
    pub fn exit_reorder_mode(&mut self) {
        self.reorder_mode = false;
        // Restore collapsed states from backup
        if let Some(backup) = self.reorder_collapsed_backup.take() {
            for (section_id, collapsed) in backup {
                if let Some(section) = self.sections.iter_mut().find(|s| s.id == section_id) {
                    section.collapsed = collapsed;
                }
            }
        }
        // Reset drag state if needed
        self.drag_section_idx = None;
        self.drag_section_current_y = 0.0;
        self.drag_item = None;
    }

    /// Hide a section (remove it from visible sidebar).
    pub fn hide_section(&mut self, section: SectionId) {
        if !self.hidden_sections.contains(&section) {
            self.hidden_sections.push(section);
            self.save_hidden_sections();
        }
    }

    /// Unhide a section (restore it to visible sidebar).
    pub fn unhide_section(&mut self, section: SectionId) {
        self.hidden_sections.retain(|s| *s != section);
        self.save_hidden_sections();
    }

    /// Save hidden sections to config.
    fn save_hidden_sections(&self) {
        let mut config = AppConfig::load();
        config.sidebar.hidden_sections = Some(
            self.hidden_sections
                .iter()
                .map(|s| format!("{:?}", s))
                .collect(),
        );
        let _ = config.save();
    }

    /// Start dragging a sidebar item (pinned location or tag) for reordering.
    pub fn start_item_drag(&mut self, section_id: SectionId, item_idx: usize) {
        self.drag_item = Some((section_id, item_idx));
    }

    /// End item drag.
    pub fn end_item_drag(&mut self) {
        self.drag_item = None;
    }

    /// Swap two items within a section during reorder.
    pub fn swap_items(&mut self, section_id: SectionId, from_idx: usize, to_idx: usize) {
        if let Some(section) = self.sections.iter_mut().find(|s| s.id == section_id) {
            if from_idx < section.items.len() && to_idx < section.items.len() {
                section.items.swap(from_idx, to_idx);
            }
        }
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

    // Add Quick Access pinned folders from Windows File Explorer
    if let Ok(quick_access_pins) = read_quick_access_pins() {
        for pin in quick_access_pins {
            let label = pin
                .as_path()
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "Pinned Folder".to_string());
            // Skip duplicates (already in pinned_locations)
            if !items.iter().any(|i| i.id.get_path().as_ref() == Some(&pin)) {
                items.push(SidebarItem {
                    id: SidebarItemId::Pinned(pin),
                    label,
                    icon: "\u{E8B7}".to_string(),
                    badge: None,
                });
            }
        }
    }

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

/// Read Quick Access pinned folders from Windows %APPDATA%\\Microsoft\\Windows\\Quick Access\\
/// Resolves .lnk shortcut targets to get the actual folder paths.
#[cfg(windows)]
fn read_quick_access_pins() -> Result<Vec<FilesPath>, Box<dyn std::error::Error>> {
    let appdata = std::env::var("APPDATA")?;
    let qa_path = std::path::PathBuf::from(&appdata)
        .join("Microsoft")
        .join("Windows")
        .join("Quick Access");

    if !qa_path.exists() {
        return Ok(Vec::new());
    }

    let mut pins = Vec::new();
    for entry in std::fs::read_dir(&qa_path)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().map_or(false, |e| e == "lnk") {
            if let Some(target) = resolve_lnk_target(&path) {
                pins.push(target);
            }
        }
    }

    Ok(pins)
}

/// Simple .lnk (Shell Link) target resolver.
/// Reads the LinkInfo structure from a .lnk binary file to extract the volume local path.
/// Format: 76-byte header, then optional structures.
/// The path is stored as a UTF16-LE string in the LinkInfo (after 0x1C offset from LinkInfo start).
#[cfg(windows)]
fn resolve_lnk_target(lnk_path: &std::path::Path) -> Option<FilesPath> {
    use std::io::Read;
    let mut file = std::fs::File::open(lnk_path).ok()?;
    let mut data = Vec::new();
    file.read_to_end(&mut data).ok()?;

    if data.len() < 76 {
        return None;
    }

    // ShellLinkHeader is 76 bytes
    // Flags at offset 0x14 (4 bytes): bit 0 = HasLinkTargetIDList, bit 1 = HasLinkInfo
    let flags = u32::from_le_bytes([data[0x14], data[0x15], data[0x16], data[0x17]]);
    let has_link_info = (flags & 0x02) != 0;

    if !has_link_info {
        return None;
    }

    // LinkTargetIDList starts at offset 0x4C (if HasLinkTargetIDList flag is set)
    let has_id_list = (flags & 0x01) != 0;
    let mut offset = 0x4C; // 76

    if has_id_list {
        // IDList size is at offset 0x4C (2 bytes)
        if offset + 2 > data.len() {
            return None;
        }
        let idlist_size = u16::from_le_bytes([data[offset], data[offset + 1]]) as usize;
        offset += 2 + idlist_size;
    }

    // LinkInfo structure
    // LinkInfoSize at current offset (4 bytes)
    if offset + 4 > data.len() {
        return None;
    }
    let link_info_size = u32::from_le_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ]) as usize;
    if link_info_size < 0x1C || offset + link_info_size > data.len() {
        return None;
    }

    // VolumeLocalPath offset within LinkInfo (at 0x1C from LinkInfo start, 4 bytes)
    let local_path_offset = u32::from_le_bytes([
        data[offset + 0x1C],
        data[offset + 0x1D],
        data[offset + 0x1E],
        data[offset + 0x1F],
    ]) as usize;

    if local_path_offset == 0 || offset + local_path_offset + 2 > data.len() {
        return None;
    }

    // The path is a null-terminated UTF16-LE string
    let path_start = offset + local_path_offset;
    let mut path_end = path_start;
    while path_end + 1 < data.len() {
        if data[path_end] == 0 && data[path_end + 1] == 0 {
            break;
        }
        path_end += 2;
    }

    let wide_chars: Vec<u16> = data[path_start..path_end]
        .chunks(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();

    let target_str = String::from_utf16_lossy(&wide_chars);
    if target_str.is_empty() {
        None
    } else {
        Some(FilesPath::new(target_str.trim_matches('\0')))
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
        let items = vec![SidebarItem {
            id: SidebarItemId::Network,
            label: "Network".to_string(),
            icon: "\u{E802}".to_string(),
            badge: None,
        }];
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
        let sections = vec![SidebarSectionData {
            id: SectionId::Favorites,
            title: "Favorites".to_string(),
            items: vec![SidebarItem {
                id: SidebarItemId::RecycleBin,
                label: "Recycle Bin".to_string(),
                icon: "\u{E74D}".to_string(),
                badge: None,
            }],
            collapsed: false,
        }];
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

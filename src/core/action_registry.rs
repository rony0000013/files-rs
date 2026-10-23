//! # Action Registry
//!
//! Central action/commands framework for files-rs, modeled on `Files-community`
//! (`CommandManager`, `HotKey`, `Keys`, `KeyModifiers`, `ActionsViewModel`).
//! Maps named `ActionKind` values to `KeyCombination` bindings (including both
//! **keyboard shortcuts** and **mouse combinations** such as `Mouse4`, `Mouse5`,
//! `MiddleClick`, `DoubleClick`, `DoubleClickBlank`, `Ctrl+WheelUp`, `Ctrl+WheelDown`),
//! and provides dispatch, conflict detection, rebinding, unbinding, and serialization.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

// ---------------------------------------------------------------------------
// KeyCombination (Keyboard + Mouse Combination)
// ---------------------------------------------------------------------------

/// A serializable representation of a keyboard shortcut or mouse combination.
///
/// Mirrors `Files-community`'s `HotKey` (`Keys` + `KeyModifiers`) and GPUI's
/// `Keystroke` / `MouseButton` / `ScrollWheelEvent`, without raw GPUI types so
/// it can be persisted cleanly in `settings.json`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct KeyCombination {
    /// Physical key or mouse trigger token:
    /// - Keyboard keys: `"a"`, `"F2"`, `"Escape"`, `"Delete"`, `"Space"`, `"Enter"`, `"Left"`, etc.
    /// - Mouse triggers: `"Mouse4"`, `"Mouse5"`, `"MiddleClick"`, `"DoubleClick"`,
    ///   `"DoubleClickBlank"`, `"LeftClick"`, `"RightClick"`, `"WheelUp"`, `"WheelDown"`
    pub key: String,
    /// Whether Ctrl is held
    pub ctrl: bool,
    /// Whether Alt is held
    pub alt: bool,
    /// Whether Shift is held
    pub shift: bool,
}

impl KeyCombination {
    pub fn new(key: &str) -> Self {
        Self {
            key: key.to_string(),
            ctrl: false,
            alt: false,
            shift: false,
        }
    }

    /// Construct a mouse combination trigger (e.g. `"Mouse4"`, `"MiddleClick"`, `"WheelUp"`).
    pub fn mouse(trigger: &str) -> Self {
        Self::new(trigger)
    }

    pub fn with_ctrl(mut self) -> Self {
        self.ctrl = true;
        self
    }

    pub fn with_alt(mut self) -> Self {
        self.alt = true;
        self
    }

    pub fn with_shift(mut self) -> Self {
        self.shift = true;
        self
    }

    pub fn with_ctrl_shift(mut self) -> Self {
        self.ctrl = true;
        self.shift = true;
        self
    }

    pub fn with_ctrl_alt(mut self) -> Self {
        self.ctrl = true;
        self.alt = true;
        self
    }

    pub fn with_alt_shift(mut self) -> Self {
        self.alt = true;
        self.shift = true;
        self
    }

    /// Returns `true` if this combination represents an unbound / empty state.
    pub fn is_empty(&self) -> bool {
        self.key.trim().is_empty()
    }

    /// Returns `true` if the trigger is a mouse button, click gesture, or scroll wheel action.
    pub fn is_mouse(&self) -> bool {
        matches!(
            self.key.to_ascii_lowercase().as_str(),
            "mouse4"
                | "mouse5"
                | "xbutton1"
                | "xbutton2"
                | "middleclick"
                | "middleclicktab"
                | "doubleclick"
                | "doubleclickblank"
                | "leftclick"
                | "rightclick"
                | "wheelup"
                | "wheeldown"
        )
    }

    /// Canonicalize key names so `"arrowleft"` and `"Left"` or `"return"` and `"Enter"` match identically.
    pub fn canonical_key_name(raw: &str) -> String {
        match raw.trim().to_ascii_lowercase().as_str() {
            "arrowleft" | "left" => "left".to_string(),
            "arrowright" | "right" => "right".to_string(),
            "arrowup" | "up" => "up".to_string(),
            "arrowdown" | "down" => "down".to_string(),
            "return" | "enter" => "enter".to_string(),
            "esc" | "escape" => "escape".to_string(),
            "back" | "backspace" => "backspace".to_string(),
            "del" | "delete" => "delete".to_string(),
            "ins" | "insert" => "insert".to_string(),
            " " | "space" | "spacebar" => "space".to_string(),
            "pgup" | "pageup" => "pageup".to_string(),
            "pgdn" | "pagedown" => "pagedown".to_string(),
            "plus" | "=" | "+" | "oemplus" | "add" => "=".to_string(),
            "minus" | "-" | "_" | "oemminus" | "subtract" => "-".to_string(),
            "comma" | "," | "oemcomma" => ",".to_string(),
            "period" | "." | "oemperiod" => ".".to_string(),
            "backquote" | "grave" | "`" | "~" | "oem3" => "`".to_string(),
            "backslash" | "\\" | "oem5" => "\\".to_string(),
            "slash" | "/" | "oem2" => "/".to_string(),
            "xbutton1" | "mouse4" | "browserback" | "goback" => "mouse4".to_string(),
            "xbutton2" | "mouse5" | "browserforward" | "goforward" => "mouse5".to_string(),
            "middleclick" | "mousemiddle" | " mouse3" | "mouse3" => "middleclick".to_string(),
            "middleclicktab" => "middleclicktab".to_string(),
            "doubleclick" | "dblclick" => "doubleclick".to_string(),
            "doubleclickblank" | "doubleclickempty" => "doubleclickblank".to_string(),
            "leftclick" | "mouse1" => "leftclick".to_string(),
            "rightclick" | "mouse2" => "rightclick".to_string(),
            "wheelup" | "scrollup" => "wheelup".to_string(),
            "wheeldown" | "scrolldown" => "wheeldown".to_string(),
            other => other.to_string(),
        }
    }

    /// Format the key or mouse token for human-friendly display in WinUI 3 `KeyboardShortcut` keycaps.
    pub fn pretty_key_label(&self) -> String {
        let canon = Self::canonical_key_name(&self.key);
        match canon.as_str() {
            "left" => "Left".to_string(),
            "right" => "Right".to_string(),
            "up" => "Up".to_string(),
            "down" => "Down".to_string(),
            "enter" => "Enter".to_string(),
            "escape" => "Esc".to_string(),
            "backspace" => "Backspace".to_string(),
            "delete" => "Delete".to_string(),
            "insert" => "Insert".to_string(),
            "space" => "Space".to_string(),
            "tab" => "Tab".to_string(),
            "home" => "Home".to_string(),
            "end" => "End".to_string(),
            "pageup" => "PageUp".to_string(),
            "pagedown" => "PageDown".to_string(),
            "=" => "Plus (+)".to_string(),
            "-" => "Minus (-)".to_string(),
            "," => "Comma (,)".to_string(),
            "." => "Period (.)".to_string(),
            "`" => "`".to_string(),
            "\\" => "\\".to_string(),
            "mouse4" => "Mouse 4 (Back)".to_string(),
            "mouse5" => "Mouse 5 (Forward)".to_string(),
            "middleclick" => "Middle Click".to_string(),
            "middleclicktab" => "Middle Click Tab".to_string(),
            "doubleclick" => "Double Click".to_string(),
            "doubleclickblank" => "Double Click Empty Space".to_string(),
            "leftclick" => "Left Click".to_string(),
            "rightclick" => "Right Click".to_string(),
            "wheelup" => "Wheel Up".to_string(),
            "wheeldown" => "Wheel Down".to_string(),
            other if other.len() == 1 => other.to_ascii_uppercase(),
            other if other.starts_with('f') && other[1..].chars().all(|c| c.is_ascii_digit()) => {
                other.to_ascii_uppercase()
            }
            _ => self.key.clone(),
        }
    }

    /// Returns individual badge tokens for rendering WinUI 3 `KeyboardShortcut` keycap pills
    /// (e.g., `["Ctrl", "Shift", "N"]` or `["Ctrl", "Wheel Up"]`).
    pub fn display_tokens(&self) -> Vec<String> {
        if self.is_empty() {
            return Vec::new();
        }
        let mut parts = Vec::with_capacity(4);
        if self.ctrl {
            parts.push("Ctrl".to_string());
        }
        if self.alt {
            parts.push("Alt".to_string());
        }
        if self.shift {
            parts.push("Shift".to_string());
        }
        parts.push(self.pretty_key_label());
        parts
    }

    /// Case-insensitive semantic equality between two combinations.
    pub fn matches(&self, other: &KeyCombination) -> bool {
        if self.is_empty() || other.is_empty() {
            return false;
        }
        self.ctrl == other.ctrl
            && self.alt == other.alt
            && self.shift == other.shift
            && Self::canonical_key_name(&self.key) == Self::canonical_key_name(&other.key)
    }

    /// Parse a humanized string like `"Ctrl+Shift+N"`, `"Alt+Left"`, `"Mouse4"`, `"Ctrl+WheelUp"`.
    pub fn parse(input: &str) -> Option<Self> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return None;
        }
        let mut ctrl = false;
        let mut alt = false;
        let mut shift = false;
        let mut main_key: Option<String> = None;

        for raw_part in trimmed.split('+') {
            let p = raw_part.trim();
            if p.is_empty() {
                main_key = Some("+".to_string());
                continue;
            }
            match p.to_ascii_lowercase().as_str() {
                "ctrl" | "control" | "cmd" => ctrl = true,
                "alt" | "menu" | "option" => alt = true,
                "shift" => shift = true,
                _ => {
                    let canon = Self::canonical_key_name(p);
                    // Preserve uppercase F-keys like "F2", "F5", "Escape", "Delete" for backward compatibility
                    let normalized = if canon.starts_with('f')
                        && canon.len() <= 3
                        && canon[1..].chars().all(|c| c.is_ascii_digit())
                    {
                        canon.to_ascii_uppercase()
                    } else {
                        match canon.as_str() {
                            "escape" => "Escape".to_string(),
                            "delete" => "Delete".to_string(),
                            "mouse4" => "Mouse4".to_string(),
                            "mouse5" => "Mouse5".to_string(),
                            "middleclick" => "MiddleClick".to_string(),
                            "middleclicktab" => "MiddleClickTab".to_string(),
                            "doubleclick" => "DoubleClick".to_string(),
                            "doubleclickblank" => "DoubleClickBlank".to_string(),
                            "leftclick" => "LeftClick".to_string(),
                            "rightclick" => "RightClick".to_string(),
                            "wheelup" => "WheelUp".to_string(),
                            "wheeldown" => "WheelDown".to_string(),
                            _ => canon,
                        }
                    };
                    main_key = Some(normalized);
                }
            }
        }

        main_key.map(|key| Self {
            key,
            ctrl,
            alt,
            shift,
        })
    }
}

impl fmt::Display for KeyCombination {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_empty() {
            return write!(f, "Unbound");
        }
        let mut parts = Vec::new();
        if self.ctrl {
            parts.push("Ctrl");
        }
        if self.alt {
            parts.push("Alt");
        }
        if self.shift {
            parts.push("Shift");
        }
        parts.push(&self.key);
        write!(f, "{}", parts.join("+"))
    }
}

// ---------------------------------------------------------------------------
// ActionCategory
// ---------------------------------------------------------------------------

/// High-level action category for filtering in Settings -> Actions and Command Palette.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ActionCategory {
    Navigation,
    FileSystem,
    Selection,
    LayoutAndView,
    TabsAndWindows,
    SplitPanes,
    MouseGestures,
    SystemAndTools,
}

impl ActionCategory {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Navigation => "Navigation",
            Self::FileSystem => "Files & Editing",
            Self::Selection => "Selection",
            Self::LayoutAndView => "Layout & View",
            Self::TabsAndWindows => "Tabs & Windows",
            Self::SplitPanes => "Dual Pane",
            Self::MouseGestures => "Mouse Combinations",
            Self::SystemAndTools => "System & Tools",
        }
    }
}

// ---------------------------------------------------------------------------
// ActionKind
// ---------------------------------------------------------------------------

/// Every built-in action in `files-rs`, matching `Files-community` (`CommandCodes`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ActionKind {
    // Navigation
    NavigateBack,
    NavigateForward,
    NavigateUp,
    NavigateToRoot,
    NavigateHome,

    // Selection & Editing
    SelectAll,
    InvertSelection,
    Copy,
    Cut,
    Paste,
    PasteAsShortcut,
    /// Copy the selected item's full path as text (Files parity: Ctrl+Shift+C)
    CopyItemPath,
    /// Copy the selected item's path with quotes (Files parity: Ctrl+Alt+C)
    CopyItemPathWithQuotes,
    NewFolder,
    NewFile,
    CreateFolderWithSelection,
    Rename,
    BulkRename,
    Delete,
    PermanentDelete,
    ClearSelection,
    OpenProperties,
    OpenItem,
    OpenInNewTab,
    OpenInNewWindow,
    ExtractArchive,
    ExtractArchiveHere,
    CreateArchive,
    EmptyRecycleBin,
    RestoreRecycleBin,
    RestoreAllRecycleBin,
    Undo,
    Redo,

    // View & Layout
    Refresh,
    Search,
    ClearFilter,
    ToggleHiddenFiles,
    ToggleDotFiles,
    ToggleFileExtensions,
    ToggleCheckboxes,
    ToggleSidebar,
    ToggleToolbar,
    ToggleDetailsPane,
    ToggleInfoPane,
    ToggleShelfPane,
    ToggleCompactOverlay,
    ToggleFullScreen,
    EditPath,
    OpenPreviewPane,
    ViewDetails,
    ViewList,
    ViewGrid,
    ViewColumns,
    CycleViewMode,
    CycleSortColumn,
    CycleGroupBy,
    ZoomIn,
    ZoomOut,
    ZoomReset,

    // Tabs & Windows
    NewTab,
    CloseTab,
    CloseOtherTabs,
    CloseAllTabs,
    ReopenClosedTab,
    DuplicateTab,
    PinTab,
    MoveTabLeft,
    MoveTabRight,
    NewWindow,
    NextTab,
    PreviousTab,

    // Split Panes
    ToggleDualPane,
    SplitVertical,
    SplitHorizontal,
    SwitchPaneFocus,

    // Mouse Combinations (explicitly customizable in Settings -> Actions)
    MouseNavigateBack,
    MouseNavigateForward,
    MiddleClickOpenNewTab,
    MiddleClickCloseTab,
    DoubleClickBlankGoUp,
    MouseWheelZoomIn,
    MouseWheelZoomOut,

    // Commands & System
    CommandPalette,
    OpenTerminal,
    OpenTerminalAsAdmin,
    OpenInIDE,
    /// Open the help/documentation URL
    OpenHelp,
    /// Open the settings page as a tab
    OpenSettings,
    /// Restore the last session after a crash
    RestoreLastSession,

    // Cross-window, layout & file-action extensions (Files-community parity)
    CloseActivePane,
    CloseTabsToLeft,
    CloseTabsToRight,
    CompressTo7z,
    CompressToTar,
    CompressToZip,
    CreateShortcut,
    DualHorizontal,
    DualVertical,
    DuplicateItem,
    ExtractHere,
    ExtractToFolder,
    FilterItems,
    FindInPage,
    FocusPathBar,
    GroupByNone,
    MoveTabToNewWindow,
    OpenInIde,
    OpenInNewPane,
    OpenSelectedItem,
    OpenStorageSense,
    PinToStart,
    UnpinFromStart,
    RotateImageLeft,
    RotateImageRight,
    RunAsAdmin,
    SetAsLockScreen,
    SetAsWallpaper,
    ShareItem,
    SortByDateModified,
    SortByName,
    SortBySize,
    SortByType,
    SwapPanes,
    ToggleItemSelection,
    TogglePreviewPane,
    ToggleSortDirection,
}

impl ActionKind {
    /// Human-readable title shown in the Settings -> Actions UI, Command Palette, and tooltips.
    pub fn label(&self) -> &'static str {
        match self {
            Self::NavigateBack => "Navigate Back",
            Self::NavigateForward => "Navigate Forward",
            Self::NavigateUp => "Navigate Up",
            Self::NavigateToRoot => "Navigate to Root",
            Self::NavigateHome => "Navigate Home",
            Self::SelectAll => "Select All",
            Self::InvertSelection => "Invert Selection",
            Self::Copy => "Copy",
            Self::Cut => "Cut",
            Self::Paste => "Paste",
            Self::PasteAsShortcut => "Paste Shortcut",
            Self::CopyItemPath => "Copy Item Path",
            Self::CopyItemPathWithQuotes => "Copy Path with Quotes",
            Self::NewFolder => "New Folder",
            Self::NewFile => "New File",
            Self::CreateFolderWithSelection => "Create Folder with Selection",
            Self::Rename => "Rename",
            Self::BulkRename => "Bulk Rename",
            Self::Delete => "Move to Trash",
            Self::PermanentDelete => "Permanent Delete",
            Self::ClearSelection => "Clear Selection",
            Self::OpenProperties => "Properties",
            Self::OpenItem => "Open Selected Item",
            Self::OpenInNewTab => "Open Folder in New Tab",
            Self::OpenInNewWindow => "Open Folder in New Window",
            Self::ExtractArchive => "Extract Archive",
            Self::ExtractArchiveHere => "Extract Archive Here",
            Self::CreateArchive => "Compress to Archive",
            Self::EmptyRecycleBin => "Empty Recycle Bin",
            Self::RestoreRecycleBin => "Restore Recycle Bin Item",
            Self::RestoreAllRecycleBin => "Restore All Recycle Bin Items",
            Self::Undo => "Undo",
            Self::Redo => "Redo",
            Self::Refresh => "Refresh",
            Self::Search => "Search",
            Self::ClearFilter => "Clear Filter / Escape",
            Self::ToggleHiddenFiles => "Toggle Hidden Files",
            Self::ToggleDotFiles => "Toggle Dot Files",
            Self::ToggleFileExtensions => "Toggle File Extensions",
            Self::ToggleCheckboxes => "Toggle Selection Checkboxes",
            Self::ToggleSidebar => "Toggle Sidebar",
            Self::ToggleToolbar => "Toggle Top Ribbon Toolbar",
            Self::ToggleDetailsPane => "Toggle Details Pane",
            Self::ToggleInfoPane => "Toggle Info / Preview Pane",
            Self::ToggleShelfPane => "Toggle Shelf Pane",
            Self::ToggleCompactOverlay => "Toggle Compact Overlay",
            Self::ToggleFullScreen => "Toggle Full Screen",
            Self::EditPath => "Edit Path",
            Self::OpenPreviewPane => "Open Preview Pane",
            Self::ViewDetails => "Details View",
            Self::ViewList => "List View",
            Self::ViewGrid => "Grid View",
            Self::ViewColumns => "Columns View",
            Self::CycleViewMode => "Cycle Layout Mode",
            Self::CycleSortColumn => "Cycle Sort Column",
            Self::CycleGroupBy => "Cycle Group By",
            Self::ZoomIn => "Zoom In",
            Self::ZoomOut => "Zoom Out",
            Self::ZoomReset => "Reset Zoom",
            Self::NewTab => "New Tab",
            Self::CloseTab => "Close Tab",
            Self::CloseOtherTabs => "Close Other Tabs",
            Self::CloseAllTabs => "Close All Tabs",
            Self::ReopenClosedTab => "Reopen Closed Tab",
            Self::DuplicateTab => "Duplicate Tab",
            Self::PinTab => "Pin / Unpin Tab",
            Self::MoveTabLeft => "Move Tab Left",
            Self::MoveTabRight => "Move Tab Right",
            Self::NewWindow => "New Window",
            Self::NextTab => "Next Tab",
            Self::PreviousTab => "Previous Tab",
            Self::ToggleDualPane => "Toggle Dual Pane",
            Self::SplitVertical => "Split Vertical",
            Self::SplitHorizontal => "Split Horizontal",
            Self::SwitchPaneFocus => "Switch Active Pane",
            Self::MouseNavigateBack => "Navigate Back (Mouse 4)",
            Self::MouseNavigateForward => "Navigate Forward (Mouse 5)",
            Self::MiddleClickOpenNewTab => "Open Folder in New Tab (Middle-Click)",
            Self::MiddleClickCloseTab => "Close Tab (Middle-Click)",
            Self::DoubleClickBlankGoUp => "Navigate Up (Double-Click Blank Space)",
            Self::MouseWheelZoomIn => "Zoom In (Ctrl + Wheel Up)",
            Self::MouseWheelZoomOut => "Zoom Out (Ctrl + Wheel Down)",
            Self::CommandPalette => "Command Palette",
            Self::OpenTerminal => "Open in Terminal",
            Self::OpenTerminalAsAdmin => "Open Terminal as Admin",
            Self::OpenInIDE => "Open in IDE",
            Self::OpenHelp => "Open Help",
            Self::OpenSettings => "Open Settings",
            Self::RestoreLastSession => "Restore Last Session",
            Self::CloseActivePane => "Close Active Pane",
            Self::CloseTabsToLeft => "Close Tabs to the Left",
            Self::CloseTabsToRight => "Close Tabs to the Right",
            Self::CompressTo7z => "Compress to 7Z Archive",
            Self::CompressToTar => "Compress to TAR Archive",
            Self::CompressToZip => "Compress to ZIP Archive",
            Self::CreateShortcut => "Create Shortcut",
            Self::DualHorizontal => "Dual Pane (Horizontal Split)",
            Self::DualVertical => "Dual Pane (Vertical Split)",
            Self::DuplicateItem => "Duplicate Selected Item",
            Self::ExtractHere => "Extract Archive Here",
            Self::ExtractToFolder => "Extract Archive to Folder",
            Self::FilterItems => "Filter Items",
            Self::FindInPage => "Find in Page",
            Self::FocusPathBar => "Focus Path Bar",
            Self::GroupByNone => "No Grouping",
            Self::MoveTabToNewWindow => "Move Tab to New Window",
            Self::OpenInIde => "Open in IDE",
            Self::OpenInNewPane => "Open in New Pane",
            Self::OpenSelectedItem => "Open Selected Item",
            Self::OpenStorageSense => "Open Storage Sense",
            Self::PinToStart => "Pin to Start",
            Self::UnpinFromStart => "Unpin from Start",
            Self::RotateImageLeft => "Rotate Image Left",
            Self::RotateImageRight => "Rotate Image Right",
            Self::RunAsAdmin => "Run as Administrator",
            Self::SetAsLockScreen => "Set as Lock Screen",
            Self::SetAsWallpaper => "Set as Wallpaper",
            Self::ShareItem => "Share Item",
            Self::SortByDateModified => "Sort by Date Modified",
            Self::SortByName => "Sort by Name",
            Self::SortBySize => "Sort by Size",
            Self::SortByType => "Sort by Type",
            Self::SwapPanes => "Swap Panes",
            Self::ToggleItemSelection => "Toggle Item Selection",
            Self::TogglePreviewPane => "Toggle Preview Pane",
            Self::ToggleSortDirection => "Toggle Sort Direction",
        }
    }

    /// Detailed description matching `Files-community` `ActionsPage.xaml` row subtitle.
    pub fn description(&self) -> &'static str {
        match self {
            Self::NavigateBack => "Navigate to the previous folder in history",
            Self::NavigateForward => "Navigate to the next folder in history",
            Self::NavigateUp => "Navigate up one directory level to the parent folder",
            Self::NavigateToRoot => "Navigate to the root of the current drive",
            Self::NavigateHome => "Navigate to the Home page with widgets and quick access",
            Self::SelectAll => "Select all items in the active file pane",
            Self::InvertSelection => "Invert the current selection in the active file pane",
            Self::Copy => "Copy selected files and folders to the clipboard",
            Self::Cut => "Cut selected files and folders to move them",
            Self::Paste => "Paste clipboard items into the current directory",
            Self::PasteAsShortcut => "Create shortcuts (.lnk) for clipboard items in current folder",
            Self::CopyItemPath => "Copy full filesystem path of selected item to clipboard",
            Self::CopyItemPathWithQuotes => "Copy full filesystem path enclosed in double quotes",
            Self::NewFolder => "Create a new folder in the active directory",
            Self::NewFile => "Create a new empty file in the active directory",
            Self::CreateFolderWithSelection => "Create a new folder and move selected items into it",
            Self::Rename => "Rename the selected item inline",
            Self::BulkRename => "Open the batch regex/pattern rename dialog for selected items",
            Self::Delete => "Move selected items to the Recycle Bin",
            Self::PermanentDelete => "Permanently delete selected items without Recycle Bin",
            Self::ClearSelection => "Clear current selection, filter, or active overlay",
            Self::OpenProperties => "Open the Properties window for the selected item",
            Self::OpenItem => "Open selected file in default app or enter folder",
            Self::OpenInNewTab => "Open the selected folder in a new tab",
            Self::OpenInNewWindow => "Open the selected folder in a new window",
            Self::ExtractArchive => "Open the extract archive dialog for the selected archive",
            Self::ExtractArchiveHere => "Extract the selected archive directly into current folder",
            Self::CreateArchive => "Compress selected items into a ZIP or 7z archive",
            Self::EmptyRecycleBin => "Permanently remove all items from the Recycle Bin",
            Self::RestoreRecycleBin => "Restore selected Recycle Bin items to original path",
            Self::RestoreAllRecycleBin => "Restore all items in the Recycle Bin to original paths",
            Self::Undo => "Undo the last file operation (rename, create folder, etc.)",
            Self::Redo => "Redo the previously undone file operation",
            Self::Refresh => "Reload items and metadata for the current directory",
            Self::Search => "Focus the Omnibar in Search mode",
            Self::ClearFilter => "Clear active search or filter query",
            Self::ToggleHiddenFiles => "Show or hide hidden files and folders",
            Self::ToggleDotFiles => "Show or hide dot-prefixed files and folders",
            Self::ToggleFileExtensions => "Show or hide file name extensions",
            Self::ToggleCheckboxes => "Show or hide selection checkboxes next to items",
            Self::ToggleSidebar => "Collapse or expand the left navigation sidebar",
            Self::ToggleToolbar => "Show or hide the top command bar ribbon above the file view",
            Self::ToggleDetailsPane => "Open or close the Details tab in the right InfoPane",
            Self::ToggleInfoPane => "Show or hide the right-side Details & Preview InfoPane",
            Self::ToggleShelfPane => "Show or hide the bottom Shelf staging area",
            Self::ToggleCompactOverlay => "Toggle compact Picture-in-Picture window mode",
            Self::ToggleFullScreen => "Toggle full-screen window mode",
            Self::EditPath => "Focus the Omnibar address bar to edit or type a path",
            Self::OpenPreviewPane => "Open the Preview tab in InfoPane or trigger QuickLook/Peek",
            Self::ViewDetails => "Switch layout to Details table view",
            Self::ViewList => "Switch layout to multi-column List view",
            Self::ViewGrid => "Switch layout to Grid thumbnail/cards view",
            Self::ViewColumns => "Switch layout to Miller Columns hierarchy view",
            Self::CycleViewMode => "Cycle through Details, List, Grid, and Columns layouts",
            Self::CycleSortColumn => "Cycle sort column between Name, Date, Size, and Type",
            Self::CycleGroupBy => "Cycle item grouping mode in the active pane",
            Self::ZoomIn => "Increase item and icon size in the current layout",
            Self::ZoomOut => "Decrease item and icon size in the current layout",
            Self::ZoomReset => "Reset item and icon size to default",
            Self::NewTab => "Open a new tab",
            Self::CloseTab => "Close the currently active tab",
            Self::CloseOtherTabs => "Close all unpinned tabs except the active tab",
            Self::CloseAllTabs => "Close all unpinned tabs",
            Self::ReopenClosedTab => "Reopen the most recently closed tab",
            Self::DuplicateTab => "Duplicate the active tab immediately to its right",
            Self::PinTab => "Pin or unpin the active tab",
            Self::MoveTabLeft => "Move the active tab one slot to the left",
            Self::MoveTabRight => "Move the active tab one slot to the right",
            Self::NewWindow => "Open a new Files-Rust window",
            Self::NextTab => "Switch to the next tab in the tab bar",
            Self::PreviousTab => "Switch to the previous tab in the tab bar",
            Self::ToggleDualPane => "Toggle Dual-Pane split view on or off",
            Self::SplitVertical => "Split the active tab into side-by-side dual panes",
            Self::SplitHorizontal => "Split the active tab into top-and-bottom dual panes",
            Self::SwitchPaneFocus => "Switch keyboard and toolbar focus between split panes",
            Self::MouseNavigateBack => "Use Mouse Button 4 (XButton1) to navigate back",
            Self::MouseNavigateForward => "Use Mouse Button 5 (XButton2) to navigate forward",
            Self::MiddleClickOpenNewTab => "Middle-click a folder or breadcrumb to open in new tab",
            Self::MiddleClickCloseTab => "Middle-click a tab in the tab strip to close it",
            Self::DoubleClickBlankGoUp => "Double-click empty space in file list to navigate up",
            Self::MouseWheelZoomIn => "Hold Ctrl and scroll mouse wheel up to zoom in",
            Self::MouseWheelZoomOut => "Hold Ctrl and scroll mouse wheel down to zoom out",
            Self::CommandPalette => "Open the interactive Command Palette in the Omnibar",
            Self::OpenTerminal => "Open the configured terminal application in current folder",
            Self::OpenTerminalAsAdmin => "Open an elevated Administrator terminal in current folder",
            Self::OpenInIDE => "Open the current folder or Git repository in your IDE",
            Self::OpenHelp => "Open documentation and help in the default browser",
            Self::OpenSettings => "Open the Settings page in a tab",
            Self::RestoreLastSession => "Restore tabs and panes from the previous session",
            Self::CloseActivePane => "Close the active pane and return to single-pane view",
            Self::CloseTabsToLeft => "Close all tabs to the left of the active tab",
            Self::CloseTabsToRight => "Close all tabs to the right of the active tab",
            Self::CompressTo7z => "Compress the selection into a 7Z archive",
            Self::CompressToTar => "Compress the selection into a TAR archive",
            Self::CompressToZip => "Compress the selection into a ZIP archive",
            Self::CreateShortcut => "Create a .lnk shortcut to the selection here",
            Self::DualHorizontal => "Split panes side by side (horizontal dual pane)",
            Self::DualVertical => "Split panes stacked (vertical dual pane)",
            Self::DuplicateItem => "Duplicate the selected item in place",
            Self::ExtractHere => "Extract the selected archive into the current folder",
            Self::ExtractToFolder => "Extract the selected archive into a new subfolder",
            Self::FilterItems => "Toggle the inline file filter box",
            Self::FindInPage => "Find text in the current view",
            Self::FocusPathBar => "Focus the address bar for path editing",
            Self::GroupByNone => "Disable item grouping",
            Self::MoveTabToNewWindow => "Detach the active tab into a new window",
            Self::OpenInIde => "Open the current folder in the configured IDE",
            Self::OpenInNewPane => "Open the selected folder in the other pane",
            Self::OpenSelectedItem => "Open the selected item",
            Self::OpenStorageSense => "Open Windows Storage Sense settings",
            Self::PinToStart => "Pin the selected item to Start",
            Self::UnpinFromStart => "Unpin the selected item from Start",
            Self::RotateImageLeft => "Rotate the selected image 90 degrees left",
            Self::RotateImageRight => "Rotate the selected image 90 degrees right",
            Self::RunAsAdmin => "Run the selected item as administrator",
            Self::SetAsLockScreen => "Set the selected image as the lock screen",
            Self::SetAsWallpaper => "Set the selected image as the desktop wallpaper",
            Self::ShareItem => "Open the Windows share dialog for the selection",
            Self::SortByDateModified => "Sort items by date modified",
            Self::SortByName => "Sort items by name",
            Self::SortBySize => "Sort items by size",
            Self::SortByType => "Sort items by type",
            Self::SwapPanes => "Swap the primary and secondary pane contents",
            Self::ToggleItemSelection => "Toggle selection of the focused item",
            Self::TogglePreviewPane => "Show or hide the preview pane",
            Self::ToggleSortDirection => "Toggle ascending / descending sort order",
        }
    }

    /// Category for grouping and filtering in Settings -> Actions.
    pub fn category(&self) -> ActionCategory {
        match self {
            Self::NavigateBack
            | Self::NavigateForward
            | Self::NavigateUp
            | Self::NavigateToRoot
            | Self::NavigateHome
            | Self::EditPath
            | Self::FocusPathBar
            | Self::OpenSelectedItem => ActionCategory::Navigation,

            Self::Copy
            | Self::Cut
            | Self::Paste
            | Self::PasteAsShortcut
            | Self::CopyItemPath
            | Self::CopyItemPathWithQuotes
            | Self::NewFolder
            | Self::NewFile
            | Self::CreateFolderWithSelection
            | Self::Rename
            | Self::BulkRename
            | Self::Delete
            | Self::PermanentDelete
            | Self::OpenProperties
            | Self::OpenItem
            | Self::OpenInNewTab
            | Self::OpenInNewWindow
            | Self::ExtractArchive
            | Self::ExtractArchiveHere
            | Self::CompressToZip
            | Self::CompressTo7z
            | Self::CompressToTar
            | Self::CreateArchive
            | Self::CreateShortcut
            | Self::DuplicateItem
            | Self::ExtractHere
            | Self::ExtractToFolder
            | Self::ShareItem
            | Self::PinToStart
            | Self::UnpinFromStart
            | Self::SetAsWallpaper
            | Self::SetAsLockScreen
            | Self::RotateImageLeft
            | Self::RotateImageRight
            | Self::EmptyRecycleBin
            | Self::RestoreRecycleBin
            | Self::RestoreAllRecycleBin
            | Self::Undo
            | Self::Redo => ActionCategory::FileSystem,

            Self::SelectAll
            | Self::InvertSelection
            | Self::ClearSelection
            | Self::ToggleCheckboxes
            | Self::ToggleItemSelection => ActionCategory::Selection,

            Self::Refresh
            | Self::Search
            | Self::ClearFilter
            | Self::ToggleHiddenFiles
            | Self::ToggleDotFiles
            | Self::ToggleFileExtensions
            | Self::ToggleSidebar
            | Self::ToggleToolbar
            | Self::ToggleDetailsPane
            | Self::ToggleInfoPane
            | Self::ToggleShelfPane
            | Self::ToggleCompactOverlay
            | Self::ToggleFullScreen
            | Self::OpenPreviewPane
            | Self::ViewDetails
            | Self::ViewList
            | Self::ViewGrid
            | Self::ViewColumns
            | Self::CycleViewMode
            | Self::CycleSortColumn
            | Self::CycleGroupBy
            | Self::SortByName
            | Self::SortByDateModified
            | Self::SortBySize
            | Self::SortByType
            | Self::ToggleSortDirection
            | Self::GroupByNone
            | Self::FilterItems
            | Self::FindInPage
            | Self::TogglePreviewPane
            | Self::ZoomIn
            | Self::ZoomOut
            | Self::ZoomReset => ActionCategory::LayoutAndView,

            Self::NewTab
            | Self::CloseTab
            | Self::CloseOtherTabs
            | Self::CloseTabsToLeft
            | Self::CloseTabsToRight
            | Self::CloseAllTabs
            | Self::ReopenClosedTab
            | Self::DuplicateTab
            | Self::PinTab
            | Self::MoveTabLeft
            | Self::MoveTabRight
            | Self::NewWindow
            | Self::NextTab
            | Self::PreviousTab => ActionCategory::TabsAndWindows,

            Self::ToggleDualPane
            | Self::SplitVertical
            | Self::SplitHorizontal
            | Self::DualHorizontal
            | Self::DualVertical
            | Self::SwapPanes
            | Self::CloseActivePane
            | Self::OpenInNewPane
            | Self::MoveTabToNewWindow
            | Self::SwitchPaneFocus => ActionCategory::SplitPanes,

            Self::MouseNavigateBack
            | Self::MouseNavigateForward
            | Self::MiddleClickOpenNewTab
            | Self::MiddleClickCloseTab
            | Self::DoubleClickBlankGoUp
            | Self::MouseWheelZoomIn
            | Self::MouseWheelZoomOut => ActionCategory::MouseGestures,

            Self::CommandPalette
            | Self::OpenTerminal
            | Self::OpenTerminalAsAdmin
            | Self::OpenInIDE
            | Self::OpenHelp
            | Self::OpenSettings
            | Self::RestoreLastSession
            | Self::RunAsAdmin
            | Self::OpenStorageSense
            | Self::OpenInIde => ActionCategory::SystemAndTools,
        }
    }

    /// Fluent icon glyph for this action.
    pub fn icon(&self) -> &'static str {
        match self {
            Self::NavigateBack | Self::MouseNavigateBack => "\u{E72B}",
            Self::NavigateForward | Self::MouseNavigateForward => "\u{E72A}",
            Self::NavigateUp | Self::NavigateToRoot | Self::DoubleClickBlankGoUp => "\u{E74A}",
            Self::NavigateHome => "\u{E80F}",
            Self::EditPath => "\u{E8B7}",
            Self::NewTab | Self::DuplicateTab | Self::OpenInNewTab | Self::MiddleClickOpenNewTab => "\u{E710}",
            Self::CloseTab | Self::CloseOtherTabs | Self::CloseAllTabs | Self::MiddleClickCloseTab => "\u{E8BB}",
            Self::ReopenClosedTab | Self::RestoreLastSession => "\u{E81C}",
        Self::CloseTabsToLeft | Self::CloseTabsToRight => "\u{E8AB}",
        Self::CloseActivePane => "\u{E8BB}",
        Self::CompressToZip | Self::CompressTo7z | Self::CompressToTar => "\u{E8F4}",
        Self::CreateShortcut => "\u{E8C8}",
        Self::DuplicateItem | Self::Copy => "\u{E8C8}",
        Self::ExtractHere | Self::ExtractToFolder => "\u{E8F4}",
        Self::FilterItems => "\u{E71C}",
        Self::FindInPage => "\u{E721}",
        Self::FocusPathBar => "\u{E8B7}",
        Self::GroupByNone => "\u{E8FD}",
        Self::MoveTabToNewWindow => "\u{E8A7}",
        Self::OpenInIde => "\u{E943}",
        Self::OpenInNewPane => "\u{E740}",
        Self::OpenSelectedItem => "\u{E8B7}",
        Self::OpenStorageSense => "\u{E9B9}",
        Self::PinToStart | Self::UnpinFromStart => "\u{E718}",
        Self::RotateImageLeft | Self::RotateImageRight => "\u{E7AD}",
        Self::RunAsAdmin => "\u{E7EF}",
        Self::SetAsWallpaper | Self::SetAsLockScreen => "\u{E91B}",
        Self::ShareItem => "\u{E72D}",
        Self::SortByName | Self::SortByDateModified | Self::SortBySize | Self::SortByType => "\u{E8CB}",
        Self::SwapPanes => "\u{E8AB}",
        Self::ToggleItemSelection => "\u{E73E}",
        Self::TogglePreviewPane => "\u{E8F1}",
        Self::ToggleSortDirection => "\u{E8CB}",
            Self::PinTab => "\u{E718}",
            Self::NextTab | Self::PreviousTab | Self::MoveTabLeft | Self::MoveTabRight => "\u{E8AB}",
            Self::NewWindow | Self::OpenInNewWindow => "\u{E8A7}",
            Self::ToggleDualPane
            | Self::SplitVertical
            | Self::SplitHorizontal
            | Self::SwitchPaneFocus
            | Self::DualHorizontal
            | Self::DualVertical => "😀",
            Self::Copy | Self::CopyItemPath | Self::CopyItemPathWithQuotes => "\u{E8C8}",
            Self::Cut => "\u{E8C6}",
            Self::Paste | Self::PasteAsShortcut => "\u{E77F}",
            Self::Rename | Self::BulkRename => "\u{E8AC}",
            Self::NewFolder | Self::CreateFolderWithSelection => "\u{E8F4}",
            Self::NewFile => "\u{E7C3}",
            Self::Delete | Self::PermanentDelete | Self::EmptyRecycleBin => "\u{E74D}",
            Self::RestoreRecycleBin | Self::RestoreAllRecycleBin => "\u{E777}",
            Self::ExtractArchive | Self::ExtractArchiveHere | Self::CreateArchive => "\u{E7B8}",
            Self::Undo => "\u{E7A7}",
            Self::Redo => "\u{E7A6}",
            Self::OpenProperties => "\u{E946}",
            Self::OpenItem => "\u{E8E5}",
            Self::SelectAll | Self::InvertSelection | Self::ClearSelection | Self::ToggleCheckboxes => "\u{E762}",
            Self::ViewDetails | Self::ViewList | Self::ViewGrid | Self::ViewColumns | Self::CycleViewMode => "\u{E8FD}",
            Self::CycleSortColumn => "\u{E8CB}",
            Self::CycleGroupBy => "\u{F168}",
            Self::ZoomIn | Self::MouseWheelZoomIn => "\u{E8A3}",
            Self::ZoomOut | Self::MouseWheelZoomOut => "\u{E71F}",
            Self::ZoomReset => "\u{E72C}",
            Self::ToggleHiddenFiles | Self::ToggleDotFiles | Self::ToggleFileExtensions => "\u{E7B3}",
            Self::ToggleSidebar => "\u{E700}",
            Self::ToggleToolbar => "\u{E712}",
            Self::ToggleDetailsPane | Self::ToggleInfoPane | Self::OpenPreviewPane => "\u{E890}",
            Self::ToggleShelfPane => "\u{E8A0}",
            Self::ToggleCompactOverlay | Self::ToggleFullScreen => "\u{E740}",
            Self::Refresh => "\u{E72C}",
            Self::Search | Self::ClearFilter => "\u{E721}",
            Self::OpenTerminal | Self::OpenTerminalAsAdmin => "\u{E756}",
            Self::OpenInIDE => "\u{E943}",
            Self::OpenSettings => "\u{E713}",
            Self::OpenHelp => "\u{E897}",
            Self::CommandPalette => "\u{E943}",
        }
    }

    /// Returns `true` if the action is conceptually a toggle (it has an on/off checked state).
    pub fn is_toggle(&self) -> bool {
        matches!(
            self,
            Self::ToggleHiddenFiles
                | Self::ToggleDotFiles
                | Self::ToggleFileExtensions
                | Self::ToggleCheckboxes
                | Self::ToggleSidebar
                | Self::ToggleToolbar
                | Self::ToggleDetailsPane
                | Self::ToggleInfoPane
                | Self::ToggleShelfPane
                | Self::ToggleDualPane
                | Self::ToggleCompactOverlay
                | Self::ToggleFullScreen
                | Self::PinTab
        )
    }

    /// Return all action variants (for iterating in Settings -> Actions and Command Palette).
    pub fn all() -> Vec<ActionKind> {
        use ActionKind::*;
        vec![
            NavigateBack,
            NavigateForward,
            NavigateUp,
            NavigateToRoot,
            NavigateHome,
            SelectAll,
            InvertSelection,
            Copy,
            Cut,
            Paste,
            PasteAsShortcut,
            CopyItemPath,
            CopyItemPathWithQuotes,
            NewFolder,
            NewFile,
            CreateFolderWithSelection,
            Rename,
            BulkRename,
            Delete,
            PermanentDelete,
            ClearSelection,
            OpenProperties,
            OpenItem,
            OpenInNewTab,
            OpenInNewWindow,
            ExtractArchive,
            ExtractArchiveHere,
            CreateArchive,
            EmptyRecycleBin,
            RestoreRecycleBin,
            RestoreAllRecycleBin,
            Refresh,
            Search,
            ClearFilter,
            ToggleHiddenFiles,
            ToggleDotFiles,
            ToggleFileExtensions,
            ToggleCheckboxes,
            ToggleSidebar,
            ToggleToolbar,
            ToggleDetailsPane,
            ToggleInfoPane,
            ToggleShelfPane,
            ToggleCompactOverlay,
            ToggleFullScreen,
            EditPath,
            OpenPreviewPane,
            ViewDetails,
            ViewList,
            ViewGrid,
            ViewColumns,
            CycleViewMode,
            CycleSortColumn,
            CycleGroupBy,
            ZoomIn,
            ZoomOut,
            ZoomReset,
            NewTab,
            CloseTab,
            CloseOtherTabs,
            CloseAllTabs,
            ReopenClosedTab,
            DuplicateTab,
            PinTab,
            MoveTabLeft,
            MoveTabRight,
            NewWindow,
            NextTab,
            PreviousTab,
            ToggleDualPane,
            SplitVertical,
            SplitHorizontal,
            SwitchPaneFocus,
            MouseNavigateBack,
            MouseNavigateForward,
            MiddleClickOpenNewTab,
            MiddleClickCloseTab,
            DoubleClickBlankGoUp,
            MouseWheelZoomIn,
            MouseWheelZoomOut,
            CommandPalette,
            OpenTerminal,
            OpenTerminalAsAdmin,
            OpenInIDE,
            OpenHelp,
            OpenSettings,
            RestoreLastSession,
            CloseActivePane,
            CloseTabsToLeft,
            CloseTabsToRight,
            CompressTo7z,
            CompressToTar,
            CompressToZip,
            CreateShortcut,
            DualHorizontal,
            DualVertical,
            DuplicateItem,
            ExtractHere,
            ExtractToFolder,
            FilterItems,
            FindInPage,
            FocusPathBar,
            GroupByNone,
            MoveTabToNewWindow,
            OpenInIde,
            OpenInNewPane,
            OpenSelectedItem,
            OpenStorageSense,
            PinToStart,
            UnpinFromStart,
            RotateImageLeft,
            RotateImageRight,
            RunAsAdmin,
            SetAsLockScreen,
            SetAsWallpaper,
            ShareItem,
            SortByName,
            SortByDateModified,
            SortBySize,
            SortByType,
            SwapPanes,
            ToggleItemSelection,
            TogglePreviewPane,
            ToggleSortDirection,
            Undo,
            Redo,
        ]
    }
}

// ---------------------------------------------------------------------------
// ActionRegistry
// ---------------------------------------------------------------------------

/// Central mapping from `ActionKind` to its current keyboard or mouse binding.
///
/// Supports reverse lookup, conflict detection, individual action reset,
/// unbinding, and full `Files-community` default restoration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionRegistry {
    /// Map: ActionKind → bound KeyCombination
    bindings: HashMap<ActionKind, KeyCombination>,
    /// Cached reverse map: KeyCombination → ActionKind (computed on deserialization)
    #[serde(skip)]
    reverse: HashMap<KeyCombination, ActionKind>,
}

impl Default for ActionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ActionRegistry {
    /// Create a new registry with default bindings loaded.
    pub fn new() -> Self {
        let bindings = default_bindings();
        let reverse = Self::build_reverse(&bindings);
        Self { bindings, reverse }
    }

    /// Build the reverse lookup cache from a binding map.
    fn build_reverse(
        bindings: &HashMap<ActionKind, KeyCombination>,
    ) -> HashMap<KeyCombination, ActionKind> {
        let mut rev = HashMap::new();
        for (action, key) in bindings {
            if !key.is_empty() {
                rev.insert(key.clone(), *action);
            }
        }
        rev
    }

    /// Look up the action bound to a given `KeyCombination` (case-insensitive & canonical key matching).
    /// Get the current key combination bound to an action, if any.
    pub fn get_key(&self, action: ActionKind) -> Option<&KeyCombination> {
        self.bindings.get(&action)
    }

    pub fn lookup(&self, key: &KeyCombination) -> Option<ActionKind> {
        if key.is_empty() {
            return None;
        }
        if let Some(&action) = self.reverse.get(key) {
            return Some(action);
        }
        let target_canon = KeyCombination::canonical_key_name(&key.key);
        self.reverse
            .iter()
            .find(|(k, _)| {
                !k.is_empty()
                    && k.ctrl == key.ctrl
                    && k.alt == key.alt
                    && k.shift == key.shift
                    && KeyCombination::canonical_key_name(&k.key) == target_canon
            })
            .map(|(_, &action)| action)
    }

    /// Check if `key` is already bound to another action (excluding `excluding_action`).
    pub fn find_conflict(
        &self,
        key: &KeyCombination,
        excluding_action: Option<ActionKind>,
    ) -> Option<ActionKind> {
        if key.is_empty() {
            return None;
        }
        let target_canon = KeyCombination::canonical_key_name(&key.key);
        for (&act, bound) in &self.bindings {
            if Some(act) == excluding_action || bound.is_empty() {
                continue;
            }
            if bound.ctrl == key.ctrl
                && bound.alt == key.alt
                && bound.shift == key.shift
                && KeyCombination::canonical_key_name(&bound.key) == target_canon
            {
                return Some(act);
            }
        }
        None
    }

    /// Look up the key/mouse combination currently bound to a given action.
    pub fn binding(&self, action: ActionKind) -> Option<&KeyCombination> {
        self.bindings.get(&action).filter(|k| !k.is_empty())
    }

    /// Return the factory-default `KeyCombination` for `action`, if one exists.
    pub fn default_binding(action: ActionKind) -> Option<KeyCombination> {
        default_bindings().remove(&action)
    }

    /// Return `true` if `action` currently uses its factory-default binding.
    pub fn is_default(&self, action: ActionKind) -> bool {
        let cur = self.binding(action);
        let def = Self::default_binding(action);
        match (cur, def) {
            (Some(c), Some(d)) => c.matches(&d),
            (None, None) => true,
            _ => false,
        }
    }

    /// Set (or rebind) a keyboard or mouse combination for an action.
    /// If the `KeyCombination` is already bound to another action, the old
    /// binding is overwritten so every combination maps unambiguously.
    pub fn rebind(&mut self, action: ActionKind, key: KeyCombination) {
        if let Some(old_key) = self.bindings.get(&action) {
            self.reverse.remove(old_key);
        }
        let conflict = self.find_conflict(&key, Some(action));
        if let Some(old_action) = conflict {
            if let Some(prev_k) = self.bindings.remove(&old_action) {
                self.reverse.remove(&prev_k);
            }
        }
        if let Some(old_action) = self.reverse.remove(&key) {
            self.bindings.remove(&old_action);
        }
        if !key.is_empty() {
            self.reverse.insert(key.clone(), action);
        }
        self.bindings.insert(action, key);
    }

    /// Remove (unbind) the shortcut/mouse combination for `action`.
    pub fn unbind(&mut self, action: ActionKind) {
        if let Some(old_key) = self.bindings.remove(&action) {
            self.reverse.remove(&old_key);
        }
    }

    /// Reset a single action back to its factory-default binding.
    pub fn reset_default(&mut self, action: ActionKind) {
        let defaults = default_bindings();
        if let Some(default_key) = defaults.get(&action) {
            self.rebind(action, default_key.clone());
        } else {
            self.unbind(action);
        }
    }

    /// Reset all actions to their factory-default bindings.
    pub fn reset_all_defaults(&mut self) {
        *self = Self::new();
    }

    /// Iterate over all bound `(ActionKind, KeyCombination)` pairs sorted alphabetically by label.
    pub fn iter(&self) -> Vec<(ActionKind, KeyCombination)> {
        let mut pairs: Vec<(ActionKind, KeyCombination)> = ActionKind::all()
            .into_iter()
            .filter_map(|a| {
                self.bindings
                    .get(&a)
                    .filter(|k| !k.is_empty())
                    .map(|k| (a, k.clone()))
            })
            .collect();
        pairs.sort_by(|(a, _), (b, _)| a.label().cmp(b.label()));
        pairs
    }

    /// Return all actions (including unbound ones) with their current optional binding and whether it is customized.
    pub fn all_entries(&self) -> Vec<(ActionKind, Option<KeyCombination>, bool)> {
        let defaults = default_bindings();
        let mut list: Vec<(ActionKind, Option<KeyCombination>, bool)> = ActionKind::all()
            .into_iter()
            .map(|a| {
                let current = self.bindings.get(&a).filter(|k| !k.is_empty()).cloned();
                let def = defaults.get(&a);
                let is_custom = match (&current, def) {
                    (Some(c), Some(d)) => !c.matches(d),
                    (None, None) => false,
                    _ => true,
                };
                (a, current, is_custom)
            })
            .collect();
        list.sort_by(|(a, _, _), (b, _, _)| a.label().cmp(b.label()));
        list
    }

    /// Number of bound actions.
    pub fn len(&self) -> usize {
        self.bindings.values().filter(|k| !k.is_empty()).count()
    }

    /// Whether the registry has no bindings.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Rebuild the reverse cache (call after deserialization) and backfill any newly added default actions.
    pub fn rebuild_cache(&mut self) {
        let defaults = default_bindings();
        for (act, def_key) in defaults {
            self.bindings.entry(act).or_insert(def_key);
        }
        self.reverse = Self::build_reverse(&self.bindings);
    }
}

// ---------------------------------------------------------------------------
// ToggleAction trait
// ---------------------------------------------------------------------------

/// Extension for actions that carry an on/off checked state.
pub trait IsToggle {
    /// Whether this action is in its "checked" (on) state.
    fn is_checked(&self) -> bool;

    /// Toggle between checked and unchecked.
    fn toggle(&mut self);
}

// ---------------------------------------------------------------------------
// Default bindings (Files-community parity: Keyboard + Mouse combinations)
// ---------------------------------------------------------------------------

/// Build the canonical default keyboard and mouse binding map.
pub fn default_bindings() -> HashMap<ActionKind, KeyCombination> {
    let mut m = HashMap::new();

    // Navigation
    m.insert(
        ActionKind::NavigateBack,
        KeyCombination::new("left").with_alt(),
    );
    m.insert(
        ActionKind::NavigateForward,
        KeyCombination::new("right").with_alt(),
    );
    m.insert(ActionKind::NavigateUp, KeyCombination::new("up").with_alt());
    m.insert(
        ActionKind::NavigateToRoot,
        KeyCombination::new("\\").with_ctrl(),
    );
    m.insert(
        ActionKind::NavigateHome,
        KeyCombination::new("h").with_ctrl_shift(),
    );

    // Selection & Editing
    m.insert(ActionKind::SelectAll, KeyCombination::new("a").with_ctrl());
    m.insert(
        ActionKind::InvertSelection,
        KeyCombination::new("i").with_ctrl(),
    );
    m.insert(ActionKind::Copy, KeyCombination::new("c").with_ctrl());
    m.insert(ActionKind::Cut, KeyCombination::new("x").with_ctrl());
    m.insert(ActionKind::Paste, KeyCombination::new("v").with_ctrl());
    m.insert(
        ActionKind::PasteAsShortcut,
        KeyCombination::new("v").with_ctrl_shift(),
    );
    m.insert(
        ActionKind::CopyItemPath,
        KeyCombination::new("c").with_ctrl_shift(),
    );
    m.insert(
        ActionKind::CopyItemPathWithQuotes,
        KeyCombination::new("c").with_ctrl_alt(),
    );
    m.insert(
        ActionKind::NewFolder,
        KeyCombination::new("n").with_ctrl_shift(),
    );
    m.insert(
        ActionKind::NewFile,
        KeyCombination::new("i").with_ctrl_shift(),
    );
    m.insert(
        ActionKind::CreateFolderWithSelection,
        KeyCombination::new("g").with_ctrl_shift(),
    );
    m.insert(ActionKind::Rename, KeyCombination::new("F2"));
    m.insert(
        ActionKind::BulkRename,
        KeyCombination::new("F2").with_ctrl(),
    );
    m.insert(ActionKind::Delete, KeyCombination::new("Delete"));
    m.insert(
        ActionKind::PermanentDelete,
        KeyCombination::new("Delete").with_shift(),
    );
    m.insert(ActionKind::ClearSelection, KeyCombination::new("Escape"));
    m.insert(
        ActionKind::OpenProperties,
        KeyCombination::new("enter").with_alt(),
    );
    m.insert(ActionKind::OpenItem, KeyCombination::new("Enter"));
    m.insert(
        ActionKind::OpenInNewTab,
        KeyCombination::new("enter").with_ctrl(),
    );
    m.insert(
        ActionKind::OpenInNewWindow,
        KeyCombination::new("enter").with_ctrl_alt(),
    );
    m.insert(
        ActionKind::ExtractArchive,
        KeyCombination::new("e").with_ctrl(),
    );
    m.insert(
        ActionKind::ExtractArchiveHere,
        KeyCombination::new("e").with_ctrl_shift(),
    );
    m.insert(
        ActionKind::CreateArchive,
        KeyCombination::new("z").with_ctrl_alt(),
    );
    m.insert(
        ActionKind::EmptyRecycleBin,
        KeyCombination::new("Delete").with_ctrl_shift(),
    );
    m.insert(
        ActionKind::RestoreRecycleBin,
        KeyCombination::new("r").with_ctrl_alt(),
    );
    m.insert(
        ActionKind::RestoreAllRecycleBin,
        KeyCombination::new("r").with_ctrl_shift(),
    );
    m.insert(ActionKind::Undo, KeyCombination::new("z").with_ctrl());
    m.insert(ActionKind::Redo, KeyCombination::new("y").with_ctrl());

    // View & Layout
    m.insert(ActionKind::Refresh, KeyCombination::new("F5"));
    m.insert(ActionKind::Search, KeyCombination::new("f").with_ctrl());
    m.insert(
        ActionKind::ToggleHiddenFiles,
        KeyCombination::new("h").with_ctrl(),
    );
    m.insert(
        ActionKind::ToggleDotFiles,
        KeyCombination::new(".").with_ctrl_shift(),
    );
    m.insert(
        ActionKind::ToggleFileExtensions,
        KeyCombination::new("F8"),
    );
    m.insert(
        ActionKind::ToggleCheckboxes,
        KeyCombination::new("x").with_ctrl_shift(),
    );
    m.insert(
        ActionKind::ToggleSidebar,
        KeyCombination::new("b").with_ctrl(),
    );
    m.insert(
        ActionKind::ToggleToolbar,
        KeyCombination::new("b").with_ctrl_shift(),
    );
    m.insert(
        ActionKind::ToggleDetailsPane,
        KeyCombination::new("p").with_alt(),
    );
    m.insert(
        ActionKind::ToggleInfoPane,
        KeyCombination::new("i").with_ctrl_alt(),
    );
    m.insert(
        ActionKind::ToggleShelfPane,
        KeyCombination::new("s").with_alt(),
    );
    m.insert(
        ActionKind::ToggleCompactOverlay,
        KeyCombination::new("F12"),
    );
    m.insert(ActionKind::ToggleFullScreen, KeyCombination::new("F11"));
    m.insert(ActionKind::EditPath, KeyCombination::new("l").with_ctrl());
    m.insert(
        ActionKind::OpenPreviewPane,
        KeyCombination::new("p").with_ctrl(),
    );
    m.insert(
        ActionKind::ViewDetails,
        KeyCombination::new("1").with_ctrl_shift(),
    );
    m.insert(
        ActionKind::ViewList,
        KeyCombination::new("2").with_ctrl_shift(),
    );
    m.insert(
        ActionKind::ViewGrid,
        KeyCombination::new("3").with_ctrl_shift(),
    );
    m.insert(
        ActionKind::ViewColumns,
        KeyCombination::new("4").with_ctrl_shift(),
    );
    m.insert(
        ActionKind::CycleViewMode,
        KeyCombination::new("5").with_ctrl_shift(),
    );
    m.insert(
        ActionKind::CycleSortColumn,
        KeyCombination::new("s").with_ctrl_alt(),
    );
    m.insert(
        ActionKind::CycleGroupBy,
        KeyCombination::new("g").with_ctrl_alt(),
    );
    m.insert(ActionKind::ZoomIn, KeyCombination::new("=").with_ctrl());
    m.insert(ActionKind::ZoomOut, KeyCombination::new("-").with_ctrl());
    m.insert(ActionKind::ZoomReset, KeyCombination::new("0").with_ctrl());

    // Tabs & Windows
    m.insert(ActionKind::NewTab, KeyCombination::new("t").with_ctrl());
    m.insert(ActionKind::CloseTab, KeyCombination::new("w").with_ctrl());
    m.insert(
        ActionKind::CloseOtherTabs,
        KeyCombination::new("w").with_ctrl().with_alt(),
    );
    m.insert(
        ActionKind::CloseAllTabs,
        KeyCombination::new("w").with_ctrl_shift(),
    );
    m.insert(
        ActionKind::ReopenClosedTab,
        KeyCombination::new("t").with_ctrl_shift(),
    );
    m.insert(
        ActionKind::DuplicateTab,
        KeyCombination::new("k").with_ctrl_shift(),
    );
    m.insert(
        ActionKind::PinTab,
        KeyCombination::new("p").with_ctrl().with_alt(),
    );
    m.insert(
        ActionKind::MoveTabLeft,
        KeyCombination::new("left").with_ctrl_shift(),
    );
    m.insert(
        ActionKind::MoveTabRight,
        KeyCombination::new("right").with_ctrl_shift(),
    );
    m.insert(ActionKind::NewWindow, KeyCombination::new("n").with_ctrl());
    m.insert(ActionKind::NextTab, KeyCombination::new("tab").with_ctrl());
    m.insert(
        ActionKind::PreviousTab,
        KeyCombination::new("tab").with_ctrl_shift(),
    );

    // Split Panes
    m.insert(
        ActionKind::ToggleDualPane,
        KeyCombination::new("s").with_ctrl_shift(),
    );
    m.insert(
        ActionKind::SplitVertical,
        KeyCombination::new("v").with_alt_shift(),
    );
    m.insert(
        ActionKind::SplitHorizontal,
        KeyCombination::new("h").with_alt_shift(),
    );
    m.insert(ActionKind::SwitchPaneFocus, KeyCombination::new("F6"));

    // Mouse Combinations (Files-community parity)
    m.insert(
        ActionKind::MouseNavigateBack,
        KeyCombination::mouse("Mouse4"),
    );
    m.insert(
        ActionKind::MouseNavigateForward,
        KeyCombination::mouse("Mouse5"),
    );
    m.insert(
        ActionKind::MiddleClickOpenNewTab,
        KeyCombination::mouse("MiddleClick"),
    );
    m.insert(
        ActionKind::MiddleClickCloseTab,
        KeyCombination::mouse("MiddleClickTab"),
    );
    m.insert(
        ActionKind::DoubleClickBlankGoUp,
        KeyCombination::mouse("DoubleClickBlank"),
    );
    m.insert(
        ActionKind::MouseWheelZoomIn,
        KeyCombination::mouse("WheelUp").with_ctrl(),
    );
    m.insert(
        ActionKind::MouseWheelZoomOut,
        KeyCombination::mouse("WheelDown").with_ctrl(),
    );

    // Commands & System
    m.insert(
        ActionKind::CommandPalette,
        KeyCombination::new("p").with_ctrl_shift(),
    );
    m.insert(
        ActionKind::OpenTerminal,
        KeyCombination::new("`").with_ctrl(),
    );
    m.insert(
        ActionKind::OpenTerminalAsAdmin,
        KeyCombination::new("`").with_ctrl_shift(),
    );
    m.insert(
        ActionKind::OpenInIDE,
        KeyCombination::new("o").with_ctrl_shift(),
    );
    m.insert(ActionKind::OpenHelp, KeyCombination::new("F1"));
    m.insert(ActionKind::OpenSettings, KeyCombination::new(",").with_ctrl());

    m
}

// ---------------------------------------------------------------------------
// Dispatch helper
// ---------------------------------------------------------------------------

/// Parse a GPUI `Keystroke` into our `KeyCombination`.
pub fn from_gpui_keystroke(keystroke: &gpui::Keystroke) -> KeyCombination {
    let key = keystroke.key.as_str().to_lowercase();
    KeyCombination {
        key,
        ctrl: keystroke.modifiers.control,
        alt: keystroke.modifiers.alt,
        shift: keystroke.modifiers.shift,
    }
}

/// Convert a GPUI `MouseButton` and `Modifiers` into a `KeyCombination`.
pub fn from_gpui_mouse_button(
    button: gpui::MouseButton,
    modifiers: gpui::Modifiers,
    click_count: usize,
) -> KeyCombination {
    let key = match button {
        gpui::MouseButton::Navigate(gpui::NavigationDirection::Back) => "Mouse4",
        gpui::MouseButton::Navigate(gpui::NavigationDirection::Forward) => "Mouse5",
        gpui::MouseButton::Middle => "MiddleClick",
        gpui::MouseButton::Right => "RightClick",
        gpui::MouseButton::Left => {
            if click_count >= 2 {
                "DoubleClick"
            } else {
                "LeftClick"
            }
        }
    };
    KeyCombination {
        key: key.to_string(),
        ctrl: modifiers.control,
        alt: modifiers.alt,
        shift: modifiers.shift,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_registry_lookup_case_insensitive() {
        let reg = ActionRegistry::new();
        assert_eq!(
            reg.lookup(&KeyCombination::new("f2")),
            Some(ActionKind::Rename)
        );
        assert_eq!(
            reg.lookup(&KeyCombination::new("F5")),
            Some(ActionKind::Refresh)
        );
        assert_eq!(
            reg.lookup(&KeyCombination::new("n").with_ctrl_shift()),
            Some(ActionKind::NewFolder)
        );
    }

    #[test]
    fn test_mouse_combinations_and_rebind() {
        let mut reg = ActionRegistry::new();
        assert_eq!(
            reg.lookup(&KeyCombination::mouse("Mouse4")),
            Some(ActionKind::MouseNavigateBack)
        );
        assert_eq!(
            reg.lookup(&KeyCombination::mouse("WheelUp").with_ctrl()),
            Some(ActionKind::MouseWheelZoomIn)
        );

        let custom = KeyCombination::parse("Ctrl+Alt+N").unwrap();
        assert!(reg.find_conflict(&custom, None).is_none());
        reg.rebind(ActionKind::NewFolder, custom.clone());
        assert!(!reg.is_default(ActionKind::NewFolder));
        assert_eq!(reg.lookup(&custom), Some(ActionKind::NewFolder));

        reg.reset_default(ActionKind::NewFolder);
        assert!(reg.is_default(ActionKind::NewFolder));
    }
}

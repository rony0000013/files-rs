//! # Action Registry
//!
//! Central action/commands framework for files-rs.
//! Maps named `ActionKind` values to `KeyCombination` bindings,
//! and provides dispatch, rebinding, and serialization.
//!
//! ## Architecture
//!
//! - `ActionKind` — enum identifying every built-in action
//! - `KeyCombination` — serializable representation of a keyboard shortcut
//! - `ActionRegistry` — owns the `(ActionKind → KeyCombination)` mapping,
//!   provides default bindings, lookup, rebinding, and two testable dispatch methods
//! - `ToggleAction` trait — actions that carry checked/unchecked state

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

// ---------------------------------------------------------------------------
// KeyCombination
// ---------------------------------------------------------------------------

/// A serializable representation of a keyboard shortcut.
///
/// Mirrors GPUI's `Keystroke` but without the raw GPUI types, so it can be
/// stored in JSON/config files.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct KeyCombination {
    /// The physical key (e.g. "a", "F2", "Escape", "Delete", "Space")
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
}

impl fmt::Display for KeyCombination {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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
// ActionKind
// ---------------------------------------------------------------------------

/// Every built-in action in the application.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ActionKind {
    // Navigation
    NavigateBack,
    NavigateForward,
    NavigateUp,
    NavigateToRoot,

    // Selection & Editing
    SelectAll,
    InvertSelection,
    Copy,
    Cut,
    Paste,
    /// Copy the selected item's full path as text (Files parity: Ctrl+Shift+C)
    CopyItemPath,
    NewFolder,
    Rename,
    Delete,
    PermanentDelete,
    ClearSelection,
    OpenProperties,
    Undo,
    Redo,

    // View
    Refresh,
    Search,
    ClearFilter,
    ToggleHiddenFiles,
    EditPath,
    OpenPreviewPane,
    ViewDetails,
    ViewList,
    ViewGrid,
    ViewColumns,

    // Tabs & Windows
    NewTab,
    CloseTab,
    CloseOtherTabs,
    ReopenClosedTab,
    DuplicateTab,
    PinTab,
    MoveTabLeft,
    MoveTabRight,
    NewWindow,
    NextTab,
    PreviousTab,

    // Split Panes
    SplitVertical,
    SplitHorizontal,
    SwitchPaneFocus,

    // Commands
    CommandPalette,
    OpenTerminal,
    /// Open the help/documentation URL
    OpenHelp,
    /// Open the settings page as a tab
    OpenSettings,
    /// Restore the last session after a crash
    RestoreLastSession,
}

impl ActionKind {
    /// Human-readable label shown in the settings UI and shortcut overlay.
    pub fn label(&self) -> &'static str {
        match self {
            Self::NavigateBack => "Navigate Back",
            Self::NavigateForward => "Navigate Forward",
            Self::NavigateUp => "Navigate Up",
            Self::NavigateToRoot => "Navigate to Root",
            Self::SelectAll => "Select All",
            Self::InvertSelection => "Invert Selection",
            Self::Copy => "Copy",
            Self::Cut => "Cut",
            Self::Paste => "Paste",
            Self::CopyItemPath => "Copy Item Path",
            Self::NewFolder => "New Folder",
            Self::Rename => "Rename",
            Self::Delete => "Move to Trash",
            Self::PermanentDelete => "Permanent Delete",
            Self::ClearSelection => "Clear Selection",
            Self::OpenProperties => "Properties",
            Self::Undo => "Undo",
            Self::Redo => "Redo",
            Self::Refresh => "Refresh",
            Self::Search => "Search",
            Self::ClearFilter => "Clear Filter / Escape",
            Self::ToggleHiddenFiles => "Toggle Hidden Files",
            Self::EditPath => "Edit Path",
            Self::OpenPreviewPane => "Open Preview Pane",
            Self::ViewDetails => "Details View",
            Self::ViewList => "List View",
            Self::ViewGrid => "Grid View",
            Self::ViewColumns => "Columns View",
            Self::NewTab => "New Tab",
            Self::CloseTab => "Close Tab",
            Self::CloseOtherTabs => "Close Other Tabs",
            Self::ReopenClosedTab => "Reopen Closed Tab",
            Self::DuplicateTab => "Duplicate Tab",
            Self::PinTab => "Pin / Unpin Tab",
            Self::MoveTabLeft => "Move Tab Left",
            Self::MoveTabRight => "Move Tab Right",
            Self::NewWindow => "New Window",
            Self::NextTab => "Next Tab",
            Self::PreviousTab => "Previous Tab",
            Self::SplitVertical => "Split Vertical",
            Self::SplitHorizontal => "Split Horizontal",
            Self::SwitchPaneFocus => "Switch Active Pane",
            Self::CommandPalette => "Command Palette",
            Self::OpenTerminal => "Open in Terminal",
            Self::OpenHelp => "Open Help",
            Self::OpenSettings => "Open Settings",
            Self::RestoreLastSession => "Restore Last Session",
        }
    }

    /// Returns `true` if the action is conceptually a toggle
    /// (it has an on/off checked state).
    pub fn is_toggle(&self) -> bool {
        matches!(self, Self::ToggleHiddenFiles | Self::PinTab)
    }

    /// Return all action variants (for iterating in the settings UI).
    pub fn all() -> Vec<ActionKind> {
        use ActionKind::*;
        vec![
            NavigateBack,
            NavigateForward,
            NavigateUp,
            NavigateToRoot,
            SelectAll,
            InvertSelection,
            Copy,
            Cut,
            Paste,
            CopyItemPath,
            NewFolder,
            Rename,
            Delete,
            PermanentDelete,
            ClearSelection,
            OpenProperties,
            Refresh,
            Search,
            ClearFilter,
            ToggleHiddenFiles,
            EditPath,
            OpenPreviewPane,
            ViewDetails,
            ViewList,
            ViewGrid,
            ViewColumns,
            NewTab,
            CloseTab,
            CloseOtherTabs,
            ReopenClosedTab,
            DuplicateTab,
            PinTab,
            MoveTabLeft,
            MoveTabRight,
            NewWindow,
            NextTab,
            PreviousTab,
            SplitVertical,
            SplitHorizontal,
            SwitchPaneFocus,
            CommandPalette,
            OpenTerminal,
            OpenHelp,
            OpenSettings,
            RestoreLastSession,
            Undo,
            Redo,
        ]
    }
}

// ---------------------------------------------------------------------------
// ActionRegistry
// ---------------------------------------------------------------------------

/// Central mapping from `ActionKind` to its current keybinding.
///
/// Also supports reverse lookup (keystroke string → `ActionKind`) for use
/// in the GPUI `on_key_down` handler.
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
            rev.insert(key.clone(), *action);
        }
        rev
    }

    /// Look up the action bound to a given `KeyCombination` (case-insensitive key matching).
    pub fn lookup(&self, key: &KeyCombination) -> Option<ActionKind> {
        if let Some(&action) = self.reverse.get(key) {
            return Some(action);
        }
        let key_lower = key.key.to_ascii_lowercase();
        self.reverse
            .iter()
            .find(|(k, _)| {
                k.ctrl == key.ctrl
                    && k.alt == key.alt
                    && k.shift == key.shift
                    && k.key.to_ascii_lowercase() == key_lower
            })
            .map(|(_, &action)| action)
    }

    /// Look up the key combination bound to a given action.
    pub fn binding(&self, action: ActionKind) -> Option<&KeyCombination> {
        self.bindings.get(&action)
    }

    /// Set (or rebind) a key combination for an action.
    /// Returns `Ok(())` on success.
    /// If the `KeyCombination` is already bound to another action, the old
    /// binding is silently overwritten (last-write-wins).
    pub fn rebind(&mut self, action: ActionKind, key: KeyCombination) {
        if let Some(old_key) = self.bindings.get(&action) {
            self.reverse.remove(old_key);
        }
        if let Some(old_action) = self.reverse.remove(&key) {
            self.bindings.remove(&old_action);
        }
        self.bindings.insert(action, key.clone());
        self.reverse.insert(key, action);
    }

    /// Reset an action back to its default keybinding.
    pub fn reset_default(&mut self, action: ActionKind) {
        let defaults = default_bindings();
        if let Some(default_key) = defaults.get(&action) {
            self.rebind(action, default_key.clone());
        }
    }

    /// Reset all actions to their default keybindings.
    pub fn reset_all_defaults(&mut self) {
        *self = Self::new();
    }

    /// Iterate over all bound (ActionKind, KeyCombination) pairs.
    pub fn iter(&self) -> Vec<(ActionKind, KeyCombination)> {
        let mut pairs: Vec<(ActionKind, KeyCombination)> = ActionKind::all()
            .into_iter()
            .filter_map(|a| self.bindings.get(&a).map(|k| (a, k.clone())))
            .collect();
        pairs.sort_by(|(a, _), (b, _)| a.label().cmp(b.label()));
        pairs
    }

    /// Number of bound actions.
    pub fn len(&self) -> usize {
        self.bindings.len()
    }

    /// Whether the registry has no bindings.
    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty()
    }

    /// Rebuild the reverse cache (call after deserialization).
    pub fn rebuild_cache(&mut self) {
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
// Default bindings
// ---------------------------------------------------------------------------

/// Build the canonical default keybinding map.
fn default_bindings() -> HashMap<ActionKind, KeyCombination> {
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
        ActionKind::CopyItemPath,
        KeyCombination::new("c").with_ctrl_shift(),
    );
    m.insert(
        ActionKind::NewFolder,
        KeyCombination::new("n").with_ctrl_shift(),
    );
    m.insert(ActionKind::Rename, KeyCombination::new("F2"));
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
    m.insert(ActionKind::Undo, KeyCombination::new("z").with_ctrl());
    m.insert(ActionKind::Redo, KeyCombination::new("y").with_ctrl());

    // View
    m.insert(ActionKind::Refresh, KeyCombination::new("F5"));
    m.insert(ActionKind::Search, KeyCombination::new("f").with_ctrl());
    m.insert(
        ActionKind::ToggleHiddenFiles,
        KeyCombination::new("h").with_ctrl(),
    );
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

    // Tabs & Windows
    m.insert(ActionKind::NewTab, KeyCombination::new("t").with_ctrl());
    m.insert(ActionKind::CloseTab, KeyCombination::new("w").with_ctrl());
    m.insert(
        ActionKind::CloseOtherTabs,
        KeyCombination::new("w").with_ctrl().with_alt(),
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
        ActionKind::SplitVertical,
        KeyCombination::new("v").with_ctrl_shift(),
    );
    m.insert(
        ActionKind::SplitHorizontal,
        KeyCombination::new("h").with_ctrl_shift(),
    );
    m.insert(ActionKind::SwitchPaneFocus, KeyCombination::new("F6"));

    // Commands
    m.insert(
        ActionKind::CommandPalette,
        KeyCombination::new("p").with_ctrl_shift(),
    );
    m.insert(
        ActionKind::OpenTerminal,
        KeyCombination::new("`").with_ctrl(),
    );
    m.insert(ActionKind::OpenHelp, KeyCombination::new("F1"));
    m.insert(ActionKind::OpenSettings, KeyCombination::new(",").with_ctrl());

    m
}

// Helper to build Ctrl+Shift combos
impl KeyCombination {
    fn with_ctrl_shift(mut self) -> Self {
        self.ctrl = true;
        self.shift = true;
        self
    }
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

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
    Rename,
    Delete,
    PermanentDelete,
    ClearSelection,

    // View
    Refresh,
    Search,
    ClearFilter,
    ToggleHiddenFiles,
    EditPath,
    OpenPreviewPane,

    // Tabs & Windows
    NewTab,
    CloseTab,
    ReopenClosedTab,
    NewWindow,

    // Commands
    CommandPalette,
    /// Open the help/documentation URL
    OpenHelp,
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
            Self::Rename => "Rename",
            Self::Delete => "Move to Trash",
            Self::PermanentDelete => "Permanent Delete",
            Self::ClearSelection => "Clear Selection",
            Self::Refresh => "Refresh",
            Self::Search => "Search",
            Self::ClearFilter => "Clear Filter / Escape",
            Self::ToggleHiddenFiles => "Toggle Hidden Files",
            Self::EditPath => "Edit Path",
            Self::OpenPreviewPane => "Open Preview Pane",
            Self::NewTab => "New Tab",
            Self::CloseTab => "Close Tab",
            Self::ReopenClosedTab => "Reopen Closed Tab",
            Self::NewWindow => "New Window",
            Self::CommandPalette => "Command Palette",
            Self::OpenHelp => "Open Help",
            Self::RestoreLastSession => "Restore Last Session",
        }
    }

    /// Returns `true` if the action is conceptually a toggle
    /// (it has an on/off checked state).
    pub fn is_toggle(&self) -> bool {
        matches!(self, Self::ToggleHiddenFiles)
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
            Rename,
            Delete,
            PermanentDelete,
            ClearSelection,
            Refresh,
            Search,
            ClearFilter,
            ToggleHiddenFiles,
            EditPath,
            OpenPreviewPane,
            NewTab,
            CloseTab,
            ReopenClosedTab,
            NewWindow,
            CommandPalette,
            OpenHelp,
            RestoreLastSession,
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

    /// Look up the action bound to a given `KeyCombination`.
    pub fn lookup(&self, key: &KeyCombination) -> Option<ActionKind> {
        self.reverse.get(key).copied()
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
        // Remove any existing binding for this action
        if let Some(old_key) = self.bindings.get(&action) {
            self.reverse.remove(old_key);
        }
        // Remove any action that was bound to this key (last-write-wins)
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
    m.insert(ActionKind::NavigateToRoot, KeyCombination::new("Home"));

    // Selection & Editing
    m.insert(ActionKind::SelectAll, KeyCombination::new("a").with_ctrl());
    m.insert(
        ActionKind::InvertSelection,
        KeyCombination::new("i").with_ctrl(),
    );
    m.insert(ActionKind::Copy, KeyCombination::new("c").with_ctrl());
    m.insert(ActionKind::Cut, KeyCombination::new("x").with_ctrl());
    m.insert(ActionKind::Paste, KeyCombination::new("v").with_ctrl());
    m.insert(ActionKind::Rename, KeyCombination::new("F2"));
    m.insert(ActionKind::Delete, KeyCombination::new("Delete"));
    m.insert(
        ActionKind::PermanentDelete,
        KeyCombination::new("Delete").with_shift(),
    );
    m.insert(ActionKind::ClearSelection, KeyCombination::new("Escape"));

    // View
    m.insert(ActionKind::Refresh, KeyCombination::new("F5"));
    m.insert(ActionKind::Search, KeyCombination::new("f").with_ctrl());
    m.insert(ActionKind::ClearFilter, KeyCombination::new("Escape"));
    m.insert(
        ActionKind::ToggleHiddenFiles,
        KeyCombination::new("h").with_ctrl(),
    );
    m.insert(ActionKind::EditPath, KeyCombination::new("l").with_ctrl());
    m.insert(ActionKind::OpenPreviewPane, KeyCombination::new("Space"));

    // Tabs & Windows
    m.insert(ActionKind::NewTab, KeyCombination::new("t").with_ctrl());
    m.insert(ActionKind::CloseTab, KeyCombination::new("w").with_ctrl());
    m.insert(
        ActionKind::ReopenClosedTab,
        KeyCombination::new("t").with_ctrl_shift(),
    );
    m.insert(ActionKind::NewWindow, KeyCombination::new("n").with_ctrl());

    // Commands
    m.insert(
        ActionKind::CommandPalette,
        KeyCombination::new("p").with_ctrl_shift(),
    );
    m.insert(ActionKind::OpenHelp, KeyCombination::new("F1"));

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
/// GPUI's `Keystroke` has `key: String` and `modifiers: Modifiers` with
/// `control`, `alt`, `shift` booleans.
pub fn from_gpui_keystroke(keystroke: &gpui::Keystroke) -> KeyCombination {
    KeyCombination {
        key: keystroke.key.clone(),
        ctrl: keystroke.modifiers.control,
        alt: keystroke.modifiers.alt,
        shift: keystroke.modifiers.shift,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::Keystroke;

    // -----------------------------------------------------------------------
    // KeyCombination tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_key_combination_display_no_modifiers() {
        let k = KeyCombination::new("F2");
        assert_eq!(k.to_string(), "F2");
    }

    #[test]
    fn test_key_combination_display_ctrl() {
        let k = KeyCombination::new("c").with_ctrl();
        assert_eq!(k.to_string(), "Ctrl+c");
    }

    #[test]
    fn test_key_combination_display_ctrl_shift() {
        let k = KeyCombination::new("p").with_ctrl_shift();
        assert_eq!(k.to_string(), "Ctrl+Shift+p");
    }

    #[test]
    fn test_key_combination_display_alt_left() {
        let k = KeyCombination::new("left").with_alt();
        assert_eq!(k.to_string(), "Alt+left");
    }

    #[test]
    fn test_key_combination_display_all_modifiers() {
        let k = KeyCombination::new("Z").with_ctrl().with_alt().with_shift();
        assert_eq!(k.to_string(), "Ctrl+Alt+Shift+Z");
    }

    #[test]
    fn test_key_combination_json_roundtrip() {
        let k = KeyCombination::new("a").with_ctrl();
        let json = serde_json::to_string(&k).unwrap();
        let k2: KeyCombination = serde_json::from_str(&json).unwrap();
        assert_eq!(k, k2);
    }

    // -----------------------------------------------------------------------
    // ActionKind tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_action_kind_labels_non_empty() {
        for action in ActionKind::all() {
            let label = action.label();
            assert!(
                !label.is_empty(),
                "ActionKind::{:?} has empty label",
                action
            );
        }
    }

    #[test]
    fn test_action_kind_toggle_detection() {
        assert!(ActionKind::ToggleHiddenFiles.is_toggle());
        assert!(!ActionKind::Copy.is_toggle());
        assert!(!ActionKind::Delete.is_toggle());
    }

    #[test]
    fn test_action_kind_all_has_no_duplicates() {
        let all = ActionKind::all();
        let mut seen = std::collections::HashSet::new();
        for a in &all {
            assert!(seen.insert(a), "Duplicate ActionKind::{:?} in all()", a);
        }
    }

    // -----------------------------------------------------------------------
    // ActionRegistry tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_registry_has_default_bindings() {
        let reg = ActionRegistry::new();
        // Should have at least 20+ default bindings
        assert!(
            reg.len() >= 20,
            "Expected >=20 default bindings, got {}",
            reg.len()
        );
    }

    #[test]
    fn test_registry_lookup_by_key() {
        let reg = ActionRegistry::new();

        // Ctrl+C → Copy
        let key = KeyCombination::new("c").with_ctrl();
        assert_eq!(reg.lookup(&key), Some(ActionKind::Copy));

        // F5 → Refresh
        let key = KeyCombination::new("F5");
        assert_eq!(reg.lookup(&key), Some(ActionKind::Refresh));

        // Escape → ClearSelection (first match is ClearSelection)
        let key = KeyCombination::new("Escape");
        assert_eq!(reg.lookup(&key), Some(ActionKind::ClearSelection));
    }

    #[test]
    fn test_registry_binding_for_action() {
        let reg = ActionRegistry::new();

        let binding = reg.binding(ActionKind::Rename);
        assert!(binding.is_some());
        assert_eq!(binding.unwrap().key, "F2");
    }

    #[test]
    fn test_registry_rebind() {
        let mut reg = ActionRegistry::new();

        // Rebind Copy from Ctrl+C to Ctrl+Y
        reg.rebind(ActionKind::Copy, KeyCombination::new("y").with_ctrl());

        let old_key = KeyCombination::new("c").with_ctrl();
        let new_key = KeyCombination::new("y").with_ctrl();

        assert_eq!(reg.lookup(&old_key), None, "Old binding should be gone");
        assert_eq!(
            reg.lookup(&new_key),
            Some(ActionKind::Copy),
            "New binding should work"
        );

        let binding = reg.binding(ActionKind::Copy);
        assert_eq!(binding.unwrap().key, "y");
    }

    #[test]
    fn test_registry_rebind_last_write_wins() {
        let mut reg = ActionRegistry::new();

        // Both Ctrl+Z → NavigateBack and Ctrl+Z → Copy
        // The second should win
        reg.rebind(
            ActionKind::NavigateBack,
            KeyCombination::new("z").with_ctrl(),
        );
        reg.rebind(ActionKind::Copy, KeyCombination::new("z").with_ctrl());

        let key = KeyCombination::new("z").with_ctrl();
        assert_eq!(reg.lookup(&key), Some(ActionKind::Copy));
        assert_eq!(
            reg.binding(ActionKind::NavigateBack),
            None,
            "NavigateBack was unbound by last-write-wins"
        );
    }

    #[test]
    fn test_registry_reset_default() {
        let mut reg = ActionRegistry::new();

        // Rebinding something
        reg.rebind(ActionKind::Refresh, KeyCombination::new("r").with_ctrl());
        assert_eq!(
            reg.lookup(&KeyCombination::new("r").with_ctrl()),
            Some(ActionKind::Refresh)
        );

        // Reset to default
        reg.reset_default(ActionKind::Refresh);
        assert_eq!(reg.lookup(&KeyCombination::new("r").with_ctrl()), None);
        assert_eq!(
            reg.lookup(&KeyCombination::new("F5")),
            Some(ActionKind::Refresh)
        );
    }

    #[test]
    fn test_registry_reset_all_defaults() {
        let mut reg = ActionRegistry::new();
        let original_len = reg.len();

        // Scramble some bindings
        reg.rebind(ActionKind::Copy, KeyCombination::new("y").with_ctrl());
        reg.rebind(ActionKind::Paste, KeyCombination::new("p").with_ctrl());

        reg.reset_all_defaults();
        assert_eq!(reg.len(), original_len, "All defaults should be restored");

        // Verify a known default is back
        assert_eq!(
            reg.lookup(&KeyCombination::new("c").with_ctrl()),
            Some(ActionKind::Copy)
        );
    }

    #[test]
    fn test_registry_iter_order() {
        let reg = ActionRegistry::new();
        let pairs = reg.iter();
        // Should be non-empty
        assert!(!pairs.is_empty());
        // First pair should have a label
        assert!(!pairs[0].0.label().is_empty());
        // Verify sorted by label
        for i in 1..pairs.len() {
            assert!(
                pairs[i - 1].0.label() <= pairs[i].0.label(),
                "Actions should be sorted by label: {} > {}",
                pairs[i - 1].0.label(),
                pairs[i].0.label()
            );
        }
    }

    #[test]
    fn test_registry_json_roundtrip() {
        let reg = ActionRegistry::new();
        let json = serde_json::to_string(&reg).unwrap();
        let mut reg2: ActionRegistry = serde_json::from_str(&json).unwrap();
        reg2.rebuild_cache();

        // Same lookups
        assert_eq!(
            reg2.lookup(&KeyCombination::new("F2")),
            Some(ActionKind::Rename)
        );
        assert_eq!(
            reg2.lookup(&KeyCombination::new("a").with_ctrl()),
            Some(ActionKind::SelectAll)
        );
    }

    // -----------------------------------------------------------------------
    // from_gpui_keystroke tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_from_gpui_keystroke_simple_key() {
        let ks = Keystroke {
            key: "F5".into(),
            modifiers: gpui::Modifiers::default(),
        };
        let kc = from_gpui_keystroke(&ks);
        assert_eq!(kc.key, "F5");
        assert!(!kc.ctrl);
        assert!(!kc.alt);
        assert!(!kc.shift);
    }

    #[test]
    fn test_from_gpui_keystroke_ctrl_c() {
        let ks = Keystroke {
            key: "c".into(),
            modifiers: gpui::Modifiers {
                control: true,
                alt: false,
                shift: false,
                ..Default::default()
            },
        };
        let kc = from_gpui_keystroke(&ks);
        assert_eq!(kc.key, "c");
        assert!(kc.ctrl);
        assert!(!kc.alt);
        assert!(!kc.shift);
    }

    #[test]
    fn test_from_gpui_keystroke_alt_left() {
        let ks = Keystroke {
            key: "left".into(),
            modifiers: gpui::Modifiers {
                control: false,
                alt: true,
                shift: false,
                ..Default::default()
            },
        };
        let kc = from_gpui_keystroke(&ks);
        assert_eq!(kc.key, "left");
        assert!(!kc.ctrl);
        assert!(kc.alt);
        assert!(!kc.shift);
    }

    #[test]
    fn test_from_gpui_keystroke_ctrl_shift_p() {
        let ks = Keystroke {
            key: "p".into(),
            modifiers: gpui::Modifiers {
                control: true,
                alt: false,
                shift: true,
                ..Default::default()
            },
        };
        let kc = from_gpui_keystroke(&ks);
        assert_eq!(kc.key, "p");
        assert!(kc.ctrl);
        assert!(!kc.alt);
        assert!(kc.shift);
    }

    // -----------------------------------------------------------------------
    // Edge cases
    // -----------------------------------------------------------------------

    #[test]
    fn test_registry_lookup_unknown_key_returns_none() {
        let reg = ActionRegistry::new();
        let unknown = KeyCombination::new("z").with_ctrl().with_alt().with_shift();
        assert_eq!(reg.lookup(&unknown), None);
    }

    #[test]
    fn test_registry_empty_state() {
        let reg = ActionRegistry::new();
        assert!(!reg.is_empty());
    }
}

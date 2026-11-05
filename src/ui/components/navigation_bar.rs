//! Navigation Bar & Omnibar — unified breadcrumb address bar, FilePilot-inspired predictive
//! path completion, live search bar (with Regex, Wildcards, AQS, Content & Metadata toggles),
//! Command Palette (`>`), Back/Forward history flyouts, and Status Center progress ring button.
//! Visually and functionally matched to Files-community (`Omnibar.xaml`, `NavigationToolbar.xaml`).

use crate::core::action_registry::{ActionKind, ActionRegistry};
use crate::core::path_util::FilesPath;
use crate::domain::metadata::FileItem;
use crate::domain::search::{
    compute_path_completions, expand_path_input, score_fuzzy_or_prefix, SearchOptions,
    SearchService,
};
use crate::storage::storage::FileType;
use crate::ui::constants::UiConstants;
use gpui::*;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum OmnibarMode {
    Path,
    Search,
    CommandPalette,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SuggestionKind {
    Path {
        path: FilesPath,
    },
    SearchHistory {
        query: String,
    },
    SearchItem {
        path: FilesPath,
        is_dir: bool,
    },
    AqsFilter {
        token: String,
    },
    Command {
        action: ActionKind,
    },
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Suggestion {
    pub kind: SuggestionKind,
    pub label: String,
    pub icon: String,
    #[serde(default)]
    pub subtitle: String,
    #[serde(default)]
    pub badge: Option<String>,
    #[serde(default)]
    pub snippet: Option<String>,
    #[serde(default)]
    pub match_indices: Vec<usize>,
    #[serde(default)]
    pub score: u32,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NavigationBarState {
    pub current_path: FilesPath,
    pub breadcrumbs: Vec<FilesPath>,
    pub edit_mode: bool,
    pub edit_text: String,
    /// Sibling folders for dropdown (loaded on demand)
    pub siblings: Vec<FilesPath>,
    /// Parent path whose siblings are currently shown
    pub open_dropdown_parent: Option<FilesPath>,

    // Omnibar/Suggestions fields:
    #[serde(default = "default_omnibar_mode")]
    pub omnibar_mode: OmnibarMode,
    #[serde(default)]
    pub search_query: String,
    #[serde(default)]
    pub command_query: String,
    #[serde(default)]
    pub show_suggestions: bool,
    #[serde(skip)]
    pub suggestions: Vec<Suggestion>,
    #[serde(default)]
    pub selected_suggestion_index: Option<usize>,
    /// Scroll position of the suggestions dropdown (auto-scroll follows keyboard nav).
    #[serde(skip)]
    pub suggestions_scroll: gpui::ScrollHandle,

    // Rich text editing, inline predictive ghost text & search options:
    /// Character index of the caret inside the active input string
    #[serde(skip)]
    pub cursor_pos: usize,
    /// Whether the entire input text is currently selected (e.g. on Ctrl+L / Ctrl+A)
    #[serde(skip)]
    pub selection_all: bool,
    /// Raw text typed by the user prior to arrow-key suggestion previewing
    #[serde(skip)]
    pub user_typed_input: String,
    /// FilePilot-style inline predictive completion suffix shown in muted ghost text after caret
    #[serde(skip)]
    pub ghost_prediction: Option<String>,
    /// Active search options (Regex, Case Sensitive, Content, Metadata, Subfolders)
    #[serde(default)]
    pub search_options: SearchOptions,
    /// Recently navigated paths for empty-query Path Mode suggestions
    #[serde(default)]
    pub path_history: Vec<String>,
    /// Persistent recent search queries
    #[serde(default = "default_search_history")]
    pub search_history: Vec<String>,
    /// Recently executed commands (boosted in Command Palette)
    #[serde(skip)]
    pub recent_commands: Vec<ActionKind>,

    // Breadcrumb sibling dropdown state:
    #[serde(default)]
    pub breadcrumb_dropdown_open: bool,
    #[serde(default)]
    pub breadcrumb_dropdown_parent: Option<FilesPath>,
    #[serde(skip)]
    pub breadcrumb_dropdown_siblings: Vec<FilesPath>,
    #[serde(default)]
    pub breadcrumb_dropdown_filter: String,
    #[serde(skip)]
    pub breadcrumb_dropdown_position: Option<(Pixels, Pixels)>,
    #[serde(default)]
    pub breadcrumb_dropdown_selected: Option<usize>,
    /// Whether the breadcrumb dropdown is showing root Quick Access & Drives (`true`) vs sibling folders (`false`)
    #[serde(skip)]
    pub breadcrumb_dropdown_is_root: bool,
    /// Breadcrumb segment currently hovered during a file drag-and-drop operation
    #[serde(skip)]
    pub file_drag_over_crumb: Option<FilesPath>,
    /// Back/Forward navigation history flyout: `Some((is_back, anchor_x))`
    #[serde(skip)]
    pub history_flyout: Option<(bool, f32)>,

    // Smooth 60-240Hz animation state:
    #[serde(skip)]
    pub focus_anim: f32,
    #[serde(skip)]
    pub dropdown_anim: f32,
    #[serde(skip)]
    pub mode_pill_pos: f32,
    #[serde(skip)]
    pub search_progress_phase: f32,
}

fn default_omnibar_mode() -> OmnibarMode {
    OmnibarMode::Path
}

fn default_search_history() -> Vec<String> {
    vec![
        "*.rs;*.toml".to_string(),
        "kind:image".to_string(),
        "size:>10mb".to_string(),
        "date:today".to_string(),
    ]
}

impl NavigationBarState {
    pub fn new(root: FilesPath) -> Self {
        let ancestors = root.ancestors();
        let initial_text = root.to_string_lossy().to_string();
        let cursor_pos = initial_text.chars().count();
        Self {
            current_path: root.clone(),
            breadcrumbs: ancestors,
            edit_mode: false,
            edit_text: initial_text.clone(),
            siblings: Vec::new(),
            open_dropdown_parent: None,
            omnibar_mode: OmnibarMode::Path,
            search_query: String::new(),
            command_query: String::new(),
            show_suggestions: false,
            suggestions: Vec::new(),
            selected_suggestion_index: None,
            suggestions_scroll: gpui::ScrollHandle::new(),
            cursor_pos,
            selection_all: false,
            user_typed_input: initial_text.clone(),
            ghost_prediction: None,
            search_options: SearchOptions::default(),
            path_history: vec![initial_text],
            search_history: default_search_history(),
            recent_commands: Vec::new(),
            breadcrumb_dropdown_open: false,
            breadcrumb_dropdown_parent: None,
            breadcrumb_dropdown_siblings: Vec::new(),
            breadcrumb_dropdown_filter: String::new(),
            breadcrumb_dropdown_position: None,
            breadcrumb_dropdown_selected: None,
            breadcrumb_dropdown_is_root: false,
            file_drag_over_crumb: None,
            history_flyout: None,
            focus_anim: 0.0,
            dropdown_anim: 0.0,
            mode_pill_pos: 0.0,
            search_progress_phase: 0.0,
        }
    }

    pub fn is_focused(&self) -> bool {
        self.edit_mode
            || matches!(
                self.omnibar_mode,
                OmnibarMode::Search | OmnibarMode::CommandPalette
            )
    }

    pub fn update_path(&mut self, path: FilesPath) {
        let path_str = path.to_string_lossy().to_string();
        if !path_str.is_empty() && path_str != "Settings" {
            self.record_path_history(&path_str);
        }
        self.current_path = path.clone();
        self.breadcrumbs = path.ancestors();
        self.edit_text = path_str.clone();
        self.user_typed_input = path_str;
        self.cursor_pos = self.edit_text.chars().count();
        self.selection_all = false;
        self.ghost_prediction = None;
        self.edit_mode = false;
        self.siblings.clear();
        self.open_dropdown_parent = None;
        self.omnibar_mode = OmnibarMode::Path;
        self.search_query.clear();
        self.command_query.clear();
        self.show_suggestions = false;
        self.suggestions.clear();
        self.selected_suggestion_index = None;
        self.breadcrumb_dropdown_open = false;
        self.breadcrumb_dropdown_parent = None;
        self.breadcrumb_dropdown_siblings.clear();
        self.breadcrumb_dropdown_filter.clear();
        self.breadcrumb_dropdown_position = None;
        self.breadcrumb_dropdown_selected = None;
        self.breadcrumb_dropdown_is_root = false;
        self.file_drag_over_crumb = None;
        self.history_flyout = None;
    }

    pub fn record_path_history(&mut self, path_str: &str) {
        let trimmed = path_str.trim();
        if trimmed.is_empty() {
            return;
        }
        self.path_history
            .retain(|p| !p.eq_ignore_ascii_case(trimmed));
        self.path_history.insert(0, trimmed.to_string());
        if self.path_history.len() > 15 {
            self.path_history.truncate(15);
        }
    }

    pub fn record_search_history(&mut self, query: &str) {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return;
        }
        self.search_history
            .retain(|q| !q.eq_ignore_ascii_case(trimmed));
        self.search_history.insert(0, trimmed.to_string());
        if self.search_history.len() > 15 {
            self.search_history.truncate(15);
        }
    }

    pub fn remove_search_history_item(&mut self, query: &str) {
        self.search_history
            .retain(|q| !q.eq_ignore_ascii_case(query));
    }

    pub fn clear_search_history(&mut self) {
        self.search_history.clear();
    }

    pub fn record_command_used(&mut self, action: ActionKind) {
        self.recent_commands.retain(|a| *a != action);
        self.recent_commands.insert(0, action);
        if self.recent_commands.len() > 8 {
            self.recent_commands.truncate(8);
        }
    }

    // ─── Active Text & Caret Manipulation ──────────────────────────────────

    pub fn active_text(&self) -> &str {
        match self.omnibar_mode {
            OmnibarMode::Path => &self.edit_text,
            OmnibarMode::Search => &self.search_query,
            OmnibarMode::CommandPalette => &self.command_query,
        }
    }

    pub fn set_active_text(&mut self, text: String, select_all: bool) {
        let count = text.chars().count();
        match self.omnibar_mode {
            OmnibarMode::Path => self.edit_text = text.clone(),
            OmnibarMode::Search => self.search_query = text.clone(),
            OmnibarMode::CommandPalette => self.command_query = text.clone(),
        }
        self.user_typed_input = text;
        self.cursor_pos = count;
        self.selection_all = select_all;
    }

    /// Preview a suggestion in the input box when cycling with Up/Down arrows
    /// without overwriting `user_typed_input`.
    pub fn preview_suggestion_text(&mut self, text: String) {
        let count = text.chars().count();
        match self.omnibar_mode {
            OmnibarMode::Path => self.edit_text = text,
            OmnibarMode::Search => self.search_query = text,
            OmnibarMode::CommandPalette => {}
        }
        self.cursor_pos = count;
        self.selection_all = false;
        self.ghost_prediction = None;
    }

    /// Revert input box text back to `user_typed_input` (when arrowing past ends or pressing Escape).
    pub fn revert_to_user_input(&mut self) {
        let saved = self.user_typed_input.clone();
        let count = saved.chars().count();
        match self.omnibar_mode {
            OmnibarMode::Path => self.edit_text = saved,
            OmnibarMode::Search => self.search_query = saved,
            OmnibarMode::CommandPalette => self.command_query = saved,
        }
        self.cursor_pos = count;
        self.selection_all = false;
        self.selected_suggestion_index = None;
    }

    pub fn select_all(&mut self) {
        let len = self.active_text().chars().count();
        self.cursor_pos = len;
        self.selection_all = len > 0;
    }

    pub fn insert_text(&mut self, s: &str) {
        if self.selection_all {
            self.set_active_text(String::new(), false);
        }
        let mut chars: Vec<char> = self.active_text().chars().collect();
        let pos = self.cursor_pos.min(chars.len());
        let insert_chars: Vec<char> = s.chars().collect();
        let insert_len = insert_chars.len();
        chars.splice(pos..pos, insert_chars);
        let updated: String = chars.into_iter().collect();
        match self.omnibar_mode {
            OmnibarMode::Path => self.edit_text = updated.clone(),
            OmnibarMode::Search => self.search_query = updated.clone(),
            OmnibarMode::CommandPalette => self.command_query = updated.clone(),
        }
        self.user_typed_input = updated;
        self.cursor_pos = pos + insert_len;
        self.selection_all = false;
    }

    pub fn backspace(&mut self, word: bool) {
        if self.selection_all {
            self.set_active_text(String::new(), false);
            return;
        }
        let mut chars: Vec<char> = self.active_text().chars().collect();
        let pos = self.cursor_pos.min(chars.len());
        if pos == 0 {
            return;
        }
        let remove_start = if word {
            let mut i = pos;
            while i > 0 && chars[i - 1].is_whitespace() {
                i -= 1;
            }
            while i > 0
                && !chars[i - 1].is_whitespace()
                && !matches!(chars[i - 1], '\\' | '/' | ':' | '.' | '-' | '_')
            {
                i -= 1;
            }
            if i == pos && i > 0 {
                i - 1
            } else {
                i
            }
        } else {
            pos - 1
        };
        chars.drain(remove_start..pos);
        let updated: String = chars.into_iter().collect();
        match self.omnibar_mode {
            OmnibarMode::Path => self.edit_text = updated.clone(),
            OmnibarMode::Search => self.search_query = updated.clone(),
            OmnibarMode::CommandPalette => self.command_query = updated.clone(),
        }
        self.user_typed_input = updated;
        self.cursor_pos = remove_start;
    }

    pub fn delete_forward(&mut self, word: bool) {
        if self.selection_all {
            self.set_active_text(String::new(), false);
            return;
        }
        let mut chars: Vec<char> = self.active_text().chars().collect();
        let pos = self.cursor_pos.min(chars.len());
        if pos >= chars.len() {
            return;
        }
        let remove_end = if word {
            let mut i = pos;
            while i < chars.len() && chars[i].is_whitespace() {
                i += 1;
            }
            while i < chars.len()
                && !chars[i].is_whitespace()
                && !matches!(chars[i], '\\' | '/' | ':' | '.' | '-' | '_')
            {
                i += 1;
            }
            if i == pos {
                (pos + 1).min(chars.len())
            } else {
                i
            }
        } else {
            pos + 1
        };
        chars.drain(pos..remove_end);
        let updated: String = chars.into_iter().collect();
        match self.omnibar_mode {
            OmnibarMode::Path => self.edit_text = updated.clone(),
            OmnibarMode::Search => self.search_query = updated.clone(),
            OmnibarMode::CommandPalette => self.command_query = updated.clone(),
        }
        self.user_typed_input = updated;
    }

    pub fn move_cursor_left(&mut self, word: bool) {
        if self.selection_all {
            self.selection_all = false;
            self.cursor_pos = 0;
            return;
        }
        let chars: Vec<char> = self.active_text().chars().collect();
        let pos = self.cursor_pos.min(chars.len());
        if pos == 0 {
            return;
        }
        if word {
            let mut i = pos;
            while i > 0 && chars[i - 1].is_whitespace() {
                i -= 1;
            }
            while i > 0
                && !chars[i - 1].is_whitespace()
                && !matches!(chars[i - 1], '\\' | '/' | ':' | '.' | '-' | '_')
            {
                i -= 1;
            }
            self.cursor_pos = if i == pos { pos - 1 } else { i };
        } else {
            self.cursor_pos = pos - 1;
        }
    }

    /// Move cursor right. If cursor is already at the end and `ghost_prediction` is available,
    /// accepts the inline ghost prediction and returns `true`!
    pub fn move_cursor_right(&mut self, word: bool) -> bool {
        let chars: Vec<char> = self.active_text().chars().collect();
        if self.selection_all {
            self.selection_all = false;
            self.cursor_pos = chars.len();
            return false;
        }
        let pos = self.cursor_pos.min(chars.len());
        if pos >= chars.len() {
            if self.ghost_prediction.is_some() {
                return self.accept_ghost_prediction();
            }
            return false;
        }
        if word {
            let mut i = pos;
            while i < chars.len() && chars[i].is_whitespace() {
                i += 1;
            }
            while i < chars.len()
                && !chars[i].is_whitespace()
                && !matches!(chars[i], '\\' | '/' | ':' | '.' | '-' | '_')
            {
                i += 1;
            }
            self.cursor_pos = if i == pos {
                (pos + 1).min(chars.len())
            } else {
                i
            };
        } else {
            self.cursor_pos = pos + 1;
        }
        false
    }

    pub fn move_cursor_home(&mut self) {
        self.selection_all = false;
        self.cursor_pos = 0;
    }

    pub fn move_cursor_end(&mut self) {
        self.selection_all = false;
        self.cursor_pos = self.active_text().chars().count();
    }

    /// Accept the FilePilot-style inline predictive ghost completion.
    pub fn accept_ghost_prediction(&mut self) -> bool {
        if let Some(ghost) = self.ghost_prediction.take() {
            if !ghost.is_empty() {
                let mut current = self.active_text().to_string();
                current.push_str(&ghost);
                self.set_active_text(current, false);
                return true;
            }
        }
        false
    }

    /// Tick smooth 60-240Hz animations for the Omnibar focus state, mode switcher pill,
    /// dropdown reveal, and active search progress shimmer.
    pub fn tick_animations(&mut self, dt: f32, is_search_loading: bool) -> bool {
        let mut active = false;
        let lerp_fast = 1.0 - (-22.0 * dt).exp();

        let target_focus = if self.is_focused() { 1.0f32 } else { 0.0f32 };
        if (target_focus - self.focus_anim).abs() > 0.005 {
            self.focus_anim += (target_focus - self.focus_anim) * lerp_fast;
            active = true;
        } else {
            self.focus_anim = target_focus;
        }

        let target_drop = if self.show_suggestions && !self.suggestions.is_empty() {
            1.0f32
        } else {
            0.0f32
        };
        if (target_drop - self.dropdown_anim).abs() > 0.005 {
            self.dropdown_anim += (target_drop - self.dropdown_anim) * lerp_fast;
            active = true;
        } else {
            self.dropdown_anim = target_drop;
        }

        let target_mode_pos = match self.omnibar_mode {
            OmnibarMode::Path => 0.0f32,
            OmnibarMode::CommandPalette => 1.0f32,
            OmnibarMode::Search => 2.0f32,
        };
        if (target_mode_pos - self.mode_pill_pos).abs() > 0.005 {
            self.mode_pill_pos += (target_mode_pos - self.mode_pill_pos) * lerp_fast;
            active = true;
        } else {
            self.mode_pill_pos = target_mode_pos;
        }

        if is_search_loading {
            self.search_progress_phase = (self.search_progress_phase + dt * 1.8) % 1.0;
            active = true;
        } else {
            self.search_progress_phase = 0.0;
        }

        active
    }

    /// Load sibling folders for a given parent path.
    /// Scroll the suggestions dropdown so `index` is fully visible.
    /// Rows are 34px (single-line) or 44px (two-line + snippet) plus 2px
    /// margins, with 4px list padding; the list viewport caps at 412px.
    pub fn ensure_suggestion_visible(&self, index: usize) {
        if self.suggestions.is_empty() {
            return;
        }
        let idx = index.min(self.suggestions.len() - 1);
        let pitch = |s: &Suggestion| -> f32 {
            if !s.subtitle.is_empty() || s.snippet.is_some() {
                46.0
            } else {
                36.0
            }
        };
        let mut top = 4.0f32;
        for s in &self.suggestions[..idx] {
            top += pitch(s);
        }
        let bottom = top + pitch(&self.suggestions[idx]);
        let viewport = 412.0;
        let total: f32 = 8.0 + self.suggestions.iter().map(pitch).sum::<f32>();
        let max = (total - 420.0).max(0.0);
        let cur = (-f32::from(self.suggestions_scroll.offset().y)).max(0.0);
        let mut new = cur;
        if top < cur {
            new = top;
        } else if bottom > cur + viewport {
            new = bottom - viewport;
        }
        let new = new.clamp(0.0, max);
        if (new - cur).abs() > 0.5 {
            self.suggestions_scroll
                .set_offset(gpui::point(gpui::px(0.0), gpui::px(-new)));
        }
    }

    pub fn load_siblings(&mut self, parent_path: &FilesPath) {
        self.siblings.clear();
        if let Ok(entries) = std::fs::read_dir(parent_path.as_path()) {
            for entry in entries.filter_map(|e| e.ok()) {
                if let Ok(metadata) = entry.metadata() {
                    if metadata.is_dir() {
                        self.siblings.push(FilesPath::from(entry.path()));
                    }
                }
            }
        }
        self.siblings.sort_by(|a, b| {
            let name_a = a
                .as_path()
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_lowercase();
            let name_b = b
                .as_path()
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_lowercase();
            name_a.cmp(&name_b)
        });
    }

    /// Update suggestions and inline ghost prediction across Path, Search, and Command Palette modes.
    pub fn update_suggestions(&mut self, registry: &ActionRegistry) {
        self.update_suggestions_with_context(registry, &[], &HashMap::new());
    }

    /// Context-aware suggestion generator that also searches current directory items,
    /// deep search results, tags, and AQS filter completions.
    pub fn update_suggestions_with_context(
        &mut self,
        registry: &ActionRegistry,
        current_items: &[FileItem],
        tags_cache: &HashMap<String, Vec<String>>,
    ) {
        self.suggestions.clear();
        self.ghost_prediction = None;

        match self.omnibar_mode {
            OmnibarMode::Path => {
                let query = self.edit_text.trim();
                let (ghost, candidates) = compute_path_completions(
                    query,
                    &self.current_path,
                    &self.path_history,
                    self.search_options.include_hidden,
                    12,
                );
                if !self.selection_all && self.cursor_pos >= self.edit_text.chars().count() {
                    self.ghost_prediction = ghost;
                }

                for cand in candidates {
                    let icon = if cand.is_dir {
                        "\u{E8B7}".to_string()
                    } else {
                        "\u{E160}".to_string()
                    };
                    self.suggestions.push(Suggestion {
                        kind: SuggestionKind::Path { path: cand.path },
                        label: cand.display_label,
                        icon,
                        subtitle: cand.subtitle.clone(),
                        badge: if cand.is_lookahead {
                            Some("Lookahead ↳".to_string())
                        } else if cand.is_dir {
                            Some("Folder".to_string())
                        } else {
                            Some(cand.subtitle)
                        },
                        snippet: None,
                        match_indices: cand.match_indices,
                        score: cand.score,
                    });
                }
            }
            OmnibarMode::Search => {
                let query = self.search_query.trim().to_string();
                let query_lower = query.to_lowercase();

                if query.is_empty() {
                    // 1. Recent search queries
                    for hist in self.search_history.iter().take(6) {
                        self.suggestions.push(Suggestion {
                            kind: SuggestionKind::SearchHistory {
                                query: hist.clone(),
                            },
                            label: hist.clone(),
                            icon: "\u{E81C}".to_string(), // History icon
                            subtitle: "Recent search".to_string(),
                            badge: Some("History".to_string()),
                            snippet: None,
                            match_indices: Vec::new(),
                            score: 900,
                        });
                    }
                    // 2. Quick AQS / Pattern syntax helpers
                    for (token, desc) in [
                        ("kind:image ", "Filter photos & images (PNG, JPG, WEBP, SVG)"),
                        ("kind:document ", "Filter documents, PDFs, spreadsheets & notes"),
                        ("kind:code ", "Filter source code & config files"),
                        ("size:>10mb ", "Files larger than 10 MB"),
                        ("date:today ", "Files modified today"),
                        ("*.rs;*.toml", "Windows multi-pattern wildcard search"),
                        ("re:^report_\\d+", "Regular expression filename/content match"),
                        ("content:", "Full-text search inside file contents"),
                        ("meta:", "Full-text search across EXIF, ID3, PDF & tags"),
                    ] {
                        self.suggestions.push(Suggestion {
                            kind: SuggestionKind::AqsFilter {
                                token: token.to_string(),
                            },
                            label: token.trim().to_string(),
                            icon: "\u{E71C}".to_string(), // Filter icon
                            subtitle: desc.to_string(),
                            badge: Some("Filter syntax".to_string()),
                            snippet: None,
                            match_indices: Vec::new(),
                            score: 700,
                        });
                    }
                } else {
                    // Check if the last token is a partial AQS prefix (e.g. `kind:im`, `size:>`, `date:t`)
                    let last_token = query.split_whitespace().last().unwrap_or("");
                    let last_lower = last_token.to_lowercase();
                    for (syntax, desc) in [
                        ("kind:image", "Photos & images"),
                        ("kind:video", "Videos & movies"),
                        ("kind:audio", "Music & audio tracks"),
                        ("kind:document", "Documents & text files"),
                        ("kind:pdf", "PDF documents"),
                        ("kind:code", "Source code files"),
                        ("kind:archive", "ZIP, 7z, TAR, RAR archives"),
                        ("kind:folder", "Folders & directories only"),
                        ("size:empty", "Empty files (0 B)"),
                        ("size:small", "Small files (16 KB – 1 MB)"),
                        ("size:medium", "Medium files (1 MB – 128 MB)"),
                        ("size:large", "Large files (128 MB – 1 GB)"),
                        ("size:huge", "Huge files (> 1 GB)"),
                        ("date:today", "Modified today"),
                        ("date:yesterday", "Modified yesterday"),
                        ("date:thisweek", "Modified within the last 7 days"),
                        ("date:thismonth", "Modified within the last 30 days"),
                        ("is:hidden", "Hidden or dot-prefixed items"),
                        ("is:readonly", "Read-only items"),
                    ] {
                        if syntax.starts_with(&last_lower) && syntax != last_lower && last_lower.contains(':') {
                            if self.ghost_prediction.is_none()
                                && self.cursor_pos >= self.search_query.chars().count()
                            {
                                let suffix = &syntax[last_lower.len()..];
                                self.ghost_prediction = Some(format!("{} ", suffix));
                            }
                            self.suggestions.push(Suggestion {
                                kind: SuggestionKind::AqsFilter {
                                    token: format!("{} ", syntax),
                                },
                                label: syntax.to_string(),
                                icon: "\u{E71C}".to_string(),
                                subtitle: desc.to_string(),
                                badge: Some("AQS Filter".to_string()),
                                snippet: None,
                                match_indices: (0..last_lower.len()).collect(),
                                score: 1950,
                            });
                        }
                    }

                    // Primary "Submit deep search" row
                    let mut mode_badges = Vec::new();
                    if self.search_options.use_regex {
                        mode_badges.push("Regex");
                    }
                    if self.search_options.case_sensitive {
                        mode_badges.push("Case");
                    }
                    if self.search_options.search_content {
                        mode_badges.push("Content");
                    }
                    if self.search_options.search_metadata {
                        mode_badges.push("Metadata");
                    }
                    if self.search_options.recursive {
                        mode_badges.push("Subfolders");
                    }
                    let badge_str = if mode_badges.is_empty() {
                        "Current folder".to_string()
                    } else {
                        mode_badges.join(" + ")
                    };

                    self.suggestions.push(Suggestion {
                        kind: SuggestionKind::SearchHistory {
                            query: query.clone(),
                        },
                        label: format!("Search for \"{}\"", query),
                        icon: "\u{E721}".to_string(),
                        subtitle: format!(
                            "in {}",
                            self.current_path.to_string_lossy()
                        ),
                        badge: Some(badge_str),
                        snippet: None,
                        match_indices: Vec::new(),
                        score: 2100,
                    });

                    // Evaluate live current-folder / deep-search items for instant predictive results
                    let mut scored_items: Vec<Suggestion> = Vec::new();
                    for item in current_items.iter().take(400) {
                        if let Some((score, indices, snippet)) = SearchService::evaluate_file_item(
                            item,
                            &query,
                            &self.search_options,
                            tags_cache,
                        ) {
                            let is_dir = matches!(item.file_type, FileType::Directory);
                            if self.ghost_prediction.is_none()
                                && !self.selection_all
                                && self.cursor_pos >= self.search_query.chars().count()
                                && item.name.to_lowercase().starts_with(&query_lower)
                            {
                                let q_chars = query.chars().count();
                                let suffix: String = item.name.chars().skip(q_chars).collect();
                                if !suffix.is_empty() {
                                    self.ghost_prediction = Some(suffix);
                                }
                            }
                            let parent_str = item
                                .path
                                .parent()
                                .map(|p| p.to_string_lossy().to_string())
                                .unwrap_or_default();
                            let size_badge = if is_dir {
                                "Folder".to_string()
                            } else {
                                crate::ui::views::helpers::format_size(item.size)
                            };
                            scored_items.push(Suggestion {
                                kind: SuggestionKind::SearchItem {
                                    path: item.path.clone(),
                                    is_dir,
                                },
                                label: item.name.clone(),
                                icon: if is_dir {
                                    "\u{E8B7}".to_string()
                                } else {
                                    "\u{E160}".to_string()
                                },
                                subtitle: parent_str,
                                badge: Some(size_badge),
                                snippet,
                                match_indices: indices,
                                score,
                            });
                        }
                    }

                    scored_items.sort_by(|a, b| b.score.cmp(&a.score));
                    for item_sug in scored_items.into_iter().take(9) {
                        self.suggestions.push(item_sug);
                    }

                    // Matching search history items
                    for hist in &self.search_history {
                        if hist.to_lowercase().contains(&query_lower)
                            && !hist.eq_ignore_ascii_case(&query)
                            && self.suggestions.len() < 12
                        {
                            let indices = score_fuzzy_or_prefix(
                                hist,
                                &hist.to_lowercase(),
                                &query,
                                &query_lower,
                            )
                            .map(|(_, idx)| idx)
                            .unwrap_or_default();
                            self.suggestions.push(Suggestion {
                                kind: SuggestionKind::SearchHistory {
                                    query: hist.clone(),
                                },
                                label: hist.clone(),
                                icon: "\u{E81C}".to_string(),
                                subtitle: "Recent search".to_string(),
                                badge: Some("History".to_string()),
                                snippet: None,
                                match_indices: indices,
                                score: 600,
                            });
                        }
                    }
                }
            }
            OmnibarMode::CommandPalette => {
                let raw_q = self.command_query.trim().trim_start_matches('>').trim();
                let query_lower = raw_q.to_lowercase();
                let mut cmd_candidates: Vec<Suggestion> = Vec::new();

                for (action, combo) in registry.iter() {
                    if matches!(action, ActionKind::CommandPalette) {
                        continue;
                    }
                    let label = action.label();
                    let label_lower = label.to_lowercase();
                    let category = Self::command_category(action);
                    let cat_lower = category.to_lowercase();
                    let combo_str = combo.to_string();

                    let matched = if query_lower.is_empty() {
                        let recent_bonus = self
                            .recent_commands
                            .iter()
                            .position(|a| *a == action)
                            .map(|pos| 500u32.saturating_sub(pos as u32 * 50))
                            .unwrap_or(0);
                        Some((1000 + recent_bonus, Vec::new()))
                    } else if let Some((score, indices)) =
                        score_fuzzy_or_prefix(label, &label_lower, raw_q, &query_lower)
                    {
                        Some((score, indices))
                    } else if cat_lower.contains(&query_lower)
                        || combo_str.to_lowercase().contains(&query_lower)
                    {
                        Some((750, Vec::new()))
                    } else {
                        None
                    };

                    if let Some((score, indices)) = matched {
                        cmd_candidates.push(Suggestion {
                            kind: SuggestionKind::Command { action },
                            label: label.to_string(),
                            icon: Self::command_icon(action).to_string(),
                            subtitle: category.to_string(),
                            badge: Some(combo_str),
                            snippet: None,
                            match_indices: indices,
                            score,
                        });
                    }
                }

                cmd_candidates.sort_by(|a, b| {
                    b.score
                        .cmp(&a.score)
                        .then_with(|| a.label.cmp(&b.label))
                });

                if !query_lower.is_empty() && !self.selection_all {
                    if let Some(first) = cmd_candidates.first() {
                        if first.label.to_lowercase().starts_with(&query_lower)
                            && self.cursor_pos >= self.command_query.chars().count()
                        {
                            let q_chars = raw_q.chars().count();
                            let suffix: String = first.label.chars().skip(q_chars).collect();
                            if !suffix.is_empty() {
                                self.ghost_prediction = Some(suffix);
                            }
                        }
                    }
                }

                cmd_candidates.truncate(14);
                self.suggestions = cmd_candidates;
            }
        }

        // Auto-select first suggestion in CommandPalette or Path mode; keep valid index otherwise
        if self.suggestions.is_empty() {
            self.selected_suggestion_index = None;
        } else if let Some(idx) = self.selected_suggestion_index {
            if idx >= self.suggestions.len() {
                self.selected_suggestion_index = Some(self.suggestions.len() - 1);
            }
        } else if matches!(self.omnibar_mode, OmnibarMode::CommandPalette) {
            self.selected_suggestion_index = Some(0);
        }
        self
            .suggestions_scroll
            .set_offset(gpui::point(gpui::px(0.0), gpui::px(0.0)));
    }

    pub fn command_category(action: ActionKind) -> &'static str {
        action.category().label()
    }

    pub fn command_icon(action: ActionKind) -> &'static str {
        action.icon()
    }

    /// Open the breadcrumb sibling dropdown at a given screen position.
    pub fn open_breadcrumb_dropdown(&mut self, parent_path: FilesPath, position: (Pixels, Pixels)) {
        self.breadcrumb_dropdown_siblings.clear();
        self.breadcrumb_dropdown_is_root = false;
        if let Ok(entries) = std::fs::read_dir(parent_path.as_path()) {
            for entry in entries.filter_map(|e| e.ok()) {
                if let Ok(metadata) = entry.metadata() {
                    if metadata.is_dir() {
                        self.breadcrumb_dropdown_siblings
                            .push(FilesPath::from(entry.path()));
                    }
                }
            }
        }
        self.breadcrumb_dropdown_siblings.sort_by(|a, b| {
            let name_a = a
                .as_path()
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_lowercase();
            let name_b = b
                .as_path()
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_lowercase();
            name_a.cmp(&name_b)
        });
        self.breadcrumb_dropdown_open = true;
        self.breadcrumb_dropdown_parent = Some(parent_path);
        self.breadcrumb_dropdown_filter.clear();
        self.breadcrumb_dropdown_selected = if self.breadcrumb_dropdown_siblings.is_empty() {
            None
        } else {
            Some(0)
        };
        self.breadcrumb_dropdown_position = Some(position);
    }

    /// Open the root Quick Access & Drives flyout from the Home chevron (`Files-community` parity).
    pub fn open_root_breadcrumb_dropdown(&mut self, position: (Pixels, Pixels)) {
        self.breadcrumb_dropdown_siblings.clear();
        self.breadcrumb_dropdown_is_root = true;

        for opt_dir in [
            dirs::home_dir(),
            dirs::desktop_dir(),
            dirs::download_dir(),
            dirs::document_dir(),
            dirs::picture_dir(),
            dirs::audio_dir(),
            dirs::video_dir(),
        ]
        .into_iter()
        .flatten()
        {
            if opt_dir.exists() {
                self.breadcrumb_dropdown_siblings
                    .push(FilesPath::from(opt_dir));
            }
        }

        #[cfg(windows)]
        {
            for letter in b'A'..=b'Z' {
                let drive = format!("{}:\\", letter as char);
                let p = std::path::PathBuf::from(&drive);
                if p.exists() {
                    self.breadcrumb_dropdown_siblings.push(FilesPath::from(p));
                }
            }
        }

        self.breadcrumb_dropdown_open = true;
        self.breadcrumb_dropdown_parent = Some(FilesPath::new("Home"));
        self.breadcrumb_dropdown_filter.clear();
        self.breadcrumb_dropdown_selected = if self.breadcrumb_dropdown_siblings.is_empty() {
            None
        } else {
            Some(0)
        };
        self.breadcrumb_dropdown_position = Some(position);
    }

    /// Close the breadcrumb sibling dropdown.
    pub fn close_breadcrumb_dropdown(&mut self) {
        self.breadcrumb_dropdown_open = false;
        self.breadcrumb_dropdown_parent = None;
        self.breadcrumb_dropdown_siblings.clear();
        self.breadcrumb_dropdown_filter.clear();
        self.breadcrumb_dropdown_position = None;
        self.breadcrumb_dropdown_selected = None;
        self.breadcrumb_dropdown_is_root = false;
    }

    /// Update the filter text for the breadcrumb dropdown.
    pub fn update_breadcrumb_filter(&mut self, filter: String) {
        self.breadcrumb_dropdown_filter = filter;
        let visible_count = self.breadcrumb_dropdown_filtered_count();
        self.breadcrumb_dropdown_selected = if visible_count == 0 { None } else { Some(0) };
    }

    /// Get the number of siblings visible after filtering.
    pub fn breadcrumb_dropdown_filtered_count(&self) -> usize {
        if self.breadcrumb_dropdown_filter.is_empty() {
            return self.breadcrumb_dropdown_siblings.len();
        }
        self.breadcrumb_dropdown_siblings
            .iter()
            .filter(|s| {
                let name = Self::crumb_display_name(s);
                fuzzy_match_indices(&name, &self.breadcrumb_dropdown_filter).is_some()
            })
            .count()
    }

    /// Get filtered siblings as (index_into_original, path) pairs.
    pub fn breadcrumb_dropdown_filtered_siblings(&self) -> Vec<(usize, &FilesPath)> {
        if self.breadcrumb_dropdown_filter.is_empty() {
            return self
                .breadcrumb_dropdown_siblings
                .iter()
                .enumerate()
                .collect();
        }
        self.breadcrumb_dropdown_siblings
            .iter()
            .enumerate()
            .filter(|(_, s)| {
                let name = Self::crumb_display_name(s);
                fuzzy_match_indices(&name, &self.breadcrumb_dropdown_filter).is_some()
            })
            .collect()
    }

    pub fn crumb_display_name(p: &FilesPath) -> String {
        let name = p
            .as_path()
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        if name.is_empty() {
            p.to_string_lossy().to_string()
        } else {
            name
        }
    }
}

pub struct NavigationBarView;

impl NavigationBarView {
    fn render_nav_btn(
        icon: &'static str,
        enabled: bool,
        on_click: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
        on_right_click: Option<Box<dyn Fn(&MouseDownEvent, &mut Window, &mut App) + 'static>>,
    ) -> impl IntoElement {
        let mut btn = div()
            .flex()
            .items_center()
            .justify_center()
            .w(px(36.0))
            .h(px(32.0))
            .rounded(px(6.0))
            .cursor_pointer()
            .text_color(if enabled {
                rgba(UiConstants::TEXT_PRIMARY_RGBA)
            } else {
                rgba(UiConstants::TEXT_MUTED_RGBA)
            });

        if enabled {
            btn = btn
                .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
                .on_mouse_down(MouseButton::Left, move |ev, win, cx| on_click(ev, win, cx));
            if let Some(rc) = on_right_click {
                btn = btn.on_mouse_down(MouseButton::Right, move |ev, win, cx| {
                    cx.stop_propagation();
                    rc(ev, win, cx);
                });
            }
        }

        btn.child(
            div()
                .text_size(px(14.0))
                .font_family(UiConstants::FONT_ICONS)
                .child(icon),
        )
    }

    fn render_nav_btn_with_tooltip(
        id: impl Into<ElementId>,
        icon: &'static str,
        tip_text: &'static str,
        tip_shortcut: Option<&'static str>,
        enabled: bool,
        view: Entity<crate::app::AppWindow>,
        on_click: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
        on_right_click: Option<Box<dyn Fn(&MouseDownEvent, &mut Window, &mut App) + 'static>>,
    ) -> impl IntoElement {
        let v_tip = view.clone();
        let v_click = view.clone();
        let text = tip_text.to_string();
        let shortcut = tip_shortcut.map(|s| s.to_string());

        let mut btn = div()
            .flex()
            .items_center()
            .justify_center()
            .w(px(36.0))
            .h(px(32.0))
            .rounded(px(6.0))
            .cursor_pointer()
            .text_color(if enabled {
                rgba(UiConstants::TEXT_PRIMARY_RGBA)
            } else {
                rgba(UiConstants::TEXT_MUTED_RGBA)
            });

        let stateful = btn.id(id);

        let final_btn = if enabled {
            let mut b = stateful
                .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
                .on_hover(move |hovering: &bool, window: &mut Window, cx: &mut App| {
                    let pos = window.mouse_position();
                    let t = text.clone();
                    let sc = shortcut.clone();
                    if *hovering {
                        v_tip.update(cx, |this, cx| {
                            this.start_button_tooltip(t, sc, pos, cx);
                        });
                    } else {
                        v_tip.update(cx, |this, cx| {
                            this.stop_button_tooltip(cx);
                        });
                    }
                })
                .on_mouse_down(MouseButton::Left, move |ev, win, cx| {
                    v_click.update(cx, |this, cx| {
                        this.stop_button_tooltip(cx);
                    });
                    on_click(ev, win, cx);
                });

            if let Some(rc) = on_right_click {
                b = b.on_mouse_down(MouseButton::Right, move |ev, win, cx| {
                    cx.stop_propagation();
                    rc(ev, win, cx);
                });
            }
            b
        } else {
            stateful
        };

        final_btn.child(
            div()
                .text_size(px(14.0))
                .font_family(UiConstants::FONT_ICONS)
                .child(icon),
        )
    }

    /// Files-community `OmnibarMode` pill button (30x28px rounded pill inside the 36px Omnibar).
    fn render_mode_icon_btn(
        icon: &'static str,
        is_active: bool,
        is_focused: bool,
        on_click: impl Fn(&mut Window, &mut App) + 'static,
    ) -> impl IntoElement {
        let (bg, text_color) = if is_active && is_focused {
            (rgba(0x0078D42Eu32), rgba(UiConstants::ACCENT_RGBA))
        } else if is_active {
            (rgba(0xFFFFFF10u32), rgba(UiConstants::TEXT_PRIMARY_RGBA))
        } else {
            (rgba(0x00000000u32), rgba(UiConstants::TEXT_MUTED_RGBA))
        };

        div()
            .flex()
            .items_center()
            .justify_center()
            .w(px(32.0))
            .h(px(28.0))
            .rounded(px(14.0))
            .bg(bg)
            .cursor_pointer()
            .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
            .on_mouse_down(MouseButton::Left, move |_ev, win, cx| {
                cx.stop_propagation();
                on_click(win, cx);
            })
            .child(
                div()
                    .text_size(px(13.0))
                    .font_family(UiConstants::FONT_ICONS)
                    .text_color(text_color)
                    .child(icon),
            )
    }

    /// Compact interactive Search Filter Toggle Pill inside the Omnibar (`.*`, `Aa`, `Txt`, `Meta`, `Sub`).
    fn render_search_option_pill(
        label: &'static str,
        active: bool,
        on_click: impl Fn(&mut Window, &mut App) + 'static,
    ) -> impl IntoElement {
        let (bg, border_c, text_c) = if active {
            (
                rgba(0x0078D438u32),
                rgba(UiConstants::ACCENT_RGBA),
                rgba(0xFFFFFFFAu32),
            )
        } else {
            (
                rgba(0xFFFFFF08u32),
                rgba(0xFFFFFF18u32),
                rgba(UiConstants::TEXT_SECONDARY_RGBA),
            )
        };

        div()
            .flex()
            .items_center()
            .justify_center()
            .px(px(6.0))
            .h(px(22.0))
            .rounded(px(11.0))
            .bg(bg)
            .border(px(1.0))
            .border_color(border_c)
            .cursor_pointer()
            .hover(|s| {
                s.bg(if active {
                    rgba(0x0078D450u32)
                } else {
                    rgba(0xFFFFFF16u32)
                })
            })
            .on_mouse_down(MouseButton::Left, move |_ev, win, cx| {
                cx.stop_propagation();
                on_click(win, cx);
            })
            .child(
                div()
                    .text_size(px(10.5))
                    .font_weight(if active {
                        FontWeight(600.0)
                    } else {
                        FontWeight(400.0)
                    })
                    .text_color(text_c)
                    .child(label),
            )
    }

    fn render_pill_separator() -> impl IntoElement {
        div()
            .w(px(1.0))
            .h(px(16.0))
            .mx(px(2.0))
            .bg(rgba(UiConstants::DIVIDER_RGBA))
    }

    /// Render the interactive text input with caret positioning, selection highlight (`Ctrl+A`),
    /// and FilePilot-style inline predictive ghost completion (`Tab ⇥`).
    fn render_text_input_with_ghost(
        text: &str,
        placeholder: &str,
        cursor_pos: usize,
        selection_all: bool,
        ghost_prediction: Option<&str>,
        prefix_badge: Option<&'static str>,
    ) -> impl IntoElement {
        let mut row = div()
            .flex()
            .items_center()
            .flex_1()
            .min_w(px(0.0))
            .h_full()
            .overflow_hidden();

        if let Some(pref) = prefix_badge {
            row = row.child(
                div()
                    .px(px(6.0))
                    .py(px(1.0))
                    .mr(px(6.0))
                    .rounded(px(4.0))
                    .bg(rgba(0x0078D42Au32))
                    .text_size(px(12.0))
                    .font_weight(FontWeight(700.0))
                    .text_color(rgba(UiConstants::ACCENT_RGBA))
                    .child(pref),
            );
        }

        if text.is_empty() {
            let caret = div()
                .w(px(1.5))
                .h(px(16.0))
                .bg(rgba(UiConstants::ACCENT_RGBA))
                .mr(px(2.0));
            return row.child(caret).child(
                div()
                    .text_size(px(13.0))
                    .text_color(rgba(UiConstants::TEXT_MUTED_RGBA))
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .child(placeholder.to_string()),
            );
        }

        if selection_all {
            return row
                .child(
                    div()
                        .px(px(3.0))
                        .rounded(px(3.0))
                        .bg(rgba(0x0078D466u32))
                        .text_size(px(13.0))
                        .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .child(text.to_string()),
                )
                .child(
                    div()
                        .w(px(1.5))
                        .h(px(16.0))
                        .bg(rgba(UiConstants::ACCENT_RGBA)),
                );
        }

        let chars: Vec<char> = text.chars().collect();
        let split = cursor_pos.min(chars.len());
        let before: String = chars[..split].iter().collect();
        let after: String = chars[split..].iter().collect();

        row = row.child(
            div()
                .text_size(px(13.0))
                .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                .whitespace_nowrap()
                .child(before),
        );
        row = row.child(
            div()
                .w(px(0.0))
                .h(px(16.0))
                .flex_shrink_0()
                .child(
                    div()
                        .absolute()
                        .left(px(-0.75))
                        .w(px(1.5))
                        .h(px(16.0))
                        .bg(rgba(UiConstants::ACCENT_RGBA)),
                ),
        );
        if !after.is_empty() {
            row = row.child(
                div()
                    .text_size(px(13.0))
                    .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                    .whitespace_nowrap()
                    .child(after),
            );
        } else if let Some(ghost) = ghost_prediction {
            if !ghost.is_empty() {
                row = row
                    .child(
                        div()
                            .text_size(px(13.0))
                            .text_color(rgba(0xFFFFFF50u32))
                            .whitespace_nowrap()
                            .child(ghost.to_string()),
                    )
                    .child(
                        div()
                            .ml(px(6.0))
                            .px(px(5.0))
                            .py(px(1.0))
                            .rounded(px(4.0))
                            .bg(rgba(0xFFFFFF10u32))
                            .border(px(1.0))
                            .border_color(rgba(0xFFFFFF1Eu32))
                            .text_size(px(10.0))
                            .text_color(rgba(UiConstants::TEXT_MUTED_RGBA))
                            .child("Tab ⇥"),
                    );
            }
        }

        row
    }

    /// Files-community `ShowStatusCenterButton` with live ProgressRing, InfoBadge state coloring,
    /// and unread / active job badge count.
    fn status_bell(
        status: &crate::ui::components::status_center::StatusCenterState,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let active_jobs = status.active_job_count();
        let badge_state = status.info_badge_state();
        let unread = status.unread_count;
        let prog = status.displayed_progress.clamp(0.0, 1.0);
        let is_open = status.is_open;

        // Ring & badge colors per Files-community `StatusIconStates` (NavigationToolbar.xaml lines 596-648)
        let (ring_border_color, badge_bg, badge_text) = match badge_state {
            1 => (
                rgba(UiConstants::ACCENT_RGBA),
                rgba(UiConstants::ACCENT_RGBA),
                Some(active_jobs.min(99).to_string()),
            ),
            2 => (
                rgba(0xF7B93EFFu32),
                rgba(0xE81123FFu32),
                Some(active_jobs.min(99).to_string()),
            ),
            3 => (
                rgba(0xE81123CCu32),
                rgba(0xE81123FFu32),
                Some("!".to_string()),
            ),
            _ => {
                if unread > 0 {
                    (
                        rgba(0x6CCB5FCCu32),
                        rgba(0x107C10FFu32),
                        Some(unread.min(99).to_string()),
                    )
                } else if !status.jobs.is_empty() {
                    (rgba(0x6CCB5F66u32), rgba(0x107C10FFu32), None)
                } else {
                    (rgba(0x00000000u32), rgba(0x00000000u32), None)
                }
            }
        };

        let mut btn = div()
            .relative()
            .flex()
            .items_center()
            .justify_center()
            .w(px(38.0))
            .h(px(32.0))
            .rounded(px(6.0))
            .cursor_pointer()
            .bg(if is_open {
                rgba(0xFFFFFF16u32)
            } else {
                rgba(0x00000000u32)
            })
            .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
            .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA));

        // Outer circular progress ring when jobs are active or recently completed
        if active_jobs > 0 || status.bell_pulse > 0.05 || badge_state == 3 {
            btn = btn.child(
                div()
                    .w(px(26.0))
                    .h(px(26.0))
                    .rounded(px(13.0))
                    .border(px(2.0))
                    .border_color(ring_border_color)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .text_size(px(13.0))
                            .font_family(UiConstants::FONT_ICONS)
                            .child("\u{E9D9}"),
                    ),
            );
        } else {
            btn = btn.child(
                div()
                    .text_size(px(15.0))
                    .font_family(UiConstants::FONT_ICONS)
                    .child("\u{E9D9}"),
            );
        }

        // Bottom mini progress bar strip on the button while transfers are running
        if active_jobs > 0 {
            btn = btn.child(
                div()
                    .absolute()
                    .bottom(px(2.0))
                    .left(px(5.0))
                    .w(px(28.0))
                    .h(px(2.5))
                    .rounded(px(1.5))
                    .bg(rgba(0xFFFFFF22u32))
                    .child(
                        div()
                            .h_full()
                            .w(px((28.0 * prog.max(0.08)).clamp(2.0, 28.0)))
                            .rounded(px(1.5))
                            .bg(rgba(UiConstants::ACCENT_RGBA)),
                    ),
            );
        }

        // InfoBadge pill (top-right)
        if let Some(label) = badge_text {
            btn = btn.child(
                div()
                    .absolute()
                    .top(px(1.0))
                    .right(px(1.0))
                    .min_w(px(15.0))
                    .h(px(15.0))
                    .px(px(3.5))
                    .rounded(px(7.5))
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(badge_bg)
                    .child(
                        div()
                            .text_size(px(9.5))
                            .font_weight(FontWeight(700.0))
                            .text_color(rgb(0xFFFFFF))
                            .child(label),
                    ),
            );
        }

        let v = view.clone();
        btn.on_mouse_down(MouseButton::Left, move |_, window: &mut Window, cx: &mut App| {
            cx.stop_propagation();
            v.update(cx, |this, cx| {
                this.toggle_status_center(window, cx);
            });
        })
    }

    pub fn build(
        state: &NavigationBarState,
        can_go_back: bool,
        can_go_forward: bool,
        _side_pane: crate::ui::views::types::SidePaneMode,
        view: Entity<crate::app::AppWindow>,
        _registry: &ActionRegistry,
        status: &crate::ui::components::status_center::StatusCenterState,
    ) -> impl IntoElement {
        let v_sidebar_toggle = view.clone();
        let v_back = view.clone();
        let v_back_rc = view.clone();
        let v_forward = view.clone();
        let v_forward_rc = view.clone();
        let v_up = view.clone();
        let v_refresh = view.clone();
        let v_bell = view.clone();

        let address_bar_pill = {
            let state_mode = state.omnibar_mode;
            let edit_mode = state.edit_mode;
            let is_focused = state.is_focused();

            // Files-community Omnibar styling (`Omnibar.xaml`):
            // Rounded pill (`CornerRadius = 18..19`), 36px height, `ControlFillColorDefaultBrush` + `CircleElevationBorderBrush`
            let mut pill = div()
                .flex()
                .items_center()
                .flex_grow()
                .min_w(px(120.0))
                .h(px(36.0))
                .rounded(px(18.0))
                .px(px(6.0))
                .relative()
                .overflow_hidden();

            if is_focused {
                pill = pill
                    .border(px(1.5))
                    .border_color(rgba(UiConstants::ACCENT_RGBA))
                    .bg(rgba(0x1E1E1EF2u32));
            } else {
                pill = pill
                    .border(px(1.0))
                    .border_color(rgba(UiConstants::BORDER_OMNIBAR_RGBA))
                    .bg(rgba(UiConstants::BG_BREADCRUMB_RGBA))
                    .hover(|s| s.bg(rgba(0xFFFFFF14u32)));
            }

            // Mode switcher handlers
            let v_path = view.clone();
            let on_path_click = move |_: &mut Window, cx: &mut App| {
                v_path.update(cx, |this, cx| {
                    this.activate_omnibar_path_mode(cx);
                });
            };

            let v_cmd = view.clone();
            let on_cmd_click = move |_: &mut Window, cx: &mut App| {
                v_cmd.update(cx, |this, cx| {
                    this.activate_omnibar_command_mode(cx);
                });
            };

            let v_search = view.clone();
            let on_search_click = move |_: &mut Window, cx: &mut App| {
                v_search.update(cx, |this, cx| {
                    this.activate_omnibar_search_mode(cx);
                });
            };

            let path_active = state_mode == OmnibarMode::Path;
            let cmd_active = state_mode == OmnibarMode::CommandPalette;
            let search_active = state_mode == OmnibarMode::Search;

            // Right-aligned Omnibar mode switcher cluster (`Omnibar.xaml` PART_ModesHostGrid: Path | Commands | Search)
            let mode_icons = div()
                .flex()
                .items_center()
                .flex_shrink_0()
                .child(Self::render_pill_separator())
                .child(Self::render_mode_icon_btn(
                    "\u{E8B7}", // Path mode
                    path_active,
                    is_focused,
                    on_path_click,
                ))
                .child(Self::render_pill_separator())
                .child(Self::render_mode_icon_btn(
                    "\u{E943}", // Command Palette mode (`>` / code/commands)
                    cmd_active,
                    is_focused,
                    on_cmd_click,
                ))
                .child(Self::render_pill_separator())
                .child(Self::render_mode_icon_btn(
                    "\u{E721}", // Search mode
                    search_active,
                    is_focused,
                    on_search_click,
                ));

            // Middle content area
            if state_mode == OmnibarMode::Path && !edit_mode {
                let v_pill = view.clone();
                let v_copy_path = view.clone();
                let mut breadcrumb_views: Vec<AnyElement> = Vec::new();

                // 1. Dedicated Home button + Root chevron (Quick Access & Drives flyout)
                let v_home = view.clone();
                let v_root_chev = view.clone();
                breadcrumb_views.push(
                    div()
                        .flex()
                        .items_center()
                        .flex_shrink_0()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .px(px(6.0))
                                .h(px(26.0))
                                .rounded(px(5.0))
                                .cursor_pointer()
                                .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
                                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                    cx.stop_propagation();
                                    v_home.update(cx, |this, cx| {
                                        if let Ok(home_path) =
                                            crate::core::navigation_service::NavigationService::get_home_path()
                                        {
                                            this.navigate(home_path, cx);
                                        }
                                    });
                                })
                                .child(
                                    div()
                                        .text_size(px(13.0))
                                        .font_family(UiConstants::FONT_ICONS)
                                        .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                        .child("\u{E80F}"),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .px(px(3.0))
                                .h(px(24.0))
                                .rounded(px(4.0))
                                .cursor_pointer()
                                .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
                                .on_mouse_down(MouseButton::Left, move |event, _, cx| {
                                    cx.stop_propagation();
                                    let mouse_x = event.position.x;
                                    v_root_chev.update(cx, |this, cx| {
                                        this.state
                                            .nav_bar
                                            .open_root_breadcrumb_dropdown((mouse_x, px(84.0)));
                                        cx.notify();
                                    });
                                })
                                .child(
                                    div()
                                        .text_size(px(11.0))
                                        .font_family(UiConstants::FONT_ICONS)
                                        .text_color(rgba(UiConstants::TEXT_MUTED_RGBA))
                                        .child("\u{E974}"),
                                ),
                        )
                        .into_any_element(),
                );

                // 2. Collapse leading segments into `•••` if breadcrumb trail has > 5 segments
                let total_crumbs = state.breadcrumbs.len();
                let start_idx = if total_crumbs > 5 {
                    let collapse_parent = state.breadcrumbs[total_crumbs - 5].clone();
                    let v_ellipsis = view.clone();
                    breadcrumb_views.push(
                        div()
                            .flex()
                            .items_center()
                            .flex_shrink_0()
                            .px(px(6.0))
                            .h(px(24.0))
                            .rounded(px(4.0))
                            .cursor_pointer()
                            .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
                            .on_mouse_down(MouseButton::Left, move |ev, _, cx| {
                                cx.stop_propagation();
                                let p = collapse_parent.clone();
                                v_ellipsis.update(cx, |this, cx| {
                                    this.state
                                        .nav_bar
                                        .open_breadcrumb_dropdown(p, (ev.position.x, px(84.0)));
                                    cx.notify();
                                });
                            })
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .font_weight(FontWeight(700.0))
                                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                    .child("•••"),
                            )
                            .into_any_element(),
                    );
                    total_crumbs - 4
                } else {
                    0
                };

                // 3. Render visible breadcrumb segments with Middle-Click (New Tab), Right-Click (Copy Path),
                //    and File-Drag-Over / Drop support!
                for i in start_idx..total_crumbs {
                    let crumb_path = &state.breadcrumbs[i];
                    let label = if i == 0 {
                        let s = crumb_path.to_string_lossy();
                        if s == "/" || s.is_empty() {
                            continue;
                        } else {
                            s.trim_end_matches(['\\', '/']).to_string()
                        }
                    } else {
                        crumb_path
                            .as_path()
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string()
                    };

                    if label.is_empty() {
                        continue;
                    }

                    if i > 0 {
                        let v_chevron = view.clone();
                        let parent_path = state.breadcrumbs[i - 1].clone();
                        breadcrumb_views.push(
                            div()
                                .flex()
                                .items_center()
                                .flex_shrink_0()
                                .px(px(3.0))
                                .h(px(24.0))
                                .rounded(px(4.0))
                                .cursor_pointer()
                                .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
                                .on_mouse_down(MouseButton::Left, move |event, _, cx| {
                                    cx.stop_propagation();
                                    let mouse_x = event.position.x;
                                    v_chevron.update(cx, |this, cx| {
                                        this.state.nav_bar.open_breadcrumb_dropdown(
                                            parent_path.clone(),
                                            (mouse_x, px(84.0)),
                                        );
                                        cx.notify();
                                    });
                                })
                                .child(
                                    div()
                                        .text_size(px(11.0))
                                        .font_family(UiConstants::FONT_ICONS)
                                        .text_color(rgba(UiConstants::TEXT_MUTED_RGBA))
                                        .child("\u{E974}"),
                                )
                                .into_any_element(),
                        );
                    }

                    let is_last = i == total_crumbs - 1;
                    let is_drop_target = state.file_drag_over_crumb.as_ref() == Some(crumb_path);
                    let path_left = crumb_path.clone();
                    let path_mid = crumb_path.clone();
                    let path_right = crumb_path.clone();
                    let path_hover = crumb_path.clone();
                    let path_drop = crumb_path.clone();
                    let v_left = view.clone();
                    let v_mid = view.clone();
                    let v_right = view.clone();
                    let v_hover = view.clone();
                    let v_drop = view.clone();

                    let mut seg = div()
                        .flex()
                        .items_center()
                        .flex_shrink_0()
                        .px(px(7.0))
                        .h(px(26.0))
                        .rounded(px(5.0))
                        .cursor_pointer()
                        .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
                        .on_mouse_down(MouseButton::Left, move |ev, _, cx| {
                            cx.stop_propagation();
                            let p = path_left.clone();
                            let ctrl = ev.modifiers.control;
                            v_left.update(cx, |this, cx| {
                                if ctrl {
                                    this.open_path_in_new_tab(p, cx);
                                } else {
                                    this.navigate(p, cx);
                                }
                            });
                        })
                        .on_mouse_down(MouseButton::Middle, move |_, _, cx| {
                            cx.stop_propagation();
                            let p = path_mid.clone();
                            v_mid.update(cx, |this, cx| {
                                this.open_path_in_new_tab(p, cx);
                            });
                        })
                        .on_mouse_down(MouseButton::Right, move |_, _, cx| {
                            cx.stop_propagation();
                            let p = path_right.clone();
                            v_right.update(cx, |this, cx| {
                                this.copy_breadcrumb_path(p, cx);
                            });
                        })
                        .on_mouse_move(move |_, _, cx| {
                            let p = path_hover.clone();
                            v_hover.update(cx, |this, cx| {
                                if this.is_dragging() {
                                    if this.state.nav_bar.file_drag_over_crumb.as_ref() != Some(&p)
                                    {
                                        this.state.nav_bar.file_drag_over_crumb = Some(p);
                                        cx.notify();
                                    }
                                } else if this.state.nav_bar.file_drag_over_crumb.is_some() {
                                    this.state.nav_bar.file_drag_over_crumb = None;
                                    cx.notify();
                                }
                            });
                        })
                        .on_mouse_up(MouseButton::Left, move |_, _, cx| {
                            let p = path_drop.clone();
                            v_drop.update(cx, |this, cx| {
                                if this.is_dragging() {
                                    this.handle_file_drop_on_breadcrumb(p, cx);
                                }
                            });
                        });

                    if is_drop_target {
                        seg = seg
                            .bg(rgba(0x0078D438u32))
                            .border(px(1.0))
                            .border_color(rgba(UiConstants::ACCENT_RGBA));
                    }

                    seg = seg.child(
                        div()
                            .text_size(px(13.0))
                            .text_color(if is_last {
                                rgba(UiConstants::TEXT_PRIMARY_RGBA)
                            } else {
                                rgba(UiConstants::TEXT_SECONDARY_RGBA)
                            })
                            .font_weight(if is_last {
                                FontWeight(600.0)
                            } else {
                                FontWeight(400.0)
                            })
                            .child(label),
                    );

                    breadcrumb_views.push(seg.into_any_element());
                }

                // Spacer fills remaining space and activates Path Edit Mode on click
                let spacer = div()
                    .flex_grow()
                    .min_w(px(24.0))
                    .h_full()
                    .cursor_text()
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        cx.stop_propagation();
                        v_pill.update(cx, |this, cx| {
                            this.activate_omnibar_path_mode(cx);
                        });
                    });

                // Quick "Copy Path" icon button at the right edge of the unfocused Omnibar
                let copy_btn = div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .flex_shrink_0()
                    .w(px(28.0))
                    .h(px(26.0))
                    .rounded(px(13.0))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        cx.stop_propagation();
                        v_copy_path.update(cx, |this, cx| {
                            let p = this.state.nav_bar.current_path.clone();
                            this.copy_breadcrumb_path(p, cx);
                        });
                    })
                    .child(
                        div()
                            .text_size(px(12.0))
                            .font_family(UiConstants::FONT_ICONS)
                            .text_color(rgba(UiConstants::TEXT_MUTED_RGBA))
                            .child("\u{E8C8}"),
                    );

                pill = pill
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .overflow_hidden()
                            .gap(px(1.0))
                            .children(breadcrumb_views),
                    )
                    .child(spacer)
                    .child(copy_btn)
                    .child(mode_icons);
            } else if state_mode == OmnibarMode::Path && edit_mode {
                let v_confirm = view.clone();
                let v_cancel = view.clone();
                pill = pill
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .flex_1()
                            .min_w(px(0.0))
                            .px(px(8.0))
                            .child(Self::render_text_input_with_ghost(
                                &state.edit_text,
                                "Enter a folder path, ~, %VAR%, or '>' for commands...",
                                state.cursor_pos,
                                state.selection_all,
                                state.ghost_prediction.as_deref(),
                                None,
                            )),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .flex_shrink_0()
                            .gap(px(2.0))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .w(px(26.0))
                                    .h(px(26.0))
                                    .rounded(px(13.0))
                                    .cursor_pointer()
                                    .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
                                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                        cx.stop_propagation();
                                        v_confirm.update(cx, |this, cx| {
                                            this.handle_suggestion_confirm(cx);
                                        });
                                    })
                                    .child(
                                        div()
                                            .text_size(px(12.0))
                                            .font_family(UiConstants::FONT_ICONS)
                                            .text_color(rgba(UiConstants::ACCENT_RGBA))
                                            .child("\u{E73E}"),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .w(px(26.0))
                                    .h(px(26.0))
                                    .rounded(px(13.0))
                                    .cursor_pointer()
                                    .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
                                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                        cx.stop_propagation();
                                        v_cancel.update(cx, |this, cx| {
                                            this.dismiss_omnibar(cx);
                                        });
                                    })
                                    .child(
                                        div()
                                            .text_size(px(11.0))
                                            .font_family(UiConstants::FONT_ICONS)
                                            .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                            .child("\u{E894}"),
                                    ),
                            ),
                    )
                    .child(mode_icons);
            } else if state_mode == OmnibarMode::CommandPalette {
                let v_cancel = view.clone();
                pill = pill
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .flex_1()
                            .min_w(px(0.0))
                            .px(px(8.0))
                            .child(Self::render_text_input_with_ghost(
                                &state.command_query,
                                "Type a command name or shortcut...",
                                state.cursor_pos,
                                state.selection_all,
                                state.ghost_prediction.as_deref(),
                                Some(">"),
                            )),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_center()
                            .flex_shrink_0()
                            .w(px(26.0))
                            .h(px(26.0))
                            .rounded(px(13.0))
                            .cursor_pointer()
                            .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
                            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                cx.stop_propagation();
                                v_cancel.update(cx, |this, cx| {
                                    this.dismiss_omnibar(cx);
                                });
                            })
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .font_family(UiConstants::FONT_ICONS)
                                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                    .child("\u{E894}"),
                            ),
                    )
                    .child(mode_icons);
            } else if state_mode == OmnibarMode::Search {
                let v_clear = view.clone();
                let v_re = view.clone();
                let v_case = view.clone();
                let v_txt = view.clone();
                let v_meta = view.clone();
                let v_sub = view.clone();
                let opts = state.search_options;

                let folder_name = state
                    .current_path
                    .as_path()
                    .file_name()
                    .map(|f| f.to_string_lossy().to_string())
                    .unwrap_or_else(|| state.current_path.to_string_lossy().to_string());
                let placeholder = format!(
                    "Search {} (wildcards *.rs, regex, kind:, size:, content:, meta:)...",
                    folder_name
                );

                pill = pill
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .flex_1()
                            .min_w(px(0.0))
                            .px(px(8.0))
                            .child(Self::render_text_input_with_ghost(
                                &state.search_query,
                                &placeholder,
                                state.cursor_pos,
                                state.selection_all,
                                state.ghost_prediction.as_deref(),
                                Some("?"),
                            )),
                    )
                    // Interactive Search Option Filter Pills (`.*`, `Aa`, `Txt`, `Meta`, `Sub`)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .flex_shrink_0()
                            .gap(px(4.0))
                            .child(Self::render_search_option_pill(
                                ".*",
                                opts.use_regex,
                                move |_, cx| {
                                    v_re.update(cx, |this, cx| {
                                        this.toggle_search_option("regex", cx);
                                    });
                                },
                            ))
                            .child(Self::render_search_option_pill(
                                "Aa",
                                opts.case_sensitive,
                                move |_, cx| {
                                    v_case.update(cx, |this, cx| {
                                        this.toggle_search_option("case", cx);
                                    });
                                },
                            ))
                            .child(Self::render_search_option_pill(
                                "Txt",
                                opts.search_content,
                                move |_, cx| {
                                    v_txt.update(cx, |this, cx| {
                                        this.toggle_search_option("content", cx);
                                    });
                                },
                            ))
                            .child(Self::render_search_option_pill(
                                "Meta",
                                opts.search_metadata,
                                move |_, cx| {
                                    v_meta.update(cx, |this, cx| {
                                        this.toggle_search_option("metadata", cx);
                                    });
                                },
                            ))
                            .child(Self::render_search_option_pill(
                                "Sub",
                                opts.recursive,
                                move |_, cx| {
                                    v_sub.update(cx, |this, cx| {
                                        this.toggle_search_option("recursive", cx);
                                    });
                                },
                            ))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .w(px(26.0))
                                    .h(px(26.0))
                                    .rounded(px(13.0))
                                    .cursor_pointer()
                                    .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
                                    .on_mouse_down(MouseButton::Left, move |_event, _win, cx| {
                                        cx.stop_propagation();
                                        v_clear.update(cx, |this, cx| {
                                            this.dismiss_omnibar(cx);
                                        });
                                    })
                                    .child(
                                        div()
                                            .text_size(px(11.0))
                                            .font_family(UiConstants::FONT_ICONS)
                                            .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                            .child("\u{E894}"),
                                    ),
                            ),
                    )
                    .child(mode_icons);
            }

            pill
        };

        div()
            .id("navigation-bar")
            .flex()
            .items_center()
            .h(Pixels::from(46.0))
            .w_full()
            .px(Pixels::from(8.0))
            .gap(px(8.0))
            .bg(rgba(UiConstants::BG_NAV_BAR_RGBA))
            .child(
                div()
                    .flex()
                    .items_center()
                    .flex_shrink_0()
                    .gap(px(2.0))
                    // SidebarPaneToggleButton (`NavigationToolbar.xaml:98-117`)
                    .child(Self::render_nav_btn_with_tooltip(
                        "nav-sidebar-toggle",
                        "\u{E700}",
                        "Toggle sidebar",
                        None,
                        true,
                        view.clone(),
                        move |_, _, cx| {
                            v_sidebar_toggle.update(cx, |this, cx| {
                                if !this.state.sidebar_visible {
                                    this.state.sidebar_visible = true;
                                } else {
                                    this.state.sidebar.toggle_collapsed();
                                }
                                cx.notify();
                            });
                        },
                        None,
                    ))
                    .child(Self::render_nav_btn_with_tooltip(
                        "nav-back",
                        "\u{E72B}",
                        "Back",
                        Some("Alt+Left"),
                        can_go_back,
                        view.clone(),
                        move |_, _, cx| {
                            v_back.update(cx, |this, cx| {
                                this.dismiss_omnibar(cx);
                                let _ = this.navigate_back(cx);
                            });
                        },
                        Some(Box::new(move |ev, _, cx| {
                            let x: f32 = ev.position.x.into();
                            v_back_rc.update(cx, |this, cx| {
                                this.state.nav_bar.history_flyout = Some((true, x));
                                cx.notify();
                            });
                        })),
                    ))
                    .child(Self::render_nav_btn_with_tooltip(
                        "nav-forward",
                        "\u{E72A}",
                        "Forward",
                        Some("Alt+Right"),
                        can_go_forward,
                        view.clone(),
                        move |_, _, cx| {
                            v_forward.update(cx, |this, cx| {
                                this.dismiss_omnibar(cx);
                                let _ = this.navigate_forward(cx);
                            });
                        },
                        Some(Box::new(move |ev, _, cx| {
                            let x: f32 = ev.position.x.into();
                            v_forward_rc.update(cx, |this, cx| {
                                this.state.nav_bar.history_flyout = Some((false, x));
                                cx.notify();
                            });
                        })),
                    ))
                    .child(Self::render_nav_btn_with_tooltip(
                        "nav-up",
                        "\u{E74A}",
                        "Up",
                        Some("Alt+Up"),
                        true,
                        view.clone(),
                        move |_, _, cx| {
                            v_up.update(cx, |this, cx| {
                                this.dismiss_omnibar(cx);
                                let _ = this.navigate_up(cx);
                            });
                        },
                        None,
                    ))
                    .child(Self::render_nav_btn_with_tooltip(
                        "nav-refresh",
                        "\u{E72C}",
                        "Refresh",
                        Some("F5"),
                        true,
                        view.clone(),
                        move |_, _, cx| {
                            v_refresh.update(cx, |this, cx| {
                                this.dismiss_omnibar(cx);
                                let _ = this.refresh(cx);
                            });
                        },
                        None,
                    )),
            )
            .child(address_bar_pill)
            .child(Self::status_bell(status, v_bell))
    }

    /// Render the Back / Forward navigation history flyout (`BackHistoryFlyout` / `ForwardHistoryFlyout`
    /// in `NavigationToolbar.xaml`).
    pub fn build_history_flyout(
        is_back: bool,
        anchor_x: f32,
        entries: &[FilesPath],
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let title = if is_back {
            "Back History"
        } else {
            "Forward History"
        };
        let mut rows = Vec::new();
        for (idx, path) in entries.iter().rev().take(12).enumerate() {
            let p = path.clone();
            let label = NavigationBarState::crumb_display_name(path);
            let full = path.to_string_lossy().to_string();
            let v_row = view.clone();
            let steps = idx + 1;
            rows.push(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(8.0))
                    .px(px(10.0))
                    .py(px(6.0))
                    .rounded(px(5.0))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        cx.stop_propagation();
                        let target = p.clone();
                        v_row.update(cx, |this, cx| {
                            this.state.nav_bar.history_flyout = None;
                            this.jump_history_steps(is_back, steps, target, cx);
                        });
                    })
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.0))
                            .flex_1()
                            .min_w(px(0.0))
                            .child(
                                div()
                                    .text_size(px(13.0))
                                    .font_family(UiConstants::FONT_ICONS)
                                    .text_color(rgb(0xF5B942u32))
                                    .child("\u{E8B7}"),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .flex_1()
                                    .min_w(px(0.0))
                                    .child(
                                        div()
                                            .text_size(px(12.5))
                                            .font_weight(FontWeight(600.0))
                                            .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                                            .overflow_hidden()
                                            .whitespace_nowrap()
                                            .child(label),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(10.5))
                                            .text_color(rgba(UiConstants::TEXT_MUTED_RGBA))
                                            .overflow_hidden()
                                            .whitespace_nowrap()
                                            .child(full),
                                    ),
                            ),
                    )
                    .into_any_element(),
            );
        }

        div()
            .absolute()
            .left(px(anchor_x.max(8.0)))
            .top(px(84.0))
            .w(px(280.0))
            .bg(rgba(0x282828F8u32))
            .border(px(1.0))
            .border_color(rgba(UiConstants::DIVIDER_RGBA))
            .rounded(px(8.0))
            .shadow_lg()
            .p(px(6.0))
            .flex()
            .flex_col()
            .gap(px(2.0))
            .child(
                div()
                    .px(px(8.0))
                    .py(px(4.0))
                    .text_size(px(11.0))
                    .font_weight(FontWeight(600.0))
                    .text_color(rgba(UiConstants::TEXT_MUTED_RGBA))
                    .child(title),
            )
            .children(rows)
    }
}

/// Render a label with matched character indices highlighted in bold (`Files-community`
/// `PrimaryDisplayPreMatched` / `PrimaryDisplayMatched` / `PrimaryDisplayPostMatched` style).
fn render_highlighted_text(
    text: &str,
    match_indices: &[usize],
    is_selected: bool,
) -> AnyElement {
    let base_color = if is_selected {
        rgba(0xFFFFFFFFu32)
    } else {
        rgba(UiConstants::TEXT_PRIMARY_RGBA)
    };
    let match_color = if is_selected {
        rgba(0xFFFFFFFFu32)
    } else {
        rgba(0x60CDFFFFu32)
    };

    if match_indices.is_empty() {
        return div()
            .text_size(px(13.0))
            .text_color(base_color)
            .overflow_hidden()
            .whitespace_nowrap()
            .child(text.to_string())
            .into_any_element();
    }

    let chars: Vec<char> = text.chars().collect();
    let idx_set: std::collections::HashSet<usize> = match_indices.iter().copied().collect();

    // Group consecutive runs of matched / unmatched characters to keep DOM element count minimal
    let mut runs: Vec<(bool, String)> = Vec::new();
    for (i, ch) in chars.into_iter().enumerate() {
        let is_m = idx_set.contains(&i);
        if let Some(last) = runs.last_mut() {
            if last.0 == is_m {
                last.1.push(ch);
                continue;
            }
        }
        let mut s = String::new();
        s.push(ch);
        runs.push((is_m, s));
    }

    let mut row = div()
        .flex()
        .items_center()
        .overflow_hidden()
        .whitespace_nowrap();
    for (is_m, chunk) in runs {
        row = row.child(
            div()
                .text_size(px(13.0))
                .font_weight(if is_m {
                    FontWeight(700.0)
                } else {
                    FontWeight(400.0)
                })
                .text_color(if is_m { match_color } else { base_color })
                .child(chunk),
        );
    }
    row.into_any_element()
}

pub fn build_suggestions_dropdown(
    state: &NavigationBarState,
    registry: &ActionRegistry,
    view: Entity<crate::app::AppWindow>,
) -> impl IntoElement {
    let selected_index = state.selected_suggestion_index;
    let mode = state.omnibar_mode;

    // Header bar with mode context + keyboard hints
    let (header_title, header_hint) = match mode {
        OmnibarMode::Path => (
            if state.edit_text.trim().is_empty() {
                "Recent Locations & Standard Folders"
            } else {
                "Predictive Path Completions"
            },
            "Tab: Autocomplete • Enter: Open • Ctrl+Enter: New Tab",
        ),
        OmnibarMode::Search => (
            if state.search_query.trim().is_empty() {
                "Recent Searches & AQS Filters"
            } else {
                "Live Predictive Search & Filter Matches"
            },
            "Alt+R: Regex • Alt+T: Content • Alt+M: Metadata • Ctrl+Enter: New Tab",
        ),
        OmnibarMode::CommandPalette => (
            "Command Palette",
            "Up/Down: Navigate • Enter: Run Command • Esc: Close",
        ),
    };

    let v_clear_hist = view.clone();
    let show_clear_history =
        mode == OmnibarMode::Search && state.search_query.trim().is_empty() && !state.search_history.is_empty();

    let header = div()
        .flex()
        .items_center()
        .justify_between()
        .px(px(12.0))
        .py(px(6.0))
        .border_b(px(1.0))
        .border_color(rgba(0xFFFFFF12u32))
        .child(
            div()
                .text_size(px(11.0))
                .font_weight(FontWeight(600.0))
                .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                .child(header_title),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.0))
                .children(if show_clear_history {
                    Some(
                        div()
                            .px(px(6.0))
                            .py(px(2.0))
                            .rounded(px(4.0))
                            .cursor_pointer()
                            .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
                            .text_size(px(10.5))
                            .text_color(rgba(UiConstants::ACCENT_RGBA))
                            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                cx.stop_propagation();
                                v_clear_hist.update(cx, |this, cx| {
                                    this.state.nav_bar.clear_search_history();
                                    this.refresh_omnibar_suggestions(cx);
                                });
                            })
                            .child("Clear history"),
                    )
                } else {
                    None
                })
                .child(
                    div()
                        .text_size(px(10.5))
                        .text_color(rgba(UiConstants::TEXT_MUTED_RGBA))
                        .child(header_hint),
                ),
        );

    let rows = state.suggestions.iter().enumerate().map(|(i, sug)| {
        let is_selected = selected_index == Some(i);
        let v_confirm = view.clone();
        let v_mid = view.clone();
        let v_del_hist = view.clone();
        let label = sug.label.clone();
        let subtitle = sug.subtitle.clone();
        let snippet = sug.snippet.clone();
        let badge = sug.badge.clone();
        let kind = sug.kind.clone();
        let match_indices = sug.match_indices.clone();

        let has_second_line = !subtitle.is_empty() || snippet.is_some();
        let row_h = if has_second_line { 44.0 } else { 34.0 };

        let mut row = div()
            .flex()
            .items_center()
            .justify_between()
            .min_h(px(row_h))
            .px(px(10.0))
            .py(px(4.0))
            .mx(px(4.0))
            .my(px(1.0))
            .rounded(px(6.0))
            .cursor_pointer()
            .on_mouse_down(MouseButton::Left, move |ev, _win, cx| {
                cx.stop_propagation();
                let ctrl = ev.modifiers.control;
                let shift = ev.modifiers.shift;
                v_confirm.update(cx, |this, cx| {
                    this.state.nav_bar.selected_suggestion_index = Some(i);
                    this.handle_suggestion_confirm_with_modifiers(ctrl, shift, false, cx);
                });
            })
            .on_mouse_down(MouseButton::Middle, move |_, _win, cx| {
                cx.stop_propagation();
                v_mid.update(cx, |this, cx| {
                    this.state.nav_bar.selected_suggestion_index = Some(i);
                    this.handle_suggestion_confirm_with_modifiers(true, false, false, cx);
                });
            });

        if is_selected {
            row = row
                .bg(rgba(0x0078D444u32))
                .border(px(1.0))
                .border_color(rgba(UiConstants::ACCENT_RGBA));
        } else {
            row = row.hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)));
        }

        let icon_color = match &kind {
            SuggestionKind::Path { .. } => rgb(0xF5B942u32),
            SuggestionKind::SearchItem { is_dir: true, .. } => rgb(0xF5B942u32),
            SuggestionKind::SearchItem { is_dir: false, .. } => rgb(0x60CDFFu32),
            SuggestionKind::AqsFilter { .. } => rgb(0x6CCB5Fu32),
            SuggestionKind::Command { .. } => rgb(0x60CDFFu32),
            SuggestionKind::SearchHistory { .. } => rgb(0xB0B0B0u32),
        };

        let mut text_col = div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w(px(0.0))
            .ml(px(10.0))
            .child(render_highlighted_text(&label, &match_indices, is_selected));

        if has_second_line {
            let mut sub_row = div()
                .flex()
                .items_center()
                .gap(px(8.0))
                .overflow_hidden()
                .whitespace_nowrap();
            if let Some(snip) = snippet {
                sub_row = sub_row.child(
                    div()
                        .px(px(5.0))
                        .py(px(1.0))
                        .rounded(px(3.0))
                        .bg(rgba(0x0078D42Eu32))
                        .text_size(px(10.5))
                        .text_color(rgba(0x88D8FFFFu32))
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .child(snip),
                );
            }
            if !subtitle.is_empty() {
                sub_row = sub_row.child(
                    div()
                        .text_size(px(11.0))
                        .text_color(rgba(UiConstants::TEXT_MUTED_RGBA))
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .child(subtitle),
                );
            }
            text_col = text_col.child(sub_row);
        }

        // Right-aligned metadata badge / shortcut keycap / delete history button
        let mut right_cluster = div().flex().items_center().flex_shrink_0().gap(px(6.0));
        if let SuggestionKind::Command { action } = kind {
            if let Some(binding) = registry.binding(action) {
                right_cluster = right_cluster.child(
                    div()
                        .px(px(6.0))
                        .py(px(2.0))
                        .rounded(px(4.0))
                        .bg(rgba(0xFFFFFF14u32))
                        .border(px(1.0))
                        .border_color(rgba(0xFFFFFF22u32))
                        .text_size(px(11.0))
                        .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                        .child(binding.to_string()),
                );
            }
        } else if let Some(b) = badge {
            right_cluster = right_cluster.child(
                div()
                    .px(px(6.0))
                    .py(px(2.0))
                    .rounded(px(4.0))
                    .bg(rgba(0xFFFFFF0Eu32))
                    .text_size(px(10.5))
                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                    .child(b),
            );
        }

        if let SuggestionKind::SearchHistory { ref query } = kind {
            if state.search_query.trim().is_empty() {
                let q_del = query.clone();
                right_cluster = right_cluster.child(
                    div()
                        .w(px(20.0))
                        .h(px(20.0))
                        .rounded(px(4.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .hover(|s| s.bg(rgba(0xE8112344u32)))
                        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                            cx.stop_propagation();
                            let q = q_del.clone();
                            v_del_hist.update(cx, |this, cx| {
                                this.state.nav_bar.remove_search_history_item(&q);
                                this.refresh_omnibar_suggestions(cx);
                            });
                        })
                        .child(
                            div()
                                .text_size(px(9.5))
                                .font_family(UiConstants::FONT_ICONS)
                                .text_color(rgba(UiConstants::TEXT_MUTED_RGBA))
                                .child("\u{E711}"),
                        ),
                );
            }
        }

        row.child(
            div()
                .flex()
                .items_center()
                .flex_1()
                .min_w(px(0.0))
                .child(
                    div()
                        .w(px(20.0))
                        .flex_shrink_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .font_family(UiConstants::FONT_ICONS)
                        .text_size(px(14.0))
                        .text_color(icon_color)
                        .child(sug.icon.clone()),
                )
                .child(text_col),
        )
        .child(right_cluster)
    });

    div()
        .bg(rgba(0x252525FAu32))
        .border(px(1.0))
        .border_color(rgba(UiConstants::DIVIDER_RGBA))
        .rounded(px(8.0))
        .shadow_lg()
        .overflow_hidden()
        .child(header)
        .child(
            div()
                .id("omnibar-suggestions-list")
                .track_scroll(&state.suggestions_scroll)
                .flex()
                .flex_col()
                .max_h(px(420.0))
                .overflow_y_scroll()
                .py(px(4.0))
                .children(rows),
        )
}

/// Truncate a string in the middle, keeping `keep` chars on each side.
fn truncate_middle(s: &str, keep: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= keep * 2 + 1 {
        return s.to_string();
    }
    let start: String = chars[..keep].iter().collect();
    let end: String = chars[chars.len() - keep..].iter().collect();
    format!("{}…{}", start, end)
}

/// Fuzzy match: returns char-level indices of matched characters in name if filter chars
/// match in order (case-insensitive). Returns None if no match.
pub fn fuzzy_match_indices(name: &str, filter: &str) -> Option<Vec<usize>> {
    if filter.is_empty() {
        return Some((0..name.chars().count()).collect());
    }
    let name_lower: Vec<char> = name.to_lowercase().chars().collect();
    let filter_lower: Vec<char> = filter.to_lowercase().chars().collect();
    let mut match_indices = Vec::new();
    let mut name_idx = 0;
    for &filter_char in &filter_lower {
        match name_lower[name_idx..]
            .iter()
            .position(|&c| c == filter_char)
        {
            Some(offset) => {
                let char_idx = name_idx + offset;
                match_indices.push(char_idx);
                name_idx = char_idx + 1;
            }
            None => return None,
        }
    }
    Some(match_indices)
}

pub fn build_breadcrumb_dropdown(
    state: &NavigationBarState,
    view: Entity<crate::app::AppWindow>,
    _focus_handle: gpui::FocusHandle,
) -> Option<Div> {
    if !state.breadcrumb_dropdown_open || state.breadcrumb_dropdown_position.is_none() {
        return None;
    }

    let position = state.breadcrumb_dropdown_position.unwrap();
    let filtered = state.breadcrumb_dropdown_filtered_siblings();
    let selected = state.breadcrumb_dropdown_selected.unwrap_or(0);
    let filter_text = state.breadcrumb_dropdown_filter.clone();

    let mut row_views = Vec::new();
    for (vis_idx, (_orig_idx, sibling_path)) in filtered.iter().enumerate() {
        let name = NavigationBarState::crumb_display_name(sibling_path);
        let path = (*sibling_path).clone();
        let is_selected = vis_idx == selected;

        let v_row = view.clone();
        let v_mid = view.clone();
        let name_clone = name.clone();
        let filter_clone = filter_text.clone();

        let display_name = truncate_middle(&name, 12);
        let highlighted_name = if filter_clone.is_empty() {
            div()
                .text_size(px(12.5))
                .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                .child(display_name)
                .into_any_element()
        } else if let Some(indices) = fuzzy_match_indices(&name_clone, &filter_clone) {
            render_highlighted_text(&name_clone, &indices, is_selected)
        } else {
            div()
                .text_size(px(12.5))
                .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                .child(display_name)
                .into_any_element()
        };

        let path_mid = path.clone();
        let mut row = div()
            .flex()
            .items_center()
            .gap(px(8.0))
            .px(px(10.0))
            .py(px(5.0))
            .mx(px(4.0))
            .rounded(px(5.0))
            .cursor_pointer()
            .on_mouse_down(MouseButton::Left, move |ev, _, cx| {
                cx.stop_propagation();
                let ctrl = ev.modifiers.control;
                let p = path.clone();
                v_row.update(cx, |this, cx| {
                    this.state.nav_bar.close_breadcrumb_dropdown();
                    if ctrl {
                        this.open_path_in_new_tab(p, cx);
                    } else {
                        this.navigate(p, cx);
                    }
                });
            })
            .on_mouse_down(MouseButton::Middle, move |_, _, cx| {
                cx.stop_propagation();
                let p = path_mid.clone();
                v_mid.update(cx, |this, cx| {
                    this.state.nav_bar.close_breadcrumb_dropdown();
                    this.open_path_in_new_tab(p, cx);
                });
            })
            .child(
                div()
                    .text_size(px(13.0))
                    .font_family(UiConstants::FONT_ICONS)
                    .text_color(rgb(0xF5B942u32))
                    .child("\u{E8B7}"),
            )
            .child(highlighted_name);

        if is_selected {
            row = row.bg(rgba(UiConstants::BG_ITEM_SELECTED_RGBA));
        } else {
            row = row.hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)));
        }

        row_views.push(row.into_any_element());
    }

    let filter_display = if filter_text.is_empty() {
        if state.breadcrumb_dropdown_is_root {
            "Filter Quick Access & Drives...".to_string()
        } else {
            "Filter subfolders...".to_string()
        }
    } else {
        filter_text.clone()
    };
    let filter_color = if filter_text.is_empty() {
        UiConstants::TEXT_MUTED_RGBA
    } else {
        UiConstants::TEXT_PRIMARY_RGBA
    };

    let cursor_text = div()
        .id("breadcrumb-cursor")
        .text_size(px(12.0))
        .text_color(rgba(filter_color))
        .child(filter_display);

    Some(
        div()
            .absolute()
            .left(position.0)
            .top(position.1)
            .w(px(248.0))
            .max_h(px(280.0))
            .flex()
            .flex_col()
            .bg(rgba(0x262626FAu32))
            .border(px(1.0))
            .border_color(rgba(UiConstants::ACCENT_RGBA))
            .rounded(px(8.0))
            .shadow_lg()
            .overflow_hidden()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .px(px(10.0))
                    .py(px(7.0))
                    .border_b(px(1.0))
                    .border_color(rgba(UiConstants::DIVIDER_RGBA))
                    .child(
                        div()
                            .text_size(px(11.0))
                            .font_family(UiConstants::FONT_ICONS)
                            .text_color(rgba(UiConstants::TEXT_MUTED_RGBA))
                            .child("\u{E721}"),
                    )
                    .child(cursor_text),
            )
            .child(
                div()
                    .id("breadcrumb-list")
                    .flex()
                    .flex_col()
                    .max_h(px(230.0))
                    .overflow_y_scroll()
                    .py(px(4.0))
                    .children(row_views),
            ),
    )
}

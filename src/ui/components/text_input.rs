//! Unified single-line and multi-line text input component for `files-rs`.
//!
//! Provides `TextInputState`, `TextInputMode`, `TextInputStyle`, and `render_text_input`
//! so every editable field in the app (inline rename, Extract Archive destination/password,
//! Add Item, Properties fields & multiline comment textarea, Settings/Tag inputs, Omnibar)
//! shares identical native selection, Shift+Arrow / Ctrl+Shift+Arrow word navigation,
//! Up/Down multi-line movement, mouse click/double-click/triple-click/drag selection,
//! horizontal/vertical scrolling, and clipboard behavior.

use crate::ui::constants::UiConstants;
use gpui::*;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

/// Operating mode of a `TextInput` widget.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextInputMode {
    /// Single horizontal scrollable row (`Enter` submits).
    SingleLine,
    /// Multi-row / textarea capable of soft word/delimiter wrapping, explicit `\n` lines
    /// (when `allow_newlines == true`), optional max line cap, and optional centering.
    MultiLine {
        max_lines: Option<usize>,
        allow_newlines: bool,
        text_align_center: bool,
    },
}

impl Default for TextInputMode {
    fn default() -> Self {
        Self::SingleLine
    }
}

/// Visual styling for `render_text_input`.
#[derive(Debug, Clone)]
pub struct TextInputStyle {
    pub font_size: f32,
    pub line_height: f32,
    pub font_family: &'static str,
    pub text_color: u32,
    pub placeholder_color: u32,
    pub selection_bg: u32,
    pub cursor_color: u32,
    pub bg: u32,
    pub bg_focused: u32,
    pub border_color: u32,
    pub border_focused_color: u32,
    pub bottom_accent_when_focused: bool,
    pub rounded: f32,
    pub padding_x: f32,
    pub padding_y: f32,
    pub min_height: f32,
    pub max_height: Option<f32>,
}

impl Default for TextInputStyle {
    fn default() -> Self {
        Self::dialog()
    }
}

impl TextInputStyle {
    /// Standard Windows 11 Fluent single-line dialog input styling.
    pub fn dialog() -> Self {
        Self {
            font_size: 12.5,
            line_height: 18.0,
            font_family: UiConstants::FONT_MAIN,
            text_color: 0xFFFFFFFF,
            placeholder_color: 0x8A8A8AFF,
            selection_bg: 0x0078D4,
            cursor_color: 0xFFFFFF,
            bg: 0x181818FF,
            bg_focused: 0x181818FF,
            border_color: 0xFFFFFF26,
            border_focused_color: 0x4CC2FFFF,
            bottom_accent_when_focused: true,
            rounded: 5.0,
            padding_x: 10.0,
            padding_y: 5.0,
            min_height: 32.0,
            max_height: None,
        }
    }

    /// Multi-line / textarea dialog box styling with vertical scrolling support.
    pub fn textarea(min_height: f32, max_height: f32) -> Self {
        Self {
            min_height,
            max_height: Some(max_height),
            padding_y: 6.0,
            ..Self::dialog()
        }
    }

    /// Compact single-line inline rename box (Details, List, Columns, Tree views).
    pub fn inline_rename_single() -> Self {
        Self {
            font_size: 12.5,
            line_height: 18.0,
            font_family: UiConstants::FONT_MAIN,
            text_color: 0xFFFFFFFF,
            placeholder_color: 0x8A8A8AFF,
            selection_bg: 0x0078D4,
            cursor_color: 0xFFFFFF,
            bg: 0x1E1E1EFF,
            bg_focused: 0x1E1E1EFF,
            border_color: 0x60CDFFFF,
            border_focused_color: 0x60CDFFFF,
            bottom_accent_when_focused: true,
            rounded: 4.0,
            padding_x: 6.0,
            padding_y: 2.0,
            min_height: 26.0,
            max_height: Some(28.0),
        }
    }

    /// Multi-line centered inline rename box (Grid view tiles).
    pub fn inline_rename_grid(font_size: f32, line_height: f32, box_height: f32) -> Self {
        Self {
            font_size,
            line_height,
            font_family: UiConstants::FONT_MAIN,
            text_color: 0xFFFFFFFF,
            placeholder_color: 0x8A8A8AFF,
            selection_bg: 0x0078D4,
            cursor_color: 0xFFFFFF,
            bg: 0x1E1E1EF0,
            bg_focused: 0x1E1E1EF0,
            border_color: 0x60CDFFFF,
            border_focused_color: 0x60CDFFFF,
            bottom_accent_when_focused: true,
            rounded: 4.0,
            padding_x: 4.0,
            padding_y: 2.0,
            min_height: box_height,
            max_height: Some(box_height),
        }
    }

    /// Compact toolbar / Settings search box styling.
    pub fn compact_search() -> Self {
        Self {
            font_size: 12.0,
            line_height: 17.0,
            min_height: 30.0,
            max_height: Some(32.0),
            padding_x: 10.0,
            padding_y: 4.0,
            ..Self::dialog()
        }
    }

    /// Borderless single-line input embedded inside the rounded Omnibar pill.
    pub fn omnibar_borderless() -> Self {
        Self {
            font_size: 13.0,
            line_height: 18.0,
            bg: 0x00000000,
            bg_focused: 0x00000000,
            border_color: 0x00000000,
            border_focused_color: 0x00000000,
            bottom_accent_when_focused: false,
            rounded: 0.0,
            padding_x: 2.0,
            padding_y: 2.0,
            min_height: 26.0,
            max_height: Some(26.0),
            ..Self::dialog()
        }
    }
}

/// Behavior options passed to `TextInputState::handle_keystroke`.
#[derive(Debug, Clone)]
pub struct TextInputKeyOptions {
    pub mode: TextInputMode,
    /// Disallow Windows invalid filename characters (`\ / : * ? " < > |`).
    pub disallow_filename_chars: bool,
    /// Whether `Enter` without modifiers emits `Submitted` instead of inserting `\n`.
    pub submit_on_enter: bool,
    /// Allow clipboard copy/cut (`false` when showing masked password input).
    pub allow_copy: bool,
    /// Accept inline autocomplete ghost suggestion on `Tab` when available.
    pub tab_autocomplete: bool,
    /// Insert/remove 4-space indentation on `Tab`/`Shift+Tab` in MultiLine `allow_newlines` mode.
    pub allow_tab_indent: bool,
}

impl Default for TextInputKeyOptions {
    fn default() -> Self {
        Self {
            mode: TextInputMode::SingleLine,
            disallow_filename_chars: false,
            submit_on_enter: true,
            allow_copy: true,
            tab_autocomplete: true,
            allow_tab_indent: true,
        }
    }
}

/// Result of handling a keyboard event inside a `TextInputState`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextInputKeyOutcome {
    Ignored,
    Handled,
    Changed,
    Submitted,
    Cancelled,
    TabPressed { shift: bool },
    AutocompleteAccepted,
    AutocompleteCycle { reverse: bool },
}

/// Measured inner viewport layout populated during paint/prepaint for pixel-exact hit testing.
#[derive(Debug, Clone, Default)]
pub struct TextInputLayoutInfo {
    pub content_left: f32,
    pub content_top: f32,
    pub content_w: f32,
    pub content_h: f32,
    pub rendered_scroll_x: f32,
    pub rendered_scroll_y: f32,
    pub wrap_chars_per_line: usize,
    pub font_size: f32,
    pub line_height: f32,
    pub mode: TextInputMode,
    pub masked: bool,
    pub shaped_text_w: f32,
    pub shaped_cursor_x: f32,
    pub text_snapshot_len: usize,
    pub cursor_snapshot: usize,
    pub meas_char_w: f32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextUndoSnapshot {
    pub text: String,
    pub cursor: usize,
    pub selection_anchor: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum TextEditKind {
    #[default]
    None,
    InsertWord,
    DeleteRun,
    Discrete,
}

/// A single rendered visual line (after splitting explicit `\n` and soft-wrapping).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WrappedLine {
    pub text: String,
    pub start_char: usize,
    pub end_char: usize,
}

/// Reusable state for both single-line and multi-line/textarea inputs.
#[derive(Debug, Clone)]
pub struct TextInputState {
    pub text: String,
    pub cursor: usize,
    pub selection_anchor: Option<usize>,
    pub scroll_x: f32,
    pub scroll_y: f32,
    pub dragging: bool,
    pub preferred_col: Option<usize>,
    pub layout: Arc<Mutex<Option<TextInputLayoutInfo>>>,
    pub undo_stack: Vec<TextUndoSnapshot>,
    pub redo_stack: Vec<TextUndoSnapshot>,
    last_edit_kind: TextEditKind,
    /// Optional autocomplete candidates (matched case-insensitively by prefix).
    pub completions: Vec<String>,
    pub completion_index: Option<usize>,
}

impl Default for TextInputState {
    fn default() -> Self {
        Self {
            text: String::new(),
            cursor: 0,
            selection_anchor: None,
            scroll_x: 0.0,
            scroll_y: 0.0,
            dragging: false,
            preferred_col: None,
            layout: Arc::new(Mutex::new(None)),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            last_edit_kind: TextEditKind::None,
            completions: Vec::new(),
            completion_index: None,
        }
    }
}

impl PartialEq for TextInputState {
    fn eq(&self, other: &Self) -> bool {
        self.text == other.text
            && self.cursor == other.cursor
            && self.selection_anchor == other.selection_anchor
    }
}

impl Eq for TextInputState {}

/// Convert a UTF-8 byte index in `text` to a clamped char index.
pub fn byte_idx_to_char_idx(text: &str, byte_idx: usize) -> usize {
    let mut b = byte_idx.min(text.len());
    while b > 0 && !text.is_char_boundary(b) {
        b -= 1;
    }
    text[..b].chars().count()
}

/// Word bounds around `char_idx` (used by double-click selection).
pub fn word_bounds_at(text: &str, char_idx: usize) -> (usize, usize) {
    let chars: Vec<char> = text.chars().collect();
    if chars.is_empty() {
        return (0, 0);
    }
    let idx = char_idx.min(chars.len().saturating_sub(1));
    let is_word = |c: char| c.is_alphanumeric() || c == '_';
    let target_is_word = is_word(chars[idx]);
    let mut start = idx;
    while start > 0
        && is_word(chars[start - 1]) == target_is_word
        && !chars[start - 1].is_whitespace()
    {
        start -= 1;
    }
    let mut end = idx + 1;
    while end < chars.len()
        && is_word(chars[end]) == target_is_word
        && !chars[end].is_whitespace()
    {
        end += 1;
    }
    (start, end)
}

pub fn prev_word_boundary(text: &str, cur: usize) -> usize {
    let chars: Vec<char> = text.chars().collect();
    if cur == 0 || chars.is_empty() {
        return 0;
    }
    let mut i = cur.min(chars.len());
    while i > 0 && !chars[i - 1].is_alphanumeric() {
        i -= 1;
    }
    while i > 0 && chars[i - 1].is_alphanumeric() {
        i -= 1;
    }
    i
}

pub fn next_word_boundary(text: &str, cur: usize) -> usize {
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    if cur >= len {
        return len;
    }
    let mut i = cur;
    while i < len && chars[i].is_alphanumeric() {
        i += 1;
    }
    while i < len && !chars[i].is_alphanumeric() {
        i += 1;
    }
    i
}

/// Break a single paragraph into soft-wrapped segments of up to `chars_per_line`, respecting natural delimiters.
fn wrap_paragraph(
    para_chars: &[char],
    base_offset: usize,
    chars_per_line: usize,
    remaining_line_budget: Option<usize>,
    out: &mut Vec<WrappedLine>,
) {
    if para_chars.is_empty() {
        out.push(WrappedLine {
            text: String::new(),
            start_char: base_offset,
            end_char: base_offset,
        });
        return;
    }

    let cpl = chars_per_line.max(6);
    let total_len = para_chars.len();
    let mut cur_start = 0;

    while cur_start < total_len {
        let is_last_allowed = remaining_line_budget
            .map(|budget| out.len() + 1 >= budget)
            .unwrap_or(false);
        let remaining = total_len - cur_start;

        if is_last_allowed || remaining <= cpl {
            out.push(WrappedLine {
                text: para_chars[cur_start..].iter().collect(),
                start_char: base_offset + cur_start,
                end_char: base_offset + total_len,
            });
            break;
        }

        let max_chunk = cpl.min(remaining);
        let mut break_idx = None;
        for i in (1..=max_chunk).rev() {
            if para_chars[cur_start + i - 1].is_whitespace() {
                break_idx = Some(cur_start + i);
                break;
            }
        }
        if break_idx.is_none() {
            let min_search = cpl.saturating_sub(6).max(1);
            for i in (min_search..=max_chunk).rev() {
                if matches!(
                    para_chars[cur_start + i - 1],
                    '.' | '-' | '_' | ')' | ']' | '(' | '[' | '/' | '\\'
                ) {
                    break_idx = Some(cur_start + i);
                    break;
                }
            }
        }

        let next_start = break_idx.unwrap_or(cur_start + max_chunk);
        out.push(WrappedLine {
            text: para_chars[cur_start..next_start].iter().collect(),
            start_char: base_offset + cur_start,
            end_char: base_offset + next_start,
        });
        cur_start = next_start;
    }
}

/// Compute visual lines for `text` under `mode` and an estimated column width `chars_per_line`.
pub fn wrap_text_lines(text: &str, mode: &TextInputMode, chars_per_line: usize) -> Vec<WrappedLine> {
    let chars: Vec<char> = text.chars().collect();
    match mode {
        TextInputMode::SingleLine => vec![WrappedLine {
            text: text.to_string(),
            start_char: 0,
            end_char: chars.len(),
        }],
        TextInputMode::MultiLine { max_lines, .. } => {
            if chars.is_empty() {
                return vec![WrappedLine {
                    text: String::new(),
                    start_char: 0,
                    end_char: 0,
                }];
            }
            let mut out = Vec::new();
            let mut para_start = 0;
            for idx in 0..=chars.len() {
                let at_end = idx == chars.len();
                let is_nl = !at_end && chars[idx] == '\n';
                if is_nl || at_end {
                    let reached_cap = max_lines.map(|m| out.len() >= m).unwrap_or(false);
                    if !reached_cap {
                        wrap_paragraph(
                            &chars[para_start..idx],
                            para_start,
                            chars_per_line,
                            *max_lines,
                            &mut out,
                        );
                    }
                    para_start = idx + 1;
                }
            }
            if out.is_empty() {
                out.push(WrappedLine {
                    text: String::new(),
                    start_char: 0,
                    end_char: 0,
                });
            }
            out
        }
    }
}

impl TextInputState {
    pub fn new(text: impl Into<String>) -> Self {
        let text = text.into();
        let len = text.chars().count();
        Self {
            text,
            cursor: len,
            ..Default::default()
        }
    }

    pub fn with_text(text: impl Into<String>, select_all: bool) -> Self {
        let mut st = Self::default();
        st.reset_keeping_layout(text, select_all);
        st
    }

    /// Replace buffer cleanly while preserving previously measured `layout` (prevents 1-frame scroll cut-off).
    pub fn reset_keeping_layout(&mut self, text: impl Into<String>, select_all: bool) {
        self.text = text.into();
        let len = self.text.chars().count();
        self.cursor = len;
        self.selection_anchor = if select_all && len > 0 { Some(0) } else { None };
        self.scroll_x = 0.0;
        self.scroll_y = 0.0;
        self.dragging = false;
        self.preferred_col = None;
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.last_edit_kind = TextEditKind::None;
        self.completion_index = None;
    }

    pub fn set_text_and_cursor_end(&mut self, text: impl Into<String>) {
        self.set_text(text);
    }

    pub fn is_all_selected(&self) -> bool {
        let len = self.len_chars();
        len > 0 && self.selection_range() == Some((0, len))
    }

    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
        let len = self.text.chars().count();
        self.cursor = len;
        self.selection_anchor = None;
        self.dragging = false;
        self.preferred_col = None;
        self.last_edit_kind = TextEditKind::None;
        self.completion_index = None;
    }

    pub fn set_text_selected_all(&mut self, text: impl Into<String>) {
        self.text = text.into();
        let len = self.text.chars().count();
        self.cursor = len;
        self.selection_anchor = if len > 0 { Some(0) } else { None };
        self.dragging = false;
        self.preferred_col = None;
        self.last_edit_kind = TextEditKind::None;
        self.completion_index = None;
    }

    fn record_undo(&mut self, kind: TextEditKind) {
        let should_coalesce = matches!(
            (kind, self.last_edit_kind),
            (TextEditKind::InsertWord, TextEditKind::InsertWord)
                | (TextEditKind::DeleteRun, TextEditKind::DeleteRun)
        ) && !self.undo_stack.is_empty();
        if !should_coalesce {
            self.undo_stack.push(TextUndoSnapshot {
                text: self.text.clone(),
                cursor: self.cursor,
                selection_anchor: self.selection_anchor,
            });
            if self.undo_stack.len() > 200 {
                self.undo_stack.remove(0);
            }
        }
        self.redo_stack.clear();
        self.last_edit_kind = kind;
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    pub fn undo(&mut self) -> bool {
        let Some(prev) = self.undo_stack.pop() else {
            return false;
        };
        self.redo_stack.push(TextUndoSnapshot {
            text: self.text.clone(),
            cursor: self.cursor,
            selection_anchor: self.selection_anchor,
        });
        let len = prev.text.chars().count();
        self.text = prev.text;
        self.cursor = prev.cursor.min(len);
        self.selection_anchor = prev.selection_anchor.map(|a| a.min(len));
        self.preferred_col = None;
        self.last_edit_kind = TextEditKind::None;
        self.completion_index = None;
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(next) = self.redo_stack.pop() else {
            return false;
        };
        self.undo_stack.push(TextUndoSnapshot {
            text: self.text.clone(),
            cursor: self.cursor,
            selection_anchor: self.selection_anchor,
        });
        let len = next.text.chars().count();
        self.text = next.text;
        self.cursor = next.cursor.min(len);
        self.selection_anchor = next.selection_anchor.map(|a| a.min(len));
        self.preferred_col = None;
        self.last_edit_kind = TextEditKind::None;
        self.completion_index = None;
        true
    }

    pub fn set_completions(&mut self, completions: Vec<String>) {
        self.completions = completions;
        self.completion_index = None;
    }

    pub fn clear_completions(&mut self) {
        self.completions.clear();
        self.completion_index = None;
    }

    pub fn ghost_completion(&self) -> Option<String> {
        if self.has_selection() || self.cursor < self.len_chars() || self.completions.is_empty() {
            return None;
        }
        let query_lower = self.text.to_lowercase();
        let candidate = if let Some(idx) = self.completion_index {
            self.completions.get(idx)?
        } else {
            self.completions.iter().find(|c| {
                let cl = c.to_lowercase();
                !query_lower.is_empty() && cl.starts_with(&query_lower) && cl.len() > query_lower.len()
            })?
        };
        let q_chars = self.len_chars();
        let cand_lower = candidate.to_lowercase();
        if cand_lower.starts_with(&query_lower) {
            let suffix: String = candidate.chars().skip(q_chars).collect();
            if suffix.is_empty() {
                None
            } else {
                Some(suffix)
            }
        } else {
            None
        }
    }

    pub fn accept_autocomplete(&mut self) -> bool {
        let Some(idx) = self.completion_index.or_else(|| {
            let ql = self.text.to_lowercase();
            self.completions.iter().position(|c| {
                let cl = c.to_lowercase();
                !ql.is_empty() && cl.starts_with(&ql) && cl.len() > ql.len()
            })
        }) else {
            return false;
        };
        let Some(cand) = self.completions.get(idx).cloned() else {
            return false;
        };
        if cand == self.text {
            return false;
        }
        self.record_undo(TextEditKind::Discrete);
        self.text = cand;
        self.cursor = self.len_chars();
        self.selection_anchor = None;
        self.preferred_col = None;
        self.completion_index = None;
        true
    }

    pub fn cycle_completion(&mut self, reverse: bool) -> bool {
        if self.completions.is_empty() {
            return false;
        }
        let n = self.completions.len();
        let next_idx = match self.completion_index {
            None => {
                if reverse {
                    n - 1
                } else {
                    0
                }
            }
            Some(cur) => {
                if reverse {
                    (cur + n - 1) % n
                } else {
                    (cur + 1) % n
                }
            }
        };
        self.completion_index = Some(next_idx);
        let cand = self.completions[next_idx].clone();
        self.record_undo(TextEditKind::Discrete);
        self.text = cand;
        self.cursor = self.len_chars();
        self.selection_anchor = None;
        self.preferred_col = None;
        true
    }

    pub fn select_all(&mut self) {
        let len = self.text.chars().count();
        self.cursor = len;
        self.selection_anchor = if len > 0 { Some(0) } else { None };
        self.preferred_col = None;
        self.last_edit_kind = TextEditKind::None;
    }

    pub fn clear_selection(&mut self) {
        self.selection_anchor = None;
        self.last_edit_kind = TextEditKind::None;
    }

    pub fn clear(&mut self) {
        if !self.text.is_empty() {
            self.record_undo(TextEditKind::Discrete);
        }
        self.text.clear();
        self.cursor = 0;
        self.selection_anchor = None;
        self.scroll_x = 0.0;
        self.scroll_y = 0.0;
        self.dragging = false;
        self.preferred_col = None;
        self.completion_index = None;
    }

    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    pub fn len_chars(&self) -> usize {
        self.text.chars().count()
    }

    pub fn display_string(&self, masked: bool) -> String {
        if !masked {
            return self.text.clone();
        }
        self.text
            .chars()
            .map(|c| if c == '\n' { '\n' } else { '•' })
            .collect()
    }

    /// Normalized `(start, end)` char range if a non-empty selection exists.
    pub fn selection_range(&self) -> Option<(usize, usize)> {
        let len = self.len_chars();
        let anchor = self.selection_anchor?.min(len);
        let cursor = self.cursor.min(len);
        if anchor == cursor {
            None
        } else {
            Some((anchor.min(cursor), anchor.max(cursor)))
        }
    }

    pub fn has_selection(&self) -> bool {
        self.selection_range().is_some()
    }

    pub fn selected_text(&self) -> String {
        let Some((s, e)) = self.selection_range() else {
            return String::new();
        };
        self.text.chars().skip(s).take(e - s).collect()
    }

    pub fn delete_selection(&mut self) -> bool {
        let Some((start, end)) = self.selection_range() else {
            return false;
        };
        self.record_undo(TextEditKind::Discrete);
        let mut chars: Vec<char> = self.text.chars().collect();
        let end = end.min(chars.len());
        let start = start.min(end);
        chars.drain(start..end);
        self.text = chars.into_iter().collect();
        self.cursor = start;
        self.selection_anchor = None;
        self.preferred_col = None;
        self.completion_index = None;
        true
    }

    pub fn replace_selection(&mut self, insert: &str) {
        let has_sel = self.selection_range().is_some();
        let is_single_non_ws =
            !has_sel && insert.chars().count() == 1 && !insert.chars().next().is_some_and(|c| c.is_whitespace());
        self.record_undo(if is_single_non_ws {
            TextEditKind::InsertWord
        } else {
            TextEditKind::Discrete
        });
        let mut chars: Vec<char> = self.text.chars().collect();
        let insert_pos = if let Some((start, end)) = self.selection_range() {
            let end = end.min(chars.len());
            let start = start.min(end);
            chars.drain(start..end);
            start
        } else {
            self.cursor.min(chars.len())
        };
        let count = insert.chars().count();
        for (i, ch) in insert.chars().enumerate() {
            chars.insert(insert_pos + i, ch);
        }
        self.text = chars.into_iter().collect();
        self.cursor = insert_pos + count;
        self.selection_anchor = None;
        self.preferred_col = None;
        self.completion_index = None;
    }

    pub fn move_cursor(&mut self, new_pos: usize, shift: bool, collapse_to_start: bool) {
        let len = self.len_chars();
        let clamped_new = new_pos.min(len);
        self.preferred_col = None;
        self.last_edit_kind = TextEditKind::None;
        if shift {
            if self.selection_anchor.is_none() {
                self.selection_anchor = Some(self.cursor.min(len));
            }
            self.cursor = clamped_new;
            if self.selection_anchor == Some(self.cursor) {
                self.selection_anchor = None;
            }
        } else if let Some((start, end)) = self.selection_range() {
            self.cursor = if collapse_to_start { start } else { end };
            self.selection_anchor = None;
        } else {
            self.cursor = clamped_new;
            self.selection_anchor = None;
        }
    }

    pub fn move_left(&mut self, shift: bool, word: bool) {
        if !shift && self.has_selection() {
            self.move_cursor(self.cursor, false, true);
            return;
        }
        let target = if word {
            prev_word_boundary(&self.text, self.cursor)
        } else {
            self.cursor.saturating_sub(1)
        };
        self.move_cursor(target, shift, true);
    }

    pub fn move_right(&mut self, shift: bool, word: bool) {
        if !shift && self.has_selection() {
            self.move_cursor(self.cursor, false, false);
            return;
        }
        let len = self.len_chars();
        let target = if word {
            next_word_boundary(&self.text, self.cursor)
        } else {
            (self.cursor + 1).min(len)
        };
        self.move_cursor(target, shift, false);
    }

    pub fn current_wrap_cols(&self) -> usize {
        self.layout
            .lock()
            .ok()
            .and_then(|g| g.as_ref().map(|l| l.wrap_chars_per_line))
            .filter(|&c| c >= 6)
            .unwrap_or(48)
    }

    pub fn move_home(&mut self, shift: bool, document: bool, mode: &TextInputMode) {
        if document || matches!(mode, TextInputMode::SingleLine) {
            self.move_cursor(0, shift, true);
            return;
        }
        let lines = wrap_text_lines(&self.text, mode, self.current_wrap_cols());
        let len = self.len_chars();
        let cur = self.cursor.min(len);
        let line_idx = Self::line_index_for_char(&lines, cur);
        let line_start = lines[line_idx].start_char;
        let allow_nl = matches!(
            mode,
            TextInputMode::MultiLine {
                allow_newlines: true,
                ..
            }
        );
        let target = if allow_nl {
            let line_chars: Vec<char> = lines[line_idx].text.chars().collect();
            let first_non_ws = line_chars
                .iter()
                .position(|c| !c.is_whitespace())
                .map(|rel| line_start + rel)
                .unwrap_or(line_start);
            if cur > first_non_ws || cur == line_start {
                if cur == line_start && first_non_ws > line_start {
                    first_non_ws
                } else if cur > first_non_ws {
                    first_non_ws
                } else {
                    line_start
                }
            } else {
                line_start
            }
        } else {
            line_start
        };
        self.move_cursor(target, shift, true);
    }

    pub fn move_end(&mut self, shift: bool, document: bool, mode: &TextInputMode) {
        let len = self.len_chars();
        if document || matches!(mode, TextInputMode::SingleLine) {
            self.move_cursor(len, shift, false);
            return;
        }
        let lines = wrap_text_lines(&self.text, mode, self.current_wrap_cols());
        let cur = self.cursor.min(len);
        let line_idx = Self::line_index_for_char(&lines, cur);
        let target = lines[line_idx].end_char;
        self.move_cursor(target, shift, false);
    }

    pub fn move_vertical_steps(&mut self, up: bool, steps: usize, shift: bool, mode: &TextInputMode) {
        if matches!(mode, TextInputMode::SingleLine) {
            let target = if up { 0 } else { self.len_chars() };
            self.move_cursor(target, shift, up);
            return;
        }
        let lines = wrap_text_lines(&self.text, mode, self.current_wrap_cols());
        if lines.is_empty() {
            return;
        }
        let len = self.len_chars();
        let cur = self.cursor.min(len);
        let line_idx = Self::line_index_for_char(&lines, cur);
        let current_col = cur.saturating_sub(lines[line_idx].start_char);
        let desired_col = self.preferred_col.unwrap_or(current_col);

        let target = if up {
            if line_idx == 0 {
                0
            } else {
                let prev = &lines[line_idx.saturating_sub(steps.max(1))];
                (prev.start_char + desired_col).min(prev.end_char)
            }
        } else if line_idx + 1 >= lines.len() {
            len
        } else {
            let next_idx = (line_idx + steps.max(1)).min(lines.len() - 1);
            let next = &lines[next_idx];
            (next.start_char + desired_col).min(next.end_char)
        };

        self.move_cursor(target, shift, up);
        self.preferred_col = Some(desired_col);
    }

    pub fn move_vertical(&mut self, up: bool, shift: bool, mode: &TextInputMode) {
        self.move_vertical_steps(up, 1, shift, mode);
    }

    pub fn move_page(&mut self, up: bool, shift: bool, mode: &TextInputMode) {
        let page_lines = self
            .layout
            .lock()
            .ok()
            .and_then(|g| {
                g.as_ref().map(|l| {
                    let lh = l.line_height.max(14.0);
                    ((l.content_h / lh).floor() as usize).saturating_sub(1).max(1)
                })
            })
            .unwrap_or(8);
        self.move_vertical_steps(up, page_lines, shift, mode);
    }

    fn line_index_for_char(lines: &[WrappedLine], pos: usize) -> usize {
        for (idx, l) in lines.iter().enumerate() {
            let next_starts_at_end = lines
                .get(idx + 1)
                .map(|next| next.start_char == l.end_char)
                .unwrap_or(false);
            if pos >= l.start_char
                && (pos < l.end_char || (pos == l.end_char && !next_starts_at_end))
            {
                return idx;
            }
        }
        lines.len().saturating_sub(1)
    }

    /// Logical paragraph spans `(line_start_char, line_end_char_excluding_nl)` split by `\n`.
    fn logical_line_spans(chars: &[char]) -> Vec<(usize, usize)> {
        let mut spans = Vec::new();
        let mut start = 0;
        for (i, &c) in chars.iter().enumerate() {
            if c == '\n' {
                spans.push((start, i));
                start = i + 1;
            }
        }
        spans.push((start, chars.len()));
        spans
    }

    fn current_line_indent(&self) -> String {
        let chars: Vec<char> = self.text.chars().collect();
        let cur = self.cursor.min(chars.len());
        let mut ls = cur;
        while ls > 0 && chars[ls - 1] != '\n' {
            ls -= 1;
        }
        chars[ls..cur]
            .iter()
            .take_while(|&&c| c == ' ' || c == '\t')
            .collect()
    }

    pub fn select_current_line(&mut self) {
        let chars: Vec<char> = self.text.chars().collect();
        let cur = self.cursor.min(chars.len());
        let spans = Self::logical_line_spans(&chars);
        let idx = spans
            .iter()
            .position(|&(s, e)| cur >= s && cur <= e)
            .unwrap_or(spans.len().saturating_sub(1));
        let (s, e) = spans[idx];
        let end_incl_nl = if e < chars.len() { e + 1 } else { e };
        self.selection_anchor = Some(s);
        self.cursor = end_incl_nl;
        self.preferred_col = None;
        self.last_edit_kind = TextEditKind::None;
    }

    pub fn delete_current_line(&mut self) -> bool {
        if self.text.is_empty() {
            return false;
        }
        self.record_undo(TextEditKind::Discrete);
        let mut chars: Vec<char> = self.text.chars().collect();
        let spans = Self::logical_line_spans(&chars);
        let cur = self.cursor.min(chars.len());
        let idx = spans
            .iter()
            .position(|&(s, e)| cur >= s && cur <= e)
            .unwrap_or(spans.len().saturating_sub(1));
        let (s, e) = spans[idx];
        let (del_s, del_e) = if e < chars.len() {
            (s, e + 1)
        } else if s > 0 {
            (s - 1, e)
        } else {
            (0, e)
        };
        chars.drain(del_s..del_e);
        self.text = chars.into_iter().collect();
        self.cursor = del_s.min(self.len_chars());
        self.selection_anchor = None;
        self.preferred_col = None;
        true
    }

    pub fn duplicate_lines(&mut self, up: bool) -> bool {
        self.record_undo(TextEditKind::Discrete);
        let chars: Vec<char> = self.text.chars().collect();
        let spans = Self::logical_line_spans(&chars);
        let (sel_s, sel_e) = self
            .selection_range()
            .unwrap_or((self.cursor.min(chars.len()), self.cursor.min(chars.len())));
        let first_idx = spans
            .iter()
            .position(|&(s, e)| sel_s >= s && sel_s <= e)
            .unwrap_or(0);
        let last_idx = spans
            .iter()
            .rposition(|&(s, e)| sel_e >= s && sel_e <= e)
            .unwrap_or(first_idx);
        let block_s = spans[first_idx].0;
        let block_e = spans[last_idx].1;
        let block: Vec<char> = chars[block_s..block_e].to_vec();
        let mut out = Vec::with_capacity(chars.len() + block.len() + 1);
        out.extend_from_slice(&chars[..block_s]);
        out.extend_from_slice(&block);
        out.push('\n');
        out.extend_from_slice(&chars[block_s..]);
        let delta = if up { 0 } else { block.len() + 1 };
        self.text = out.into_iter().collect();
        self.cursor = (self.cursor + delta).min(self.len_chars());
        self.selection_anchor = self
            .selection_anchor
            .map(|a| (a + delta).min(self.len_chars()));
        self.preferred_col = None;
        true
    }

    pub fn move_lines_vertical(&mut self, up: bool) -> bool {
        let chars: Vec<char> = self.text.chars().collect();
        let spans = Self::logical_line_spans(&chars);
        if spans.len() < 2 {
            return false;
        }
        let (sel_s, sel_e) = self
            .selection_range()
            .unwrap_or((self.cursor.min(chars.len()), self.cursor.min(chars.len())));
        let first_idx = spans
            .iter()
            .position(|&(s, e)| sel_s >= s && sel_s <= e)
            .unwrap_or(0);
        let last_idx = spans
            .iter()
            .rposition(|&(s, e)| sel_e >= s && sel_e <= e)
            .unwrap_or(first_idx);

        if (up && first_idx == 0) || (!up && last_idx + 1 >= spans.len()) {
            return false;
        }

        self.record_undo(TextEditKind::Discrete);
        let mut lines_vec: Vec<String> = spans
            .iter()
            .map(|&(s, e)| chars[s..e].iter().collect())
            .collect();

        let (old_s, _) = spans[first_idx];
        let old_cur_rel = self.cursor.saturating_sub(old_s);
        let old_anc_rel = self.selection_anchor.map(|a| a.saturating_sub(old_s));

        if up {
            let swap = lines_vec.remove(first_idx - 1);
            lines_vec.insert(last_idx, swap);
            let new_first_idx = first_idx - 1;
            let new_base: usize = lines_vec[..new_first_idx]
                .iter()
                .map(|l| l.chars().count() + 1)
                .sum();
            self.text = lines_vec.join("\n");
            self.cursor = (new_base + old_cur_rel).min(self.len_chars());
            self.selection_anchor = old_anc_rel.map(|r| (new_base + r).min(self.len_chars()));
        } else {
            let swap = lines_vec.remove(last_idx + 1);
            lines_vec.insert(first_idx, swap);
            let new_first_idx = first_idx + 1;
            let new_base: usize = lines_vec[..new_first_idx]
                .iter()
                .map(|l| l.chars().count() + 1)
                .sum();
            self.text = lines_vec.join("\n");
            self.cursor = (new_base + old_cur_rel).min(self.len_chars());
            self.selection_anchor = old_anc_rel.map(|r| (new_base + r).min(self.len_chars()));
        }
        self.preferred_col = None;
        true
    }

    pub fn indent_lines(&mut self, outdent: bool) -> bool {
        if !outdent && !self.has_selection() {
            self.replace_selection("    ");
            return true;
        }
        let chars: Vec<char> = self.text.chars().collect();
        let spans = Self::logical_line_spans(&chars);
        let (sel_s, sel_e) = self
            .selection_range()
            .unwrap_or((self.cursor.min(chars.len()), self.cursor.min(chars.len())));
        let first_idx = spans
            .iter()
            .position(|&(s, e)| sel_s >= s && sel_s <= e)
            .unwrap_or(0);
        let last_idx = spans
            .iter()
            .rposition(|&(s, e)| sel_e >= s && sel_e <= e)
            .unwrap_or(first_idx);

        let mut lines_vec: Vec<String> = spans
            .iter()
            .map(|&(s, e)| chars[s..e].iter().collect())
            .collect();
        let mut changed = false;
        let mut total_delta: isize = 0;
        let mut first_delta: isize = 0;

        for idx in first_idx..=last_idx {
            if outdent {
                let mut removed = 0usize;
                if lines_vec[idx].starts_with('\t') {
                    lines_vec[idx].remove(0);
                    removed = 1;
                } else {
                    while removed < 4 && lines_vec[idx].starts_with(' ') {
                        lines_vec[idx].remove(0);
                        removed += 1;
                    }
                }
                if removed > 0 {
                    changed = true;
                    let d = -(removed as isize);
                    if idx == first_idx {
                        first_delta = d;
                    }
                    total_delta += d;
                }
            } else {
                lines_vec[idx].insert_str(0, "    ");
                changed = true;
                if idx == first_idx {
                    first_delta = 4;
                }
                total_delta += 4;
            }
        }
        if !changed {
            return false;
        }
        self.record_undo(TextEditKind::Discrete);
        self.text = lines_vec.join("\n");
        let new_len = self.len_chars() as isize;
        if let Some(anc) = self.selection_anchor {
            let (s0, s1) = if anc <= self.cursor {
                (
                    (anc as isize + first_delta).clamp(0, new_len) as usize,
                    (self.cursor as isize + total_delta).clamp(0, new_len) as usize,
                )
            } else {
                (
                    (anc as isize + total_delta).clamp(0, new_len) as usize,
                    (self.cursor as isize + first_delta).clamp(0, new_len) as usize,
                )
            };
            self.selection_anchor = Some(s0);
            self.cursor = s1;
        } else {
            self.cursor = (self.cursor as isize + first_delta).clamp(0, new_len) as usize;
        }
        self.preferred_col = None;
        true
    }

    pub fn backspace(&mut self, word: bool) -> bool {
        if self.delete_selection() {
            return true;
        }
        if self.cursor == 0 || self.text.is_empty() {
            return false;
        }
        self.record_undo(if word {
            TextEditKind::Discrete
        } else {
            TextEditKind::DeleteRun
        });
        let mut chars: Vec<char> = self.text.chars().collect();
        let pos = self.cursor.min(chars.len());
        let del_from = if word {
            prev_word_boundary(&self.text, pos)
        } else {
            pos - 1
        };
        if del_from < pos {
            chars.drain(del_from..pos);
            self.text = chars.into_iter().collect();
            self.cursor = del_from;
            self.preferred_col = None;
            self.completion_index = None;
            return true;
        }
        false
    }

    pub fn delete(&mut self, word: bool) -> bool {
        if self.delete_selection() {
            return true;
        }
        let mut chars: Vec<char> = self.text.chars().collect();
        let pos = self.cursor.min(chars.len());
        if pos >= chars.len() {
            return false;
        }
        self.record_undo(if word {
            TextEditKind::Discrete
        } else {
            TextEditKind::DeleteRun
        });
        let del_to = if word {
            next_word_boundary(&self.text, pos)
        } else {
            pos + 1
        };
        if pos < del_to {
            chars.drain(pos..del_to.min(chars.len()));
            self.text = chars.into_iter().collect();
            self.preferred_col = None;
            self.completion_index = None;
            return true;
        }
        false
    }

    fn filter_inserted_text(raw: &str, opts: &TextInputKeyOptions) -> String {
        let allow_nl = matches!(
            opts.mode,
            TextInputMode::MultiLine {
                allow_newlines: true,
                ..
            }
        );
        raw.chars()
            .filter(|&ch| {
                if ch == '\r' {
                    return false;
                }
                if ch == '\n' {
                    return allow_nl;
                }
                if ch.is_control() {
                    return false;
                }
                if opts.disallow_filename_chars
                    && ['/', '\\', ':', '*', '?', '"', '<', '>', '|'].contains(&ch)
                {
                    return false;
                }
                true
            })
            .collect()
    }

    /// Process a GPUI keystroke cleanly across SingleLine and MultiLine modes.
    pub fn handle_keystroke(
        &mut self,
        keystroke: &gpui::Keystroke,
        cx: &mut App,
        opts: &TextInputKeyOptions,
    ) -> TextInputKeyOutcome {
        let key = keystroke.key.as_str();
        let key_lower = key.to_ascii_lowercase();
        let is_ctrl = keystroke.modifiers.control || keystroke.modifiers.platform;
        let is_shift = keystroke.modifiers.shift;
        let is_alt = keystroke.modifiers.alt;
        let allow_nl = matches!(
            opts.mode,
            TextInputMode::MultiLine {
                allow_newlines: true,
                ..
            }
        );

        if is_alt && !is_ctrl {
            if allow_nl {
                match key_lower.as_str() {
                    "up" | "arrowup" => {
                        let changed = if is_shift {
                            self.duplicate_lines(true)
                        } else {
                            self.move_lines_vertical(true)
                        };
                        return if changed {
                            TextInputKeyOutcome::Changed
                        } else {
                            TextInputKeyOutcome::Handled
                        };
                    }
                    "down" | "arrowdown" => {
                        let changed = if is_shift {
                            self.duplicate_lines(false)
                        } else {
                            self.move_lines_vertical(false)
                        };
                        return if changed {
                            TextInputKeyOutcome::Changed
                        } else {
                            TextInputKeyOutcome::Handled
                        };
                    }
                    _ => {}
                }
            }
            return TextInputKeyOutcome::Ignored;
        }

        if is_ctrl {
            match key_lower.as_str() {
                "z" => {
                    let changed = if is_shift { self.redo() } else { self.undo() };
                    return if changed {
                        TextInputKeyOutcome::Changed
                    } else {
                        TextInputKeyOutcome::Handled
                    };
                }
                "y" => {
                    return if self.redo() {
                        TextInputKeyOutcome::Changed
                    } else {
                        TextInputKeyOutcome::Handled
                    };
                }
                "space" => {
                    if !self.completions.is_empty() && self.cycle_completion(is_shift) {
                        return TextInputKeyOutcome::Changed;
                    }
                    return TextInputKeyOutcome::Handled;
                }
                "k" if is_shift && allow_nl => {
                    return if self.delete_current_line() {
                        TextInputKeyOutcome::Changed
                    } else {
                        TextInputKeyOutcome::Handled
                    };
                }
                "d" if allow_nl => {
                    return if self.duplicate_lines(false) {
                        TextInputKeyOutcome::Changed
                    } else {
                        TextInputKeyOutcome::Handled
                    };
                }
                "l" if allow_nl && !is_shift => {
                    self.select_current_line();
                    return TextInputKeyOutcome::Handled;
                }
                "a" => {
                    self.select_all();
                    return TextInputKeyOutcome::Handled;
                }
                "c" => {
                    if opts.allow_copy {
                        let slice = self.selected_text();
                        if !slice.is_empty() {
                            cx.write_to_clipboard(gpui::ClipboardItem::new_string(slice));
                        }
                    }
                    return TextInputKeyOutcome::Handled;
                }
                "x" => {
                    if opts.allow_copy {
                        let slice = self.selected_text();
                        if !slice.is_empty() {
                            cx.write_to_clipboard(gpui::ClipboardItem::new_string(slice));
                            self.delete_selection();
                            return TextInputKeyOutcome::Changed;
                        }
                    }
                    return TextInputKeyOutcome::Handled;
                }
                "v" => {
                    if let Some(clip) = cx.read_from_clipboard().and_then(|c| c.text()) {
                        let clean = Self::filter_inserted_text(&clip, opts);
                        if !clean.is_empty() {
                            self.replace_selection(&clean);
                            return TextInputKeyOutcome::Changed;
                        }
                    }
                    return TextInputKeyOutcome::Handled;
                }
                "u" => {
                    if !self.text.is_empty() {
                        self.clear();
                        return TextInputKeyOutcome::Changed;
                    }
                    return TextInputKeyOutcome::Handled;
                }
                "left" | "arrowleft" => {
                    self.move_left(is_shift, true);
                    return TextInputKeyOutcome::Handled;
                }
                "right" | "arrowright" => {
                    self.move_right(is_shift, true);
                    return TextInputKeyOutcome::Handled;
                }
                "home" => {
                    self.move_home(is_shift, true, &opts.mode);
                    return TextInputKeyOutcome::Handled;
                }
                "end" => {
                    self.move_end(is_shift, true, &opts.mode);
                    return TextInputKeyOutcome::Handled;
                }
                "backspace" => {
                    return if self.backspace(true) {
                        TextInputKeyOutcome::Changed
                    } else {
                        TextInputKeyOutcome::Handled
                    };
                }
                "delete" => {
                    return if self.delete(true) {
                        TextInputKeyOutcome::Changed
                    } else {
                        TextInputKeyOutcome::Handled
                    };
                }
                "enter" | "return" => {
                    return TextInputKeyOutcome::Submitted;
                }
                _ => return TextInputKeyOutcome::Ignored,
            }
        }

        match key_lower.as_str() {
            "escape" => TextInputKeyOutcome::Cancelled,
            "enter" | "return" => {
                if allow_nl && !opts.submit_on_enter {
                    let indent = self.current_line_indent();
                    let nl = if indent.is_empty() {
                        "\n".to_string()
                    } else {
                        format!("\n{}", indent)
                    };
                    self.replace_selection(&nl);
                    TextInputKeyOutcome::Changed
                } else {
                    TextInputKeyOutcome::Submitted
                }
            }
            "tab" => {
                if opts.tab_autocomplete && !is_shift && self.ghost_completion().is_some() {
                    if self.accept_autocomplete() {
                        return TextInputKeyOutcome::AutocompleteAccepted;
                    }
                }
                if allow_nl && opts.allow_tab_indent {
                    return if self.indent_lines(is_shift) {
                        TextInputKeyOutcome::Changed
                    } else {
                        TextInputKeyOutcome::Handled
                    };
                }
                TextInputKeyOutcome::TabPressed { shift: is_shift }
            }
            "backspace" => {
                if self.backspace(false) {
                    TextInputKeyOutcome::Changed
                } else {
                    TextInputKeyOutcome::Handled
                }
            }
            "delete" => {
                if self.delete(false) {
                    TextInputKeyOutcome::Changed
                } else {
                    TextInputKeyOutcome::Handled
                }
            }
            "left" | "arrowleft" => {
                self.move_left(is_shift, false);
                TextInputKeyOutcome::Handled
            }
            "right" | "arrowright" => {
                self.move_right(is_shift, false);
                TextInputKeyOutcome::Handled
            }
            "up" | "arrowup" => {
                self.move_vertical(true, is_shift, &opts.mode);
                TextInputKeyOutcome::Handled
            }
            "down" | "arrowdown" => {
                self.move_vertical(false, is_shift, &opts.mode);
                TextInputKeyOutcome::Handled
            }
            "pageup" => {
                self.move_page(true, is_shift, &opts.mode);
                TextInputKeyOutcome::Handled
            }
            "pagedown" => {
                self.move_page(false, is_shift, &opts.mode);
                TextInputKeyOutcome::Handled
            }
            "home" => {
                self.move_home(is_shift, false, &opts.mode);
                TextInputKeyOutcome::Handled
            }
            "end" => {
                self.move_end(is_shift, false, &opts.mode);
                TextInputKeyOutcome::Handled
            }
            "space" => {
                self.replace_selection(" ");
                TextInputKeyOutcome::Changed
            }
            _ => {
                let typed_str = keystroke.key_char.as_deref().unwrap_or(key);
                if typed_str.chars().count() == 1 || key.chars().count() == 1 {
                    let mut clean = Self::filter_inserted_text(typed_str, opts);
                    if is_shift {
                        if let Some(ch) = clean.chars().next() {
                            if ch.is_ascii_lowercase() && clean.chars().count() == 1 {
                                clean = ch.to_ascii_uppercase().to_string();
                            }
                        }
                    }
                    if !clean.is_empty() {
                        self.replace_selection(&clean);
                        return TextInputKeyOutcome::Changed;
                    }
                }
                TextInputKeyOutcome::Handled
            }
        }
    }

    /// Map window-space mouse coordinates `(mouse_x, mouse_y)` to a char index using GPUI text shaping.
    pub fn hit_test_point(
        display_text: &str,
        mouse_x: f32,
        mouse_y: f32,
        style: &TextInputStyle,
        layout_opt: &Option<TextInputLayoutInfo>,
        window: &Window,
    ) -> usize {
        if display_text.is_empty() {
            return 0;
        }
        let info = layout_opt.clone().unwrap_or(TextInputLayoutInfo {
            content_left: mouse_x,
            content_top: mouse_y,
            content_w: 220.0,
            content_h: style.line_height,
            rendered_scroll_x: 0.0,
            rendered_scroll_y: 0.0,
            wrap_chars_per_line: ((220.0 / (style.font_size * 0.56)).floor() as usize).max(8),
            font_size: style.font_size,
            line_height: style.line_height,
            mode: TextInputMode::SingleLine,
            masked: false,
            shaped_text_w: 0.0,
            shaped_cursor_x: 0.0,
            text_snapshot_len: 0,
            cursor_snapshot: 0,
            meas_char_w: style.font_size * 0.52,
        });

        let mut text_style = window.text_style();
        text_style.font_family = SharedString::from(style.font_family);
        let font = text_style.font();

        let make_run = |len: usize| TextRun {
            len,
            font: font.clone(),
            color: gpui::white(),
            background_color: None,
            underline: None,
            strikethrough: None,
        };

        match &info.mode {
            TextInputMode::SingleLine => {
                let local_x = (mouse_x - info.content_left + info.rendered_scroll_x).max(0.0);
                let shaped = window.text_system().shape_line(
                    SharedString::from(display_text.to_string()),
                    px(info.font_size),
                    &[make_run(display_text.len())],
                    None,
                );
                let byte_idx = shaped.closest_index_for_x(px(local_x));
                byte_idx_to_char_idx(display_text, byte_idx)
            }
            TextInputMode::MultiLine {
                text_align_center, ..
            } => {
                let lines = wrap_text_lines(display_text, &info.mode, info.wrap_chars_per_line);
                if lines.is_empty() {
                    return 0;
                }
                let total_h = lines.len() as f32 * info.line_height;
                let vertical_offset = if *text_align_center {
                    ((info.content_h - total_h).max(0.0)) * 0.5
                } else {
                    0.0
                };
                let first_line_top = info.content_top + vertical_offset - info.rendered_scroll_y;
                let line_idx = (((mouse_y - first_line_top) / info.line_height).floor() as isize)
                    .clamp(0, (lines.len() - 1) as isize) as usize;
                let line = &lines[line_idx];
                if line.text.is_empty() {
                    return line.start_char;
                }
                let shaped = window.text_system().shape_line(
                    SharedString::from(line.text.clone()),
                    px(info.font_size),
                    &[make_run(line.text.len())],
                    None,
                );
                let line_w: f32 = shaped.width.into();
                let line_left = if *text_align_center {
                    info.content_left + ((info.content_w - line_w).max(0.0)) * 0.5
                } else {
                    info.content_left
                };
                let local_x = (mouse_x - line_left).max(0.0);
                let byte_idx = shaped.closest_index_for_x(px(local_x));
                line.start_char + byte_idx_to_char_idx(&line.text, byte_idx)
            }
        }
    }

    /// Update selection cursor during a pointer move / drag (called both by the widget and window-level drag handler).
    /// NOTE: Keep `selection_anchor` intact while `dragging == true` so tiny jitter over the starting character never wipes out drag-selection!
    pub fn apply_pointer_drag(&mut self, mouse_x: f32, mouse_y: f32, style: &TextInputStyle, window: &Window) {
        if !self.dragging {
            return;
        }
        let layout_opt = self.layout.lock().ok().and_then(|g| g.clone());
        let masked = layout_opt.as_ref().map(|l| l.masked).unwrap_or(false);
        let disp = self.display_string(masked);
        let pos = Self::hit_test_point(&disp, mouse_x, mouse_y, style, &layout_opt, window);
        if self.selection_anchor.is_none() {
            self.selection_anchor = Some(self.cursor.min(self.len_chars()));
        }
        self.cursor = pos;
        self.preferred_col = None;
        self.last_edit_kind = TextEditKind::None;
    }

    /// Update selection cursor using the font metrics stored inside the cached `TextInputLayoutInfo`.
    pub fn apply_pointer_drag_cached(&mut self, mouse_x: f32, mouse_y: f32, window: &Window) {
        if !self.dragging {
            return;
        }
        let layout_opt = self.layout.lock().ok().and_then(|g| g.clone());
        let mut style = TextInputStyle::dialog();
        if let Some(ref l) = layout_opt {
            if l.font_size > 1.0 {
                style.font_size = l.font_size;
            }
            if l.line_height > 1.0 {
                style.line_height = l.line_height;
            }
            if matches!(
                l.mode,
                TextInputMode::MultiLine {
                    text_align_center: true,
                    ..
                }
            ) {
                style.padding_x = 4.0;
                style.padding_y = 2.0;
            }
        }
        let masked = layout_opt.as_ref().map(|l| l.masked).unwrap_or(false);
        let disp = self.display_string(masked);
        let pos = Self::hit_test_point(&disp, mouse_x, mouse_y, &style, &layout_opt, window);
        if self.selection_anchor.is_none() {
            self.selection_anchor = Some(self.cursor.min(self.len_chars()));
        }
        self.cursor = pos;
        self.preferred_col = None;
        self.last_edit_kind = TextEditKind::None;
    }
}

/// Callback hooks for connecting any `TextInputState` inside `AppWindow` to `render_text_input`.
pub type TextInputMutator = Rc<dyn Fn(&mut crate::app::AppWindow) -> Option<&mut TextInputState>>;
pub type TextInputFocusHook = Rc<dyn Fn(&mut crate::app::AppWindow, &mut Window, &mut Context<crate::app::AppWindow>)>;

/// Floating right-click context menu state for any `render_text_input`.
#[derive(Clone)]
pub struct TextInputContextMenuState {
    pub position: gpui::Point<gpui::Pixels>,
    pub access: TextInputMutator,
    pub masked: bool,
    pub allow_newlines: bool,
    pub can_undo: bool,
    pub can_redo: bool,
    pub has_selection: bool,
    pub has_text: bool,
    pub has_autocomplete: bool,
}

impl std::fmt::Debug for TextInputContextMenuState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TextInputContextMenuState")
            .field("position", &self.position)
            .field("can_undo", &self.can_undo)
            .field("can_redo", &self.can_redo)
            .finish()
    }
}

fn char_idx_to_byte_idx(text: &str, char_idx: usize) -> usize {
    text.char_indices()
        .nth(char_idx)
        .map(|(b, _)| b)
        .unwrap_or(text.len())
}

/// Render a unified, interactive single-line or multi-line/textarea input backed by `TextInputState`.
#[allow(clippy::too_many_arguments)]
pub fn render_text_input(
    id: impl Into<ElementId>,
    state: &TextInputState,
    placeholder: impl Into<String>,
    focused: bool,
    masked: bool,
    mode: TextInputMode,
    style: TextInputStyle,
    view: Entity<crate::app::AppWindow>,
    access: TextInputMutator,
    on_focus_click: Option<TextInputFocusHook>,
) -> AnyElement {
    let elem_id: ElementId = id.into();
    let placeholder_str: String = placeholder.into();
    let display_text = state.display_string(masked);
    let total_chars = state.len_chars();
    let clamped_pos = state.cursor.min(total_chars);
    let norm_sel = state.selection_range();
    let inline_ghost = if focused
        && !masked
        && norm_sel.is_none()
        && clamped_pos >= total_chars
        && matches!(mode, TextInputMode::SingleLine)
    {
        state.ghost_completion()
    } else {
        None
    };

    let prev_layout = state.layout.lock().ok().and_then(|g| g.clone());
    let wrap_cols = prev_layout
        .as_ref()
        .map(|l| l.wrap_chars_per_line)
        .filter(|&c| c >= 6)
        .unwrap_or_else(|| match &mode {
            TextInputMode::SingleLine => 1024,
            TextInputMode::MultiLine { .. } => {
                let est_w = prev_layout.as_ref().map(|l| l.content_w).unwrap_or(240.0);
                ((est_w.max(60.0) / (style.font_size * 0.54)).floor() as usize).max(8)
            }
        });

    let layout_arc_canvas = state.layout.clone();
    let layout_arc_down = state.layout.clone();
    let layout_arc_right = state.layout.clone();

    let is_center = matches!(
        mode,
        TextInputMode::MultiLine {
            text_align_center: true,
            ..
        }
    );
    let is_multiline = !matches!(mode, TextInputMode::SingleLine);
    let allow_nl_flag = matches!(
        mode,
        TextInputMode::MultiLine {
            allow_newlines: true,
            ..
        }
    );

    // Horizontal scroll computation:
    // - If `prev_layout` is None (first frame), start at 0.0 instead of guessing a small 240px width
    //   that cuts off the left side of wide inputs like the Omnibar.
    // - Clamp `scroll_x` cleanly to `[0.0, max_scroll]` so text that fits never scrolls off-screen.
    let scroll_x = if matches!(mode, TextInputMode::SingleLine) {
        if let Some(ref prev) = prev_layout {
            let cw = prev.content_w.max(1.0);
            let char_w = if prev.meas_char_w > 1.0 {
                prev.meas_char_w
            } else {
                style.font_size * 0.52
            };
            let est_total_w = if total_chars == prev.text_snapshot_len && prev.shaped_text_w >= 0.0
            {
                prev.shaped_text_w
            } else {
                total_chars as f32 * char_w
            };
            let est_cursor_x = if total_chars == prev.text_snapshot_len
                && clamped_pos == prev.cursor_snapshot
                && prev.shaped_cursor_x >= 0.0
            {
                prev.shaped_cursor_x
            } else if prev.text_snapshot_len > 0 && prev.shaped_text_w > 0.0 {
                (clamped_pos as f32 / prev.text_snapshot_len as f32) * prev.shaped_text_w
            } else {
                clamped_pos as f32 * char_w
            };
            let max_scroll = (est_total_w - cw + 4.0).max(0.0);
            let mut sx = if state.text.is_empty() || clamped_pos == 0 || max_scroll <= 0.0 {
                0.0
            } else {
                prev.rendered_scroll_x.clamp(0.0, max_scroll)
            };
            if est_cursor_x < sx + 4.0 {
                sx = (est_cursor_x - 16.0).max(0.0);
            } else if est_cursor_x > sx + cw - 8.0 {
                sx = (est_cursor_x - cw + 16.0).max(0.0);
            }
            sx.clamp(0.0, max_scroll)
        } else {
            0.0
        }
    } else {
        0.0
    };
    let scroll_y = if is_multiline {
        state.scroll_y.max(0.0)
    } else {
        0.0
    };

    let border_col = if focused {
        rgba(style.border_focused_color)
    } else {
        rgba(style.border_color)
    };

    let mut outer = div()
        .id(elem_id)
        .relative()
        .w_full()
        .min_h(px(style.min_height))
        .px(px(style.padding_x))
        .py(px(style.padding_y))
        .rounded(px(style.rounded))
        .bg(rgba(if focused { style.bg_focused } else { style.bg }))
        .border_1()
        .border_color(border_col)
        .cursor_text();

    if let Some(max_h) = style.max_height {
        outer = outer.max_h(px(max_h));
    }
    if focused && style.bottom_accent_when_focused {
        outer = outer.border_b(px(2.0));
    }
    if is_multiline {
        outer = outer.flex().flex_col();
        if is_center {
            outer = outer.justify_center();
        }
    } else {
        outer = outer.h(px(style.min_height)).flex().flex_row().items_center();
    }

    let style_for_canvas = style.clone();
    let mode_for_canvas = mode.clone();
    let display_text_canvas = display_text.clone();
    let style_for_down = style.clone();
    let style_for_right = style.clone();
    let style_for_move = style.clone();
    let access_down = access.clone();
    let access_right = access.clone();
    let access_move = access.clone();
    let access_up = access.clone();
    let access_wheel = access;
    let on_focus_right = on_focus_click.clone();
    let view_down = view.clone();
    let view_right = view.clone();
    let view_move = view.clone();
    let view_up = view.clone();
    let view_wheel = view;

    outer = outer
        .on_mouse_down(MouseButton::Left, move |ev, win, cx| {
            cx.stop_propagation();
            let mx: f32 = ev.position.x.into();
            let my: f32 = ev.position.y.into();
            let shift = ev.modifiers.shift;
            let clicks = ev.click_count;
            let l_arc = layout_arc_down.clone();
            let style_ref = style_for_down.clone();
            let acc = access_down.clone();
            let focus_hook = on_focus_click.clone();
            view_down.update(cx, |this, cx| {
                this.state.text_context_menu = None;
                if let Some(hook) = focus_hook {
                    hook(this, win, cx);
                } else if let Some(ref focus) = this.state.main_focus {
                    win.focus(focus);
                }
                let layout_snapshot = l_arc.lock().ok().and_then(|g| g.clone());
                if let Some(ti) = acc(this) {
                    let masked_flag = layout_snapshot.as_ref().map(|l| l.masked).unwrap_or(masked);
                    let disp = ti.display_string(masked_flag);
                    let clicked = TextInputState::hit_test_point(
                        &disp,
                        mx,
                        my,
                        &style_ref,
                        &layout_snapshot,
                        win,
                    );
                    ti.last_edit_kind = TextEditKind::None;
                    if clicks >= 3 {
                        ti.select_all();
                        ti.dragging = false;
                    } else if clicks == 2 {
                        let (ws, we) = word_bounds_at(&disp, clicked);
                        ti.selection_anchor = if ws != we { Some(ws) } else { None };
                        ti.cursor = we;
                        ti.dragging = false;
                        ti.preferred_col = None;
                    } else if shift {
                        ti.move_cursor(clicked, true, false);
                        if ti.selection_anchor.is_none() {
                            ti.selection_anchor = Some(clicked);
                        }
                        ti.dragging = true;
                    } else {
                        ti.selection_anchor = Some(clicked);
                        ti.cursor = clicked;
                        ti.dragging = true;
                        ti.preferred_col = None;
                    }
                    cx.notify();
                }
            });
        })
        .on_mouse_down(MouseButton::Right, move |ev, win, cx| {
            cx.stop_propagation();
            let mx: f32 = ev.position.x.into();
            let my: f32 = ev.position.y.into();
            let pos = ev.position;
            let l_arc = layout_arc_right.clone();
            let style_ref = style_for_right.clone();
            let acc = access_right.clone();
            let acc_menu = acc.clone();
            let focus_hook = on_focus_right.clone();
            view_right.update(cx, |this, cx| {
                if let Some(hook) = focus_hook {
                    hook(this, win, cx);
                } else if let Some(ref focus) = this.state.main_focus {
                    win.focus(focus);
                }
                let layout_snapshot = l_arc.lock().ok().and_then(|g| g.clone());
                if let Some(ti) = acc(this) {
                    ti.dragging = false;
                    let masked_flag = layout_snapshot.as_ref().map(|l| l.masked).unwrap_or(masked);
                    let disp = ti.display_string(masked_flag);
                    let clicked = TextInputState::hit_test_point(
                        &disp,
                        mx,
                        my,
                        &style_ref,
                        &layout_snapshot,
                        win,
                    );
                    let in_selection = ti
                        .selection_range()
                        .map(|(s, e)| clicked >= s && clicked <= e)
                        .unwrap_or(false);
                    if !in_selection {
                        ti.selection_anchor = None;
                        ti.cursor = clicked;
                        ti.preferred_col = None;
                    }
                    this.state.text_context_menu = Some(TextInputContextMenuState {
                        position: pos,
                        access: acc_menu,
                        masked: masked_flag,
                        allow_newlines: allow_nl_flag,
                        can_undo: ti.can_undo(),
                        can_redo: ti.can_redo(),
                        has_selection: ti.has_selection(),
                        has_text: !ti.text.is_empty(),
                        has_autocomplete: ti.ghost_completion().is_some(),
                    });
                    cx.notify();
                }
            });
        })
        .on_mouse_move(move |ev, win, cx| {
            let mx: f32 = ev.position.x.into();
            let my: f32 = ev.position.y.into();
            let pressed_left = ev.dragging();
            let style_ref = style_for_move.clone();
            let acc = access_move.clone();
            view_move.update(cx, |this, cx| {
                if let Some(ti) = acc(this) {
                    if ti.dragging {
                        if !pressed_left {
                            ti.dragging = false;
                            if ti.selection_anchor == Some(ti.cursor) {
                                ti.selection_anchor = None;
                            }
                            cx.notify();
                        } else {
                            ti.apply_pointer_drag(mx, my, &style_ref, win);
                            cx.notify();
                        }
                    }
                }
            });
        })
        .on_mouse_up(MouseButton::Left, move |_, _, cx| {
            let acc = access_up.clone();
            view_up.update(cx, |this, cx| {
                if let Some(ti) = acc(this) {
                    if ti.dragging {
                        cx.stop_propagation();
                        ti.dragging = false;
                        if ti.selection_anchor == Some(ti.cursor) {
                            ti.selection_anchor = None;
                        }
                        cx.notify();
                    }
                }
            });
        });

    if is_multiline {
        let mode_wheel = mode.clone();
        let line_h = style.line_height;
        outer = outer.on_scroll_wheel(move |ev, _, cx| {
            cx.stop_propagation();
            let delta = ev.delta.pixel_delta(px(line_h));
            let dy: f32 = delta.y.into();
            if dy.abs() < 0.25 {
                return;
            }
            let acc = access_wheel.clone();
            let m = mode_wheel.clone();
            view_wheel.update(cx, |this, cx| {
                if let Some(ti) = acc(this) {
                    let lines = wrap_text_lines(&ti.text, &m, ti.current_wrap_cols());
                    let view_h = ti
                        .layout
                        .lock()
                        .ok()
                        .and_then(|g| g.as_ref().map(|l| l.content_h))
                        .unwrap_or(60.0);
                    let max_scroll = (lines.len() as f32 * line_h - view_h).max(0.0);
                    ti.scroll_y = (ti.scroll_y - dy).clamp(0.0, max_scroll);
                    cx.notify();
                }
            });
        });
    }

    // Build inner viewport containing measured canvas + rendered lines
    let mut viewport = div()
        .relative()
        .w_full()
        .flex_1()
        .min_w(px(0.0))
        .overflow_hidden()
        .flex()
        .flex_col()
        .child(
            gpui::canvas(
                move |bounds, win, _cx| {
                    let cw = f32::from(bounds.size.width).max(1.0);
                    let ch = f32::from(bounds.size.height).max(1.0);
                    let mut text_style = win.text_style();
                    text_style.font_family = SharedString::from(style_for_canvas.font_family);
                    let font = text_style.font();
                    let sample = win.text_system().shape_line(
                        SharedString::from("Mg00"),
                        px(style_for_canvas.font_size),
                        &[TextRun {
                            len: 4,
                            font: font.clone(),
                            color: gpui::white(),
                            background_color: None,
                            underline: None,
                            strikethrough: None,
                        }],
                        None,
                    );
                    let meas_cw = (f32::from(sample.width) / 4.0)
                        .clamp(4.0, style_for_canvas.font_size * 0.9);
                    let wrap_c = ((cw / meas_cw).floor() as usize).max(6);

                    let (shaped_w, shaped_cur_x, ideal_sx) =
                        if matches!(mode_for_canvas, TextInputMode::SingleLine)
                            && !display_text_canvas.is_empty()
                        {
                            let shaped = win.text_system().shape_line(
                                SharedString::from(display_text_canvas.clone()),
                                px(style_for_canvas.font_size),
                                &[TextRun {
                                    len: display_text_canvas.len(),
                                    font: font.clone(),
                                    color: gpui::white(),
                                    background_color: None,
                                    underline: None,
                                    strikethrough: None,
                                }],
                                None,
                            );
                            let tw = f32::from(shaped.width);
                            let cur_b = char_idx_to_byte_idx(&display_text_canvas, clamped_pos);
                            let cx_px = f32::from(shaped.x_for_index(cur_b));
                            let max_s = (tw - cw + 4.0).max(0.0);
                            let mut sx = if max_s <= 0.0 || clamped_pos == 0 {
                                0.0
                            } else {
                                scroll_x.clamp(0.0, max_s)
                            };
                            if cx_px < sx + 4.0 {
                                sx = (cx_px - 16.0).max(0.0);
                            } else if cx_px > sx + cw - 8.0 {
                                sx = (cx_px - cw + 16.0).max(0.0);
                            }
                            sx = sx.clamp(0.0, max_s);
                            (tw, cx_px, sx)
                        } else {
                            (0.0, 0.0, 0.0)
                        };

                    let need_refresh = matches!(mode_for_canvas, TextInputMode::SingleLine)
                        && (ideal_sx - scroll_x).abs() > 1.5;

                    if let Ok(mut slot) = layout_arc_canvas.lock() {
                        *slot = Some(TextInputLayoutInfo {
                            content_left: f32::from(bounds.origin.x),
                            content_top: f32::from(bounds.origin.y),
                            content_w: cw,
                            content_h: ch,
                            rendered_scroll_x: ideal_sx,
                            rendered_scroll_y: scroll_y,
                            wrap_chars_per_line: wrap_c,
                            font_size: style_for_canvas.font_size,
                            line_height: style_for_canvas.line_height,
                            mode: mode_for_canvas.clone(),
                            masked,
                            shaped_text_w: shaped_w,
                            shaped_cursor_x: shaped_cur_x,
                            text_snapshot_len: total_chars,
                            cursor_snapshot: clamped_pos,
                            meas_char_w: meas_cw,
                        });
                    }
                    if need_refresh {
                        win.refresh();
                    }
                },
                |_, _, _, _| {},
            )
            .absolute()
            .inset_0(),
        );

    if is_multiline && is_center {
        viewport = viewport.justify_center();
    } else if !is_multiline {
        viewport = viewport.h_full().justify_center();
    }

    if display_text.is_empty() {
        let ph_line = if !focused {
            div()
                .text_size(px(style.font_size))
                .line_height(px(style.line_height))
                .font_family(style.font_family)
                .text_color(rgba(style.placeholder_color))
                .truncate()
                .child(placeholder_str)
                .into_any_element()
        } else {
            let caret = render_caret_bar(style.line_height, style.cursor_color);
            let mut row = div()
                .relative()
                .w_full()
                .flex()
                .flex_row()
                .items_center()
                .child(caret);
            if !placeholder_str.is_empty() {
                row = row.child(
                    div()
                        .ml(px(2.0))
                        .text_size(px(style.font_size))
                        .line_height(px(style.line_height))
                        .font_family(style.font_family)
                        .text_color(rgba(style.placeholder_color))
                        .truncate()
                        .child(placeholder_str),
                );
            }
            if is_center {
                row = row.justify_center();
            }
            row.into_any_element()
        };
        viewport = viewport.child(ph_line);
    } else {
        let lines = wrap_text_lines(&display_text, &mode, wrap_cols);
        let num_lines = lines.len();
        let mut lines_col = div().flex().flex_col();
        if !is_multiline {
            lines_col = lines_col.ml(px(-scroll_x));
        } else {
            lines_col = lines_col.mt(px(-scroll_y));
        }

        for (line_idx, line) in lines.iter().enumerate() {
            let is_last_line = line_idx + 1 == num_lines;
            let next_starts_at_end = lines
                .get(line_idx + 1)
                .map(|next| next.start_char == line.end_char)
                .unwrap_or(false);
            let is_active_line = clamped_pos >= line.start_char
                && (clamped_pos < line.end_char
                    || (clamped_pos == line.end_char && !next_starts_at_end)
                    || (is_last_line && clamped_pos >= line.end_char));
            let line_chars: Vec<char> = line.text.chars().collect();
            let line_sel = norm_sel.and_then(|(s0, s1)| {
                let is0 = s0.max(line.start_char);
                let is1 = s1.min(line.end_char);
                if is0 < is1 {
                    Some((
                        is0 - line.start_char,
                        (is1 - line.start_char).min(line_chars.len()),
                    ))
                } else {
                    None
                }
            });

            let mut text_block = div()
                .relative()
                .flex()
                .items_center()
                .whitespace_nowrap();

            if let Some((ls0, ls1)) = line_sel {
                let before_sel: String = line_chars[..ls0].iter().collect();
                let sel_slice: String = line_chars[ls0..ls1].iter().collect();
                text_block = text_block.child(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .bottom_0()
                        .flex()
                        .flex_row()
                        .items_center()
                        .child(
                            div()
                                .opacity(0.0)
                                .text_size(px(style.font_size))
                                .line_height(px(style.line_height))
                                .font_family(style.font_family)
                                .whitespace_nowrap()
                                .child(before_sel),
                        )
                        .child(
                            div()
                                .bg(rgb(style.selection_bg))
                                .rounded(px(2.0))
                                .text_color(rgba(0x00000000))
                                .text_size(px(style.font_size))
                                .line_height(px(style.line_height))
                                .font_family(style.font_family)
                                .whitespace_nowrap()
                                .child(sel_slice),
                        ),
                );
            }

            text_block = text_block.child(
                div()
                    .text_size(px(style.font_size))
                    .line_height(px(style.line_height))
                    .font_family(style.font_family)
                    .text_color(rgba(style.text_color))
                    .whitespace_nowrap()
                    .child(if line.text.is_empty() {
                        " ".to_string()
                    } else {
                        line.text.clone()
                    }),
            );

            if is_last_line {
                if let Some(ref ghost) = inline_ghost {
                    text_block = text_block.child(
                        div()
                            .text_size(px(style.font_size))
                            .line_height(px(style.line_height))
                            .font_family(style.font_family)
                            .text_color(rgba(0xFFFFFF4Au32))
                            .whitespace_nowrap()
                            .child(ghost.clone()),
                    );
                }
            }

            if focused && is_active_line {
                let col = clamped_pos
                    .saturating_sub(line.start_char)
                    .min(line_chars.len());
                let before: String = line_chars[..col].iter().collect();
                let caret = render_caret_bar(style.line_height, style.cursor_color);
                text_block = text_block.child(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .bottom_0()
                        .flex()
                        .flex_row()
                        .items_center()
                        .child(
                            div()
                                .opacity(0.0)
                                .text_size(px(style.font_size))
                                .line_height(px(style.line_height))
                                .font_family(style.font_family)
                                .whitespace_nowrap()
                                .child(before),
                        )
                        .child(caret),
                );
            }

            let mut line_wrap = div()
                .w_full()
                .min_h(px(style.line_height))
                .flex()
                .items_center();
            if is_center {
                line_wrap = line_wrap.justify_center();
            }
            lines_col = lines_col.child(line_wrap.child(text_block));
        }

        viewport = viewport.child(lines_col);
    }

    outer.child(viewport).into_any_element()
}

/// Stationary solid text caret bar (non-blinking).
fn render_caret_bar(line_height: f32, color_rgb: u32) -> Div {
    div()
        .w(px(0.0))
        .min_w(px(0.0))
        .h(px(line_height.max(14.0)))
        .flex_shrink_0()
        .child(
            div()
                .absolute()
                .left(px(-0.75))
                .w(px(1.5))
                .h(px(line_height.max(14.0)))
                .bg(rgb(color_rgb)),
        )
}

/// Render the floating Fluent right-click context menu for any active text field.
pub fn render_text_context_menu(
    menu: &TextInputContextMenuState,
    view: Entity<crate::app::AppWindow>,
) -> AnyElement {
    #[derive(Clone, Copy)]
    enum TextMenuAction {
        Undo,
        Redo,
        Cut,
        Copy,
        Paste,
        Delete,
        SelectAll,
        Complete,
    }

    let make_row = |glyph: &'static str,
                    label: &'static str,
                    shortcut: &'static str,
                    enabled: bool,
                    action: TextMenuAction| {
        let acc = menu.access.clone();
        let v = view.clone();
        let masked = menu.masked;
        let allow_nl = menu.allow_newlines;
        let mut row = div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .h(px(28.0))
            .px(px(10.0))
            .rounded(px(5.0))
            .text_size(px(12.5))
            .text_color(rgba(if enabled {
                UiConstants::TEXT_PRIMARY_RGBA
            } else {
                0xFFFFFF44u32
            }));

        if enabled {
            row = row
                .cursor_pointer()
                .hover(|s| s.bg(rgba(0xFFFFFF18u32)))
                .on_mouse_down(MouseButton::Left, move |_, win, cx| {
                    cx.stop_propagation();
                    let acc_inner = acc.clone();
                    v.update(cx, |this, cx| {
                        this.state.text_context_menu = None;
                        if let Some(ref focus) = this.state.main_focus {
                            win.focus(focus);
                        }
                        if let Some(ti) = acc_inner(this) {
                            match action {
                                TextMenuAction::Undo => {
                                    ti.undo();
                                }
                                TextMenuAction::Redo => {
                                    ti.redo();
                                }
                                TextMenuAction::Cut => {
                                    if !masked {
                                        let slice = ti.selected_text();
                                        if !slice.is_empty() {
                                            cx.write_to_clipboard(
                                                gpui::ClipboardItem::new_string(slice),
                                            );
                                            ti.delete_selection();
                                        }
                                    }
                                }
                                TextMenuAction::Copy => {
                                    if !masked {
                                        let slice = ti.selected_text();
                                        if !slice.is_empty() {
                                            cx.write_to_clipboard(
                                                gpui::ClipboardItem::new_string(slice),
                                            );
                                        }
                                    }
                                }
                                TextMenuAction::Paste => {
                                    if let Some(clip) =
                                        cx.read_from_clipboard().and_then(|c| c.text())
                                    {
                                        let opts = TextInputKeyOptions {
                                            mode: if allow_nl {
                                                TextInputMode::MultiLine {
                                                    max_lines: None,
                                                    allow_newlines: true,
                                                    text_align_center: false,
                                                }
                                            } else {
                                                TextInputMode::SingleLine
                                            },
                                            ..Default::default()
                                        };
                                        let clean =
                                            TextInputState::filter_inserted_text(&clip, &opts);
                                        if !clean.is_empty() {
                                            ti.replace_selection(&clean);
                                        }
                                    }
                                }
                                TextMenuAction::Delete => {
                                    if !ti.delete_selection() {
                                        ti.delete(false);
                                    }
                                }
                                TextMenuAction::SelectAll => {
                                    ti.select_all();
                                }
                                TextMenuAction::Complete => {
                                    ti.accept_autocomplete();
                                }
                            }
                        }
                        this.sync_after_text_input_change();
                        cx.notify();
                    });
                });
        } else {
            row = row.on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation());
        }

        row.child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(9.0))
                .child(
                    div()
                        .w(px(14.0))
                        .font_family(UiConstants::FONT_ICONS)
                        .text_size(px(12.0))
                        .text_color(rgba(if enabled {
                            UiConstants::TEXT_SECONDARY_RGBA
                        } else {
                            0xFFFFFF38u32
                        }))
                        .child(glyph),
                )
                .child(label),
        )
        .child(
            div()
                .ml(px(18.0))
                .text_size(px(11.0))
                .text_color(rgba(0xFFFFFF66u32))
                .child(shortcut),
        )
    };

    let v_dismiss = view.clone();
    let sep = || {
        div()
            .my(px(4.0))
            .h(px(1.0))
            .w_full()
            .bg(rgba(UiConstants::DIVIDER_RGBA))
    };

    let mut card = div()
        .w(px(214.0))
        .p(px(5.0))
        .rounded(px(8.0))
        .bg(rgba(0x262626FAu32))
        .border_1()
        .border_color(rgba(0xFFFFFF22u32))
        .shadow_lg()
        .flex()
        .flex_col()
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
        .child(make_row("\u{E7A7}", "Undo", "Ctrl+Z", menu.can_undo, TextMenuAction::Undo))
        .child(make_row("\u{E7A6}", "Redo", "Ctrl+Y", menu.can_redo, TextMenuAction::Redo))
        .child(sep())
        .child(make_row(
            "\u{E8C6}",
            "Cut",
            "Ctrl+X",
            menu.has_selection && !menu.masked,
            TextMenuAction::Cut,
        ))
        .child(make_row(
            "\u{E8C8}",
            "Copy",
            "Ctrl+C",
            menu.has_selection && !menu.masked,
            TextMenuAction::Copy,
        ))
        .child(make_row("\u{E77F}", "Paste", "Ctrl+V", true, TextMenuAction::Paste))
        .child(make_row(
            "\u{E74D}",
            "Delete",
            "Del",
            menu.has_selection || menu.has_text,
            TextMenuAction::Delete,
        ))
        .child(sep())
        .child(make_row(
            "\u{E8B3}",
            "Select All",
            "Ctrl+A",
            menu.has_text,
            TextMenuAction::SelectAll,
        ));

    if menu.has_autocomplete {
        card = card.child(make_row(
            "\u{E76C}",
            "Complete Suggestion",
            "Tab",
            true,
            TextMenuAction::Complete,
        ));
    }

    div()
        .id("unified-text-context-menu-overlay")
        .absolute()
        .inset_0()
        .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            cx.stop_propagation();
            v_dismiss.update(cx, |this, cx| {
                this.state.text_context_menu = None;
                cx.notify();
            });
        })
        .on_mouse_down(MouseButton::Right, {
            let v_d2 = view;
            move |_, _, cx| {
                cx.stop_propagation();
                v_d2.update(cx, |this, cx| {
                    this.state.text_context_menu = None;
                    cx.notify();
                });
            }
        })
        .child(
            div()
                .absolute()
                .left(menu.position.x)
                .top(menu.position.y)
                .child(card),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::{wrap_text_lines, TextInputMode, TextInputState};

    #[::core::prelude::rust_2024::test]
    fn test_single_line_selection_word_jump_and_edit() {
        let mut st = TextInputState::new("hello_world_file.txt");
        st.move_cursor(11, false, true);
        // Ctrl+Shift+Left stops at delimiter '_' ("world"), then extends to "hello_world"
        st.move_left(true, true);
        assert_eq!(st.selected_text(), "world");
        st.move_left(true, true);
        assert_eq!(st.selected_text(), "hello_world");

        // Typing replaces selection
        st.replace_selection("demo");
        assert_eq!(st.text, "demo_file.txt");
        assert_eq!(st.cursor, 4);
        assert_eq!(st.selection_range(), None);

        // Shift+End selects "_file.txt"
        st.move_end(true, false, &TextInputMode::SingleLine);
        assert_eq!(st.selected_text(), "_file.txt");
        assert!(st.delete_selection());
        assert_eq!(st.text, "demo");
    }

    #[::core::prelude::rust_2024::test]
    fn test_undo_redo_coalescing_and_discrete() {
        let mut st = TextInputState::new("abc");
        st.replace_selection("d");
        st.replace_selection("e");
        st.replace_selection("f");
        assert_eq!(st.text, "abcdef");
        // Single-char typing coalesces into one undo step
        assert!(st.undo());
        assert_eq!(st.text, "abc");
        assert!(st.redo());
        assert_eq!(st.text, "abcdef");

        // Discrete replace after space starts a new undo entry
        st.replace_selection(" ");
        st.replace_selection("xyz");
        assert_eq!(st.text, "abcdef xyz");
        assert!(st.undo());
        assert_eq!(st.text, "abcdef ");
        assert!(st.undo());
        assert_eq!(st.text, "abcdef");
        assert!(st.redo());
        assert_eq!(st.text, "abcdef ");
    }

    #[::core::prelude::rust_2024::test]
    fn test_editor_multiline_features_indent_move_dup_delete_line() {
        let mut st = TextInputState::new("one\ntwo\nthree");
        st.cursor = 4; // on "two"
        assert!(st.indent_lines(false));
        assert_eq!(st.text, "one\n    two\nthree");
        assert!(st.move_lines_vertical(true));
        assert_eq!(st.text, "    two\none\nthree");
        assert!(st.duplicate_lines(false));
        assert_eq!(st.text, "    two\n    two\none\nthree");
        assert!(st.delete_current_line());
        assert_eq!(st.text, "    two\none\nthree");
        assert!(st.undo());
        assert_eq!(st.text, "    two\n    two\none\nthree");
    }

    #[::core::prelude::rust_2024::test]
    fn test_autocomplete_ghost_and_accept() {
        let mut st = TextInputState::new("C:\\Users\\ro");
        st.completions = vec![
            "C:\\Users\\rony0".to_string(),
            "C:\\Users\\root".to_string(),
        ];
        assert_eq!(st.ghost_completion().as_deref(), Some("ny0"));
        assert!(st.accept_autocomplete());
        assert_eq!(st.text, "C:\\Users\\rony0");
        assert!(st.undo());
        assert_eq!(st.text, "C:\\Users\\ro");
    }

    #[::core::prelude::rust_2024::test]
    fn test_multiline_textarea_newlines_wrapping_and_vertical_nav() {
        let mode = TextInputMode::MultiLine {
            max_lines: None,
            allow_newlines: true,
            text_align_center: false,
        };
        let mut st = TextInputState::new("alpha beta\ngamma\ndelta epsilon");
        let lines = wrap_text_lines(&st.text, &mode, 20);
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0].text, "alpha beta");
        assert_eq!(lines[1].text, "gamma");
        assert_eq!(lines[2].text, "delta epsilon");

        // Place cursor at "beta" col 7 on line 0, move down -> "gamma" clamped to col 5 (pos 16) -> down -> "delta epsilon" col 7 (pos 24)
        st.cursor = 7;
        st.move_vertical(false, false, &mode);
        assert_eq!(st.cursor, 16);
        st.move_vertical(false, true, &mode); // Shift+Down selects from 16 to line 2 col 7 (24)
        assert_eq!(st.cursor, 24);
        assert_eq!(st.selected_text(), "\ndelta e");
    }

    #[::core::prelude::rust_2024::test]
    fn test_masked_display_preserves_newlines() {
        let st = TextInputState::new("secret\nkey");
        assert_eq!(st.display_string(true), "••••••\n•••");
        assert_eq!(st.display_string(false), "secret\nkey");
    }
}

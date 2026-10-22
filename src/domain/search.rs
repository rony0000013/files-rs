use crate::core::path_util::FilesPath;
use crate::domain::extended_metadata::ExtendedMetadataExtractor;
use crate::domain::metadata::FileItem;
use crate::storage::storage::FileType;
use crate::ui::views::types::{SearchResult, SearchState};
use nucleo::{Config, Matcher, Utf32Str};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

// ═══════════════════════════════════════════════════════════════════════════
// SEARCH OPTIONS & AQS FILTER PARSER
// ═══════════════════════════════════════════════════════════════════════════

/// Interactive search options controlled by the Omnibar toggle pills or inline prefixes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SearchOptions {
    /// Treat the search pattern as a Regular Expression (`.*` pill or `re:` / `/.../` syntax)
    pub use_regex: bool,
    /// Match character case (`Aa` pill)
    pub case_sensitive: bool,
    /// Search inside file text contents (`Txt` pill or `content:` syntax)
    pub search_content: bool,
    /// Search across extended file metadata: EXIF, ID3, PDF, dimensions, tags, dates, types (`Meta` pill or `meta:` syntax)
    pub search_metadata: bool,
    /// Recurse into subdirectories (`Sub` pill)
    pub recursive: bool,
    /// Include hidden and dot-prefixed files
    pub include_hidden: bool,
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self {
            use_regex: false,
            case_sensitive: false,
            search_content: false,
            search_metadata: false,
            recursive: true,
            include_hidden: false,
        }
    }
}

/// Parsed AQS (Advanced Query Syntax) filter constraints + extracted pattern query.
/// Supports Windows Explorer & Files-community AQS syntax:
/// `kind:`, `ext:`, `size:`, `date:`, `modified:`, `created:`, `tag:`, `NOT tag:`,
/// `content:`, `meta:`, `author:`, `camera:`, `album:`, `dimensions:`, `re:`,
/// `is:dir`, `is:file`, `is:hidden`, `is:readonly`, `is:empty`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AqsFilters {
    pub kind: Option<String>,
    pub extension: Option<String>,
    pub extensions: Vec<String>,
    pub size_min: Option<u64>,
    pub size_max: Option<u64>,
    pub date_after: Option<i64>,
    pub date_before: Option<i64>,
    pub created_after: Option<i64>,
    pub created_before: Option<i64>,
    pub tags_include: Vec<String>,
    pub tags_exclude: Vec<String>,
    pub content_query: Option<String>,
    pub metadata_query: Option<String>,
    pub regex_pattern: Option<String>,
    pub is_dir: Option<bool>,
    pub is_hidden: Option<bool>,
    pub is_readonly: Option<bool>,
    pub is_empty: Option<bool>,
    /// Remaining non-AQS search terms (filename/wildcard/regex query)
    pub text_query: String,
}

impl AqsFilters {
    /// Returns true if any structured AQS filter constraint is active.
    pub fn has_structured_filters(&self) -> bool {
        self.kind.is_some()
            || self.extension.is_some()
            || !self.extensions.is_empty()
            || self.size_min.is_some()
            || self.size_max.is_some()
            || self.date_after.is_some()
            || self.date_before.is_some()
            || self.created_after.is_some()
            || self.created_before.is_some()
            || !self.tags_include.is_empty()
            || !self.tags_exclude.is_empty()
            || self.content_query.is_some()
            || self.metadata_query.is_some()
            || self.regex_pattern.is_some()
            || self.is_dir.is_some()
            || self.is_hidden.is_some()
            || self.is_readonly.is_some()
            || self.is_empty.is_some()
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// ZERO-BACKTRACKING THOMPSON NFA REGULAR EXPRESSION ENGINE
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
enum RegexAtom {
    Any,
    Literal(char),
    CharClass {
        ranges: Vec<(char, char)>,
        negated: bool,
    },
    Digit(bool),
    Word(bool),
    Space(bool),
    Group(Vec<Vec<RegexPiece>>), // Alternation branches
}

#[derive(Debug, Clone)]
struct RegexPiece {
    atom: RegexAtom,
    min: usize,
    max: Option<usize>, // None = unbounded
}

#[derive(Debug, Clone)]
struct RegexBranch {
    anchor_start: bool,
    anchor_end: bool,
    pieces: Vec<RegexPiece>,
}

/// Fast compiled Regular Expression matcher supporting standard regex syntax:
/// `^`, `$`, `.`, `*`, `+`, `?`, `{m,n}`, `[a-z0-9_]`, `[^...]`, `\d`, `\D`, `\w`, `\W`, `\s`, `\S`,
/// `(a|b|c)` grouping and alternation, and case-sensitive / case-insensitive matching.
#[derive(Debug, Clone)]
pub struct SimpleRegex {
    branches: Vec<RegexBranch>,
    case_sensitive: bool,
}

impl SimpleRegex {
    pub fn compile(pattern: &str, mut case_sensitive: bool) -> Result<Self, String> {
        let mut pat = pattern.trim();
        // Support `/pattern/i` or `/pattern/` slash notation
        if pat.starts_with('/') && pat.len() >= 2 {
            if let Some(last_slash) = pat[1..].rfind('/') {
                let flags = &pat[1 + last_slash + 1..];
                if flags.chars().all(|c| matches!(c, 'i' | 'g' | 'm' | 's')) {
                    if flags.contains('i') {
                        case_sensitive = false;
                    }
                    pat = &pat[1..1 + last_slash];
                }
            }
        }
        // Support inline `(?i)` or `(?-i)` prefix
        if let Some(rest) = pat.strip_prefix("(?i)") {
            case_sensitive = false;
            pat = rest;
        } else if let Some(rest) = pat.strip_prefix("(?-i)") {
            case_sensitive = true;
            pat = rest;
        }

        let chars: Vec<char> = pat.chars().collect();
        let branches = Self::parse_alternation(&chars, case_sensitive)?;
        Ok(Self {
            branches,
            case_sensitive,
        })
    }

    fn parse_alternation(chars: &[char], case_sensitive: bool) -> Result<Vec<RegexBranch>, String> {
        let mut branches = Vec::new();
        let mut depth = 0i32;
        let mut in_class = false;
        let mut start = 0usize;
        let mut i = 0usize;

        while i < chars.len() {
            let c = chars[i];
            if c == '\\' {
                i += 2;
                continue;
            }
            if c == '[' && !in_class {
                in_class = true;
            } else if c == ']' && in_class {
                in_class = false;
            } else if !in_class {
                if c == '(' {
                    depth += 1;
                } else if c == ')' {
                    depth = (depth - 1).max(0);
                } else if c == '|' && depth == 0 {
                    branches.push(Self::parse_branch(&chars[start..i], case_sensitive)?);
                    start = i + 1;
                }
            }
            i += 1;
        }
        branches.push(Self::parse_branch(&chars[start..], case_sensitive)?);
        Ok(branches)
    }

    fn parse_branch(chars: &[char], case_sensitive: bool) -> Result<RegexBranch, String> {
        let mut slice = chars;
        let mut anchor_start = false;
        let mut anchor_end = false;

        if slice.first() == Some(&'^') {
            anchor_start = true;
            slice = &slice[1..];
        }
        if slice.last() == Some(&'$') && (slice.len() < 2 || slice[slice.len() - 2] != '\\') {
            anchor_end = true;
            slice = &slice[..slice.len() - 1];
        }

        let pieces = Self::parse_pieces(slice, case_sensitive)?;
        Ok(RegexBranch {
            anchor_start,
            anchor_end,
            pieces,
        })
    }

    fn parse_pieces(chars: &[char], case_sensitive: bool) -> Result<Vec<RegexPiece>, String> {
        let mut pieces = Vec::new();
        let mut i = 0usize;

        while i < chars.len() {
            let c = chars[i];
            let atom = match c {
                '.' => {
                    i += 1;
                    RegexAtom::Any
                }
                '\\' => {
                    i += 1;
                    if i >= chars.len() {
                        RegexAtom::Literal('\\')
                    } else {
                        let esc = chars[i];
                        i += 1;
                        match esc {
                            'd' => RegexAtom::Digit(true),
                            'D' => RegexAtom::Digit(false),
                            'w' => RegexAtom::Word(true),
                            'W' => RegexAtom::Word(false),
                            's' => RegexAtom::Space(true),
                            'S' => RegexAtom::Space(false),
                            't' => RegexAtom::Literal('\t'),
                            'n' => RegexAtom::Literal('\n'),
                            'r' => RegexAtom::Literal('\r'),
                            other => {
                                let ch = if case_sensitive {
                                    other
                                } else {
                                    other.to_lowercase().next().unwrap_or(other)
                                };
                                RegexAtom::Literal(ch)
                            }
                        }
                    }
                }
                '[' => {
                    i += 1;
                    let mut negated = false;
                    if i < chars.len() && (chars[i] == '^' || chars[i] == '!') {
                        negated = true;
                        i += 1;
                    }
                    let mut ranges = Vec::new();
                    while i < chars.len() && chars[i] != ']' {
                        let mut ch = chars[i];
                        if ch == '\\' && i + 1 < chars.len() {
                            i += 1;
                            ch = chars[i];
                            match ch {
                                'd' => {
                                    ranges.push(('0', '9'));
                                    i += 1;
                                    continue;
                                }
                                'w' => {
                                    ranges.push(('a', 'z'));
                                    ranges.push(('A', 'Z'));
                                    ranges.push(('0', '9'));
                                    ranges.push(('_', '_'));
                                    i += 1;
                                    continue;
                                }
                                's' => {
                                    ranges.push((' ', ' '));
                                    ranges.push(('\t', '\r'));
                                    i += 1;
                                    continue;
                                }
                                _ => {}
                            }
                        }
                        if !case_sensitive {
                            ch = ch.to_lowercase().next().unwrap_or(ch);
                        }
                        if i + 2 < chars.len() && chars[i + 1] == '-' && chars[i + 2] != ']' {
                            let mut end_ch = chars[i + 2];
                            if !case_sensitive {
                                end_ch = end_ch.to_lowercase().next().unwrap_or(end_ch);
                            }
                            ranges.push((ch.min(end_ch), ch.max(end_ch)));
                            i += 3;
                        } else {
                            ranges.push((ch, ch));
                            i += 1;
                        }
                    }
                    if i < chars.len() && chars[i] == ']' {
                        i += 1;
                    }
                    RegexAtom::CharClass { ranges, negated }
                }
                '(' => {
                    i += 1;
                    // Skip non-capturing `(?:`
                    if i + 1 < chars.len() && chars[i] == '?' && chars[i + 1] == ':' {
                        i += 2;
                    }
                    let group_start = i;
                    let mut depth = 1i32;
                    while i < chars.len() && depth > 0 {
                        if chars[i] == '\\' {
                            i += 2;
                            continue;
                        }
                        if chars[i] == '(' {
                            depth += 1;
                        } else if chars[i] == ')' {
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                        }
                        i += 1;
                    }
                    let group_slice = &chars[group_start..i.min(chars.len())];
                    if i < chars.len() && chars[i] == ')' {
                        i += 1;
                    }
                    let sub_branches = Self::parse_alternation(group_slice, case_sensitive)?;
                    RegexAtom::Group(sub_branches.into_iter().map(|b| b.pieces).collect())
                }
                '*' | '+' | '?' => {
                    // Leading quantifier without atom -> treat as literal
                    i += 1;
                    RegexAtom::Literal(c)
                }
                other => {
                    i += 1;
                    let ch = if case_sensitive {
                        other
                    } else {
                        other.to_lowercase().next().unwrap_or(other)
                    };
                    RegexAtom::Literal(ch)
                }
            };

            // Parse optional quantifier (`*`, `+`, `?`, `{m,n}`)
            let mut min = 1usize;
            let mut max = Some(1usize);
            if i < chars.len() {
                match chars[i] {
                    '*' => {
                        min = 0;
                        max = None;
                        i += 1;
                    }
                    '+' => {
                        min = 1;
                        max = None;
                        i += 1;
                    }
                    '?' => {
                        min = 0;
                        max = Some(1);
                        i += 1;
                    }
                    '{' => {
                        let q_start = i + 1;
                        let mut q_end = q_start;
                        while q_end < chars.len() && chars[q_end] != '}' {
                            q_end += 1;
                        }
                        if q_end < chars.len() {
                            let spec: String = chars[q_start..q_end].iter().collect();
                            if let Some((a, b)) = spec.split_once(',') {
                                if let Ok(m) = a.trim().parse::<usize>() {
                                    min = m;
                                    max = if b.trim().is_empty() {
                                        None
                                    } else {
                                        b.trim().parse::<usize>().ok()
                                    };
                                    i = q_end + 1;
                                }
                            } else if let Ok(exact) = spec.trim().parse::<usize>() {
                                min = exact;
                                max = Some(exact);
                                i = q_end + 1;
                            }
                        }
                    }
                    _ => {}
                }
                // Ignore lazy modifier `?` after quantifier (e.g. `.*?`)
                if i < chars.len() && chars[i] == '?' && (max.is_none() || min == 0) {
                    i += 1;
                }
            }

            pieces.push(RegexPiece { atom, min, max });
        }

        Ok(pieces)
    }

    /// Find the first match `(start_char_idx, end_char_idx)` in `text`.
    pub fn find_span(&self, text: &str) -> Option<(usize, usize)> {
        let chars: Vec<char> = if self.case_sensitive {
            text.chars().collect()
        } else {
            text.chars()
                .map(|c| c.to_lowercase().next().unwrap_or(c))
                .collect()
        };

        for branch in &self.branches {
            if branch.anchor_start {
                if let Some(end) = Self::match_pieces(&branch.pieces, &chars, 0, branch.anchor_end)
                {
                    return Some((0, end));
                }
            } else {
                for start in 0..=chars.len() {
                    if let Some(end) =
                        Self::match_pieces(&branch.pieces, &chars, start, branch.anchor_end)
                    {
                        return Some((start, end));
                    }
                }
            }
        }
        None
    }

    pub fn is_match(&self, text: &str) -> bool {
        self.find_span(text).is_some()
    }

    pub fn matched_indices(&self, text: &str) -> Option<Vec<usize>> {
        let (start, end) = self.find_span(text)?;
        Some((start..end).collect())
    }

    fn match_pieces(
        pieces: &[RegexPiece],
        chars: &[char],
        pos: usize,
        anchor_end: bool,
    ) -> Option<usize> {
        if pieces.is_empty() {
            if !anchor_end || pos == chars.len() {
                return Some(pos);
            }
            return None;
        }

        let piece = &pieces[0];
        let rest = &pieces[1..];

        // Collect reachable positions after matching `piece.atom` between `piece.min` and `piece.max` times
        let mut reachable = vec![(pos, 0usize)];
        let mut valid_positions = Vec::new();
        let max_reps = piece.max.unwrap_or(chars.len().saturating_sub(pos) + 1);

        let mut idx = 0usize;
        while idx < reachable.len() {
            let (cur_pos, reps) = reachable[idx];
            idx += 1;

            if reps >= piece.min {
                valid_positions.push(cur_pos);
            }
            if reps < max_reps && cur_pos < chars.len() {
                for next_pos in Self::match_atom(&piece.atom, chars, cur_pos) {
                    if next_pos > cur_pos || reps < piece.min {
                        reachable.push((next_pos, reps + 1));
                    }
                }
            }
        }

        // Greedy: try longest matches first
        for &candidate_pos in valid_positions.iter().rev() {
            if let Some(final_pos) = Self::match_pieces(rest, chars, candidate_pos, anchor_end) {
                return Some(final_pos);
            }
        }
        None
    }

    fn match_atom(atom: &RegexAtom, chars: &[char], pos: usize) -> Vec<usize> {
        if pos >= chars.len() {
            if let RegexAtom::Group(branches) = atom {
                let mut out = Vec::new();
                for b in branches {
                    if let Some(p) = Self::match_pieces(b, chars, pos, false) {
                        out.push(p);
                    }
                }
                return out;
            }
            return Vec::new();
        }
        let c = chars[pos];
        match atom {
            RegexAtom::Any => vec![pos + 1],
            RegexAtom::Literal(expected) => {
                if c == *expected {
                    vec![pos + 1]
                } else {
                    Vec::new()
                }
            }
            RegexAtom::Digit(expected) => {
                if c.is_ascii_digit() == *expected {
                    vec![pos + 1]
                } else {
                    Vec::new()
                }
            }
            RegexAtom::Word(expected) => {
                let is_w = c.is_alphanumeric() || c == '_';
                if is_w == *expected {
                    vec![pos + 1]
                } else {
                    Vec::new()
                }
            }
            RegexAtom::Space(expected) => {
                if c.is_whitespace() == *expected {
                    vec![pos + 1]
                } else {
                    Vec::new()
                }
            }
            RegexAtom::CharClass { ranges, negated } => {
                let in_range = ranges.iter().any(|&(lo, hi)| c >= lo && c <= hi);
                if in_range != *negated {
                    vec![pos + 1]
                } else {
                    Vec::new()
                }
            }
            RegexAtom::Group(branches) => {
                let mut out = Vec::new();
                for b in branches {
                    if let Some(p) = Self::match_pieces(b, chars, pos, false) {
                        out.push(p);
                    }
                }
                out
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// WINDOWS SEARCH WILDCARD & MULTI-PATTERN ENGINE (`*.rs;*.toml`, `doc_??.*`)
// ═══════════════════════════════════════════════════════════════════════════

/// Matches Windows Search wildcard expressions (`*`, `?`, `;`-separated multi-patterns,
/// leading `.ext` shorthand like `.docx` -> `*.docx*`, and `!exclude` patterns).
pub struct WildcardMatcher;

impl WildcardMatcher {
    /// Returns true if the query looks like a Windows wildcard or multi-pattern expression.
    pub fn is_wildcard_query(query: &str) -> bool {
        let q = query.trim();
        q.contains('*')
            || q.contains('?')
            || (q.contains(';') && q.len() > 2)
            || (q.starts_with('.') && q.len() > 1 && !q[1..].contains('.'))
    }

    /// Match `name` against a Windows Search pattern expression (e.g. `*.rs;*.toml !*test*`).
    /// Returns matched character indices if matched.
    pub fn match_indices(name: &str, pattern_expr: &str, case_sensitive: bool) -> Option<Vec<usize>> {
        let expr = pattern_expr.trim();
        if expr.is_empty() {
            return Some(Vec::new());
        }

        // Split by `;` or whitespace-prefixed `!` exclusions
        let mut positive_patterns: Vec<String> = Vec::new();
        let mut negative_patterns: Vec<String> = Vec::new();

        for part in expr.split(';') {
            for token in part.split_whitespace() {
                if let Some(neg) = token.strip_prefix('!').or_else(|| token.strip_prefix('-')) {
                    if !neg.is_empty() {
                        negative_patterns.push(Self::normalize_wildcard(neg));
                    }
                } else if !token.is_empty() {
                    positive_patterns.push(Self::normalize_wildcard(token));
                }
            }
        }

        // Check negative patterns first
        for neg in &negative_patterns {
            if Self::glob_match_indices(name, neg, case_sensitive).is_some() {
                return None;
            }
        }

        if positive_patterns.is_empty() {
            return Some((0..name.chars().count()).collect());
        }

        for pos in &positive_patterns {
            if let Some(indices) = Self::glob_match_indices(name, pos, case_sensitive) {
                return Some(indices);
            }
        }
        None
    }

    fn normalize_wildcard(pat: &str) -> String {
        let p = pat.trim();
        // Files-community FolderSearch.cs rule: ".docx" -> "*.docx*"
        if p.starts_with('.') && !p.contains('*') && !p.contains('?') {
            format!("*{}*", p)
        } else if !p.contains('*') && !p.contains('?') {
            format!("*{}*", p)
        } else {
            p.to_string()
        }
    }

    /// Glob matching (`*` and `?`) that also records which character indices were matched by
    /// non-`*` pattern characters so the UI can highlight them!
    pub fn glob_match_indices(
        text: &str,
        pattern: &str,
        case_sensitive: bool,
    ) -> Option<Vec<usize>> {
        let t_chars: Vec<char> = if case_sensitive {
            text.chars().collect()
        } else {
            text.chars()
                .map(|c| c.to_lowercase().next().unwrap_or(c))
                .collect()
        };
        let p_chars: Vec<char> = if case_sensitive {
            pattern.chars().collect()
        } else {
            pattern
                .chars()
                .map(|c| c.to_lowercase().next().unwrap_or(c))
                .collect()
        };

        let mut ti = 0usize;
        let mut pi = 0usize;
        let mut star_pi: Option<usize> = None;
        let mut star_ti = 0usize;
        let mut matched = Vec::new();
        let mut matched_at_star = 0usize;

        while ti < t_chars.len() {
            if pi < p_chars.len() && (p_chars[pi] == '?' || p_chars[pi] == t_chars[ti]) {
                matched.push(ti);
                ti += 1;
                pi += 1;
            } else if pi < p_chars.len() && p_chars[pi] == '*' {
                star_pi = Some(pi);
                star_ti = ti;
                matched_at_star = matched.len();
                pi += 1;
            } else if let Some(sp) = star_pi {
                pi = sp + 1;
                star_ti += 1;
                ti = star_ti;
                matched.truncate(matched_at_star);
            } else {
                return None;
            }
        }

        while pi < p_chars.len() && p_chars[pi] == '*' {
            pi += 1;
        }

        if pi == p_chars.len() {
            Some(matched)
        } else {
            None
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// FILEPILOT-STYLE PREDICTIVE PATH & DIRECTORY CACHE ENGINE
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
pub struct CachedDirEntry {
    pub name: String,
    pub name_lower: String,
    pub path: FilesPath,
    pub is_dir: bool,
    pub is_hidden: bool,
    pub size: u64,
    pub modified_secs: Option<u64>,
}

struct CachedDirListing {
    fetched_at: Instant,
    entries: Arc<Vec<CachedDirEntry>>,
}

static DIR_COMPLETION_CACHE: OnceLock<Mutex<HashMap<PathBuf, CachedDirListing>>> = OnceLock::new();

fn dir_cache() -> &'static Mutex<HashMap<PathBuf, CachedDirListing>> {
    DIR_COMPLETION_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Read directory entries with a 5-second in-memory cache so typing in the Omnibar
/// never stalls on disk I/O across consecutive keystrokes.
pub fn get_cached_dir_entries(dir: &Path) -> Arc<Vec<CachedDirEntry>> {
    let key = dir.to_path_buf();
    let now = Instant::now();
    if let Ok(guard) = dir_cache().lock() {
        if let Some(cached) = guard.get(&key) {
            if now.duration_since(cached.fetched_at) < Duration::from_secs(5) {
                return cached.entries.clone();
            }
        }
    }

    let mut list = Vec::new();
    if let Ok(rd) = std::fs::read_dir(dir) {
        for entry in rd.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let name_lower = name.to_lowercase();
            let ft = entry.file_type().ok();
            let is_dir = ft.map(|t| t.is_dir()).unwrap_or(false);
            let is_hidden = name.starts_with('.');
            let (size, modified_secs) = if let Ok(md) = entry.metadata() {
                let sz = if is_dir { 0 } else { md.len() };
                let m = md
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .map(|d| d.as_secs());
                (sz, m)
            } else {
                (0, None)
            };
            list.push(CachedDirEntry {
                name,
                name_lower,
                path: FilesPath::from(entry.path()),
                is_dir,
                is_hidden,
                size,
                modified_secs,
            });
        }
    }

    // Sort directories first, then natural case-insensitive alphabetical order
    list.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.name_lower.cmp(&b.name_lower))
    });

    let arc = Arc::new(list);
    if let Ok(mut guard) = dir_cache().lock() {
        if guard.len() >= 64 {
            guard.clear();
        }
        guard.insert(
            key,
            CachedDirListing {
                fetched_at: now,
                entries: arc.clone(),
            },
        );
    }
    arc
}

/// Expand environment variables (`%USERPROFILE%`, `%APPDATA%`, `%TEMP%`, `~`) and
/// special shell location names (`Home`, `Desktop`, `Downloads`, `Documents`, etc.).
pub fn expand_path_input(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    // Expand `~` prefix to home directory
    if trimmed == "~" || trimmed.starts_with("~/") || trimmed.starts_with("~\\") {
        if let Some(home) = dirs::home_dir() {
            let suffix = &trimmed[1..];
            return format!("{}{}", home.to_string_lossy(), suffix);
        }
    }

    // Expand `%VAR%` environment variables
    if trimmed.contains('%') {
        let mut out = String::with_capacity(trimmed.len() + 32);
        let mut rest = trimmed;
        while let Some(start) = rest.find('%') {
            out.push_str(&rest[..start]);
            let after_start = &rest[start + 1..];
            if let Some(end) = after_start.find('%') {
                let var_name = &after_start[..end];
                if !var_name.is_empty() {
                    if let Ok(val) = std::env::var(var_name) {
                        out.push_str(&val);
                    } else {
                        out.push('%');
                        out.push_str(var_name);
                        out.push('%');
                    }
                }
                rest = &after_start[end + 1..];
            } else {
                out.push('%');
                rest = after_start;
                break;
            }
        }
        out.push_str(rest);
        return out;
    }

    // Special named shell folders
    match trimmed.to_lowercase().as_str() {
        "desktop" => {
            if let Some(p) = dirs::desktop_dir() {
                return p.to_string_lossy().to_string();
            }
        }
        "downloads" | "download" => {
            if let Some(p) = dirs::download_dir() {
                return p.to_string_lossy().to_string();
            }
        }
        "documents" | "docs" => {
            if let Some(p) = dirs::document_dir() {
                return p.to_string_lossy().to_string();
            }
        }
        "pictures" | "photos" => {
            if let Some(p) = dirs::picture_dir() {
                return p.to_string_lossy().to_string();
            }
        }
        "music" => {
            if let Some(p) = dirs::audio_dir() {
                return p.to_string_lossy().to_string();
            }
        }
        "videos" => {
            if let Some(p) = dirs::video_dir() {
                return p.to_string_lossy().to_string();
            }
        }
        _ => {}
    }

    trimmed.to_string()
}

/// A scored predictive path completion candidate.
#[derive(Debug, Clone)]
pub struct PathCompletionCandidate {
    pub path: FilesPath,
    pub display_label: String,
    pub subtitle: String,
    pub is_dir: bool,
    pub is_lookahead: bool,
    pub match_indices: Vec<usize>,
    pub score: u32,
}

/// Compute FilePilot-style fast path completions + inline predictive ghost suffix.
/// Returns `(ghost_suffix, candidates)`.
pub fn compute_path_completions(
    raw_input: &str,
    current_dir: &FilesPath,
    recent_paths: &[String],
    include_hidden: bool,
    max_results: usize,
) -> (Option<String>, Vec<PathCompletionCandidate>) {
    let trimmed = raw_input.trim();

    // Empty input or special page names -> suggest recent paths + standard user folders
    if trimmed.is_empty()
        || trimmed.eq_ignore_ascii_case("Home")
        || trimmed.eq_ignore_ascii_case("Settings")
    {
        let mut out = Vec::new();
        for recent in recent_paths.iter().take(6) {
            let fp = FilesPath::new(recent);
            out.push(PathCompletionCandidate {
                path: fp,
                display_label: recent.clone(),
                subtitle: "Recent location".to_string(),
                is_dir: true,
                is_lookahead: false,
                match_indices: Vec::new(),
                score: 900,
            });
        }
        for (label, opt_dir) in [
            ("Home", dirs::home_dir()),
            ("Desktop", dirs::desktop_dir()),
            ("Downloads", dirs::download_dir()),
            ("Documents", dirs::document_dir()),
            ("Pictures", dirs::picture_dir()),
        ] {
            if let Some(p) = opt_dir {
                let s = p.to_string_lossy().to_string();
                if !out.iter().any(|c| c.display_label.eq_ignore_ascii_case(&s)) {
                    out.push(PathCompletionCandidate {
                        path: FilesPath::from(p),
                        display_label: s,
                        subtitle: format!("{} folder", label),
                        is_dir: true,
                        is_lookahead: false,
                        match_indices: Vec::new(),
                        score: 800,
                    });
                }
            }
        }
        out.truncate(max_results);
        return (None, out);
    }

    let expanded = expand_path_input(trimmed);
    let sep = if expanded.contains('/') && !expanded.contains('\\') {
        '/'
    } else {
        std::path::MAIN_SEPARATOR
    };

    // Support relative path resolution from `current_dir` when user types `sub/foo` without drive/root
    let resolved_str = if !expanded.contains(':')
        && !expanded.starts_with('\\')
        && !expanded.starts_with('/')
        && current_dir.to_string_lossy() != "Home"
        && current_dir.to_string_lossy() != "Settings"
    {
        let base = current_dir.to_string_lossy();
        let base_trimmed = base.trim_end_matches(['\\', '/']);
        format!("{}{}{}", base_trimmed, sep, expanded)
    } else {
        expanded.clone()
    };

    let ends_with_sep = resolved_str.ends_with('\\') || resolved_str.ends_with('/');
    let path_obj = Path::new(&resolved_str);

    let (parent_dir, prefix) = if ends_with_sep {
        (Some(path_obj), "")
    } else {
        let p = path_obj.parent();
        let pref = path_obj.file_name().and_then(|f| f.to_str()).unwrap_or("");
        (p, pref)
    };

    let mut candidates: Vec<PathCompletionCandidate> = Vec::new();
    let prefix_lower = prefix.to_lowercase();
    let mut best_ghost: Option<String> = None;

    if let Some(parent) = parent_dir {
        let entries = get_cached_dir_entries(parent);
        let parent_display = parent.to_string_lossy();
        let parent_clean = parent_display.trim_end_matches(['\\', '/']);

        for entry in entries.iter() {
            if entry.is_hidden && !include_hidden && !prefix.starts_with('.') {
                continue;
            }

            if prefix.is_empty() {
                let full_label = format!("{}{}{}", parent_clean, sep, entry.name);
                candidates.push(PathCompletionCandidate {
                    path: entry.path.clone(),
                    display_label: full_label,
                    subtitle: if entry.is_dir {
                        "Folder".to_string()
                    } else {
                        format_size_compact(entry.size)
                    },
                    is_dir: entry.is_dir,
                    is_lookahead: false,
                    match_indices: Vec::new(),
                    score: if entry.is_dir { 1000 } else { 500 },
                });
            } else if let Some((score, local_indices)) =
                score_fuzzy_or_prefix(&entry.name, &entry.name_lower, prefix, &prefix_lower)
            {
                let full_label = format!("{}{}{}", parent_clean, sep, entry.name);
                let offset = full_label.chars().count().saturating_sub(entry.name.chars().count());
                let shifted_indices = local_indices.into_iter().map(|i| i + offset).collect();

                // Compute FilePilot inline ghost completion from the top exact prefix match
                if best_ghost.is_none() && entry.name_lower.starts_with(&prefix_lower) {
                    let prefix_chars = prefix.chars().count();
                    let suffix: String = entry.name.chars().skip(prefix_chars).collect();
                    if !suffix.is_empty() || entry.is_dir {
                        let trailing = if entry.is_dir {
                            format!("{}{}", suffix, sep)
                        } else {
                            suffix
                        };
                        if !trailing.is_empty() {
                            best_ghost = Some(trailing);
                        }
                    }
                }

                candidates.push(PathCompletionCandidate {
                    path: entry.path.clone(),
                    display_label: full_label,
                    subtitle: if entry.is_dir {
                        "Folder".to_string()
                    } else {
                        format_size_compact(entry.size)
                    },
                    is_dir: entry.is_dir,
                    is_lookahead: false,
                    match_indices: shifted_indices,
                    score: score + if entry.is_dir { 250 } else { 0 },
                });
            }
        }

        // Sort by score descending, then natural name
        candidates.sort_by(|a, b| {
            b.score
                .cmp(&a.score)
                .then_with(|| a.display_label.cmp(&b.display_label))
        });

        // Files-community + FilePilot 1-level-deeper lookahead:
        // If there are between 1 and 5 matching folders, peek inside the top matching folder
        // so the user can jump directly to a child subfolder in one click/Enter!
        let dir_matches: Vec<FilesPath> = candidates
            .iter()
            .filter(|c| c.is_dir)
            .take(2)
            .map(|c| c.path.clone())
            .collect();

        if !prefix.is_empty() && !dir_matches.is_empty() && candidates.len() < max_results {
            if let Some(first_dir) = dir_matches.first() {
                let sub_entries = get_cached_dir_entries(first_dir.as_path());
                let first_display = first_dir.to_string_lossy();
                let first_clean = first_display.trim_end_matches(['\\', '/']);
                for sub in sub_entries.iter().filter(|e| e.is_dir && (!e.is_hidden || include_hidden)).take(4) {
                    if candidates.len() >= max_results {
                        break;
                    }
                    let full_sub = format!("{}{}{}", first_clean, sep, sub.name);
                    candidates.push(PathCompletionCandidate {
                        path: sub.path.clone(),
                        display_label: full_sub,
                        subtitle: "Subfolder lookahead".to_string(),
                        is_dir: true,
                        is_lookahead: true,
                        match_indices: Vec::new(),
                        score: 400,
                    });
                }
            }
        }
    }

    candidates.truncate(max_results);
    (best_ghost, candidates)
}

/// Score a candidate string against a query using FilePilot-inspired tiered scoring:
/// Tier 1 (2000+): Exact prefix match
/// Tier 2 (1500+): Word-boundary / camelCase acronym match
/// Tier 3 (1200+): Contiguous substring match
/// Tier 4 (800+): Fuzzy subsequence match
pub fn score_fuzzy_or_prefix(
    name: &str,
    name_lower: &str,
    query: &str,
    query_lower: &str,
) -> Option<(u32, Vec<usize>)> {
    if query.is_empty() {
        return Some((1000, Vec::new()));
    }

    let name_chars: Vec<char> = name.chars().collect();
    let name_lower_chars: Vec<char> = name_lower.chars().collect();
    let query_lower_chars: Vec<char> = query_lower.chars().collect();

    if query_lower_chars.len() > name_lower_chars.len() {
        return None;
    }

    // 1. Exact prefix match
    if name_lower.starts_with(query_lower) {
        let len_bonus = 100u32.saturating_sub(name_chars.len() as u32);
        let exact_case_bonus = if name.starts_with(query) { 50 } else { 0 };
        let indices = (0..query_lower_chars.len()).collect();
        return Some((2000 + len_bonus + exact_case_bonus, indices));
    }

    // 2. Contiguous substring match (prefer word boundaries like `_`, `-`, `.`, ` `)
    if let Some(byte_pos) = name_lower.find(query_lower) {
        let char_start = name_lower[..byte_pos].chars().count();
        let char_end = char_start + query_lower_chars.len();
        let is_boundary = char_start == 0
            || matches!(
                name_chars.get(char_start.saturating_sub(1)),
                Some('_' | '-' | '.' | ' ' | '/' | '\\')
            );
        let base = if is_boundary { 1650 } else { 1300 };
        let pos_bonus = 60u32.saturating_sub(char_start as u32 * 2);
        return Some((base + pos_bonus, (char_start..char_end).collect()));
    }

    // 3. Fuzzy subsequence match with word-boundary / camelCase bonus
    let mut indices = Vec::with_capacity(query_lower_chars.len());
    let mut ni = 0usize;
    let mut consecutive_bonus = 0u32;
    let mut boundary_bonus = 0u32;

    for &qc in &query_lower_chars {
        let mut found = None;
        // First try to find a match on a word boundary or camelCase transition
        for idx in ni..name_lower_chars.len() {
            if name_lower_chars[idx] == qc {
                let is_boundary = idx == 0
                    || matches!(
                        name_chars.get(idx.saturating_sub(1)),
                        Some('_' | '-' | '.' | ' ' | '/' | '\\')
                    )
                    || (name_chars.get(idx).map(|c| c.is_uppercase()).unwrap_or(false)
                        && name_chars
                            .get(idx.saturating_sub(1))
                            .map(|c| c.is_lowercase())
                            .unwrap_or(false));
                if is_boundary {
                    found = Some((idx, true));
                    break;
                } else if found.is_none() {
                    found = Some((idx, false));
                }
            }
        }
        if let Some((idx, is_bound)) = found {
            if let Some(&prev) = indices.last() {
                if idx == prev + 1 {
                    consecutive_bonus += 25;
                }
            }
            if is_bound {
                boundary_bonus += 35;
            }
            indices.push(idx);
            ni = idx + 1;
        } else {
            return None;
        }
    }

    let spread_penalty = indices
        .last()
        .copied()
        .unwrap_or(0)
        .saturating_sub(indices.first().copied().unwrap_or(0)) as u32;
    let score = (850 + consecutive_bonus + boundary_bonus).saturating_sub(spread_penalty.min(200));
    Some((score, indices))
}

fn format_size_compact(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// FULL-TEXT CONTENT & METADATA SCANNER
// ═══════════════════════════════════════════════════════════════════════════

/// Fast check if a file extension or header is suitable for full-text content scanning.
fn is_likely_text_file(path: &Path) -> bool {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    // Skip known binary/media/archive formats immediately without opening
    !matches!(
        ext.as_str(),
        "png"
            | "jpg"
            | "jpeg"
            | "gif"
            | "webp"
            | "bmp"
            | "ico"
            | "tiff"
            | "tif"
            | "mp3"
            | "mp4"
            | "mkv"
            | "avi"
            | "mov"
            | "wav"
            | "flac"
            | "ogg"
            | "zip"
            | "7z"
            | "rar"
            | "tar"
            | "gz"
            | "bz2"
            | "xz"
            | "exe"
            | "dll"
            | "so"
            | "dylib"
            | "pdb"
            | "obj"
            | "o"
            | "a"
            | "lib"
            | "ttf"
            | "otf"
            | "woff"
            | "woff2"
            | "sqlite"
            | "db"
    )
}

/// Search inside a file's contents using memory-mapping (`memmap2`) for files >= 32 KB
/// or direct stack buffer read for small files.
/// Returns `Some("L<line>: <snippet>")` if matched.
pub fn search_file_content(
    path: &Path,
    needle: &str,
    regex: Option<&SimpleRegex>,
    case_sensitive: bool,
) -> Option<String> {
    if needle.is_empty() && regex.is_none() {
        return None;
    }
    if !is_likely_text_file(path) {
        return None;
    }

    let file = std::fs::File::open(path).ok()?;
    let meta = file.metadata().ok()?;
    let len = meta.len();
    // Skip empty files or huge files > 16 MB to keep interactive search blazingly fast
    if len == 0 || len > 16 * 1024 * 1024 {
        return None;
    }

    let bytes: Vec<u8> = if len >= 32 * 1024 {
        if let Ok(mmap) = unsafe { memmap2::Mmap::map(&file) } {
            // Check first 512 bytes for NUL byte (binary file guard)
            let check_len = mmap.len().min(512);
            if mmap[..check_len].contains(&0) {
                return None;
            }
            mmap.to_vec()
        } else {
            return None;
        }
    } else {
        let mut buf = Vec::with_capacity(len as usize);
        use std::io::Read;
        let mut take = file.take(len);
        take.read_to_end(&mut buf).ok()?;
        let check_len = buf.len().min(512);
        if buf[..check_len].contains(&0) {
            return None;
        }
        buf
    };

    let content = std::str::from_utf8(&bytes).ok()?;
    let needle_lower = needle.to_lowercase();

    for (line_idx, line) in content.lines().enumerate() {
        let matched = if let Some(re) = regex {
            re.is_match(line)
        } else if case_sensitive {
            line.contains(needle)
        } else {
            line.to_lowercase().contains(&needle_lower)
        };

        if matched {
            let trimmed = line.trim();
            let snippet: String = trimmed.chars().take(72).collect();
            return Some(format!("L{}: {}", line_idx + 1, snippet));
        }
    }
    None
}

/// Build a searchable metadata string for an item (combining basic file attributes,
/// human-readable type/size/dates, tags, and `ExtendedMetadataExtractor` EXIF/ID3/PDF properties)
/// and match it against `needle` or `regex`.
pub fn search_file_metadata(
    path: &FilesPath,
    name: &str,
    is_dir: bool,
    size: u64,
    modified_str: Option<&str>,
    tags: &[String],
    needle: &str,
    regex: Option<&SimpleRegex>,
    case_sensitive: bool,
) -> Option<String> {
    if needle.is_empty() && regex.is_none() {
        return None;
    }

    let ext = Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let kind_label = classify_file_kind(&ext, is_dir);
    let size_label = if is_dir {
        "folder directory".to_string()
    } else {
        format!("{} ({} bytes)", format_size_compact(size), size)
    };

    let mut meta_parts = Vec::with_capacity(6);
    meta_parts.push(kind_label.to_string());
    if !ext.is_empty() {
        meta_parts.push(format!(".{} file", ext));
    }
    meta_parts.push(size_label);
    if let Some(m) = modified_str {
        meta_parts.push(format!("Modified {}", m));
    }
    if !tags.is_empty() {
        meta_parts.push(format!("Tags: {}", tags.join(", ")));
    }

    // Extract extended metadata (image dimensions, EXIF camera/author, MP3 ID3 artist/album/title, PDF pages)
    if !is_dir {
        if let Ok(ext_props) = ExtendedMetadataExtractor::extract(path) {
            let summary = ext_props.summary_string();
            if !summary.is_empty() {
                meta_parts.insert(0, summary);
            }
        }
    }

    let full_meta = meta_parts.join(" • ");
    let needle_lower = needle.to_lowercase();

    let matched = if let Some(re) = regex {
        re.is_match(&full_meta)
    } else if case_sensitive {
        full_meta.contains(needle)
    } else {
        full_meta.to_lowercase().contains(&needle_lower)
    };

    if matched {
        let display: String = full_meta.chars().take(84).collect();
        Some(format!("Meta: {}", display))
    } else {
        None
    }
}

/// Classify a file extension into canonical AQS `kind:` categories.
pub fn classify_file_kind(ext_lower: &str, is_dir: bool) -> &'static str {
    if is_dir {
        return "folder";
    }
    match ext_lower {
        "jpg" | "jpeg" | "png" | "gif" | "bmp" | "webp" | "ico" | "svg" | "tiff" | "tif"
        | "heic" | "heif" | "raw" | "cr2" | "nef" => "image",
        "mp4" | "mkv" | "avi" | "mov" | "wmv" | "flv" | "webm" | "m4v" | "mpeg" | "mpg" => "video",
        "mp3" | "wav" | "flac" | "aac" | "ogg" | "wma" | "m4a" | "opus" => "audio",
        "pdf" => "pdf",
        "doc" | "docx" | "txt" | "rtf" | "odt" | "md" | "markdown" | "tex" | "log" => "document",
        "xls" | "xlsx" | "csv" | "tsv" | "ods" => "spreadsheet",
        "ppt" | "pptx" | "odp" | "key" => "presentation",
        "zip" | "7z" | "rar" | "tar" | "gz" | "bz2" | "xz" | "zst" | "iso" => "archive",
        "rs" | "py" | "js" | "ts" | "tsx" | "jsx" | "c" | "cpp" | "h" | "hpp" | "cs" | "go"
        | "java" | "kt" | "swift" | "rb" | "php" | "html" | "css" | "scss" | "json" | "toml"
        | "yaml" | "yml" | "xml" | "sql" | "sh" | "ps1" | "bat" | "cmd" => "code",
        "exe" | "msi" | "appx" | "msix" | "com" | "scr" => "executable",
        _ => "file",
    }
}

pub fn matches_kind_filter(ext_lower: &str, is_dir: bool, kind_filter: &str) -> bool {
    let canonical = classify_file_kind(ext_lower, is_dir);
    match kind_filter {
        "folder" | "folders" | "dir" | "directory" => is_dir,
        "image" | "images" | "photo" | "photos" | "picture" | "pictures" | "pic" => {
            canonical == "image"
        }
        "video" | "videos" | "movie" | "movies" => canonical == "video",
        "audio" | "music" | "sound" | "song" | "songs" => canonical == "audio",
        "pdf" | "pdfs" => canonical == "pdf",
        "doc" | "docs" | "document" | "documents" | "text" => {
            matches!(
                canonical,
                "document" | "pdf" | "spreadsheet" | "presentation"
            )
        }
        "spreadsheet" | "spreadsheets" | "sheet" | "excel" => canonical == "spreadsheet",
        "presentation" | "presentations" | "slide" | "slides" | "deck" => {
            canonical == "presentation"
        }
        "archive" | "archives" | "zip" | "compressed" => canonical == "archive",
        "code" | "source" | "dev" | "script" => canonical == "code",
        "exe" | "executable" | "app" | "application" | "program" => canonical == "executable",
        other => canonical == other || ext_lower == other,
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// HIGH-PERFORMANCE SEARCH SERVICE
// ═══════════════════════════════════════════════════════════════════════════

/// High-performance fuzzy, wildcard, regex, AQS, content, and metadata search service for the UI.
pub struct SearchService {
    matcher: Mutex<Matcher>,
    config: Config,
}

impl SearchService {
    pub fn new() -> Self {
        let config = Config::DEFAULT;
        Self {
            matcher: Mutex::new(Matcher::new(config.clone())),
            config,
        }
    }

    /// Filter a list of items based on a query string.
    pub fn filter_items(&self, query: &str, items: &[String]) -> Vec<usize> {
        if query.is_empty() {
            return (0..items.len()).collect();
        }

        let mut matches = Vec::new();

        // To use nucleo, we need to provide buffers for Utf32 conversion
        let mut needle_buf = Vec::new();
        let needle = Utf32Str::new(query, &mut needle_buf);

        let mut matcher = self.matcher.lock().unwrap();

        for (idx, item) in items.iter().enumerate() {
            let mut haystack_buf = Vec::new();
            let haystack = Utf32Str::new(item, &mut haystack_buf);

            if let Some(score) = matcher.fuzzy_match(haystack, needle) {
                matches.push((idx, score));
            }
        }

        // Sort by score descending
        matches.sort_by(|a, b| b.1.cmp(&a.1));

        matches.into_iter().map(|(idx, _)| idx).collect()
    }

    /// Get the configuration used by this service.
    #[allow(dead_code)]
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Filter items by file extension (case-insensitive).
    /// Empty extension filter returns all items.
    pub fn filter_by_extension(items: &[(String, String)], ext: &str) -> Vec<(String, String)> {
        if ext.is_empty() {
            return items.to_vec();
        }
        let ext_lower = ext.to_lowercase();
        items
            .iter()
            .filter(|(name, _)| {
                std::path::Path::new(name)
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| e.to_lowercase() == ext_lower)
                    .unwrap_or(false)
            })
            .cloned()
            .collect()
    }

    /// Filter items by size range [min, max].
    /// max=0 means no upper bound.
    pub fn filter_by_size(items: &[(String, u64)], min: u64, max: u64) -> Vec<(String, u64)> {
        items
            .iter()
            .filter(|(_, size)| *size >= min && (max == 0 || *size <= max))
            .cloned()
            .collect()
    }

    /// Parse AQS (Advanced Query Syntax) filters and remaining query text.
    /// Supports:
    /// - `kind:type` (`image`, `video`, `audio`, `document`, `code`, `archive`, `pdf`, `folder`)
    /// - `ext:extension` (`ext:rs`, `ext:.txt`, `ext:jpg,png`)
    /// - `size:>1mb`, `size:<500kb`, `size:=2gb`, `size:1mb..10mb`, `size:empty`, `size:large`, `size:huge`
    /// - `date:today`, `date:yesterday`, `date:thisweek`, `date:thismonth`, `date:thisyear`, `date:>2024-01-01`
    /// - `modified:...`, `created:...`
    /// - `tag:work`, `NOT tag:old`
    /// - `content:phrase`, `meta:phrase`, `author:name`, `camera:model`, `album:name`
    /// - `re:pattern`
    /// - `is:dir`, `is:file`, `is:hidden`, `is:readonly`, `is:empty`
    pub fn parse_aqs(query: &str) -> AqsFilters {
        let mut filters = AqsFilters::default();
        let mut text_parts: Vec<String> = Vec::new();
        let mut negate_next = false;

        for token in query.split_whitespace() {
            if token.eq_ignore_ascii_case("NOT") {
                negate_next = true;
                continue;
            }
            if token.eq_ignore_ascii_case("AND") || token.eq_ignore_ascii_case("OR") {
                continue;
            }

            let lower = token.to_lowercase();
            if let Some(value) = lower.strip_prefix("kind:").or_else(|| lower.strip_prefix("type:"))
            {
                if !value.is_empty() {
                    filters.kind = Some(value.to_string());
                }
            } else if let Some(value) = lower.strip_prefix("ext:") {
                let cleaned = value.trim_start_matches('.').to_string();
                if !cleaned.is_empty() {
                    let parts: Vec<String> = cleaned
                        .split([',', ';'])
                        .map(|s| s.trim().trim_start_matches('.').to_string())
                        .filter(|s| !s.is_empty())
                        .collect();
                    if let Some(first) = parts.first() {
                        filters.extension = Some(first.clone());
                    }
                    filters.extensions.extend(parts);
                }
            } else if let Some(value) = lower.strip_prefix("size:") {
                Self::parse_size_filter(value, &mut filters);
            } else if let Some(value) = lower
                .strip_prefix("date:")
                .or_else(|| lower.strip_prefix("modified:"))
                .or_else(|| lower.strip_prefix("datemodified:"))
            {
                Self::parse_date_filter(value, &mut filters);
            } else if let Some(value) = lower
                .strip_prefix("created:")
                .or_else(|| lower.strip_prefix("datecreated:"))
            {
                let mut tmp = AqsFilters::default();
                Self::parse_date_filter(value, &mut tmp);
                filters.created_after = tmp.date_after;
                filters.created_before = tmp.date_before;
            } else if let Some(value) = token
                .strip_prefix("tag:")
                .or_else(|| token.strip_prefix("TAG:"))
            {
                let clean = value.trim_matches('"');
                for t in clean.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()) {
                    if negate_next {
                        filters.tags_exclude.push(t.to_lowercase());
                    } else {
                        filters.tags_include.push(t.to_lowercase());
                    }
                }
            } else if let Some(value) = token
                .strip_prefix("content:")
                .or_else(|| token.strip_prefix("contents:"))
            {
                let clean = value.trim_matches('"').to_string();
                if !clean.is_empty() {
                    filters.content_query = Some(clean);
                }
            } else if let Some(value) = token
                .strip_prefix("meta:")
                .or_else(|| token.strip_prefix("metadata:"))
                .or_else(|| token.strip_prefix("author:"))
                .or_else(|| token.strip_prefix("artist:"))
                .or_else(|| token.strip_prefix("camera:"))
                .or_else(|| token.strip_prefix("album:"))
                .or_else(|| token.strip_prefix("dimensions:"))
            {
                let clean = value.trim_matches('"').to_string();
                if !clean.is_empty() {
                    filters.metadata_query = Some(clean);
                }
            } else if let Some(value) = token
                .strip_prefix("re:")
                .or_else(|| token.strip_prefix("regex:"))
            {
                if !value.is_empty() {
                    filters.regex_pattern = Some(value.to_string());
                }
            } else if let Some(value) = lower.strip_prefix("is:") {
                match value {
                    "dir" | "folder" | "directory" => filters.is_dir = Some(!negate_next),
                    "file" => filters.is_dir = Some(negate_next),
                    "hidden" => filters.is_hidden = Some(!negate_next),
                    "readonly" | "read-only" => filters.is_readonly = Some(!negate_next),
                    "empty" => filters.is_empty = Some(!negate_next),
                    _ => {}
                }
            } else {
                text_parts.push(token.to_string());
            }
            negate_next = false;
        }

        filters.text_query = text_parts.join(" ");
        filters
    }

    fn parse_size_filter(value: &str, filters: &mut AqsFilters) {
        match value {
            "empty" => {
                filters.size_min = Some(0);
                filters.size_max = Some(0);
                return;
            }
            "tiny" => {
                filters.size_min = Some(1);
                filters.size_max = Some(16 * 1024);
                return;
            }
            "small" => {
                filters.size_min = Some(16 * 1024);
                filters.size_max = Some(1024 * 1024);
                return;
            }
            "medium" => {
                filters.size_min = Some(1024 * 1024);
                filters.size_max = Some(128 * 1024 * 1024);
                return;
            }
            "large" => {
                filters.size_min = Some(128 * 1024 * 1024);
                filters.size_max = Some(1024 * 1024 * 1024);
                return;
            }
            "huge" | "gigantic" => {
                filters.size_min = Some(1024 * 1024 * 1024);
                return;
            }
            _ => {}
        }

        if let Some((min_s, max_s)) = value.split_once("..") {
            if let Some(b) = Self::parse_size_value(min_s) {
                filters.size_min = Some(b);
            }
            if let Some(b) = Self::parse_size_value(max_s) {
                filters.size_max = Some(b);
            }
        } else if let Some(num) = value.strip_prefix(">=") {
            if let Some(b) = Self::parse_size_value(num) {
                filters.size_min = Some(b);
            }
        } else if let Some(num) = value.strip_prefix("<=") {
            if let Some(b) = Self::parse_size_value(num) {
                filters.size_max = Some(b);
            }
        } else if let Some(num) = value.strip_prefix('>') {
            if let Some(b) = Self::parse_size_value(num) {
                filters.size_min = Some(b);
            }
        } else if let Some(num) = value.strip_prefix('<') {
            if let Some(b) = Self::parse_size_value(num) {
                filters.size_max = Some(b);
            }
        } else if let Some(num) = value.strip_prefix('=') {
            if let Some(b) = Self::parse_size_value(num) {
                filters.size_min = Some(b);
                filters.size_max = Some(b);
            }
        }
    }

    fn parse_size_value(value: &str) -> Option<u64> {
        let v = value.trim().to_lowercase();
        if let Some(n) = v.strip_suffix("tb") {
            n.trim()
                .parse::<f64>()
                .ok()
                .map(|x| (x * 1_099_511_627_776.0) as u64)
        } else if let Some(n) = v.strip_suffix("gb") {
            n.trim()
                .parse::<f64>()
                .ok()
                .map(|x| (x * 1_073_741_824.0) as u64)
        } else if let Some(n) = v.strip_suffix("mb") {
            n.trim()
                .parse::<f64>()
                .ok()
                .map(|x| (x * 1_048_576.0) as u64)
        } else if let Some(n) = v.strip_suffix("kb") {
            n.trim().parse::<f64>().ok().map(|x| (x * 1024.0) as u64)
        } else if let Some(n) = v.strip_suffix('b') {
            n.trim().parse::<u64>().ok()
        } else {
            v.parse::<u64>().ok()
        }
    }

    fn parse_date_filter(value: &str, filters: &mut AqsFilters) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let day = 86400i64;
        match value {
            "yesterday" => {
                filters.date_after = Some(now - day * 2);
                filters.date_before = Some(now - day);
            }
            "today" => {
                let secs_since_midnight = now % day;
                filters.date_after = Some(now - secs_since_midnight);
            }
            "thisweek" | "week" => filters.date_after = Some(now - day * 7),
            "lastweek" => {
                filters.date_after = Some(now - day * 14);
                filters.date_before = Some(now - day * 7);
            }
            "thismonth" | "month" => filters.date_after = Some(now - day * 30),
            "thisyear" | "year" => filters.date_after = Some(now - day * 365),
            _ => {
                if let Some(s) = value.strip_prefix(">=").or_else(|| value.strip_prefix('>')) {
                    if let Some(t) = Self::parse_date_value(s) {
                        filters.date_after = Some(t);
                    }
                } else if let Some(s) = value.strip_prefix("<=").or_else(|| value.strip_prefix('<'))
                {
                    if let Some(t) = Self::parse_date_value(s) {
                        filters.date_before = Some(t);
                    }
                } else if let Some(ts) = Self::parse_date_value(value) {
                    filters.date_after = Some(ts);
                    filters.date_before = Some(ts + day);
                }
            }
        }
    }

    fn parse_date_value(value: &str) -> Option<i64> {
        let parts: Vec<&str> = value.split('-').collect();
        if parts.len() != 3 {
            return None;
        }
        let y = parts[0].parse::<i64>().ok()?;
        let m = parts[1].parse::<u64>().ok()?;
        let d = parts[2].parse::<u64>().ok()?;
        if m < 1 || m > 12 || d < 1 || d > 31 {
            return None;
        }
        let mut total = 0i64;
        for year in 1970..y {
            total += if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) {
                366
            } else {
                365
            };
        }
        let dim = if y % 4 == 0 && (y % 100 != 0 || y % 400 == 0) {
            [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
        } else {
            [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
        };
        for i in 0..(m as usize - 1) {
            total += dim[i] as i64;
        }
        total += (d - 1) as i64;
        Some(total * 86400)
    }

    /// Evaluate a single `FileItem` in memory against a full query + `SearchOptions`
    /// (including AQS filters, Regex, Windows Wildcards, Fuzzy/Substring, Content, and Metadata).
    /// Returns `Some((score, match_indices, match_snippet))` if matched.
    pub fn evaluate_file_item(
        item: &FileItem,
        raw_query: &str,
        options: &SearchOptions,
        tags_cache: &HashMap<String, Vec<String>>,
    ) -> Option<(u32, Vec<usize>, Option<String>)> {
        let trimmed = raw_query.trim();
        if trimmed.is_empty() {
            return Some((100, Vec::new(), None));
        }

        let aqs = Self::parse_aqs(trimmed);
        let is_dir = matches!(item.file_type, FileType::Directory);

        // 1. Check structured AQS constraints
        if let Some(expected_dir) = aqs.is_dir {
            if is_dir != expected_dir {
                return None;
            }
        }
        if let Some(expected_hidden) = aqs.is_hidden {
            let hidden = item.is_hidden || item.name.starts_with('.');
            if hidden != expected_hidden {
                return None;
            }
        }
        if let Some(expected_empty) = aqs.is_empty {
            let empty = !is_dir && item.size == 0;
            if empty != expected_empty {
                return None;
            }
        }

        let ext_lower = Path::new(&item.name)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        if let Some(ref kind) = aqs.kind {
            if !matches_kind_filter(&ext_lower, is_dir, kind) {
                return None;
            }
        }
        if !aqs.extensions.is_empty() {
            if !aqs.extensions.iter().any(|e| e == &ext_lower) {
                return None;
            }
        } else if let Some(ref ext) = aqs.extension {
            if &ext_lower != ext {
                return None;
            }
        }

        if let Some(min_sz) = aqs.size_min {
            if is_dir || item.size < min_sz {
                return None;
            }
        }
        if let Some(max_sz) = aqs.size_max {
            if is_dir || item.size > max_sz {
                return None;
            }
        }

        if aqs.date_after.is_some() || aqs.date_before.is_some() {
            let mod_ts = item
                .modified
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64)?;
            if let Some(after) = aqs.date_after {
                if mod_ts < after {
                    return None;
                }
            }
            if let Some(before) = aqs.date_before {
                if mod_ts > before {
                    return None;
                }
            }
        }

        if aqs.created_after.is_some() || aqs.created_before.is_some() {
            let crt_ts = item
                .created
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64)?;
            if let Some(after) = aqs.created_after {
                if crt_ts < after {
                    return None;
                }
            }
            if let Some(before) = aqs.created_before {
                if crt_ts > before {
                    return None;
                }
            }
        }

        let item_key = item.path.to_string_lossy().to_string();
        let item_tags: &[String] = tags_cache
            .get(&item_key)
            .map(|v| v.as_slice())
            .unwrap_or(&[]);

        if !aqs.tags_include.is_empty() {
            let has_all = aqs.tags_include.iter().all(|req| {
                item_tags
                    .iter()
                    .any(|t| t.to_lowercase().contains(req.as_str()))
            });
            if !has_all {
                return None;
            }
        }
        if !aqs.tags_exclude.is_empty() {
            let has_excluded = aqs.tags_exclude.iter().any(|exc| {
                item_tags
                    .iter()
                    .any(|t| t.to_lowercase().contains(exc.as_str()))
            });
            if has_excluded {
                return None;
            }
        }

        // 2. Compile regex if requested via `options.use_regex`, `re:...`, or `/.../`
        let text_q = aqs.text_query.trim();
        let is_slash_regex = text_q.starts_with('/') && text_q.len() > 2 && text_q[1..].contains('/');
        let compiled_regex = if let Some(ref re_pat) = aqs.regex_pattern {
            SimpleRegex::compile(re_pat, options.case_sensitive).ok()
        } else if options.use_regex || is_slash_regex {
            if !text_q.is_empty() {
                SimpleRegex::compile(text_q, options.case_sensitive).ok()
            } else {
                None
            }
        } else {
            None
        };

        // 3. Check explicit inline `content:` or `meta:` AQS queries
        let mut snippet: Option<String> = None;
        if let Some(ref cq) = aqs.content_query {
            if is_dir {
                return None;
            }
            let s = search_file_content(
                item.path.as_path(),
                cq,
                compiled_regex.as_ref(),
                options.case_sensitive,
            )?;
            snippet = Some(s);
        }
        if let Some(ref mq) = aqs.metadata_query {
            let s = search_file_metadata(
                &item.path,
                &item.name,
                is_dir,
                item.size,
                None,
                item_tags,
                mq,
                compiled_regex.as_ref(),
                options.case_sensitive,
            )?;
            snippet = Some(s);
        }

        // 4. If there is no remaining filename/text query, it's a pure AQS filter match!
        if text_q.is_empty() && aqs.regex_pattern.is_none() {
            return Some((1000, Vec::new(), snippet));
        }

        // 5. Try matching the filename first (Regex -> Wildcard -> Fuzzy/Substring)
        let name_match: Option<(u32, Vec<usize>)> = if let Some(ref re) = compiled_regex {
            re.matched_indices(&item.name).map(|idx| (1800, idx))
        } else if WildcardMatcher::is_wildcard_query(text_q) {
            WildcardMatcher::match_indices(&item.name, text_q, options.case_sensitive)
                .map(|idx| (1750, idx))
        } else if options.case_sensitive {
            if let Some(pos) = item.name.find(text_q) {
                let start = item.name[..pos].chars().count();
                let end = start + text_q.chars().count();
                Some((1800, (start..end).collect()))
            } else {
                None
            }
        } else {
            score_fuzzy_or_prefix(
                &item.name,
                &item.name.to_lowercase(),
                text_q,
                &text_q.to_lowercase(),
            )
        };

        if let Some((score, indices)) = name_match {
            // If metadata search is also enabled, optionally attach metadata summary
            if snippet.is_none() && options.search_metadata && !is_dir {
                if let Ok(ext_props) = ExtendedMetadataExtractor::extract(&item.path) {
                    let s = ext_props.summary_string();
                    if !s.is_empty() {
                        snippet = Some(format!("Meta: {}", s));
                    }
                }
            }
            return Some((score, indices, snippet));
        }

        // 6. If filename didn't match, check Full-Text Metadata Search (when enabled)
        if options.search_metadata {
            if let Some(meta_snip) = search_file_metadata(
                &item.path,
                &item.name,
                is_dir,
                item.size,
                None,
                item_tags,
                text_q,
                compiled_regex.as_ref(),
                options.case_sensitive,
            ) {
                return Some((950, Vec::new(), Some(meta_snip)));
            }
        }

        // 7. If filename and metadata didn't match, check Full-Text Content Search (when enabled)
        if options.search_content && !is_dir {
            if let Some(content_snip) = search_file_content(
                item.path.as_path(),
                text_q,
                compiled_regex.as_ref(),
                options.case_sensitive,
            ) {
                return Some((900, Vec::new(), Some(content_snip)));
            }
        }

        None
    }
}

impl Default for SearchService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_service_new() {
        let service = SearchService::new();
        assert!(service.matcher.lock().is_ok());
    }

    #[test]
    fn test_filter_items_empty_query_returns_all() {
        let service = SearchService::new();
        let items = vec![
            "document.txt".to_string(),
            "image.png".to_string(),
            "readme.md".to_string(),
        ];

        let result = service.filter_items("", &items);
        assert_eq!(result, vec![0, 1, 2]);
    }

    #[test]
    fn test_filter_items_exact_match() {
        let service = SearchService::new();
        let items = vec![
            "document.txt".to_string(),
            "image.png".to_string(),
            "readme.md".to_string(),
        ];

        let result = service.filter_items("readme", &items);
        assert!(result.contains(&2));
    }

    #[test]
    fn test_filter_items_case_insensitive() {
        let service = SearchService::new();
        let items = vec![
            "Document.txt".to_string(),
            "IMAGE.PNG".to_string(),
            "README.md".to_string(),
        ];

        let result = service.filter_items("doc", &items);
        assert!(!result.is_empty());
    }

    #[test]
    fn test_filter_items_partial_match() {
        let service = SearchService::new();
        let items = vec![
            "apples.txt".to_string(),
            "bananas.txt".to_string(),
            "cherries.txt".to_string(),
        ];

        let result = service.filter_items("app", &items);
        assert!(result.contains(&0));
    }

    #[test]
    fn test_filter_items_no_match() {
        let service = SearchService::new();
        let items = vec!["apples.txt".to_string(), "bananas.txt".to_string()];

        let result = service.filter_items("xyz", &items);
        assert!(result.is_empty());
    }

    #[test]
    fn test_filter_items_empty_items() {
        let service = SearchService::new();
        let items: Vec<String> = vec![];

        let result = service.filter_items("test", &items);
        assert!(result.is_empty());
    }

    #[test]
    fn test_filter_items_sorted_by_relevance() {
        let service = SearchService::new();
        let items = vec![
            "test_file.txt".to_string(),
            "test.txt".to_string(),
            "file_test.txt".to_string(),
        ];

        let result = service.filter_items("test", &items);
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn test_filter_items_with_special_characters() {
        let service = SearchService::new();
        let items = vec![
            "file (1).txt".to_string(),
            "file [2].txt".to_string(),
            "file {3}.txt".to_string(),
        ];

        let result = service.filter_items("file", &items);
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn test_filter_items_unicode() {
        let service = SearchService::new();
        let items = vec![
            "文档.txt".to_string(),
            "其他.txt".to_string(),
            "picture.png".to_string(),
        ];

        let result = service.filter_items("文档", &items);
        assert!(result.contains(&0));
    }

    #[test]
    fn test_filter_items_single_item() {
        let service = SearchService::new();
        let items = vec!["only_one.txt".to_string()];

        let result = service.filter_items("only", &items);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], 0);
    }

    #[test]
    fn test_filter_items_prefix_match() {
        let service = SearchService::new();
        let items = vec![
            "backup_old.txt".to_string(),
            "backup_new.txt".to_string(),
            "backup_2024.txt".to_string(),
        ];

        let result = service.filter_items("backup", &items);
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn test_filter_by_extension_txt() {
        let items = vec![
            ("readme.md".to_string(), "readme.md".to_string()),
            ("notes.txt".to_string(), "notes.txt".to_string()),
            ("image.png".to_string(), "image.png".to_string()),
        ];
        let result = SearchService::filter_by_extension(&items, "txt");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, "notes.txt");
    }

    #[test]
    fn test_filter_by_extension_empty_returns_all() {
        let items = vec![
            ("a.txt".to_string(), "".to_string()),
            ("b.rs".to_string(), "".to_string()),
        ];
        let result = SearchService::filter_by_extension(&items, "");
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_filter_by_extension_case_insensitive() {
        let items = vec![
            ("File.TXT".to_string(), "".to_string()),
            ("file.txt".to_string(), "".to_string()),
        ];
        let result = SearchService::filter_by_extension(&items, "TXT");
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_filter_by_extension_no_match() {
        let items = vec![
            ("doc.pdf".to_string(), "".to_string()),
            ("image.png".to_string(), "".to_string()),
        ];
        let result = SearchService::filter_by_extension(&items, "txt");
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_filter_by_size_exact_range() {
        let items = vec![
            ("small.txt".to_string(), 100u64),
            ("medium.txt".to_string(), 500),
            ("large.txt".to_string(), 5000),
        ];
        let result = SearchService::filter_by_size(&items, 200, 1000);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, "medium.txt");
    }

    #[test]
    fn test_filter_by_size_zero_min() {
        let items = vec![
            ("small.txt".to_string(), 10u64),
            ("big.txt".to_string(), 1000),
        ];
        let result = SearchService::filter_by_size(&items, 0, 100);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_filter_by_size_zero_max() {
        let items = vec![
            ("small.txt".to_string(), 10u64),
            ("big.txt".to_string(), 1000),
        ];
        let result = SearchService::filter_by_size(&items, 500, 0);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_filter_by_size_no_match() {
        let items = vec![("tiny.txt".to_string(), 5u64)];
        let result = SearchService::filter_by_size(&items, 100, 1000);
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_filter_by_extension_and_size() {
        let items = vec![
            ("small.txt".to_string(), 100u64),
            ("large.txt".to_string(), 5000),
            ("small.png".to_string(), 200),
        ];
        let by_ext: Vec<(String, u64)> = items
            .into_iter()
            .filter(|(n, _)| n.ends_with(".txt"))
            .collect();
        let result = SearchService::filter_by_size(&by_ext, 50, 0);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_search_result_default() {
        let state = SearchState::default();
        assert!(!state.is_searching);
        assert!(state.query.is_empty());
        assert!(!state.loading);
        assert!(!state.search_content);
        assert_eq!(state.size_min, 0);
        assert_eq!(state.size_max, 0);
        assert_eq!(state.extension_filter, "");
        assert!(!state.cancelled);
    }

    #[test]
    fn test_search_state_has_filters() {
        let state = SearchState {
            search_content: true,
            size_min: 1024,
            size_max: 1048576,
            extension_filter: "txt".to_string(),
            cancelled: false,
            ..Default::default()
        };
        assert!(state.search_content);
        assert_eq!(state.size_min, 1024);
        assert_eq!(state.size_max, 1048576);
        assert_eq!(state.extension_filter, "txt");
    }

    #[test]
    fn test_parse_aqs_kind_document() {
        let filters = SearchService::parse_aqs("kind:document");
        assert_eq!(filters.kind.as_deref(), Some("document"));
    }

    #[test]
    fn test_parse_aqs_kind_image() {
        let filters = SearchService::parse_aqs("kind:image");
        assert_eq!(filters.kind.as_deref(), Some("image"));
    }

    #[test]
    fn test_parse_aqs_size_greater_than() {
        let filters = SearchService::parse_aqs("size:>1mb");
        assert_eq!(filters.size_min, Some(1_048_576));
        assert!(filters.size_max.is_none());
    }

    #[test]
    fn test_parse_aqs_size_less_than() {
        let filters = SearchService::parse_aqs("size:<500kb");
        assert_eq!(filters.size_max, Some(500 * 1024));
        assert!(filters.size_min.is_none());
    }

    #[test]
    fn test_parse_aqs_size_exact() {
        let filters = SearchService::parse_aqs("size:=2gb");
        assert_eq!(filters.size_min, Some(2_147_483_648));
        assert_eq!(filters.size_max, Some(2_147_483_648));
    }

    #[test]
    fn test_parse_aqs_date_yesterday() {
        let filters = SearchService::parse_aqs("date:yesterday");
        assert!(filters.date_after.is_some());
    }

    #[test]
    fn test_parse_aqs_date_thisweek() {
        let filters = SearchService::parse_aqs("date:thisweek");
        assert!(filters.date_after.is_some());
    }

    #[test]
    fn test_parse_aqs_date_greater_than() {
        let filters = SearchService::parse_aqs("date:>2024-01-01");
        assert!(filters.date_after.is_some());
    }

    #[test]
    fn test_parse_aqs_ext() {
        let filters = SearchService::parse_aqs("ext:.txt");
        assert_eq!(filters.extension, Some("txt".to_string()));
    }

    #[test]
    fn test_parse_aqs_multiple_filters() {
        let filters = SearchService::parse_aqs("kind:image size:>500kb");
        assert_eq!(filters.kind.as_deref(), Some("image"));
        assert!(filters.size_min.unwrap() > 500_000);
    }

    #[test]
    fn test_parse_aqs_plain_text_returns_none() {
        let filters = SearchService::parse_aqs("hello world");
        assert!(filters.kind.is_none());
        assert!(filters.extension.is_none());
        assert!(filters.size_min.is_none());
        assert!(filters.date_after.is_none());
        assert_eq!(filters.text_query, "hello world");
    }

    #[test]
    fn test_parse_aqs_empty() {
        let filters = SearchService::parse_aqs("");
        assert!(filters.kind.is_none());
        assert!(filters.extension.is_none());
    }

    #[test]
    fn test_simple_regex_matching() {
        let re = SimpleRegex::compile(r"^report_\d{2,4}\.(pdf|docx)$", false).unwrap();
        assert!(re.is_match("report_2025.pdf"));
        assert!(re.is_match("REPORT_99.DOCX"));
        assert!(!re.is_match("report_1.pdf"));
        assert!(!re.is_match("report_2025.txt"));
    }

    #[test]
    fn test_wildcard_multi_pattern() {
        assert!(WildcardMatcher::match_indices("main.rs", "*.rs;*.toml !*test*", false).is_some());
        assert!(WildcardMatcher::match_indices("Cargo.toml", "*.rs;*.toml !*test*", false).is_some());
        assert!(WildcardMatcher::match_indices("main_test.rs", "*.rs;*.toml !*test*", false).is_none());
    }
}

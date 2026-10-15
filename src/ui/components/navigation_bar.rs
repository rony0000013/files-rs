//! Navigation Bar — breadcrumb path display, navigation controls, and search.
//! Per DESIGN_SPEC.md section 5.2 / 6.1.
//!
//! Redesigned per Files Community spec: compact breadcrumb trail with
//! dropdown arrows between segments, clickable folder path segments,
//! last segment highlighted/bold, dark background with blue accent.

use crate::core::action_registry::ActionRegistry;
use crate::core::path_util::FilesPath;
use crate::ui::components::dialogs::DialogType;
use crate::ui::constants::UiConstants;
use gpui::*;

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
    Command {
        action: crate::core::action_registry::ActionKind,
    },
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Suggestion {
    pub kind: SuggestionKind,
    pub label: String,
    pub icon: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NavigationBarState {
    pub current_path: FilesPath,
    pub breadcrumbs: Vec<FilesPath>,
    pub edit_mode: bool,
    pub edit_text: String,
    /// Sibling folders for dropdown (loaded on demand)
    pub siblings: Vec<FilesPath>,
    /// Parent path whose siblings are currently shown (-1 = none)
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

    // Breadcrumb sibling dropdown state
    /// Whether the breadcrumb sibling dropdown is open
    #[serde(default)]
    pub breadcrumb_dropdown_open: bool,
    /// Parent path whose siblings are shown
    #[serde(default)]
    pub breadcrumb_dropdown_parent: Option<FilesPath>,
    /// Sibling folders for the breadcrumb dropdown
    #[serde(skip)]
    pub breadcrumb_dropdown_siblings: Vec<FilesPath>,
    /// Filter text for the breadcrumb dropdown search
    #[serde(default)]
    pub breadcrumb_dropdown_filter: String,
    /// Screen position (x, y) for the breadcrumb dropdown
    #[serde(skip)]
    pub breadcrumb_dropdown_position: Option<(Pixels, Pixels)>,
    /// Selected index in the breadcrumb dropdown
    #[serde(default)]
    pub breadcrumb_dropdown_selected: Option<usize>,
}

fn default_omnibar_mode() -> OmnibarMode {
    OmnibarMode::Path
}

impl NavigationBarState {
    pub fn new(root: FilesPath) -> Self {
        let ancestors = root.ancestors();
        Self {
            current_path: root.clone(),
            breadcrumbs: ancestors,
            edit_mode: false,
            edit_text: root.to_string_lossy().to_string(),
            siblings: Vec::new(),
            open_dropdown_parent: None,
            omnibar_mode: OmnibarMode::Path,
            search_query: String::new(),
            command_query: String::new(),
            show_suggestions: false,
            suggestions: Vec::new(),
            selected_suggestion_index: None,
            breadcrumb_dropdown_open: false,
            breadcrumb_dropdown_parent: None,
            breadcrumb_dropdown_siblings: Vec::new(),
            breadcrumb_dropdown_filter: String::new(),
            breadcrumb_dropdown_position: None,
            breadcrumb_dropdown_selected: None,
        }
    }

    pub fn update_path(&mut self, path: FilesPath) {
        self.current_path = path.clone();
        self.breadcrumbs = path.ancestors();
        self.edit_text = path.to_string_lossy().to_string();
        self.edit_mode = false;
        self.siblings.clear();
        self.open_dropdown_parent = None;
        self.omnibar_mode = OmnibarMode::Path;
        self.search_query.clear();
        self.command_query.clear();
        self.show_suggestions = false;
        self.suggestions.clear();
        self.selected_suggestion_index = None;
        // Clear breadcrumb dropdown state
        self.breadcrumb_dropdown_open = false;
        self.breadcrumb_dropdown_parent = None;
        self.breadcrumb_dropdown_siblings.clear();
        self.breadcrumb_dropdown_filter.clear();
        self.breadcrumb_dropdown_position = None;
        self.breadcrumb_dropdown_selected = None;
    }

    /// Load sibling folders for a given parent path.
    /// Returns a sorted list of sibling directory names.
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
        // Sort by name (case-insensitive)
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

    pub fn update_suggestions(&mut self, registry: &ActionRegistry) {
        self.suggestions.clear();
        match self.omnibar_mode {
            OmnibarMode::Path => {
                let query = self.edit_text.trim();
                if query.is_empty() {
                    // Suggest drives and standard user folders
                    #[cfg(windows)]
                    {
                        if let Some(user_profile) = std::env::var_os("USERPROFILE") {
                            let home = std::path::PathBuf::from(user_profile);
                            let docs = home.join("Documents");
                            let downloads = home.join("Downloads");
                            let desktop = home.join("Desktop");

                            for p in &[home, docs, downloads, desktop] {
                                if p.exists() {
                                    let fp = FilesPath::from(p.clone());
                                    self.suggestions.push(Suggestion {
                                        label: fp.to_string_lossy().to_string(),
                                        icon: "\u{E8B7}".to_string(), // Folder icon
                                        kind: SuggestionKind::Path { path: fp },
                                    });
                                }
                            }
                        }
                    }
                } else {
                    // Try to autocomplete the path query
                    let path = std::path::Path::new(query);
                    let (parent, file_prefix) =
                        if query.ends_with('\\') || query.ends_with('/') || path.is_dir() {
                            (Some(path), "")
                        } else {
                            (
                                path.parent(),
                                path.file_name().and_then(|f| f.to_str()).unwrap_or(""),
                            )
                        };

                    if let Some(parent_path) = parent {
                        if parent_path.exists() {
                            if let Ok(entries) = std::fs::read_dir(parent_path) {
                                let prefix_lower = file_prefix.to_lowercase();
                                for entry in entries.filter_map(|e| e.ok()) {
                                    let entry_name =
                                        entry.file_name().to_string_lossy().to_string();
                                    if entry_name.to_lowercase().starts_with(&prefix_lower) {
                                        let is_dir =
                                            entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
                                        let icon = if is_dir {
                                            "\u{E8B7}".to_string() // Folder icon
                                        } else {
                                            "\u{E160}".to_string() // File icon
                                        };
                                        let fp = FilesPath::from(entry.path());
                                        self.suggestions.push(Suggestion {
                                            label: fp.to_string_lossy().to_string(),
                                            icon,
                                            kind: SuggestionKind::Path { path: fp },
                                        });
                                        if self.suggestions.len() >= 10 {
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            OmnibarMode::Search => {
                let query = self.search_query.trim();
                self.suggestions.push(Suggestion {
                    label: format!("Search for '{}' in current folder", query),
                    icon: "\u{E721}".to_string(), // Search icon
                    kind: SuggestionKind::SearchHistory {
                        query: query.to_string(),
                    },
                });

                let mock_histories = vec!["documents", "reports", "todo.txt", "images"];
                for hist in mock_histories {
                    if hist.contains(query) && hist != query {
                        self.suggestions.push(Suggestion {
                            label: hist.to_string(),
                            icon: "\u{E81C}".to_string(), // History icon
                            kind: SuggestionKind::SearchHistory {
                                query: hist.to_string(),
                            },
                        });
                    }
                }
            }
            OmnibarMode::CommandPalette => {
                let query = self.command_query.trim().to_lowercase();
                for (action, _combo) in registry.iter() {
                    let label = action.label();
                    if query.is_empty() || label.to_lowercase().contains(&query) {
                        self.suggestions.push(Suggestion {
                            label: label.to_string(),
                            icon: "\u{E762}".to_string(), // Action icon
                            kind: SuggestionKind::Command { action },
                        });
                        if self.suggestions.len() >= 15 {
                            break;
                        }
                    }
                }
            }
        }

        // Clamp selected index
        if let Some(idx) = self.selected_suggestion_index {
            if idx >= self.suggestions.len() {
                self.selected_suggestion_index = if self.suggestions.is_empty() {
                    None
                } else {
                    Some(self.suggestions.len() - 1)
                };
            }
        }
    }

    /// Open the breadcrumb sibling dropdown at a given screen position.
    pub fn open_breadcrumb_dropdown(&mut self, parent_path: FilesPath, position: (Pixels, Pixels)) {
        self.breadcrumb_dropdown_siblings.clear();
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

    /// Close the breadcrumb sibling dropdown.
    pub fn close_breadcrumb_dropdown(&mut self) {
        self.breadcrumb_dropdown_open = false;
        self.breadcrumb_dropdown_parent = None;
        self.breadcrumb_dropdown_siblings.clear();
        self.breadcrumb_dropdown_filter.clear();
        self.breadcrumb_dropdown_position = None;
        self.breadcrumb_dropdown_selected = None;
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
                let name = s
                    .as_path()
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy();
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
                let name = s
                    .as_path()
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy();
                fuzzy_match_indices(&name, &self.breadcrumb_dropdown_filter).is_some()
            })
            .collect()
    }
}

pub struct NavigationBarView;

impl NavigationBarView {
    fn render_nav_btn<F>(icon: &'static str, enabled: bool, on_click: F) -> impl IntoElement
    where
        F: Fn(&mut Window, &mut App) + 'static,
    {
        let mut btn = div()
            .flex()
            .items_center()
            .justify_center()
            .w(px(36.0))
            .h(px(32.0))
            .rounded(px(4.0))
            .cursor_pointer()
            .text_color(if enabled {
                rgba(UiConstants::TEXT_PRIMARY_RGBA)
            } else {
                rgba(UiConstants::TEXT_MUTED_RGBA)
            });

        if enabled {
            btn = btn
                .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
                .on_mouse_down(MouseButton::Left, move |_ev, win, cx| on_click(win, cx));
        }

        btn.child(
            div()
                .text_size(px(14.0))
                .font_family(UiConstants::FONT_ICONS)
                .child(icon),
        )
    }

    fn render_mode_icon_btn(
        icon: &'static str,
        is_active: bool,
        is_focused: bool,
        on_click: impl Fn(&mut Window, &mut App) + 'static,
    ) -> impl IntoElement {
        let text_color = if is_active && is_focused {
            rgba(UiConstants::ACCENT_RGBA)
        } else if is_active {
            rgba(UiConstants::TEXT_PRIMARY_RGBA)
        } else {
            rgba(UiConstants::TEXT_MUTED_RGBA)
        };

        div()
            .flex()
            .items_center()
            .justify_center()
            .w(px(28.0))
            .h(px(28.0))
            .rounded(px(4.0))
            .cursor_pointer()
            .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
            .on_mouse_down(MouseButton::Left, move |_ev, _win, cx| {
                cx.stop_propagation();
                on_click(_win, cx);
            })
            .child(
                div()
                    .text_size(px(13.0))
                    .font_family(UiConstants::FONT_ICONS)
                    .text_color(text_color)
                    .child(icon),
            )
    }

    fn render_pill_separator() -> impl IntoElement {
        div()
            .w(px(1.0))
            .h(px(14.0))
            .bg(rgba(UiConstants::DIVIDER_RGBA))
    }

    pub fn build(
        state: &NavigationBarState,
        can_go_back: bool,
        can_go_forward: bool,
        _side_pane: crate::ui::views::types::SidePaneMode,
        view: Entity<crate::app::AppWindow>,
        registry: &ActionRegistry,
    ) -> impl IntoElement {
        let v_back = view.clone();
        let v_forward = view.clone();
        let v_up = view.clone();
        let v_refresh = view.clone();

        let address_bar_pill = {
            let state_mode = state.omnibar_mode;
            let edit_mode = state.edit_mode;

            let mut pill = div()
                .flex()
                .items_center()
                .flex_grow()
                .h(Pixels::from(32.0))
                .bg(rgba(0xFFFFFF0Au32))
                .rounded(px(6.0))
                .border(px(1.0))
                .px(px(8.0))
                .relative();

            let is_focused = edit_mode
                || state_mode == OmnibarMode::CommandPalette
                || state_mode == OmnibarMode::Search;
            if is_focused {
                pill = pill
                    .border_color(rgba(UiConstants::ACCENT_RGBA))
                    .bg(rgba(0xFFFFFF14u32));
            } else {
                pill = pill
                    .border_color(rgba(0xFFFFFF20u32))
                    .hover(|s| s.bg(rgba(0xFFFFFF0Fu32)));
            }

            // Mode button click helpers
            let v_path = view.clone();
            let on_path_click = move |_: &mut Window, cx: &mut App| {
                v_path.update(cx, |this, cx| {
                    this.state.nav_bar.omnibar_mode = OmnibarMode::Path;
                    this.state.nav_bar.edit_mode = true;
                    this.state.nav_bar.edit_text = this
                        .state
                        .nav_bar
                        .current_path
                        .to_string_lossy()
                        .to_string();
                    let registry = this.state.action_registry.clone();
                    this.state.nav_bar.update_suggestions(&registry);
                    cx.notify();
                });
            };

            let v_cmd = view.clone();
            let on_cmd_click = move |_: &mut Window, cx: &mut App| {
                v_cmd.update(cx, |this, cx| {
                    this.state.nav_bar.omnibar_mode = OmnibarMode::CommandPalette;
                    this.state.nav_bar.command_query.clear();
                    this.state.nav_bar.show_suggestions = true;
                    let registry = this.state.action_registry.clone();
                    this.state.nav_bar.update_suggestions(&registry);
                    cx.notify();
                });
            };

            let v_search = view.clone();
            let on_search_click = move |_: &mut Window, cx: &mut App| {
                v_search.update(cx, |this, cx| {
                    this.state.nav_bar.omnibar_mode = OmnibarMode::Search;
                    this.state.nav_bar.search_query.clear();
                    this.state.nav_bar.show_suggestions = true;
                    let registry = this.state.action_registry.clone();
                    this.state.nav_bar.update_suggestions(&registry);
                    this.state.main_view.search_state.is_searching = true;
                    cx.notify();
                });
            };

            // Determine active mode for the mode icons
            let path_active = state_mode == OmnibarMode::Path;
            let cmd_active = state_mode == OmnibarMode::CommandPalette;
            let search_active = state_mode == OmnibarMode::Search;

            // --- LEFT: Mode icons (always present, always first) ---
            let mode_icons = div()
                .flex()
                .items_center()
                .gap(px(0.0))
                .child(Self::render_mode_icon_btn(
                    "\u{E8B7}", // Folder icon for Path
                    path_active,
                    is_focused,
                    on_path_click,
                ))
                .child(Self::render_pill_separator())
                .child(Self::render_mode_icon_btn(
                    "\u{E721}", // Search icon
                    search_active,
                    is_focused,
                    on_search_click,
                ))
                .child(Self::render_pill_separator())
                .child(Self::render_mode_icon_btn(
                    "\u{E101}", // Command icon (ActionCenterRemote)
                    cmd_active,
                    is_focused,
                    on_cmd_click,
                ));

            pill = pill.child(mode_icons);

            // --- MIDDLE: Content area (varies by mode) ---
            if state_mode == OmnibarMode::Path && !edit_mode {
                let v_pill = view.clone();

                // Build breadcrumbs trail
                let mut breadcrumb_views = Vec::new();

                // Render dedicated Home icon at the start
                let v_home = view.clone();
                breadcrumb_views.push(
                    div()
                        .flex()
                        .items_center()
                        .px(px(6.0))
                        .h(px(24.0))
                        .rounded(px(4.0))
                        .cursor_pointer()
                        .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
                        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                            cx.stop_propagation();
                            v_home.update(cx, |this, cx| {
                                if let Ok(home_path) = crate::core::navigation_service::NavigationService::get_home_path() {
                                    this.navigate(home_path, cx);
                                }
                            });
                        })
                        .child(
                            div()
                                .text_size(px(13.0))
                                .font_family(UiConstants::FONT_ICONS)
                                .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                .child("\u{E80F}") // Home icon
                        )
                        .into_any_element()
                );

                // Add chevrons and crumb segments
                for (i, crumb_path) in state.breadcrumbs.iter().enumerate() {
                    let label = if i == 0 {
                        let s = crumb_path.to_string_lossy();
                        if s == "/" || s.is_empty() {
                            "\u{E80F}".to_string()
                        } else {
                            s.split(':').next().unwrap_or(&s).to_string() // Drive letter
                        }
                    } else {
                        crumb_path
                            .as_path()
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string()
                    };

                    // Skip the first breadcrumb if it is the Home icon to avoid duplicate
                    if i == 0 && label == "\u{E80F}" {
                        continue;
                    }

                    // Add a chevron before this segment
                    let v_chevron = view.clone();
                    let parent_path = if i > 0 {
                        state.breadcrumbs[i - 1].clone()
                    } else {
                        crumb_path.clone()
                    };
                    breadcrumb_views.push(
                        div()
                            .flex()
                            .items_center()
                            .px(px(4.0))
                            .py(px(2.0))
                            .rounded(px(4.0))
                            .cursor_pointer()
                            .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
                            .on_mouse_down(MouseButton::Left, move |event, _, cx| {
                                // Position dropdown below nav bar, aligned with pointer X
                                let mouse_x = event.position.x;
                                v_chevron.update(cx, |this, cx| {
                                    this.state.nav_bar.open_breadcrumb_dropdown(
                                        parent_path.clone(),
                                        (mouse_x, px(84.0)), // below nav bar (tab=40 + nav=44)
                                    );
                                    cx.notify();
                                });
                            })
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .font_family(UiConstants::FONT_ICONS)
                                    .text_color(rgba(UiConstants::TEXT_MUTED_RGBA))
                                    .child("\u{E974}"), // ChevronRight
                            )
                            .into_any_element(),
                    );

                    // Add the segment itself
                    let is_last = i == state.breadcrumbs.len() - 1;
                    let path = crumb_path.clone();
                    let v_crumb = view.clone();

                    breadcrumb_views.push(
                        div()
                            .flex()
                            .items_center()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .px(px(6.0))
                                    .h(px(24.0))
                                    .rounded(px(4.0))
                                    .cursor_pointer()
                                    .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
                                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                        cx.stop_propagation();
                                        let p = path.clone();
                                        v_crumb.update(cx, |this, cx| {
                                            this.navigate(p, cx);
                                        });
                                    })
                                    .child(
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
                                                FontWeight::default()
                                            })
                                            .child(label),
                                    ),
                            )
                            .into_any_element(),
                    );
                }

                // Spacer fills remaining space and triggers Edit Mode when clicked
                let spacer =
                    div()
                        .flex_grow()
                        .h_full()
                        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                            v_pill.update(cx, |this, cx| {
                                this.state.nav_bar.edit_mode = true;
                                this.state.nav_bar.omnibar_mode = OmnibarMode::Path;
                                this.state.nav_bar.edit_text = this
                                    .state
                                    .nav_bar
                                    .current_path
                                    .to_string_lossy()
                                    .to_string();
                                let registry = this.state.action_registry.clone();
                                this.state.nav_bar.update_suggestions(&registry);
                                cx.notify();
                            });
                        });

                pill = pill
                    .child(Self::render_pill_separator())
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(2.0))
                            .children(breadcrumb_views),
                    )
                    .child(spacer);
            } else if state_mode == OmnibarMode::Path && edit_mode {
                let edit_text = state.edit_text.clone();
                let v_confirm = view.clone();
                let v_cancel = view.clone();
                pill = pill
                    .child(Self::render_pill_separator())
                    .child(
                        div().flex().items_center().flex_grow().gap(px(6.0)).child(
                            div()
                                .text_size(px(13.0))
                                .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                                .child(format!("{}{}", edit_text, "|")),
                        ),
                    )
                    // --- RIGHT: Clear / Confirm / Cancel ---
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(4.0))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .w(px(24.0))
                                    .h(px(24.0))
                                    .rounded(px(4.0))
                                    .cursor_pointer()
                                    .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
                                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                        v_confirm.update(cx, |this, cx| {
                                            this.handle_suggestion_confirm(cx);
                                        });
                                    })
                                    .child(
                                        div()
                                            .text_size(px(12.0))
                                            .font_family(UiConstants::FONT_ICONS)
                                            .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                            .child("\u{E73E}"), // Checkmark
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .w(px(24.0))
                                    .h(px(24.0))
                                    .rounded(px(4.0))
                                    .cursor_pointer()
                                    .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
                                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                        v_cancel.update(cx, |this, cx| {
                                            this.state.nav_bar.edit_mode = false;
                                            this.state.nav_bar.show_suggestions = false;
                                            cx.notify();
                                        });
                                    })
                                    .child(
                                        div()
                                            .text_size(px(12.0))
                                            .font_family(UiConstants::FONT_ICONS)
                                            .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                            .child("\u{E711}"), // Cancel
                                    ),
                            ),
                    );
            } else if state_mode == OmnibarMode::CommandPalette {
                let command_query = state.command_query.clone();
                let v_cancel = view.clone();
                pill = pill
                    .child(Self::render_pill_separator())
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .flex_grow()
                            .gap(px(8.0))
                            .child(
                                div()
                                    .text_size(px(13.0))
                                    .text_color(rgba(UiConstants::ACCENT_RGBA))
                                    .child(">"),
                            )
                            .child(
                                div()
                                    .text_size(px(13.0))
                                    .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                                    .child(format!("{}{}", command_query, "|")),
                            ),
                    )
                    // --- RIGHT: Close ---
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_center()
                            .w(px(24.0))
                            .h(px(24.0))
                            .rounded(px(4.0))
                            .cursor_pointer()
                            .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
                            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                v_cancel.update(cx, |this, cx| {
                                    this.state.nav_bar.omnibar_mode = OmnibarMode::Path;
                                    this.state.nav_bar.show_suggestions = false;
                                    cx.notify();
                                });
                            })
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .font_family(UiConstants::FONT_ICONS)
                                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                    .child("\u{E711}"), // Close
                            ),
                    );
            } else if state_mode == OmnibarMode::Search {
                let search_query = state.search_query.clone();
                let v_clear = view.clone();
                pill = pill
                    .child(Self::render_pill_separator())
                    .child(
                        div().flex().items_center().flex_grow().gap(px(8.0)).child(
                            div()
                                .text_size(px(13.0))
                                .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                                .child(format!("{}{}", search_query, "|")),
                        ),
                    )
                    // --- RIGHT: Clear ---
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_center()
                            .w(px(24.0))
                            .h(px(24.0))
                            .rounded(px(4.0))
                            .cursor_pointer()
                            .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
                            .on_mouse_down(MouseButton::Left, move |_event, _win, cx| {
                                cx.stop_propagation();
                                v_clear.update(cx, |this, cx| {
                                    this.state.nav_bar.omnibar_mode = OmnibarMode::Path;
                                    this.clear_search(cx);
                                });
                            })
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .font_family(UiConstants::FONT_ICONS)
                                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                    .child("\u{E711}"), // Close
                            ),
                    );
            }

            pill
        };

        // NOTE: Suggestions dropdown is rendered at app root level (not inside
        // the nav bar) to avoid being clipped by the nav bar's fixed height.
        // See build_omnibar_dropdown() and app.rs render().

        div()
            .id("navigation-bar")
            .flex()
            .items_center()
            .h(Pixels::from(48.0))
            .w_full()
            .px(Pixels::from(12.0))
            .gap(px(8.0))
            .bg(rgb(UiConstants::BG_MAIN_RGB))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(2.0))
                    .child(Self::render_nav_btn(
                        "\u{E72B}",
                        can_go_back,
                        move |_, cx| {
                            v_back.update(cx, |this, cx| {
                                // Dismiss omnibar before navigating
                                this.state.nav_bar.edit_mode = false;
                                this.state.nav_bar.show_suggestions = false;
                                if matches!(this.state.nav_bar.omnibar_mode, OmnibarMode::Search) {
                                    this.state.nav_bar.omnibar_mode = OmnibarMode::Path;
                                } else if matches!(
                                    this.state.nav_bar.omnibar_mode,
                                    OmnibarMode::CommandPalette
                                ) {
                                    this.state.nav_bar.omnibar_mode = OmnibarMode::Path;
                                }
                                let _ = this.navigate_back(cx);
                            });
                        },
                    ))
                    .child(Self::render_nav_btn(
                        "\u{E72A}",
                        can_go_forward,
                        move |_, cx| {
                            v_forward.update(cx, |this, cx| {
                                this.state.nav_bar.edit_mode = false;
                                this.state.nav_bar.show_suggestions = false;
                                if matches!(this.state.nav_bar.omnibar_mode, OmnibarMode::Search) {
                                    this.state.nav_bar.omnibar_mode = OmnibarMode::Path;
                                } else if matches!(
                                    this.state.nav_bar.omnibar_mode,
                                    OmnibarMode::CommandPalette
                                ) {
                                    this.state.nav_bar.omnibar_mode = OmnibarMode::Path;
                                }
                                let _ = this.navigate_forward(cx);
                            });
                        },
                    ))
                    .child(Self::render_nav_btn("\u{E197}", true, move |_, cx| {
                        v_up.update(cx, |this, cx| {
                            this.state.nav_bar.edit_mode = false;
                            this.state.nav_bar.show_suggestions = false;
                            if matches!(this.state.nav_bar.omnibar_mode, OmnibarMode::Search) {
                                this.state.nav_bar.omnibar_mode = OmnibarMode::Path;
                            } else if matches!(
                                this.state.nav_bar.omnibar_mode,
                                OmnibarMode::CommandPalette
                            ) {
                                this.state.nav_bar.omnibar_mode = OmnibarMode::Path;
                            }
                            let _ = this.navigate_up(cx);
                        });
                    }))
                    .child(Self::render_nav_btn("\u{E72C}", true, move |_, cx| {
                        v_refresh.update(cx, |this, cx| {
                            this.state.nav_bar.edit_mode = false;
                            this.state.nav_bar.show_suggestions = false;
                            if matches!(this.state.nav_bar.omnibar_mode, OmnibarMode::Search) {
                                this.state.nav_bar.omnibar_mode = OmnibarMode::Path;
                            } else if matches!(
                                this.state.nav_bar.omnibar_mode,
                                OmnibarMode::CommandPalette
                            ) {
                                this.state.nav_bar.omnibar_mode = OmnibarMode::Path;
                            }
                            let _ = this.refresh(cx);
                        });
                    })),
            )
            .child(address_bar_pill)
    }
}

pub fn build_suggestions_dropdown(
    state: &NavigationBarState,
    registry: &ActionRegistry,
    view: Entity<crate::app::AppWindow>,
) -> impl IntoElement {
    let selected_index = state.selected_suggestion_index;

    div()
        .bg(rgb(0x2D2D2Du32))
        .border(px(1.0))
        .border_color(rgba(UiConstants::DIVIDER_RGBA))
        .rounded(px(6.0))
        .py(px(4.0))
        .child(
            div()
                .flex_col()
                .children(state.suggestions.iter().enumerate().map(|(i, sug)| {
                    let is_selected = selected_index == Some(i);
                    let v_confirm = view.clone();
                    let label = sug.label.clone();
                    let kind = sug.kind.clone();

                    let mut row = div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .h(px(32.0))
                        .px(px(12.0))
                        .cursor_pointer()
                        .on_mouse_down(MouseButton::Left, move |_event, _win, cx| {
                            cx.stop_propagation();
                            v_confirm.update(cx, |this, cx| {
                                this.state.nav_bar.selected_suggestion_index = Some(i);
                                this.handle_suggestion_confirm(cx);
                            });
                        });

                    if is_selected {
                        row = row.bg(rgba(UiConstants::ACCENT_RGBA));
                    } else {
                        row = row.hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)));
                    }

                    row.child(
                        div()
                            .flex()
                            .items_center()
                            .child(
                                div()
                                    .font_family(UiConstants::FONT_ICONS)
                                    .text_size(px(14.0))
                                    .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                                    .child(sug.icon.clone()),
                            )
                            .child(
                                div()
                                    .ml(px(8.0))
                                    .text_size(px(13.0))
                                    .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                                    .child(label),
                            ),
                    )
                    .children({
                        if let SuggestionKind::Command { action } = kind {
                            if let Some(binding) = registry.binding(action) {
                                Some(
                                    div()
                                        .text_size(px(11.0))
                                        .text_color(rgba(UiConstants::TEXT_MUTED_RGBA))
                                        .child(binding.to_string()),
                                )
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    })
                })),
        )
}

/// Truncate a string in the middle, keeping `keep` chars on each side.
/// e.g. "very-long-folder-name" → "very-lo…ame" (keep=4)
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
    focus_handle: gpui::FocusHandle,
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
        let name = sibling_path
            .as_path()
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let path = (*sibling_path).clone();
        let is_selected = vis_idx == selected;

        let v_row = view.clone();
        let name_clone = name.clone();
        let filter_clone = filter_text.clone();

        let display_name = truncate_middle(&name, 8);
        let highlighted_name = if filter_clone.is_empty() {
            div()
                .text_size(px(12.0))
                .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                .child(display_name)
                .into_any_element()
        } else if let Some(indices) = fuzzy_match_indices(&name_clone, &filter_clone) {
            // Truncate in middle but preserve match highlighting
            let truncated = truncate_middle(&name_clone, 8);
            let truncated_chars: Vec<char> = truncated.chars().collect();
            let keep = 8usize;
            let original_len = name_clone.chars().count();
            let index_set: std::collections::HashSet<usize> = indices.into_iter().collect();
            let mut spans = Vec::new();
            for (ci, &ch) in truncated_chars.iter().enumerate() {
                // Map truncated index back to original index
                let orig_idx = if ci < keep {
                    ci
                } else {
                    original_len - (truncated_chars.len() - ci)
                };
                let ch_str = ch.to_string();
                if index_set.contains(&orig_idx) {
                    spans.push(
                        div()
                            .text_size(px(12.0))
                            .text_color(rgba(UiConstants::ACCENT_RGBA))
                            .font_weight(FontWeight(600.0))
                            .child(ch_str)
                            .into_any_element(),
                    );
                } else {
                    spans.push(
                        div()
                            .text_size(px(12.0))
                            .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                            .child(ch_str)
                            .into_any_element(),
                    );
                }
            }
            div().flex().children(spans).into_any_element()
        } else {
            div()
                .text_size(px(12.0))
                .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                .child(display_name)
                .into_any_element()
        };

        let mut row = div()
            .flex()
            .items_center()
            .gap(px(6.0))
            .px(px(8.0))
            .py(px(4.0))
            .rounded(px(4.0))
            .cursor_pointer()
            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                cx.stop_propagation();
                v_row.update(cx, |this, cx| {
                    this.state.nav_bar.close_breadcrumb_dropdown();
                    this.navigate(path.clone(), cx);
                });
            })
            .child(
                div()
                    .text_size(px(12.0))
                    .font_family(UiConstants::FONT_ICONS)
                    .text_color(rgba(UiConstants::ACCENT_RGBA))
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
        "Search folders...".to_string()
    } else {
        filter_text.clone()
    };
    let filter_color = if filter_text.is_empty() {
        UiConstants::TEXT_MUTED_RGBA
    } else {
        UiConstants::TEXT_PRIMARY_RGBA
    };
    // Blinking cursor via repeating animation
    let cursor_text = div()
        .id("breadcrumb-cursor")
        .text_size(px(12.0))
        .text_color(rgba(UiConstants::ACCENT_RGBA))
        .child(filter_display.clone())
        .with_animation(
            "breadcrumb-cursor-blink",
            Animation::new(std::time::Duration::from_millis(530)).repeat(),
            move |elem, delta| {
                // Show cursor for first half, hide for second half
                if delta < 0.5 {
                    elem.child("|")
                } else {
                    elem.child(" ")
                }
            },
        );

    Some(
        div()
            .absolute()
            .left(position.0)
            .top(position.1)
            .w(px(220.0))
            .max_h(px(240.0))
            .flex_col()
            .bg(rgb(0x2D2D2Du32))
            .border(px(1.0))
            .border_color(rgba(UiConstants::ACCENT_RGBA))
            .rounded(px(6.0))
            .overflow_hidden()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .px(px(8.0))
                    .py(px(6.0))
                    .border_b(px(1.0))
                    .border_color(rgba(UiConstants::DIVIDER_RGBA))
                    .child(
                        div()
                            .text_size(px(11.0))
                            .font_family(UiConstants::FONT_ICONS)
                            .text_color(rgba(UiConstants::TEXT_MUTED_RGBA))
                            .child("\u{E721}"),
                    )
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(rgba(filter_color))
                            .child(cursor_text),
                    ),
            )
            .child(
                div()
                    .id("breadcrumb-list")
                    .flex_col()
                    .max_h(px(200.0))
                    .overflow_scroll()
                    .py(px(2.0))
                    .children(row_views),
            ),
    )
}

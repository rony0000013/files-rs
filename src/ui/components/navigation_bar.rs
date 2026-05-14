//! Navigation Bar — breadcrumb path display, navigation controls, and search.
//! Per DESIGN_SPEC.md section 5.2 / 6.1.
//!
//! Redesigned per Files Community spec: compact breadcrumb trail with
//! dropdown arrows between segments, clickable folder path segments,
//! last segment highlighted/bold, dark background with blue accent.

use crate::core::path_util::FilesPath;
use crate::ui::components::dialogs::DialogType;
use crate::ui::constants::UiConstants;
use gpui::*;

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
        }
    }

    pub fn update_path(&mut self, path: FilesPath) {
        self.current_path = path.clone();
        self.breadcrumbs = path.ancestors();
        self.edit_text = path.to_string_lossy().to_string();
        self.edit_mode = false;
        self.siblings.clear();
        self.open_dropdown_parent = None;
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
            .w(px(32.0))
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
                .child(icon)
        )
    }

    pub fn build(
        state: &NavigationBarState,
        can_go_back: bool,
        can_go_forward: bool,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let view_edit = view.clone();
        let view_edit_commit = view.clone();

        let in_edit_mode = state.edit_mode;
        let edit_text = state.edit_text.clone();
        let path_str = state.current_path.to_string_lossy().to_string();

        let v_back = view.clone();
        let v_forward = view.clone();
        let v_up = view.clone();
        let v_refresh = view.clone();

        div()
            .id("navigation-bar")
            .flex()
            .items_center()
            .h(Pixels::from(44.0))
            .w_full()
            .px(Pixels::from(12.0))
            .gap(px(8.0))
            .bg(rgb(UiConstants::BG_MAIN_RGB))   // Same surface as tab bar
            .border_b(px(1.0))
            .border_color(rgba(UiConstants::DIVIDER_RGBA))
            // Navigation controls (Back, Forward, Up, Refresh)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(2.0))
                    .child(Self::render_nav_btn("\u{E72B}", can_go_back, move |_, cx| {
                        v_back.update(cx, |this, cx| { let _ = this.navigate_back(cx); });
                    }))
                    .child(Self::render_nav_btn("\u{E72A}", can_go_forward, move |_, cx| {
                        v_forward.update(cx, |this, cx| { let _ = this.navigate_forward(cx); });
                    }))
                    .child(Self::render_nav_btn("\u{E197}", true, move |_, cx| {
                        v_up.update(cx, |this, cx| { let _ = this.navigate_up(cx); });
                    }))
                    .child(Self::render_nav_btn("\u{E72C}", true, move |_, cx| {
                        v_refresh.update(cx, |this, cx| { let _ = this.refresh(cx); });
                    }))
            )
            // Address bar container — styled like Files-community rounded pill
            .child(
                div()
                    .flex()
                    .items_center()
                    .flex_grow()
                    .h(Pixels::from(32.0))
                    .bg(rgba(0xFFFFFF0Au32))  // ~4% white fill on Mica
                    .rounded(px(20.0))         // Fully pill-shaped
                    .border(px(1.0))
                    .border_color(rgba(0xFFFFFF20u32))  // ~12% white border
                    .px(px(4.0))
                    .children(state.breadcrumbs.iter().enumerate().map(|(i, crumb_path)| {
                        let is_last = i == state.breadcrumbs.len() - 1;
                        let label = if i == 0 {
                            let s = crumb_path.to_string_lossy();
                            if s == "/" || s.is_empty() {
                                "\u{E80F}".to_string() // Home icon
                            } else {
                                s.split(':').next().unwrap_or(&s).to_string() // Drive letter
                            }
                        } else {
                            crumb_path.as_path().file_name().unwrap_or_default().to_string_lossy().to_string()
                        };

                        let path = crumb_path.clone();
                        let v_crumb = view.clone();

                        div()
                            .flex()
                            .items_center()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .px(px(6.0))
                                    .h(px(28.0))
                                    .rounded(px(4.0))
                                    .cursor_pointer()
                                    .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
                                    .on_mouse_down(MouseButton::Left, {
                                        let p = path.clone();
                                        move |_event, _window, cx| {
                                            v_crumb.update(cx, |this, cx| {
                                                this.navigate(p.clone(), cx);
                                            });
                                        }
                                    })
                                    .child(
                                        div()
                                            .text_size(px(13.0))
                                            .font_family(if i == 0 && label == "\u{E80F}" { UiConstants::FONT_ICONS } else { UiConstants::FONT_FAMILY_DEFAULT })
                                            .text_color(if is_last { rgba(UiConstants::TEXT_PRIMARY_RGBA) } else { rgba(UiConstants::TEXT_SECONDARY_RGBA) })
                                            .font_weight(if is_last { FontWeight(600.0) } else { FontWeight::default() })
                                            .child(label)
                                    )
                            )
                            .children(if !is_last {
                                Some(div()
                                    .px(px(2.0))
                                    .child(
                                        div()
                                            .text_size(px(10.0))
                                            .font_family(UiConstants::FONT_ICONS)
                                            .text_color(rgba(UiConstants::TEXT_MUTED_RGBA))
                                            .child("\u{E974}") // ChevronRight
                                    ))
                            } else {
                                None
                            })
                    }))
            )
            // Search / Status indicator
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .w(px(200.0))
                    .h(px(32.0))
                    .ml(px(8.0))
                    .bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA))
                    .rounded(px(6.0))
                    .border(px(1.0))
                    .border_color(rgba(UiConstants::DIVIDER_RGBA))
                    .px(px(12.0))
                    .child(
                        div()
                            .w_full()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .text_size(px(13.0))
                                    .text_color(rgba(UiConstants::TEXT_MUTED_RGBA))
                                    .child("Search...")
                            )
                            .child(
                                div()
                                    .text_size(px(14.0))
                                    .font_family(UiConstants::FONT_ICONS)
                                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                    .child("\u{E721}") // Search icon
                            )
                    )
            )
    }
}

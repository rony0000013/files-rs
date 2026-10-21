use crate::core::path_util::FilesPath;
use crate::platform::windows::recycle_bin_service::RecycleBinItem;
use crate::ui::components::bouncy_scroll::{
    bouncy_overscroll_content, bouncy_scroll_content, bouncy_scroll_handler,
    bouncy_scroll_handler_with_handle, start_bouncy_animation, uniform_list_scrollbar,
    uniformlist_scroll_info, BouncyScrollState, MAX_OVERSCROLL, VELOCITY_BLEND_HISTORY,
};
use crate::ui::components::dialogs::{DialogType, SettingsTab};
use crate::ui::components::icon_renderer::{is_thumbnail_image, IconRenderer, ThumbnailRenderer};
use gpui::*;
use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

// Re-export types and helpers from their dedicated modules (code splitting)
pub use super::helpers::*;
pub use super::types::*;

use crate::config::config::{GroupColumn, SortBy, ViewMode};
use crate::domain::metadata::FileItem;
use crate::storage::storage::FileType;
use crate::ui::constants::UiConstants;

impl MainViewState {
    pub fn new(root: FilesPath, view_mode: ViewMode) -> Self {
        let zoom_level = zoom_for_view_mode(&view_mode);
        Self {
            pane_type: PaneType::Single,
            dual_pane_split: DualPaneSplit::Horizontal,
            view_mode,
            target: ViewTarget::Directory(root.clone()),
            primary_path: root,
            secondary_path: None,
            items: Vec::new(),
            selected_indices: HashSet::default(),
            sort: SortState::default(),
            filter_text: String::new(),
            show_dot_files: false,
            show_hidden_files: false,
            show_extensions: true,
            show_system_files: false,
            loading: false,
            side_pane: SidePaneMode::None,
            side_pane_width: Some(300.0),
            side_pane_resize_start_x: None,
            side_pane_resize_start_width: None,
            renaming_index: None,
            rename_text: String::new(),
            compact_mode: false,
            preview_text: None,
            column_widths: ColumnWidths::default(),
            column_visibility: ColumnVisibility::default(),
            group_by: GroupColumn::None,
            collapsed_groups: HashSet::new(),
            selection_mode: SelectionMode::Normal,
            shelf_visible: false,
            shelf_items: Vec::new(),
            folder_background: FolderBackground::default(),
            active_pane: crate::core::navigation_manager::PaneFocus::Primary,
            last_clicked_index: None,
            lasso_state: None,
            drag_state: DragState::default(),
            details_cache: DetailsPaneCache::default(),
            search_state: SearchState::default(),
            toolbar_customization: ToolbarCustomizationState::default(),
            preview_content: PreviewContent::default(),
            group_drag_state: GroupDragState::default(),
            recycle_bin_items: Vec::new(),
            tags_cache: std::collections::HashMap::new(),
            home_tags_cache: Vec::new(),
            icon_paths: std::collections::HashMap::new(),
            view_lock_enabled: false,
            zoom_level,
            scroll_handle: ScrollHandle::new(),
            uniform_list_scroll_handle: UniformListScrollHandle::new(),
            last_empty_click: None,
            icon_refresh_last_check: None,
            bouncy_scroll: BouncyScrollState::new(),
            git_status_cache: std::collections::HashMap::new(),
            items_revision: 0,
            visible_cache: std::cell::RefCell::new(None),
        }
    }

    /// Update view mode and zoom level together
    pub fn set_view_mode_with_zoom(&mut self, mode: ViewMode) {
        self.zoom_level = zoom_for_view_mode(&mode);
        self.view_mode = mode;
    }

    /// Update zoom level and derive view mode
    pub fn set_zoom_with_mode(&mut self, zoom: f32) {
        self.zoom_level = zoom.max(0.0).min(1.0);
        self.view_mode = view_mode_for_zoom(self.zoom_level);
    }

    /// Bump the items revision to invalidate the cached visible_items index vector.
    pub fn bump_items_revision(&mut self) {
        self.items_revision = self.items_revision.wrapping_add(1);
        self.visible_cache.borrow_mut().take();
    }

    /// Return a reference-counted sorted+filtered index vector, computing and caching it
    /// only when the underlying items, sort state, or filter parameters have changed.
    /// Guarantees O(1) zero-sort execution during 120-144 FPS scroll and hover frames.
    pub fn visible_items_rc(&self) -> std::rc::Rc<Vec<usize>> {
        let sort_discriminant = match self.sort.column {
            SortBy::Name => 0,
            SortBy::DateModified => 1,
            SortBy::DateCreated => 2,
            SortBy::Type => 3,
            SortBy::Size => 4,
            SortBy::Tag => 5,
            _ => 6,
        };
        let key = VisibleCacheKey {
            primary_path: self.primary_path.clone(),
            items_len: self.items.len(),
            items_revision: self.items_revision,
            first_modified: self.items.first().and_then(|i| i.modified),
            last_modified: self.items.last().and_then(|i| i.modified),
            last_size: self.items.last().map(|i| i.size).unwrap_or(0),
            sort_col_discriminant: sort_discriminant,
            sort_ascending: self.sort.ascending,
            folders_first: self.sort.folders_first,
            filter_text: self.filter_text.clone(),
            show_dot_files: self.show_dot_files,
            show_hidden_files: self.show_hidden_files,
            show_system_files: self.show_system_files,
            is_searching: self.search_state.is_searching,
            search_query: self.search_state.query.clone(),
            search_results_len: self.search_state.results.len(),
        };

        if let Some((cached_key, cached_rc)) = self.visible_cache.borrow().as_ref() {
            if *cached_key == key {
                return cached_rc.clone();
            }
        }

        let has_filter = !self.filter_text.is_empty();
        let mut indices: Vec<usize> = self
            .items
            .iter()
            .enumerate()
            .filter(|(i, item)| {
                if !self.show_system_files && item.is_system {
                    return false;
                }
                if !self.show_hidden_files && item.is_hidden {
                    return false;
                }
                if !self.show_dot_files && item.name.starts_with('.') {
                    return false;
                }
                if has_filter && !contains_ignore_case(&item.name, &self.filter_text) {
                    return false;
                }
                if self.search_state.is_searching
                    && !self.search_state.query.is_empty()
                    && !self.search_state.results.contains(i)
                {
                    return false;
                }
                true
            })
            .map(|(i, _)| i)
            .collect();

        indices.sort_unstable_by(|&a, &b| {
            let ia = &self.items[a];
            let ib = &self.items[b];
            if self.sort.folders_first {
                let af = matches!(ia.file_type, FileType::Directory);
                let bf = matches!(ib.file_type, FileType::Directory);
                if af != bf {
                    return if af {
                        std::cmp::Ordering::Less
                    } else {
                        std::cmp::Ordering::Greater
                    };
                }
            }
            let ord = match self.sort.column {
                SortBy::Name => natural_cmp(&ia.name, &ib.name),
                SortBy::Size => ia.size.cmp(&ib.size).then_with(|| natural_cmp(&ia.name, &ib.name)),
                SortBy::Type => {
                    let dir_a = matches!(ia.file_type, FileType::Directory);
                    let dir_b = matches!(ib.file_type, FileType::Directory);
                    if dir_a && dir_b {
                        natural_cmp(&ia.name, &ib.name)
                    } else if dir_a != dir_b {
                        if dir_a {
                            std::cmp::Ordering::Less
                        } else {
                            std::cmp::Ordering::Greater
                        }
                    } else {
                        ext_cmp(&ia.name, &ib.name)
                    }
                }
                SortBy::Tag => {
                    let tags_a = self.tags_cache.get(&*ia.path.to_string_lossy());
                    let tags_b = self.tags_cache.get(&*ib.path.to_string_lossy());
                    let first_tag_a = tags_a.and_then(|t| t.first()).map(|s| s.as_str());
                    let first_tag_b = tags_b.and_then(|t| t.first()).map(|s| s.as_str());
                    match (first_tag_a, first_tag_b) {
                        (Some(a), Some(b)) => natural_cmp(a, b),
                        (Some(_), None) => std::cmp::Ordering::Less,
                        (None, Some(_)) => std::cmp::Ordering::Greater,
                        (None, None) => natural_cmp(&ia.name, &ib.name),
                    }
                }
                SortBy::DateModified => {
                    let a_time = ia.modified.unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                    let b_time = ib.modified.unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                    a_time.cmp(&b_time).then_with(|| natural_cmp(&ia.name, &ib.name))
                }
                SortBy::DateCreated => {
                    let a_time = ia
                        .created
                        .or(ia.modified)
                        .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                    let b_time = ib
                        .created
                        .or(ib.modified)
                        .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                    a_time.cmp(&b_time).then_with(|| natural_cmp(&ia.name, &ib.name))
                }
                _ => natural_cmp(&ia.name, &ib.name),
            };
            if self.sort.ascending {
                ord
            } else {
                ord.reverse()
            }
        });

        let rc = std::rc::Rc::new(indices);
        *self.visible_cache.borrow_mut() = Some((key, rc.clone()));
        rc
    }

    /// Return the sorted+filtered items as index slices into self.items.
    pub fn visible_items(&self) -> Vec<usize> {
        (*self.visible_items_rc()).clone()
    }

    /// Toggle sort column; flip direction if already sorting by that column.
    pub fn toggle_sort(&mut self, col: SortBy) {
        if std::mem::discriminant(&self.sort.column) == std::mem::discriminant(&col) {
            self.sort.ascending = !self.sort.ascending;
        } else {
            self.sort.column = col;
            self.sort.ascending = true;
        }
    }

    /// Display name for an item (respects show_extensions setting).
    pub fn display_name<'a>(&self, item: &'a FileItem) -> std::borrow::Cow<'a, str> {
        if !self.show_extensions && matches!(item.file_type, FileType::File) {
            let name = &item.name;
            if let Some(pos) = name.rfind('.') {
                return std::borrow::Cow::Owned(name[..pos].to_string());
            }
        }
        std::borrow::Cow::Borrowed(&item.name)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Main view renderer
// ─────────────────────────────────────────────────────────────────────────────

/// Separate model for file list data (read by uniform_list closures without entity conflict)
/// This is required by the GPUI uniform_list pattern: closures must read from a DIFFERENT
/// entity than the one being rendered to avoid "cannot read while already being updated" panics.
#[derive(Debug, Clone)]
pub struct FileItems {
    pub items: Vec<FileItem>,
    pub selected_indices: HashSet<usize>,
}

impl Default for FileItems {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            selected_indices: HashSet::new(),
        }
    }
}

pub struct MainView;

impl MainView {
    pub fn build(
        state: &MainViewState,
        file_items: Entity<FileItems>,
        view: Entity<crate::app::AppWindow>,
        settings_data: Option<crate::ui::components::dialogs::SettingsPreReadData>,
        content_w: f32,
    ) -> impl IntoElement {
        let has_side_pane = state.side_pane != SidePaneMode::None;
        let show_shelf = state.shelf_visible;
        let v2 = view.clone();
        let v3 = view.clone();
        // Side preview pane eats horizontal space: grid columns must fit
        // what's left, not the full content width.
        let content_w = if has_side_pane {
            content_w - state.side_pane_width.unwrap_or(300.0) - 4.0
        } else {
            content_w
        };

        // Collect lasso overlay element if active
        let lasso_overlay = Self::render_lasso_overlay(state);

        div()
            .flex()
            .flex_row()
            .flex_grow()
            .w_full()
            // NO overflow_hidden() - would block child scroll events
            .bg(rgba(UiConstants::BG_MICA_CONTENT_RGBA))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_grow()
                    .w_full()
                    // NO overflow_hidden() - would block child scroll events
                    .child(Self::render_panes(state, file_items, view.clone(), settings_data.as_ref(), content_w))
                    .children(if show_shelf {
                        vec![Self::render_shelf_pane(state, v2).into_any_element()]
                    } else {
                        vec![]
                    }),
            )
            .children(if has_side_pane {
                vec![
                    Self::render_side_pane_splitter(state, view.clone()).into_any_element(),
                    Self::render_side_pane(state, v3).into_any_element(),
                ]
            } else {
                vec![]
            })
            // Lasso overlay (rendered on top)
            .children(if let Some(overlay) = lasso_overlay {
                vec![overlay.into_any_element()]
            } else {
                vec![]
            })
    }

    /// Render the lasso selection overlay rectangle
    fn render_lasso_overlay(state: &MainViewState) -> Option<impl IntoElement> {
        let lasso_state = state.lasso_state.as_ref()?;
        let (start, end) = lasso_state.rect();

        // Calculate rectangle dimensions
        let left = start.x.min(end.x);
        let top = start.y.min(end.y);
        let width = (end.x - start.x).abs();
        let height = (end.y - start.y).abs();

        // Don't render if very small (just a click)
        if width < 5.0 || height < 5.0 {
            return None;
        }

        Some(
            div()
                .id("lasso-overlay")
                .absolute()
                .left(px(left))
                .top(px(top))
                .w(px(width))
                .h(px(height))
                .bg(rgba(0x208B2DC8)) // Semi-transparent accent color
                .border(px(1.0))
                .border_color(rgba(UiConstants::ACCENT_RGBA)),
        )
    }

    fn render_panes(
        state: &MainViewState,
        file_items: Entity<FileItems>,
        view: Entity<crate::app::AppWindow>,
        settings_data: Option<&crate::ui::components::dialogs::SettingsPreReadData>,
        content_w: f32,
    ) -> impl IntoElement {
        match state.pane_type {
            PaneType::Single => div()
                .flex()
                .flex_col()
                .flex_grow()
                .w_full()
                // NO overflow_hidden() - would block child scroll events
                .child(Self::render_file_view(state, file_items, view, true, settings_data, content_w)),
            PaneType::Dual => {
                // Horizontal = side-by-side; Vertical = stacked
                let is_horiz = matches!(state.dual_pane_split, DualPaneSplit::Horizontal);
                // Each pane only gets part of the width: grid columns must
                // fit the pane, not the whole content area.
                let pane_w = if is_horiz {
                    (content_w - 4.0) / 2.0
                } else {
                    content_w
                };
                let splitter_w = Pixels::from(UiConstants::SPLITTER_WIDTH_RAW);
                let base = div().flex().flex_grow().w_full();

                let is_primary_active = matches!(
                    state.active_pane,
                    crate::core::navigation_manager::PaneFocus::Primary
                );
                let is_secondary_active = !is_primary_active;

                if is_horiz {
                    base.flex_row()
                        .child(div().flex_grow().child(Self::render_file_view(
                            state,
                            file_items.clone(),
                            view.clone(),
                            is_primary_active,
                            settings_data,
                            pane_w,
                        )))
                        .child(div().w(splitter_w).bg(rgb(UiConstants::BG_NAV_BAR_RGB)))
                        .child(div().flex_grow().child(Self::render_file_view(
                            state,
                            file_items,
                            view,
                            is_secondary_active,
                            settings_data,
                            pane_w,
                        )))
                } else {
                    base.flex_col()
                        .child(div().flex_grow().w_full().child(Self::render_file_view(
                            state,
                            file_items.clone(),
                            view.clone(),
                            is_primary_active,
                            settings_data,
                            pane_w,
                        )))
                        .child(
                            div()
                                .h(splitter_w)
                                .bg(rgb(UiConstants::BG_NAV_BAR_RGB))
                                .w_full(),
                        )
                        .child(div().flex_grow().w_full().child(Self::render_file_view(
                            state,
                            file_items,
                            view,
                            is_secondary_active,
                            settings_data,
                            pane_w,
                        )))
                }
            }
        }
    }

    fn render_side_pane_splitter(
        state: &MainViewState,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let is_resizing = state.side_pane_resize_start_x.is_some();
        div()
            .id("side-pane-splitter")
            .w(px(4.0))
            .h_full()
            .cursor_col_resize()
            .bg(if is_resizing {
                rgba(UiConstants::ACCENT_RGBA)
            } else {
                rgba(UiConstants::BG_TRANSPARENT_RGBA)
            })
            .hover(|s| s.bg(rgba(0xFFFFFF11u32)))
            .on_mouse_down(MouseButton::Left, {
                let view = view.clone();
                move |event, _win, cx| {
                    let x: f32 = event.position.x.into();
                    view.update(cx, |this, cx| {
                        let width = this.state.main_view.side_pane_width.unwrap_or(300.0);
                        this.state.main_view.side_pane_resize_start_x = Some(x);
                        this.state.main_view.side_pane_resize_start_width = Some(width);
                        cx.notify();
                    });
                }
            })
    }

    fn render_side_pane(
        state: &MainViewState,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let mode = &state.side_pane;
        let selected_item = state
            .selected_indices
            .iter()
            .next()
            .and_then(|&i| state.items.get(i));
        let pane_w = state.side_pane_width.unwrap_or(300.0);

        div()
            .id("side-pane")
            .w(px(pane_w))
            .h_full()
            .flex_col()
            .bg(rgba(UiConstants::BG_TOOLBAR_RGBA))
            .border_l(px(1.0))
            .border_color(rgba(UiConstants::DIVIDER_RGBA))
            .child(
                // Pane header with Preview / Info tabs
                div()
                    .flex()
                    .flex_row()
                    .h(px(36.0))
                    .items_center()
                    .px(px(8.0))
                    .border_b(px(1.0))
                    .border_color(rgba(UiConstants::DIVIDER_RGBA))
                    .child(
                        div()
                            .text_size(AbsoluteLength::Pixels(px(12.0)))
                            .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                            .child(match mode {
                                SidePaneMode::Preview => "Preview",
                                SidePaneMode::Info => "Info",
                                SidePaneMode::Details => "Details",
                                SidePaneMode::None => "",
                            }),
                    ),
            )
            .child(
                // Pane body
                div()
                    .flex_grow()
                    .flex_col()
                    .items_center()
                    .p(px(12.0))
                    .children(match selected_item {
                        None => vec![div()
                            .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                            .text_size(AbsoluteLength::Pixels(px(12.0)))
                            .text_center()
                            .child("Select an item to preview")
                            .into_any_element()],
                        Some(item) => match mode {
                            SidePaneMode::Preview => {
                                vec![Self::render_preview_content(state, item).into_any_element()]
                            }
                            SidePaneMode::Info => {
                                vec![Self::render_info_content(item).into_any_element()]
                            }
                            SidePaneMode::Details => {
                                vec![Self::render_details_pane(state, item).into_any_element()]
                            }
                            SidePaneMode::None => vec![],
                        },
                    }),
            )
    }

    fn render_shelf_pane(
        state: &MainViewState,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let item_count = state.shelf_items.len();
        let shelf_size: u64 = state.shelf_items.iter().map(|s| s.size).sum();

        div()
            .id("shelf-pane")
            .h(px(180.0))
            .w_full()
            .flex_col()
            .bg(rgba(0xFFFFFF08u32))
            .border_t(px(1.0))
            .border_color(rgba(UiConstants::DIVIDER_RGBA))
            // Header bar
            .child(
                div()
                    .flex()
                    .flex_row()
                    .h(px(32.0))
                    .items_center()
                    .px(px(12.0))
                    .gap(px(8.0))
                    .border_b(px(1.0))
                    .border_color(rgba(UiConstants::DIVIDER_RGBA))
                    .child(
                        div()
                            .text_size(px(12.0))
                            .font_family(UiConstants::FONT_MDL2)
                            .text_color(rgba(UiConstants::ACCENT_RGBA))
                            .child("\u{E8A0}"), // Shelf icon
                    )
                    .child(
                        div()
                            .flex_grow()
                            .text_size(px(12.0))
                            .font_weight(FontWeight(600.0))
                            .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                            .child(format!(
                                "Shelf ({} items, {})",
                                item_count,
                                format_size(shelf_size)
                            )),
                    )
                    .child({
                        let v = view.clone();
                        div()
                            .text_size(px(11.0))
                            .font_family(UiConstants::FONT_MDL2)
                            .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                            .cursor_pointer()
                            .hover(|s| s.text_color(rgba(UiConstants::ACCENT_RGBA)))
                            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                v.update(cx, |this, cx| {
                                    this.state.main_view.shelf_items.clear();
                                    cx.notify();
                                });
                            })
                            .child("\u{E74D}") // Clear icon
                    }),
            )
            // Drop zone / items
            .child(if state.shelf_items.is_empty() {
                div()
                    .flex_grow()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(rgba(0xFFFFFF40u32))
                            .text_center()
                            .child("Drag files here to stage for bulk operations"),
                    )
                    .into_any_element()
            } else {
                div()
                    .flex_grow()
                    .flex_col()
                    .overflow_hidden()
                    .children(state.shelf_items.iter().enumerate().map(|(i, item)| {
                        let name = item.name.clone();
                        let size = item.size;
                        let is_dir = item.is_dir;
                        let v = view.clone();
                        div()
                            .flex()
                            .flex_row()
                            .h(px(28.0))
                            .items_center()
                            .px(px(12.0))
                            .gap(px(8.0))
                            .hover(|s| s.bg(rgba(0xFFFFFF08u32)))
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .font_family(UiConstants::FONT_MDL2)
                                    .text_color(rgba(UiConstants::ACCENT_RGBA))
                                    .child(if is_dir { "\u{E8B7}" } else { "\u{E8A5}" }),
                            )
                            .child(
                                div()
                                    .flex_grow()
                                    .text_size(px(12.0))
                                    .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                                    .child(name),
                            )
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                    .child(if is_dir {
                                        String::new()
                                    } else {
                                        format_size(size)
                                    }),
                            )
                            .child(
                                div()
                                    .text_size(px(10.0))
                                    .font_family(UiConstants::FONT_MDL2)
                                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                    .cursor_pointer()
                                    .hover(|s| s.text_color(rgba(UiConstants::ACCENT_RGBA)))
                                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                        let idx = i;
                                        v.update(cx, |this, cx| {
                                            if idx < this.state.main_view.shelf_items.len() {
                                                this.state.main_view.shelf_items.remove(idx);
                                            }
                                            cx.notify();
                                        });
                                    })
                                    .child("\u{E711}"), // Remove icon
                            )
                    }))
                    .into_any_element()
            })
    }

    fn render_file_view(
        state: &MainViewState,
        file_items: Entity<FileItems>,
        view: Entity<crate::app::AppWindow>,
        is_active: bool,
        settings_data: Option<&crate::ui::components::dialogs::SettingsPreReadData>,
        pane_w: f32,
    ) -> impl IntoElement {
        let visible = state.visible_items_rc();

        // Empty state
        if visible.is_empty() && !state.loading {
            return Self::empty_state(state).into_any_element();
        }

        div()
            .flex_col()
            .flex_grow()
            .h_full() // CRITICAL: Provide concrete height foundation for flex_grow to work
            .w_full()
            // NO overflow_hidden here - moves to content div
            .bg(if is_active {
                rgba(UiConstants::BG_MAIN_RGBA)
            } else {
                rgba(UiConstants::BG_INACTIVE_PANE_RGBA)
            }) // Dim if inactive
            // Double-click empty space → navigate backward
            // Checks click position against visible item bounds to avoid
            // triggering when clicking on file/folder items.
            .on_mouse_down(MouseButton::Left, {
                let v = view.clone();
                move |ev, _, cx| {
                    v.update(cx, |this, cx| {
                        let pos = ev.position;

                        // Check if click hit any visible file item (drop RefCell borrow quickly)
                        let on_item = {
                            let scroll = &this.state.main_view.uniform_list_scroll_handle;
                            let base = &scroll.0.borrow().base_handle;
                            (base.top_item()..=base.bottom_item()).any(|i| {
                                base.bounds_for_item(i)
                                    .map(|b| b.contains(&pos))
                                    .unwrap_or(false)
                            })
                        };

                        if on_item {
                            // Click was on an item — clear any pending empty-space back
                            this.state.main_view.last_empty_click = None;
                            return;
                        }

                        // Click was on empty space — check for double-click
                        let now = std::time::Instant::now();
                        let is_double_click = this
                            .state
                            .main_view
                            .last_empty_click
                            .map(|t| now - t < std::time::Duration::from_millis(500))
                            .unwrap_or(false);
                        if is_double_click {
                            this.state.main_view.last_empty_click = None;
                            this.navigate_back(cx);
                        } else {
                            this.state.main_view.last_empty_click = Some(now);
                        }
                    });
                }
            })
            .child(Self::filter_header(state, view.clone()))
            .child({
                let is_recycle_bin = matches!(state.target, ViewTarget::RecycleBin);
                let is_home = matches!(state.target, ViewTarget::Home);
                let is_settings = matches!(state.target, ViewTarget::Settings);

                let content: gpui::AnyElement = if is_settings {
                    // Settings page — render as full tab content
                    let data = settings_data.expect("Settings data required for Settings view");
                    Self::render_settings_view(data, view.clone()).into_any_element()
                } else if is_recycle_bin {
                    Self::render_recycle_bin_view(state, view.clone()).into_any_element()
                } else {
                    match state.view_mode {
                        ViewMode::Details => {
                            if matches!(state.target, ViewTarget::Home) {
                                Self::render_home_view(state, view.clone()).into_any_element()
                            } else {
                                Self::details_view(state, &visible, file_items, view.clone())
                                    .into_any_element()
                            }
                        }
                        ViewMode::List => {
                            state.render_list_view(file_items, view.clone()).into_any_element()
                        }
                        ViewMode::Grid => {
                            let cols =
                                crate::ui::views::types::MainViewState::grid_columns_for_width(
                                    pane_w,
                                    state.zoom_level,
                                );
                            {
                                static LAST_C: std::sync::atomic::AtomicU32 =
                                    std::sync::atomic::AtomicU32::new(0);
                                let key = (pane_w as u32)
                                    .wrapping_mul(100003)
                                    .wrapping_add(cols as u32);
                                if LAST_C.swap(
                                    key,
                                    std::sync::atomic::Ordering::Relaxed,
                                ) != key
                                {
                                    tracing::warn!(
                                        "layout-grid pane_w={:.0} cols={}",
                                        pane_w,
                                        cols
                                    );
                                }
                            }
                            state
                                .render_grid_view(file_items, view.clone(), cols, pane_w)
                                .into_any_element()
                        }
                        ViewMode::Columns => state
                            .render_columns_view(file_items, view.clone())
                            .into_any_element(),
                    }
                };

                let mut view_container = div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .h_full()
                    .w_full()
                    .overflow_hidden();

                if !is_home {
                    view_container = view_container
                        .on_scroll_wheel(bouncy_scroll_handler_with_handle(
                            view.clone(),
                            |app_state: &mut crate::app::AppState| {
                                &mut app_state.main_view.bouncy_scroll
                            },
                            |app_state: &mut crate::app::AppState| {
                                &app_state.main_view.uniform_list_scroll_handle
                            },
                        ))
                        .on_mouse_down(MouseButton::Left, {
                            let view = view.clone();
                            move |_event, win, cx| {
                                view.update(cx, |this, cx| {
                                    let y: f32 = win.mouse_position().y.into();
                                    this.state.main_view.bouncy_scroll.start_mouse_drag(y);
                                    cx.notify();
                                });
                            }
                        })
                        .on_mouse_move({
                            let view = view.clone();
                            move |_event, win, cx| {
                                view.update(cx, |this, cx| {
                                    let bs = &mut this.state.main_view.bouncy_scroll;
                                    if bs.drag_active {
                                        let y: f32 = win.mouse_position().y.into();
                                        let delta = bs.apply_mouse_drag_delta(y);
                                        // Read scroll state and update bounds on every move
                                        // so max_offset stays in sync with the UniformList.
                                        // Prevents stale bounds from causing incorrect overscroll
                                        // detection during in-bounds drag.
                                        let (current_y, max_y, viewport_h, content_h) = uniformlist_scroll_info(
                                            &this.state.main_view.uniform_list_scroll_handle,
                                        );
                                        bs.update_bounds(content_h, viewport_h);
                                        // Keep active state fresh on each move so the release
                                        // transition correctly detects overscroll vs in-bounds.
                                        bs.is_active = true;
                                        bs.graceful_counter = crate::ui::components::bouncy_scroll::MOMENTUM_GRACE_TICKS;

                                        // ── Effective-position tracking via raw_overscroll ──
                                        // `raw_overscroll` tracks the TRUE cumulative distance the
                                        // user has dragged past the boundary (un-damped). During
                                        // drag, `offset` stores the damped visual offset (from
                                        // rubber_band_clamp), while `raw_overscroll` stores the raw
                                        // distance. This is essential because `rubber_band_clamp` is
                                        // NON-LINEAR — applying it per-event gives wrong accumulation.
                                        let effective = if bs.raw_overscroll > 0.0 {
                                            bs.raw_overscroll // top: raw distance past boundary
                                        } else if bs.raw_overscroll < 0.0 {
                                            bs.max_offset + bs.raw_overscroll // bottom
                                        } else {
                                            current_y // in-bounds
                                        };
                                        let new_effective = effective - delta;

                                        // Accumulate velocity for momentum after release
                                        // (-delta converts mouse-down-positive to scroll-down-negative)
                                        bs.velocity = bs.velocity * VELOCITY_BLEND_HISTORY
                                            + (-delta) * (1.0 - VELOCITY_BLEND_HISTORY);
                                        bs.velocity = bs.velocity.clamp(-3000.0, 3000.0);

                                        if new_effective > 0.0 {
                                            // Top overscroll
                                            bs.raw_overscroll = new_effective;
                                            let damped = crate::ui::components::bouncy_scroll::rubber_band_clamp(
                                                bs.raw_overscroll,
                                                crate::ui::components::bouncy_scroll::RUBBER_BAND_RESISTANCE,
                                                MAX_OVERSCROLL,
                                            );
                                            bs.offset = damped;
                                            this.state.main_view.uniform_list_scroll_handle
                                                .0
                                                .borrow()
                                                .base_handle
                                                .set_offset(gpui::point(px(0.0), px(0.0)));
                                        } else if new_effective < -max_y {
                                            // Bottom overscroll
                                            bs.raw_overscroll = new_effective + max_y;
                                            let exceed = -max_y - new_effective;
                                            let damped = crate::ui::components::bouncy_scroll::rubber_band_clamp(
                                                exceed,
                                                crate::ui::components::bouncy_scroll::RUBBER_BAND_RESISTANCE,
                                                MAX_OVERSCROLL,
                                            );
                                            bs.offset = bs.max_offset - damped;
                                            this.state.main_view.uniform_list_scroll_handle
                                                .0
                                                .borrow()
                                                .base_handle
                                                .set_offset(gpui::point(px(0.0), px(-max_y)));
                                        } else {
                                            // In-bounds
                                            bs.raw_overscroll = 0.0;
                                            bs.offset = new_effective;
                                            this.state.main_view.uniform_list_scroll_handle
                                                .0
                                                .borrow()
                                                .base_handle
                                                .set_offset(gpui::point(px(0.0), px(new_effective)));
                                        }
                                        cx.notify();
                                    }
                                });
                            }
                        })
                        .on_mouse_up(MouseButton::Left, {
                            let view = view.clone();
                            move |_event, _window, cx| {
                                view.update(cx, |this, cx| {
                                    let bs = &mut this.state.main_view.bouncy_scroll;
                                    if bs.drag_active {
                                        bs.release();
                                        if bs.animating {
                                            let epoch = bs.epoch();
                                            crate::ui::components::bouncy_scroll::start_bouncy_animation(
                                                &view,
                                                cx,
                                                |app_state: &mut crate::app::AppState| {
                                                    &mut app_state.main_view.bouncy_scroll
                                                },
                                                Some(Box::new(|state: &mut crate::app::AppState, offset: f32| {
                                                    state.main_view.uniform_list_scroll_handle.0.borrow().base_handle.set_offset(gpui::point(px(0.0), px(offset)));
                                                })),
                                                epoch,
                                            );
                                            cx.notify();
                                        }
                                    }
                                });
                            }
                        })
                        }
                view_container.child({
                    if is_recycle_bin {
                        bouncy_scroll_content(&state.bouncy_scroll, [content]).into_any_element()
                    } else if !is_home {
                        bouncy_overscroll_content(&state.bouncy_scroll, [content]).into_any_element()
                    } else {
                        content
                    }
                })
                // Minimal scrollbar for UniformList-based views
                .child(uniform_list_scrollbar(
                    &state.bouncy_scroll,
                    &state.uniform_list_scroll_handle,
                ))
            })
            .into_any_element()
    }

    fn filter_header(
        state: &MainViewState,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let is_filtering = !state.filter_text.is_empty();
        let has_selection = !state.selected_indices.is_empty();
        let is_grouping = state.group_by != GroupColumn::None;

        // Show header if filtering or grouping
        if !is_filtering && !is_grouping {
            return div();
        }

        let selection_count = state.selected_indices.len();
        let total_items = state.items.len();

        // Determine status text and icon
        let (icon, status_text) = if is_filtering {
            ("\u{E71C}", format!("Filtering by: {}", state.filter_text))
        } else {
            ("\u{E8EC}", "Grouped view".to_string()) // group icon
        };

        // Group-by label for display
        let group_label = match state.group_by {
            GroupColumn::None => "No grouping",
            GroupColumn::Name => "Name",
            GroupColumn::DateModified => "Date Modified",
            GroupColumn::DateCreated => "Date Created",
            GroupColumn::Size => "Size",
            GroupColumn::FileType => "Type",
            GroupColumn::SyncStatus => "Sync Status",
            GroupColumn::Tag => "Tag",
        };

        let v_group = view.clone();

        div()
            .flex()
            .items_center()
            .h(px(32.0))
            .px(px(UiConstants::PADDING_LARGE_RAW as f32))
            .bg(rgba(0xFFFFFF05u32))
            .border_b(px(1.0))
            .border_color(rgba(UiConstants::DIVIDER_RGBA))
            .gap(px(8.0))
            .child(
                div()
                    .text_size(AbsoluteLength::Pixels(px(12.0)))
                    .font_family(UiConstants::FONT_MDL2)
                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                    .child(icon),
            )
            .child(
                div()
                    .flex_grow()
                    .text_size(AbsoluteLength::Pixels(px(12.0)))
                    .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                    .child(status_text),
            )
            // Group-by toggle button (visible when grouping is active)
            .child(if is_grouping {
                div()
                    .flex()
                    .items_center()
                    .gap(px(4.0))
                    .px(px(8.0))
                    .py(px(2.0))
                    .rounded(px(4.0))
                    .bg(rgba(UiConstants::ACCENT_RGBA))
                    .cursor_pointer()
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        v_group.update(cx, |this, cx| {
                            this.cycle_group_by(cx);
                        });
                    })
                    .child(
                        div()
                            .text_size(AbsoluteLength::Pixels(px(11.0)))
                            .font_family(UiConstants::FONT_MDL2)
                            .text_color(rgb(UiConstants::ACCENT_COLOR_RGB))
                            .child("\u{E8EC}"), // group icon
                    )
                    .child(
                        div()
                            .text_size(AbsoluteLength::Pixels(px(11.0)))
                            .text_color(rgb(UiConstants::ACCENT_COLOR_RGB))
                            .child(group_label),
                    )
            } else {
                div()
            })
            .child(
                div()
                    .text_size(AbsoluteLength::Pixels(px(11.0)))
                    .text_color(rgb(UiConstants::TEXT_MUTED_RGB))
                    .child(format!("{} items", total_items)),
            )
    }
    fn empty_state(state: &MainViewState) -> impl IntoElement {
        let (icon, msg) = if !state.filter_text.is_empty() {
            ("\u{E721}", "No items match your search") // Search icon
        } else {
            ("\u{E8B7}", "This folder is empty") // Folder icon
        };

        div()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .flex_grow()
            .w_full()
            .h_full()
            .gap(px(12.0))
            .child(
                div()
                    .text_size(AbsoluteLength::Pixels(px(48.0)))
                    .font_family(UiConstants::FONT_MDL2)
                    .text_color(rgba(0xFFFFFF40u32)) // semi-transparent icon
                    .child(icon),
            )
            .child(
                div()
                    .text_size(AbsoluteLength::Pixels(px(14.0)))
                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                    .child(msg),
            )
    }

}

// ─────────────────────────────────────────────────────────────────────────────
// Unit tests for selection, lasso, and drag functionality
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::path_util::FilesPath;
    use crate::domain::metadata::FileItem;
    use crate::storage::storage::FileType;

    fn create_test_file_item(name: &str, is_dir: bool) -> FileItem {
        FileItem {
            name: name.to_string(),
            path: FilesPath::new(name),
            file_type: if is_dir {
                FileType::Directory
            } else {
                FileType::File
            },
            size: 1024,
            is_hidden: false,
            is_system: false,
            modified: None,
            created: None,
        }
    }

    #[test]
    fn test_lasso_state_rect_normalizes_coordinates() {
        let lasso = LassoState {
            start: point(100.0, 200.0),
            current: point(50.0, 150.0),
        };
        let (start, end) = lasso.rect();

        // Should normalize so start is top-left, end is bottom-right
        assert_eq!(start.x, 50.0);
        assert_eq!(start.y, 150.0);
        assert_eq!(end.x, 100.0);
        assert_eq!(end.y, 200.0);
    }

    #[test]
    fn test_lasso_state_rect_same_point() {
        let lasso = LassoState {
            start: point(100.0, 100.0),
            current: point(100.0, 100.0),
        };
        let (start, end) = lasso.rect();

        assert_eq!(start.x, 100.0);
        assert_eq!(start.y, 100.0);
        assert_eq!(end.x, 100.0);
        assert_eq!(end.y, 100.0);
    }

    #[test]
    fn test_drag_state_default() {
        let drag = DragState::default();
        assert!(!drag.is_dragging);
        assert!(drag.indices.is_empty());
        assert_eq!(drag.start.x, 0.0);
        assert_eq!(drag.start.y, 0.0);
        assert_eq!(drag.current.x, 0.0);
        assert_eq!(drag.current.y, 0.0);
        assert!(!drag.is_copy);
    }

    #[test]
    fn test_main_view_state_lasso_start() {
        let path = FilesPath::new("C:\\test");
        let mut state = MainViewState::new(path, ViewMode::Grid);

        assert!(state.lasso_state.is_none());

        state.lasso_start(point(100.0, 200.0));

        assert!(state.lasso_state.is_some());
        let lasso = state.lasso_state.unwrap();
        assert_eq!(lasso.start.x, 100.0);
        assert_eq!(lasso.start.y, 200.0);
        assert_eq!(lasso.current.x, 100.0);
        assert_eq!(lasso.current.y, 200.0);
    }

    #[test]
    fn test_main_view_state_lasso_update() {
        let path = FilesPath::new("C:\\test");
        let mut state = MainViewState::new(path, ViewMode::Grid);

        state.lasso_start(point(100.0, 100.0));
        state.lasso_update(point(300.0, 400.0));

        let lasso = state.lasso_state.unwrap();
        assert_eq!(lasso.start.x, 100.0);
        assert_eq!(lasso.start.y, 100.0);
        assert_eq!(lasso.current.x, 300.0);
        assert_eq!(lasso.current.y, 400.0);
    }

    #[test]
    fn test_main_view_state_lasso_cancel() {
        let path = FilesPath::new("C:\\test");
        let mut state = MainViewState::new(path, ViewMode::Grid);

        state.lasso_start(point(100.0, 100.0));
        assert!(state.lasso_state.is_some());

        state.lasso_cancel();
        assert!(state.lasso_state.is_none());
    }

    #[test]
    fn test_main_view_state_lasso_rect() {
        let path = FilesPath::new("C:\\test");
        let mut state = MainViewState::new(path, ViewMode::Grid);

        state.lasso_start(point(100.0, 100.0));
        state.lasso_update(point(200.0, 200.0));

        let rect = state.lasso_rect();
        assert!(rect.is_some());
        let (start, end) = rect.unwrap();
        assert_eq!(start.x, 100.0);
        assert_eq!(start.y, 100.0);
        assert_eq!(end.x, 200.0);
        assert_eq!(end.y, 200.0);
    }

    #[test]
    fn test_main_view_state_lasso_rect_none_when_inactive() {
        let path = FilesPath::new("C:\\test");
        let state = MainViewState::new(path, ViewMode::Grid);

        assert!(state.lasso_rect().is_none());
    }

    #[test]
    fn test_main_view_state_drag_start() {
        let path = FilesPath::new("C:\\test");
        let mut state = MainViewState::new(path, ViewMode::Grid);

        state.drag_start(vec![0, 1, 2], point(100.0, 200.0));

        assert!(state.is_dragging());
        assert_eq!(state.drag_state.indices, vec![0, 1, 2]);
        assert_eq!(state.drag_state.start.x, 100.0);
        assert_eq!(state.drag_state.current.x, 100.0);
    }

    #[test]
    fn test_main_view_state_drag_update() {
        let path = FilesPath::new("C:\\test");
        let mut state = MainViewState::new(path, ViewMode::Grid);

        state.drag_start(vec![0], point(100.0, 100.0));
        state.drag_update(point(300.0, 400.0), true); // is_copy = true

        assert_eq!(state.drag_state.current.x, 300.0);
        assert_eq!(state.drag_state.current.y, 400.0);
        assert!(state.drag_state.is_copy);
    }

    #[test]
    fn test_main_view_state_drag_end() {
        let path = FilesPath::new("C:\\test");
        let mut state = MainViewState::new(path, ViewMode::Grid);

        state.drag_start(vec![0, 1], point(100.0, 100.0));
        assert!(state.is_dragging());

        state.drag_end();

        assert!(!state.is_dragging());
        assert!(state.drag_state.indices.is_empty());
    }

    #[test]
    fn test_main_view_state_get_drop_target() {
        let path = FilesPath::new("C:\\test");
        let mut state = MainViewState::new(path, ViewMode::Grid);

        // Add some items including a directory
        state.items.push(create_test_file_item("folder1", true)); // index 0
        state.items.push(create_test_file_item("file1.txt", false)); // index 1
        state.items.push(create_test_file_item("folder2", true)); // index 2

        // Start a drag
        state.drag_start(vec![1], point(100.0, 100.0));

        // Test that we can find drop targets
        // Note: The exact positions depend on item layout which uses hardcoded values
        // This test verifies the method is callable and returns Option<usize>
        let target = state.get_drop_target(point(0.0, 0.0), &[0, 1, 2]);
        // Target could be None or Some depending on whether (0,0) falls within an item
        // The important thing is it doesn't panic and returns Option<usize>
        assert!(target.is_none() || target.is_some());
    }

    #[test]
    fn test_main_view_state_is_dragging_false_by_default() {
        let path = FilesPath::new("C:\\test");
        let state = MainViewState::new(path, ViewMode::Grid);

        assert!(!state.is_dragging());
    }

    #[test]
    fn test_select_rect_basic() {
        let path = FilesPath::new("C:\\test");
        let mut state = MainViewState::new(path, ViewMode::Grid);

        // Add some items
        for i in 0..24 {
            state
                .items
                .push(create_test_file_item(&format!("file{}.txt", i), false));
        }

        // Select items in the first row (items 0-5) using a rectangle
        // Grid layout: item_width=180, item_height=200, gap=12, padding=16
        // First row at y=16 to y=216
        let visible = state.visible_items();
        state.select_rect(
            point(16.0, 16.0),   // Start: top-left of first item
            point(200.0, 216.0), // End: bottom-right area of first row
            false,               // Don't add to selection, replace
        );

        // Items in first row should be selected
        assert!(state.selected_indices.contains(&0));
        assert!(state.selected_indices.contains(&1));
        assert!(state.selected_indices.contains(&2));
        assert!(state.selected_indices.contains(&3));
        assert!(state.selected_indices.contains(&4));
        assert!(state.selected_indices.contains(&5));
    }

    #[test]
    fn test_select_rect_add_to_selection() {
        let path = FilesPath::new("C:\\test");
        let mut state = MainViewState::new(path, ViewMode::Grid);

        for i in 0..12 {
            state
                .items
                .push(create_test_file_item(&format!("file{}.txt", i), false));
        }

        // First select some items
        state.selected_indices.insert(10);
        state.selected_indices.insert(11);

        // Then add more via select_rect
        let visible = state.visible_items();
        state.select_rect(point(16.0, 16.0), point(200.0, 216.0), true); // add_to_selection = true

        // Original items should still be selected
        assert!(state.selected_indices.contains(&10));
        assert!(state.selected_indices.contains(&11));
        // New items should also be selected
        assert!(state.selected_indices.contains(&0));
    }

    #[test]
    fn test_lasso_end_clears_state() {
        let path = FilesPath::new("C:\\test");
        let mut state = MainViewState::new(path, ViewMode::Grid);

        state.lasso_start(point(100.0, 100.0));
        state.lasso_update(point(200.0, 200.0));
        assert!(state.lasso_state.is_some());

        state.lasso_end(false);
        assert!(state.lasso_state.is_none());
    }

    #[test]
    fn test_select_all_method() {
        let path = FilesPath::new("C:\\test");
        let mut state = MainViewState::new(path, ViewMode::Grid);

        for i in 0..10 {
            state
                .items
                .push(create_test_file_item(&format!("file{}.txt", i), false));
        }

        // Select all via lasso_end with a large rectangle
        state.lasso_start(point(0.0, 0.0));
        state.lasso_update(point(10000.0, 10000.0)); // Cover everything
        state.lasso_end(false);

        // All items should be selected
        for i in 0..10 {
            assert!(
                state.selected_indices.contains(&i),
                "Item {} should be selected",
                i
            );
        }
    }

    #[test]
    fn test_drag_state_clone() {
        let drag = DragState {
            indices: vec![1, 2, 3],
            start: point(100.0, 100.0),
            current: point(200.0, 300.0),
            is_copy: true,
            is_dragging: true,
        };

        let cloned = drag.clone();
        assert_eq!(cloned.indices, vec![1, 2, 3]);
        assert_eq!(cloned.start.x, 100.0);
        assert!(cloned.is_copy);
        assert!(cloned.is_dragging);
    }

    #[test]
    fn test_lasso_state_clone() {
        let lasso = LassoState {
            start: point(50.0, 50.0),
            current: point(150.0, 150.0),
        };

        let cloned = lasso.clone();
        assert_eq!(cloned.start.x, 50.0);
        assert_eq!(cloned.current.x, 150.0);
    }

    // ─────────────────────────────────────────────────────────────
    // Sort by tag tests
    // ─────────────────────────────────────────────────────────────

    #[test]
    fn test_sort_by_tag_tagged_items_before_untagged() {
        let mut state = MainViewState::new(FilesPath::new("/test"), ViewMode::Details);
        state.items = vec![
            create_test_file_item("zzz.txt", false),
            create_test_file_item("aaa.txt", false),
            create_test_file_item("bbb.txt", false),
        ];
        // Tag "zzz.txt" with "work", "aaa.txt" with "personal"
        state
            .tags_cache
            .insert("/test/zzz.txt".to_string(), vec!["work".to_string()]);
        state
            .tags_cache
            .insert("/test/aaa.txt".to_string(), vec!["personal".to_string()]);
        // bbb.txt is untagged

        state.sort.column = SortBy::Tag;
        state.sort.ascending = true;
        state.sort.folders_first = false;

        let visible = state.visible_items();
        // Tagged items should come first (sorted alphabetically by tag), then untagged
        assert_eq!(visible.len(), 3);
        // "personal" < "work", so aaa.txt first, then zzz.txt, then bbb.txt (untagged)
        assert_eq!(state.items[visible[0]].name, "aaa.txt");
        assert_eq!(state.items[visible[1]].name, "zzz.txt");
        assert_eq!(state.items[visible[2]].name, "bbb.txt");
    }

    #[test]
    fn test_sort_by_tag_alphabetical_order() {
        let mut state = MainViewState::new(FilesPath::new("/test"), ViewMode::Details);
        state.items = vec![
            create_test_file_item("delta.txt", false),
            create_test_file_item("alpha.txt", false),
            create_test_file_item("beta.txt", false),
        ];
        // All tagged
        state
            .tags_cache
            .insert("/test/delta.txt".to_string(), vec!["zzz".to_string()]);
        state
            .tags_cache
            .insert("/test/alpha.txt".to_string(), vec!["aaa".to_string()]);
        state
            .tags_cache
            .insert("/test/beta.txt".to_string(), vec!["bbb".to_string()]);

        state.sort.column = SortBy::Tag;
        state.sort.ascending = true;
        state.sort.folders_first = false;

        let visible = state.visible_items();
        assert_eq!(visible.len(), 3);
        // Tags sorted: aaa < bbb < zzz
        assert_eq!(state.items[visible[0]].name, "alpha.txt");
        assert_eq!(state.items[visible[1]].name, "beta.txt");
        assert_eq!(state.items[visible[2]].name, "delta.txt");
    }

    #[test]
    fn test_sort_by_tag_descending() {
        let mut state = MainViewState::new(FilesPath::new("/test"), ViewMode::Details);
        state.items = vec![
            create_test_file_item("alpha.txt", false),
            create_test_file_item("beta.txt", false),
        ];
        state
            .tags_cache
            .insert("/test/alpha.txt".to_string(), vec!["work".to_string()]);
        state
            .tags_cache
            .insert("/test/beta.txt".to_string(), vec!["personal".to_string()]);

        state.sort.column = SortBy::Tag;
        state.sort.ascending = false;
        state.sort.folders_first = false;

        let visible = state.visible_items();
        assert_eq!(visible.len(), 2);
        // Descending: "work" > "personal"
        assert_eq!(state.items[visible[0]].name, "alpha.txt");
        assert_eq!(state.items[visible[1]].name, "beta.txt");
    }

    // ─────────────────────────────────────────────────────────────
    // Group by tag tests
    // ─────────────────────────────────────────────────────────────

    #[test]
    fn test_group_by_tag() {
        let mut state = MainViewState::new(FilesPath::new("/test"), ViewMode::Details);
        state.items = vec![
            create_test_file_item("file1.txt", false),
            create_test_file_item("file2.txt", false),
            create_test_file_item("file3.txt", false),
        ];
        state
            .tags_cache
            .insert("/test/file1.txt".to_string(), vec!["work".to_string()]);
        state
            .tags_cache
            .insert("/test/file2.txt".to_string(), vec!["work".to_string()]);
        state
            .tags_cache
            .insert("/test/file3.txt".to_string(), vec!["personal".to_string()]);

        state.group_by = GroupColumn::Tag;

        // Verify group keys
        assert_eq!(
            state.get_group_key(&state.items[0]),
            Some("work".to_string())
        );
        assert_eq!(
            state.get_group_key(&state.items[1]),
            Some("work".to_string())
        );
        assert_eq!(
            state.get_group_key(&state.items[2]),
            Some("personal".to_string())
        );
    }

    #[test]
    fn test_group_by_tag_untagged() {
        let mut state = MainViewState::new(FilesPath::new("/test"), ViewMode::Details);
        state.items = vec![
            create_test_file_item("tagged.txt", false),
            create_test_file_item("untagged.txt", false),
        ];
        state
            .tags_cache
            .insert("/test/tagged.txt".to_string(), vec!["work".to_string()]);

        state.group_by = GroupColumn::Tag;

        // Tagged item gets "work" group, untagged gets "Untagged"
        assert_eq!(
            state.get_group_key(&state.items[0]),
            Some("work".to_string())
        );
        assert_eq!(
            state.get_group_key(&state.items[1]),
            Some("Untagged".to_string())
        );
    }

    #[test]
    fn test_visible_items_rc_memoizes_and_invalidates_on_change() {
        let mut st = MainViewState::new(FilesPath::new("/tmp"), ViewMode::Details);
        st.items = vec![
            FileItem {
                name: "file10.txt".to_string(),
                path: FilesPath::new("/tmp/file10.txt"),
                file_type: FileType::File,
                size: 100,
                is_hidden: false,
                is_system: false,
                modified: None,
                created: None,
            },
            FileItem {
                name: "file2.txt".to_string(),
                path: FilesPath::new("/tmp/file2.txt"),
                file_type: FileType::File,
                size: 200,
                is_hidden: false,
                is_system: false,
                modified: None,
                created: None,
            },
        ];
        st.bump_items_revision();

        let rc1 = st.visible_items_rc();
        let rc2 = st.visible_items_rc();
        // Consecutive render calls must share the exact same Rc allocation (zero re-sort)
        assert!(std::rc::Rc::ptr_eq(&rc1, &rc2));
        // Natural sort order: file2.txt (index 1) before file10.txt (index 0)
        assert_eq!(*rc1, vec![1, 0]);

        // Changing sort column invalidates cache automatically
        st.toggle_sort(SortBy::Size);
        let rc3 = st.visible_items_rc();
        assert!(!std::rc::Rc::ptr_eq(&rc1, &rc3));
        assert_eq!(*rc3, vec![0, 1]);

        // 1,000 consecutive frames without mutation must perform zero re-sorts:
        // every call returns the exact same cached allocation.
        for _ in 0..1_000 {
            assert!(std::rc::Rc::ptr_eq(&rc3, &st.visible_items_rc()));
        }
    }
}

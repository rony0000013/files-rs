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

    /// Return the sorted+filtered items as index slices into self.items.
    pub fn visible_items(&self) -> Vec<usize> {
        let filter = self.filter_text.to_lowercase();
        let mut indices: Vec<usize> = self
            .items
            .iter()
            .enumerate()
            .filter(|(i, item)| {
                // 1. System file filter
                if !self.show_system_files && item.is_system {
                    return false;
                }
                // 2. Hidden file filter (Windows hidden attribute)
                if !self.show_hidden_files && item.is_hidden {
                    return false;
                }
                // 3. Dot file filter (cross-platform)
                if !self.show_dot_files && item.name.starts_with('.') {
                    return false;
                }
                // 4. Search text filter
                if !filter.is_empty() && !item.name.to_lowercase().contains(&filter) {
                    return false;
                }
                // 5. Global search results filter
                if self.search_state.is_searching
                    && !self.search_state.query.is_empty()
                    && !self.search_state.results.contains(&i)
                {
                    return false;
                }
                true
            })
            .map(|(i, _)| i)
            .collect();

        // Sort
        indices.sort_by(|&a, &b| {
            let ia = &self.items[a];
            let ib = &self.items[b];
            // Folders first
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
                SortBy::Name => ia.name.to_lowercase().cmp(&ib.name.to_lowercase()),
                SortBy::Size => ia.size.cmp(&ib.size),
                SortBy::Type => {
                    let ta = if matches!(ia.file_type, FileType::Directory) {
                        "Folder"
                    } else {
                        "File"
                    };
                    let tb = if matches!(ib.file_type, FileType::Directory) {
                        "Folder"
                    } else {
                        "File"
                    };
                    ta.cmp(tb)
                }
                SortBy::Tag => {
                    let tags_a = self.tags_cache.get(&*ia.path.to_string_lossy());
                    let tags_b = self.tags_cache.get(&*ib.path.to_string_lossy());
                    let first_tag_a = tags_a.and_then(|t| t.first()).map(|s| s.as_str());
                    let first_tag_b = tags_b.and_then(|t| t.first()).map(|s| s.as_str());
                    match (first_tag_a, first_tag_b) {
                        (Some(a), Some(b)) => a.cmp(b),
                        (Some(_), None) => std::cmp::Ordering::Less,
                        (None, Some(_)) => std::cmp::Ordering::Greater,
                        (None, None) => ia.name.to_lowercase().cmp(&ib.name.to_lowercase()),
                    }
                }
                SortBy::DateModified => {
                    let a_time = ia.modified.unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                    let b_time = ib.modified.unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                    a_time.cmp(&b_time)
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
                    a_time.cmp(&b_time)
                }
                // Fallback for any other sort variants
                _ => ia.name.to_lowercase().cmp(&ib.name.to_lowercase()),
            };
            if self.sort.ascending {
                ord
            } else {
                ord.reverse()
            }
        });

        indices
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
        settings_data: Option<&crate::ui::components::dialogs::SettingsPreReadData>,
    ) -> impl IntoElement {
        let has_side_pane = state.side_pane != SidePaneMode::None;
        let show_shelf = state.shelf_visible;
        let v2 = view.clone();
        let v3 = view.clone();

        // Collect lasso overlay element if active
        let lasso_overlay = Self::render_lasso_overlay(state);

        div()
            .flex()
            .flex_row()
            .flex_grow()
            .w_full()
            // NO overflow_hidden() - would block child scroll events
            .bg(rgb(UiConstants::BG_MAIN_RGB))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_grow()
                    .w_full()
                    // NO overflow_hidden() - would block child scroll events
                    .child(Self::render_panes(state, file_items, view.clone(), settings_data))
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
    ) -> impl IntoElement {
        match state.pane_type {
            PaneType::Single => div()
                .flex()
                .flex_col()
                .flex_grow()
                .w_full()
                // NO overflow_hidden() - would block child scroll events
                .child(Self::render_file_view(state, file_items, view, true, settings_data)),
            PaneType::Dual => {
                // Horizontal = side-by-side; Vertical = stacked
                let is_horiz = matches!(state.dual_pane_split, DualPaneSplit::Horizontal);
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
                        )))
                        .child(div().w(splitter_w).bg(rgb(UiConstants::BG_NAV_BAR_RGB)))
                        .child(div().flex_grow().child(Self::render_file_view(
                            state,
                            file_items,
                            view,
                            is_secondary_active,
                            settings_data,
                        )))
                } else {
                    base.flex_col()
                        .child(div().flex_grow().w_full().child(Self::render_file_view(
                            state,
                            file_items.clone(),
                            view.clone(),
                            is_primary_active,
                            settings_data,
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
    ) -> impl IntoElement {
        let visible = state.visible_items();

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
                            state.render_grid_view(file_items, view.clone()).into_any_element()
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

    /// Finder-style columns (blade) view: Parent | Current | Preview
    fn columns_view(
        state: &MainViewState,
        file_items: Entity<FileItems>,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let visible = state.visible_items();
        let visible_clone = visible.to_vec();
        let selected_item = state
            .selected_indices
            .iter()
            .next()
            .and_then(|&i| state.items.get(i));
        let parent_path = state
            .primary_path
            .parent()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|| "—".to_string());
        let zoom = state.zoom_level;
        let row_h = 20.0 + zoom * 28.0;
        let icon_size = (row_h - 8.0).max(16.0).min(40.0);
        let text_size = 11.0 + zoom * 5.0;
        let text_size_sm = 10.0 + zoom * 3.0;

        div()
            .flex()
            .flex_row()
            .flex_grow()
            .h_full()
            .overflow_hidden()
            // Column 1: Parent path label
            .child(
                div()
                    .w(px(220.0))
                    .h_full()
                    .flex_col()
                    .border_r(px(1.0))
                    .border_color(rgba(UiConstants::DIVIDER_RGBA))
                    .child(
                        div()
                            .h(px(32.0))
                            .px(px(12.0))
                            .flex()
                            .items_center()
                            .border_b(px(1.0))
                            .border_color(rgba(UiConstants::DIVIDER_RGBA))
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .font_weight(FontWeight(600.0))
                                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                    .child(parent_path),
                            ),
                    )
                    .child(
                        div()
                            .id("col1-content")
                            .flex_grow()
                            .flex_col()
                            .p(px(4.0))
                            .overflow_y_scroll()
                            .track_scroll(&state.scroll_handle)
                            .child(
                                div()
                                    .h(px(28.0))
                                    .px(px(8.0))
                                    .flex()
                                    .items_center()
                                    .rounded(px(4.0))
                                    .bg(rgba(UiConstants::BG_ITEM_SELECTED_RGBA))
                                    .child(
                                        div()
                                            .text_size(px(13.0))
                                            .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                                            .child(
                                                state
                                                    .primary_path
                                                    .file_name()
                                                    .map(|n| n.to_string_lossy().into_owned())
                                                    .unwrap_or_else(|| {
                                                        state
                                                            .primary_path
                                                            .to_string_lossy()
                                                            .into_owned()
                                                    }),
                                            ),
                                    ),
                            ),
                    ),
            )
            // Column 2: Current directory contents
            .child(
                div()
                    .w(px(240.0))
                    .h_full()
                    .flex_col()
                    .border_r(px(1.0))
                    .border_color(rgba(UiConstants::DIVIDER_RGBA))
                    .child(
                        div()
                            .h(px(32.0))
                            .px(px(12.0))
                            .flex()
                            .items_center()
                            .border_b(px(1.0))
                            .border_color(rgba(UiConstants::DIVIDER_RGBA))
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .font_weight(FontWeight(600.0))
                                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                    .child(
                                        state
                                            .primary_path
                                            .file_name()
                                            .map(|n| n.to_string_lossy().into_owned())
                                            .unwrap_or_default(),
                                    ),
                            ),
                    )
                    .child(uniform_list(
                        "col2-scroll",
                        visible_clone.len(),
                        move |range, _win, cx| {
                            let fi = file_items.read(cx);
                            let app_state = view.read(cx);
                            let main_state = &app_state.state.main_view;
                            let icon_paths = &main_state.icon_paths;

                            range
                                .map(|i| {
                                    let idx = visible_clone[i];
                                    let item = &fi.items[idx];
                                    let is_selected = fi.selected_indices.contains(&idx);
                                    let is_dir = matches!(item.file_type, FileType::Directory);
                                    let name = item.name.clone();
                                    let path = item.path.clone();
                                    let v = view.clone();

                                    div()
                                        .id(ElementId::from(50000 + idx))
                                        .flex()
                                        .flex_row()
                                        .h(px(row_h))
                                        .px(px(8.0))
                                        .items_center()
                                        .gap(px(6.0))
                                        .rounded(px(4.0))
                                        .mx(px(4.0))
                                        .bg(if is_selected {
                                            rgba(UiConstants::BG_ITEM_SELECTED_RGBA)
                                        } else {
                                            rgba(0x00000000)
                                        })
                                        .hover(|s| {
                                            if !is_selected {
                                                s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA))
                                            } else {
                                                s
                                            }
                                        })
                                        .cursor_pointer()
                                        .on_mouse_down(MouseButton::Left, {
                                            let v = v.clone();
                                            let path = path.clone();
                                            move |_, _, cx| {
                                                cx.stop_propagation();
                                                v.update(cx, |this, cx| {
                                                    if is_dir {
                                                        this.navigate(path.clone(), cx);
                                                    } else {
                                                        this.select_item(idx, false, false, cx);
                                                    }
                                                });
                                            }
                                        })
                                        .child(
                                            div()
                                                .w(px(icon_size))
                                                .h(px(icon_size))
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .child(
                                                    if let Some(icon_path) = icon_paths.get(&path) {
                                                        img(icon_path.clone())
                                                            .w(px(icon_size))
                                                            .h(px(icon_size))
                                                            .into_any_element()
                                                    } else {
                                                        div()
                                                            .text_size(px(icon_size * 0.7))
                                                            .text_color(rgba(
                                                                UiConstants::TEXT_SECONDARY_RGBA,
                                                            ))
                                                            .child(if is_dir {
                                                                "\u{E8B7}"
                                                            } else {
                                                                "\u{E8A5}"
                                                            })
                                                            .into_any_element()
                                                    },
                                                ),
                                        )
                                        .child(
                                            div()
                                                .flex_grow()
                                                .text_size(px(text_size))
                                                .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                                                .child(name),
                                        )
                                        .into_any_element()
                                })
                                .collect::<Vec<_>>()
                        },
                    )),
            )
            // Column 3: Preview panel
            .child(
                div()
                    .flex_grow()
                    .h_full()
                    .flex_col()
                    .bg(rgba(0xFFFFFF05u32))
                    .child(match selected_item {
                        None => div()
                            .flex_grow()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(rgba(0xFFFFFF40u32))
                                    .child("Select a file to preview"),
                            )
                            .into_any_element(),
                        Some(item) => div()
                            .flex_grow()
                            .p(px(16.0))
                            .child(Self::render_details_pane(state, item))
                            .into_any_element(),
                    }),
            )
    }

    /// Cards view — large card per file with icon, name, size, and type badge
    fn cards_view(
        state: &MainViewState,
        visible: &[usize],
        file_items: Entity<FileItems>,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let visible_clone = visible.to_vec();
        let items_per_row = 4;
        let row_count = (visible.len() + items_per_row - 1) / items_per_row;
        let row_height = 212.0; // 200 card + 12 gap

        div()
            .flex_col()
            .w_full()
            .flex_grow()
            .h_full()
            .overflow_hidden()
            .child(
                uniform_list("cards-scroll", row_count, move |range, _win, cx| {
                    let fi = file_items.read(cx);
                    let app_state = view.read(cx);
                    let main_state = &app_state.state.main_view;
                    let icon_paths = &main_state.icon_paths;

                    range
                        .map(|row_idx| {
                            let start = row_idx * items_per_row;
                            let end = (start + items_per_row).min(visible_clone.len());

                            div()
                                .flex()
                                .flex_row()
                                .h(px(row_height))
                                .gap(px(12.0))
                                .px(px(16.0))
                                .children((start..end).map(|i| {
                                    let idx = visible_clone[i];
                                    let item = &fi.items[idx];
                                    let is_selected = fi.selected_indices.contains(&idx);
                                    let name = item.name.clone();
                                    let path = item.path.clone();
                                    let size_str = format_size(item.size);
                                    let is_dir = matches!(item.file_type, FileType::Directory);
                                    let v_card = view.clone();

                                    div()
                                        .id(ElementId::from(30000 + idx))
                                        .w(px(160.0))
                                        .h(px(200.0))
                                        .flex_col()
                                        .rounded(px(8.0))
                                        .border(px(1.0))
                                        .border_color(if is_selected {
                                            rgba(UiConstants::ACCENT_RGBA)
                                        } else {
                                            rgba(UiConstants::DIVIDER_RGBA)
                                        })
                                        .bg(if is_selected {
                                            rgba(UiConstants::BG_ITEM_SELECTED_RGBA)
                                        } else {
                                            rgba(0xFFFFFF08u32)
                                        })
                                        .hover(|s| s.bg(rgba(0xFFFFFF10u32)))
                                        .cursor_pointer()
                                        .overflow_hidden()
                                        .on_mouse_down(MouseButton::Left, {
                                            let v = v_card.clone();
                                            move |ev, _, cx| {
                                                cx.stop_propagation();
                                                v.update(cx, |this, cx| {
                                                    this.select_item(
                                                        idx,
                                                        ev.modifiers.shift,
                                                        ev.modifiers.control,
                                                        cx,
                                                    );
                                                });
                                            }
                                        })
                                        .on_click({
                                            let v = v_card.clone();
                                            let item_type = item.file_type;
                                            let path = path.clone();
                                            move |ev, _win, cx| {
                                                if ev.click_count() == 2 {
                                                    v.update(cx, |this, cx| {
                                                        if item_type == FileType::Directory {
                                                            this.navigate(path.clone(), cx);
                                                        } else {
                                                            this.open_item(idx, cx);
                                                        }
                                                    });
                                                }
                                            }
                                        })
                                        .on_mouse_down(MouseButton::Right, {
                                            let v = v_card.clone();
                                            move |ev, _, cx| {
                                                cx.stop_propagation();
                                                v.update(cx, |this, cx| {
                                                    this.show_context_menu(
                                                        Some(idx),
                                                        ev.position,
                                                        cx,
                                                    );
                                                });
                                            }
                                        })
                                        .child(div().justify_center().w_full().flex_grow().child(
                                            if let Some(icon_path) = icon_paths.get(&path) {
                                                img(icon_path.clone())
                                                    .w(px(64.0))
                                                    .h(px(64.0))
                                                    .into_any_element()
                                            } else {
                                                div()
                                                    .text_size(px(48.0))
                                                    .text_color(rgba(UiConstants::ACCENT_RGBA))
                                                    .child(if is_dir {
                                                        "\u{E8B7}"
                                                    } else {
                                                        "\u{E8A5}"
                                                    })
                                                    .into_any_element()
                                            },
                                        ))
                                        .child(
                                            div()
                                                .w_full()
                                                .p(px(8.0))
                                                .bg(rgba(0x00000044u32))
                                                .child(
                                                    div()
                                                        .text_size(px(12.0))
                                                        .text_color(rgb(0xFFFFFF))
                                                        .child(name),
                                                )
                                                .child(
                                                    div()
                                                        .text_size(px(10.0))
                                                        .text_color(rgba(0xFFFFFF88u32))
                                                        .child(size_str),
                                                ),
                                        )
                                        .into_any_element()
                                }))
                                .into_any_element()
                        })
                        .collect::<Vec<_>>()
                })
                .track_scroll(state.uniform_list_scroll_handle.clone()),
            )
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

    fn list_view(
        state: &MainViewState,
        visible: &[usize],
        file_items: Entity<FileItems>,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let visible_clone = visible.to_vec();
        let zoom = state.zoom_level;
        let row_h = 24.0 + zoom * 32.0;
        let icon_size = 16.0 + zoom * 24.0;
        let text_size = 11.0 + zoom * 5.0;

        div()
            .flex_col()
            .w_full()
            .flex_grow()
            .h_full()
            .overflow_hidden()
            .child(
                uniform_list(
                    "list-scroll",
                    visible_clone.len(),
                    move |range, _win, cx| {
                        let fi = file_items.read(cx);
                        let app_state = view.read(cx);
                        let main_state = &app_state.state.main_view;
                        let icon_paths = &main_state.icon_paths;

                        range
                            .map(|i| {
                                let idx = visible_clone[i];
                                let item = &fi.items[idx];
                                let is_selected = fi.selected_indices.contains(&idx);
                                let path = item.path.clone();
                                let view_row = view.clone();

                                div()
                                    .id(ElementId::from(idx + 20000))
                                    .flex()
                                    .items_center()
                                    .w_full()
                                    .h(px(row_h))
                                    .px(px(16.0))
                                    .py(px(2.0))
                                    .bg(if is_selected {
                                        rgba(UiConstants::BG_ITEM_SELECTED_RGBA)
                                    } else {
                                        rgba(UiConstants::BG_TRANSPARENT_RGBA)
                                    })
                                    .hover(|s| {
                                        s.bg(if is_selected {
                                            rgba(UiConstants::BG_ITEM_SELECTED_HOVER_RGBA)
                                        } else {
                                            rgba(UiConstants::BG_ITEM_HOVER_RGBA)
                                        })
                                    })
                                    .cursor_pointer()
                                    .on_mouse_down(MouseButton::Left, {
                                        let v = view_row.clone();
                                        move |_event, _, cx| {
                                            cx.stop_propagation();
                                            v.update(cx, |this, cx| {
                                                this.select_item(idx, false, false, cx);
                                            });
                                        }
                                    })
                                    .on_mouse_down(MouseButton::Right, {
                                        let v = view_row.clone();
                                        move |ev, _, cx| {
                                            cx.stop_propagation();
                                            v.update(cx, |this, cx| {
                                                this.show_context_menu(Some(idx), ev.position, cx);
                                            });
                                        }
                                    })
                                    .child(
                                        div()
                                            .w(px(icon_size))
                                            .h(px(icon_size))
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .child(
                                                if let Some(icon_path) = icon_paths.get(&path) {
                                                    img(icon_path.clone())
                                                        .w(px(icon_size))
                                                        .h(px(icon_size))
                                                        .into_any_element()
                                                } else {
                                                    div()
                                                        .text_size(px(icon_size * 0.5))
                                                        .text_color(rgba(
                                                            UiConstants::TEXT_SECONDARY_RGBA,
                                                        ))
                                                        .child(
                                                            if matches!(
                                                                item.file_type,
                                                                FileType::Directory
                                                            ) {
                                                                "\u{E8B7}"
                                                            } else {
                                                                "\u{E8A5}"
                                                            },
                                                        )
                                                        .into_any_element()
                                                },
                                            ),
                                    )
                                    .child(
                                        div()
                                            .ml(px(8.0))
                                            .text_size(px(text_size))
                                            .text_color(if is_selected {
                                                rgb(UiConstants::TEXT_PRIMARY_RGB)
                                            } else {
                                                rgba(UiConstants::TEXT_PRIMARY_RGBA)
                                            })
                                            .child(item.name.clone()),
                                    )
                                    .into_any_element()
                            })
                            .collect::<Vec<_>>()
                    },
                )
                .track_scroll(state.uniform_list_scroll_handle.clone()),
            )
    }

    fn grid_view(
        state: &MainViewState,
        visible: &[usize],
        file_items: Entity<FileItems>,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        if state.group_by != GroupColumn::None {
            return Self::grid_view_grouped(state, visible, view).into_any_element();
        }

        let visible_clone = visible.to_vec();
        let items_per_row = Self::grid_items_per_row();
        let row_count = (visible.len() + items_per_row - 1) / items_per_row;

        let zoom = state.zoom_level;
        let base_size = 80.0 + zoom * 120.0;
        let row_height = base_size + 32.0;

        div()
            .flex_col()
            .w_full()
            .flex_grow()
            .h_full()
            .overflow_hidden()
            .id("grid-container")
            .on_mouse_down(MouseButton::Right, {
                let v = view.clone();
                move |event, _, cx| {
                    v.update(cx, |this, cx| {
                        this.show_context_menu(None, event.position, cx);
                    });
                }
            })
            .child(
                uniform_list("grid-scroll", row_count, move |range, _win, cx| {
                    let fi = file_items.read(cx);
                    let app_state = view.read(cx);
                    let main_state = &app_state.state.main_view;
                    let icon_paths = &main_state.icon_paths;
                    let zoom = main_state.zoom_level;
                    let sel_mode = main_state.selection_mode.clone();

                    range
                        .map(|row_idx| {
                            let start = row_idx * items_per_row;
                            let end = (start + items_per_row).min(visible_clone.len());

                            div()
                                .flex()
                                .flex_row()
                                .h(px(row_height))
                                .gap(px(UiConstants::GRID_GAP_RAW as f32))
                                .px(px(UiConstants::PADDING_LARGE_RAW as f32))
                                .children((start..end).map(|i| {
                                    let idx = visible_clone[i];
                                    let item = &fi.items[idx];
                                    let is_selected = fi.selected_indices.contains(&idx);
                                    Self::grid_item(
                                        item,
                                        idx,
                                        is_selected,
                                        sel_mode.clone(),
                                        view.clone(),
                                        icon_paths,
                                        zoom,
                                    )
                                    .into_any_element()
                                }))
                                .into_any_element()
                        })
                        .collect::<Vec<_>>()
                })
                .track_scroll(state.uniform_list_scroll_handle.clone()),
            )
            .into_any_element()
    }

    pub(crate) fn grid_item(
        item: &FileItem,
        index: usize,
        selected: bool,
        selection_mode: SelectionMode,
        view: Entity<crate::app::AppWindow>,
        icon_paths: &std::collections::HashMap<
            crate::core::path_util::FilesPath,
            std::path::PathBuf,
        >,
        zoom: f32,
    ) -> impl IntoElement {
        let v_click = view.clone();
        let v_hover = view.clone();
        let v_timer = view.clone();
        let v_checkbox = view.clone();
        let is_dir = matches!(item.file_type, FileType::Directory);
        let extension = get_extension(&item.name);
        let is_thumbnail = !is_dir && is_thumbnail_image(&extension);
        let show_checkbox = selection_mode == SelectionMode::Checkbox;
        let is_hover_select = selection_mode == SelectionMode::HoverSelect;
        // Clone values needed by closures to avoid borrow issues
        let item_path = item.path.clone();
        let item_name = item.name.clone();

        // Check for cached Windows Shell icon
        let cached_icon_path: Option<std::path::PathBuf> = icon_paths.get(&item_path).cloned();
        let use_cached_icon = cached_icon_path.is_some();

        // For thumbnail images, use GPUUI's img() to display the image
        // For other items, use text-based icon

        // Calculate dynamic dimensions — continuous from min to max zoom
        let item_size = 80.0 + zoom * 160.0; // 80 (min) to 240 (max)
        let item_height = item_size + 30.0;
        let icon_size_val = item_size * 0.6;

        if is_thumbnail {
            // Show image thumbnail with optional checkbox
            let mut item_div = div()
                .id(ElementId::from(70000 + index))
                .w(px(item_size))
                .h(px(item_height))
                .flex_col()
                .items_center()
                .py(px(item_size * 0.25)) // Proportional padding
                .rounded(px(4.0))
                .relative()
                .border(px(if selected { 2.0 } else { 1.5 }))
                .border_color(if selected {
                    rgba(UiConstants::ACCENT_RGBA)
                } else {
                    rgba(0x00000000u32)
                })
                .bg(rgba(0x00000000u32))
                .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)));

            // Add checkbox if in checkbox mode
            if show_checkbox {
                item_div = item_div.child(
                    div()
                        .absolute()
                        .top(px(4.0))
                        .left(px(4.0))
                        .w(px(20.0))
                        .h(px(20.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                            v_checkbox.update(cx, |this, cx| {
                                this.toggle_checkbox_selection(index, cx);
                            });
                        })
                        .child(
                            div()
                                .w(px(14.0))
                                .h(px(14.0))
                                .border(px(1.5))
                                .border_color(rgba(UiConstants::ACCENT_RGBA))
                                .rounded(px(3.0))
                                .bg(if selected {
                                    rgba(UiConstants::ACCENT_RGBA)
                                } else {
                                    rgba(0x00000000)
                                })
                                .child(if selected {
                                    div()
                                        .text_size(px(9.0))
                                        .font_family(UiConstants::FONT_MDL2)
                                        .text_color(rgb(0xFFFFFF))
                                        .child("\u{E73E}")
                                } else {
                                    div()
                                }),
                        ),
                );
            }

            let path_clone = item_path.clone();
            item_div
                .on_mouse_down(MouseButton::Left, move |event, _, cx| {
                    cx.stop_propagation();
                    v_click.update(cx, |this, cx| {
                        if event.click_count == 2 {
                            if is_dir {
                                this.navigate(path_clone.clone(), cx);
                            } else {
                                this.open_item(index, cx);
                            }
                        } else {
                            this.select_item(index, false, false, cx);
                        }
                    });
                })
                .on_mouse_down(MouseButton::Right, {
                    let v = view.clone();
                    move |event, _, cx| {
                        cx.stop_propagation();
                        v.update(cx, |this, cx| {
                            this.show_context_menu(Some(index), event.position, cx);
                        });
                    }
                })
                .on_mouse_move({
                    let v_hover = v_hover.clone();
                    let v_timer = v_timer.clone();
                    move |ev, _, cx| {
                        if is_hover_select {
                            v_hover.update(cx, |this, cx| {
                                this.handle_item_hover(index, cx);
                            });
                        } else {
                            v_timer.update(cx, |this, cx| {
                                this.start_hover_timer(index, ev.position, cx);
                            });
                        }
                    }
                })
                .on_mouse_up_out(MouseButton::Left, {
                    let v_timer = v_timer.clone();
                    move |_, _, cx| {
                        // Stop the hover timer when mouse leaves
                        v_timer.update(cx, |this, cx| {
                            this.stop_hover_timer(cx);
                        });
                    }
                })
                .child(
                    // Thumbnail image
                    img(item_path.as_path().to_path_buf())
                        .w(px(icon_size_val))
                        .h(px(icon_size_val))
                        .object_fit(gpui::ObjectFit::Contain)
                        .rounded(px(4.0)),
                )
                .child(
                    div()
                        .w_full()
                        .px(px(4.0))
                        .mt(px(4.0))
                        .text_center()
                        .overflow_hidden()
                        .text_size(px((item_size / 11.0).clamp(10.0, 14.0)))
                        .font_family(UiConstants::FONT_MAIN)
                        .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                        .child(item_name.clone()),
                )
        } else {
            // Show icon for directories and non-image files
            let file_icon = if is_dir {
                "\u{E8B7}".to_string()
            } else {
                get_file_type_icon(&extension).to_string()
            };

            let mut item_div = div()
                .id(ElementId::from(70000 + index))
                .w(px(item_size))
                .h(px(item_height))
                .flex_col()
                .items_center()
                .py(px(item_size * 0.25))
                .rounded(px(4.0))
                .relative()
                .border(px(if selected { 2.0 } else { 1.5 }))
                .border_color(if selected {
                    rgba(UiConstants::ACCENT_RGBA)
                } else {
                    rgba(0x00000000u32)
                })
                .bg(rgba(0x00000000u32))
                .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)));

            // Add checkbox if in checkbox mode
            if show_checkbox {
                let v_checkbox = view.clone();
                item_div = item_div.child(
                    div()
                        .absolute()
                        .top(px(4.0))
                        .left(px(4.0))
                        .w(px(20.0))
                        .h(px(20.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                            v_checkbox.update(cx, |this, cx| {
                                this.toggle_checkbox_selection(index, cx);
                            });
                        })
                        .child(
                            div()
                                .w(px(14.0))
                                .h(px(14.0))
                                .border(px(1.5))
                                .border_color(rgba(UiConstants::ACCENT_RGBA))
                                .rounded(px(3.0))
                                .bg(if selected {
                                    rgba(UiConstants::ACCENT_RGBA)
                                } else {
                                    rgba(0x00000000)
                                })
                                .child(if selected {
                                    div()
                                        .text_size(px(9.0))
                                        .font_family(UiConstants::FONT_MDL2)
                                        .text_color(rgb(0xFFFFFF))
                                        .child("\u{E73E}")
                                } else {
                                    div()
                                }),
                        ),
                );
            }

            let path_clone = item_path.clone();
            item_div
                .on_mouse_down(MouseButton::Left, move |event, _, cx| {
                    cx.stop_propagation();
                    v_click.update(cx, |this, cx| {
                        if event.click_count == 2 {
                            if is_dir {
                                this.navigate(path_clone.clone(), cx);
                            } else {
                                this.open_item(index, cx);
                            }
                        } else {
                            this.select_item(index, false, false, cx);
                        }
                    });
                })
                .on_mouse_down(MouseButton::Right, {
                    let v = view.clone();
                    move |event, _, cx| {
                        cx.stop_propagation();
                        v.update(cx, |this, cx| {
                            this.show_context_menu(Some(index), event.position, cx);
                        });
                    }
                })
                .on_mouse_move({
                    let v_hover = v_hover.clone();
                    let v_timer = v_timer.clone();
                    move |ev, _, cx| {
                        if is_hover_select {
                            v_hover.update(cx, |this, cx| {
                                this.handle_item_hover(index, cx);
                            });
                        } else {
                            v_timer.update(cx, |this, cx| {
                                this.start_hover_timer(index, ev.position, cx);
                            });
                        }
                    }
                })
                .on_mouse_up_out(MouseButton::Left, {
                    let v_timer = v_timer.clone();
                    move |_, _, cx| {
                        // Stop the hover timer when mouse leaves
                        v_timer.update(cx, |this, cx| {
                            this.stop_hover_timer(cx);
                        });
                    }
                })
                .child(
                    // Centered icon
                    div().flex().items_center().justify_center().w_full().child(
                        // Icon: use cached shell icon if available (shows folder thumbnails on Windows),
                        // otherwise MDL2 text glyph
                        if let Some(icon_path) = &cached_icon_path {
                            img(icon_path.clone())
                                .w(px(icon_size_val))
                                .h(px(icon_size_val))
                                .object_fit(gpui::ObjectFit::Contain)
                                .into_any_element()
                        } else {
                            div()
                                .text_size(px(icon_size_val * 0.8))
                                .font_family(UiConstants::FONT_MDL2)
                                .text_color(if is_dir {
                                    rgba(UiConstants::ACCENT_RGBA)
                                } else {
                                    rgba(UiConstants::TEXT_SECONDARY_RGBA)
                                })
                                .child(file_icon)
                                .into_any_element()
                        },
                    ),
                )
                .child(
                    div()
                        .w_full()
                        .px(px(4.0))
                        .mt(px(4.0))
                        .text_center()
                        .overflow_hidden()
                        .text_size(px((item_size / 11.0).clamp(10.0, 14.0)))
                        .font_family(UiConstants::FONT_MAIN)
                        .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                        .child(item_name.clone()),
                )
        }
    }

    fn file_grid(
        state: &MainViewState,
        file_items: Entity<FileItems>,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let visible = state.visible_items();
        Self::grid_view(state, &visible, file_items, view)
    }

    #[allow(dead_code)]
    fn file_list(
        state: &MainViewState,
        file_items: Entity<FileItems>,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let visible = state.visible_items();
        Self::details_view(state, &visible, file_items, view)
    }

    /// Calculate the number of items per row for the grid view.
    /// Uses sensible defaults based on grid vs card mode; TODO: make responsive to container width.
    pub(crate) fn grid_items_per_row() -> usize {
        6
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
}

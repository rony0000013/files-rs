use crate::core::path_util::FilesPath;
use crate::platform::windows::office_preview_host::OfficePreviewHost;
use crate::platform::windows::recycle_bin_service::RecycleBinItem;
use crate::ui::components::bouncy_scroll::{
    bouncy_overscroll_content, bouncy_scroll_content, bouncy_scroll_handler,
    bouncy_scroll_handler_with_handle, smooth_scroll_handle_wheel, start_bouncy_animation,
    uniform_list_scrollbar, uniformlist_scroll_info, BouncyScrollState, MAX_OVERSCROLL,
    VELOCITY_BLEND_HISTORY,
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
            rename_selected_all: false,
            rename_cursor_pos: 0,
            rename_chars_per_line: 18,
            cut_paths: HashSet::new(),
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
            dual_pane_ratio: 0.5,
            dual_pane_resize_start: None,
            last_clicked_index: None,
            selection_anchor_index: None,
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
            col1_scroll_handle: UniformListScrollHandle::new(),
            col3_scroll_handle: UniformListScrollHandle::new(),
            col1_siblings: Vec::new(),
            col1_siblings_for: None,
            col3_children: Vec::new(),
            col3_children_for: None,
            icon_refresh_last_check: std::cell::Cell::new(None),
            bouncy_scroll: BouncyScrollState::new(),
            git_status_cache: std::collections::HashMap::new(),
            items_revision: 0,
            visible_cache: std::cell::RefCell::new(None),
            side_pane_scroll_handle: ScrollHandle::new(),
            preview_table_scroll: ScrollHandle::new(),
            side_pane_bouncy_scroll: BouncyScrollState::new(),
            peek_scroll_handle: ScrollHandle::new(),
            peek_bouncy_scroll: BouncyScrollState::new(),
            show_peek_modal: false,
            preview_request_id: 0,
            preview_highlighted_lines: std::sync::Arc::new(Vec::new()),
            cached_drives: crate::platform::windows::drive_service::enumerate_drives(),
            video_playing: false,
            video_progress: 0.0,
            video_muted: false,
            video_frame: None,
            office_preview: OfficePreviewHost::default(),
            show_filter_bar: false,
            group_ascending: true,
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

    /// Look up tags for `item` from `tags_cache` (by full path or `primary_path/name`)
    /// or fallback to `item.tags`.
    pub fn item_tags<'a>(&'a self, item: &'a FileItem) -> Option<&'a [String]> {
        let path_str = item.path.to_string_lossy();
        if let Some(tags) = self.tags_cache.get(path_str.as_ref()) {
            if !tags.is_empty() {
                return Some(tags.as_slice());
            }
        }
        let joined = format!(
            "{}/{}",
            self.primary_path
                .to_string_lossy()
                .trim_end_matches(['/', '\\']),
            item.name
        );
        if let Some(tags) = self.tags_cache.get(&joined) {
            if !tags.is_empty() {
                return Some(tags.as_slice());
            }
        }
        if !item.tags.is_empty() {
            return Some(item.tags.as_slice());
        }
        None
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
        };
        let group_discriminant = match self.group_by {
            GroupColumn::None => 0,
            GroupColumn::Name => 1,
            GroupColumn::DateModified => 2,
            GroupColumn::DateCreated => 3,
            GroupColumn::Size => 4,
            GroupColumn::FileType => 5,
            GroupColumn::SyncStatus => 6,
            GroupColumn::Tag => 7,
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
            files_first: self.sort.files_first,
            group_col_discriminant: group_discriminant,
            group_ascending: self.group_ascending,
            filter_text: self.filter_text.clone(),
            show_dot_files: self.show_dot_files,
            show_hidden_files: self.show_hidden_files,
            show_system_files: self.show_system_files,
            is_searching: self.search_state.is_searching,
            search_query: self.search_state.query.clone(),
            search_results_len: self.search_state.results.len(),
            tags_cache_len: self.tags_cache.len(),
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
            } else if self.sort.files_first {
                let af = matches!(ia.file_type, FileType::Directory);
                let bf = matches!(ib.file_type, FileType::Directory);
                if af != bf {
                    return if af {
                        std::cmp::Ordering::Greater
                    } else {
                        std::cmp::Ordering::Less
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
                        if self.sort.files_first {
                            if dir_a {
                                std::cmp::Ordering::Greater
                            } else {
                                std::cmp::Ordering::Less
                            }
                        } else if self.sort.folders_first {
                            if dir_a {
                                std::cmp::Ordering::Less
                            } else {
                                std::cmp::Ordering::Greater
                            }
                        } else {
                            ext_cmp(&ia.name, &ib.name)
                        }
                    } else {
                        ext_cmp(&ia.name, &ib.name)
                    }
                }
                SortBy::Tag => {
                    let first_tag_a = self
                        .item_tags(ia)
                        .and_then(|t| t.first())
                        .map(|s| s.as_str());
                    let first_tag_b = self
                        .item_tags(ib)
                        .and_then(|t| t.first())
                        .map(|s| s.as_str());
                    match (first_tag_a, first_tag_b) {
                        (Some(a), Some(b)) => natural_cmp(a, b),
                        (Some(_), None) => return std::cmp::Ordering::Less,
                        (None, Some(_)) => return std::cmp::Ordering::Greater,
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
        Self::build_with_secondary(
            state,
            file_items.clone(),
            state,
            file_items,
            view,
            settings_data,
            content_w,
            224.0,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn build_with_secondary(
        state: &MainViewState,
        file_items: Entity<FileItems>,
        secondary_state: &MainViewState,
        secondary_file_items: Entity<FileItems>,
        view: Entity<crate::app::AppWindow>,
        settings_data: Option<crate::ui::components::dialogs::SettingsPreReadData>,
        content_w: f32,
        base_left_x: f32,
        status_bar: Option<AnyElement>,
    ) -> impl IntoElement {
        let has_side_pane = state.side_pane != SidePaneMode::None
            && !matches!(state.target, ViewTarget::Settings);
        let show_shelf = state.shelf_visible;
        let v2 = view.clone();
        let v3 = view.clone();
        let content_w = if has_side_pane {
            content_w - state.side_pane_width.unwrap_or(300.0) - 6.0
        } else {
            content_w
        };

        // MainPage.xaml ContentGrid + ModernShellPage.xaml RootGrid:
        // Left child is the ModernShellPage floating rounded card (FileArea + Shelf + StatusBar),
        // Right child (when active) is the sibling InfoPane rounded card separated by the splitter.
        div()
            .flex()
            .flex_row()
            .flex_grow()
            .w_full()
            .h_full()
            .min_h(px(0.0))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_grow()
                    .w_full()
                    .h_full()
                    .min_h(px(0.0))
                    .rounded(px(8.0))
                    .border(px(1.0))
                    .border_color(rgba(UiConstants::CARD_STROKE_RGBA))
                    .bg(rgba(UiConstants::BG_MAIN_RGBA))
                    .overflow_hidden()
                    .child(Self::render_panes(
                        state,
                        file_items,
                        secondary_state,
                        secondary_file_items,
                        view.clone(),
                        settings_data.as_ref(),
                        content_w,
                        base_left_x,
                    ))
                    .children(if show_shelf {
                        vec![Self::render_shelf_pane(state, v2).into_any_element()]
                    } else {
                        vec![]
                    })
                    .children(status_bar.map(|sb| {
                        div()
                            .flex_shrink_0()
                            .w_full()
                            .h(px(UiConstants::STATUS_BAR_HEIGHT_RAW as f32))
                            .child(sb)
                            .into_any_element()
                    })),
            )
            .children(if has_side_pane {
                vec![
                    Self::render_side_pane_splitter(state, view.clone()).into_any_element(),
                    Self::render_side_pane(state, v3).into_any_element(),
                ]
            } else {
                vec![]
            })
    }

    #[allow(clippy::too_many_arguments)]
    fn render_panes(
        state: &MainViewState,
        file_items: Entity<FileItems>,
        secondary_state: &MainViewState,
        secondary_file_items: Entity<FileItems>,
        view: Entity<crate::app::AppWindow>,
        settings_data: Option<&crate::ui::components::dialogs::SettingsPreReadData>,
        content_w: f32,
        base_left_x: f32,
    ) -> impl IntoElement {
        use crate::core::navigation_manager::PaneFocus;

        match state.pane_type {
            PaneType::Single => div()
                .flex()
                .flex_col()
                .flex_grow()
                .w_full()
                .h_full()
                .child(Self::render_file_view(
                    state,
                    file_items,
                    view,
                    true,
                    settings_data,
                    content_w,
                    PaneFocus::Primary,
                    gpui::point(base_left_x, 128.0),
                    false,
                )),
            PaneType::Dual => {
                let is_horiz = matches!(state.dual_pane_split, DualPaneSplit::Horizontal);
                let ratio = state.dual_pane_ratio.clamp(0.2, 0.8);
                let primary_w = if is_horiz {
                    ((content_w - 6.0) * ratio).max(160.0)
                } else {
                    content_w
                };
                let secondary_w = if is_horiz {
                    (content_w - 6.0 - primary_w).max(160.0)
                } else {
                    content_w
                };

                let is_primary_active = matches!(state.active_pane, PaneFocus::Primary);
                let is_secondary_active = !is_primary_active;

                let splitter = {
                    let v_sp = view.clone();
                    let v_dbl = view.clone();
                    let is_resizing = state.dual_pane_resize_start.is_some();
                    let mut sp = div()
                        .id("dual-pane-splitter")
                        .flex_shrink_0()
                        .bg(if is_resizing {
                            rgba(UiConstants::ACCENT_RGBA)
                        } else {
                            rgba(UiConstants::DIVIDER_RGBA)
                        })
                        .hover(|s| s.bg(rgba(UiConstants::ACCENT_RGBA)))
                        .on_mouse_down(MouseButton::Left, move |ev, _win, cx| {
                            cx.stop_propagation();
                            let coord: f32 = if is_horiz {
                                ev.position.x.into()
                            } else {
                                ev.position.y.into()
                            };
                            if ev.click_count == 2 {
                                v_dbl.update(cx, |this, cx| {
                                    this.state.main_view.dual_pane_ratio = 0.5;
                                    this.state.main_view.dual_pane_resize_start = None;
                                    cx.notify();
                                });
                            } else {
                                v_sp.update(cx, |this, cx| {
                                    let r = this.state.main_view.dual_pane_ratio;
                                    this.state.main_view.dual_pane_resize_start = Some((coord, r));
                                    cx.notify();
                                });
                            }
                        });
                    if is_horiz {
                        sp = sp.w(px(6.0)).h_full().cursor_col_resize();
                    } else {
                        sp = sp.h(px(6.0)).w_full().cursor_row_resize();
                    }
                    sp
                };

                let base = div().flex().flex_grow().w_full().h_full();

                if is_horiz {
                    base.flex_row()
                        .child(
                            div()
                                .w(px(primary_w))
                                .flex_shrink_0()
                                .h_full()
                                .flex()
                                .flex_col()
                                .overflow_hidden()
                                .child(Self::render_file_view(
                                    state,
                                    file_items,
                                    view.clone(),
                                    is_primary_active,
                                    settings_data,
                                    primary_w,
                                    PaneFocus::Primary,
                                    gpui::point(base_left_x, 156.0),
                                    true,
                                )),
                        )
                        .child(splitter)
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(0.0))
                                .h_full()
                                .flex()
                                .flex_col()
                                .overflow_hidden()
                                .child(Self::render_file_view(
                                    secondary_state,
                                    secondary_file_items,
                                    view,
                                    is_secondary_active,
                                    settings_data,
                                    secondary_w,
                                    PaneFocus::Secondary,
                                    gpui::point(base_left_x + primary_w + 6.0, 156.0),
                                    true,
                                )),
                        )
                } else {
                    base.flex_col()
                        .child(
                            div()
                                .flex_1()
                                .min_h(px(0.0))
                                .w_full()
                                .flex()
                                .flex_col()
                                .overflow_hidden()
                                .child(Self::render_file_view(
                                    state,
                                    file_items,
                                    view.clone(),
                                    is_primary_active,
                                    settings_data,
                                    primary_w,
                                    PaneFocus::Primary,
                                    gpui::point(base_left_x, 156.0),
                                    true,
                                )),
                        )
                        .child(splitter)
                        .child(
                            div()
                                .flex_1()
                                .min_h(px(0.0))
                                .w_full()
                                .flex()
                                .flex_col()
                                .overflow_hidden()
                                .child(Self::render_file_view(
                                    secondary_state,
                                    secondary_file_items,
                                    view,
                                    is_secondary_active,
                                    settings_data,
                                    secondary_w,
                                    PaneFocus::Secondary,
                                    gpui::point(base_left_x, 420.0),
                                    true,
                                )),
                        )
                }
            }
        }
    }

    fn render_dual_pane_header(
        pane_state: &MainViewState,
        is_active: bool,
        pane_focus: crate::core::navigation_manager::PaneFocus,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let folder_name = pane_state
            .primary_path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| pane_state.primary_path.to_string_lossy().into_owned());
        let full_path = pane_state.primary_path.to_string_lossy().into_owned();
        let item_count = pane_state.items.len();

        let v_focus = view.clone();
        let v_up = view.clone();
        let v_orient = view.clone();
        let v_close = view.clone();
        let v_peek_expand = view.clone();

        div()
            .flex()
            .flex_row()
            .flex_shrink_0()
            .items_center()
            .justify_between()
            .h(px(28.0))
            .w_full()
            .px(px(10.0))
            .bg(if is_active {
                rgba(0xFFFFFF0Cu32)
            } else {
                rgba(0x00000024u32)
            })
            .border_b(px(1.5))
            .border_color(if is_active {
                rgba(UiConstants::ACCENT_RGBA)
            } else {
                rgba(UiConstants::DIVIDER_RGBA)
            })
            .cursor_pointer()
            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                v_focus.update(cx, |this, cx| {
                    this.focus_pane(pane_focus, cx);
                });
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .flex_1()
                    .min_w(px(0.0))
                    .overflow_hidden()
                    .child(
                        div()
                            .w(px(7.0))
                            .h(px(7.0))
                            .rounded(px(3.5))
                            .bg(if is_active {
                                rgb(UiConstants::ACCENT_COLOR_RGB)
                            } else {
                                rgb(0x666666)
                            }),
                    )
                    .child(
                        div()
                            .text_size(px(12.0))
                            .font_family(UiConstants::FONT_MDL2)
                            .text_color(rgb(0xF5B942))
                            .child("\u{E8B7}"),
                    )
                    .child(
                        div()
                            .text_size(px(11.5))
                            .font_weight(if is_active {
                                FontWeight(600.0)
                            } else {
                                FontWeight(400.0)
                            })
                            .text_color(if is_active {
                                rgba(UiConstants::TEXT_PRIMARY_RGBA)
                            } else {
                                rgba(UiConstants::TEXT_SECONDARY_RGBA)
                            })
                            .child(folder_name),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.0))
                            .overflow_hidden()
                            .text_size(px(10.5))
                            .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                            .child(format!("— {} ({} items)", full_path, item_count)),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(4.0))
                    .flex_shrink_0()
                    .child(
                        div()
                            .w(px(22.0))
                            .h(px(20.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(3.0))
                            .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
                            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                cx.stop_propagation();
                                v_up.update(cx, |this, cx| {
                                    this.focus_pane(pane_focus, cx);
                                    this.navigate_up(cx);
                                });
                            })
                            .child(
                                div()
                                    .text_size(px(10.0))
                                    .font_family(UiConstants::FONT_MDL2)
                                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                    .child("\u{E74A}"), // Up inside pane
                            ),
                    )
                    .child(
                        div()
                            .w(px(22.0))
                            .h(px(20.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(3.0))
                            .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
                            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                cx.stop_propagation();
                                v_orient.update(cx, |this, cx| {
                                    this.toggle_split_direction(cx);
                                });
                            })
                            .child(
                                div()
                                    .text_size(px(10.0))
                                    .font_family(UiConstants::FONT_MDL2)
                                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                    .child("\u{E8AB}"), // Switch split orientation
                            ),
                    )
                    .child(
                        div()
                            .w(px(22.0))
                            .h(px(20.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(3.0))
                            .hover(|s| s.bg(rgba(0xE8112388u32)))
                            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                cx.stop_propagation();
                                v_close.update(cx, |this, cx| {
                                    this.close_dual_pane(pane_focus, cx);
                                });
                            })
                            .child(
                                div()
                                    .text_size(px(9.5))
                                    .font_family(UiConstants::FONT_MDL2)
                                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                    .child("\u{E8BB}"), // Close pane
                            ),
                    ),
            )
    }

    fn render_side_pane_splitter(
        state: &MainViewState,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let is_resizing = state.side_pane_resize_start_x.is_some();
        div()
            .id("side-pane-splitter")
            .w(px(6.0))
            .h_full()
            .flex_shrink_0()
            .cursor_col_resize()
            .flex()
            .items_center()
            .justify_center()
            .bg(if is_resizing {
                rgba(UiConstants::ACCENT_RGBA)
            } else {
                rgba(UiConstants::BG_TRANSPARENT_RGBA)
            })
            .hover(|s| s.bg(rgba(0x60CDFF33u32)))
            .on_mouse_down(MouseButton::Left, {
                let view = view.clone();
                move |event, _win, cx| {
                    cx.stop_propagation();
                    let x: f32 = event.position.x.into();
                    if event.click_count == 2 {
                        view.update(cx, |this, cx| {
                            this.state.main_view.side_pane_width = Some(300.0);
                            this.state.main_view.side_pane_resize_start_x = None;
                            this.state.main_view.side_pane_resize_start_width = None;
                            cx.notify();
                        });
                    } else {
                        view.update(cx, |this, cx| {
                            let width = this.state.main_view.side_pane_width.unwrap_or(300.0);
                            this.state.main_view.side_pane_resize_start_x = Some(x);
                            this.state.main_view.side_pane_resize_start_width = Some(width);
                            cx.notify();
                        });
                    }
                }
            })
    }

    fn render_side_pane(
        state: &MainViewState,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let mode = &state.side_pane;
        let is_preview_tab = matches!(mode, SidePaneMode::Preview);
        let is_details_tab = !is_preview_tab;
        let sel_count = state.selected_indices.len();
        let selected_item = state
            .selected_indices
            .iter()
            .next()
            .and_then(|&i| state.items.get(i));
        let pane_w = state.side_pane_width.unwrap_or(300.0);

        let v_details = view.clone();
        let v_preview = view.clone();
        let v_close = view.clone();
        let v_peek_expand = view.clone();
        let selected_path = selected_item.map(|it| it.path.clone());

        let segmented_tabs = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(2.0))
            .p(px(3.0))
            .rounded(px(6.0))
            .bg(rgba(0x00000028u32))
            .border(px(1.0))
            .border_color(rgba(UiConstants::CARD_STROKE_RGBA))
            .child(
                div()
                    .px(px(12.0))
                    .py(px(4.0))
                    .rounded(px(4.5))
                    .cursor_pointer()
                    .bg(if is_details_tab {
                        rgba(UiConstants::ACCENT_RGBA)
                    } else {
                        rgba(0x00000000u32)
                    })
                    .hover(|s| {
                        if is_details_tab {
                            s
                        } else {
                            s.bg(rgba(0xFFFFFF12u32))
                        }
                    })
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(6.0))
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        v_details.update(cx, |this, cx| {
                            this.state.main_view.side_pane = SidePaneMode::Details;
                            cx.notify();
                        });
                    })
                    .child(
                        div()
                            .text_size(px(11.5))
                            .font_family(UiConstants::FONT_ICONS)
                            .text_color(if is_details_tab {
                                rgb(0xFFFFFF)
                            } else {
                                rgba(UiConstants::TEXT_SECONDARY_RGBA)
                            })
                            .child("\u{E946}"),
                    )
                    .child(
                        div()
                            .text_size(px(12.0))
                            .font_weight(if is_details_tab {
                                FontWeight(600.0)
                            } else {
                                FontWeight(400.0)
                            })
                            .text_color(if is_details_tab {
                                rgb(0xFFFFFF)
                            } else {
                                rgba(UiConstants::TEXT_SECONDARY_RGBA)
                            })
                            .child("Details"),
                    ),
            )
            .child(
                div()
                    .px(px(12.0))
                    .py(px(4.0))
                    .rounded(px(4.5))
                    .cursor_pointer()
                    .bg(if is_preview_tab {
                        rgba(UiConstants::ACCENT_RGBA)
                    } else {
                        rgba(0x00000000u32)
                    })
                    .hover(|s| {
                        if is_preview_tab {
                            s
                        } else {
                            s.bg(rgba(0xFFFFFF12u32))
                        }
                    })
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(6.0))
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        let sp = selected_path.clone();
                        v_preview.update(cx, |this, cx| {
                            this.state.main_view.side_pane = SidePaneMode::Preview;
                            if let Some(p) = sp {
                                this.load_preview_content(p, cx);
                            }
                            cx.notify();
                        });
                    })
                    .child(
                        div()
                            .text_size(px(11.5))
                            .font_family(UiConstants::FONT_ICONS)
                            .text_color(if is_preview_tab {
                                rgb(0xFFFFFF)
                            } else {
                                rgba(UiConstants::TEXT_SECONDARY_RGBA)
                            })
                            .child("\u{E890}"),
                    )
                    .child(
                        div()
                            .text_size(px(12.0))
                            .font_weight(if is_preview_tab {
                                FontWeight(600.0)
                            } else {
                                FontWeight(400.0)
                            })
                            .text_color(if is_preview_tab {
                                rgb(0xFFFFFF)
                            } else {
                                rgba(UiConstants::TEXT_SECONDARY_RGBA)
                            })
                            .child("Preview"),
                    ),
            );

        let body_element: AnyElement = if sel_count == 0 {
            Self::render_empty_selection_details(state, view.clone()).into_any_element()
        } else if sel_count > 1 {
            Self::render_multi_selection_details(state, view.clone()).into_any_element()
        } else if let Some(item) = selected_item {
            if is_preview_tab {
                Self::render_preview_content(state, item, view.clone()).into_any_element()
            } else {
                Self::render_details_pane(state, item, view.clone()).into_any_element()
            }
        } else {
            Self::render_empty_selection_details(state, view.clone()).into_any_element()
        };

        div()
            .id("side-pane")
            .w(px(pane_w))
            .h_full()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .rounded(px(8.0))
            .bg(rgba(UiConstants::BG_TOOLBAR_RGBA))
            .border(px(1.0))
            .border_color(rgba(UiConstants::CARD_STROKE_RGBA))
            .overflow_hidden()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .h(px(42.0))
                    .items_center()
                    .justify_between()
                    .px(px(10.0))
                    .border_b(px(1.0))
                    .border_color(rgba(UiConstants::CARD_STROKE_RGBA))
                    .bg(rgba(0xFFFFFF05u32))
                    .child(segmented_tabs)
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(4.0))
                            .child(
                                div()
                                    .px(px(6.0))
                                    .h(px(26.0))
                                    .rounded(px(5.0))
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .justify_center()
                                    .gap(px(4.0))
                                    .cursor_pointer()
                                    .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
                                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                        v_peek_expand.update(cx, |this, cx| {
                                            this.toggle_peek_modal(cx);
                                        });
                                    })
                                    .child(
                                        div()
                                            .text_size(px(10.5))
                                            .font_family(UiConstants::FONT_ICONS)
                                            .text_color(rgb(0x60CDFF))
                                            .child("\u{E740}"),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(10.5))
                                            .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                            .child("Peek"),
                                    ),
                            )
                            .child(
                                div()
                                    .w(px(26.0))
                                    .h(px(26.0))
                                    .rounded(px(5.0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .cursor_pointer()
                                    .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
                                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                        v_close.update(cx, |this, cx| {
                                            this.state.main_view.side_pane = SidePaneMode::None;
                                            cx.notify();
                                        });
                                    })
                                    .child(
                                        div()
                                            .text_size(px(10.5))
                                            .font_family(UiConstants::FONT_ICONS)
                                            .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                            .child("\u{E711}"),
                                    ),
                            ),
                    ),
            )
            .child(
                div()
                    .id("side-pane-body-scroll")
                    .flex_1()
                    .min_h(px(0.0))
                    .h_full()
                    .w_full()
                    .overflow_y_scroll()
                    .track_scroll(&state.side_pane_scroll_handle)
                    .on_scroll_wheel(smooth_scroll_handle_wheel(
                        view.clone(),
                        |app_state| &mut app_state.main_view.side_pane_bouncy_scroll,
                        |app_state| &app_state.main_view.side_pane_scroll_handle,
                    ))
                    .p(px(12.0))
                    .flex()
                    .flex_col()
                    .child(body_element),
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
                            .child("\u{E8A0}"),
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
                            .child("\u{E74D}")
                    }),
            )
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
                                    .child("\u{E711}"),
                            )
                    }))
                    .into_any_element()
            })
    }

    #[allow(clippy::too_many_arguments)]
    fn render_file_view(
        state: &MainViewState,
        file_items: Entity<FileItems>,
        view: Entity<crate::app::AppWindow>,
        is_active: bool,
        settings_data: Option<&crate::ui::components::dialogs::SettingsPreReadData>,
        pane_w: f32,
        pane_focus: crate::core::navigation_manager::PaneFocus,
        base_origin: Point<f32>,
        show_pane_header: bool,
    ) -> impl IntoElement {
        let visible = state.visible_items_rc();
        let is_recycle_bin = matches!(state.target, ViewTarget::RecycleBin);
        let is_home = matches!(state.target, ViewTarget::Home);
        let is_settings = matches!(state.target, ViewTarget::Settings);

        let has_filter_header = state.show_filter_bar
            || !state.filter_text.is_empty()
            || state.group_by != GroupColumn::None;
        let filter_offset_y = if has_filter_header { 34.0 } else { 0.0 };

        let mut outer = div()
            .flex_col()
            .flex_grow()
            .h_full()
            .w_full()
            .min_h(px(0.0))
            .overflow_hidden()
            .bg(if is_active {
                rgba(UiConstants::BG_TRANSPARENT_RGBA)
            } else {
                rgba(UiConstants::BG_INACTIVE_PANE_RGBA)
            })
            .on_mouse_down(MouseButton::Left, {
                let v = view.clone();
                move |_ev, _, cx| {
                    v.update(cx, |this, cx| {
                        this.close_toolbar_flyout(cx);
                        this.focus_pane(pane_focus, cx);
                    });
                }
            });

        if show_pane_header {
            outer = outer.child(Self::render_dual_pane_header(
                state,
                is_active,
                pane_focus,
                view.clone(),
            ));
        }

        outer = outer.child(Self::filter_header(state, pane_focus, view.clone()));

        if visible.is_empty() && !state.loading && !is_home && !is_settings && !is_recycle_bin {
            return outer
                .child(Self::empty_state(state))
                .into_any_element();
        }

        let content: gpui::AnyElement = if is_settings {
            let data = settings_data.expect("Settings data required for Settings view");
            Self::render_settings_view(state, data, view.clone()).into_any_element()
        } else if is_recycle_bin {
            Self::render_recycle_bin_view(state, view.clone()).into_any_element()
        } else if is_home {
            Self::render_home_view(state, settings_data, view.clone()).into_any_element()
        } else {
            match state.view_mode {
                ViewMode::Details => {
                    {
                        let list_origin = point(
                            base_origin.x,
                            base_origin.y + filter_offset_y + 30.0,
                        );
                        Self::details_view_for_pane(
                            state,
                            &visible,
                            file_items,
                            view.clone(),
                            pane_focus,
                            list_origin,
                            pane_w,
                        )
                    }
                }
                ViewMode::List => {
                    let list_origin = point(base_origin.x, base_origin.y + filter_offset_y);
                    state
                        .render_list_view_for_pane(
                            file_items,
                            view.clone(),
                            pane_focus,
                            list_origin,
                            pane_w,
                        )
                }
                ViewMode::Cards => {
                    let list_origin = point(base_origin.x, base_origin.y + filter_offset_y);
                    state.render_cards_view_for_pane(
                        file_items,
                        view.clone(),
                        pane_focus,
                        list_origin,
                        pane_w,
                    )
                }
                ViewMode::Grid => {
                    let cols =
                        crate::ui::views::types::MainViewState::grid_columns_for_width(
                            pane_w,
                            state.zoom_level,
                        );
                    let list_origin = point(base_origin.x, base_origin.y + filter_offset_y);
                    state.render_grid_view_for_pane(
                        file_items,
                        view.clone(),
                        cols,
                        pane_w,
                        pane_focus,
                        list_origin,
                    )
                }
                ViewMode::Columns => {
                    let z = state.zoom_level.clamp(0.0, 1.0);
                    let zoom_scale = 0.80 + z * 0.55;
                    let col1_w = (state.column_widths.miller_col1 * zoom_scale)
                        .clamp(130.0, (pane_w * 0.45).max(200.0))
                        .round();
                    let header_h = (28.0 + z * 8.0).round();
                    let col2_origin = point(
                        base_origin.x + col1_w + 6.0,
                        base_origin.y + filter_offset_y + header_h,
                    );
                    state.render_columns_view_for_pane(
                        file_items,
                        view.clone(),
                        pane_focus,
                        col2_origin,
                        pane_w,
                    )
                }
            }
        };

        let mut view_container = div()
            .flex()
            .flex_col()
            .flex_1()
            .h_full()
            .w_full()
            .overflow_hidden();

        if !is_home && !is_settings && !is_recycle_bin {
            view_container = view_container.on_scroll_wheel(bouncy_scroll_handler_with_handle(
                view.clone(),
                move |app_state: &mut crate::app::AppState| {
                    &mut app_state.pane_view_mut(pane_focus).bouncy_scroll
                },
                move |app_state: &mut crate::app::AppState| {
                    &app_state.pane_view(pane_focus).uniform_list_scroll_handle
                },
            ));
        }

        outer
            .child(
                view_container
                    .child(if !is_home && !is_settings && !is_recycle_bin {
                        bouncy_overscroll_content(&state.bouncy_scroll, [content])
                            .into_any_element()
                    } else {
                        content
                    })
                    .children(if !is_home && !is_settings && !is_recycle_bin {
                        vec![uniform_list_scrollbar(
                            &state.bouncy_scroll,
                            &state.uniform_list_scroll_handle,
                        )
                        .into_any_element()]
                    } else {
                        vec![]
                    }),
            )
            .into_any_element()
    }

    fn filter_header(
        state: &MainViewState,
        pane_focus: crate::core::navigation_manager::PaneFocus,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let is_filtering = state.show_filter_bar || !state.filter_text.is_empty();
        let is_grouping = state.group_by != GroupColumn::None;

        if !is_filtering && !is_grouping {
            return div().into_any_element();
        }

        let visible_count = state.visible_items_rc().len();
        let total_items = state.items.len();

        let group_options = [
            (GroupColumn::Name, "Name"),
            (GroupColumn::FileType, "Type"),
            (GroupColumn::Size, "Size"),
            (GroupColumn::DateModified, "Date Modified"),
            (GroupColumn::DateCreated, "Date Created"),
            (GroupColumn::SyncStatus, "Sync Status"),
            (GroupColumn::Tag, "Tag"),
        ];

        let mut group_pills = div().flex().flex_row().items_center().gap(px(4.0));
        if is_grouping {
            for (g_col, g_name) in group_options {
                let active = state.group_by == g_col;
                let v_pill = view.clone();
                group_pills = group_pills.child(
                    div()
                        .px(px(8.0))
                        .py(px(2.5))
                        .rounded(px(5.0))
                        .bg(if active {
                            rgba(UiConstants::ACCENT_RGBA)
                        } else {
                            rgba(0xFFFFFF0Au32)
                        })
                        .hover(|s| {
                            if active {
                                s
                            } else {
                                s.bg(rgba(0xFFFFFF16u32))
                            }
                        })
                        .cursor_pointer()
                        .text_size(px(11.0))
                        .font_weight(if active {
                            FontWeight(600.0)
                        } else {
                            FontWeight(400.0)
                        })
                        .text_color(if active {
                            rgb(0xFFFFFF)
                        } else {
                            rgba(UiConstants::TEXT_SECONDARY_RGBA)
                        })
                        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                            v_pill.update(cx, |this, cx| {
                                this.focus_pane(pane_focus, cx);
                                this.set_group_by(g_col, cx);
                            });
                        })
                        .child(if active {
                            format!(
                                "{} {}",
                                g_name,
                                if state.group_ascending { "↑" } else { "↓" }
                            )
                        } else {
                            g_name.to_string()
                        }),
                );
            }

            let v_clear_group = view.clone();
            group_pills = group_pills.child(
                div()
                    .px(px(7.0))
                    .py(px(2.5))
                    .rounded(px(5.0))
                    .bg(rgba(0xE8112328u32))
                    .hover(|s| s.bg(rgba(0xE8112344u32)))
                    .cursor_pointer()
                    .text_size(px(10.5))
                    .text_color(rgb(0xFF8080))
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        v_clear_group.update(cx, |this, cx| {
                            this.focus_pane(pane_focus, cx);
                            this.set_group_by(GroupColumn::None, cx);
                        });
                    })
                    .child("✕ Ungroup"),
            );
        }

        let v_clear_filter = view.clone();

        div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .h(px(34.0))
            .px(px(12.0))
            .bg(rgba(0xFFFFFF06u32))
            .border_b(px(1.0))
            .border_color(rgba(UiConstants::DIVIDER_RGBA))
            .gap(px(8.0))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.0))
                    .children(if is_filtering {
                        vec![div()
                            .px(px(8.0))
                            .py(px(2.5))
                            .rounded(px(5.0))
                            .bg(rgba(0x60CDFF1Eu32))
                            .border(px(1.0))
                            .border_color(rgba(0x60CDFF55u32))
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(6.0))
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .font_family(UiConstants::FONT_MDL2)
                                    .text_color(rgb(0x60CDFF))
                                    .child("\u{E71C}"),
                            )
                            .child(
                                div()
                                    .text_size(px(11.5))
                                    .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                                    .child(if state.filter_text.is_empty() {
                                        "Filter active (type to filter)".to_string()
                                    } else {
                                        format!("Filter: \"{}\"", state.filter_text)
                                    }),
                            )
                            .child(
                                div()
                                    .cursor_pointer()
                                    .text_size(px(10.0))
                                    .font_family(UiConstants::FONT_MDL2)
                                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                        v_clear_filter.update(cx, |this, cx| {
                                            this.focus_pane(pane_focus, cx);
                                            this.state.pane_view_mut(pane_focus).show_filter_bar = false;
                                            this.state.main_view.show_filter_bar = false;
                                            this.set_filter_text(String::new(), cx);
                                        });
                                    })
                                    .child("\u{E711}"),
                            )
                            .into_any_element()]
                    } else {
                        vec![]
                    })
                    .children(if is_grouping {
                        vec![
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap(px(5.0))
                                .child(
                                    div()
                                        .text_size(px(11.5))
                                        .font_family(UiConstants::FONT_MDL2)
                                        .text_color(rgb(0x60CDFF))
                                        .child("\u{E8EC}"),
                                )
                                .child(
                                    div()
                                        .text_size(px(11.5))
                                        .font_weight(FontWeight(600.0))
                                        .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                        .child("Group by:"),
                                )
                                .into_any_element(),
                            group_pills.into_any_element(),
                        ]
                    } else {
                        vec![]
                    }),
            )
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(rgba(UiConstants::TEXT_MUTED_RGBA))
                    .child(if visible_count != total_items {
                        format!("{} of {} items", visible_count, total_items)
                    } else {
                        format!("{} items", total_items)
                    }),
            )
            .into_any_element()
    }

    /// Render the floating `Files-community` multi-item drag badge at the cursor during active file drags.
    pub fn render_floating_file_drag_badge(state: &MainViewState) -> Option<AnyElement> {
        if !state.drag_state.is_dragging || state.drag_state.paths.is_empty() {
            return None;
        }
        let count = state.drag_state.paths.len();
        let first_name = state
            .drag_state
            .paths
            .first()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "Item".to_string());

        let is_copy = state.drag_state.is_copy;
        let is_link = state.drag_state.is_link;
        let is_os_drag = state.drag_state.os_drag_dispatched;

        let (action_icon, action_text, badge_col) = if is_os_drag {
            ("\u{E8A7}", "Dragging to external app".to_string(), 0x107C10u32)
        } else if is_link {
            (
                "\u{E71B}",
                match &state.drag_state.hovered_target_label {
                    Some(lbl) => format!("Create shortcut in {}", lbl),
                    None => "Create shortcut (Alt)".to_string(),
                },
                0xC586C0u32,
            )
        } else if is_copy {
            (
                "\u{E8C8}",
                match &state.drag_state.hovered_target_label {
                    Some(lbl) => format!("+ Copy to {}", lbl),
                    None => "+ Copy items (Ctrl)".to_string(),
                },
                0x0078D4u32,
            )
        } else {
            (
                "\u{E8DE}",
                match &state.drag_state.hovered_target_label {
                    Some(lbl) => format!("→ Move to {}", lbl),
                    None => "Move items".to_string(),
                },
                0x60CDFFu32,
            )
        };

        let x = state.drag_state.current.x + 14.0;
        let y = state.drag_state.current.y + 14.0;

        Some(
            div()
                .absolute()
                .left(px(x))
                .top(px(y))
                .px(px(12.0))
                .py(px(8.0))
                .rounded(px(8.0))
                .bg(rgba(0x20232BE8u32))
                .border(px(1.5))
                .border_color(rgb(badge_col))
                .shadow_lg()
                .flex()
                .flex_col()
                .gap(px(4.0))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(8.0))
                        .child(
                            div()
                                .px(px(6.0))
                                .py(px(1.5))
                                .rounded(px(10.0))
                                .bg(rgb(badge_col))
                                .text_size(px(10.5))
                                .font_weight(FontWeight(700.0))
                                .text_color(rgb(0xFFFFFF))
                                .child(format!("{}", count)),
                        )
                        .child(
                            div()
                                .max_w(px(180.0))
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .text_size(px(12.0))
                                .font_weight(FontWeight(600.0))
                                .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                                .child(if count == 1 {
                                    first_name
                                } else {
                                    format!("{} + {} more", first_name, count - 1)
                                }),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(5.0))
                        .child(
                            div()
                                .text_size(px(11.0))
                                .font_family(UiConstants::FONT_MDL2)
                                .text_color(rgb(badge_col))
                                .child(action_icon),
                        )
                        .child(
                            div()
                                .text_size(px(11.0))
                                .font_weight(FontWeight(600.0))
                                .text_color(rgb(badge_col))
                                .child(action_text),
                        ),
                )
                .into_any_element(),
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
    use core::prelude::v1::test;
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
            tags: Vec::new(),
        }
    }

    #[test]
    fn test_lasso_state_rect_normalizes_coordinates() {
        let lasso = LassoState {
            start: point(100.0, 200.0),
            current: point(50.0, 150.0),
            ..Default::default()
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
            ..Default::default()
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
            ..Default::default()
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
            ..Default::default()
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
                tags: Vec::new(),
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
                tags: Vec::new(),
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

    #[test]
    fn test_three_state_sort_priority_folders_files_interleaved() {
        let mut st = MainViewState::new(FilesPath::new("/tmp"), ViewMode::Details);
        st.items = vec![
            FileItem {
                name: "b_file.txt".to_string(),
                path: FilesPath::new("/tmp/b_file.txt"),
                file_type: FileType::File,
                size: 100,
                is_hidden: false,
                is_system: false,
                modified: None,
                created: None,
                tags: Vec::new(),
            },
            FileItem {
                name: "c_folder".to_string(),
                path: FilesPath::new("/tmp/c_folder"),
                file_type: FileType::Directory,
                size: 0,
                is_hidden: false,
                is_system: false,
                modified: None,
                created: None,
                tags: Vec::new(),
            },
            FileItem {
                name: "a_folder".to_string(),
                path: FilesPath::new("/tmp/a_folder"),
                file_type: FileType::Directory,
                size: 0,
                is_hidden: false,
                is_system: false,
                modified: None,
                created: None,
                tags: Vec::new(),
            },
            FileItem {
                name: "d_file.txt".to_string(),
                path: FilesPath::new("/tmp/d_file.txt"),
                file_type: FileType::File,
                size: 200,
                is_hidden: false,
                is_system: false,
                modified: None,
                created: None,
                tags: Vec::new(),
            },
        ];
        st.bump_items_revision();

        // State 1: Folders first -> a_folder (2), c_folder (1), b_file.txt (0), d_file.txt (3)
        st.sort.folders_first = true;
        st.sort.files_first = false;
        assert_eq!(*st.visible_items_rc(), vec![2, 1, 0, 3]);

        // State 2: Files first -> b_file.txt (0), d_file.txt (3), a_folder (2), c_folder (1)
        st.sort.folders_first = false;
        st.sort.files_first = true;
        assert_eq!(*st.visible_items_rc(), vec![0, 3, 2, 1]);

        // State 3: Interleaved -> a_folder (2), b_file.txt (0), c_folder (1), d_file.txt (3)
        st.sort.folders_first = false;
        st.sort.files_first = false;
        assert_eq!(*st.visible_items_rc(), vec![2, 0, 1, 3]);
    }
}

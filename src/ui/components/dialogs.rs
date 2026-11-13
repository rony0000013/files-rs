use crate::ui::constants::UiConstants;
use gpui::*;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq)]
pub enum SidebarMenuTarget {
    Item {
        label: String,
        path: Option<String>,
        is_pinned: bool,
    },
    Group {
        section_id: crate::ui::components::sidebar_state::SectionId,
    },
    Background,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
/// Which flyout submenu is open inside a context menu (Files-community
/// parity: Layout / Sort by / Group by / New / Open with / Compress / Send
/// to expand as hover flyouts instead of flat sections).
pub enum SubmenuKind {
    Layout,
    SortBy,
    GroupBy,
    New,
    OpenWith,
    Compress,
    SendTo,
    FileTags,
    ShowMore,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DialogType {
    Rename {
        item_index: usize,
        current_name: String,
    },
    AddItem {
        is_folder: bool,
        name: String,
        selected_all: bool,
    },
    Dynamic {
        title: String,
        message: String,
    },
    /// Read-only item properties sheet (Alt+Enter).
    Properties {
        item: crate::domain::metadata::FileItem,
    },
    CommandPalette,
    BulkRename {
        item_indices: Vec<usize>,
    },
    ContextMenu {
        item_index: Option<usize>,
        position: Point<Pixels>,
        /// Pre-read data to avoid view.read(cx) crash during render
        has_selection: bool,
        selected_path: Option<crate::core::path_util::FilesPath>,
        selected_count: usize,
        is_dir: bool,
        open_submenu: Option<SubmenuKind>,
        selected_paths: Vec<std::path::PathBuf>,
        current_dir: std::path::PathBuf,
        /// Inline classic ("Show more options") entries, loaded on first hover.
        classic: Option<Vec<crate::platform::windows::classic_menu::ClassicMenuEntry>>,
        classic_loading: bool,
        /// Open path (indices) inside nested classic submenus.
        classic_open: Vec<usize>,
        /// True when flyouts must open to the left (no room on the right).
        flip: bool,
        /// Expanded style: classic items inlined, no ShowMore row.
        expanded: bool,
        /// Viewport-derived cap: the menu scrolls internally past this.
        max_h: f32,
        /// Pre-read window width to avoid view.read(cx) crash during render
        win_w: f32,
        /// Pre-read window height to avoid view.read(cx) crash during render
        win_h: f32,
        /// Scroll offset for the items viewport
        scroll_offset: f32,
        current_sort: crate::config::config::SortBy,
        current_group: crate::config::config::GroupColumn,
        is_pinned_to_sidebar: bool,
        available_tags: Vec<crate::config::config::TagDefinition>,
        item_tags: Vec<String>,
    },
    /// Sidebar context menu with target details
    SidebarContextMenu {
        target: SidebarMenuTarget,
        position: Point<Pixels>,
        reorder_mode: bool,
        hidden_sections: Vec<crate::ui::components::sidebar_state::SectionId>,
        /// Distance from click Y to window bottom edge, for flip-up positioning.
        /// When > 0, the menu bottom is placed `bottom_y` above the window bottom.
        bottom_y: f32,
    },
    /// Breadcrumb sibling folders dropdown
    SiblingDropdown {
        parent_path: crate::core::path_util::FilesPath,
        siblings: Vec<crate::core::path_util::FilesPath>,
    },
    /// Status & Notification Center showing live transfer jobs, conflicts, and notifications
    StatusCenter {
        active_operations: Vec<crate::core::progress::FileOperationJob>,
        expanded: Vec<uuid::Uuid>,
        notifications: Vec<crate::ui::components::status_center::NotificationItem>,
        conflicts: Vec<crate::ui::components::status_center::ConflictInfo>,
        active_filter: crate::ui::components::status_center::StatusCenterFilter,
        selected_index: usize,
        position: Option<Point<Pixels>>,
    },
    /// Tab context menu (shown on right-click of tab)
    TabContextMenu {
        tab_index: usize,
        position: Point<Pixels>,
        tab_count: usize,
        is_pinned: bool,
        can_reopen_closed: bool,
        other_windows: Vec<(u64, String)>,
    },
    /// Tab actions dropdown (shown on click of tab actions button)
    TabActionsDropdown {
        position: Point<Pixels>,
        open_tabs: Vec<(usize, String, String, bool, bool)>,
        can_reopen_closed: bool,
        is_dual_pane: bool,
    },
    /// Column visibility toggle menu (shown on right-click of column header)
    ColumnVisibilityMenu {
        position: Point<Pixels>,
    },
    /// Toolbar customization dialog
    ToolbarCustomization,
    /// Create shortcut dialog
    CreateShortcut {
        state: Box<crate::ui::dialog_services::shortcut_dialog::CreateShortcutState>,
    },
    /// Reorder sidebar items dialog
    ReorderSidebarItems,
    /// Create Archive Dialog
    CreateArchive {
        state: Box<crate::ui::dialog_services::archive_dialog::CreateArchiveState>,
    },
    /// Decompress Archive Dialog
    DecompressArchive {
        state: Box<crate::ui::dialog_services::decompress_dialog::DecompressState>,
    },
    /// Custom Properties Dialog (Compatibility, Signatures, etc.)
    CustomProperties {
        state: Box<crate::ui::dialog_services::custom_properties_dialog::CustomPropertiesState>,
    },
    /// Conflict Resolution Dialog for file operations
    Conflict {
        state: Box<crate::ui::dialog_services::conflict_dialog::ConflictDialogState>,
    },
    /// Clone Repository Dialog
    CloneRepo {
        state: Box<crate::ui::dialog_services::clone_repo_dialog::CloneRepoState>,
    },
    /// Zoom slider / view mode switcher popover
    ZoomSlider {
        position: Point<Pixels>,
        /// Live zoom_level snapshot — updated alongside main_view.zoom_level during drag
        current_zoom: f32,
        /// Snapshot of view_mode at creation time
        current_mode: crate::config::config::ViewMode,
        /// Whether folder view lock is enabled
        is_locked: bool,
    },
    /// Restore Crashed Session Dialog
    RestoreCrashedSession {
        crash_count: u32,
        last_path: String,
    },
    /// Unhide sections dialog — shows hidden groups and lets user select which to restore
    UnhideSections {
        hidden_sections: Vec<crate::ui::components::sidebar_state::SectionId>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SettingsTab {
    General,
    Appearance,
    Layout,
    Folders,
    Tags,
    Actions,
    Toolbar,
    Advanced,
    DevTools,
    About,
}

/// Interactive state for recording a new keyboard or mouse combination in Settings -> Actions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyBindingRecorderState {
    pub action: crate::core::action_registry::ActionKind,
    pub is_add_mode: bool,
    pub recorded_combo: Option<crate::core::action_registry::KeyCombination>,
    pub conflict_action: Option<crate::core::action_registry::ActionKind>,
    pub validation_error: Option<String>,
}

/// Interactive UI state for the Settings page (expanders, search, shortcut recorder, tag editor, IDE editor).
#[derive(Debug, Clone)]
pub struct SettingsUiState {
    pub search_query: String,
    pub search_focused: bool,
    pub expanded_sections: std::collections::HashSet<String>,
    pub actions_search_query: String,
    pub actions_search_focused: bool,
    pub actions_filter: String,
    pub recorder: Option<KeyBindingRecorderState>,
    pub confirming_restore_defaults: bool,
    pub pending_delete_action: Option<crate::core::action_registry::ActionKind>,
    pub is_creating_tag: bool,
    pub new_tag_name: String,
    pub new_tag_color: String,
    pub editing_tag_index: Option<usize>,
    pub editing_tag_name: String,
    pub editing_tag_color: String,
    pub is_editing_ide: bool,
    pub edit_ide_name: String,
    pub edit_ide_path: String,
    pub ide_focused_field: Option<String>,
    pub disk_thumb_count: usize,
    pub disk_thumb_bytes: u64,
    pub thumb_slider_dragging: bool,
    pub thumb_slider_track_bounds: std::sync::Arc<std::sync::Mutex<(f32, f32)>>,
}

impl Default for SettingsUiState {
    fn default() -> Self {
        let mut expanded = std::collections::HashSet::new();
        for key in [
            "home_quick_access",
            "home_drives",
            "home_network",
            "home_file_tags",
            "home_recent_files",
            "general_startup",
            "general_widgets",
            "general_context_menu",
            "appearance_theme_colors",
            "appearance_address_bar",
            "appearance_toolbar",
            "layout_sort",
            "layout_group",
            "layout_columns",
            "folders_hidden",
            "folders_single_click",
            "folders_sizes",
            "actions_mouse",
            "actions_commands",
            "tags_list",
            "devtools_ide",
            "advanced_cache",
            "about_feedback",
            "about_oss",
        ] {
            expanded.insert(key.to_string());
        }
        Self {
            search_query: String::new(),
            search_focused: false,
            expanded_sections: expanded,
            actions_search_query: String::new(),
            actions_search_focused: false,
            actions_filter: "all".to_string(),
            recorder: None,
            confirming_restore_defaults: false,
            pending_delete_action: None,
            is_creating_tag: false,
            new_tag_name: String::new(),
            new_tag_color: "#0078D4".to_string(),
            editing_tag_index: None,
            editing_tag_name: String::new(),
            editing_tag_color: "#0078D4".to_string(),
            is_editing_ide: false,
            edit_ide_name: "Visual Studio Code".to_string(),
            edit_ide_path: "code".to_string(),
            ide_focused_field: None,
            disk_thumb_count: 0,
            disk_thumb_bytes: 0,
            thumb_slider_dragging: false,
            thumb_slider_track_bounds: std::sync::Arc::new(std::sync::Mutex::new((0.0, 0.0))),
        }
    }
}

impl SettingsUiState {
    pub fn is_expanded(&self, key: &str) -> bool {
        self.expanded_sections.contains(key)
    }

    pub fn toggle_expander(&mut self, key: &str) {
        if self.expanded_sections.contains(key) {
            self.expanded_sections.remove(key);
        } else {
            self.expanded_sections.insert(key.to_string());
        }
    }
}

pub struct DialogsView;

/// Pre-read data for the Settings dialog to avoid BorrowMutError during render.
#[derive(Debug, Clone)]
pub struct SettingsPreReadData {
    pub active_tab: SettingsTab,
    pub config: crate::config::config::AppConfig,
    pub action_pairs: Vec<(
        crate::core::action_registry::ActionKind,
        crate::core::action_registry::KeyCombination,
    )>,
    pub all_actions: Vec<(
        crate::core::action_registry::ActionKind,
        Option<crate::core::action_registry::KeyCombination>,
        bool,
    )>,
    pub ui_state: SettingsUiState,
    pub tag_counts: std::collections::HashMap<String, i64>,
    pub cached_icon_count: usize,
    pub disk_thumb_count: usize,
    pub disk_thumb_bytes: u64,
    pub gpu_thumb_cap: usize,
    pub ram_thumb_cap: usize,
    pub toolbar_new_folder: bool,
    pub toolbar_new_file: bool,
    pub toolbar_cut: bool,
    pub toolbar_copy: bool,
    pub toolbar_paste: bool,
    pub toolbar_rename: bool,
    pub toolbar_delete: bool,
    pub toolbar_details_view: bool,
    pub toolbar_grid_view: bool,
    pub toolbar_columns_view: bool,
    pub toolbar_preview_pane: bool,
    pub toolbar_info_pane: bool,
}

impl DialogsView {
    /// Pre-read settings data before rendering to avoid BorrowMutError.
    /// Must be called from the render method where self.state is accessible.
    pub fn pre_read_settings_data(
        state: &crate::app::AppState,
        cx: &mut Context<crate::app::AppWindow>,
    ) -> SettingsPreReadData {
        let config = state.settings.read(cx).config().clone();
        let action_pairs = state.action_registry.iter();
        let all_actions = state.action_registry.all_entries();
        let tb = &state.main_view.toolbar_customization;
        let tag_counts = state
            .tag_service
            .get_all_tags()
            .unwrap_or_default()
            .into_iter()
            .collect();
        let cached_icon_count = state
            .icon_cache
            .lock()
            .map(|c| c.len())
            .unwrap_or(0);
        let ram_thumb_cap = config.experimental.thumbnail_cache_size_limit as usize;
        let gpu_thumb_cap = crate::ui::components::bounded_image_cache::gpu_cache_capacity_for_limit(
            config.experimental.thumbnail_cache_size_limit,
        );
        SettingsPreReadData {
            active_tab: state.active_settings_tab,
            config,
            action_pairs,
            all_actions,
            ui_state: state.settings_ui.clone(),
            tag_counts,
            cached_icon_count,
            disk_thumb_count: state.settings_ui.disk_thumb_count,
            disk_thumb_bytes: state.settings_ui.disk_thumb_bytes,
            gpu_thumb_cap,
            ram_thumb_cap,
            toolbar_new_folder: tb.is_visible("new_folder"),
            toolbar_new_file: tb.is_visible("new_file"),
            toolbar_cut: tb.is_visible("cut"),
            toolbar_copy: tb.is_visible("copy"),
            toolbar_paste: tb.is_visible("paste"),
            toolbar_rename: tb.is_visible("rename"),
            toolbar_delete: tb.is_visible("delete"),
            toolbar_details_view: tb.is_visible("details_view"),
            toolbar_grid_view: tb.is_visible("grid_view"),
            toolbar_columns_view: tb.is_visible("columns_view"),
            toolbar_preview_pane: tb.is_visible("preview_pane"),
            toolbar_info_pane: tb.is_visible("info_pane"),
        }
    }

    /// Pre-read settings data from an Entity<AppWindow> (for use in render methods).
    pub fn pre_read_settings_data_from_view(
        state: &crate::app::AppState,
        cx: &mut Context<crate::app::AppWindow>,
    ) -> SettingsPreReadData {
        Self::pre_read_settings_data(state, cx)
    }

    pub fn build(
        dialog: &DialogType,
        view: Entity<crate::app::AppWindow>,
        cx: &mut Context<crate::app::AppWindow>,
        settings_data: Option<SettingsPreReadData>,
        mica_geo: Option<crate::wallpaper_mica::MicaGeo>,
    ) -> impl IntoElement {
        let content = match dialog {
            DialogType::Rename {
                item_index,
                current_name,
            } => Self::build_rename_dialog(*item_index, current_name, view.clone())
                .into_any_element(),
            DialogType::AddItem {
                is_folder,
                name,
                selected_all,
            } => Self::build_add_item_dialog(*is_folder, name, *selected_all, view.clone())
                .into_any_element(),
            DialogType::SiblingDropdown {
                parent_path,
                siblings,
            } => Self::build_sibling_dropdown(parent_path, siblings, view.clone(), cx)
                .into_any_element(),
            DialogType::StatusCenter {
                active_operations,
                expanded,
                notifications,
                conflicts,
                active_filter,
                selected_index,
                ..
            } => Self::build_status_center(
                active_operations,
                expanded,
                notifications,
                conflicts,
                *active_filter,
                *selected_index,
                view.clone(),
            )
            .into_any_element(),
            DialogType::Dynamic { title, message } => {
                Self::build_dynamic_dialog(title, message, view.clone()).into_any_element()
            }
            DialogType::Properties { item } => {
                Self::build_item_properties_dialog(item, view.clone()).into_any_element()
            }
            DialogType::CommandPalette => {
                Self::build_command_palette_dialog(view.clone()).into_any_element()
            }
            DialogType::BulkRename { item_indices } => {
                Self::build_bulk_rename_dialog(item_indices, view.clone()).into_any_element()
            }
            DialogType::ContextMenu {
                item_index,
                position,
                has_selection,
                selected_path,
                selected_count,
                is_dir,
                open_submenu,
                selected_paths,
                current_dir,
                classic,
                classic_loading,
                classic_open,
                flip,
                expanded,
                max_h,
                win_w,
                win_h,
                scroll_offset,
                current_sort,
                current_group,
                is_pinned_to_sidebar,
                available_tags,
                item_tags,
            } => Self::build_context_menu_dialog(
                *item_index,
                *position,
                *has_selection,
                selected_path.clone(),
                *selected_count,
                *is_dir,
                *open_submenu,
                selected_paths.clone(),
                current_dir.clone(),
                classic.clone(),
                *classic_loading,
                classic_open.clone(),
                *flip,
                *expanded,
                *max_h,
                *win_w,
                *win_h,
                *scroll_offset,
                current_sort.clone(),
                current_group.clone(),
                *is_pinned_to_sidebar,
                available_tags.clone(),
                item_tags.clone(),
                view.clone(),
                cx,
                mica_geo.clone(),
            )
            .into_any_element(),
            DialogType::SidebarContextMenu {
                target,
                position,
                reorder_mode,
                hidden_sections,
                ..
            } => Self::build_sidebar_context_menu(
                target,
                *position,
                *reorder_mode,
                hidden_sections.clone(),
                view.clone(),
                cx,
            )
            .into_any_element(),
            DialogType::TabContextMenu {
                tab_index,
                position,
                tab_count,
                is_pinned,
                can_reopen_closed,
                other_windows,
            } => Self::build_tab_context_menu(
                *tab_index,
                *position,
                *tab_count,
                *is_pinned,
                *can_reopen_closed,
                other_windows.clone(),
                view.clone(),
            )
            .into_any_element(),
            DialogType::TabActionsDropdown {
                position,
                open_tabs,
                can_reopen_closed,
                is_dual_pane,
            } => Self::build_tab_actions_dropdown(
                *position,
                open_tabs.clone(),
                *can_reopen_closed,
                *is_dual_pane,
                view.clone(),
            )
            .into_any_element(),
            DialogType::ColumnVisibilityMenu { position } => {
                Self::build_column_visibility_menu(*position, view.clone()).into_any_element()
            }
            DialogType::ToolbarCustomization => {
                Self::build_toolbar_customization_dialog(view.clone()).into_any_element()
            }
            DialogType::CreateShortcut { state } => {
                Self::build_create_shortcut_dialog(state, view.clone()).into_any_element()
            }
            DialogType::ReorderSidebarItems => {
                Self::build_reorder_sidebar_items_dialog(view.clone()).into_any_element()
            }
            DialogType::CreateArchive { state } => {
                Self::build_create_archive_dialog(state, view.clone()).into_any_element()
            }
            DialogType::DecompressArchive { state } => {
                Self::build_decompress_archive_dialog(state, view.clone()).into_any_element()
            }
            DialogType::CustomProperties { state } => {
                Self::build_custom_properties_dialog(state, view.clone()).into_any_element()
            }
            DialogType::Conflict { state } => {
                Self::build_conflict_dialog(state, view.clone()).into_any_element()
            }
            DialogType::CloneRepo { state } => {
                Self::build_clone_repo_dialog(state, view.clone()).into_any_element()
            }
            DialogType::ZoomSlider {
                current_zoom,
                current_mode,
                is_locked,
                ..
            } => Self::build_zoom_slider(*current_zoom, *current_mode, *is_locked, view.clone(), cx)
                .into_any_element(),
            DialogType::RestoreCrashedSession {
                crash_count,
                last_path,
            } => Self::build_restore_crashed_session_dialog(*crash_count, last_path, view.clone())
                .into_any_element(),
            DialogType::UnhideSections { hidden_sections } => {
                Self::build_unhide_sections_dialog(hidden_sections, view.clone()).into_any_element()
            }
        };

        // Modal overlay
        let is_zoom = matches!(dialog, DialogType::ZoomSlider { .. });
        let is_popup_menu = matches!(
            dialog,
            DialogType::ContextMenu { .. }
                | DialogType::TabContextMenu { .. }
                | DialogType::TabActionsDropdown { .. }
                | DialogType::SidebarContextMenu { .. }
                | DialogType::ColumnVisibilityMenu { .. }
                | DialogType::SiblingDropdown { .. }
                | DialogType::ZoomSlider { .. }
        );
        let v_overlay_close = view.clone();

        // For ZoomSlider, add drag handlers to the overlay so they capture
        // mouse events even when the pointer leaves the dialog.
        let mut overlay = div()
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center();

        if is_zoom {
            let thumb_travel_total = 120.0;
            let v_drag = view.clone();
            overlay = overlay
                .on_mouse_down(MouseButton::Left, {
                    let v = v_overlay_close.clone();
                    move |_, _, cx| {
                        // Only close on overlay background clicks (dialog container blocks propagation)
                        v.update(cx, |this, cx| this.close_dialog(cx));
                    }
                })
                .on_mouse_move(move |event, _win, cx| {
                    v_drag.update(cx, |this, cx| {
                        // Cancel file preview tooltip when mouse enters dialog area
                        this.stop_hover_timer(cx);
                        if let Some((start_y, initial_zoom)) = this.zoom_drag_start {
                            let current_y: f32 = event.position.y.into();
                            let delta_y = current_y - start_y;
                            let new_zoom =
                                (initial_zoom - delta_y / thumb_travel_total).clamp(0.0, 1.0);
                            this.set_zoom_level(new_zoom, cx);
                        }
                    });
                })
                .on_mouse_up(MouseButton::Left, move |_event, _win, cx| {
                    v_overlay_close.update(cx, |this, cx| {
                        this.end_zoom_drag(cx);
                    });
                });
        } else {
            if !is_popup_menu {
                // WinUI 3 ContentDialog Smoke backdrop
                overlay = overlay.bg(rgba(0x00000075u32));
            }
            overlay = overlay
                .on_mouse_down(MouseButton::Left, {
                    let v = v_overlay_close.clone();
                    move |_, _, cx| {
                        v.update(cx, |this, cx| {
                            this.stop_hover_timer(cx);
                            this.close_dialog(cx);
                        });
                    }
                });
        }

        let mut container = div()
            // Stop clicks on the dialog itself from bubbling up to the overlay and closing it
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation());

        if is_popup_menu {
            match dialog {
                DialogType::ContextMenu { .. } => {
                    // ContextMenu manages its own container div with BG_FLYOUT_MICA_RGBA
                    // and SURFACE_STROKE_FLYOUT_RGBA to avoid duplicate borders or opaque backplates
                }
                _ => {
                    let (abs_x, abs_y) = match dialog {
                        DialogType::TabContextMenu { position, .. }
                        | DialogType::TabActionsDropdown { position, .. }
                        | DialogType::ColumnVisibilityMenu { position } => {
                            (f32::from(position.x), f32::from(position.y))
                        }
                        DialogType::ZoomSlider { .. } => {
                            let win_w = mica_geo.as_ref().map_or(1000.0, |g| g.w);
                            let win_h = mica_geo.as_ref().map_or(700.0, |g| g.h);
                            (win_w - 10.0 - 180.0, win_h - 48.0 - 168.0)
                        }
                        DialogType::SidebarContextMenu { position, bottom_y, .. } => {
                            let y = if *bottom_y > 0.0 { f32::from(position.y) - *bottom_y } else { f32::from(position.y) };
                            (f32::from(position.x), y)
                        }
                        _ => (0.0, 0.0),
                    };
                    container = container
                        .rounded(px(8.0))
                        .overflow_hidden()
                        .border(px(1.0))
                        .border_color(rgba(UiConstants::SURFACE_STROKE_FLYOUT_RGBA))
                        .shadow_lg();
                    if let Some(ref geo) = mica_geo {
                        container = container
                            .child(
                                div().absolute().inset_0().overflow_hidden().child(
                                    img(geo.path.clone())
                                        .absolute()
                                        .left(px(geo.left - abs_x))
                                        .top(px(geo.top - abs_y))
                                        .w(px(geo.w))
                                        .h(px(geo.h)),
                                ),
                            )
                            .child(
                                div().absolute().inset_0().bg(rgba(0x202020E0)),
                            );
                    } else {
                        container = container.bg(rgb(UiConstants::BG_FLYOUT_MICA_RGB));
                    }
                }
            }
        } else {
            container = container
                .bg(rgba(UiConstants::BG_DIALOG_RGBA))
                .border(px(1.0))
                .border_color(rgba(UiConstants::DIVIDER_RGBA))
                .rounded(px(8.0))
                .shadow_lg();
        }

        container = container.child(content);

        match dialog {
            DialogType::ContextMenu { position, .. } => {
                // Exactly the menu width: the inner menu paints its own
                // border, so any slack here would read as a dark strip and
                // any squeeze would open a gap before the flyout panels.
                container = container
                    .absolute()
                    .top(position.y)
                    .left(position.x)
                    .w(px(292.0));
            }
            DialogType::TabContextMenu { position, .. }
            | DialogType::TabActionsDropdown { position, .. }
            | DialogType::ColumnVisibilityMenu { position } => {
                container = container
                    .absolute()
                    .top(position.y)
                    .left(position.x)
                    .w(px(280.0));
            }
            DialogType::SidebarContextMenu {
                position, bottom_y, ..
            } => {
                if *bottom_y > 0.0 {
                    container = container
                        .absolute()
                        .bottom(px(*bottom_y + 4.0)) // +4px padding from edge
                        .left(position.x)
                        .w(px(280.0));
                } else {
                    container = container
                        .absolute()
                        .top(position.y)
                        .left(position.x)
                        .w(px(280.0));
                }
            }
            DialogType::ZoomSlider { .. } => {
                // Anchor to bottom-right clearly floating above the status bar with a 12px gap
                container = container
                    .absolute()
                    .bottom(px(48.0))
                    .right(px(10.0))
                    .w(px(180.0));
            }
            DialogType::StatusCenter { .. } => {
                container = container
                    .absolute()
                    .top(px(88.0))
                    .right(px(12.0))
                    .w(px(412.0))
                    .p(px(12.0));
            }
            DialogType::CustomProperties { .. } => {
                container = container.p(px(0.0)).overflow_hidden();
            }
            _ => {
                container = container
                    .w(px(400.0))
                    .p(px(UiConstants::PADDING_LARGE_RAW as f32));
            }
        }

        // Fluent DialogThemeTransition: quick fade on open.
        let container = container.with_animation(
            "dialog-entrance",
            Animation::new(std::time::Duration::from_millis(120)),
            |elem, delta| elem.opacity(delta),
        );
        overlay.child(container)
    }

    fn build_rename_dialog(
        _index: usize,
        current_name: &str,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let v_cancel = view.clone();
        let v_ok = view.clone();

        div()
            .flex_col()
            .gap(px(18.0))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(10.0))
                    .child(
                        div()
                            .text_size(px(18.0))
                            .font_family(UiConstants::FONT_ICONS)
                            .text_color(rgb(0x60CDFF))
                            .child("\u{E8AC}"),
                    )
                    .child(
                        div()
                            .text_size(px(18.0))
                            .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                            .font_weight(FontWeight(600.0))
                            .child("Rename"),
                    ),
            )
            .child(
                div()
                    .flex_col()
                    .gap(px(6.0))
                    .child(
                        div()
                            .text_size(px(13.0))
                            .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                            .child("Enter a new name:"),
                    )
                    // WinUI 3 Fluent TextBox
                    .child(
                        div()
                            .w_full()
                            .h(px(36.0))
                            .bg(rgba(0x1E1E1EFF))
                            .border(px(1.0))
                            .border_color(rgba(UiConstants::STROKE_RGBA))
                            .border_b(px(2.0))
                            .border_color(rgb(0x60CDFF))
                            .rounded(px(4.0))
                            .px(px(10.0))
                            .flex()
                            .items_center()
                            .gap(px(1.0))
                            .child(
                                div()
                                    .text_size(px(13.5))
                                    .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                                    .child(current_name.to_string()),
                            )
                            .child(
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
                                            .bg(rgb(0xFFFFFF))
                                            .with_animation(
                                                "add-cursor-blink",
                                                Animation::new(std::time::Duration::from_millis(500)).repeat(),
                                                |elem, delta| elem.opacity(if delta > 0.5 { 0.0 } else { 1.0 }),
                                            ),
                                    ),
                            ),
                    ),
            )
            // WinUI 3 ContentDialog Footer
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap(px(10.0))
                    .pt(px(14.0))
                    .border_t(px(1.0))
                    .border_color(rgba(UiConstants::DIVIDER_RGBA))
                    .child(Self::build_button("Rename", true, move |_, _, cx| {
                        v_ok.update(cx, |this, cx| this.close_dialog(cx));
                    }))
                    .child(Self::build_button("Cancel", false, move |_, _, cx| {
                        v_cancel.update(cx, |this, cx| this.close_dialog(cx));
                    })),
            )
    }

    fn build_add_item_dialog(
        is_folder: bool,
        name: &str,
        selected_all: bool,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let v_cancel = view.clone();
        let v_ok = view.clone();
        let title = if is_folder { "Create new Folder" } else { "Create new File" };
        let default_placeholder = if is_folder { "New folder" } else { "New Text Document.txt" };
        let icon_glyph = if is_folder { "\u{E8B7}" } else { "\u{E8A5}" };
        let icon_color = if is_folder { 0xFFE680u32 } else { 0x60CDFFu32 };

        div()
            .flex_col()
            .gap(px(18.0))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(10.0))
                    .child(
                        div()
                            .text_size(px(20.0))
                            .font_family(UiConstants::FONT_ICONS)
                            .text_color(rgb(icon_color))
                            .child(icon_glyph),
                    )
                    .child(
                        div()
                            .text_size(px(18.0))
                            .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                            .font_weight(FontWeight(600.0))
                            .child(title),
                    ),
            )
            .child(
                div()
                    .flex_col()
                    .gap(px(6.0))
                    .child(
                        div()
                            .text_size(px(13.0))
                            .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                            .child("Enter an item name:"),
                    )
                    // WinUI 3 Fluent TextBox
                    .child(
                        div()
                            .w_full()
                            .h(px(36.0))
                            .bg(rgba(0x1E1E1EFF))
                            .border(px(1.0))
                            .border_color(rgba(UiConstants::STROKE_RGBA))
                            .border_b(px(2.0))
                            .border_color(rgb(0x60CDFF))
                            .rounded(px(4.0))
                            .px(px(10.0))
                            .flex()
                            .items_center()
                            .gap(px(1.0))
                            .children(if name.is_empty() {
                                vec![
                                    div()
                                        .text_size(px(13.5))
                                        .text_color(rgba(UiConstants::TEXT_MUTED_RGBA))
                                        .child(default_placeholder.to_string())
                                        .into_any_element(),
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
                                                .bg(rgb(0xFFFFFF))
                                                .with_animation(
                                                    "add-item-cursor-blink",
                                                    Animation::new(std::time::Duration::from_millis(500)).repeat(),
                                                    |elem, delta| elem.opacity(if delta > 0.5 { 0.0 } else { 1.0 }),
                                                ),
                                        )
                                        .into_any_element(),
                                ]
                            } else if selected_all {
                                vec![
                                    div()
                                        .bg(rgb(0x0078D4))
                                        .text_color(rgb(0xFFFFFF))
                                        .rounded(px(2.0))
                                        .px(px(2.0))
                                        .text_size(px(13.5))
                                        .child(name.to_string())
                                        .into_any_element(),
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
                                                .bg(rgb(0xFFFFFF))
                                                .with_animation(
                                                    "add-item-cursor-blink",
                                                    Animation::new(std::time::Duration::from_millis(500)).repeat(),
                                                    |elem, delta| elem.opacity(if delta > 0.5 { 0.0 } else { 1.0 }),
                                                ),
                                        )
                                        .into_any_element(),
                                ]
                            } else {
                                vec![
                                    div()
                                        .text_size(px(13.5))
                                        .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                                        .child(name.to_string())
                                        .into_any_element(),
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
                                                .bg(rgb(0xFFFFFF))
                                                .with_animation(
                                                    "add-item-cursor-blink",
                                                    Animation::new(std::time::Duration::from_millis(500)).repeat(),
                                                    |elem, delta| elem.opacity(if delta > 0.5 { 0.0 } else { 1.0 }),
                                                ),
                                        )
                                        .into_any_element(),
                                ]
                            }),
                    ),
            )
            // WinUI 3 ContentDialog Footer
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap(px(10.0))
                    .pt(px(14.0))
                    .border_t(px(1.0))
                    .border_color(rgba(UiConstants::DIVIDER_RGBA))
                    .child(Self::build_button("Create", true, move |_, _, cx| {
                        v_ok.update(cx, |this, cx| this.confirm_add_item(cx));
                    }))
                    .child(Self::build_button("Cancel", false, move |_, _, cx| {
                        v_cancel.update(cx, |this, cx| this.close_dialog(cx));
                    })),
            )
    }

    fn build_create_archive_dialog(
        state: &crate::ui::dialog_services::archive_dialog::CreateArchiveState,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let v_cancel = view.clone();
        let v_ok = view.clone();
        let format_str = state.format.extension();
        let level_str = state.level.description();

        div()
            .flex_col()
            .gap(px(16.0))
            .child(
                div()
                    .text_size(AbsoluteLength::Pixels(px(18.0)))
                    .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                    .font_weight(FontWeight(600.0))
                    .child("Create Archive"),
            )
            .child(
                div()
                    .flex_col()
                    .gap(px(8.0))
                    .child(Self::build_settings_row("Format", format_str))
                    .child(Self::build_settings_row("Compression Level", level_str))
                    .child(Self::build_settings_row(
                        "Encryption",
                        if state.encrypt { "Enabled" } else { "Disabled" },
                    )),
            )
            .child(
                div()
                    .text_size(AbsoluteLength::Pixels(px(12.0)))
                    .text_color(rgb(UiConstants::TEXT_MUTED_RGB))
                    .child(format!(
                        "Destination: {}",
                        state.destination_path().display()
                    )),
            )
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap(px(8.0))
                    .mt(px(8.0))
                    .child(Self::build_button("Cancel", false, move |_, _, cx| {
                        v_cancel.update(cx, |this, cx| this.close_dialog(cx));
                    }))
                    .child(Self::build_button(
                        "Create Archive",
                        true,
                        move |_, _, cx| {
                            v_ok.update(cx, |this, cx| this.close_dialog(cx));
                        },
                    )),
            )
    }

    fn build_create_shortcut_dialog(
        state: &crate::ui::dialog_services::shortcut_dialog::CreateShortcutState,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let v_cancel = view.clone();
        let v_ok = view.clone();
        let target_name = state
            .target_path
            .file_name()
            .map_or("None", |s| s.to_str().unwrap_or("None"));

        div()
            .flex_col()
            .gap(px(16.0))
            .child(
                div()
                    .text_size(AbsoluteLength::Pixels(px(18.0)))
                    .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                    .font_weight(FontWeight(600.0))
                    .child("Create Shortcut"),
            )
            .child(
                div()
                    .flex_col()
                    .gap(px(8.0))
                    .child(Self::build_settings_row("Target", target_name))
                    .child(Self::build_settings_row("Name", &state.shortcut_name))
                    .child(Self::build_settings_row(
                        "Run as Admin",
                        if state.run_as_admin { "Yes" } else { "No" },
                    )),
            )
            .child(
                div()
                    .text_size(AbsoluteLength::Pixels(px(12.0)))
                    .text_color(rgb(UiConstants::TEXT_MUTED_RGB))
                    .child(format!("Location: {}", state.shortcut_directory.display())),
            )
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap(px(8.0))
                    .mt(px(8.0))
                    .child(Self::build_button("Cancel", false, move |_, _, cx| {
                        v_cancel.update(cx, |this, cx| this.close_dialog(cx));
                    }))
                    .child(Self::build_button("Create", true, move |_, _, cx| {
                        v_ok.update(cx, |this, cx| this.close_dialog(cx));
                    })),
            )
    }

    fn build_decompress_archive_dialog(
        state: &crate::ui::dialog_services::decompress_dialog::DecompressState,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        use crate::ui::dialog_services::decompress_dialog::DecompressInputField;

        let v_cancel = view.clone();
        let v_ok = view.clone();
        let v_browse = view.clone();
        let v_dest_focus = view.clone();
        let v_pwd_focus = view.clone();
        let v_pwd_show = view.clone();
        let v_sel_all = view.clone();
        let v_desel_all = view.clone();
        let v_sub = view.clone();
        let v_overwrite = view.clone();
        let v_open_dest = view.clone();

        let archive_name = state
            .archive_path
            .file_name()
            .map_or("Unknown".to_string(), |s| s.to_string_lossy().to_string());
        let badge = if state.format_badge.is_empty() {
            "ARCHIVE".to_string()
        } else {
            state.format_badge.clone()
        };

        let dest_focused = state.focused_field == Some(DecompressInputField::Destination);
        let pwd_focused = state.focused_field == Some(DecompressInputField::Password);
        let dest_display = if state.destination_text.is_empty() {
            state.destination.to_string_lossy().to_string()
        } else {
            state.destination_text.clone()
        };
        let resolved_preview = state.effective_destination_preview();

        let total_entries = state.contents.len();
        let selected_count = state.selected_count();
        let selected_size_str =
            crate::ui::components::status_bar::format_size(state.selected_size());
        let compressed_str = if state.compressed_size > 0 {
            crate::ui::components::status_bar::format_size(state.compressed_size)
        } else {
            "—".to_string()
        };
        let summary_text = if state.loading {
            "Scanning archive entries...".to_string()
        } else {
            format!(
                "{} / {} items • Uncompressed: {} (Compressed: {})",
                selected_count, total_entries, selected_size_str, compressed_str
            )
        };

        let raw_pwd = state.password.as_deref().unwrap_or("");
        let pwd_display = if state.show_password {
            raw_pwd.to_string()
        } else {
            "•".repeat(raw_pwd.chars().count())
        };

        let render_checkbox = |id_str: &'static str,
                               checked: bool,
                               label: &'static str,
                               on_click: Box<dyn Fn(&gpui::MouseDownEvent, &mut Window, &mut App) + 'static>| {
            div()
                .id(SharedString::from(id_str))
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.0))
                .py(px(3.0))
                .cursor_pointer()
                .on_mouse_down(MouseButton::Left, on_click)
                .child(
                    div()
                        .w(px(16.0))
                        .h(px(16.0))
                        .rounded(px(4.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(if checked {
                            rgb(UiConstants::ACCENT_COLOR_RGB)
                        } else {
                            rgba(0xFFFFFF10u32).into()
                        })
                        .border_1()
                        .border_color(if checked {
                            rgb(UiConstants::ACCENT_COLOR_RGB)
                        } else {
                            rgba(0xFFFFFF30u32).into()
                        })
                        .children(if checked {
                            Some(
                                div()
                                    .font_family(UiConstants::FONT_ICONS)
                                    .text_size(px(10.0))
                                    .text_color(rgb(0xFFFFFF))
                                    .child("\u{E73E}"),
                            )
                        } else {
                            None
                        }),
                )
                .child(
                    div()
                        .text_size(px(12.5))
                        .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                        .child(label),
                )
        };

        // Preview first 120 entries in scrollable list to keep dialog fast even on 10k-entry archives
        let visible_limit = 120usize;
        let truncated_entries = total_entries.saturating_sub(visible_limit);

        div()
            .flex()
            .flex_col()
            .w(px(520.0))
            .gap(px(12.0))
            // Header + Format Pill
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(8.0))
                            .child(
                                div()
                                    .font_family(UiConstants::FONT_ICONS)
                                    .text_size(px(18.0))
                                    .text_color(rgb(UiConstants::ACCENT_COLOR_RGB))
                                    .child("\u{F012}"),
                            )
                            .child(
                                div()
                                    .text_size(AbsoluteLength::Pixels(px(18.0)))
                                    .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                                    .font_weight(FontWeight(600.0))
                                    .child("Extract Archive"),
                            ),
                    )
                    .child(
                        div()
                            .px(px(8.0))
                            .py(px(2.0))
                            .rounded(px(4.0))
                            .bg(rgba(0x4CC2FF22u32))
                            .border_1()
                            .border_color(rgba(0x4CC2FF55u32))
                            .text_size(px(11.0))
                            .font_weight(FontWeight(600.0))
                            .text_color(rgb(0x4CC2FF))
                            .child(badge),
                    ),
            )
            // Archive metadata summary bar
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(4.0))
                    .p(px(10.0))
                    .rounded(px(6.0))
                    .bg(rgba(0xFFFFFF08u32))
                    .border_1()
                    .border_color(rgba(0xFFFFFF12u32))
                    .child(Self::build_settings_row("Archive", &archive_name))
                    .child(Self::build_settings_row("Selection", &summary_text)),
            )
            // Scrollable entry checklist
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.0))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .font_weight(FontWeight(600.0))
                                    .text_color(rgb(UiConstants::TEXT_MUTED_RGB))
                                    .child("Files to extract"),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .gap(px(6.0))
                                    .child(Self::build_compact_btn(
                                        "Select All",
                                        false,
                                        move |_, _, cx| {
                                            v_sel_all.update(cx, |this, cx| {
                                                if let Some(DialogType::DecompressArchive {
                                                    ref mut state,
                                                }) = this.state.active_dialog
                                                {
                                                    state.select_all();
                                                }
                                                cx.notify();
                                            });
                                        },
                                    ))
                                    .child(Self::build_compact_btn(
                                        "Deselect All",
                                        false,
                                        move |_, _, cx| {
                                            v_desel_all.update(cx, |this, cx| {
                                                if let Some(DialogType::DecompressArchive {
                                                    ref mut state,
                                                }) = this.state.active_dialog
                                                {
                                                    state.deselect_all();
                                                }
                                                cx.notify();
                                            });
                                        },
                                    )),
                            ),
                    )
                    .child(
                        div()
                            .id("decompress-entries-scroll")
                            .flex()
                            .flex_col()
                            .max_h(px(150.0))
                            .overflow_y_scroll()
                            .rounded(px(6.0))
                            .border_1()
                            .border_color(rgba(0xFFFFFF12u32))
                            .bg(rgba(0x00000028u32))
                            .p(px(4.0))
                            .gap(px(2.0))
                            .children(if state.loading {
                                vec![div()
                                    .p(px(12.0))
                                    .text_size(px(12.0))
                                    .text_color(rgb(UiConstants::TEXT_MUTED_RGB))
                                    .child("Reading archive index safely...")
                                    .into_any_element()]
                            } else if state.contents.is_empty() {
                                vec![div()
                                    .p(px(12.0))
                                    .text_size(px(12.0))
                                    .text_color(rgb(UiConstants::TEXT_MUTED_RGB))
                                    .child("Archive contains no extractable entries.")
                                    .into_any_element()]
                            } else {
                                let mut rows: Vec<AnyElement> = state
                                    .contents
                                    .iter()
                                    .take(visible_limit)
                                    .map(|entry| {
                                        let idx = entry.index;
                                        let checked = state.selected_entries.contains(&idx);
                                        let v_row = view.clone();
                                        let size_lbl = if entry.is_dir {
                                            "Folder".to_string()
                                        } else {
                                            crate::ui::components::status_bar::format_size(entry.size)
                                        };
                                        div()
                                            .id(SharedString::from(format!("decomp-entry-{}", idx)))
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .justify_between()
                                            .px(px(6.0))
                                            .py(px(4.0))
                                            .rounded(px(4.0))
                                            .cursor_pointer()
                                            .hover(|s| s.bg(rgba(0xFFFFFF0Fu32)))
                                            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                                v_row.update(cx, |this, cx| {
                                                    if let Some(DialogType::DecompressArchive {
                                                        ref mut state,
                                                    }) = this.state.active_dialog
                                                    {
                                                        state.toggle_entry(idx);
                                                    }
                                                    cx.notify();
                                                });
                                            })
                                            .child(
                                                div()
                                                    .flex()
                                                    .flex_row()
                                                    .items_center()
                                                    .gap(px(8.0))
                                                    .min_w_0()
                                                    .child(
                                                        div()
                                                            .w(px(14.0))
                                                            .h(px(14.0))
                                                            .rounded(px(3.0))
                                                            .flex()
                                                            .items_center()
                                                            .justify_center()
                                                            .bg(if checked {
                                                                rgb(UiConstants::ACCENT_COLOR_RGB)
                                                            } else {
                                                                rgba(0xFFFFFF10u32).into()
                                                            })
                                                            .border_1()
                                                            .border_color(rgba(0xFFFFFF2Cu32))
                                                            .children(if checked {
                                                                Some(
                                                                    div()
                                                                        .font_family(
                                                                            UiConstants::FONT_ICONS,
                                                                        )
                                                                        .text_size(px(9.0))
                                                                        .text_color(rgb(0xFFFFFF))
                                                                        .child("\u{E73E}"),
                                                                )
                                                            } else {
                                                                None
                                                            }),
                                                    )
                                                    .child(
                                                        div()
                                                            .font_family(UiConstants::FONT_ICONS)
                                                            .text_size(px(12.0))
                                                            .text_color(if entry.is_dir {
                                                                rgb(0xF5C242)
                                                            } else {
                                                                rgb(UiConstants::TEXT_MUTED_RGB)
                                                            })
                                                            .child(if entry.is_dir {
                                                                "\u{E8B7}"
                                                            } else {
                                                                "\u{E7C3}"
                                                            }),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_size(px(12.0))
                                                            .text_color(rgb(
                                                                UiConstants::TEXT_PRIMARY_RGB,
                                                            ))
                                                            .truncate()
                                                            .child(entry.path.clone()),
                                                    ),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(11.0))
                                                    .text_color(rgb(UiConstants::TEXT_MUTED_RGB))
                                                    .pl(px(8.0))
                                                    .child(size_lbl),
                                            )
                                            .into_any_element()
                                    })
                                    .collect();
                                if truncated_entries > 0 {
                                    rows.push(
                                        div()
                                            .px(px(6.0))
                                            .py(px(4.0))
                                            .text_size(px(11.0))
                                            .text_color(rgb(UiConstants::TEXT_MUTED_RGB))
                                            .child(format!(
                                                "... and {} more entries (included when selected)",
                                                truncated_entries
                                            ))
                                            .into_any_element(),
                                    );
                                }
                                rows
                            }),
                    ),
            )
            // Destination folder path + Browse button
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(5.0))
                    .child(
                        div()
                            .text_size(px(12.0))
                            .font_weight(FontWeight(600.0))
                            .text_color(rgb(UiConstants::TEXT_MUTED_RGB))
                            .child("Destination folder"),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(8.0))
                            .child(
                                div()
                                    .id("decomp-dest-input")
                                    .flex_1()
                                    .h(px(32.0))
                                    .px(px(10.0))
                                    .flex()
                                    .items_center()
                                    .rounded(px(5.0))
                                    .bg(rgba(0x00000038u32))
                                    .border_1()
                                    .border_color(if dest_focused {
                                        rgb(UiConstants::ACCENT_COLOR_RGB).into()
                                    } else {
                                        rgba(0xFFFFFF22u32)
                                    })
                                    .cursor_text()
                                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                        v_dest_focus.update(cx, |this, cx| {
                                            if let Some(DialogType::DecompressArchive {
                                                ref mut state,
                                            }) = this.state.active_dialog
                                            {
                                                state.focused_field =
                                                    Some(DecompressInputField::Destination);
                                            }
                                            cx.notify();
                                        });
                                    })
                                    .child(
                                        div()
                                            .w_full()
                                            .text_size(px(12.5))
                                            .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                                            .truncate()
                                            .child(if dest_display.is_empty() {
                                                "Enter destination path...".to_string()
                                            } else if dest_focused {
                                                format!("{}|", dest_display)
                                            } else {
                                                dest_display
                                            }),
                                    ),
                            )
                            .child(Self::build_button(
                                "Browse...",
                                false,
                                move |_, _, cx| {
                                    v_browse.update(cx, |this, cx| {
                                        this.browse_decompress_destination(cx);
                                    });
                                },
                            )),
                    )
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(rgb(UiConstants::TEXT_MUTED_RGB))
                            .truncate()
                            .child(format!("Output path: {}", resolved_preview.display())),
                    ),
            )
            // Options checkboxes + optional Password row
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .child(render_checkbox(
                        "decomp-opt-subfolder",
                        state.create_subfolder,
                        "Extract files into archive subfolder",
                        Box::new(move |_, _, cx| {
                            v_sub.update(cx, |this, cx| {
                                if let Some(DialogType::DecompressArchive { ref mut state }) =
                                    this.state.active_dialog
                                {
                                    state.create_subfolder = !state.create_subfolder;
                                }
                                cx.notify();
                            });
                        }),
                    ))
                    .child(render_checkbox(
                        "decomp-opt-overwrite",
                        state.overwrite,
                        "Overwrite existing files without prompting",
                        Box::new(move |_, _, cx| {
                            v_overwrite.update(cx, |this, cx| {
                                if let Some(DialogType::DecompressArchive { ref mut state }) =
                                    this.state.active_dialog
                                {
                                    state.toggle_overwrite();
                                }
                                cx.notify();
                            });
                        }),
                    ))
                    .child(render_checkbox(
                        "decomp-opt-open-dest",
                        state.open_destination,
                        "Show extracted files when complete",
                        Box::new(move |_, _, cx| {
                            v_open_dest.update(cx, |this, cx| {
                                if let Some(DialogType::DecompressArchive { ref mut state }) =
                                    this.state.active_dialog
                                {
                                    state.open_destination = !state.open_destination;
                                }
                                cx.notify();
                            });
                        }),
                    )),
            )
            // Password row (always accessible for encrypted zips/7z)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        div()
                            .w(px(80.0))
                            .text_size(px(12.0))
                            .text_color(rgb(UiConstants::TEXT_MUTED_RGB))
                            .child(if state.is_encrypted {
                                "Password *:"
                            } else {
                                "Password:"
                            }),
                    )
                    .child(
                        div()
                            .id("decomp-pwd-input")
                            .flex_1()
                            .h(px(30.0))
                            .px(px(10.0))
                            .flex()
                            .items_center()
                            .rounded(px(5.0))
                            .bg(rgba(0x00000038u32))
                            .border_1()
                            .border_color(if pwd_focused {
                                rgb(UiConstants::ACCENT_COLOR_RGB).into()
                            } else {
                                rgba(0xFFFFFF20u32)
                            })
                            .cursor_text()
                            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                v_pwd_focus.update(cx, |this, cx| {
                                    if let Some(DialogType::DecompressArchive { ref mut state }) =
                                        this.state.active_dialog
                                    {
                                        state.focused_field =
                                            Some(DecompressInputField::Password);
                                    }
                                    cx.notify();
                                });
                            })
                            .child(
                                div()
                                    .w_full()
                                    .text_size(px(12.0))
                                    .text_color(if raw_pwd.is_empty() && !pwd_focused {
                                        rgb(UiConstants::TEXT_MUTED_RGB)
                                    } else {
                                        rgb(UiConstants::TEXT_PRIMARY_RGB)
                                    })
                                    .truncate()
                                    .child(if raw_pwd.is_empty() {
                                        if pwd_focused {
                                            "|".to_string()
                                        } else if state.is_encrypted {
                                            "Required for encrypted entries".to_string()
                                        } else {
                                            "Optional (only if archive is encrypted)".to_string()
                                        }
                                    } else if pwd_focused {
                                        format!("{}|", pwd_display)
                                    } else {
                                        pwd_display
                                    }),
                            ),
                    )
                    .child(Self::build_compact_btn(
                        if state.show_password { "Hide" } else { "Show" },
                        state.show_password,
                        move |_, _, cx| {
                            v_pwd_show.update(cx, |this, cx| {
                                if let Some(DialogType::DecompressArchive { ref mut state }) =
                                    this.state.active_dialog
                                {
                                    state.toggle_password();
                                }
                                cx.notify();
                            });
                        },
                    )),
            )
            // Error banner if validation or scan failed
            .children(state.error.as_ref().map(|err| {
                div()
                    .p(px(8.0))
                    .rounded(px(5.0))
                    .bg(rgba(0xC42B1C28u32))
                    .border_1()
                    .border_color(rgba(0xFF6B6B55u32))
                    .text_size(px(12.0))
                    .text_color(rgb(0xFF8E8E))
                    .child(err.clone())
            }))
            // Action buttons
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap(px(8.0))
                    .mt(px(4.0))
                    .child(Self::build_button("Cancel", false, move |_, _, cx| {
                        v_cancel.update(cx, |this, cx| this.close_dialog(cx));
                    }))
                    .child(Self::build_button("Extract", true, move |_, _, cx| {
                        v_ok.update(cx, |this, cx| this.execute_decompress_dialog(cx));
                    })),
            )
    }

    fn build_sibling_dropdown(
        _parent_path: &crate::core::path_util::FilesPath,
        siblings: &[crate::core::path_util::FilesPath],
        view: Entity<crate::app::AppWindow>,
        _cx: &mut Context<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let v_close = view.clone();
        let v_nav = Arc::new(view.clone());

        div()
            .flex_col()
            .gap(px(4.0))
            .child(
                div()
                    .text_size(AbsoluteLength::Pixels(px(14.0)))
                    .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                    .font_weight(FontWeight(600.0))
                    .child("Folders in this location"),
            )
            .child(
                div()
                    .flex_col()
                    .max_h(px(300.0))
                    .overflow_hidden()
                    .children(siblings.iter().map(|sibling| {
                        let v_nav = v_nav.clone();
                        let name = sibling
                            .as_path()
                            .file_name()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_else(|| sibling.as_path().to_string_lossy().into_owned());
                        let path = sibling.clone();
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.0))
                            .px(px(8.0))
                            .py(px(6.0))
                            .rounded(px(4.0))
                            .hover(|s| s.bg(rgba(0xFFFFFF15u32)))
                            .cursor_pointer()
                            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                let v_nav = v_nav.clone();
                                v_nav.update(cx, |this, cx| {
                                    this.navigate_to_sibling(path.clone(), cx);
                                });
                            })
                            .child(
                                div()
                                    .text_size(px(14.0))
                                    .font_family(UiConstants::FONT_MDL2)
                                    .text_color(rgb(UiConstants::ACCENT_COLOR_RGB))
                                    .child("\u{E8B7}"),
                            )
                            .child(
                                div()
                                    .text_size(AbsoluteLength::Pixels(px(13.0)))
                                    .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                                    .child(name),
                            )
                    })),
            )
            .child(
                div()
                    .flex()
                    .justify_end()
                    .mt(px(8.0))
                    .child(Self::build_button("Cancel", false, move |_, _, cx| {
                        v_close.update(cx, |this, cx| this.close_dialog(cx));
                    })),
            )
    }

    /// Status & Notification Center flyout (Files-community `StatusCenter.xaml` parity):
    /// Header + filter tabs (`All`, `Active`, `Completed`, `Alerts`) + "Clear completed",
    /// interactive cards with Pause/Resume, Cancel, Open Folder, 88px expandable SpeedGraph,
    /// pending conflict resolution cards, and notification history.
    fn build_status_center(
        operations: &[crate::core::progress::FileOperationJob],
        expanded: &[uuid::Uuid],
        notifications: &[crate::ui::components::status_center::NotificationItem],
        conflicts: &[crate::ui::components::status_center::ConflictInfo],
        active_filter: crate::ui::components::status_center::StatusCenterFilter,
        selected_index: usize,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        use crate::core::progress::OperationStatus;
        use crate::ui::components::status_center::StatusCenterFilter;

        let v_head = view.clone();
        let active_count = operations
            .iter()
            .filter(|j| j.is_running_or_paused())
            .count();
        let completed_count = operations
            .iter()
            .filter(|j| matches!(j.status, OperationStatus::Completed))
            .count();
        let alert_count = operations
            .iter()
            .filter(|j| matches!(j.status, OperationStatus::Failed(_) | OperationStatus::Cancelled))
            .count()
            + notifications.len()
            + conflicts.len();

        let any_clearable = operations.iter().any(|j| !j.is_running_or_paused())
            || !notifications.is_empty();

        // 1. Header Bar (`HeaderGrid` in StatusCenter.xaml)
        let header = div()
            .flex()
            .items_center()
            .justify_between()
            .px(px(4.0))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        div()
                            .text_size(px(14.5))
                            .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                            .font_weight(FontWeight(600.0))
                            .child("Status Center"),
                    )
                    .children(if active_count > 0 {
                        Some(
                            div()
                                .px(px(7.0))
                                .py(px(2.0))
                                .rounded(px(10.0))
                                .bg(rgba(0x0078D438u32))
                                .text_size(px(10.5))
                                .font_weight(FontWeight(600.0))
                                .text_color(rgba(0x60CDFFFFu32))
                                .child(format!("{} active", active_count)),
                        )
                    } else {
                        None
                    }),
            )
            .child(if any_clearable {
                div()
                    .px(px(8.0))
                    .py(px(4.0))
                    .rounded(px(5.0))
                    .bg(rgba(0xFFFFFF0Cu32))
                    .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
                    .text_size(px(11.5))
                    .text_color(rgb(UiConstants::TEXT_SECONDARY_RGB))
                    .cursor_pointer()
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        v_head.update(cx, |this, cx| {
                            this.state.status_center.clear_finished();
                            this.sync_status_center_dialog();
                            cx.notify();
                        });
                    })
                    .child("Clear completed")
            } else {
                div()
            });

        // 2. Segmented Filter Tabs (`All` | `Active` | `Completed` | `Alerts`)
        let make_filter_pill = |label: &str, count: usize, filter: StatusCenterFilter| {
            let is_active = active_filter == filter;
            let v_f = view.clone();
            div()
                .flex()
                .items_center()
                .justify_center()
                .gap(px(4.0))
                .flex_1()
                .py(px(4.0))
                .px(px(6.0))
                .rounded(px(5.0))
                .cursor_pointer()
                .bg(if is_active {
                    rgba(0x0078D438u32)
                } else {
                    rgba(0x00000000u32)
                })
                .border(px(1.0))
                .border_color(if is_active {
                    rgba(UiConstants::ACCENT_RGBA)
                } else {
                    rgba(0x00000000u32)
                })
                .hover(|s| {
                    s.bg(if is_active {
                        rgba(0x0078D448u32)
                    } else {
                        rgba(0xFFFFFF0Eu32)
                    })
                })
                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                    v_f.update(cx, |this, cx| {
                        this.state.status_center.active_filter = filter;
                        this.state.status_center.selected_index = 0;
                        this.sync_status_center_dialog();
                        cx.notify();
                    });
                })
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
                        .child(label.to_string()),
                )
                .children(if count > 0 {
                    Some(
                        div()
                            .px(px(4.5))
                            .rounded(px(6.0))
                            .bg(rgba(0xFFFFFF16u32))
                            .text_size(px(10.0))
                            .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                            .child(count.to_string()),
                    )
                } else {
                    None
                })
        };

        let total_all = operations.len() + notifications.len() + conflicts.len();
        let filter_bar = div()
            .flex()
            .items_center()
            .gap(px(4.0))
            .p(px(3.0))
            .rounded(px(7.0))
            .bg(rgba(0x00000028u32))
            .border(px(1.0))
            .border_color(rgba(0xFFFFFF10u32))
            .child(make_filter_pill("All", total_all, StatusCenterFilter::All))
            .child(make_filter_pill(
                "Active",
                active_count,
                StatusCenterFilter::Active,
            ))
            .child(make_filter_pill(
                "Done",
                completed_count,
                StatusCenterFilter::Completed,
            ))
            .child(make_filter_pill(
                "Alerts",
                alert_count,
                StatusCenterFilter::Alerts,
            ));

        // 3. Collect filtered cards
        let mut cards: Vec<AnyElement> = Vec::new();
        let mut card_ordinal = 0usize;

        // 3a. Pending Conflict Cards (shown in All, Active, Alerts)
        if matches!(
            active_filter,
            StatusCenterFilter::All | StatusCenterFilter::Active | StatusCenterFilter::Alerts
        ) {
            for conflict in conflicts {
                let is_sel = card_ordinal == selected_index;
                card_ordinal += 1;
                cards.push(Self::conflict_card(conflict, is_sel, view.clone()).into_any_element());
            }
        }

        // 3b. File Operation Jobs (newest first, matching Files-community `StatusCenterItems.Insert(0, banner)`)
        for job in operations.iter().rev() {
            let include = match active_filter {
                StatusCenterFilter::All => true,
                StatusCenterFilter::Active => job.is_running_or_paused(),
                StatusCenterFilter::Completed => matches!(job.status, OperationStatus::Completed),
                StatusCenterFilter::Alerts => {
                    matches!(
                        job.status,
                        OperationStatus::Failed(_) | OperationStatus::Cancelled
                    )
                }
            };
            if !include {
                continue;
            }
            let is_exp = expanded.iter().any(|e| *e == job.id);
            let is_sel = card_ordinal == selected_index;
            card_ordinal += 1;
            cards.push(Self::status_card(job, is_exp, is_sel, view.clone()).into_any_element());
        }

        // 3c. Notification History Cards (newest first)
        if matches!(
            active_filter,
            StatusCenterFilter::All | StatusCenterFilter::Alerts
        ) {
            for notif in notifications.iter().rev() {
                let is_sel = card_ordinal == selected_index;
                card_ordinal += 1;
                cards.push(Self::notification_card(notif, is_sel, view.clone()).into_any_element());
            }
        }

        let mut col = div()
            .flex()
            .flex_col()
            .gap(px(10.0))
            .w_full()
            .child(header)
            .child(filter_bar);

        if cards.is_empty() {
            col = col.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(10.0))
                    .items_center()
                    .justify_center()
                    .py(px(32.0))
                    .child(
                        div()
                            .w(px(44.0))
                            .h(px(44.0))
                            .rounded(px(22.0))
                            .bg(rgba(0xFFFFFF08u32))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                div()
                                    .text_size(px(22.0))
                                    .font_family(UiConstants::FONT_ICONS)
                                    .text_color(rgba(UiConstants::TEXT_MUTED_RGBA))
                                    .child("\u{E9D9}"),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(13.5))
                            .font_weight(FontWeight(600.0))
                            .text_color(rgb(UiConstants::TEXT_SECONDARY_RGB))
                            .child("No file operations"),
                    )
                    .child(
                        div()
                            .text_size(px(11.5))
                            .text_color(rgba(UiConstants::TEXT_MUTED_RGBA))
                            .child("Ongoing transfers, speed graphs, and alerts appear here"),
                    ),
            );
        } else {
            col = col.child(
                div()
                    .id("status-center-scroll-list")
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .max_h(px(420.0))
                    .overflow_y_scroll()
                    .pr(px(2.0))
                    .children(cards),
            );
        }

        // Footer keyboard shortcuts bar
        col.child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .pt(px(4.0))
                .border_t(px(1.0))
                .border_color(rgba(0xFFFFFF10u32))
                .text_size(px(10.5))
                .text_color(rgba(UiConstants::TEXT_MUTED_RGBA))
                .child("↑/↓: Select • Space: Pause/Resume • E: Graph • Enter: Open • Del: Dismiss"),
        )
    }

    fn conflict_card(
        conflict: &crate::ui::components::status_center::ConflictInfo,
        is_selected: bool,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let cid = conflict.id;
        let file_name = conflict
            .dest_path
            .as_path()
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| conflict.dest_path.to_string_lossy().to_string());
        let v_rep = view.clone();
        let v_keep = view.clone();
        let v_skip = view.clone();

        div()
            .flex()
            .flex_col()
            .gap(px(8.0))
            .p(px(10.0))
            .rounded(px(8.0))
            .bg(rgba(0xF7B93E14u32))
            .border(px(1.0))
            .border_color(if is_selected {
                rgba(UiConstants::ACCENT_RGBA)
            } else {
                rgba(0xF7B93E66u32)
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.0))
                    .child(
                        div()
                            .w(px(32.0))
                            .h(px(32.0))
                            .rounded(px(16.0))
                            .flex_shrink_0()
                            .flex()
                            .items_center()
                            .justify_center()
                            .bg(rgba(0xF7B93E30u32))
                            .child(
                                div()
                                    .text_size(px(15.0))
                                    .font_family(UiConstants::FONT_ICONS)
                                    .text_color(rgba(0xF7B93EFFu32))
                                    .child("\u{E7BA}"),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w(px(0.0))
                            .child(
                                div()
                                    .text_size(px(13.0))
                                    .font_weight(FontWeight(600.0))
                                    .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                                    .child(format!("Conflict: {}", file_name)),
                            )
                            .child(
                                div()
                                    .text_size(px(11.5))
                                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                    .child("Destination file already exists"),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_end()
                    .gap(px(6.0))
                    .child(
                        div()
                            .px(px(8.0))
                            .py(px(4.0))
                            .rounded(px(4.0))
                            .bg(rgba(0x0078D444u32))
                            .hover(|s| s.bg(rgba(0x0078D466u32)))
                            .cursor_pointer()
                            .text_size(px(11.0))
                            .font_weight(FontWeight(600.0))
                            .text_color(rgb(0xFFFFFF))
                            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                v_rep.update(cx, |this, cx| {
                                    this.state.status_center.remove_conflict(cid);
                                    this.sync_status_center_dialog();
                                    cx.notify();
                                });
                            })
                            .child("Replace"),
                    )
                    .child(
                        div()
                            .px(px(8.0))
                            .py(px(4.0))
                            .rounded(px(4.0))
                            .bg(rgba(0xFFFFFF14u32))
                            .hover(|s| s.bg(rgba(0xFFFFFF24u32)))
                            .cursor_pointer()
                            .text_size(px(11.0))
                            .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                v_keep.update(cx, |this, cx| {
                                    this.state.status_center.remove_conflict(cid);
                                    this.sync_status_center_dialog();
                                    cx.notify();
                                });
                            })
                            .child("Keep both"),
                    )
                    .child(
                        div()
                            .px(px(8.0))
                            .py(px(4.0))
                            .rounded(px(4.0))
                            .bg(rgba(0xFFFFFF0Cu32))
                            .hover(|s| s.bg(rgba(0xFFFFFF1Cu32)))
                            .cursor_pointer()
                            .text_size(px(11.0))
                            .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                v_skip.update(cx, |this, cx| {
                                    this.state.status_center.remove_conflict(cid);
                                    this.sync_status_center_dialog();
                                    cx.notify();
                                });
                            })
                            .child("Skip"),
                    ),
            )
    }

    fn notification_card(
        notif: &crate::ui::components::status_center::NotificationItem,
        is_selected: bool,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        use crate::ui::components::status_center::NotificationSeverity;
        let nid = notif.id;
        let dest = notif.target_path.clone();
        let v_dis = view.clone();
        let v_open = view.clone();

        let (ring_bg, glyph, glyph_color) = match notif.severity {
            NotificationSeverity::Success => (rgba(0x107C1038u32), "\u{E73E}", rgba(0x6CCB5FFFu32)),
            NotificationSeverity::Error => (rgba(0xE8112338u32), "\u{E783}", rgba(0xF1707BFFu32)),
            NotificationSeverity::Warning => (rgba(0xF7B93E38u32), "\u{E7BA}", rgba(0xF7B93EFFu32)),
            NotificationSeverity::Info => (
                rgba(0x0078D438u32),
                "\u{E946}",
                rgba(UiConstants::ACCENT_RGBA),
            ),
        };

        div()
            .flex()
            .items_center()
            .gap(px(10.0))
            .p(px(10.0))
            .rounded(px(8.0))
            .bg(rgba(0xFFFFFF08u32))
            .border(px(1.0))
            .border_color(if is_selected {
                rgba(UiConstants::ACCENT_RGBA)
            } else {
                rgba(UiConstants::STROKE_RGBA)
            })
            .child(
                div()
                    .w(px(32.0))
                    .h(px(32.0))
                    .rounded(px(16.0))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(ring_bg)
                    .child(
                        div()
                            .text_size(px(14.0))
                            .font_family(UiConstants::FONT_ICONS)
                            .text_color(glyph_color)
                            .child(glyph),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w(px(0.0))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap(px(6.0))
                            .child(
                                div()
                                    .text_size(px(12.5))
                                    .font_weight(FontWeight(600.0))
                                    .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                                    .overflow_hidden()
                                    .whitespace_nowrap()
                                    .child(notif.title.clone()),
                            )
                            .child(
                                div()
                                    .text_size(px(10.5))
                                    .text_color(rgba(UiConstants::TEXT_MUTED_RGBA))
                                    .child(notif.elapsed_age_label()),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(11.5))
                            .text_color(rgb(UiConstants::TEXT_SECONDARY_RGB))
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .child(notif.message.clone()),
                    ),
            )
            .children(if let Some(target) = dest {
                Some(
                    div()
                        .px(px(8.0))
                        .py(px(4.0))
                        .rounded(px(4.0))
                        .bg(rgba(0x0078D433u32))
                        .hover(|s| s.bg(rgba(0x0078D455u32)))
                        .cursor_pointer()
                        .text_size(px(11.0))
                        .font_weight(FontWeight(600.0))
                        .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                            let p = target.clone();
                            v_open.update(cx, |this, cx| {
                                this.close_dialog(cx);
                                let dir = if p.as_path().is_file() {
                                    p.parent().unwrap_or_else(|| p.clone())
                                } else {
                                    p
                                };
                                this.navigate(dir, cx);
                            });
                        })
                        .child("Open"),
                )
            } else {
                None
            })
            .child(
                div()
                    .w(px(28.0))
                    .h(px(28.0))
                    .rounded(px(4.0))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        v_dis.update(cx, |this, cx| {
                            this.state.status_center.remove_notification(nid);
                            this.sync_status_center_dialog();
                            cx.notify();
                        });
                    })
                    .child(
                        div()
                            .text_size(px(11.0))
                            .font_family(UiConstants::FONT_ICONS)
                            .child("\u{E711}"),
                    ),
            )
    }

    fn status_card(
        job: &crate::core::progress::FileOperationJob,
        expanded: bool,
        is_selected: bool,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        use crate::core::progress::{OpKind, OperationStatus};
        let running = job.is_running_or_paused();
        let paused = job.is_paused();
        let op_glyph = match job.kind {
            OpKind::Copy => "\u{E8C8}",
            OpKind::Move => "\u{E8C6}",
            OpKind::Delete => "\u{E74D}",
            OpKind::Extract => "\u{E8DE}",
            OpKind::Compress => "\u{E8B7}",
            OpKind::Git => "\u{E8AD}",
            OpKind::BulkRename => "\u{E8AC}",
            OpKind::Other => "\u{E9D9}",
        };
        let (ring, glyph, glyph_color) = match &job.status {
            OperationStatus::Pending => (
                rgba(0xFFFFFF20u32),
                op_glyph,
                rgba(UiConstants::TEXT_SECONDARY_RGBA),
            ),
            OperationStatus::InProgress { .. } => (
                rgba(0x0078D440u32),
                op_glyph,
                rgba(UiConstants::ACCENT_RGBA),
            ),
            OperationStatus::Paused { .. } => (
                rgba(0xF7B93E40u32),
                "\u{E769}",
                rgba(0xF7B93EFFu32),
            ),
            OperationStatus::Completed => (
                rgba(0x107C1040u32),
                "\u{E73E}",
                rgba(0x6CCB5FFFu32),
            ),
            OperationStatus::Failed(_) => (
                rgba(0xE8112340u32),
                "\u{E783}",
                rgba(0xF1707BFFu32),
            ),
            OperationStatus::Cancelled => (
                rgba(0xFFFFFF20u32),
                "\u{E711}",
                rgba(UiConstants::TEXT_SECONDARY_RGBA),
            ),
        };

        let subtitle = if running {
            Self::status_message(job)
        } else {
            match &job.status {
                OperationStatus::Completed => {
                    if let Some(ref dest) = job.destination_path {
                        format!("Completed • {}", dest.to_string_lossy())
                    } else {
                        "Completed successfully".to_string()
                    }
                }
                OperationStatus::Failed(r) => format!("Failed: {}", r),
                OperationStatus::Cancelled => "Operation cancelled".to_string(),
                _ => String::new(),
            }
        };

        let mut card = div()
            .flex()
            .flex_col()
            .gap(px(8.0))
            .p(px(10.0))
            .rounded(px(8.0))
            .bg(rgba(0xFFFFFF0Au32))
            .border(px(1.0))
            .border_color(if is_selected {
                rgba(UiConstants::ACCENT_RGBA)
            } else {
                rgba(UiConstants::STROKE_RGBA)
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.0))
                    .child(
                        div()
                            .w(px(32.0))
                            .h(px(32.0))
                            .rounded(px(16.0))
                            .flex_shrink_0()
                            .flex()
                            .items_center()
                            .justify_center()
                            .bg(ring)
                            .child(
                                div()
                                    .text_size(px(15.0))
                                    .font_family(UiConstants::FONT_ICONS)
                                    .text_color(glyph_color)
                                    .child(glyph.to_string()),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w(px(0.0))
                            .overflow_hidden()
                            .child(
                                div()
                                    .text_size(px(13.0))
                                    .font_weight(FontWeight(600.0))
                                    .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                                    .overflow_hidden()
                                    .whitespace_nowrap()
                                    .child(job.name.clone()),
                            )
                            .child(
                                div()
                                    .text_size(px(11.5))
                                    .text_color(if paused {
                                        rgba(0xF7B93EFFu32)
                                    } else {
                                        rgba(UiConstants::TEXT_SECONDARY_RGBA)
                                    })
                                    .overflow_hidden()
                                    .whitespace_nowrap()
                                    .child(subtitle),
                            ),
                    )
                    .child(Self::status_actions(
                        job,
                        running,
                        paused,
                        expanded,
                        view.clone(),
                    )),
            );

        if running {
            card = card.child(Self::status_progress(job, expanded, view.clone()));
        }
        card
    }

    /// Right-side card buttons: Pause/Resume + Expand Chevron + Cancel (when running/paused),
    /// or Open Folder + Dismiss (when finished).
    fn status_actions(
        job: &crate::core::progress::FileOperationJob,
        running: bool,
        paused: bool,
        expanded: bool,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let id = job.id;
        let dest_path = job.destination_path.clone();
        let mut row = div().flex().items_center().gap(px(4.0)).flex_shrink_0();
        if running {
            // Pause / Resume button
            let v_pause = view.clone();
            row = row.child(
                div()
                    .w(px(28.0))
                    .h(px(28.0))
                    .rounded(px(4.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .bg(if paused {
                        rgba(0xF7B93E28u32)
                    } else {
                        rgba(0x00000000u32)
                    })
                    .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
                    .text_color(if paused {
                        rgba(0xF7B93EFFu32)
                    } else {
                        rgba(UiConstants::TEXT_SECONDARY_RGBA)
                    })
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        v_pause.update(cx, |this, cx| {
                            this.toggle_pause_job(id, cx);
                        });
                    })
                    .child(
                        div()
                            .text_size(px(12.5))
                            .font_family(UiConstants::FONT_ICONS)
                            .child(if paused { "\u{E768}" } else { "\u{E769}" }),
                    ),
            );

            // Expand / Collapse SpeedGraph button
            let v_exp = view.clone();
            row = row.child(
                div()
                    .w(px(28.0))
                    .h(px(28.0))
                    .rounded(px(4.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        v_exp.update(cx, |this, cx| {
                            this.state.status_center.toggle_expanded(id);
                            this.sync_status_center_dialog();
                            cx.notify();
                        });
                    })
                    .child(
                        div()
                            .text_size(px(12.5))
                            .font_family(UiConstants::FONT_ICONS)
                            .child(if expanded { "\u{E70E}" } else { "\u{E70D}" }),
                    ),
            );

            // Cancel button
            let v_cancel = view.clone();
            row = row.child(
                div()
                    .w(px(28.0))
                    .h(px(28.0))
                    .rounded(px(4.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .hover(|s| s.bg(rgba(0xE8112344u32)))
                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        v_cancel.update(cx, |this, cx| {
                            this.state.progress_engine.request_cancel(id);
                            cx.notify();
                        });
                    })
                    .child(
                        div()
                            .text_size(px(11.5))
                            .font_family(UiConstants::FONT_ICONS)
                            .child("\u{E711}"),
                    ),
            );
        } else {
            if let Some(dest) = dest_path {
                let v_open = view.clone();
                row = row.child(
                    div()
                        .px(px(8.0))
                        .py(px(4.0))
                        .rounded(px(4.0))
                        .bg(rgba(0x0078D433u32))
                        .hover(|s| s.bg(rgba(0x0078D455u32)))
                        .cursor_pointer()
                        .text_size(px(11.0))
                        .font_weight(FontWeight(600.0))
                        .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                            let p = dest.clone();
                            v_open.update(cx, |this, cx| {
                                this.close_dialog(cx);
                                let dir = if p.as_path().is_file() {
                                    p.parent().unwrap_or_else(|| p.clone())
                                } else {
                                    p
                                };
                                this.navigate(dir, cx);
                            });
                        })
                        .child("Open"),
                );
            }

            let v_dis = view.clone();
            row = row.child(
                div()
                    .w(px(28.0))
                    .h(px(28.0))
                    .rounded(px(4.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        v_dis.update(cx, |this, cx| {
                            this.state.status_center.jobs.retain(|j| j.id != id);
                            this.state.status_center.expanded.retain(|e| *e != id);
                            this.sync_status_center_dialog();
                            cx.notify();
                        });
                    })
                    .child(
                        div()
                            .text_size(px(11.5))
                            .font_family(UiConstants::FONT_ICONS)
                            .child("\u{E711}"),
                    ),
            );
        }
        row
    }

    /// Progress bar + (when expanded) the 88px SpeedGraph and two-line item details (`StatusCenter.xaml`).
    fn status_progress(
        job: &crate::core::progress::FileOperationJob,
        expanded: bool,
        _view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let frac = job.progress_fraction();
        let paused = job.is_paused();
        let bar_color = if paused {
            rgba(0xF7B93EFFu32)
        } else {
            rgba(UiConstants::ACCENT_RGBA)
        };
        let mut col = div().flex().flex_col().gap(px(6.0));
        if !expanded {
            col = col.child(
                div()
                    .relative()
                    .h(px(4.5))
                    .w_full()
                    .rounded(px(2.5))
                    .bg(rgba(0xFFFFFF24u32))
                    .child(
                        div()
                            .absolute()
                            .left(px(0.0))
                            .top(px(0.0))
                            .h_full()
                            .w(px((frac * 364.0).clamp(4.0, 364.0)))
                            .rounded(px(2.5))
                            .bg(bar_color),
                    ),
            );
        } else {
            col = col.child(Self::speed_graph(job));
            if !job.current_file.is_empty() {
                col = col.child(
                    div()
                        .text_size(px(11.5))
                        .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .child(format!("Name: {}", job.current_file)),
                );
            }
            col = col.child(
                div()
                    .text_size(px(11.0))
                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .child(Self::status_message(job)),
            );
        }
        col
    }

    /// Subtitle line: percentage • bytes • item count • speed • ETA.
    fn status_message(job: &crate::core::progress::FileOperationJob) -> String {
        use crate::core::progress::{OpKind, OperationStatus};
        match &job.status {
            OperationStatus::InProgress {
                progress,
                bytes_processed,
                total_bytes,
                speed,
                eta,
            } => {
                let pct = (progress.clamp(0.0, 1.0) * 100.0).round() as u32;
                if job.kind == OpKind::Delete {
                    return format!(
                        "{}% • {} of {} items",
                        pct,
                        bytes_processed.min(total_bytes),
                        total_bytes
                    );
                }
                let mut parts = Vec::new();
                parts.push(format!("{}%", pct));
                parts.push(format!(
                    "{} of {}",
                    Self::fmt_bytes(*bytes_processed),
                    Self::fmt_bytes(*total_bytes)
                ));
                if job.total_items > 1 {
                    parts.push(format!("{}/{} items", job.items_processed, job.total_items));
                }
                if *speed > 0.5 {
                    parts.push(format!("{}/s", Self::fmt_bytes(*speed as u64)));
                }
                if let Some(d) = eta {
                    let s = d.as_secs();
                    if s > 0 {
                        parts.push(if s >= 60 {
                            format!("{}m {}s left", s / 60, s % 60)
                        } else {
                            format!("{}s left", s)
                        });
                    }
                }
                parts.join(" • ")
            }
            OperationStatus::Paused {
                progress,
                bytes_processed,
                total_bytes,
            } => {
                let pct = (progress.clamp(0.0, 1.0) * 100.0).round() as u32;
                format!(
                    "Paused ({}%) • {} of {}",
                    pct,
                    Self::fmt_bytes(*bytes_processed),
                    Self::fmt_bytes(*total_bytes)
                )
            }
            _ => String::new(),
        }
    }

    /// 88px SpeedGraph (`MainGraphCartesianChartClipGrid` in `StatusCenter.xaml`):
    /// Cartesian gridlines + 28 trailing speed bars + top-right "Speed:" readout + progress fill bar.
    fn speed_graph(job: &crate::core::progress::FileOperationJob) -> impl IntoElement {
        use crate::core::progress::OperationStatus;
        let speed = match &job.status {
            OperationStatus::InProgress { speed, .. } => *speed,
            _ => 0.0,
        };
        let paused = job.is_paused();
        let bar_color = if paused {
            rgba(0xF7B93ECCu32)
        } else {
            rgba(UiConstants::ACCENT_RGBA)
        };
        let samples: Vec<f32> = job
            .speed_history
            .iter()
            .rev()
            .take(28)
            .rev()
            .cloned()
            .collect();
        let max = samples.iter().cloned().fold(1.0f32, f32::max);
        let mut bars = div().flex().flex_row().items_end().gap(px(2.0)).h(px(68.0));
        for s in samples {
            let h = 4.0 + 62.0 * (s / max).clamp(0.0, 1.0);
            bars = bars.child(
                div()
                    .w(px(7.0))
                    .h(px(h))
                    .rounded_t(px(2.0))
                    .bg(bar_color),
            );
        }
        div()
            .relative()
            .flex()
            .items_end()
            .justify_between()
            .gap(px(8.0))
            .h(px(88.0))
            .px(px(10.0))
            .py(px(8.0))
            .rounded(px(6.0))
            .bg(rgba(0x00000030u32))
            .border(px(1.0))
            .border_color(rgba(UiConstants::STROKE_RGBA))
            .overflow_hidden()
            .child(div().flex_grow().flex().flex_row().items_end().child(bars))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_end()
                    .justify_between()
                    .h_full()
                    .flex_shrink_0()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .items_end()
                            .child(
                                div()
                                    .text_size(px(10.0))
                                    .text_color(rgb(UiConstants::TEXT_SECONDARY_RGB))
                                    .child("Speed:"),
                            )
                            .child(
                                div()
                                    .text_size(px(13.0))
                                    .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                                    .font_weight(FontWeight(600.0))
                                    .child(if paused {
                                        "Paused".to_string()
                                    } else if speed > 0.5 {
                                        format!("{}/s", Self::fmt_bytes(speed as u64))
                                    } else {
                                        "—".to_string()
                                    }),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(10.5))
                            .font_weight(FontWeight(600.0))
                            .text_color(rgba(0x60CDFFFFu32))
                            .child(format!(
                                "{:.0}%",
                                (job.progress_fraction() * 100.0).round()
                            )),
                    ),
            )
    }

    fn fmt_bytes(n: u64) -> String {
        const U: [(&str, f64); 5] = [
            ("B", 1.0),
            ("KB", 1024.0),
            ("MB", 1024.0 * 1024.0),
            ("GB", 1024.0 * 1024.0 * 1024.0),
            ("TB", 1024.0 * 1024.0 * 1024.0 * 1024.0),
        ];
        let mut v = n as f64;
        let mut u = "B";
        for (name, size) in U.iter() {
            if v >= *size {
                u = name;
            }
        }
        let div = U.iter().find(|(name, _)| *name == u).map(|(_, s)| *s).unwrap_or(1.0);
        v /= div;
        if u == "B" {
            format!("{} {}", n, u)
        } else {
            format!("{:.1} {}", v, u)
        }
    }

    fn build_dynamic_dialog(
        title: &str,
        message: &str,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let v_ok = view.clone();

        div()
            .flex_col()
            .gap(px(16.0))
            .child(
                div()
                    .text_size(AbsoluteLength::Pixels(px(18.0)))
                    .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                    .font_weight(FontWeight(600.0))
                    .child(title.to_string()),
            )
            .child(
                div()
                    .text_size(AbsoluteLength::Pixels(px(14.0)))
                    .text_color(rgb(UiConstants::TEXT_SECONDARY_RGB))
                    .child(message.to_string()),
            )
            // Buttons
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap(px(8.0))
                    .mt(px(8.0))
                    .child(Self::build_button("OK", true, move |_, _, cx| {
                        v_ok.update(cx, |this, cx| this.close_dialog(cx));
                    })),
            )
    }

    fn build_item_properties_dialog(
        item: &crate::domain::metadata::FileItem,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        use crate::storage::storage::FileType;
        use crate::ui::views::helpers::format_size;

        let v_ok = view.clone();
        let kind = if matches!(item.file_type, FileType::Directory) {
            "Folder"
        } else {
            "File"
        };
        let rows: Vec<(String, String)> = vec![
            ("Name".to_string(), item.name.clone()),
            ("Type".to_string(), kind.to_string()),
            ("Size".to_string(), format_size(item.size)),
            (
                "Hidden".to_string(),
                if item.is_hidden { "Yes" } else { "No" }.to_string(),
            ),
            (
                "Location".to_string(),
                item.path.to_string_lossy().into_owned(),
            ),
        ];

        div()
            .flex_col()
            .gap(px(12.0))
            .child(
                div()
                    .text_size(AbsoluteLength::Pixels(px(18.0)))
                    .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                    .font_weight(FontWeight(600.0))
                    .child("Properties"),
            )
            .children(rows.into_iter().map(|(label, value)| {
                div()
                    .flex()
                    .gap(px(12.0))
                    .child(
                        div()
                            .w(px(90.0))
                            .flex_shrink_0()
                            .text_size(AbsoluteLength::Pixels(px(13.0)))
                            .text_color(rgb(UiConstants::TEXT_SECONDARY_RGB))
                            .child(label),
                    )
                    .child(
                        div()
                            .text_size(AbsoluteLength::Pixels(px(13.0)))
                            .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                            .child(value),
                    )
            }))
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap(px(8.0))
                    .mt(px(8.0))
                    .child(Self::build_button("OK", true, move |_, _, cx| {
                        v_ok.update(cx, |this, cx| this.close_dialog(cx));
                    })),
            )
    }

    fn build_bulk_rename_dialog(
        indices: &[usize],
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let v_cancel = view.clone();
        let count = indices.len();

        div()
            .flex_col()
            .gap(px(16.0))
            .child(
                div()
                    .text_size(AbsoluteLength::Pixels(px(18.0)))
                    .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                    .font_weight(FontWeight(600.0))
                    .child(format!("Bulk Rename ({} items)", count)),
            )
            .child(
                div()
                    .flex_col()
                    .gap(px(8.0))
                    .child(Self::build_rename_field("Find", "text to find"))
                    .child(Self::build_rename_field("Replace", "replacement text")),
            )
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap(px(8.0))
                    .child(Self::build_button("Cancel", false, {
                        let v = v_cancel.clone();
                        move |_, _, cx| {
                            v.update(cx, |this, cx| this.close_dialog(cx));
                        }
                    }))
                    .child(Self::build_button("Rename All", true, {
                        let v = v_cancel.clone();
                        move |_, _, cx| {
                            v.update(cx, |this, cx| this.close_dialog(cx));
                        }
                    })),
            )
    }

    fn build_rename_field(label: &str, placeholder: &str) -> impl IntoElement {
        div()
            .flex_col()
            .gap(px(4.0))
            .child(
                div()
                    .text_size(AbsoluteLength::Pixels(px(12.0)))
                    .text_color(rgb(UiConstants::TEXT_SECONDARY_RGB))
                    .child(label.to_string()),
            )
            .child(
                div()
                    .h(px(32.0))
                    .bg(rgba(0xFFFFFF10u32))
                    .border(px(1.0))
                    .border_color(rgba(UiConstants::STROKE_RGBA))
                    .rounded(px(4.0))
                    .px(px(8.0))
                    .flex()
                    .items_center()
                    .child(
                        div()
                            .text_color(rgb(UiConstants::TEXT_MUTED_RGB))
                            .child(placeholder.to_string()),
                    ),
            )
    }

    pub(crate) fn build_settings_row(label: &str, value: &str) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .justify_between()
            .py(px(4.0))
            .border_b_1()
            .border_color(rgba(UiConstants::DIVIDER_RGBA))
            .child(
                div()
                    .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                    .child(label.to_string()),
            )
            .child(
                div()
                    .text_color(rgb(UiConstants::TEXT_MUTED_RGB))
                    .child(value.to_string()),
            )
    }

    fn build_command_palette_dialog(view: Entity<crate::app::AppWindow>) -> impl IntoElement {
        let v_close = view.clone();

        div()
            .flex_col()
            .gap(px(8.0))
            .child(
                // Search box
                div()
                    .flex()
                    .items_center()
                    .h(px(40.0))
                    .bg(rgba(0xFFFFFF10u32))
                    .border(px(1.0))
                    .border_color(rgba(UiConstants::STROKE_RGBA))
                    .rounded(px(6.0))
                    .px(px(12.0))
                    .child(
                        div()
                            .text_color(rgb(UiConstants::TEXT_MUTED_RGB))
                            .child("Search commands..."),
                    ),
            )
            .child(
                // Command list
                div()
                    .flex_col()
                    .max_h(px(300.0))
                    // .overflow_y_scroll()
                    .child(Self::build_command_item("Navigate: Home", "Alt+Home"))
                    .child(Self::build_command_item("View: Details", "Ctrl+1"))
                    .child(Self::build_command_item("View: Grid", "Ctrl+2"))
                    .child(Self::build_command_item("File: New Folder", "Ctrl+Shift+N"))
                    .child(Self::build_command_item("File: New File", "Ctrl+N"))
                    .child(Self::build_command_item(
                        "Window: New Window",
                        "Ctrl+Shift+W",
                    )),
            )
    }

    fn build_command_item(label: &str, shortcut: &str) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .justify_between()
            .px(px(8.0))
            .py(px(8.0))
            .rounded(px(4.0))
            .hover(|s| s.bg(rgba(0xFFFFFF15u32)))
            .cursor_pointer()
            .child(
                div()
                    .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                    .child(label.to_string()),
            )
            .child(
                div()
                    .text_color(rgb(UiConstants::TEXT_MUTED_RGB))
                    .text_size(AbsoluteLength::Pixels(px(12.0)))
                    .child(shortcut.to_string()),
            )
    }

    pub(crate) fn build_button(
        label: &str,
        primary: bool,
        on_click: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    ) -> impl IntoElement {
        let mut btn = div()
            .min_w(px(84.0))
            .h(px(32.0))
            .px(px(16.0))
            .rounded(px(4.0))
            .cursor_pointer()
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(13.0))
            .font_weight(FontWeight(500.0))
            .on_mouse_down(MouseButton::Left, on_click);

        if primary {
            btn = btn
                .bg(rgb(0x0078D4))
                .text_color(rgb(0xFFFFFF))
                .hover(|s| s.bg(rgb(0x1084E3)));
        } else {
            btn = btn
                .bg(rgba(0xFFFFFF10u32))
                .border(px(1.0))
                .border_color(rgba(UiConstants::STROKE_RGBA))
                .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                .hover(|s| s.bg(rgba(0xFFFFFF1Au32)));
        }
        btn.child(label.to_string())
    }

    /// Build zoom slider / view mode switcher popover (WinUI 3 Fluent + File Pilot inspired)
    fn build_zoom_slider(
        current_zoom: f32,
        current_mode: crate::config::config::ViewMode,
        is_locked: bool,
        view: Entity<crate::app::AppWindow>,
        _cx: &mut Context<crate::app::AppWindow>,
    ) -> impl IntoElement {
        use crate::config::config::ViewMode;

        struct MenuItem {
            name: &'static str,
            icon: &'static str,
            mode: ViewMode,
            is_active: bool,
            divider_after: bool,
        }

        let items = vec![
            MenuItem {
                name: "Icons",
                icon: "\u{E80A}",
                mode: ViewMode::Grid,
                is_active: matches!(current_mode, ViewMode::Grid),
                divider_after: false,
            },
            MenuItem {
                name: "Cards",
                icon: "\u{E8A9}",
                mode: ViewMode::Cards,
                is_active: matches!(current_mode, ViewMode::Cards),
                divider_after: false,
            },
            MenuItem {
                name: "Columns",
                icon: "\u{E8F1}",
                mode: ViewMode::Columns,
                is_active: matches!(current_mode, ViewMode::Columns),
                divider_after: false,
            },
            MenuItem {
                name: "List",
                icon: "\u{EA37}",
                mode: ViewMode::List,
                is_active: matches!(current_mode, ViewMode::List),
                divider_after: false,
            },
            MenuItem {
                name: "Details",
                icon: "\u{E8FD}",
                mode: ViewMode::Details,
                is_active: matches!(current_mode, ViewMode::Details),
                divider_after: false,
            },
        ];

        // Fixed dialog height for 5 view mode items
        let dialog_ht = 168.0;
        let pad_y = 8.0;
        let inner_ht = dialog_ht - pad_y * 2.0; // 152.0
        let track_h = 136.0;
        let thumb_travel = track_h - 16.0; // 120.0
        let clamped_zoom = current_zoom.clamp(0.0, 1.0);
        let thumb_pos_ratio = 1.0 - clamped_zoom;
        let v_wheel = view.clone();

        div()
            .id("zoom-slider-flyout")
            .flex()
            .flex_row()
            .w_full()
            .h(px(dialog_ht))
            .p(px(pad_y))
            .gap(px(6.0))
            .on_scroll_wheel(move |event, _win, cx| {
                cx.stop_propagation();
                let delta = event.delta.pixel_delta(px(22.0));
                let delta_y: f32 = delta.y.into();
                if delta_y.abs() < 0.25 {
                    return;
                }
                let step = if delta_y > 0.0 { 0.05 } else { -0.05 };
                v_wheel.update(cx, |this, cx| {
                    let next = (this.state.main_view.zoom_level + step).clamp(0.0, 1.0);
                    this.set_zoom_level(next, cx);
                });
            })
            .child(
                // LEFT: 5 View mode items
                div()
                    .flex()
                    .flex_col()
                    .justify_between()
                    .flex_grow()
                    .h(px(inner_ht))
                    .children(items.into_iter().map(|item| {
                        let is_active = item.is_active;
                        let v_btn = view.clone();
                        let mode = item.mode;
                        let divider_after = item.divider_after;

                        div()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap(px(10.0))
                                    .px(px(8.0))
                                    .py(px(4.0))
                                    .rounded(px(5.0))
                                    .bg(if is_active {
                                        rgba(UiConstants::ACCENT_RGBA)
                                    } else {
                                        rgba(0x00000000u32)
                                    })
                                    .cursor_pointer()
                                    .hover(|s| {
                                        s.bg(if is_active {
                                            rgba(UiConstants::ACCENT_RGBA)
                                        } else {
                                            rgba(0xFFFFFF12u32)
                                        })
                                    })
                                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                        let mode = mode.clone();
                                        v_btn.update(cx, |this, cx| {
                                            this.set_view_mode(mode, cx);
                                        });
                                    })
                                    .child(
                                        div()
                                            .text_size(px(13.5))
                                            .font_family(UiConstants::FONT_ICONS)
                                            .text_color(if is_active {
                                                rgb(0xFFFFFF)
                                            } else {
                                                rgba(UiConstants::TEXT_SECONDARY_RGBA).into()
                                            })
                                            .child(item.icon),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(12.0))
                                            .font_weight(if is_active {
                                                FontWeight(600.0)
                                            } else {
                                                FontWeight(400.0)
                                            })
                                            .text_color(if is_active {
                                                rgb(0xFFFFFF)
                                            } else {
                                                rgba(UiConstants::TEXT_PRIMARY_RGBA).into()
                                            })
                                            .child(item.name),
                                    ),
                            )
                            .children(if divider_after {
                                Some(
                                    div()
                                        .h(px(1.0))
                                        .bg(rgba(0xFFFFFF10u32))
                                        .mx(px(6.0))
                                        .my(px(1.0)),
                                )
                            } else {
                                None
                            })
                    })),
            )
            .child(
                // RIGHT: Vertical slider — track + vibrant WinUI 3 blue active fill bar + circular thumb
                div()
                    .relative()
                    .w(px(28.0))
                    .h(px(inner_ht))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .on_mouse_down(MouseButton::Left, {
                        let v = view.clone();
                        move |event, win, cx| {
                            let click_y: f32 = event.position.y.into();
                            let win_h: f32 = win.viewport_size().height.into();
                            let track_top = win_h - 48.0 - dialog_ht + 16.0;
                            let rel_y = click_y - track_top;
                            let thumb_top = (rel_y - 8.0).clamp(0.0, thumb_travel);
                            let snapped_zoom = (1.0 - thumb_top / thumb_travel).clamp(0.0, 1.0);
                            v.update(cx, |this, cx| {
                                this.set_zoom_level(snapped_zoom, cx);
                                this.zoom_drag_start = Some((click_y, snapped_zoom));
                            });
                        }
                    })
                    .child(
                        // Pill track (6px wide, centered)
                        div()
                            .relative()
                            .w(px(6.0))
                            .h(px(track_h))
                            .bg(rgba(0xFFFFFF25u32))
                            .rounded(px(3.0))
                            .overflow_hidden()
                            .child(
                                // Vibrant solid blue active fill bar filling from bottom up
                                div()
                                    .absolute()
                                    .bottom_0()
                                    .left_0()
                                    .right_0()
                                    .h(px((clamped_zoom * thumb_travel + 8.0).min(track_h)))
                                    .rounded(px(3.0))
                                    .bg(rgb(0x0078D4)),
                            ),
                    )
                    .child(
                        // Circular accent thumb (16px) sitting directly over the track
                        div()
                            .absolute()
                            .top(px(thumb_pos_ratio * thumb_travel + 8.0))
                            .left(px(6.0))
                            .w(px(16.0))
                            .h(px(16.0))
                            .rounded(px(8.0))
                            .bg(rgb(0x0078D4))
                            .border(px(2.0))
                            .border_color(rgb(0xFFFFFF))
                            .shadow_md()
                            .cursor_pointer(),
                    ),
            )
    }

    /// Build toolbar customization dialog for managing toolbar buttons
    fn build_toolbar_customization_dialog(view: Entity<crate::app::AppWindow>) -> impl IntoElement {
        // Use separate clones for each closure to avoid move errors
        let v_close1 = view.clone();
        let v_close2 = view.clone();
        let v_close3 = view.clone();
        let v_reset = view.clone();

        // Define available toolbar buttons
        let available_buttons = vec![
            ("New", "\u{E710}", true),
            ("Cut", "\u{E8C6}", true),
            ("Copy", "\u{E8C8}", true),
            ("Paste", "\u{E77F}", true),
            ("Delete", "\u{E74D}", true),
            ("Rename", "\u{E8AC}", true),
            ("Search", "\u{E721}", true),
            ("View Toggle", "\u{E8A9}", true),
            ("Sort", "\u{E8DE}", true),
            ("Select All", "\u{E8B3}", true),
        ];

        // Define shown buttons (current toolbar state)
        let shown_buttons = vec![
            ("New", "\u{E710}"),
            ("Cut", "\u{E8C6}"),
            ("Copy", "\u{E8C8}"),
            ("Paste", "\u{E77F}"),
            ("View Toggle", "\u{E8A9}"),
        ];

        div()
            .flex_col()
            .gap(px(16.0))
            .w(px(450.0))
            // Header
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(AbsoluteLength::Pixels(px(18.0)))
                            .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                            .font_weight(FontWeight(600.0))
                            .child("Customize Toolbar"),
                    )
                    .child(
                        div()
                            .cursor_pointer()
                            .text_color(rgb(UiConstants::TEXT_MUTED_RGB))
                            .hover(|s| s.text_color(rgb(UiConstants::TEXT_PRIMARY_RGB)))
                            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                v_close1.update(cx, |this, cx| this.close_dialog(cx));
                            })
                            .child("\u{E8BB}"),
                    ),
            )
            // Description
            .child(
                div()
                    .text_size(AbsoluteLength::Pixels(px(13.0)))
                    .text_color(rgb(UiConstants::TEXT_SECONDARY_RGB))
                    .child("Drag items to reorder. Click to show or hide."),
            )
            // Current toolbar preview
            .child(
                div()
                    .flex_col()
                    .gap(px(8.0))
                    .child(
                        div()
                            .text_size(AbsoluteLength::Pixels(px(12.0)))
                            .text_color(rgb(UiConstants::TEXT_MUTED_RGB))
                            .mb(px(4.0))
                            .child("CURRENT TOOLBAR"),
                    )
                    .child(
                        div()
                            .flex()
                            .gap(px(4.0))
                            .p(px(8.0))
                            .bg(rgba(0xFFFFFF10u32))
                            .rounded(px(6.0))
                            .border(px(1.0))
                            .border_color(rgba(UiConstants::DIVIDER_RGBA))
                            .children(shown_buttons.iter().map(|(name, icon)| {
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(6.0))
                                    .px(px(10.0))
                                    .py(px(6.0))
                                    .bg(rgba(0xFFFFFF15u32))
                                    .rounded(px(4.0))
                                    .text_color(rgb(UiConstants::ACCENT_COLOR_RGB))
                                    .child(*icon)
                                    .child(
                                        div()
                                            .text_size(px(13.0))
                                            .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                                            .child(*name),
                                    )
                            })),
                    ),
            )
            // Available buttons
            .child(
                div()
                    .flex_col()
                    .gap(px(8.0))
                    .child(
                        div()
                            .text_size(AbsoluteLength::Pixels(px(12.0)))
                            .text_color(rgb(UiConstants::TEXT_MUTED_RGB))
                            .mb(px(4.0))
                            .child("AVAILABLE BUTTONS"),
                    )
                    .child(
                        div()
                            .flex_col()
                            .gap(px(2.0))
                            .p(px(4.0))
                            .bg(rgba(0xFFFFFF08u32))
                            .rounded(px(6.0))
                            .border(px(1.0))
                            .border_color(rgba(UiConstants::DIVIDER_RGBA))
                            .children(
                                available_buttons
                                    .iter()
                                    .filter(|(_, _, available)| *available)
                                    .map(|(name, icon, _)| {
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap(px(8.0))
                                            .px(px(8.0))
                                            .py(px(6.0))
                                            .rounded(px(4.0))
                                            .hover(|s| s.bg(rgba(0xFFFFFF10u32)))
                                            .cursor_pointer()
                                            .text_size(px(14.0))
                                            .text_color(rgb(UiConstants::ACCENT_COLOR_RGB))
                                            .child(*icon)
                                            .child(
                                                div()
                                                    .text_size(px(13.0))
                                                    .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                                                    .child(*name),
                                            )
                                    }),
                            ),
                    ),
            )
            // Buttons
            .child(
                div()
                    .flex()
                    .justify_between()
                    .mt(px(8.0))
                    .child(
                        div()
                            .text_size(px(13.0))
                            .text_color(rgb(UiConstants::ACCENT_COLOR_RGB))
                            .cursor_pointer()
                            .hover(|s| s.text_color(rgb(UiConstants::TEXT_PRIMARY_RGB)))
                            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                v_reset.update(cx, |this, cx| {
                                    // Reset toolbar to default
                                    this.close_dialog(cx);
                                });
                            })
                            .child("Reset to Defaults"),
                    )
                    .child(
                        div()
                            .flex()
                            .gap(px(8.0))
                            .child(Self::build_button("Cancel", false, move |_, _, cx| {
                                v_close2.update(cx, |this, cx| this.close_dialog(cx));
                            }))
                            .child(Self::build_button("Done", true, move |_, _, cx| {
                                v_close3.update(cx, |this, cx| this.close_dialog(cx));
                            })),
                    ),
            )
    }

    /// Build reorder sidebar items dialog for managing sidebar item order
    fn build_reorder_sidebar_items_dialog(view: Entity<crate::app::AppWindow>) -> impl IntoElement {
        // Use separate clones for each closure to avoid move errors
        let v_close1 = view.clone();
        let v_close2 = view.clone();
        let v_close3 = view.clone();
        let v_up = view.clone();
        let v_down = view.clone();
        let v_remove = view.clone();

        // Mock sidebar items for demonstration
        let items = vec![
            ("Quick Access", "\u{E8B7}"),
            ("Desktop", "\u{E8FC}"),
            ("Documents", "\u{E8B5}"),
            ("Downloads", "\u{E896}"),
            ("Pictures", "\u{E91B}"),
            ("This PC", "\u{E8EE}"),
        ];

        div()
            .flex_col()
            .gap(px(16.0))
            .w(px(450.0))
            // Header
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(AbsoluteLength::Pixels(px(18.0)))
                            .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                            .font_weight(FontWeight(600.0))
                            .child("Reorder Sidebar Items"),
                    )
                    .child(
                        div()
                            .text_size(px(16.0))
                            .text_color(rgb(UiConstants::TEXT_MUTED_RGB))
                            .cursor_pointer()
                            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                v_close1.update(cx, |this, cx| this.close_dialog(cx));
                            })
                            .child("\u{E8BB}"),
                    ),
            )
            // Instructions
            .child(
                div()
                    .text_size(AbsoluteLength::Pixels(px(12.0)))
                    .text_color(rgb(UiConstants::TEXT_SECONDARY_RGB))
                    .child("Drag items to reorder or use the buttons below"),
            )
            // Items list
            .child(
                div()
                    .flex_col()
                    .gap(px(4.0))
                    .p(px(8.0))
                    .bg(rgba(0x0AFFFFFFu32))
                    .rounded(px(6.0))
                    .children(items.iter().enumerate().map(|(idx, (name, icon))| {
                        let is_first = idx == 0;
                        let is_last = idx == items.len() - 1;
                        let item_idx = idx;
                        let v_up_clone = v_up.clone();
                        let v_down_clone = v_down.clone();
                        let v_remove_clone = v_remove.clone();
                        let v_close_clone = v_close3.clone();

                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.0))
                            .px(px(8.0))
                            .py(px(6.0))
                            .rounded(px(4.0))
                            .hover(|s| s.bg(rgba(0xFFFFFF10u32)))
                            .child(
                                div()
                                    .text_size(px(14.0))
                                    .text_color(rgb(UiConstants::ACCENT_COLOR_RGB))
                                    .child(*icon),
                            )
                            .child(
                                div()
                                    .flex_grow()
                                    .text_size(AbsoluteLength::Pixels(px(14.0)))
                                    .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                                    .child(*name),
                            )
                            .child(
                                div()
                                    .flex()
                                    .gap(px(4.0))
                                    .child(
                                        div()
                                            .px(px(6.0))
                                            .py(px(2.0))
                                            .text_size(px(12.0))
                                            .text_color(if is_first {
                                                rgb(UiConstants::TEXT_MUTED_RGB)
                                            } else {
                                                rgb(UiConstants::ACCENT_COLOR_RGB)
                                            })
                                            .cursor_pointer()
                                            .hover(|s| {
                                                if !is_first {
                                                    s.text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                                                } else {
                                                    s
                                                }
                                            })
                                            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                                if !is_first {
                                                    v_up_clone.update(cx, |this, cx| {
                                                        // Reorder logic would go here
                                                        this.close_dialog(cx);
                                                    });
                                                }
                                            })
                                            .child("\u{E70E}"), // Up arrow
                                    )
                                    .child(
                                        div()
                                            .px(px(6.0))
                                            .py(px(2.0))
                                            .text_size(px(12.0))
                                            .text_color(if is_last {
                                                rgb(UiConstants::TEXT_MUTED_RGB)
                                            } else {
                                                rgb(UiConstants::ACCENT_COLOR_RGB)
                                            })
                                            .cursor_pointer()
                                            .hover(|s| {
                                                if !is_last {
                                                    s.text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                                                } else {
                                                    s
                                                }
                                            })
                                            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                                if !is_last {
                                                    v_down_clone.update(cx, |this, cx| {
                                                        // Reorder logic would go here
                                                        this.close_dialog(cx);
                                                    });
                                                }
                                            })
                                            .child("\u{E70D}"), // Down arrow
                                    )
                                    .child(
                                        div()
                                            .px(px(6.0))
                                            .py(px(2.0))
                                            .text_size(px(12.0))
                                            .text_color(rgb(UiConstants::TEXT_MUTED_RGB))
                                            .cursor_pointer()
                                            .hover(|s| s.text_color(rgb(0xFF9999)))
                                            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                                v_remove_clone.update(cx, |this, cx| {
                                                    // Remove from sidebar logic would go here
                                                    this.close_dialog(cx);
                                                });
                                            })
                                            .child("\u{E74D}"), // Remove icon
                                    ),
                            )
                    })),
            )
            // Buttons
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap(px(8.0))
                    .child(Self::build_button("Cancel", false, move |_, _, cx| {
                        v_close2.update(cx, |this, cx| this.close_dialog(cx));
                    }))
                    .child(Self::build_button("Done", true, move |_, _, cx| {
                        v_close3.update(cx, |this, cx| this.close_dialog(cx));
                    })),
            )
    }

    // ═══════════════════════════════════════════════════════════════
    // BUILD: Custom Properties Dialog (Compatibility, Signatures tabs)
    // ═══════════════════════════════════════════════════════════════

    // ═══════════════════════════════════════════════════════════════
    // BUILD: Restore Crashed Session Dialog
    // ═══════════════════════════════════════════════════════════════
    fn build_restore_crashed_session_dialog(
        crash_count: u32,
        last_path: &str,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        use crate::app::AppWindow;
        let v_restore = view.clone();
        let v_new = view.clone();

        div()
            .flex_col()
            .w(px(460.0))
            .gap(px(16.0))
            // Title
            .child(
                div()
                    .text_size(px(18.0))
                    .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                    .font_weight(FontWeight(600.0))
                    .child("Session Crashed"),
            )
            // Message
            .child(
                div()
                    .text_size(px(13.0))
                    .text_color(rgb(UiConstants::TEXT_SECONDARY_RGB))
                    .child(format!(
                        "The previous session did not close properly (crash count: {}). \
                         You can restore your previous location or start fresh.",
                        crash_count
                    )),
            )
            // Last path info
            .child(
                div()
                    .flex_col()
                    .gap(px(4.0))
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(rgb(UiConstants::TEXT_MUTED_RGB))
                            .child("Last location:"),
                    )
                    .child(
                        div()
                            .text_size(px(13.0))
                            .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                            .child(if last_path.is_empty() {
                                "None".to_string()
                            } else {
                                last_path.to_string()
                            }),
                    ),
            )
            // Buttons
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap(px(8.0))
                    .child(Self::build_button("Start Fresh", false, move |_, _, cx| {
                        v_new.update(cx, |this, cx| {
                            this.close_dialog(cx);
                            cx.notify();
                        });
                    }))
                    .child(Self::build_button("Restore", true, move |_, _, cx| {
                        v_restore.update(cx, |this, cx| {
                            this.close_dialog(cx);
                            // Navigate to last path if available
                            let path_str = {
                                let session = &this.state.session_service;
                                session.last_path().to_string()
                            };
                            if !path_str.is_empty() {
                                let path = crate::core::path_util::FilesPath::new(&path_str);
                                this.navigate(path, cx);
                            }
                            cx.notify();
                        });
                    })),
            )
    }

    // ────────── Helper: Checkbox ──────────

    // ═══════════════════════════════════════════════════════════════
    // BUILD: Clone Repository Dialog
    // ═══════════════════════════════════════════════════════════════
    fn build_clone_repo_dialog(
        state: &crate::ui::dialog_services::clone_repo_dialog::CloneRepoState,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let v_cancel = view.clone();
        let v_clone = view.clone();

        div()
            .flex_col()
            .gap(px(16.0))
            .child(
                div()
                    .text_size(AbsoluteLength::Pixels(px(18.0)))
                    .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                    .font_weight(FontWeight(600.0))
                    .child("Clone Repository"),
            )
            .child(
                div()
                    .flex_col()
                    .gap(px(8.0))
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(rgb(UiConstants::TEXT_SECONDARY_RGB))
                            .child("Repository URL:"),
                    )
                    .child(
                        div()
                            .w_full()
                            .h(px(32.0))
                            .bg(rgba(0xFFFFFF10u32))
                            .border(px(1.0))
                            .border_color(rgba(UiConstants::STROKE_RGBA))
                            .rounded(px(4.0))
                            .px(px(8.0))
                            .flex()
                            .items_center()
                            .child(
                                div()
                                    .text_size(px(13.0))
                                    .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                                    .child(if state.url.is_empty() {
                                        "https://github.com/user/repo.git ...".to_string()
                                    } else {
                                        state.url.clone()
                                    }),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(rgb(UiConstants::TEXT_SECONDARY_RGB))
                            .child("Destination:"),
                    )
                    .child(
                        div()
                            .w_full()
                            .h(px(32.0))
                            .bg(rgba(0xFFFFFF10u32))
                            .border(px(1.0))
                            .border_color(rgba(UiConstants::STROKE_RGBA))
                            .rounded(px(4.0))
                            .px(px(8.0))
                            .flex()
                            .items_center()
                            .child(
                                div()
                                    .text_size(px(13.0))
                                    .text_color(match state.validate_destination() {
                                        Ok(_) => rgb(UiConstants::TEXT_PRIMARY_RGB),
                                        Err(_) => rgb(UiConstants::TEXT_SECONDARY_RGB),
                                    })
                                    .child(if state.destination_path.as_os_str().is_empty() {
                                        "Choose destination folder...".to_string()
                                    } else {
                                        state.destination_path.to_string_lossy().to_string()
                                    }),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap(px(8.0))
                    .mt(px(8.0))
                    .child(Self::build_button("Cancel", false, move |_, _, cx| {
                        v_cancel.update(cx, |this, cx| this.close_dialog(cx));
                    }))
                    .child(Self::build_button("Clone", true, move |_, _, cx| {
                        v_clone.update(cx, |this, cx| {
                            // Trigger clone action
                            this.git_clone(cx);
                        });
                    })),
            )
    }

    // ═══════════════════════════════════════════════════════════════
    // BUILD: Conflict Resolution Dialog
    // ═══════════════════════════════════════════════════════════════
    fn build_conflict_dialog(
        state: &crate::ui::dialog_services::conflict_dialog::ConflictDialogState,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let v_replace = view.clone();
        let v_skip = view.clone();
        let v_rename = view.clone();
        let v_toggle = view.clone();
        let v_cancel = view.clone();

        use crate::ui::dialog_services::conflict_dialog::ConflictResolution;

        div()
            .flex_col()
            .w(px(520.0))
            .bg(rgb(0x202020))
            .rounded(px(8.0))
            .border(px(1.0))
            .border_color(rgba(UiConstants::DIVIDER_RGBA))
            .p(px(16.0))
            .gap(px(12.0))
            // Title
            .child(
                div()
                    .text_size(px(18.0))
                    .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                    .font_weight(FontWeight(600.0))
                    .child("File Conflict"),
            )
            .child(
                div()
                    .text_size(px(13.0))
                    .text_color(rgb(UiConstants::TEXT_SECONDARY_RGB))
                    .child("This destination already has a file with the same name."),
            )
            // Side-by-side file info cards
            .child(
                div()
                    .flex()
                    .gap(px(12.0))
                    .child(Self::build_file_info_card(
                        "Source",
                        state.source_filename(),
                        state.source_size,
                        &state.source_modified,
                        state.source_created.as_deref(),
                    ))
                    .child(Self::build_file_info_card(
                        "Destination",
                        state.dest_filename(),
                        state.dest_size,
                        &state.dest_modified,
                        state.dest_created.as_deref(),
                    )),
            )
            // Action buttons
            .child(
                div()
                    .flex()
                    .gap(px(8.0))
                    .child(Self::build_button("Replace", true, move |_, _, cx| {
                        v_replace.update(cx, |this, cx| {
                            if let Some(crate::ui::components::dialogs::DialogType::Conflict {
                                ref mut state,
                            }) = this.state.active_dialog
                            {
                                state.choose(ConflictResolution::Replace);
                                cx.notify();
                            }
                        });
                    }))
                    .child(Self::build_button("Skip", false, move |_, _, cx| {
                        v_skip.update(cx, |this, cx| {
                            if let Some(crate::ui::components::dialogs::DialogType::Conflict {
                                ref mut state,
                            }) = this.state.active_dialog
                            {
                                state.choose(ConflictResolution::Skip);
                                cx.notify();
                            }
                        });
                    }))
                    .child(Self::build_button("Rename", false, move |_, _, cx| {
                        v_rename.update(cx, |this, cx| {
                            if let Some(crate::ui::components::dialogs::DialogType::Conflict {
                                ref mut state,
                            }) = this.state.active_dialog
                            {
                                state.choose(ConflictResolution::Rename);
                                cx.notify();
                            }
                        });
                    })),
            )
            // "Apply to all" checkbox
            .child(Self::properties_checkbox(
                "Apply to all conflicts",
                state.apply_to_all,
                {
                    let v = v_toggle.clone();
                    move |_, _, cx| {
                        v.update(cx, |this, cx| {
                            if let Some(crate::ui::components::dialogs::DialogType::Conflict {
                                ref mut state,
                            }) = this.state.active_dialog
                            {
                                state.toggle_apply_to_all();
                                cx.notify();
                            }
                        });
                    }
                },
            ))
            // Cancel button
            .child(
                div()
                    .flex()
                    .justify_end()
                    .mt(px(4.0))
                    .child(Self::build_button("Cancel", false, move |_, _, cx| {
                        v_cancel.update(cx, |this, cx| this.close_dialog(cx));
                    })),
            )
    }

    /// Build a file info card for conflict dialog (source or destination)
    fn build_file_info_card(
        label: &str,
        filename: &str,
        size: u64,
        modified: &str,
        created: Option<&str>,
    ) -> impl IntoElement {
        div()
            .flex_col()
            .flex_grow()
            .p(px(12.0))
            .bg(rgba(0xFFFFFF10u32))
            .border(px(1.0))
            .border_color(rgba(UiConstants::STROKE_RGBA))
            .rounded(px(6.0))
            .gap(px(6.0))
            .child(
                div()
                    .text_size(px(14.0))
                    .text_color(rgb(UiConstants::ACCENT_COLOR_RGB))
                    .font_weight(FontWeight(600.0))
                    .child(label.to_string()),
            )
            .child(
                div()
                    .flex_col()
                    .gap(px(4.0))
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(rgb(UiConstants::TEXT_MUTED_RGB))
                            .child("Name:"),
                    )
                    .child(
                        div()
                            .text_size(px(13.0))
                            .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                            .child(filename.to_string()),
                    )
                    .child(
                        div()
                            .mt(px(4.0))
                            .text_size(px(12.0))
                            .text_color(rgb(UiConstants::TEXT_MUTED_RGB))
                            .child("Size:"),
                    )
                    .child(
                        div()
                            .text_size(px(13.0))
                            .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                            .child(crate::ui::dialog_services::conflict_dialog::ConflictDialogState::format_size(size)),
                    )
                    .child(
                        div()
                            .mt(px(4.0))
                            .text_size(px(12.0))
                            .text_color(rgb(UiConstants::TEXT_MUTED_RGB))
                            .child("Modified:"),
                    )
                    .child(
                        div()
                            .text_size(px(13.0))
                            .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                            .child(modified.to_string()),
                    )
                    .child(if let Some(created) = created {
                        div()
                            .child(
                                div()
                                    .mt(px(4.0))
                                    .text_size(px(12.0))
                                    .text_color(rgb(UiConstants::TEXT_MUTED_RGB))
                                    .child("Created:"),
                            )
                            .child(
                                div()
                                    .text_size(px(13.0))
                                    .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                                    .child(created.to_string()),
                            )
                    } else {
                        div()
                    }),
            )
    }
}

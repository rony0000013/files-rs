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

#[derive(Debug, Clone, PartialEq)]
pub enum DialogType {
    Rename {
        item_index: usize,
        current_name: String,
    },
    AddItem {
        is_folder: bool,
        name: String,
    },
    Dynamic {
        title: String,
        message: String,
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
    /// Status Center showing active operations
    StatusCenter {
        active_operations: Vec<OperationInfo>,
        position: Option<Point<Pixels>>,
    },
    /// Tab context menu (shown on right-click of tab)
    TabContextMenu {
        tab_index: usize,
        position: Point<Pixels>,
        tab_count: usize,
    },
    /// Tab actions dropdown (shown on click of tab actions button)
    TabActionsDropdown {
        position: Point<Pixels>,
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

/// Info about an in-progress operation for StatusCenter
#[derive(Debug, Clone, PartialEq)]
pub struct OperationInfo {
    pub name: String,
    pub progress: Option<f32>,
    pub status: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    pub toolbar_new_folder: bool,
    pub toolbar_new_file: bool,
    pub toolbar_cut: bool,
    pub toolbar_copy: bool,
    pub toolbar_paste: bool,
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
        let tb = &state.main_view.toolbar_customization;
        SettingsPreReadData {
            active_tab: state.active_settings_tab,
            config,
            action_pairs,
            toolbar_new_folder: tb.is_visible("new_folder"),
            toolbar_new_file: tb.is_visible("new_file"),
            toolbar_cut: tb.is_visible("cut"),
            toolbar_copy: tb.is_visible("copy"),
            toolbar_paste: tb.is_visible("paste"),
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
    ) -> impl IntoElement {
        let content = match dialog {
            DialogType::Rename {
                item_index,
                current_name,
            } => Self::build_rename_dialog(*item_index, current_name, view.clone())
                .into_any_element(),
            DialogType::AddItem { is_folder, name } => {
                Self::build_add_item_dialog(*is_folder, name, view.clone()).into_any_element()
            }
            DialogType::SiblingDropdown {
                parent_path,
                siblings,
            } => Self::build_sibling_dropdown(parent_path, siblings, view.clone(), cx)
                .into_any_element(),
            DialogType::StatusCenter {
                active_operations, ..
            } => Self::build_status_center(active_operations, view.clone()).into_any_element(),
            DialogType::Dynamic { title, message } => {
                Self::build_dynamic_dialog(title, message, view.clone()).into_any_element()
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
            } => Self::build_context_menu_dialog(
                *item_index,
                *position,
                *has_selection,
                selected_path.clone(),
                *selected_count,
                *is_dir,
                view.clone(),
                cx,
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
            } => Self::build_tab_context_menu(*tab_index, *position, *tab_count, view.clone())
                .into_any_element(),
            DialogType::TabActionsDropdown { position } => {
                Self::build_tab_actions_dropdown(*position, view.clone()).into_any_element()
            }
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
                ..
            } => Self::build_zoom_slider(*current_zoom, *current_mode, view.clone(), cx)
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
            // Subtle backdrop for modal feel
            overlay = overlay.bg(rgba(0x00000020u32));
            let thumb_travel_total = 140.0; // matches dialog_ht - 20 - 20 = track thumb travel
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
            overlay = overlay
                .on_mouse_down(MouseButton::Left, {
                    let v = v_overlay_close.clone();
                    move |_, _, cx| {
                        v.update(cx, |this, cx| {
                            this.stop_hover_timer(cx);
                            this.close_dialog(cx);
                        });
                    }
                })
                .on_mouse_move(|_, _, cx| cx.stop_propagation());
        }

        let mut container = div()
            .bg(rgb(0x202020))
            .border(px(1.0))
            .border_color(rgba(UiConstants::DIVIDER_RGBA))
            .rounded(px(8.0))
            .shadow_lg()
            // Stop clicks on the dialog itself from bubbling up to the overlay and closing it
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(content);

        // For ZoomSlider, let mouse_move pass through so the drag handler can receive it.
        if !is_zoom {
            container = container.on_mouse_move(|_, _, cx| cx.stop_propagation());
        }

        match dialog {
            DialogType::ContextMenu { position, .. }
            | DialogType::TabContextMenu { position, .. }
            | DialogType::TabActionsDropdown { position }
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
                // Anchor to bottom-right above the status bar
                container = container
                    .absolute()
                    .bottom(px(40.0))
                    .right(px(8.0))
                    .w(px(220.0));
            }
            DialogType::StatusCenter { position, .. } => {
                let pos = position.unwrap_or(Point {
                    x: px(0.0), // Caller should provide position; fallback to left edge
                    y: px(52.0),
                });
                container = container
                    .absolute()
                    .top(pos.y)
                    .left(pos.x)
                    .w(px(400.0))
                    .p(px(UiConstants::PADDING_LARGE_RAW as f32));
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
            .gap(px(16.0))
            .child(
                div()
                    .text_size(AbsoluteLength::Pixels(px(18.0)))
                    .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                    .font_weight(FontWeight(600.0))
                    .child("Rename Item"),
            )
            .child(
                div()
                    .text_size(AbsoluteLength::Pixels(px(14.0)))
                    .text_color(rgb(UiConstants::TEXT_SECONDARY_RGB))
                    .child("Enter a new name:"),
            )
            // Mock input box
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
                            .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                            .child(current_name.to_string()),
                    ),
            )
            // Buttons
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap(px(8.0))
                    .mt(px(8.0))
                    .child(Self::build_button("Cancel", false, move |_, _, cx| {
                        v_cancel.update(cx, |this, cx| this.close_dialog(cx));
                    }))
                    .child(Self::build_button("Rename", true, move |_, _, cx| {
                        // Normally we would get text from input and call rename service
                        v_ok.update(cx, |this, cx| this.close_dialog(cx));
                    })),
            )
    }

    fn build_add_item_dialog(
        is_folder: bool,
        name: &str,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let v_cancel = view.clone();
        let v_ok = view.clone();
        let title = if is_folder { "New Folder" } else { "New File" };
        let name_display = if name.is_empty() {
            "Name...".to_string()
        } else {
            name.to_string()
        };

        div()
            .flex_col()
            .gap(px(16.0))
            .child(
                div()
                    .text_size(AbsoluteLength::Pixels(px(18.0)))
                    .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                    .font_weight(FontWeight(600.0))
                    .child(title),
            )
            .child(
                div()
                    .text_size(AbsoluteLength::Pixels(px(14.0)))
                    .text_color(rgb(UiConstants::TEXT_SECONDARY_RGB))
                    .child("Enter a name for the new item:"),
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
                            .text_color(rgb(UiConstants::TEXT_SECONDARY_RGB))
                            .child(name_display),
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
                    .child(Self::build_button("Create", true, move |_, _, cx| {
                        v_ok.update(cx, |this, cx| this.confirm_add_item(cx));
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
        let v_cancel = view.clone();
        let v_ok = view.clone();
        let archive_name = state
            .archive_path
            .file_name()
            .map_or("Unknown", |s| s.to_str().unwrap_or("Unknown"));

        div()
            .flex_col()
            .gap(px(16.0))
            .child(
                div()
                    .text_size(AbsoluteLength::Pixels(px(18.0)))
                    .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                    .font_weight(FontWeight(600.0))
                    .child("Extract Archive"),
            )
            .child(
                div()
                    .flex_col()
                    .gap(px(8.0))
                    .child(Self::build_settings_row("Archive", archive_name))
                    .child(Self::build_settings_row(
                        "Selected Items",
                        &format!("{} items", state.selected_count()),
                    ))
                    .child(Self::build_settings_row(
                        "Size",
                        &crate::ui::components::status_bar::format_size(state.selected_size()),
                    )),
            )
            .child(
                div()
                    .text_size(AbsoluteLength::Pixels(px(12.0)))
                    .text_color(rgb(UiConstants::TEXT_MUTED_RGB))
                    .child(format!("Destination: {}", state.destination.display())),
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
                    .child(Self::build_button("Extract", true, move |_, _, cx| {
                        v_ok.update(cx, |this, cx| this.close_dialog(cx));
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

    fn build_status_center(
        operations: &[OperationInfo],
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let v_close = view.clone();

        let content = if operations.is_empty() {
            div()
                .flex_col()
                .gap(px(12.0))
                .items_center()
                .py(px(24.0))
                .child(
                    div()
                        .text_size(px(32.0))
                        .text_color(rgba(UiConstants::TEXT_MUTED_RGBA))
                        .child("\u{E9D9}"),
                )
                .child(
                    div()
                        .text_size(AbsoluteLength::Pixels(px(14.0)))
                        .text_color(rgb(UiConstants::TEXT_SECONDARY_RGB))
                        .child("No active operations"),
                )
        } else {
            div()
                .flex_col()
                .gap(px(8.0))
                .children(operations.iter().map(|op| {
                    div()
                        .flex_col()
                        .gap(px(4.0))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .child(
                                    div()
                                        .text_size(AbsoluteLength::Pixels(px(13.0)))
                                        .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                                        .child(op.name.clone()),
                                )
                                .child(
                                    div()
                                        .text_size(AbsoluteLength::Pixels(px(12.0)))
                                        .text_color(rgb(UiConstants::TEXT_MUTED_RGB))
                                        .child(op.status.clone()),
                                ),
                        )
                        .child(
                            div()
                                .h(px(4.0))
                                .bg(rgba(0xFFFFFF30u32))
                                .rounded(px(2.0))
                                .child(
                                    div()
                                        .h(px(4.0))
                                        .bg(rgba(UiConstants::ACCENT_RGBA))
                                        .rounded(px(2.0))
                                        .w(px(100.0)),
                                ),
                        )
                }))
        };

        div()
            .flex_col()
            .gap(px(16.0))
            .w(px(400.0))
            .child(
                div()
                    .text_size(AbsoluteLength::Pixels(px(18.0)))
                    .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                    .font_weight(FontWeight(600.0))
                    .child("Status Center"),
            )
            .child(content)
            .child(div().flex().justify_end().child(Self::build_button(
                "Close",
                false,
                move |_, _, cx| {
                    v_close.update(cx, |this, cx| this.close_dialog(cx));
                },
            )))
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
            .px(px(16.0))
            .py(px(6.0))
            .rounded(px(4.0))
            .cursor_pointer()
            .flex()
            .items_center()
            .justify_center()
            .on_mouse_down(MouseButton::Left, on_click);

        if primary {
            btn = btn
                .bg(rgba(UiConstants::ACCENT_RGBA))
                .hover(|s| s.bg(rgba(0xFF0088FFu32)));
        } else {
            btn = btn
                .bg(rgba(0xFFFFFF20u32))
                .border(px(1.0))
                .border_color(rgba(UiConstants::STROKE_RGBA))
                .hover(|s| s.bg(rgba(0xFFFFFF30u32)));
        }
        btn.child(label.to_string())
    }

    /// Build zoom slider / view mode switcher popover
    fn build_zoom_slider(
        current_zoom: f32,
        current_mode: crate::config::config::ViewMode,
        view: Entity<crate::app::AppWindow>,
        cx: &mut Context<crate::app::AppWindow>,
    ) -> impl IntoElement {
        use crate::config::config::ViewMode;
        use crate::ui::views::main_view::{zoom_mode_icon, zoom_mode_name};

        struct MenuItem {
            name: &'static str,
            icon: &'static str,
            mode: ViewMode,
            /// Vertical padding
            item_py: f32,
            /// Icon font size
            icon_size: f32,
            /// Text font size
            text_size: f32,
            /// Whether to show a divider after
            divider_after: bool,
        }

        let items = vec![
            MenuItem {
                name: "Details",
                icon: "\u{E783}",
                mode: ViewMode::Details,
                item_py: 6.0,
                icon_size: 16.0,
                text_size: 12.0,
                divider_after: true,
            },
            MenuItem {
                name: "List",
                icon: "\u{E77B}",
                mode: ViewMode::List,
                item_py: 6.0,
                icon_size: 16.0,
                text_size: 12.0,
                divider_after: true,
            },
            MenuItem {
                name: "Columns",
                icon: "\u{E8A8}",
                mode: ViewMode::Columns,
                item_py: 6.0,
                icon_size: 16.0,
                text_size: 12.0,
                divider_after: true,
            },
            MenuItem {
                name: "Grid",
                icon: "\u{E80A}",
                mode: ViewMode::Grid,
                item_py: 6.0,
                icon_size: 16.0,
                text_size: 12.0,
                divider_after: false,
            },
        ];

        // Fixed dialog height
        let dialog_ht = 180.0;

        div()
            .flex()
            .flex_row()
            .w_full()
            .h(px(dialog_ht))
            .child(
                // LEFT: Mode list — compact
                div()
                    .flex_col()
                    .flex_grow()
                    .py(px(4.0))
                    .children(items.iter().map(|item| {
                        let is_active = match item.name {
                            "Details" => matches!(current_mode, ViewMode::Details),
                            "List" => matches!(current_mode, ViewMode::List),
                            "Columns" => matches!(current_mode, ViewMode::Columns),
                            "Grid" => matches!(current_mode, ViewMode::Grid),
                            _ => false,
                        };
                        let v_btn = view.clone();
                        let mode = item.mode.clone();
                        let item_py = item.item_py;
                        let icon_size = item.icon_size;
                        let text_size = item.text_size;
                        let divider_after = item.divider_after;

                        div()
                            .flex_col()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(10.0))
                                    .px(px(8.0))
                                    .py(px(item_py))
                                    .rounded(px(4.0))
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
                                            rgba(0xFFFFFF10u32)
                                        })
                                    })
                                    .on_mouse_down(MouseButton::Left, {
                                        let v_btn = v_btn.clone();
                                        move |_, _, cx| {
                                            let mode = mode.clone();
                                            v_btn.update(cx, |this, cx| {
                                                this.set_view_mode(mode.clone(), cx);
                                                this.close_dialog(cx);
                                            });
                                        }
                                    })
                                    .child(
                                        div()
                                            .text_size(px(icon_size))
                                            .font_family(UiConstants::FONT_MDL2)
                                            .text_color(if is_active {
                                                rgb(0xFFFFFF)
                                            } else {
                                                rgba(UiConstants::TEXT_SECONDARY_RGBA)
                                            })
                                            .child(item.icon),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(text_size))
                                            .font_weight(if is_active {
                                                FontWeight(600.0)
                                            } else {
                                                FontWeight::default()
                                            })
                                            .text_color(if is_active {
                                                rgb(0xFFFFFF)
                                            } else {
                                                rgba(UiConstants::TEXT_PRIMARY_RGBA)
                                            })
                                            .child(item.name),
                                    ),
                            )
                            .children(if divider_after {
                                Some(
                                    div()
                                        .h(px(1.0))
                                        .bg(rgba(UiConstants::DIVIDER_RGBA))
                                        .mx(px(8.0))
                                        .my(px(2.0)),
                                )
                            } else {
                                None
                            })
                    })),
            )
            .child(
                // RIGHT: Vertical slider — whole 30px column is clickable
                div()
                    .w(px(30.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .py(px(10.0))
                    .cursor_pointer()
                    .on_mouse_down(MouseButton::Left, {
                        let v = view.clone();
                        move |event, win, cx| {
                            let click_y: f32 = event.position.y.into();
                            let win_h: f32 = win.viewport_size().height.into();
                            // Dialog: bottom(40), height dialog_ht (180), column py(10)
                            let track_top = win_h - 40.0 - dialog_ht + 10.0;
                            let rel_y = click_y - track_top;
                            // thumb_travel = dialog_ht - py*2 - thumb_h = 180 - 20 - 20 = 140
                            let tt = dialog_ht - 20.0 - 20.0;
                            // Center the 20px thumb on the click position
                            let thumb_top = (rel_y - 10.0).clamp(0.0, tt);
                            let snapped_zoom = 1.0 - thumb_top / tt;
                            v.update(cx, |this, cx| {
                                this.set_zoom_level(snapped_zoom, cx);
                                this.zoom_drag_start = Some((click_y, snapped_zoom));
                            });
                        }
                    })
                    .child(
                        // Visual track (4px line) — not the hitbox
                        div()
                            .relative()
                            .w(px(4.0))
                            .h_full()
                            .bg(rgba(0xFFFFFF15u32))
                            .rounded(px(2.0))
                            .child({
                                let thumb_travel = dialog_ht - 20.0 - 20.0;
                                let thumb_pos_ratio = 1.0 - current_zoom;
                                div()
                                    .absolute()
                                    .top(px(thumb_pos_ratio * thumb_travel))
                                    .left(px(-8.0))
                                    .w(px(20.0))
                                    .h(px(20.0))
                                    .rounded(px(10.0))
                                    .bg(rgba(UiConstants::ACCENT_RGBA))
                                    .shadow_md()
                                    .cursor_pointer()
                            }),
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

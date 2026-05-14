use crate::ui::constants::UiConstants;
use gpui::*;
use std::sync::Arc;

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
    Settings,
    CommandPalette,
    BulkRename {
        item_indices: Vec<usize>,
    },
    ContextMenu {
        item_index: Option<usize>,
        position: Point<Pixels>,
    },
    /// Sidebar context menu with item info
    SidebarContextMenu {
        item_label: String,
        item_path: Option<String>,
        is_pinned: bool,
        position: Point<Pixels>,
    },
    /// Breadcrumb sibling folders dropdown
    SiblingDropdown {
        parent_path: crate::core::path_util::FilesPath,
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
    /// Restore Crashed Session Dialog
    RestoreCrashedSession {
        crash_count: u32,
        last_path: String,
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

impl DialogsView {
    pub fn build(
        dialog: &DialogType,
        view: Entity<crate::app::AppWindow>,
        cx: &mut Context<crate::app::AppWindow>,
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
            DialogType::SiblingDropdown { parent_path } => {
                Self::build_sibling_dropdown(parent_path, view.clone(), cx).into_any_element()
            }
            DialogType::StatusCenter { active_operations, .. } => {
                Self::build_status_center(active_operations, view.clone()).into_any_element()
            }
            DialogType::Dynamic { title, message } => {
                Self::build_dynamic_dialog(title, message, view.clone()).into_any_element()
            }
            DialogType::Settings => {
                Self::build_settings_dialog(view.clone(), cx).into_any_element()
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
            } => Self::build_context_menu_dialog(*item_index, *position, view.clone(), cx)
                .into_any_element(),
            DialogType::SidebarContextMenu {
                item_label,
                item_path,
                is_pinned,
                position,
            } => Self::build_sidebar_context_menu(
                item_label,
                item_path.clone(),
                *is_pinned,
                *position,
                view.clone(),
            )
            .into_any_element(),
            DialogType::TabContextMenu {
                tab_index,
                position,
                tab_count,
            } => Self::build_tab_context_menu(*tab_index, *position, *tab_count, view.clone())
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
            DialogType::RestoreCrashedSession {
                crash_count,
                last_path,
            } => Self::build_restore_crashed_session_dialog(*crash_count, last_path, view.clone())
                .into_any_element(),
        };

        // Modal overlay
        div().absolute().inset_0().flex().items_center().justify_center().child({
            let mut container = div()
                .bg(rgb(0x202020))
                .border(px(1.0))
                .border_color(rgba(UiConstants::DIVIDER_RGBA))
                .rounded(px(8.0))
                .shadow_lg()
                .child(content);

            match dialog {
                DialogType::ContextMenu { position, .. }
                | DialogType::TabContextMenu { position, .. }
                | DialogType::SidebarContextMenu { position, .. }
                | DialogType::ColumnVisibilityMenu { position } => {
                    container = container
                        .absolute()
                        .top(position.y)
                        .left(position.x)
                        .w(px(200.0));
                }
                DialogType::StatusCenter { position, .. } => {
                    let pos = position.unwrap_or(Point {
                        x: px(1280.0 - 416.0), // Approximate default for 1280 width
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
            container
        })
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
                        v_ok.update(cx, |this, cx| this.close_dialog(cx));
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
                    .child(Self::build_settings_row("Encryption", if state.encrypt { "Enabled" } else { "Disabled" }))
            )
            .child(
                div()
                    .text_size(AbsoluteLength::Pixels(px(12.0)))
                    .text_color(rgb(UiConstants::TEXT_MUTED_RGB))
                    .child(format!("Destination: {}", state.destination_path().display())),
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
                    .child(Self::build_button("Create Archive", true, move |_, _, cx| {
                        v_ok.update(cx, |this, cx| this.close_dialog(cx));
                    })),
            )
    }

    fn build_create_shortcut_dialog(
        state: &crate::ui::dialog_services::shortcut_dialog::CreateShortcutState,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let v_cancel = view.clone();
        let v_ok = view.clone();
        let target_name = state.target_path.file_name().map_or("None", |s| s.to_str().unwrap_or("None"));

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
                    .child(Self::build_settings_row("Run as Admin", if state.run_as_admin { "Yes" } else { "No" }))
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
        let archive_name = state.archive_path.file_name().map_or("Unknown", |s| s.to_str().unwrap_or("Unknown"));

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
                    .child(Self::build_settings_row("Selected Items", &format!("{} items", state.selected_count())))
                    .child(Self::build_settings_row("Size", &crate::ui::components::status_bar::format_size(state.selected_size())))
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
        view: Entity<crate::app::AppWindow>,
        cx: &mut Context<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let v_close = view.clone();
        let v_nav = Arc::new(view.clone());
        let siblings = view.read(cx).state.nav_bar.siblings.clone();

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
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string();
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

    fn build_settings_dialog(
        view: Entity<crate::app::AppWindow>,
        cx: &mut Context<crate::app::AppWindow>,
    ) -> impl IntoElement {
        tracing::info!("DEBUG: Building Settings dialog");
        let v_close = view.clone();
        let app = view.read(cx);
        let active_tab = app.state.active_settings_tab;
        let _selected_item = app
            .state
            .main_view
            .selected_indices
            .iter()
            .next()
            .and_then(|&i| app.state.main_view.items.get(i));
        let settings_model = app.state.settings.read(cx).config().clone();

        div()
            .flex_col()
            .w(px(700.0))
            .h(px(500.0))
            .bg(rgb(UiConstants::BG_DIALOG_RGB))
            .rounded(px(8.0))
            .border(px(1.0))
            .border_color(rgba(UiConstants::DIVIDER_RGBA))
            .child(
                // Header
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .px(px(16.0))
                    .py(px(12.0))
                    .border_b(px(1.0))
                    .border_color(rgba(UiConstants::DIVIDER_RGBA))
                    .child(
                        div()
                            .text_size(AbsoluteLength::Pixels(px(18.0)))
                            .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                            .font_weight(FontWeight(600.0))
                            .child("Settings"),
                    )
                    .child(
                        div()
                            .cursor_pointer()
                            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                v_close.update(cx, |this, cx| this.close_dialog(cx));
                            })
                            .child("\u{E8BB}"),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_grow()
                    .overflow_hidden()
                    .child(
                        // Settings Sidebar
                        div()
                            .w(px(180.0))
                            .flex_col()
                            .border_r(px(1.0))
                            .border_color(rgba(UiConstants::DIVIDER_RGBA))
                            .p(px(8.0))
                            .gap(px(4.0))
                            .child(Self::settings_tab_btn(
                                "General",
                                SettingsTab::General,
                                active_tab == SettingsTab::General,
                                view.clone(),
                            ))
                            .child(Self::settings_tab_btn(
                                "Appearance",
                                SettingsTab::Appearance,
                                active_tab == SettingsTab::Appearance,
                                view.clone(),
                            ))
                            .child(Self::settings_tab_btn(
                                "Layout",
                                SettingsTab::Layout,
                                active_tab == SettingsTab::Layout,
                                view.clone(),
                            ))
                            .child(Self::settings_tab_btn(
                                "Folders",
                                SettingsTab::Folders,
                                active_tab == SettingsTab::Folders,
                                view.clone(),
                            ))
                            .child(Self::settings_tab_btn(
                                "Tags",
                                SettingsTab::Tags,
                                active_tab == SettingsTab::Tags,
                                view.clone(),
                            ))
                            .child(Self::settings_tab_btn(
                                "Actions",
                                SettingsTab::Actions,
                                active_tab == SettingsTab::Actions,
                                view.clone(),
                            ))
                            .child(Self::settings_tab_btn(
                                "Toolbar",
                                SettingsTab::Toolbar,
                                active_tab == SettingsTab::Toolbar,
                                view.clone(),
                            ))
                            .child(Self::settings_tab_btn(
                                "Advanced",
                                SettingsTab::Advanced,
                                active_tab == SettingsTab::Advanced,
                                view.clone(),
                            ))
                            .child(Self::settings_tab_btn(
                                "DevTools",
                                SettingsTab::DevTools,
                                active_tab == SettingsTab::DevTools,
                                view.clone(),
                            ))
                            .child(Self::settings_tab_btn(
                                "About",
                                SettingsTab::About,
                                active_tab == SettingsTab::About,
                                view.clone(),
                            )),
                    )
                    .child(
                        // Settings Content
                        div()
                            .flex_grow()
                            .flex_col()
                            .max_h(px(300.0))
                            .id("settings-scroll")
                            .p(px(24.0))
                            .gap(px(16.0))
                            .child(Self::build_settings_content(
                                active_tab,
                                &settings_model,
                                view.clone(),
                                cx,
                            )),
                    ),
            )
    }

    fn settings_tab_btn(
        label: &str,
        tab: SettingsTab,
        active: bool,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        div()
            .px(px(10.0))
            .py(px(8.0))
            .rounded(px(6.0))
            .bg(if active {
                rgba(0xFFFFFF15u32)
            } else {
                rgba(0x00000000u32)
            })
            .hover(|s| s.bg(rgba(0xFFFFFF10u32)))
            .cursor_pointer()
            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                view.update(cx, |this, cx| this.set_settings_tab(tab, cx));
            })
            .child(
                div()
                    .text_size(AbsoluteLength::Pixels(px(14.0)))
                    .text_color(if active {
                        rgb(UiConstants::TEXT_PRIMARY_RGB)
                    } else {
                        rgb(UiConstants::TEXT_SECONDARY_RGB)
                    })
                    .child(label.to_string()),
            )
    }

    fn build_settings_content<'a>(
        tab: SettingsTab,
        config: &'a crate::config::config::AppConfig,
        view: Entity<crate::app::AppWindow>,
        cx: &'a mut Context<crate::app::AppWindow>,
    ) -> impl IntoElement + 'a {
        match tab {
            SettingsTab::General => {
                div().flex_col().gap(px(12.0))
                    .child(div().font_weight(FontWeight(600.0)).child("General"))
                    .child(Self::settings_toggle("Show hidden files", config.preferences.show_hidden_files, "show_hidden_files", view.clone()))
                    .child(Self::settings_toggle("Show dot files", config.preferences.show_dot_files, "show_dot_files", view.clone()))
                    .child(Self::settings_toggle("Confirm on delete", config.preferences.confirm_delete, "confirm_delete", view.clone()))
            }
            SettingsTab::Appearance => {
                div().flex_col().gap(px(12.0))
                    .child(div().font_weight(FontWeight(600.0)).child("Appearance"))
                    .child(Self::settings_toggle("Mica Effect (Windows 11)", config.appearance.mica_enabled, "mica_enabled", view.clone()))
                    .child(Self::settings_toggle("Mica Alt Backdrop (saturated)", config.appearance.mica_alt, "mica_alt", view.clone()))
                    .child(Self::settings_toggle("Show file extensions", config.preferences.show_file_extensions, "show_file_extensions", view.clone()))
                    .child(Self::settings_toggle("Show hidden files", config.preferences.show_hidden_files, "show_hidden_files", view.clone()))
                    .child(Self::settings_toggle("Show dot files", config.preferences.show_dot_files, "show_dot_files", view.clone()))
                    .child(div().mt(px(8.0)).h(px(1.0)).w_full().bg(rgba(UiConstants::DIVIDER_RGBA)))
                    .child(div().mt(px(8.0)).font_weight(FontWeight(600.0)).child("Custom Fonts"))
                    .child(Self::settings_text_input("Font Family", &config.appearance.font_family, "font_family", view.clone()))
                    .child(Self::settings_number_input("Font Size", config.appearance.font_size, "font_size", view.clone()))
            }
            SettingsTab::Layout => {
                div().flex_col().gap(px(12.0))
                    .child(div().font_weight(FontWeight(600.0)).child("Layout"))
                    .child(Self::settings_toggle("Folders on top", config.view.folder_on_top, "folder_on_top", view.clone()))
                    .child(Self::settings_toggle("Show hidden files", config.preferences.show_hidden_files, "show_hidden_files", view.clone()))
                    .child(div().mt(px(8.0)).text_color(rgb(UiConstants::TEXT_SECONDARY_RGB)).child("Default View Mode"))
                    .child(div().flex().gap(px(8.0))
                        .child(Self::build_button("Details", config.view.default_view_mode == crate::config::config::ViewMode::Details, |_ , _, _| {}))
                        .child(Self::build_button("Grid", config.view.default_view_mode == crate::config::config::ViewMode::Grid, |_ , _, _| {}))
                        .child(Self::build_button("List", config.view.default_view_mode == crate::config::config::ViewMode::List, |_ , _, _| {}))
                    )
                    .child(div().mt(px(8.0)).text_color(rgb(UiConstants::TEXT_SECONDARY_RGB)).child("Group By"))
                    .child(div().flex_col().gap(px(4.0))
                        .child(div().flex().gap(px(4.0))
                            .child(Self::build_button("None", config.view.group_by == crate::config::config::GroupColumn::None, {
                                let v = view.clone();
                                move |_, _, cx| {
                                    v.update(cx, |this, _| {
                                        this.state.main_view.group_by = crate::config::config::GroupColumn::None;
                                    });
                                }
                            }))
                            .child(Self::build_button("Name", config.view.group_by == crate::config::config::GroupColumn::Name, {
                                let v = view.clone();
                                move |_, _, cx| {
                                    v.update(cx, |this, _| {
                                        this.state.main_view.group_by = crate::config::config::GroupColumn::Name;
                                    });
                                }
                            }))
                            .child(Self::build_button("Date Modified", config.view.group_by == crate::config::config::GroupColumn::DateModified, {
                                let v = view.clone();
                                move |_, _, cx| {
                                    v.update(cx, |this, _| {
                                        this.state.main_view.group_by = crate::config::config::GroupColumn::DateModified;
                                    });
                                }
                            }))
                        )
                        .child(div().flex().gap(px(4.0))
                            .child(Self::build_button("Date Created", config.view.group_by == crate::config::config::GroupColumn::DateCreated, {
                                let v = view.clone();
                                move |_, _, cx| {
                                    v.update(cx, |this, _| {
                                        this.state.main_view.group_by = crate::config::config::GroupColumn::DateCreated;
                                    });
                                }
                            }))
                            .child(Self::build_button("Size", config.view.group_by == crate::config::config::GroupColumn::Size, {
                                let v = view.clone();
                                move |_, _, cx| {
                                    v.update(cx, |this, _| {
                                        this.state.main_view.group_by = crate::config::config::GroupColumn::Size;
                                    });
                                }
                            }))
                            .child(Self::build_button("File Type", config.view.group_by == crate::config::config::GroupColumn::FileType, {
                                let v = view.clone();
                                move |_, _, cx| {
                                    v.update(cx, |this, _| {
                                        this.state.main_view.group_by = crate::config::config::GroupColumn::FileType;
                                    });
                                }
                            }))
                        )
                    )
            }
            SettingsTab::Folders => {
                div().flex_col().gap(px(12.0))
                    .child(div().font_weight(FontWeight(600.0)).child("Folders"))
                    .child(Self::settings_toggle("Show file extensions", config.preferences.show_file_extensions, "show_file_extensions", view.clone()))
                    .child(Self::settings_toggle("Show dot files", config.preferences.show_dot_files, "show_dot_files", view.clone()))
                    .child(Self::settings_toggle("Show hidden files", config.preferences.show_hidden_files, "show_hidden_files", view.clone()))
                    .child(Self::settings_toggle("Show protected system files", config.preferences.show_system_files, "show_system_files", view.clone()))
            }
            SettingsTab::Tags => {
                div().flex_col().gap(px(12.0))
                    .child(div().font_weight(FontWeight(600.0)).child("Tags"))
                    .child(div().text_color(rgb(UiConstants::TEXT_SECONDARY_RGB)).child("Manage your file tags here."))
                    .child(div().flex_col().gap(px(4.0))
                        .child(Self::build_tag_item("Work", 0xFF5555))
                        .child(Self::build_tag_item("Personal", 0x55FF55))
                        .child(Self::build_tag_item("Important", 0x5555FF))
                    )
                    .child(Self::build_button("Add New Tag", false, |_ , _, _| {}))
            }
            SettingsTab::Actions => {
                let v = view.clone();
                let pairs = view.read(cx).state.action_registry.iter();
                div().flex_col().gap(px(12.0))
                    .child(div().font_weight(FontWeight(600.0)).child("Actions & Shortcuts"))
                    .child(div().text_size(px(13.0)).text_color(rgb(UiConstants::TEXT_SECONDARY_RGB))
                        .child("Click a shortcut to rebind. Press Escape to cancel."))
                    .child(
                        div().flex_col().gap(px(2.0))
                            .children(pairs.into_iter().map(|(action, key)| {
                                let action_label = action.label().to_string();
                                let key_str = key.to_string();
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .py(px(4.0))
                                    .px(px(8.0))
                                    .rounded(px(4.0))
                                    .hover(|s| s.bg(rgba(0xFFFFFF10u32)))
                                    .child(div().text_size(px(14.0)).child(action_label))
                                    .child(
                                        div()
                                            .bg(rgba(0xFFFFFF20u32))
                                            .px(px(8.0))
                                            .py(px(2.0))
                                            .rounded(px(4.0))
                                            .text_size(px(13.0))
                                            .child(key_str)
                                    )
                            }))
                    )
                    .child(
                        div().mt(px(12.0)).flex().gap(px(8.0))
                            .child(Self::build_button("Reset to Defaults", false, move |_, _, cx| {
                                v.update(cx, |this, _| {
                                    this.state.action_registry.reset_all_defaults();
                                });
                            }))
                    )
            }
            SettingsTab::Toolbar => {
                let v_customize = view.clone();
                let app_state = view.read(cx);
                let tb = &app_state.state.main_view.toolbar_customization;
                let vis_new_folder = tb.is_visible("new_folder");
                let vis_new_file = tb.is_visible("new_file");
                let vis_cut = tb.is_visible("cut");
                let vis_copy = tb.is_visible("copy");
                let vis_paste = tb.is_visible("paste");
                let vis_details = tb.is_visible("details_view");
                let vis_grid = tb.is_visible("grid_view");
                let vis_columns = tb.is_visible("columns_view");
                let vis_preview = tb.is_visible("preview_pane");
                let vis_info = tb.is_visible("info_pane");
                let vis_search = tb.is_visible("search");
                let vis_compact = tb.is_visible("compact_mode");
                div().flex_col().gap(px(12.0))
                    .child(div().font_weight(FontWeight(600.0)).child("Toolbar Customization"))
                    .child(div().text_size(px(13.0)).text_color(rgb(UiConstants::TEXT_SECONDARY_RGB))
                        .child("Choose which buttons appear on the toolbar. Changes apply immediately."))
                    .child(Self::settings_toggle("New Folder", vis_new_folder, "toolbar_new_folder", v_customize.clone()))
                    .child(Self::settings_toggle("New File", vis_new_file, "toolbar_new_file", v_customize.clone()))
                    .child(Self::settings_toggle("Cut", vis_cut, "toolbar_cut", v_customize.clone()))
                    .child(Self::settings_toggle("Copy", vis_copy, "toolbar_copy", v_customize.clone()))
                    .child(Self::settings_toggle("Paste", vis_paste, "toolbar_paste", v_customize.clone()))
                    .child(Self::settings_toggle("Details View", vis_details, "toolbar_details_view", v_customize.clone()))
                    .child(Self::settings_toggle("Grid View", vis_grid, "toolbar_grid_view", v_customize.clone()))
                    .child(Self::settings_toggle("Columns View", vis_columns, "toolbar_columns_view", v_customize.clone()))
                    .child(Self::settings_toggle("Preview Pane", vis_preview, "toolbar_preview_pane", v_customize.clone()))
                    .child(Self::settings_toggle("Info Pane", vis_info, "toolbar_info_pane", v_customize.clone()))
                    .child(Self::settings_toggle("Search", vis_search, "toolbar_search", v_customize.clone()))
                    .child(Self::settings_toggle("Compact Mode", vis_compact, "toolbar_compact_mode", v_customize.clone()))
            }
            SettingsTab::Advanced => {
                div().flex_col().gap(px(12.0))
                    .child(div().font_weight(FontWeight(600.0)).child("Advanced"))
                    .child(Self::settings_toggle("Enable experimental tabs", config.experimental.enable_tabs, "enable_tabs", view.clone()))
                    .child(Self::settings_toggle("Enable preview pane", config.experimental.enable_preview_pane, "enable_preview_pane", view.clone()))
                    .child(Self::settings_toggle("Indexing service (Fast Search)", true, "indexing_service", view.clone()))
            }
            SettingsTab::DevTools => {
                div().flex_col().gap(px(12.0))
                    .child(div().font_weight(FontWeight(600.0)).child("Developer Tools"))
                    .child(div().bg(rgb(0x101010)).p(px(8.0)).rounded(px(4.0)).child(
                        div().text_size(px(12.0)).text_color(rgb(0xAAAAAA)).child("Log Viewer (Mock)\n[INFO] App started\n[DEBUG] Loading drive C:\\\n[WARN] Thumbnail extraction failed for image.psd")
                    ))
                    .child(Self::build_button("Export Logs", false, |_ , _, _| {}))
            }
            SettingsTab::About => {
                div().flex_col().gap(px(12.0))
                    .child(div().font_weight(FontWeight(600.0)).child("About"))
                    .child(
                        div()
                            .flex_col()
                            .gap(px(4.0))
                            .child(
                                div()
                                    .text_size(AbsoluteLength::Pixels(px(16.0)))
                                    .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                                    .child("Files-Rust")
                            )
                            .child(
                                div()
                                    .text_size(AbsoluteLength::Pixels(px(13.0)))
                                    .text_color(rgb(UiConstants::TEXT_SECONDARY_RGB))
                                    .child("Version 0.1.0 (Alpha)")
                            )
                            .child(
                                div()
                                    .text_size(AbsoluteLength::Pixels(px(13.0)))
                                    .text_color(rgb(UiConstants::TEXT_MUTED_RGB))
                                    .child("A high-performance, idiomatic Rust port of the Files file manager.")
                            )
                            .child(
                                div().mt(px(8.0))
                                    .text_size(AbsoluteLength::Pixels(px(13.0)))
                                    .text_color(rgb(UiConstants::ACCENT_COLOR_RGB))
                                    .cursor_pointer()
                                    .child("GitHub Repository")
                                    .on_mouse_down(MouseButton::Left, {
                                        let v = view.clone();
                                        move |_, _, cx| {
                                            v.update(cx, |this, cx| this.open_help(cx));
                                        }
                                    })
                            )
                            .child(
                                div().mt(px(4.0))
                                    .text_size(AbsoluteLength::Pixels(px(13.0)))
                                    .text_color(rgb(UiConstants::TEXT_MUTED_RGB))
                                    .child("Files-Rust is a Rust port of the Files file manager for Windows.")
                            )
                            .child(
                                div().mt(px(12.0)).h(px(1.0)).w_full().bg(rgba(UiConstants::DIVIDER_RGBA))
                            )
                            .child(
                                div().mt(px(8.0)).font_weight(FontWeight(600.0)).child("Release Notes")
                            )
                            .child(
                                div().flex_col().gap(px(4.0)).text_size(AbsoluteLength::Pixels(px(13.0)))
                                    .child(div().text_color(rgb(UiConstants::TEXT_PRIMARY_RGB)).child("v0.1.0 — Initial Alpha Release"))
                                    .child(div().text_color(rgb(UiConstants::TEXT_SECONDARY_RGB)).child("• File browsing with Details, Grid, List, and Column views"))
                                    .child(div().text_color(rgb(UiConstants::TEXT_SECONDARY_RGB)).child("• Keyboard shortcuts overlay (Ctrl+Shift+P)"))
                                    .child(div().text_color(rgb(UiConstants::TEXT_SECONDARY_RGB)).child("• Session persistence and crash recovery"))
                                    .child(div().text_color(rgb(UiConstants::TEXT_SECONDARY_RGB)).child("• Tag system with file categorization"))
                                    .child(div().text_color(rgb(UiConstants::TEXT_SECONDARY_RGB)).child("• Drag-and-drop item reordering between groups"))
                                    .child(div().text_color(rgb(UiConstants::TEXT_SECONDARY_RGB)).child("• Background customization with folder images"))
                                    .child(div().text_color(rgb(UiConstants::TEXT_SECONDARY_RGB)).child("• Recycle Bin integration with item restoration"))
                            )
                    )
            }
        }
    }

    fn build_tag_item(label: &str, color: u32) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .justify_between()
            .py(px(4.0))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        div()
                            .w(px(12.0))
                            .h(px(12.0))
                            .bg(rgb(color))
                            .rounded(px(6.0)),
                    )
                    .child(label.to_string()),
            )
            .child(
                div()
                    .text_color(rgb(UiConstants::TEXT_MUTED_RGB))
                    .child("\u{E70F}"),
            ) // Edit icon
    }

    fn build_shortcut_item(label: &str, shortcut: &str) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .justify_between()
            .py(px(4.0))
            .child(label.to_string())
            .child(
                div()
                    .bg(rgba(0xFFFFFF20u32))
                    .px(px(6.0))
                    .py(px(2.0))
                    .rounded(px(4.0))
                    .text_size(px(12.0))
                    .child(shortcut.to_string()),
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

    fn settings_tab(label: &str, active: bool) -> impl IntoElement {
        div()
            .px(px(8.0))
            .py(px(6.0))
            .rounded(px(4.0))
            .bg(if active {
                rgba(0xFFFFFF15u32)
            } else {
                rgba(0x00000000u32)
            })
            .hover(|s| s.bg(rgba(0xFFFFFF10u32)))
            .cursor_pointer()
            .child(
                div()
                    .text_size(AbsoluteLength::Pixels(px(14.0)))
                    .text_color(if active {
                        rgb(UiConstants::TEXT_PRIMARY_RGB)
                    } else {
                        rgb(UiConstants::TEXT_SECONDARY_RGB)
                    })
                    .child(label.to_string()),
            )
    }

    fn settings_toggle(
        label: &str,
        on: bool,
        key: &'static str,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let v_toggle = view.clone();
        div()
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .text_size(AbsoluteLength::Pixels(px(13.0)))
                    .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                    .child(label.to_string()),
            )
            .child(
                div()
                    .w(px(32.0))
                    .h(px(16.0))
                    .bg(if on {
                        rgba(UiConstants::ACCENT_RGBA)
                    } else {
                        rgba(0xFFFFFF30u32)
                    })
                    .rounded(px(8.0))
                    .flex()
                    .items_center()
                    .px(px(2.0))
                    .cursor_pointer()
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        v_toggle.update(cx, |this, cx| this.toggle_setting(key, cx));
                    })
                    .child(
                        div()
                            .w(px(12.0))
                            .h(px(12.0))
                            .bg(rgb(0xFFFFFF))
                            .rounded(px(6.0))
                            .ml(if on { px(16.0) } else { px(0.0) }),
                    ),
            )
    }

    fn settings_text_input(
        label: &str,
        value: &str,
        key: &'static str,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let v_edit = view.clone();
        let label_str = label.to_string();
        let val = value.to_string();
        div()
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .text_size(AbsoluteLength::Pixels(px(13.0)))
                    .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                    .child(label_str),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .h(px(28.0))
                    .min_w(px(160.0))
                    .px(px(8.0))
                    .bg(rgba(0xFFFFFF15u32))
                    .border(px(1.0))
                    .border_color(rgba(UiConstants::STROKE_RGBA))
                    .rounded(px(4.0))
                    .text_size(px(13.0))
                    .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                    .child(val.clone())
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        // For now, just log; real input would use a more complex GPUI text input
                        tracing::debug!("Settings text input clicked: {} = {}", key, val);
                    }),
            )
    }

    fn settings_number_input(
        label: &str,
        value: f64,
        key: &'static str,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let val_str = format!("{:.1}", value);
        Self::settings_text_input(label, &val_str, key, view)
    }

    fn build_settings_row(label: &str, value: &str) -> impl IntoElement {
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

    fn build_button(
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

fn build_context_menu_dialog(
        item_index: Option<usize>,
        _position: Point<Pixels>,
        view: Entity<crate::app::AppWindow>,
        cx: &mut Context<crate::app::AppWindow>,
    ) -> Div {
        let app = view.read(cx);
        let items = &app.state.main_view.items;
        let selected_indices = &app.state.main_view.selected_indices;

        // Determine if items are selected
        let has_selection = !selected_indices.is_empty();

        // Get info about first selected item for "Open" actions
        let first_selected_item = selected_indices
            .iter()
            .next()
            .and_then(|&idx| items.get(idx));

        // Clone view for closures
        let v_close = view.clone();
        let v_open = view.clone();
        let v_open_new_tab = view.clone();
        let v_open_new_window = view.clone();
        let v_copy = view.clone();
        let v_cut = view.clone();
        let v_paste = view.clone();
        let v_rename = view.clone();
        let v_delete = view.clone();

        // Get the selected item path for Open actions
        let selected_path = first_selected_item.map(|i| i.path.clone());

        // Context menu container with proper sizing per DESIGN_SPEC.md:
        // - max width 250px
        // - max height 480px
        let mut menu = div()
            .flex_col()
            .bg(rgba(0xF21A1A1Au32)) // LayerFillColorDefault dark
            .border(px(1.0))
            .border_color(rgba(UiConstants::STROKE_RGBA))
            .rounded(px(8.0))
            .shadow_lg()
            .w(px(250.0))
            .max_h(px(480.0));

        if has_selection {
            // ═══════════════════════════════════════════════════════════
            // PRIMARY ITEMS (48px tall): Open, Open With, Open in New Tab, etc.
            // ═══════════════════════════════════════════════════════════

            // Open
            menu = menu.child(
                Self::context_menu_item_48px(
                    "\u{E8DA}",
                    "Open",
                    {
                        let path = selected_path.clone();
                        move |_, _, cx| {
                            if let Some(ref p) = path {
                                let path = p.clone();
                                v_open.update(cx, |this, cx| {
                                    // If it's a folder (no extension), navigate into it; if file, just navigate
                                    // Note: open_item would be ideal but doesn't exist yet
                                    // For now, treat as folder navigation for directories
                                    this.navigate(path, cx);
                                    this.close_dialog(cx);
                                });
                            }
                        }
                    },
                ),
            );

            // Open With - opens the Windows "Open With" dialog
            menu = menu.child(Self::context_menu_item_48px(
                "\\u{E8DA}",
                "Open With",
                {
                    let v = v_close.clone();
                    let path = selected_path.clone();
                    move |_, _, cx| {
                        if let Some(ref p) = path {
                            let path_str = p.to_string_lossy().to_string();
                            let _ = std::process::Command::new("rundll32")
                                .args(["shell32.dll,OpenAs_RunDLL", &path_str])
                                .spawn();
                        }
                        v.update(cx, |this, cx| this.close_dialog(cx));
                    }
                },
            ));

            // Open in New Tab
            menu = menu.child(Self::context_menu_item_48px(
                "\u{E8A7}",
                "Open in New Tab",
                {
                    let path = selected_path.clone();
                    move |_, _, cx| {
                        if let Some(ref p) = path {
                            let path = p.clone();
                            v_open_new_tab.update(cx, |this, cx| {
                                this.open_in_new_tab(path, cx);
                                this.close_dialog(cx);
                            });
                        }
                    }
                },
            ));

            // Open in New Pane (stub - requires dual pane navigation)
            menu = menu.child(Self::context_menu_item_48px(
                "\u{E8A7}",
                "Open in New Pane",
                {
                    let v = v_close.clone();
                    let path = selected_path.clone();
                    move |_, _, cx| {
                        if let Some(ref p) = path {
                            // Navigate to the selected path (opens in current pane)
                            v.update(cx, |this, cx| {
                                this.navigate(p.clone(), cx);
                                this.close_dialog(cx);
                            });
                        } else {
                            v.update(cx, |this, cx| this.close_dialog(cx));
                        }
                    }
                },
            ));

            // Open in New Window
            menu = menu.child(Self::context_menu_item_48px(
                "\u{E8A7}",
                "Open in New Window",
                {
                    let path = selected_path.clone();
                    move |_, _, cx| {
                        if let Some(ref p) = path {
                            let path = p.clone();
                            v_open_new_window.update(cx, |this, cx| {
                                this.open_in_new_window(path, cx);
                                this.close_dialog(cx);
                            });
                        }
                    }
                },
            ));

            // Open in VS Code - opens the selected folder or current directory in VS Code
            menu = menu.child(Self::context_menu_item_48px(
                "\\u{E8A7}",
                "Open in VS Code",
                {
                    let v = v_close.clone();
                    let path = selected_path.clone();
                    move |_, _, cx| {
                        v.update(cx, |this, cx| {
                            this.open_in_ide(path.clone(), cx);
                            this.close_dialog(cx);
                        });
                    }
                },
            ));

            // ═══════════════════════════════════════════════════════════
            // SEPARATOR
            // ═══════════════════════════════════════════════════════════
            menu = menu.child(Self::context_menu_separator());

            // ═══════════════════════════════════════════════════════════
            // SECONDARY ITEMS (32px tall): Cut, Copy, Copy as Path, Paste
            // ═══════════════════════════════════════════════════════════

            // Cut
            menu = menu.child(Self::context_menu_item(
                "\u{E8C6}",
                "Cut",
                {
                    let v = v_cut.clone();
                    move |_, _, cx| {
                        v.update(cx, |this, cx| this.cut_selected(cx));
                    }
                },
            ));

            // Copy
            menu = menu.child(Self::context_menu_item(
                "\u{E8C8}",
                "Copy",
                {
                    let v = v_copy.clone();
                    move |_, _, cx| {
                        v.update(cx, |this, cx| this.copy_selected(cx));
                    }
                },
            ));

            // Copy as Path - copies the full path to the system clipboard
            menu = menu.child(Self::context_menu_item(
                "\\u{E8C8}",
                "Copy as Path",
                {
                    let v = v_close.clone();
                    let path = selected_path.clone();
                    move |_, _, cx| {
                        if let Some(ref p) = path {
                            let path_str = p.to_string_lossy().to_string();
                            let _ = std::process::Command::new("cmd")
                                .args(["/C", "echo", &path_str, "|", "clip"])
                                .spawn();
                        }
                        v.update(cx, |this, cx| this.close_dialog(cx));
                    }
                },
            ));

            // Paste
            menu = menu.child(Self::context_menu_item("\u{E77F}", "Paste", {
                let v = v_paste.clone();
                move |_, _, cx| {
                    v.update(cx, |this, cx| this.paste(cx));
                }
            }));

            // ═══════════════════════════════════════════════════════════
            // SEPARATOR
            // ═══════════════════════════════════════════════════════════
            menu = menu.child(Self::context_menu_separator());

            // ═══════════════════════════════════════════════════════════
            // Rename, Delete, Delete Permanently
            // ═══════════════════════════════════════════════════════════

            // Rename
            menu = menu.child(Self::context_menu_item(
                "\u{E8AC}",
                "Rename",
                {
                    let v = v_rename.clone();
                    let idx = item_index;
                    move |_, _, cx| {
                        if let Some(i) = idx {
                            v.update(cx, |this, cx| this.start_rename(i, cx));
                        }
                    }
                },
            ));

            // Delete
            menu = menu.child(Self::context_menu_item(
                "\\u{E74D}",
                "Delete",
                {
                    let v = v_delete.clone();
                    move |_, _, cx| {
                        v.update(cx, |this, cx| this.delete_selected(cx));
                    }
                },
            ));

            // Delete Permanently - bypasses recycle bin and permanently removes files/folders
            menu = menu.child(Self::context_menu_item(
                "\\u{E74D}",
                "Delete Permanently",
                {
                    let v = v_close.clone();
                    move |_, _, cx| {
                        v.update(cx, |this, cx| this.delete_permanently(cx));
                    }
                },
            ));

            // ═══════════════════════════════════════════════════════════
            // SEPARATOR
            // ═══════════════════════════════════════════════════════════
            menu = menu.child(Self::context_menu_separator());

            // ═══════════════════════════════════════════════════════════
            // Create Shortcut, Compress, Extract Here
            // ═══════════════════════════════════════════════════════════

            // Create Shortcut - opens the Create Shortcut dialog
            menu = menu.child(Self::context_menu_item(
                "\\u{E71B}",
                "Create Shortcut",
                {
                    let v = v_close.clone();
                    let path = selected_path.clone();
                    move |_, _, cx| {
                        v.update(cx, |this, cx| {
                            if let Some(ref p) = path {
                                let target_path = p.as_path().to_path_buf();
                                let current_dir = this.state.main_view.primary_path.as_path().to_path_buf();
                                let file_name = p.file_name()
                                    .map(|s| s.to_string_lossy().to_string())
                                    .unwrap_or_else(|| "Shortcut".to_string());
                                let shortcut_name = format!("{} - Shortcut", file_name);
                                this.state.active_dialog = Some(DialogType::CreateShortcut {
                                    state: Box::new(
                                        crate::ui::dialog_services::shortcut_dialog::CreateShortcutState::new(
                                            target_path,
                                            current_dir,
                                            shortcut_name,
                                        )
                                    ),
                                });
                            }
                            cx.notify();
                        });
                    }
                },
            ));

            // Compress - opens the Create Archive dialog with selected items
            menu = menu.child(Self::context_menu_item(
                "\\u{E8B7}",
                "Compress",
                {
                    let v = v_close.clone();
                    move |_, _, cx| {
                        v.update(cx, |this, cx| {
                            // Collect selected paths
                            let sources: Vec<std::path::PathBuf> = this.state.main_view.selected_indices.iter()
                                .filter_map(|&i| this.state.main_view.items.get(i))
                                .map(|item| item.path.as_path().to_path_buf())
                                .collect();

                            if !sources.is_empty() {
                                // Default destination: current directory + archive name
                                let current_dir = this.state.main_view.primary_path.as_path().to_path_buf();
                                let first_name = sources[0]
                                    .file_name()
                                    .map(|s| s.to_string_lossy().to_string())
                                    .unwrap_or_else(|| "archive".to_string());
                                let archive_name = format!("{}.zip", first_name);
                                let destination = current_dir.join(&archive_name);

                                this.state.active_dialog = Some(DialogType::CreateArchive {
                                    state: Box::new(
                                        crate::ui::dialog_services::archive_dialog::CreateArchiveState::new(
                                            sources,
                                            destination,
                                        )
                                    ),
                                });
                            }
                            cx.notify();
                        });
                    }
                },
            ));

            // Extract Here (shown when archive is selected)
            if first_selected_item
                .map(|i| {
                    let name = i.name.to_lowercase();
                    name.ends_with(".zip")
                        || name.ends_with(".7z")
                        || name.ends_with(".tar")
                        || name.ends_with(".tar.gz")
                        || name.ends_with(".rar")
                })
                .unwrap_or(false)
            {
                menu = menu.child(Self::context_menu_item(
                    "\\u{E8B7}",
                    "Extract Here",
                    {
                        let v = v_close.clone();
                        let path = selected_path.clone();
                        move |_, _, cx| {
                            v.update(cx, |this, cx| {
                                if let Some(ref p) = path {
                                    let archive_path = p.as_path().to_path_buf();
                                    let current_dir = this.state.main_view.primary_path.as_path().to_path_buf();
                                    this.state.active_dialog = Some(DialogType::DecompressArchive {
                                        state: Box::new(
                                            crate::ui::dialog_services::decompress_dialog::DecompressState::new(
                                                archive_path,
                                                current_dir,
                                            )
                                        ),
                                    });
                                }
                                cx.notify();
                            });
                        }
                    },
                ));
            }

            // ═══════════════════════════════════════════════════════════
            // SEPARATOR
            // ═══════════════════════════════════════════════════════════
            menu = menu.child(Self::context_menu_separator());

            // ═══════════════════════════════════════════════════════════
            // Tags Submenu (cascading) - dynamic tag items
            // ═══════════════════════════════════════════════════════════
            menu = menu.child(Self::context_menu_tags_submenu(view.clone(), cx));

            // ═══════════════════════════════════════════════════════════
            // Share - invokes the Windows Share dialog for selected items
            // ═══════════════════════════════════════════════════════════
            menu = menu.child(Self::context_menu_item(
                "\\u{E72D}",
                "Share",
                {
                    let v = v_close.clone();
                    let path = selected_path.clone();
                    move |_, _, cx| {
                        v.update(cx, |this, cx| {
                            if let Some(ref p) = path {
                                #[cfg(windows)]
                                {
                                    let share_path = p.clone();
                                    // Spawn share service in a background task
                                    cx.background_executor().spawn(async move {
                                        let service = crate::platform::windows::share_service::ShareService::new();
                                        let _ = service.share_file(0, &share_path).await;
                                    }).detach();
                                }
                                #[cfg(not(windows))]
                                {
                                    tracing::info!("Share not implemented on this platform");
                                }
                            }
                            cx.notify();
                        });
                    }
                },
            ));

            // ═══════════════════════════════════════════════════════════
            // Properties - opens the Windows Properties dialog
            // ═══════════════════════════════════════════════════════════
            menu = menu.child(Self::context_menu_separator());
            menu = menu.child(Self::context_menu_item(
                "\u{E946}",
                "Properties",
                {
                    let v = v_close.clone();
                    let path = selected_path.clone();
                    move |_, _, cx| {
                        if let Some(ref p) = path {
                            let path_str = p.to_string_lossy().to_string();
                            let _ = std::process::Command::new("rundll32")
                                .args(["shell32.dll,PropertiesDialogW", &path_str])
                                .spawn();
                        }
                        v.update(cx, |this, cx| this.close_dialog(cx));
                    }
                },
            ));
        } else {
            // ═══════════════════════════════════════════════════════════
            // NO SELECTION - Background context menu
            // ═══════════════════════════════════════════════════════════

            // New Folder - creates a new folder at the current path
            menu = menu.child(Self::context_menu_item_48px(
                "\\u{E8F4}",
                "New Folder",
                {
                    let v = v_close.clone();
                    move |_, _, cx| {
                        v.update(cx, |this, cx| {
                            let current_dir = this.state.main_view.primary_path.as_path().to_path_buf();
                            // Auto-increment folder name if "New Folder" already exists
                            let mut folder_name = "New Folder".to_string();
                            let mut counter = 1;
                            while current_dir.join(&folder_name).exists() {
                                folder_name = format!("New Folder ({})", counter);
                                counter += 1;
                            }
                            let new_folder_path = current_dir.join(&folder_name);
                            match std::fs::create_dir(&new_folder_path) {
                                Ok(_) => {
                                    tracing::info!("Created folder: {:?}", new_folder_path);
                                    this.refresh(cx);
                                }
                                Err(e) => {
                                    tracing::error!("Failed to create folder {:?}: {}", new_folder_path, e);
                                }
                            }
                            cx.notify();
                        });
                    }
                },
            ));

            // New File - creates a new empty text file at the current path
            menu = menu.child(Self::context_menu_item_48px(
                "\\u{E7C3}",
                "New File",
                {
                    let v = v_close.clone();
                    move |_, _, cx| {
                        v.update(cx, |this, cx| {
                            let current_dir = this.state.main_view.primary_path.as_path().to_path_buf();
                            // Auto-increment file name if "New File.txt" already exists
                            let mut file_name = "New File.txt".to_string();
                            let mut counter = 1;
                            while current_dir.join(&file_name).exists() {
                                file_name = format!("New File ({}).txt", counter);
                                counter += 1;
                            }
                            let new_file_path = current_dir.join(&file_name);
                            match std::fs::File::create(&new_file_path) {
                                Ok(_) => {
                                    tracing::info!("Created file: {:?}", new_file_path);
                                    this.refresh(cx);
                                }
                                Err(e) => {
                                    tracing::error!("Failed to create file {:?}: {}", new_file_path, e);
                                }
                            }
                            cx.notify();
                        });
                    }
                },
            ));

            // Paste
            menu = menu.child(Self::context_menu_item("\u{E77F}", "Paste", {
                let v = v_paste.clone();
                move |_, _, cx| {
                    v.update(cx, |this, cx| this.paste(cx));
                }
            }));

            // Separator
            menu = menu.child(Self::context_menu_separator());

            // View options submenu could go here

            // Properties (for folder)
            menu = menu.child(Self::context_menu_item(
                "\u{E946}",
                "Properties",
                {
                    let v = v_close.clone();
                    move |_, _, cx| {
                        v.update(cx, |this, cx| {
                            let path_str = this.state.main_view.primary_path.to_string_lossy().to_string();
                            let _ = std::process::Command::new("rundll32")
                                .args(["shell32.dll,PropertiesDialogW", &path_str])
                                .spawn();
                            this.close_dialog(cx);
                        });
                    }
                },
            ));
        }

        menu
    }

    /// Build context menu items for all available tags with toggle logic
    fn context_menu_tags_submenu(
        view: Entity<crate::app::AppWindow>,
        cx: &mut Context<crate::app::AppWindow>,
    ) -> Div {
        let app = view.read(cx);
        let common_tags = app.get_common_tags_for_selected();

        let mut container = div().flex_col();

        // "Tags" header
        container = container.child(
            div()
                .flex()
                .items_center()
                .gap(px(12.0))
                .px(px(12.0))
                .py(px(6.0))
                .child(
                    div()
                        .text_size(px(14.0))
                        .text_color(rgb(UiConstants::ACCENT_COLOR_RGB))
                        .child("\\u{E8EC}"),
                )
                .child(
                    div()
                        .text_size(px(13.0))
                        .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                        .child("Tags"),
                ),
        );

        if common_tags.is_empty() {
            container = container.child(
                div()
                    .flex()
                    .items_center()
                    .px(px(12.0))
                    .py(px(4.0))
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(rgb(UiConstants::TEXT_SECONDARY_RGB))
                            .child("No tags available"),
                    ),
            );
        } else {
            for (tag_name, tag_state) in &common_tags {
                let is_all = *tag_state == crate::app::TagState::All;
                let tag_name_clone = tag_name.clone();
                let v = view.clone();

                let (glyph, label) = if is_all {
                    ("\\u{E73E}", format!("{} (remove)", tag_name))
                } else {
                    ("\\u{E8EC}", format!("{} (add)", tag_name))
                };

                container = container.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(12.0))
                        .px(px(12.0))
                        .py(px(8.0))
                        .cursor_pointer()
                        .hover(|s| s.bg(rgba(0xFFFFFF15u32)))
                        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                            if is_all {
                                v.update(cx, |this, cx| {
                                    this.remove_tag_from_selected(&tag_name_clone, cx);
                                });
                            } else {
                                v.update(cx, |this, cx| {
                                    this.add_tag_to_selected(tag_name_clone.clone(), cx);
                                });
                            }
                        })
                        .child(
                            div()
                                .text_size(px(14.0))
                                .text_color(rgb(UiConstants::ACCENT_COLOR_RGB))
                                .child(glyph),
                        )
                        .child(
                            div()
                                .text_size(px(13.0))
                                .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                                .child(label),
                        ),
                );
            }
        }

        container
    }

    fn context_menu_item(
        glyph: &str,
        label: &str,
        on_click: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    ) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .gap(px(12.0))
            .px(px(12.0))
            .py(px(8.0))
            .cursor_pointer()
            .hover(|s| s.bg(rgba(0xFFFFFF15u32)))
            .on_mouse_down(MouseButton::Left, on_click)
            .child(
                div()
                    .text_size(px(14.0))
                    .text_color(rgb(UiConstants::ACCENT_COLOR_RGB))
                    .child(glyph.to_string()),
            )
            .child(
                div()
                    .text_size(px(13.0))
                    .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                    .child(label.to_string()),
            )
    }

    /// 48px tall context menu item (for primary items like Open, New Folder, etc.)
    /// Per DESIGN_SPEC.md: primary items are 48px tall
    fn context_menu_item_48px(
        glyph: &str,
        label: &str,
        on_click: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    ) -> Div {
        div()
            .flex()
            .items_center()
            .gap(px(12.0))
            .px(px(12.0))
            .py(px(10.0))
            .cursor_pointer()
            .hover(|s| s.bg(rgba(0xFFFFFF15u32)))
            .on_mouse_down(MouseButton::Left, on_click)
            .child(
                div()
                    .text_size(px(16.0))
                    .text_color(rgb(UiConstants::ACCENT_COLOR_RGB))
                    .child(glyph.to_string()),
            )
            .child(
                div()
                    .text_size(px(14.0))
                    .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                    .child(label.to_string()),
            )
    }

    /// Separator line for context menu sections
    fn context_menu_separator() -> Div {
        div()
            .h(px(1.0))
            .bg(rgba(UiConstants::DIVIDER_RGBA))
            .mx(px(8.0))
            .my(px(4.0))
    }

    /// Build a context menu for sidebar items with actions like Open, Pin/Unpin, Open in New Tab
    fn build_sidebar_context_menu(
        item_label: &str,
        item_path: Option<String>,
        is_pinned: bool,
        _position: Point<Pixels>,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let v_close = view.clone();
        let v_open = view.clone();
        let v_new_tab = view.clone();
        let v_new_window = view.clone();
        let v_pin = view.clone();
        let v_unpin = view.clone();

        let path_for_open = item_path.clone();
        let path_for_tab = item_path.clone();
        let path_for_window = item_path.clone();
        let path_for_unpin = item_path.clone();
        let path_for_pin_action = item_path.clone();

        div()
            .flex_col()
            .py(px(4.0))
            // Open
            .child(Self::context_menu_item(
                "\u{E8DA}",
                "Open",
                move |_, _, cx| {
                    if let Some(ref p) = path_for_open {
                        let path = crate::core::path_util::FilesPath::new(p);
                        v_open.update(cx, |this, cx| {
                            this.navigate(path, cx);
                            this.close_dialog(cx);
                        });
                    }
                },
            ))
            // Open in New Tab
            .child(Self::context_menu_item(
                "\u{E8A7}",
                "Open in New Tab",
                move |_, _, cx| {
                    if let Some(ref p) = path_for_tab {
                        let path = crate::core::path_util::FilesPath::new(p);
                        v_new_tab.update(cx, |this, cx| {
                            this.open_in_new_tab(path, cx);
                            this.close_dialog(cx);
                        });
                    }
                },
            ))
            // Open in New Window
            .child(Self::context_menu_item(
                "\u{E8A7}",
                "Open in New Window",
                move |_, _, cx| {
                    if let Some(ref p) = path_for_window {
                        let path = crate::core::path_util::FilesPath::new(p);
                        v_new_window.update(cx, |this, cx| {
                            this.open_in_new_window(path, cx);
                            this.close_dialog(cx);
                        });
                    }
                },
            ))
            .child(
                div()
                    .h(px(1.0))
                    .bg(rgba(UiConstants::DIVIDER_RGBA))
                    .mx(px(8.0))
                    .my(px(4.0)),
            )
            // Pin/Unpin item - always show (if pinned, show Unpin; if not pinned and has path, show Pin)
            // Pin/Unpin item - always show (if pinned, show Unpin; if not pinned and has path, show Pin)
            .child(if is_pinned {
                // Unpin action
                let v = v_unpin.clone();
                let p = path_for_unpin.clone();
                div().child(Self::context_menu_item(
                    "\u{E718}",
                    "Unpin from sidebar",
                    move |_, _, cx| {
                        if let Some(ref path_str) = p {
                            let path = crate::core::path_util::FilesPath::new(path_str);
                            v.update(cx, |this, cx| {
                                this.unpin_folder(path, cx);
                                this.close_dialog(cx);
                            });
                        }
                    },
                ))
            } else if item_path.is_some() {
                // Pin action (only if we have a path)
                let v = v_pin.clone();
                let p = path_for_pin_action.clone();
                div().child(Self::context_menu_item(
                    "\u{E718}",
                    "Pin to sidebar",
                    move |_, _, cx| {
                        if let Some(ref path_str) = p {
                            let path = crate::core::path_util::FilesPath::new(path_str);
                            v.update(cx, |this, cx| {
                                this.pin_folder(path, cx);
                                this.close_dialog(cx);
                            });
                        }
                    },
                ))
            } else {
                div()
            })
            .child(
                div()
                    .h(px(1.0))
                    .bg(rgba(UiConstants::DIVIDER_RGBA))
                    .mx(px(8.0))
                    .my(px(4.0)),
            )
            // Properties
            .child(Self::context_menu_item(
                "\u{E946}",
                "Properties",
                move |_, _, cx| {
                    v_close.update(cx, |this, cx| {
                        this.close_dialog(cx);
                    });
                },
            ))
    }

    /// Build a context menu for tab bar right-click with options like Close Tab, Close Other Tabs, etc.
    fn build_tab_context_menu(
        tab_index: usize,
        _position: Point<Pixels>,
        tab_count: usize,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let v_close = view.clone();
        let v_close_others = view.clone();
        let v_close_right = view.clone();
        let v_duplicate = view.clone();
        let v_pin = view.clone();
        let v_new_tab = view.clone();
        let idx = tab_index;

        div()
            .flex_col()
            .py(px(4.0))
            // Close Tab
            .child(Self::context_menu_item("\u{E8BB}", "Close Tab", {
                let v = v_close.clone();
                move |_, _, cx| {
                    v.update(cx, |this, cx| {
                        this.close_tab(idx, cx);
                        this.close_dialog(cx);
                    });
                }
            }))
            // Close Other Tabs (only show if more than 1 tab)
            .child(if tab_count > 1 {
                div().child(Self::context_menu_item("\u{E8BB}", "Close Other Tabs", {
                    let v = v_close_others.clone();
                    let idx = idx;
                    move |_, _, cx| {
                        v.update(cx, |this, cx| {
                            this.close_other_tabs(idx, cx);
                            this.close_dialog(cx);
                        });
                    }
                }))
            } else {
                div()
            })
            // Close Tabs to the Right (only show if not the last tab)
            .child(if tab_index < tab_count - 1 {
                div().child(Self::context_menu_item(
                    "\u{E72B}",
                    "Close Tabs to the Right",
                    {
                        let v = v_close_right.clone();
                        let idx = idx;
                        move |_, _, cx| {
                            v.update(cx, |this, cx| {
                                this.close_tabs_to_right(idx, cx);
                                this.close_dialog(cx);
                            });
                        }
                    },
                ))
            } else {
                div()
            })
            .child(
                div()
                    .h(px(1.0))
                    .bg(rgba(UiConstants::DIVIDER_RGBA))
                    .mx(px(8.0))
                    .my(px(4.0)),
            )
            // Duplicate Tab
            .child(Self::context_menu_item("\u{E8C8}", "Duplicate Tab", {
                let v = v_duplicate.clone();
                let idx = idx;
                move |_, _, cx| {
                    v.update(cx, |this, cx| {
                        this.duplicate_tab(idx, cx);
                        this.close_dialog(cx);
                    });
                }
            }))
            // Pin Tab
            .child(Self::context_menu_item("\u{E718}", "Pin Tab", {
                let v = v_pin.clone();
                let idx = idx;
                move |_, _, cx| {
                    v.update(cx, |this, cx| {
                        this.pin_tab(idx, cx);
                        this.close_dialog(cx);
                    });
                }
            }))
            .child(
                div()
                    .h(px(1.0))
                    .bg(rgba(UiConstants::DIVIDER_RGBA))
                    .mx(px(8.0))
                    .my(px(4.0)),
            )
            // New Tab
            .child(Self::context_menu_item("\u{E710}", "New Tab", {
                let v = v_new_tab.clone();
                move |_, _, cx| {
                    v.update(cx, |this, cx| {
                        this.new_tab(cx);
                        this.close_dialog(cx);
                    });
                }
            }))
    }

    /// Build column visibility toggle menu (shown on right-click of column header)
    fn build_column_visibility_menu(
        _position: Point<Pixels>,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let v_close = view.clone();

        div()
            .flex_col()
            .py(px(4.0))
            .child(
                div()
                    .px(px(8.0))
                    .py(px(6.0))
                    .text_size(AbsoluteLength::Pixels(px(12.0)))
                    .text_color(rgb(UiConstants::TEXT_SECONDARY_RGB))
                    .child("Click column header to sort"),
            )
            .child(
                div()
                    .h(px(1.0))
                    .bg(rgba(UiConstants::DIVIDER_RGBA))
                    .mx(px(8.0))
                    .my(px(4.0)),
            )
            .child(Self::context_menu_item("\u{E8FB}", "Name", {
                let v = v_close.clone();
                move |_, _, cx| {
                    v.update(cx, |this, cx| {
                        this.close_dialog(cx);
                    });
                }
            }))
            .child(Self::context_menu_item("\u{E787}", "Date Modified", {
                let v = v_close.clone();
                move |_, _, cx| {
                    v.update(cx, |this, cx| {
                        this.toggle_column_visibility("Date Modified".to_string(), cx);
                        this.close_dialog(cx);
                    });
                }
            }))
            .child(Self::context_menu_item("\u{E8A5}", "Type", {
                let v = v_close.clone();
                move |_, _, cx| {
                    v.update(cx, |this, cx| {
                        this.toggle_column_visibility("Type".to_string(), cx);
                        this.close_dialog(cx);
                    });
                }
            }))
            .child(Self::context_menu_item("\u{E8FD}", "Size", {
                let v = v_close.clone();
                move |_, _, cx| {
                    v.update(cx, |this, cx| {
                        this.toggle_column_visibility("Size".to_string(), cx);
                        this.close_dialog(cx);
                    });
                }
            }))
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
    fn build_custom_properties_dialog(
        state: &crate::ui::dialog_services::custom_properties_dialog::CustomPropertiesState,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let v_close = view.clone();
        let active_tab = state.active_tab;
        let tabs = crate::ui::dialog_services::custom_properties_dialog::PropertiesTab::all();

        div()
            .flex_col()
            .w(px(600.0))
            .h(px(500.0))
            .bg(rgb(0x202020))
            .rounded(px(8.0))
            .border(px(1.0))
            .border_color(rgba(UiConstants::DIVIDER_RGBA))
            // Header
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .px(px(16.0))
                    .py(px(12.0))
                    .border_b(px(1.0))
                    .border_color(rgba(UiConstants::DIVIDER_RGBA))
                    .child(
                        div()
                            .text_size(px(18.0))
                            .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                            .font_weight(FontWeight(600.0))
                            .child(format!("{} Properties", state.name)),
                    )
                    .child(
                        div()
                            .cursor_pointer()
                            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                v_close.update(cx, |this, cx| this.close_dialog(cx));
                            })
                            .child("\u{E8BB}"), // Close icon (X)
                    ),
            )
            // Tab bar + content
            .child(
                div()
                    .flex()
                    .flex_grow()
                    .overflow_hidden()
                    // Tab sidebar
                    .child(
                        div()
                            .w(px(160.0))
                            .flex_col()
                            .border_r(px(1.0))
                            .border_color(rgba(UiConstants::DIVIDER_RGBA))
                            .p(px(8.0))
                            .gap(px(4.0))
                            .children(tabs.iter().map(|&tab| {
                                let v_tab = view.clone();
                                let is_active = tab == active_tab;
                                Self::properties_tab_btn(tab.display_name(), tab, is_active, v_tab)
                            })),
                    )
                    // Tab content
                    .child(
                        div()
                            .flex_grow()
                            .flex_col()
                            .p(px(16.0))
                            .gap(px(12.0))
                            .child(Self::build_properties_tab_content(state, view.clone())),
                    ),
            )
            // Footer with OK/Cancel buttons
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap(px(8.0))
                    .px(px(16.0))
                    .py(px(12.0))
                    .border_t(px(1.0))
                    .border_color(rgba(UiConstants::DIVIDER_RGBA))
                    .child({
                        let v = view.clone();
                        Self::build_button("Cancel", false, move |_, _, cx| {
                            v.update(cx, |this, cx| this.close_dialog(cx));
                        })
                    })
                    .child({
                        let v = view.clone();
                        Self::build_button("OK", true, move |_, _, cx| {
                            v.update(cx, |this, cx| this.close_dialog(cx));
                        })
                    }),
            )
    }

    /// Render a tab button in the properties dialog sidebar
    fn properties_tab_btn(
        label: &str,
        tab: crate::ui::dialog_services::custom_properties_dialog::PropertiesTab,
        active: bool,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        div()
            .px(px(10.0))
            .py(px(8.0))
            .rounded(px(6.0))
            .bg(if active { rgba(0xFFFFFF15u32) } else { rgba(0x00000000u32) })
            .hover(|s| s.bg(rgba(0xFFFFFF10u32)))
            .cursor_pointer()
            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                view.update(cx, |this, cx| {
                    // The state lives in DialogType::CustomProperties
                    if let Some(crate::ui::components::dialogs::DialogType::CustomProperties { ref mut state }) =
                        this.state.active_dialog
                    {
                        state.set_tab(tab);
                        cx.notify();
                    }
                });
            })
            .child(
                div()
                    .text_size(px(14.0))
                    .text_color(if active {
                        rgb(UiConstants::TEXT_PRIMARY_RGB)
                    } else {
                        rgb(UiConstants::TEXT_SECONDARY_RGB)
                    })
                    .child(label.to_string()),
            )
    }

    /// Render content for the currently active properties tab
    fn build_properties_tab_content(
        state: &crate::ui::dialog_services::custom_properties_dialog::CustomPropertiesState,
        view: Entity<crate::app::AppWindow>,
    ) -> AnyElement {
        match state.active_tab {
            crate::ui::dialog_services::custom_properties_dialog::PropertiesTab::General => {
                Self::build_properties_general_tab(state).into_any_element()
            }
            crate::ui::dialog_services::custom_properties_dialog::PropertiesTab::Security => {
                Self::build_properties_security_tab(state).into_any_element()
            }
            crate::ui::dialog_services::custom_properties_dialog::PropertiesTab::Customization => {
                Self::build_properties_customization_tab(state).into_any_element()
            }
            crate::ui::dialog_services::custom_properties_dialog::PropertiesTab::Hashes => {
                Self::build_properties_hashes_tab(state).into_any_element()
            }
            crate::ui::dialog_services::custom_properties_dialog::PropertiesTab::Compatibility => {
                Self::build_properties_compatibility_tab(state, view).into_any_element()
            }
            crate::ui::dialog_services::custom_properties_dialog::PropertiesTab::Signatures => {
                Self::build_properties_signatures_tab(state).into_any_element()
            }
        }
    }

    // ────────── General Tab ──────────
    fn build_properties_general_tab(
        state: &crate::ui::dialog_services::custom_properties_dialog::CustomPropertiesState,
    ) -> impl IntoElement {
        div()
            .flex_col()
            .gap(px(8.0))
            .child(
                div()
                    .text_size(px(16.0))
                    .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                    .font_weight(FontWeight(600.0))
                    .child("General"),
            )
            .child(Self::build_settings_row("Name", &state.name))
            .child(Self::build_settings_row("Type", &state.file_type))
            .child(Self::build_settings_row("Size", &state.formatted_size()))
            .child(if let Some(ref created) = state.created {
                Self::build_settings_row("Created", created).into_any_element()
            } else {
                div().into_any_element()
            })
            .child(if let Some(ref modified) = state.modified {
                Self::build_settings_row("Modified", modified).into_any_element()
            } else {
                div().into_any_element()
            })
            .child(if let Some(ref accessed) = state.accessed {
                Self::build_settings_row("Accessed", accessed).into_any_element()
            } else {
                div().into_any_element()
            })
            .child(Self::build_settings_row("Attributes", &state.attribute_summary()))
    }

    // ────────── Security Tab ──────────
    fn build_properties_security_tab(
        state: &crate::ui::dialog_services::custom_properties_dialog::CustomPropertiesState,
    ) -> impl IntoElement {
        let sec = &state.security;
        div()
            .flex_col()
            .gap(px(8.0))
            .child(
                div()
                    .text_size(px(16.0))
                    .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                    .font_weight(FontWeight(600.0))
                    .child("Security"),
            )
            .child(if let Some(ref owner) = sec.owner {
                Self::build_settings_row("Owner", owner).into_any_element()
            } else {
                div().into_any_element()
            })
            .child(Self::build_settings_row("Permissions", &sec.permissions_summary))
            .child(Self::properties_checkbox("Readable", sec.can_read, |_, _, _| {}))
            .child(Self::properties_checkbox("Writable", sec.can_write, |_, _, _| {}))
            .child(Self::properties_checkbox("Deletable", sec.can_delete, |_, _, _| {}))
            .child(Self::properties_checkbox("Read-only", sec.is_readonly, |_, _, _| {}))
            .child(Self::properties_checkbox("Hidden", sec.is_hidden, |_, _, _| {}))
            .child(Self::properties_checkbox("System", sec.is_system, |_, _, _| {}))
    }

    // ────────── Customization Tab ──────────
    fn build_properties_customization_tab(
        state: &crate::ui::dialog_services::custom_properties_dialog::CustomPropertiesState,
    ) -> impl IntoElement {
        let cust = &state.customization;
        div()
            .flex_col()
            .gap(px(8.0))
            .child(
                div()
                    .text_size(px(16.0))
                    .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                    .font_weight(FontWeight(600.0))
                    .child("Customization"),
            )
            .child(if let Some(ref icon_path) = cust.folder_icon_path {
                Self::build_settings_row("Icon", icon_path).into_any_element()
            } else {
                div().into_any_element()
            })
            .child(if let Some(ref icon_index) = cust.folder_icon_index {
                Self::build_settings_row("Icon Index", &icon_index.to_string()).into_any_element()
            } else {
                div().into_any_element()
            })
            .child(if let Some(ref bg_img) = cust.folder_background_image {
                Self::build_settings_row("Background", bg_img).into_any_element()
            } else {
                div().into_any_element()
            })
            .child(Self::build_settings_row("Opacity", &format!("{}%", cust.folder_background_opacity)))
            .child(if let Some(ref ftype) = cust.folder_type {
                Self::build_settings_row("Folder Type", ftype).into_any_element()
            } else {
                div().into_any_element()
            })
            .child(Self::properties_checkbox(
                "Apply to subfolders",
                cust.apply_to_subfolders,
                |_, _, _| {},
            ))
    }

    // ────────── Hashes Tab ──────────
    fn build_properties_hashes_tab(
        state: &crate::ui::dialog_services::custom_properties_dialog::CustomPropertiesState,
    ) -> impl IntoElement {
        let hashes = &state.hashes;
        div()
            .flex_col()
            .gap(px(8.0))
            .child(
                div()
                    .text_size(px(16.0))
                    .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                    .font_weight(FontWeight(600.0))
                    .child("File Hashes"),
            )
            .child(if state.loading_hashes {
                div()
                    .text_color(rgb(UiConstants::TEXT_MUTED_RGB))
                    .child("Computing hashes...")
            } else {
                div().flex_col().gap(px(4.0))
                    .child(Self::build_hash_row("MD5", hashes.md5.as_deref()))
                    .child(Self::build_hash_row("SHA-1", hashes.sha1.as_deref()))
                    .child(Self::build_hash_row("SHA-256", hashes.sha256.as_deref()))
                    .child(Self::build_hash_row("SHA-384", hashes.sha384.as_deref()))
                    .child(Self::build_hash_row("SHA-512", hashes.sha512.as_deref()))
                    .child(Self::build_hash_row("CRC32", hashes.crc32.as_deref()))
            })
    }

    fn build_hash_row(label: &str, value: Option<&str>) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .gap(px(8.0))
            .child(
                div()
                    .w(px(70.0))
                    .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                    .child(label.to_string()),
            )
            .child(
                div()
                    .text_size(px(12.0))
                    .font_family("Consolas")
                    .text_color(rgb(UiConstants::TEXT_MUTED_RGB))
                    .child(value.unwrap_or("—").to_string()),
            )
    }

    // ────────── Compatibility Tab ──────────
    fn build_properties_compatibility_tab(
        state: &crate::ui::dialog_services::custom_properties_dialog::CustomPropertiesState,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let compat = &state.compatibility;
        let emulation_options = [
            ("None", ""),
            ("Windows 95", "Windows 95"),
            ("Windows 98/ME", "Windows 98"),
            ("Windows XP (SP2)", "Windows XP"),
            ("Windows XP (SP3)", "Windows XP SP3"),
            ("Windows Vista", "Windows Vista"),
            ("Windows 7", "Windows 7"),
            ("Windows 8", "Windows 8"),
        ];

        let dpi_options = [
            ("None", ""),
            ("Application", "Application"),
            ("System", "System"),
            ("System (Enhanced)", "System (Enhanced)"),
        ];

        div()
            .flex_col()
            .gap(px(12.0))
            .child(
                div()
                    .text_size(px(16.0))
                    .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                    .font_weight(FontWeight(600.0))
                    .child("Compatibility"),
            )
            // Run as administrator checkbox
            .child(
                Self::properties_checkbox("Run this program as an administrator", compat.run_as_admin, {
                    let v = view.clone();
                    move |_, _, cx| {
                        v.update(cx, |this, cx| {
                            if let Some(crate::ui::components::dialogs::DialogType::CustomProperties { ref mut state }) =
                                this.state.active_dialog
                            {
                                state.compatibility.run_as_admin = !state.compatibility.run_as_admin;
                                cx.notify();
                            }
                        });
                    }
                }),
            )
            // Reduced color mode checkbox
            .child(
                Self::properties_checkbox("Reduced color mode", compat.color_mode.is_some(), {
                    let v = view.clone();
                    move |_, _, cx| {
                        v.update(cx, |this, cx| {
                            if let Some(crate::ui::components::dialogs::DialogType::CustomProperties { ref mut state }) =
                                this.state.active_dialog
                            {
                                state.compatibility.color_mode = if state.compatibility.color_mode.is_some() {
                                    None
                                } else {
                                    Some("256-color".to_string())
                                };
                                cx.notify();
                            }
                        });
                    }
                }),
            )
            // Disable theming checkbox
            .child(
                Self::properties_checkbox("Disable visual themes", compat.disable_theming, {
                    let v = view.clone();
                    move |_, _, cx| {
                        v.update(cx, |this, cx| {
                            if let Some(crate::ui::components::dialogs::DialogType::CustomProperties { ref mut state }) =
                                this.state.active_dialog
                            {
                                state.compatibility.disable_theming = !state.compatibility.disable_theming;
                                cx.notify();
                            }
                        });
                    }
                }),
            )
            // Disable desktop composition checkbox
            .child(
                Self::properties_checkbox("Disable desktop composition", compat.disable_visual_features, {
                    let v = view.clone();
                    move |_, _, cx| {
                        v.update(cx, |this, cx| {
                            if let Some(crate::ui::components::dialogs::DialogType::CustomProperties { ref mut state }) =
                                this.state.active_dialog
                            {
                                state.compatibility.disable_visual_features = !state.compatibility.disable_visual_features;
                                cx.notify();
                            }
                        });
                    }
                }),
            )
            // Run in 640x480 screen resolution checkbox
            .child(
                Self::properties_checkbox("Run in 640×480 screen resolution", false, |_, _, _| {}),
            )
            // Override high DPI scaling behavior
            .child(
                div()
                    .mt(px(8.0))
                    .child(div().text_color(rgb(UiConstants::TEXT_SECONDARY_RGB)).child("Override high DPI scaling behavior:")),
            )
            // Emulation mode dropdown
            .child(
                div()
                    .mt(px(4.0))
                    .child(div().text_color(rgb(UiConstants::TEXT_SECONDARY_RGB)).child("Run this program in compatibility mode for:")),
            )
            .child(
                Self::properties_dropdown(
                    emulation_options.iter().map(|(l, v)| (*l, *v)).collect(),
                    compat.emulation_mode.as_deref().unwrap_or(""),
                    {
                        let v = view.clone();
                        let val = std::sync::Arc::new(std::sync::Mutex::new(None::<String>));
                        let val_clone = val.clone();
                        move |selected_value: &str| {
                            let v = v.clone();
                            let new_val = if selected_value.is_empty() { None } else { Some(selected_value.to_string()) };
                            *val_clone.lock().unwrap() = new_val;
                            // The dropdown callback runs in GPUI event context; state will be read on next render
                        }
                    },
                ),
            )
            // DPI settings dropdown
            .child(
                div()
                    .mt(px(8.0))
                    .child(div().text_color(rgb(UiConstants::TEXT_SECONDARY_RGB)).child("Change high DPI settings:")),
            )
            .child(
                Self::properties_dropdown(
                    dpi_options.iter().map(|(l, v)| (*l, *v)).collect(),
                    compat.dpi_settings.as_deref().unwrap_or(""),
                    {
                        let v = view.clone();
                        let val = std::sync::Arc::new(std::sync::Mutex::new(None::<String>));
                        let val_clone = val.clone();
                        move |selected_value: &str| {
                            let v = v.clone();
                            let new_val = if selected_value.is_empty() { None } else { Some(selected_value.to_string()) };
                            *val_clone.lock().unwrap() = new_val;
                        }
                    },
                ),
            )
            // Run in sandbox checkbox
            .child(
                Self::properties_checkbox("Run this program in a sandbox", compat.run_in_sandbox, {
                    let v = view.clone();
                    move |_, _, cx| {
                        v.update(cx, |this, cx| {
                            if let Some(crate::ui::components::dialogs::DialogType::CustomProperties { ref mut state }) =
                                this.state.active_dialog
                            {
                                state.compatibility.run_in_sandbox = !state.compatibility.run_in_sandbox;
                                cx.notify();
                            }
                        });
                    }
                }),
            )
    }

    // ────────── Signatures Tab ──────────
    fn build_properties_signatures_tab(
        state: &crate::ui::dialog_services::custom_properties_dialog::CustomPropertiesState,
    ) -> impl IntoElement {
        let sig = &state.signature;

        div()
            .flex_col()
            .gap(px(12.0))
            .child(
                div()
                    .text_size(px(16.0))
                    .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                    .font_weight(FontWeight(600.0))
                    .child("Digital Signatures"),
            )
            // Signature status summary
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        div()
                            .text_size(px(32.0))
                            .text_color(if sig.is_signed && sig.is_valid {
                                rgb(0x66DD66)
                            } else if sig.is_signed && !sig.is_valid {
                                rgb(0xDD6666)
                            } else {
                                rgb(UiConstants::TEXT_MUTED_RGB)
                            })
                            .child(if sig.is_signed && sig.is_valid {
                                "\u{E930}" // Checkmark
                            } else if sig.is_signed {
                                "\u{E92B}" // Warning
                            } else {
                                "\u{E946}" // Info
                            }),
                    )
                    .child(
                        div()
                            .flex_col()
                            .child(
                                div()
                                    .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                                    .child(if sig.is_signed {
                                        if sig.is_valid {
                                            "✓ Valid Signature".to_string()
                                        } else {
                                            "✗ Invalid Signature".to_string()
                                        }
                                    } else {
                                        "No digital signature".to_string()
                                    }),
                            )
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(rgb(UiConstants::TEXT_MUTED_RGB))
                                    .child(if sig.is_signed {
                                        format!("Signed by: {}", sig.signer_name.as_deref().unwrap_or("Unknown"))
                                    } else {
                                        "This file is not digitally signed.".to_string()
                                    }),
                            ),
                    ),
            )
            // Signer details (expandable section)
            .child(if sig.is_signed {
                div()
                    .flex_col()
                    .gap(px(8.0))
                    .mt(px(8.0))
                    .child(
                        div()
                            .text_size(px(14.0))
                            .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                            .font_weight(FontWeight(600.0))
                            .child("Signer Details"),
                    )
                    .child(Self::build_settings_row("Signer", sig.signer_name.as_deref().unwrap_or("—")))
                    .child(Self::build_settings_row("Issuer", sig.issuer_name.as_deref().unwrap_or("—")))
                    .child(Self::build_settings_row("Serial #", sig.serial_number.as_deref().unwrap_or("—")))
                    .child(Self::build_settings_row("Thumbprint", sig.thumbprint.as_deref().unwrap_or("—")))
                    .child(if let Some(ref from) = sig.valid_from {
                        Self::build_settings_row("Valid From", from).into_any_element()
                    } else {
                        div().into_any_element()
                    })
                    .child(if let Some(ref to) = sig.valid_to {
                        Self::build_settings_row("Valid To", to).into_any_element()
                    } else {
                        div().into_any_element()
                    })
                    // View Details button placeholder
                    .child(
                        div()
                            .mt(px(12.0))
                            .child(Self::build_button("View Details", false, |_, _, _| {})),
                    )
            } else {
                div()
                    .mt(px(4.0))
                    .text_color(rgb(UiConstants::TEXT_MUTED_RGB))
                    .child("Digital signatures help verify the authenticity and integrity of files. Only signed files from trusted publishers are shown.")
            })
    }

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
    fn properties_checkbox(
        label: &str,
        checked: bool,
        on_toggle: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    ) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .gap(px(8.0))
            .cursor_pointer()
            .on_mouse_down(MouseButton::Left, on_toggle)
            .child(
                div()
                    .w(px(18.0))
                    .h(px(18.0))
                    .border(px(1.0))
                    .border_color(rgba(UiConstants::STROKE_RGBA))
                    .rounded(px(3.0))
                    .bg(if checked { rgba(UiConstants::ACCENT_RGBA) } else { rgba(0x12FFFFFFu32) })
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(if checked {
                        div()
                            .text_size(px(12.0))
                            .text_color(rgb(0xFFFFFF))
                            .child("\u{E8FB}") // Checkmark glyph
                    } else {
                        div()
                    }),
            )
            .child(
                div()
                    .text_size(px(13.0))
                    .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                    .child(label.to_string()),
            )
    }

    // ────────── Helper: Dropdown ──────────
    fn properties_dropdown<'a>(
        options: Vec<(&'a str, &'a str)>,
        current_value: &'a str,
        on_select: impl Fn(&str) + 'static,
    ) -> impl IntoElement + 'a {
        let v_on_select = std::sync::Arc::new(on_select);
        let display = options.iter().find(|(_, v)| *v == current_value)
            .map(|(l, _)| (*l).to_string())
            .unwrap_or_else(|| "None".to_string());
        div()
            .flex_col()
            .gap(px(4.0))
            .child(
                div()
                    .flex()
                    .items_center()
                    .h(px(32.0))
                    .px(px(8.0))
                    .bg(rgba(0x12FFFFFFu32))
                    .border(px(1.0))
                    .border_color(rgba(UiConstants::STROKE_RGBA))
                    .rounded(px(4.0))
                    .text_color(rgb(UiConstants::TEXT_PRIMARY_RGB))
                    .text_size(px(13.0))
                    .child(div().child(display))
                    .child(
                        div()
                            .ml(px(8.0))
                            .text_size(px(10.0))
                            .text_color(rgb(UiConstants::TEXT_MUTED_RGB))
                            .child("\u{E70D}"), // dropdown arrow
                    ),
            )
            .child(
                div()
                    .flex_col()
                    .gap(px(2.0))
                    .children(options.iter().map(|(label, value)| {
                        let v_on_select = v_on_select.clone();
                        let val_owned = (*value).to_string();
                        let label_owned = (*label).to_string();
                        let is_current = val_owned == current_value;
                        div()
                            .px(px(8.0))
                            .py(px(2.0))
                            .rounded(px(2.0))
                            .hover(|s| s.bg(rgba(0xFFFFFF15u32)))
                            .cursor_pointer()
                            .on_mouse_down(MouseButton::Left, move |_, _, _| {
                                (v_on_select)(&val_owned);
                            })
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(if is_current {
                                        rgb(UiConstants::ACCENT_COLOR_RGB)
                                    } else {
                                        rgb(UiConstants::TEXT_SECONDARY_RGB)
                                    })
                                    .child(label_owned),
                            )
                    })),
            )
    }

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
                    )
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
                            if let Some(crate::ui::components::dialogs::DialogType::Conflict { ref mut state }) =
                                this.state.active_dialog
                            {
                                state.choose(ConflictResolution::Replace);
                                cx.notify();
                            }
                        });
                    }))
                    .child(Self::build_button("Skip", false, move |_, _, cx| {
                        v_skip.update(cx, |this, cx| {
                            if let Some(crate::ui::components::dialogs::DialogType::Conflict { ref mut state }) =
                                this.state.active_dialog
                            {
                                state.choose(ConflictResolution::Skip);
                                cx.notify();
                            }
                        });
                    }))
                    .child(Self::build_button("Rename", false, move |_, _, cx| {
                        v_rename.update(cx, |this, cx| {
                            if let Some(crate::ui::components::dialogs::DialogType::Conflict { ref mut state }) =
                                this.state.active_dialog
                            {
                                state.choose(ConflictResolution::Rename);
                                cx.notify();
                            }
                        });
                    })),
            )
            // "Apply to all" checkbox
            .child(
                Self::properties_checkbox("Apply to all conflicts", state.apply_to_all, {
                    let v = v_toggle.clone();
                    move |_, _, cx| {
                        v.update(cx, |this, cx| {
                            if let Some(crate::ui::components::dialogs::DialogType::Conflict { ref mut state }) =
                                this.state.active_dialog
                            {
                                state.toggle_apply_to_all();
                                cx.notify();
                            }
                        });
                    }
                }),
            )
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

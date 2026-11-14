//! Home page rendering for MainView (`Files-community` HomePage.xaml parity).
//!
//! Implements the complete `Files-community` modular widget layout:
//! - Collapsible Widget Expanders (`Quick access`, `Drives`, `Network locations`, `File Tags`, `Recent files`)
//! - Respects `HomeWidgetsConfig` visibility toggles from `Settings > General > Widgets`
//! - Interactive `Quick access` Folder Cards (Left-click navigate, Middle-click open in new tab, Pin/Unpin)
//! - Interactive `Drives` Cards with 32x32 shell/bundled icons, live capacity progress bars, and `(i)` Properties button
//! - Interactive `Network locations` Cards with Fluent InfoBar empty state & one-click `Disable` action
//! - Interactive `File Tags` Container Pills with configured tag colors, item counts, and quick filtering
//! - Interactive `Recent files` Table with real session recent paths and Amber Warning InfoBar empty state

use gpui::*;
use std::path::{Path, PathBuf};

use crate::core::path_util::FilesPath;
use crate::platform::windows::drive_service::{DriveInfo, DriveType};
use crate::ui::components::dialogs::{SettingsPreReadData, SettingsTab};
use crate::ui::components::sidebar_state::{bundled_icon_path, SidebarItemId};
use crate::ui::constants::UiConstants;

use super::main_view::{MainView, MainViewState};

#[derive(Debug, Clone, PartialEq, Eq)]
struct QuickAccessCard {
    label: String,
    glyph: &'static str,
    accent_rgb: u32,
    path: PathBuf,
    is_custom_pinned: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RecentFileEntry {
    name: String,
    parent: String,
    full_path: String,
    modified: &'static str,
    is_dir: bool,
}

/// Resolve a Home widget icon path in priority order:
/// `state.home_shell_icons` -> `state.icon_paths` -> bundled PNG asset (`folder_icon.png` / `sidebar_drives.png`).
fn resolve_home_icon(
    state: &MainViewState,
    path: &FilesPath,
    bundled_name: &str,
) -> Option<PathBuf> {
    state
        .home_shell_icons
        .get(path)
        .or_else(|| state.icon_paths.get(path))
        .cloned()
        .or_else(|| bundled_icon_path(bundled_name))
}

/// Render a 32x32 icon for a Home widget card, using `gpui::img` when a shell or bundled icon is available
/// and falling back to a Fluent glyph.
fn render_home_icon_32(
    icon_path: Option<PathBuf>,
    fallback_glyph: &str,
    fallback_rgb: u32,
) -> AnyElement {
    let mut container = div()
        .w(px(32.0))
        .h(px(32.0))
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center();

    if let Some(path) = icon_path {
        container = container.child(
            img(path)
                .w(px(32.0))
                .h(px(32.0))
                .object_fit(gpui::ObjectFit::Contain),
        );
    } else {
        container = container.child(
            div()
                .text_size(px(20.0))
                .font_family(UiConstants::FONT_ICONS)
                .text_color(rgb(fallback_rgb))
                .child(fallback_glyph.to_string()),
        );
    }

    container.into_any_element()
}

/// Resolve a standard user folder matching `SidebarState`'s known-folder fallback logic so its
/// `FilesPath` aligns with cached shell icons even when OneDrive folder redirection is active.
fn resolve_standard_folder(
    lib_name: &str,
    fallback_dir: Option<PathBuf>,
    home: &Path,
) -> PathBuf {
    SidebarItemId::Library(lib_name.to_string())
        .get_path()
        .map(|p| p.as_path().to_path_buf())
        .or(fallback_dir)
        .unwrap_or_else(|| home.join(lib_name))
}

/// Build the ordered list of Quick Access folder cards, appending deduplicated user-pinned folders.
fn collect_quick_access_cards(pinned_folders: &[String]) -> Vec<QuickAccessCard> {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("C:\\Users\\User"));
    let mut cards = vec![
        QuickAccessCard {
            label: "Desktop".to_string(),
            glyph: "\u{E8FC}",
            accent_rgb: 0x60CDFF,
            path: resolve_standard_folder("Desktop", dirs::desktop_dir(), &home),
            is_custom_pinned: false,
        },
        QuickAccessCard {
            label: "Downloads".to_string(),
            glyph: "\u{E896}",
            accent_rgb: 0x4CCB5E,
            path: resolve_standard_folder("Downloads", dirs::download_dir(), &home),
            is_custom_pinned: false,
        },
        QuickAccessCard {
            label: "Documents".to_string(),
            glyph: "\u{E8A5}",
            accent_rgb: 0xF7B93E,
            path: resolve_standard_folder("Documents", dirs::document_dir(), &home),
            is_custom_pinned: false,
        },
        QuickAccessCard {
            label: "Pictures".to_string(),
            glyph: "\u{EB9F}",
            accent_rgb: 0xB4A0FF,
            path: resolve_standard_folder("Pictures", dirs::picture_dir(), &home),
            is_custom_pinned: false,
        },
        QuickAccessCard {
            label: "Music".to_string(),
            glyph: "\u{EC4F}",
            accent_rgb: 0xFF7B9C,
            path: resolve_standard_folder("Music", dirs::audio_dir(), &home),
            is_custom_pinned: false,
        },
        QuickAccessCard {
            label: "Videos".to_string(),
            glyph: "\u{E714}",
            accent_rgb: 0xFF9E5E,
            path: resolve_standard_folder("Videos", dirs::video_dir(), &home),
            is_custom_pinned: false,
        },
    ];

    for pinned in pinned_folders {
        let trimmed = pinned.trim();
        if trimmed.is_empty() {
            continue;
        }
        let pb = PathBuf::from(trimmed);
        if !cards.iter().any(|c| c.path == pb) {
            let label = pb
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| trimmed.to_string());
            cards.push(QuickAccessCard {
                label,
                glyph: "\u{E8B7}",
                accent_rgb: 0xFCCB45,
                path: pb,
                is_custom_pinned: true,
            });
        }
    }

    cards
}

/// Split cached drives into `(local_and_removable_drives, network_drives)` to mirror
/// `Files-community`'s `DrivesWidgetViewModel` and `NetworkLocationsWidgetViewModel`.
fn partition_home_drives(cached_drives: &[DriveInfo]) -> (Vec<DriveInfo>, Vec<DriveInfo>) {
    let mut local = Vec::new();
    let mut network = Vec::new();
    for drive in cached_drives {
        if drive.drive_type == DriveType::Network {
            network.push(drive.clone());
        } else {
            local.push(drive.clone());
        }
    }
    (local, network)
}

/// Filter persisted session `recent_paths` into displayable Recent Files entries (max 12),
/// excluding internal virtual views (`Home`, `Settings`, `RecycleBin`).
fn collect_recent_entries(recent_paths: &[String]) -> Vec<RecentFileEntry> {
    let mut entries = Vec::new();
    for p_str in recent_paths {
        if entries.len() >= 12 {
            break;
        }
        if p_str.is_empty()
            || p_str == "Home"
            || p_str == "Settings"
            || p_str.contains("RecycleBin")
        {
            continue;
        }
        let pb = PathBuf::from(p_str);
        let name = pb
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| p_str.clone());
        let parent = pb
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| p_str.clone());
        let is_dir =
            p_str.ends_with('\\') || p_str.ends_with('/') || pb.extension().is_none();
        entries.push(RecentFileEntry {
            name,
            parent,
            full_path: p_str.clone(),
            modified: "Recently accessed",
            is_dir,
        });
    }
    entries
}

/// Shared Fluent `InfoBar` card used for empty/disabled Home widget states.
fn render_home_infobar(
    bg_rgba: u32,
    border_rgba: u32,
    icon_glyph: &str,
    icon_rgb: u32,
    message: &str,
    text_rgb: u32,
    action_button: Option<AnyElement>,
) -> Div {
    let left = div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(10.0))
        .flex_1()
        .min_w(px(0.0))
        .child(
            div()
                .text_size(px(16.0))
                .font_family(UiConstants::FONT_ICONS)
                .text_color(rgb(icon_rgb))
                .flex_shrink_0()
                .child(icon_glyph.to_string()),
        )
        .child(
            div()
                .text_size(px(13.0))
                .font_family(UiConstants::FONT_MAIN)
                .text_color(rgb(text_rgb))
                .child(message.to_string()),
        );

    let mut row = div()
        .w_full()
        .min_h(px(46.0))
        .px(px(14.0))
        .py(px(10.0))
        .rounded(px(6.0))
        .bg(rgba(bg_rgba))
        .border_1()
        .border_color(rgba(border_rgba))
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .gap(px(12.0))
        .child(left);

    if let Some(btn) = action_button {
        row = row.child(btn);
    }

    row
}

impl MainView {
    pub(crate) fn render_home_view(
        state: &MainViewState,
        settings_data: Option<&SettingsPreReadData>,
        view: Entity<crate::app::AppWindow>,
    ) -> impl IntoElement {
        let (
            show_quick_access,
            show_drives,
            show_network,
            show_tags,
            show_recent,
            pinned_folders,
            recent_paths,
            defined_tags,
            expanded_sections,
        ) = if let Some(d) = settings_data {
            (
                d.config.home_widgets.show_quick_access,
                d.config.home_widgets.show_drives,
                d.config.home_widgets.show_network_locations,
                d.config.home_widgets.show_file_tags,
                d.config.home_widgets.show_recent_files,
                d.config.sidebar.pinned_locations.clone(),
                d.config.session.recent_paths.clone(),
                d.config.tag_definitions.clone(),
                d.ui_state.expanded_sections.clone(),
            )
        } else {
            let mut default_exp = std::collections::HashSet::new();
            for k in [
                "home_quick_access",
                "home_drives",
                "home_network",
                "home_file_tags",
                "home_recent_files",
            ] {
                default_exp.insert(k.to_string());
            }
            (
                true,
                true,
                true,
                true,
                true,
                Vec::new(),
                Vec::new(),
                Vec::new(),
                default_exp,
            )
        };

        // Merge state.home_tags_cache with defined_tags from config so colors & zero-count tags are always accurate
        let mut tags: Vec<(String, String, i64)> = Vec::new();
        for def in &defined_tags {
            let count = state
                .home_tags_cache
                .iter()
                .find(|(n, _, _)| n.eq_ignore_ascii_case(&def.name))
                .map(|(_, _, c)| *c)
                .unwrap_or(0);
            tags.push((def.name.clone(), def.color.clone(), count));
        }
        for (name, color, count) in &state.home_tags_cache {
            if !tags.iter().any(|(n, _, _)| n.eq_ignore_ascii_case(name)) {
                tags.push((name.clone(), color.clone(), *count));
            }
        }

        let mut widgets: Vec<AnyElement> = Vec::new();

        if show_quick_access {
            let expanded = expanded_sections.contains("home_quick_access");
            widgets.push(
                Self::build_quick_access_widget(state, expanded, &pinned_folders, view.clone())
                    .into_any_element(),
            );
        }

        if show_drives {
            let expanded = expanded_sections.contains("home_drives");
            widgets.push(
                Self::build_drives_widget(state, expanded, &state.cached_drives, view.clone())
                    .into_any_element(),
            );
        }

        if show_network {
            let expanded = expanded_sections.contains("home_network");
            widgets.push(
                Self::build_network_widget_section(
                    state,
                    expanded,
                    &state.cached_drives,
                    view.clone(),
                )
                .into_any_element(),
            );
        }

        if show_tags {
            let expanded = expanded_sections.contains("home_file_tags");
            widgets.push(
                Self::build_file_tags_widget(expanded, tags, view.clone()).into_any_element(),
            );
        }

        if show_recent {
            let expanded = expanded_sections.contains("home_recent_files");
            widgets.push(
                Self::build_recent_files_widget(state, expanded, &recent_paths, view.clone())
                    .into_any_element(),
            );
        }

        if widgets.is_empty() {
            let v_open_settings = view.clone();
            widgets.push(
                div()
                    .w_full()
                    .py(px(48.0))
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap(px(12.0))
                    .child(
                        div()
                            .text_size(px(36.0))
                            .font_family(UiConstants::FONT_ICONS)
                            .text_color(rgba(UiConstants::TEXT_MUTED_RGBA))
                            .child("\u{E80F}"),
                    )
                    .child(
                        div()
                            .text_size(px(15.0))
                            .font_weight(FontWeight(600.0))
                            .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                            .child("All Home widgets are hidden"),
                    )
                    .child(
                        div()
                            .px(px(14.0))
                            .py(px(6.0))
                            .rounded(px(6.0))
                            .bg(rgba(UiConstants::ACCENT_RGBA))
                            .text_size(px(12.5))
                            .font_weight(FontWeight(600.0))
                            .text_color(rgb(0xFFFFFF))
                            .cursor_pointer()
                            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                v_open_settings.update(cx, |this, cx| {
                                    this.set_settings_tab(SettingsTab::General, cx);
                                    this.open_settings_tab(cx);
                                });
                            })
                            .child("Configure Widgets in Settings"),
                    )
                    .into_any_element(),
            );
        }

        div()
            .id("home-scroll")
            .flex()
            .flex_col()
            .flex_grow()
            .w_full()
            .h_full()
            .px(px(24.0))
            .py(px(18.0))
            .gap(px(18.0))
            .min_h(px(0.0))
            .overflow_y_scroll()
            .track_scroll(&state.scroll_handle)
            .on_scroll_wheel(crate::ui::components::bouncy_scroll::smooth_scroll_handle_wheel(
                view.clone(),
                |app_state| &mut app_state.main_view.bouncy_scroll,
                |app_state| &app_state.main_view.scroll_handle,
            ))
            .children(widgets)
    }

    /// Reusable `Files-community` collapsible Widget Container with clean chevron + title header
    /// and optional right-side action button.
    fn build_widget_container(
        expander_key: &'static str,
        expanded: bool,
        title: &str,
        right_action: Option<AnyElement>,
        body: AnyElement,
        view: Entity<crate::app::AppWindow>,
    ) -> Div {
        let v_toggle = view.clone();
        let left = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.0))
            .px(px(6.0))
            .py(px(4.0))
            .rounded(px(4.0))
            .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
            .cursor_pointer()
            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                v_toggle.update(cx, |this, cx| {
                    this.state.settings_ui.toggle_expander(expander_key);
                    cx.notify();
                });
            })
            .child(
                div()
                    .w(px(18.0))
                    .h(px(18.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .text_size(px(10.5))
                            .font_family(UiConstants::FONT_ICONS)
                            .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                            .child(if expanded { "\u{E70D}" } else { "\u{E76C}" }),
                    ),
            )
            .child(
                div()
                    .text_size(px(13.0))
                    .font_weight(FontWeight(600.0))
                    .font_family(UiConstants::FONT_MAIN)
                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                    .child(title.to_string()),
            );

        let mut header = div()
            .w_full()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .pb(px(2.0))
            .child(left);

        if let Some(act) = right_action {
            header = header.child(act);
        }

        let mut container = div().w_full().flex().flex_col().gap(px(8.0)).child(header);
        if expanded {
            container = container.child(body);
        }
        container
    }

    /// 1. Quick access (Folders) Widget (`QuickAccessWidget.xaml` parity).
    fn build_quick_access_widget(
        state: &MainViewState,
        expanded: bool,
        pinned_folders: &[String],
        view: Entity<crate::app::AppWindow>,
    ) -> Div {
        let all_cards = collect_quick_access_cards(pinned_folders);
        let mut grid = div().w_full().flex().flex_row().flex_wrap().gap(px(10.0));

        for card in all_cards {
            let path_fp = FilesPath::from(card.path.clone());
            let parent_sub = card
                .path
                .parent()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|| card.path.to_string_lossy().to_string());
            let icon_path = resolve_home_icon(state, &path_fp, "folder_icon.png");

            let v_click = view.clone();
            let v_mid = view.clone();
            let v_unpin = view.clone();
            let p_click = path_fp.clone();
            let p_mid = path_fp.clone();
            let p_unpin = path_fp.clone();
            let is_custom_pinned = card.is_custom_pinned;

            grid = grid.child(
                div()
                    .w(px(208.0))
                    .h(px(62.0))
                    .px(px(12.0))
                    .py(px(10.0))
                    .rounded(px(6.0))
                    .bg(rgba(0xFFFFFF0Cu32))
                    .border_1()
                    .border_color(rgba(0xFFFFFF12u32))
                    .hover(|s| {
                        s.bg(rgba(0xFFFFFF14u32))
                            .border_color(rgba(0xFFFFFF20u32))
                    })
                    .cursor_pointer()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .gap(px(10.0))
                    .on_mouse_down(MouseButton::Left, move |ev, _, cx| {
                        let p = p_click.clone();
                        let ctrl = ev.modifiers.control;
                        v_click.update(cx, |this, cx| {
                            if ctrl {
                                this.open_in_new_tab(p, cx);
                            } else {
                                this.navigate(p, cx);
                            }
                        });
                    })
                    .on_mouse_down(MouseButton::Middle, move |_, _, cx| {
                        let p = p_mid.clone();
                        v_mid.update(cx, |this, cx| {
                            this.open_in_new_tab(p, cx);
                        });
                    })
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(10.0))
                            .flex_1()
                            .min_w(px(0.0))
                            .child(render_home_icon_32(
                                icon_path,
                                card.glyph,
                                card.accent_rgb,
                            ))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(1.0))
                                    .flex_1()
                                    .min_w(px(0.0))
                                    .overflow_hidden()
                                    .child(
                                        div()
                                            .text_size(px(13.0))
                                            .font_weight(FontWeight(600.0))
                                            .font_family(UiConstants::FONT_MAIN)
                                            .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                                            .overflow_hidden()
                                            .whitespace_nowrap()
                                            .text_ellipsis()
                                            .child(card.label),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(11.0))
                                            .text_color(rgba(UiConstants::TEXT_MUTED_RGBA))
                                            .overflow_hidden()
                                            .whitespace_nowrap()
                                            .text_ellipsis()
                                            .child(parent_sub),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .w(px(22.0))
                            .h(px(22.0))
                            .rounded(px(4.0))
                            .flex_shrink_0()
                            .flex()
                            .items_center()
                            .justify_center()
                            .hover(|s| s.bg(rgba(0xFFFFFF18u32)))
                            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                if is_custom_pinned {
                                    cx.stop_propagation();
                                    let p = p_unpin.clone();
                                    v_unpin.update(cx, |this, cx| {
                                        this.unpin_folder(p, cx);
                                    });
                                }
                            })
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .font_family(UiConstants::FONT_ICONS)
                                    .text_color(if is_custom_pinned {
                                        rgb(0x60CDFF)
                                    } else {
                                        rgba(UiConstants::TEXT_MUTED_RGBA)
                                    })
                                    .child("\u{E718}"), // Pin icon
                            ),
                    ),
            );
        }

        Self::build_widget_container(
            "home_quick_access",
            expanded,
            "Quick access",
            None,
            grid.into_any_element(),
            view,
        )
    }

    /// Shared 250x72 Fluent drive card used by both `Drives` and `Network locations`.
    fn render_drive_card(
        state: &MainViewState,
        drive: DriveInfo,
        view: Entity<crate::app::AppWindow>,
    ) -> Div {
        let root_fp = drive.root.clone();
        let display_title = drive.display_name();
        let capacity_text = drive.capacity_summary();
        let ratio = if drive.total_bytes == 0 {
            0.0
        } else {
            (drive.usage_percent() / 100.0).clamp(0.02, 1.0)
        };
        let bar_color = if drive.is_low_storage() || drive.usage_percent() >= 90.0 {
            0xE81123u32
        } else if drive.usage_percent() >= 78.0 {
            0xF7931Eu32
        } else {
            0x60CDFFu32
        };
        let icon_path = resolve_home_icon(state, &root_fp, "sidebar_drives.png");
        let icon_glyph = drive.icon();

        let v_click = view.clone();
        let v_mid = view.clone();
        let v_info = view.clone();
        let p_click = root_fp.clone();
        let p_mid = root_fp.clone();
        let p_info = root_fp;

        div()
            .w(px(250.0))
            .h(px(72.0))
            .px(px(12.0))
            .py(px(7.0))
            .rounded(px(6.0))
            .bg(rgba(0xFFFFFF0Cu32))
            .border_1()
            .border_color(rgba(0xFFFFFF12u32))
            .hover(|s| {
                s.bg(rgba(0xFFFFFF14u32))
                    .border_color(rgba(0xFFFFFF20u32))
            })
            .cursor_pointer()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(10.0))
            .on_mouse_down(MouseButton::Left, move |ev, _, cx| {
                let p = p_click.clone();
                if ev.modifiers.control {
                    v_click.update(cx, |this, cx| this.open_in_new_tab(p, cx));
                } else {
                    v_click.update(cx, |this, cx| this.navigate(p, cx));
                }
            })
            .on_mouse_down(MouseButton::Middle, move |_, _, cx| {
                let p = p_mid.clone();
                v_mid.update(cx, |this, cx| this.open_in_new_tab(p, cx));
            })
            .child(render_home_icon_32(icon_path, icon_glyph, 0x60CDFF))
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.0))
                    .flex()
                    .flex_col()
                    .justify_center()
                    .gap(px(2.0))
                    .child(
                        div()
                            .text_size(px(13.0))
                            .font_weight(FontWeight(600.0))
                            .font_family(UiConstants::FONT_MAIN)
                            .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .child(display_title),
                    )
                    .child(
                        div()
                            .w_full()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(6.0))
                            .child(
                                div()
                                    .flex_1()
                                    .h(px(5.0))
                                    .rounded(px(2.5))
                                    .bg(rgba(0xFFFFFF18u32))
                                    .overflow_hidden()
                                    .child(
                                        div()
                                            .h_full()
                                            .w(relative(ratio as f32))
                                            .rounded(px(2.5))
                                            .bg(rgb(bar_color)),
                                    ),
                            )
                            .child(
                                div()
                                    .w(px(18.0))
                                    .h(px(18.0))
                                    .rounded(px(4.0))
                                    .flex_shrink_0()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .cursor_pointer()
                                    .text_size(px(12.0))
                                    .font_family(UiConstants::FONT_ICONS)
                                    .text_color(rgb(0xA0A0A0))
                                    .hover(|s| {
                                        s.bg(rgba(0xFFFFFF16u32)).text_color(rgb(0xFFFFFF))
                                    })
                                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                        cx.stop_propagation();
                                        let p = p_info.clone();
                                        v_info.update(cx, |this, cx| {
                                            this.open_properties_for_path(p, cx);
                                        });
                                    })
                                    .child("\u{E946}"),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(11.5))
                            .font_family(UiConstants::FONT_MAIN)
                            .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .child(capacity_text),
                    ),
            )
    }

    /// 2. Drives Widget (`DrivesWidget.xaml` parity) with live capacity bars, `(i)` properties button,
    /// and Storage Sense button. Uses `state.cached_drives` so `render()` never blocks on Win32 queries.
    fn build_drives_widget(
        state: &MainViewState,
        expanded: bool,
        cached_drives: &[DriveInfo],
        view: Entity<crate::app::AppWindow>,
    ) -> Div {
        let (mut drives, _) = partition_home_drives(cached_drives);
        if cached_drives.is_empty() {
            drives = vec![
                DriveInfo {
                    root: FilesPath::new("C:\\"),
                    label: "Windows".to_string(),
                    drive_type: DriveType::Fixed,
                    file_system: "NTFS".to_string(),
                    total_bytes: 512 * 1024 * 1024 * 1024,
                    free_bytes: 186 * 1024 * 1024 * 1024,
                    is_ready: true,
                },
                DriveInfo {
                    root: FilesPath::new("D:\\"),
                    label: "Data".to_string(),
                    drive_type: DriveType::Fixed,
                    file_system: "NTFS".to_string(),
                    total_bytes: 1024 * 1024 * 1024 * 1024,
                    free_bytes: 740 * 1024 * 1024 * 1024,
                    is_ready: true,
                },
            ];
        }

        let mut grid = div().w_full().flex().flex_row().flex_wrap().gap(px(10.0));
        for drive in drives {
            grid = grid.child(Self::render_drive_card(state, drive, view.clone()));
        }

        let v_storage = view.clone();
        let storage_btn = div()
            .px(px(10.0))
            .py(px(4.0))
            .rounded(px(5.0))
            .bg(rgba(0xFFFFFF0Cu32))
            .border_1()
            .border_color(rgba(UiConstants::CARD_STROKE_RGBA))
            .hover(|s| s.bg(rgba(0xFFFFFF16u32)))
            .cursor_pointer()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.0))
            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                v_storage.update(cx, |this, cx| {
                    this.dispatch_action(
                        crate::core::action_registry::ActionKind::OpenStorageSense,
                        cx,
                    );
                });
            })
            .child(
                div()
                    .text_size(px(12.0))
                    .font_family(UiConstants::FONT_ICONS)
                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                    .child("\u{E770}"),
            )
            .child(
                div()
                    .text_size(px(11.5))
                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                    .child("Storage Sense"),
            )
            .into_any_element();

        Self::build_widget_container(
            "home_drives",
            expanded,
            "Drives",
            Some(storage_btn),
            grid.into_any_element(),
            view,
        )
    }

    /// 3. Network locations Widget (`NetworkLocationsWidget.xaml` parity).
    fn build_network_widget_section(
        state: &MainViewState,
        expanded: bool,
        cached_drives: &[DriveInfo],
        view: Entity<crate::app::AppWindow>,
    ) -> Div {
        let (_, network_drives) = partition_home_drives(cached_drives);

        let body: AnyElement = if network_drives.is_empty() {
            let v_disable = view.clone();
            let disable_btn = div()
                .px(px(12.0))
                .py(px(5.0))
                .rounded(px(4.0))
                .bg(rgba(0xFFFFFF12u32))
                .border_1()
                .border_color(rgba(0xFFFFFF18u32))
                .hover(|s| s.bg(rgba(0xFFFFFF1Cu32)))
                .cursor_pointer()
                .flex_shrink_0()
                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                    cx.stop_propagation();
                    v_disable.update(cx, |this, cx| {
                        this.toggle_setting("widget_network", cx);
                    });
                })
                .child(
                    div()
                        .text_size(px(12.5))
                        .font_family(UiConstants::FONT_MAIN)
                        .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                        .child("Disable"),
                )
                .into_any_element();

            render_home_infobar(
                0xFFFFFF08u32,
                0xFFFFFF12u32,
                "\u{E946}",
                0x60CDFF,
                "There are no network locations. If you don't use network locations, you can disable the widget.",
                0xE5E5E5,
                Some(disable_btn),
            )
            .into_any_element()
        } else {
            let mut row = div().w_full().flex().flex_row().flex_wrap().gap(px(10.0));
            for drive in network_drives {
                row = row.child(Self::render_drive_card(state, drive, view.clone()));
            }
            row.into_any_element()
        };

        Self::build_widget_container(
            "home_network",
            expanded,
            "Network locations",
            None,
            body,
            view,
        )
    }

    /// 4. File Tags Widget (`FileTagsWidget.xaml` parity).
    pub(crate) fn build_file_tags_widget(
        expanded: bool,
        tags: Vec<(String, String, i64)>,
        view: Entity<crate::app::AppWindow>,
    ) -> Div {
        let mut tag_cards = div().w_full().flex().flex_row().gap(px(10.0)).flex_wrap();

        for (name, color, count) in tags {
            let v_click = view.clone();
            let tag_name = name.clone();
            let chip = Self::build_tag_chip(name, color, count).on_mouse_down(
                MouseButton::Left,
                move |_, _, cx| {
                    let t = tag_name.clone();
                    v_click.update(cx, |this, cx| {
                        if count > 0 {
                            this.open_all_tagged_items(t, cx);
                        } else {
                            this.set_settings_tab(SettingsTab::Tags, cx);
                            this.open_settings_tab(cx);
                        }
                    });
                },
            );
            tag_cards = tag_cards.child(chip);
        }

        let v_manage_tags = view.clone();
        let manage_btn = div()
            .px(px(10.0))
            .py(px(4.0))
            .rounded(px(5.0))
            .bg(rgba(0xFFFFFF0Cu32))
            .border_1()
            .border_color(rgba(UiConstants::CARD_STROKE_RGBA))
            .hover(|s| s.bg(rgba(0xFFFFFF16u32)))
            .cursor_pointer()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.0))
            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                v_manage_tags.update(cx, |this, cx| {
                    this.set_settings_tab(SettingsTab::Tags, cx);
                    this.open_settings_tab(cx);
                });
            })
            .child(
                div()
                    .text_size(px(12.0))
                    .font_family(UiConstants::FONT_ICONS)
                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                    .child("\u{E8EC}"),
            )
            .child(
                div()
                    .text_size(px(11.5))
                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                    .child("Manage Tags"),
            )
            .into_any_element();

        Self::build_widget_container(
            "home_file_tags",
            expanded,
            "File Tags",
            Some(manage_btn),
            tag_cards.into_any_element(),
            view,
        )
    }

    pub(crate) fn build_tag_chip(name: String, color: String, count: i64) -> Div {
        let hex_color = color.trim_start_matches('#');
        let r = hex_color
            .get(0..2)
            .and_then(|s| u8::from_str_radix(s, 16).ok())
            .unwrap_or(0x00);
        let g = hex_color
            .get(2..4)
            .and_then(|s| u8::from_str_radix(s, 16).ok())
            .unwrap_or(0x78);
        let b = hex_color
            .get(4..6)
            .and_then(|s| u8::from_str_radix(s, 16).ok())
            .unwrap_or(0xD4);
        let rgb_value = ((r as u32) << 16) | ((g as u32) << 8) | (b as u32);

        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.0))
            .px(px(12.0))
            .py(px(7.0))
            .bg(rgba(0xFFFFFF09u32))
            .rounded(px(8.0))
            .border_1()
            .border_color(rgba(UiConstants::CARD_STROKE_RGBA))
            .hover(|s| {
                s.bg(rgba(0xFFFFFF14u32))
                    .border_color(rgba((rgb_value << 8) | 0x88))
            })
            .cursor_pointer()
            .child(
                div()
                    .w(px(10.0))
                    .h(px(10.0))
                    .rounded(px(5.0))
                    .bg(rgb(rgb_value)),
            )
            .child(
                div()
                    .text_size(px(13.0))
                    .font_weight(FontWeight(500.0))
                    .font_family(UiConstants::FONT_MAIN)
                    .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                    .child(name),
            )
            .child(
                div()
                    .px(px(6.0))
                    .py(px(1.0))
                    .rounded(px(8.0))
                    .bg(rgba(0xFFFFFF10u32))
                    .text_size(px(11.0))
                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                    .child(if count == 1 {
                        "1 item".to_string()
                    } else {
                        format!("{} items", count)
                    }),
            )
    }

    /// 5. Recent files Widget (`RecentFilesWidget.xaml` parity).
    fn build_recent_files_widget(
        state: &MainViewState,
        expanded: bool,
        recent_paths: &[String],
        view: Entity<crate::app::AppWindow>,
    ) -> Div {
        let entries = collect_recent_entries(recent_paths);

        if entries.is_empty() {
            let warning_bar = render_home_infobar(
                0x433519CCu32,
                0xF7B93E44u32,
                "\u{E7BA}",
                0xFCE100,
                "Recently used files is currently disabled in Windows File Explorer.",
                0xF5F5F5,
                None,
            );
            return Self::build_widget_container(
                "home_recent_files",
                expanded,
                "Recent files",
                None,
                warning_bar.into_any_element(),
                view,
            );
        }

        let mut table = div()
            .w_full()
            .flex()
            .flex_col()
            .bg(rgba(0xFFFFFF09u32))
            .rounded(px(8.0))
            .border_1()
            .border_color(rgba(UiConstants::CARD_STROKE_RGBA))
            .overflow_hidden()
            .child(
                // Table Header
                div()
                    .w_full()
                    .h(px(34.0))
                    .px(px(14.0))
                    .flex()
                    .flex_row()
                    .items_center()
                    .border_b(px(1.0))
                    .border_color(rgba(UiConstants::CARD_STROKE_RGBA))
                    .bg(rgba(0xFFFFFF05u32))
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(11.5))
                            .font_weight(FontWeight(600.0))
                            .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                            .child("Name"),
                    )
                    .child(
                        div()
                            .w(px(280.0))
                            .text_size(px(11.5))
                            .font_weight(FontWeight(600.0))
                            .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                            .child("Original Location"),
                    )
                    .child(
                        div()
                            .w(px(140.0))
                            .text_size(px(11.5))
                            .font_weight(FontWeight(600.0))
                            .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                            .child("Date Accessed"),
                    )
                    .child(
                        div()
                            .w(px(68.0))
                            .text_right()
                            .text_size(px(11.5))
                            .font_weight(FontWeight(600.0))
                            .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                            .child("Actions"),
                    ),
            );

        for (idx, entry) in entries.into_iter().enumerate() {
            let entry_fp = FilesPath::new(&entry.full_path);
            let cached_icon = state
                .home_shell_icons
                .get(&entry_fp)
                .or_else(|| state.icon_paths.get(&entry_fp))
                .cloned();
            let v_open = view.clone();
            let v_loc = view.clone();
            let v_rem = view.clone();
            let p_open = entry.full_path.clone();
            let p_loc = entry.parent.clone();
            let p_rem = entry.full_path;

            let icon_el: AnyElement = if let Some(icon_path) = cached_icon {
                img(icon_path)
                    .w(px(16.0))
                    .h(px(16.0))
                    .object_fit(gpui::ObjectFit::Contain)
                    .into_any_element()
            } else {
                div()
                    .text_size(px(15.0))
                    .font_family(UiConstants::FONT_ICONS)
                    .text_color(if entry.is_dir {
                        rgb(0xFCCB45)
                    } else {
                        rgb(0x60CDFF)
                    })
                    .child(if entry.is_dir { "\u{E8B7}" } else { "\u{E8A5}" })
                    .into_any_element()
            };

            table = table.child(
                div()
                    .w_full()
                    .h(px(38.0))
                    .px(px(14.0))
                    .flex()
                    .flex_row()
                    .items_center()
                    .border_t(if idx > 0 { px(1.0) } else { px(0.0) })
                    .border_color(rgba(0xFFFFFF08u32))
                    .hover(|s| s.bg(rgba(0xFFFFFF0Cu32)))
                    .cursor_pointer()
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        let p = FilesPath::new(&p_open);
                        v_open.update(cx, |this, cx| {
                            this.navigate(p, cx);
                        });
                    })
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.0))
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(10.0))
                            .child(icon_el)
                            .child(
                                div()
                                    .text_size(px(13.0))
                                    .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                                    .overflow_hidden()
                                    .whitespace_nowrap()
                                    .text_ellipsis()
                                    .child(entry.name),
                            ),
                    )
                    .child(
                        div()
                            .w(px(280.0))
                            .pr(px(12.0))
                            .text_size(px(12.0))
                            .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .child(entry.parent),
                    )
                    .child(
                        div()
                            .w(px(140.0))
                            .text_size(px(12.0))
                            .text_color(rgba(UiConstants::TEXT_MUTED_RGBA))
                            .child(entry.modified),
                    )
                    .child(
                        div()
                            .w(px(68.0))
                            .flex()
                            .flex_row()
                            .items_center()
                            .justify_end()
                            .gap(px(4.0))
                            .child(
                                div()
                                    .w(px(26.0))
                                    .h(px(26.0))
                                    .rounded(px(4.0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .hover(|s| s.bg(rgba(0xFFFFFF18u32)))
                                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                        cx.stop_propagation();
                                        let p = FilesPath::new(&p_loc);
                                        v_loc.update(cx, |this, cx| {
                                            this.navigate(p, cx);
                                        });
                                    })
                                    .child(
                                        div()
                                            .text_size(px(12.0))
                                            .font_family(UiConstants::FONT_ICONS)
                                            .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                            .child("\u{E8DA}"), // Open folder location
                                    ),
                            )
                            .child(
                                div()
                                    .w(px(26.0))
                                    .h(px(26.0))
                                    .rounded(px(4.0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .hover(|s| s.bg(rgba(0xE8112333u32)))
                                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                        cx.stop_propagation();
                                        let p = p_rem.clone();
                                        v_rem.update(cx, |this, cx| {
                                            this.remove_recent_file(p, cx);
                                        });
                                    })
                                    .child(
                                        div()
                                            .text_size(px(11.0))
                                            .font_family(UiConstants::FONT_ICONS)
                                            .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                            .child("\u{E711}"), // Remove from list
                                    ),
                            ),
                    ),
            );
        }

        let v_clear = view.clone();
        let clear_btn = div()
            .px(px(10.0))
            .py(px(4.0))
            .rounded(px(5.0))
            .bg(rgba(0xFFFFFF0Cu32))
            .border_1()
            .border_color(rgba(UiConstants::CARD_STROKE_RGBA))
            .hover(|s| s.bg(rgba(0xFFFFFF16u32)))
            .cursor_pointer()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.0))
            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                v_clear.update(cx, |this, cx| {
                    this.clear_recent_files(cx);
                });
            })
            .child(
                div()
                    .text_size(px(11.5))
                    .font_family(UiConstants::FONT_ICONS)
                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                    .child("\u{E74D}"),
            )
            .child(
                div()
                    .text_size(px(11.5))
                    .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                    .child("Clear Recent"),
            )
            .into_any_element();

        Self::build_widget_container(
            "home_recent_files",
            expanded,
            "Recent files",
            Some(clear_btn),
            table.into_any_element(),
            view,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::config::ViewMode;

    fn make_drive(root: &str, label: &str, drive_type: DriveType) -> DriveInfo {
        DriveInfo {
            root: FilesPath::new(root),
            label: label.to_string(),
            drive_type,
            total_bytes: 1_000_000_000,
            free_bytes: 400_000_000,
            file_system: "NTFS".to_string(),
            is_ready: true,
        }
    }

    #[::core::prelude::rust_2024::test]
    fn test_resolve_home_icon_precedence() {
        let mut state = MainViewState::new(FilesPath::new("Home"), ViewMode::Details);
        let key = FilesPath::new("C:\\");
        let shell_png = PathBuf::from("shell_drive.png");
        let dir_png = PathBuf::from("dir_drive.png");

        state.icon_paths.insert(key.clone(), dir_png.clone());
        assert_eq!(
            resolve_home_icon(&state, &key, "sidebar_drives.png"),
            Some(dir_png)
        );

        state.home_shell_icons.insert(key.clone(), shell_png.clone());
        assert_eq!(
            resolve_home_icon(&state, &key, "sidebar_drives.png"),
            Some(shell_png)
        );

        let uncached = FilesPath::new("Z:\\UncachedDrive");
        assert_eq!(
            resolve_home_icon(&state, &uncached, "sidebar_drives.png"),
            bundled_icon_path("sidebar_drives.png")
        );
        assert_eq!(
            resolve_home_icon(&state, &uncached, "folder_icon.png"),
            bundled_icon_path("folder_icon.png")
        );
        assert_eq!(
            resolve_home_icon(&state, &uncached, "nonexistent_home_icon_xyz.png"),
            None
        );
    }

    #[::core::prelude::rust_2024::test]
    fn test_partition_home_drives_separates_network() {
        let input = vec![
            make_drive("C:\\", "Windows", DriveType::Fixed),
            make_drive("Z:\\", "NAS", DriveType::Network),
            make_drive("D:\\", "Data", DriveType::Fixed),
        ];
        let (local, network) = partition_home_drives(&input);
        assert_eq!(local.len(), 2);
        assert_eq!(local[0].display_name(), "Windows (C:)");
        assert_eq!(local[1].display_name(), "Data (D:)");
        assert_eq!(network.len(), 1);
        assert_eq!(network[0].display_name(), "NAS (Z:)");
    }

    #[::core::prelude::rust_2024::test]
    fn test_collect_recent_entries_filters_virtual_and_preserves_empty() {
        assert!(collect_recent_entries(&[]).is_empty());
        assert!(collect_recent_entries(&[
            "".to_string(),
            "Home".to_string(),
            "Settings".to_string(),
            "shell:RecycleBinFolder".to_string(),
        ])
        .is_empty());

        let work_dir = std::env::temp_dir().join("files_rs_home_work");
        let mut many: Vec<String> = vec!["Home".to_string(), "Settings".to_string()];
        for i in 0..15 {
            many.push(
                work_dir
                    .join(format!("file_{i}.txt"))
                    .to_string_lossy()
                    .to_string(),
            );
        }
        let entries = collect_recent_entries(&many);
        assert_eq!(entries.len(), 12);
        assert_eq!(entries[0].name, "file_0.txt");
        assert_eq!(entries[0].parent, work_dir.to_string_lossy());
        assert!(!entries[0].is_dir);
    }

    #[::core::prelude::rust_2024::test]
    fn test_collect_quick_access_cards_dedups_and_appends_custom_pins() {
        let std_only = collect_quick_access_cards(&[]);
        assert_eq!(std_only.len(), 6);
        assert!(std_only.iter().all(|c| !c.is_custom_pinned));

        let desktop_str = std_only[0].path.to_string_lossy().to_string();
        let custom_dir = std::env::temp_dir()
            .join("Projects")
            .join("files-rs")
            .to_string_lossy()
            .to_string();
        let with_custom = collect_quick_access_cards(&[
            "".to_string(),
            desktop_str,
            custom_dir,
        ]);
        assert_eq!(with_custom.len(), 7);
        assert_eq!(with_custom[6].label, "files-rs");
        assert!(with_custom[6].is_custom_pinned);
    }
}

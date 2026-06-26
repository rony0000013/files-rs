use crate::app_state::AppWindow;
use crate::config::config::{SortBy, ViewMode};
use crate::core::navigation_manager::PaneLayout;
use crate::storage::storage::FileType;
use crate::ui::components::app_background::AppBackground;
use crate::ui::components::clipboard_manager::ClipboardState;
use crate::ui::components::dialogs::{DialogType, DialogsView, SettingsTab};
use crate::ui::components::navigation_bar::{
    build_breadcrumb_dropdown, build_suggestions_dropdown, NavigationBarView,
};
#[cfg(windows)]
use crate::ui::components::sidebar::{SidebarSplitter, SidebarView};
use crate::ui::components::status_bar::StatusBarView;
use crate::ui::components::tab_bar::TabBarView;
use crate::ui::components::toolbar::ToolbarView;
use crate::ui::constants::UiConstants;
use crate::ui::views::main_view::{ColumnWidths, MainView, MainViewState};
use gpui::*;

impl Render for AppWindow {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Poll file system events — refresh if any folder changes detected
        self.poll_file_events(cx);
        // file_items is synced in the background task update callback, not during render
        // icon_paths are synced after Phase 1, Phase 2, and icon loading — no per-frame clone needed

        let (can_go_back, can_go_forward, pane_layout) = {
            let nav = self.state.navigation.read(cx);
            let can_go_back = nav.tabs.active_tab().can_go_back();
            let can_go_forward = nav.tabs.active_tab().can_go_forward();
            let pane_layout = nav.pane_layout;
            (can_go_back, can_go_forward, pane_layout)
        };

        self.state.main_view.pane_type = match pane_layout {
            PaneLayout::Single => crate::ui::views::main_view::PaneType::Single,
            _ => crate::ui::views::main_view::PaneType::Dual,
        };

        // Dismiss omnibar if window lost focus (e.g. user alt-tabbed or clicked outside)
        if !_window.is_window_active() {
            self.dismiss_omnibar(cx);
            // Also close breadcrumb dropdown on window deactivation
            if self.state.nav_bar.breadcrumb_dropdown_open {
                self.state.nav_bar.close_breadcrumb_dropdown();
            }
        }

        // Initialize main focus handle before borrowing state
        if self.state.main_focus.is_none() {
            self.state.main_focus = Some(cx.focus_handle());
            // Request focus on the main content so keyboard events work
            let focus = self.state.main_focus.clone().unwrap();
            _window.focus(&focus);
        }

        let state = &self.state;
        let _nav = state.navigation.read(cx);

        // Batch settings reads into a single entity borrow
        let (mica_enabled, mica_alt, font_family, font_size) = {
            let settings = self.state.settings.read(cx);
            let appearance = &settings.config().appearance;
            (
                appearance.mica_enabled,
                appearance.mica_alt,
                appearance.font_family.clone(),
                appearance.font_size,
            )
        };

        let main_content = AppBackground::build(
            &state.main_view.folder_background,
            mica_enabled,
            mica_alt,
            vec![
                // Main content area
                div()
                    .flex()
                    .flex_col()
                    .w_full()
                    .h_full()
                    .child({
                        let main_focus = self.state.main_focus.clone().unwrap();
                        div()
                            .id("main-content")
                            .track_focus(&main_focus)
                            .flex()
                            .flex_row()
                            .w_full()
                            .h_full()
                            .font_family(font_family.clone())
                            .text_size(px(font_size as f32))
                            .on_key_down({
                                let view = cx.entity().clone();
                                move |event: &gpui::KeyDownEvent,
                                      _window: &mut Window,
                                      cx: &mut App| {
                                    view.update(cx, |this, cx| {
                                        this.handle_key_event(event, cx);
                                    });
                                }
                            })
                            .border(px(0.0))
                            .cursor_default()
                            .on_mouse_down(MouseButton::Left, {
                                let view = cx.entity().clone();
                                let main_focus = self.state.main_focus.clone().unwrap();
                                move |_, window, cx| {
                                    // Refocus main content so keyboard events work
                                    window.focus(&main_focus);
                                    view.update(cx, |this, cx| {
                                        this.dismiss_omnibar(cx);
                                    });
                                }
                            })
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .flex_grow()
                                    .h_full()
                                    .w_full()
                                    .child({
                                        // TabBar (fixed height) — also dismisses omnibar on click
                                        let view = cx.entity();
                                        div().flex_shrink_0().w_full()
                                            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                                view.update(cx, |this, cx| {
                                                    this.dismiss_omnibar(cx);
                                                });
                                            })
                                            .child(TabBarView::build(
                                                &state.navigation.read(cx),
                                                cx.entity(),
                                                &state.tab_thumbnail_cache,
                                                &state.tab_drag,
                                                state.is_maximized,
                                            ))
                                    })
                                    .child(
                                        // Navigation Bar (fixed height)
                                        div().flex_shrink_0().w_full().child(
                                            NavigationBarView::build(
                                                &state.nav_bar,
                                                can_go_back,
                                                can_go_forward,
                                                state.main_view.side_pane.clone(),
                                                cx.entity(),
                                                &state.action_registry,
                                            ),
                                        ),
                                    )
                                    .child({
                                        let view = cx.entity();
                                        let mut row = div()
                                            .flex()
                                            .flex_row()
                                            .overflow_hidden()
                                            .flex_grow()
                                            .h_full()
                                            .w_full()
                                            // Track mouse moves ANYWHERE in this row during sidebar resize
                                            .on_mouse_move({
                                                let view = view.clone();
                                                move |event, win, cx| {
                                                    view.update(cx, |this, cx| {
                                                        if this
                                                            .state
                                                            .sidebar
                                                            .resize_start_x
                                                            .is_some()
                                                        {
                                                            let x: f32 =
                                                                win.mouse_position().x.into();
                                                            if this
                                                                .state
                                                                .sidebar
                                                                .update_resize_drag(x)
                                                            {
                                                                cx.notify();
                                                            }
                                                        }
                                                        if let (Some(start_x), Some(start_width)) = (
                                                            this.state.main_view.side_pane_resize_start_x,
                                                            this.state.main_view.side_pane_resize_start_width,
                                                        ) {
                                                            let x: f32 = win.mouse_position().x.into();
                                                            let delta = x - start_x;
                                                            let win_width: f32 = win.viewport_size().width.into();
                                                            let min_width = 150.0;
                                                            let max_width = win_width * 0.65;
                                                            let new_width = (start_width - delta).max(min_width).min(max_width);
                                                            this.state.main_view.side_pane_width = Some(new_width);
                                                            cx.notify();
                                                        }
                                                        // Handle column resize drag
                                                        if this.column_resize.is_some() {
                                                            let x: f32 =
                                                                win.mouse_position().x.into();
                                                            this.update_column_resize(x);
                                                            cx.notify();
                                                        }
                                                    });
                                                }
                                            })
                                            .on_mouse_up(MouseButton::Left, {
                                                let view = view.clone();
                                                move |_event, _win, cx| {
                                                    view.update(cx, |this, cx| {
                                                        if this
                                                            .state
                                                            .sidebar
                                                            .resize_start_x
                                                            .is_some()
                                                        {
                                                            this.state.sidebar.end_resize_drag();
                                                        }
                                                        if this.state.main_view.side_pane_resize_start_x.is_some() {
                                                            this.state.main_view.side_pane_resize_start_x = None;
                                                            this.state.main_view.side_pane_resize_start_width = None;
                                                            cx.notify();
                                                        }
                                                        if this.column_resize.is_some() {
                                                            this.end_column_resize();
                                                        }
                                                    });
                                                }
                                            })
                                            // Dismiss omnibar active state when clicking
                                            // anywhere in the content / sidebar area.
                                            .on_mouse_down(MouseButton::Left, {
                                                let view = view.clone();
                                                move |_event, _win, cx| {
                                                    view.update(cx, |this, cx| {
                                                        this.dismiss_omnibar(cx);
                                                    });
                                                }
                                            });
                                        if state.sidebar_visible {
                                            #[cfg(windows)]
                                            {
                                                row = row
                                                    .child(SidebarView::build(
                                                        &state.sidebar,
                                                        state,
                                                        cx.entity(),
                                                    ))
                                                    .child(SidebarSplitter::build(
                                                        &state.sidebar,
                                                        cx.entity(),
                                                    ));
                                            }
                                        }
                                        row.child(
                                            div()
                                                .flex()
                                                .flex_col()
                                                .flex_grow()
                                                .w_full()
                                                .h_full()
                                                .on_mouse_down(MouseButton::Left, {
                                                    let view = cx.entity();
                                                    move |_event, _window, cx| {
                                                        view.update(cx, |this, cx| {
                                                            if this.state.sidebar.reorder_mode {
                                                                this.state
                                                                    .sidebar
                                                                    .exit_reorder_mode();
                                                                cx.notify();
                                                            }
                                                        });
                                                    }
                                                })
                                                .child(
                                                    // Toolbar (fixed height)
                                                    div().flex_shrink_0().w_full().child(
                                                        ToolbarView::build(
                                                            &state.main_view.view_mode,
                                                            &state.main_view.toolbar_customization,
                                                            !state.main_view.selected_indices.is_empty(),
                                                            state.main_view.selected_indices.len() == 1,
                                                            !matches!(state.clipboard, ClipboardState::Empty),
                                                             state.main_view.filter_text.clone(),
                                                             cx.entity(),
                                                        ),
                                                    ),
                                                )
                                                .child(
                                                    // Main Content (flex_grow to fill remaining space)
                                                    div()
                                                        .flex()
                                                        .flex_col()
                                                        .flex_grow()
                                                        .w_full()
                                                        .h_full()
                                                        .child({
                                                            let settings_data = if matches!(state.main_view.target, crate::ui::views::types::ViewTarget::Settings) {
                                                                Some(crate::ui::components::dialogs::DialogsView::pre_read_settings_data(&self.state, cx))
                                                            } else {
                                                                None
                                                            };
                                                            MainView::build(
                                                                &state.main_view,
                                                                state.file_items.clone(),
                                                                cx.entity(),
                                                                settings_data.as_ref(),
                                                            )
                                                        }),
                                                )
                                                .child(
                                                    // Status bar (fixed height at bottom)
                                                    div()
                                                        .flex_shrink_0()
                                                        .w_full()
                                                        .h(px(UiConstants::STATUS_BAR_HEIGHT_RAW
                                                            as f32))
                                                        .child(StatusBarView::build(
                                                            &state.status_bar,
                                                            state,
                                                            cx.entity(),
                                                        )),
                                                ),
                                        )
                                    }),
                            )
                    })
                    .into_any_element(),
            ],
        );

        if state.sidebar_visible && state.sidebar.anim_start_time.is_some() {
            if state.sidebar.anim_start_time.unwrap().elapsed()
                < std::time::Duration::from_millis(200)
            {
                _window.request_animation_frame();
            }
        }

        let mut base = {
            let _view = cx.entity();
            div().relative().w_full().h_full()
        };
        if let Some(dialog) = &state.active_dialog {
            // Pre-read settings dialog data to avoid BorrowMutError during render tree construction
            let settings_data = None; // Settings is now a tab, not a dialog
            base =
                base.child(main_content)
                    .child(crate::ui::components::dialogs::DialogsView::build(
                        dialog,
                        cx.entity(),
                        cx,
                        settings_data,
                    ));
        } else {
            base = base.child(main_content);
        }

        // Render sidebar collapsed-mode tooltip at app level (outside overflow-hidden areas)
        if state.sidebar.collapsed {
            if let Some(sid) = state.sidebar.tooltip_section {
                // Find the section index in the current display order to compute Y offset
                let section_idx = state.sidebar.sections.iter().position(|s| s.id == sid);
                if let (Some(idx), Some(section)) = (
                    section_idx,
                    state.sidebar.sections.iter().find(|s| s.id == sid),
                ) {
                    // Y offset: 40px home toggle + 12px scroll padding + idx * 36px per collapsed icon
                    let tooltip_y = 52.0 + idx as f32 * 36.0;
                    base = base.child({
                        let sidebar_w = px(UiConstants::SIDEBAR_WIDTH_COLLAPSED_RAW as f32);
                        let tooltip_left = sidebar_w + px(4.0);
                        crate::ui::components::sidebar::SidebarView::render_tooltip(
                            section,
                            tooltip_y,
                            tooltip_left,
                            cx.entity(),
                        )
                    });
                }
            }
        }

        // Render omnibar suggestions dropdown at app root level (outside nav bar's
        // fixed-height container) so it can extend below the nav bar.
        if state.nav_bar.show_suggestions && !state.nav_bar.suggestions.is_empty() {
            // Position dropdown below the nav bar:
            // TabBar=40px + NavBar=44px = 84px from top
            // NavBar padding=12px + 4 buttons(32px each) + 3 gaps(2px) + pill_gap(8px) = 154px from left
            let nav_btn_area = 4.0 * 32.0 + 3.0 * 2.0; // 134px
            let pill_left = 12.0 + nav_btn_area + 8.0; // 154px
            let dropdown_top = 40.0 + 44.0; // 84px
            let vp = _window.viewport_size();
            let dropdown_width = vp.width - px(pill_left) - px(12.0); // minus right padding

            let dropdown =
                build_suggestions_dropdown(&state.nav_bar, &state.action_registry, cx.entity());
            // The wrapper dismisses the omnibar when clicked. Individual suggestion
            // rows inside the dropdown call stop_propagation(), so their clicks are
            // handled by the row handlers (navigation / command execution) instead.
            let view = cx.entity();
            base = base.child(
                div()
                    .absolute()
                    .left(px(pill_left))
                    .top(px(dropdown_top))
                    .w(dropdown_width)
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        view.update(cx, |this, cx| {
                            this.dismiss_omnibar(cx);
                        });
                    })
                    .child(dropdown),
            );
        }

        // Render breadcrumb sibling dropdown at app root level (like tooltip pattern)
        if state.nav_bar.breadcrumb_dropdown_open {
            if let Some((_dx, _dy)) = state.nav_bar.breadcrumb_dropdown_position {
                let breadcrumb_focus = cx.focus_handle();
                if let Some(bd_dropdown) =
                    build_breadcrumb_dropdown(&state.nav_bar, cx.entity(), breadcrumb_focus.clone())
                {
                    // Request focus so keyboard events go to the dropdown
                    _window.focus(&breadcrumb_focus);

                    let view = cx.entity();
                    let view2 = cx.entity();
                    base = base.child(
                        bd_dropdown
                            .track_focus(&breadcrumb_focus)
                            .on_mouse_down_out(move |_, _, cx| {
                                view2.update(cx, |this, cx| {
                                    this.state.nav_bar.close_breadcrumb_dropdown();
                                    cx.notify();
                                });
                            })
                            .on_scroll_wheel(
                                move |event: &gpui::ScrollWheelEvent,
                                      _window: &mut Window,
                                      cx: &mut App| {
                                    // Stop scroll propagation so main view doesn't scroll
                                    cx.stop_propagation();
                                },
                            )
                            .on_key_down(
                                move |event: &gpui::KeyDownEvent,
                                      _window: &mut Window,
                                      cx: &mut App| {
                                    let keystroke = &event.keystroke;
                                    view.update(cx, |this, cx| {
                                        if !this.state.nav_bar.breadcrumb_dropdown_open {
                                            return;
                                        }
                                        let key = keystroke.key.as_str();
                                        match key {
                                            "escape" | "Escape" => {
                                                this.state.nav_bar.close_breadcrumb_dropdown();
                                                cx.notify();
                                            }
                                            "enter" | "return" | "Enter" | "Return" => {
                                                if let Some(idx) =
                                                    this.state.nav_bar.breadcrumb_dropdown_selected
                                                {
                                                    let filtered = this
                                                        .state
                                                        .nav_bar
                                                        .breadcrumb_dropdown_filtered_siblings();
                                                    if let Some((_, path)) = filtered.get(idx) {
                                                        let path = (*path).clone();
                                                        this.state
                                                            .nav_bar
                                                            .close_breadcrumb_dropdown();
                                                        this.navigate(path, cx);
                                                    }
                                                }
                                            }
                                            "up" | "Up" => {
                                                let count = this
                                                    .state
                                                    .nav_bar
                                                    .breadcrumb_dropdown_filtered_siblings()
                                                    .len();
                                                if count > 0 {
                                                    let current = this
                                                        .state
                                                        .nav_bar
                                                        .breadcrumb_dropdown_selected
                                                        .unwrap_or(0);
                                                    this.state
                                                        .nav_bar
                                                        .breadcrumb_dropdown_selected =
                                                        Some(if current > 0 {
                                                            current - 1
                                                        } else {
                                                            count - 1
                                                        });
                                                    cx.notify();
                                                }
                                            }
                                            "down" | "Down" => {
                                                let count = this
                                                    .state
                                                    .nav_bar
                                                    .breadcrumb_dropdown_filtered_siblings()
                                                    .len();
                                                if count > 0 {
                                                    let current = this
                                                        .state
                                                        .nav_bar
                                                        .breadcrumb_dropdown_selected
                                                        .unwrap_or(0);
                                                    this.state
                                                        .nav_bar
                                                        .breadcrumb_dropdown_selected =
                                                        Some(if current + 1 < count {
                                                            current + 1
                                                        } else {
                                                            0
                                                        });
                                                    cx.notify();
                                                }
                                            }
                                            "backspace" | "Backspace" => {
                                                this.state.nav_bar.breadcrumb_dropdown_filter.pop();
                                                this.state.nav_bar.breadcrumb_dropdown_selected =
                                                    Some(0);
                                                cx.notify();
                                            }
                                            _ => {
                                                if !keystroke.modifiers.control
                                                    && !keystroke.modifiers.alt
                                                    && !keystroke.modifiers.platform
                                                {
                                                    if keystroke.key.chars().count() == 1 {
                                                        this.state
                                                            .nav_bar
                                                            .breadcrumb_dropdown_filter
                                                            .push_str(&keystroke.key);
                                                        this.state
                                                            .nav_bar
                                                            .breadcrumb_dropdown_selected = Some(0);
                                                        cx.notify();
                                                    }
                                                }
                                            }
                                        }
                                    });
                                },
                            ),
                    );
                }
            }
        }

        // Render file preview tooltip on hover (at app level, outside overflow containers)
        if let Some(tip) = &self.hover_tooltip {
            use crate::ui::views::helpers::format_size;
            let item = &tip.item;
            let pos = tip.position;

            // ── Date formatting ──
            let date_str = item
                .modified
                .as_ref()
                .map(|t| {
                    use std::time::{SystemTime, UNIX_EPOCH};
                    let since_epoch = t.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
                    let secs_per_day: u64 = 86400;
                    let days = since_epoch / secs_per_day;
                    let remaining = since_epoch % secs_per_day;
                    let hours = remaining / 3600;
                    let minutes = (remaining % 3600) / 60;
                    let mut y = 2000i64;
                    let mut d = days as i64 - (11017i64);
                    loop {
                        let days_in_year = if y % 4 == 0 && (y % 100 != 0 || y % 400 == 0) {
                            366
                        } else {
                            365
                        };
                        if d < days_in_year {
                            break;
                        }
                        d -= days_in_year;
                        y += 1;
                    }
                    let leap = y % 4 == 0 && (y % 100 != 0 || y % 400 == 0);
                    let month_days = [
                        31u64,
                        if leap { 29 } else { 28 },
                        31,
                        30,
                        31,
                        30,
                        31,
                        31,
                        30,
                        31,
                        30,
                        31,
                    ];
                    let mut m = 0usize;
                    while d as u64 >= month_days[m] {
                        d -= month_days[m] as i64;
                        m += 1;
                    }
                    let day = d + 1;
                    format!(
                        "{:04}-{:02}-{:02} {:02}:{:02}",
                        y,
                        m + 1,
                        day,
                        hours,
                        minutes
                    )
                })
                .unwrap_or_default();
            let size_bytes = item.size;
            let size_hr = format_size(size_bytes);

            // ── File type string ──
            let file_type_str = if matches!(item.file_type, FileType::Directory) {
                "Folder".to_string()
            } else {
                let ext = item
                    .name
                    .rsplit('.')
                    .next()
                    .map(|e| e.to_uppercase())
                    .unwrap_or_default();
                if ext.is_empty() {
                    "File".to_string()
                } else {
                    format!("{} File", ext)
                }
            };

            // ── Determine if this is an image with a cached thumbnail ──
            let extension = item
                .name
                .rsplit('.')
                .next()
                .map(|e| e.to_lowercase())
                .unwrap_or_default();
            let image_exts = [
                "jpg", "jpeg", "png", "gif", "bmp", "webp", "ico", "tiff", "tif", "heic", "heif",
            ];
            let is_image = image_exts.contains(&extension.as_str());
            let thumbnail_path = if is_image {
                self.state.main_view.icon_paths.get(&item.path).cloned()
            } else {
                None
            };

            // ── Tooltip sizing ──
            let tooltip_w = 280.0;
            let want_thumb_h = if thumbnail_path.is_some() { 110.0 } else { 0.0 };
            // Approximate text block height: 13px title + 3×11px lines + margins ≈ 70px
            let text_h = 70.0;
            let tooltip_h = 20.0 + want_thumb_h + text_h; // padding + thumb + info

            // ── Viewport-aware positioning ──
            let win_bounds = _window.bounds();
            let win_w = win_bounds.size.width;
            let win_h = win_bounds.size.height;

            // Preferred: tooltip to the RIGHT and ABOVE the cursor
            let mut tx = pos.x + px(16.0);
            let mut ty = pos.y - px(tooltip_h) - px(8.0);

            // If tooltip would go off right edge, flip to left of cursor
            if tx + px(tooltip_w) + px(12.0) > win_w {
                tx = pos.x - px(tooltip_w) - px(16.0);
            }
            // Clamp left edge
            if tx < px(8.0) {
                tx = px(8.0);
            }
            // If tooltip would go off top edge, show below cursor
            if ty < px(4.0) {
                ty = pos.y + px(16.0);
            }
            // Clamp bottom edge
            if ty + px(tooltip_h) + px(4.0) > win_h {
                ty = (win_h - px(tooltip_h) - px(4.0)).max(px(4.0));
            }

            // ── Build tooltip element ──
            let mut info_div = div().flex().flex_col();

            // Add thumbnail image preview if available
            if let Some(thumb_path) = &thumbnail_path {
                info_div = info_div.child(
                    div()
                        .w_full()
                        .h(px(want_thumb_h))
                        .flex()
                        .items_center()
                        .justify_center()
                        .border_b(px(1.0))
                        .border_color(rgb(0x3A3A3A))
                        .mb(px(6.0))
                        .child(
                            img(thumb_path.clone())
                                .w(px(130.0))
                                .h(px(want_thumb_h - 8.0))
                                .object_fit(ObjectFit::Contain)
                                .rounded(px(4.0)),
                        ),
                );
            }

            // File info text
            info_div = info_div
                .child(
                    div()
                        .text_size(px(13.0))
                        .font_family(UiConstants::FONT_MAIN)
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(0xFFFFFF))
                        .child(item.name.clone()),
                )
                .child(
                    div()
                        .mt(px(4.0))
                        .text_size(px(11.0))
                        .font_family(UiConstants::FONT_MAIN)
                        .text_color(rgb(0xAAAAAA))
                        .child(format!("Type: {}", file_type_str)),
                )
                .child(
                    div()
                        .text_size(px(11.0))
                        .font_family(UiConstants::FONT_MAIN)
                        .text_color(rgb(0xAAAAAA))
                        .child(format!("Size: {} ({} bytes)", size_hr, size_bytes)),
                )
                .child(
                    div()
                        .text_size(px(11.0))
                        .font_family(UiConstants::FONT_MAIN)
                        .text_color(rgb(0xAAAAAA))
                        .child(format!("Date: {}", date_str)),
                );

            // Tooltip container with fade-in animation
            let tooltip = div()
                .absolute()
                .left(tx)
                .top(ty)
                .w(px(tooltip_w))
                .bg(rgb(0x2B2B2B))
                .border(px(1.0))
                .border_color(rgb(0x444444))
                .rounded(px(8.0))
                .py(px(10.0))
                .px(px(12.0))
                .overflow_hidden()
                .child(info_div);

            base = base.child(tooltip.with_animation(
                "tooltip-fade",
                Animation::new(std::time::Duration::from_millis(120)),
                |elem, delta| elem.opacity(delta),
            ));
        }
        if state.show_shortcut_overlay {
            base = base.child(self.build_shortcut_overlay(cx));
        }

        if state.splash_state.visible {
            base = base.child(crate::ui::components::splash_screen::SplashScreen::build(
                &state.splash_state,
                cx.entity(),
            ));
        }

        // Opportunistic icon refresh check: if the viewport shows items
        // without cached icons (e.g. after scrolling), schedule a load.
        // This runs outside the render element tree and is idempotent.
        if !self.state.main_view.items.is_empty()
            && self.state.main_view.items.len() > self.state.main_view.icon_paths.len()
        {
            // Only check every ~500ms max to avoid hammering the executor
            let now = std::time::Instant::now();
            let last_check = self.state.main_view.icon_refresh_last_check;
            let elapsed = last_check
                .map(|t| now - t)
                .unwrap_or(std::time::Duration::MAX);
            if elapsed > std::time::Duration::from_millis(500) {
                self.state.main_view.icon_refresh_last_check = Some(now);
                let mut trigger = cx.to_async();
                let weak = cx.weak_entity();
                // Defer to next tick to avoid side-effects inside render
                cx.foreground_executor()
                    .spawn(async move {
                        weak.update(&mut trigger, |this, cx| {
                            this.refresh_icons_for_viewport(cx);
                        })
                        .ok();
                    })
                    .detach();
            }
        }

        base
    }
}

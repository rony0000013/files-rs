use crate::app_state;
use crate::config::config::{SortBy, ViewMode};
use crate::config::settings_model::SettingsModel;
use crate::core::navigation_manager::{NavigationState, PaneLayout};
use crate::core::navigation_service::NavigationService;
use crate::core::path_util::FilesPath;
use crate::domain::extended_metadata::ExtendedMetadataExtractor;
use crate::domain::git_service::GitService;
use crate::domain::metadata::FileItem;
use crate::storage::storage::{FileType, LocalFileSystem, StorageService};
use crate::ui::components::app_background::AppBackground;
use crate::ui::components::dialogs::{DialogType, DialogsView, SettingsTab};
use crate::ui::components::navigation_bar::{
    build_breadcrumb_dropdown, build_suggestions_dropdown, NavigationBarState, NavigationBarView,
};
#[cfg(windows)]
use crate::ui::components::sidebar::{SidebarSplitter, SidebarView};
use crate::ui::components::sidebar_state::SidebarItemId;
use crate::ui::components::sidebar_state::SidebarState;
use crate::ui::components::splash_screen::SplashState;
use crate::ui::components::status_bar::{StatusBarState, StatusBarView};
use crate::ui::components::tab_bar::TabBarView;
use crate::ui::components::toolbar::ToolbarView;
use crate::ui::constants::UiConstants;
use crate::ui::views::main_view::{
    ColumnResizeState, ColumnWidths, DetailsPaneCache, FileItems, GroupDragState, MainView,
    MainViewState,
};
pub use app_state::{
    icons_cache_dir, AppState, AppWindow, HoverTooltipData, SidebarDragState,
    SidebarPinnedDragState, TagState,
};
use gpui::*;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

/// Round-robin cursor for background thumbnail trickle-fill (see load_icons).
/// Guarantees every item eventually gets queued even if viewport tracking
/// ever reports a stale range — the viewport slice always goes first.
static TRICKLE_CURSOR: AtomicUsize = AtomicUsize::new(0);
use std::sync::{Arc, Mutex};

use crate::platform::windows::recycle_bin_service::RecycleBinService;
use crate::platform::windows::thumbnail_service::{ThumbnailService, ThumbnailSize};
use crate::ui::components::clipboard_manager::{ClipboardItem, ClipboardOperation, ClipboardState};

impl AppWindow {
    pub fn new(
        state: AppState,
        mut rx: tokio::sync::mpsc::UnboundedReceiver<crate::core::progress::JobUpdate>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut window_view = Self {
            state,
            hover_item_index: None,
            hover_tooltip: None,
            menu_tip: None,
            hover_timer_task: None,
            menu_task: None,
            submenu_grace: None,
            submenu_grace_gen: 0,
            sidebar_pinned_drag: None,
            zoom_drag_start: None,
            column_resize: None,
            fps_last_time: None,
            fps_frame_count: 0,
            fps_current: 0.0,
            fps_prev_frame: None,
            fps_worst_ms: 0.0,
            last_render_time: None,
            file_watcher: None,
            file_event_rx: None,
            file_watcher_secondary: None,
            file_event_rx_secondary: None,
            focus_out_subscription: None,
        };
        let initial_path = window_view.state.main_view.primary_path.clone();

        // Bounded GPU image cache for file-view thumbnails (replaces GPUI's
        // never-evicting global cache for our tiles; see bounded_image_cache).
        window_view.state.image_cache = Some(cx.new(|_| {
            crate::ui::components::bounded_image_cache::BoundedImageCache::new(
                crate::ui::components::bounded_image_cache::MAX_GPU_IMAGE_ENTRIES,
            )
        }));

        // Restore tabs from saved session if available (BEFORE initial navigate,
        // so the saved session data is still intact in memory)
        let restored = {
            let config = window_view.state.settings.read(cx);
            let session = &config.config.session;
            if !session.open_tabs.is_empty() {
                let new_tabs: Vec<crate::core::navigation_manager::TabState> = session
                    .open_tabs
                    .iter()
                    .map(|t| {
                        let path = crate::core::path_util::FilesPath::new(&t.path);
                        crate::core::navigation_manager::TabState::new_with_zoom(
                            path,
                            t.view_mode.clone(),
                            t.zoom_level,
                        )
                    })
                    .collect();

                let active_index = session.active_tab_index.min(new_tabs.len() - 1);

                // Restore pane layout and active pane from saved strings
                let pane_layout = match session.pane_layout.as_str() {
                    "horizontal" => crate::core::navigation_manager::PaneLayout::Horizontal,
                    "vertical" => crate::core::navigation_manager::PaneLayout::Vertical,
                    _ => crate::core::navigation_manager::PaneLayout::Single,
                };
                let active_pane = match session.active_pane.as_str() {
                    "secondary" => crate::core::navigation_manager::PaneFocus::Secondary,
                    _ => crate::core::navigation_manager::PaneFocus::Primary,
                };

                window_view.state.navigation.update(cx, |nav, _cx| {
                    nav.tabs.tabs = new_tabs;
                    nav.tabs.active_tab_index = active_index;
                    nav.tabs.closed_tabs.clear();
                    nav.pane_layout = pane_layout;
                    nav.active_pane = active_pane;
                });

                // Sync main_view's pane_type to match restored pane layout
                window_view.state.main_view.pane_type = match pane_layout {
                    crate::core::navigation_manager::PaneLayout::Single => {
                        crate::ui::views::main_view::PaneType::Single
                    }
                    _ => crate::ui::views::main_view::PaneType::Dual,
                };

                true // restored
            } else {
                false
            }
        };

        if restored {
            // Navigate to the active restored tab's path
            let active_path = window_view
                .state
                .navigation
                .read(cx)
                .tabs
                .active_tab()
                .path
                .clone();
            window_view.state.main_view.primary_path = active_path.clone();
            window_view.navigate(active_path, cx);

            // After restoring, also set the zoom level from the saved tab
            let restored_zoom = window_view
                .state
                .navigation
                .read(cx)
                .tabs
                .active_tab()
                .zoom_level;
            window_view.state.main_view.zoom_level = restored_zoom;

            // Ensure the session service path matches restored active tab
            let session_path = window_view.state.navigation.read(cx).active_pane_path();
            window_view
                .state
                .session_service
                .set_last_path(session_path.to_string_lossy().as_ref());
            window_view.load_tab_thumbnails(cx);
        } else {
            // No saved session — navigate to the default home path
            window_view.navigate(initial_path, cx);
        }

        // Crash detection: silently restore without showing dialog
        {
            let session = &window_view.state.session_service;
            if session.detect_crash() {
                tracing::warn!("Previous session crashed. Silently restoring...");
                // Reset crash count and start fresh
                window_view.state.session_service.start_session();
            }
        }

        // Start a new session (marks clean_shutdown=false until end_session)
        window_view.state.session_service.start_session();

        // Splash auto-dismiss timer removed (splash disabled)
        // Dismiss omnibar when window loses focus (e.g. user clicks outside the app).
        // NOTE: the Subscription must be stored on the view — dropping it
        // unsubscribes immediately and the handler would never fire.
        {
            let focus = cx.focus_handle();
            let sub = cx.on_focus_out(&focus, _window, move |this, _event, _window, cx| {
                // Pointer capture is lost with focus: a drag (scrollbar,
                // resize, tab, sidebar) would otherwise stick forever since
                // its mouse-up never arrives. Cancel first, then dismiss.
                this.cancel_pointer_tracking(cx);
                this.dismiss_omnibar(cx);
            });
            window_view.focus_out_subscription = Some(sub);
        }

        // Progress listener: forwards engine JobUpdates to the status center.
        // Drain-per-wake (one re-render per burst, not per update) and no
        // entity is held across an await, avoiding the lease conflicts that
        // got the previous version commented out.
        {
            let this_weak = cx.weak_entity();
            let mut async_cx = cx.to_async();
            cx.foreground_executor()
                .spawn(async move {
                    while let Some(first) = rx.recv().await {
                        let mut pending = vec![first];
                        while let Ok(u) = rx.try_recv() {
                            pending.push(u);
                        }
                        this_weak
                            .update(&mut async_cx, |this, cx| {
                                for update in pending {
                                    this.handle_job_update(update, cx);
                                }
                            })
                            .ok();
                    }
                })
                .detach();
        }

        window_view
    }

    /// Cancel all active pointer-tracking states (drag scroll, column
    /// resize, zoom drag, tab drag, sidebar resize/reorder drags).
    /// Called on window focus loss: without this, a drag in progress sticks
    /// forever because its mouse-up is delivered to another app (or never).
    pub fn cancel_pointer_tracking(&mut self, cx: &mut Context<Self>) {
        self.state.main_view.bouncy_scroll.release();
        self.column_resize = None;
        self.zoom_drag_start = None;
        self.sidebar_pinned_drag = None;
        self.state.tab_drag = crate::app_state::TabDragState::default();
        self.state.sidebar.resize_start_x = None;
        self.state.sidebar.resize_start_width = None;
        self.state.sidebar.drag_section_idx = None;
        self.state.sidebar.drag_item = None;
        // Tab strip drag: preserve scroll position, drop the drag.
        let saved_tab_scroll = self.state.tab_drag.scroll_offset;
        self.state.tab_drag = crate::app_state::TabDragState::default();
        self.state.tab_drag.scroll_offset = saved_tab_scroll;
        cx.notify();
    }

    fn handle_job_update(
        &mut self,
        update: crate::core::progress::JobUpdate,
        cx: &mut Context<Self>,
    ) {
        use crate::core::progress::JobUpdate;
        match update {
            JobUpdate::UpdateStatus(id, status) => {
                self.state.status_center.update_job(id, status);
            }
            JobUpdate::Complete(id) => {
                self.state.status_center.complete_job(id);
            }
            JobUpdate::Fail(id, reason) => {
                self.state.status_center.fail_job(id, reason);
            }
            JobUpdate::NewJob(job) => {
                self.state.status_center.add_job(job);
            }
        }
        // Keep an open StatusCenter dialog in sync (render reads the snapshot,
        // never the live view, to avoid BorrowMutError during render).
        if let Some(DialogType::StatusCenter {
            active_operations, ..
        }) = self.state.active_dialog.as_mut()
        {
            *active_operations = self.state.status_center.jobs.clone();
        }
        cx.notify();
    }

    /// Handle a keyboard event by dispatching to the appropriate handler.
    pub fn handle_key_event(
        &mut self,
        event: &gpui::KeyDownEvent,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        let keystroke = &event.keystroke;
        let key_combo =
            crate::core::action_registry::from_gpui_keystroke(keystroke);

        use crate::ui::components::dialogs::DialogType;

        // Menu key (VK_APPS) / Shift+F10: open the context menu for the
        // current selection, like Files does (Files-community parity).
        // Anchored at viewport center; mouse users get item-anchored menus.
        let key_name = keystroke.key.as_str();
        if key_name == "menu"
            || (key_name.eq_ignore_ascii_case("f10") && keystroke.modifiers.shift)
        {
            let vp = window.viewport_size();
            let pos = gpui::Point {
                x: (vp.width - gpui::px(250.0)) / 2.0,
                y: vp.height * 0.4,
            };
            let first_selected = self.state.main_view.selected_indices.iter().next().copied();
            self.show_context_menu(first_selected, pos, window, cx);
            return;
        }

        // Ctrl+Tab / Ctrl+Shift+Tab: cycle tabs (must be before nav bar intercept
        // so it works even when the omnibar is focused)
        if keystroke.modifiers.control && !keystroke.modifiers.alt && !keystroke.modifiers.platform {
            let key = keystroke.key.as_str();
            match key {
                "tab" | "Tab" => {
                    if keystroke.modifiers.shift {
                        self.cycle_tab_prev(cx);
                    } else {
                        self.cycle_tab_next(cx);
                    }
                    return;
                }
                _ => {}
            }
        }

        // Ctrl+1-9: switch to tab by position (browser-style)
        if keystroke.modifiers.control && !keystroke.modifiers.alt && !keystroke.modifiers.platform && !keystroke.modifiers.shift {
            let key = keystroke.key.as_str();
            if let Some(digit) = key.chars().next().and_then(|c| c.to_digit(10)) {
                let tab_count = self.state.navigation.read(cx).tabs.tabs.len();
                if digit >= 1 {
                    let idx = (digit - 1) as usize;
                    if idx < tab_count {
                        self.switch_tab(idx, cx);
                        return;
                    }
                }
            }
            // Ctrl+9 always switches to the last tab
            if key == "9" {
                let tab_count = self.state.navigation.read(cx).tabs.tabs.len();
                if tab_count > 0 {
                    self.switch_tab(tab_count - 1, cx);
                    return;
                }
            }
        }

        // Intercept Navigation Bar input typing
        let is_nav_bar_active = self.state.nav_bar.edit_mode
            || matches!(self.state.nav_bar.omnibar_mode, crate::ui::components::navigation_bar::OmnibarMode::Search)
            || matches!(self.state.nav_bar.omnibar_mode, crate::ui::components::navigation_bar::OmnibarMode::CommandPalette);

        if is_nav_bar_active {
            use crate::ui::components::navigation_bar::OmnibarMode;
            let key = keystroke.key.as_str();
            match key {
                "escape" | "Escape" => {
                    self.dismiss_omnibar(cx);
                }
                "enter" | "return" | "Enter" | "Return" => {
                    self.handle_suggestion_confirm(cx);
                    cx.notify();
                }
                "up" | "Up" => {
                    if self.state.nav_bar.show_suggestions && !self.state.nav_bar.suggestions.is_empty() {
                        let current = self.state.nav_bar.selected_suggestion_index.unwrap_or(0);
                        if current > 0 {
                            self.state.nav_bar.selected_suggestion_index = Some(current - 1);
                        } else {
                            self.state.nav_bar.selected_suggestion_index = Some(self.state.nav_bar.suggestions.len() - 1);
                        }
                        cx.notify();
                    }
                }
                "down" | "Down" => {
                    if self.state.nav_bar.show_suggestions && !self.state.nav_bar.suggestions.is_empty() {
                        let current = self.state.nav_bar.selected_suggestion_index.unwrap_or(0);
                        if current + 1 < self.state.nav_bar.suggestions.len() {
                            self.state.nav_bar.selected_suggestion_index = Some(current + 1);
                        } else {
                            self.state.nav_bar.selected_suggestion_index = Some(0);
                        }
                        cx.notify();
                    }
                }
                "tab" | "Tab" => {
                    if let Some(idx) = self.state.nav_bar.selected_suggestion_index {
                        if let Some(suggestion) = self.state.nav_bar.suggestions.get(idx).cloned() {
                            match suggestion.kind {
                                crate::ui::components::navigation_bar::SuggestionKind::Path { path } => {
                                    self.state.nav_bar.edit_text = path.to_string_lossy().to_string();
                                    let registry = self.state.action_registry.clone();
                                    self.state.nav_bar.update_suggestions(&registry);
                                }
                                crate::ui::components::navigation_bar::SuggestionKind::SearchHistory { query } => {
                                    self.state.nav_bar.search_query = query;
                                    let registry = self.state.action_registry.clone();
                                    self.state.nav_bar.update_suggestions(&registry);
                                }
                                _ => {}
                            }
                            cx.notify();
                        }
                    }
                }
                "backspace" | "Backspace" => {
                    match self.state.nav_bar.omnibar_mode {
                        OmnibarMode::Path => {
                            self.state.nav_bar.edit_text.pop();
                        }
                        OmnibarMode::Search => {
                            self.state.nav_bar.search_query.pop();
                            let query = self.state.nav_bar.search_query.clone();
                            self.update_search(query, cx);
                        }
                        OmnibarMode::CommandPalette => {
                            self.state.nav_bar.command_query.pop();
                        }
                    }
                    let registry = self.state.action_registry.clone();
                    self.state.nav_bar.update_suggestions(&registry);
                    cx.notify();
                }
                "space" | "Space" => {
                    match self.state.nav_bar.omnibar_mode {
                        OmnibarMode::Path => {
                            self.state.nav_bar.edit_text.push(' ');
                        }
                        OmnibarMode::Search => {
                            self.state.nav_bar.search_query.push(' ');
                            let query = self.state.nav_bar.search_query.clone();
                            self.update_search(query, cx);
                        }
                        OmnibarMode::CommandPalette => {
                            self.state.nav_bar.command_query.push(' ');
                        }
                    }
                    let registry = self.state.action_registry.clone();
                    self.state.nav_bar.update_suggestions(&registry);
                    cx.notify();
                }
                _ => {
                    if !keystroke.modifiers.control && !keystroke.modifiers.alt && !keystroke.modifiers.platform {
                        if keystroke.key.chars().count() == 1 {
                            match self.state.nav_bar.omnibar_mode {
                                OmnibarMode::Path => {
                                    self.state.nav_bar.edit_text.push_str(&keystroke.key);
                                }
                                OmnibarMode::Search => {
                                    self.state.nav_bar.search_query.push_str(&keystroke.key);
                                    let query = self.state.nav_bar.search_query.clone();
                                    self.update_search(query, cx);
                                }
                                OmnibarMode::CommandPalette => {
                                    self.state.nav_bar.command_query.push_str(&keystroke.key);
                                }
                            }
                            self.state.nav_bar.show_suggestions = true;
                            let registry = self.state.action_registry.clone();
                            self.state.nav_bar.update_suggestions(&registry);
                            cx.notify();
                        }
                    }
                }
            }
            return;
        }

        // Intercept AddItem dialog input typing
        if let Some(DialogType::AddItem { ref mut name, .. }) = self.state.active_dialog {
            let key = keystroke.key.as_str();
            match key {
                "escape" | "Escape" => {
                    self.close_dialog(cx);
                }
                "enter" | "return" | "Enter" | "Return" => {
                    self.confirm_add_item(cx);
                }
                "backspace" | "Backspace" => {
                    name.pop();
                    cx.notify();
                }
                "space" | "Space" => {
                    name.push(' ');
                    cx.notify();
                }
                _ => {
                    if !keystroke.modifiers.control && !keystroke.modifiers.alt && !keystroke.modifiers.platform {
                        if keystroke.key.chars().count() == 1 {
                            name.push_str(&keystroke.key);
                            cx.notify();
                        }
                    }
                }
            }
            return;
        }

        // Intercept inline renaming input typing
        if self.state.main_view.renaming_index.is_some() {
            let key = keystroke.key.as_str();
            match key {
                "escape" | "Escape" => {
                    self.cancel_rename(cx);
                }
                "enter" | "return" | "Enter" | "Return" => {
                    self.commit_rename(cx);
                }
                "backspace" | "Backspace" => {
                    self.state.main_view.rename_text.pop();
                    cx.notify();
                }
                "space" | "Space" => {
                    self.state.main_view.rename_text.push(' ');
                    cx.notify();
                }
                _ => {
                    if !keystroke.modifiers.control && !keystroke.modifiers.alt && !keystroke.modifiers.platform {
                        if keystroke.key.chars().count() == 1 {
                            self.state.main_view.rename_text.push_str(&keystroke.key);
                            cx.notify();
                        }
                    }
                }
            }
            return;
        }

        // Files shows Ctrl+R for Refresh (F5 also works) and Ctrl+` for
        // opening a terminal here; both bypass the registry's 1:1 bindings.
        if keystroke.modifiers.control
            && !keystroke.modifiers.alt
            && !keystroke.modifiers.platform
            && !keystroke.modifiers.shift
            && !is_nav_bar_active
        {
            match keystroke.key.as_str() {
                "r" | "R" => {
                    self.refresh(cx);
                    return;
                }
                "`" => {
                    self.open_terminal_at_current(cx);
                    return;
                }
                _ => {}
            }
        }

        if let Some(action) =
            self.state.action_registry.lookup(&key_combo)
        {
            self.dispatch_action(action, cx);
        } else {
            if !keystroke.modifiers.modified() {
                match keystroke.key.as_str() {
                    "escape" | "Escape" => {
                        if self.state.splash_state.visible {
                            self.dismiss_splash(cx);
                        } else if self
                            .state
                            .main_view
                            .search_state
                            .is_searching
                        {
                            self.clear_search(cx);
                        } else if !self
                            .state
                            .main_view
                            .filter_text
                            .is_empty()
                        {
                            self.set_filter_text(String::new(), cx);
                        } else if !self
                            .state
                            .main_view
                            .selected_indices
                            .is_empty()
                        {
                            self.clear_selection(cx);
                        }
                    }
                    "ArrowDown" => {
                        let len = self.state.main_view.items.len();
                        let current = self
                            .state
                            .main_view
                            .last_clicked_index
                            .unwrap_or(0)
                            .min(len.saturating_sub(1));
                        let next = (current + 1)
                            .min(len.saturating_sub(1));
                        self.select_item(
                            next,
                            event.keystroke.modifiers.shift,
                            false,
                            cx,
                        );
                        self.state.main_view.last_clicked_index =
                            Some(next);
                    }
                    "ArrowUp" => {
                        let current = self
                            .state
                            .main_view
                            .last_clicked_index
                            .unwrap_or(0);
                        let prev = current.saturating_sub(1);
                        self.select_item(
                            prev,
                            event.keystroke.modifiers.shift,
                            false,
                            cx,
                        );
                        self.state.main_view.last_clicked_index =
                            Some(prev);
                    }
                    "Home" => {
                        if !self.state.main_view.items.is_empty() {
                            self.select_item(
                                0,
                                event.keystroke.modifiers.shift,
                                false,
                                cx,
                            );
                            self.state
                                .main_view
                                .last_clicked_index = Some(0);
                        }
                    }
                    "End" => {
                        let last = self
                            .state
                            .main_view
                            .items
                            .len()
                            .saturating_sub(1);
                        self.select_item(
                            last,
                            event.keystroke.modifiers.shift,
                            false,
                            cx,
                        );
                        self.state.main_view.last_clicked_index =
                            Some(last);
                    }
                    _ => {}
                }
            }
            let key = keystroke.key.as_str();
            if keystroke.modifiers.control
                && (key == "a" || key == "A")
            {
                self.select_all(cx);
            }
            if key == "enter" || key == "return" {
                if let Some(&idx) = self
                    .state
                    .main_view
                    .selected_indices
                    .iter()
                    .next()
                {
                    self.open_item(idx, cx);
                }
            }
            if key == "backspace" {
                self.navigate_up(cx);
            }
        }
    }

    /// Dismiss the omnibar (path edit, search, or command palette).
    /// Returns true if the omnibar was active and was dismissed.
    pub fn dismiss_omnibar(&mut self, cx: &mut Context<Self>) -> bool {
        use crate::ui::components::navigation_bar::OmnibarMode;
        let was_active = self.state.nav_bar.edit_mode
            || matches!(
                self.state.nav_bar.omnibar_mode,
                OmnibarMode::Search | OmnibarMode::CommandPalette
            );
        if was_active {
            self.state.nav_bar.edit_mode = false;
            self.state.nav_bar.show_suggestions = false;
            if matches!(self.state.nav_bar.omnibar_mode, OmnibarMode::Search) {
                self.state.nav_bar.omnibar_mode = OmnibarMode::Path;
                self.clear_search(cx);
            } else if matches!(self.state.nav_bar.omnibar_mode, OmnibarMode::CommandPalette) {
                self.state.nav_bar.omnibar_mode = OmnibarMode::Path;
            }
            cx.notify();
        }
        was_active
    }

    pub fn save_session(&self, cx: &mut Context<Self>) {
        let nav = self.state.navigation.read(cx);
        let settings = self.state.settings.clone();

        let open_tabs: Vec<crate::config::config::TabSession> = nav
            .tabs
            .tabs
            .iter()
            .map(|t| crate::config::config::TabSession {
                path: t.path.to_string_lossy().into_owned(),
                view_mode: t.view_mode.clone(),
                zoom_level: t.zoom_level,
                secondary_pane_path: t
                    .secondary_pane_path
                    .as_ref()
                    .map(|p| p.to_string_lossy().into_owned()),
            })
            .collect();

        let active_index = nav.tabs.active_tab_index;
        let pane_layout_str = match nav.pane_layout {
            crate::core::navigation_manager::PaneLayout::Single => "single",
            crate::core::navigation_manager::PaneLayout::Horizontal => "horizontal",
            crate::core::navigation_manager::PaneLayout::Vertical => "vertical",
        }
        .to_string();
        let active_pane_str = match nav.active_pane {
            crate::core::navigation_manager::PaneFocus::Primary => "primary",
            crate::core::navigation_manager::PaneFocus::Secondary => "secondary",
        }
        .to_string();

        settings.update(cx, move |s, _cx| {
            s.config.session.open_tabs = open_tabs;
            s.config.session.active_tab_index = active_index;
            s.config.session.pane_layout = pane_layout_str;
            s.config.session.active_pane = active_pane_str;
            let _ = s.config.save();
        });
    }

    /// Save the current active path to the session service for crash recovery.
    pub fn save_session_path(&mut self, cx: &mut Context<Self>) {
        let path = self.state.navigation.read(cx).active_pane_path();
        self.state
            .session_service
            .set_last_path(path.to_string_lossy().as_ref());
    }

    /// Open the documentation URL in the default browser.
    pub fn open_help(&self, _cx: &mut Context<Self>) {
        let url = "https://hermes-agent.nousresearch.com/docs";
        tracing::info!("Opening help URL: {}", url);
        #[cfg(windows)]
        {
            use std::ffi::OsStr;
            use std::os::windows::ffi::OsStrExt;
            use windows::Win32::UI::Shell::ShellExecuteW;
            use windows::Win32::UI::WindowsAndMessaging::SW_SHOW;

            let url_wide: Vec<u16> = OsStr::new(url)
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();

            unsafe {
                ShellExecuteW(
                    None,
                    windows::core::PCWSTR::null(),
                    windows::core::PCWSTR(url_wide.as_ptr()),
                    windows::core::PCWSTR::null(),
                    None,
                    SW_SHOW,
                );
            }
        }
        #[cfg(not(windows))]
        {
            let _ = std::process::Command::new("xdg-open").arg(url).spawn();
        }
    }

    /// Restore the last session tabs and path from the session config.
    pub fn restore_last_session(&mut self, cx: &mut Context<Self>) {
        let config = self.state.settings.read(cx);
        let session = &config.config.session;
        let has_tabs = !session.open_tabs.is_empty();

        if has_tabs {
            let new_tabs: Vec<crate::core::navigation_manager::TabState> = session
                .open_tabs
                .iter()
                .map(|t| {
                    let path = crate::core::path_util::FilesPath::new(&t.path);
                    crate::core::navigation_manager::TabState::new_with_zoom(
                        path,
                        t.view_mode.clone(),
                        t.zoom_level,
                    )
                })
                .collect();

            let active_index = session
                .active_tab_index
                .min(new_tabs.len().saturating_sub(1));

            let pane_layout = match session.pane_layout.as_str() {
                "horizontal" => crate::core::navigation_manager::PaneLayout::Horizontal,
                "vertical" => crate::core::navigation_manager::PaneLayout::Vertical,
                _ => crate::core::navigation_manager::PaneLayout::Single,
            };
            let active_pane = match session.active_pane.as_str() {
                "secondary" => crate::core::navigation_manager::PaneFocus::Secondary,
                _ => crate::core::navigation_manager::PaneFocus::Primary,
            };

            self.state.navigation.update(cx, |nav, _cx| {
                nav.tabs.tabs = new_tabs;
                nav.tabs.active_tab_index = active_index;
                nav.tabs.closed_tabs.clear();
                nav.pane_layout = pane_layout;
                nav.active_pane = active_pane;
            });

            self.state.main_view.pane_type = match pane_layout {
                crate::core::navigation_manager::PaneLayout::Single => {
                    crate::ui::views::main_view::PaneType::Single
                }
                _ => crate::ui::views::main_view::PaneType::Dual,
            };

            let active_path = self
                .state
                .navigation
                .read(cx)
                .tabs
                .active_tab()
                .path
                .clone();
            self.navigate(active_path, cx);

            // Restore the zoom level from the active tab
            let restored_zoom = self.state.navigation.read(cx).tabs.active_tab().zoom_level;
            self.state.main_view.zoom_level = restored_zoom;
            self.load_tab_thumbnails(cx);
        } else {
            let last_path = self.state.session_service.last_path().to_string();
            if !last_path.is_empty() {
                tracing::info!("Restoring last session path: {}", last_path);
                let path = crate::core::path_util::FilesPath::new(&last_path);
                self.navigate(path, cx);
            } else {
                tracing::info!("No last session path to restore.");
            }
        }
    }

    // ---- Tab Operations ----

    /// Open a new tab with settings page content.
    pub fn open_settings_tab(&mut self, cx: &mut Context<Self>) {
        // Check if a settings tab is already open
        let nav = self.state.navigation.read(cx);
        let existing_index = nav.tabs.tabs.iter().position(|t| {
            matches!(t.target, crate::ui::views::types::ViewTarget::Settings)
        });
        drop(nav);

        if let Some(idx) = existing_index {
            // Switch to existing settings tab
            self.switch_tab(idx, cx);
        } else {
            // Create a new tab with Settings target
            self.state.navigation.update(cx, |nav, _cx| {
                nav.tabs.add_settings_tab();
            });
            self.state.main_view.target = crate::ui::views::types::ViewTarget::Settings;
            self.save_session(cx);
            cx.notify();
        }
    }

    pub fn new_tab(&mut self, cx: &mut Context<Self>) {
        let current_path = self.state.main_view.primary_path.clone();
        let mode = self.state.main_view.view_mode.clone();
        let zoom = self.state.main_view.zoom_level;
        self.state.navigation.update(cx, |nav, _cx| {
            nav.tabs.add_tab_with_zoom(current_path, mode, zoom);
        });
        self.save_session(cx);
        self.load_tab_thumbnails(cx);
        cx.notify();
    }

    pub fn close_tab(&mut self, index: usize, cx: &mut Context<Self>) {
        self.state.navigation.update(cx, |nav, _cx| {
            nav.tabs.close_tab(index);
        });
        self.save_session(cx);
        self.clamp_scroll_offset(cx);
        let new_path = self
            .state
            .navigation
            .read(cx)
            .tabs
            .active_tab()
            .path
            .clone();
        self.navigate(new_path, cx);
    }

    pub fn reopen_closed_tab(&mut self, cx: &mut Context<Self>) {
        let reopened = self
            .state
            .navigation
            .update(cx, |nav, _cx| nav.tabs.reopen_closed_tab());
        if reopened {
            self.save_session(cx);
            let (new_path, tab_target) = {
                let nav = self.state.navigation.read(cx);
                let tab = nav.tabs.active_tab();
                (tab.path.clone(), tab.target.clone())
            };
            self.state.main_view.target = tab_target.clone();
            match tab_target {
                crate::ui::views::types::ViewTarget::Settings => {
                    self.state
                        .nav_bar
                        .update_path(FilesPath::from("Settings"));
                    cx.notify();
                }
                _ => {
                    self.navigate(new_path, cx);
                }
            }
        }
    }

    pub fn switch_tab(&mut self, index: usize, cx: &mut Context<Self>) {
        let already_active = self
            .state
            .navigation
            .read(cx)
            .tabs
            .active_tab_index
            == index;
        if already_active {
            return;
        }
        self.state.navigation.update(cx, |nav, _cx| {
            nav.tabs.switch_tab(index);
        });
        self.save_session(cx);

        let (new_path, tab_target) = {
            let nav = self.state.navigation.read(cx);
            let tab = nav.tabs.active_tab();
            (tab.path.clone(), tab.target.clone())
        };
        self.state.main_view.target = tab_target.clone();
        match tab_target {
            crate::ui::views::types::ViewTarget::Settings => {
                self.state.nav_bar.update_path(FilesPath::from("Settings"));
                cx.notify();
            }
            _ => {
                self.navigate(new_path, cx);
            }
        }
    }

    pub fn cycle_tab_next(&mut self, cx: &mut Context<Self>) {
        let use_mru = self
            .state
            .settings
            .read(cx)
            .config
            .session
            .mru_tab_cycle;
        self.state.navigation.update(cx, |nav, _cx| {
            if use_mru {
                nav.tabs.next_tab_mru();
            } else {
                nav.tabs.next_tab();
            }
        });
        self.save_session(cx);
        let (new_path, tab_target) = {
            let nav = self.state.navigation.read(cx);
            let tab = nav.tabs.active_tab();
            (tab.path.clone(), tab.target.clone())
        };
        self.state.main_view.target = tab_target.clone();
        match tab_target {
            crate::ui::views::types::ViewTarget::Settings => {
                self.state.nav_bar.update_path(FilesPath::from("Settings"));
                cx.notify();
            }
            _ => {
                self.navigate(new_path, cx);
            }
        }
    }

    pub fn cycle_tab_prev(&mut self, cx: &mut Context<Self>) {
        let use_mru = self
            .state
            .settings
            .read(cx)
            .config
            .session
            .mru_tab_cycle;
        self.state.navigation.update(cx, |nav, _cx| {
            if use_mru {
                nav.tabs.previous_tab_mru();
            } else {
                nav.tabs.previous_tab();
            }
        });
        self.save_session(cx);
        let (new_path, tab_target) = {
            let nav = self.state.navigation.read(cx);
            let tab = nav.tabs.active_tab();
            (tab.path.clone(), tab.target.clone())
        };
        self.state.main_view.target = tab_target.clone();
        match tab_target {
            crate::ui::views::types::ViewTarget::Settings => {
                self.state.nav_bar.update_path(FilesPath::from("Settings"));
                cx.notify();
            }
            _ => {
                self.navigate(new_path, cx);
            }
        }
    }

    /// Load folder thumbnails for all open tabs (for tab bar icons).
    #[cfg(windows)]
    pub fn load_tab_thumbnails(&mut self, cx: &mut Context<Self>) {
        use crate::platform::windows::thumbnail_service::ThumbnailSize;
        use crate::app_state::icons_cache_dir;

        let tab_paths: Vec<FilesPath> = {
            let nav = self.state.navigation.read(cx);
            nav.tabs.tabs.iter().map(|t| t.path.clone()).collect()
        };

        let thumbnail_service = self.state.thumbnail_service.clone();
        let tab_cache = self.state.tab_thumbnail_cache.clone();
        let icon_dir = icons_cache_dir();
        let mut async_cx = cx.to_async();

        cx.foreground_executor()
            .spawn(async move {
                let _com_init = unsafe {
                    windows::Win32::System::Com::CoInitializeEx(
                        None,
                        windows::Win32::System::Com::COINIT_APARTMENTTHREADED,
                    )
                };

                for path in &tab_paths {
                    // Check cache first
                    if let Ok(cache) = tab_cache.lock() {
                        if cache.contains_key(path) {
                            continue;
                        }
                    }

                    // Check disk cache (full-path hash key: no cross-folder collisions)
                    let cached_png = icon_dir.join(format!(
                        "tab_{}.png",
                        crate::app_state::disk_cache_key(path)
                    ));
                    if cached_png.exists() {
                        if let Ok(mut cache) = tab_cache.lock() {
                            cache.insert(path.clone(), cached_png);
                        }
                        continue;
                    }

                    // Extract thumbnail
                    if let Ok(thumb) = thumbnail_service.extract_thumbnail(path, ThumbnailSize::Small) {
                        use image::{ImageBuffer, Rgba};
                        let w = thumb.width;
                        let h = thumb.height;
                        if w > 0 && h > 0 && thumb.data.len() as u32 >= w * h * 4 {
                            let mut rgba_data = thumb.data.clone();
                            for ch in rgba_data.chunks_exact_mut(4) {
                                ch.swap(0, 2);
                            }
                            if let Some(img) = ImageBuffer::<Rgba<u8>, _>::from_raw(w, h, rgba_data) {
                                let _ = img.save(&cached_png);
                                if let Ok(mut cache) = tab_cache.lock() {
                                    cache.insert(path.clone(), cached_png);
                                }
                            }
                        }
                    }
                }
            })
            .detach();
    }

    #[cfg(not(windows))]
    pub fn load_tab_thumbnails(&mut self, _cx: &mut Context<Self>) {}

    pub fn toggle_dual_pane(&mut self, cx: &mut Context<Self>) {
        self.state.navigation.update(cx, |nav, _cx| {
            nav.toggle_dual_pane();
        });
        let layout = self.state.navigation.read(cx).pane_layout;
        self.state.main_view.pane_type = match layout {
            PaneLayout::Single => crate::ui::views::main_view::PaneType::Single,
            _ => crate::ui::views::main_view::PaneType::Dual,
        };
        cx.notify();
    }

    /// Show a context menu for a tab at the given screen position.
    pub fn show_tab_context_menu(
        &mut self,
        tab_index: usize,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let tab_count = self.state.navigation.read(cx).tabs.tabs.len();
        self.state.active_dialog = Some(DialogType::TabContextMenu {
            tab_index,
            position,
            tab_count,
        });
        cx.notify();
    }

    /// Toggle the tab actions dropdown menu (split pane options).
    pub fn toggle_tab_actions_menu(&mut self, cx: &mut Context<Self>) {
        // Toggle: if already open, close it; otherwise open it
        if matches!(self.state.active_dialog, Some(DialogType::TabActionsDropdown { .. })) {
            self.state.active_dialog = None;
        } else {
            // Position near the tab actions button (top-left area)
            let position = Point {
                x: px(4.0),
                y: px(40.0), // Below the tab bar
            };
            self.state.active_dialog = Some(DialogType::TabActionsDropdown { position });
        }
        cx.notify();
    }

    /// Toggle window maximize/restore state.
    #[cfg(target_os = "windows")]
    pub fn toggle_maximize(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        use raw_window_handle::HasWindowHandle;
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::{
            IsZoomed, ShowWindowAsync, SW_MAXIMIZE, SW_RESTORE,
        };

        if let Ok(raw_handle) = HasWindowHandle::window_handle(window) {
            if let raw_window_handle::RawWindowHandle::Win32(win32) = raw_handle.as_raw() {
                unsafe {
                    let hwnd = HWND(win32.hwnd.get() as *mut _);
                    if IsZoomed(hwnd).as_bool() {
                        // Currently maximized → restore
                        let _ = ShowWindowAsync(hwnd, SW_RESTORE);
                        self.state.is_maximized = false;
                    } else {
                        // Currently restored → maximize
                        let _ = ShowWindowAsync(hwnd, SW_MAXIMIZE);
                        self.state.is_maximized = true;
                    }
                }
            }
        }
        cx.notify();
    }

    #[cfg(not(target_os = "windows"))]
    pub fn toggle_maximize(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.state.is_maximized = !self.state.is_maximized;
        window.zoom_window();
        cx.notify();
    }

    /// Start dragging a tab (records position and time for threshold).
    pub fn start_tab_drag(&mut self, tab_index: usize, cx: &mut Context<Self>) {
        self.state.tab_drag.dragged_index = Some(tab_index);
        self.state.tab_drag.is_dragging = true;
        self.state.tab_drag.drag_active = false;
        self.state.tab_drag.is_dragging_out = false;
        self.state.tab_drag.drag_start_x = self.state.tab_drag.current_x;
        self.state.tab_drag.drag_start_y = self.state.tab_drag.current_y;
        self.state.tab_drag.drag_start_time = Some(std::time::Instant::now());
        cx.notify();
    }

    /// Update tab drag position. Drag activates only after 500ms hold + 5px movement.
    pub fn update_tab_drag(&mut self, x: f32, y: f32, cx: &mut Context<Self>) {
        if self.state.tab_drag.is_dragging {
            self.state.tab_drag.current_x = x;
            self.state.tab_drag.current_y = y;

            if !self.state.tab_drag.drag_active {
                // Files-like: drag engages on movement alone (5px). The old
                // 500ms hold requirement made reordering feel broken.
                let dx = x - self.state.tab_drag.drag_start_x;
                let dy = y - self.state.tab_drag.drag_start_y;
                let dist_sq = dx * dx + dy * dy;
                if dist_sq > 25.0 {
                    self.state.tab_drag.drag_active = true;
                }
            }

            if self.state.tab_drag.drag_active {
                self.state.tab_drag.is_dragging_out = y < 0.0 || y > 48.0;
                let tab_count = self.state.navigation.read(cx).tabs.tabs.len();
                self.state.tab_drag.drop_target_index =
                    self.calculate_drop_index(x, tab_count);
            }
            cx.notify();
        }
    }

    /// Calculate the drop index based on mouse x position.
    pub fn calculate_drop_index(&self, mouse_x: f32, tab_count: usize) -> Option<usize> {
        if tab_count == 0 {
            return None;
        }
        // Tabs are TAB_W wide starting after the strip chrome (~40px).
        let tab_start_x = 40.0;
        let tab_width = 150.0;
        let relative_x = mouse_x - tab_start_x + self.state.tab_drag.scroll_offset;
        let index = (relative_x / tab_width) as usize;
        Some(index.min(tab_count - 1))
    }

    /// Complete tab drag (drop). Only reorders if drag was actually active.
    pub fn end_tab_drag(&mut self, _drop_index: Option<usize>, cx: &mut Context<Self>) {
        if self.state.tab_drag.drag_active {
            if let Some(dragged_idx) = self.state.tab_drag.dragged_index {
                if let Some(target_idx) = self.state.tab_drag.drop_target_index {
                    if dragged_idx != target_idx {
                        self.state.navigation.update(cx, |nav, _cx| {
                            nav.tabs.reorder_tab(dragged_idx, target_idx);
                        });
                        self.save_session(cx);
                    }
                } else if self.state.tab_drag.is_dragging_out {
                    tracing::info!("Tab dragged out of window - creating new window");
                }
            }
        }
        // Preserve scroll_offset — only reset drag state
        let saved_offset = self.state.tab_drag.scroll_offset;
        self.state.tab_drag = crate::app_state::TabDragState::default();
        self.state.tab_drag.scroll_offset = saved_offset;
        cx.notify();
    }

    /// Cancel tab drag.
    pub fn cancel_tab_drag(&mut self, cx: &mut Context<Self>) {
        self.state.tab_drag = crate::app_state::TabDragState::default();
        cx.notify();
    }

    /// Scroll the tab bar left (smooth pixel increment).
    pub fn scroll_tab_bar_left(&mut self, cx: &mut Context<Self>) {
        let step = 30.0;
        self.state.tab_drag.scroll_offset =
            (self.state.tab_drag.scroll_offset - step).max(0.0);
        cx.notify();
    }

    /// Scroll the tab bar right (smooth pixel increment).
    pub fn scroll_tab_bar_right(&mut self, cx: &mut Context<Self>) {
        let step = 30.0;
        let max_scroll =
            (self.state.navigation.read(cx).tabs.tabs.len() as f32 * 200.0) - 600.0;
        let max_scroll = max_scroll.max(0.0);
        self.state.tab_drag.scroll_offset =
            (self.state.tab_drag.scroll_offset + step).min(max_scroll);
        cx.notify();
    }

    /// Clamp scroll_offset: reset to 0 if tabs no longer overflow,
    /// otherwise clamp so we don't scroll past the end.
    fn clamp_scroll_offset(&mut self, cx: &mut Context<Self>) {
        let tab_count = self.state.navigation.read(cx).tabs.tabs.len() as f32;
        let tabs_min_total_w = tab_count * 100.0; // TAB_MIN_W
        let estimated_strip_w = 800.0;
        if tabs_min_total_w <= estimated_strip_w {
            // Tabs fit — exit scroll mode completely
            self.state.tab_drag.scroll_offset = 0.0;
        } else {
            // Tabs still overflow — clamp so we don't scroll past the end
            let max_scroll = (tab_count * 200.0) - 600.0;
            let max_scroll = max_scroll.max(0.0);
            self.state.tab_drag.scroll_offset =
                self.state.tab_drag.scroll_offset.min(max_scroll);
        }
    }

    /// Close all tabs except the specified one.
    pub fn close_other_tabs(&mut self, keep_index: usize, cx: &mut Context<Self>) {
        let tab_count = self.state.navigation.read(cx).tabs.tabs.len();
        self.state.navigation.update(cx, |nav, _cx| {
            for i in (0..tab_count).rev() {
                if i != keep_index {
                    nav.tabs.close_tab(i);
                }
            }
        });
        self.save_session(cx);
        self.clamp_scroll_offset(cx);
        cx.notify();
    }

    /// Close all tabs to the right of the specified one.
    pub fn close_tabs_to_right(&mut self, from_index: usize, cx: &mut Context<Self>) {
        let tab_count = self.state.navigation.read(cx).tabs.tabs.len();
        self.state.navigation.update(cx, |nav, _cx| {
            for i in (from_index + 1..tab_count).rev() {
                nav.tabs.close_tab(i);
            }
        });
        self.save_session(cx);
        self.clamp_scroll_offset(cx);
        cx.notify();
    }

    /// Duplicate the specified tab.
    pub fn duplicate_tab(&mut self, index: usize, cx: &mut Context<Self>) {
        let tab = self.state.navigation.read(cx).tabs.tabs.get(index).cloned();
        if let Some(tab) = tab {
            self.state.navigation.update(cx, |nav, _cx| {
                nav.tabs
                    .add_tab_with_zoom(tab.path.clone(), tab.view_mode.clone(), tab.zoom_level);
            });
            self.save_session(cx);
            cx.notify();
        }
    }

    /// Close the current tab and open a new empty tab.
    pub fn pin_tab(&mut self, _index: usize, cx: &mut Context<Self>) {
        // TODO: Implement tab pinning (keep tab open even after closing all folders)
        cx.notify();
    }

    // ---- Navigation Operations ----
    /// Start watching a folder for file system changes.
    /// `secondary` determines if this is for the secondary pane (split view).
    pub fn start_file_watcher_for(&mut self, path: FilesPath, secondary: bool) {
        use notify::{Config, EventKind, RecursiveMode, Watcher};
        use tokio::sync::mpsc;

        let (tx, rx) = mpsc::unbounded_channel();
        let mut watcher = notify::RecommendedWatcher::new(
            move |res: notify::Result<notify::Event>| {
                if let Ok(event) = res {
                    let kind = match event.kind {
                        EventKind::Create(_) => Some(crate::storage::storage::FileEvent::Created(
                            crate::core::path_util::FilesPath::new(event.paths[0].clone()),
                        )),
                        EventKind::Modify(_) => Some(crate::storage::storage::FileEvent::Modified(
                            crate::core::path_util::FilesPath::new(event.paths[0].clone()),
                        )),
                        EventKind::Remove(_) => Some(crate::storage::storage::FileEvent::Removed(
                            crate::core::path_util::FilesPath::new(event.paths[0].clone()),
                        )),
                        _ => None,
                    };
                    if let Some(e) = kind {
                        let _ = tx.send(e);
                    }
                }
            },
            Config::default(),
        );

        if let Ok(mut w) = watcher {
            let _ = w.watch(path.as_path(), RecursiveMode::NonRecursive);
            if secondary {
                self.file_watcher_secondary = Some(w);
                self.file_event_rx_secondary = Some(rx);
            } else {
                self.file_watcher = Some(w);
                self.file_event_rx = Some(rx);
            }
        }
    }

    /// Start watching primary folder
    pub fn start_file_watcher(&mut self, path: FilesPath) {
        self.stop_file_watcher(false);
        self.start_file_watcher_for(path, false);
    }

    /// Start watching secondary folder (split view)
    pub fn start_file_watcher_secondary(&mut self, path: FilesPath) {
        self.stop_file_watcher(true);
        self.start_file_watcher_for(path, true);
    }

    /// Stop file watcher. `secondary` determines which watcher to stop.
    pub fn stop_file_watcher(&mut self, secondary: bool) {
        if secondary {
            self.file_watcher_secondary = None;
            self.file_event_rx_secondary = None;
        } else {
            self.file_watcher = None;
            self.file_event_rx = None;
        }
    }

    /// Stop all file watchers
    pub fn stop_all_file_watchers(&mut self) {
        self.file_watcher = None;
        self.file_event_rx = None;
        self.file_watcher_secondary = None;
        self.file_event_rx_secondary = None;
    }

    /// Poll for file system events and refresh if any are received.
    pub fn poll_file_events(&mut self, cx: &mut Context<Self>) {
        // Poll primary pane
        if let Some(rx) = &mut self.file_event_rx {
            while let Ok(_event) = rx.try_recv() {
                self.refresh(cx);
                break;
            }
        }
        // Poll secondary pane
        if let Some(rx) = &mut self.file_event_rx_secondary {
            while let Ok(_event) = rx.try_recv() {
                self.refresh_secondary_pane(cx);
                break;
            }
        }
    }

    /// Called at the start of render(). Ticks active bouncy scroll physics,
    /// pushes offsets to scroll handles, and calls cx.notify().
    /// 
    /// cx.notify() in render() works: draw() calls set_dirty(false) at its
    /// START, and notify() sets it back to true — the dirty flag persists
    /// past draw() and the next frame callback will re-render.
    pub fn tick_bouncy_scrolls(&mut self, _window: &Window, cx: &mut Context<Self>) {
        // Compute frame-time delta for frame-rate-independent physics
        let dt = if let Some(last) = self.last_render_time {
            let elapsed = std::time::Instant::now()
                .duration_since(last)
                .as_secs_f64() as f32;
            elapsed.clamp(1.0 / 240.0, 1.0 / 15.0)
        } else {
            1.0 / 60.0
        };
        self.last_render_time = Some(std::time::Instant::now());

        let sidebar_tick;
        let main_tick;
        {
            let bs = &mut self.state.sidebar.bouncy_scroll;
            // Drain the post-settle cooldown every frame, even when idle:
            // gating it behind `animating` deadlocks scrolling permanently
            // after the first settle (wheel input needs cooldown == 0 to
            // re-arm, but nothing would ever decrement it).
            bs.post_settle_cooldown = bs.post_settle_cooldown.saturating_sub(1);
            sidebar_tick = if bs.animating {
                bs.tick_spring(dt)
            } else {
                false
            };
        }
        {
            let bs = &mut self.state.main_view.bouncy_scroll;
            // Same unconditional drain as sidebar (see above).
            bs.post_settle_cooldown = bs.post_settle_cooldown.saturating_sub(1);
            main_tick = if bs.animating {
                bs.tick_spring(dt)
            } else {
                false
            };
        }
        if sidebar_tick || main_tick {
            // Push main_view's scroll offset to UniformList handle
            let offset = self.state.main_view.bouncy_scroll.offset;
            let handle = &self.state.main_view.uniform_list_scroll_handle;
            handle.0.borrow().base_handle.set_offset(gpui::point(px(0.0), px(offset)));

            // Mark the window dirty so the next frame callback re-renders.
            // draw() clears the dirty flag at its START, then calls render(),
            // which calls this. notify() sets dirty back to true, persisting
            // past draw()'s completion.
            cx.notify();
        }
    }

    /// Refresh the secondary pane (for split view)
    pub fn refresh_secondary_pane(&mut self, cx: &mut Context<Self>) {
        let secondary_path = self
            .state
            .navigation
            .read(cx)
            .tabs
            .active_tab()
            .secondary_pane_path
            .clone();
        if let Some(path) = secondary_path {
            // Trigger a re-render which will pick up the new file list
            cx.notify();
        }
    }

    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        let path = self.state.main_view.primary_path.clone();
        self.navigate(path, cx);
    }

    /// Initialize a git repository in the current directory.
    pub fn git_init(&mut self, cx: &mut Context<Self>) -> anyhow::Result<()> {
        let path = self.state.main_view.primary_path.clone();
        git2::Repository::init(path.as_path())?;
        // Refresh git info in status bar
        self.state.status_bar.show_git_actions = GitService::is_repo(&path);
        self.state.status_bar.git_branch = GitService::get_branch(&path);
        cx.notify();
        Ok(())
    }

    /// Clone a git repository from the URL in the CloneRepo dialog
    pub fn git_clone(&mut self, cx: &mut Context<Self>) {
        // Check if there's an active CloneRepo dialog
        if let Some(DialogType::CloneRepo { ref mut state }) = self.state.active_dialog {
            // Validate inputs
            if let Err(e) = state.validate_url() {
                tracing::error!("Clone validation error: {}", e);
                return;
            }
            if let Err(e) = state.validate_destination() {
                tracing::error!("Clone validation error: {}", e);
                return;
            }

            let url = state.url.clone();
            let dest = state.destination_path.clone();
            let branch = state.branch.clone();
            let this_weak = cx.weak_entity();

            state.start_cloning();

            let mut async_cx = cx.to_async();
            cx.foreground_executor()
                .spawn(async move {
                    let result = if let Some(ref branch_name) = branch {
                        // Clone with specific branch using FetchOptions
                        let mut fo = git2::FetchOptions::new();
                        let mut builder = git2::build::RepoBuilder::new();
                        builder.fetch_options(fo);
                        builder.branch(branch_name);
                        builder.clone(&url, &dest)
                    } else {
                        git2::Repository::clone(&url, &dest)
                    };

                    this_weak
                        .update(&mut async_cx, |this, cx| {
                            match result {
                                Ok(_) => {
                                    tracing::info!("Successfully cloned {} to {:?}", url, dest);
                                    // Mark as complete and close dialog
                                    if let Some(DialogType::CloneRepo { ref mut state }) =
                                        this.state.active_dialog
                                    {
                                        state.complete_cloning();
                                    }
                                    this.close_dialog(cx);
                                    // Navigate to the cloned repo
                                    let path = crate::core::path_util::FilesPath::from(
                                        dest.to_str().unwrap_or(""),
                                    );
                                    this.navigate(path, cx);
                                }
                                Err(e) => {
                                    tracing::error!("Failed to clone {}: {}", url, e);
                                    // Update state with error
                                    if let Some(DialogType::CloneRepo { ref mut state }) =
                                        this.state.active_dialog
                                    {
                                        state.is_cloning = false;
                                    }
                                    cx.notify();
                                }
                            }
                        })
                        .ok();
                })
                .detach();
        }
        cx.notify();
    }
    /// Delete selected items (move to trash)
    pub fn delete_selected(&mut self, cx: &mut Context<Self>) {
        if self.state.main_view.selected_indices.is_empty() {
            return;
        }

        // Get selected items
        let items: Vec<_> = self
            .state
            .main_view
            .selected_indices
            .iter()
            .filter_map(|&i| self.state.main_view.items.get(i))
            .collect();

        if items.is_empty() {
            return;
        }

        let bufs: Vec<std::path::PathBuf> = items
            .iter()
            .map(|item| item.path.as_path().to_path_buf())
            .collect();
        let engine = self.state.progress_engine.clone();
        let weak = cx.weak_entity();
        let mut async_cx = cx.to_async();
        cx.foreground_executor()
            .spawn(async move {
                use crate::core::progress::OpKind;
                use crate::core::transfer::{TransferOutcome, run_delete_job};
                let n = bufs.len();
                let job = engine
                    .start_job(format!("Delete ({} items)", n), OpKind::Delete)
                    .await;
                match run_delete_job(&engine, job, bufs, true).await {
                    TransferOutcome::Done => {
                        engine.complete_job(job).await;
                    }
                    TransferOutcome::Cancelled => {
                        weak.update(&mut async_cx, |this, cx| {
                            this.state.status_center.update_job(
                                job,
                                crate::core::progress::OperationStatus::Cancelled,
                            );
                            cx.notify();
                        })
                        .ok();
                    }
                    TransferOutcome::Failed(reason) => {
                        engine.fail_job(job, reason).await;
                    }
                }
                weak.update(&mut async_cx, |this, cx| {
                    this.clear_selection(cx);
                    this.refresh(cx);
                })
                .ok();
            })
            .detach();
    }

    /// Permanently delete selected items (bypasses recycle bin)
    /// Uses std::fs::remove_file for files and std::fs::remove_dir_all for directories
    pub fn delete_permanently(&mut self, cx: &mut Context<Self>) {
        if self.state.main_view.selected_indices.is_empty() {
            return;
        }

        // Get selected item paths
        let paths: Vec<_> = self
            .state
            .main_view
            .selected_indices
            .iter()
            .filter_map(|&i| self.state.main_view.items.get(i))
            .map(|item| item.path.clone())
            .collect();

        if paths.is_empty() {
            return;
        }
        let bufs: Vec<std::path::PathBuf> =
            paths.iter().map(|p| p.as_path().to_path_buf()).collect();
        let engine = self.state.progress_engine.clone();
        let weak = cx.weak_entity();
        let mut async_cx = cx.to_async();
        cx.foreground_executor()
            .spawn(async move {
                use crate::core::progress::OpKind;
                use crate::core::transfer::{TransferOutcome, run_delete_job};
                let n = bufs.len();
                let job = engine
                    .start_job(format!("Delete ({} items)", n), OpKind::Delete)
                    .await;
                match run_delete_job(&engine, job, bufs, false).await {
                    TransferOutcome::Done => {
                        engine.complete_job(job).await;
                    }
                    TransferOutcome::Cancelled => {
                        weak.update(&mut async_cx, |this, cx| {
                            this.state.status_center.update_job(
                                job,
                                crate::core::progress::OperationStatus::Cancelled,
                            );
                            cx.notify();
                        })
                        .ok();
                    }
                    TransferOutcome::Failed(reason) => {
                        engine.fail_job(job, reason).await;
                    }
                }
                weak.update(&mut async_cx, |this, cx| {
                    this.clear_selection(cx);
                    this.refresh(cx);
                })
                .ok();
            })
            .detach();
    }

    pub fn start_edit_mode(&mut self, cx: &mut Context<Self>) {
        self.state.nav_bar.edit_mode = true;
        cx.notify();
    }

    /// Enter the navigation bar edit mode (alias for start_edit_mode)
    pub fn enter_edit_mode(&mut self, cx: &mut Context<Self>) {
        self.start_edit_mode(cx);
    }
    pub fn toggle_sidebar(&mut self, cx: &mut Context<Self>) {
        self.state.sidebar_visible = !self.state.sidebar_visible;
        cx.notify();
    }

    /// Open a path in the preferred IDE (VS Code by default).
    /// Checks if a `.git` folder exists in the selected directory (or current dir if no selection).
    pub fn open_in_ide(&mut self, path: Option<FilesPath>, cx: &mut Context<Self>) {
        let dir_path = path
            .as_ref()
            .cloned()
            .unwrap_or_else(|| self.state.main_view.primary_path.clone());

        let project_path = dir_path.as_path().to_path_buf();

        // If the path is a file, use its parent directory
        let project_dir = if project_path.is_file() {
            project_path.parent().unwrap_or(&project_path).to_path_buf()
        } else {
            project_path.clone()
        };

        let has_git = project_dir.join(".git").exists();
        tracing::info!(
            "Open in IDE: {:?} (has_git: {}, is_dir: {})",
            project_dir,
            has_git,
            project_dir.is_dir()
        );

        #[cfg(windows)]
        {
            let service = crate::platform::windows::open_in_ide_service::OpenInIDEService::new();
            let path_for_ide = project_dir.clone();
            cx.background_executor()
                .spawn(async move {
                    let _result = service.open_in_vscode(&path_for_ide).await;
                    tracing::info!("Open in IDE result: {:?}", _result);
                })
                .detach();
        }

        #[cfg(not(windows))]
        {
            let _ = project_dir;
            tracing::info!("Open in IDE not implemented on this platform");
        }

        cx.notify();
    }

    /// Sort the file list by the given column (toggles direction if same column).
    pub fn sort_by(&mut self, col: SortBy, cx: &mut Context<Self>) {
        self.state.main_view.toggle_sort(col);
        cx.notify();
    }

    /// Select a single item by its index in `main_view.items`.
    pub fn select_item(&mut self, index: usize, shift: bool, ctrl: bool, cx: &mut Context<Self>) {
        if ctrl {
            if self.state.main_view.selected_indices.contains(&index) {
                self.state.main_view.selected_indices.remove(&index);
            } else {
                self.state.main_view.selected_indices.insert(index);
            }
        } else if shift {
            if let Some(&first) = self.state.main_view.selected_indices.iter().next() {
                let start = first.min(index);
                let end = first.max(index);
                self.state.main_view.selected_indices.clear();
                for i in start..=end {
                    self.state.main_view.selected_indices.insert(i);
                }
            } else {
                self.state.main_view.selected_indices.insert(index);
            }
        } else {
            self.state.main_view.selected_indices.clear();
            self.state.main_view.selected_indices.insert(index);
        }

        // Trigger preview load if single item selected
        if self.state.main_view.selected_indices.len() == 1 {
            let idx = *self.state.main_view.selected_indices.iter().next().unwrap();
            if let Some(item) = self.state.main_view.items.get(idx) {
                // Clone data we need to avoid borrow conflict
                let item_path = item.path.clone();
                let is_file = matches!(item.file_type, FileType::File);

                if is_file {
                    self.load_preview_content(item_path.clone(), cx);
                } else {
                    self.state.main_view.preview_text =
                        Some("Loading folder contents...".to_string());
                    self.load_folder_preview(item_path.clone(), cx);
                }

                // Update details pane cache (must do after releasing item borrow)
                self.update_details_cache_from_path(&item_path, cx);
            }
        } else {
            self.state.main_view.preview_text = None;
            self.state.main_view.preview_content =
                crate::ui::views::main_view::PreviewContent::default();
            self.state.main_view.details_cache = DetailsPaneCache::default();
        }

        // Update status bar with selection info
        let selected_size: u64 = self
            .state
            .main_view
            .selected_indices
            .iter()
            .filter_map(|&i| self.state.main_view.items.get(i))
            .map(|item| item.size)
            .sum();
        self.state.status_bar.selected_count = self.state.main_view.selected_indices.len();
        self.state.status_bar.selected_size_bytes = selected_size;

        // Sync file_items model for uniform_list closures
        self.state.file_items.update(cx, |m, _| {
            m.items = self.state.main_view.items.clone();
            m.selected_indices = self.state.main_view.selected_indices.clone();
        });

        cx.notify();
    }

    /// Select all items in the current view.
    pub fn select_all(&mut self, cx: &mut Context<Self>) {
        for i in 0..self.state.main_view.items.len() {
            self.state.main_view.selected_indices.insert(i);
        }
        self.state.status_bar.selected_count = self.state.main_view.selected_indices.len();
        let selected_size: u64 = self
            .state
            .main_view
            .selected_indices
            .iter()
            .filter_map(|&i| self.state.main_view.items.get(i))
            .map(|item| item.size)
            .sum();
        self.state.status_bar.selected_size_bytes = selected_size;
        cx.notify();
    }

    /// Clear all selections.
    pub fn clear_selection(&mut self, cx: &mut Context<Self>) {
        self.state.main_view.selected_indices.clear();
        self.state.status_bar.selected_count = 0;
        self.state.status_bar.selected_size_bytes = 0;
        self.state.main_view.preview_text = None;

        // Sync file_items model
        self.state.file_items.update(cx, |m, _| {
            m.selected_indices.clear();
        });

        cx.notify();
    }

    /// Invert selection (toggle selected state for all items).
    pub fn invert_selection(&mut self, cx: &mut Context<Self>) {
        for i in 0..self.state.main_view.items.len() {
            if self.state.main_view.selected_indices.contains(&i) {
                self.state.main_view.selected_indices.remove(&i);
            } else {
                self.state.main_view.selected_indices.insert(i);
            }
        }
        self.state.status_bar.selected_count = self.state.main_view.selected_indices.len();
        let selected_size: u64 = self
            .state
            .main_view
            .selected_indices
            .iter()
            .filter_map(|&i| self.state.main_view.items.get(i))
            .map(|item| item.size)
            .sum();
        self.state.status_bar.selected_size_bytes = selected_size;

        // Sync file_items model
        self.state.file_items.update(cx, |m, _| {
            m.selected_indices = self.state.main_view.selected_indices.clone();
        });

        cx.notify();
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Column resize handling (Details view column header drag-to-resize)
    // ─────────────────────────────────────────────────────────────────────────

    /// Start a column resize operation. Called from resize handle on_mouse_down.
    pub fn start_column_resize(&mut self, column: String, start_x: f32, cx: &mut Context<Self>) {
        let widths = self.state.main_view.column_widths;
        let initial_width = match column.as_str() {
            "name" => widths.name,
            "date" => widths.date,
            "type" => widths.type_col,
            "size" => widths.size,
            _ => return,
        };
        self.column_resize = Some(ColumnResizeState {
            column,
            start_x,
            initial_width,
            original_widths: widths,
        });
    }

    /// Update column width during a resize drag. Called from on_mouse_move.
    pub fn update_column_resize(&mut self, current_x: f32) {
        if let Some(ref resize) = self.column_resize.clone() {
            let delta = current_x - resize.start_x;
            let new_width = (resize.initial_width + delta).max(40.0); // min 40px
            match resize.column.as_str() {
                "name" => self.state.main_view.column_widths.name = new_width,
                "date" => self.state.main_view.column_widths.date = new_width,
                "type" => self.state.main_view.column_widths.type_col = new_width,
                "size" => self.state.main_view.column_widths.size = new_width,
                _ => {}
            }
        }
    }

    /// End a column resize operation. Called from on_mouse_up.
    pub fn end_column_resize(&mut self) {
        self.column_resize = None;
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Drag and drop handling
    // ─────────────────────────────────────────────────────────────────────────

    /// Start dragging selected items from the given position.
    pub fn start_drag(&mut self, x: f32, y: f32, cx: &mut Context<Self>) {
        let indices: Vec<usize> = self
            .state
            .main_view
            .selected_indices
            .iter()
            .cloned()
            .collect();
        if indices.is_empty() {
            return;
        }

        use gpui::Point;
        self.state.main_view.drag_start(indices, Point::new(x, y));
        cx.notify();
    }

    /// Update the current drag position. Call this on mouse move while dragging.
    /// is_copy indicates whether Ctrl modifier is held (for copy vs move).
    pub fn update_drag(&mut self, x: f32, y: f32, is_copy: bool, cx: &mut Context<Self>) {
        use gpui::Point;
        self.state.main_view.drag_update(Point::new(x, y), is_copy);
        cx.notify();
    }

    /// End the drag operation. Call this on mouse up.
    /// If a valid drop target is under the cursor, performs the operation.
    pub fn end_drag(&mut self, x: f32, y: f32, cx: &mut Context<Self>) {
        use gpui::Point;
        let point = Point::new(x, y);
        let visible = self.state.main_view.visible_items_rc();

        // Check if we're over a valid drop target (directory)
        if let Some(target_idx) = self.state.main_view.get_drop_target(point, &visible) {
            if let Some(target_item) = self.state.main_view.items.get(target_idx) {
                let indices = self.state.main_view.drag_state.indices.clone();
                let is_copy = self.state.main_view.drag_state.is_copy;

                // Perform the move/copy operation
                for &idx in &indices {
                    if let Some(source_item) = self.state.main_view.items.get(idx) {
                        let operation = if is_copy { "copy" } else { "move" };
                        tracing::info!(
                            "Would {} {:?} to {:?}",
                            operation,
                            source_item.path,
                            target_item.path
                        );
                        // TODO: Actually perform the file operation using storage module
                    }
                }
            }
        }

        self.state.main_view.drag_end();
        cx.notify();
    }

    /// Cancel the current drag operation.
    pub fn cancel_drag(&mut self, cx: &mut Context<Self>) {
        self.state.main_view.drag_end();
        cx.notify();
    }

    /// Check if a drag operation is currently in progress.
    pub fn is_dragging(&self) -> bool {
        self.state.main_view.is_dragging()
    }

    /// Get the index of the item under the cursor for drop target highlighting.
    pub fn get_drag_drop_target(&self) -> Option<usize> {
        if !self.is_dragging() {
            return None;
        }
        let point = self.state.main_view.drag_state.current;
        let visible = self.state.main_view.visible_items_rc();
        self.state.main_view.get_drop_target(point, &visible)
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Selection mode methods
    // ─────────────────────────────────────────────────────────────────────────

    /// Set the selection mode (Normal, Checkbox, or HoverSelect)
    pub fn set_selection_mode(
        &mut self,
        mode: crate::ui::views::main_view::SelectionMode,
        cx: &mut Context<Self>,
    ) {
        self.state.main_view.selection_mode = mode;
        cx.notify();
    }

    /// Get the current selection mode
    pub fn get_selection_mode(&self) -> crate::ui::views::main_view::SelectionMode {
        self.state.main_view.selection_mode.clone()
    }

    /// Cycle through selection modes: Normal → Checkbox → HoverSelect → Normal
    pub fn cycle_selection_mode(&mut self, cx: &mut Context<Self>) {
        use crate::ui::views::main_view::SelectionMode;
        let next = match self.state.main_view.selection_mode {
            SelectionMode::Normal => SelectionMode::Checkbox,
            SelectionMode::Checkbox => SelectionMode::HoverSelect,
            SelectionMode::HoverSelect => SelectionMode::Normal,
        };
        self.state.main_view.selection_mode = next;
        cx.notify();
    }

    /// Handle item hover for hover selection mode
    pub fn handle_item_hover(&mut self, index: usize, cx: &mut Context<Self>) {
        if matches!(
            self.state.main_view.selection_mode,
            crate::ui::views::main_view::SelectionMode::HoverSelect
        ) {
            self.state.main_view.selected_indices.clear();
            self.state.main_view.selected_indices.insert(index);
            self.state.status_bar.selected_count = 1;
            if let Some(item) = self.state.main_view.items.get(index) {
                self.state.status_bar.selected_size_bytes = item.size;
            }
            cx.notify();
        }
    }

    /// Toggle checkbox selection for an item
    pub fn toggle_checkbox_selection(&mut self, index: usize, cx: &mut Context<Self>) {
        if matches!(
            self.state.main_view.selection_mode,
            crate::ui::views::main_view::SelectionMode::Checkbox
        ) {
            if self.state.main_view.selected_indices.contains(&index) {
                self.state.main_view.selected_indices.remove(&index);
            } else {
                self.state.main_view.selected_indices.insert(index);
            }
            self.state.status_bar.selected_count = self.state.main_view.selected_indices.len();
            let selected_size: u64 = self
                .state
                .main_view
                .selected_indices
                .iter()
                .filter_map(|&i| self.state.main_view.items.get(i))
                .map(|item| item.size)
                .sum();
            self.state.status_bar.selected_size_bytes = selected_size;
            cx.notify();
        }
    }

    /// Select item by index, respecting current selection mode
    pub fn select_item_mode_aware(
        &mut self,
        index: usize,
        shift: bool,
        ctrl: bool,
        cx: &mut Context<Self>,
    ) {
        // If in checkbox mode and ctrl not held, clear selection first
        if matches!(
            self.state.main_view.selection_mode,
            crate::ui::views::main_view::SelectionMode::Checkbox
        ) && !ctrl
        {
            self.state.main_view.selected_indices.clear();
        }
        self.select_item(index, shift, ctrl, cx);
    }

    /// Select an item by keyboard delta (Arrow keys + Shift, Home/End).
    /// Positive delta = forward (down), negative = backward (up).
    /// Large delta (isize::MIN/MAX) = first/last item.
    pub fn select_item_by_keyboard(&mut self, delta: isize, extend: bool, cx: &mut Context<Self>) {
        let item_count = self.state.main_view.items.len();
        if item_count == 0 {
            return;
        }

        // Determine new index based on current last_clicked_index
        let current = self
            .state
            .main_view
            .last_clicked_index
            .unwrap_or(0)
            .min(item_count.saturating_sub(1));

        let new_idx = if delta == isize::MIN {
            // Home: first item
            0
        } else if delta == isize::MAX {
            // End: last item
            item_count.saturating_sub(1)
        } else {
            (current as isize + delta).clamp(0, item_count.saturating_sub(1) as isize) as usize
        };

        self.select_item(new_idx, extend, false, cx);
        self.state.main_view.last_clicked_index = Some(new_idx);
    }

    /// Update the details pane cache with tags and extended properties from path
    fn update_details_cache_from_path(&mut self, path: &FilesPath, _cx: &mut Context<Self>) {
        let mut cache = DetailsPaneCache::default();

        // Get tags from TagService
        cache.tags = self.get_file_tags(path);

        // Get extended properties for media files
        if let Ok(ext_props) = ExtendedMetadataExtractor::extract(path) {
            // Dimensions (images/video)
            if let Some((w, h)) = ext_props.dimensions {
                cache.exif_dimensions = Some(format!("{}x{}", w, h));
            }

            // Duration (audio/video)
            if let Some(dur) = ext_props.duration {
                let total_secs = dur.as_secs();
                let mins = total_secs / 60;
                let secs = total_secs % 60;
                cache.audio_duration = Some(format!("{}:{:02}", mins, secs));
                cache.video_duration = Some(format!("{}:{:02}", mins, secs));
            }
        }

        self.state.main_view.details_cache = cache;
    }

    // ═══════════════════════════════════════════════════════════════
    // SEARCH METHODS
    // ═══════════════════════════════════════════════════════════════

    /// Start a global search with the given query
    pub fn start_search(&mut self, query: String, cx: &mut Context<Self>) {
        if query.is_empty() {
            self.clear_search(cx);
            return;
        }

        self.state.main_view.search_state.is_searching = true;
        self.state.main_view.search_state.query = query.clone();
        self.state.main_view.search_state.loading = true;

        // Perform immediate name search in current items
        let search_service = crate::domain::search::SearchService::new();
        let item_names: Vec<String> = self
            .state
            .main_view
            .items
            .iter()
            .map(|i| i.name.clone())
            .collect();
        let matching_indices = search_service.filter_items(&query, &item_names);

        self.state.main_view.search_state.results = matching_indices.into_iter().collect();
        self.state.main_view.search_state.loading = false;

        // If query contains special chars, might be a deep search
        if query.len() > 2
            && (query.contains('/') || query.contains("\\") || query.starts_with('*'))
        {
            // Trigger deep recursive search with cancellation support
            let cancel_flag = Arc::new(AtomicBool::new(false));
            self.state.main_view.search_state.cancel_token = Some(cancel_flag.clone());
            let path_clone = self.state.main_view.primary_path.clone();
            let query_clone = query.clone();
            let weak_entity = cx.weak_entity();
            let mut async_cx = cx.to_async();

            let _task = cx.foreground_executor().spawn(async move {
                let results = Self::do_deep_search(path_clone, query_clone, cancel_flag);
                let _ = weak_entity.update(&mut async_cx, |this, cx| {
                    this.state.main_view.search_state.deep_results = results.clone();

                    let file_items: Vec<FileItem> = results
                        .iter()
                        .map(|res| {
                            let modified = std::fs::metadata(res.path.as_path())
                                .ok()
                                .and_then(|m| m.modified().ok());
                            let created = std::fs::metadata(res.path.as_path())
                                .ok()
                                .and_then(|m| m.created().ok());
                            FileItem {
                                name: res.name.clone(),
                                path: res.path.clone(),
                                file_type: if res.is_dir {
                                    FileType::Directory
                                } else {
                                    FileType::File
                                },
                                size: res.size,
                                is_hidden: false,
                                is_system: false,
                                modified,
                                created,
                            }
                        })
                        .collect();

                    this.state.main_view.items = file_items.clone();
                    this.state.main_view.bump_items_revision();
                    this.state.file_items.update(cx, |m, _| {
                        m.items = file_items;
                        m.selected_indices.clear();
                    });
                    this.state.main_view.search_state.results =
                        (0..this.state.main_view.items.len()).collect();

                    this.state.main_view.search_state.loading = false;
                    cx.notify();
                });
            });
        }

        cx.notify();
    }

    /// Perform a deep recursive search in a folder (blocking, called from async context)
    fn do_deep_search(
        base_path: FilesPath,
        query: String,
        cancel_flag: Arc<AtomicBool>,
    ) -> Vec<crate::ui::views::main_view::SearchResult> {
        use crate::ui::views::main_view::SearchResult;

        let mut results = Vec::new();
        let query_lower = query.to_lowercase();

        fn is_match(name: &str, query: &str) -> bool {
            let q = query.trim_start_matches('*');
            if query.starts_with('*') && query.ends_with('*') {
                name.to_lowercase().contains(&q.to_lowercase())
            } else if query.starts_with('*') {
                name.to_lowercase().ends_with(&q.to_lowercase())
            } else if query.ends_with('*') {
                name.to_lowercase().starts_with(&q.to_lowercase())
            } else {
                name.to_lowercase().contains(&query.to_lowercase())
            }
        }

        fn scan_dir(
            dir: &std::path::Path,
            query: &str,
            results: &mut Vec<SearchResult>,
            depth: usize,
            cancel_flag: &AtomicBool,
        ) {
            if depth > 10 {
                return;
            } // Limit recursion depth
            if cancel_flag.load(Ordering::Relaxed) {
                return;
            } // Check cancellation

            let Ok(entries) = std::fs::read_dir(dir) else {
                return;
            };

            for entry in entries.flatten() {
                let Ok(metadata) = entry.metadata() else {
                    continue;
                };
                let name = entry.file_name().to_string_lossy().to_string();

                if is_match(&name, query) {
                    let path = entry.path();
                    let modified = metadata.modified().ok().and_then(|t| {
                        // Format: YYYY-MM-DD HH:MM
                        let duration = t.duration_since(std::time::UNIX_EPOCH).ok()?;
                        let secs = duration.as_secs();
                        let days_since_epoch = secs / 86400;
                        let seconds_in_day = secs % 86400;
                        let hours = seconds_in_day / 3600;
                        let minutes = (seconds_in_day % 3600) / 60;

                        // Simple year calculation
                        let mut year = 1970i64;
                        let mut remaining_days = days_since_epoch as i64;
                        while remaining_days > 365 {
                            let days_in_year =
                                if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) {
                                    366
                                } else {
                                    365
                                };
                            if remaining_days >= days_in_year {
                                remaining_days -= days_in_year;
                                year += 1;
                            } else {
                                break;
                            }
                        }

                        // Get month/day
                        let is_leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
                        let days_in_months: [u64; 12] = if is_leap {
                            [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
                        } else {
                            [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
                        };
                        let mut month = 1u64;
                        for days in days_in_months.iter() {
                            if remaining_days < *days as i64 {
                                break;
                            }
                            remaining_days -= *days as i64;
                            month += 1;
                        }
                        let day = remaining_days + 1;

                        Some(format!(
                            "{:04}-{:02}-{:02} {:02}:{:02}",
                            year, month, day, hours, minutes
                        ))
                    });
                    let is_dir = metadata.is_dir();
                    let size = if is_dir { 0 } else { metadata.len() };

                    results.push(SearchResult {
                        path: FilesPath::new(path.clone()),
                        name,
                        parent_path: dir.to_string_lossy().to_string(),
                        is_dir,
                        size,
                        modified,
                    });
                }

                // Recurse into directories
                if entry.metadata().map(|m| m.is_dir()).unwrap_or(false) {
                    if let Ok(child_dir) = entry.path().canonicalize() {
                        scan_dir(&child_dir, query, results, depth + 1, cancel_flag);
                    }
                }
            }
        }

        scan_dir(
            base_path.as_path(),
            &query_lower,
            &mut results,
            0,
            &cancel_flag,
        );
        results
    }

    /// Update the search query (called as user types)
    pub fn update_search(&mut self, query: String, cx: &mut Context<Self>) {
        self.start_search(query, cx);
    }

    /// Clear search and return to normal view
    pub fn clear_search(&mut self, cx: &mut Context<Self>) {
        // Cancel any in-flight deep search
        if let Some(token) = self.state.main_view.search_state.cancel_token.take() {
            token.store(true, Ordering::Relaxed);
        }
        self.state.main_view.search_state = crate::ui::views::main_view::SearchState::default();
        self.navigate(self.state.main_view.primary_path.clone(), cx);
        cx.notify();
    }

    /// Confirm selected suggestion or raw query from the navigation bar/omnibar
    pub fn handle_suggestion_confirm(&mut self, cx: &mut Context<Self>) {
        use crate::ui::components::navigation_bar::{OmnibarMode, SuggestionKind};
        if let Some(idx) = self.state.nav_bar.selected_suggestion_index {
            if let Some(suggestion) = self.state.nav_bar.suggestions.get(idx).cloned() {
                match suggestion.kind {
                    SuggestionKind::Path { path } => {
                        self.state.nav_bar.edit_mode = false;
                        self.state.nav_bar.show_suggestions = false;
                        self.state.nav_bar.selected_suggestion_index = None;
                        self.navigate(path, cx);
                    }
                    SuggestionKind::SearchHistory { query } => {
                        self.state.nav_bar.show_suggestions = false;
                        self.state.nav_bar.selected_suggestion_index = None;
                        self.state.nav_bar.search_query = query.clone();
                        self.update_search(query, cx);
                    }
                    SuggestionKind::Command { action, .. } => {
                        self.state.nav_bar.show_suggestions = false;
                        self.state.nav_bar.selected_suggestion_index = None;
                        self.state.nav_bar.omnibar_mode = OmnibarMode::Path;
                        self.dispatch_action(action, cx);
                    }
                }
                return;
            }
        }

        // If no suggestion selected, handle based on current mode
        match self.state.nav_bar.omnibar_mode {
            OmnibarMode::Path => {
                let path_str = self.state.nav_bar.edit_text.trim().to_string();
                self.state.nav_bar.edit_mode = false;
                self.state.nav_bar.show_suggestions = false;
                self.state.nav_bar.selected_suggestion_index = None;
                if !path_str.is_empty() {
                    let path = FilesPath::from(std::path::PathBuf::from(path_str));
                    self.navigate(path, cx);
                }
            }
            OmnibarMode::Search => {
                self.state.nav_bar.show_suggestions = false;
                self.state.nav_bar.selected_suggestion_index = None;
                let query = self.state.nav_bar.search_query.clone();
                self.update_search(query, cx);
            }
            OmnibarMode::CommandPalette => {
                if let Some(suggestion) = self.state.nav_bar.suggestions.first().cloned() {
                    if let SuggestionKind::Command { action, .. } = suggestion.kind {
                        self.state.nav_bar.show_suggestions = false;
                        self.state.nav_bar.selected_suggestion_index = None;
                        self.state.nav_bar.omnibar_mode = OmnibarMode::Path;
                        self.dispatch_action(action, cx);
                    }
                }
            }
        }
    }

    /// Check if search is currently active
    pub fn is_searching(&self) -> bool {
        self.state.main_view.search_state.is_searching
    }

    // ═══════════════════════════════════════════════════════════════
    // TERMINAL METHODS
    // ═══════════════════════════════════════════════════════════════

    /// Open a terminal at the specified path
    #[cfg(windows)]
    pub fn open_terminal_here(&mut self, path: &FilesPath, cx: &mut Context<Self>) {
        use std::process::Command;

        // Read terminal preference from config, or detect if not explicitly set
        let terminal_type = {
            let settings = self.state.settings.read(cx);
            settings.config().terminal_app.clone()
        };

        let path_buf = path.as_path().to_path_buf();
        let (cmd, args) =
            crate::core::terminal_service::TerminalService::get_command(&terminal_type, &path_buf);

        let result = Command::new(&cmd).args(&args).spawn();

        match result {
            Ok(_) => {
                tracing::info!("Opened terminal ({}) at: {:?}", cmd, path);
            }
            Err(e) => {
                tracing::error!("Failed to open terminal: {}", e);
            }
        }
    }

    /// Open a terminal as administrator at the specified path
    #[cfg(windows)]
    pub fn open_terminal_as_admin(&mut self, path: &FilesPath, _cx: &mut Context<Self>) {
        use std::process::Command;

        let path_str = path.as_path().to_string_lossy();

        // Use PowerShell's Start-Process with -Verb RunAs to elevate
        let ps_command = format!(
            "Start-Process powershell.exe -ArgumentList '-NoExit','cd \\'{}\\'' -Verb RunAs",
            path_str.replace('\'', "''")
        );

        let result = Command::new("powershell")
            .args(["-Command", &ps_command])
            .spawn();

        match result {
            Ok(_) => {
                tracing::info!("Opened elevated terminal at: {}", path_str);
            }
            Err(e) => {
                tracing::error!("Failed to open elevated terminal: {}", e);
            }
        }
    }

    /// Open terminal at the current primary path
    #[cfg(windows)]
    pub fn open_terminal_at_current(&mut self, cx: &mut Context<Self>) {
        let path = self.state.main_view.primary_path.clone();
        self.open_terminal_here(&path, cx);
    }

    #[cfg(not(windows))]
    pub fn open_terminal_here(&mut self, path: &FilesPath, cx: &mut Context<Self>) {
        use std::process::Command;

        // Read terminal preference from config
        let terminal_type = {
            let settings = self.state.settings.read(cx);
            settings.config().terminal_app.clone()
        };

        let path_buf = path.as_path().to_path_buf();
        let (cmd, args) =
            crate::core::terminal_service::TerminalService::get_command(&terminal_type, &path_buf);

        let result = Command::new(&cmd).args(&args).spawn();

        match result {
            Ok(_) => {
                tracing::info!("Opened terminal ({}) at: {:?}", cmd, path);
            }
            Err(e) => {
                tracing::error!("Failed to open terminal: {}", e);
            }
        }
    }

    #[cfg(not(windows))]
    pub fn open_terminal_as_admin(&mut self, _path: &FilesPath, _cx: &mut Context<Self>) {
        tracing::warn!("Open elevated terminal not implemented for this platform");
    }

    /// Load preview content for a file based on its type.
    /// Handles text, PDF, audio/video, and archive previews.

    /// Load text preview content for a file (existing behavior, limited to 10k chars)

    /// Load archive preview - list contents without extracting

    /// Check which third-party preview handler is available on this system.
    /// Returns: Peek (PowerToys), QuickLook, SeerPro, or None.

    /// Toggle the Preview pane.

    pub fn close_dialog(&mut self, cx: &mut Context<Self>) {
        self.state.active_dialog = None;
        self.menu_tip = None;
        cx.notify();
    }

    pub fn confirm_add_item(&mut self, cx: &mut Context<Self>) {
        if let Some(DialogType::AddItem { is_folder, name }) = self.state.active_dialog.clone() {
            let trimmed = name.trim();
            if !trimmed.is_empty() {
                let mut target_path = self.state.main_view.primary_path.as_path().to_path_buf();
                target_path.push(trimmed);
                if is_folder {
                    let _ = std::fs::create_dir_all(&target_path);
                } else {
                    let _ = std::fs::write(&target_path, b"");
                }
                // Refresh directory
                let current_path = self.state.main_view.primary_path.clone();
                self.navigate(current_path, cx);
            }
        }
        self.state.active_dialog = None;
        cx.notify();
    }

    pub fn open_zoom_slider(&mut self, position: gpui::Point<Pixels>, cx: &mut Context<Self>) {
        use crate::ui::components::dialogs::DialogType;
        // Reset any stale drag state from a previous incomplete drag
        self.zoom_drag_start = None;
        let current_zoom = self.state.main_view.zoom_level;
        let current_mode = self.state.main_view.view_mode.clone();
        self.state.active_dialog = Some(DialogType::ZoomSlider {
            position,
            current_zoom,
            current_mode,
        });
        cx.notify();
    }

    pub fn end_zoom_drag(&mut self, _cx: &mut Context<Self>) {
        self.zoom_drag_start = None;
    }

    pub fn set_zoom_level(&mut self, level: f32, cx: &mut Context<Self>) {
        let level = level.clamp(0.0, 1.0);
        self.state.main_view.zoom_level = level;

        // Keep dialog snapshots in sync so re-renders use live values
        if let Some(crate::ui::components::dialogs::DialogType::ZoomSlider {
            ref mut current_zoom,
            ..
        }) = self.state.active_dialog
        {
            *current_zoom = level;
        }

        // Persist zoom level on the active tab (per-view-mode)
        self.state.navigation.update(cx, |nav, _cx| {
            let tab = nav.tabs.active_tab_mut();
            tab.set_zoom_for_view(tab.view_mode.clone(), level);
        });

        // Save last used zoom (always) + per-directory preferences if lock is enabled
        let path = self.state.main_view.primary_path.clone();
        let view_mode = self.state.main_view.view_mode.clone();
        let view_lock = self.state.main_view.view_lock_enabled;
        let settings = self.state.settings.clone();
        settings.update(cx, move |s, _cx| {
            // Always persist as the global "last used" zoom level
            s.config.view.last_zoom_level = level;

            // Save to per-directory preferences if lock is enabled
            if view_lock {
                s.config.set_folder_preferences(
                    &path,
                    crate::config::config::FolderPreferences {
                        view_mode,
                        zoom_level: level,
                        sort_by: s.config.view.sort_by.clone(),
                        sort_ascending: s.config.view.sort_ascending,
                        folder_on_top: s.config.view.folder_on_top,
                        group_by: s.config.view.group_by,
                    },
                );
            }
            let _ = s.config.save();
        });

        // Persist session so per-tab zoom levels are saved to disk
        self.save_session(cx);

        cx.notify();
    }

    pub fn dismiss_splash(&mut self, cx: &mut Context<Self>) {
        self.state.splash_state.visible = false;
        cx.notify();
    }

    /// Load Windows Shell icons for all items in the current directory.
    /// Extracts icons via ThumbnailService, saves as PNG, caches paths.
    pub fn load_icons(&mut self, cx: &mut Context<Self>) {
        #[cfg(windows)]
        {
            use crate::platform::windows::thumbnail_service::ThumbnailSize;
            use std::path::PathBuf;

            // Determine which items are visible in the viewport so we prioritize them
            let visible_start = self
                .state
                .main_view
                .uniform_list_scroll_handle
                .0
                .borrow()
                .base_handle
                .top_item();
            let visible_end = self
                .state
                .main_view
                .uniform_list_scroll_handle
                .0
                .borrow()
                .base_handle
                .bottom_item();

            let icon_dir = icons_cache_dir();
            // Arc: per-batch clones below are refcount bumps, not 50k-item copies.
            let items = std::sync::Arc::new(self.state.main_view.items.clone());
            let thumbnail_service = self.state.thumbnail_service.clone();
            let icon_cache = self.state.icon_cache.clone();
            // Abort stale extractions when the user navigates away mid-flight.
            let cancel = self.state.nav_cancel_guard.token();
            let weak_entity = cx.weak_entity();
            let mut async_cx = cx.to_async();

            // Viewport-bounded extraction: only the visible window (+ 32-item
            // lookahead) is queued. The full-directory storm (all 50k items)
            // saturated CPU/disk and spiked RAM past 800 MB; the persistent
            // disk cache still warms incrementally as the user scrolls.
            // Viewport handles report 0..0 before first layout — fall back to
            // the first 120 items so icons still populate on cold open.
            let total = items.len();
            let mut priority_items: Vec<usize> = if visible_end > visible_start
                && visible_start < total
            {
                let end = (visible_end + 32).min(total).max(visible_start);
                (visible_start..end).collect()
            } else {
                (0..total.min(120)).collect()
            };
            // Trickle-fill: append the next uncached chunk (round-robin) so
            // rows outside any reported viewport still resolve within minutes
            // instead of waiting on viewport tracking alone. Batches that find
            // everything cached set no flags and trigger no re-render.
            // Scan is windowed (<= 4,096 slots/call) to stay off the UI budget.
            if total > 0 {
                if let Ok(cache) = icon_cache.lock() {
                    let queued: std::collections::HashSet<usize> =
                        priority_items.iter().copied().collect();
                    let start = TRICKLE_CURSOR.load(Ordering::Relaxed) % total;
                    let mut added = 0usize;
                    let mut scanned = 0usize;
                    while added < 25 && scanned < 4_096 {
                        let idx = (start + scanned) % total;
                        scanned += 1;
                        if queued.contains(&idx) {
                            continue;
                        }
                        match items.get(idx) {
                            Some(item) if !cache.contains(&item.path) => {
                                priority_items.push(idx);
                                added += 1;
                            }
                            _ => {}
                        }
                    }
                    TRICKLE_CURSOR.store(start + scanned, Ordering::Relaxed);
                }
            }

            let batch_size = 25;

            // Use foreground executor (no Send requirement) that delegates blocking
            // extraction work to the background executor batch by batch.
            cx.foreground_executor()
                .spawn(async move {
                    for chunk in priority_items.chunks(batch_size) {
                        // Navigated away mid-flight: drop remaining batches.
                        if cancel.is_cancelled() {
                            tracing::debug!(
                                "load_icons: batch aborted by nav generation change"
                            );
                            break;
                        }
                        let chunk_vec = chunk.to_vec();
                        // Clone only Send types for the background task
                        let items_clone = items.clone();
                        let thumb_clone = thumbnail_service.clone();
                        let cache_clone = icon_cache.clone();
                        let dir_clone = icon_dir.clone();
                        // Grid tiles render up to ~220px wide: extract at Large
                        // (256) so downscaling stays sharp. Medium (96) tiles
                        // upscaled look washed out next to Files Community.
                        let size = ThumbnailSize::Large;

                        // Blocking thumbnail extraction runs on background thread pool
                        let batch = async_cx
                            .background_executor()
                            .spawn(async move {
                                let _com_init = unsafe {
                                    windows::Win32::System::Com::CoInitializeEx(
                                        None,
                                        windows::Win32::System::Com::COINIT_APARTMENTTHREADED,
                                    )
                                };

                                let mut batch_extracted = false;
                                for &idx in &chunk_vec {
                                    if idx >= items_clone.len() {
                                        continue;
                                    }
                                    let item = &items_clone[idx];
                                    let path = &item.path;

                                    {
                                        if let Ok(cache) = cache_clone.lock() {
                                            if cache.contains(path) {
                                                continue;
                                            }
                                        }
                                        // Check persistent disk cache — avoids expensive COM
                                        // re-extraction for items already processed in a
                                        // previous session. Populates in-memory cache so
                                        // thumbnails appear instantly on fresh start.
                                        // Check persistent disk cache (full-path hash key)
                                        let cached_png = dir_clone.join(format!(
                                            "{}.png",
                                            crate::app_state::disk_cache_key(path)
                                        ));
                                        if cached_png.exists() {
                                            if let Ok(mut cache) = cache_clone.lock() {
                                                cache.put(path.clone(), cached_png);
                                                batch_extracted = true;
                                            }
                                            continue;
                                        }
                                    }

                                    if let Ok(thumb) = thumb_clone.extract_thumbnail(path, size) {
                                        use image::{ImageBuffer, Rgba};
                                        let w = thumb.width;
                                        let h = thumb.height;
                                        if w > 0 && h > 0 && thumb.data.len() as u32 >= w * h * 4 {
                                            let mut rgba_data = thumb.data.clone();
                                            for ch in rgba_data.chunks_exact_mut(4) {
                                                ch.swap(0, 2);
                                            }
                                            // Reject blank extractions (single solid color or
                                            // fully transparent): caching them poisons the
                                            // disk cache with white/empty tiles forever.
                                            // Real shell thumbnails always vary + have
                                            // opaque pixels, so this never fires on good data.
                                            let mut distinct =
                                                std::collections::HashSet::new();
                                            let mut any_opaque = false;
                                            for px in rgba_data.chunks_exact(4).step_by(64) {
                                                if px[3] != 0 {
                                                    any_opaque = true;
                                                }
                                                distinct.insert((px[0], px[1], px[2], px[3]));
                                                if distinct.len() > 4 {
                                                    break;
                                                }
                                            }
                                            if distinct.len() <= 1 || !any_opaque {
                                                continue;
                                            }
                                            if let Some(img) = ImageBuffer::<Rgba<u8>, _>::from_raw(
                                                w, h, rgba_data,
                                            ) {
                                                let cache_path = dir_clone.join(format!(
                                                    "{}.png",
                                                    crate::app_state::disk_cache_key(path)
                                                ));
                                                if img.save(&cache_path).is_ok() {
                                                    if let Ok(mut cache) = cache_clone.lock() {
                                                        cache.put(path.clone(), cache_path);
                                                        batch_extracted = true;
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }

                                unsafe {
                                    windows::Win32::System::Com::CoUninitialize();
                                }

                                batch_extracted
                            })
                            .await;

                        // Back on foreground — publish only this directory's icons.
                        // The LRU cache spans the session; icon_paths stays
                        // proportional to the visible folder.
                        if batch {
                            let _ = weak_entity.update(&mut async_cx, |this, cx| {
                                let dir =
                                    this.state.main_view.primary_path.as_path().to_path_buf();
                                let cache_len = this
                                    .state
                                    .icon_cache
                                    .lock()
                                    .map(|cache| cache.len())
                                    .unwrap_or(0);
                                this.state.main_view.icon_paths = this
                                    .state
                                    .icon_cache
                                    .lock()
                                    .map(|cache| {
                                        crate::app_state::icon_paths_for_dir(&cache, &dir)
                                    })
                                    .unwrap_or_default();
                                tracing::debug!(
                                    "load_icons: published {} icon_paths for {:?} (cache {})",
                                    this.state.main_view.icon_paths.len(),
                                    dir,
                                    cache_len
                                );
                                cx.notify();
                            });
                        }
                    }
                })
                .detach();
        }
        #[cfg(not(windows))]
        {
            let _ = cx;
        }
    }

    /// Check if visible viewport items lack icons, and if so, trigger
    /// icon loading for them. Safe to call repeatedly — already-cached
    /// icons are skipped.
    pub fn refresh_icons_for_viewport(&mut self, cx: &mut Context<Self>) {
        let (top, bottom) = {
            let scroll = &self.state.main_view.uniform_list_scroll_handle;
            let base = &scroll.0.borrow().base_handle;
            (base.top_item(), base.bottom_item())
        };
        let items = &self.state.main_view.items;
        let icon_paths = &self.state.main_view.icon_paths;
        let limit = bottom.min(items.len().saturating_sub(1));

        // Check if any visible items in the viewport still lack icons
        let mut needs_reload = (top..=limit).any(|i| {
            if i >= items.len() {
                return false;
            }
            !icon_paths.contains_key(&items[i].path)
        });

        // Viewport tracking can report a stale (e.g. 0..0) range even after
        // scrolling, which would permanently gate all loading behind row 0.
        // Fall back to a bounded scan of the trickle window: if uncached
        // items exist anywhere ahead of the cursor, keep pumping. Windowed
        // (<= 4,096 slots) to stay off the UI budget. Scoped to directories
        // that fit the LRU cap — beyond it, eviction would make "everything
        // cached" unreachable and the pump would churn forever.
        if !needs_reload {
            let total = items.len();
            if total > 0 && total <= crate::app_state::MAX_ICON_CACHE_ENTRIES {
                if let Ok(cache) = self.state.icon_cache.lock() {
                    let start =
                        TRICKLE_CURSOR.load(Ordering::Relaxed) % total;
                    needs_reload = (0..4_096.min(total)).any(|o| {
                        let idx = (start + o) % total;
                        items
                            .get(idx)
                            .map_or(false, |it| !cache.contains(&it.path))
                    });
                }
            }
        }

        tracing::debug!(
            "refresh_icons_for_viewport: range {}..{} of {}, cached views {}, reload={}",
            top,
            bottom,
            self.state.main_view.items.len(),
            self.state.main_view.icon_paths.len(),
            needs_reload
        );

        if needs_reload {
            tracing::debug!(
                "Viewport icons stale (viewport items {}..{}, total items {}), reloading",
                top,
                limit,
                items.len()
            );
            self.load_icons(cx);
        }
    }

    pub fn add_selected_to_shelf(&mut self, cx: &mut Context<Self>) {
        let items = &self.state.main_view.items;
        let shelf_items = &mut self.state.main_view.shelf_items;
        for &idx in &self.state.main_view.selected_indices {
            if let Some(item) = items.get(idx) {
                // Avoid duplicates
                if !shelf_items.iter().any(|s| s.path == item.path) {
                    shelf_items.push(crate::ui::views::main_view::ShelfItem {
                        path: item.path.clone(),
                        name: item.name.clone(),
                        is_dir: matches!(
                            item.file_type,
                            crate::storage::storage::FileType::Directory
                        ),
                        size: item.size,
                    });
                }
            }
        }
        cx.notify();
    }

    pub fn clear_shelf(&mut self, cx: &mut Context<Self>) {
        self.state.main_view.shelf_items.clear();
        cx.notify();
    }

    pub fn remove_from_shelf(&mut self, path: FilesPath, cx: &mut Context<Self>) {
        self.state.main_view.shelf_items.retain(|i| i.path != path);
        cx.notify();
    }

    pub fn open_command_palette(&mut self, cx: &mut Context<Self>) {
        self.state.active_dialog = Some(DialogType::CommandPalette);
        cx.notify();
    }

    pub fn new_window(&mut self, cx: &mut Context<Self>) {
        let progress_engine = self.state.progress_engine.clone();
        // Create a new database for the new window (shared same file)
        let db_path = dirs::data_local_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("Files-Rust")
            .join("files-rs.db");
        let db = Arc::new(
            crate::core::database::Database::new(db_path).unwrap_or_else(|_| {
                crate::core::database::Database::new(std::path::PathBuf::from(":memory:"))
                    .expect("Failed to create in-memory database")
            }),
        );
        let fi_res = cx.reserve_entity();
        let file_items = cx.insert_entity(fi_res, |_| {
            crate::ui::views::main_view::FileItems::default()
        });
        let app_state = AppState::new(
            self.state.main_view.primary_path.clone(),
            file_items,
            self.state.settings.clone(),
            self.state.localization.clone(),
            self.state.navigation.clone(),
            progress_engine.clone(),
            db,
            cx,
        );

        let options = gpui::WindowOptions {
            window_bounds: Some(gpui::WindowBounds::Windowed(gpui::Bounds {
                origin: Default::default(),
                size: gpui::size(gpui::Pixels::from(1280.0), gpui::Pixels::from(800.0)),
            })),
            titlebar: None,
            window_background: gpui::WindowBackgroundAppearance::Opaque,
            ..Default::default()
        };

        cx.open_window(options, move |window, cx| {
            let rx = progress_engine.subscribe();
            cx.new(|cx| AppWindow::new(app_state, rx, window, cx))
        })
        .expect("Failed to open window");
    }
    // ─────────────────────────────────────────────────────────────────────────
    // Hover-to-preview tooltip (delayed appearance for files)
    // ─────────────────────────────────────────────────────────────────────────

    const HOVER_TOOLTIP_DELAY_MS: u64 = 900;

    /// Start the hover timer for a file item. After HOVER_TOOLTIP_DELAY_MS,
    /// the tooltip appears. Calling again with a different index cancels the
    /// previous timer and restarts.
    pub fn start_hover_timer(
        &mut self,
        index: usize,
        position: gpui::Point<gpui::Pixels>,
        cx: &mut Context<Self>,
    ) {
        // Don't start if in hover selection mode
        if matches!(
            self.state.main_view.selection_mode,
            crate::ui::views::main_view::SelectionMode::HoverSelect
        ) {
            return;
        }

        // Don't start if a dialog/menu is open on top of the main view
        if self.state.active_dialog.is_some() {
            return;
        }

        // Validate item exists and is a file (not directory)
        {
            let Some(item) = self.state.main_view.items.get(index) else {
                return;
            };
            if matches!(item.file_type, FileType::Directory) {
                return;
            }
            // Already tracking this exact item - skip
            if self.hover_item_index == Some(index) {
                return;
            }
        }

        // Cancel any previous hover timer — assignment drops the old task
        self.hover_timer_task = None;

        // Store tracking state immediately
        self.hover_item_index = Some(index);

        let target_index = index;
        let tooltip_pos = position;
        let mut async_app = cx.to_async();

        let task = cx.spawn(
            move |this_weak: WeakEntity<Self>, _: &mut AsyncApp| async move {
                // Wait for hover delay on background
                async_app
                    .background_executor()
                    .timer(std::time::Duration::from_millis(
                        Self::HOVER_TOOLTIP_DELAY_MS,
                    ))
                    .await;

                // Check if we're still hovering the same item
                let still_hovering = this_weak
                    .update(&mut async_app, |this, _cx| {
                        this.hover_item_index == Some(target_index)
                    })
                    .unwrap_or(false);

                if still_hovering {
                    // Show file preview tooltip
                    this_weak
                        .update(&mut async_app, |this, cx| {
                            if let Some(item) = this.state.main_view.items.get(target_index) {
                                this.hover_tooltip = Some(HoverTooltipData {
                                    position: tooltip_pos,
                                    item: item.clone(),
                                });
                                cx.notify();
                            }
                        })
                        .ok();
                }
            },
        );

        self.hover_timer_task = Some(task);
    }

    /// Stop the hover timer and clear the tooltip.
    pub fn stop_hover_timer(&mut self, cx: &mut Context<Self>) {
        // Drop the old task to cancel it
        self.hover_timer_task = None;
        self.hover_item_index = None;
        self.hover_tooltip = None;
        cx.notify();
    }
    /// Returns true if the action was handled.
    pub fn dispatch_action(
        &mut self,
        action: crate::core::action_registry::ActionKind,
        cx: &mut Context<Self>,
    ) -> bool {
        match action {
            crate::core::action_registry::ActionKind::NavigateBack => self.navigate_back(cx),
            crate::core::action_registry::ActionKind::NavigateForward => self.navigate_forward(cx),
            crate::core::action_registry::ActionKind::NavigateUp => self.navigate_up(cx),
            crate::core::action_registry::ActionKind::NavigateToRoot => self.navigate_to_root(cx),
            crate::core::action_registry::ActionKind::SelectAll => self.select_all(cx),
            crate::core::action_registry::ActionKind::InvertSelection => self.invert_selection(cx),
            crate::core::action_registry::ActionKind::Copy => self.copy_selected(cx),
            crate::core::action_registry::ActionKind::Cut => self.cut_selected(cx),
            crate::core::action_registry::ActionKind::Paste => self.paste(cx),
            crate::core::action_registry::ActionKind::CopyItemPath => {
                self.copy_item_path(cx)
            }
            crate::core::action_registry::ActionKind::Rename => {
                if let Some(&idx) = self.state.main_view.selected_indices.iter().next() {
                    self.start_rename(idx, cx);
                }
            }
            crate::core::action_registry::ActionKind::Delete => self.delete_selected(cx),
            crate::core::action_registry::ActionKind::PermanentDelete => {
                // Shift+Delete: permanent delete - fallback for now
                self.delete_selected(cx);
            }
            crate::core::action_registry::ActionKind::ClearSelection => {
                if self.state.main_view.search_state.is_searching {
                    self.clear_search(cx);
                } else if !self.state.main_view.filter_text.is_empty() {
                    self.set_filter_text(String::new(), cx);
                } else if !self.state.main_view.selected_indices.is_empty() {
                    self.clear_selection(cx);
                }
            }
            crate::core::action_registry::ActionKind::Undo => self.undo(cx),
            crate::core::action_registry::ActionKind::Redo => self.redo(cx),
            crate::core::action_registry::ActionKind::Refresh => self.refresh(cx),
            crate::core::action_registry::ActionKind::Search => {
                self.state.nav_bar.omnibar_mode =
                    crate::ui::components::navigation_bar::OmnibarMode::Search;
                self.state.nav_bar.search_query.clear();
                self.state.nav_bar.show_suggestions = true;
                let registry = self.state.action_registry.clone();
                self.state.nav_bar.update_suggestions(&registry);
                self.state.main_view.search_state.is_searching = true;
                tracing::debug!("Ctrl+F: Search activated");
            }
            crate::core::action_registry::ActionKind::ClearFilter => {
                if self.state.main_view.search_state.is_searching {
                    self.clear_search(cx);
                } else if !self.state.main_view.filter_text.is_empty() {
                    self.set_filter_text(String::new(), cx);
                }
            }
            crate::core::action_registry::ActionKind::ToggleHiddenFiles => {
                self.toggle_setting("show_hidden_files", cx)
            }
            crate::core::action_registry::ActionKind::EditPath => {
                self.state.nav_bar.omnibar_mode =
                    crate::ui::components::navigation_bar::OmnibarMode::Path;
                self.state.nav_bar.edit_mode = true;
                self.state.nav_bar.edit_text = self
                    .state
                    .nav_bar
                    .current_path
                    .to_string_lossy()
                    .to_string();
                self.state.nav_bar.show_suggestions = true;
                let registry = self.state.action_registry.clone();
                self.state.nav_bar.update_suggestions(&registry);
                tracing::debug!("Edit path requested (Ctrl+L)");
            }
            crate::core::action_registry::ActionKind::OpenPreviewPane => {
                self.invoke_third_party_preview(cx)
            }
            crate::core::action_registry::ActionKind::CommandPalette => {
                self.state.nav_bar.omnibar_mode =
                    crate::ui::components::navigation_bar::OmnibarMode::CommandPalette;
                self.state.nav_bar.command_query.clear();
                self.state.nav_bar.show_suggestions = true;
                let registry = self.state.action_registry.clone();
                self.state.nav_bar.update_suggestions(&registry);
                tracing::debug!("Command Palette activated (Ctrl+Shift+P)");
            }
            crate::core::action_registry::ActionKind::NewTab => {
                self.new_tab(cx);
            }
            crate::core::action_registry::ActionKind::CloseTab => {
                let active = self.state.navigation.read(cx).tabs.active_tab_index;
                self.close_tab(active, cx);
            }
            crate::core::action_registry::ActionKind::ReopenClosedTab => {
                self.reopen_closed_tab(cx);
            }
            crate::core::action_registry::ActionKind::NewWindow => {
                tracing::debug!("New window requested (Ctrl+N) - not yet implemented");
            }
            crate::core::action_registry::ActionKind::NextTab => {
                self.cycle_tab_next(cx);
            }
            crate::core::action_registry::ActionKind::PreviousTab => {
                self.cycle_tab_prev(cx);
            }
            crate::core::action_registry::ActionKind::SplitVertical => {
                self.state.navigation.update(cx, |nav, _cx| {
                    nav.split_pane_vertical();
                });
                let layout = self.state.navigation.read(cx).pane_layout;
                self.state.main_view.pane_type = match layout {
                    crate::core::navigation_manager::PaneLayout::Single => crate::ui::views::main_view::PaneType::Single,
                    _ => crate::ui::views::main_view::PaneType::Dual,
                };
                cx.notify();
            }
            crate::core::action_registry::ActionKind::SplitHorizontal => {
                self.state.navigation.update(cx, |nav, _cx| {
                    nav.split_pane_horizontal();
                });
                let layout = self.state.navigation.read(cx).pane_layout;
                self.state.main_view.pane_type = match layout {
                    crate::core::navigation_manager::PaneLayout::Single => crate::ui::views::main_view::PaneType::Single,
                    _ => crate::ui::views::main_view::PaneType::Dual,
                };
                cx.notify();
            }
            crate::core::action_registry::ActionKind::OpenHelp => {
                self.open_help(cx);
            }
            crate::core::action_registry::ActionKind::OpenSettings => {
                self.open_settings_tab(cx);
            }
            crate::core::action_registry::ActionKind::RestoreLastSession => {
                self.restore_last_session(cx);
            }
        }
        true
    }

    /// Build the keyboard shortcut overlay modal.
    pub fn build_shortcut_overlay(&self, cx: &mut Context<Self>) -> impl IntoElement {
        use gpui::*;
        let registry = &self.state.action_registry;
        let pairs = registry.iter();

        // Group actions by category
        let navigation: Vec<_> = pairs
            .iter()
            .filter(|(a, _)| {
                matches!(
                    a,
                    crate::core::action_registry::ActionKind::NavigateBack
                        | crate::core::action_registry::ActionKind::NavigateForward
                        | crate::core::action_registry::ActionKind::NavigateUp
                        | crate::core::action_registry::ActionKind::NavigateToRoot
                )
            })
            .cloned()
            .collect();

        let selection: Vec<_> = pairs
            .iter()
            .filter(|(a, _)| {
                matches!(
                    a,
                    crate::core::action_registry::ActionKind::SelectAll
                        | crate::core::action_registry::ActionKind::InvertSelection
                        | crate::core::action_registry::ActionKind::Copy
                        | crate::core::action_registry::ActionKind::Cut
                        | crate::core::action_registry::ActionKind::Paste
                        | crate::core::action_registry::ActionKind::Rename
                        | crate::core::action_registry::ActionKind::Delete
                        | crate::core::action_registry::ActionKind::PermanentDelete
                        | crate::core::action_registry::ActionKind::ClearSelection
                )
            })
            .cloned()
            .collect();

        let view_actions: Vec<_> = pairs
            .iter()
            .filter(|(a, _)| {
                matches!(
                    a,
                    crate::core::action_registry::ActionKind::Refresh
                        | crate::core::action_registry::ActionKind::Search
                        | crate::core::action_registry::ActionKind::ClearFilter
                        | crate::core::action_registry::ActionKind::ToggleHiddenFiles
                        | crate::core::action_registry::ActionKind::EditPath
                        | crate::core::action_registry::ActionKind::OpenPreviewPane
                )
            })
            .cloned()
            .collect();

        let tabs: Vec<_> = pairs
            .iter()
            .filter(|(a, _)| {
                matches!(
                    a,
                    crate::core::action_registry::ActionKind::NewTab
                        | crate::core::action_registry::ActionKind::CloseTab
                        | crate::core::action_registry::ActionKind::ReopenClosedTab
                        | crate::core::action_registry::ActionKind::NewWindow
                )
            })
            .cloned()
            .collect();

        let others: Vec<_> = pairs
            .iter()
            .filter(|(a, _)| matches!(a, crate::core::action_registry::ActionKind::CommandPalette))
            .cloned()
            .collect();

        let view = cx.entity().clone();

        div()
            .absolute()
            .top(px(0.0))
            .left(px(0.0))
            .right(px(0.0))
            .bottom(px(0.0))
            .bg(rgba(0x000000AAu32))
            .flex()
            .items_center()
            .justify_center()
            .on_mouse_down(MouseButton::Left, {
                let view = view.clone();
                move |_, _, cx| {
                    view.update(cx, |this, cx| {
                        this.state.show_shortcut_overlay = false;
                        cx.notify();
                    });
                }
            })
            .child(
                div()
                    .bg(rgb(0x2D2D2Du32))
                    .rounded(px(12.0))
                    .p(px(24.0))
                    .w(px(560.0))
                    .h(px(440.0))
                    .on_mouse_down(MouseButton::Left, {
                        let view = view.clone();
                        move |_, _, cx| {
                            // Clicking inside doesn't close
                            cx.stop_propagation();
                        }
                    })
                    .child(
                        div()
                            .flex_col()
                            .gap(px(16.0))
                            .h_full()
                            .overflow_hidden()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .child(
                                        div()
                                            .font_weight(FontWeight(700.0))
                                            .text_size(px(18.0))
                                            .child("Keyboard Shortcuts"),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(13.0))
                                            .text_color(rgba(0xFFFFFF99u32))
                                            .child("Press Escape to close"),
                                    ),
                            )
                            .child(div().h(px(1.0)).w_full().bg(rgba(0xFFFFFF30u32)))
                            .child({
                                // Build sections as Vec to handle different types
                                let mut sections: Vec<AnyElement> = Vec::new();
                                if !navigation.is_empty() {
                                    sections.push(
                                        Self::shortcut_section("Navigation", &navigation)
                                            .into_any_element(),
                                    );
                                }
                                if !selection.is_empty() {
                                    sections.push(
                                        Self::shortcut_section("Selection & Editing", &selection)
                                            .into_any_element(),
                                    );
                                }
                                if !view_actions.is_empty() {
                                    sections.push(
                                        Self::shortcut_section("View", &view_actions)
                                            .into_any_element(),
                                    );
                                }
                                if !tabs.is_empty() {
                                    sections.push(
                                        Self::shortcut_section("Tabs & Windows", &tabs)
                                            .into_any_element(),
                                    );
                                }
                                if !others.is_empty() {
                                    sections.push(
                                        Self::shortcut_section("Commands", &others)
                                            .into_any_element(),
                                    );
                                }
                                div()
                                    .flex_col()
                                    .gap(px(12.0))
                                    .flex_grow()
                                    .overflow_hidden()
                                    .children(sections)
                            }),
                    ),
            )
    }

    fn shortcut_section<'a>(
        title: &'a str,
        items: &'a [(
            crate::core::action_registry::ActionKind,
            crate::core::action_registry::KeyCombination,
        )],
    ) -> impl IntoElement + 'a {
        if items.is_empty() {
            return div().into_any_element();
        }
        div()
            .flex_col()
            .gap(px(6.0))
            .child(
                div()
                    .font_weight(FontWeight(600.0))
                    .text_size(px(14.0))
                    .text_color(rgba(0xFFFFFFCCu32))
                    .child(title.to_string()),
            )
            .child(
                div()
                    .flex_col()
                    .gap(px(2.0))
                    .children(items.iter().map(|(action, key)| {
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .py(px(3.0))
                            .px(px(4.0))
                            .child(div().text_size(px(13.0)).child(action.label()))
                            .child(
                                div()
                                    .bg(rgba(0xFFFFFF30u32))
                                    .px(px(6.0))
                                    .py(px(2.0))
                                    .rounded(px(4.0))
                                    .text_size(px(12.0))
                                    .child(key.to_string()),
                            )
                    })),
            )
            .into_any_element()
    }
}

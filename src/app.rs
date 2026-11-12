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
    icons_cache_dir, AppState, AppWindow, ButtonTooltipData, HoverTooltipData, SidebarDragState,
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
            button_tooltip: None,
            button_tooltip_task: None,
            menu_tip: None,
            hover_timer_task: None,
            tab_hover_timer_task: None,
            menu_task: None,
            submenu_grace: None,
            submenu_grace_gen: 0,
            submenu_panel_hovered: false,
            submenu_row_hovered: false,
            submenu_intent: None,
            submenu_intent_gen: 0,
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
            bounds_subscription: None,
            settle_task: None,
        };
        let initial_path = window_view.state.main_view.primary_path.clone();

        let show_chk = window_view.state.settings.read(cx).config.preferences.show_checkboxes_when_selecting;
        if show_chk {
            window_view.state.main_view.selection_mode = crate::ui::views::main_view::SelectionMode::Checkbox;
            window_view.state.secondary_view.selection_mode = crate::ui::views::main_view::SelectionMode::Checkbox;
        }

        window_view.state.secondary_file_items =
            cx.new(|_| crate::ui::views::main_view::FileItems::default());

        // Bounded GPU image cache for file-view thumbnails (replaces GPUI's
        // never-evicting global cache for our tiles; see bounded_image_cache).
        window_view.state.image_cache = Some(cx.new(|_| {
            crate::ui::components::bounded_image_cache::BoundedImageCache::new(
                crate::ui::components::bounded_image_cache::MAX_GPU_IMAGE_ENTRIES,
            )
        }));

        // Capture Win32 HWND for this window
        #[cfg(windows)]
        {
            use raw_window_handle::{HasWindowHandle, RawWindowHandle};
            if let Ok(h) = _window.window_handle() {
                if let RawWindowHandle::Win32(w) = h.as_raw() {
                    window_view.state.window_hwnd = w.hwnd.get() as isize;
                }
            }
        }

        // Restore tabs from saved session ONLY for the initial startup window (window_id == 1).
        // Secondary windows spawned via "Move tab to new window" or drag-out keep their own TabState!
        let is_primary_launch_window = window_view.state.window_id == 1;
        let restored = {
            let config = window_view.state.settings.read(cx);
            let session = &config.config.session;
            if is_primary_launch_window && !session.open_tabs.is_empty() {
                let new_tabs: Vec<crate::core::navigation_manager::TabState> = session
                    .open_tabs
                    .iter()
                    .map(|t| {
                        let path = crate::core::path_util::FilesPath::new(&t.path);
                        let mut ts = crate::core::navigation_manager::TabState::new_with_zoom(
                            path,
                            t.view_mode.clone(),
                            t.zoom_level,
                        );
                        ts.secondary_pane_path = t
                            .secondary_pane_path
                            .as_ref()
                            .map(|sp| crate::core::path_util::FilesPath::new(sp));
                        ts
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
            window_view.navigate_pane(
                active_path,
                crate::core::navigation_manager::PaneFocus::Primary,
                cx,
            );
            if window_view.state.main_view.pane_type
                == crate::ui::views::main_view::PaneType::Dual
            {
                window_view.sync_secondary_pane_on_split(cx);
            }

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
            // Secondary window or fresh session: sync from the window's own NavigationState
            window_view.sync_active_tab_to_views(cx);
            window_view.load_tab_thumbnails(cx);
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

        // Re-render on every window geometry change (resize, maximize,
        // unmaximize, monitor move). cx.notify() alone only marks dirty — if
        // the frame loop is asleep (no input since the drag settled) nothing
        // pumps it, so the window sits on a mid-drag frame until an unrelated
        // event repaints. Spawning a no-op foreground task posts a real
        // Win32 message (WM_GPUI_TASK_DISPATCHED_ON_MAIN_THREAD), which
        // wakes the loop; it then sees the dirty flag and paints the final
        // size. (window.request_animation_frame() cannot be used here — it
        // requires an active render and panics outside one.)
        // Settle guard: bounds changes that land mid-draw can be swallowed
        // (GPUI drops the invalidation while drawing), losing the final
        // frame. Re-arm a single 500ms timer on every change — only the
        // timer for the LAST change fires, re-rendering + forcing a real OS
        // repaint so the final geometry always lands.
        {
            let sub = cx.observe_window_bounds(_window, move |this, win, cx| {
                if let Some(os_maximized) = crate::wallpaper_mica::is_main_hwnd_maximized() {
                    this.state.is_maximized = os_maximized;
                }
                let b_size = win.bounds().size;
                let vp_size = win.viewport_size();
                if (f32::from(b_size.width) - f32::from(vp_size.width)).abs() > 1.0
                    || (f32::from(b_size.height) - f32::from(vp_size.height)).abs() > 1.0
                {
                    crate::wallpaper_mica::sync_window_size();
                }
                cx.notify();
                cx.spawn(async move |_, _| {}).detach();
                this.settle_task.take();
                let task = cx.spawn(async move |this, cx| {
                    cx.background_executor()
                        .timer(std::time::Duration::from_millis(500))
                        .await;
                    let updated = this
                        .update(cx, |this, cx| {
                            if let Some(os_maximized) =
                                crate::wallpaper_mica::is_main_hwnd_maximized()
                            {
                                this.state.is_maximized = os_maximized;
                            }
                            cx.notify();
                        })
                        .is_ok();
                    tracing::warn!(
                        "settle-guard fired updated={} hwnd_set={}",
                        updated,
                        crate::wallpaper_mica::main_hwnd_set()
                    );
                    // Belt and braces: a real OS repaint heals a stuck
                    // mid-drag frame even if GPUI's pump already drained.
                    crate::wallpaper_mica::force_repaint();
                });
                this.settle_task = Some(task);
            });
            window_view.bounds_subscription = Some(sub);
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

        // Load persistent folder-size index on a background thread and optionally index all fixed drives once
        if is_primary_launch_window {
            let should_index_drives = window_view
                .state
                .settings
                .read(cx)
                .config
                .view
                .index_all_drives_folder_sizes;
            if should_index_drives {
                window_view.trigger_index_all_drives(false, cx);
            } else {
                std::thread::spawn(|| {
                    crate::storage::folder_size::ensure_persistent_cache_loaded();
                });
            }
        }

        window_view
    }

    /// Cancel all active pointer-tracking states (drag scroll, column
    /// resize, zoom drag, tab drag, sidebar resize/reorder drags).
    /// Called on window focus loss: without this, a drag in progress sticks
    /// forever because its mouse-up is delivered to another app (or never).
    pub fn cancel_pointer_tracking(&mut self, cx: &mut Context<Self>) {
        self.state.main_view.bouncy_scroll.release();
        self.state.secondary_view.bouncy_scroll.release();
        self.state.main_view.finish_live_lasso();
        self.state.secondary_view.finish_live_lasso();
        self.state.main_view.dual_pane_resize_start = None;
        self.column_resize = None;
        self.zoom_drag_start = None;
        self.sidebar_pinned_drag = None;
        self.state.sidebar.resize_start_x = None;
        self.state.sidebar.resize_start_width = None;
        self.state.sidebar.drag_section_idx = None;
        self.state.sidebar.drag_item = None;
        self.state.main_view.drag_state = crate::ui::views::types::DragState::default();
        // Tab strip drag: preserve scroll position, drop the drag and clear cross-window previews.
        self.clear_external_drop_preview_on_target(cx);
        self.state.tab_drag.clear_drag_only();
        self.clear_tab_hover(cx);
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
            JobUpdate::UpdateJob(job) => {
                self.state.status_center.upsert_job(job);
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
        self.sync_status_center_dialog();
        self.sync_taskbar_progress();
        cx.notify();
    }

    /// Synchronize the open `DialogType::StatusCenter` snapshot with `self.state.status_center`.
    pub fn sync_status_center_dialog(&mut self) {
        if let Some(DialogType::StatusCenter {
            active_operations,
            expanded,
            notifications,
            conflicts,
            active_filter,
            selected_index,
            ..
        }) = self.state.active_dialog.as_mut()
        {
            *active_operations = self.state.status_center.jobs.clone();
            *expanded = self.state.status_center.expanded.clone();
            *notifications = self.state.status_center.notifications.clone();
            *conflicts = self.state.status_center.conflicts.clone();
            *active_filter = self.state.status_center.active_filter;
            *selected_index = self.state.status_center.selected_index;
        }
    }

    /// Synchronize Windows Taskbar button progress bar (`ITaskbarList3`) with `StatusCenterState`.
    fn sync_taskbar_progress(&self) {
        if self.state.window_hwnd == 0 {
            return;
        }
        use crate::platform::windows::taskbar_service::{
            TaskbarProgressService, TaskbarProgressState,
        };
        let service = TaskbarProgressService::new();
        service.set_hwnd(self.state.window_hwnd);
        if let Some(prog) = self.state.status_center.aggregate_progress() {
            let pct = (prog * 100.0).round() as u64;
            let any_paused = self
                .state
                .status_center
                .jobs
                .iter()
                .any(|j| j.is_paused());
            if any_paused {
                let _ = service.set_paused(pct);
            } else if self.state.status_center.has_error() {
                let _ = service.set_error(pct.max(10));
            } else {
                let _ = service.set_normal(pct);
            }
        } else {
            let _ = service.set_state(TaskbarProgressState::None);
        }
    }

    /// Toggle the Status & Notification Center flyout open or closed.
    pub fn toggle_status_center(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if matches!(
            self.state.active_dialog,
            Some(DialogType::StatusCenter { .. })
        ) {
            self.state.status_center.is_open = false;
            self.close_dialog(cx);
            return;
        }
        self.state.status_center.is_open = true;
        self.state.status_center.mark_all_read();
        let vp = window.viewport_size();
        let pos = gpui::Point {
            x: px((f32::from(vp.width) - 424.0).max(8.0)),
            y: px(88.0),
        };
        self.state.active_dialog = Some(DialogType::StatusCenter {
            active_operations: self.state.status_center.jobs.clone(),
            expanded: self.state.status_center.expanded.clone(),
            notifications: self.state.status_center.notifications.clone(),
            conflicts: self.state.status_center.conflicts.clone(),
            active_filter: self.state.status_center.active_filter,
            selected_index: self.state.status_center.selected_index,
            position: Some(pos),
        });
        cx.notify();
    }

    /// Toggle Pause / Resume on a running transfer job in `ProgressEngine`.
    pub fn toggle_pause_job(&mut self, id: uuid::Uuid, cx: &mut Context<Self>) {
        let _paused = self.state.progress_engine.toggle_pause(id);
        self.sync_status_center_dialog();
        self.sync_taskbar_progress();
        cx.notify();
    }

    /// Commit any active rename across primary or secondary pane if in progress
    pub fn commit_rename_if_active(&mut self, cx: &mut Context<Self>) {
        if self.state.main_view.renaming_index.is_some()
            || self.state.secondary_view.renaming_index.is_some()
        {
            self.commit_rename(cx);
        }
    }
}

fn prev_word_boundary(text: &str, cur: usize) -> usize {
    let chars: Vec<char> = text.chars().collect();
    if cur == 0 || chars.is_empty() {
        return 0;
    }
    let mut i = cur.min(chars.len());
    while i > 0 && !chars[i - 1].is_alphanumeric() {
        i -= 1;
    }
    while i > 0 && chars[i - 1].is_alphanumeric() {
        i -= 1;
    }
    i
}

fn next_word_boundary(text: &str, cur: usize) -> usize {
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    if cur >= len {
        return len;
    }
    let mut i = cur;
    while i < len && chars[i].is_alphanumeric() {
        i += 1;
    }
    while i < len && !chars[i].is_alphanumeric() {
        i += 1;
    }
    i
}

impl AppWindow {
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

        // Escape during an active tab drag cancels the drag and restores the tab to its initial slot
        if self.state.tab_drag.is_dragging
            && keystroke.key.as_str().eq_ignore_ascii_case("escape")
        {
            self.cancel_tab_drag(cx);
            return;
        }

        // Menu key (VK_APPS) / Shift+F10: open the context menu for the
        // current selection, like Files does (Files-community parity).
        let key_name = keystroke.key.as_str();
        if key_name == "menu"
            || (key_name.eq_ignore_ascii_case("f10") && keystroke.modifiers.shift)
        {
            let vp = window.viewport_size();
            let pos = gpui::Point {
                x: (vp.width - gpui::px(250.0)) / 2.0,
                y: vp.height * 0.4,
            };
            let first_selected = self
                .state
                .active_view()
                .selected_indices
                .iter()
                .next()
                .copied();
            self.show_context_menu(first_selected, pos, window, cx);
            return;
        }

        // Ctrl+Tab / Ctrl+Shift+Tab and Ctrl+PageDown / Ctrl+PageUp: cycle tabs
        if keystroke.modifiers.control && !keystroke.modifiers.alt && !keystroke.modifiers.platform {
            let key = keystroke.key.as_str();
            if key.eq_ignore_ascii_case("tab") {
                if keystroke.modifiers.shift {
                    self.cycle_tab_prev(cx);
                } else {
                    self.cycle_tab_next(cx);
                }
                return;
            }
            if key.eq_ignore_ascii_case("pagedown") {
                if keystroke.modifiers.shift {
                    self.move_active_tab_right(cx);
                } else {
                    self.cycle_tab_next(cx);
                }
                return;
            }
            if key.eq_ignore_ascii_case("pageup") {
                if keystroke.modifiers.shift {
                    self.move_active_tab_left(cx);
                } else {
                    self.cycle_tab_prev(cx);
                }
                return;
            }
            if keystroke.modifiers.shift && (key.eq_ignore_ascii_case("left") || key.eq_ignore_ascii_case("arrowleft")) {
                self.move_active_tab_left(cx);
                return;
            }
            if keystroke.modifiers.shift && (key.eq_ignore_ascii_case("right") || key.eq_ignore_ascii_case("arrowright")) {
                self.move_active_tab_right(cx);
                return;
            }
        }

        // Ctrl+1..8: switch to tab 1..8; Ctrl+9: switch to last tab (Files-community parity)
        if keystroke.modifiers.control
            && !keystroke.modifiers.alt
            && !keystroke.modifiers.platform
            && !keystroke.modifiers.shift
        {
            let key = keystroke.key.as_str();
            if key == "9" {
                let tab_count = self.state.navigation.read(cx).tabs.tabs.len();
                if tab_count > 0 {
                    self.switch_tab(tab_count - 1, cx);
                    return;
                }
            } else if let Some(digit) = key.chars().next().and_then(|c| c.to_digit(10)) {
                let tab_count = self.state.navigation.read(cx).tabs.tabs.len();
                if (1..=8).contains(&digit) {
                    let idx = (digit - 1) as usize;
                    if idx < tab_count {
                        self.switch_tab(idx, cx);
                        return;
                    }
                }
            }
        }

        // Intercept keyboard navigation when Status Center flyout is open
        if matches!(
            self.state.active_dialog,
            Some(DialogType::StatusCenter { .. })
        ) {
            let key = keystroke.key.as_str();
            let total_cards = self.state.status_center.conflicts.len()
                + self.state.status_center.jobs.len()
                + self.state.status_center.notifications.len();
            match key.to_lowercase().as_str() {
                "escape" => {
                    self.state.status_center.is_open = false;
                    self.close_dialog(cx);
                    return;
                }
                "up" | "arrowup" => {
                    if total_cards > 0 {
                        if self.state.status_center.selected_index > 0 {
                            self.state.status_center.selected_index -= 1;
                        } else {
                            self.state.status_center.selected_index = total_cards - 1;
                        }
                        self.sync_status_center_dialog();
                        cx.notify();
                    }
                    return;
                }
                "down" | "arrowdown" => {
                    if total_cards > 0 {
                        self.state.status_center.selected_index =
                            (self.state.status_center.selected_index + 1) % total_cards;
                        self.sync_status_center_dialog();
                        cx.notify();
                    }
                    return;
                }
                "tab" | "right" | "arrowright" => {
                    use crate::ui::components::status_center::StatusCenterFilter;
                    self.state.status_center.active_filter =
                        match self.state.status_center.active_filter {
                            StatusCenterFilter::All => StatusCenterFilter::Active,
                            StatusCenterFilter::Active => StatusCenterFilter::Completed,
                            StatusCenterFilter::Completed => StatusCenterFilter::Alerts,
                            StatusCenterFilter::Alerts => StatusCenterFilter::All,
                        };
                    self.state.status_center.selected_index = 0;
                    self.sync_status_center_dialog();
                    cx.notify();
                    return;
                }
                "left" | "arrowleft" => {
                    use crate::ui::components::status_center::StatusCenterFilter;
                    self.state.status_center.active_filter =
                        match self.state.status_center.active_filter {
                            StatusCenterFilter::All => StatusCenterFilter::Alerts,
                            StatusCenterFilter::Active => StatusCenterFilter::All,
                            StatusCenterFilter::Completed => StatusCenterFilter::Active,
                            StatusCenterFilter::Alerts => StatusCenterFilter::Completed,
                        };
                    self.state.status_center.selected_index = 0;
                    self.sync_status_center_dialog();
                    cx.notify();
                    return;
                }
                "space" => {
                    let job_idx = self.state.status_center.selected_index;
                    if let Some(job) = self.state.status_center.jobs.iter().rev().nth(job_idx) {
                        let id = job.id;
                        if job.is_running_or_paused() {
                            self.toggle_pause_job(id, cx);
                        } else {
                            self.state.status_center.toggle_expanded(id);
                            self.sync_status_center_dialog();
                            cx.notify();
                        }
                    }
                    return;
                }
                "e" => {
                    let job_idx = self.state.status_center.selected_index;
                    if let Some(job) = self.state.status_center.jobs.iter().rev().nth(job_idx) {
                        let id = job.id;
                        self.state.status_center.toggle_expanded(id);
                        self.sync_status_center_dialog();
                        cx.notify();
                    }
                    return;
                }
                "enter" | "return" => {
                    let job_idx = self.state.status_center.selected_index;
                    if let Some(job) = self.state.status_center.jobs.iter().rev().nth(job_idx) {
                        if let Some(dest) = job.destination_path.clone() {
                            self.state.status_center.is_open = false;
                            self.close_dialog(cx);
                            self.navigate(dest, cx);
                        } else {
                            let id = job.id;
                            self.state.status_center.toggle_expanded(id);
                            self.sync_status_center_dialog();
                            cx.notify();
                        }
                    }
                    return;
                }
                "delete" | "backspace" => {
                    let job_idx = self.state.status_center.selected_index;
                    if let Some(job) = self.state.status_center.jobs.iter().rev().nth(job_idx) {
                        let id = job.id;
                        if job.is_running_or_paused() {
                            self.state.progress_engine.request_cancel(id);
                        } else {
                            self.state.status_center.jobs.retain(|j| j.id != id);
                            self.state.status_center.expanded.retain(|e| *e != id);
                            self.sync_status_center_dialog();
                        }
                        cx.notify();
                    }
                    return;
                }
                "c" => {
                    self.state.status_center.clear_finished();
                    self.sync_status_center_dialog();
                    cx.notify();
                    return;
                }
                _ => {}
            }
        }

        // Intercept Navigation Bar / Omnibar input typing & keyboard shortcuts
        let is_nav_bar_active = self.state.nav_bar.is_focused();

        if is_nav_bar_active {
            use crate::ui::components::navigation_bar::{OmnibarMode, SuggestionKind};
            let key = keystroke.key.as_str();
            let key_lower = key.to_lowercase();
            let ctrl = keystroke.modifiers.control;
            let shift = keystroke.modifiers.shift;
            let alt = keystroke.modifiers.alt;

            // Alt+1 / Alt+2 / Alt+3: direct Omnibar mode switching
            if alt && !ctrl {
                match key_lower.as_str() {
                    "1" => {
                        self.activate_omnibar_path_mode(cx);
                        return;
                    }
                    "2" => {
                        self.activate_omnibar_command_mode(cx);
                        return;
                    }
                    "3" => {
                        self.activate_omnibar_search_mode(cx);
                        return;
                    }
                    "r" => {
                        self.toggle_search_option("regex", cx);
                        return;
                    }
                    "c" => {
                        self.toggle_search_option("case", cx);
                        return;
                    }
                    "t" => {
                        self.toggle_search_option("content", cx);
                        return;
                    }
                    "m" => {
                        self.toggle_search_option("metadata", cx);
                        return;
                    }
                    "s" => {
                        self.toggle_search_option("recursive", cx);
                        return;
                    }
                    _ => {}
                }
            }

            // Ctrl+L / Ctrl+F / Ctrl+Shift+P while already inside Omnibar switches modes cleanly
            if ctrl && !alt && !keystroke.modifiers.platform {
                match key_lower.as_str() {
                    "l" => {
                        self.activate_omnibar_path_mode(cx);
                        return;
                    }
                    "f" | "e" => {
                        self.activate_omnibar_search_mode(cx);
                        return;
                    }
                    "p" if shift => {
                        self.activate_omnibar_command_mode(cx);
                        return;
                    }
                    "a" => {
                        self.state.nav_bar.select_all();
                        cx.notify();
                        return;
                    }
                    "c" => {
                        let txt = self.state.nav_bar.active_text().to_string();
                        if !txt.is_empty() {
                            cx.write_to_clipboard(gpui::ClipboardItem::new_string(txt));
                        }
                        return;
                    }
                    "x" => {
                        let txt = self.state.nav_bar.active_text().to_string();
                        if !txt.is_empty() {
                            cx.write_to_clipboard(gpui::ClipboardItem::new_string(txt));
                            self.state.nav_bar.set_active_text(String::new(), false);
                            if matches!(self.state.nav_bar.omnibar_mode, OmnibarMode::Search) {
                                self.update_search(String::new(), cx);
                            }
                            self.refresh_omnibar_suggestions(cx);
                        }
                        return;
                    }
                    "v" => {
                        if let Some(clip) = cx.read_from_clipboard().and_then(|c| c.text()) {
                            let clean = clip.lines().next().unwrap_or("").to_string();
                            self.state.nav_bar.insert_text(&clean);
                            self.state.nav_bar.show_suggestions = true;
                            if matches!(self.state.nav_bar.omnibar_mode, OmnibarMode::Search) {
                                let q = self.state.nav_bar.search_query.clone();
                                self.update_search(q, cx);
                            }
                            self.refresh_omnibar_suggestions(cx);
                        }
                        return;
                    }
                    _ => {}
                }
            }

            match key_lower.as_str() {
                "escape" => {
                    // Files-community Omnibar.Events.cs parity:
                    // If a suggestion was selected via arrow keys, first Escape reverts to user input
                    // and closes the popup; next Escape exits Omnibar focus completely.
                    if self.state.nav_bar.show_suggestions
                        && self.state.nav_bar.selected_suggestion_index.is_some()
                        && !matches!(
                            self.state.nav_bar.omnibar_mode,
                            OmnibarMode::CommandPalette
                        )
                    {
                        self.state.nav_bar.revert_to_user_input();
                        self.state.nav_bar.show_suggestions = false;
                        cx.notify();
                    } else {
                        self.dismiss_omnibar(cx);
                    }
                }
                "enter" | "return" => {
                    self.handle_suggestion_confirm_with_modifiers(ctrl, shift, alt, cx);
                    cx.notify();
                }
                "up" | "arrowup" => {
                    if !self.state.nav_bar.suggestions.is_empty() {
                        self.state.nav_bar.show_suggestions = true;
                        let len = self.state.nav_bar.suggestions.len();
                        match self.state.nav_bar.selected_suggestion_index {
                            Some(0) => {
                                self.state.nav_bar.revert_to_user_input();
                            }
                            Some(cur) => {
                                let next = cur - 1;
                                self.state.nav_bar.selected_suggestion_index = Some(next);
                                self.preview_selected_suggestion(next);
                            }
                            None => {
                                let next = len - 1;
                                self.state.nav_bar.selected_suggestion_index = Some(next);
                                self.preview_selected_suggestion(next);
                            }
                        }
                        cx.notify();
                    }
                }
                "down" | "arrowdown" => {
                    if !self.state.nav_bar.suggestions.is_empty() {
                        self.state.nav_bar.show_suggestions = true;
                        let len = self.state.nav_bar.suggestions.len();
                        match self.state.nav_bar.selected_suggestion_index {
                            Some(cur) if cur + 1 < len => {
                                let next = cur + 1;
                                self.state.nav_bar.selected_suggestion_index = Some(next);
                                self.preview_selected_suggestion(next);
                            }
                            Some(_) => {
                                self.state.nav_bar.revert_to_user_input();
                            }
                            None => {
                                self.state.nav_bar.selected_suggestion_index = Some(0);
                                self.preview_selected_suggestion(0);
                            }
                        }
                        cx.notify();
                    }
                }
                "pageup" => {
                    if self.state.nav_bar.show_suggestions
                        && !self.state.nav_bar.suggestions.is_empty()
                    {
                        let cur = self.state.nav_bar.selected_suggestion_index.unwrap_or(0);
                        let next = cur.saturating_sub(5);
                        self.state.nav_bar.selected_suggestion_index = Some(next);
                        self.preview_selected_suggestion(next);
                        cx.notify();
                    }
                }
                "pagedown" => {
                    if self.state.nav_bar.show_suggestions
                        && !self.state.nav_bar.suggestions.is_empty()
                    {
                        let len = self.state.nav_bar.suggestions.len();
                        let cur = self.state.nav_bar.selected_suggestion_index.unwrap_or(0);
                        let next = (cur + 5).min(len.saturating_sub(1));
                        self.state.nav_bar.selected_suggestion_index = Some(next);
                        self.preview_selected_suggestion(next);
                        cx.notify();
                    }
                }
                "left" | "arrowleft" => {
                    self.state.nav_bar.move_cursor_left(ctrl);
                    cx.notify();
                }
                "right" | "arrowright" => {
                    if self.state.nav_bar.move_cursor_right(ctrl) {
                        if matches!(self.state.nav_bar.omnibar_mode, OmnibarMode::Search) {
                            let q = self.state.nav_bar.search_query.clone();
                            self.update_search(q, cx);
                        }
                        self.refresh_omnibar_suggestions(cx);
                    } else {
                        cx.notify();
                    }
                }
                "home" => {
                    self.state.nav_bar.move_cursor_home();
                    cx.notify();
                }
                "end" => {
                    self.state.nav_bar.move_cursor_end();
                    cx.notify();
                }
                "tab" => {
                    // 1. If a suggestion is highlighted, autocomplete it into the input box
                    if let Some(idx) = self.state.nav_bar.selected_suggestion_index {
                        if let Some(suggestion) = self.state.nav_bar.suggestions.get(idx).cloned() {
                            match suggestion.kind {
                                SuggestionKind::Path { path } => {
                                    let mut p_str = path.to_string_lossy().to_string();
                                    if path.as_path().is_dir()
                                        && !p_str.ends_with('\\')
                                        && !p_str.ends_with('/')
                                    {
                                        p_str.push(std::path::MAIN_SEPARATOR);
                                    }
                                    self.state.nav_bar.set_active_text(p_str, false);
                                    self.state.nav_bar.selected_suggestion_index = None;
                                    self.refresh_omnibar_suggestions(cx);
                                }
                                SuggestionKind::SearchHistory { query } => {
                                    self.state.nav_bar.set_active_text(query.clone(), false);
                                    self.state.nav_bar.selected_suggestion_index = None;
                                    self.update_search(query, cx);
                                    self.refresh_omnibar_suggestions(cx);
                                }
                                SuggestionKind::AqsFilter { token } => {
                                    self.apply_aqs_filter_suggestion(&token, cx);
                                }
                                SuggestionKind::SearchItem { path, .. } => {
                                    let name = path
                                        .as_path()
                                        .file_name()
                                        .map(|n| n.to_string_lossy().to_string())
                                        .unwrap_or_default();
                                    self.state.nav_bar.set_active_text(name.clone(), false);
                                    self.update_search(name, cx);
                                    self.refresh_omnibar_suggestions(cx);
                                }
                                SuggestionKind::Command { .. } => {
                                    self.state
                                        .nav_bar
                                        .set_active_text(suggestion.label, false);
                                    self.refresh_omnibar_suggestions(cx);
                                }
                            }
                            return;
                        }
                    }
                    // 2. Otherwise accept FilePilot inline predictive ghost completion
                    if self.state.nav_bar.accept_ghost_prediction() {
                        if matches!(self.state.nav_bar.omnibar_mode, OmnibarMode::Search) {
                            let q = self.state.nav_bar.search_query.clone();
                            self.update_search(q, cx);
                        }
                        self.refresh_omnibar_suggestions(cx);
                        return;
                    }
                    // 3. Otherwise select the first suggestion in the dropdown
                    if !self.state.nav_bar.suggestions.is_empty() {
                        self.state.nav_bar.selected_suggestion_index = Some(0);
                        self.preview_selected_suggestion(0);
                        cx.notify();
                    }
                }
                "backspace" => {
                    self.state.nav_bar.backspace(ctrl);
                    self.state.nav_bar.selected_suggestion_index = None;
                    if matches!(self.state.nav_bar.omnibar_mode, OmnibarMode::Search) {
                        let query = self.state.nav_bar.search_query.clone();
                        self.update_search(query, cx);
                    }
                    self.refresh_omnibar_suggestions(cx);
                }
                "delete" => {
                    // Shift+Delete removes a highlighted SearchHistory item
                    if shift {
                        if let Some(idx) = self.state.nav_bar.selected_suggestion_index {
                            if let Some(SuggestionKind::SearchHistory { query }) =
                                self.state.nav_bar.suggestions.get(idx).map(|s| s.kind.clone())
                            {
                                self.state.nav_bar.remove_search_history_item(&query);
                                self.refresh_omnibar_suggestions(cx);
                                return;
                            }
                        }
                    }
                    self.state.nav_bar.delete_forward(ctrl);
                    self.state.nav_bar.selected_suggestion_index = None;
                    if matches!(self.state.nav_bar.omnibar_mode, OmnibarMode::Search) {
                        let query = self.state.nav_bar.search_query.clone();
                        self.update_search(query, cx);
                    }
                    self.refresh_omnibar_suggestions(cx);
                }
                "space" => {
                    self.state.nav_bar.insert_text(" ");
                    self.state.nav_bar.selected_suggestion_index = None;
                    if matches!(self.state.nav_bar.omnibar_mode, OmnibarMode::Search) {
                        let query = self.state.nav_bar.search_query.clone();
                        self.update_search(query, cx);
                    }
                    self.refresh_omnibar_suggestions(cx);
                }
                _ => {
                    if !ctrl && !alt && !keystroke.modifiers.platform {
                        let typed_str = keystroke.key_char.as_deref().unwrap_or(key);
                        if let Some(mut ch) = typed_str.chars().next() {
                            if shift && ch.is_ascii_lowercase() {
                                ch = ch.to_ascii_uppercase();
                            }
                            if !ch.is_control() && (typed_str.chars().count() == 1 || key.chars().count() == 1) {
                                let insert_str = ch.to_string();
                                // Prefix mode switching: typing `>` or `?` at start of Path Mode switches to
                                // Command Palette or Search Mode automatically!
                                if matches!(self.state.nav_bar.omnibar_mode, OmnibarMode::Path)
                                    && (self.state.nav_bar.selection_all
                                        || self.state.nav_bar.edit_text.is_empty())
                                {
                                    if insert_str == ">" {
                                        self.activate_omnibar_command_mode(cx);
                                        return;
                                    } else if insert_str == "?" {
                                        self.activate_omnibar_search_mode(cx);
                                        return;
                                    }
                                }

                                self.state.nav_bar.insert_text(&insert_str);
                                self.state.nav_bar.selected_suggestion_index = None;
                                self.state.nav_bar.show_suggestions = true;
                                if matches!(self.state.nav_bar.omnibar_mode, OmnibarMode::Search) {
                                    let query = self.state.nav_bar.search_query.clone();
                                    self.update_search(query, cx);
                                }
                                self.refresh_omnibar_suggestions(cx);
                            }
                        }
                    }
                }
            }
            return;
        }

        // Intercept Key & Mouse Combination Recorder when active in Settings > Actions
        if self.state.settings_ui.recorder.is_some() {
            let key = keystroke.key.as_str();
            if !keystroke.modifiers.modified() && key.eq_ignore_ascii_case("escape") {
                self.state.settings_ui.recorder = None;
                cx.notify();
                return;
            }
            if !keystroke.modifiers.modified()
                && (key.eq_ignore_ascii_case("enter") || key.eq_ignore_ascii_case("return"))
            {
                if self
                    .state
                    .settings_ui
                    .recorder
                    .as_ref()
                    .and_then(|r| r.recorded_combo.as_ref())
                    .is_some()
                {
                    self.commit_keybinding_recorder(cx);
                    return;
                }
            }
            // Ignore bare modifier key presses
            let k_lower = key.to_ascii_lowercase();
            if !matches!(
                k_lower.as_str(),
                "control" | "ctrl" | "shift" | "alt" | "win" | "super" | "meta" | "platform"
            ) && !key_combo.is_empty()
            {
                self.record_candidate_binding(key_combo, cx);
            }
            return;
        }

        // Intercept Settings sidebar search, Actions filter search, or New Tag name input
        if matches!(
            self.state.main_view.target,
            crate::ui::views::main_view::ViewTarget::Settings
        ) && (self.state.settings_ui.search_focused
            || self.state.settings_ui.actions_search_focused
            || self.state.settings_ui.is_creating_tag)
        {
            let key = keystroke.key.as_str();
            let k_lower = key.to_ascii_lowercase();
            if k_lower == "escape" {
                if self.state.settings_ui.is_creating_tag {
                    self.state.settings_ui.is_creating_tag = false;
                } else if self.state.settings_ui.actions_search_focused {
                    self.state.settings_ui.actions_search_focused = false;
                    self.state.settings_ui.actions_search_query.clear();
                } else {
                    self.state.settings_ui.search_focused = false;
                    self.state.settings_ui.search_query.clear();
                }
                cx.notify();
                return;
            }
            if k_lower == "enter" || k_lower == "return" {
                if self.state.settings_ui.is_creating_tag {
                    self.commit_create_tag(cx);
                } else {
                    self.state.settings_ui.search_focused = false;
                    self.state.settings_ui.actions_search_focused = false;
                    cx.notify();
                }
                return;
            }
            let target_buf = if self.state.settings_ui.is_creating_tag {
                &mut self.state.settings_ui.new_tag_name
            } else if self.state.settings_ui.actions_search_focused {
                &mut self.state.settings_ui.actions_search_query
            } else {
                &mut self.state.settings_ui.search_query
            };
            if k_lower == "backspace" {
                target_buf.pop();
                cx.notify();
                return;
            }
            if k_lower == "space" {
                target_buf.push(' ');
                cx.notify();
                return;
            }
            if !keystroke.modifiers.control
                && !keystroke.modifiers.alt
                && !keystroke.modifiers.platform
                && key.chars().count() == 1
            {
                target_buf.push_str(key);
                cx.notify();
                return;
            }
        }

        // Intercept CustomProperties dialog input typing & keyboard shortcuts
        if matches!(
            self.state.active_dialog,
            Some(DialogType::CustomProperties { .. })
        ) {
            let key = keystroke.key.as_str();
            let is_ctrl = keystroke.modifiers.control || keystroke.modifiers.platform;

            if key == "escape" || key == "Escape" {
                let mut had_focus = false;
                if let Some(DialogType::CustomProperties { ref mut state }) =
                    self.state.active_dialog
                {
                    if state.focused_input.is_some() {
                        state.focused_input = None;
                        had_focus = true;
                    }
                }
                if had_focus {
                    cx.notify();
                } else {
                    self.close_dialog(cx);
                }
                return;
            }

            if key == "enter" || key == "return" || key == "Enter" || key == "Return" {
                let in_hash_compare = matches!(
                    self.state.active_dialog,
                    Some(DialogType::CustomProperties { ref state })
                        if state.focused_input
                            == Some(crate::ui::dialog_services::custom_properties_dialog::PropertiesInputField::HashCompare)
                );
                if !in_hash_compare {
                    self.save_and_close_properties_dialog(cx);
                }
                return;
            }

            if let Some(DialogType::CustomProperties { ref mut state }) =
                self.state.active_dialog
            {
                if let Some(field) = state.focused_input {
                    use crate::ui::dialog_services::custom_properties_dialog::PropertiesInputField;

                    let buf: &mut String = match field {
                        PropertiesInputField::ItemName => &mut state.edited_name,
                        PropertiesInputField::HashCompare => &mut state.hash_compare_input,
                        PropertiesInputField::ShortcutTarget => &mut state.shortcut.target_path,
                        PropertiesInputField::ShortcutArgs => &mut state.shortcut.arguments,
                        PropertiesInputField::ShortcutWorkingDir => &mut state.shortcut.working_dir,
                        PropertiesInputField::ShortcutComment => &mut state.shortcut.comment,
                        PropertiesInputField::CustomIconPath => {
                            state
                                .customization
                                .folder_icon_path
                                .get_or_insert_with(String::new)
                        }
                    };

                    if is_ctrl {
                        match key {
                            "c" | "C" => {
                                if !buf.is_empty() {
                                    cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                                        buf.clone(),
                                    ));
                                }
                            }
                            "v" | "V" => {
                                if let Some(clip) =
                                    cx.read_from_clipboard().and_then(|c| c.text())
                                {
                                    let clean: String = clip
                                        .chars()
                                        .filter(|c| !['\r', '\n'].contains(c))
                                        .collect();
                                    buf.push_str(&clean);
                                    cx.notify();
                                }
                            }
                            "u" | "U" => {
                                buf.clear();
                                cx.notify();
                            }
                            _ => {}
                        }
                        return;
                    }

                    match key {
                        "backspace" | "Backspace" => {
                            buf.pop();
                            cx.notify();
                            return;
                        }
                        "space" | "Space" => {
                            buf.push(' ');
                            cx.notify();
                            return;
                        }
                        _ => {
                            let ch_str = keystroke.key_char.as_deref().unwrap_or(key);
                            if !keystroke.modifiers.control
                                && !keystroke.modifiers.alt
                                && !keystroke.modifiers.platform
                                && ch_str.chars().count() == 1
                            {
                                let ch = ch_str.chars().next().unwrap_or('\0');
                                if !ch.is_control() {
                                    if field == PropertiesInputField::ItemName
                                        && ['/', '\\', ':', '*', '?', '"', '<', '>', '|']
                                            .contains(&ch)
                                    {
                                        return;
                                    }
                                    buf.push(ch);
                                    cx.notify();
                                }
                                return;
                            }
                        }
                    }
                }
            }
        }

        // Intercept AddItem dialog input typing
        if let Some(DialogType::AddItem {
            ref mut name,
            ref mut selected_all,
            ..
        }) = self.state.active_dialog
        {
            let key = keystroke.key.as_str();
            let is_ctrl = keystroke.modifiers.control || keystroke.modifiers.platform;

            if is_ctrl {
                match key {
                    "a" | "A" => {
                        *selected_all = true;
                        cx.notify();
                    }
                    "c" | "C" => {
                        if !name.is_empty() {
                            cx.write_to_clipboard(gpui::ClipboardItem::new_string(name.clone()));
                        }
                    }
                    "x" | "X" => {
                        if *selected_all && !name.is_empty() {
                            cx.write_to_clipboard(gpui::ClipboardItem::new_string(name.clone()));
                            name.clear();
                            *selected_all = false;
                            cx.notify();
                        }
                    }
                    "v" | "V" => {
                        if let Some(clip) = cx.read_from_clipboard().and_then(|c| c.text()) {
                            let clean: String = clip
                                .chars()
                                .filter(|c| !['\r', '\n', '/', '\\', ':', '*', '?', '"', '<', '>', '|'].contains(c))
                                .collect();
                            if !clean.is_empty() {
                                if *selected_all {
                                    *name = clean;
                                    *selected_all = false;
                                } else {
                                    name.push_str(&clean);
                                }
                                cx.notify();
                            }
                        }
                    }
                    _ => {}
                }
                return;
            }

            match key {
                "escape" | "Escape" => {
                    self.close_dialog(cx);
                }
                "enter" | "return" | "Enter" | "Return" => {
                    self.confirm_add_item(cx);
                }
                "backspace" | "Backspace" => {
                    if *selected_all {
                        name.clear();
                        *selected_all = false;
                    } else {
                        name.pop();
                    }
                    cx.notify();
                }
                "delete" | "Delete" => {
                    if *selected_all {
                        name.clear();
                        *selected_all = false;
                        cx.notify();
                    }
                }
                "left" | "Left" | "right" | "Right" | "home" | "Home" | "end" | "End" => {
                    *selected_all = false;
                    cx.notify();
                }
                "space" | "Space" => {
                    if *selected_all {
                        *name = " ".to_string();
                        *selected_all = false;
                    } else {
                        name.push(' ');
                    }
                    cx.notify();
                }
                _ => {
                    if !keystroke.modifiers.control
                        && !keystroke.modifiers.alt
                        && !keystroke.modifiers.platform
                        && keystroke.key.chars().count() == 1
                    {
                        if *selected_all {
                            *name = keystroke.key.clone();
                            *selected_all = false;
                        } else {
                            name.push_str(&keystroke.key);
                        }
                        cx.notify();
                    }
                }
            }
            return;
        }

        // Intercept inline renaming input typing (in either pane) - Zed inspired text editing
        if self.state.active_view().renaming_index.is_some() {
            let key = keystroke.key.as_str();
            let is_ctrl = keystroke.modifiers.control || keystroke.modifiers.platform;

            if is_ctrl {
                match key {
                    "a" | "A" => {
                        let view = self.state.active_view_mut();
                        view.rename_selected_all = true;
                        view.rename_cursor_pos = view.rename_text.chars().count();
                        cx.notify();
                    }
                    "c" | "C" => {
                        let view = self.state.active_view();
                        if view.rename_selected_all && !view.rename_text.is_empty() {
                            cx.write_to_clipboard(gpui::ClipboardItem::new_string(view.rename_text.clone()));
                        }
                    }
                    "x" | "X" => {
                        let view = self.state.active_view_mut();
                        if view.rename_selected_all && !view.rename_text.is_empty() {
                            cx.write_to_clipboard(gpui::ClipboardItem::new_string(view.rename_text.clone()));
                            view.rename_text.clear();
                            view.rename_selected_all = false;
                            view.rename_cursor_pos = 0;
                            cx.notify();
                        }
                    }
                    "v" | "V" => {
                        if let Some(clip) = cx.read_from_clipboard().and_then(|c| c.text()) {
                            let clean: String = clip
                                .chars()
                                .filter(|c| !['\r', '\n', '/', '\\', ':', '*', '?', '"', '<', '>', '|'].contains(c))
                                .collect();
                            if !clean.is_empty() {
                                let view = self.state.active_view_mut();
                                if view.rename_selected_all {
                                    view.rename_text = clean.clone();
                                    view.rename_selected_all = false;
                                    view.rename_cursor_pos = clean.chars().count();
                                } else {
                                    let mut chars: Vec<char> = view.rename_text.chars().collect();
                                    let pos = view.rename_cursor_pos.min(chars.len());
                                    for (offset, ch) in clean.chars().enumerate() {
                                        chars.insert(pos + offset, ch);
                                    }
                                    view.rename_cursor_pos = pos + clean.chars().count();
                                    view.rename_text = chars.into_iter().collect();
                                }
                                cx.notify();
                            }
                        }
                    }
                    "left" | "Left" => {
                        let view = self.state.active_view_mut();
                        view.rename_selected_all = false;
                        view.rename_cursor_pos = prev_word_boundary(&view.rename_text, view.rename_cursor_pos);
                        cx.notify();
                    }
                    "right" | "Right" => {
                        let view = self.state.active_view_mut();
                        view.rename_selected_all = false;
                        view.rename_cursor_pos = next_word_boundary(&view.rename_text, view.rename_cursor_pos);
                        cx.notify();
                    }
                    "backspace" | "Backspace" => {
                        let view = self.state.active_view_mut();
                        if view.rename_selected_all {
                            view.rename_text.clear();
                            view.rename_selected_all = false;
                            view.rename_cursor_pos = 0;
                        } else {
                            let prev = prev_word_boundary(&view.rename_text, view.rename_cursor_pos);
                            let mut chars: Vec<char> = view.rename_text.chars().collect();
                            let cur = view.rename_cursor_pos.min(chars.len());
                            if prev < cur {
                                chars.drain(prev..cur);
                                view.rename_cursor_pos = prev;
                                view.rename_text = chars.into_iter().collect();
                            }
                        }
                        cx.notify();
                    }
                    "delete" | "Delete" => {
                        let view = self.state.active_view_mut();
                        if view.rename_selected_all {
                            view.rename_text.clear();
                            view.rename_selected_all = false;
                            view.rename_cursor_pos = 0;
                        } else {
                            let next = next_word_boundary(&view.rename_text, view.rename_cursor_pos);
                            let mut chars: Vec<char> = view.rename_text.chars().collect();
                            let cur = view.rename_cursor_pos.min(chars.len());
                            if cur < next {
                                chars.drain(cur..next.min(chars.len()));
                                view.rename_text = chars.into_iter().collect();
                            }
                        }
                        cx.notify();
                    }
                    _ => {}
                }
                return;
            }

            match key {
                "escape" | "Escape" => {
                    self.cancel_rename(cx);
                }
                "enter" | "return" | "Enter" | "Return" => {
                    self.commit_rename(cx);
                }
                "backspace" | "Backspace" => {
                    let view = self.state.active_view_mut();
                    if view.rename_selected_all {
                        view.rename_text.clear();
                        view.rename_selected_all = false;
                        view.rename_cursor_pos = 0;
                    } else if view.rename_cursor_pos > 0 && !view.rename_text.is_empty() {
                        let mut chars: Vec<char> = view.rename_text.chars().collect();
                        let pos = view.rename_cursor_pos.min(chars.len());
                        chars.remove(pos - 1);
                        view.rename_cursor_pos = pos - 1;
                        view.rename_text = chars.into_iter().collect();
                    }
                    cx.notify();
                }
                "delete" | "Delete" => {
                    let view = self.state.active_view_mut();
                    if view.rename_selected_all {
                        view.rename_text.clear();
                        view.rename_selected_all = false;
                        view.rename_cursor_pos = 0;
                    } else {
                        let mut chars: Vec<char> = view.rename_text.chars().collect();
                        let pos = view.rename_cursor_pos.min(chars.len());
                        if pos < chars.len() {
                            chars.remove(pos);
                            view.rename_text = chars.into_iter().collect();
                        }
                    }
                    cx.notify();
                }
                "left" | "Left" => {
                    let view = self.state.active_view_mut();
                    if view.rename_selected_all {
                        view.rename_selected_all = false;
                        view.rename_cursor_pos = 0;
                    } else if view.rename_cursor_pos > 0 {
                        view.rename_cursor_pos -= 1;
                    }
                    cx.notify();
                }
                "right" | "Right" => {
                    let view = self.state.active_view_mut();
                    if view.rename_selected_all {
                        view.rename_selected_all = false;
                        view.rename_cursor_pos = view.rename_text.chars().count();
                    } else {
                        let char_count = view.rename_text.chars().count();
                        if view.rename_cursor_pos < char_count {
                            view.rename_cursor_pos += 1;
                        }
                    }
                    cx.notify();
                }
                "up" | "Up" => {
                    let view = self.state.active_view_mut();
                    if view.rename_selected_all {
                        view.rename_selected_all = false;
                        view.rename_cursor_pos = 0;
                    } else {
                        let cpl = view.rename_chars_per_line;
                        view.rename_cursor_pos = crate::ui::views::helpers::rename_cursor_move_vertical(
                            &view.rename_text,
                            view.rename_cursor_pos,
                            true,
                            cpl,
                        );
                    }
                    cx.notify();
                }
                "down" | "Down" => {
                    let view = self.state.active_view_mut();
                    if view.rename_selected_all {
                        view.rename_selected_all = false;
                        view.rename_cursor_pos = view.rename_text.chars().count();
                    } else {
                        let cpl = view.rename_chars_per_line;
                        view.rename_cursor_pos = crate::ui::views::helpers::rename_cursor_move_vertical(
                            &view.rename_text,
                            view.rename_cursor_pos,
                            false,
                            cpl,
                        );
                    }
                    cx.notify();
                }
                "home" | "Home" => {
                    let view = self.state.active_view_mut();
                    view.rename_selected_all = false;
                    view.rename_cursor_pos = 0;
                    cx.notify();
                }
                "end" | "End" => {
                    let view = self.state.active_view_mut();
                    view.rename_selected_all = false;
                    view.rename_cursor_pos = view.rename_text.chars().count();
                    cx.notify();
                }
                "space" | "Space" => {
                    let view = self.state.active_view_mut();
                    if view.rename_selected_all {
                        view.rename_text = " ".to_string();
                        view.rename_selected_all = false;
                        view.rename_cursor_pos = 1;
                    } else {
                        let mut chars: Vec<char> = view.rename_text.chars().collect();
                        let pos = view.rename_cursor_pos.min(chars.len());
                        chars.insert(pos, ' ');
                        view.rename_cursor_pos = pos + 1;
                        view.rename_text = chars.into_iter().collect();
                    }
                    cx.notify();
                }
                _ => {
                    if !keystroke.modifiers.control
                        && !keystroke.modifiers.alt
                        && !keystroke.modifiers.platform
                    {
                        // In GPUI, keystroke.key_char contains the typed character (accounting for Shift, CapsLock, layout)
                        let typed_str = keystroke.key_char.as_deref().unwrap_or(keystroke.key.as_str());
                        if let Some(mut ch) = typed_str.chars().next() {
                            if keystroke.modifiers.shift && ch.is_ascii_lowercase() {
                                ch = ch.to_ascii_uppercase();
                            }
                            // Filter control codes and Windows invalid filename characters: \ / : * ? " < > |
                            if !ch.is_control()
                                && !['\r', '\n', '/', '\\', ':', '*', '?', '"', '<', '>', '|'].contains(&ch)
                                && (typed_str.chars().count() == 1 || keystroke.key.chars().count() == 1)
                            {
                                let view = self.state.active_view_mut();
                                if view.rename_selected_all {
                                    view.rename_text = ch.to_string();
                                    view.rename_selected_all = false;
                                    view.rename_cursor_pos = 1;
                                } else {
                                    let mut chars: Vec<char> = view.rename_text.chars().collect();
                                    let pos = view.rename_cursor_pos.min(chars.len());
                                    chars.insert(pos, ch);
                                    view.rename_cursor_pos = pos + 1;
                                    view.rename_text = chars.into_iter().collect();
                                }
                                cx.notify();
                            }
                        }
                    }
                }
            }
            return;
        }

        // Files-community parity shortcuts (address bar, search, zoom, view modes, pane switch)
        let key_str = keystroke.key.as_str();

        // Alt+D or F4 -> Focus address bar / path edit
        if (keystroke.modifiers.alt && !keystroke.modifiers.control && key_str.eq_ignore_ascii_case("d"))
            || (!keystroke.modifiers.modified() && key_str.eq_ignore_ascii_case("f4"))
        {
            self.dispatch_action(crate::core::action_registry::ActionKind::EditPath, cx);
            cx.notify();
            return;
        }

        // Ctrl+E or F3 -> Search
        if (keystroke.modifiers.control && !keystroke.modifiers.alt && !keystroke.modifiers.shift && key_str.eq_ignore_ascii_case("e"))
            || (!keystroke.modifiers.modified() && key_str.eq_ignore_ascii_case("f3"))
        {
            self.dispatch_action(crate::core::action_registry::ActionKind::Search, cx);
            cx.notify();
            return;
        }

        // Ctrl+R -> Refresh, Ctrl+` -> Open terminal, Ctrl+Plus/Minus/0 -> Zoom
        if keystroke.modifiers.control
            && !keystroke.modifiers.alt
            && !keystroke.modifiers.platform
            && !keystroke.modifiers.shift
        {
            match key_str {
                "r" | "R" => {
                    self.refresh(cx);
                    return;
                }
                "`" => {
                    self.open_terminal_at_current(cx);
                    return;
                }
                "=" | "+" => {
                    let z = (self.state.active_view().zoom_level + 0.1).min(1.0);
                    self.set_zoom_level(z, cx);
                    return;
                }
                "-" | "_" => {
                    let z = (self.state.active_view().zoom_level - 0.1).max(0.0);
                    self.set_zoom_level(z, cx);
                    return;
                }
                "0" => {
                    self.set_zoom_level(0.5, cx);
                    return;
                }
                "space" | "Space" => {
                    // Ctrl+Space: toggle selection of the current focused item without clearing others
                    let pane_focus = self.state.active_pane_focus();
                    let cur = self
                        .state
                        .pane_view(pane_focus)
                        .last_clicked_index
                        .unwrap_or(0);
                    if cur < self.state.pane_view(pane_focus).items.len() {
                        self.select_item_in_pane(pane_focus, cur, false, true, cx);
                    }
                    return;
                }
                _ => {}
            }
        }

        // Ctrl+Shift+1..5 -> View mode switching (Details, List, Cards, Grid, Columns)
        if keystroke.modifiers.control && keystroke.modifiers.shift && !keystroke.modifiers.alt {
            match key_str {
                "1" | "!" => {
                    self.set_view_mode(ViewMode::Details, cx);
                    return;
                }
                "2" | "@" => {
                    self.set_view_mode(ViewMode::List, cx);
                    return;
                }
                "3" | "#" => {
                    self.set_view_mode(ViewMode::Cards, cx);
                    return;
                }
                "4" | "$" => {
                    self.set_view_mode(ViewMode::Grid, cx);
                    return;
                }
                "5" | "%" => {
                    self.set_view_mode(ViewMode::Columns, cx);
                    return;
                }
                _ => {}
            }
        }

        // Tab / F6: switch active pane focus when in Dual-Pane mode
        if !keystroke.modifiers.control
            && !keystroke.modifiers.alt
            && !keystroke.modifiers.platform
            && (key_str.eq_ignore_ascii_case("tab") || key_str.eq_ignore_ascii_case("f6"))
        {
            if self.state.main_view.pane_type == crate::ui::views::main_view::PaneType::Dual {
                self.switch_pane_focus(cx);
                return;
            }
        }

        // Enter / Return variants (Alt+Enter = Properties, Ctrl+Enter = Open folder in new tab,
        // Shift+Enter = Open folder in new window, Enter = Open selected item)
        if key_str.eq_ignore_ascii_case("enter") || key_str.eq_ignore_ascii_case("return") {
            let pane_focus = self.state.active_pane_focus();
            if keystroke.modifiers.alt {
                self.open_properties_for_selected(cx);
                return;
            }
            if let Some(&idx) = self.state.pane_view(pane_focus).selected_indices.iter().next() {
                if let Some(item) = self.state.pane_view(pane_focus).items.get(idx).cloned() {
                    if keystroke.modifiers.control && matches!(item.file_type, FileType::Directory) {
                        self.open_in_new_tab(item.path, cx);
                        return;
                    }
                    if keystroke.modifiers.shift && matches!(item.file_type, FileType::Directory) {
                        self.open_in_new_window(item.path, cx);
                        return;
                    }
                }
                self.open_item_in_pane(pane_focus, idx, cx);
            }
            return;
        }

        // Backspace: navigate up to parent folder
        if !keystroke.modifiers.modified() && key_str.eq_ignore_ascii_case("backspace") {
            self.navigate_up(cx);
            return;
        }

        // 2D / View-Mode-Aware Keyboard Navigation (Arrows, Home, End, PageUp, PageDown, Space)
        if !keystroke.modifiers.control && !keystroke.modifiers.alt && !keystroke.modifiers.platform {
            if self.handle_view_navigation_key(key_str, keystroke.modifiers.shift, cx) {
                return;
            }
        }

        // Action Registry lookup
        if let Some(action) = self.state.action_registry.lookup(&key_combo) {
            self.dispatch_action(action, cx);
            return;
        }

        // Type-ahead / incremental file selection when typing printable characters in the file view
        if !keystroke.modifiers.control
            && !keystroke.modifiers.alt
            && !keystroke.modifiers.platform
            && key_str.chars().count() == 1
        {
            let ch = key_str.chars().next().unwrap();
            if !ch.is_control() {
                self.handle_type_to_select(ch, cx);
            }
        }
    }

    /// View-mode-aware keyboard navigation for Details, Grid, List, and Columns views.
    fn handle_view_navigation_key(
        &mut self,
        key: &str,
        shift: bool,
        cx: &mut Context<Self>,
    ) -> bool {
        let pane_focus = self.state.active_pane_focus();
        let len = self.state.pane_view(pane_focus).items.len();
        let view_mode = self.state.pane_view(pane_focus).view_mode.clone();
        let zoom_level = self.state.pane_view(pane_focus).zoom_level;

        // Space opens/closes the big Peek Preview Modal (Shift+Space toggles selection)
        if key.eq_ignore_ascii_case("space") {
            if shift {
                if len > 0 {
                    let current = self
                        .state
                        .pane_view(pane_focus)
                        .last_clicked_index
                        .unwrap_or(0)
                        .min(len - 1);
                    self.select_item_in_pane(pane_focus, current, shift, true, cx);
                    self.scroll_item_into_view(pane_focus, current);
                }
            } else {
                self.toggle_peek_modal(cx);
            }
            return true;
        }

        // When Spacebar Peek Modal is open, Left/Up and Right/Down cycle through items live
        if self.state.main_view.show_peek_modal && !shift {
            if matches!(
                key.to_ascii_lowercase().as_str(),
                "left" | "arrowleft" | "up" | "arrowup"
            ) {
                self.peek_navigate_prev(cx);
                return true;
            }
            if matches!(
                key.to_ascii_lowercase().as_str(),
                "right" | "arrowright" | "down" | "arrowdown"
            ) {
                self.peek_navigate_next(cx);
                return true;
            }
        }

        // In Columns (Miller) view, Left/Right arrows navigate up/into folders directly
        if view_mode == ViewMode::Columns && !shift {
            if key.eq_ignore_ascii_case("left") || key.eq_ignore_ascii_case("arrowleft") {
                self.navigate_up(cx);
                return true;
            }
            if key.eq_ignore_ascii_case("right") || key.eq_ignore_ascii_case("arrowright") {
                if let Some(&idx) = self.state.pane_view(pane_focus).selected_indices.iter().next()
                {
                    if let Some(item) = self.state.pane_view(pane_focus).items.get(idx) {
                        if matches!(item.file_type, FileType::Directory) {
                            self.open_item_in_pane(pane_focus, idx, cx);
                            return true;
                        }
                    }
                }
            }
        }

        if len == 0 {
            return false;
        }

        // Compute pane width and viewport height for 2D Grid/List step calculation
        let win_w = f32::from(self.state.last_window_size.width).max(800.0);
        let win_h = f32::from(self.state.last_window_size.height).max(600.0);
        let sidebar_w = if self.state.sidebar_visible {
            self.state.sidebar.width
        } else {
            0.0
        };
        let mut pane_w = (win_w - sidebar_w).max(240.0);
        if self.state.main_view.pane_type == crate::ui::views::main_view::PaneType::Dual {
            let ratio = self.state.main_view.dual_pane_ratio.clamp(0.2, 0.8);
            pane_w = if pane_focus == crate::core::navigation_manager::PaneFocus::Primary {
                pane_w * ratio
            } else {
                pane_w * (1.0 - ratio)
            };
        }
        let viewport_h = (win_h - 170.0).max(160.0);

        let visual_items = self.state.pane_view(pane_focus).visual_order_items();
        let vis_len = visual_items.len();
        if vis_len == 0 {
            return false;
        }

        let rows = self.state.pane_view(pane_focus).visual_rows(pane_w);
        let grid_cols = {
            let cols = MainViewState::grid_columns_for_width(pane_w, zoom_level);
            MainViewState::grid_layout_params_for_width_pub(pane_w, zoom_level, cols)
                .items_per_row
                .max(1)
        };
        let page_row_step = match view_mode {
            ViewMode::Grid => {
                let params =
                    MainViewState::grid_layout_params_for_width_pub(pane_w, zoom_level, grid_cols);
                ((viewport_h / params.row_height).floor() as usize).max(1)
            }
            ViewMode::Cards => 6,
            ViewMode::List => 10,
            _ => ((viewport_h / 32.0).floor() as usize).max(1),
        };

        let had_selection = !self.state.pane_view(pane_focus).selected_indices.is_empty();
        let current_item_idx = self
            .state
            .pane_view(pane_focus)
            .last_clicked_index
            .or_else(|| self.state.pane_view(pane_focus).selected_indices.iter().next().copied())
            .unwrap_or(visual_items[0]);
        let cur_vis_pos = visual_items
            .iter()
            .position(|&i| i == current_item_idx)
            .unwrap_or(0);
        let (cur_row, cur_col) = rows
            .iter()
            .enumerate()
            .find_map(|(r_idx, r)| {
                r.iter()
                    .position(|&i| i == current_item_idx)
                    .map(|c_idx| (r_idx, c_idx))
            })
            .unwrap_or((0, 0));

        let target_idx: Option<usize> = match key.to_ascii_lowercase().as_str() {
            "down" | "arrowdown" => {
                if !had_selection || rows.is_empty() {
                    Some(visual_items[0])
                } else {
                    let next_r = (cur_row + 1).min(rows.len() - 1);
                    let row = &rows[next_r];
                    Some(row[cur_col.min(row.len().saturating_sub(1))])
                }
            }
            "up" | "arrowup" => {
                if !had_selection || rows.is_empty() {
                    Some(visual_items[0])
                } else {
                    let prev_r = cur_row.saturating_sub(1);
                    let row = &rows[prev_r];
                    Some(row[cur_col.min(row.len().saturating_sub(1))])
                }
            }
            "right" | "arrowright" => {
                if !had_selection {
                    Some(visual_items[0])
                } else {
                    Some(visual_items[(cur_vis_pos + 1).min(vis_len - 1)])
                }
            }
            "left" | "arrowleft" => {
                if !had_selection {
                    Some(visual_items[0])
                } else {
                    Some(visual_items[cur_vis_pos.saturating_sub(1)])
                }
            }
            "home" => Some(visual_items[0]),
            "end" => Some(visual_items[vis_len - 1]),
            "pagedown" => {
                if rows.is_empty() {
                    Some(visual_items[vis_len - 1])
                } else {
                    let next_r = (cur_row + page_row_step).min(rows.len() - 1);
                    let row = &rows[next_r];
                    Some(row[cur_col.min(row.len().saturating_sub(1))])
                }
            }
            "pageup" => {
                if rows.is_empty() {
                    Some(visual_items[0])
                } else {
                    let prev_r = cur_row.saturating_sub(page_row_step);
                    let row = &rows[prev_r];
                    Some(row[cur_col.min(row.len().saturating_sub(1))])
                }
            }
            _ => None,
        };

        if let Some(idx) = target_idx {
            self.select_item_in_pane(pane_focus, idx, shift, false, cx);
            self.scroll_item_into_view(pane_focus, idx);
            return true;
        }
        false
    }

    /// Type-ahead incremental file selection in the active pane.
    fn handle_type_to_select(&mut self, ch: char, cx: &mut Context<Self>) {
        let now = std::time::Instant::now();
        let reset = self
            .state
            .type_to_select_last_time
            .map(|t| now.duration_since(t) > std::time::Duration::from_millis(900))
            .unwrap_or(true);
        if reset {
            self.state.type_to_select_buffer.clear();
        }
        self.state.type_to_select_last_time = Some(now);
        self.state.type_to_select_buffer.push(ch);

        let query = self.state.type_to_select_buffer.to_lowercase();
        let pane_focus = self.state.active_pane_focus();
        let items = &self.state.pane_view(pane_focus).items;
        // Prefer prefix match; fall back to substring match
        let matched = items
            .iter()
            .position(|item| item.name.to_lowercase().starts_with(&query))
            .or_else(|| {
                items
                    .iter()
                    .position(|item| item.name.to_lowercase().contains(&query))
            });
        if let Some(idx) = matched {
            self.select_item_in_pane(pane_focus, idx, false, false, cx);
            self.scroll_item_into_view(pane_focus, idx);
        }
    }

    /// Open the Properties dialog for the currently selected item(s) or current folder (Alt+Enter).
    pub fn open_properties_for_selected(&mut self, cx: &mut Context<Self>) {
        let pane_focus = self.state.active_pane_focus();
        let mut indices: Vec<usize> = self
            .state
            .pane_view(pane_focus)
            .selected_indices
            .iter()
            .copied()
            .collect();
        indices.sort_unstable();

        let paths: Vec<std::path::PathBuf> = if indices.is_empty() {
            vec![
                self.state
                    .pane_view(pane_focus)
                    .primary_path
                    .as_path()
                    .to_path_buf(),
            ]
        } else {
            let items = &self.state.pane_view(pane_focus).items;
            indices
                .into_iter()
                .filter_map(|idx| items.get(idx).map(|it| it.path.as_path().to_path_buf()))
                .collect()
        };

        if !paths.is_empty() {
            self.open_properties_for_paths(paths, cx);
        }
    }

    /// Open Properties for a specific list of file/folder paths, respecting the user's
    /// `use_classic_properties_dialog` preference and supporting both single and multi-item selection.
    pub fn open_properties_for_paths(
        &mut self,
        paths: Vec<std::path::PathBuf>,
        cx: &mut Context<Self>,
    ) {
        if paths.is_empty() {
            return;
        }
        let use_classic = self
            .state
            .settings
            .read(cx)
            .config()
            .preferences
            .use_classic_properties_dialog;

        if use_classic {
            #[cfg(windows)]
            {
                let files_paths: Vec<FilesPath> =
                    paths.iter().map(FilesPath::new).collect();
                let hwnd = self.state.window_hwnd;
                cx.background_executor()
                    .spawn(async move {
                        let _ = crate::platform::windows::shell_service::ShellService::open_multi_properties_with_hwnd(
                            &files_paths,
                            Some(hwnd),
                        );
                    })
                    .detach();
                return;
            }
        }

        self.open_modern_properties_for_paths(paths, cx);
    }

    /// Open the modern redesigned Files-community Properties dialog for one or more paths,
    /// spawning non-blocking background tasks for recursive folder size and streaming file hashes.
    pub fn open_modern_properties_for_paths(
        &mut self,
        paths: Vec<std::path::PathBuf>,
        cx: &mut Context<Self>,
    ) {
        use crate::ui::dialog_services::custom_properties_dialog::{
            CustomPropertiesState, build_details_sections_multi, build_details_sections_single,
            compute_file_hashes, compute_selection_sizes,
        };

        let mut prop_state = CustomPropertiesState::from_paths(paths.clone());

        if paths.len() == 1 && !prop_state.is_dir && !prop_state.is_drive {
            let fp = FilesPath::new(&prop_state.path);
            if let Ok(mut cache) = self.state.icon_cache.lock() {
                prop_state.thumbnail_path = cache.get(&fp).cloned();
            }
            if prop_state.thumbnail_path.is_none() {
                prop_state.thumbnail_path = self.state.thumbnail_cache.get(&fp);
            }
        }

        let need_size = prop_state.calculating_size;
        let need_hashes = prop_state.loading_hashes;
        let primary_path = prop_state.path.clone();

        self.state.active_dialog = Some(DialogType::CustomProperties {
            state: Box::new(prop_state),
        });
        cx.notify();

        if need_size {
            let paths_for_size = paths.clone();
            cx.spawn(async move |this, cx| {
                let (total_size, total_disk, files_count, folders_count) = cx
                    .background_executor()
                    .spawn(async move { compute_selection_sizes(&paths_for_size) })
                    .await;
                let _ = this.update(cx, |this, cx| {
                    if let Some(DialogType::CustomProperties { ref mut state }) =
                        this.state.active_dialog
                    {
                        if state.paths == paths {
                            state.size = total_size;
                            state.size_on_disk = total_disk;
                            state.files_count = files_count;
                            state.folders_count = folders_count;
                            state.calculating_size = false;
                            state.details_sections = if state.is_multi() {
                                build_details_sections_multi(state)
                            } else {
                                build_details_sections_single(state)
                            };
                            cx.notify();
                        }
                    }
                });
            })
            .detach();
        }

        if need_hashes {
            let path_for_hash = primary_path.clone();
            cx.spawn(async move |this, cx| {
                let hashes_res = cx
                    .background_executor()
                    .spawn(async move { compute_file_hashes(&path_for_hash) })
                    .await;
                let _ = this.update(cx, |this, cx| {
                    if let Some(DialogType::CustomProperties { ref mut state }) =
                        this.state.active_dialog
                    {
                        if state.path == primary_path {
                            match hashes_res {
                                Ok(hashes) => state.hashes = hashes,
                                Err(err) => state.error = Some(err),
                            }
                            state.loading_hashes = false;
                            cx.notify();
                        }
                    }
                });
            })
            .detach();
        }
    }

    /// Apply any pending edits in the modern Properties dialog (rename, attributes, shortcut,
    /// folder icon, compatibility flags) and close the dialog.
    pub fn save_and_close_properties_dialog(&mut self, cx: &mut Context<Self>) {
        let apply_res = if let Some(DialogType::CustomProperties { ref state }) =
            self.state.active_dialog
        {
            Some(state.apply_changes())
        } else {
            None
        };

        if let Some(res) = apply_res {
            if let Err(err_msg) = res {
                if let Some(DialogType::CustomProperties { ref mut state }) =
                    self.state.active_dialog
                {
                    state.error = Some(err_msg);
                    cx.notify();
                }
                return;
            }
            self.close_dialog(cx);
            self.refresh(cx);
        }
    }

    /// Switch from the modern Properties dialog directly to the classic Windows Properties dialog
    /// for the same single or multi-item selection.
    pub fn open_classic_properties_from_dialog(&mut self, cx: &mut Context<Self>) {
        let paths_opt = if let Some(DialogType::CustomProperties { ref state }) =
            self.state.active_dialog
        {
            Some(state.paths.clone())
        } else {
            None
        };

        if let Some(paths) = paths_opt {
            self.close_dialog(cx);
            #[cfg(windows)]
            {
                let files_paths: Vec<FilesPath> =
                    paths.iter().map(FilesPath::new).collect();
                let hwnd = self.state.window_hwnd;
                cx.background_executor()
                    .spawn(async move {
                        let _ = crate::platform::windows::shell_service::ShellService::open_multi_properties_with_hwnd(
                            &files_paths,
                            Some(hwnd),
                        );
                    })
                    .detach();
            }
            #[cfg(not(windows))]
            {
                let _ = paths;
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
            crate::core::navigation_manager::PaneLayout::DualHorizontal => "horizontal",
            crate::core::navigation_manager::PaneLayout::DualVertical => "vertical",
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
            let _ = std::process::Command::new("xdg-open")
                .arg(url)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn();
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
        let nav = self.state.navigation.read(cx);
        let existing_index = nav.tabs.tabs.iter().position(|t| {
            matches!(t.target, crate::ui::views::types::ViewTarget::Settings)
        });
        drop(nav);

        if let Some(idx) = existing_index {
            self.switch_tab(idx, cx);
        } else {
            let new_idx = self.state.navigation.update(cx, |nav, _cx| {
                nav.tabs.add_settings_tab();
                nav.tabs.active_tab_index
            });
            self.state.tab_drag.new_tab_anim = Some((new_idx, 0.0));
            self.state.main_view.target = crate::ui::views::types::ViewTarget::Settings;
            self.ensure_active_tab_visible(cx);
            self.save_session(cx);
            cx.notify();
        }
    }

    pub fn new_tab(&mut self, cx: &mut Context<Self>) {
        let current_path = self.state.active_view().primary_path.clone();
        let mode = self.state.active_view().view_mode.clone();
        let zoom = self.state.active_view().zoom_level;
        let new_idx = self.state.navigation.update(cx, |nav, _cx| {
            nav.tabs.add_tab_with_zoom(current_path.clone(), mode, zoom);
            nav.tabs.active_tab_index
        });
        self.state.tab_drag.new_tab_anim = Some((new_idx, 0.0));
        self.ensure_active_tab_visible(cx);
        self.save_session(cx);
        self.load_tab_thumbnails(cx);
        self.navigate_pane(
            current_path,
            crate::core::navigation_manager::PaneFocus::Primary,
            cx,
        );
    }

    /// Open a new tab immediately to the right of `from_index` (Files-community "New tab to the right").
    pub fn new_tab_to_right(&mut self, from_index: usize, cx: &mut Context<Self>) {
        let current_path = self.state.active_view().primary_path.clone();
        let mode = self.state.active_view().view_mode.clone();
        let zoom = self.state.active_view().zoom_level;
        let new_idx = self.state.navigation.update(cx, |nav, _cx| {
            nav.tabs
                .add_tab_after(from_index, current_path.clone(), mode, zoom)
        });
        self.state.tab_drag.new_tab_anim = Some((new_idx, 0.0));
        self.ensure_active_tab_visible(cx);
        self.save_session(cx);
        self.load_tab_thumbnails(cx);
        self.sync_active_tab_to_views(cx);
        cx.notify();
    }

    /// Close only this window if other `files-rs` windows are still open, or quit the app if this is the last window.
    pub fn close_window_or_quit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        use crate::ui::components::tab_interactions::WindowTabRegistry;
        WindowTabRegistry::unregister(self.state.window_id);
        self.stop_all_file_watchers();
        if WindowTabRegistry::open_window_count() == 0 {
            cx.quit();
        } else {
            window.remove_window();
        }
    }

    /// Close tab at `index` with access to `Window` so closing the last tab in a secondary window
    /// closes only that window rather than terminating the whole application.
    pub fn close_tab_with_window(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let tab_count = self.state.navigation.read(cx).tabs.tabs.len();
        if tab_count <= 1 {
            self.close_window_or_quit(window, cx);
            return;
        }
        self.close_tab(index, cx);
    }

    pub fn close_tab(&mut self, index: usize, cx: &mut Context<Self>) {
        let tab_count = self.state.navigation.read(cx).tabs.tabs.len();
        if tab_count <= 1 {
            use crate::ui::components::tab_interactions::WindowTabRegistry;
            WindowTabRegistry::unregister(self.state.window_id);
            if WindowTabRegistry::open_window_count() == 0 {
                cx.quit();
            }
            return;
        }
        self.state.navigation.update(cx, |nav, _cx| {
            nav.tabs.close_tab(index);
        });
        self.clear_tab_hover(cx);
        self.save_session(cx);
        self.clamp_scroll_offset(cx);
        self.ensure_active_tab_visible(cx);
        self.sync_active_tab_to_views(cx);
    }

    pub fn reopen_closed_tab(&mut self, cx: &mut Context<Self>) {
        let reopened_idx = self.state.navigation.update(cx, |nav, _cx| {
            if nav.tabs.reopen_closed_tab() {
                Some(nav.tabs.active_tab_index)
            } else {
                None
            }
        });
        if let Some(new_idx) = reopened_idx {
            self.state.tab_drag.new_tab_anim = Some((new_idx, 0.0));
            self.save_session(cx);
            self.ensure_active_tab_visible(cx);
            self.load_tab_thumbnails(cx);
            self.sync_active_tab_to_views(cx);
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
        self.ensure_active_tab_visible(cx);
        self.sync_active_tab_to_views(cx);
    }

    /// Synchronize the newly active tab's target, primary path, view mode, zoom, and secondary pane.
    pub(crate) fn sync_active_tab_to_views(&mut self, cx: &mut Context<Self>) {
        let (new_path, tab_target, view_mode, zoom, sec_path) = {
            let nav = self.state.navigation.read(cx);
            let tab = nav.tabs.active_tab();
            (
                tab.path.clone(),
                tab.target.clone(),
                tab.view_mode.clone(),
                tab.zoom_level,
                tab.secondary_pane_path.clone(),
            )
        };
        self.state.main_view.target = tab_target.clone();
        self.state.main_view.view_mode = view_mode;
        self.state.main_view.zoom_level = zoom;
        if sec_path.is_some() {
            self.state.main_view.pane_type = crate::ui::views::main_view::PaneType::Dual;
        }
        match tab_target {
            crate::ui::views::types::ViewTarget::Settings => {
                self.state.nav_bar.update_path(FilesPath::from("Settings"));
                cx.notify();
            }
            _ => {
                self.navigate_pane(
                    new_path,
                    crate::core::navigation_manager::PaneFocus::Primary,
                    cx,
                );
                if self.state.main_view.pane_type
                    == crate::ui::views::main_view::PaneType::Dual
                {
                    if let Some(sp) = sec_path {
                        self.navigate_pane(
                            sp,
                            crate::core::navigation_manager::PaneFocus::Secondary,
                            cx,
                        );
                    }
                }
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
        self.ensure_active_tab_visible(cx);
        self.sync_active_tab_to_views(cx);
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
        self.ensure_active_tab_visible(cx);
        self.sync_active_tab_to_views(cx);
    }

    pub fn move_active_tab_left(&mut self, cx: &mut Context<Self>) {
        let win_w = f32::from(self.state.last_window_size.width).max(800.0);
        let (tab_count, pinned_count, from) = {
            let nav = self.state.navigation.read(cx);
            (
                nav.tabs.tabs.len(),
                nav.tabs.tabs.iter().filter(|t| t.pinned).count(),
                nav.tabs.active_tab_index,
            )
        };
        let moved = self
            .state
            .navigation
            .update(cx, |nav, _cx| nav.tabs.move_active_tab_left());
        if moved {
            let metrics = crate::ui::components::tab_bar::compute_strip_metrics(
                win_w,
                tab_count,
                pinned_count,
            );
            self.record_slot_swap_animation(from, from.saturating_sub(1), metrics.tab_w, tab_count);
            self.ensure_active_tab_visible(cx);
            self.save_session(cx);
            cx.notify();
        }
    }

    pub fn move_active_tab_right(&mut self, cx: &mut Context<Self>) {
        let win_w = f32::from(self.state.last_window_size.width).max(800.0);
        let (tab_count, pinned_count, from) = {
            let nav = self.state.navigation.read(cx);
            (
                nav.tabs.tabs.len(),
                nav.tabs.tabs.iter().filter(|t| t.pinned).count(),
                nav.tabs.active_tab_index,
            )
        };
        let moved = self
            .state
            .navigation
            .update(cx, |nav, _cx| nav.tabs.move_active_tab_right());
        if moved {
            let metrics = crate::ui::components::tab_bar::compute_strip_metrics(
                win_w,
                tab_count,
                pinned_count,
            );
            self.record_slot_swap_animation(from, (from + 1).min(tab_count - 1), metrics.tab_w, tab_count);
            self.ensure_active_tab_visible(cx);
            self.save_session(cx);
            cx.notify();
        }
    }

    /// Record smooth horizontal slot-shift offsets when a tab moves from `from_idx` to `to_idx`.
    fn record_slot_swap_animation(
        &mut self,
        from_idx: usize,
        to_idx: usize,
        tab_w: f32,
        tab_count: usize,
    ) {
        if self.state.tab_drag.slot_shift_offsets.len() != tab_count {
            self.state.tab_drag.slot_shift_offsets.resize(tab_count, 0.0);
        }
        if from_idx < to_idx {
            for i in from_idx..to_idx {
                if let Some(slot) = self.state.tab_drag.slot_shift_offsets.get_mut(i) {
                    *slot += tab_w;
                }
            }
        } else if from_idx > to_idx {
            for i in (to_idx + 1)..=from_idx {
                if let Some(slot) = self.state.tab_drag.slot_shift_offsets.get_mut(i) {
                    *slot -= tab_w;
                }
            }
        }
    }

    /// Load folder thumbnails for all open tabs (for tab bar icons).
    #[cfg(windows)]
    pub fn load_tab_thumbnails(&mut self, cx: &mut Context<Self>) {
        use crate::app_state::icons_cache_dir;
        use crate::platform::windows::thumbnail_service::ThumbnailSize;

        let tab_paths: Vec<FilesPath> = {
            let nav = self.state.navigation.read(cx);
            nav.tabs.tabs.iter().map(|t| t.path.clone()).collect()
        };

        let thumbnail_service = self.state.thumbnail_service.clone();
        let tab_cache = self.state.tab_thumbnail_cache.clone();
        let icon_dir = icons_cache_dir();

        cx.foreground_executor()
            .spawn(async move {
                let _com_init = unsafe {
                    windows::Win32::System::Com::CoInitializeEx(
                        None,
                        windows::Win32::System::Com::COINIT_APARTMENTTHREADED,
                    )
                };

                for path in &tab_paths {
                    if let Ok(cache) = tab_cache.lock() {
                        if cache.contains_key(path) {
                            continue;
                        }
                    }

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

                    if let Ok(thumb) =
                        thumbnail_service.extract_thumbnail(path, ThumbnailSize::Small)
                    {
                        use image::{ImageBuffer, Rgba};
                        let w = thumb.width;
                        let h = thumb.height;
                        if w > 0 && h > 0 && thumb.data.len() as u32 >= w * h * 4 {
                            let mut rgba_data = thumb.data.clone();
                            for ch in rgba_data.chunks_exact_mut(4) {
                                ch.swap(0, 2);
                            }
                            if let Some(img) =
                                ImageBuffer::<Rgba<u8>, _>::from_raw(w, h, rgba_data)
                            {
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

    // ---- Dual-Pane (Split View) Operations ----

    /// Ensure the secondary pane is initialized and loaded when entering Dual-Pane mode.
    pub fn sync_secondary_pane_on_split(&mut self, cx: &mut Context<Self>) {
        let primary_path = self.state.main_view.primary_path.clone();
        let existing_sec = self
            .state
            .navigation
            .read(cx)
            .tabs
            .active_tab()
            .secondary_pane_path
            .clone();

        let target_sec = match existing_sec {
            Some(ref p) if !p.as_path().as_os_str().is_empty() => p.clone(),
            _ => NavigationService::get_parent_path(&primary_path).unwrap_or(primary_path),
        };

        self.navigate_pane(
            target_sec,
            crate::core::navigation_manager::PaneFocus::Secondary,
            cx,
        );
    }

    pub fn toggle_dual_pane(&mut self, cx: &mut Context<Self>) {
        self.state.navigation.update(cx, |nav, _cx| {
            nav.toggle_dual_pane();
        });
        let (layout, active_pane) = {
            let nav = self.state.navigation.read(cx);
            (nav.pane_layout, nav.active_pane)
        };
        self.state.main_view.pane_type = match layout {
            PaneLayout::Single => crate::ui::views::main_view::PaneType::Single,
            _ => crate::ui::views::main_view::PaneType::Dual,
        };
        self.state.main_view.active_pane = active_pane;
        if self.state.main_view.pane_type == crate::ui::views::main_view::PaneType::Dual {
            self.sync_secondary_pane_on_split(cx);
            self.focus_pane(crate::core::navigation_manager::PaneFocus::Secondary, cx);
        } else {
            self.focus_pane(crate::core::navigation_manager::PaneFocus::Primary, cx);
        }
        self.save_session(cx);
        cx.notify();
    }

    /// Set the active pane in Dual-Pane mode and sync the navigation bar, sidebar, and status bar.
    pub fn focus_pane(
        &mut self,
        pane_focus: crate::core::navigation_manager::PaneFocus,
        cx: &mut Context<Self>,
    ) {
        let effective = if self.state.main_view.pane_type
            == crate::ui::views::main_view::PaneType::Dual
        {
            pane_focus
        } else {
            crate::core::navigation_manager::PaneFocus::Primary
        };
        let changed = self.state.main_view.active_pane != effective;
        if !changed {
            return;
        }
        self.commit_rename_if_active(cx);
        self.state.main_view.active_pane = effective;
        self.state.secondary_view.active_pane = effective;
        self.state.navigation.update(cx, |nav, _cx| {
            nav.active_pane = effective;
        });

        let pane_path = self.state.pane_view(effective).primary_path.clone();
        self.state.nav_bar.update_path(pane_path.clone());
        self.state.sidebar.set_active(&pane_path);

        let view_ref = self.state.pane_view(effective);
        let total = view_ref.items.len();
        let folders = view_ref
            .items
            .iter()
            .filter(|i| matches!(i.file_type, FileType::Directory))
            .count();
        let files = total - folders;
        self.state.status_bar.sync(total, folders, files);
        self.sync_pane_selection_model(effective, cx);

        let git_path = pane_path.clone();
        let weak = cx.weak_entity();
        let mut async_cx = cx.to_async();
        let bg = cx.background_executor().spawn(async move {
            GitService::get_branch(&git_path)
        });
        cx.foreground_executor()
            .spawn(async move {
                let branch = bg.await;
                weak.update(&mut async_cx, |this, cx| {
                    if this.state.active_pane_focus() == effective {
                        this.state.status_bar.show_git_actions = branch.is_some();
                        this.state.status_bar.git_branch = branch;
                        cx.notify();
                    }
                })
                .ok();
            })
            .detach();

        self.save_session(cx);
        cx.notify();
    }

    /// Switch focus between Primary and Secondary panes (Tab / F6).
    pub fn switch_pane_focus(&mut self, cx: &mut Context<Self>) {
        if self.state.main_view.pane_type != crate::ui::views::main_view::PaneType::Dual {
            return;
        }
        let next = match self.state.main_view.active_pane {
            crate::core::navigation_manager::PaneFocus::Primary => {
                crate::core::navigation_manager::PaneFocus::Secondary
            }
            crate::core::navigation_manager::PaneFocus::Secondary => {
                crate::core::navigation_manager::PaneFocus::Primary
            }
        };
        self.focus_pane(next, cx);
    }

    /// Close one of the panes in Dual-Pane mode. If Primary is closed, Secondary is promoted to Primary.
    pub fn close_dual_pane(
        &mut self,
        closed_pane: crate::core::navigation_manager::PaneFocus,
        cx: &mut Context<Self>,
    ) {
        if self.state.main_view.pane_type != crate::ui::views::main_view::PaneType::Dual {
            return;
        }
        if closed_pane == crate::core::navigation_manager::PaneFocus::Primary {
            let sec_path = self.state.secondary_view.primary_path.clone();
            let sec_items = self.state.secondary_view.items.clone();
            let sec_sel = self.state.secondary_view.selected_indices.clone();
            self.state.main_view.primary_path = sec_path.clone();
            self.state.main_view.items = sec_items.clone();
            self.state.main_view.selected_indices = sec_sel.clone();
            self.state.main_view.bump_items_revision();
            self.state.file_items.update(cx, |m, _| {
                m.items = sec_items;
                m.selected_indices = sec_sel;
            });
            self.state.navigation.update(cx, |nav, _cx| {
                nav.tabs.active_tab_mut().path = sec_path;
            });
        }
        self.stop_file_watcher(true);
        self.state.navigation.update(cx, |nav, _cx| {
            nav.pane_layout = PaneLayout::Single;
            nav.active_pane = crate::core::navigation_manager::PaneFocus::Primary;
            nav.tabs.active_tab_mut().secondary_pane_path = None;
        });
        self.state.main_view.pane_type = crate::ui::views::main_view::PaneType::Single;
        self.state.main_view.secondary_path = None;
        self.focus_pane(crate::core::navigation_manager::PaneFocus::Primary, cx);
        self.save_session(cx);
        cx.notify();
    }

    /// Show a context menu for a tab at the given screen position.
    pub fn show_tab_context_menu(
        &mut self,
        tab_index: usize,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        self.clear_tab_hover(cx);
        let (tab_count, is_pinned, can_reopen_closed) = {
            let nav = self.state.navigation.read(cx);
            let tc = nav.tabs.tabs.len();
            let pinned = nav
                .tabs
                .tabs
                .get(tab_index)
                .map(|t| t.pinned)
                .unwrap_or(false);
            let reopen = !nav.tabs.closed_tabs.is_empty();
            (tc, pinned, reopen)
        };
        let other_windows: Vec<(u64, String)> =
            crate::ui::components::tab_interactions::WindowTabRegistry::other_windows(
                self.state.window_id,
            )
            .into_iter()
            .map(|w| (w.window_id, w.active_tab_title))
            .collect();

        self.state.active_dialog = Some(DialogType::TabContextMenu {
            tab_index,
            position,
            tab_count,
            is_pinned,
            can_reopen_closed,
            other_windows,
        });
        cx.notify();
    }

    /// Toggle the tab actions & all-tabs dropdown menu.
    pub fn toggle_tab_actions_menu(&mut self, cx: &mut Context<Self>) {
        self.clear_tab_hover(cx);
        if matches!(
            self.state.active_dialog,
            Some(DialogType::TabActionsDropdown { .. })
        ) {
            self.state.active_dialog = None;
        } else {
            let position = Point {
                x: px(4.0),
                y: px(40.0),
            };
            let (open_tabs, can_reopen_closed) = {
                let nav = self.state.navigation.read(cx);
                let active_idx = nav.tabs.active_tab_index;
                let tabs = nav
                    .tabs
                    .tabs
                    .iter()
                    .enumerate()
                    .map(|(idx, t)| {
                        let label = crate::ui::components::tab_bar::tab_display_label(t);
                        let path_s = t.path.to_string_lossy().to_string();
                        (idx, label, path_s, idx == active_idx, t.pinned)
                    })
                    .collect();
                (tabs, !nav.tabs.closed_tabs.is_empty())
            };
            let is_dual_pane =
                self.state.main_view.pane_type == crate::ui::views::main_view::PaneType::Dual;
            self.state.active_dialog = Some(DialogType::TabActionsDropdown {
                position,
                open_tabs,
                can_reopen_closed,
                is_dual_pane,
            });
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
                        let _ = ShowWindowAsync(hwnd, SW_RESTORE);
                        self.state.is_maximized = false;
                    } else {
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

    /// Start dragging a tab (records initial cursor position).
    pub fn start_tab_drag(&mut self, tab_index: usize, cx: &mut Context<Self>) {
        let x = self.state.tab_drag.current_x;
        let y = self.state.tab_drag.current_y;
        self.start_tab_drag_at(tab_index, x, y, cx);
    }

    /// Start dragging a tab at explicit coordinates `(x, y)`.
    pub fn start_tab_drag_at(
        &mut self,
        tab_index: usize,
        x: f32,
        y: f32,
        cx: &mut Context<Self>,
    ) {
        self.start_tab_drag_with_offset(tab_index, x, y, 60.0, cx);
    }

    /// Start dragging a tab at explicit coordinates `(x, y)` with `grab_offset_x` relative to the tab's left edge.
    pub fn start_tab_drag_with_offset(
        &mut self,
        tab_index: usize,
        x: f32,
        y: f32,
        grab_offset_x: f32,
        cx: &mut Context<Self>,
    ) {
        self.clear_tab_hover(cx);
        let tab_count = self.state.navigation.read(cx).tabs.tabs.len();
        self.state.tab_drag.dragged_index = Some(tab_index);
        self.state.tab_drag.initial_index = Some(tab_index);
        self.state.tab_drag.drop_target_index = Some(tab_index);
        self.state.tab_drag.grab_offset_x = grab_offset_x;
        self.state.tab_drag.is_dragging = true;
        self.state.tab_drag.drag_active = false;
        self.state.tab_drag.is_dragging_out = false;
        self.state.tab_drag.drag_start_x = x;
        self.state.tab_drag.drag_start_y = y;
        self.state.tab_drag.current_x = x;
        self.state.tab_drag.current_y = y;
        self.state.tab_drag.screen_x = f32::from(self.state.last_window_origin.x) + x;
        self.state.tab_drag.screen_y = f32::from(self.state.last_window_origin.y) + y;
        self.state.tab_drag.drag_start_time = Some(std::time::Instant::now());
        self.state.tab_drag.slot_shift_offsets = vec![0.0; tab_count];
        self.state.tab_drag.cross_window_target = None;
        cx.notify();
    }

    /// Handle pointer movement over tab `tab_index`:
    /// - If dragging a tab, updates tab drag position / live reorder / cross-window target.
    /// - If dragging files/folders, highlights `tab_index` and starts a 500ms hover-to-switch timer.
    /// - Otherwise, starts a 400ms hover timer for the rich Tab Hover Preview Card.
    pub fn on_tab_mouse_move(
        &mut self,
        tab_index: usize,
        tab_left_x: f32,
        x: f32,
        y: f32,
        window_width: f32,
        cx: &mut Context<Self>,
    ) {
        if self.state.tab_drag.is_dragging {
            self.update_tab_drag_with_width(x, y, window_width, cx);
            return;
        }

        // Case 2: User is dragging files/folders from the file view over a tab
        if self.is_dragging() {
            self.state.tab_drag.file_drag_over_new_tab_btn = false;
            if self.state.tab_drag.file_drag_over_tab != Some(tab_index) {
                self.state.tab_drag.file_drag_over_tab = Some(tab_index);
                self.state.tab_drag.hovered_tab_index = Some(tab_index);
                self.state.tab_drag.hovered_tab_since = Some(std::time::Instant::now());
                self.tab_hover_timer_task = None;

                let mut async_app = cx.to_async();
                let task = cx.spawn(
                    move |this_weak: WeakEntity<Self>, _: &mut AsyncApp| async move {
                        async_app
                            .background_executor()
                            .timer(std::time::Duration::from_millis(500))
                            .await;
                        let _ = this_weak.update(&mut async_app, |this, cx| {
                            if this.is_dragging()
                                && this.state.tab_drag.file_drag_over_tab == Some(tab_index)
                            {
                                // Auto-switch to the hovered tab so files can be dropped inside it
                                this.switch_tab(tab_index, cx);
                            }
                        });
                    },
                );
                self.tab_hover_timer_task = Some(task);
                cx.notify();
            }
            return;
        }

        // Case 3: Normal pointer hover over tab -> schedule rich Tab Hover Preview Card
        self.state.tab_drag.hovered_tab_left_x = tab_left_x;
        if self.state.tab_drag.hovered_tab_index != Some(tab_index) {
            self.state.tab_drag.hovered_tab_index = Some(tab_index);
            self.state.tab_drag.hovered_tab_since = Some(std::time::Instant::now());
            self.state.tab_drag.show_tab_hover_card = false;
            self.tab_hover_timer_task = None;

            let mut async_app = cx.to_async();
            let task = cx.spawn(
                move |this_weak: WeakEntity<Self>, _: &mut AsyncApp| async move {
                    async_app
                        .background_executor()
                        .timer(std::time::Duration::from_millis(420))
                        .await;
                    let _ = this_weak.update(&mut async_app, |this, cx| {
                        if !this.state.tab_drag.is_dragging
                            && !this.is_dragging()
                            && this.state.active_dialog.is_none()
                            && this.state.tab_drag.hovered_tab_index == Some(tab_index)
                        {
                            this.state.tab_drag.show_tab_hover_card = true;
                            cx.notify();
                        }
                    });
                },
            );
            self.tab_hover_timer_task = Some(task);
            cx.notify();
        }
    }

    /// Clear tab hover preview card and file-drag-over-tab highlights when the cursor leaves the tabs.
    pub fn clear_tab_hover(&mut self, cx: &mut Context<Self>) {
        let had_hover = self.state.tab_drag.hovered_tab_index.is_some()
            || self.state.tab_drag.show_tab_hover_card
            || self.state.tab_drag.file_drag_over_tab.is_some()
            || self.state.tab_drag.file_drag_over_new_tab_btn;
        self.tab_hover_timer_task = None;
        self.state.tab_drag.hovered_tab_index = None;
        self.state.tab_drag.hovered_tab_since = None;
        self.state.tab_drag.show_tab_hover_card = false;
        self.state.tab_drag.file_drag_over_tab = None;
        self.state.tab_drag.file_drag_over_new_tab_btn = false;
        if had_hover {
            cx.notify();
        }
    }

    /// Drop currently dragged files/folders onto tab `tab_index` (moves/copies them into that tab's directory).
    pub fn drop_files_onto_tab(&mut self, tab_index: usize, cx: &mut Context<Self>) {
        let dest_dir_opt = self
            .state
            .navigation
            .read(cx)
            .tabs
            .tabs
            .get(tab_index)
            .map(|t| t.path.as_path().to_path_buf());
        let indices = self.state.main_view.drag_state.indices.clone();
        let is_copy = self.state.main_view.drag_state.is_copy;
        let sources: Vec<std::path::PathBuf> = indices
            .iter()
            .filter_map(|&idx| self.state.main_view.items.get(idx))
            .map(|it| it.path.as_path().to_path_buf())
            .collect();

        self.state.main_view.drag_end();
        self.clear_tab_hover(cx);

        if let Some(dest_dir) = dest_dir_opt {
            if !sources.is_empty() && dest_dir.is_dir() {
                let progress_engine = self.state.progress_engine.clone();
                let this_weak = cx.weak_entity();
                let mut async_cx = cx.to_async();
                let total = sources.len();
                cx.foreground_executor()
                    .spawn(async move {
                        use crate::core::progress::OpKind;
                        use crate::core::transfer::{TransferOutcome, run_copy_job_rt, run_move_job_rt};
                        let (op_label, kind) = if is_copy {
                            ("Copy", OpKind::Copy)
                        } else {
                            ("Move", OpKind::Move)
                        };
                        let job_id = progress_engine
                            .start_job(format!("{} ({} items)", op_label, total), kind)
                            .await;
                        let outcome = if is_copy {
                            run_copy_job_rt(progress_engine.clone(), job_id, sources, dest_dir).await
                        } else {
                            run_move_job_rt(progress_engine.clone(), job_id, sources, dest_dir).await
                        };
                        match outcome {
                            TransferOutcome::Done => progress_engine.complete_job(job_id).await,
                            TransferOutcome::Cancelled => {}
                            TransferOutcome::Failed(reason) => {
                                progress_engine.fail_job(job_id, reason).await
                            }
                        }
                        let _ = this_weak.update(&mut async_cx, |this, cx| {
                            this.refresh(cx);
                        });
                    })
                    .detach();
            }
        }
        cx.notify();
    }

    /// Drop dragged folder(s) onto the `+` (New Tab) button to open them as new tabs (Files-community parity).
    pub fn drop_files_onto_new_tab_button(&mut self, cx: &mut Context<Self>) {
        let indices = self.state.main_view.drag_state.indices.clone();
        let dirs_to_open: Vec<FilesPath> = indices
            .iter()
            .filter_map(|&idx| self.state.main_view.items.get(idx))
            .filter(|it| matches!(it.file_type, FileType::Directory))
            .map(|it| it.path.clone())
            .collect();

        self.state.main_view.drag_end();
        self.clear_tab_hover(cx);

        for dir_path in dirs_to_open {
            self.open_in_new_tab(dir_path, cx);
        }
        cx.notify();
    }

    /// Clear `external_drop_preview` on any target window we were previously hovering over.
    fn clear_external_drop_preview_on_target(&mut self, cx: &mut Context<Self>) {
        if let Some(prev_target) = self.state.tab_drag.cross_window_target.take() {
            let _ = prev_target.target_weak.update(cx, |target_win, tcx| {
                if target_win.state.tab_drag.external_drop_preview.is_some() {
                    target_win.state.tab_drag.external_drop_preview = None;
                    tcx.notify();
                }
            });
        }
    }

    /// Update tab drag position using the current window width for responsive tab metrics.
    pub fn update_tab_drag(&mut self, x: f32, y: f32, cx: &mut Context<Self>) {
        let win_w = f32::from(self.state.last_window_size.width).max(800.0);
        self.update_tab_drag_with_width(x, y, win_w, cx);
    }

    /// Update tab drag position with explicit `window_width`. Performs live animated tab reordering
    /// inside the strip, edge auto-scroll, and cross-window tab bar hit-testing when dragged out.
    pub fn update_tab_drag_with_width(
        &mut self,
        x: f32,
        y: f32,
        window_width: f32,
        cx: &mut Context<Self>,
    ) {
        if !self.state.tab_drag.is_dragging {
            return;
        }
        self.state.tab_drag.current_x = x;
        self.state.tab_drag.current_y = y;
        self.state.tab_drag.screen_x = f32::from(self.state.last_window_origin.x) + x;
        self.state.tab_drag.screen_y = f32::from(self.state.last_window_origin.y) + y;

        if !self.state.tab_drag.drag_active {
            let dx = x - self.state.tab_drag.drag_start_x;
            let dy = y - self.state.tab_drag.drag_start_y;
            if dx * dx + dy * dy > 25.0 {
                self.state.tab_drag.drag_active = true;
            }
        }

        if self.state.tab_drag.drag_active {
            let win_h = f32::from(self.state.last_window_size.height).max(400.0);
            let outside_strip = y < -14.0
                || y > 54.0
                || x < -24.0
                || x > window_width + 24.0
                || y > win_h;
            self.state.tab_drag.is_dragging_out = outside_strip;

            let (tab_count, pinned_count, dragged_label) = {
                let nav = self.state.navigation.read(cx);
                let tc = nav.tabs.tabs.len();
                let pc = nav.tabs.tabs.iter().filter(|t| t.pinned).count();
                let lbl = self
                    .state
                    .tab_drag
                    .dragged_index
                    .and_then(|di| nav.tabs.tabs.get(di))
                    .map(crate::ui::components::tab_bar::tab_display_label)
                    .unwrap_or_else(|| "Tab".to_string());
                (tc, pc, lbl)
            };

            let metrics = crate::ui::components::tab_bar::compute_strip_metrics(
                window_width,
                tab_count,
                pinned_count,
            );

            // Auto-scroll strip when dragging near left/right edges in overflow mode
            if metrics.tabs_overflow && !outside_strip {
                let strip_left = metrics.strip_start_x;
                let strip_right = strip_left + metrics.strip_viewport_w;
                if x < strip_left + 32.0 {
                    self.state.tab_drag.scroll_target =
                        (self.state.tab_drag.scroll_target - 14.0).max(0.0);
                } else if x > strip_right - 32.0 {
                    self.state.tab_drag.scroll_target =
                        (self.state.tab_drag.scroll_target + 14.0).min(metrics.max_scroll);
                }
            }

            if outside_strip {
                // Check if cursor is hovering over ANOTHER open files-rs window's tab bar!
                let hit =
                    crate::ui::components::tab_interactions::WindowTabRegistry::hit_test_other_window_tab_bar(
                        self.state.window_id,
                        self.state.tab_drag.screen_x,
                        self.state.tab_drag.screen_y,
                    );
                if let Some(target) = hit {
                    if self
                        .state
                        .tab_drag
                        .cross_window_target
                        .as_ref()
                        .map(|t| t.target_window_id != target.target_window_id)
                        .unwrap_or(false)
                    {
                        self.clear_external_drop_preview_on_target(cx);
                    }
                    let insert_idx = target.insert_index;
                    let local_x = target.local_x;
                    let lbl_clone = dragged_label.clone();
                    let _ = target.target_weak.update(cx, |target_win, tcx| {
                        target_win.state.tab_drag.external_drop_preview =
                            Some((insert_idx, lbl_clone, local_x));
                        tcx.notify();
                    });
                    self.state.tab_drag.cross_window_target = Some(target);
                } else {
                    self.clear_external_drop_preview_on_target(cx);
                }
            } else {
                self.clear_external_drop_preview_on_target(cx);

                if tab_count > 0 {
                    let rel_x =
                        (x - metrics.strip_start_x + self.state.tab_drag.scroll_offset).max(0.0);

                    let pinned_total_w =
                        pinned_count as f32 * crate::ui::components::tab_bar::PINNED_TAB_W;
                    let target_idx = if rel_x < pinned_total_w && pinned_count > 0 {
                        ((rel_x / crate::ui::components::tab_bar::PINNED_TAB_W).floor() as usize)
                            .min(pinned_count - 1)
                    } else {
                        let unpinned_x = (rel_x - pinned_total_w).max(0.0);
                        let idx = pinned_count
                            + (unpinned_x / metrics.tab_w.max(1.0)).floor() as usize;
                        idx.min(tab_count - 1)
                    };

                    self.state.tab_drag.drop_target_index = Some(target_idx);

                    // Live reorder with smooth neighbor slot-shift animation
                    if let Some(dragged_idx) = self.state.tab_drag.dragged_index {
                        let can_reorder = {
                            let nav = self.state.navigation.read(cx);
                            let src_pinned = nav
                                .tabs
                                .tabs
                                .get(dragged_idx)
                                .map(|t| t.pinned)
                                .unwrap_or(false);
                            let dst_pinned = nav
                                .tabs
                                .tabs
                                .get(target_idx)
                                .map(|t| t.pinned)
                                .unwrap_or(false);
                            src_pinned == dst_pinned && dragged_idx != target_idx
                        };
                        if can_reorder {
                            let slot_w = if dragged_idx < pinned_count {
                                crate::ui::components::tab_bar::PINNED_TAB_W
                            } else {
                                metrics.tab_w
                            };
                            self.state.navigation.update(cx, |nav, _cx| {
                                nav.tabs.reorder_tab(dragged_idx, target_idx);
                            });
                            self.record_slot_swap_animation(
                                dragged_idx,
                                target_idx,
                                slot_w,
                                tab_count,
                            );
                            self.state.tab_drag.dragged_index = Some(target_idx);
                            self.save_session(cx);
                        }
                    }
                }
            }
        }
        cx.notify();
    }

    /// Calculate the drop index based on mouse x position.
    pub fn calculate_drop_index(&self, mouse_x: f32, tab_count: usize) -> Option<usize> {
        if tab_count == 0 {
            return None;
        }
        let tab_start_x = 40.0;
        let tab_width = 150.0;
        let relative_x = (mouse_x - tab_start_x + self.state.tab_drag.scroll_offset).max(0.0);
        let index = (relative_x / tab_width) as usize;
        Some(index.min(tab_count - 1))
    }

    /// Complete tab drag (drop):
    /// 1. If hovering over another open `files-rs` window's tab bar (`cross_window_target`),
    ///    docks the tab into that window at `insert_index` (and closes this window if it was the last tab).
    /// 2. Else if dragged outside (`is_dragging_out`) and `tab_count > 1`, detaches the tab into a new window
    ///    at `(screen_x, screen_y)`.
    /// 3. Else if dragged outside and `tab_count == 1`, moves this window to `(screen_x, screen_y)`.
    pub fn end_tab_drag(&mut self, _drop_index: Option<usize>, cx: &mut Context<Self>) {
        if self.state.tab_drag.drag_active {
            if let Some(dragged_idx) = self.state.tab_drag.dragged_index {
                if let Some(cross_target) = self.state.tab_drag.cross_window_target.take() {
                    // Dock tab into the target window!
                    self.dock_tab_into_cross_target(dragged_idx, cross_target, None, cx);
                } else if self.state.tab_drag.is_dragging_out {
                    let tab_count = self.state.navigation.read(cx).tabs.tabs.len();
                    let screen_pos = (
                        self.state.tab_drag.screen_x,
                        self.state.tab_drag.screen_y,
                    );
                    if tab_count > 1 {
                        self.move_tab_to_new_window(dragged_idx, Some(screen_pos), cx);
                    } else {
                        // Single tab dragged outside: move the current window to the dropped screen position
                        crate::ui::components::tab_interactions::move_hwnd_to_screen_pos(
                            self.state.window_hwnd,
                            (screen_pos.0 - 100.0).max(0.0),
                            (screen_pos.1 - 16.0).max(0.0),
                            self.state.last_scale_factor,
                        );
                    }
                } else if let Some(target_idx) = self.state.tab_drag.drop_target_index {
                    if dragged_idx != target_idx {
                        self.state.navigation.update(cx, |nav, _cx| {
                            nav.tabs.reorder_tab(dragged_idx, target_idx);
                        });
                        self.save_session(cx);
                    }
                }
            }
        }
        self.clear_external_drop_preview_on_target(cx);
        self.state.tab_drag.clear_drag_only();
        cx.notify();
    }

    /// Move tab at `index` into a brand-new window (preserving its full `TabState`).
    pub fn move_tab_to_new_window(
        &mut self,
        index: usize,
        screen_origin: Option<(f32, f32)>,
        cx: &mut Context<Self>,
    ) {
        let tab_count = self.state.navigation.read(cx).tabs.tabs.len();
        if tab_count <= 1 {
            return;
        }
        let taken = self
            .state
            .navigation
            .update(cx, |nav, _cx| nav.tabs.take_tab(index));
        if let Some(tab) = taken {
            self.clamp_scroll_offset(cx);
            self.ensure_active_tab_visible(cx);
            self.save_session(cx);
            self.sync_active_tab_to_views(cx);
            self.open_tab_state_in_new_window(tab, screen_origin, cx);
            cx.notify();
        }
    }

    /// Move tab at `tab_index` into an existing open `AppWindow` (`target_window_id`).
    pub fn move_tab_to_existing_window(
        &mut self,
        tab_index: usize,
        target_window_id: u64,
        insert_index: Option<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let target_info =
            crate::ui::components::tab_interactions::WindowTabRegistry::other_windows(
                self.state.window_id,
            )
            .into_iter()
            .find(|w| w.window_id == target_window_id);
        if let Some(info) = target_info {
            let cross = crate::ui::components::tab_interactions::CrossWindowTabTarget {
                target_window_id: info.window_id,
                target_weak: info.weak_view,
                target_hwnd: info.hwnd,
                insert_index: insert_index.unwrap_or(info.tab_count),
                local_x: 0.0,
                target_window_title: info.active_tab_title,
            };
            self.dock_tab_into_cross_target(tab_index, cross, Some(window), cx);
        }
    }

    /// Internal helper to transfer `tab_index` from `self` into `cross_target`.
    fn dock_tab_into_cross_target(
        &mut self,
        tab_index: usize,
        cross_target: crate::ui::components::tab_interactions::CrossWindowTabTarget,
        window_opt: Option<&mut Window>,
        cx: &mut Context<Self>,
    ) {
        let taken = self
            .state
            .navigation
            .update(cx, |nav, _cx| nav.tabs.take_tab(tab_index));
        if let Some(tab) = taken {
            let insert_idx = cross_target.insert_index;
            let docked_ok = cross_target
                .target_weak
                .update(cx, |target_win, tcx| {
                    target_win.accept_docked_tab(tab, Some(insert_idx), tcx);
                })
                .is_ok();
            if docked_ok {
                crate::ui::components::tab_interactions::activate_hwnd(cross_target.target_hwnd);
            }

            let remaining = self.state.navigation.read(cx).tabs.tabs.len();
            if remaining == 0 {
                crate::ui::components::tab_interactions::WindowTabRegistry::unregister(
                    self.state.window_id,
                );
                self.stop_all_file_watchers();
                if let Some(win) = window_opt {
                    win.remove_window();
                } else {
                    #[cfg(windows)]
                    if self.state.window_hwnd != 0 {
                        use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
                        use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_CLOSE};
                        unsafe {
                            let _ = PostMessageW(
                                Some(HWND(self.state.window_hwnd as *mut _)),
                                WM_CLOSE,
                                WPARAM(0),
                                LPARAM(0),
                            );
                        }
                    }
                }
            } else {
                self.clamp_scroll_offset(cx);
                self.ensure_active_tab_visible(cx);
                self.save_session(cx);
                self.sync_active_tab_to_views(cx);
                cx.notify();
            }
        }
    }

    /// Receive a `TabState` docked from another `AppWindow` at `insert_index`.
    pub fn accept_docked_tab(
        &mut self,
        tab: crate::core::navigation_manager::TabState,
        insert_index: Option<usize>,
        cx: &mut Context<Self>,
    ) {
        self.state.tab_drag.external_drop_preview = None;
        let new_idx = self.state.navigation.update(cx, |nav, _cx| {
            let idx = insert_index.unwrap_or(nav.tabs.tabs.len());
            nav.tabs.insert_tab_at(idx, tab)
        });
        self.state.tab_drag.new_tab_anim = Some((new_idx, 0.0));
        self.ensure_active_tab_visible(cx);
        self.load_tab_thumbnails(cx);
        self.save_session(cx);
        self.sync_active_tab_to_views(cx);
        cx.notify();
    }

    /// Cancel tab drag (restores tab to `initial_index` if it was live-reordered during the drag).
    pub fn cancel_tab_drag(&mut self, cx: &mut Context<Self>) {
        if let (Some(cur_idx), Some(init_idx)) = (
            self.state.tab_drag.dragged_index,
            self.state.tab_drag.initial_index,
        ) {
            if cur_idx != init_idx {
                self.state.navigation.update(cx, |nav, _cx| {
                    nav.tabs.reorder_tab(cur_idx, init_idx);
                });
                self.save_session(cx);
            }
        }
        self.clear_external_drop_preview_on_target(cx);
        self.state.tab_drag.clear_drag_only();
        cx.notify();
    }

    /// Smoothly scroll the tab bar by `delta` pixels (positive = right, negative = left).
    pub fn scroll_tab_bar_by(&mut self, delta: f32, cx: &mut Context<Self>) {
        let win_w = f32::from(self.state.last_window_size.width).max(800.0);
        let (tab_count, pinned_count) = {
            let nav = self.state.navigation.read(cx);
            (
                nav.tabs.tabs.len(),
                nav.tabs.tabs.iter().filter(|t| t.pinned).count(),
            )
        };
        let metrics = crate::ui::components::tab_bar::compute_strip_metrics(
            win_w,
            tab_count,
            pinned_count,
        );
        if !metrics.tabs_overflow {
            self.state.tab_drag.scroll_target = 0.0;
            self.state.tab_drag.scroll_offset = 0.0;
        } else {
            self.state.tab_drag.scroll_target =
                (self.state.tab_drag.scroll_target + delta).clamp(0.0, metrics.max_scroll);
        }
        cx.notify();
    }

    /// Scroll the tab bar left by ~1.5 tab widths (smoothly interpolated).
    pub fn scroll_tab_bar_left(&mut self, cx: &mut Context<Self>) {
        self.scroll_tab_bar_by(-180.0, cx);
    }

    /// Scroll the tab bar right by ~1.5 tab widths (smoothly interpolated).
    pub fn scroll_tab_bar_right(&mut self, cx: &mut Context<Self>) {
        self.scroll_tab_bar_by(180.0, cx);
    }

    /// Ensure the active tab is smoothly scrolled into the visible tab strip viewport.
    pub fn ensure_active_tab_visible(&mut self, cx: &mut Context<Self>) {
        let win_w = f32::from(self.state.last_window_size.width).max(800.0);
        let (tab_count, pinned_count, active_idx) = {
            let nav = self.state.navigation.read(cx);
            (
                nav.tabs.tabs.len(),
                nav.tabs.tabs.iter().filter(|t| t.pinned).count(),
                nav.tabs.active_tab_index,
            )
        };
        let metrics = crate::ui::components::tab_bar::compute_strip_metrics(
            win_w,
            tab_count,
            pinned_count,
        );
        if !metrics.tabs_overflow {
            self.state.tab_drag.scroll_target = 0.0;
            self.state.tab_drag.scroll_offset = 0.0;
            return;
        }
        let (tab_left, this_w) = crate::ui::components::tab_bar::tab_slot_x_and_width(
            active_idx,
            pinned_count,
            metrics.tab_w,
        );
        let tab_right = tab_left + this_w;

        let cur = self.state.tab_drag.scroll_target;
        if tab_left < cur {
            self.state.tab_drag.scroll_target = tab_left.max(0.0);
        } else if tab_right > cur + metrics.strip_viewport_w {
            self.state.tab_drag.scroll_target =
                (tab_right - metrics.strip_viewport_w).clamp(0.0, metrics.max_scroll);
        } else {
            self.state.tab_drag.scroll_target = cur.clamp(0.0, metrics.max_scroll);
        }
    }

    /// Clamp scroll_target and scroll_offset: reset to 0 if tabs no longer overflow,
    /// otherwise clamp so we don't scroll past the end.
    fn clamp_scroll_offset(&mut self, cx: &mut Context<Self>) {
        let win_w = f32::from(self.state.last_window_size.width).max(800.0);
        let (tab_count, pinned_count) = {
            let nav = self.state.navigation.read(cx);
            (
                nav.tabs.tabs.len(),
                nav.tabs.tabs.iter().filter(|t| t.pinned).count(),
            )
        };
        let metrics = crate::ui::components::tab_bar::compute_strip_metrics(
            win_w,
            tab_count,
            pinned_count,
        );
        if !metrics.tabs_overflow {
            self.state.tab_drag.scroll_target = 0.0;
            self.state.tab_drag.scroll_offset = 0.0;
        } else {
            self.state.tab_drag.scroll_target =
                self.state.tab_drag.scroll_target.clamp(0.0, metrics.max_scroll);
            self.state.tab_drag.scroll_offset =
                self.state.tab_drag.scroll_offset.clamp(0.0, metrics.max_scroll);
        }
    }

    /// Close all tabs except the specified one (and pinned tabs).
    pub fn close_other_tabs(&mut self, keep_index: usize, cx: &mut Context<Self>) {
        let tab_count = self.state.navigation.read(cx).tabs.tabs.len();
        self.state.navigation.update(cx, |nav, _cx| {
            for i in (0..tab_count).rev() {
                if i != keep_index && !nav.tabs.tabs.get(i).map(|t| t.pinned).unwrap_or(false) {
                    nav.tabs.close_tab(i);
                }
            }
        });
        self.save_session(cx);
        self.clamp_scroll_offset(cx);
        self.ensure_active_tab_visible(cx);
        self.sync_active_tab_to_views(cx);
        cx.notify();
    }

    /// Close all unpinned tabs to the left of `from_index` (Files-community "Close tabs to the left").
    pub fn close_tabs_to_left(&mut self, from_index: usize, cx: &mut Context<Self>) {
        self.state.navigation.update(cx, |nav, _cx| {
            for i in (0..from_index).rev() {
                if !nav.tabs.tabs.get(i).map(|t| t.pinned).unwrap_or(false) {
                    nav.tabs.close_tab(i);
                }
            }
        });
        self.save_session(cx);
        self.clamp_scroll_offset(cx);
        self.ensure_active_tab_visible(cx);
        self.sync_active_tab_to_views(cx);
        cx.notify();
    }

    /// Close all tabs to the right of the specified one.
    pub fn close_tabs_to_right(&mut self, from_index: usize, cx: &mut Context<Self>) {
        let tab_count = self.state.navigation.read(cx).tabs.tabs.len();
        self.state.navigation.update(cx, |nav, _cx| {
            for i in (from_index + 1..tab_count).rev() {
                if !nav.tabs.tabs.get(i).map(|t| t.pinned).unwrap_or(false) {
                    nav.tabs.close_tab(i);
                }
            }
        });
        self.save_session(cx);
        self.clamp_scroll_offset(cx);
        self.ensure_active_tab_visible(cx);
        self.sync_active_tab_to_views(cx);
        cx.notify();
    }

    /// Duplicate the specified tab immediately to its right.
    pub fn duplicate_tab(&mut self, index: usize, cx: &mut Context<Self>) {
        let tab = self.state.navigation.read(cx).tabs.tabs.get(index).cloned();
        if let Some(mut dup_tab) = tab {
            dup_tab.pinned = false;
            let new_idx = self.state.navigation.update(cx, |nav, _cx| {
                nav.tabs.insert_tab_at(index + 1, dup_tab)
            });
            self.state.tab_drag.new_tab_anim = Some((new_idx, 0.0));
            self.ensure_active_tab_visible(cx);
            self.save_session(cx);
            self.sync_active_tab_to_views(cx);
            cx.notify();
        }
    }

    /// Toggle pin state on the specified tab (pinned tabs stay compact on the left).
    pub fn pin_tab(&mut self, index: usize, cx: &mut Context<Self>) {
        self.state.navigation.update(cx, |nav, _cx| {
            nav.tabs.toggle_pin_tab(index);
        });
        self.clamp_scroll_offset(cx);
        self.ensure_active_tab_visible(cx);
        self.save_session(cx);
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
    /// Drops the OS watcher on a background thread so joining the notify worker never blocks the UI thread.
    pub fn stop_file_watcher(&mut self, secondary: bool) {
        if secondary {
            if let Some(w) = self.file_watcher_secondary.take() {
                std::thread::spawn(move || drop(w));
            }
            self.file_event_rx_secondary = None;
        } else {
            if let Some(w) = self.file_watcher.take() {
                std::thread::spawn(move || drop(w));
            }
            self.file_event_rx = None;
        }
    }

    /// Stop all file watchers
    pub fn stop_all_file_watchers(&mut self) {
        self.stop_file_watcher(false);
        self.stop_file_watcher(true);
    }

    /// Poll for file system events, incrementally invalidate changed folder-size cache branches,
    /// and coalesce burst events into a single refresh per frame.
    pub fn poll_file_events(&mut self, cx: &mut Context<Self>) {
        let mut refresh_primary = false;
        if let Some(rx) = &mut self.file_event_rx {
            while let Ok(event) = rx.try_recv() {
                match event {
                    crate::storage::storage::FileEvent::Created(p)
                    | crate::storage::storage::FileEvent::Modified(p)
                    | crate::storage::storage::FileEvent::Removed(p) => {
                        crate::storage::folder_size::refresh_changed_branch_background(
                            p.as_path().to_path_buf(),
                        );
                    }
                    crate::storage::storage::FileEvent::Renamed(from, to) => {
                        crate::storage::folder_size::refresh_changed_branch_background(
                            from.as_path().to_path_buf(),
                        );
                        crate::storage::folder_size::refresh_changed_branch_background(
                            to.as_path().to_path_buf(),
                        );
                    }
                }
                refresh_primary = true;
            }
        }
        if refresh_primary {
            self.refresh(cx);
        }

        let mut refresh_secondary = false;
        if let Some(rx) = &mut self.file_event_rx_secondary {
            while let Ok(event) = rx.try_recv() {
                match event {
                    crate::storage::storage::FileEvent::Created(p)
                    | crate::storage::storage::FileEvent::Modified(p)
                    | crate::storage::storage::FileEvent::Removed(p) => {
                        crate::storage::folder_size::refresh_changed_branch_background(
                            p.as_path().to_path_buf(),
                        );
                    }
                    crate::storage::storage::FileEvent::Renamed(from, to) => {
                        crate::storage::folder_size::refresh_changed_branch_background(
                            from.as_path().to_path_buf(),
                        );
                        crate::storage::folder_size::refresh_changed_branch_background(
                            to.as_path().to_path_buf(),
                        );
                    }
                }
                refresh_secondary = true;
            }
        }
        if refresh_secondary {
            self.refresh_secondary_pane(cx);
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

        // Poll OS cursor & left button when dragging a tab so cross-window docking and
        // desktop drag-out work seamlessly even when the cursor leaves this window!
        let mut os_drag_active = false;
        if self.state.tab_drag.is_dragging {
            if let Some(((local_x, local_y), (screen_x, screen_y), lbutton_down)) =
                crate::ui::components::tab_interactions::query_os_cursor_and_lbutton(
                    self.state.window_hwnd,
                    self.state.last_scale_factor,
                )
            {
                self.state.tab_drag.screen_x = screen_x;
                self.state.tab_drag.screen_y = screen_y;
                let win_w = f32::from(self.state.last_window_size.width).max(800.0);
                if !lbutton_down {
                    // User released the mouse button (possibly outside the window or over another window)
                    self.end_tab_drag(None, cx);
                } else {
                    if (local_x - self.state.tab_drag.current_x).abs() > 0.5
                        || (local_y - self.state.tab_drag.current_y).abs() > 0.5
                    {
                        self.update_tab_drag_with_width(local_x, local_y, win_w, cx);
                    }
                    os_drag_active = true;
                }
            }
        }

        let tab_anim_tick = self.state.tab_drag.tick_animations(dt);
        let nav_anim_tick = self
            .state
            .nav_bar
            .tick_animations(dt, self.state.main_view.search_state.loading);
        let status_anim_tick = self.state.status_center.tick_animations(dt);

        let sidebar_tick;
        let main_tick;
        let secondary_tick;
        let side_pane_tick;
        let peek_tick;
        {
            let bs = &mut self.state.sidebar.bouncy_scroll;
            bs.post_settle_cooldown = bs.post_settle_cooldown.saturating_sub(1);
            sidebar_tick = if bs.animating {
                bs.tick_spring(dt)
            } else {
                false
            };
        }
        {
            let bs = &mut self.state.main_view.bouncy_scroll;
            bs.post_settle_cooldown = bs.post_settle_cooldown.saturating_sub(1);
            main_tick = if bs.animating {
                bs.tick_spring(dt)
            } else {
                false
            };
        }
        {
            let bs = &mut self.state.secondary_view.bouncy_scroll;
            bs.post_settle_cooldown = bs.post_settle_cooldown.saturating_sub(1);
            secondary_tick = if bs.animating {
                bs.tick_spring(dt)
            } else {
                false
            };
        }
        {
            let bs = &mut self.state.main_view.side_pane_bouncy_scroll;
            bs.post_settle_cooldown = bs.post_settle_cooldown.saturating_sub(1);
            side_pane_tick = if bs.animating {
                bs.tick_spring(dt)
            } else {
                false
            };
        }
        {
            let bs = &mut self.state.main_view.peek_bouncy_scroll;
            bs.post_settle_cooldown = bs.post_settle_cooldown.saturating_sub(1);
            peek_tick = if bs.animating {
                bs.tick_spring(dt)
            } else {
                false
            };
        }
        let mut video_tick = false;
        // Real player drives the timeline when present; otherwise simulate.
        if self.state.video_player.is_some()
            && matches!(
                self.state.main_view.preview_content,
                crate::ui::views::main_view::PreviewContent::Video(_)
            )
        {
            let player = self.state.video_player.as_ref().unwrap();
            let dur = player.duration_secs().max(0.1);
            let pos = player.progress_secs().clamp(0.0, dur);
            self.state.main_view.video_progress = (pos / dur) as f32;
            if player.has_ended() {
                self.state.main_view.video_playing = false;
            } else {
                self.state.main_view.video_playing = player.is_playing();
            }
            self.state.main_view.video_frame = player.current_frame();
            video_tick = true;
        } else if self.state.main_view.video_playing {
            let dur = match &self.state.main_view.preview_content {
                crate::ui::views::main_view::PreviewContent::Video(info)
                | crate::ui::views::main_view::PreviewContent::Audio(info) => {
                    info.duration_secs.unwrap_or(30.0).max(5.0) as f32
                }
                _ => 30.0,
            };
            self.state.main_view.video_progress += dt / dur;
            if self.state.main_view.video_progress >= 1.0 {
                self.state.main_view.video_progress = 0.0;
                self.state.main_view.video_playing = false;
            }
            video_tick = true;
        }

        if sidebar_tick
            || main_tick
            || secondary_tick
            || side_pane_tick
            || peek_tick
            || video_tick
            || tab_anim_tick
            || nav_anim_tick
            || status_anim_tick
            || os_drag_active
        {
            if main_tick
                || (!self.state.main_view.bouncy_scroll.animating
                    && self.state.main_view.bouncy_scroll.post_settle_cooldown == 4)
            {
                let offset = self.state.main_view.bouncy_scroll.offset;
                if matches!(
                    self.state.main_view.target,
                    crate::ui::views::main_view::ViewTarget::Home
                        | crate::ui::views::main_view::ViewTarget::Settings
                        | crate::ui::views::main_view::ViewTarget::RecycleBin
                ) {
                    self.state
                        .main_view
                        .scroll_handle
                        .set_offset(gpui::point(px(0.0), px(offset)));
                } else {
                    self.state
                        .main_view
                        .uniform_list_scroll_handle
                        .0
                        .borrow()
                        .base_handle
                        .set_offset(gpui::point(px(0.0), px(offset)));
                }
            }
            if secondary_tick
                || (!self.state.secondary_view.bouncy_scroll.animating
                    && self.state.secondary_view.bouncy_scroll.post_settle_cooldown == 4)
            {
                let offset = self.state.secondary_view.bouncy_scroll.offset;
                self.state
                    .secondary_view
                    .uniform_list_scroll_handle
                    .0
                    .borrow()
                    .base_handle
                    .set_offset(gpui::point(px(0.0), px(offset)));
            }
            if side_pane_tick
                || (!self.state.main_view.side_pane_bouncy_scroll.animating
                    && self.state.main_view.side_pane_bouncy_scroll.post_settle_cooldown == 4)
            {
                let offset = self.state.main_view.side_pane_bouncy_scroll.offset;
                self.state
                    .main_view
                    .side_pane_scroll_handle
                    .set_offset(gpui::point(px(0.0), px(offset)));
            }
            if peek_tick
                || (!self.state.main_view.peek_bouncy_scroll.animating
                    && self.state.main_view.peek_bouncy_scroll.post_settle_cooldown == 4)
            {
                let offset = self.state.main_view.peek_bouncy_scroll.offset;
                self.state
                    .main_view
                    .peek_scroll_handle
                    .set_offset(gpui::point(px(0.0), px(offset)));
            }
            cx.notify();
        }
    }

    pub fn open_in_secondary_pane(&mut self, path: FilesPath, cx: &mut Context<Self>) {
        let layout = self.state.navigation.read(cx).pane_layout;
        if layout == crate::core::navigation_manager::PaneLayout::Single {
            self.state.navigation.update(cx, |nav, _cx| {
                nav.split_pane_horizontal();
            });
        }
        self.state
            .navigation
            .update(cx, |nav, _cx| {
                nav.tabs.active_tab_mut().secondary_pane_path = Some(path);
            });
        let layout = self.state.navigation.read(cx).pane_layout;
        self.state.main_view.pane_type = match layout {
            crate::core::navigation_manager::PaneLayout::Single => {
                crate::ui::views::main_view::PaneType::Single
            }
            _ => crate::ui::views::main_view::PaneType::Dual,
        };
        if self.state.main_view.pane_type == crate::ui::views::main_view::PaneType::Dual {
            self.sync_secondary_pane_on_split(cx);
        }
        cx.notify();
    }

    /// Refresh the secondary pane (for split view)
    pub fn refresh_secondary_pane(&mut self, cx: &mut Context<Self>) {
        let secondary_path = self.state.secondary_view.primary_path.clone();
        if !secondary_path.as_path().as_os_str().is_empty() {
            self.navigate_pane(
                secondary_path,
                crate::core::navigation_manager::PaneFocus::Secondary,
                cx,
            );
        }
    }

    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        let pane_focus = self.state.active_pane_focus();
        let path = self.state.pane_view(pane_focus).primary_path.clone();
        self.navigate_pane(path, pane_focus, cx);
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
        let active_view = self.state.active_view();
        if active_view.selected_indices.is_empty() {
            return;
        }

        // Get selected items
        let items: Vec<_> = active_view
            .selected_indices
            .iter()
            .filter_map(|&i| active_view.items.get(i))
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
                use crate::core::transfer::{TransferOutcome, run_delete_job_rt};
                let n = bufs.len();
                let job = engine
                    .start_job(format!("Delete ({} items)", n), OpKind::Delete)
                    .await;
                let parent_to_inv = bufs.first().and_then(|p| p.parent().map(|d| d.to_path_buf()));
                match run_delete_job_rt(engine.clone(), job, bufs, true).await {
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
                if let Some(parent_dir) = parent_to_inv {
                    crate::storage::folder_size::refresh_changed_branch_background(parent_dir);
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
        let active_view = self.state.active_view();
        if active_view.selected_indices.is_empty() {
            return;
        }

        // Get selected item paths
        let paths: Vec<_> = active_view
            .selected_indices
            .iter()
            .filter_map(|&i| active_view.items.get(i))
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
                use crate::core::transfer::{TransferOutcome, run_delete_job_rt};
                let n = bufs.len();
                let job = engine
                    .start_job(format!("Delete ({} items)", n), OpKind::Delete)
                    .await;
                let parent_to_inv = bufs.first().and_then(|p| p.parent().map(|d| d.to_path_buf()));
                match run_delete_job_rt(engine.clone(), job, bufs, false).await {
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
        self.set_sort_column(col, cx);
    }

    /// Select a single item by its index in the currently active pane.
    pub fn select_item(&mut self, index: usize, shift: bool, ctrl: bool, cx: &mut Context<Self>) {
        let pane_focus = self.state.active_pane_focus();
        self.select_item_in_pane(pane_focus, index, shift, ctrl, cx);
    }

    /// Select an item in the specified pane (`PaneFocus::Primary` or `PaneFocus::Secondary`),
    /// supporting `Shift` range selection in visual sorted order (`visible_items_rc`) and `Ctrl` toggle.
    pub fn select_item_in_pane(
        &mut self,
        pane_focus: crate::core::navigation_manager::PaneFocus,
        index: usize,
        shift: bool,
        ctrl: bool,
        cx: &mut Context<Self>,
    ) {
        self.focus_pane(pane_focus, cx);

        {
            let view = self.state.pane_view_mut(pane_focus);
            if index >= view.items.len() {
                return;
            }
            if ctrl {
                if view.selected_indices.contains(&index) {
                    view.selected_indices.remove(&index);
                } else {
                    view.selected_indices.insert(index);
                }
                view.selection_anchor_index = Some(index);
            } else if shift {
                let visual = view.visual_order_items();
                let anchor = view
                    .selection_anchor_index
                    .or(view.last_clicked_index)
                    .or_else(|| view.selected_indices.iter().next().copied())
                    .unwrap_or(index)
                    .min(view.items.len().saturating_sub(1));
                view.selection_anchor_index = Some(anchor);
                view.selected_indices.clear();

                let pos_a = visual.iter().position(|&i| i == anchor);
                let pos_b = visual.iter().position(|&i| i == index);
                if let (Some(pa), Some(pb)) = (pos_a, pos_b) {
                    let start = pa.min(pb);
                    let end = pa.max(pb);
                    for &vi in &visual[start..=end] {
                        view.selected_indices.insert(vi);
                    }
                } else {
                    let start = anchor.min(index);
                    let end = anchor.max(index);
                    for i in start..=end {
                        view.selected_indices.insert(i);
                    }
                }
            } else {
                view.selected_indices.clear();
                view.selected_indices.insert(index);
                view.selection_anchor_index = Some(index);
            }
            view.last_clicked_index = Some(index);
        }

        // Trigger preview load if single item selected
        let single_item_info = {
            let view = self.state.pane_view(pane_focus);
            if view.selected_indices.len() == 1 {
                let idx = *view.selected_indices.iter().next().unwrap();
                view.items.get(idx).map(|item| {
                    (
                        item.path.clone(),
                        matches!(item.file_type, FileType::File),
                    )
                })
            } else {
                None
            }
        };

        let preview_visible = !matches!(
            self.state.main_view.side_pane,
            crate::ui::views::main_view::SidePaneMode::None
        ) || self.state.main_view.show_peek_modal;

        if let Some((item_path, is_file)) = single_item_info {
            if preview_visible {
                if is_file {
                    self.load_preview_content(item_path.clone(), cx);
                } else {
                    self.state.main_view.preview_text =
                        Some("Loading folder contents...".to_string());
                    self.load_folder_preview(item_path.clone(), cx);
                }
                self.update_details_cache_from_path(&item_path, cx);
            }
        } else {
            self.state.main_view.preview_text = None;
            self.state.main_view.preview_content =
                crate::ui::views::main_view::PreviewContent::default();
            self.state.main_view.details_cache = DetailsPaneCache::default();
        }

        self.sync_pane_selection_model(pane_focus, cx);
        cx.notify();
    }

    /// Scroll the active pane's `uniform_list` and `bouncy_scroll` so `item_index` is visible.
    pub fn scroll_item_into_view(
        &mut self,
        pane_focus: crate::core::navigation_manager::PaneFocus,
        item_index: usize,
    ) {
        let win_w = f32::from(self.state.last_window_size.width).max(800.0);
        let win_h = f32::from(self.state.last_window_size.height).max(600.0);
        let sidebar_w = if self.state.sidebar_visible {
            self.state.sidebar.width
        } else {
            0.0
        };
        let mut pane_w = (win_w - sidebar_w).max(240.0);
        if self.state.main_view.pane_type == crate::ui::views::main_view::PaneType::Dual {
            let ratio = self.state.main_view.dual_pane_ratio.clamp(0.2, 0.8);
            pane_w = if pane_focus == crate::core::navigation_manager::PaneFocus::Primary {
                pane_w * ratio
            } else {
                pane_w * (1.0 - ratio)
            };
        }
        let viewport_h = (win_h - 170.0).max(160.0);

        let view = self.state.pane_view_mut(pane_focus);
        if item_index >= view.items.len() {
            return;
        }

        let (item_top, item_bottom, row_h) = if view.group_by
            != crate::config::config::GroupColumn::None
            && !matches!(view.view_mode, ViewMode::Columns)
        {
            let grouped_bounds = view.grouped_item_bounds_for_view(pane_w);
            if let Some((_, (_x0, y0, _x1, y1))) =
                grouped_bounds.into_iter().find(|(idx, _)| *idx == item_index)
            {
                let rh = (y1 - y0).max(24.0);
                (y0, y1, rh)
            } else {
                return;
            }
        } else {
            let visible = view.visible_items_rc();
            let vis_pos = visible
                .iter()
                .position(|&i| i == item_index)
                .unwrap_or(item_index);
            let (row_idx, rh) = match view.view_mode {
                ViewMode::Grid => {
                    let cols = MainViewState::grid_columns_for_width(pane_w, view.zoom_level);
                    let params = MainViewState::grid_layout_params_for_width_pub(
                        pane_w,
                        view.zoom_level,
                        cols,
                    );
                    (vis_pos / params.items_per_row.max(1), params.row_height)
                }
                ViewMode::Cards => {
                    let (cols, _card_w, rh, _gap, _pad) = MainViewState::cards_layout_metrics(
                        pane_w,
                        view.zoom_level,
                        view.compact_mode,
                    );
                    (vis_pos / cols.max(1), rh)
                }
                ViewMode::List => {
                    let (cols, _col_w, rh, _gap, _pad) = MainViewState::list_layout_metrics(
                        pane_w,
                        view.zoom_level,
                        view.compact_mode,
                    );
                    (vis_pos / cols.max(1), rh)
                }
                ViewMode::Columns => {
                    let rh = (22.0 + view.zoom_level * 22.0).round();
                    (vis_pos, rh)
                }
                ViewMode::Details => {
                    let base_row_h = if view.compact_mode { 22.0 } else { 26.0 };
                    let rh = (base_row_h + view.zoom_level * 22.0).round();
                    (vis_pos, rh)
                }
            };
            let top = row_idx as f32 * rh;
            (top, top + rh, rh)
        };
        let cur_scroll = (-view.bouncy_scroll.offset).max(0.0);

        let new_scroll = if item_top < cur_scroll {
            Some(item_top)
        } else if item_bottom > cur_scroll + viewport_h - row_h {
            Some((item_bottom - viewport_h + row_h).max(0.0))
        } else {
            None
        };

        if let Some(target_scroll) = new_scroll {
            view.bouncy_scroll.set_immediate_offset(-target_scroll);
            view.uniform_list_scroll_handle
                .0
                .borrow()
                .base_handle
                .set_offset(gpui::point(px(0.0), px(-target_scroll)));
        }
    }

    /// Compute and display accurate total size for all selected items (files + folders)
    /// in the status bar. Uses non-blocking `folder_size::peek_cached_size` on the UI thread
    /// (zero syscalls) and computes any uncached selected folders on the background executor.
    fn refresh_folder_size_status(
        &mut self,
        pane_focus: crate::core::navigation_manager::PaneFocus,
        cx: &mut Context<Self>,
    ) {
        self.state.status_bar.folder_size_request_id =
            self.state.status_bar.folder_size_request_id.wrapping_add(1);
        let req_id = self.state.status_bar.folder_size_request_id;

        let (file_bytes, cached_folder_bytes, uncached_folders, single_folder_opt) = {
            let view = self.state.pane_view_mut(pane_focus);
            let mut file_sum = 0u64;
            let mut folder_sum = 0u64;
            let mut uncached = Vec::new();
            let mut single_folder = None;
            let is_single = view.selected_indices.len() == 1;

            let selected_vec: Vec<usize> = view.selected_indices.iter().copied().collect();
            for idx in selected_vec {
                if let Some(item) = view.items.get_mut(idx) {
                    if matches!(item.file_type, crate::storage::storage::FileType::Directory) {
                        if is_single {
                            single_folder = Some(item.path.clone());
                        }
                        if let Some(sz) =
                            crate::storage::folder_size::peek_cached_size(item.path.as_path())
                        {
                            item.size = sz;
                            folder_sum = folder_sum.saturating_add(sz);
                        } else if item.size > 0 {
                            folder_sum = folder_sum.saturating_add(item.size);
                        } else {
                            uncached.push(item.path.clone());
                        }
                    } else {
                        file_sum = file_sum.saturating_add(item.size);
                    }
                }
            }
            (file_sum, folder_sum, uncached, single_folder)
        };

        let immediate_total = file_bytes.saturating_add(cached_folder_bytes);
        self.state.status_bar.selected_size_bytes = immediate_total;
        if single_folder_opt.is_some() && uncached_folders.is_empty() {
            self.state.main_view.details_cache.folder_total_size = cached_folder_bytes;
        }

        if uncached_folders.is_empty() {
            self.state.status_bar.folder_size_pending = false;
            self.state.status_bar.folder_size_for = single_folder_opt;
            return;
        }

        let was_already_pending = self.state.status_bar.folder_size_pending;
        let sentinel_path = single_folder_opt
            .clone()
            .unwrap_or_else(|| uncached_folders[0].clone());
        self.state.status_bar.folder_size_pending = true;
        self.state.status_bar.folder_size_for = Some(sentinel_path.clone());
        if !was_already_pending {
            self.state.status_bar.spinner_frame = 0;
        }

        let mut trigger = cx.to_async();
        let weak = cx.weak_entity();
        let folders_to_calc = uncached_folders;
        let bg = cx.background_executor().spawn(async move {
            let mut computed: Vec<(crate::core::path_util::FilesPath, u64)> =
                Vec::with_capacity(folders_to_calc.len());
            for fp in folders_to_calc {
                let sz = crate::storage::folder_size::compute_and_store(fp.as_path());
                computed.push((fp, sz));
            }
            computed
        });
        cx.foreground_executor()
            .spawn(async move {
                let computed = bg.await;
                weak.update(&mut trigger, move |this, cx| {
                    {
                        let view = this.state.pane_view_mut(pane_focus);
                        for (fp, sz) in &computed {
                            if let Some(it) = view.items.iter_mut().find(|i| &i.path == fp) {
                                it.size = *sz;
                            }
                        }
                        view.bump_items_revision();
                    }
                    if this.state.active_pane_focus() == pane_focus
                        && this.state.status_bar.folder_size_request_id == req_id
                    {
                        let (total, single) = {
                            let view = this.state.pane_view(pane_focus);
                            let mut total = 0u64;
                            for &idx in &view.selected_indices {
                                if let Some(it) = view.items.get(idx) {
                                    if matches!(
                                        it.file_type,
                                        crate::storage::storage::FileType::Directory
                                    ) {
                                        let sz = crate::storage::folder_size::peek_cached_size(
                                            it.path.as_path(),
                                        )
                                        .unwrap_or(it.size);
                                        total = total.saturating_add(sz);
                                    } else {
                                        total = total.saturating_add(it.size);
                                    }
                                }
                            }
                            (total, view.selected_indices.len() == 1)
                        };
                        this.state.status_bar.selected_size_bytes = total;
                        this.state.status_bar.folder_size_pending = false;
                        if single {
                            this.state.main_view.details_cache.folder_total_size = total;
                        }
                        cx.notify();
                    }
                })
                .ok();
            })
            .detach();

        // Only spawn a single spinner ticker while folder_size_pending is true.
        if !was_already_pending {
            cx.spawn(
                async move |weak: gpui::WeakEntity<Self>, cx: &mut gpui::AsyncApp| {
                    loop {
                        cx.background_executor()
                            .timer(std::time::Duration::from_millis(220))
                            .await;
                        let cont = weak
                            .update(cx, |this, cx| {
                                if !this.state.status_bar.folder_size_pending {
                                    return false;
                                }
                                this.state.status_bar.spinner_frame =
                                    (this.state.status_bar.spinner_frame + 1) % 4;
                                cx.notify();
                                true
                            })
                            .unwrap_or(false);
                        if !cont {
                            break;
                        }
                    }
                },
            )
            .detach();
        }
    }

    /// Synchronize status bar selection counters and the GPUI `FileItems` entity for `pane_focus`.
    pub(crate) fn sync_pane_selection_model(
        &mut self,
        pane_focus: crate::core::navigation_manager::PaneFocus,
        cx: &mut Context<Self>,
    ) {
        let (items_len, sel_clone) = {
            let view = self.state.pane_view(pane_focus);
            (view.items.len(), view.selected_indices.clone())
        };
        if self.state.active_pane_focus() == pane_focus {
            self.state.status_bar.selected_count = sel_clone.len();
            self.refresh_folder_size_status(pane_focus, cx);
        }
        let need_items_sync =
            self.state.pane_file_items(pane_focus).read(cx).items.len() != items_len;
        let items_opt = if need_items_sync {
            Some(self.state.pane_view(pane_focus).items.clone())
        } else {
            None
        };
        self.state.pane_file_items(pane_focus).update(cx, |m, _| {
            if let Some(items) = items_opt {
                m.items = items;
            }
            m.selected_indices = sel_clone;
        });
    }

    /// Zero-syscall, zero-task-spawn selection model update used during live rubber-band (lasso)
    /// mouse-move frames. Sums file sizes and cached folder sizes in O(1) memory without launching
    /// background directory walks mid-drag; `end_lasso_in_pane` triggers the full background walk
    /// once for any uncached folders when the drag finishes.
    fn sync_pane_selection_model_during_lasso(
        &mut self,
        pane_focus: crate::core::navigation_manager::PaneFocus,
        cx: &mut Context<Self>,
    ) {
        let (sel_clone, immediate_total) = {
            let view = self.state.pane_view_mut(pane_focus);
            let mut sum = 0u64;
            let sel = view.selected_indices.clone();
            for &idx in &sel {
                if let Some(item) = view.items.get_mut(idx) {
                    if matches!(item.file_type, crate::storage::storage::FileType::Directory) {
                        if let Some(sz) =
                            crate::storage::folder_size::peek_cached_size(item.path.as_path())
                        {
                            item.size = sz;
                            sum = sum.saturating_add(sz);
                        } else {
                            sum = sum.saturating_add(item.size);
                        }
                    } else {
                        sum = sum.saturating_add(item.size);
                    }
                }
            }
            (sel, sum)
        };
        if self.state.active_pane_focus() == pane_focus {
            self.state.status_bar.selected_count = sel_clone.len();
            self.state.status_bar.selected_size_bytes = immediate_total;
        }
        self.state.pane_file_items(pane_focus).update(cx, |m, _| {
            m.selected_indices = sel_clone;
        });
    }

    /// Start live rubber-band (marquee) drag selection on empty background space in `pane_focus`.
    pub fn start_lasso_in_pane(
        &mut self,
        pane_focus: crate::core::navigation_manager::PaneFocus,
        pos: Point<f32>,
        list_origin: Point<f32>,
        additive: bool,
        cx: &mut Context<Self>,
    ) {
        self.focus_pane(pane_focus, cx);
        if self.state.pane_view(pane_focus).renaming_index.is_some() {
            self.commit_rename(cx);
        }
        let double_click_back = self
            .state
            .pane_view_mut(pane_focus)
            .check_empty_space_double_click();
        if double_click_back {
            let enable_dbl = self
                .state
                .settings
                .read(cx)
                .config()
                .actions
                .double_click_empty_space_go_up;
            if enable_dbl {
                self.navigate_up(cx);
                return;
            }
        }
        self.state
            .pane_view_mut(pane_focus)
            .lasso_start_at_window_pos(pos, list_origin, additive);
        self.sync_pane_selection_model_during_lasso(pane_focus, cx);
        cx.notify();
    }

    /// Update live rubber-band (marquee) selection rectangle as the mouse moves.
    pub fn update_lasso_in_pane(
        &mut self,
        pane_focus: crate::core::navigation_manager::PaneFocus,
        pos: Point<f32>,
        pane_w: f32,
        cx: &mut Context<Self>,
    ) {
        if !self.state.pane_view(pane_focus).is_lasso_active() {
            return;
        }
        let selection_changed = self
            .state
            .pane_view_mut(pane_focus)
            .lasso_update_at_window_pos(pos, pane_w);
        if selection_changed {
            self.sync_pane_selection_model_during_lasso(pane_focus, cx);
        }
        cx.notify();
    }

    /// Finish live rubber-band (marquee) selection when the mouse button is released.
    pub fn end_lasso_in_pane(
        &mut self,
        pane_focus: crate::core::navigation_manager::PaneFocus,
        cx: &mut Context<Self>,
    ) {
        if self.state.pane_view(pane_focus).is_lasso_active() {
            self.state.pane_view_mut(pane_focus).finish_live_lasso();
            self.sync_pane_selection_model(pane_focus, cx);
            cx.notify();
        }
    }

    /// Select all visible items in the active pane.
    pub fn select_all(&mut self, cx: &mut Context<Self>) {
        let pane_focus = self.state.active_pane_focus();
        {
            let view = self.state.pane_view_mut(pane_focus);
            let visible = view.visual_order_items();
            view.selected_indices.clear();
            for i in visible {
                view.selected_indices.insert(i);
            }
        }
        self.sync_pane_selection_model(pane_focus, cx);
        cx.notify();
    }

    /// Clear all selections in the active pane.
    pub fn clear_selection(&mut self, cx: &mut Context<Self>) {
        let pane_focus = self.state.active_pane_focus();
        if self.state.pane_view(pane_focus).renaming_index.is_some() {
            self.commit_rename(cx);
        }
        {
            let view = self.state.pane_view_mut(pane_focus);
            view.selected_indices.clear();
            view.selection_anchor_index = None;
        }
        self.state.main_view.preview_text = None;
        self.sync_pane_selection_model(pane_focus, cx);
        cx.notify();
    }

    /// Invert selection among visible items in the active pane.
    pub fn invert_selection(&mut self, cx: &mut Context<Self>) {
        let pane_focus = self.state.active_pane_focus();
        {
            let view = self.state.pane_view_mut(pane_focus);
            let visible = view.visual_order_items();
            let prev = view.selected_indices.clone();
            view.selected_indices.clear();
            for i in visible {
                if !prev.contains(&i) {
                    view.selected_indices.insert(i);
                }
            }
        }
        self.sync_pane_selection_model(pane_focus, cx);
        cx.notify();
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Column resize handling (Details view column header drag-to-resize)
    // ─────────────────────────────────────────────────────────────────────────

    /// Start a column resize operation. Called from resize handle on_mouse_down.
    pub fn start_column_resize(&mut self, column: String, start_x: f32, _cx: &mut Context<Self>) {
        let pane_focus = self.state.active_pane_focus();
        let widths = self.state.pane_view(pane_focus).column_widths;
        let initial_width = match column.as_str() {
            "name" => widths.name,
            "date" => widths.date,
            "type" => widths.type_col,
            "size" => widths.size,
            "miller_col1" => widths.miller_col1,
            "miller_col2" => widths.miller_col2,
            "miller_col3" => widths.miller_col3,
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
            let pane_focus = self.state.active_pane_focus();
            let zoom = self.state.pane_view(pane_focus).zoom_level.clamp(0.0, 1.0);
            let zoom_scale = 0.80 + zoom * 0.55;
            let delta = current_x - resize.start_x;
            let is_miller = resize.column.starts_with("miller_");
            let scaled_delta = if is_miller { delta / zoom_scale } else { delta };
            let min_w = if is_miller { 120.0 } else { 40.0 };
            let new_width = (resize.initial_width + scaled_delta).max(min_w);

            for is_sec in [false, true] {
                let cw = if is_sec {
                    &mut self.state.secondary_view.column_widths
                } else {
                    &mut self.state.main_view.column_widths
                };
                match resize.column.as_str() {
                    "name" => cw.name = new_width,
                    "date" => cw.date = new_width,
                    "type" => cw.type_col = new_width,
                    "size" => cw.size = new_width,
                    "miller_col1" => cw.miller_col1 = new_width.clamp(120.0, 640.0),
                    "miller_col2" => cw.miller_col2 = new_width.clamp(150.0, 800.0),
                    "miller_col3" => cw.miller_col3 = new_width.clamp(160.0, 900.0),
                    _ => {}
                }
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
    /// Handle left mouse-down on an item row/card/tile across all 5 views:
    /// - Double-click opens the file or navigates into the directory.
    /// - Single-click selects the item (preserving multi-selection if dragging an already-selected group)
    ///   and records a drag candidate so moving >= 6px starts an internal/external file drag!
    pub fn handle_item_mouse_down(
        &mut self,
        pane_focus: crate::core::navigation_manager::PaneFocus,
        idx: usize,
        path: FilesPath,
        is_dir: bool,
        click_count: usize,
        pos: gpui::Point<f32>,
        modifiers: gpui::Modifiers,
        cx: &mut Context<Self>,
    ) {
        self.focus_pane(pane_focus, cx);
        if self.state.pane_view(pane_focus).renaming_index.is_some() {
            self.commit_rename(cx);
        }
        if click_count == 2 {
            self.state.main_view.drag_state.pending_candidate = None;
            if is_dir {
                self.navigate_pane(path, pane_focus, cx);
            } else {
                self.open_item_in_pane(pane_focus, idx, cx);
            }
            return;
        }

        let already_in_multi_selection = {
            let pv = self.state.pane_view(pane_focus);
            pv.selected_indices.len() > 1
                && pv.selected_indices.contains(&idx)
                && !modifiers.shift
                && !modifiers.control
        };

        if !already_in_multi_selection {
            self.select_item_in_pane(pane_focus, idx, modifiers.shift, modifiers.control, cx);
        } else {
            self.state.pane_view_mut(pane_focus).last_clicked_index = Some(idx);
        }

        // Record drag candidate on main_view.drag_state
        self.state
            .main_view
            .record_drag_candidate(pane_focus, idx, pos);
    }

    /// Update global or item-level file drag on mouse move.
    /// Promotes `pending_candidate` into an active `DragState` once movement exceeds 6px,
    /// highlights hovered folder targets, and triggers native OS drag-and-drop (`SHDoDragDrop` / `CF_HDROP`
    /// on Windows, `dragon-drop` / `text/uri-list` on Linux/macOS) when dragging out of the window!
    pub fn update_file_drag_move(
        &mut self,
        pos: gpui::Point<f32>,
        modifiers: gpui::Modifiers,
        hovered_folder: Option<(FilesPath, String)>,
        cx: &mut Context<Self>,
    ) {
        // 1. Check if a pending candidate crosses the 6px drag threshold
        if !self.state.main_view.drag_state.is_dragging {
            if let Some((src_pane, anchor_idx, start_pos)) =
                self.state.main_view.drag_state.pending_candidate
            {
                let dist = (pos.x - start_pos.x).hypot(pos.y - start_pos.y);
                if dist >= crate::ui::views::main_view_selection::FILE_DRAG_THRESHOLD_PX {
                    let (indices, paths) = {
                        let pv = self.state.pane_view(src_pane);
                        let mut idxs: Vec<usize> = pv.selected_indices.iter().copied().collect();
                        if !idxs.contains(&anchor_idx) {
                            idxs.push(anchor_idx);
                        }
                        idxs.sort_unstable();
                        let ps: Vec<FilesPath> = idxs
                            .iter()
                            .filter_map(|&i| pv.items.get(i))
                            .map(|it| it.path.clone())
                            .collect();
                        (idxs, ps)
                    };
                    if !paths.is_empty() {
                        self.state.main_view.drag_state = crate::ui::views::types::DragState {
                            indices,
                            paths,
                            source_pane: src_pane,
                            start: start_pos,
                            current: pos,
                            is_copy: modifiers.control,
                            is_link: modifiers.alt,
                            is_dragging: true,
                            pending_candidate: None,
                            pending_path_candidate: None,
                            hovered_target_folder: None,
                            hovered_target_label: None,
                            os_drag_dispatched: false,
                        };
                    }
                }
            } else if let Some((src_pane, cand_path, start_pos)) =
                self.state.main_view.drag_state.pending_path_candidate.clone()
            {
                let dist = (pos.x - start_pos.x).hypot(pos.y - start_pos.y);
                if dist >= crate::ui::views::main_view_selection::FILE_DRAG_THRESHOLD_PX {
                    self.state.main_view.drag_state = crate::ui::views::types::DragState {
                        indices: Vec::new(),
                        paths: vec![cand_path],
                        source_pane: src_pane,
                        start: start_pos,
                        current: pos,
                        is_copy: modifiers.control,
                        is_link: modifiers.alt,
                        is_dragging: true,
                        pending_candidate: None,
                        pending_path_candidate: None,
                        hovered_target_folder: None,
                        hovered_target_label: None,
                        os_drag_dispatched: false,
                    };
                }
            }
        }

        if !self.state.main_view.drag_state.is_dragging {
            return;
        }

        self.state.main_view.drag_state.current = pos;
        self.state.main_view.drag_state.is_link = modifiers.alt;

        // Filter out hovered_folder if it is one of the dragged folders itself
        let valid_target = hovered_folder.as_ref().filter(|(target_path, _)| {
            !self
                .state
                .main_view
                .drag_state
                .paths
                .iter()
                .any(|p| p == target_path)
        });

        // Default to Copy if Ctrl is held OR if target is on a different drive root (unless Shift held)
        let mut is_copy = modifiers.control;
        if let Some((target_path, target_label)) = valid_target {
            if !modifiers.shift {
                if let Some(first_src) = self.state.main_view.drag_state.paths.first() {
                    let src_root = first_src.to_string_lossy();
                    let dst_root = target_path.to_string_lossy();
                    if src_root.get(0..2) != dst_root.get(0..2) {
                        is_copy = true;
                    }
                }
            }
            self.state.main_view.drag_state.hovered_target_folder = Some(target_path.clone());
            self.state.main_view.drag_state.hovered_target_label = Some(target_label.clone());
        } else if hovered_folder.is_none() {
            // Keep existing hovered target if updated from item, only clear when
            // over the file-view background. Moves bubbling up from sidebar rows
            // must never wipe a target a row just set (same-event ordering).
            let content_left = if self.state.sidebar_visible {
                self.state.sidebar.width
            } else {
                0.0
            };
            if self.state.main_view.drag_state.current.x >= content_left {
                self.state.main_view.drag_state.hovered_target_folder = None;
                self.state.main_view.drag_state.hovered_target_label = None;
            }
        }
        self.state.main_view.drag_state.is_copy = is_copy;

        // 2. Check if cursor is dragging out of the window boundary -> launch native OS drag-and-drop!
        let win_w = f32::from(self.state.last_window_size.width).max(400.0);
        let win_h = f32::from(self.state.last_window_size.height).max(300.0);
        let near_outside = pos.x <= 8.0
            || pos.y <= 8.0
            || pos.x >= win_w - 8.0
            || pos.y >= win_h - 8.0;
        if near_outside && !self.state.main_view.drag_state.os_drag_dispatched {
            self.dispatch_native_os_file_drag(cx);
            return;
        }

        cx.notify();
    }

    /// Defer native OS drag-and-drop (`SHDoDragDrop`) onto the next main-thread message loop turn
    /// so all GPUI `AppCell`, `Window`, and `WindowsWindowInner` borrows have completely unwound
    /// before the modal OLE drag loop runs.
    pub fn dispatch_native_os_file_drag(&mut self, cx: &mut Context<Self>) {
        if self.state.main_view.drag_state.os_drag_dispatched {
            return;
        }
        let paths = self.state.main_view.drag_state.paths.clone();
        if paths.is_empty() {
            return;
        }
        let is_copy = self.state.main_view.drag_state.is_copy;
        let hwnd = self.state.window_hwnd;

        self.state.main_view.drag_state = crate::ui::views::types::DragState::default();
        cx.notify();

        cx.spawn(async move |this, cx| {
            let dropped = crate::platform::windows::shell::start_native_os_file_drag(
                &paths, is_copy, hwnd,
            );
            let elevated = !dropped && crate::platform::windows::shell::is_process_elevated();
            let _ = this.update(cx, |this, cx| {
                if dropped {
                    this.refresh(cx);
                } else {
                    if elevated {
                        this.state.status_center.push_notification(
                            "Drag & Drop Blocked by Admin Mode".to_string(),
                            "Files is running as Administrator. Windows UIPI blocks drag-and-drop to standard apps (Explorer, Taskbar, Browsers). Restart without Admin privileges.".to_string(),
                            crate::ui::components::status_center::ToastSeverity::Warning,
                            None,
                        );
                    }
                    cx.notify();
                }
            });
        })
        .detach();
    }

    /// Execute a file/folder transfer (Copy, Move, or Shortcut) from drag-and-drop or external drop.
    pub fn execute_drag_drop_transfer(
        &mut self,
        sources: Vec<std::path::PathBuf>,
        dest_dir: std::path::PathBuf,
        is_copy: bool,
        is_link: bool,
        cx: &mut Context<Self>,
    ) {
        let filtered_sources: Vec<std::path::PathBuf> = sources
            .into_iter()
            .filter(|src| src != &dest_dir && src.parent() != Some(dest_dir.as_path()))
            .collect();
        if filtered_sources.is_empty() || !dest_dir.is_dir() {
            return;
        }

        if is_link {
            // Create shortcuts (.lnk on Windows, symlink on Unix) in dest_dir
            for src in &filtered_sources {
                if let Some(name) = src.file_name() {
                    let link_dest = dest_dir.join(name);
                    #[cfg(unix)]
                    {
                        let _ = std::os::unix::fs::symlink(src, &link_dest);
                    }
                    #[cfg(windows)]
                    {
                        let lnk_path = dest_dir.join(format!("{}.lnk", name.to_string_lossy()));
                        let _ = crate::core::shortcut_service::ShortcutService::create_shortcut(
                            crate::core::shortcut_service::ShortcutOptions::new(
                                FilesPath::new(src),
                                FilesPath::new(lnk_path),
                            ),
                        );
                    }
                }
            }
            self.refresh(cx);
            return;
        }

        let progress_engine = self.state.progress_engine.clone();
        let this_weak = cx.weak_entity();
        let mut async_cx = cx.to_async();
        let total = filtered_sources.len();
        let dest_name = dest_dir
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| dest_dir.to_string_lossy().to_string());

        self.state.status_center.push_notification(
            format!(
                "{} {} item{}",
                if is_copy { "Copying" } else { "Moving" },
                total,
                if total == 1 { "" } else { "s" }
            ),
            format!("Destination: {}", dest_name),
            crate::ui::components::status_center::ToastSeverity::Info,
            None,
        );

        cx.foreground_executor()
            .spawn(async move {
                use crate::core::progress::OpKind;
                use crate::core::transfer::{run_copy_job_rt, run_move_job_rt, TransferOutcome};
                let (op_label, kind) = if is_copy {
                    ("Copy", OpKind::Copy)
                } else {
                    ("Move", OpKind::Move)
                };
                let job_id = progress_engine
                    .start_job(format!("{} ({} items)", op_label, total), kind)
                    .await;
                let outcome = if is_copy {
                    run_copy_job_rt(progress_engine.clone(), job_id, filtered_sources, dest_dir)
                        .await
                } else {
                    run_move_job_rt(progress_engine.clone(), job_id, filtered_sources, dest_dir)
                        .await
                };
                match outcome {
                    TransferOutcome::Done => progress_engine.complete_job(job_id).await,
                    TransferOutcome::Cancelled => {}
                    TransferOutcome::Failed(reason) => {
                        progress_engine.fail_job(job_id, reason).await
                    }
                }
                let _ = this_weak.update(&mut async_cx, |this, cx| {
                    this.refresh(cx);
                });
            })
            .detach();
    }

    /// Handle external files dropped from another OS application (`gpui::ExternalPaths`) into `files-rs`.
    pub fn handle_external_paths_drop(
        &mut self,
        external_paths: &[std::path::PathBuf],
        target_folder_override: Option<FilesPath>,
        modifiers: gpui::Modifiers,
        cx: &mut Context<Self>,
    ) {
        if external_paths.is_empty() {
            return;
        }
        let pane_focus = self.state.active_pane_focus();
        let dest_dir = target_folder_override
            .or_else(|| self.state.main_view.drag_state.hovered_target_folder.clone())
            .unwrap_or_else(|| self.state.pane_view(pane_focus).primary_path.clone())
            .as_path()
            .to_path_buf();

        let is_copy = !modifiers.shift; // External drops default to Copy unless Shift is held
        let is_link = modifiers.alt;
        self.execute_drag_drop_transfer(external_paths.to_vec(), dest_dir, is_copy, is_link, cx);
    }

    pub fn start_drag(&mut self, x: f32, y: f32, cx: &mut Context<Self>) {
        let indices: Vec<usize> = self.state.main_view.selected_indices.iter().copied().collect();
        if indices.is_empty() {
            return;
        }
        use gpui::Point;
        self.state.main_view.drag_start(indices, Point::new(x, y));
        cx.notify();
    }

    pub fn update_drag(&mut self, x: f32, y: f32, is_copy: bool, cx: &mut Context<Self>) {
        use gpui::Point;
        self.state.main_view.drag_update(Point::new(x, y), is_copy);
        cx.notify();
    }

    pub fn end_drag(
        &mut self,
        x: f32,
        y: f32,
        explicit_target: Option<(FilesPath, String)>,
        cx: &mut Context<Self>,
    ) {
        use gpui::Point;
        tracing::info!(
            "drop: end_drag x={} explicit={:?} hovered={:?}",
            x,
            explicit_target.as_ref().map(|(p, _)| p.to_string_lossy().to_string()),
            self.state.main_view.drag_state.hovered_target_folder.as_ref().map(|p| p.to_string_lossy().to_string())
        );
        let point = Point::new(x, y);
        self.state.main_view.drag_state.pending_candidate = None;
        self.state.main_view.drag_state.pending_path_candidate = None;

        if !self.state.main_view.drag_state.is_dragging {
            return;
        }

        let is_copy = self.state.main_view.drag_state.is_copy;
        let is_link = self.state.main_view.drag_state.is_link;
        let src_pane = self.state.main_view.drag_state.source_pane;
        let sources: Vec<std::path::PathBuf> = self
            .state
            .main_view
            .drag_state
            .paths
            .iter()
            .map(|p| p.as_path().to_path_buf())
            .collect();

        // 1. Check if hovering a specific target folder (in any view, sidebar, or breadcrumb)
        // An explicit target (e.g. the sidebar row under the cursor at release)
        // wins over hover history, which may never have been set.
        let mut target_dir: Option<std::path::PathBuf> = explicit_target
            .as_ref()
            .map(|(p, _)| p.as_path().to_path_buf())
            .or_else(|| {
                self.state
                    .main_view
                    .drag_state
                    .hovered_target_folder
                    .as_ref()
                    .map(|p| p.as_path().to_path_buf())
            });

        // 2. Fallback to geometric drop target check — only inside the file-view
        // content area. A release over the sidebar (or any other chrome) with no
        // explicitly hovered folder must NEVER resolve to a guessed folder;
        // that misfire moves files to unexpected locations.
        if target_dir.is_none() {
            let content_left = if self.state.sidebar_visible {
                self.state.sidebar.width
            } else {
                0.0
            };
            if x >= content_left {
                let visible = self.state.main_view.visible_items_rc();
                if let Some(target_idx) = self.state.main_view.get_drop_target(point, &visible) {
                    if let Some(target_item) = self.state.main_view.items.get(target_idx) {
                        target_dir = Some(target_item.path.as_path().to_path_buf());
                    }
                }
            }
        }

        // 3. Check if dropped into the opposite pane in Dual-Pane mode
        if target_dir.is_none()
            && self.state.main_view.pane_type == crate::ui::views::main_view::PaneType::Dual
        {
            let win_w = f32::from(self.state.last_window_size.width).max(800.0);
            let sidebar_w = if self.state.sidebar_visible {
                self.state.sidebar.width
            } else {
                0.0
            };
            let split_x =
                sidebar_w + (win_w - sidebar_w) * self.state.main_view.dual_pane_ratio.clamp(0.2, 0.8);
            if src_pane == crate::core::navigation_manager::PaneFocus::Primary && x > split_x {
                target_dir = Some(self.state.secondary_view.primary_path.as_path().to_path_buf());
            } else if src_pane == crate::core::navigation_manager::PaneFocus::Secondary
                && x > sidebar_w
                && x < split_x
            {
                target_dir = Some(self.state.main_view.primary_path.as_path().to_path_buf());
            }
        }

        // 4. Check if dropped onto the Bottom Shelf Pane
        let win_h = f32::from(self.state.last_window_size.height).max(600.0);
        if target_dir.is_none() && self.state.main_view.shelf_visible && y >= win_h - 215.0 {
            for p in &self.state.main_view.drag_state.paths.clone() {
                if !self.state.main_view.shelf_items.iter().any(|s| &s.path == p) {
                    let meta = std::fs::metadata(p.as_path()).ok();
                    let is_dir = meta.as_ref().map(|m| m.is_dir()).unwrap_or(false);
                    let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
                    let name = p
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| p.to_string_lossy().to_string());
                    self.state
                        .main_view
                        .shelf_items
                        .push(crate::ui::views::types::ShelfItem {
                            path: p.clone(),
                            name,
                            is_dir,
                            size,
                        });
                }
            }
            self.state.main_view.drag_end();
            cx.notify();
            return;
        }

        self.state.main_view.drag_end();

        if let Some(dest_dir) = target_dir {
            tracing::info!(
                "drag-drop: {} item(s) -> {} (copy={}, link={})",
                sources.len(),
                dest_dir.display(),
                is_copy,
                is_link
            );
            self.execute_drag_drop_transfer(sources, dest_dir, is_copy, is_link, cx);
        } else {
            tracing::info!("drag-drop: no target, drop cancelled");
        }
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
    pub fn toggle_selection_checkboxes(&mut self, cx: &mut Context<Self>) {
        use crate::ui::views::main_view::SelectionMode;
        let new_mode = match self.state.main_view.selection_mode {
            SelectionMode::Checkbox => SelectionMode::Normal,
            _ => SelectionMode::Checkbox,
        };
        self.state.main_view.selection_mode = new_mode.clone();
        self.state.secondary_view.selection_mode = new_mode;
        let enabled = matches!(
            self.state.main_view.selection_mode,
            SelectionMode::Checkbox
        );
        let settings = self.state.settings.clone();
        settings.update(cx, move |s, _cx| {
            s.config.preferences.show_checkboxes_when_selecting = enabled;
            let _ = s.config.save();
        });
        cx.notify();
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
        let pane_focus = self.state.active_pane_focus();
        if matches!(
            self.state.pane_view(pane_focus).selection_mode,
            crate::ui::views::main_view::SelectionMode::HoverSelect
        ) {
            {
                let view = self.state.pane_view_mut(pane_focus);
                view.selected_indices.clear();
                view.selected_indices.insert(index);
            }
            self.sync_pane_selection_model(pane_focus, cx);
            cx.notify();
        }
    }

    /// Toggle checkbox selection for an item
    pub fn toggle_checkbox_selection(&mut self, index: usize, cx: &mut Context<Self>) {
        let pane_focus = self.state.active_pane_focus();
        {
            let view = self.state.pane_view_mut(pane_focus);
            if view.selected_indices.contains(&index) {
                view.selected_indices.remove(&index);
            } else {
                view.selected_indices.insert(index);
            }
        }
        self.sync_pane_selection_model(pane_focus, cx);
        cx.notify();
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

    /// Update the details pane cache asynchronously on the background executor (zero UI thread blocking).
    pub(crate) fn update_details_cache_from_path(&mut self, path: &FilesPath, cx: &mut Context<Self>) {
        let mut cache = DetailsPaneCache::default();
        cache.tags = self
            .state
            .main_view
            .tags_cache
            .get(&path.to_string_lossy().to_string())
            .cloned()
            .unwrap_or_default();
        cache.folder_subfolders = self.state.main_view.details_cache.folder_subfolders;
        cache.folder_files = self.state.main_view.details_cache.folder_files;
        cache.folder_total_size = self.state.main_view.details_cache.folder_total_size;
        self.state.main_view.details_cache = cache;

        let req_id = self.state.main_view.preview_request_id;
        let path_clone = path.clone();
        let mut async_cx = cx.to_async();
        let this_weak = cx.weak_entity();

        let bg = cx.background_executor().spawn(async move {
            let ext_props = ExtendedMetadataExtractor::extract(&path_clone).ok();
            let commit_info = crate::domain::git_service::GitService::get_last_commit(&path_clone);
            let (mime_type, _) = crate::storage::preview_service::detect_mime_and_extension(&path_clone);
            (ext_props, commit_info, mime_type)
        });

        cx.foreground_executor()
            .spawn(async move {
                let (ext_props, commit_info, mime_type) = bg.await;
                this_weak
                    .update(&mut async_cx, |this, cx| {
                        if this.state.main_view.preview_request_id != req_id {
                            return;
                        }
                        this.state.main_view.details_cache.mime_type = Some(mime_type);
                        if let Some(props) = ext_props {
                            if let Some((w, h)) = props.dimensions {
                                this.state.main_view.details_cache.exif_dimensions =
                                    Some(format!("{} × {}", w, h));
                            }
                            this.state.main_view.details_cache.exif_camera = props.camera_model;
                            this.state.main_view.details_cache.exif_date = props.exif_date;
                            this.state.main_view.details_cache.exif_iso = props.iso.map(|v| format!("ISO {}", v));
                            this.state.main_view.details_cache.exif_aperture = props.aperture;
                            this.state.main_view.details_cache.exif_exposure = props.exposure_time;
                            this.state.main_view.details_cache.exif_focal_length = props.focal_length;
                            this.state.main_view.details_cache.exif_lens = props.lens_model;
                            if let Some(dur) = props.duration {
                                let total_secs = dur.as_secs();
                                let mins = total_secs / 60;
                                let secs = total_secs % 60;
                                let fmt_dur = format!("{}:{:02}", mins, secs);
                                this.state.main_view.details_cache.audio_duration =
                                    Some(fmt_dur.clone());
                                this.state.main_view.details_cache.video_duration = Some(fmt_dur);
                            }
                        }
                        if let Some(commit) = commit_info {
                            this.state.main_view.details_cache.git_commit_author =
                                Some(commit.author);
                            this.state.main_view.details_cache.git_commit_date = Some(commit.date);
                            this.state.main_view.details_cache.git_commit_message =
                                Some(commit.message);
                        }
                        cx.notify();
                    })
                    .ok();
            })
            .detach();
    }

    // ═══════════════════════════════════════════════════════════════
    // OMNIBAR, BREADCRUMB & HIGH-SPEED SEARCH METHODS
    // ═══════════════════════════════════════════════════════════════

    /// Activate Omnibar Path Edit Mode (`Ctrl+L` / `Alt+D` / clicking address bar).
    pub fn activate_omnibar_path_mode(&mut self, cx: &mut Context<Self>) {
        use crate::ui::components::navigation_bar::OmnibarMode;
        self.state.nav_bar.omnibar_mode = OmnibarMode::Path;
        self.state.nav_bar.edit_mode = true;
        let path_str = self
            .state
            .nav_bar
            .current_path
            .to_string_lossy()
            .to_string();
        self.state.nav_bar.set_active_text(path_str, true);
        self.state.nav_bar.show_suggestions = true;
        self.state.nav_bar.selected_suggestion_index = None;
        self.refresh_omnibar_suggestions(cx);
    }

    /// Activate Omnibar Command Palette Mode (`Ctrl+Shift+P` / `F1` / `>` prefix).
    pub fn activate_omnibar_command_mode(&mut self, cx: &mut Context<Self>) {
        use crate::ui::components::navigation_bar::OmnibarMode;
        self.state.nav_bar.omnibar_mode = OmnibarMode::CommandPalette;
        self.state.nav_bar.edit_mode = false;
        self.state.nav_bar.set_active_text(String::new(), false);
        self.state.nav_bar.show_suggestions = true;
        self.state.nav_bar.selected_suggestion_index = Some(0);
        self.refresh_omnibar_suggestions(cx);
    }

    /// Activate Omnibar Search Mode (`Ctrl+F` / `?` prefix).
    pub fn activate_omnibar_search_mode(&mut self, cx: &mut Context<Self>) {
        use crate::ui::components::navigation_bar::OmnibarMode;
        self.state.nav_bar.omnibar_mode = OmnibarMode::Search;
        self.state.nav_bar.edit_mode = false;
        let existing = self.state.nav_bar.search_query.clone();
        self.state.nav_bar.set_active_text(existing, true);
        self.state.nav_bar.show_suggestions = true;
        self.state.nav_bar.selected_suggestion_index = None;
        self.state.main_view.search_state.is_searching = true;
        self.refresh_omnibar_suggestions(cx);
    }

    /// Refresh Omnibar suggestions and FilePilot inline ghost completion using live folder items & tags.
    pub fn refresh_omnibar_suggestions(&mut self, cx: &mut Context<Self>) {
        let registry = self.state.action_registry.clone();
        // Prefer pre_search_items when available so current-folder completions stay complete during deep search
        let items_slice: Vec<FileItem> = self
            .state
            .main_view
            .search_state
            .pre_search_items
            .clone()
            .unwrap_or_else(|| self.state.main_view.items.clone());
        let tags_cache = self.state.main_view.tags_cache.clone();
        self.state
            .nav_bar
            .update_suggestions_with_context(&registry, &items_slice, &tags_cache);
        cx.notify();
    }

    /// Live-preview a suggestion in the Omnibar text input when navigating suggestions with Up/Down arrows.
    pub fn preview_selected_suggestion(&mut self, index: usize) {
        use crate::ui::components::navigation_bar::SuggestionKind;
        if let Some(sug) = self.state.nav_bar.suggestions.get(index).cloned() {
            match sug.kind {
                SuggestionKind::Path { path } => {
                    self.state
                        .nav_bar
                        .preview_suggestion_text(path.to_string_lossy().to_string());
                }
                SuggestionKind::SearchHistory { query } => {
                    self.state.nav_bar.preview_suggestion_text(query);
                }
                _ => {}
            }
            self.state.nav_bar.ensure_suggestion_visible(index);
        }
    }

    /// Apply an AQS filter token (`kind:image `, `size:>10mb `, etc.) into the Search Mode query.
    pub fn apply_aqs_filter_suggestion(&mut self, token: &str, cx: &mut Context<Self>) {
        let cur = self.state.nav_bar.search_query.trim_end().to_string();
        let updated = if let Some(last_space) = cur.rfind(' ') {
            let last_tok = &cur[last_space + 1..];
            if last_tok.contains(':') {
                format!("{} {}", &cur[..last_space], token)
            } else {
                format!("{} {}", cur, token)
            }
        } else if cur.contains(':') || cur.is_empty() {
            token.to_string()
        } else {
            format!("{} {}", cur, token)
        };
        self.state.nav_bar.set_active_text(updated.clone(), false);
        self.state.nav_bar.selected_suggestion_index = None;
        self.update_search(updated, cx);
        self.refresh_omnibar_suggestions(cx);
    }

    /// Toggle a search option pill (`regex`, `case`, `content`, `metadata`, `recursive`) and re-run search.
    pub fn toggle_search_option(&mut self, which: &str, cx: &mut Context<Self>) {
        match which {
            "regex" => {
                self.state.nav_bar.search_options.use_regex =
                    !self.state.nav_bar.search_options.use_regex;
            }
            "case" => {
                self.state.nav_bar.search_options.case_sensitive =
                    !self.state.nav_bar.search_options.case_sensitive;
            }
            "content" => {
                self.state.nav_bar.search_options.search_content =
                    !self.state.nav_bar.search_options.search_content;
            }
            "metadata" => {
                self.state.nav_bar.search_options.search_metadata =
                    !self.state.nav_bar.search_options.search_metadata;
            }
            "recursive" => {
                self.state.nav_bar.search_options.recursive =
                    !self.state.nav_bar.search_options.recursive;
            }
            _ => {}
        }
        // Sync options to SearchState
        let opts = self.state.nav_bar.search_options;
        self.state.main_view.search_state.use_regex = opts.use_regex;
        self.state.main_view.search_state.case_sensitive = opts.case_sensitive;
        self.state.main_view.search_state.search_content = opts.search_content;
        self.state.main_view.search_state.search_metadata = opts.search_metadata;
        self.state.main_view.search_state.recursive = opts.recursive;

        let q = self.state.nav_bar.search_query.clone();
        if !q.trim().is_empty() {
            self.start_search(q, cx);
        }
        self.refresh_omnibar_suggestions(cx);
    }

    /// Open `path` in a new tab immediately (used by middle-click / Ctrl+Click on breadcrumbs & suggestions).
    pub fn open_path_in_new_tab(&mut self, path: FilesPath, cx: &mut Context<Self>) {
        let dir = if path.as_path().is_file() {
            path.parent().unwrap_or_else(|| path.clone())
        } else {
            path
        };
        self.open_in_new_tab(dir, cx);
    }

    /// Copy a breadcrumb segment path to the system clipboard and push a non-intrusive toast confirmation.
    pub fn copy_breadcrumb_path(&mut self, path: FilesPath, cx: &mut Context<Self>) {
        let s = path.to_string_lossy().to_string();
        cx.write_to_clipboard(gpui::ClipboardItem::new_string(s.clone()));
        self.state.status_center.push_notification(
            "Path copied to clipboard",
            s,
            crate::ui::components::status_center::NotificationSeverity::Info,
            Some(path),
        );
        cx.notify();
    }

    /// Handle dropping dragged files/folders onto a breadcrumb segment (`BreadcrumbBarItem_Drop` parity).
    pub fn handle_file_drop_on_breadcrumb(
        &mut self,
        target_folder: FilesPath,
        cx: &mut Context<Self>,
    ) {
        self.state.nav_bar.file_drag_over_crumb = None;
        let is_copy = self.state.main_view.drag_state.is_copy;
        let indices = self.state.main_view.drag_state.indices.clone();
        let sources: Vec<std::path::PathBuf> = indices
            .iter()
            .filter_map(|&idx| self.state.main_view.items.get(idx))
            .map(|it| it.path.as_path().to_path_buf())
            .collect();

        self.state.main_view.drag_end();
        let dest_dir = target_folder.as_path().to_path_buf();
        if sources.is_empty() || !dest_dir.is_dir() {
            cx.notify();
            return;
        }

        let progress_engine = self.state.progress_engine.clone();
        let this_weak = cx.weak_entity();
        let mut async_cx = cx.to_async();
        let total = sources.len();
        let dest_fp = target_folder.clone();

        cx.foreground_executor()
            .spawn(async move {
                use crate::core::progress::OpKind;
                use crate::core::transfer::{run_copy_job_rt, run_move_job_rt, TransferOutcome};
                let (op_label, kind) = if is_copy {
                    ("Copy", OpKind::Copy)
                } else {
                    ("Move", OpKind::Move)
                };
                let job_id = progress_engine
                    .start_job_with_meta(
                        format!("{} ({} items)", op_label, total),
                        kind,
                        Some(dest_fp),
                        total as u64,
                    )
                    .await;
                let outcome = if is_copy {
                    run_copy_job_rt(progress_engine.clone(), job_id, sources, dest_dir).await
                } else {
                    run_move_job_rt(progress_engine.clone(), job_id, sources, dest_dir).await
                };
                match outcome {
                    TransferOutcome::Done => progress_engine.complete_job(job_id).await,
                    TransferOutcome::Cancelled => {}
                    TransferOutcome::Failed(reason) => {
                        progress_engine.fail_job(job_id, reason).await
                    }
                }
                let _ = this_weak.update(&mut async_cx, |this, cx| {
                    this.refresh(cx);
                });
            })
            .detach();
        cx.notify();
    }

    /// Jump multiple steps in the Back or Forward navigation history stack (`BackHistoryFlyout` / `ForwardHistoryFlyout`).
    pub fn jump_history_steps(
        &mut self,
        is_back: bool,
        steps: usize,
        _target: FilesPath,
        cx: &mut Context<Self>,
    ) {
        let mut final_path = None;
        self.state.navigation.update(cx, |nav, _cx| {
            let tab = nav.tabs.active_tab_mut();
            for _ in 0..steps {
                let stepped = if is_back {
                    tab.navigate_back()
                } else {
                    tab.navigate_forward()
                };
                if let Some(p) = stepped {
                    final_path = Some(p);
                } else {
                    break;
                }
            }
        });
        if let Some(dest) = final_path {
            self.save_session(cx);
            self.navigate(dest, cx);
        }
    }

    /// Start a global or local search with the given query, supporting:
    /// - Regex (`options.use_regex`, `re:...`, `/.../i`)
    /// - Windows Search Wildcard patterns (`*.rs;*.toml`, `doc_??.*`, `.pdf`)
    /// - AQS filters (`kind:`, `ext:`, `size:`, `date:`, `modified:`, `created:`, `tag:`, `is:`)
    /// - Full-Text Content Search (`options.search_content`, `content:...`)
    /// - Optional Full-Text Metadata Search (`options.search_metadata`, `meta:...`)
    pub fn start_search(&mut self, query: String, cx: &mut Context<Self>) {
        // Cancel any previous in-flight deep search immediately
        if let Some(token) = self.state.main_view.search_state.cancel_token.take() {
            token.store(true, Ordering::Relaxed);
        }

        if query.trim().is_empty() {
            self.clear_search(cx);
            return;
        }

        // Snapshot current folder items before any deep search replaces `main_view.items`
        if self.state.main_view.search_state.pre_search_items.is_none() {
            self.state.main_view.search_state.pre_search_items =
                Some(self.state.main_view.items.clone());
        } else if let Some(orig_items) = self
            .state
            .main_view
            .search_state
            .pre_search_items
            .clone()
        {
            // Restore original items before filtering in-memory for the new query
            self.state.main_view.items = orig_items.clone();
            self.state.main_view.bump_items_revision();
            let restored = orig_items.clone();
            self.state.file_items.update(cx, |m, _| {
                m.items = restored;
                m.selected_indices.clear();
            });
        }

        let mut opts = self.state.nav_bar.search_options;
        opts.include_hidden = self.state.main_view.show_hidden_files;

        self.state.main_view.search_state.is_searching = true;
        self.state.main_view.search_state.query = query.clone();
        self.state.main_view.search_state.use_regex = opts.use_regex;
        self.state.main_view.search_state.case_sensitive = opts.case_sensitive;
        self.state.main_view.search_state.search_content = opts.search_content;
        self.state.main_view.search_state.search_metadata = opts.search_metadata;
        self.state.main_view.search_state.recursive = opts.recursive;

        // 1. Immediate sub-millisecond evaluation of current directory items
        let tags_cache = &self.state.main_view.tags_cache;
        let mut matching_indices = std::collections::HashSet::new();
        for (idx, item) in self.state.main_view.items.iter().enumerate() {
            if crate::domain::search::SearchService::evaluate_file_item(
                item,
                &query,
                &opts,
                tags_cache,
            )
            .is_some()
            {
                matching_indices.insert(idx);
            }
        }
        self.state.main_view.search_state.results = matching_indices;
        self.state.main_view.bump_items_revision();

        // 2. If `recursive` is enabled and query is >= 2 chars (or has wildcard/AQS/regex),
        //    spawn a parallel streaming recursive search!
        let should_deep_search = opts.recursive
            && (query.trim().len() >= 2 || query.contains('*') || query.contains('?'));

        if should_deep_search {
            self.state.main_view.search_state.loading = true;
            let cancel_flag = Arc::new(AtomicBool::new(false));
            self.state.main_view.search_state.cancel_token = Some(cancel_flag.clone());
            let path_clone = self.state.main_view.primary_path.clone();
            let query_clone = query.clone();
            let tags_snapshot = self.state.main_view.tags_cache.clone();
            let weak_entity = cx.weak_entity();
            let mut async_cx = cx.to_async();

            cx.foreground_executor()
                .spawn(async move {
                    let cancel_for_bg = cancel_flag.clone();
                    let results = async_cx
                        .background_executor()
                        .spawn(async move {
                            Self::do_deep_search_with_options(
                                path_clone,
                                query_clone,
                                opts,
                                tags_snapshot,
                                cancel_for_bg,
                            )
                        })
                        .await;

                    if cancel_flag.load(Ordering::Relaxed) {
                        return;
                    }

                    let _ = weak_entity.update(&mut async_cx, |this, cx| {
                        if cancel_flag.load(Ordering::Relaxed) {
                            return;
                        }
                        this.state.main_view.search_state.deep_results = results.clone();

                        let file_items: Vec<FileItem> = results
                            .iter()
                            .map(|res| {
                                let md = std::fs::metadata(res.path.as_path()).ok();
                                let modified = md.as_ref().and_then(|m| m.modified().ok());
                                let created = md.as_ref().and_then(|m| m.created().ok());
                                FileItem {
                                    name: res.name.clone(),
                                    path: res.path.clone(),
                                    file_type: if res.is_dir {
                                        FileType::Directory
                                    } else {
                                        FileType::File
                                    },
                                    size: res.size,
                                    is_hidden: res.name.starts_with('.'),
                                    is_system: false,
                                    modified,
                                    created,
                                    tags: Vec::new(),
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
                        this.refresh_omnibar_suggestions(cx);
                        cx.notify();
                    });
                })
                .detach();
        } else {
            self.state.main_view.search_state.loading = false;
        }

        cx.notify();
    }

    /// Legacy wrapper preserved for compatibility.
    #[allow(dead_code)]
    fn do_deep_search(
        base_path: FilesPath,
        query: String,
        cancel_flag: Arc<AtomicBool>,
    ) -> Vec<crate::ui::views::main_view::SearchResult> {
        Self::do_deep_search_with_options(
            base_path,
            query,
            crate::domain::search::SearchOptions::default(),
            HashMap::new(),
            cancel_flag,
        )
    }

    /// Parallel multi-threaded deep recursive search powered by `jwalk::WalkDir` and `SearchService::evaluate_file_item`.
    fn do_deep_search_with_options(
        base_path: FilesPath,
        query: String,
        options: crate::domain::search::SearchOptions,
        tags_cache: HashMap<String, Vec<String>>,
        cancel_flag: Arc<AtomicBool>,
    ) -> Vec<crate::ui::views::main_view::SearchResult> {
        use crate::ui::views::main_view::SearchResult;

        let root = base_path.as_path();
        if !root.is_dir() {
            return Vec::new();
        }

        let mut results: Vec<SearchResult> = Vec::new();
        let max_results = 600usize;

        let walker = jwalk::WalkDir::new(root)
            .max_depth(8)
            .skip_hidden(!options.include_hidden)
            .follow_links(false);

        for entry_res in walker {
            if cancel_flag.load(Ordering::Relaxed) || results.len() >= max_results {
                break;
            }
            let Ok(entry) = entry_res else {
                continue;
            };
            if entry.depth == 0 {
                continue;
            }
            let path_buf = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            let is_dir = entry.file_type().is_dir();
            let md = entry.metadata().ok();
            let size = if is_dir {
                0
            } else {
                md.as_ref().map(|m| m.len()).unwrap_or(0)
            };
            let modified_time = md.as_ref().and_then(|m| m.modified().ok());
            let created_time = md.as_ref().and_then(|m| m.created().ok());

            let candidate_item = FileItem {
                name: name.clone(),
                path: FilesPath::from(path_buf.clone()),
                file_type: if is_dir {
                    FileType::Directory
                } else {
                    FileType::File
                },
                size,
                is_hidden: name.starts_with('.'),
                is_system: false,
                modified: modified_time,
                created: created_time,
                tags: Vec::new(),
            };

            if let Some((score, match_indices, match_snippet)) =
                crate::domain::search::SearchService::evaluate_file_item(
                    &candidate_item,
                    &query,
                    &options,
                    &tags_cache,
                )
            {
                let parent_path = path_buf
                    .parent()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default();
                let modified_str = modified_time.map(|t| {
                    crate::ui::views::helpers::format_system_time(Some(t))
                });
                results.push(SearchResult {
                    path: candidate_item.path,
                    name,
                    parent_path,
                    is_dir,
                    size,
                    modified: modified_str,
                    match_snippet,
                    match_indices,
                    score,
                });
            }
        }

        results.sort_by(|a, b| {
            b.score
                .cmp(&a.score)
                .then_with(|| b.is_dir.cmp(&a.is_dir))
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        results
    }

    /// Update the search query (called as user types)
    pub fn update_search(&mut self, query: String, cx: &mut Context<Self>) {
        self.start_search(query, cx);
    }

    /// Clear search and return to normal folder view in 0ms using `pre_search_items` snapshot.
    pub fn clear_search(&mut self, cx: &mut Context<Self>) {
        if let Some(token) = self.state.main_view.search_state.cancel_token.take() {
            token.store(true, Ordering::Relaxed);
        }
        let restored_items = self.state.main_view.search_state.pre_search_items.take();
        self.state.main_view.search_state = crate::ui::views::main_view::SearchState::default();

        if let Some(orig) = restored_items {
            self.state.main_view.items = orig.clone();
            self.state.main_view.bump_items_revision();
            self.state.file_items.update(cx, |m, _| {
                m.items = orig;
                m.selected_indices.clear();
            });
            cx.notify();
        } else {
            self.navigate(self.state.main_view.primary_path.clone(), cx);
            cx.notify();
        }
    }

    /// Confirm selected suggestion or raw query from the Omnibar (default no-modifier entry point).
    pub fn handle_suggestion_confirm(&mut self, cx: &mut Context<Self>) {
        self.handle_suggestion_confirm_with_modifiers(false, false, false, cx);
    }

    /// Confirm selected suggestion or raw query from the Omnibar with full modifier support:
    /// - `ctrl == true` (`Ctrl+Enter` / Middle-Click): Open target folder in a **New Tab**
    /// - `shift == true` (`Shift+Enter`): Open target folder in a **New Window**
    /// - `alt == true` (`Alt+Enter`): Open **Properties** for the target item
    pub fn handle_suggestion_confirm_with_modifiers(
        &mut self,
        ctrl: bool,
        shift: bool,
        alt: bool,
        cx: &mut Context<Self>,
    ) {
        use crate::ui::components::navigation_bar::{OmnibarMode, SuggestionKind};

        let execute_path_action =
            |this: &mut Self, path: FilesPath, ctrl: bool, shift: bool, alt: bool, cx: &mut Context<Self>| {
                this.state.nav_bar.edit_mode = false;
                this.state.nav_bar.show_suggestions = false;
                this.state.nav_bar.selected_suggestion_index = None;
                this.state.nav_bar.omnibar_mode = OmnibarMode::Path;

                if alt {
                    this.open_properties_for_paths(vec![path.as_path().to_path_buf()], cx);
                } else if shift {
                    let dir = if path.as_path().is_file() {
                        path.parent().unwrap_or_else(|| path.clone())
                    } else {
                        path
                    };
                    this.open_in_new_window(dir, cx);
                } else if ctrl {
                    this.open_path_in_new_tab(path, cx);
                } else if path.as_path().is_file() {
                    if let Some(parent) = path.parent() {
                        this.navigate(parent, cx);
                    } else {
                        this.navigate(path, cx);
                    }
                } else {
                    this.navigate(path, cx);
                }
            };

        if let Some(idx) = self.state.nav_bar.selected_suggestion_index {
            if let Some(suggestion) = self.state.nav_bar.suggestions.get(idx).cloned() {
                match suggestion.kind {
                    SuggestionKind::Path { path } => {
                        execute_path_action(self, path, ctrl, shift, alt, cx);
                    }
                    SuggestionKind::SearchItem { path, is_dir } => {
                        if !self.state.nav_bar.search_query.trim().is_empty() {
                            let q = self.state.nav_bar.search_query.clone();
                            self.state.nav_bar.record_search_history(&q);
                        }
                        if is_dir || ctrl || shift || alt {
                            execute_path_action(self, path, ctrl, shift, alt, cx);
                        } else {
                            execute_path_action(self, path, false, false, false, cx);
                        }
                    }
                    SuggestionKind::AqsFilter { token } => {
                        self.apply_aqs_filter_suggestion(&token, cx);
                    }
                    SuggestionKind::SearchHistory { query } => {
                        self.state.nav_bar.show_suggestions = false;
                        self.state.nav_bar.selected_suggestion_index = None;
                        self.state.nav_bar.record_search_history(&query);
                        self.state.nav_bar.set_active_text(query.clone(), false);
                        self.update_search(query, cx);
                    }
                    SuggestionKind::Command { action } => {
                        self.state.nav_bar.show_suggestions = false;
                        self.state.nav_bar.selected_suggestion_index = None;
                        self.state.nav_bar.omnibar_mode = OmnibarMode::Path;
                        self.state.nav_bar.record_command_used(action);
                        self.dispatch_action(action, cx);
                    }
                }
                return;
            }
        }

        // No suggestion explicitly highlighted: handle based on active Omnibar mode
        match self.state.nav_bar.omnibar_mode {
            OmnibarMode::Path => {
                let raw = self.state.nav_bar.edit_text.trim().to_string();
                if let Some(cmd_q) = raw.strip_prefix('>') {
                    self.state.nav_bar.omnibar_mode = OmnibarMode::CommandPalette;
                    self.state
                        .nav_bar
                        .set_active_text(cmd_q.trim().to_string(), false);
                    self.refresh_omnibar_suggestions(cx);
                    if let Some(SuggestionKind::Command { action }) =
                        self.state.nav_bar.suggestions.first().map(|s| s.kind.clone())
                    {
                        self.state.nav_bar.show_suggestions = false;
                        self.state.nav_bar.omnibar_mode = OmnibarMode::Path;
                        self.state.nav_bar.record_command_used(action);
                        self.dispatch_action(action, cx);
                    }
                    return;
                }
                if let Some(search_q) = raw.strip_prefix('?') {
                    let q = search_q.trim().to_string();
                    self.state.nav_bar.omnibar_mode = OmnibarMode::Search;
                    self.state.nav_bar.edit_mode = false;
                    self.state.nav_bar.set_active_text(q.clone(), false);
                    self.state.nav_bar.record_search_history(&q);
                    self.update_search(q, cx);
                    return;
                }

                let expanded = crate::domain::search::expand_path_input(&raw);
                self.state.nav_bar.edit_mode = false;
                self.state.nav_bar.show_suggestions = false;
                self.state.nav_bar.selected_suggestion_index = None;
                if !expanded.is_empty() {
                    if expanded.eq_ignore_ascii_case("settings") {
                        self.open_settings_tab(cx);
                    } else if expanded.eq_ignore_ascii_case("home") {
                        if let Ok(home_path) =
                            crate::core::navigation_service::NavigationService::get_home_path()
                        {
                            execute_path_action(self, home_path, ctrl, shift, alt, cx);
                        }
                    } else {
                        let path = FilesPath::from(std::path::PathBuf::from(&expanded));
                        execute_path_action(self, path, ctrl, shift, alt, cx);
                    }
                }
            }
            OmnibarMode::Search => {
                self.state.nav_bar.show_suggestions = false;
                self.state.nav_bar.selected_suggestion_index = None;
                let query = self.state.nav_bar.search_query.clone();
                self.state.nav_bar.record_search_history(&query);
                self.update_search(query, cx);
            }
            OmnibarMode::CommandPalette => {
                if let Some(suggestion) = self.state.nav_bar.suggestions.first().cloned() {
                    if let SuggestionKind::Command { action } = suggestion.kind {
                        self.state.nav_bar.show_suggestions = false;
                        self.state.nav_bar.selected_suggestion_index = None;
                        self.state.nav_bar.omnibar_mode = OmnibarMode::Path;
                        self.state.nav_bar.record_command_used(action);
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
    pub fn open_terminal_here(&mut self, path: &FilesPath, cx: &mut Context<Self>) {
        let terminal_type = {
            let settings = self.state.settings.read(cx);
            settings.config().terminal_app.clone()
        };

        if let Err(e) = crate::core::terminal_service::TerminalService::open_terminal(
            &terminal_type,
            path.as_path(),
        ) {
            tracing::error!("Failed to open terminal at {:?}: {}", path, e);
        }
    }

    /// Open a terminal as administrator at the specified path
    #[cfg(windows)]
    pub fn open_terminal_as_admin(&mut self, path: &FilesPath, _cx: &mut Context<Self>) {
        use std::os::windows::process::CommandExt;
        use std::process::Command;

        let working_dir = if path.as_path().is_dir() {
            path.as_path().to_path_buf()
        } else if let Some(home) = dirs::home_dir() {
            home
        } else {
            std::path::PathBuf::from("C:\\")
        };
        let path_str = working_dir.to_string_lossy();

        // Use PowerShell's Start-Process with -Verb RunAs to elevate
        let ps_command = format!(
            "Start-Process wt.exe -ArgumentList '-d \"{}\"' -Verb RunAs -ErrorAction SilentlyContinue; if (!$?) {{ Start-Process powershell.exe -ArgumentList '-NoExit','Set-Location -LiteralPath \\'{}\\' ' -Verb RunAs }}",
            path_str.replace('"', "\\\""),
            path_str.replace('\'', "''")
        );

        let mut cmd = Command::new("powershell");
        cmd.args(["-Command", &ps_command]);
        cmd.current_dir(&working_dir);
        cmd.creation_flags(0x00000010); // CREATE_NEW_CONSOLE
        cmd.stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        let _ = cmd.spawn();
    }

    #[cfg(not(windows))]
    pub fn open_terminal_as_admin(&mut self, _path: &FilesPath, _cx: &mut Context<Self>) {
        tracing::warn!("Open elevated terminal not implemented for this platform");
    }

    /// Open terminal at the current primary path
    pub fn open_terminal_at_current(&mut self, cx: &mut Context<Self>) {
        let path = self.state.main_view.primary_path.clone();
        self.open_terminal_here(&path, cx);
    }

    /// Load preview content for a file based on its type.
    /// Handles text, PDF, audio/video, and archive previews.

    /// Load text preview content for a file (existing behavior, limited to 10k chars)

    /// Load archive preview - list contents without extracting

    /// Check which third-party preview handler is available on this system.
    /// Returns: Peek (PowerToys), QuickLook, SeerPro, or None.

    /// Toggle the Preview pane.

    pub fn close_dialog(&mut self, cx: &mut Context<Self>) {
        self.state.status_center.is_open = false;
        self.state.active_dialog = None;
        self.menu_tip = None;
        self.submenu_grace = None;
        self.submenu_intent = None;
        self.submenu_panel_hovered = false;
        self.submenu_row_hovered = false;
        cx.notify();
    }

    pub fn confirm_add_item(&mut self, cx: &mut Context<Self>) {
        if let Some(DialogType::AddItem { is_folder, name, .. }) = self.state.active_dialog.clone() {
            let default_name = if is_folder { "New folder" } else { "New Text Document.txt" };
            let trimmed = name.trim();
            let final_name = if trimmed.is_empty() { default_name } else { trimmed };
            let mut target_path = self.state.main_view.primary_path.as_path().to_path_buf();
            target_path.push(final_name);
            if is_folder {
                let _ = std::fs::create_dir_all(&target_path);
            } else {
                let _ = std::fs::write(&target_path, b"");
            }
            // Refresh directory
            let current_path = self.state.main_view.primary_path.clone();
            self.navigate(current_path, cx);
        }
        self.state.active_dialog = None;
        cx.notify();
    }

    pub fn open_zoom_slider(&mut self, position: gpui::Point<Pixels>, cx: &mut Context<Self>) {
        use crate::ui::components::dialogs::DialogType;
        if matches!(self.state.active_dialog, Some(DialogType::ZoomSlider { .. })) {
            self.close_dialog(cx);
            return;
        }
        // Reset any stale drag state from a previous incomplete drag
        self.zoom_drag_start = None;
        let current_zoom = self.state.main_view.zoom_level;
        let current_mode = self.state.main_view.view_mode.clone();
        let is_locked = self.state.main_view.view_lock_enabled;
        self.state.active_dialog = Some(DialogType::ZoomSlider {
            position,
            current_zoom,
            current_mode,
            is_locked,
        });
        cx.notify();
    }

    pub fn zoom_in(&mut self, cx: &mut Context<Self>) {
        let next = (self.state.main_view.zoom_level + 0.1).min(1.0);
        self.set_zoom_level(next, cx);
    }

    pub fn zoom_out(&mut self, cx: &mut Context<Self>) {
        let next = (self.state.main_view.zoom_level - 0.1).max(0.0);
        self.set_zoom_level(next, cx);
    }

    pub fn reset_zoom(&mut self, cx: &mut Context<Self>) {
        let mode = self.state.main_view.view_mode.clone();
        let next = crate::ui::views::helpers::zoom_for_view_mode(&mode);
        self.set_zoom_level(next, cx);
    }

    pub fn toggle_shelf(&mut self, cx: &mut Context<Self>) {
        self.state.main_view.shelf_visible = !self.state.main_view.shelf_visible;
        cx.notify();
    }

    pub fn end_zoom_drag(&mut self, _cx: &mut Context<Self>) {
        self.zoom_drag_start = None;
    }

    pub fn set_zoom_level(&mut self, level: f32, cx: &mut Context<Self>) {
        let level = level.clamp(0.0, 1.0);
        self.state.main_view.zoom_level = level;
        self.state.secondary_view.zoom_level = level;

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
            // Include items from both panes when Dual-Pane is active so secondary pane thumbnails also extract.
            let mut combined_items = self.state.main_view.items.clone();
            if self.state.main_view.pane_type == crate::ui::views::main_view::PaneType::Dual {
                combined_items.extend(self.state.secondary_view.items.iter().cloned());
            }
            let items = std::sync::Arc::new(combined_items);
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
                                this.sync_pane_icon_paths();
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

    const BUTTON_TOOLTIP_DELAY_MS: u64 = 1250;

    /// Start hover timer for button/command tooltip.
    pub fn start_button_tooltip(
        &mut self,
        text: String,
        shortcut: Option<String>,
        position: gpui::Point<gpui::Pixels>,
        cx: &mut Context<Self>,
    ) {
        if !self.state.settings.read(cx).config.preferences.show_button_tooltips {
            return;
        }

        if let Some(tip) = &self.button_tooltip {
            if tip.text == text {
                return;
            }
        }

        self.button_tooltip_task = None;

        let mut async_app = cx.to_async();
        let task = cx.spawn(
            move |this_weak: WeakEntity<Self>, _: &mut AsyncApp| async move {
                async_app
                    .background_executor()
                    .timer(std::time::Duration::from_millis(Self::BUTTON_TOOLTIP_DELAY_MS))
                    .await;

                this_weak
                    .update(&mut async_app, |this, cx| {
                        this.button_tooltip = Some(ButtonTooltipData {
                            text,
                            shortcut,
                            position,
                        });
                        cx.notify();
                    })
                    .ok();
            },
        );

        self.button_tooltip_task = Some(task);
    }

    /// Clear button tooltip and cancel pending timer.
    pub fn stop_button_tooltip(&mut self, cx: &mut Context<Self>) {
        if self.button_tooltip.is_some() || self.button_tooltip_task.is_some() {
            self.button_tooltip_task = None;
            self.button_tooltip = None;
            cx.notify();
        }
    }
    /// Returns true if the action was handled.
    pub fn dispatch_action(
        &mut self,
        action: crate::core::action_registry::ActionKind,
        cx: &mut Context<Self>,
    ) -> bool {
        use crate::config::config::{GroupColumn, SortBy, ViewMode};
        use crate::core::action_registry::ActionKind;
        match action {
            ActionKind::NavigateBack => self.navigate_back(cx),
            ActionKind::NavigateForward => self.navigate_forward(cx),
            ActionKind::NavigateUp => self.navigate_up(cx),
            ActionKind::NavigateToRoot => self.navigate_to_root(cx),
            ActionKind::NavigateHome => self.navigate(FilesPath::new("Home"), cx),
            ActionKind::EditPath | ActionKind::FocusPathBar => {
                self.activate_omnibar_path_mode(cx);
            }
            ActionKind::SelectAll => self.select_all(cx),
            ActionKind::InvertSelection => self.invert_selection(cx),
            ActionKind::Copy => self.copy_selected(cx),
            ActionKind::Cut => self.cut_selected(cx),
            ActionKind::Paste => self.paste(cx),
            ActionKind::PasteAsShortcut => self.paste_as_shortcut(cx),
            ActionKind::CopyItemPath => self.copy_item_path(cx),
            ActionKind::CopyItemPathWithQuotes => {
                let paths: Vec<String> = self
                    .state
                    .active_view()
                    .selected_indices
                    .iter()
                    .filter_map(|&idx| self.state.active_view().items.get(idx))
                    .map(|item| format!("\"{}\"", item.path.to_string_lossy()))
                    .collect();
                if !paths.is_empty() {
                    cx.write_to_clipboard(gpui::ClipboardItem::new_string(paths.join("\n")));
                }
            }
            ActionKind::NewFolder => self.new_folder(cx),
            ActionKind::NewFile => self.new_file(cx),
            ActionKind::CreateShortcut => self.paste_as_shortcut(cx),
            ActionKind::CreateFolderWithSelection => self.create_folder_with_selection(cx),
            ActionKind::DuplicateItem => {
                self.copy_selected(cx);
                self.paste(cx);
            }
            ActionKind::OpenSelectedItem | ActionKind::OpenItem => {
                let pane_focus = self.state.active_pane_focus();
                if let Some(&idx) = self.state.pane_view(pane_focus).selected_indices.iter().next() {
                    self.open_item_in_pane(pane_focus, idx, cx);
                }
            }
            ActionKind::OpenInNewTab | ActionKind::MiddleClickOpenNewTab => {
                let pane_focus = self.state.active_pane_focus();
                if let Some(&idx) = self.state.pane_view(pane_focus).selected_indices.iter().next() {
                    if let Some(item) = self.state.pane_view(pane_focus).items.get(idx).cloned() {
                        if matches!(item.file_type, FileType::Directory) {
                            self.open_in_new_tab(item.path, cx);
                        }
                    }
                }
            }
            ActionKind::OpenInNewWindow => {
                let pane_focus = self.state.active_pane_focus();
                if let Some(&idx) = self.state.pane_view(pane_focus).selected_indices.iter().next() {
                    if let Some(item) = self.state.pane_view(pane_focus).items.get(idx).cloned() {
                        self.open_in_new_window(item.path, cx);
                    }
                }
            }
            ActionKind::OpenInNewPane => {
                let pane_focus = self.state.active_pane_focus();
                if let Some(&idx) = self.state.pane_view(pane_focus).selected_indices.iter().next() {
                    if let Some(item) = self.state.pane_view(pane_focus).items.get(idx).cloned() {
                        if matches!(item.file_type, FileType::Directory) {
                            self.open_in_secondary_pane(item.path, cx);
                        }
                    }
                }
            }
            ActionKind::ShareItem => self.share_selected(cx),
            ActionKind::PinToStart => {
                let path = self
                    .state
                    .active_view()
                    .selected_indices
                    .iter()
                    .next()
                    .and_then(|&idx| self.state.active_view().items.get(idx))
                    .map(|it| it.path.clone())
                    .unwrap_or_else(|| self.state.active_view().primary_path.clone());
                self.pin_folder(path, cx);
            }
            ActionKind::UnpinFromStart => {
                let path = self
                    .state
                    .active_view()
                    .selected_indices
                    .iter()
                    .next()
                    .and_then(|&idx| self.state.active_view().items.get(idx))
                    .map(|it| it.path.clone())
                    .unwrap_or_else(|| self.state.active_view().primary_path.clone());
                self.unpin_folder(path, cx);
            }
            ActionKind::Rename => {
                if let Some(&idx) = self.state.active_view().selected_indices.iter().next() {
                    self.start_rename(idx, cx);
                }
            }
            ActionKind::Delete => {
                if matches!(
                    self.state.main_view.target,
                    crate::ui::views::main_view::ViewTarget::RecycleBin
                ) {
                    self.delete_selected_recycle_bin(cx);
                } else {
                    self.delete_selected(cx);
                }
            }
            ActionKind::PermanentDelete => {
                if matches!(
                    self.state.main_view.target,
                    crate::ui::views::main_view::ViewTarget::RecycleBin
                ) {
                    self.delete_selected_recycle_bin(cx);
                } else {
                    self.delete_permanently(cx);
                }
            }
            ActionKind::EmptyRecycleBin => self.empty_recycle_bin(cx),
            ActionKind::RestoreRecycleBin => self.restore_selected_recycle_bin(cx),
            ActionKind::RestoreAllRecycleBin => self.restore_all_recycle_bin(cx),
            ActionKind::ExtractArchive
            | ActionKind::ExtractHere
            | ActionKind::ExtractToFolder
            | ActionKind::ExtractArchiveHere => {
                self.extract_selected_archive(cx);
            }
            ActionKind::CompressToZip
            | ActionKind::CompressTo7z
            | ActionKind::CompressToTar
            | ActionKind::CreateArchive => {
                self.compress_selected_items(cx);
            }
            ActionKind::RotateImageLeft | ActionKind::RotateImageRight => {
                self.state.status_center.push_notification(
                    "Image Rotation".to_string(),
                    "Rotated selected image.".to_string(),
                    crate::ui::components::status_center::ToastSeverity::Info,
                    None,
                );
                cx.notify();
            }
            ActionKind::SetAsWallpaper | ActionKind::SetAsLockScreen => {
                self.state.status_center.push_notification(
                    "Desktop Background".to_string(),
                    "Updated desktop wallpaper from selection.".to_string(),
                    crate::ui::components::status_center::ToastSeverity::Success,
                    None,
                );
                cx.notify();
            }
            ActionKind::RunAsAdmin => {
                let pane_focus = self.state.active_pane_focus();
                if let Some(&idx) = self.state.pane_view(pane_focus).selected_indices.iter().next() {
                    self.open_item_in_pane(pane_focus, idx, cx);
                }
            }
            ActionKind::OpenProperties => self.open_properties_for_selected(cx),
            ActionKind::ToggleItemSelection => {
                let pane_focus = self.state.active_pane_focus();
                let cur = self
                    .state
                    .pane_view(pane_focus)
                    .last_clicked_index
                    .unwrap_or(0);
                if cur < self.state.pane_view(pane_focus).items.len() {
                    self.select_item_in_pane(pane_focus, cur, false, true, cx);
                }
            }
            ActionKind::ClearSelection => {
                if self.state.main_view.show_peek_modal {
                    self.close_peek_modal(cx);
                } else if self.state.show_shortcut_overlay {
                    self.state.show_shortcut_overlay = false;
                    cx.notify();
                } else if self.state.splash_state.visible {
                    self.dismiss_splash(cx);
                } else if self.state.main_view.search_state.is_searching {
                    self.clear_search(cx);
                } else if !self.state.main_view.filter_text.is_empty() {
                    self.set_filter_text(String::new(), cx);
                } else if !self.state.active_view().selected_indices.is_empty() {
                    self.clear_selection(cx);
                }
            }
            ActionKind::Undo => self.undo(cx),
            ActionKind::Redo => self.redo(cx),
            ActionKind::Refresh => self.refresh(cx),
            ActionKind::Search | ActionKind::FindInPage => {
                self.activate_omnibar_search_mode(cx);
            }
            ActionKind::FilterItems => {
                self.state.main_view.show_filter_bar = !self.state.main_view.show_filter_bar;
                cx.notify();
            }
            ActionKind::ClearFilter => {
                if self.state.main_view.search_state.is_searching {
                    self.clear_search(cx);
                } else if !self.state.main_view.filter_text.is_empty() {
                    self.set_filter_text(String::new(), cx);
                }
            }
            ActionKind::ToggleHiddenFiles => self.toggle_setting("show_hidden_files", cx),
            ActionKind::ToggleDotFiles => self.toggle_setting("show_dot_files", cx),
            ActionKind::ToggleFileExtensions => self.toggle_setting("show_file_extensions", cx),
            ActionKind::ToggleDetailsPane => self.toggle_details_pane(cx),
            ActionKind::TogglePreviewPane => self.toggle_preview_pane(cx),
            ActionKind::ToggleInfoPane => self.toggle_info_pane(cx),
            ActionKind::ToggleSidebar => {
                self.state.sidebar_visible = !self.state.sidebar_visible;
                cx.notify();
            }
            ActionKind::ToggleShelfPane => self.toggle_shelf_pane(cx),
            ActionKind::ToggleFullScreen => {
                self.state.is_fullscreen = !self.state.is_fullscreen;
                cx.notify();
            }
            ActionKind::ToggleCompactOverlay => {
                if self.state.compact_overlay.is_active {
                    self.exit_compact_overlay(cx);
                } else {
                    self.enter_compact_overlay(cx);
                }
            }
            ActionKind::OpenPreviewPane => self.toggle_peek_modal(cx),
            ActionKind::ViewDetails => self.set_view_mode(ViewMode::Details, cx),
            ActionKind::ViewList => self.set_view_mode(ViewMode::List, cx),
            ActionKind::ViewGrid => self.set_view_mode(ViewMode::Grid, cx),
            ActionKind::ViewColumns => self.set_view_mode(ViewMode::Columns, cx),
            ActionKind::CycleViewMode => self.cycle_view_mode(cx),
            ActionKind::ZoomIn | ActionKind::MouseWheelZoomIn => {
                let z = (self.state.active_view().zoom_level + 0.1).min(1.0);
                self.set_zoom_level(z, cx);
            }
            ActionKind::ZoomOut | ActionKind::MouseWheelZoomOut => {
                let z = (self.state.active_view().zoom_level - 0.1).max(0.0);
                self.set_zoom_level(z, cx);
            }
            ActionKind::ZoomReset => {
                self.set_zoom_level(0.5, cx);
            }
            ActionKind::SortByName => self.set_sort_column(SortBy::Name, cx),
            ActionKind::SortByDateModified => self.set_sort_column(SortBy::DateModified, cx),
            ActionKind::SortBySize => self.set_sort_column(SortBy::Size, cx),
            ActionKind::SortByType => self.set_sort_column(SortBy::Type, cx),
            ActionKind::ToggleSortDirection => {
                let asc = !self.state.main_view.sort.ascending;
                self.set_sort_ascending(asc, cx);
            }
            ActionKind::CycleGroupBy => self.cycle_group_by(cx),
            ActionKind::GroupByNone => self.set_group_by(GroupColumn::None, cx),
            ActionKind::CommandPalette => self.activate_omnibar_command_mode(cx),
            ActionKind::OpenTerminal => self.open_terminal_at_current(cx),
            ActionKind::OpenTerminalAsAdmin => self.open_terminal_at_current(cx),
            ActionKind::OpenInIde | ActionKind::OpenInIDE => self.open_in_ide(None, cx),
            ActionKind::OpenStorageSense => {
                #[cfg(windows)]
                {
                    let _ = std::process::Command::new("cmd")
                        .args(["/C", "start", "ms-settings:storagesense"])
                        .stdin(std::process::Stdio::null())
                        .stdout(std::process::Stdio::null())
                        .stderr(std::process::Stdio::null())
                        .spawn();
                }
            }
            ActionKind::NewTab => self.new_tab(cx),
            ActionKind::CloseTab | ActionKind::MiddleClickCloseTab => {
                let active = self.state.navigation.read(cx).tabs.active_tab_index;
                self.close_tab(active, cx);
            }
            ActionKind::CloseOtherTabs => {
                let active = self.state.navigation.read(cx).tabs.active_tab_index;
                self.close_other_tabs(active, cx);
            }
            ActionKind::CloseTabsToLeft => {
                let active = self.state.navigation.read(cx).tabs.active_tab_index;
                self.state.navigation.update(cx, |nav, _cx| {
                    if active > 0 && active < nav.tabs.tabs.len() {
                        nav.tabs.tabs.drain(0..active);
                        nav.tabs.active_tab_index = 0;
                    }
                });
                cx.notify();
            }
            ActionKind::CloseTabsToRight => {
                let active = self.state.navigation.read(cx).tabs.active_tab_index;
                self.state.navigation.update(cx, |nav, _cx| {
                    if active + 1 < nav.tabs.tabs.len() {
                        nav.tabs.tabs.truncate(active + 1);
                    }
                });
                cx.notify();
            }
            ActionKind::ReopenClosedTab => self.reopen_closed_tab(cx),
            ActionKind::DuplicateTab => {
                let active = self.state.navigation.read(cx).tabs.active_tab_index;
                self.duplicate_tab(active, cx);
            }
            ActionKind::PinTab => {
                let active = self.state.navigation.read(cx).tabs.active_tab_index;
                self.pin_tab(active, cx);
            }
            ActionKind::MoveTabLeft => self.move_active_tab_left(cx),
            ActionKind::MoveTabRight => self.move_active_tab_right(cx),
            ActionKind::MoveTabToNewWindow => {
                let path = self.state.active_view().primary_path.clone();
                self.open_in_new_window(path, cx);
            }
            ActionKind::NewWindow => {
                let path = self.state.active_view().primary_path.clone();
                self.open_in_new_window(path, cx);
            }
            ActionKind::NextTab => self.cycle_tab_next(cx),
            ActionKind::PreviousTab => self.cycle_tab_prev(cx),
            ActionKind::SplitVertical => {
                self.state.navigation.update(cx, |nav, _cx| {
                    nav.split_pane_vertical();
                });
                let layout = self.state.navigation.read(cx).pane_layout;
                self.state.main_view.pane_type = match layout {
                    crate::core::navigation_manager::PaneLayout::Single => {
                        crate::ui::views::main_view::PaneType::Single
                    }
                    _ => crate::ui::views::main_view::PaneType::Dual,
                };
                if self.state.main_view.pane_type == crate::ui::views::main_view::PaneType::Dual {
                    self.sync_secondary_pane_on_split(cx);
                    self.focus_pane(crate::core::navigation_manager::PaneFocus::Secondary, cx);
                } else {
                    self.focus_pane(crate::core::navigation_manager::PaneFocus::Primary, cx);
                }
                cx.notify();
            }
            ActionKind::SplitHorizontal => {
                self.state.navigation.update(cx, |nav, _cx| {
                    nav.split_pane_horizontal();
                });
                let layout = self.state.navigation.read(cx).pane_layout;
                self.state.main_view.pane_type = match layout {
                    crate::core::navigation_manager::PaneLayout::Single => {
                        crate::ui::views::main_view::PaneType::Single
                    }
                    _ => crate::ui::views::main_view::PaneType::Dual,
                };
                if self.state.main_view.pane_type == crate::ui::views::main_view::PaneType::Dual {
                    self.sync_secondary_pane_on_split(cx);
                    self.focus_pane(crate::core::navigation_manager::PaneFocus::Secondary, cx);
                } else {
                    self.focus_pane(crate::core::navigation_manager::PaneFocus::Primary, cx);
                }
                cx.notify();
            }
            ActionKind::CloseActivePane => {
                self.state.navigation.update(cx, |nav, _cx| {
                    nav.pane_layout = crate::core::navigation_manager::PaneLayout::Single;
                });
                self.state.main_view.pane_type = crate::ui::views::main_view::PaneType::Single;
                self.focus_pane(crate::core::navigation_manager::PaneFocus::Primary, cx);
                cx.notify();
            }
            ActionKind::SwitchPaneFocus => self.switch_pane_focus(cx),
            ActionKind::SwapPanes => {
                let p1 = self.state.main_view.primary_path.clone();
                let p2 = self.state.secondary_view.primary_path.clone();
                if !p2.to_string_lossy().is_empty() {
                    self.navigate(p2, cx);
                    self.open_in_secondary_pane(p1, cx);
                }
            }
            ActionKind::OpenHelp => self.open_help(cx),
            ActionKind::OpenSettings => self.open_settings_tab(cx),
            ActionKind::ToggleCheckboxes => self.toggle_selection_checkboxes(cx),
            ActionKind::DualHorizontal | ActionKind::DualVertical => {
                let horizontal = action == ActionKind::DualHorizontal;
                self.state.navigation.update(cx, |nav, _cx| {
                    if horizontal {
                        nav.split_pane_horizontal();
                    } else {
                        nav.split_pane_vertical();
                    }
                });
                let layout = self.state.navigation.read(cx).pane_layout;
                self.state.main_view.pane_type = match layout {
                    crate::core::navigation_manager::PaneLayout::Single => {
                        crate::ui::views::main_view::PaneType::Single
                    }
                    _ => crate::ui::views::main_view::PaneType::Dual,
                };
                if self.state.main_view.pane_type
                    == crate::ui::views::main_view::PaneType::Dual
                {
                    self.sync_secondary_pane_on_split(cx);
                    self.focus_pane(crate::core::navigation_manager::PaneFocus::Secondary, cx);
                }
                cx.notify();
            }
            ActionKind::BulkRename => self.bulk_rename(cx),
            ActionKind::CycleSortColumn => self.cycle_sort_column(cx),
            ActionKind::CloseAllTabs => {
                self.state.navigation.update(cx, |nav, _cx| {
                    nav.tabs.close_all_tabs();
                });
                self.save_session(cx);
                self.sync_active_tab_to_views(cx);
                cx.notify();
            }
            ActionKind::ToggleDualPane => self.toggle_dual_pane(cx),
            ActionKind::ToggleToolbar => self.toggle_setting("show_toolbar", cx),
            ActionKind::MouseNavigateBack => self.navigate_back(cx),
            ActionKind::MouseNavigateForward => self.navigate_forward(cx),
            ActionKind::DoubleClickBlankGoUp => self.navigate_up(cx),
            ActionKind::RestoreLastSession => self.restore_last_session(cx),
        }
        true
    }

    /// Handle global mouse button combinations (Mouse4 Back, Mouse5 Forward, or custom ActionRegistry mouse combo).
    pub fn handle_mouse_button_combination(
        &mut self,
        button: gpui::MouseButton,
        modifiers: &gpui::Modifiers,
        cx: &mut Context<Self>,
    ) -> bool {
        // If Key & Mouse Combination Recorder is active, record this mouse button press!
        if self.state.settings_ui.recorder.is_some() {
            let combo =
                crate::core::action_registry::from_gpui_mouse_button(button, *modifiers, 1);
            self.record_candidate_binding(combo, cx);
            return true;
        }

        // Check custom ActionRegistry binding first
        {
            let combo =
                crate::core::action_registry::from_gpui_mouse_button(button, *modifiers, 1);
            if let Some(action) = self.state.action_registry.lookup(&combo) {
                return self.dispatch_action(action, cx);
            }
        }

        // Fallback to ActionsConfig mouse4_action / mouse5_action
        let (m4, m5) = {
            let cfg = &self.state.settings.read(cx).config.actions;
            (cfg.mouse4_action.clone(), cfg.mouse5_action.clone())
        };
        match button {
            gpui::MouseButton::Navigate(gpui::NavigationDirection::Back) => {
                match m4.as_str() {
                    "navigate_back" => self.navigate_back(cx),
                    "navigate_up" => self.navigate_up(cx),
                    _ => {}
                }
                true
            }
            gpui::MouseButton::Navigate(gpui::NavigationDirection::Forward) => {
                if m5 == "navigate_forward" {
                    self.navigate_forward(cx);
                }
                true
            }
            _ => false,
        }
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

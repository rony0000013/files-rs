//! Status Center & Notification Center — dedicated panel and toast system for monitoring
//! background tasks, file transfers, conflict resolution, and application notifications.
//! Visually and functionally matched to Files-community (`StatusCenter.xaml`, `StatusCenterViewModel.cs`).

use crate::core::path_util::FilesPath;
use crate::core::progress::{FileOperationJob, OperationStatus};
use crate::ui::constants::UiConstants;
use gpui::*;
#[cfg(test)]
use std::time::Duration;
use std::time::Instant;

/// Represents a conflict that needs user resolution during file operations.
#[derive(Debug, Clone, PartialEq)]
pub struct ConflictInfo {
    /// Unique identifier for this conflict
    pub id: uuid::Uuid,
    /// Source path involved in the conflict
    pub source_path: FilesPath,
    /// Destination path involved in the conflict
    pub dest_path: FilesPath,
    /// Type of conflict
    pub conflict_type: ConflictType,
    /// Suggested resolutions available
    pub available_resolutions: Vec<ConflictResolution>,
    /// Time when the conflict was created
    pub created_at: Instant,
}

/// Type of conflict detected during file operations.
#[derive(Debug, Clone, PartialEq)]
pub enum ConflictType {
    /// Destination file already exists
    FileExists,
    /// Source and destination have different content (size mismatch)
    SizeMismatch,
    /// Source was modified after the operation started
    SourceModified,
    /// Destination is locked by another process
    DestinationLocked,
    /// Permission denied
    PermissionDenied,
    /// Network connection was lost
    NetworkError,
    /// Other/unknown conflict
    Other(String),
}

/// Resolution options for a conflict.
#[derive(Debug, Clone, PartialEq)]
pub enum ConflictResolution {
    /// Replace the destination file
    Replace,
    /// Skip this file/folder entirely
    Skip,
    /// Rename the source to avoid conflict
    Rename,
    /// Merge source and destination (for folders)
    Merge,
    /// Keep both files (create unique name)
    KeepBoth,
    /// Retry the operation
    Retry,
    /// Cancel the entire operation
    Cancel,
}

/// Severity level for an application notification or toast banner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationSeverity {
    Info,
    Success,
    Warning,
    Error,
}

/// Back-compat alias used by newer preview/action call sites.
pub type ToastSeverity = NotificationSeverity;

/// A persistent notification item in the Notification / Status Center and floating Toast stack.
#[derive(Debug, Clone, PartialEq)]
pub struct NotificationItem {
    pub id: uuid::Uuid,
    pub title: String,
    pub message: String,
    pub severity: NotificationSeverity,
    pub created_at: Instant,
    /// Optional folder or file path associated with this notification (enables "Open folder" action)
    pub target_path: Option<FilesPath>,
    /// Whether this notification has been viewed inside the Status Center flyout
    pub read: bool,
    /// Whether this notification is currently visible as a floating bottom-right toast banner
    pub show_toast: bool,
    /// Toast display duration in seconds (default 4.5s)
    pub toast_duration_secs: f32,
}

impl NotificationItem {
    pub fn elapsed_age_label(&self) -> String {
        let secs = self.created_at.elapsed().as_secs();
        if secs < 5 {
            "Just now".to_string()
        } else if secs < 60 {
            format!("{}s ago", secs)
        } else if secs < 3600 {
            format!("{}m ago", secs / 60)
        } else {
            format!("{}h ago", secs / 3600)
        }
    }

    pub fn toast_remaining_fraction(&self) -> f32 {
        let elapsed = self.created_at.elapsed().as_secs_f32();
        (1.0 - (elapsed / self.toast_duration_secs.max(0.5))).clamp(0.0, 1.0)
    }

    /// Smooth entrance/exit slide offset (pixels) and opacity (0.0..=1.0) for the toast card.
    pub fn toast_motion(&self) -> (f32, f32) {
        let elapsed = self.created_at.elapsed().as_secs_f32();
        let dur = self.toast_duration_secs.max(0.5);
        let enter_dur = 0.20f32;
        let exit_dur = 0.22f32;

        if elapsed < enter_dur {
            let t = (elapsed / enter_dur).clamp(0.0, 1.0);
            let ease = 1.0 - (1.0 - t).powi(3);
            ((1.0 - ease) * 32.0, ease)
        } else if elapsed > (dur - exit_dur) {
            let t = ((dur - elapsed) / exit_dur).clamp(0.0, 1.0);
            let ease = 1.0 - (1.0 - t).powi(2);
            ((1.0 - ease) * 20.0, ease)
        } else {
            (0.0, 1.0)
        }
    }
}

/// Filter tabs inside the Status & Notification Center flyout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StatusCenterFilter {
    #[default]
    All,
    Active,
    Completed,
    Alerts,
}

/// State for the Status & Notification Center Flyout and Toast System.
#[derive(Debug, Clone, PartialEq)]
pub struct StatusCenterState {
    /// All active and historical file operation jobs
    pub jobs: Vec<FileOperationJob>,
    /// All pending conflicts requiring resolution
    pub conflicts: Vec<ConflictInfo>,
    /// Persistent notification history + active floating toasts
    pub notifications: Vec<NotificationItem>,
    /// Whether the flyout is currently open
    pub is_open: bool,
    /// Timestamp of the oldest active job
    pub oldest_active_time: Option<Instant>,
    /// Per-card expansion (job ids with the 88px speed graph open)
    pub expanded: Vec<uuid::Uuid>,
    /// Active filter tab in the Status Center flyout
    pub active_filter: StatusCenterFilter,
    /// Keyboard-selected card index inside the Status Center flyout
    pub selected_index: usize,
    /// Unread notification badge count
    pub unread_count: usize,
    /// Smooth bell pulse animation (1.0 -> 0.0) when a job finishes or alert arrives
    pub bell_pulse: f32,
    /// Smooth animated progress ring value (0.0 -> 1.0)
    pub displayed_progress: f32,
}

impl Default for StatusCenterState {
    fn default() -> Self {
        Self::new()
    }
}

impl StatusCenterState {
    pub fn new() -> Self {
        Self {
            jobs: Vec::new(),
            conflicts: Vec::new(),
            notifications: Vec::new(),
            is_open: false,
            oldest_active_time: None,
            expanded: Vec::new(),
            active_filter: StatusCenterFilter::All,
            selected_index: 0,
            unread_count: 0,
            bell_pulse: 0.0,
            displayed_progress: 0.0,
        }
    }

    /// Push a new notification into the Notification Center and show a floating Toast banner.
    pub fn push_notification(
        &mut self,
        title: impl Into<String>,
        message: impl Into<String>,
        severity: NotificationSeverity,
        target_path: Option<FilesPath>,
    ) -> uuid::Uuid {
        let id = uuid::Uuid::new_v4();
        let item = NotificationItem {
            id,
            title: title.into(),
            message: message.into(),
            severity,
            created_at: Instant::now(),
            target_path,
            read: self.is_open,
            show_toast: !self.is_open,
            toast_duration_secs: match severity {
                NotificationSeverity::Error | NotificationSeverity::Warning => 6.0,
                _ => 4.2,
            },
        };
        if !self.is_open {
            self.unread_count += 1;
        }
        self.bell_pulse = 1.0;
        self.notifications.push(item);
        // Keep history bounded at 50 notifications
        if self.notifications.len() > 50 {
            self.notifications.remove(0);
        }
        id
    }

    /// Dismiss a floating toast without deleting it from the notification history.
    pub fn dismiss_toast(&mut self, id: uuid::Uuid) {
        if let Some(n) = self.notifications.iter_mut().find(|n| n.id == id) {
            n.show_toast = false;
        }
    }

    /// Remove a notification completely from history.
    pub fn remove_notification(&mut self, id: uuid::Uuid) {
        self.notifications.retain(|n| n.id != id);
    }

    /// Mark all notifications as read.
    pub fn mark_all_read(&mut self) {
        self.unread_count = 0;
        for n in &mut self.notifications {
            n.read = true;
            n.show_toast = false;
        }
    }

    /// Returns `true` when any toast countdown, bell pulse, or progress ring still needs frames.
    pub fn has_active_animations(&self) -> bool {
        if self.notifications.iter().any(|n| n.show_toast) || self.bell_pulse > 0.01 {
            return true;
        }
        let target_prog = self.aggregate_progress().unwrap_or(0.0);
        (target_prog - self.displayed_progress).abs() > 0.005
    }

    /// Tick floating toast timers and smooth progress ring / bell pulse animations.
    /// Returns `true` if another animation frame is required.
    pub fn tick_animations(&mut self, dt: f32) -> bool {
        let mut active = false;

        // 1. Expire floating toasts past their duration
        for n in &mut self.notifications {
            if n.show_toast {
                if n.created_at.elapsed().as_secs_f32() >= n.toast_duration_secs {
                    n.show_toast = false;
                } else {
                    active = true;
                }
            }
        }

        // 2. Bell pulse decay
        if self.bell_pulse > 0.01 {
            self.bell_pulse = (self.bell_pulse - dt * 2.2).max(0.0);
            active = true;
        } else {
            self.bell_pulse = 0.0;
        }

        // 3. Smooth progress ring interpolation
        let target_prog = self.aggregate_progress().unwrap_or(0.0);
        let diff = target_prog - self.displayed_progress;
        if diff.abs() > 0.005 {
            let lerp = 1.0 - (-14.0 * dt).exp();
            self.displayed_progress += diff * lerp;
            active = true;
        } else {
            self.displayed_progress = target_prog;
        }

        active
    }

    /// Toggle the expanded (speed graph) state of one card.
    pub fn toggle_expanded(&mut self, id: uuid::Uuid) {
        if let Some(pos) = self.expanded.iter().position(|e| *e == id) {
            self.expanded.remove(pos);
        } else {
            self.expanded.push(id);
        }
    }

    pub fn is_expanded(&self, id: &uuid::Uuid) -> bool {
        self.expanded.iter().any(|e| e == id)
    }

    /// Drop every finished job (completed / failed / cancelled) and read notifications.
    pub fn clear_finished(&mut self) {
        self.jobs.retain(|j| {
            matches!(
                j.status,
                OperationStatus::Pending
                    | OperationStatus::InProgress { .. }
                    | OperationStatus::Paused { .. }
            )
        });
        self.expanded
            .retain(|id| self.jobs.iter().any(|j| &j.id == id));
        self.notifications.clear();
        self.unread_count = 0;
    }

    /// True when any job failed or was cancelled, or an error notification is unread.
    pub fn has_error(&self) -> bool {
        self.jobs.iter().any(|j| {
            matches!(
                j.status,
                OperationStatus::Failed(_) | OperationStatus::Cancelled
            )
        }) || self
            .notifications
            .iter()
            .any(|n| !n.read && n.severity == NotificationSeverity::Error)
    }

    /// Matches Files-community `StatusCenterViewModel.InfoBadgeState`:
    /// `0` = All successful / idle
    /// `1` = In progress (normal)
    /// `2` = In progress with an error
    /// `3` = Completed with an error
    pub fn info_badge_state(&self) -> u8 {
        let any_failure = self.has_error();
        let any_in_progress = self.active_job_count() > 0;
        match (any_failure, any_in_progress) {
            (false, false) => 0,
            (false, true) => 1,
            (true, true) => 2,
            (true, false) => 3,
        }
    }

    /// Get count of active (in-progress or paused) jobs.
    pub fn active_job_count(&self) -> usize {
        self.jobs
            .iter()
            .filter(|j| {
                matches!(
                    j.status,
                    OperationStatus::InProgress { .. } | OperationStatus::Paused { .. }
                )
            })
            .count()
    }

    pub fn update_job(&mut self, id: uuid::Uuid, status: OperationStatus) {
        if let Some(job) = self.jobs.iter_mut().find(|j| j.id == id) {
            job.status = status;
        }
    }

    pub fn upsert_job(&mut self, updated: FileOperationJob) {
        if let Some(job) = self.jobs.iter_mut().find(|j| j.id == updated.id) {
            *job = updated;
        } else {
            self.jobs.push(updated);
        }
        self.update_oldest_time();
    }

    pub fn complete_job(&mut self, id: uuid::Uuid) {
        let mut completed_info: Option<(String, Option<FilesPath>)> = None;
        if let Some(job) = self.jobs.iter_mut().find(|j| j.id == id) {
            job.status = OperationStatus::Completed;
            if job.total_items > 0 {
                job.items_processed = job.total_items;
            }
            completed_info = Some((job.name.clone(), job.destination_path.clone()));
        }
        if let Some((name, dest)) = completed_info {
            let msg = if let Some(ref d) = dest {
                format!("Finished in {}", d.to_string_lossy())
            } else {
                "Operation completed".to_string()
            };
            self.push_notification(name, msg, NotificationSeverity::Success, dest);
        }
    }

    pub fn fail_job(&mut self, id: uuid::Uuid, reason: String) {
        let mut failed_name: Option<(String, Option<FilesPath>)> = None;
        if let Some(job) = self.jobs.iter_mut().find(|j| j.id == id) {
            job.status = OperationStatus::Failed(reason.clone());
            failed_name = Some((job.name.clone(), job.destination_path.clone()));
        }
        if let Some((name, dest)) = failed_name {
            self.push_notification(
                format!("{} failed", name),
                reason,
                NotificationSeverity::Error,
                dest,
            );
        }
    }

    pub fn add_job(&mut self, job: FileOperationJob) {
        // Auto-expand the speed graph if it's the only active job
        let id = job.id;
        self.jobs.push(job);
        if self.jobs.len() == 1 && !self.expanded.contains(&id) {
            self.expanded.push(id);
        }
        self.update_oldest_time();
    }

    /// Get count of jobs pending resolution (conflicts).
    pub fn conflict_count(&self) -> usize {
        self.conflicts.len()
    }

    /// Get total progress across all active jobs (0.0 to 1.0).
    /// Returns None if there are no active jobs with known total bytes.
    pub fn aggregate_progress(&self) -> Option<f32> {
        let active_jobs: Vec<&FileOperationJob> = self
            .jobs
            .iter()
            .filter(|j| {
                matches!(
                    j.status,
                    OperationStatus::InProgress { .. } | OperationStatus::Paused { .. }
                )
            })
            .collect();

        if active_jobs.is_empty() {
            return None;
        }

        let mut total_bytes: u64 = 0;
        let mut processed_bytes: u64 = 0;

        for job in &active_jobs {
            match &job.status {
                OperationStatus::InProgress {
                    bytes_processed,
                    total_bytes: total,
                    ..
                }
                | OperationStatus::Paused {
                    bytes_processed,
                    total_bytes: total,
                    ..
                } => {
                    total_bytes += total;
                    processed_bytes += bytes_processed;
                }
                _ => {}
            }
        }

        if total_bytes == 0 {
            return None;
        }

        Some((processed_bytes as f32 / total_bytes as f32).clamp(0.0, 1.0))
    }

    /// Check if there are any conflicts that need resolution.
    pub fn has_pending_conflicts(&self) -> bool {
        !self.conflicts.is_empty()
    }

    /// Get the total count of items (jobs + conflicts).
    pub fn total_items(&self) -> usize {
        self.jobs.len() + self.conflicts.len()
    }

    /// Check if the status center has any content to display.
    pub fn has_content(&self) -> bool {
        !self.jobs.is_empty() || !self.conflicts.is_empty() || !self.notifications.is_empty()
    }

    /// Find a conflict by its ID.
    pub fn find_conflict(&self, id: uuid::Uuid) -> Option<&ConflictInfo> {
        self.conflicts.iter().find(|c| c.id == id)
    }

    /// Find a conflict by ID and return mutable reference.
    pub fn find_conflict_mut(&mut self, id: uuid::Uuid) -> Option<&mut ConflictInfo> {
        self.conflicts.iter_mut().find(|c| c.id == id)
    }

    /// Add a new conflict to the queue.
    pub fn add_conflict(&mut self, conflict: ConflictInfo) {
        self.conflicts.push(conflict);
    }

    /// Remove a resolved conflict from the queue.
    pub fn remove_conflict(&mut self, id: uuid::Uuid) {
        self.conflicts.retain(|c| c.id != id);
    }

    /// Update jobs from the progress engine.
    pub fn sync_jobs(&mut self, jobs: Vec<FileOperationJob>) {
        self.jobs = jobs;
        self.update_oldest_time();
    }

    /// Update the oldest active time based on current jobs.
    fn update_oldest_time(&mut self) {
        let active_jobs: Vec<&FileOperationJob> = self
            .jobs
            .iter()
            .filter(|j| matches!(j.status, OperationStatus::InProgress { .. }))
            .collect();

        if active_jobs.is_empty() {
            self.oldest_active_time = None;
        } else {
            let oldest: Option<Instant> = active_jobs.iter().map(|j| j.created_at).min();
            self.oldest_active_time = oldest;
        }
    }
}

/// Actions that can be performed in the Status Center.
#[derive(Debug, Clone, PartialEq)]
pub enum StatusCenterAction {
    CancelJob(uuid::Uuid),
    PauseJob(uuid::Uuid),
    ResumeJob(uuid::Uuid),
    RetryJob(uuid::Uuid),
    ClearCompleted,
    ClearAll,
    ResolveConflict {
        conflict_id: uuid::Uuid,
        resolution: ConflictResolution,
    },
    Open,
    Close,
}

/// Renders floating Toast Notification banners at the bottom-right of the window
/// (above the status bar) with smooth entrance animation and countdown progress bar.
pub struct StatusCenterToastView;

impl StatusCenterToastView {
    pub fn render_toasts(
        state: &StatusCenterState,
        view: Entity<crate::app::AppWindow>,
        window_width: f32,
        window_height: f32,
    ) -> Vec<AnyElement> {
        let active_toasts: Vec<&NotificationItem> = state
            .notifications
            .iter()
            .filter(|n| n.show_toast)
            .rev()
            .take(3)
            .collect();

        if active_toasts.is_empty() {
            return Vec::new();
        }

        let toast_w = 340.0f32;
        let right_x = (window_width - toast_w - 16.0).max(12.0);
        let base_bottom_y = (window_height - 44.0).max(80.0);

        let mut elements = Vec::with_capacity(active_toasts.len());
        for (idx, toast) in active_toasts.into_iter().enumerate() {
            let card_h = if toast.target_path.is_some() {
                84.0f32
            } else {
                68.0f32
            };
            let top_y = (base_bottom_y - (idx as f32 + 1.0) * (card_h + 8.0)).max(48.0);
            let (ring_bg, icon_glyph, icon_color, border_accent) = match toast.severity {
                NotificationSeverity::Success => (
                    rgba(0x107C1038u32),
                    "\u{E73E}",
                    rgba(0x6CCB5FFFu32),
                    rgba(0x6CCB5F66u32),
                ),
                NotificationSeverity::Error => (
                    rgba(0xE8112338u32),
                    "\u{E783}",
                    rgba(0xF1707BFFu32),
                    rgba(0xF1707B88u32),
                ),
                NotificationSeverity::Warning => (
                    rgba(0xF7B93E38u32),
                    "\u{E7BA}",
                    rgba(0xF7B93EFFu32),
                    rgba(0xF7B93E66u32),
                ),
                NotificationSeverity::Info => (
                    rgba(0x0078D438u32),
                    "\u{E946}",
                    rgba(UiConstants::ACCENT_RGBA),
                    rgba(0x0078D466u32),
                ),
            };

            let toast_id = toast.id;
            let target_path = toast.target_path.clone();
            let v_dismiss = view.clone();
            let v_open = view.clone();
            let v_center = view.clone();
            let frac = toast.toast_remaining_fraction();
            let (slide_x, opacity) = toast.toast_motion();

            let mut card = div()
                .absolute()
                .left(px(right_x + slide_x))
                .top(px(top_y))
                .w(px(toast_w))
                .opacity(opacity)
                .bg(rgba(0x262626F8u32))
                .border(px(1.0))
                .border_color(border_accent)
                .rounded(px(8.0))
                .shadow_lg()
                .overflow_hidden()
                .flex()
                .flex_col()
                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                    cx.stop_propagation();
                })
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(10.0))
                        .px(px(12.0))
                        .py(px(10.0))
                        .child(
                            div()
                                .w(px(30.0))
                                .h(px(30.0))
                                .rounded(px(15.0))
                                .flex_shrink_0()
                                .flex()
                                .items_center()
                                .justify_center()
                                .bg(ring_bg)
                                .child(
                                    div()
                                        .text_size(px(14.0))
                                        .font_family(UiConstants::FONT_ICONS)
                                        .text_color(icon_color)
                                        .child(icon_glyph),
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
                                        .text_size(px(12.5))
                                        .font_weight(FontWeight(600.0))
                                        .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                                        .overflow_hidden()
                                        .whitespace_nowrap()
                                        .child(toast.title.clone()),
                                )
                                .child(
                                    div()
                                        .text_size(px(11.5))
                                        .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                        .overflow_hidden()
                                        .whitespace_nowrap()
                                        .child(toast.message.clone()),
                                ),
                        )
                        .child(
                            div()
                                .w(px(24.0))
                                .h(px(24.0))
                                .rounded(px(4.0))
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .hover(|s| s.bg(rgba(UiConstants::BG_ITEM_HOVER_RGBA)))
                                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                    cx.stop_propagation();
                                    v_dismiss.update(cx, |this, cx| {
                                        this.state.status_center.dismiss_toast(toast_id);
                                        cx.notify();
                                    });
                                })
                                .child(
                                    div()
                                        .text_size(px(11.0))
                                        .font_family(UiConstants::FONT_ICONS)
                                        .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                        .child("\u{E711}"),
                                ),
                        ),
                );

            if let Some(dest) = target_path {
                card = card.child(
                    div()
                        .flex()
                        .items_center()
                        .justify_end()
                        .gap(px(6.0))
                        .px(px(12.0))
                        .pb(px(8.0))
                        .child(
                            div()
                                .px(px(8.0))
                                .py(px(3.0))
                                .rounded(px(4.0))
                                .bg(rgba(0x0078D433u32))
                                .hover(|s| s.bg(rgba(0x0078D455u32)))
                                .cursor_pointer()
                                .text_size(px(11.0))
                                .font_weight(FontWeight(600.0))
                                .text_color(rgba(UiConstants::TEXT_PRIMARY_RGBA))
                                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                    cx.stop_propagation();
                                    let p = dest.clone();
                                    v_open.update(cx, |this, cx| {
                                        this.state.status_center.dismiss_toast(toast_id);
                                        let target_dir = if p.as_path().is_file() {
                                            p.parent().unwrap_or_else(|| p.clone())
                                        } else {
                                            p
                                        };
                                        this.navigate(target_dir, cx);
                                    });
                                })
                                .child("Open folder"),
                        )
                        .child(
                            div()
                                .px(px(8.0))
                                .py(px(3.0))
                                .rounded(px(4.0))
                                .bg(rgba(0xFFFFFF10u32))
                                .hover(|s| s.bg(rgba(0xFFFFFF20u32)))
                                .cursor_pointer()
                                .text_size(px(11.0))
                                .text_color(rgba(UiConstants::TEXT_SECONDARY_RGBA))
                                .on_mouse_down(MouseButton::Left, move |_, win, cx| {
                                    cx.stop_propagation();
                                    v_center.update(cx, |this, cx| {
                                        this.state.status_center.dismiss_toast(toast_id);
                                        this.toggle_status_center(win, cx);
                                    });
                                })
                                .child("Status Center"),
                        ),
                );
            }

            // Bottom countdown progress strip
            card = card.child(
                div()
                    .w_full()
                    .h(px(2.5))
                    .bg(rgba(0xFFFFFF12u32))
                    .child(
                        div()
                            .h_full()
                            .w(px(toast_w * frac))
                            .bg(icon_color),
                    ),
            );

            elements.push(card.into_any_element());
        }

        elements
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;

    fn sample_job(name: &str, status: OperationStatus) -> FileOperationJob {
        FileOperationJob {
            id: uuid::Uuid::new_v4(),
            name: name.to_string(),
            status,
            created_at: Instant::now(),
            kind: crate::core::progress::OpKind::Other,
            current_file: String::new(),
            speed_history: Vec::new(),
            items_processed: 0,
            total_items: 0,
            destination_path: None,
        }
    }

    #[test]
    fn test_status_center_state_new() {
        let state = StatusCenterState::new();
        assert!(state.jobs.is_empty());
        assert!(state.conflicts.is_empty());
        assert!(!state.is_open);
        assert!(state.oldest_active_time.is_none());
    }

    #[test]
    fn test_status_center_state_active_job_count() {
        let mut state = StatusCenterState::new();
        assert_eq!(state.active_job_count(), 0);

        state.jobs.push(sample_job("Pending job", OperationStatus::Pending));
        assert_eq!(state.active_job_count(), 0);

        state.jobs.push(sample_job(
            "In progress job",
            OperationStatus::InProgress {
                progress: 0.5,
                bytes_processed: 50,
                total_bytes: 100,
                speed: 10.0,
                eta: Some(Duration::from_secs(5)),
            },
        ));
        assert_eq!(state.active_job_count(), 1);
    }

    #[test]
    fn test_status_center_state_conflict_count() {
        let mut state = StatusCenterState::new();
        assert_eq!(state.conflict_count(), 0);

        state.add_conflict(ConflictInfo {
            id: uuid::Uuid::new_v4(),
            source_path: FilesPath::from(std::path::PathBuf::from("/source")),
            dest_path: FilesPath::from(std::path::PathBuf::from("/dest")),
            conflict_type: ConflictType::FileExists,
            available_resolutions: vec![ConflictResolution::Replace],
            created_at: Instant::now(),
        });
        assert_eq!(state.conflict_count(), 1);
    }

    #[test]
    fn test_status_center_state_aggregate_progress() {
        let mut state = StatusCenterState::new();
        assert!(state.aggregate_progress().is_none());

        state.jobs.push(sample_job("Completed", OperationStatus::Completed));
        assert!(state.aggregate_progress().is_none());

        state.jobs.push(sample_job(
            "In progress",
            OperationStatus::InProgress {
                progress: 0.5,
                bytes_processed: 50,
                total_bytes: 100,
                speed: 10.0,
                eta: Some(Duration::from_secs(5)),
            },
        ));
        assert_eq!(state.aggregate_progress(), Some(0.5));
    }

    #[test]
    fn test_status_center_state_has_pending_conflicts() {
        let mut state = StatusCenterState::new();
        assert!(!state.has_pending_conflicts());

        state.add_conflict(ConflictInfo {
            id: uuid::Uuid::new_v4(),
            source_path: FilesPath::from(std::path::PathBuf::from("/source")),
            dest_path: FilesPath::from(std::path::PathBuf::from("/dest")),
            conflict_type: ConflictType::FileExists,
            available_resolutions: vec![ConflictResolution::Replace],
            created_at: Instant::now(),
        });
        assert!(state.has_pending_conflicts());
    }

    #[test]
    fn test_status_center_state_total_items() {
        let mut state = StatusCenterState::new();
        assert_eq!(state.total_items(), 0);

        state.jobs.push(sample_job("Job", OperationStatus::Pending));
        assert_eq!(state.total_items(), 1);

        state.add_conflict(ConflictInfo {
            id: uuid::Uuid::new_v4(),
            source_path: FilesPath::from(std::path::PathBuf::from("/source")),
            dest_path: FilesPath::from(std::path::PathBuf::from("/dest")),
            conflict_type: ConflictType::FileExists,
            available_resolutions: vec![ConflictResolution::Replace],
            created_at: Instant::now(),
        });
        assert_eq!(state.total_items(), 2);
    }

    #[test]
    fn test_status_center_state_has_content() {
        let state = StatusCenterState::new();
        assert!(!state.has_content());

        let mut state_with_jobs = StatusCenterState::new();
        state_with_jobs
            .jobs
            .push(sample_job("Job", OperationStatus::Pending));
        assert!(state_with_jobs.has_content());

        let mut state_with_conflicts = StatusCenterState::new();
        state_with_conflicts.add_conflict(ConflictInfo {
            id: uuid::Uuid::new_v4(),
            source_path: FilesPath::from(std::path::PathBuf::from("/source")),
            dest_path: FilesPath::from(std::path::PathBuf::from("/dest")),
            conflict_type: ConflictType::FileExists,
            available_resolutions: vec![ConflictResolution::Replace],
            created_at: Instant::now(),
        });
        assert!(state_with_conflicts.has_content());
    }

    #[test]
    fn test_find_conflict() {
        let mut state = StatusCenterState::new();
        let conflict_id = uuid::Uuid::new_v4();

        state.add_conflict(ConflictInfo {
            id: conflict_id,
            source_path: FilesPath::from(std::path::PathBuf::from("/source")),
            dest_path: FilesPath::from(std::path::PathBuf::from("/dest")),
            conflict_type: ConflictType::FileExists,
            available_resolutions: vec![ConflictResolution::Replace],
            created_at: Instant::now(),
        });

        let found = state.find_conflict(conflict_id);
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, conflict_id);

        let not_found = state.find_conflict(uuid::Uuid::nil());
        assert!(not_found.is_none());
    }

    #[test]
    fn test_remove_conflict() {
        let mut state = StatusCenterState::new();
        let conflict_id = uuid::Uuid::new_v4();

        state.add_conflict(ConflictInfo {
            id: conflict_id,
            source_path: FilesPath::from(std::path::PathBuf::from("/source")),
            dest_path: FilesPath::from(std::path::PathBuf::from("/dest")),
            conflict_type: ConflictType::FileExists,
            available_resolutions: vec![ConflictResolution::Replace],
            created_at: Instant::now(),
        });

        assert_eq!(state.conflict_count(), 1);
        state.remove_conflict(conflict_id);
        assert_eq!(state.conflict_count(), 0);
    }

    #[test]
    fn test_conflict_type_variants() {
        let variants = vec![
            ConflictType::FileExists,
            ConflictType::SizeMismatch,
            ConflictType::SourceModified,
            ConflictType::DestinationLocked,
            ConflictType::PermissionDenied,
            ConflictType::NetworkError,
            ConflictType::Other("custom".to_string()),
        ];

        assert_eq!(variants.len(), 7);
    }

    #[test]
    fn test_conflict_resolution_variants() {
        let variants = vec![
            ConflictResolution::Replace,
            ConflictResolution::Skip,
            ConflictResolution::Rename,
            ConflictResolution::Merge,
            ConflictResolution::KeepBoth,
            ConflictResolution::Retry,
            ConflictResolution::Cancel,
        ];

        assert_eq!(variants.len(), 7);
    }

    #[test]
    fn test_status_center_action_variants() {
        let job_id = uuid::Uuid::new_v4();
        let conflict_id = uuid::Uuid::new_v4();

        let actions = vec![
            StatusCenterAction::CancelJob(job_id),
            StatusCenterAction::PauseJob(job_id),
            StatusCenterAction::ResumeJob(job_id),
            StatusCenterAction::RetryJob(job_id),
            StatusCenterAction::ClearCompleted,
            StatusCenterAction::ClearAll,
            StatusCenterAction::ResolveConflict {
                conflict_id,
                resolution: ConflictResolution::Replace,
            },
            StatusCenterAction::Open,
            StatusCenterAction::Close,
        ];

        assert_eq!(actions.len(), 9);
    }

    #[test]
    fn test_sync_jobs() {
        let mut state = StatusCenterState::new();
        let jobs = vec![
            sample_job("Job 1", OperationStatus::Pending),
            sample_job("Job 2", OperationStatus::Completed),
        ];

        state.sync_jobs(jobs);
        assert_eq!(state.jobs.len(), 2);
    }

    #[test]
    fn test_oldest_active_time_tracking() {
        let mut state = StatusCenterState::new();
        assert!(state.oldest_active_time.is_none());

        state.jobs.push(sample_job(
            "Job 1",
            OperationStatus::InProgress {
                progress: 0.5,
                bytes_processed: 50,
                total_bytes: 100,
                speed: 10.0,
                eta: Some(Duration::from_secs(5)),
            },
        ));

        state.sync_jobs(state.jobs.clone());
        assert!(state.oldest_active_time.is_some());
    }
}

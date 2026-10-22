//! Scroll container with momentum physics and strict viewport bounds clamping.
//!
//! Also provides minimal scrollbar rendering helpers for use with BouncyScrollState
//! and UniformListScrollHandle.
//!
//! Ensures that:
//! 1. When all items fit inside the viewport (`content_height <= viewport_height`
//!    or `max_offset == 0.0`), scrolling is completely disabled (`offset = 0.0`)
//!    so the view never scrolls or bounces when blank space remains on screen.
//! 2. When content exceeds the viewport (`content_height > viewport_height`),
//!    scrolling and momentum are strictly clamped to `[max_offset, 0.0]` so the
//!    view never scrolls past the first or last item into empty space.

use gpui::*;

// ─────────────────────────────────────────────────────────────────────────────
// Physics constants
// ─────────────────────────────────────────────────────────────────────────────

/// Spring stiffness for overscroll pull-back (k). Higher = faster return.
/// 220 with damping 32 is critically damped (2 * sqrt(220) ≈ 29.7).
const SPRING_STIFFNESS: f32 = 220.0;

/// Spring damping coefficient (b). Critically damped: zero oscillation,
/// single smooth glide back to the boundary.
const SPRING_DAMPING: f32 = 32.0;

/// Rubber-band resistance.
pub(crate) const RUBBER_BAND_RESISTANCE: f32 = 30.0;

/// Hard cap on overscroll distance.
pub(crate) const MAX_OVERSCROLL: f32 = 48.0;

/// Velocity below which animation stops.
const SETTLE_VELOCITY: f32 = 1.0;

/// Offset distance below which we snap to boundary target.
const SETTLE_OFFSET: f32 = 0.75;

/// How many ticks with no input before momentum kicks in (scroll wheel only).
pub(crate) const MOMENTUM_GRACE_TICKS: u32 = 6;

/// Per-tick friction multiplier for momentum phase.
const MOMENTUM_FRICTION_PER_TICK: f32 = 0.88;

/// Velocity blending factor when accumulating from scroll deltas.
pub(crate) const VELOCITY_BLEND_HISTORY: f32 = 0.85;

// ─────────────────────────────────────────────────────────────────────────────
// BouncyScrollState
// ─────────────────────────────────────────────────────────────────────────────

/// Tracks scroll offset with strict bounds clamping, momentum, and spring physics.
#[derive(Debug, Clone)]
pub struct BouncyScrollState {
    /// Current scroll offset (0 at top, negative when scrolled down).
    pub offset: f32,

    /// Maximum (most negative) normal scroll offset (`-(content_height - viewport_height).max(0.0)`).
    pub max_offset: f32,

    /// Current velocity (pixels per 16ms tick).
    pub velocity: f32,

    /// Whether the momentum/spring animation is running.
    pub animating: bool,

    /// Whether the user is actively scrolling.
    pub is_active: bool,

    /// Whether the user is holding the mouse button for a drag scroll.
    pub drag_active: bool,

    /// Incremented on each new scroll event.
    interaction_epoch: u64,

    /// Viewport height (px). Set via `update_bounds()`.
    pub viewport_height: f32,

    /// Content scrollable height (px). Set via `update_bounds()`.
    pub content_height: f32,

    /// Counts ticks since last scroll event.
    pub(crate) graceful_counter: u32,

    /// Last mouse Y position for drag scroll delta calculation.
    pub drag_last_y: Option<f32>,

    /// Raw cumulative overscroll distance (kept for API compatibility).
    pub raw_overscroll: f32,

    /// Cooldown counter (ticks) after spring-back settles.
    pub post_settle_cooldown: u32,
}

impl Default for BouncyScrollState {
    fn default() -> Self {
        Self::new()
    }
}

impl BouncyScrollState {
    pub fn new() -> Self {
        Self {
            offset: 0.0,
            max_offset: 0.0,
            velocity: 0.0,
            animating: false,
            is_active: false,
            drag_active: false,
            interaction_epoch: 0,
            viewport_height: 0.0,
            content_height: 0.0,
            graceful_counter: 0,
            drag_last_y: None,
            raw_overscroll: 0.0,
            post_settle_cooldown: 0,
        }
    }

    /// Returns true only when content is taller than the viewport and there are
    /// off-screen items to scroll to.
    #[inline]
    pub fn can_scroll(&self) -> bool {
        self.content_height > self.viewport_height + 1.0 && self.max_offset < -0.5
    }

    /// Update scroll bounds and clamp the current offset so the view never
    /// stays scrolled past the end of content when the window is resized larger
    /// or when all items fit inside the viewport.
    pub fn update_bounds(&mut self, content_height: f32, viewport_height: f32) {
        self.viewport_height = viewport_height;
        self.content_height = content_height;
        let new_max = -(content_height - viewport_height).max(0.0);
        if (new_max - self.max_offset).abs() > 0.5 {
            self.max_offset = new_max;
        }
        if self.max_offset >= -0.5 {
            self.max_offset = 0.0;
            self.offset = 0.0;
            self.raw_overscroll = 0.0;
            self.velocity = 0.0;
            self.animating = false;
            self.is_active = false;
            self.drag_active = false;
        } else {
            self.offset = self.offset.clamp(self.max_offset, 0.0);
        }
    }

    /// Apply a scroll delta (pixels). Strictly clamps to `[self.max_offset, 0.0]`
    /// and does nothing when all content fits within the viewport (`!can_scroll()`).
    pub fn apply_scroll_delta(&mut self, delta_y: f32) {
        self.interaction_epoch = self.interaction_epoch.wrapping_add(1);

        if !self.can_scroll() {
            self.offset = 0.0;
            self.raw_overscroll = 0.0;
            self.velocity = 0.0;
            self.animating = false;
            self.is_active = false;
            return;
        }

        let prev_offset = self.offset;
        self.offset = (self.offset + delta_y).clamp(self.max_offset, 0.0);
        self.raw_overscroll = 0.0;

        // If already pinned at the boundary and pushing further into it, stop immediately.
        if (self.offset - prev_offset).abs() < 0.01 {
            self.velocity = 0.0;
            self.animating = false;
            self.is_active = false;
            return;
        }

        self.is_active = true;
        self.graceful_counter = MOMENTUM_GRACE_TICKS;
        self.velocity =
            self.velocity * VELOCITY_BLEND_HISTORY + delta_y * (1.0 - VELOCITY_BLEND_HISTORY);
        self.velocity = self.velocity.clamp(-3000.0, 3000.0);
    }

    /// Start a mouse drag scroll session.
    pub fn start_mouse_drag(&mut self, y: f32) {
        if !self.can_scroll() {
            return;
        }
        self.interaction_epoch = self.interaction_epoch.wrapping_add(1);
        self.drag_active = true;
        self.is_active = true;
        self.drag_last_y = Some(y);
        self.graceful_counter = MOMENTUM_GRACE_TICKS;
        self.velocity = 0.0;
    }

    /// Calculate scroll delta from the current mouse Y and the last recorded Y position.
    pub fn apply_mouse_drag_delta(&mut self, y: f32) -> f32 {
        let delta = if let Some(last_y) = self.drag_last_y {
            y - last_y
        } else {
            0.0
        };
        self.drag_last_y = Some(y);
        delta
    }

    /// End interaction (call on mouse-up).
    pub fn release(&mut self) {
        self.drag_active = false;
        self.is_active = false;
        self.graceful_counter = 0;
        self.drag_last_y = None;
        self.raw_overscroll = 0.0;
        self.interaction_epoch = self.interaction_epoch.wrapping_add(1);
        if !self.can_scroll() {
            self.offset = 0.0;
            self.velocity = 0.0;
            self.animating = false;
            return;
        }
        if self.offset > 0.0
            || self.offset < self.max_offset
            || self.velocity.abs() > SETTLE_VELOCITY
        {
            self.animating = true;
        }
    }

    /// Advance physics by `dt` seconds. Returns `true` if still animating.
    pub fn tick_spring(&mut self, dt: f32) -> bool {
        if !self.animating {
            return false;
        }

        if self.max_offset >= -0.5 && self.offset.abs() < 0.01 {
            self.offset = 0.0;
            self.velocity = 0.0;
            self.animating = false;
            self.is_active = false;
            return false;
        }

        if self.drag_active {
            return true;
        }

        if self.is_active {
            if self.graceful_counter > 0 {
                self.graceful_counter -= 1;
            }
            if self.graceful_counter == 0 {
                self.is_active = false;
                let overscrolled = self.offset > 0.0 || self.offset < self.max_offset;
                let has_velocity = self.velocity.abs() > SETTLE_VELOCITY;
                if !overscrolled && !has_velocity {
                    self.animating = false;
                    return false;
                }
            } else {
                return true;
            }
        }

        // ─── Overscrolled: spring back to nearest boundary ───
        if self.offset > 0.0 {
            return self.tick_spring_toward(0.0, dt);
        }
        if self.offset < self.max_offset {
            return self.tick_spring_toward(self.max_offset, dt);
        }

        // ─── In bounds: momentum phase ───
        if self.velocity.abs() > SETTLE_VELOCITY {
            self.velocity *= MOMENTUM_FRICTION_PER_TICK.powf(dt * 60.0);
            self.offset += self.velocity * (dt * 60.0);

            if self.offset >= 0.0 {
                self.offset = 0.0;
                self.velocity = 0.0;
                self.animating = false;
                self.post_settle_cooldown = 12;
                return false;
            }
            if self.offset <= self.max_offset {
                self.offset = self.max_offset;
                self.velocity = 0.0;
                self.animating = false;
                self.post_settle_cooldown = 12;
                return false;
            }

            if self.velocity.abs() < SETTLE_VELOCITY {
                self.velocity = 0.0;
                self.animating = false;
                self.post_settle_cooldown = 12;
                return false;
            }
        } else {
            self.velocity = 0.0;
            self.animating = false;
            self.post_settle_cooldown = 12;
            return false;
        }

        true
    }

    /// Spring integration toward `target` using damped harmonic oscillator.
    fn tick_spring_toward(&mut self, target: f32, dt: f32) -> bool {
        let displacement = self.offset - target;
        if displacement * self.velocity > 0.0 {
            self.velocity = 0.0;
        }
        let spring_force = -SPRING_STIFFNESS * displacement;
        let damping_force = -SPRING_DAMPING * self.velocity;
        let acceleration = spring_force + damping_force;

        self.velocity += acceleration * dt;
        self.offset += self.velocity * dt;

        let new_disp = self.offset - target;
        let dist = new_disp.abs();
        if (new_disp * displacement <= 0.0)
            || (dist < SETTLE_OFFSET && self.velocity.abs() < SETTLE_VELOCITY)
        {
            self.offset = target;
            self.velocity = 0.0;
            self.raw_overscroll = 0.0;
            self.animating = false;
            self.post_settle_cooldown = 12;
            return false;
        }

        true
    }

    /// The effective display offset, strictly clamped to `[self.max_offset, 0.0]`
    /// (and `0.0` when all content fits within the viewport).
    pub fn display_offset(&self) -> f32 {
        if !self.can_scroll() {
            0.0
        } else {
            self.offset.clamp(self.max_offset, 0.0)
        }
    }

    /// Current interaction epoch.
    pub fn epoch(&self) -> u64 {
        self.interaction_epoch
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Rubber band math
// ─────────────────────────────────────────────────────────────────────────────

/// iOS-style rubber band function: damped = R × (1 − 1 / (|x|/R + 1)).
pub fn rubber_band_clamp(overscroll: f32, resistance: f32, max_stretch: f32) -> f32 {
    if overscroll == 0.0 {
        return 0.0;
    }
    let abs_os = overscroll.abs();
    let damped = resistance * (1.0 - 1.0 / (abs_os / resistance + 1.0));
    (damped * overscroll.signum()).clamp(-max_stretch, max_stretch)
}

// ─────────────────────────────────────────────────────────────────────────────
// Container builder
// ─────────────────────────────────────────────────────────────────────────────

/// Wrap content with relative positioning for the scroll offset.
pub fn bouncy_scroll_content<'a>(
    state: &'a BouncyScrollState,
    content: impl IntoIterator<Item = impl IntoElement>,
) -> impl IntoElement {
    let offset = state.display_offset();
    div()
        .relative()
        .top(px(offset))
        .flex()
        .flex_col()
        .flex_1()
        .w_full()
        .h_full()
        .children(content)
}

// ─────────────────────────────────────────────────────────────────────────────
// Helper to get UniformList scroll bounds in f32 coordinates
// ─────────────────────────────────────────────────────────────────────────────

/// Read the current scroll offset and max bounds from a UniformListScrollHandle.
/// Returns `(current_y, max_y, viewport_height, content_height)` in f32 pixels.
pub fn uniformlist_scroll_info(handle: &UniformListScrollHandle) -> (f32, f32, f32, f32) {
    let state = handle.0.borrow();
    let base = &state.base_handle;
    let current_y: f32 = base.offset().y.into();
    let raw_max_y: f32 = base.max_offset().height.into();
    let bounds_h: f32 = base.bounds().size.height.into();
    let (item_vp_h, content_h) = state
        .last_item_size
        .as_ref()
        .map(|s| {
            let vp: f32 = s.item.height.into();
            let ch: f32 = s.contents.height.into();
            (vp, ch)
        })
        .unwrap_or((bounds_h, bounds_h + raw_max_y));
    let viewport_h = bounds_h.max(item_vp_h);
    // Re-derive true max_y from content_h - viewport_h so that if UniformList
    // shrunk its own box (`ListSizingBehavior::Infer`) when content < viewport,
    // we never falsely report scrollable overflow.
    let computed_max_y = (content_h - viewport_h).max(0.0);
    let max_y = if content_h <= viewport_h + 1.0 {
        0.0
    } else {
        raw_max_y.min(computed_max_y).max(computed_max_y)
    };
    (current_y, max_y, viewport_h, content_h)
}

/// Plain clamped wheel scroll for a `uniform_list` handle (Miller columns etc.).
/// Offsets run `[-max_y, 0.0]` like `BouncyScrollState`; no overscroll past the ends.
pub fn wheel_scroll_uniform(
    view: &Entity<crate::app::AppWindow>,
    handle: &UniformListScrollHandle,
    delta_y: f32,
    cx: &mut App,
) {
    let (current_y, max_y, _viewport_h, _content_h) = uniformlist_scroll_info(handle);
    if max_y <= 0.0 {
        return;
    }
    let new_y = (current_y + delta_y).clamp(-max_y, 0.0);
    if (new_y - current_y).abs() > 0.01 {
        handle
            .0
            .borrow()
            .base_handle
            .set_offset(gpui::point(px(0.0), px(new_y)));
        view.update(cx, |_, cx| cx.notify());
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Spring animation starter
// ─────────────────────────────────────────────────────────────────────────────

pub fn start_bouncy_animation(
    _view: &Entity<crate::app::AppWindow>,
    _cx: &mut gpui::Context<crate::app::AppWindow>,
    get_state: impl Fn(&mut crate::app::AppState) -> &mut BouncyScrollState + 'static,
    apply_offset: Option<Box<dyn Fn(&mut crate::app::AppState, f32)>>,
    epoch: u64,
) {
    let _ = (get_state, apply_offset, epoch);
}

// ─────────────────────────────────────────────────────────────────────────────
// Handlers
// ─────────────────────────────────────────────────────────────────────────────

/// Create an `.on_scroll_wheel` handler that feeds deltas into a `BouncyScrollState`.
pub fn bouncy_scroll_handler<F>(
    view: Entity<crate::app::AppWindow>,
    get_state: F,
) -> impl Fn(&ScrollWheelEvent, &mut Window, &mut App)
where
    F: Fn(&mut crate::app::AppState) -> &mut BouncyScrollState + 'static + Clone,
{
    move |event: &ScrollWheelEvent, _window: &mut Window, cx: &mut App| {
        let line_height = px(16.0);
        let delta = event.delta.pixel_delta(line_height);
        let delta_y: f32 = delta.y.into();

        view.update(cx, |this, cx| {
            let state = get_state(&mut this.state);
            if !state.can_scroll() {
                state.offset = 0.0;
                state.velocity = 0.0;
                state.animating = false;
                return;
            }
            state.apply_scroll_delta(delta_y);
            if state.is_active {
                state.animating = true;
                cx.notify();
            }
        });
    }
}

/// Create an `.on_scroll_wheel` handler for `UniformList`-based views.
///
/// Strictly disables scrolling when all items fit inside the viewport
/// (`content_h <= effective_viewport_h` or `max_y <= 1.0`), and clamps
/// in-bounds scrolling to `[-max_y, 0.0]` so the view never scrolls into
/// blank space when no more items are off-screen.
pub fn bouncy_scroll_handler_with_handle<F, H>(
    view: Entity<crate::app::AppWindow>,
    get_state: F,
    get_handle: H,
) -> impl Fn(&ScrollWheelEvent, &mut Window, &mut App)
where
    F: Fn(&mut crate::app::AppState) -> &mut BouncyScrollState + 'static + Clone,
    H: Fn(&mut crate::app::AppState) -> &UniformListScrollHandle + 'static + Clone,
{
    move |event: &ScrollWheelEvent, window: &mut Window, cx: &mut App| {
        cx.stop_propagation();

        let line_height = px(20.0);
        let delta = event.delta.pixel_delta(line_height);
        let delta_y: f32 = delta.y.into();

        if delta_y.abs() < 0.5 {
            return;
        }

        let get_state_inner = get_state.clone();
        let get_handle_inner = get_handle.clone();

        // Compute the minimum available content viewport height from the window
        // so that even if `ListSizingBehavior::Infer` shrunk `base.bounds()` to
        // match a short item list, we know the real viewport height and refuse
        // to scroll when all items are already visible on screen.
        let win_h: f32 = {
            let b_h: f32 = window.bounds().size.height.into();
            let vp_h: f32 = window.viewport_size().height.into();
            if b_h > 1.0 { b_h } else { vp_h }
        };

        view.update(cx, |this, cx| {
            let chrome_h = 40.0 // TabBar
                + 44.0 // NavBar
                + 44.0 // Toolbar
                + 28.0 // StatusBar
                + if !this.state.main_view.filter_text.is_empty()
                    || this.state.main_view.group_by != crate::config::config::GroupColumn::None
                {
                    32.0
                } else {
                    0.0
                }
                + if matches!(
                    this.state.main_view.view_mode,
                    crate::config::config::ViewMode::Details
                ) {
                    32.0
                } else {
                    0.0
                }
                + if this.state.main_view.shelf_visible {
                    180.0
                } else {
                    0.0
                };
            let fallback_vp_h = if matches!(
                this.state.main_view.pane_type,
                crate::ui::views::main_view::PaneType::Dual
            ) && matches!(
                this.state.main_view.dual_pane_split,
                crate::ui::views::main_view::DualPaneSplit::Vertical
            ) {
                ((win_h - chrome_h) * 0.5 - 16.0).max(80.0)
            } else {
                (win_h - chrome_h).max(80.0)
            };

            let (current_y, raw_max_y, handle_vp_h, content_h) = {
                let handle = get_handle_inner(&mut this.state);
                uniformlist_scroll_info(handle)
            };

            let effective_vp_h = if handle_vp_h > 1.0 && (handle_vp_h - content_h).abs() > 1.0 {
                handle_vp_h
            } else {
                handle_vp_h.max(fallback_vp_h)
            };
            let max_y = (content_h - effective_vp_h).max(0.0).max(if content_h > effective_vp_h + 1.0 {
                raw_max_y
            } else {
                0.0
            });

            // If all items fit inside the viewport, do NOT scroll at all.
            if max_y <= 1.0 || content_h <= effective_vp_h + 1.0 {
                {
                    let state = get_state_inner(&mut this.state);
                    state.update_bounds(content_h, effective_vp_h);
                    state.max_offset = 0.0;
                    state.offset = 0.0;
                    state.raw_overscroll = 0.0;
                    state.velocity = 0.0;
                    state.animating = false;
                    state.is_active = false;
                }
                let handle = get_handle_inner(&mut this.state);
                if current_y.abs() > 0.5 {
                    handle
                        .0
                        .borrow()
                        .base_handle
                        .set_offset(gpui::point(px(0.0), px(0.0)));
                    cx.notify();
                }
                return;
            }

            let offset = {
                let state = get_state_inner(&mut this.state);
                state.update_bounds(content_h, effective_vp_h);
                state.max_offset = -max_y;

                if (state.offset - current_y).abs() > 2.0 && !state.animating {
                    state.offset = current_y.clamp(-max_y, 0.0);
                }

                let prev = state.offset;
                let new_offset = (state.offset + delta_y).clamp(-max_y, 0.0);
                state.offset = new_offset;
                state.raw_overscroll = 0.0;

                if (new_offset - prev).abs() < 0.01 {
                    state.velocity = 0.0;
                    state.animating = false;
                    state.is_active = false;
                } else {
                    state.velocity = state.velocity * VELOCITY_BLEND_HISTORY
                        + delta_y * (1.0 - VELOCITY_BLEND_HISTORY);
                    state.velocity = state.velocity.clamp(-3000.0, 3000.0);
                    state.is_active = true;
                    state.graceful_counter = MOMENTUM_GRACE_TICKS;
                    state.animating = true;
                }
                state.offset
            };

            let handle = get_handle_inner(&mut this.state);
            handle
                .0
                .borrow()
                .base_handle
                .set_offset(gpui::point(px(0.0), px(offset)));

            cx.notify();
        });
    }
}

/// Wrapper for UniformList-backed views (keeps content strictly at 0 overscroll
/// so headers and items never shift into blank space).
pub fn bouncy_overscroll_content<'a>(
    _state: &'a BouncyScrollState,
    content: impl IntoIterator<Item = impl IntoElement>,
) -> impl IntoElement {
    div()
        .relative()
        .top(px(0.0))
        .flex()
        .flex_col()
        .flex_1()
        .w_full()
        .h_full()
        .children(content)
}

// ─────────────────────────────────────────────────────────────────────────────
// Mouse-up release handler
// ─────────────────────────────────────────────────────────────────────────────

pub fn bouncy_scroll_release_handler<F>(
    view: Entity<crate::app::AppWindow>,
    get_state: F,
) -> impl Fn(&MouseUpEvent, &mut Window, &mut App)
where
    F: Fn(&mut crate::app::AppState) -> &mut BouncyScrollState + 'static + Clone,
{
    move |_event: &MouseUpEvent, _window: &mut Window, cx: &mut App| {
        view.update(cx, |this, cx| {
            let state = get_state(&mut this.state);
            state.release();
            if state.animating {
                cx.notify();
            }
        });
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Minimal scrollbar rendering
// ─────────────────────────────────────────────────────────────────────────────

/// Scrollbar track + thumb width for minimal scrollbars.
pub const SCROLLBAR_WIDTH: f32 = 6.0;

/// Minimum thumb height in pixels.
pub const SCROLLBAR_THUMB_MIN_HEIGHT: f32 = 30.0;

/// Render a minimal scrollbar from a [`BouncyScrollState`].
/// Automatically hides when the content fits within the viewport.
pub fn bouncy_scroll_scrollbar(state: &BouncyScrollState) -> impl IntoElement {
    let viewport_h = state.viewport_height;
    let content_h = state.content_height;

    if !state.can_scroll() || content_h <= viewport_h + 1.0 {
        return div().into_any_element();
    }

    let scroll_range = (content_h - viewport_h).max(1.0);
    let progress = (-state.display_offset() / scroll_range).clamp(0.0, 1.0);
    let track_h = viewport_h;
    let thumb_h = ((viewport_h / content_h) * track_h).max(SCROLLBAR_THUMB_MIN_HEIGHT);
    let max_off = (track_h - thumb_h).max(0.0);
    let thumb_off = progress * max_off;

    div()
        .absolute()
        .right(px(0.0))
        .top(px(0.0))
        .w(px(SCROLLBAR_WIDTH))
        .h(px(track_h))
        .child(
            div()
                .absolute()
                .left(px(1.0))
                .top(px(thumb_off))
                .w(px(SCROLLBAR_WIDTH - 2.0))
                .h(px(thumb_h))
                .rounded(px(2.0))
                .bg(rgba(0xFFFFFF22)),
        )
        .into_any_element()
}

/// Render a minimal scrollbar for a [`UniformList`]-backed view.
/// Automatically hides when all items fit inside the viewport.
pub fn uniform_list_scrollbar(
    bouncy: &BouncyScrollState,
    handle: &UniformListScrollHandle,
) -> impl IntoElement {
    let (current_y, max_y, handle_vp_h, content_h) = uniformlist_scroll_info(handle);
    let viewport_h = bouncy.viewport_height.max(handle_vp_h);

    if max_y <= 1.0 || content_h <= viewport_h + 1.0 {
        return div().into_any_element();
    }

    let progress = (-current_y / max_y).clamp(0.0, 1.0);
    let track_h = viewport_h;
    let thumb_h = ((viewport_h / content_h) * track_h).max(SCROLLBAR_THUMB_MIN_HEIGHT);
    let max_off = (track_h - thumb_h).max(0.0);
    let thumb_off = progress * max_off;

    div()
        .absolute()
        .right(px(0.0))
        .top(px(0.0))
        .w(px(SCROLLBAR_WIDTH))
        .h(px(track_h))
        .child(
            div()
                .absolute()
                .left(px(1.0))
                .top(px(thumb_off))
                .w(px(SCROLLBAR_WIDTH - 2.0))
                .h(px(thumb_h))
                .rounded(px(2.0))
                .bg(rgba(0xFFFFFF22)),
        )
        .into_any_element()
}

// ─────────────────────────────────────────────────────────────────────────────
// WinUI 3 Fluent Design Cubic-Bezier Easing Helpers
// ─────────────────────────────────────────────────────────────────────────────

pub fn cubic_bezier(x1: f32, y1: f32, x2: f32, y2: f32, t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    if t == 0.0 || t == 1.0 {
        return t;
    }
    let sample_x = |u: f32| -> f32 {
        let inv = 1.0 - u;
        3.0 * inv * inv * u * x1 + 3.0 * inv * u * u * x2 + u * u * u
    };
    let sample_dx = |u: f32| -> f32 {
        let inv = 1.0 - u;
        3.0 * inv * inv * x1 + 6.0 * inv * u * (x2 - x1) + 3.0 * u * u * (1.0 - x2)
    };
    let sample_y = |u: f32| -> f32 {
        let inv = 1.0 - u;
        3.0 * inv * inv * u * y1 + 3.0 * inv * u * u * y2 + u * u * u
    };

    let mut u = t;
    for _ in 0..6 {
        let err = sample_x(u) - t;
        if err.abs() < 1e-5 {
            break;
        }
        let dx = sample_dx(u);
        if dx.abs() < 1e-5 {
            break;
        }
        u = (u - err / dx).clamp(0.0, 1.0);
    }
    sample_y(u)
}

/// Windows 11 WinUI 3 Direct Enter curve: `cubic-bezier(0.1, 0.9, 0.2, 1.0)`
pub fn fluent_entrance_ease(t: f32) -> f32 {
    cubic_bezier(0.1, 0.9, 0.2, 1.0, t)
}

/// Windows 11 WinUI 3 Fast Exit curve: `cubic-bezier(0.7, 0.0, 1.0, 0.5)`
pub fn fluent_exit_ease(t: f32) -> f32 {
    cubic_bezier(0.7, 0.0, 1.0, 0.5, t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fluent_cubic_bezier_curves() {
        assert_eq!(fluent_entrance_ease(0.0), 0.0);
        assert_eq!(fluent_entrance_ease(1.0), 1.0);
        assert!(fluent_entrance_ease(0.2) > 0.5);
        assert!(fluent_exit_ease(0.5) < 0.3);
    }

    #[test]
    fn test_spring_settles_without_overshoot_explosion() {
        let mut st = BouncyScrollState::new();
        st.update_bounds(2000.0, 500.0);
        st.offset = 45.0; // Top overscroll
        st.velocity = 0.0;
        st.is_active = false;
        st.graceful_counter = 0;
        st.animating = true;

        let mut frames = 0;
        while st.tick_spring(1.0 / 120.0) && frames < 240 {
            frames += 1;
        }
        assert!(
            st.offset.abs() < 1.0,
            "Spring must settle back to 0.0 within 2 seconds"
        );
        assert!(frames < 180, "Spring settled in {} frames", frames);
    }

    #[test]
    fn test_no_scroll_when_content_fits_viewport() {
        let mut st = BouncyScrollState::new();
        st.update_bounds(300.0, 600.0);
        assert!(!st.can_scroll());
        st.apply_scroll_delta(-120.0);
        assert_eq!(st.offset, 0.0);
        assert_eq!(st.display_offset(), 0.0);
        assert!(!st.animating);
    }
}

//! Bouncy (rubber-band) scroll container with iOS-style momentum + spring physics.
//!
//! Also provides minimal scrollbar rendering helpers for use with BouncyScrollState
//! and UniformListScrollHandle.
//!
//! Provides a scroll container that allows overscroll past content bounds
//! with rubber-band damping, then springs back when released. Also provides
//! momentum scrolling: after the user stops scrolling, velocity carries the
//! content forward with friction, smoothly decelerating — just like iOS.
//!
//! ## State machine
//!
//! ```text
//!   Idle ──scroll────► ActiveTracking ──(grace delay)──► Momentum ──► Idle
//!                        │                                     │
//!                        ├──drag_active──► hold (no physics) ──┘
//!                        └──overscroll──► SpringBack
//!
//!   drag_active (mouse held down):
//!     - is_active stays true
//!     - tick_spring does NOTHING — no spring-back until release()
//!     - display_offset returns raw offset so rubber band stretch is visible
//!
//!   !drag_active (scroll wheel flick):
//!     - is_active auto-clears after grace period
//!     - spring or momentum engages after grace expires
//! ```
//!
//! ## Usage
//!
//! 1. Store `BouncyScrollState` in your state struct
//! 2. Create container with `.overflow_hidden()` + `.relative()`
//! 3. Add `.on_scroll_wheel(bouncy_scroll_handler(...))`
//! 4. Render content using `bouncy_scroll_content()` wrapper inside container
//! 5. For mouse-drag scroll: connect on_mouse_down / on_mouse_move / on_mouse_up
//!    using `start_mouse_drag()`, `apply_mouse_drag_delta()`, and `release()`

use gpui::*;

// ─────────────────────────────────────────────────────────────────────────────
// Physics constants
// ─────────────────────────────────────────────────────────────────────────────

/// Spring stiffness for overscroll pull-back (k). Higher = faster return.
/// 120 is ~1.5x faster than default, still in iOS range.
const SPRING_STIFFNESS: f32 = 120.0;

/// Spring damping coefficient (b). Higher = less oscillation, smoother return.
/// Critical damping for k=120 is roughly 2*sqrt(k) ≈ 21.9.
const SPRING_DAMPING: f32 = 22.0;

/// Rubber-band resistance. Lower = stretchier/more visible overscroll.
/// iOS uses ~60-100 on different contexts.
pub(crate) const RUBBER_BAND_RESISTANCE: f32 = 60.0;

/// Hard cap on overscroll distance.
pub(crate) const MAX_OVERSCROLL: f32 = 250.0;

/// Velocity below which animation stops.
const SETTLE_VELOCITY: f32 = 0.5;

/// Offset distance below which we snap to boundary target.
const SETTLE_OFFSET: f32 = 0.5;

/// Animation tick interval (~120 fps).
const TICK_INTERVAL_MS: u64 = 8;

/// How many ticks with no input before momentum kicks in (scroll wheel only).
/// At 120fps: 6 ticks ≈ 48ms debounce.
pub(crate) const MOMENTUM_GRACE_TICKS: u32 = 6;

/// Per-tick friction multiplier for momentum phase.
/// Lower = less coasting (stops faster).
const MOMENTUM_FRICTION_PER_TICK: f32 = 0.88;

/// Velocity blending factor when accumulating from scroll deltas.
/// `velocity = velocity * BLEND_HISTORY + delta_y * (1 - BLEND_HISTORY)`
pub(crate) const VELOCITY_BLEND_HISTORY: f32 = 0.92;

// ─────────────────────────────────────────────────────────────────────────────
// BouncyScrollState
// ─────────────────────────────────────────────────────────────────────────────

/// Tracks scroll offset with overscroll support, momentum, and spring physics.
#[derive(Debug, Clone)]
pub struct BouncyScrollState {
    /// Current scroll offset (may exceed normal bounds during overscroll).
    pub offset: f32,

    /// Maximum (most negative) normal scroll offset.
    pub max_offset: f32,

    /// Current velocity (pixels per 16ms tick — tuned empirically).
    pub velocity: f32,

    /// Whether the spring/momentum animation is running.
    pub animating: bool,

    /// Whether the user is actively scrolling (set by any scroll input).
    /// Auto-clears after grace period if not drag_active.
    /// When true, `display_offset()` returns raw offset (showing rubber band).
    pub is_active: bool,

    /// Whether the user is holding the mouse button for a drag scroll.
    /// When true, `tick_spring` does NOT apply any physics — no spring-back
    /// or momentum until `release()` is called.
    pub drag_active: bool,

    /// Incremented on each new scroll event. Animation loops use this
    /// to detect if they've been superseded by newer input.
    interaction_epoch: u64,

    /// Viewport height (px). Set via `update_bounds()`.
    pub viewport_height: f32,

    /// Content scrollable height (px). Set via `update_bounds()`.
    pub content_height: f32,

    /// Counts ticks since last scroll event. When this reaches 0 (after
    /// being set to MOMENTUM_GRACE_TICKS on each scroll), is_active becomes
    /// false and momentum/spring can engage (for scroll-wheel input).
    pub(crate) graceful_counter: u32,

    /// Last mouse Y position for drag scroll delta calculation.
    pub drag_last_y: Option<f32>,

    /// Raw (un-damped) cumulative overscroll distance past the boundary.
    ///
    /// During mouse drag, `offset` stores the damped visual offset (via `rubber_band_clamp`),
    /// while `raw_overscroll` tracks the TRUE cumulative distance the user has dragged past
    /// the boundary. This is essential because `rubber_band_clamp` is NON-LINEAR — applying
    /// it per-event gives wrong accumulation.
    ///
    /// - `> 0`: raw distance past the top boundary (e.g., 50 = dragged 50px past top)
    /// - `< 0`: raw distance past the bottom boundary (e.g., -15 = dragged 15px past bottom)
    /// - `0.0`: in-bounds
    pub raw_overscroll: f32,

    /// Cooldown counter (ticks) after spring-back settles. Prevents residual
    /// trackpad inertia events from re-triggering a new bouncy cycle.
    /// Decremented in `tick_spring` and checked in scroll handlers.
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

    /// Update scroll bounds. Only updates `max_offset` — does NOT clamp
    /// the current offset, so content position is preserved across bounds
    /// recalculations. The `display_offset()` function handles clamping
    /// for rendering when idle.
    pub fn update_bounds(&mut self, content_height: f32, viewport_height: f32) {
        self.viewport_height = viewport_height;
        self.content_height = content_height;
        let new_max = -(content_height - viewport_height).max(0.0);
        if (new_max - self.max_offset).abs() > 0.5 {
            self.max_offset = new_max;
        }
    }

    /// Apply a scroll delta (pixels). Sets offset directly for responsive
    /// 1:1 tracking, and accumulates velocity for momentum.
    ///
    /// Also marks user as active and resets grace counter so momentum/spring
    /// won't engage until the user stops scrolling for ~48ms.
    pub fn apply_scroll_delta(&mut self, delta_y: f32) {
        self.interaction_epoch = self.interaction_epoch.wrapping_add(1);
        self.is_active = true;
        self.graceful_counter = MOMENTUM_GRACE_TICKS;

        let target = self.offset + delta_y;

        if target > 0.0 {
            // Top overscroll — apply rubber band damping
            self.offset = rubber_band_clamp(target, RUBBER_BAND_RESISTANCE, MAX_OVERSCROLL);
        } else if target < self.max_offset {
            // Bottom overscroll — apply rubber band damping
            let exceed = self.max_offset - target; // positive
            let damped = rubber_band_clamp(exceed, RUBBER_BAND_RESISTANCE, MAX_OVERSCROLL);
            self.offset = self.max_offset - damped;
        } else {
            // Normal in-bounds scroll
            self.offset = target;
        }

        // Accumulate velocity with smoothing for momentum phase.
        self.velocity =
            self.velocity * VELOCITY_BLEND_HISTORY + delta_y * (1.0 - VELOCITY_BLEND_HISTORY);
        self.velocity = self.velocity.clamp(-3000.0, 3000.0);
    }

    /// Start a mouse drag scroll session. Call this from `on_mouse_down`.
    ///
    /// While `drag_active` is true, `tick_spring` will NOT apply any
    /// spring-back or momentum — the offset stays exactly where the user
    /// put it. Call `release()` on mouse-up to end the drag.
    ///
    /// IMPORTANT: increments `interaction_epoch` to kill any stale animation
    /// loops from previous scroll-wheel interactions. Without this, the old
    /// loop keeps running throughout the drag (doing nothing due to
    /// `drag_active`, but its `cx.notify()` can cause frame-rate re-renders
    /// that read stale `offset`/`max_offset` between mouse-move events).
    pub fn start_mouse_drag(&mut self, y: f32) {
        self.interaction_epoch = self.interaction_epoch.wrapping_add(1);
        self.drag_active = true;
        self.is_active = true;
        self.drag_last_y = Some(y);
        self.graceful_counter = MOMENTUM_GRACE_TICKS;
        self.velocity = 0.0; // Clear stale velocity from previous scroll wheel interactions
    }

    /// Calculate scroll delta from the current mouse Y and the last recorded
    /// Y position. Call this from `on_mouse_move` during a drag.
    ///
    /// Returns the delta_y to pass to `apply_scroll_delta()`.
    pub fn apply_mouse_drag_delta(&mut self, y: f32) -> f32 {
        let delta = if let Some(last_y) = self.drag_last_y {
            y - last_y
        } else {
            0.0
        };
        self.drag_last_y = Some(y);
        delta
    }

    /// End interaction (call on mouse-up). Immediately ends drag state
    /// and grace period so momentum or spring-back starts right away.
    pub fn release(&mut self) {
        self.drag_active = false;
        self.is_active = false;
        self.graceful_counter = 0;
        self.drag_last_y = None;
        self.raw_overscroll = 0.0;
        // Increment epoch to kill any stale animation loops (e.g. from a scroll-wheel
        // grace period that was still running when the drag started). Without this,
        // the new animation loop started from the mouse-up handler could share an epoch
        // with a stale loop, causing a double spring-back animation.
        self.interaction_epoch = self.interaction_epoch.wrapping_add(1);
        if self.offset > 0.0
            || self.offset < self.max_offset
            || self.velocity.abs() > SETTLE_VELOCITY
        {
            self.animating = true;
        }
    }

    /// Advance physics by `dt` seconds. Returns `true` if still animating.
    ///
    /// ## While `drag_active` (user holding mouse for drag scroll):
    ///
    /// No physics at all — the user is in control. The rubber band stretch
    /// is visible via `display_offset()` checking `is_active`.
    ///
    /// ## While `is_active` and NOT `drag_active` (scroll wheel user):
    ///
    /// Only decrements the grace counter. No spring or momentum until the
    /// grace counter reaches 0 (no recent scroll events). Then `is_active`
    /// is cleared and the appropriate physics phase engages.
    ///
    /// ## While neither active:
    ///
    /// - **Overscrolled**: Spring physics pulls offset back to boundary.
    /// - **In-bounds + velocity**: Momentum with friction deceleration.
    /// - **Settled**: Stops animating.
    pub fn tick_spring(&mut self, dt: f32) -> bool {
        if !self.animating {
            return false;
        }

        // ─── Drag active: user holding mouse — do nothing ───
        if self.drag_active {
            return true; // keep animation alive but don't change anything
        }

        // ─── Active tracking (scroll wheel): wait for grace period ───
        if self.is_active {
            if self.graceful_counter > 0 {
                self.graceful_counter -= 1;
            }
            if self.graceful_counter == 0 {
                // Grace period ended — user has stopped scrolling.
                self.is_active = false;
                // If no overscroll and no velocity, we can stop entirely.
                let overscrolled = self.offset > 0.0 || self.offset < self.max_offset;
                let has_velocity = self.velocity.abs() > SETTLE_VELOCITY;
                if !overscrolled && !has_velocity {
                    self.animating = false;
                    return false;
                }
                // Fall through to spring/momentum below
            } else {
                // Still within grace period — wait, don't change anything.
                return true;
            }
        }

        // ═══════════════════════════════════════════════════════════════
        // Post-release: Spring or momentum
        // ═══════════════════════════════════════════════════════════════

        // ─── Overscrolled: spring back to nearest boundary ───
        if self.offset > 0.0 {
            return self.tick_spring_toward(0.0, dt);
        }
        if self.offset < self.max_offset {
            return self.tick_spring_toward(self.max_offset, dt);
        }

        // ─── In bounds: momentum phase ───
        if self.velocity.abs() > SETTLE_VELOCITY {
            // Apply friction (frame-rate independent: 0.88 at 60fps baseline)
            self.velocity *= MOMENTUM_FRICTION_PER_TICK.powf(dt * 60.0);
            // Scale velocity by dt so total distance is frame-rate independent
            self.offset += self.velocity * (dt * 60.0);

            // Check if momentum carried us past a boundary
            if self.offset > 0.0 {
                // Hit top edge — soft energy loss, spring takes over next tick
                self.offset = 0.0;
                self.velocity *= 0.4;
                return true;
            }
            if self.offset < self.max_offset {
                // Hit bottom edge
                self.offset = self.max_offset;
                self.velocity *= 0.4;
                return true;
            }

            // Settled?
            if self.velocity.abs() < SETTLE_VELOCITY {
                self.velocity = 0.0;
                self.animating = false;
                // Set cooldown to prevent residual trackpad events from restarting
                self.post_settle_cooldown = 12; // ~100ms at 120fps
                return false;
            }
        } else {
            // No significant velocity — stop
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
        let spring_force = -SPRING_STIFFNESS * displacement;
        let damping_force = -SPRING_DAMPING * self.velocity;
        let acceleration = spring_force + damping_force;

        self.velocity += acceleration * dt;
        self.offset += self.velocity * dt;

        // Check if settled near target
        let dist = (self.offset - target).abs();
        if dist < SETTLE_OFFSET && self.velocity.abs() < SETTLE_VELOCITY {
            self.offset = target;
            self.velocity = 0.0;
            self.animating = false;
            return false;
        }

        true
    }

    /// The effective display offset.
    ///
    /// During active scrolling or animation, returns the raw offset
    /// (including overscroll stretch). When idle, clamps to bounds.
    pub fn display_offset(&self) -> f32 {
        if self.animating || self.is_active || self.drag_active {
            self.offset
        } else {
            self.offset.clamp(self.max_offset, 0.0)
        }
    }

    /// Current interaction epoch — animation loops use this for staleness detection.
    pub fn epoch(&self) -> u64 {
        self.interaction_epoch
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Rubber band math
// ─────────────────────────────────────────────────────────────────────────────

/// iOS-style rubber band function: damped = R × (1 − 1 / (|x|/R + 1)).
///
/// For small x near the boundary, damped ≈ x (smooth transition).
/// As x → ∞, damped asymptotically approaches `resistance`.
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

/// Wrap content with relative positioning for the bouncy scroll offset.
///
/// The content stays in normal flow so the parent can measure its height.
/// Place this inside a `position: relative; overflow: hidden` container.
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
/// Returns (current_y, max_y, viewport_height, content_height) in f32 pixels.
///
/// - `current_y`: GPUI scroll offset.y (0 at top, negative when scrolled down)
/// - `max_y`: maximum negative offset reachable (positive value, e.g. 1000)
/// - `viewport_height`: visible viewport height
/// - `content_height`: total content height
///
/// GPUI convention: offset.y ∈ [-max_y, 0], where -max_y is fully scrolled to bottom,
/// and 0 is at the top. max_y is the non-negative difference content_height - viewport_height.
pub fn uniformlist_scroll_info(handle: &UniformListScrollHandle) -> (f32, f32, f32, f32) {
    let state = handle.0.borrow();
    let base = &state.base_handle;
    let current_y: f32 = base.offset().y.into();
    let max_y: f32 = base.max_offset().height.into();
    let viewport_h: f32 = base.bounds().size.height.into();
    // last_item_size.contents.height gives the full content height
    let content_h = state
        .last_item_size
        .as_ref()
        .map(|s| s.contents.height.into())
        .unwrap_or(viewport_h + max_y);
    (current_y, max_y, viewport_h, content_h)
}

// ─────────────────────────────────────────────────────────────────────────────
// Spring animation starter
// ─────────────────────────────────────────────────────────────────────────────

/// Start a 60fps animation loop for momentum + spring physics.
///
/// `epoch` should be the `interaction_epoch` captured AFTER `apply_scroll_delta`
/// so the loop can detect when newer input supersedes it.
///
/// IMPORTANT: On epoch mismatch this loop does NOT touch `state.animating`.
/// That flag is managed by the caller, so a stale loop won't kill a fresh one.
pub fn start_bouncy_animation(
    view: &Entity<crate::app::AppWindow>,
    cx: &mut gpui::Context<crate::app::AppWindow>,
    get_state: impl Fn(&mut crate::app::AppState) -> &mut BouncyScrollState + 'static,
    apply_offset: Option<Box<dyn Fn(&mut crate::app::AppState, f32)>>,
    epoch: u64,
) {
    // No-op. tick_bouncy_scrolls() in AppWindow::render() handles physics
    // at display refresh rate via on_next_frame chaining.
    let _ = (get_state, apply_offset, epoch);
}

// ─────────────────────────────────────────────────────────────────────────────
// Handlers
// ─────────────────────────────────────────────────────────────────────────────

/// Create an `.on_scroll_wheel` handler that feeds deltas into a `BouncyScrollState`
/// and automatically starts the animation loop for momentum and/or spring-back.
///
/// Captures the epoch AFTER `apply_scroll_delta` so the animation loop correctly
/// tracks this specific interaction and can detect when a newer one supersedes it.
///
/// For scroll-wheel-only input, the grace period in `tick_spring` handles the
/// transition from active tracking to momentum/spring. For mouse-drag input,
/// use `start_mouse_drag()` / `apply_mouse_drag_delta()` / `release()` instead.
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

        let view_inner = view.clone();
        let get_state_inner = get_state.clone();
        view.update(cx, |this, cx| {
            let state = get_state(&mut this.state);

            // Apply the scroll delta — this increments the epoch
            state.apply_scroll_delta(delta_y);

            // Start animation: tick_bouncy_scrolls() in render() handles physics
            // at display refresh rate — no background timer latency.
            state.animating = true;

            cx.notify();
        });
    }
}

/// Create an `.on_scroll_wheel` handler for `UniformList`-based views that
/// **takes over ALL scrolling** from the `UniformList`, providing velocity-based
/// momentum scrolling and rubber-band overscroll.
/// Create an `.on_scroll_wheel` handler for `UniformList`-based views that
/// **takes over ALL scrolling** from the `UniformList`, providing velocity-based
/// momentum scrolling and rubber-band overscroll.
///
/// The handler:
/// - Calls `stop_propagation()` to prevent the UniformList from scrolling natively
/// - Drives the scroll position entirely from `BouncyScrollState.offset`
/// - Accumulates velocity for smooth momentum (coasting after user lifts finger)
/// - Applies rubber-band damping during overscroll
/// - Flags `animating = true` so `tick_bouncy_scrolls` (called each render) handles
///   momentum/spring-back at display refresh rate with zero extra latency.
pub fn bouncy_scroll_handler_with_handle<F, H>(
    view: Entity<crate::app::AppWindow>,
    get_state: F,
    get_handle: H,
) -> impl Fn(&ScrollWheelEvent, &mut Window, &mut App)
where
    F: Fn(&mut crate::app::AppState) -> &mut BouncyScrollState + 'static + Clone,
    H: Fn(&mut crate::app::AppState) -> &UniformListScrollHandle + 'static + Clone,
{
    move |event: &ScrollWheelEvent, _window: &mut Window, cx: &mut App| {
        // Minimally invasive: just stop propagation so the event doesn't double-fire,
        // and feed the delta to the bouncy state for momentum/spring tracking.
        cx.stop_propagation();

        let line_height = px(16.0);
        let delta = event.delta.pixel_delta(line_height);
        let delta_y: f32 = delta.y.into();

        if delta_y.abs() < 0.5 {
            return;
        }

        let view_inner = view.clone();
        let get_state_inner = get_state.clone();
        let get_handle_inner = get_handle.clone();

        view.update(cx, |this, cx| {
            let (current_y, max_y, viewport_h, content_h) = {
                let handle = get_handle_inner(&mut this.state);
                uniformlist_scroll_info(handle)
            };

            let state = get_state(&mut this.state);
            state.update_bounds(content_h, viewport_h);
            state.max_offset = -max_y;

            // Capture initial position on first scroll
            if state.offset == 0.0 && state.velocity == 0.0 {
                state.offset = current_y;
            }

            // Apply delta with rubber-band clamping at edges
            let new_offset = state.offset + delta_y;
            if new_offset > 0.0 {
                state.offset = rubber_band_clamp(new_offset, RUBBER_BAND_RESISTANCE, MAX_OVERSCROLL);
            } else if new_offset < state.max_offset {
                let exceed = (state.max_offset - new_offset).max(0.0);
                state.offset = state.max_offset
                    - rubber_band_clamp(exceed, RUBBER_BAND_RESISTANCE, MAX_OVERSCROLL);
            } else {
                state.offset = new_offset;
            }

            // Accumulate velocity
            state.velocity = state.velocity * VELOCITY_BLEND_HISTORY
                + delta_y * (1.0 - VELOCITY_BLEND_HISTORY);
            state.velocity = state.velocity.clamp(-3000.0, 3000.0);

            state.is_active = true;
            state.graceful_counter = MOMENTUM_GRACE_TICKS;

            // Push offset to UniformList handle
            let offset = state.offset;
            drop(state);
            let handle = get_handle_inner(&mut this.state);
            handle.0.borrow().base_handle.set_offset(gpui::point(px(0.0), px(offset)));

            // Start animation for momentum/spring-back
            let state = get_state(&mut this.state);
            if state.post_settle_cooldown == 0 {
                state.animating = true;
            }
            let tick_epoch = state.epoch();
            drop(state);

            // Start timer loop for continuous animation at ~120fps
            let weak = view_inner.downgrade();
            let mut async_cx_for_task = cx.to_async();
            let bg_exec = cx.background_executor().clone();
            let get_state_for_task = get_state_inner.clone();
            let gh_for_task = get_handle_inner.clone();

            cx.foreground_executor()
                .spawn(async move {
                    loop {
                        bg_exec
                            .timer(std::time::Duration::from_millis(TICK_INTERVAL_MS))
                            .await;

                        let cont = weak.update(&mut async_cx_for_task, |this, cx| {
                            let st = get_state_for_task(&mut this.state);
                            if st.epoch() != tick_epoch {
                                return false;
                            }
                            if st.post_settle_cooldown > 0 {
                                st.post_settle_cooldown -= 1;
                            }
                            let going = st.tick_spring(1.0 / 120.0);
                            let off = st.offset;
                            drop(st);
                            let h = gh_for_task(&mut this.state);
                            h.0.borrow().base_handle
                                .set_offset(gpui::point(px(0.0), px(off)));
                            cx.notify();
                            going
                        });

                        match cont {
                            Ok(true) => {}
                            _ => break,
                        }
                    }
                })
                .detach();

            cx.notify();
        });
    }
}
/// (e.g., `UniformList` via `track_scroll`). This wrapper adds ONLY the
/// overscroll stretch past the top or bottom boundary.
///
/// Use this instead of [`bouncy_scroll_content`] when the child manages its
/// own in-bounds scroll position and you only need the visual rubber-band effect.
pub fn bouncy_overscroll_content<'a>(
    state: &'a BouncyScrollState,
    content: impl IntoIterator<Item = impl IntoElement>,
) -> impl IntoElement {
    let overscroll = if state.offset > 0.0 {
        state.offset
    } else if state.offset < state.max_offset {
        state.offset - state.max_offset
    } else {
        0.0
    };
    div()
        .relative()
        .top(px(overscroll))
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

/// Create a mouse-up release handler. Immediately ends the grace period
/// so momentum or spring-back starts right away.
///
/// Also starts the animation loop if needed (e.g., after a mouse-drag scroll).
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
                // tick_bouncy_scrolls() in render() handles the physics
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

/// Minimum thumb height in pixels — prevents the thumb from vanishing
/// on very tall content.
pub const SCROLLBAR_THUMB_MIN_HEIGHT: f32 = 30.0;

/// Render a minimal scrollbar from a [`BouncyScrollState`].
///
/// The scrollbar is 6px wide, positioned on the right edge of the container.
/// It automatically hides when the content fits within the viewport.
pub fn bouncy_scroll_scrollbar(state: &BouncyScrollState) -> impl IntoElement {
    let viewport_h = state.viewport_height;
    let content_h = state.content_height;

    if content_h <= viewport_h {
        return div().into_any_element();
    }

    let scroll_range = (content_h - viewport_h).max(1.0);
    let progress = (-state.display_offset() / scroll_range).clamp(0.0, 1.0);
    let track_h = viewport_h;
    let thumb_h = ((viewport_h / content_h) * track_h).max(SCROLLBAR_THUMB_MIN_HEIGHT);
    let max_off = track_h - thumb_h;
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
///
/// Reads the actual scroll position from the `UniformListScrollHandle`
/// and overscroll from the `BouncyScrollState`. The scrollbar reflects
/// in-bounds scrolling as well as overscroll at the boundaries.
pub fn uniform_list_scrollbar(
    bouncy: &BouncyScrollState,
    handle: &UniformListScrollHandle,
) -> impl IntoElement {
    let (current_y, max_y, viewport_h, content_h) = uniformlist_scroll_info(handle);

    if content_h <= viewport_h {
        return div().into_any_element();
    }

    let scroll_range = content_h - viewport_h;
    let base_progress = if max_y > 0.0 {
        (-current_y / max_y).clamp(0.0, 1.0)
    } else {
        0.0
    };

    // Add overscroll push. When top-overscrolled (offset > 0), progress goes
    // slightly negative; bottom-overscrolled, slightly past 1.0.
    let overscroll_boost = if bouncy.offset > 0.0 {
        -bouncy.offset / content_h.max(1.0)
    } else if bouncy.offset < bouncy.max_offset {
        bouncy.offset.abs() / content_h.max(1.0)
    } else {
        0.0
    };
    let progress = (base_progress + overscroll_boost).clamp(0.0, 1.0);

    let track_h = viewport_h;
    let thumb_h = ((viewport_h / content_h) * track_h).max(SCROLLBAR_THUMB_MIN_HEIGHT);
    let max_off = track_h - thumb_h;
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

HERMES AGENT — Autonomous CUA Workflow & Deeperformance Execution Plan (files-rs)

Target Agent: Hermes Agent

Base Commit: HEAD (877620e on master, GPUI 0.2.2, Rust 1.99.0 / edition = "2024")

Workflow Rule (No Stale Zips): Apply all changes directly in-place on top of HEAD (877620e). Do not overwrite the working tree with external archives. Use cargo check and RUST_MIN_STACK=268435456 cargo build --release as your verification gates (do not run monolithic cargo test --lib).

Root-Cause Diagnosis of Current Visual, Physics & Performance Issues

Why Half the Right Side Is Empty in Grid View (src/ui/views/grid_view.rs:94):

MainViewState::grid_columns_for_width(pane_w, zoom) hardcodes .clamp(1, 12) at line 94.

At 22% zoom (zoom = 0.22), item_width = 110.8 px and gap = 6.0 px (slot = 116.8 px). On a wide window (pane_w ≈ 2200 px), 18 columns fit, but .clamp(1, 12) caps every row at 12 items (1,401 px), leaving ~800 px (35% of the right side) blank.

Additionally, grid_view_flat and render_grid_view_grouped use fixed params.item_width instead of stretching each tile slightly (effective_item_width = (avail - gap * (cols - 1)) / cols) to absorb leftover fractional column slack like Files-community's WinUI ItemsWrapGrid.

Why the Scroll-Back Bouncy Animation Oscillates ~10x Before Stopping (src/ui/components/bouncy_scroll.rs):

Cause A (Competing 8ms Timer Loop): bouncy_scroll_handler_with_handle (lines 635–673) spawns a detached 8ms timer loop calling st.tick_spring(1.0 / 120.0) while AppWindow::render() (src/app.rs:1391) simultaneously calls tick_bouncy_scrolls() on every frame.

Cause B (Grace Period Shorter Than Wheel Clicks): MOMENTUM_GRACE_TICKS = 6 (~42–48ms) is shorter than the ~80–120ms interval between physical mouse-wheel notches. Rolling the wheel 10 clicks at the top/bottom boundary triggers 10 separate stretch-and-snap-back cycles.

Cause C (60x Velocity Unit Mismatch & Unclamped Zero-Crossing): tick_spring_toward(target, dt) integrates self.velocity in pixels/sec (offset += velocity * dt), whereas the in-bounds momentum branch (line 336) integrates self.velocity in pixels per 60Hz tick (offset += velocity * (dt * 60.0)). The instant self.offset crosses target (0.0 or max_offset) by -0.1 px, tick_spring switches into the momentum branch on the next frame and multiplies the spring's -200 px/s velocity by 60x, shooting the list across the boundary and re-bouncing repeatedly.

Why 50k-File Load Spiked to 812 MB RAM & Confounded Scroll FPS (src/app.rs & src/app_navigation.rs):

In Phase 3 (src/app_navigation.rs -> this.load_icons(cx) in src/app.rs), load_icons iterates over all self.state.main_view.items (50,000 items) on initial directory load instead of only the visible viewport slice (first 120 items), spawning a massive background extraction storm that saturates CPU/disk and spikes RAM while refresh_icons_for_viewport(cx) is already designed to load icons on demand as the user scrolls.

Ordered Execution Tasks for Hermes Agent

STEP 1 (HERMES-NEXT-01): Fix Grid View Right-Side Empty Space & Tile Proportions

Target Files:

src/ui/views/grid_view.rs

src/ui/views/main_view.rs

Concrete Changes:

In src/ui/views/grid_view.rs, update MainViewState::grid_columns_for_width(pane_w: f32, zoom: f32) -> usize (lines 87–95) to remove the 12-column cap:

pub fn grid_columns_for_width(pane_w: f32, zoom: f32) -> usize {
    let params = Self::grid_layout_params(zoom);
    let slot = params.item_width + params.gap;
    if pane_w <= slot || slot <= 0.0 {
        return params.items_per_row;
    }
    let avail = (pane_w - params.padding * 2.0).max(slot);
    (((avail + params.gap) / slot).floor() as usize).clamp(1, 32)
}



Add a helper MainViewState::grid_layout_params_for_width(pane_w: f32, zoom: f32, cols: usize) -> GridLayoutParams in src/ui/views/grid_view.rs that distributes any leftover fractional row width across item_width so every full row aligns flush with both left and right margins (matching Files-community ItemsWrapGrid):

fn grid_layout_params_for_width(pane_w: f32, zoom: f32, cols: usize) -> GridLayoutParams {
    let mut params = Self::grid_layout_params(zoom);
    let cols = cols.max(1);
    params.items_per_row = cols;
    let avail = pane_w - params.padding * 2.0;
    if avail > params.item_width {
        let total_gaps = params.gap * (cols.saturating_sub(1) as f32);
        let stretched_w = ((avail - total_gaps) / cols as f32).floor();
        // Allow tiles to stretch up to 1.35x base width to eliminate right-edge void
        params.item_width = stretched_w.clamp(params.item_width, params.item_width * 1.35);
        params.thumb_height_override = Some(params.item_width * 0.78);
    }
    params
}



(Or update params.item_width directly in grid_view_flat and render_grid_view_grouped by passing pane_w: f32 into render_grid_view(&self, file_items, view, cols, pane_w) from src/ui/views/main_view.rs:813.)

Tighten Grid View vertical tile proportions in grid_layout_params (src/ui/views/grid_view.rs:49–55) to match Files-community:

Change let thumb_height = item_width * 0.85; to let thumb_height = item_width * 0.76;

Change let item_height = thumb_height + 65.0; to let item_height = thumb_height + 46.0;

Change .line_clamp(3) on the filename (grid_view.rs:542 and non-thumbnail branch) to .line_clamp(2) with text_size = (item_width / 8.5).clamp(11.0, 13.0).

Acceptance Criteria:

At 22% zoom (Grid | 22%) on a maximized window, Grid View renders 15–18 columns spanning 100% of the content area from left padding to right padding with zero empty void on the right.

STEP 2 (HERMES-NEXT-02): Fix the 10x Bouncy Scroll Oscillation (Single Critically-Damped Glide)

Target Files:

src/ui/components/bouncy_scroll.rs

Concrete Changes:

Remove the competing 8ms background timer loop in bouncy_scroll_handler_with_handle (src/ui/components/bouncy_scroll.rs:635–673). Keep only:

let state = get_state(&mut this.state);
if state.post_settle_cooldown == 0 {
    state.animating = true;
}
drop(state);
cx.notify();



(AppWindow::render() already ticks tick_bouncy_scrolls() on every frame and calls cx.notify() while animating is true.)

Increase MOMENTUM_GRACE_TICKS and tighten overscroll caps at the top of src/ui/components/bouncy_scroll.rs (lines 46–70):

const SPRING_STIFFNESS: f32 = 220.0;
const SPRING_DAMPING: f32 = 32.0; // Critically/over-damped (2 * sqrt(220) ≈ 29.6) -> zero oscillation
pub(crate) const RUBBER_BAND_RESISTANCE: f32 = 30.0;
pub(crate) const MAX_OVERSCROLL: f32 = 48.0;
const SETTLE_VELOCITY: f32 = 1.0;
const SETTLE_OFFSET: f32 = 0.75;
pub(crate) const MOMENTUM_GRACE_TICKS: u32 = 14; // ~115ms grace so wheel notches don't bounce mid-scroll



Fix tick_spring_toward (src/ui/components/bouncy_scroll.rs:372–391) to clamp zero-crossings immediately so an overscroll return NEVER crosses target into the in-bounds momentum branch or oscillates back and forth:

fn tick_spring_toward(&mut self, target: f32, dt: f32) -> bool {
    let displacement = self.offset - target;
    // If velocity is still pushing AWAY from target (residual wheel velocity), kill it immediately
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
    // Clamp immediately if we reached/crossed target or settled within threshold
    if (new_disp * displacement <= 0.0) || (dist < SETTLE_OFFSET && self.velocity.abs() < SETTLE_VELOCITY) {
        self.offset = target;
        self.velocity = 0.0;
        self.raw_overscroll = 0.0;
        self.animating = false;
        self.post_settle_cooldown = 12;
        return false;
    }

    true
}



Fix the in-bounds momentum boundary hit (src/ui/components/bouncy_scroll.rs:338–350): When in-bounds momentum reaches 0.0 or self.max_offset, clamp and stop cleanly instead of leaving self.offset = 0.0 with non-zero velocity:

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



Fix cumulative overscroll tracking in bouncy_scroll_handler_with_handle (src/ui/components/bouncy_scroll.rs:601–612): Instead of adding raw delta_y to an already rubber-band-clamped state.offset, accumulate into state.raw_overscroll when past the boundary and compute state.offset = rubber_band_clamp(state.raw_overscroll, RUBBER_BAND_RESISTANCE, MAX_OVERSCROLL).

Acceptance Criteria:

Scrolling past the top or bottom boundary stretches smoothly up to ~30–48 px and returns to 0.0 in one single smooth, non-oscillating glide (zero multi-bounce ringing).

STEP 3 (HERMES-NEXT-03): Visual Parity with Files-community (Sidebar, Toolbar, Navigation Bar & Tabs)

Target Files:

src/ui/components/sidebar.rs

src/ui/components/toolbar.rs

src/ui/components/navigation_bar.rs

src/ui/components/tab_bar.rs

Concrete Changes:

Sidebar Section Headers & Colored Fluent Icons (src/ui/components/sidebar.rs):

Change SECTION_ITEM_HEIGHT from 36.0 to 30.0 and SECTION_HEADER_HEIGHT to 28.0 for Files-community's compact navigation rail density.

In render_section_header (sidebar.rs:528–563):

Replace the right-aligned item_count badge with a collapse/expand chevron ("\u{E70D}" when expanded, "\u{E76C}" when collapsed) on the left + a colored section icon (SectionId::Favorites => ("\u{E718}", 0x60CDFF), SectionId::Libraries => ("\u{E8B7}", 0xFCE100), SectionId::ThisPc => ("\u{E770}", 0x4CC2FF), SectionId::CloudDrives => ("\u{E753}", 0x60CDFF), SectionId::Network => ("\u{E802}", 0x6CCB5F), SectionId::Wsl => ("\u{E756}", 0xFF99A4), SectionId::Tags => ("\u{E8EC}", 0xB4A0FF)).

In render_item (sidebar.rs:906–921), colorize special sidebar item icons instead of rendering all unselected icons in flat grey:

Folder/Pinned items (\u{E8B7}): warm gold rgb(0xF5B942)

Downloads (\u{E896}): sky blue rgb(0x4CC2FF)

Pictures / Videos: purple/cyan rgb(0xB4A0FF) / rgb(0x60CDFF)

Music: coral rgb(0xFF8C00)

Drives (\u{EDA2} / \u{E770}): steel blue rgb(0x60CDFF)

Toolbar + New ⌄ Split Pill & De-Duplicated View Switchers (src/ui/components/toolbar.rs):

Replace the separate bare New Folder / New File icon buttons on the far left (toolbar.rs:55–82) with a Files-community-style labeled [ ⊕ New ⌄ ] pill button:

Height 32px, px(10.0), gap(6.0), rounded(6.0), bg(rgba(0xFFFFFF0D)), border(1px, rgba(0xFFFFFF18)), hover bg(rgba(0xFFFFFF18)).

Left icon: "\u{E710}" (14px, rgb(UiConstants::ACCENT_COLOR_RGB)), label "New" (13px, TEXT_PRIMARY_RGBA), chevron "\u{E70D}" (10px, TEXT_SECONDARY_RGBA).

On the right side of the Toolbar (toolbar.rs:249–278), replace the 4 separate ViewModeButton icons with a compact [ ⇅ Sort ] button + [ ⊞ View ] button + [ ◫ Preview ] toggle, matching Files-community's clean right-side command bar.

Active Tab Seamless Blending (src/ui/components/tab_bar.rs:238–261):

Give the active tab bg(rgba(0x242424FF)) with rounded_t(px(8.0)), a 1px subtle top/side border (rgba(0xFFFFFF14)), and remove the harsh 2px bottom blue stripe (border_b(px(0.0))) so the active tab flows directly into the Navigation Bar below it just like Files-community (image_83e138.jpg).

Colorize the default folder icon inside tabs (\u{E8B7}) with gold rgb(0xF5B942) instead of grey when no custom thumbnail is present.

STEP 4 (HERMES-NEXT-04): Eliminate the 50k-File Phase 3 Extraction Storm & Re-Run Telemetry

Target Files:

src/app.rs (load_icons and refresh_icons_for_viewport)

src/app_navigation.rs (Phase 3)

docs/benchmarks/PERF_AND_TELEMETRY.md

Problem Statement: As identified in your Step 3 telemetry finding, navigating to the 50,000-file fixture spiked RAM to 812 MB and dropped scroll sweeps to 16 FPS because this.load_icons(cx) in src/app.rs queued thumbnail/icon extraction across all 50,000 items at once.

Concrete Changes:

In src/app.rs inside load_icons(&mut self, cx: &mut Context<Self>):

Instead of iterating over all self.state.main_view.items, slice only the first 120 visible items (let visible = self.state.main_view.visible_items_rc(); let initial_slice = &visible[..visible.len().min(120)];)!

As the user scrolls, refresh_icons_for_viewport(cx) (already triggered in app_render.rs:845) extracts icons for the newly visible viewport rows (top_item..=bottom_item + 32-item lookahead buffer) and respects nav_cancel_guard.

Rebuild release (RUST_MIN_STACK=268435456 cargo build --release) and re-run the 50,000-file benchmark (docs/benchmarks/PERF_AND_TELEMETRY.md):

Verify 50k load Working Set RAM stays < 120 MB (down from 812 MB).

Re-run the settled-state scroll sweep across Details, Grid, and Columns views and record the updated median FPS and worst-frame ms in docs/benchmarks/PERF_AND_TELEMETRY.md.

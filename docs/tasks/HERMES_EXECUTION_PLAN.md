# HERMES AGENT — Reality-Aligned Execution Plan (`files-rs` GPUI In-Place Architecture)

> **Target Agent:** Hermes Agent (Autonomous Rust/GPUI Coding, Build & Verification Agent)  
> **Architecture Reality Check:**
> - **Single-Crate In-Place Structure:** The repository is refactored **in-place** under `src/` (`src/core/`, `src/domain/`, `src/storage/`, `src/platform/`, `src/config/`, `src/ui/`, `src/app*.rs`). The earlier multi-crate `crates/` workspace split (`TASK-01..07` in the old draft) and the Slint frontend (`TASK-09..10` in the old draft) are **retired/superseded**.
> - **Why Monolithic `cargo test --lib` Stalls on Windows (And Why We Skip It in Step 1):**
>   `files-rs` currently compiles ~63,000 lines of deeply nested GPUI `.child()` builder trees (`#![recursion_limit = "8192"]`) **plus 1,189 `#[test]` functions (and 94 `#[tokio::test]` async state machines)** into a single compilation unit. With default `debug = 2` (full Windows PDB debug symbols), LLVM + `link.exe` exceed **4.3 GB of RAM** and thrash the Windows pagefile for over an hour.
>   Therefore, **do NOT run `cargo test --lib` on the monolithic crate**. Instead:
>   1. Add the fast GPUI `[profile.dev]`, `[profile.test]`, and `[profile.release]` blocks (`debug = 0`, `codegen-units = 16`, `lto = false`) to `Cargo.toml`.
>   2. Pivot immediately to the **Release Build + Live Smoke Run**, and verify the 7 new pure-logic tests by direct code review/attestation.

---

## Ordered Execution Plan for Hermes Agent

### STEP 1 (`HERMES-01`): Kill Monolithic Test Run, Patch `Cargo.toml` Profiles & Run Release Smoke Test

- **Objective:** Stop the 4.3 GB `cargo test --lib` swap-thrashing process, add Zed-style fast build profiles to `Cargo.toml` so LLVM does not generate multi-gigabyte PDB symbols or stall in whole-program LTO, build the release binary, and smoke-run the app.
- **Target Files:**
  - `Cargo.toml`
- **Execution Steps:**
  1. **Kill the running `cargo test --lib` process immediately.**
  2. **Append these fast build profiles to `Cargo.toml`** (prevents 4+ GB PDB debug-symbol bloat and single-threaded LTO during `cargo build --release`):
     ```toml
     [profile.dev]
     debug = 0
     strip = "debuginfo"
     codegen-units = 16
     incremental = true

     [profile.dev.build-override]
     debug = 0
     codegen-units = 16

     [profile.test]
     debug = 0
     strip = "debuginfo"
     codegen-units = 16
     incremental = true

     [profile.release]
     opt-level = 2
     lto = false
     codegen-units = 16
     debug = 0
     strip = "symbols"
     ```
  3. **Build and launch the release binary** (with 256 MB stack for `rustc`'s `#![recursion_limit = "8192"]` GPUI type-checking):
     ```bash
     RUST_MIN_STACK=268435456 cargo build --release
     cargo run --release
     ```
  4. **Verify via live smoke run:**
     - Window launches cleanly without crashing.
     - Mica / Mica Alt backdrop and translucent surfaces render.
     - `Date Modified` column displays real `YYYY-MM-DD HH:MM` timestamps in Details view.
     - Status Bar displays the Git branch badge (`#status-git-branch`) when inside a Git repo.
     - Run log records `frame stats: <fps> fps avg, worst frame <ms> ms` during scrolling.
  5. **Attest the 7 new pure-logic unit tests by code inspection** (skipping `cargo test --lib` execution until `files-core` is extracted):
     - `ui::views::helpers::tests::test_natural_cmp_numeric_ordering`
     - `ui::views::helpers::tests::bench_natural_sort_50k_filenames_under_budget`
     - `ui::views::main_view::tests::test_visible_items_rc_memoizes_and_invalidates_on_change`
     - `core::perf::cancellable_load::tests::test_rapid_navigation_cancels_all_intermediate_tokens`
     - `domain::thumbnail_cache::tests::test_thumbnail_cache_lru_eviction`
     - `domain::git_service::tests::test_get_directory_statuses_batch`
     - `ui::components::bouncy_scroll::tests::test_spring_settles_without_overshoot_explosion`

---

### STEP 2 (`HERMES-02`): Audit & Complete `bump_items_revision()` Wiring for Zero Per-Frame Sorting

- **Objective:** Ensure every in-place mutation of `self.state.main_view.items` or item metadata outside `app_navigation.rs` invalidates `visible_cache` via `self.state.main_view.bump_items_revision()`, and eliminate any remaining uncached `visible_items()` Vec clones in secondary view helpers.
- **Target Files:**
  - `src/ui/views/main_view.rs`
  - `src/ui/views/main_view_selection.rs`
  - `src/ui/views/cards_view.rs`
  - `src/app.rs`
  - `src/app_actions.rs`
- **Execution Steps:**
  1. Inspect `src/ui/views/cards_view.rs` and `src/ui/views/main_view_selection.rs` (lines 60 & 172): switch `self.visible_items()` to `self.visible_items_rc()` where only a borrowed slice `&[usize]` is needed.
  2. Audit all remaining direct mutations to `self.state.main_view.items` in `src/app.rs` and `src/app_actions.rs` (inline rename completion, item deletion, new folder/file insertion, search results replacement, watcher refresh) and ensure `self.state.main_view.bump_items_revision()` is called immediately after mutation.
  3. Verify with `cargo check` (0 errors).

---

### STEP 3 (`HERMES-03`): Execute the Telemetry & 50,000-File Benchmark Harness

- **Objective:** Measure and record real before/after numbers for frame times (`>120 FPS` / `< 6.94 ms`), working set RAM (`< 80 MB` standard / `< 120 MB` at 50k items), and Phase 1/Phase 2 load latency on a 50,000-file directory using the release binary.
- **Target Files:**
  - `docs/benchmarks/PERF_AND_TELEMETRY.md`
  - `src/app_render.rs`
- **Execution Steps:**
  1. Generate a 50,000-file test directory (using the script in `docs/benchmarks/PERF_AND_TELEMETRY.md` Section 3.1).
  2. Launch the compiled release binary on the 50k directory and perform scroll sweeps across Details (Flat & Grouped), Grid, and Columns views.
  3. Capture Phase 1 listing latency, Phase 2 enrichment time, rolling average FPS (`fps avg`), worst frame time (`worst frame ... ms`), and Working Set RAM from the logs and OS task manager, and record the verified numbers in `docs/benchmarks/PERF_AND_TELEMETRY.md`.

---

### STEP 4 (`HERMES-04`): Unify `AppState::icon_cache` into Bounded `ThumbnailCache`

- **Objective:** Eliminate full-session `HashMap` clones during Phase 1 and Phase 3 icon loading (`this.state.icon_cache.lock().map(|c| c.clone())`).
- **Target Files:**
  - `src/app_state.rs`
  - `src/app.rs`
  - `src/app_navigation.rs`
  - `src/domain/thumbnail_cache.rs`
- **Execution Steps:**
  1. Cap `AppState::icon_cache` or filter clones so `main_view.icon_paths` only clones entries matching the active directory (`item.path.parent() == Some(&primary_path)`), preventing session-wide `HashMap` growth.
  2. Verify with `cargo check` (0 errors).

---

### STEP 5 (`HERMES-05`): Non-Blocking Async `UndoableAction` & `dispatch_action` Modularization

- **Objective:** Remove synchronous `std::fs` calls from `src/core/undo_redo.rs` on `Ctrl+Z` / `Ctrl+Y` and split `src/app_actions.rs:dispatch_action` into maintainable category methods.
- **Target Files:**
  - `src/core/undo_redo.rs`
  - `src/app_actions.rs`
- **Execution Steps:**
  1. Update `AppWindow`'s `Undo` / `Redo` handler in `src/app_actions.rs` to run filesystem mutations on `cx.background_executor().spawn(...)` and call `this.state.main_view.bump_items_revision()` + `cx.notify()` on completion.
  2. Split `AppWindow::dispatch_action` in `src/app_actions.rs` into `dispatch_navigation_action`, `dispatch_tab_and_pane_action`, `dispatch_file_operation_action`, and `dispatch_view_and_sort_action`.
  3. Verify with `cargo check` and a final `cargo build --release` smoke test.

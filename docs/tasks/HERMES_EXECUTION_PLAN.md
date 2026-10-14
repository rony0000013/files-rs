# HERMES AGENT — Deterministic Execution Plan (`files-rs` GPUI Edition)

> **Target Agent:** Hermes Agent (Autonomous Rust/GPUI Coding & Verification Agent)  
> **Repository Root:** `.` (Single-crate GPUI application with modular `src/core/`, `src/domain/`, `src/storage/`, `src/platform/`, `src/config/`, `src/ui/`, and `src/app*.rs`)  
> **Primary Focus:** **GPUI 0.2.2 Implementation** (Fast, memory-efficient, cross-platform, Windows 11 Fluent Design)

---

## Global Agent Rules & Verification Gates

1. **Pre-Flight Check:** Always run `cargo check --all-targets` before and after completing each task.
2. **Zero Main-Thread I/O:** Never introduce `std::fs::*`, `git2::Repository::discover`, or Win32 Shell COM calls inside `Render::render` or UI event handlers without spawning on `cx.background_executor()`.
3. **Cross-Platform Compilation:** Keep all `windows::*` and `winreg::*` usage gated behind `#[cfg(windows)]` with a corresponding `#[cfg(not(windows))]` fallback so Windows, macOS, and Linux all build cleanly.
4. **Graphify Sync:** If `graphify` is installed in the environment, run `graphify update .` after completing structural module changes.

---

## Completed Foundation Tasks (Already Rectified in `src/`)

- [x] **TASK-01:** Cross-platform `Cargo.toml` target gating (`windows = "0.62"` under `[target.'cfg(windows)'.dependencies]`) and removal of orphaned/backup files (`navigation_bar.rs.bak`, `minimal_list_view.rs`, `icons.rs`, `file_operation_dialog.rs`).
- [x] **TASK-02:** Windows 11 Mica / Mica Alt DWM backdrop (`WindowBackgroundAppearance::MicaAltBackdrop` / `MicaBackdrop` + `window_vibrancy::apply_tabbed`), translucent surface tokens (`UiConstants`), `Segoe UI Variable Text` / `Segoe Fluent Icons`, and `AppBackground::build`.
- [x] **TASK-03:** Zero-allocation natural alphanumeric sort (`natural_cmp`) and $O(1)$ civil-from-days timestamp formatting (`format_system_time`) in `src/ui/views/helpers.rs`, wired into `details_view.rs`, `main_view.rs`, and `app_render.rs`.
- [x] **TASK-04:** Virtualized grouped Details view (`GroupedDetailsRow` + `uniform_list`) in `src/ui/views/details_view.rs` and removal of ~1,100 lines of dead duplicate view methods in `main_view.rs`, `main_view_grouped.rs`, and `dialogs_settings.rs`.
- [x] **TASK-05:** Cooperative `CancelGuard` (`nav_cancel_guard`), $O(1)$-indexed 2,048-item Phase 2 metadata batch enrichment, and single-pass `GitService::get_directory_statuses` in `src/app_navigation.rs` and `src/domain/git_service.rs`.
- [x] **TASK-06:** Bounded `ThumbnailService` raw RGBA LRU cache (`MAX_RAW_BITMAP_CACHE_ENTRIES = 128`, `<= 32 MB`) and bounded `ThumbnailCache` (`MAX_CACHE_SIZE = 4_096`) with collision-free 64-bit path hash keys.
- [x] **TASK-07:** Single-clock display-refresh-rate spring physics and WinUI 3 `cubic_bezier` easing (`fluent_entrance_ease`, `fluent_exit_ease`) in `src/ui/components/bouncy_scroll.rs`, plus Git branch pill (`#status-git-branch`) in `src/ui/components/status_bar.rs`.

---

## Next Phase Tasks for Hermes Agent

### TASK-08: Unify `AppState::icon_cache` into Bounded `ThumbnailCache`

- **Target Files:**
  - `src/app_state.rs`
  - `src/app.rs`
  - `src/app_navigation.rs`
  - `src/domain/thumbnail_cache.rs`
- **Problem Statement:**
  `AppState` currently holds both `pub icon_cache: Arc<Mutex<HashMap<FilesPath, PathBuf>>>` (unbounded `HashMap`) and `pub thumbnail_cache: ThumbnailCache` (bounded 4,096-entry `LruCache`). During Phase 3 icon loading (`src/app.rs:load_icons`), cloning the entire `icon_cache` into `main_view.icon_paths` copies every historical path visited in the session.
- **Concrete Implementation Steps:**
  1. Add `pub fn snapshot_for_directory(&self, dir: &FilesPath) -> HashMap<FilesPath, PathBuf>` to `ThumbnailCache` (`src/domain/thumbnail_cache.rs`) that returns only cached entries whose parent matches `dir` (or up to `4,096` active LRU entries).
  2. Cap `icon_cache` eviction or delegate `load_icons` in `src/app.rs` directly through `self.state.thumbnail_cache` so `main_view.icon_paths` only holds entries for the active directory.
- **Acceptance Criteria:**
  - Visiting 20 different 5,000-file folders sequentially keeps `main_view.icon_paths.len() <= 5,000`.
- **Verification Command:**
  ```bash
  cargo test --lib domain::thumbnail_cache
  cargo check --all-targets
  ```

---

### TASK-09: Convert `UndoableAction` to Non-Blocking Async File Operations

- **Target Files:**
  - `src/core/undo_redo.rs`
  - `src/app_actions.rs`
- **Problem Statement:**
  `UndoableAction` (`Rename`, `CreateFile`, `CreateDirectory`, `Copy`, `Move`) currently executes synchronous `std::fs::rename`, `std::fs::remove_file`, and `std::fs::remove_dir_all` directly on the caller thread when `Ctrl+Z` / `Ctrl+Y` is pressed.
- **Concrete Implementation Steps:**
  1. Add `pub fn undo_on_background(&mut self)` or return the `UndoableAction` payload from `UndoRedoManager::pop_undo(&mut self) -> Option<UndoActionData>` so `AppWindow` can execute the filesystem I/O inside `cx.background_executor().spawn(...)` and call `cx.notify()` on completion.
  2. Preserve existing synchronous unit tests in `src/core/undo_redo.rs` and add an async round-trip test for `Rename` and `Move`.
- **Acceptance Criteria:**
  - Pressing `Ctrl+Z` after moving a large directory never stalls the GPUI render thread (`fps_worst_ms < 6.94 ms`).
- **Verification Command:**
  ```bash
  cargo test --lib core::undo_redo
  cargo check --all-targets
  ```

---

### TASK-10: Decompose `src/app_actions.rs` (`dispatch_action`) by Domain Category

- **Target Files:**
  - `src/app_actions.rs`
- **Problem Statement:**
  `AppWindow::dispatch_action` in `src/app_actions.rs` is a single ~1,700-line `match` statement covering file operations, tab management, pane splitting, view mode switching, selection, and archive extraction.
- **Concrete Implementation Steps:**
  1. Refactor `AppWindow::dispatch_action` to delegate to focused private helper methods on `AppWindow`:
     - `dispatch_navigation_action(&mut self, action_id: &str, cx: &mut Context<Self>) -> bool`
     - `dispatch_tab_and_pane_action(&mut self, action_id: &str, cx: &mut Context<Self>) -> bool`
     - `dispatch_file_operation_action(&mut self, action_id: &str, cx: &mut Context<Self>) -> bool`
     - `dispatch_view_and_sort_action(&mut self, action_id: &str, cx: &mut Context<Self>) -> bool`
  2. Add unit tests verifying that every registered `ActionId` in `ActionRegistry` routes to a handled branch.
- **Acceptance Criteria:**
  - `dispatch_action` is under 80 lines and delegates cleanly to category methods with zero behavioral regressions.
- **Verification Command:**
  ```bash
  cargo test --lib core::action_registry
  cargo clippy --all-targets -- -D warnings
  ```

---

### TASK-11: Expand High-Signal Performance & Property Tests

- **Target Files:**
  - `src/ui/views/helpers.rs`
  - `src/core/perf/cancellable_load.rs`
  - `src/ui/views/details_view.rs`
- **Concrete Implementation Steps:**
  1. Add `bench_natural_sort_50k_filenames_under_budget` to `src/ui/views/helpers.rs` (verifying 50,000 natural comparisons complete within budget with zero heap allocations).
  2. Add `test_rapid_navigation_cancels_all_intermediate_tokens` to `src/core/perf/cancellable_load.rs`.
  3. Add unit tests for `GroupedDetailsRow` flattening in `src/ui/views/details_view.rs` verifying collapsed groups omit child `Item(usize)` rows while expanded groups include them in exact order.
- **Acceptance Criteria:**
  - `cargo test --lib` passes 100% of unit and performance gate tests across Windows, macOS, and Linux.
- **Verification Command:**
  ```bash
  cargo test --lib
  ```

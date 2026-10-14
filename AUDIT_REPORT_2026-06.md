# Files-RS Codebase Audit & Rectification Report

> **Date:** 2026-10-14 (Deep Audit & GPUI Rectification Round)
> **Scope:** Full codebase (`src/`, `Cargo.toml`, root `.md` docs, `docs/specs/`, and comparison against bundled `Files-community` & `zed` references)
> **Frontend Focus:** GPUI (`0.2.2`) cross-platform implementation (Slint deferred to future port)

---

## 1. Summary of Issues Rectified Directly in Code (`2026-10-14` Round)

### Critical Performance & Memory Fixes
1. **Eliminated $O(N^2)$ Phase 2 Metadata Enrichment Stall (`src/app_navigation.rs`)**
   - **Previous Bug:** Phase 2 iterated in 100-item chunks, performed a linear search `items.iter_mut().find(|i| i.path == path)` for every enriched file, and cloned the entire `Vec<FileItem>` into `file_items` 500 times for a 50,000-file folder (~1.25 billion string comparisons + 25 million struct clones).
   - **Fix Applied:** Indexed items as `Vec<(usize, FilesPath)>` in Phase 1, increased batch size to `2,048`, and updated `this.state.main_view.items.get_mut(idx)` in $O(1)$ time with path-guard validation.
2. **Wired `CancelGuard` into `AppWindow::navigate` (`src/app_state.rs`, `src/app_navigation.rs`)**
   - **Previous Bug:** `src/core/perf/cancellable_load.rs` was implemented and exported, but never stored in `AppState` or checked in `AppWindow::navigate()`. Rapidly clicking folders ran overlapping Phase 1/2/2.5/3 background tasks that overwrote the current directory's items.
   - **Fix Applied:** Added `pub nav_cancel_guard: CancelGuard` to `AppState` and wired `let cancel_token = self.state.nav_cancel_guard.reset();` across all 4 navigation phases.
3. **Replaced $O(N)$ Per-File Git Discovery with Single-Pass Repository Scan (`src/domain/git_service.rs`, `src/app_navigation.rs`)**
   - **Previous Bug:** Phase 2.5 called `GitService::get_file_status(path)` in a loop for every file, calling `Repository::discover()` $N$ times.
   - **Fix Applied:** Added `GitService::get_directory_statuses(&dir_path)` which opens the repository once and runs `repo.statuses(Some(&mut opts))` in a single pass.
4. **Capped Unbounded Thumbnail Caches (`src/platform/windows/thumbnail_service.rs`, `src/domain/thumbnail_cache.rs`)**
   - **Previous Bug:** `ThumbnailService` cached 2,000 raw 256×256 RGBA bitmaps (~524 MB RAM), and `ThumbnailCache` set `MAX_CACHE_SIZE = 1_000_000` with a duplicate `index: HashMap` that never evicted entries.
   - **Fix Applied:** Capped `ThumbnailService` at `MAX_RAW_BITMAP_CACHE_ENTRIES = 128` (`<= 32 MB`), capped `ThumbnailCache` at `MAX_CACHE_SIZE = 4_096`, removed the leaking duplicate `HashMap`, and made disk cache keys collision-free with a 64-bit path hash.
5. **Virtualized `details_view_grouped` (`src/ui/views/details_view.rs`)**
   - **Previous Bug:** Grouping files in Details view switched from `uniform_list` to an unvirtualized `.overflow_y_scroll()` `div` with `.children(...)`, creating thousands of GPUI elements at once.
   - **Fix Applied:** Flattened group headers and file rows into `Vec<GroupedDetailsRow>` and rendered via virtualized `uniform_list("details-grouped-list", ...)`.
6. **Removed Competing 8ms Scroll Timer (`src/ui/components/bouncy_scroll.rs`)**
   - **Previous Bug:** `bouncy_scroll_handler_with_handle` spawned a detached `8ms` timer loop calling `tick_spring(1.0 / 120.0)` while `AppWindow::render()` simultaneously called `tick_bouncy_scrolls()` on every frame, causing dual-clock micro-stutters.
   - **Fix Applied:** Removed the detached timer loop from `bouncy_scroll_handler_with_handle` so `tick_bouncy_scrolls()` drives physics cleanly at native display refresh rate; added WinUI 3 `cubic_bezier`, `fluent_entrance_ease`, and `fluent_exit_ease` helpers.

### Visual Parity, UX & Cross-Platform Fixes
7. **Enabled True Windows 11 Mica / Mica Alt & Translucent Surfaces (`src/main.rs`, `src/ui/constants.rs`, `src/ui/components/app_background.rs`)**
   - **Previous Bug:** `main.rs` passed `WindowBackgroundAppearance::Opaque` and called `apply_mica` even when `mica_alt` was requested; `UiConstants` used opaque `0xFF` surface alphas (`0x191919FF`, `0x1F1F1FFF`); `AppBackground::build` ignored `mica_enabled`, `mica_alt`, and `folder_bg`.
   - **Fix Applied:** Configured `MicaAltBackdrop` / `MicaBackdrop` + `window_vibrancy::apply_tabbed`, updated surface alphas (`BG_MAIN_RGBA = 0x191919CC`, `BG_MAIN_MICA_ALT_RGBA = 0x141414CC`, `CARD_STROKE_RGBA = 0xFFFFFF0F`), updated typography to `"Segoe UI Variable Text"` and `"Segoe Fluent Icons"`, and wired `AppBackground::build` to render custom folder background images (`desktop.ini`) and Mica tint layers.
8. **Implemented Real `Date Modified` Formatting & Natural Alphanumeric Sort (`src/ui/views/helpers.rs`, `src/ui/views/details_view.rs`, `src/ui/views/main_view.rs`, `src/app_render.rs`)**
   - **Previous Bug:** `details_view.rs` hardcoded `"\u{2014}"` (`—`) for every row's `Date Modified` column, `app_render.rs` had a slow 50-line loop from year 2000 for hover tooltips, and `visible_items()` used ASCII sort (`file10.txt` before `file2.txt`).
   - **Fix Applied:** Implemented $O(1)$ civil-from-days `format_system_time` and zero-allocation `natural_cmp` in `helpers.rs` and wired them across `details_view.rs`, `app_render.rs`, and `main_view.rs`.
9. **Rendered Git Branch Badge in Status Bar (`src/ui/components/status_bar.rs`)**
   - **Previous Bug:** `StatusBarState.git_branch` was populated on every navigation but never rendered in `StatusBarView::build`.
   - **Fix Applied:** Added the `#status-git-branch` pill and selected file size summary to `StatusBarView::build`.
10. **Restored Cross-Platform Compilation & Sidebar Visibility (`Cargo.toml`, `src/main.rs`, `src/app_render.rs`, `src/app_navigation.rs`, `src/core/ipc.rs`, `src/core/path_util.rs`, `src/platform/windows/windows_interop.rs`, `src/platform/windows/wmi_service.rs`, `src/platform/windows/thumbnail_service.rs`)**
    - **Fix Applied:** Moved `windows` crate under `[target.'cfg(windows)'.dependencies]`, guarded all remaining Win32 imports, enabled `SidebarView` and `Logger::init()` on all platforms, added `:memory:` SQLite fallback, and added macOS/Linux file-open (`open` / `xdg-open`) and image thumbnail decoding support.

### Dead Code & Useless File Cleanup
- Deleted orphaned/backup files: `src/ui/components/navigation_bar.rs.bak`, `src/ui/views/minimal_list_view.rs`, `src/ui/components/icons.rs`, `src/ui/dialog_services/file_operation_dialog.rs`, `all_files.txt`, `compile_errors.txt`, `errors.txt`, and `logs/files-rs.log.*`.
- Removed ~1,100 lines of dead duplicate view methods from `src/ui/views/main_view.rs`, `src/ui/views/main_view_grouped.rs`, and `src/ui/components/dialogs_settings.rs`.

---

## 2. Remaining Architectural Refactoring Targets (Tracked in `docs/tasks/HERMES_EXECUTION_PLAN.md`)

1. **Decompose `src/app.rs` (3,369 lines) & `src/app_actions.rs` (1,785 lines):**
   - Split `dispatch_action` into domain-specific handler modules (`actions_file.rs`, `actions_nav.rs`, `actions_tab.rs`, `actions_view.rs`).
2. **Async `UndoableAction` Trait (`src/core/undo_redo.rs`):**
   - Migrate synchronous `std::fs::rename` / `remove_file` calls inside `UndoableAction::undo` and `redo` to `tokio::fs` on the background executor.
3. **Centralize Icon Disk-Cache Lookup (`src/app.rs` & `src/domain/thumbnail_cache.rs`):**
   - Unify `AppState::icon_cache` (`HashMap<FilesPath, PathBuf>`) with `AppState::thumbnail_cache` (`ThumbnailCache`) so only one bounded LRU cache manages extracted PNG paths.

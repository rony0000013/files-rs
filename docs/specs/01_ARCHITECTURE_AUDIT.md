# 01 — Architecture Audit & Code Rectification Report (`files-rs` GPUI Edition)

**Target Repository:** `files-rs` (Rust `edition = "2024"`, `gpui = "0.2.2"`, `tokio = "1.53"`, `windows = "0.62"`, `lru = "0.18"`, `rusqlite = "0.40"`, `zip = "9.0.3"`, `lopdf = "0.45"`, `sentry = "0.49"`)  
**Reference Baselines:**
- **Visual, UX & Feature Reference:** `Files-community/` (`Files.App` WinUI 3 / Windows App SDK)
- **GPU Rendering & Layout Reference:** `zed/` (`crates/gpui` hybrid immediate/retained GPU UI, `uniform_list`, Taffy layout)
- **Frontend Strategy:** **GPUI-first** cross-platform implementation (Slint port deferred to a future phase)

---

## 1. Executive Summary

An exhaustive file-by-file audit of the updated `files-rs` codebase (140 Rust source files across `src/core/`, `src/domain/`, `src/storage/`, `src/platform/`, `src/config/`, `src/ui/`, and `src/app*.rs`) was conducted against the four core technical mandates:
1. **Visual & Behavioral Parity with `Files-community`** (Windows 11 Fluent Design, Mica / Mica Alt, `Segoe UI Variable Text`, `Segoe Fluent Icons`, `#FFFFFF0F` 1px card strokes, natural alphanumeric sorting, real timestamp formatting, and Git status badges).
2. **Locked >120 FPS / 144 Hz Frame Pacing** (`< 6.94 ms` frame budget, single-clock VSync spring physics, WinUI 3 cubic-bezier easing, and $O(\text{viewport})$ `uniform_list` virtualization across both flat and grouped views).
3. **Ultra-Low Memory Footprint** (`< 60–80 MB` active working set on standard folders, `< 120 MB` on 50,000+ file directories, bounded `<= 32 MB` raw RGBA thumbnail cache).
4. **Cross-Platform Portability & Zero Main-Thread Blocking** (clean `#[cfg(windows)]` target separation so Windows, macOS, and Linux build and run cleanly; cooperative `CancelGuard` cancellation; $O(1)$-indexed batch metadata enrichment; and single-pass Git repository status scanning).

All critical and high-severity bugs identified during this audit have been **rectified directly in `src/`**, ~1,100 lines of dead duplicate view code and 4 orphaned source/backup files have been removed, and high-signal unit tests have been added.

---

## 2. File-by-File Audit & Rectifications Applied in `src/`

### 2.1 Window Backdrop, Fluent Tokens & Surface Layering

| File | Disparity / Bug Found | Rectification Applied in Code |
| :--- | :--- | :--- |
| `src/main.rs` | Passed `WindowBackgroundAppearance::Opaque` to `gpui::WindowOptions` and always called `window_vibrancy::apply_mica(&win, Some(true))` even when `appearance.mica_alt` was enabled. Also gated `Logger::init()` behind `#[cfg(windows)]` and panicked if `files-rs.db` could not be opened on disk. | Configured `WindowBackgroundAppearance::MicaAltBackdrop` / `MicaBackdrop` when `mica_enabled` is true; wired `window_vibrancy::apply_tabbed(&win, Some(true))` for `mica_alt`; enabled `Logger::init()` on all platforms; added automatic `:memory:` SQLite fallback; added cross-platform root path fallback (`/` on Unix, `C:\` on Windows). |
| `src/ui/constants.rs` | `FONT_MAIN` was `"Segoe UI"` instead of `"Segoe UI Variable Text"`, `FONT_MDL2` was `"Segoe MDL2 Assets"` instead of `"Segoe Fluent Icons"`, and `BG_MAIN_RGBA` (`0x191919FF`), `BG_SIDEBAR_RGBA` (`0x1F1F1FFF`), `BG_TAB_BAR_RGBA` (`0x141414FF`), and `BG_STATUS_BAR_RGBA` (`0x1F1F1FFF`) had `0xFF` (100% opaque) alpha channels that completely blocked the DWM Mica backdrop. | Updated `FONT_MAIN` and `FONT_FAMILY_DEFAULT` to `"Segoe UI Variable Text"`, `FONT_MDL2` to `"Segoe Fluent Icons"`, added `CARD_STROKE_RGBA = 0xFFFFFF0F` and `BG_MAIN_MICA_ALT_RGBA = 0x141414CC`, and updated surface fills to translucent alphas (`0x191919CC`, `0x1F1F1FB3`, `0x14141499`) so Mica shines through. |
| `src/ui/components/app_background.rs` | `AppBackground::build(_folder_bg, _mica_enabled, _mica_alt, children)` ignored all three parameters and returned a plain `div().size_full()`. | Implemented `AppBackground::build` to apply the translucent Mica / Mica Alt base tint and render optional per-folder background images (`desktop.ini` `image_path`, `opacity`, and `BackgroundFit` mapping to `ObjectFit`). |
| `src/ui/components/status_bar.rs` | `StatusBarState.git_branch` was populated during navigation (`app_navigation.rs`), but `StatusBarView::build` never rendered it. | Added the `#status-git-branch` Fluent badge (with branch icon `\u{E8AD}` and branch name) to the right side of `StatusBarView::build`, plus selected file byte size formatting (`format_size(state.selected_size_bytes)`). |

### 2.2 Directory Navigation, Thread Contention & Algorithmic Bottlenecks

| File | Disparity / Bug Found | Rectification Applied in Code |
| :--- | :--- | :--- |
| `src/app_state.rs` & `src/app_navigation.rs` | `src/core/perf/cancellable_load.rs` (`CancelGuard` / `CancelToken`) existed in the crate but was never instantiated in `AppState` or checked in `AppWindow::navigate()`. Rapidly clicking folders ran overlapping Phase 1/2/2.5/3 background tasks that raced to overwrite `main_view.items`. | Added `pub nav_cancel_guard: CancelGuard` to `AppState` and wired `let cancel_token = self.state.nav_cancel_guard.reset();` across all 4 phases of `AppWindow::navigate()`. |
| `src/app_navigation.rs` (Phase 2) | Phase 2 metadata enrichment iterated over `item_paths.chunks(100)`, ran a linear search `this.state.main_view.items.iter_mut().find(|i| i.path == path)` for every item ($O(N^2)$ = 1.25B comparisons at 50k items), and cloned the entire 50,000-element `Vec<FileItem>` into `file_items` 500 times. | Recorded `(idx, FilesPath)` in Phase 1, increased batch size to `2,048`, and updated `this.state.main_view.items.get_mut(idx)` in $O(1)$ time with directory-match verification (`primary_path == target_dir`). |
| `src/domain/git_service.rs` & `src/app_navigation.rs` (Phase 2.5) | Phase 2.5 looped over every file in the directory and called `GitService::get_file_status(path)`, which called `Repository::discover(path)` per file ($N$ repository discoveries and index reads). | Implemented `GitService::get_directory_statuses(&dir_path)` which opens the repository once and runs a single `repo.statuses(Some(&mut opts))` scan, returning a `HashMap<FilesPath, GitFileStatus>`. |

### 2.3 Virtualization, Sorting, Timestamps & Animation Physics

| File | Disparity / Bug Found | Rectification Applied in Code |
| :--- | :--- | :--- |
| `src/ui/views/details_view.rs` | 1. Hardcoded `"\u{2014}"` (`—`) for the `Date Modified` column in both flat (line 257) and grouped (line 506) views, ignoring `item.modified`.<br>2. `details_view_grouped` used an unvirtualized `.overflow_y_scroll()` `div` with `.children(...)`, instantiating all items on the main thread when grouping was active. | 1. Replaced hardcoded `"—"` with `format_system_time(item.modified)`.<br>2. Refactored `details_view_grouped` to flatten group headers and file rows into `Rc<Vec<GroupedDetailsRow>>` and render via virtualized `uniform_list("details-grouped-list", ...)`. |
| `src/ui/views/helpers.rs` & `src/ui/views/main_view.rs` | `MainViewState::visible_items()` sorted `SortBy::Name` using `a.name.to_lowercase().cmp(&b.name.to_lowercase())`, which allocated two `String`s per comparison ($O(N \log N)$ heap allocations) and sorted `file10.txt` before `file2.txt`. | Implemented zero-allocation `natural_cmp(a: &str, b: &str) -> Ordering` and $O(1)$ civil-from-days `format_system_time(Option<SystemTime>) -> String` in `helpers.rs`, and wired `natural_cmp` into `MainViewState::visible_items()`. |
| `src/app_render.rs` | Contained a 50-line `loop` counting days from year 2000 inside `render()` for the hover tooltip date string. | Replaced the 50-line loop with `crate::ui::views::helpers::format_system_time(item.modified)`. |
| `src/ui/components/bouncy_scroll.rs` | `bouncy_scroll_handler_with_handle` spawned a detached `8ms` timer loop (`TICK_INTERVAL_MS`) calling `tick_spring(1.0 / 120.0)` while `AppWindow::render()` simultaneously called `tick_bouncy_scrolls()` on every frame, causing dual-clock jitter. | Removed the detached `8ms` timer loop from `bouncy_scroll_handler_with_handle` so `AppWindow::render()` drives spring physics exclusively at display refresh rate; added WinUI 3 `cubic_bezier`, `fluent_entrance_ease`, and `fluent_exit_ease` functions. |

### 2.4 Memory Bounds & Cache Eviction

| File | Disparity / Bug Found | Rectification Applied in Code |
| :--- | :--- | :--- |
| `src/platform/windows/thumbnail_service.rs` | `ThumbnailService::new()` allocated an `LruCache` of `2000` raw `ThumbnailResult` bitmaps. At `256x256x4` bytes (262 KB) each, 2,000 entries consumed up to **524 MB of RAM**. | Capped `MAX_RAW_BITMAP_CACHE_ENTRIES = 128` (**32 MB strict ceiling**), enabled the cache cross-platform, and added a non-Windows `extract_thumbnail` implementation using the `image` crate (`FilterType::Lanczos3`). |
| `src/domain/thumbnail_cache.rs` | Set `MAX_CACHE_SIZE = 1_000_000` and maintained a duplicate `index: Arc<Mutex<HashMap<String, ThumbnailEntry>>>` that was never pruned on LRU eviction, plus a `sanitize_path` function that collided when paths differed only by separator characters. | Capped `MAX_CACHE_SIZE = 4_096`, removed the leaking duplicate `HashMap`, and upgraded `sanitize_path` to append a 64-bit `DefaultHasher` hex digest. |

### 2.5 Cross-Platform Hygiene (`Windows`, `macOS`, `Linux`)

| File | Disparity / Bug Found | Rectification Applied in Code |
| :--- | :--- | :--- |
| `Cargo.toml` | Listed `windows = { version = "0.62", ... }` under unconditional `[dependencies]`, forcing Win32 crate compilation on macOS and Linux. | Moved `windows = { version = "0.62", ... }` under `[target.'cfg(windows)'.dependencies]`. |
| `src/app_render.rs` & `src/app_navigation.rs` | Gated `SidebarView`, `SidebarSplitter`, `sidebar.set_active(&path)`, and `RecycleBinService` inconsistently, hiding the Sidebar on macOS/Linux and failing compilation on `RecycleBinService::new()`. | Enabled `SidebarView`, `SidebarSplitter`, and `sidebar.set_active(&path)` across all platforms; guarded `RecycleBinService` properly; added `open` (macOS) and `xdg-open` (Linux) support in `AppWindow::open_item`. |
| `src/core/ipc.rs`, `src/core/path_util.rs`, `src/platform/windows/windows_interop.rs`, `src/platform/windows/wmi_service.rs` | Had unguarded `use windows::...` imports at the top of the file that broke non-Windows builds. | Added `#[cfg(windows)]` guards and clean `#[cfg(not(windows))]` fallbacks (`UserDirs` for known folders in `path_util.rs`). |

### 2.6 Dead Code & Useless Files Removed

1. **Deleted Orphaned / Backup Source Files:**
   - `src/ui/components/navigation_bar.rs.bak` (41 KB stale backup file)
   - `src/ui/views/minimal_list_view.rs` (unreferenced debug stub not in `views/mod.rs`)
   - `src/ui/components/icons.rs` (unreferenced debug stub not in `components/mod.rs`)
   - `src/ui/dialog_services/file_operation_dialog.rs` (unreferenced 386-line file superseded by `src/ui/components/dialogs.rs`)
   - `all_files.txt`, `compile_errors.txt`, `errors.txt`, and `logs/files-rs.log.*` (18 stale runtime logs)
2. **Deleted ~1,100 Lines of Dead Duplicate Methods:**
   - `src/ui/views/main_view.rs`: Removed dead `#[allow(dead_code)]` duplicate methods (`columns_view`, `cards_view`, `list_view`, `grid_view`, `grid_item`, `file_grid`, `file_list`) that were already extracted into `columns_view.rs`, `cards_view.rs`, `list_view.rs`, and `grid_view.rs`.
   - `src/ui/views/main_view_grouped.rs`: Removed dead `grid_view_grouped` method (superseded by `grid_view.rs`).
   - `src/ui/components/dialogs_settings.rs`: Removed dead `build_settings_dialog` modal wrapper (Settings is rendered as a tab via `build_settings_page`).

---

## 3. Audit of Existing Test Suite & Test Improvements

### 3.1 Findings on Existing Tests
- Several test modules in `src/ui/` and `src/platform/` previously tested **local helper structs defined inside `#[cfg(test)] mod tests`** rather than calling production functions, or were gated behind `#[cfg(all(test, windows))]` even when testing pure cross-platform logic (`wmi_service.rs`, `windows_interop.rs`).
- Critical hot-path algorithms (`natural_cmp`, `format_system_time`, `GitService::get_directory_statuses`, `ThumbnailCache` LRU eviction, `BouncyScrollState::tick_spring`, `fluent_entrance_ease`) had no unit tests.

### 3.2 Production Unit Tests Added in `src/`
- `src/ui/views/helpers.rs`: `test_natural_cmp_numeric_ordering`, `test_natural_cmp_leading_zeros`, `test_format_system_time_epoch_and_none`
- `src/domain/git_service.rs`: `test_get_directory_statuses_batch`
- `src/domain/thumbnail_cache.rs`: `test_thumbnail_cache_new_and_bounded_capacity`, `test_thumbnail_cache_lru_eviction`, `test_sanitize_path_collision_free`
- `src/ui/components/bouncy_scroll.rs`: `test_fluent_cubic_bezier_curves`, `test_spring_settles_without_overshoot_explosion`
- `src/ui/components/status_bar.rs`: `test_status_bar_format_size`, `test_status_bar_state_sync`
- `src/platform/windows/windows_interop.rs` & `src/platform/windows/wmi_service.rs`: Enabled cross-platform unit tests on non-Windows hosts.

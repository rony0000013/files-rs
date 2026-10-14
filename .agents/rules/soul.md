---
trigger: always_on
---

# Project: Rust-GPUI File Explorer (Files-Rust)

## Objective

A fast, memory-efficient, cross-platform, idiomatic Rust port of the `Files` (`Files-community`) file manager built on Zed's **GPUI (`0.2.2`)** GPU-accelerated UI framework.
- **Primary UI Focus:** **GPUI implementation** (`src/ui/`, `src/app*.rs`) targeting locked **120+ FPS / 144 Hz** frame pacing (`< 6.94 ms` frame time), `< 60–80 MB` idle/standard working set (`< 120 MB` at 50,000+ items), and pixel-accurate Windows 11 Fluent Design (Mica / Mica Alt, `#FFFFFF0F` 1px card strokes, `Segoe UI Variable Text`, `Segoe Fluent Icons`).
- **Deferred Port:** Any alternative frontend (such as Slint) is deferred to a future port after the GPUI implementation and core engine are 100% complete.
- **Cross-Platform Mandate:** While Windows 11 receives native Win32/COM/Shell/Mica integration (`#[cfg(windows)]`), all core navigation, views, sidebar, toolbar, status bar, thumbnails, and file operations must compile and run cleanly on **Windows, macOS, and Linux**.

## Core Stack (Rust 2024 Edition)

- **UI & GPU Pipeline:** `gpui = "0.2.2"` (DirectX / Metal / Vulkan / Blade hardware-accelerated rendering, `uniform_list` viewport virtualization).
- **Window Vibrancy:** `window-vibrancy = "0.7.1"` (`apply_tabbed` for Mica Alt, `apply_mica` for standard Mica) paired with GPUI `WindowBackgroundAppearance::MicaAltBackdrop` / `MicaBackdrop`.
- **Async & Parallel Runtime:** `tokio = "1.53"`, `rayon = "1.11"`, `jwalk = "0.8"`.
- **Database & Caching:** `rusqlite = "0.40"` (bundled SQLite for tags/indexing, with automatic `:memory:` fallback), `lru = "0.18"` (strictly bounded in-memory caches).
- **Windows Shell Interop (`[target.'cfg(windows)'.dependencies]`):** `windows = "0.62"`, `winreg = "0.56"`.

## Non-Negotiable Engineering Invariants

1. **Zero Main-Thread Blocking:** Never run uncached `std::fs`, `git2`, or Shell COM calls on the UI thread during `render()`. Use `cx.background_executor()` and `CancelGuard` (`nav_cancel_guard`) to abort stale loads on rapid navigation.
2. **Strict Viewport Virtualization:** Every flat and grouped file view (`details_view`, `grid_view`, `columns_view`) must use GPUI's `uniform_list` so layout and GPU cost remain $O(\text{viewport rows})$.
3. **Bounded Memory Caches:** Never allocate unbounded `HashMap` or multi-million-entry caches for bitmaps. Raw RGBA bitmaps in `ThumbnailService` are capped at `128` entries (`<= 32 MB`), and path-level `ThumbnailCache` is capped at `4,096` entries.
4. **Single-Clock Animation Physics:** Drive scroll momentum and spring physics exclusively from `AppWindow::render()` (`tick_bouncy_scrolls`) at the monitor's native VSync rate. Never spawn detached `8ms` timer loops that fight the render loop.
5. **Cross-Platform `#[cfg(windows)]` Hygiene:** Keep `windows` and `winreg` under `[target.'cfg(windows)'.dependencies]` and never hide core UI components (`SidebarView`, `StatusBarView`, `NavigationBarView`) behind `#[cfg(windows)]`.

## Behavioral Guidelines

1. **Think Before Coding:** Verify against `Files-community/` and `zed/` reference implementations in the repo, and consult `docs/specs/` and `docs/tasks/HERMES_EXECUTION_PLAN.md`.
2. **Simplicity First:** Delete dead/duplicate view implementations instead of leaving unreachable code.
3. **Surgical Changes:** Touch only what is needed and clean up any orphaned imports or files.
4. **Goal-Driven Execution & High-Value Testing:** Write unit and performance tests that exercise real production functions (`natural_cmp`, `format_system_time`, `GitService::get_directory_statuses`, `ThumbnailCache` LRU eviction, `BouncyScrollState::tick_spring`).

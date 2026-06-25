# Files-RS Codebase Audit — Remaining Issues

> **Date:** 2026-06-25 (final update)
> **Scope:** Full codebase (~63K lines, ~140 `.rs` files)
> **Reference:** DESIGN_SPEC.md (Files-community C#/WinUI)
>
> **Fixed across all rounds:** 49 issues (8 Critical + 11 High + 30 Medium/Low)
> This report contains only **unresolved** issues.

---

## MEDIUM — Performance (need architectural refactor)

### 1. Blocking I/O in navigation (synchronous `read_dir`)

**File:** `src/app_navigation.rs:107`

Phase 1 listing uses synchronous `std::fs::read_dir` inside an async block. Needs `spawn_blocking` or `tokio::fs`.

### 2. Blocking `std::fs` I/O inside `UndoableAction`

**File:** `src/core/undo_redo.rs:78,82,96,100`

Needs async redesign of the `UndoableAction` trait.

---

## MEDIUM — Code Quality (broad refactors)

### 3. Excessive `.clone()` — 376 occurrences

GPUI entity clones are required by framework; `FilesPath`/`String` clones are the main targets.

### 4. `dispatch_action` is 1771 lines

Split into sub-methods by category.

### 5. Keyboard string matching

Use GPUI typed key events instead of `"escape" | "Escape"`.

---

## LOW — Style

### 6-14. Minor style items

- `AppWindow::new()` inline restore logic
- Duplicated omnibar-dismiss logic (4 copies)
- Duplicated checkbox rendering (3 copies)
- `on_mouse_down` vs `on_click`
- Dialog focus trapping
- `SearchService` dead code
- Logger hardcoded path
- metadata.rs unused reads

---

## Reference Deviation (15 missing features)

Win10 Acrylic fallback, accent color detection, theme detection, high-contrast mode, tab drag, sidebar context menus, natural sort, compact overlay, tray persistence, relative date refresh, app background image.

---

## Architecture (5 items)

God objects (AppState, MainViewState), file watching, async_trait on sync impls, duplicate service patterns.

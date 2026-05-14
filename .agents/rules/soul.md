---
trigger: always_on
---

# Project: Rust-GPUI File Explorer (Files-Rust)

## Objective

A high-performance, idiidiomatic Rust port of the "Files" (Files-community) file manager.
Targeting 120fps UI using GPUI, with a focus on Windows (Mica/Fluent) while maintaining a clean abstraction for macOS/Linux. Rmember to look into the Files-community before porting.

## Core Stack

- **UI:** `gpui` (Metal/Vulkan/DirectX backed rendering).
- **Shell Integration (Windows):** `windows-rs` for COM/Shell API access.
- **Vibrancy:** `window-vibrancy` for Mica/Acrylic.
- **I/O:** `tokio` for async FS operations, `rayon` for CPU-bound metadata processing.
- **Database:** `sqlite` (via `rusqlite`) for indexing and file tagging (replacing C# internal caches).
- **Serialization:** `serde` for settings and state.

## Architecture & Design Patterns

### 1. Model-View-Action (MVA)

Instead of MVVM, we use GPUI's native patterns:

- **Models:** Managed by `ModelContext`, representing the state (e.g., `FolderContents`, `Sidebar`).
- **Views:** Reactive components that render based on Model state.
- **Actions:** Centrally defined structs for user intent (e.g., `OpenFile`, `NavigateBack`).

### 2. The Storage Abstract Layer (SAL)

Mirroring `Files.Core.Storage` but idiomatic:

- `Storable` trait: Common interface for Files and Folders.
- `StorageService` trait: Implementation for `LocalFS`, `FTP`, `Archive`, and `Cloud`.
- **Abstraction implementation:** Use `opendal` to provide a unified interface for local and cloud storage.

### 3. Virtualized Layouts

GPUI's `List` and `UniformList` will handle high-cardinality directories (100k+ items) with zero UI lag, replacing the XAML `GridView/ListView`.

# Behavior
Behavioral guidelines to reduce common LLM coding mistakes. Merge with project-specific instructions as needed.

**Tradeoff:** These guidelines bias toward caution over speed. For trivial tasks, use judgment.

## 1. Think Before Coding

**Don't assume. Don't hide confusion. Surface tradeoffs.**

Before implementing:
- State your assumptions explicitly. If uncertain, ask.
- If multiple interpretations exist, present them - don't pick silently.
- If a simpler approach exists, say so. Push back when warranted.
- If something is unclear, stop. Name what's confusing. Ask.

## 2. Simplicity First

**Minimum code that solves the problem. Nothing speculative.**

- No features beyond what was asked.
- No abstractions for single-use code.
- No "flexibility" or "configurability" that wasn't requested.
- No error handling for impossible scenarios.
- If you write 200 lines and it could be 50, rewrite it.

Ask yourself: "Would a senior engineer say this is overcomplicated?" If yes, simplify.

## 3. Surgical Changes

**Touch only what you must. Clean up only your own mess.**

When editing existing code:
- Don't "improve" adjacent code, comments, or formatting.
- Don't refactor things that aren't broken.
- Match existing style, even if you'd do it differently.
- If you notice unrelated dead code, mention it - don't delete it.

When your changes create orphans:
- Remove imports/variables/functions that YOUR changes made unused.
- Don't remove pre-existing dead code unless asked.

The test: Every changed line should trace directly to the user's request.

## 4. Goal-Driven Execution

**Define success criteria. Loop until verified.**

Transform tasks into verifiable goals:
- "Add validation" → "Write tests for invalid inputs, then make them pass"
- "Fix the bug" → "Write a test that reproduces it, then make it pass"
- "Refactor X" → "Ensure tests pass before and after"

For multi-step tasks, state a brief plan:
```
1. [Step] → verify: [check]
2. [Step] → verify: [check]
3. [Step] → verify: [check]
```

Strong success criteria let you loop independently. Weak criteria ("make it work") require constant clarification.

---

**These guidelines are working if:** fewer unnecessary changes in diffs, fewer rewrites due to overcomplication, and clarifying questions come before implementation rather than after mistakes.

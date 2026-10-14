# Agent Guide & Specification Index (`files-rs`)

## 1. Architecture & Execution Specifications (`docs/`)

Autonomous coding agents (including **Hermes Agent**) working on `files-rs` must read and follow these specification documents in order:

1. **`docs/specs/01_ARCHITECTURE_AUDIT.md`** — Comprehensive architectural audit of the repository, resolved critical bugs, remaining refactoring targets, Fluent Design token table, and comparison against `Files-community` (WinUI 3) and `zed` (GPUI).
2. **`docs/specs/02_CORE_ENGINE_SPEC.md`** — Core engine specification (`src/core/`, `src/domain/`, `src/storage/`): `CancelGuard` cooperative cancellation, $O(1)$-indexed progressive directory loading, single-pass `GitService::get_directory_statuses`, and 32 MB bounded `ThumbnailService` / `ThumbnailCache` pipelines.
3. **`docs/specs/03_GPUI_IMPLEMENTATION_PLAN.md`** — GPUI frontend specification (`src/ui/`, `src/app*.rs`): Windows 11 Mica / Mica Alt backdrop pipeline, translucent surface layering, `uniform_list` virtualization across flat and grouped views, single-clock display-refresh-rate spring physics, and WinUI 3 cubic-bezier easing (`fluent_entrance_ease`, `fluent_exit_ease`).
4. **`docs/specs/04_TESTING_AND_CROSS_PLATFORM_SPEC.md`** — Cross-platform portability architecture (Windows, macOS, Linux `#[cfg(windows)]` rules) and high-value unit, property, and performance benchmark testing specification (replacing the deferred Slint port).
5. **`docs/tasks/HERMES_EXECUTION_PLAN.md`** — Step-by-step, deterministic task execution plan with exact file paths, concrete Rust interfaces, acceptance criteria, and `cargo` verification commands.
6. **`docs/benchmarks/PERF_AND_TELEMETRY.md`** — Performance telemetry harness for validating `>120 FPS` (`< 6.94 ms` frame time), `< 80 MB` standard RAM (`< 120 MB` at 50k items), and `< 80 ms` cold startup.

---

## 2. Core Build & Verification Commands

- **Type & Borrow Check:** `cargo check --all-targets`
- **Linter (Zero Warnings):** `cargo clippy --all-targets -- -D warnings`
- **Unit & Integration Test Suite:** `cargo test --lib`
- **Release Binary:** `cargo build --release`

---

## 3. `graphify` Knowledge Graph Rules

This project has a knowledge graph at `graphify-out/` with god nodes, community structure, and cross-file relationships (plus reference graphs in `Files-community/graphify-out/` and `zed/graphify-out/`).

When the user types `/graphify`, invoke the `skill` tool with `skill: "graphify"` before doing anything else.

Rules:
- For codebase questions, first run `graphify query "<question>"` when `graphify-out/graph.json` exists. Use `graphify path "<A>" "<B>"` for relationships and `graphify explain "<concept>"` for focused concepts. These return a scoped subgraph, usually much smaller than `GRAPH_REPORT.md` or raw grep output.
- Dirty `graphify-out/` files are expected after hooks or incremental updates; dirty graph files are not a reason to skip graphify. Only skip graphify if the task is about stale or incorrect graph output, or the user explicitly says not to use it.
- If `graphify-out/wiki/index.md` exists, use it for broad navigation instead of raw source browsing.
- Read `graphify-out/GRAPH_REPORT.md` only for broad architecture review or when `query`/`path`/`explain` do not surface enough context.
- After modifying code, run `graphify update .` to keep the graph current (AST-only, no API cost).

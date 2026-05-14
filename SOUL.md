# Project: Rust-GPUI File Explorer (Files-Rust)

## Objective

A high-performance, idiidiomatic Rust port of the "Files" (Files-community) file manager.
Targeting 120fps UI using GPUI, with a focus on Windows (Mica/Fluent) while maintaining a clean abstraction for macOS/Linux. Rmember to look into the Files-community before porting.

## Project Structure
src/
    ├── app.rs                 # AppWindow and AppState logic
    ├── lib.rs                 # Crate root, module declarations
    ├── main.rs                # Entry point
    ├── core/                  # Cross-platform core logic
    ├── config/                # Configuration and settings
    ├── domain/                # Data models and domain services
    ├── localization/          # i18n logic
    ├── storage/               # Storage abstraction layer
    ├── platform/              # OS-specific implementations
    │   ├── windows/
    │   ├── macos/             # Ready for future implementation
    │   └── linux/             # Ready for future implementation
    └── ui/                    # GPUI User Interface
        ├── components/        # Reusable UI widgets
        ├── views/             # High-level page/view layouts
        └── theme/             # Design tokens and layout logic

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

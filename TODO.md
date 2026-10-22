# Files-RS — Comprehensive Feature Parity & Engineering Audit (`TODO.md`)

> **Legend:**
> - `[x]` **Implemented & Wired End-to-End** — Functional in both backend logic and the live GPUI `AppWindow` UI.
> - `[~]` **Half-Baked / Scaffolded / Partially Wired** — Structs, services, or unit tests exist in `src/`, but the feature is either not connected to the live `AppWindow` UI, uses mock/placeholder data, or relies on partial stubs.
> - `[ ]` **Missing / Not Implemented** — Feature present in `Files-community` (`Files.App`) that has not yet been built in `files-rs`.

## 0. Executive Summary of Codebase State (2026-10 Audit)

| Subsystem | Status | Working End-to-End (`[x]`) | Half-Baked / Unwired Scaffolds (`[~]`) | Missing vs. `Files-community` (`[ ]`) |
| :--- | :--- | :--- | :--- | :--- |
| **Window Chrome, Mica & Tab Bar** | **Solid** | Custom Win32 caption drag/snap/maximize, live Mica backdrop, dynamic tab sizing (`96..200px`), pinned tabs, overflow scroll, live tab drag-reorder, drag-out to new window, non-shrinking caption buttons (`Min`/`Max`/`Close`) | Cross-window tab docking into an *existing* window (`tab_interactions.rs` is standalone); tab hover preview card | Native Win11 Snap Layouts flyout on maximize button hover (`HTMAXBUTTON`) |
| **File Views (`Details`, `Grid`, `List`, `Columns`)** | **Solid** | All 4 views virtualized (`uniform_list` + `ListSizingBehavior::Auto`), live rubber-band marquee selection, `Shift`/`Ctrl` multi-select, double-click open, inline `F2` rename, multi-column `List` view, 3-column Miller `Columns` view | `AdaptiveViewService` (`adaptive_view.rs`) is not wired to auto-switch layout on folder navigation; `GroupedDetailsRow` drag-reorder between groups is a stub | Column re-ordering via drag-and-drop in `Details` header; arbitrary N-depth horizontal scrolling Miller columns |
| **Dual-Pane / Split View** | **Solid** | Independent `Primary` & `Secondary` pane states, independent directory loading & back/forward history, pane header bars, draggable splitter (`20%..80%`), `Tab`/`F6` focus toggle, promote-on-close | Cross-pane drag-and-drop file copy/move between left and right panes | Synchronized dual-pane scrolling / compare folders mode |
| **Scroll & Spring Physics** | **Solid** | Display-refresh-rate spring physics, strict content-vs-viewport clamping (`content_height <= viewport_height` locks at `0.0`), no blank-space overscroll in Sidebar or MainView | Custom visual scrollbar thumb drag in `Grid`/`List`/`Columns` views | Horizontal scrollbar in `List` view when columns overflow horizontally |
| **Keyboard Navigation & Shortcuts** | **Solid** | Case-insensitive `ActionRegistry`, 2D arrow navigation in `Grid` & multi-column `List`, `Shift+Arrow` range selection with auto-scroll, `PageUp`/`PageDown`, `Home`/`End`, `Space`, type-ahead file jump, `Ctrl+1..9`, `Ctrl+Shift+Left/Right`, `Ctrl+Enter`/`Shift+Enter`/`Alt+Enter` | `QuickJumpDialog` (`Ctrl+G`, `quick_jump_dialog.rs`) is standalone and not wired into `DialogType` | Full custom multi-chord keybinding recorder in Settings -> Actions |
| **Storage, Transfers & Undo/Redo** | **Partial** | Async phased directory loading, `ProgressEngine` copy/move/delete jobs, `StatusCenter` flyout, `UndoRedoManager` for Rename & CreateFolder | `ConflictResolutionService`, `FileLockService` (RestartManager stub), `DebouncedFileWatcher`, `TaskbarProgressService`, and `BulkOps` are not wired into live transfers; `UndoRedoManager` does not track Copy/Move/Delete | Native `IFileOperation` COM shell transfer engine (for UAC elevation & shell undo integration); FTP/SFTP/WebDAV/Cloud `opendal` VFS |
| **Windows Shell & System Integration** | **Partial** | Win32 `IContextMenu` classic verbs (`classic_menu.rs`), `ThumbnailService` (`IShellItemImageFactory`) + disk/LRU cache, `RecycleBinService`, `DriveService`, `CloudDriveService`, `WslService` | `JumpListService`, `QuickAccessService`, `RecentItemsService`, `ShellNewService`, `BitLockerService`, `MtpDeviceService`, `DefaultFileManagerService`, `ToastService`, `WallpaperService`, `FormatDriveService`, `RunAsService`, `StartMenuPinService` are standalone scaffolds | Native Win32 OLE drag-and-drop (`IDragSource`/`IDropTarget`) to/from external apps (Explorer, browsers, Discord) |
| **Search & Omnibar** | **Partial** | Omnibar Path/Search/CommandPalette modes, `nucleo` fuzzy filter, cancellable recursive deep search (`do_deep_search`) | Search history suggestions in `navigation_bar.rs` use hardcoded mock strings; AQS parser (`kind:`, `size:`, `date:`) is not wired into `do_deep_search` | Windows Search Indexer (`Windows.Storage.Search` / OLE DB) fast indexed queries |
| **Preview, Details & Shelf Panes** | **Partial** | Text/Code/Markdown/HTML/PDF/Audio/Archive metadata preview in right side pane; `Space` triggers QuickLook/PowerToys Peek/SeerPro; `Shelf` staging pane | Media preview shows a static `"Player placeholder"` box; `IPreviewHandler` (`shell_preview_service.rs`) returns a placeholder string; `RichPreview` (`rich_preview.rs`) is unwired | Interactive PDF page rendering, real audio/video playback controls, live Office/HTML `IPreviewHandler` HWND hosting |
| **Git Integration** | **Partial** | Single-pass `git2` file status badges, status bar branch pill, `git init`, `git clone` dialog | Status bar branch click & git actions (`Fetch`, `Pull`, `Push`, `Sync`, branch switch/create) are not wired in UI | `GitHubLoginDialog` OAuth flow, commit history graph, stage/unstage/commit UI |

## 1. Completed Core UI & UX Fixes (2026-10)

- [x] **Scroll Clamping & Bouncy Scroll Physics (`src/ui/components/bouncy_scroll.rs`, `src/ui/components/sidebar.rs`, `src/ui/views/*.rs`)**
  - [x] Added `BouncyScrollState::can_scroll()` and strict `content_height <= viewport_height + 1.0` clamping so neither Sidebar nor MainView scrolls or bounces when all items fit on screen.
  - [x] Replaced hardcoded `400.0px` viewport height estimate in `bouncy_scroll_handler_with_handle` with true dynamic viewport height calculation from window bounds.
  - [x] Added `.with_sizing_behavior(ListSizingBehavior::Auto)` on all `uniform_list` instances (`Details`, `Grid`, `List`, `Columns`).
  - [x] Added `compute_sidebar_content_height()` and `compute_sidebar_viewport_height()` in `sidebar.rs` and updated bounds every frame in `app_render.rs`.
- [x] **Top Tab Bar & Caption Buttons (`src/ui/components/tab_bar.rs`, `src/core/navigation_manager.rs`, `src/app.rs`, `src/main.rs`)**
  - [x] Fixed caption buttons (`Minimize`, `Maximize/Restore`, `Close`) disappearing at small window widths by making all right-hand chrome `flex_shrink_0`, wrapping the tab strip in `.min_w(px(0.0)).overflow_hidden()`, and enforcing `window_min_size: Some(size(px(500.0), px(340.0)))`.
  - [x] Implemented `compute_strip_metrics(window_width, tab_count, pinned_count)` for responsive tab widths (`TAB_MAX_W = 200.0` down to `TAB_MIN_W = 96.0`, `TAB_PINNED_W = 44.0`).
  - [x] Fixed multi-tab scrolling area so all tabs remain in the strip, scroll smoothly via wheel or chevron buttons, and auto-scroll (`ensure_active_tab_visible`) when creating, switching, or cycling tabs.
  - [x] Implemented live tab drag-and-drop reordering and vertical drag-out (`y < -12.0 || y > 52.0`) to detach a tab into a new independent window.
  - [x] Implemented tab pinning (`pin_tab` / `toggle_pin_tab`) with compact icon-only pinned tabs anchored to the left of unpinned tabs.
- [x] **Independent Dual-Pane / Split View (`src/app_state.rs`, `src/app_navigation.rs`, `src/ui/views/main_view.rs`, `src/app.rs`, `src/app_render.rs`)**
  - [x] Added dedicated `secondary_view: MainViewState` and `secondary_file_items: Entity<FileItems>` on `AppState` so the secondary pane has independent directory items, selection, scroll offset, rename state, and lasso state.
  - [x] Added per-pane history stacks (`secondary_history_back`, `secondary_history_forward`) on `TabState` and `navigate_pane(path, pane_focus, cx)` in `app_navigation.rs`.
  - [x] Added interactive pane header bars (showing active accent indicator, folder icon, folder name, full path, and close `×` button) and a draggable splitter (`dual_pane_ratio` clamped to `0.20..0.80`).
  - [x] Added `Tab` and `F6` keyboard shortcut support to switch focus between `Primary` and `Secondary` panes.
- [x] **All 4 File Views (`Details`, `Grid`, `List`, `Columns`), Rubber-Band Selection & Keyboard Parity (`src/ui/views/*.rs`, `src/app.rs`, `src/core/action_registry.rs`)**
  - [x] Implemented live rubber-band (marquee) drag selection with visual selection overlay (`MainView::render_lasso_box`) across `Details`, `Grid`, `List`, and `Columns` views.
  - [x] Upgraded `Grid` view (`grid_view.rs`) with `Shift`/`Ctrl` click selection, double-click open, inline `F2` rename input, context menu, and 2D keyboard navigation.
  - [x] Rebuilt `List` view (`list_view.rs`) as a Files-community multi-column flow list with `Shift`/`Ctrl` selection, double-click open, inline `F2` rename, rubber-band selection, and `Left`/`Right` column-jumping arrow navigation.
  - [x] Rebuilt `Columns` view (`columns_view.rs`) as an interactive 3-column Miller browser (Parent column, Current directory column with live rubber-band selection and inline rename, and Child directory / file preview column) with `Left`/`Right` hierarchy navigation.
  - [x] Fixed `ActionRegistry::lookup` case-sensitivity bug (previously lowercased GPUI keystrokes failed to match `"F2"`, `"Delete"`, `"F5"`, `"Escape"`, `"F1"`).
  - [x] Added 2D view-aware arrow keys (`Up`/`Down`/`Left`/`Right`), `Shift+Arrow` range selection from `selection_anchor_index`, `PageUp`/`PageDown`, `Home`/`End` with automatic `scroll_item_into_view`, `Space`/`Ctrl+Space` selection toggle, type-ahead incremental file selection (`handle_type_to_select`), `Ctrl+Enter` (open folder in new tab), `Shift+Enter` (open folder in new window), `Alt+Enter` (Properties), `Ctrl+Shift+N` (New Folder), `Shift+Delete` (Permanent Delete), `Ctrl+Shift+1..4` (View modes), `Ctrl+Plus`/`Ctrl+Minus`/`Ctrl+0` (Zoom), `Ctrl+1..9` (Tab index switch), and `Ctrl+Shift+Left/Right` (Move active tab).

## 2. Half-Baked, Scaffolded, or Partially Wired Modules (`[~]`)

The following modules exist in `src/` and have unit tests, but are **not yet wired into the live application** or contain partial stubs:

### 2.1 Unwired Core & Storage Services
- [~] **Debounced File Watcher (`src/core/debounced_file_watcher.rs`)**
  - *Current state*: Implements a 300ms batching watcher with unit tests, but `app.rs` (`start_file_watcher_for`) still spawns a raw `notify::RecommendedWatcher` that triggers a full `self.refresh(cx)` on the very first event.
  - *Needed*: Replace raw `notify` in `app.rs` with `DebouncedFileWatcher` and apply incremental item deltas (`Created`/`Modified`/`Removed`) without resetting scroll or re-reading the entire directory.
- [~] **Conflict Resolution Service (`src/core/conflict_resolution.rs` & `src/ui/dialog_services/conflict_dialog.rs`)**
  - *Current state*: `ConflictResolutionService` and `DialogType::Conflict` exist in code, but `src/core/transfer.rs` (`run_copy_job` / `run_move_job`) overwrites or fails without prompting the user via `DialogType::Conflict`.
  - *Needed*: Pause transfer jobs on destination collision in `transfer.rs`, open `DialogType::Conflict`, and resume with `Replace`, `Skip`, or `KeepBoth` (auto-rename).
- [~] **File Lock Detection (`src/core/file_lock_service.rs`)**
  - *Current state*: Struct holds `_stub: u64`; `get_locking_processes`, `force_unlock`, and `shutdown_process` are stubs and never called when delete/move fails with `ERROR_SHARING_VIOLATION`.
  - *Needed*: Wire real Win32 Restart Manager API (`RmStartSession`, `RmRegisterResources`, `RmGetList`, `RmShutdown`) and surface a "File in Use" dialog during failed transfers/deletes.
- [~] **Bulk Rename Engine (`src/core/bulk_ops.rs`)**
  - *Current state*: `bulk_ops.rs` supports regex/pattern/numbering/case transforms, but `AppWindow::execute_bulk_rename` in `app_actions.rs` only performs a basic `String::replace(find, replace)` and ignores `bulk_ops.rs`.
  - *Needed*: Wire `BulkRenameDialog` UI fields (regex, numbering sequence, case transformation, live preview list) to `bulk_ops.rs` and record a batch `UndoableAction`.
- [~] **Undo / Redo Completeness (`src/core/undo_redo.rs`)**
  - *Current state*: Only `RenameAction` and `CreateFolderAction` implement `UndoableAction`. Copy, Move, Paste, Delete, and Bulk Rename are not pushed to `UndoRedoManager`.
  - *Needed*: Add `CopyAction`, `MoveAction`, `DeleteToTrashAction`, and `BulkRenameAction` to `UndoRedoManager`.
- [~] **Folder Size Calculation (`src/storage/folder_size.rs`)**
  - *Current state*: Standalone recursive size calculator; never invoked when rendering directories in `Details` view or `Properties` dialog unless `calculate_folder_sizes` is manually triggered.
  - *Needed*: Wire background folder size calculation into Phase 2.5 when `config.view.calculate_folder_sizes` is enabled, and display live-updating folder sizes in `Details` view and `Properties`.
- [~] **NTFS Alternate Data Streams (`src/storage/ads.rs`)**
  - *Current state*: Shells out to `powershell.exe Get-Item -Stream *` instead of using native Win32 `FindFirstStreamW` / `FindNextStreamW`, making it too slow for per-file operations.
  - *Needed*: Replace PowerShell subprocess calls with native Win32 `FindFirstStreamW` and `CreateFileW("path:stream")` APIs.
- [~] **Desktop.ini Parser (`src/storage/desktop_ini.rs` & `src/platform/windows/desktop_ini_service.rs`)**
  - *Current state*: `storage/desktop_ini.rs` only parses custom `BackgroundImage=` and `BackgroundOpacity=` lines; `platform/windows/desktop_ini_service.rs` parses `IconResource` and `LocalizedResourceName` but is completely unwired from `app_navigation.rs`.
  - *Needed*: Unify both modules and apply custom folder icons (`IconResource`) and localized folder display names (`LocalizedResourceName`).
- [~] **Performance Helpers (`src/core/perf/*.rs`)**
  - *Current state*: `benchmarks.rs`, `mmap_preview.rs`, `parallel_batch.rs`, and `tracing_spans.rs` are standalone test modules not called by `app_navigation.rs` or `preview_service.rs`.
  - *Needed*: Wire `mmap_preview.rs` into `load_text_preview` for large files and `tracing_spans.rs` into the navigation pipeline.

### 2.2 Unwired Windows Platform Services (`src/platform/windows/*.rs`)
- [~] **Taskbar Progress Indicator (`src/platform/windows/taskbar_service.rs`)**
  - *Current state*: Contains `_stub: ()` with no-op methods instead of `ITaskbarList3::SetProgressValue` / `SetProgressState`, and is not connected to `ProgressEngine`.
- [~] **Toast Notifications (`src/platform/windows/toast_service.rs`)**
  - *Current state*: Standalone module; never called when long-running archive/copy/move jobs finish in `AppWindow::handle_job_update`.
- [~] **Shell New Menu (`src/platform/windows/shell_new_service.rs`)**
  - *Current state*: Registry scanner for `ShellNew` templates exists in `shell_new_service.rs`, but the context menu's `New >` flyout in `dialogs_context.rs` only shows hardcoded `New Folder` and `New Text Document`.
- [~] **Jump List, Quick Access & Recent Items (`jump_list_service.rs`, `quick_access_service.rs`, `recent_items_service.rs`)**
  - *Current state*: Standalone services; `app_navigation.rs` only logs `tracing::info!("Adding {} to jump list", path_str)` without calling `JumpListService`, and `main_view_home.rs` displays 4 hardcoded placeholder rows (`"Project_Proposal.docx"`, `"Budget_2025.xlsx"`, etc.) instead of calling `RecentItemsService`.
- [~] **BitLocker, MTP Devices, Format Drive, WMI & Wallpaper (`bitlocker_service.rs`, `mtp_device_service.rs`, `format_drive_service.rs`, `wmi_service.rs`, `wallpaper_service.rs`)**
  - *Current state*: Standalone services not wired into Sidebar drive badges, drive context menus, or image context menus (`Set as Wallpaper`).
- [~] **Single-Instance IPC, System Tray, Startup & Default File Manager (`ipc.rs`, `tray_service.rs`, `startup_service.rs`, `default_file_manager_service.rs`, `launcher_service.rs`, `update_service.rs`, `crash_reporter.rs`)**
  - *Current state*: None of these lifecycle services are initialized in `src/main.rs`.

### 2.3 Half-Baked UI Components & Previews
- [~] **Localization (`src/localization/localization.rs`)**
  - *Current state*: `LocalizationManager::t()` is a stub that returns the raw key string verbatim; UI components use hardcoded English strings rather than `.ftl` lookups.
- [~] **Bounded GPU Image Cache (`src/ui/components/bounded_image_cache.rs`)**
  - *Current state*: Allocated on `AppState`, but wrapping `MainView` in `gpui::image_cache(...)` in `app_render.rs` was reverted because it blanked the viewport in GPUI 0.2.2.
- [~] **Rich Previews & Shell Preview Handler (`src/ui/components/rich_preview.rs`, `src/platform/windows/shell_preview_service.rs`, `src/ui/views/main_view_preview.rs`)**
  - *Current state*: Audio/video preview renders a static `"Player placeholder"` box; `shell_preview_service.rs` returns a static message (`"Preview is not yet implemented for this file type"`); `rich_preview.rs` is unwired.
- [~] **Compact Overlay (`src/ui/components/compact_overlay.rs`)**
  - *Current state*: `enter_compact_overlay` only hides the sidebar in state without setting `HWND_TOPMOST` (`SetWindowPos`) or resizing the OS window to a Picture-in-Picture mini-window.
- [~] **Standalone UI Component Modules (`background_warmup.rs`, `breadcrumb_bar.rs`, `empty_states.rs`, `quick_jump_dialog.rs`, `relative_date_formatter.rs`, `tab_interactions.rs`)**
  - *Current state*: Built as standalone structs with unit tests, while `navigation_bar.rs`, `tab_bar.rs`, and `main_view.rs` implement their own inline equivalents.
- [~] **Omnibar Search History & AQS Filtering (`src/ui/components/navigation_bar.rs`, `src/domain/search.rs`)**
  - *Current state*: `navigation_bar.rs` populates search history suggestions from a hardcoded `vec!["documents", "reports", "todo.txt", "images"]`; `do_deep_search` in `app.rs` only matches filenames and does not use the AQS parser (`kind:`, `size:`, `date:`) from `domain/search.rs`.

## 3. Missing Features Compared to `Files-community` (`[ ]`)

- [ ] **Native OS External Drag-and-Drop (`IDragSource` / `IDropTarget` OLE Interop)**
  - Dragging files out of `files-rs` into Windows Explorer, web browsers, Discord, or IDEs, and dropping external files into `files-rs` panes, tabs, or sidebar items.
- [ ] **Cross-Pane & Cross-Tab File Drag-and-Drop**
  - Dragging selected items from the Primary pane to the Secondary pane (or hovering over a tab / breadcrumb segment / sidebar folder to auto-navigate and drop).
- [ ] **Native Windows `IFileOperation` Shell Engine**
  - Delegating copy/move/delete/rename to Windows `IFileOperation` COM interface so UAC elevation prompts, shell extensions, and Explorer undo history work natively.
- [ ] **Full Git Workflow UI in Status Bar & Context Menu**
  - Branch switcher popup on clicking the status bar branch pill, create/delete branch dialog (`AddBranchDialog`), `Fetch` / `Pull` / `Push` / `Sync` buttons with credential/SSH support, and `GitHubLoginDialog` OAuth integration.
- [ ] **Windows Search Indexer Integration (`Windows.Storage.Search` / OLE DB)**
  - Instant indexed sub-second global search across millions of files via the Windows Search service before falling back to manual directory walking.
- [ ] **Cloud Storage Sync Status & Context Menus (`CloudFilterService` / WinRT StorageProvider)**
  - Real per-file cloud hydration status icons (Available offline, Online-only, Syncing, Pinned) and "Always keep on this device" / "Free up space" context menu actions.
- [ ] **Remote Network Protocols (FTP / SFTP / WebDAV / S3)**
  - Virtual filesystem (`StorageService` dispatch) for mounting and browsing `ftp://`, `sftp://`, and WebDAV shares directly inside tabs.
- [ ] **Interactive Media & Document Previewers**
  - Real audio/video playback controls in the Preview Pane, multi-page rendered PDF canvas, and live Win32 `IPreviewHandler` child-HWND hosting for Office documents.
- [ ] **Details View Column Drag-Reorder & Per-Column Sort/Filter Flyouts**
  - Dragging column headers to reorder columns, plus additional optional columns (`Date Created`, `Original Path`, `Sync Status`, `Bitrate`, `Dimensions`).
- [ ] **Native Win11 Snap Layouts Flyout on Maximize Button (`HTMAXBUTTON`)**
  - Returning `HTMAXBUTTON` in `WM_NCHITTEST` when hovering the custom maximize caption button so Windows 11 displays the native Snap Layouts popup grid while still triggering maximize on click.

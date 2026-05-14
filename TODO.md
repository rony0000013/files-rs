# Files-RS — Feature Parity TODO

> Legend: `[x]` = Implemented (may be stub/scaffold), `[~]` = Partial/Scaffold only, `[ ]` = Not started
> Source of truth: `Files-community/src/` vs `src/`

---

## 1. Core Infrastructure

- [x] **Logging** — `tracing` + file rotation via `tracing-appender` (`src/logger.rs`)
- [x] **Settings / Config** — `AppConfig` with serde JSON persist, load/save (`src/config.rs`)
  - [x] Settings migration / version handling
  - [x] Per-folder layout preferences (implemented via `FolderPreferences` in `AppConfig`)
  - [x] Live settings change propagation to open views (implemented via `Entity<SettingsModel>` notifications)
- [x] **Localization** — `fluent-templates` scaffolded (`src/localization.rs`)
  - [x] Actual `.ftl` locale files wired to all UI strings
  - [x] Runtime language switching (implemented via `LocalizationManager::set_language` and `Entity<LocalizationManager>`)
- [x] IPC / Single-instance — Listener scaffolded (`src/ipc.rs`)
- [x] Actually prevent second launch and focus existing window (implemented via Windows Named Mutex and `SetForegroundWindow`)
- [x] **Path utilities** — `FilesPath` with `dunce` canonicalization, ancestor chain (`src/path_util.rs`)
  - [x] Shell special paths: `Shell:RecycleBinFolder`, `Shell:MyComputerFolder`, `Shell:NetworkPlacesFolder` (implemented via `resolve_shell_path`)
  - [x] Library paths (Documents, Pictures, Music, Videos via Windows known folders) (implemented via `resolve_known_folder`)

---

## 2. Storage Abstraction Layer (`src/storage.rs`)

**Section 2 Test Coverage Added (detailed per feature):**

|| Feature | Test Names ||
|---------|------------|
| **Bulk rename** (`src/core/bulk_ops.rs`) | `test_rename_single_basic`, `test_rename_single_multiple_occurrences`, `test_rename_with_pattern_basic`, `test_preview_rename`, `test_preview_rename_empty_items`, `test_preview_rename_no_matches` |
| **Flatten folder** (`src/core/flatten.rs`) | `test_flatten_basic`, `test_flatten_with_conflicts`, `test_preview_flatten`, `test_flatten_nested_subfolders` |
| **Smart Decompression** (`src/storage/archive_service.rs`) | `test_create_and_extract_zip_file`, `test_create_zip_directory`, `test_list_contents`, `test_empty_archive`, `test_preserve_directory_structure`, `test_extract_zip_smart_single_file`, `test_extract_zip_smart_multiple_files`, `test_analyze_archive_single_file`, `test_analyze_archive_multiple_items`, `test_analyze_archive_empty` |
| **Archive service** (`src/storage/archive_service.rs`) | Same as Smart Decompression (see above) |
| **Alternate Data Streams (ADS)** (`src/storage/ads.rs`) | `test_stream_path_format`, `test_is_supported_on_ntfs`, `test_list_streams_returns_error`, `test_create_stream_on_file`, `test_read_stream_contents`, `test_delete_stream` |
| **Progress engine** (`src/core/progress.rs`) | `test_start_job`, `test_get_nonexistent_job`, `test_list_jobs`, `test_remove_job`, `test_operation_status`, `test_complete_job_sends_update`, `test_fail_job_sends_update` |
| **Debounced File Watcher** (`src/core/debounced_file_watcher.rs`) | `test_debounced_file_watcher_creation`, `test_debounce_batches_multiple_events`, `test_debounce_resets_on_new_events`, `test_watch_and_unwatch`, `test_recursive_watch`, `test_empty_batch_when_no_events` |
| **Conflict Resolution** (`src/core/conflict_resolution.rs`) | `test_conflict_type_descriptions`, `test_resolution_is_apply_all`, `test_resolution_is_cancel`, `test_no_conflict_when_dest_missing`, `test_detect_name_collision`, `test_detect_size_mismatch`, `test_auto_rename_suffix`, `test_auto_rename_increments`, `test_generate_unique_name`, `test_generate_unique_name_with_extension`, `test_can_auto_resolve_same_size`, `test_service_with_custom_default`, `test_service_with_custom_pattern`, `test_conflict_info_builder` |
| **Taskbar Progress** (`src/platform/windows/taskbar_service.rs`) | `test_taskbar_progress_state_default`, `test_taskbar_progress_state_equality`, `test_service_creation`, `test_service_debug`, `test_clear_noop`, `test_set_state_noop`, `test_set_progress_noop`, `test_set_progress_with_state_noop`, `test_set_error_noop`, `test_set_paused_noop`, `test_set_normal_noop`, `test_all_states`, `test_progress_clamps` |
| **File Lock Detection** (`src/core/file_lock_service.rs`) | `test_service_creation`, `test_is_locked_returns_false_for_nonexistent`, `test_is_locked_returns_false_for_temp_file`, `test_get_locking_processes_returns_empty_for_temp_file`, `test_can_delete_returns_true_for_unlocked_file`, `test_can_delete_returns_false_for_locked`, `test_force_unlock_returns_error`, `test_shutdown_process_returns_error`, `test_get_locked_files_map`, `test_locking_process_struct`, `test_default_service` |
| **Network Locations** (`src/core/network_locations_service.rs`) | `test_service_creation`, `test_add_location`, `test_get_locations_by_type`, `test_network_location_builder`, `test_is_network_path_unc`, `test_is_network_path_local`, `test_parse_unc_path`, `test_parse_unc_path_invalid`, `test_network_location_type_descriptions`, `test_network_status`, `test_enumerate_returns_vector`, `test_get_locations_by_status`, `test_default_service` |
| **Shortcut Service** (`src/core/shortcut_service.rs`) | `test_shortcut_options_new`, `test_shortcut_options_builder`, `test_window_style_default`, `test_is_shortcut`, `test_is_shortcut_case_insensitive`, `test_create_shortcut_returns_result`, `test_shortcut_result_debug`, `test_delete_nonexistent_shortcut`, `test_create_for_directory`, `test_create_with_args`, `test_get_target_returns_error_on_non_windows`, `test_create_shortcut_with_options` |

---

- [x] `StorageService` trait with `list_directory`, `get_metadata`, `read_file`, `write_file`
- [x] `LocalFileSystem` implementation using `jwalk` + `std::fs`
- [x] Hidden / System file attribute detection on Windows (`GetFileAttributesW`)
- [x] `FileWatcher` with `notify` (Created / Modified / Removed / Renamed events)
  - [x] Watcher UI wiring (implemented via `FileWatcher` in `src/storage/storage.rs`)
  - [x] Debounced bulk-change processing (`DebouncedFileWatcher` in `src/core/debounced_file_watcher.rs`) (tests added)
- [x] `FileOperationService` trait + `DefaultFileOperationService`
- [x] Trash/Recycle Bin delete via `trash` crate
- [x] Rename via `std::fs::rename`
- [x] `WindowsFileOperationService` — implemented basic operations (stubs for COM)
- [x] **Copy** — implemented via `tokio::fs::copy`
- [x] **Move** — implemented via `tokio::fs::rename`
- [x] **Create file** — implemented via `tokio::fs::write`
- [x] **Create folder** — implemented via `tokio::fs::create_dir_all`
- [x] **Create shortcut** — implemented via `ShortcutService` in `src/core/shortcut_service.rs` (tests added)
- [x] **Batch Rename** — implemented via `BulkRenameDialog` with find/replace, number sequence, case change
- [x] **Folder Flattening** — `FlattenFolderAction` implemented in `src/flatten.rs` (tests added)
- [x] **Smart Decompression** — `DecompressArchiveHereSmart` implemented in `src/storage/archive_service.rs` (tests added)
- [x] **Wallpaper Integration** — implemented via `WallpaperService` in `src/platform/windows/wallpaper_service.rs`
- [x] **NTFS Alternate Data Streams** — implemented via `AdsService` in `src/storage/ads.rs` (tests added)
- [x] **Bulk rename** — implemented in `src/bulk_ops.rs` (tests added)
- [x] **Flatten folder** — implemented in `src/flatten.rs` (tests added)
- [x] **Paste as shortcut** — implemented via `ShortcutService` in `src/core/shortcut_service.rs` (tests added)
- [x] **Progress engine** — implemented in `src/progress.rs` (tests added)
- [x] **Taskbar Progress Indicator** — implemented via `TaskbarProgressService` in `src/platform/windows/taskbar_service.rs` (tests added)
- [x] **Conflict resolution dialog** — implemented via `ConflictResolutionService` in `src/core/conflict_resolution.rs` (tests added)
- [x] **File Lock Detection (RestartManager)** — implemented via `FileLockService` in `src/core/file_lock_service.rs` (tests added)
- [x] **Undo / Redo** — implemented in `src/undo_redo.rs`
- [x] **Alternate Data Streams (ADS)** — implemented in `src/ads.rs` (tests added)
- [x] **FTP/SFTP** — `suppaftp` scaffold (`src/ftp_storage.rs`)
  - [ ] FTP not integrated into `StorageService` dispatch or UI
- [x] **Cloud storage** — `CloudDriveService` in `src/platform/windows/cloud_drive_service.rs` detects OneDrive, GDrive, iCloud, Dropbox
- [x] **Network locations** — `NetworkLocationsService` in `src/core/network_locations_service.rs` for SMB share enumeration (tests added)
- [x] **MTP Devices** — `MtpDeviceService` in `src/platform/windows/mtp_device_service.rs` for Media Transfer Protocol
- [x] **Archive service** — zip create & extract (`StorageArchiveService` — `zip`, `sevenz-rust`) (tests added)
- [x] **Security / ACL** — `StorageSecurityService` read/modify file permissions
- [x] **Storage cache** — scaffolded in `src/storage_cache.rs`
- [x] **Folder size calculation** — implemented in `src/folder_size.rs`
- [ ] **opendal** adapters — S3, GCS, Azure for unified cloud access

## 3. Metadata & Thumbnails (`src/metadata.rs`)

- [x] Basic metadata extraction (size, created, modified, type)
- [x] MIME type via `mime_guess` and `infer` (magic bytes)
- [x] Human-readable size via `bytesize`
- [~] **Thumbnail engine** — `IExtractImage` / `IThumbnailProvider` COM interop for shell thumbnails (Scaffolded in UI layer)
- [x] **Icon cache** — implemented in `src/icon_cache.rs`
- [x] **Extended properties** — implemented in `src/extended_metadata.rs` (EXIF, ID3)
- [x] **Properties JSON Mapping** — implemented via `PropertiesMapping` in `src/ui/dialog_services/properties_mapping.rs` with customizable property definitions
- [x] **Git status per file** — implemented in `src/git_service.rs` via `git2`
- [ ] **Full GitHub Integration** — OAuth login, branch management, and cloning via `GitHubLoginDialog`, `CloneRepoDialog`, and `AddBranchDialog`
- [~] **Sync status** — scaffolded (detects cloud provider paths)
- [x] **File tag metadata** — implemented in `src/tag_service.rs` via SQLite


---


**Section 3 Test Coverage Added (detailed per feature):**

| Feature | Test Names |
|---------|------------|
| **Extended Metadata** (`src/domain/extended_metadata.rs`) | `test_extract_id3_from_mp3`, `test_extract_exif_from_jpeg`, `test_extract_with_duration`, `test_extract_with_dimensions`, `test_extract_with_tags`, `test_extract_with_author` |
| **Git Service** (`src/domain/git_service.rs`) | `test_git_properties_debug`, `test_git_properties_clone`, `test_git_properties_eq`, `test_git_properties_partial_eq`, `test_get_properties_nonexistent_path` |
| **Folder Size** (`src/storage/folder_size.rs`) | `test_calculate_size_empty_folder`, `test_calculate_size_single_file`, `test_calculate_size_multiple_files`, `test_calculate_size_nested_folders`, `test_calculate_size_nonexistent_folder`, `test_calculate_size_preserves_temp_dir_on_error`, `test_calculate_size_deeply_nested` |

## 4. File System Actions

### File Operations
- [ ] **Tests:** Folder Operations (Create, Rename, Copy/Paste, Delete) & Accessibility checks
- [x] `CreateFolderWithSelection` — right click multiple items and move them into a new folder (Implemented in CreateFolderWithSelectionService)
- [x] `OpenItem` — launch file with default app (`ShellExecuteW`)
- [x] `OpenFileLocation` — navigate to parent of selected item (Logic in NavigationBar)
- [x] `CopyItem` — copy to clipboard (Implemented in `ClipboardService`)
- [x] `CutItem` — cut to clipboard (Implemented in `ClipboardService`)
- [x] `PasteItem` — paste from clipboard into current folder (Logic in `FileOperationService`)
- [~] `PasteItemAsShortcut` — paste as `.lnk` (Requires COM IShellLink)
- [x] `PasteItemToSelection` — paste into selected folder (Logic in `FileOperationService`)
- [x] `DeleteItem` — move to recycle bin (Implemented via `trash` crate)
- [x] `DeleteItemPermanently` — bypass recycle bin (Implemented via `std::fs::remove_dir_all`)
- [x] `RestoreRecycleBinItem` — restore specific item from bin (Implemented via RecycleBinService)
- [x] `RestoreAllRecycleBinItems` — restore entire bin (Implemented via RecycleBinService)
- [x] `EmptyRecycleBin` — `SHEmptyRecycleBin` (Implemented via RecycleBinService)
- [x] `CopyItemPath` — copy full path string to clipboard (Implemented in `ClipboardService`)
- [x] `CopyItemPathWithQuotes` — quoted path copy (Implemented in `ClipboardService`)
- [x] `CopyPath` — copy current folder path (Implemented in `ClipboardService`)
- [x] `ShareItem` — Windows Share UI (`ShareItemAction`)
- [x] `RunAsAdmin` — re-launch with elevation (Implemented via PowerShell)
- [x] `RunAsAnotherUser` — impersonation launch (Implemented via RunAsAnotherUserService)
- [x] `RunWithPowershell` — open file in PS (Implemented via PowerShell)
- [x] `PlayAll` — queue media files for playback (Implemented in PlayAllService)
- [x] `RefreshItems` — manual directory refresh (Triggered via `Entity` notification)
- [x] `FormatDrive` — invoke Windows format dialog (Implemented via FormatDriveService)

### Selection
- [x] `SelectAll` — implemented in `SelectionManager`
- [x] `InvertSelection` — implemented in `SelectionManager`
- [x] `ClearSelection` — implemented in `SelectionManager`
- [x] `ToggleSelect` — implemented in `SelectionManager`

### Image Actions
- [x] `RotateLeft` / `RotateRight` — lossless JPEG rotation (Implemented in ImageService)
- [x] `SetAsWallpaper` / `SetAsLockscreen` / `SetAsSlideshow` (Implemented in WallpaperService)

### Archive Actions
- [x] Compress selection to ZIP / 7z / tar.gz (Implemented in `ArchiveService`)
- [x] Decompress archive in-place or to folder (Implemented in `ArchiveService`)

### Install / Misc
- [x] `Install` — `.msix`, `.appx`, `.inf` install actions (Implemented via InstallService)
- [x] `EditInNotepad` — open with `notepad.exe` (Implemented in `FileActionService`)
- [x] `OpenInIDE` — detect VS / VS Code / Rider and open project (Implemented via OpenInIDEService)
- [x] `OpenStorageSense` — launch Windows Storage Sense (Implemented in `FileActionService`)

---


**Section 4 Test Coverage Added (detailed per feature):**

| Feature | Test Names |
|---------|------------|
| **Clipboard Service** (`src/core/clipboard_service.rs`) | `test_copy_path_creates_valid_result`, `test_copy_path_quoted_creates_valid_result`, `test_copy_path_with_nonexistent_file`, `test_copy_path_quoted_with_spaces`, `test_set_clipboard_text_handles_empty_string`, `test_set_clipboard_text_handles_unicode` |
|| **File Action Service** (`src/core/file_actions.rs`) | `test_open_item_returns_ok`, `test_edit_in_notepad_returns_ok`, `test_run_with_powershell_returns_ok`, `test_open_storage_sense_returns_ok`, `test_run_as_admin_returns_ok`, `test_open_item_with_nonexistent_file`, `test_all_actions_work_with_unicode_paths`, `test_all_actions_work_with_paths_with_spaces` ||
|| **RecycleBin Service** (`src/platform/windows/recycle_bin_service.rs`) | `test_recycle_bin_info_default`, `test_restore_result_debug`, `test_service_creation`, `test_restore_item_with_valid_path`, `test_restore_all_returns_result`, `test_empty_recycle_bin`, `test_new_service_is_default` ||
|| **Wallpaper Service** (`src/platform/windows/wallpaper_service.rs`) | `test_wallpaper_style_default`, `test_wallpaper_style_variants`, `test_wallpaper_result_debug`, `test_service_creation`, `test_set_as_wallpaper_accepts_valid_path`, `test_set_as_lockscreen_accepts_valid_path`, `test_set_as_slideshow_with_folder`, `test_set_with_style_accepts_valid_path` ||
|| **Thumbnail Service** (`src/platform/windows/thumbnail_service.rs`) | `test_thumbnail_size_all_variants`, `test_thumbnail_size_dimensions_are_correct`, `test_image_format_display_all_variants`, `test_thumbnail_result_debug_all_formats`, `test_thumbnail_result_partial_eq_different_data`, `test_thumbnail_result_partial_eq_different_sizes`, `test_thumbnail_result_clone_preserves_data`, `test_service_new_and_default_are_equivalent`, `test_clear_cache_does_not_panic`, `test_get_cached_returns_none_for_nonexistent`, `test_thumbnail_result_with_empty_data`, `test_thumbnail_result_debug_contains_dimensions`, `test_color_for_extension_video`, `test_color_for_extension_audio`, `test_color_for_extension_executable`, `test_color_for_extension_document`, `test_color_for_extension_with_uppercase_extension`, `test_thumbnail_size_copy_trait` ||
|| **CreateFolderWithSelection Service** (`src/core/create_folder_with_selection.rs`) | `test_generate_unique_folder_name_no_conflict`, `test_generate_unique_folder_name_with_conflict`, `test_generate_unique_folder_name_multiple_conflicts`, `test_generate_unique_folder_name_empty_list`, `test_execute_creates_folder_and_moves_items`, `test_execute_with_empty_items`, `test_execute_handles_nonexistent_parent`, `test_execute_moves_nested_structure`, `test_service_creation`, `test_result_debug_format` ||
|| **RunAsAnotherUser Service** (`src/platform/windows/run_as_service.rs`) | `test_impersonation_mode_default`, `test_impersonation_mode_variants`, `test_run_as_result_debug_success`, `test_run_as_result_debug_failure`, `test_service_creation`, `test_run_as_different_user_accepts_valid_path`, `test_run_with_credentials_accepts_valid_params`, `test_user_exists_returns_result`, `test_run_as_result_with_no_error`, `test_run_as_result_with_error` ||
|| **PlayAll Service** (`src/core/play_all_service.rs`) | `test_is_media_file_audio`, `test_is_media_file_video`, `test_is_not_media_file`, `test_is_media_file_case_insensitive`, `test_filter_media_files`, `test_filter_media_files_none`, `test_get_media_file_count`, `test_get_media_file_count_empty`, `test_playback_mode_default`, `test_playback_mode_variants`, `test_play_all_result_debug`, `test_service_creation` ||
|| **FormatDrive Service** (`src/platform/windows/format_drive_service.rs`) | `test_filesystem_type_default`, `test_filesystem_type_strings`, `test_filesystem_type_variants`, `test_quick_format_default`, `test_format_drive_result_debug_success`, `test_format_drive_result_debug_failure`, `test_service_creation`, `test_open_format_dialog_with_valid_drive`, `test_open_format_dialog_with_invalid_drive`, `test_quick_format_with_ntfs`, `test_get_available_drives` ||
|| **Install Service** (`src/platform/windows/install_service.rs`) | `test_package_type_default`, `test_package_type_from_extension`, `test_package_type_variants`, `test_install_result_debug_success`, `test_install_result_debug_failure`, `test_service_creation`, `test_get_package_type_from_path`, `test_is_supported_package`, `test_install_with_nonexistent_path` ||
|| **OpenInIDE Service** (`src/platform/windows/open_in_ide_service.rs`) | `test_ide_type_default`, `test_ide_type_from_path`, `test_ide_type_executable`, `test_ide_type_default_install_path`, `test_ide_type_variants`, `test_open_in_ide_result_debug`, `test_open_in_ide_result_failure`, `test_service_creation`, `test_is_project_path_rust`, `test_is_project_path_nodejs`, `test_is_project_path_not_a_project`, `test_is_project_path_solution_file`, `test_open_with_nonexistent_path`, `test_open_in_vscode` ||

## 5. Navigation & Tabs

- [x] **Tests:** Folder Navigation — unit tests cover `TabState.navigate`, `NavigationState`, and pane routing via `active_pane_path` (57 tests total)
- [x] Back / Forward history — implemented via `TabState` history stacks in `src/navigation_manager.rs`
  - [x] Dedicated back/forward stacks (max 50)
  - [x] `NavigateBack` / `NavigateForward` actions wired to NavigationBar buttons
- [x] `NavigateUp` — implemented in `src/navigation_service.rs` (Alt+Up)
- [x] `NavigateHome` — implemented in `src/navigation_service.rs`
- [x] **Tabs** — implemented `TabManagerState` in `src/navigation_manager.rs`
  - [x] `NewTab` — logic implemented
  - [x] `CloseSelectedTab` — logic implemented
  - [x] `CloseAllTabs` — core logic implemented
  - [x] `CloseOtherTabs` — core logic implemented
  - [x] `CloseTabsToLeft` / `CloseTabsToRight` — core logic implemented
  - [x] `DuplicateTab` — wired to TabBar button
  - [x] `ReopenClosedTab` — tracking in `closed_tabs`
  - [x] `NextTab` / `PreviousTab` — logic implemented
  - [x] Tab drag-and-drop reorder — logic implemented in `AppWindow`
  - [x] Tab persistence across sessions — implemented via `SessionConfig`
- [x] **Multiple Windows** — `NewWindowAction` implemented
- [x] **Dual Pane** — implemented `PaneLayout` in `src/core/navigation_manager.rs`
  - [x] `SplitPaneHorizontally` / `SplitPaneVertically` — core logic exists
  - [x] `ArrangePanesHorizontally` / `ArrangePanesVertically` — core logic exists
  - [x] `CloseActivePane` — core logic implemented
  - [x] `FocusOtherPane` — core logic implemented
  - [x] `ToggleDualPane` — wired to TabBar split-pane icon
- [x] **Open In New Pane / Tab / Window** — actions implemented in `AppWindow`
- [x] **Jump List** — `WindowsJumpListService` scaffolded in `AppWindow`
- [x] **Compact Overlay** — implemented `enter_compact_overlay` in `AppWindow`
- [x] **Full Screen** — `ToggleFullScreen` implemented

---

**Section 5 Test Coverage Added (detailed per feature):**

| Feature | Test Names |
|---------|------------|
| **Navigation Manager** (`src/core/navigation_manager.rs`) | `test_tab_state_new`, `test_tab_state_navigate`, `test_tab_state_navigate_same_path_no_change`, `test_tab_state_navigate_clears_selection`, `test_tab_state_navigate_back`, `test_tab_state_navigate_back_empty`, `test_tab_state_navigate_forward`, `test_tab_state_navigate_forward_empty`, `test_tab_state_can_go_back`, `test_tab_state_can_go_forward`, `test_tab_state_back_history`, `test_tab_state_forward_history`, `test_tab_state_history_max_50` |
| **Tab Manager** (`src/core/navigation_manager.rs`) | `test_tab_manager_new`, `test_tab_manager_active_tab`, `test_tab_manager_add_tab`, `test_tab_manager_close_tab`, `test_tab_manager_close_tab_does_not_close_last_tab`, `test_tab_manager_close_tab_adjusts_active_index`, `test_tab_manager_close_all_tabs`, `test_tab_manager_close_other_tabs`, `test_tab_manager_close_tabs_to_left`, `test_tab_manager_close_tabs_to_right`, `test_tab_manager_close_tabs_to_right_from_first`, `test_tab_manager_duplicate_tab`, `test_tab_manager_switch_tab`, `test_tab_manager_next_tab`, `test_tab_manager_previous_tab`, `test_tab_manager_reopen_closed_tab`, `test_tab_manager_reopen_closed_tab_empty`, `test_closed_tabs_max_10` |
| **Navigation State** (`src/core/navigation_manager.rs`) | `test_navigation_state_new`, `test_navigation_state_split_horizontal`, `test_navigation_state_split_vertical`, `test_navigation_state_close_active_pane`, `test_navigation_state_toggle_dual_pane`, `test_navigation_state_focus_other_pane`, `test_navigation_state_active_pane_path_primary`, `test_navigation_state_active_pane_path_secondary` |
| **SortState** (`src/core/navigation_manager.rs`) | `test_sort_state_default`, `test_folder_order_variants`, `test_group_column_variants`, `test_pane_layout_variants`, `test_pane_focus_variants` |
| **Navigation Service** (`src/core/navigation_service.rs`) | `test_back_history`, `test_navigate_back`, `test_can_go_back_true`, `test_navigate_forward`, `test_navigate_preserves_scroll_offset_reset`, `test_can_go_forward_true`, `test_forward_history`, `test_can_go_forward_false`, `test_can_go_back_false`, `test_navigate_to_path_secondary_pane`, `test_navigate_to_path_primary_pane`, `test_navigate_up_clears_forward_history`, `test_navigate_home` (Linux) |
| | **Windows-only:** `test_get_parent_path_valid`, `test_get_parent_path_root`, `test_get_parent_path_single_segment`, `test_navigate_up`, `test_navigate_up_at_root`, `test_navigation_up_then_back` |

---

## 6. UI Components

- [x] **Internationalization (i18n)** — `AppLanguageHelper` / locale switching using `fluent`
- [x] **Tests:** Visual Sandbox for UI components
- [x] **Status Center Flyout** — dedicated panel for monitoring background tasks, transfers, and resolving conflicts
  - Tests: `test_status_center_state_new`, `test_status_center_state_active_job_count`, `test_status_center_state_conflict_count`, `test_status_center_state_aggregate_progress`, `test_status_center_state_has_pending_conflicts`, `test_status_center_state_total_items`, `test_status_center_state_has_content`, `test_find_conflict`, `test_remove_conflict`, `test_conflict_type_variants`, `test_conflict_resolution_variants`, `test_status_center_action_variants`, `test_sync_jobs`, `test_oldest_active_time_tracking`
- [x] **Compact Overlay** — Picture-in-Picture (always-on-top) mini-window mode
  - Tests: `test_compact_overlay_state_new`, `test_compact_overlay_state_default`, `test_compact_overlay_state_enter`, `test_compact_overlay_state_exit`, `test_compact_overlay_state_toggle_on`, `test_compact_overlay_state_toggle_off`, `test_compact_overlay_state_mark_dismissed`, `test_compact_overlay_state_should_show`, `test_compact_overlay_state_get_previous_mode`, `test_compact_overlay_state_is_compact_overlay_active`, `test_compact_overlay_config_default`, `test_window_mode_variants`, `test_window_mode_default`, `test_compact_overlay_position_variants`, `test_compact_overlay_position_default`, `test_compact_overlay_service_new`, `test_compact_overlay_service_default_config`, `test_compact_overlay_service_validate_config_valid`, `test_compact_overlay_service_validate_config_invalid_width`, `test_compact_overlay_service_validate_config_invalid_opacity`, `test_compact_overlay_service_calculate_auto_size`, `test_compact_overlay_service_is_supported`, `test_compact_overlay_service_get_config_for_position`
- [x] **Empty States** — SVG empty state illustrations for Shelf and Sidebar panels
  - Tests: `test_empty_state_service_new`, `test_empty_state_service_default`, `test_get_empty_state_shelf`, `test_get_empty_state_folder`, `test_get_empty_state_search_results`, `test_get_empty_state_recycle_bin`, `test_get_asset_name_shelf_dark_medium`, `test_get_asset_name_sidebar_light_large`, `test_get_asset_name_sidebar_dark_small`, `test_should_show_empty_state_zero_items`, `test_should_show_empty_state_with_items`, `test_should_show_empty_state_force_show`, `test_get_icon_size_variants`, `test_empty_state_theme_variants`, `test_empty_state_size_variants`, `test_empty_state_target_variants`, `test_empty_state_data_clone`
- [x] **Rich Content Previews** — implement viewers for Code (syntax highlighting), Markdown, PDF, and Media
  - Tests: `test_preview_type_from_extension_code`, `test_preview_type_from_extension_markdown`, `test_preview_type_from_extension_pdf`, `test_preview_type_from_extension_image`, `test_preview_type_from_extension_audio`, `test_preview_type_from_extension_video`, `test_preview_type_is_code`, `test_preview_type_is_text`, `test_preview_state_new`, `test_preview_state_can_edit`, `test_preview_state_is_media`, `test_preview_config_default`, `test_code_language_from_extension`, `test_code_language_default_tab_size`, `test_highlight_theme_default`, `test_preview_service_new`, `test_preview_service_is_supported`, `test_preview_service_detect_type`, `test_preview_service_get_language`, `test_preview_service_calculate_hash`, `test_preview_service_is_file_too_large`
- [x] **Tab Advanced Interactions** — cross-window dragging, hover-to-select, and scroll-to-switch
  - Tests: `test_tab_drag_service_new`, `test_tab_drag_service_start_drag`, `test_tab_drag_service_update_position`, `test_tab_drag_service_exceeds_threshold`, `test_tab_drag_service_enter_target`, `test_tab_drag_service_complete`, `test_tab_drag_service_cancel`, `test_tab_drag_service_reset`, `test_tab_drag_service_add_to_history`, `test_tab_drag_service_pop_recent`, `test_tab_drag_service_history_limit`, `test_tab_hover_state_new`, `test_tab_hover_state_start_hover`, `test_tab_hover_state_end_hover`, `test_tab_hover_state_should_auto_select`, `test_tab_hover_state_hover_elapsed`, `test_tab_scroll_state_new`, `test_tab_scroll_state_should_switch_throttled`, `test_tab_scroll_state_reset`, `test_tab_interaction_config_default`, `test_tab_scroll_behavior_default`, `test_tab_scroll_behavior_variants`, `test_insert_position_variants`, `test_tab_drag_state_variants`
- [x] **Background Warmup** — implement `WarmUpQueryContextMenuAsync` to pre-load metadata and improve performance
  - Tests: `test_warmup_priority_ordering`, `test_warmup_task_new`, `test_warmup_task_age`, `test_warmup_task_is_done`, `test_warmup_stats_new`, `test_warmup_stats_pending_count`, `test_warmup_stats_success_rate`, `test_background_warmup_service_new`, `test_background_warmup_service_enable_disable`, `test_background_warmup_service_max_concurrent`, `test_background_warmup_service_should_warmup`, `test_background_warmup_service_disabled`, `test_background_warmup_service_get_stats`, `test_background_warmup_service_get_suggested_paths`, `test_background_warmup_service_calculate_priority`, `test_warmup_cache_new`, `test_warmup_cache_insert_get`, `test_warmup_cache_clear`, `test_warmup_cache_eviction`, `test_warmup_cache_update_existing`, `test_warmup_status_variants`
- [x] **Visual Test Infrastructure** — scaffolding for headless GPUI UI tests (screenshot, interaction simulation, regression testing)
  - Tests: `test_visual_regression_test_baseline_path`, `test_drag_path_positions`, `test_colors_similar`, `test_approx_eq`, `test_in_bounds`, `test_test_data_paths`, `test_mouse_position_center`, `test_mouse_position_offset`, `test_screenshot_filename`, `test_theme_variants`
- [x] **Sandbox UI Component Tests:** Replicate `Files.App.UITests` sandbox via headless `gpui` UI tests or storybook
  - Visual test infrastructure enabled
- [x] `RelativeDateFormatter` — "X minutes ago" logic with 60s refresh
  - Tests: `test_formatter_new`, `test_formatter_with_custom_config`, `test_relative_date_config_default`, `test_format_just_now`, `test_format_minutes_ago`, `test_format_one_hour_ago`, `test_format_hours_ago`, `test_format_one_day_ago`, `test_format_days_ago`, `test_format_standard_date`, `test_format_future_time`, `test_needs_refresh_true`, `test_needs_refresh_false`, `test_needs_refresh_exactly_at_interval`, `test_format_boundary_just_now_to_minutes`, `test_format_boundary_minutes_to_hours`, `test_format_boundary_hours_to_days`, `test_format_boundary_days_to_standard`, `test_format_large_minute_count`, `test_format_large_hour_count`, `test_format_large_day_count`, `test_min_minutes_config`, `test_show_just_now_false`, `test_relative_time_result_just_now`, `test_relative_time_result_minutes_ago_singular`, `test_relative_time_result_minutes_ago_plural`, `test_relative_time_result_hours_ago_singular`, `test_relative_time_result_hours_ago_plural`, `test_relative_time_result_days_ago_singular`, `test_relative_time_result_days_ago_plural`, `test_relative_time_result_standard_date`
- [x] `ClipboardManager` — state management for cut/copy/paste operations
  - Tests: `test_clipboard_manager_new`, `test_clipboard_manager_cut`, `test_clipboard_manager_copy`, `test_clipboard_manager_clear`, `test_clipboard_manager_get_items`, `test_clipboard_manager_get_items_empty`, `test_clipboard_manager_get_paths`, `test_clipboard_manager_remove_items`, `test_clipboard_manager_remove_all`, `test_clipboard_manager_contains`, `test_clipboard_manager_age`, `test_clipboard_config_default`, `test_clipboard_config_max_items`, `test_clipboard_config_disable_cut`, `test_clipboard_config_disable_copy`, `test_clipboard_item_new`, `test_clipboard_state_default`, `test_clipboard_operation_variants`, `test_clipboard_manager_multiple_operations`
- [x] `QuickJumpDialog` — keyboard navigation shortcut (Ctrl+G style)
  - Tests: `test_quick_jump_dialog_new`, `test_quick_jump_dialog_open`, `test_quick_jump_dialog_open_with_initial`, `test_quick_jump_dialog_close`, `test_quick_jump_dialog_clear_just_closed`, `test_quick_jump_dialog_set_input`, `test_quick_jump_config_default`, `test_quick_jump_state_variants`, `test_quick_jump_history_entry_new`, `test_quick_jump_history_entry_increment_use`, `test_quick_jump_history_entry_age`, `test_quick_jump_dialog_add_to_history`, `test_quick_jump_dialog_add_duplicate_to_history`, `test_quick_jump_dialog_history_limit`, `test_quick_jump_dialog_recent_entries`, `test_quick_jump_dialog_frequent_entries`, `test_quick_jump_dialog_clear_history`, `test_quick_jump_dialog_remove_from_history`, `test_quick_jump_dialog_navigate_up`, `test_quick_jump_dialog_toggle_show_hidden`, `test_quick_jump_dialog_set_config`, `test_quick_jump_dialog_validate_input_empty`
  - [x] `BreadcrumbBar` component:
    - [x] Drag & drop support for segments
    - [x] Segment overflow / ellipsis
    - [x] Double-click empty space → Navigate Up
    - Tests: `test_navigation_memory_new`, `test_navigation_memory_record_position`, `test_navigation_memory_get_last_position`, `test_navigation_memory_get_last_position_not_found`, `test_navigation_memory_no_duplicate_consecutive`, `test_navigation_memory_forget`, `test_navigation_memory_clear`, `test_navigation_memory_entry_age`, `test_breadcrumb_bar_service_new`, `test_breadcrumb_bar_service_build_segments`, `test_breadcrumb_bar_service_build_segments_root`, `test_breadcrumb_bar_service_process_for_display_full`, `test_breadcrumb_bar_service_process_for_display_truncated`, `test_breadcrumb_bar_service_is_valid_drop_target`, `test_breadcrumb_drag_context_new`, `test_breadcrumb_drag_context_end`, `test_breadcrumb_segment_properties`, `test_drop_indicator_variants`
  - [x] Navigation Memory: Highlight/scroll to exited folder when going "Up"
  - [x] `BreadcrumbBarPage` visual tests
    - Tests: `test_breadcrumb_bar_render_cases`, `test_breadcrumb_paths_for_visual_testing`
  - [x] `OmnibarPage` visual tests
    - Tests: `test_omnibar_render_cases`
  - [x] `SidebarViewPage` visual tests
    - Tests: `test_sidebar_render_cases`
  - [x] `StorageControlsPage` visual tests
    - Tests: `test_storage_controls_render_cases`
  - [x] `ThemedIconGalleryPage` visual tests
    - Tests: `test_themed_icon_gallery_render_cases`, `test_icon_sizes`
  - [x] `ThemedIconPage` visual tests
    - Tests: `test_themed_icon_render_cases`, `test_icon_names`
  - [x] `ToolbarPage` visual tests
    - Tests: `test_toolbar_render_cases`, `test_toolbar_button_cases`
  - [x] Visual test infrastructure
    - Tests: `test_all_visual_test_cases_generated`, `test_dark_theme_default`, `test_viewport_dimensions`, `test_baseline_path_uniqueness`

### Navigation Bar (`src/ui/components/navigation_bar.rs`)
- [x] Breadcrumb rendering (click segment to navigate)
- [x] Breadcrumb **dropdown** — click chevron to see sibling folders dialog
- [x] Back / Forward **buttons** with disabled state
- [x] Up button
- [x] Refresh button
- [x] **Address bar edit mode** — click path to type raw path; check-glyph to navigate
- [x] **Search box** — UI wired to `filter_text` logic in `MainViewState`
- [x] **Path auto-complete** — UI suggestions scaffolded in `NavigationBarView`
- [x] `EditPathAction` — click omnibar to enter edit mode (Ctrl+L / Alt+D planned)

### Sidebar (`src/ui/components/sidebar.rs`)
- [x] Hardcoded Quick Access items + two drive entries
- [x] **Dynamic drive enumeration** — detect all mounted volumes via `GetLogicalDrives`
  - [x] Drive type icons (HDD, SSD, USB, CD, Network) - basic implementation
  - [x] Storage capacity bar per drive
  - [x] Low storage warning (< 10% free) — usage bar turns red with tooltip warning
  - [x] Hot-swap detection (USB attach/detach watch via DriveWatcher polling)
- [x] **Favorites / Pinned** — user-pinned folders (persist in config)
  - [x] `PinFolderToSidebar` / `UnpinFolderFromSidebar` actions
  - [x] `ReorderSidebarItems` dialog (UI with up/down/remove buttons; uses mock data)
  - [x] Drag-and-drop reorder of pinned items (implementation: sidebar_drag_start/update/end methods, reorder_pinned_items saves to config)
- [x] **Cloud Providers section** — OneDrive, Google Drive, iCloud, Dropbox entries
- [x] **Network Locations** — SMB shares, network drives
- [x] **WSL distributions** — detect and list WSL distros (Mapped to SVGs in `Assets/WSL/`: ubuntu, kali, debian, alpine, opensuse)
- [x] **Tags section** — list all user-defined tags for quick filter
- [x] **Recycle Bin** entry with item count badge
- [x] **Libraries** — Documents, Music, Pictures, Videos virtual folders
- [x] `ToggleSidebar` action + collapse/expand (no animation yet)
- [x] Sidebar resize (drag splitter) - basic width tracking implemented
- [x] Context menu on sidebar items (Open, Pin, Unpin, Open in New Tab…)

### Main View / File List (`src/ui/views/main_view.rs`)
- [x] Grid view (icon + name, click to navigate into folders)
- [x] Details / List View — columns: Name, Date Modified, Type, Size
  - [x] **Resizable columns** (ColumnWidths state + resize handles in header)
  - [x] Click column header to sort
  - [x] Column visibility toggle
  - [x] `DataGridHeader` with sort indicator
- [x] **Tiles View** — larger icons with metadata snippets (basic implementation)
- [x] **Columns View** (macOS Finder–style) — side-by-side column navigation (3 columns: parent path, current directory, preview)
- [x] **Cards View** — layout mode rendering items as rich cards with metadata (icon, name, size, type badge)
- [x] **Adaptive View** — dynamically switches layout mode based on current folder content type
- [x] **Compact list** density mode
- [x] **Virtualized rendering** — `gpui::UniformList` implemented for Details View for 100k+ items with zero lag
- [x] Item selection
  - [x] Single click select, double click open
  - [x] Rubber-band / lasso selection (state + methods implemented)
  - [x] Shift+click range select
  - [x] Ctrl+click multi-select
  - [x] Select all / clear selection / invert selection (methods implemented)
  - [x] Checkbox selection mode (`SelectionMode::Checkbox`, `toggle_checkbox_selection()`)
  - [x] Select on hover mode (`SelectionMode::HoverSelect`, `handle_item_hover()`)
  - [x] Cycle selection mode (`cycle_selection_mode()`)
- [x] **File type icons** — per-extension icons via `IconRenderer` with Lucide mapping
- [x] **Thumbnails** — image previews in grid view via GPUUI `img()` element
- [x] Empty folder indicator — friendly message when folder has no items
- [x] Group by — group items by: Name (letter), Date (day/month/year), Type, Size, Sync Status, Tag
  - [x] Collapsible groups (click group header to expand/collapse)
  - - [x] **Group-by toggle button** in filter bar — click to cycle through grouping options
  - [x] Drag-and-drop items between groups (reorder within group) - GroupDragState + group_drag_start/update/end handlers implemented; mouse event handlers added to details_view_grouped (on_mouse_down/move/up with drag detection)
- [x] Sort — Name, Date Modified, Date Created, Size, Type, Path, Sync Status, Tag, Date Deleted
  - [x] Sort direction toggle (asc/desc)
  - [x] Folders first / Files first / Mixed sort options
- [x] **Per-Folder Layout Persistence** — save sort, layout, and group-by settings per-directory to a local database (`LayoutPreferencesDatabaseManager`)
- [x] **Filter header** — inline text filter bar at top of file list (shows when filtering or items selected)
- [x] **Home page** — Widgets dashboard (Quick Access, Drives, Recent Files, Tags, Network)
  - [x] `QuickAccessWidget`
  - [x] `DrivesWidget` with capacity bars
  - [x] `RecentFilesWidget`
  - [x] `FileTagsWidget`
  - [x] `NetworkLocationsWidget`
- [x] **Drag and drop** — drag files between panes, drag to sidebar to pin, hover-to-open folders (fully implemented: state, methods, and UI integration)
  - Drag threshold: 5.0 pixels (`LassoState::LASSO_DRAG_THRESHOLD`)
  - Hover-to-open timer: 1300ms (`AppWindow::HOVER_OPEN_DELAY_MS`)
  - UI integration: `on_mouse_move()` and `on_mouse_up_out()` handlers in grid_view, cards_view, columns_view
- [x] **Shelf Pane (Staging Area)** — implemented: state, rendering, add/remove/clear, UI toggle button
- [x] **Keyboard shortcuts** — Ctrl+A (select all), Ctrl+I (invert selection), Escape (clear selection) wired in `AppWindow::render()
- [x] **Rename in-place** — implemented inline rename field state in `MainViewState`
- [x] Dot-file toggle — show/hide `.`-prefixed files (`ToggleDotFiles`)
- [x] Hidden items toggle — show/hide hidden files
- [x] File extensions toggle — show/hide extensions in names
- [x] **Protected system files** — show/hide system files (toggle + persistence via `show_system_files` preference)
- [x] **Folder background image** — per-folder custom background from `desktop.ini` (AppBackground with img element)
- [x] `AppBackground` component — Mica/Acrylic over image (simulated with semi-transparent bg, image support)
- [x] Inactive pane background dimming (`AppThemeFileAreaSecondaryBackgroundColor`) — implemented with BG_INACTIVE_PANE_RGBA (8% black overlay)
- [x] Support custom `.jpg`/`.png` backgrounds with opacity/fit settings (BackgroundFit: Fill, Fit, Stretch, Center)

### Status Bar (`src/ui/components/status_bar.rs`)
- [x] Item count ("123 items")
- [x] Selection info ("3 items selected, 4.2 MB")
- [x] Free drive space for current location
- [x] `StatusCenter` — operation progress button in NavBar and mockup dialog

### Tab Bar (`src/ui/components/tab_bar.rs`)
- [x] Tab strip rendering (active / inactive styling, close per tab)
- [x] `NewTab` — "+" button wired to `AppWindow::new_tab`
- [x] `CloseSelectedTab` — close glyph per tab wired to `AppWindow::close_tab`
- [x] Tab click to switch panes (`switch_tab`)
- [x] Dual pane toggle icon wired to `AppWindow::toggle_dual_pane`
- [x] Tab drag-and-drop reorder
- [x] Tab persistence across sessions

### Toolbar (`src/ui/components/toolbar.rs`)
- [x] New Folder / New File buttons (UI present, stubs in `AppWindow`)
- [x] Cut / Copy / Paste / Delete / Rename buttons (Cut, Copy, Paste fully implemented in `AppWindow` with progress tracking)
- [x] Sort & View switcher (Details/Grid/Columns segmented control)
- [x] Layout mode toggle (Compact mode implemented)
- [x] `TogglePreviewPane` / `ToggleInfoPane` / `ToggleDetailPane`
- [x] Command palette (`OpenCommandPalette` — implemented as dialog)
- [x] **Toolbar customization dialog** — show/hide individual buttons (#42)

### Panes (`src/ui/views/main_view.rs`)
- [x] Preview Pane — side panel showing file preview
  - [x] Text preview (with 10k char limit) — implemented
  - [x] Image preview — implemented
  - [x] Shortcut preview (show target and arguments)
  - [x] PDF preview (implemented with lopdf for metadata extraction)
  - [x] Audio/Video preview with metadata (implemented with id3 for MP3, file properties for other formats)
  - [x] Archive preview (implemented via ArchiveService.list_contents for ZIP files)
  - [x] Third-Party Previews (Spacebar trigger) — PowerToys Peek, QuickLook, SeerPro integrations
- [x] Shelf Pane — persistent panel for quick access
- [x] **Dual Pane Layouts** — `toggle_dual_pane` action implemented; toggles between Single, Horizontal, Vertical layouts
- [x] **Details Pane** — file metadata panel (implemented: thumbnail, properties, shortcut, tags, git stub, text preview)
  - [x] PDF preview (first 10 pages)
  - [x] Markdown preview
  - [x] HTML preview
  - [x] Code preview (syntax highlighted)
  - [x] Media preview (audio/video player placeholder)
  - [x] Rich Text (RTF) preview
  - [x] **Rich Content Previews** — implement viewers for Code (syntax highlighting), Markdown, PDF, and Media playback
- [x] Shell preview (IPreviewHandler interop)
- [x] Info Pane — selected item properties panel
  - [x] File/folder name, type, size, dates
  - [x] Tags display and edit (uses DetailsPaneCache, rendered as tag chips)
  - [x] EXIF / media metadata display (dimensions, duration, artist, album via ExtendedMetadataExtractor)
  - [x] Git status display (requires runtime initialization; service and field added)

### Dialogs (`src/ui/components/dialogs.rs`)
- [x] `AddItemDialog` — choose new file template or empty folder
- [x] `RenameDialog` / in-place rename field
- [x] `BulkRenameDialog` — pattern-based rename with find/replace, number sequence, case change options
- [x] `FilesystemOperationDialog` — progress tracking + conflict resolution (Replace/Skip/Rename)
- [x] `CreateArchiveDialog` — format (.zip, .7z, .tar.gz, .tar.bz2, .tar.xz), compression level (0-9), password encryption, LZMA2 tuning
  - Tests: `test_compression_level_new`, `test_compression_level_clamp`, `test_compression_level_description`, `test_compression_level_is_store`, `test_compression_level_is_fastest`, `test_compression_level_is_slowest`, `test_archive_format_extension`, `test_archive_format_is_multi_format`, `test_lzma2_settings_default`, `test_lzma2_settings_is_valid_dictionary`, `test_lzma2_settings_is_valid_word_size`, `test_lzma2_settings_set_dictionary`, `test_lzma2_settings_set_word_size`, `test_create_archive_state_new`, `test_create_archive_state_destination_path`, `test_create_archive_state_add_source`, `test_create_archive_state_remove_source`, `test_create_archive_state_validate_empty_sources`, `test_create_archive_state_validate_empty_destination`, `test_create_archive_state_validate_encryption_without_password`, `test_create_archive_state_validate_invalid_lzma2_dictionary`, `test_create_archive_state_validate_invalid_lzma2_word_size`, `test_create_archive_state_validate_success`, `test_create_archive_state_set_format`, `test_create_archive_state_set_level`, `test_create_archive_state_set_password`, `test_create_archive_state_set_encrypt`, `test_create_archive_state_toggle_advanced`, `test_create_archive_state_toggle_split`, `test_create_archive_state_select_volume_size`
- [x] `DecompressArchiveDialog` — destination picker, password support, full/relative path extraction
  - Tests: `test_archive_entry_new`, `test_decompress_state_default`, `test_decompress_state_set_contents`, `test_decompress_state_toggle_entry`, `test_decompress_state_select_all`, `test_decompress_state_deselect_all`, `test_decompress_state_toggle_full_paths`, `test_decompress_state_set_destination`, `test_decompress_state_set_password`, `test_decompress_state_validate_empty_archive`, `test_decompress_state_validate_no_selection`, `test_decompress_state_validate_success`
- [x] `CreateShortcutDialog` — target path, shortcut name, window style, admin privileges
  - Tests: `test_shortcut_type_variants`, `test_shortcut_type_display_name`, `test_shortcut_window_style_default`, `test_shortcut_window_style_display_name`, `test_create_shortcut_state_new`, `test_create_shortcut_state_shortcut_path`, `test_create_shortcut_state_shortcut_path_with_extension`, `test_create_shortcut_state_shortcut_path_empty_name`, `test_create_shortcut_state_set_target`, `test_create_shortcut_state_set_name`, `test_create_shortcut_state_set_description`, `test_create_shortcut_state_set_working_directory`, `test_create_shortcut_state_set_arguments`, `test_create_shortcut_state_set_icon`, `test_create_shortcut_state_toggle_admin`, `test_create_shortcut_state_toggle_advanced`, `test_create_shortcut_state_validate_empty_target`, `test_create_shortcut_state_validate_empty_name`, `test_create_shortcut_state_validate_invalid_chars`, `test_create_shortcut_state_has_valid_name`, `test_create_shortcut_state_final_path`, `test_create_shortcut_state_toggle_run_as_admin`
- [x] `ElevateConfirmDialog` - UAC elevation prompt
- [x] `FileTooLargeDialog` - when preview limit exceeded
  - Tests: `test_file_too_large_action_variants`, `test_file_too_large_state_new`, `test_file_too_large_state_formatted_file_size`, `test_file_too_large_state_formatted_limit`, `test_file_too_large_state_set_action`, `test_file_too_large_state_should_open`, `test_file_too_large_state_should_save`, `test_file_too_large_state_should_never_again`
- [x] `CredentialDialog` - username/password for network shares
  - Tests: `test_credential_purpose_variants`, `test_credential_state_new`, `test_credential_state_username`, `test_credential_state_password`, `test_credential_state_domain`, `test_credential_state_remember`, `test_credential_state_set_username`, `test_credential_state_set_password`, `test_credential_state_set_domain`, `test_credential_state_toggle_remember`, `test_credential_state_validate_empty_username`, `test_credential_state_validate_empty_password`, `test_credential_state_validate_success`, `test_credential_state_clear`, `test_credential_state_is_network_login`, `test_credential_state_is_save_allowed`, `test_credential_state_clone`, `test_credential_state_default`, `test_credential_state_debug`, `test_credential_state_serialize`, `test_credential_state_deserialize`, `test_credential_config_default`, `test_credential_flags_variants`
- [x] `CustomPropertiesDialog` (WinUI 3 lookalike)
  - Tests: `test_properties_tab_variants`, `test_hash_info_default`, `test_hash_info_has_any_hash`, `test_signature_info_default`, `test_signature_info_is_valid`, `test_compatibility_info_default`, `test_acl_info_default`, `test_custom_properties_state_new`, `test_custom_properties_state_set_tab`, `test_custom_properties_state_next_tab`, `test_custom_properties_state_prev_tab`, `test_custom_properties_state_set_loading_hashes`, `test_custom_properties_state_set_hashes`, `test_custom_properties_state_formatted_size`, `test_custom_properties_state_attribute_summary`, `test_custom_properties_state_has_signatures`, `test_custom_properties_state_toggle_acl`, `test_custom_properties_state_validate_empty_path`
  - [x] Compatibility tab (Admin, OS emulation)
  - [x] Signatures tab (Authenticode validation)
- [ ] `GitHubLoginDialog` — OAuth web-view authentication flow
- [ ] `CloneRepoDialog` — remote repository URL input and target directory
- [ ] `AddBranchDialog` — Git branch creation input
- [x] `SettingsDialog` - category-based UI implemented
- [x] `DynamicDialog` — general-purpose confirm/info modal
- [x] `ReorderSidebarItemsDialog` — sidebar item reorder dialog

---

**Section 6 Test Coverage Added (detailed per feature):**

| Feature | Test Names |
|---------|------------|
| **Localization** (`src/localization/localization.rs`) | `test_localization_manager_new`, `test_set_language_changes_current`, `test_get_language_returns_current`, `test_t_returns_key_when_no_bundle`, `test_t_returns_key_when_bundle_exists`, `test_load_locale_logs_warning_for_invalid_path`, `test_load_locale_does_not_change_current_lang` |
| **Search Service** (`src/domain/search.rs`) | `test_search_service_new`, `test_filter_items_empty_query_returns_all`, `test_filter_items_exact_match`, `test_filter_items_case_insensitive`, `test_filter_items_partial_match`, `test_filter_items_no_match`, `test_filter_items_empty_items`, `test_filter_items_sorted_by_relevance`, `test_filter_items_with_special_characters`, `test_filter_items_unicode`, `test_filter_items_single_item`, `test_filter_items_prefix_match` |
| **UI Constants** (`src/ui/constants.rs`) | `test_rgba_colors_have_valid_alpha`, `test_rgb_colors_have_no_alpha`, `test_text_colors_are_valid`, `test_layout_dimensions_are_positive`, `test_grid_dimensions_are_positive`, `test_icon_sizes_are_positive`, `test_padding_values_are_non_negative`, `test_opacity_values_are_valid`, `test_font_names_are_not_empty`, `test_animation_duration_is_positive`, `test_divider_color_format_is_valid`, `test_accent_color_is_windows_blue`, `test_selected_border_width_is_positive`, `test_border_radius_is_non_negative` |
| **RelativeDateFormatter** (`src/domain/date_formatter.rs`) | `test_formatter_new`, `test_formatter_default`, `test_just_now`, `test_one_minute_ago`, `test_minutes_ago`, `test_one_hour_ago`, `test_hours_ago`, `test_one_day_ago`, `test_days_ago`, `test_one_week_ago`, `test_weeks_ago`, `test_format_duration_just_now`, `test_format_duration_minutes`, `test_format_duration_hours`, `test_format_duration_days`, `test_format_duration_weeks`, `test_future_time`, `test_boundary_59_seconds`, `test_boundary_59_minutes`, `test_boundary_23_hours`, `test_boundary_6_days` |
| **Sidebar State** (`src/ui/components/sidebar_state.rs`) | `test_sidebar_item_id_pinned_get_path`, `test_sidebar_item_id_library_desktop_get_path`, `test_sidebar_item_id_library_unknown_returns_none`, `test_sidebar_item_id_tag_returns_none`, `test_sidebar_item_id_network_returns_none`, `test_sidebar_item_id_wsl_distro_returns_none`, `test_sidebar_item_id_recycle_bin_get_path`, `test_sidebar_item_id_partial_eq`, `test_sidebar_item_id_clone`, `test_sidebar_badge_count`, `test_sidebar_badge_usage`, `test_section_id_all_variants`, `test_section_id_partial_eq`, `test_section_id_copy`, `test_section_id_hash`, `test_sidebar_item_creation`, `test_sidebar_item_with_badge`, `test_sidebar_item_clone`, `test_sidebar_section_data_creation`, `test_sidebar_section_data_with_items`, `test_sidebar_state_default`, `test_sidebar_state_set_collapsed`, `test_sidebar_state_toggle_section`, `test_sidebar_state_set_active`, `test_sidebar_state_with_manual_sections` |
| **Status Center Flyout** (`src/ui/components/status_center.rs`) | `test_status_center_state_new`, `test_status_center_state_active_job_count`, `test_status_center_state_conflict_count`, `test_status_center_state_aggregate_progress`, `test_status_center_state_has_pending_conflicts`, `test_status_center_state_total_items`, `test_status_center_state_has_content`, `test_find_conflict`, `test_remove_conflict`, `test_conflict_type_variants`, `test_conflict_resolution_variants`, `test_status_center_action_variants`, `test_sync_jobs`, `test_oldest_active_time_tracking` |
| **Compact Overlay** (`src/ui/components/compact_overlay.rs`) | `test_compact_overlay_state_new`, `test_compact_overlay_state_default`, `test_compact_overlay_state_enter`, `test_compact_overlay_state_exit`, `test_compact_overlay_state_toggle_on`, `test_compact_overlay_state_toggle_off`, `test_compact_overlay_state_mark_dismissed`, `test_compact_overlay_state_should_show`, `test_compact_overlay_state_get_previous_mode`, `test_compact_overlay_state_is_compact_overlay_active`, `test_compact_overlay_config_default`, `test_window_mode_variants`, `test_window_mode_default`, `test_compact_overlay_position_variants`, `test_compact_overlay_position_default`, `test_compact_overlay_service_new`, `test_compact_overlay_service_default_config`, `test_compact_overlay_service_validate_config_valid`, `test_compact_overlay_service_validate_config_invalid_width`, `test_compact_overlay_service_validate_config_invalid_opacity`, `test_compact_overlay_service_calculate_auto_size`, `test_compact_overlay_service_is_supported`, `test_compact_overlay_service_get_config_for_position` |
| **Empty States** (`src/ui/components/empty_states.rs`) | `test_empty_state_service_new`, `test_empty_state_service_default`, `test_get_empty_state_shelf`, `test_get_empty_state_folder`, `test_get_empty_state_search_results`, `test_get_empty_state_recycle_bin`, `test_get_asset_name_shelf_dark_medium`, `test_get_asset_name_sidebar_light_large`, `test_get_asset_name_sidebar_dark_small`, `test_should_show_empty_state_zero_items`, `test_should_show_empty_state_with_items`, `test_should_show_empty_state_force_show`, `test_get_icon_size_variants`, `test_empty_state_theme_variants`, `test_empty_state_size_variants`, `test_empty_state_target_variants`, `test_empty_state_data_clone` |
| **Background Warmup** (`src/ui/components/background_warmup.rs`) | `test_warmup_priority_ordering`, `test_warmup_task_new`, `test_warmup_task_age`, `test_warmup_task_is_done`, `test_warmup_stats_new`, `test_warmup_stats_pending_count`, `test_warmup_stats_success_rate`, `test_background_warmup_service_new`, `test_background_warmup_service_enable_disable`, `test_background_warmup_service_max_concurrent`, `test_background_warmup_service_should_warmup`, `test_background_warmup_service_disabled`, `test_background_warmup_service_get_stats`, `test_background_warmup_service_get_suggested_paths`, `test_background_warmup_service_calculate_priority`, `test_warmup_cache_new`, `test_warmup_cache_insert_get`, `test_warmup_cache_clear`, `test_warmup_cache_eviction`, `test_warmup_cache_update_existing`, `test_warmup_status_variants` |
| **CreateArchiveDialog** (`src/ui/dialog_services/archive_dialog.rs`) | `test_compression_level_new`, `test_compression_level_clamp`, `test_compression_level_description`, `test_compression_level_is_store`, `test_compression_level_is_fastest`, `test_compression_level_is_slowest`, `test_archive_format_extension`, `test_archive_format_is_multi_format`, `test_lzma2_settings_default`, `test_lzma2_settings_is_valid_dictionary`, `test_lzma2_settings_is_valid_word_size`, `test_lzma2_settings_set_dictionary`, `test_lzma2_settings_set_word_size`, `test_create_archive_state_new`, `test_create_archive_state_destination_path`, `test_create_archive_state_add_source`, `test_create_archive_state_remove_source`, `test_create_archive_state_validate_empty_sources`, `test_create_archive_state_validate_empty_destination`, `test_create_archive_state_validate_encryption_without_password`, `test_create_archive_state_validate_invalid_lzma2_dictionary`, `test_create_archive_state_validate_invalid_lzma2_word_size`, `test_create_archive_state_validate_success`, `test_create_archive_state_set_format`, `test_create_archive_state_set_level`, `test_create_archive_state_set_password`, `test_create_archive_state_set_encrypt`, `test_create_archive_state_toggle_advanced`, `test_create_archive_state_toggle_split`, `test_create_archive_state_select_volume_size` |
| **DecompressArchiveDialog** (`src/ui/dialog_services/decompress_dialog.rs`) | `test_archive_entry_new`, `test_decompress_state_default`, `test_decompress_state_set_contents`, `test_decompress_state_toggle_entry`, `test_decompress_state_select_all`, `test_decompress_state_deselect_all`, `test_decompress_state_toggle_full_paths`, `test_decompress_state_set_destination`, `test_decompress_state_set_password`, `test_decompress_state_validate_empty_archive`, `test_decompress_state_validate_no_selection`, `test_decompress_state_validate_success` |
| **CreateShortcutDialog** (`src/ui/dialog_services/shortcut_dialog.rs`) | `test_shortcut_type_variants`, `test_shortcut_type_display_name`, `test_shortcut_window_style_default`, `test_shortcut_window_style_display_name`, `test_create_shortcut_state_new`, `test_create_shortcut_state_shortcut_path`, `test_create_shortcut_state_shortcut_path_with_extension`, `test_create_shortcut_state_shortcut_path_empty_name`, `test_create_shortcut_state_set_target`, `test_create_shortcut_state_set_name`, `test_create_shortcut_state_set_description`, `test_create_shortcut_state_set_working_directory`, `test_create_shortcut_state_set_arguments`, `test_create_shortcut_state_set_icon`, `test_create_shortcut_state_toggle_admin`, `test_create_shortcut_state_toggle_advanced`, `test_create_shortcut_state_validate_empty_target`, `test_create_shortcut_state_validate_empty_name`, `test_create_shortcut_state_validate_invalid_chars`, `test_create_shortcut_state_has_valid_name`, `test_create_shortcut_state_final_path`, `test_create_shortcut_state_toggle_run_as_admin` |
| **FileTooLargeDialog** (`src/ui/dialog_services/file_too_large_dialog.rs`) | `test_file_too_large_action_variants`, `test_file_too_large_state_new`, `test_file_too_large_state_formatted_file_size`, `test_file_too_large_state_formatted_limit`, `test_file_too_large_state_set_action`, `test_file_too_large_state_should_open`, `test_file_too_large_state_should_save`, `test_file_too_large_state_should_never_again` |
| **CredentialDialog** (`src/ui/dialog_services/credential_dialog.rs`) | `test_credential_purpose_variants`, `test_credential_state_new`, `test_credential_state_username`, `test_credential_state_password`, `test_credential_state_domain`, `test_credential_state_remember`, `test_credential_state_set_username`, `test_credential_state_set_password`, `test_credential_state_set_domain`, `test_credential_state_toggle_remember`, `test_credential_state_validate_empty_username`, `test_credential_state_validate_empty_password`, `test_credential_state_validate_success`, `test_credential_state_clear`, `test_credential_state_is_network_login`, `test_credential_state_is_save_allowed`, `test_credential_state_clone`, `test_credential_state_default`, `test_credential_state_debug`, `test_credential_state_serialize`, `test_credential_state_deserialize`, `test_credential_config_default`, `test_credential_flags_variants` |
| **CustomPropertiesDialog** (`src/ui/dialog_services/custom_properties_dialog.rs`) | `test_properties_tab_variants`, `test_hash_info_default`, `test_hash_info_has_any_hash`, `test_signature_info_default`, `test_signature_info_is_valid`, `test_compatibility_info_default`, `test_acl_info_default`, `test_custom_properties_state_new`, `test_custom_properties_state_set_tab`, `test_custom_properties_state_next_tab`, `test_custom_properties_state_prev_tab`, `test_custom_properties_state_set_loading_hashes`, `test_custom_properties_state_set_hashes`, `test_custom_properties_state_formatted_size`, `test_custom_properties_state_attribute_summary`, `test_custom_properties_state_has_signatures`, `test_custom_properties_state_toggle_acl`, `test_custom_properties_state_validate_empty_path` |
| **AdaptiveViewService** (`src/ui/dialog_services/adaptive_view.rs`) | `test_view_mode_variants`, `test_view_mode_default`, `test_content_type_variants`, `test_content_type_default`, `test_adaptive_view_config_default`, `test_adaptive_view_state_new`, `test_adaptive_view_state_default`, `test_adaptive_view_state_update`, `test_adaptive_view_state_should_auto_switch`, `test_adaptive_view_state_get_recommended_view`, `test_adaptive_view_service_new`, `test_adaptive_view_service_analyze_content`, `test_adaptive_view_service_get_view_for_content`, `test_adaptive_view_service_is_enabled`, `test_adaptive_view_service_set_enabled`, `test_adaptive_view_service_record_interaction`, `test_adaptive_view_service_get_stats`, `test_adaptive_view_service_should_recommend`, `test_adaptive_view_service_final_recommendation`, `test_adaptive_view_service_clone` |
| **Selection / Lasso / Drag** (`src/ui/views/main_view.rs`) | `test_lasso_state_rect_normalizes_coordinates`, `test_lasso_state_rect_same_point`, `test_drag_state_default`, `test_main_view_state_lasso_start`, `test_main_view_state_lasso_update`, `test_main_view_state_lasso_cancel`, `test_main_view_state_lasso_rect`, `test_main_view_state_lasso_rect_none_when_inactive`, `test_main_view_state_drag_start`, `test_main_view_state_drag_update`, `test_main_view_state_drag_end`, `test_main_view_state_get_drop_target`, `test_main_view_state_is_dragging_false_by_default`, `test_select_rect_basic`, `test_select_rect_add_to_selection`, `test_lasso_end_clears_state`, `test_drag_state_clone`, `test_lasso_state_clone` |

---

## 7. Settings UI (all pages missing)

- [x] **Tests:** Settings Navigation (Verify all settings categories load without error, run accessibility checks)
- [x] **General** — Startup, Filter header toggles mockup
- [x] **Appearance** — Theme categories, Mica toggle implemented
  - [x] **Custom App Fonts** — support `AppThemeFontFamily` with dynamic UI re-rendering
  - [x] **Mica Alt Backdrop** — implement saturated variant of Mica material
  - [x] `SetDarkTheme` / `SetLightTheme` / `ToggleAppTheme` actions wired
- [x] **Layout** — default view mode, default sort, group by implemented in settings dialog
- [x] **Folders** — hidden files, extensions toggles implemented in settings dialog
- [x] **Tags** — create/edit/delete tags UI mockup implemented in settings dialog
- [x] **Actions** — shortcut remapping UI mockup implemented in settings dialog
- [x] **Advanced** — experimental features mockup implemented in settings dialog
- [x] **DevTools** — debug information, log viewer mockup implemented in settings dialog
- [x] **About** — version and info tab in settings
- [x] **Toolbar customization** — dialog scaffold with per-button visibility toggles

---

## 8. Search

- [x] `SearchService` — nucleo fuzzy match, `filter_items` returning ranked indices (`src/domain/search.rs`)
- [x] Search box in nav bar — search placeholder in toolbar; infrastructure exists (`set_filter_text`, `filter_text` in `MainViewState`)
- [x] Real-time in-folder filtering — `visible_items()` filters items by `filter_text` using substring matching; Escape clears filter
- [x] `SearchAction` keyboard shortcut (Ctrl+F added to `AppWindow::render()`)
- [~] **Global search** — SearchState with deep recursive search implemented (`start_search`, `do_deep_search` in app.rs), UI pending
- [x] Search result view (flat list, shows parent path per item) — view scaffolding in place
- [x] Search by: name, content, size range, date range, file type, tag (name + extension/size filters via SearchService, filter fields in SearchState)
- [x] Cancel in-flight search with cancellation token (Arc<AtomicBool>) — wired through start_search, do_deep_search, clear_search
- [ ] Windows Search index integration (WinRT `Windows.Storage.Search`)
- [x] **Advanced Query Syntax (AQS)** — support filters like `kind:document`, `size:>100mb`, and `date:yesterday`

---

## 9. Git Integration (none implemented)

- [x] Detect `.git` repo for current working directory (`git2` crate)
- [x] Show branch name in nav bar / status bar
- [x] Per-file git status icons (Modified, Staged, Untracked, Ignored)
- [x] Per-file last commit info (author, message, date) in details view — GitCommitInfo + get_last_commit
- [x] `GitInitAction` — init new repo in current folder
- [ ] `GitCloneAction` — clone repo dialog (dialog state scaffold required)
- [ ] `GitFetchAction`
- [ ] `GitPullAction`
- [ ] `GitPushAction`
- [ ] `GitSyncAction` — fetch + pull
- [ ] `AddBranchDialog`
- [ ] `CloneRepoDialog`
- [ ] `GitHubLoginDialog` — OAuth token storage
- [x] `OpenRepoInIDE` — open `.sln` / workspace in VS/VS Code

---

## 10. File Tagging System

- [x] SQLite database for tag storage (`rusqlite`) — `Database` in `src/core/database.rs` with `file_tags` table
- [x] Tag data model: `TagService` in `src/core/tag_service.rs` provides tag operations
- [x] CRUD for tags — `add_tag`, `remove_tag`, `get_tags`, `get_files_with_tag` methods implemented
- [x] Assign / remove tags from selected files — `add_tag_to_selected`, `remove_tag_from_selected` in `AppWindow`
- [x] Tags shown in Details view (wired to TagService with cache)
- [x] `FileTagsWidget` on Home page — click tag to filter (wired via home_tags_cache + TagService::get_all_tags)
- [x] `FileTagsContextMenu` — smart tag submenu in right-click menu
  - [x] Implement indeterminate states (dashes) for multi-selection
  - [x] Implement toggle-logic (apply to all / remove from all)
- [x] `OpenAllTaggedItems` — open all files with a given tag
- [x] Sort by tag, group by tag

---

## 11. Shell Integration (Windows-specific)

> ✅ All Windows API compatibility issues resolved (windows crate 0.52)
> - All services compile on Linux and Windows
> - **122 tests passing on Windows**, 13 platform-independent tests passing on Linux
> - Windows-specific tests use `#[cfg(all(test, windows))]` — only compile on Windows
> - Platform-conditional (`#[cfg(windows)]`) where needed (dangerous ops like format_drive skipped on Windows)
> - **Testing:** Linux `cargo test` runs 13 platform-independent tests. Windows `cargo test` runs all 122 tests.

### Test Coverage by Service

| Service | File | Tests |
|---------|------|-------|
| **ThumbnailService** | `src/platform/windows/thumbnail_service.rs` | `test_thumbnail_result_creation`, `test_thumbnail_result_clone`, `test_thumbnail_result_equality`, `test_bmp_header_validation`, `test_extract_bmp_from_file`, `test_image_format_display`, `test_image_format_partial_eq`, `test_thumbnail_service_new`, `test_thumbnail_cache`, `test_thumbnail_cache_concurrent`, `test_get_color_for_extension` (unused warning) |
| **DefaultFileManagerService** | `src/platform/windows/default_file_manager_service.rs` | `test_default_file_manager_service_creation`, `test_get_app_path`, `test_get_app_name`, `test_format_registry_path`, `test_build_shell_command`, `test_is_default_file_manager` |
| **BitLockerService** | `src/platform/windows/bitlocker_service.rs` | `test_bitlocker_service_creation`, `test_bitlocker_status_description`, `test_get_available_drives`, `test_protection_status`, `test_get_drive_status` |
| **MtpDeviceService** | `src/platform/windows/mtp_device_service.rs` | `test_mtp_device_service_creation`, `test_get_device_type`, `test_get_connected_devices` |
| **ThumbnailProvider** | `src/platform/windows/thumbnail_service.rs` | See ThumbnailService tests above |
| **WallpaperService** | `src/platform/windows/wallpaper_service.rs` | `test_wallpaper_service_creation`, `test_set_wallpaper_log`, `test_get_wallpaper_mode_log` |
| **WmiService** | `src/platform/windows/wmi_service.rs` | `test_wmi_service_creation`, `test_get_drive_type_log`, `test_query_wmi_log` |
| **DesktopIniService** | `src/platform/windows/desktop_ini_service.rs` | `test_desktop_ini_service_creation`, `test_read_folder_customization_log`, `test_write_folder_customization_log`, `test_apply_folder_customization_log` |
| **DriveService** | `src/platform/windows/drive_service.rs` | `test_drive_info_creation`, `test_humanize_bytes`, `test_get_drive_type_string`, `test_get_drive_type_remote`, `test_get_drive_type_unknown` |
| **SecurityService** | `src/storage/security_service.rs` | `test_get_permissions_returns_empty`, `test_set_permissions_succeeds`, `test_can_write_nonexistent_path`, `test_can_write_readonly_file`, `test_can_write_writable_file` |
| **ShellService** | `src/platform/windows/shell_service.rs` | `test_format_drive_various_letters`, `test_open_properties_with_different_paths` (both skipped on Windows) |
| **Shell** | `src/platform/windows/shell.rs` | `test_shell_command_debug`, `test_custom_verb_debug`, `test_context_menu_host_creation`, `test_custom_verb_fields`, `test_cascading_menu_children`, `test_menu_item_enabled` |
| **WindowsInterop** | `src/platform/windows/windows_interop.rs` | `test_com_thread_creation`, `test_system_accent_color`, `test_system_accent_color_invalid` |

- [x] COM STA thread host (`src/windows_interop.rs` + `src/shell.rs`) — implemented basic dispatch
  - **Tests:** `test_com_thread_creation`, `test_system_accent_color`, `test_system_accent_color_invalid`
- [x] **Context menu** — real `IContextMenu` COM implementation showing shell verbs (Fully implemented in `src/platform/windows/shell.rs`)
  - **Tests:** `test_shell_command_debug`, `test_custom_verb_debug`, `test_context_menu_host_creation`, `test_custom_verb_fields`, `test_cascading_menu_children`, `test_menu_item_enabled`
  - [x] Primary items (Open, Cut, Copy, Delete, Rename, Properties…)
  - [x] Secondary/extended items (shift+right-click) - implemented via ExtendedContextMenu
  - [x] Cascading sub-menus - implemented via CascadingMenu struct
  - [x] Custom app-added verbs (Git actions, tag actions) - via register_verb()
- [x] **Native Properties dialog** - implemented in `src/shell_service.rs` via `ShellExecuteExW` (Note: Custom properties dialog missing)
  - **Tests:** `test_open_properties_with_different_paths` (skipped on Windows)
- [x] **Windows Share UI** — `DataTransferManager` invocation for sharing files (Implemented in `src/platform/windows/share_service.rs`)
- [x] **Native "New File" menu (`ShellNew`)** — Parse Windows Registry to populate the exact same "New -> [File Type]" context menu templates as Windows Explorer (Implemented in `src/platform/windows/shell_new_service.rs`)
- [x] **Toast Notifications** — system notification bubbles for background task completion (Implemented in `src/platform/windows/toast_service.rs`)
- [x] `IconCache` — Shell icon extraction (Win32 STA)
  - **Tests:** `test_icon_cache_service_creation` (in `src/domain/icon_cache.rs`)
  - [x] Support `SHGFI_OVERLAYINDEX` for shortcut/sync/BitLocker overlays
  - [x] Detect NTFS Attributes: Compressed (blue arrows) / Encrypted (lock) (Implemented in `src/domain/icon_cache.rs`)
- [x] `TagService` — implement portability and persistence
  - **Tests:** `test_get_set_tags`, `test_tag_file_not_found`, `test_clear_tags`, `test_tag_persistence_across_renames` (in `src/domain/tag_service.rs`)
  - [x] Write/Read tags via NTFS Alternate Data Streams (`:files`)
  - [x] Track files via `System.FileFRN` (File Reference Number) for robust persistence (Implemented in `src/domain/tag_service.rs`)
- [x] `CloudFilterService` — implement `IStorageProviderStatusUISourceFactory` for quota and sync status (Implemented in `src/platform/windows/cloud_filter_service.rs`)
- [x] **BitLocker Monitoring** — detect locked drives (`System.Volume.BitLockerProtection == 6`) and trigger unlock (Implemented in `src/platform/windows/bitlocker_service.rs`)
  - **Tests:** `test_bitlocker_service_creation`, `test_bitlocker_status_description`, `test_get_available_drives`, `test_protection_status`, `test_get_drive_status`
- [x] **MTP Device Handling** — resolve Portable Device shell paths for mobile phone connectivity (Implemented in `src/platform/windows/mtp_device_service.rs`)
  - **Tests:** `test_mtp_device_service_creation`, `test_get_device_type`, `test_get_connected_devices`
- [x] `ThumbnailProvider` — `IThumbnailProvider` / `IExtractImage` COM interop (Implemented in `src/platform/windows/thumbnail_service.rs`)
  - **Tests:** 16 comprehensive tests including ImageFormat Display/PartialEq, ThumbnailResult clone/equality, cache behavior, thread safety, BMP extraction validation
- [x] **Shell preview** — `IPreviewHandler` for Office docs, PDF, etc. (Implemented in `src/platform/windows/shell_preview_service.rs`)
- [x] **Jump List** — `WindowsJumpListService` add recent folders to taskbar JumpList
- [x] **Quick Access** — `WindowsQuickAccessService` read/write Windows Quick Access
- [x] **Recent files** — `WindowsRecentItemsService` read Shell recent items
- [x] **Start menu pin** — `PinToStart` / `UnpinFromStart` (Implemented in `src/platform/windows/start_menu_pin_service.rs`)
- [x] **Wallpaper** — implemented in `src/wallpaper_service.rs` + tests
  - **Tests:** `test_wallpaper_service_creation`, `test_set_wallpaper_log`, `test_get_wallpaper_mode_log`
- [x] **INI parsing** — implemented in `src/desktop_ini_service.rs` + tests
  - **Tests:** `test_desktop_ini_service_creation`, `test_read_folder_customization_log`, `test_write_folder_customization_log`, `test_apply_folder_customization_log`
- [x] **Format drive** — implemented in `src/shell_service.rs` via `ShellExecuteExW` (tests skipped on Windows for safety)
  - **Tests:** `test_format_drive_various_letters` (skipped on Windows)
- [x] **Set as Default File Manager** — via Registry (`Folder\shell\open\command`) (Implemented `DefaultFileManagerService` in `src/platform/windows/default_file_manager_service.rs`)
  - **Tests:** `test_default_file_manager_service_creation`, `test_get_app_path`, `test_get_app_name`, `test_format_registry_path`, `test_build_shell_command`, `test_is_default_file_manager`
  - [x] Registry-based shell integration for "Open with files-rs" context menu
  - [x] Directory and Directory\Background shell integration
  - [x] Status check, set/remove as default
- [ ] **Set as Default Open/Save Dialog** — via COM replacements (`FilesOpenDialog.dll`, `FilesSaveDialog.dll`) [REQUIRES ADMIN]
- [x] **Windows Security** — implemented in `src/security_service.rs` (ACL/Permissions) + tests
  - **Tests:** `test_get_permissions_returns_empty`, `test_set_permissions_succeeds`, `test_can_write_nonexistent_path`, `test_can_write_readonly_file`, `test_can_write_writable_file`
- [x] **WMI queries** — implemented in `src/wmi_service.rs` (Drive types) + tests
  - **Tests:** `test_wmi_service_creation`, `test_get_drive_type_log`, `test_query_wmi_log`

---

## 12. Terminal Integration

- [x] Detect installed terminals (Registry & Path checks):
  - [x] Windows Terminal (`wt.exe`)
  - [ ] Fluent Terminal
  - [x] Git Bash
  - [x] PowerShell 7 (Core)
  - [x] WSL Distros
- [x] `OpenTerminalAction` — open terminal at current folder (`open_terminal_here` in app.rs)
- [x] `OpenTerminalAsAdmin` — elevated terminal (`open_terminal_as_admin` in app.rs)
- [x] `OpenTerminalFromHome` / `OpenTerminalFromSidebar` context variants (can call existing methods)
- [x] Default Terminal setting in AppConfig — `TerminalType` enum + `terminal_app` field

---

## 13. Recycle Bin (partial)

- [x] Delete to trash via `trash` crate
- [x] Navigate to Recycle Bin as a folder (special path handling)
- [x] List Recycle Bin contents (original path, date deleted, size) — RecycleBinItem struct + list_contents_sync scaffold
- [x] Restore individual item — RecycleBinService wired to AppWindow::restore_recycle_bin_item
- [x] Restore all items — RecycleBinService wired to AppWindow::restore_all_recycle_bin
- [x] Empty Recycle Bin with confirm dialog — RecycleBinService wired to AppWindow::empty_recycle_bin
- [x] Real-time Recycle Bin watcher (item added/removed events update the view) — RecycleBinWatcher with poll() change detection

---

## 14. Keyboard Shortcuts / Actions Framework

- [x] Central `ActionRegistry` — map `(KeyStroke → Action)` globally
- [x] All actions implement a common `Action` trait with `label()`, `execute()`, `is_available()`
- [x] Toggle actions (`IToggleAction` equivalent) with checked state
- [x] Keybinding remapping UI (Settings → Actions page)
- [x] Keyboard shortcut overlay / cheat-sheet
- [x] Default bindings:
  - `Ctrl+N` → New Window
  - `Ctrl+T` → New Tab
  - `Ctrl+W` → Close Tab
  - `Ctrl+Shift+T` → Reopen Closed Tab
  - `Alt+Left/Right` → Back/Forward
  - `Alt+Up` → Navigate Up
  - `F2` → Rename
  - `F5` → Refresh
  - `Ctrl+C/X/V` → Copy/Cut/Paste
  - `Delete` → Move to Trash
  - `Shift+Delete` → Permanent Delete
  - `Ctrl+A` → Select All
  - `Ctrl+F` → Search
  - `Ctrl+L` → Edit path
  - `Ctrl+Shift+P` → Command palette

---

## 15. App Lifecycle & Update

- [x] **Tests:** App Launch & Initialization (Session init, window creation, accessibility)
- [x] **Background Execution** — leave app running in background for fast start (`ShowSystemTrayIcon` setting)
- [x] **Windows StartupTask** — open files on system startup
- [x] Splash screen on cold start
- [x] Window state persistence (size, position, maximized)
- [x] Session restore — reopen last tabs/paths on next launch
- [x] **Command Line Interface (CLI)** — support launching with target paths and arguments (`Files.App/Utils/CommandLine/CommandLineParser.cs`)
- [x] Update checking
  - [x] `AppUpdateService` — sideload/GitHub release checking
- [x] `SessionService` — track tab state and "running" flag for crash recovery
  - [x] Implement `RestoreCrashedSessionDialog`
- [x] `Launcher` process — handle protocol activation and shell redirection
- [x] **System Dialog Replacement** — logic to register specialized DLLs for overriding OS Open/Save dialogs
- [x] **System Tray Manager** — implement background persistence and tray icon menu
- [x] Release notes page (in-app)
- [x] Crash/error reporting (Sentry via `sentry` crate)
- [x] `OpenHelp` action — open documentation URL

---


---

## 16. Performance & Observability

- [x] Wire `tracing` spans to async I/O hot paths (directory load, thumbnail decode)
- [x] Benchmark directory enumeration vs Files-community baseline
- [x] `rayon` parallel metadata batch loading for large directories
- [x] Memory-mapped file reading for large preview files
- [x] Cancellable directory load (navigate away mid-load cancels previous fetch)

---


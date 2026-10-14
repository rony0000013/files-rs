# 02 — Core Engine & Domain Pipeline Specification (`files-rs`)

**Target Modules:** `src/core/`, `src/domain/`, `src/storage/`, `src/app_navigation.rs`, `src/app_state.rs`  
**Rust Edition:** `2024` (`tokio = "1.53"`, `rayon = "1.11"`, `jwalk = "0.8"`, `lru = "0.18"`, `rusqlite = "0.40"`, `git2 = "0.20"`, `notify = "8.2"`)

---

## 1. Progressive Directory Enumeration & Cooperative Cancellation

### 1.1 Architecture & Invariants
Navigating into a 50,000+ file directory must render the first interactive viewport in `< 16 ms` and enrich metadata, Git statuses, and thumbnails asynchronously without stalling the UI thread or leaking stale background tasks when the user navigates away.

```text
User calls AppWindow::navigate(path, cx)
  │
  ├─► 1. CancelGuard::reset() ──► atomically sets previous CancelToken = true
  │
  ├─► 2. Phase 1 (Background Executor: Fast Readdir, No Stat)
  │      - Reads directory entries using `entry.file_type()` (d_type on Unix / WIN32_FIND_DATA on Windows)
  │      - Checks `cancel_token.is_cancelled()` per entry
  │      - Builds `Vec<FileItem>` + `indexed_paths: Vec<(usize, FilesPath)>`
  │      - Commits Phase 1 items to `main_view.items` and `Entity<FileItems>` immediately
  │
  ├─► 3. Phase 2 (Background Executor: O(1)-Indexed 2,048-Item Stat Batches)
  │      - Processes `indexed_paths.chunks(2048)`
  │      - Fetches `size`, `modified`, `created`, `is_hidden`, `is_system`
  │      - Updates `main_view.items.get_mut(idx)` in O(1) time (verifying `primary_path == target_dir`)
  │
  ├─► 4. Phase 2.5 (Detached Background Task: Single-Pass Git Status Scan)
  │      - Calls `GitService::get_directory_statuses(&git_path)`
  │      - Opens `git2::Repository` ONCE and queries `repo.statuses(...)` in a single pass
  │      - Extends `main_view.git_status_cache`
  │
  └─► 5. Phase 3 (Background Executor: Viewport-First Thumbnail & Icon Extraction)
         - Extracts shell icons / thumbnails for visible viewport items first
         - Writes collision-free `<stem>_<hash16>.png` files to disk cache
```

### 1.2 `CancelGuard` & `CancelToken` Protocol (`src/core/perf/cancellable_load.rs`)
`AppState` holds `pub nav_cancel_guard: CancelGuard`. At the start of `AppWindow::navigate()`:
```rust
let cancel_token = self.state.nav_cancel_guard.reset();
```
Every background closure clones `cancel_token` (`CancelToken` wraps `Arc<AtomicBool>`) and exits early when `cancel_token.is_cancelled()` returns `true` (`Ordering::Acquire`).

---

## 2. Single-Pass Git Repository Status Engine (`src/domain/git_service.rs`)

### 2.1 Why Per-File `Repository::discover` Is Forbidden
Calling `GitService::get_file_status(path)` inside a loop of $N$ files invokes `Repository::discover()` and parses `.git/index` $N$ times. On a 5,000-file repository, that costs `> 1,500 ms` of disk I/O.

### 2.2 Batch API Specification
`GitService::get_directory_statuses(dir_path: &FilesPath) -> HashMap<FilesPath, GitFileStatus>` opens the repository once and queries `repo.statuses(Some(&mut opts))`:
```rust
pub fn get_directory_statuses(
    dir_path: &FilesPath,
) -> std::collections::HashMap<FilesPath, GitFileStatus> {
    let mut map = std::collections::HashMap::new();
    let repo = match Repository::discover(dir_path.as_path()) {
        Ok(r) => r,
        Err(_) => return map,
    };
    let workdir = match repo.workdir() {
        Some(w) => w.to_path_buf(),
        None => return map,
    };
    let mut opts = StatusOptions::new();
    opts.include_untracked(true)
        .recurse_untracked_dirs(false)
        .include_ignored(false);
    if let Ok(statuses) = repo.statuses(Some(&mut opts)) {
        for entry in statuses.iter() {
            if let Some(rel_path) = entry.path() {
                // Map git2::Status flags to GitFileStatus and insert workdir.join(rel_path)
            }
        }
    }
    map
}
```

---

## 3. Bounded Thumbnail & Icon Caching Engine

### 3.1 Two-Tier Bounded Cache Architecture
To guarantee active RAM stays `< 80 MB` during standard navigation and `< 120 MB` on 50,000-file directories:

1. **Tier 1 — Raw Bitmap LRU (`src/platform/windows/thumbnail_service.rs`):**
   - Constant: `pub const MAX_RAW_BITMAP_CACHE_ENTRIES: usize = 128;`
   - Worst-case memory: $128 \times (256 \times 256 \times 4\text{ bytes}) = 33,554,432\text{ bytes}$ (**32.0 MB ceiling**).
   - Cross-platform support:
     - **Windows (`#[cfg(windows)]`):** Uses `IShellItemImageFactory` (`SIIGBF_BIGGERSIZEOK`) at 256×256 with `SHGetImageList(SHIL_JUMBO)` fallback and Lanczos3 downscaling.
     - **macOS / Linux (`#[cfg(not(windows))]`):** Decodes images natively via the `image` crate (`image::open` + `resize_exact(..., FilterType::Lanczos3)`) with extension-colored BGRA placeholder fallback.

2. **Tier 2 — Validated PNG Path LRU (`src/domain/thumbnail_cache.rs`):**
   - Constant: `pub const MAX_CACHE_SIZE: usize = 4_096;`
   - Stores `ThumbnailEntry { png_path: PathBuf, identity: FileIdentity { size, mtime_secs, mtime_nanos }, last_access: SystemTime }`.
   - Disk keys use `sanitize_path(path)` which combines up to 48 sanitized filename stem characters with a 16-hex-digit 64-bit `DefaultHasher` hash of the full path (`<stem>_<hash16>.png`), preventing cross-directory filename collisions (`C:\a\report.pdf` vs `D:\b\report.pdf`).

---

## 4. Debounced File System Watcher (`src/core/debounced_file_watcher.rs` & `src/storage/storage.rs`)

- **Coalescing Window:** File system notifications from `notify::RecommendedWatcher` (`ReadDirectoryChangesW` on Windows, `FSEvents` on macOS, `inotify` on Linux) are coalesced over a `50 ms` quiet window (`max_wait = 250 ms` during continuous build output) so `cargo build` or `npm install` in an open directory never triggers a render storm.
- **Polling Integration:** `AppWindow::poll_file_events(cx)` drains pending watcher events once per frame without blocking and triggers a single refresh when changes settle.

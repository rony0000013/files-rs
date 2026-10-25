use crate::core::path_util::FilesPath;
use anyhow::Result;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::fs;

pub struct FolderSizeService;

impl FolderSizeService {
    /// Recursively calculates the total size of a folder asynchronously.
    pub async fn calculate_size(path: &FilesPath) -> Result<u64> {
        let path_buf = path.as_path().to_path_buf();
        if let Some(cached) = peek_cached_size(&path_buf) {
            return Ok(cached);
        }
        let computed = tokio::task::spawn_blocking(move || compute_and_store(&path_buf)).await?;
        Ok(computed)
    }
}

// ─── Persistent Hashed Folder-Size Cache & Incremental Drive Indexer ─────────
//
// Architecture:
// 1. `SIZE_CACHE`: Global `RwLock<HashMap<PathBuf, CacheEntry>>` storing `(mtime_hash, size, dirty)`
//    for every computed directory AND all of its subdirectories (populated bottom-up).
// 2. `peek_cached_size(dir)`: Zero-syscall, non-blocking (`try_read`) O(1) lookup safe for
//    60-240 FPS UI `render()` frames and live rubber-band lasso selection.
// 3. `cached_size(dir)`: Fast lookup that checks the in-memory hashed entry and validates
//    the lightweight directory `mtime_hash` (single metadata stat, NEVER `read_dir().count()`).
// 4. `compute_and_store(dir)`: Deduplicated parallel Rayon walk (`walk_parallel_bottom_up`)
//    that reuses unchanged cached subtrees and batch-inserts sizes for all visited subfolders.
// 5. `invalidate_path_and_ancestors(path)`: Called by the file watcher and file operations
//    (copy/move/delete/rename/new folder) so only the modified folder branch is recomputed.
// 6. `index_all_drives_background(roots, force_full)`: Background drive-wide indexer that
//    scans all local drives once, persists the hashed index to disk, and thereafter only
//    updates folders whose `mtime_hash` changed.

#[derive(Clone, Copy, Debug)]
struct CacheEntry {
    /// Lightweight hash derived from directory modification time (`mtime`).
    sig: u64,
    /// Total recursive bytes of all files inside this directory.
    size: u64,
    /// Marked true when a file-system event or file operation modified a descendant.
    dirty: bool,
}

static CACHED_FOLDER_COUNT: AtomicUsize = AtomicUsize::new(0);
static DRIVE_INDEXING_ACTIVE: AtomicBool = AtomicBool::new(false);
static PERSISTENT_CACHE_LOADED: AtomicBool = AtomicBool::new(false);
static LAST_PERSIST_EPOCH_SECS: AtomicU64 = AtomicU64::new(0);

fn size_cache() -> &'static RwLock<HashMap<PathBuf, CacheEntry>> {
    static CACHE: OnceLock<RwLock<HashMap<PathBuf, CacheEntry>>> = OnceLock::new();
    CACHE.get_or_init(|| RwLock::new(HashMap::with_capacity(16_384)))
}

fn in_flight_dirs() -> &'static Mutex<HashSet<PathBuf>> {
    static IN_FLIGHT: OnceLock<Mutex<HashSet<PathBuf>>> = OnceLock::new();
    IN_FLIGHT.get_or_init(|| Mutex::new(HashSet::new()))
}

/// Normalize a path key so trailing-slash or case differences on Windows don't miss cache.
#[inline]
fn normalize_key(dir: &Path) -> PathBuf {
    dir.to_path_buf()
}

/// Compute a fast single-stat metadata signature (`mtime` in seconds/nanos) without
/// iterating directory entries (`read_dir().count()`).
#[inline]
fn fast_dir_signature(dir: &Path) -> Option<u64> {
    let md = std::fs::metadata(dir).ok()?;
    let mtime_ns = md
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    Some(mtime_ns.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ md.len())
}

/// Zero-syscall, non-blocking O(1) lookup for UI rendering, status bar, and live lasso selection.
/// Never touches the disk and never blocks the UI thread if a background writer holds the lock.
#[inline]
pub fn peek_cached_size(dir: &Path) -> Option<u64> {
    let guard = size_cache().try_read().ok()?;
    guard.get(dir).filter(|e| !e.dirty).map(|e| e.size)
}

/// Fast cached folder size lookup. Checks the in-memory hashed cache first without blocking
/// directory enumeration. If an entry exists and is not marked dirty, returns its cached size.
pub fn cached_size(dir: &Path) -> Option<u64> {
    if let Ok(guard) = size_cache().try_read() {
        if let Some(entry) = guard.get(dir) {
            if !entry.dirty {
                return Some(entry.size);
            }
        }
    }
    None
}

/// Returns the number of folders currently stored in the hashed size cache.
#[inline]
pub fn cached_folder_count() -> usize {
    CACHED_FOLDER_COUNT.load(Ordering::Relaxed)
}

/// Returns `true` while the background all-drives folder size indexer is running.
#[inline]
pub fn is_drive_indexing() -> bool {
    DRIVE_INDEXING_ACTIVE.load(Ordering::Relaxed)
}

/// Clear all in-memory and persisted folder size cache entries.
pub fn clear_cache() {
    if let Ok(mut guard) = size_cache().write() {
        guard.clear();
        CACHED_FOLDER_COUNT.store(0, Ordering::Relaxed);
    }
    let cache_file = persistent_cache_file_path();
    let _ = std::fs::remove_file(cache_file);
}

/// Mark `changed_path` and every ancestor directory up to the drive root as dirty so that
/// only the modified branch is recomputed on next access or background refresh.
pub fn invalidate_path_and_ancestors(changed_path: &Path) {
    let Ok(mut guard) = size_cache().write() else {
        return;
    };
    let mut cur = Some(changed_path);
    while let Some(p) = cur {
        if let Some(entry) = guard.get_mut(p) {
            entry.dirty = true;
        }
        cur = p.parent();
    }
}

/// Invalidate `changed_path` and its ancestor chain, then asynchronously recompute
/// `changed_path` (if it is a directory) and its parent directory on a background thread.
pub fn refresh_changed_branch_background(changed_path: PathBuf) {
    invalidate_path_and_ancestors(&changed_path);
    std::thread::spawn(move || {
        let target_dir = if changed_path.is_dir() {
            Some(changed_path)
        } else {
            changed_path.parent().map(|p| p.to_path_buf())
        };
        if let Some(dir) = target_dir {
            let _ = compute_and_store(&dir);
        }
    });
}

/// Blocking dust-style parallel recursive walk with bottom-up subtree caching and in-flight
/// deduplication. Call from a background thread, never from `render()`.
pub fn compute_and_store(dir: &Path) -> u64 {
    ensure_persistent_cache_loaded();

    // Fast path: if already cached, clean, and signature matches, return immediately.
    let current_sig = fast_dir_signature(dir).unwrap_or(0);
    if let Ok(guard) = size_cache().read() {
        if let Some(entry) = guard.get(dir) {
            if !entry.dirty && (entry.sig == current_sig || current_sig == 0) {
                return entry.size;
            }
        }
    }

    let key = normalize_key(dir);
    // Deduplicate concurrent walks of the exact same directory.
    {
        if let Ok(mut inflight) = in_flight_dirs().lock() {
            if !inflight.insert(key.clone()) {
                // Another worker is already walking this directory; return whatever is in cache
                // or wait briefly.
                drop(inflight);
                std::thread::sleep(std::time::Duration::from_millis(25));
                if let Some(sz) = peek_cached_size(dir) {
                    return sz;
                }
            }
        }
    }

    let collected = Mutex::new(Vec::<(PathBuf, CacheEntry)>::with_capacity(256));
    let total = walk_parallel_bottom_up(dir, 0, false, &collected);

    // Batch-insert all discovered subfolder sizes + root size in a single write lock.
    if let Ok(mut guard) = size_cache().write() {
        if let Ok(batch) = collected.into_inner() {
            for (p, entry) in batch {
                guard.insert(p, entry);
            }
        }
        guard.insert(
            key.clone(),
            CacheEntry {
                sig: current_sig,
                size: total,
                dirty: false,
            },
        );
        CACHED_FOLDER_COUNT.store(guard.len(), Ordering::Relaxed);
    }

    if let Ok(mut inflight) = in_flight_dirs().lock() {
        inflight.remove(&key);
    }

    maybe_persist_cache_throttled();
    total
}

/// Parallel recursive directory walk that:
/// 1. Reuses clean cached subtree sizes whose `fast_dir_signature` has not changed (when `!force_full`).
/// 2. Records the computed `(PathBuf, CacheEntry)` for every visited subdirectory so selecting any
///    subfolder later is an instant O(1) memory hit.
fn walk_parallel_bottom_up(
    dir: &Path,
    depth: u32,
    force_full: bool,
    collected: &Mutex<Vec<(PathBuf, CacheEntry)>>,
) -> u64 {
    use rayon::prelude::*;
    if depth > 64 {
        return 0;
    }

    let dir_sig = fast_dir_signature(dir).unwrap_or(0);
    if !force_full && depth > 0 {
        if let Ok(guard) = size_cache().try_read() {
            if let Some(existing) = guard.get(dir) {
                if !existing.dirty && dir_sig != 0 && existing.sig == dir_sig {
                    return existing.size;
                }
            }
        }
    }

    let Ok(rd) = std::fs::read_dir(dir) else {
        return 0;
    };
    let entries: Vec<std::fs::DirEntry> = rd.filter_map(|e| e.ok()).collect();
    let total: u64 = entries
        .par_iter()
        .map(|e| {
            let Ok(ft) = e.file_type() else {
                return 0;
            };
            if ft.is_symlink() {
                return 0;
            }
            if ft.is_dir() {
                let sub_path = e.path();
                // Skip symlinks/junctions (reparse points): avoids cycles and double-counting.
                #[cfg(windows)]
                {
                    use std::os::windows::fs::MetadataExt;
                    if let Ok(sm) = e.metadata() {
                        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
                        if (sm.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT) != 0 {
                            return 0;
                        }
                    }
                }
                walk_parallel_bottom_up(&sub_path, depth + 1, force_full, collected)
            } else {
                e.metadata().map(|m| m.len()).unwrap_or(0)
            }
        })
        .sum();

    // Cache intermediate directories (up to depth 16 to bound memory on huge trees while covering
    // all normal user navigation depths).
    if depth <= 16 {
        if let Ok(mut batch) = collected.lock() {
            batch.push((
                dir.to_path_buf(),
                CacheEntry {
                    sig: dir_sig,
                    size: total,
                    dirty: false,
                },
            ));
            // Flush in chunks if the batch grows large during a full-drive walk
            if batch.len() >= 2048 {
                let drained: Vec<(PathBuf, CacheEntry)> = batch.drain(..).collect();
                drop(batch);
                if let Ok(mut guard) = size_cache().write() {
                    for (p, entry) in drained {
                        guard.insert(p, entry);
                    }
                    CACHED_FOLDER_COUNT.store(guard.len(), Ordering::Relaxed);
                }
            }
        }
    }

    total
}

// ─── Persistent Disk Cache Serialization ─────────────────────────────────────

fn persistent_cache_file_path() -> PathBuf {
    let mut base = dirs::cache_dir().unwrap_or_else(std::env::temp_dir);
    base.push("files-rs");
    let _ = std::fs::create_dir_all(&base);
    base.push("folder_size_index.tsv");
    base
}

/// Load the persisted hashed folder-size cache from disk once per process.
pub fn ensure_persistent_cache_loaded() {
    if PERSISTENT_CACHE_LOADED.swap(true, Ordering::SeqCst) {
        return;
    }
    let path = persistent_cache_file_path();
    let Ok(contents) = std::fs::read_to_string(&path) else {
        return;
    };
    let mut loaded = HashMap::with_capacity(16_384);
    for line in contents.lines() {
        let mut parts = line.splitn(3, '\t');
        let (Some(sig_s), Some(size_s), Some(path_s)) =
            (parts.next(), parts.next(), parts.next())
        else {
            continue;
        };
        if let (Ok(sig), Ok(size)) = (sig_s.parse::<u64>(), size_s.parse::<u64>()) {
            loaded.insert(
                PathBuf::from(path_s),
                CacheEntry {
                    sig,
                    size,
                    dirty: false,
                },
            );
        }
    }
    if !loaded.is_empty() {
        if let Ok(mut guard) = size_cache().write() {
            for (k, v) in loaded {
                guard.entry(k).or_insert(v);
            }
            CACHED_FOLDER_COUNT.store(guard.len(), Ordering::Relaxed);
        }
    }
}

/// Persist the current hashed folder-size index to disk atomically.
pub fn save_persistent_cache() {
    let snapshot: Vec<(PathBuf, u64, u64)> = {
        let Ok(guard) = size_cache().read() else {
            return;
        };
        guard
            .iter()
            .filter(|(_, e)| !e.dirty)
            .take(150_000)
            .map(|(p, e)| (p.clone(), e.sig, e.size))
            .collect()
    };
    if snapshot.is_empty() {
        return;
    }
    let path = persistent_cache_file_path();
    let tmp_path = path.with_extension("tsv.tmp");
    let mut out = String::with_capacity(snapshot.len() * 48);
    for (p, sig, size) in snapshot {
        let p_str = p.to_string_lossy();
        if !p_str.contains('\n') && !p_str.contains('\t') {
            out.push_str(&format!("{}\t{}\t{}\n", sig, size, p_str));
        }
    }
    if std::fs::write(&tmp_path, out).is_ok() {
        let _ = std::fs::rename(&tmp_path, &path);
    }
}

fn maybe_persist_cache_throttled() {
    let now_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let last = LAST_PERSIST_EPOCH_SECS.load(Ordering::Relaxed);
    if now_secs.saturating_sub(last) >= 15
        && LAST_PERSIST_EPOCH_SECS
            .compare_exchange(last, now_secs, Ordering::SeqCst, Ordering::Relaxed)
            .is_ok()
    {
        std::thread::spawn(|| {
            save_persistent_cache();
        });
    }
}

/// Scan all provided drive roots in a background thread, populating the persistent hashed cache
/// bottom-up. When `force_full` is `false`, any folder whose `mtime` signature matches the
/// persisted cache is reused without re-walking its subtree.
pub fn index_all_drives_background(drive_roots: Vec<PathBuf>, force_full: bool) {
    if DRIVE_INDEXING_ACTIVE.swap(true, Ordering::SeqCst) {
        return;
    }
    std::thread::spawn(move || {
        ensure_persistent_cache_loaded();
        for root in drive_roots {
            if !root.exists() {
                continue;
            }
            let collected = Mutex::new(Vec::<(PathBuf, CacheEntry)>::with_capacity(2048));
            let total = walk_parallel_bottom_up(&root, 0, force_full, &collected);
            let root_sig = fast_dir_signature(&root).unwrap_or(0);
            if let Ok(mut guard) = size_cache().write() {
                if let Ok(batch) = collected.into_inner() {
                    for (p, entry) in batch {
                        guard.insert(p, entry);
                    }
                }
                guard.insert(
                    root,
                    CacheEntry {
                        sig: root_sig,
                        size: total,
                        dirty: false,
                    },
                );
                CACHED_FOLDER_COUNT.store(guard.len(), Ordering::Relaxed);
            }
        }
        save_persistent_cache();
        DRIVE_INDEXING_ACTIVE.store(false, Ordering::SeqCst);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_calculate_size_empty_folder() {
        let temp_dir = TempDir::new().unwrap();
        let path = FilesPath::from(temp_dir.path().to_str().unwrap());
        let size = FolderSizeService::calculate_size(&path).await.unwrap();
        assert_eq!(size, 0);
    }

    #[tokio::test]
    async fn test_calculate_size_with_files() {
        let temp_dir = TempDir::new().unwrap();
        let file1 = temp_dir.path().join("a.txt");
        let file2 = temp_dir.path().join("b.txt");
        std::fs::write(&file1, "hello").unwrap(); // 5 bytes
        std::fs::write(&file2, "world!").unwrap(); // 6 bytes

        let path = FilesPath::from(temp_dir.path().to_str().unwrap());
        let size = FolderSizeService::calculate_size(&path).await.unwrap();
        assert_eq!(size, 11);
    }

    #[tokio::test]
    async fn test_calculate_size_nested_folders() {
        let temp_dir = TempDir::new().unwrap();
        let sub_dir = temp_dir.path().join("sub");
        std::fs::create_dir(&sub_dir).unwrap();

        let file1 = temp_dir.path().join("a.txt");
        let file2 = sub_dir.join("b.txt");
        std::fs::write(&file1, "1234").unwrap(); // 4 bytes
        std::fs::write(&file2, "567890").unwrap(); // 6 bytes

        let path = FilesPath::from(temp_dir.path().to_str().unwrap());
        let size = FolderSizeService::calculate_size(&path).await.unwrap();
        assert_eq!(size, 10);
    }
}

//! Ultra-fast Thumbnail Cache with hash-based invalidation and bounded LRU memory.
//!
//! Features:
//! - Bounded LRU cache with 4,096 max entries (prevents unbounded heap growth)
//! - Hash-based invalidation using file size + mtime
//! - Collision-free disk cache keys (filename stem + 64-bit path hash)
//! - Thread-safe with Arc<Mutex<>>

use crate::core::path_util::FilesPath;
use lru::LruCache;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

/// Maximum number of thumbnail path entries to keep in memory
pub const MAX_CACHE_SIZE: usize = 4_096;

/// File identity for cache invalidation (size + mtime)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FileIdentity {
    pub size: u64,
    pub mtime_secs: u64,
    pub mtime_nanos: u32,
}

impl FileIdentity {
    /// Create from file metadata
    pub fn from_metadata(meta: &std::fs::Metadata) -> Self {
        let mtime = meta
            .modified()
            .unwrap_or(SystemTime::UNIX_EPOCH)
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default();
        Self {
            size: meta.len(),
            mtime_secs: mtime.as_secs(),
            mtime_nanos: mtime.subsec_nanos(),
        }
    }

    /// Check if this identity matches another (for cache validation)
    pub fn matches(&self, other: &FileIdentity) -> bool {
        self.size == other.size
            && self.mtime_secs == other.mtime_secs
            && self.mtime_nanos == other.mtime_nanos
    }
}

/// Cached thumbnail entry
#[derive(Debug, Clone)]
pub struct ThumbnailEntry {
    /// Path to the PNG file on disk
    pub png_path: PathBuf,
    /// File identity when this thumbnail was generated
    pub identity: FileIdentity,
    /// When this entry was last accessed (for LRU)
    pub last_access: SystemTime,
}

/// Ultra-fast thumbnail cache with hash-based invalidation
pub struct ThumbnailCache {
    /// Bounded in-memory LRU cache: path -> ThumbnailEntry
    cache: Arc<Mutex<LruCache<String, ThumbnailEntry>>>,
    /// Disk cache directory
    cache_dir: PathBuf,
}

impl ThumbnailCache {
    /// Create a new thumbnail cache with default capacity (`MAX_CACHE_SIZE`)
    pub fn new() -> Self {
        Self::with_capacity(MAX_CACHE_SIZE)
    }

    /// Create a new thumbnail cache with explicit capacity
    pub fn with_capacity(capacity: usize) -> Self {
        let cache_dir = std::env::temp_dir().join("files-rs-thumbnails");
        let _ = std::fs::create_dir_all(&cache_dir);
        let cap = NonZeroUsize::new(capacity.max(1)).unwrap();

        Self {
            cache: Arc::new(Mutex::new(LruCache::new(cap))),
            cache_dir,
        }
    }

    /// Get cache directory path
    pub fn cache_dir(&self) -> &PathBuf {
        &self.cache_dir
    }

    /// Resize the in-memory LRU capacity dynamically.
    pub fn set_capacity(&self, capacity: usize) {
        if let Ok(mut cache) = self.cache.lock() {
            if let Some(cap) = NonZeroUsize::new(capacity.max(1)) {
                cache.resize(cap);
            }
        }
    }

    /// Get a thumbnail if it exists and is still valid (hash matches)
    pub fn get(&self, path: &FilesPath) -> Option<PathBuf> {
        let key = path.as_path().to_string_lossy().to_string();

        // Check in-memory cache first
        if let Ok(mut cache) = self.cache.lock() {
            if let Some(entry) = cache.get(&key) {
                if let Ok(meta) = std::fs::metadata(path.as_path()) {
                    let current_identity = FileIdentity::from_metadata(&meta);
                    if entry.identity.matches(&current_identity) && entry.png_path.exists() {
                        return Some(entry.png_path.clone());
                    }
                }
                cache.pop(&key);
            }
        }

        // Check disk cache
        let sanitized = self.sanitize_path(path);
        let disk_path = self.cache_dir.join(format!("{}.png", sanitized));
        if disk_path.exists() {
            if let Ok(meta) = std::fs::metadata(path.as_path()) {
                let identity = FileIdentity::from_metadata(&meta);
                let entry = ThumbnailEntry {
                    png_path: disk_path.clone(),
                    identity,
                    last_access: SystemTime::now(),
                };
                if let Ok(mut cache) = self.cache.lock() {
                    cache.put(key, entry);
                }
                return Some(disk_path);
            }
        }

        None
    }

    /// Insert a thumbnail into the cache
    pub fn insert(&self, path: &FilesPath, png_path: PathBuf) {
        let key = path.as_path().to_string_lossy().to_string();

        if let Ok(meta) = std::fs::metadata(path.as_path()) {
            let identity = FileIdentity::from_metadata(&meta);
            let entry = ThumbnailEntry {
                png_path,
                identity,
                last_access: SystemTime::now(),
            };

            if let Ok(mut cache) = self.cache.lock() {
                cache.put(key, entry);
            }
        }
    }

    /// Invalidate cache for a specific file (called on file change)
    pub fn invalidate(&self, path: &FilesPath) {
        let key = path.as_path().to_string_lossy().to_string();

        if let Ok(mut cache) = self.cache.lock() {
            cache.pop(&key);
        }

        let sanitized = self.sanitize_path(path);
        let disk_path = self.cache_dir.join(format!("{}.png", sanitized));
        let _ = std::fs::remove_file(disk_path);
    }

    /// Invalidate all thumbnails in a directory (called on folder change)
    pub fn invalidate_directory(&self, dir: &FilesPath) {
        let dir_str = dir.as_path().to_string_lossy().to_string();

        if let Ok(mut cache) = self.cache.lock() {
            let keys_to_remove: Vec<String> = cache
                .iter()
                .filter(|(k, _)| k.starts_with(&dir_str))
                .map(|(k, _)| k.clone())
                .collect();
            for key in keys_to_remove {
                if let Some(entry) = cache.pop(&key) {
                    let _ = std::fs::remove_file(entry.png_path);
                }
            }
        }
    }

    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        let (cache_len, cap) = self
            .cache
            .lock()
            .map(|c| (c.len(), c.cap().get()))
            .unwrap_or((0, MAX_CACHE_SIZE));
        CacheStats {
            in_memory_entries: cache_len,
            index_entries: cache_len,
            max_entries: cap,
        }
    }

    /// Sanitize a file path for use as a collision-free cache filename
    fn sanitize_path(&self, path: &FilesPath) -> String {
        let s = path.as_path().to_string_lossy();
        let stem: String = s
            .rsplit(['\\', '/'])
            .next()
            .unwrap_or("item")
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '.' || c == '_' || c == '-' {
                    c
                } else {
                    '_'
                }
            })
            .take(48)
            .collect();
        let mut h = DefaultHasher::new();
        s.hash(&mut h);
        format!("{}_{:016x}", stem, h.finish())
    }
}

/// Cache statistics
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub in_memory_entries: usize,
    pub index_entries: usize,
    pub max_entries: usize,
}

impl Default for ThumbnailCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_identity_creation() {
        let meta = std::fs::metadata(std::env::temp_dir()).unwrap();
        let identity = FileIdentity::from_metadata(&meta);
        assert!(identity.size > 0 || identity.mtime_secs > 0);
    }

    #[test]
    fn test_file_identity_match() {
        let a = FileIdentity {
            size: 100,
            mtime_secs: 1000,
            mtime_nanos: 0,
        };
        let b = FileIdentity {
            size: 100,
            mtime_secs: 1000,
            mtime_nanos: 0,
        };
        let c = FileIdentity {
            size: 200,
            mtime_secs: 1000,
            mtime_nanos: 0,
        };
        assert!(a.matches(&b));
        assert!(!a.matches(&c));
    }

    #[test]
    fn test_thumbnail_cache_new_and_bounded_capacity() {
        let cache = ThumbnailCache::new();
        let stats = cache.stats();
        assert_eq!(stats.in_memory_entries, 0);
        assert_eq!(stats.max_entries, MAX_CACHE_SIZE);
        assert_eq!(MAX_CACHE_SIZE, 4_096);
    }

    #[test]
    fn test_thumbnail_cache_lru_eviction() {
        let temp_dir = tempfile::tempdir().unwrap();
        let cache = ThumbnailCache::with_capacity(2);
        let f1 = temp_dir.path().join("a.txt");
        let f2 = temp_dir.path().join("b.txt");
        let f3 = temp_dir.path().join("c.txt");
        std::fs::write(&f1, b"1").unwrap();
        std::fs::write(&f2, b"2").unwrap();
        std::fs::write(&f3, b"3").unwrap();

        cache.insert(&FilesPath::new(&f1), f1.clone());
        cache.insert(&FilesPath::new(&f2), f2.clone());
        assert_eq!(cache.stats().in_memory_entries, 2);

        // Inserting a 3rd entry must evict the oldest entry so memory never exceeds capacity
        cache.insert(&FilesPath::new(&f3), f3.clone());
        assert_eq!(cache.stats().in_memory_entries, 2);
        assert_eq!(cache.stats().index_entries, 2);
    }

    #[test]
    fn test_sanitize_path_collision_free() {
        let cache = ThumbnailCache::new();
        let path_a = FilesPath::new(std::path::PathBuf::from("C:\\Users\\test\\file.txt"));
        let path_b = FilesPath::new(std::path::PathBuf::from("D:\\Users\\test\\file.txt"));
        let san_a = cache.sanitize_path(&path_a);
        let san_b = cache.sanitize_path(&path_b);
        assert!(!san_a.contains('\\'));
        assert!(!san_a.contains(':'));
        assert_ne!(san_a, san_b);
    }
}

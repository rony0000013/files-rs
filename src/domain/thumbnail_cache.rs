//! Ultra-fast Thumbnail Cache with hash-based invalidation.
//!
//! Features:
//! - LRU cache with 1M max entries
//! - Hash-based invalidation using file size + mtime
//! - Disk cache for persistence across sessions
//! - Thread-safe with Arc<Mutex<>>
//! - Dynamic loading for any folder, tab thumbnail, etc.

use crate::core::path_util::FilesPath;
use lru::LruCache;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

/// Maximum number of thumbnails to keep in memory
const MAX_CACHE_SIZE: usize = 1_000_000;

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
        self.size == other.size && self.mtime_secs == other.mtime_secs && self.mtime_nanos == other.mtime_nanos
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
    /// In-memory LRU cache: path -> ThumbnailEntry
    cache: Arc<Mutex<LruCache<String, ThumbnailEntry>>>,
    /// Disk cache directory
    cache_dir: PathBuf,
    /// Index of all cached entries for fast lookup
    index: Arc<Mutex<HashMap<String, ThumbnailEntry>>>,
}

impl ThumbnailCache {
    /// Create a new thumbnail cache
    pub fn new() -> Self {
        let cache_dir = std::env::temp_dir().join("files-rs-thumbnails");
        let _ = std::fs::create_dir_all(&cache_dir);

        Self {
            cache: Arc::new(Mutex::new(
                LruCache::new(std::num::NonZeroUsize::new(MAX_CACHE_SIZE).unwrap()),
            )),
            cache_dir,
            index: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Get cache directory path
    pub fn cache_dir(&self) -> &PathBuf {
        &self.cache_dir
    }

    /// Get a thumbnail if it exists and is still valid (hash matches)
    pub fn get(&self, path: &FilesPath) -> Option<PathBuf> {
        let key = path.as_path().to_string_lossy().to_string();

        // Check in-memory cache first
        if let Ok(mut cache) = self.cache.lock() {
            if let Some(entry) = cache.get(&key) {
                // Validate the cached thumbnail is still current
                if let Ok(meta) = std::fs::metadata(path.as_path()) {
                    let current_identity = FileIdentity::from_metadata(&meta);
                    if entry.identity.matches(&current_identity) && entry.png_path.exists() {
                        return Some(entry.png_path.clone());
                    }
                }
                // Stale entry — remove it
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
                // Add to in-memory cache
                if let Ok(mut cache) = self.cache.lock() {
                    cache.put(key.clone(), entry.clone());
                }
                if let Ok(mut index) = self.index.lock() {
                    index.insert(key, entry);
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
                png_path: png_path.clone(),
                identity,
                last_access: SystemTime::now(),
            };

            // Add to in-memory cache
            if let Ok(mut cache) = self.cache.lock() {
                cache.put(key.clone(), entry.clone());
            }
            if let Ok(mut index) = self.index.lock() {
                index.insert(key, entry);
            }
        }
    }

    /// Invalidate cache for a specific file (called on file change)
    pub fn invalidate(&self, path: &FilesPath) {
        let key = path.as_path().to_string_lossy().to_string();

        // Remove from in-memory cache
        if let Ok(mut cache) = self.cache.lock() {
            cache.pop(&key);
        }
        if let Ok(mut index) = self.index.lock() {
            index.remove(&key);
        }

        // Remove from disk cache
        let sanitized = self.sanitize_path(path);
        let disk_path = self.cache_dir.join(format!("{}.png", sanitized));
        let _ = std::fs::remove_file(disk_path);
    }

    /// Invalidate all thumbnails in a directory (called on folder change)
    pub fn invalidate_directory(&self, dir: &FilesPath) {
        let dir_str = dir.as_path().to_string_lossy().to_string();

        // Remove from in-memory cache
        if let Ok(mut cache) = self.cache.lock() {
            let keys_to_remove: Vec<String> = cache
                .iter()
                .filter(|(k, _)| k.starts_with(&dir_str))
                .map(|(k, _)| k.clone())
                .collect();
            for key in keys_to_remove {
                cache.pop(&key);
            }
        }

        // Remove from index
        if let Ok(mut index) = self.index.lock() {
            let keys_to_remove: Vec<String> = index
                .keys()
                .filter(|k| k.starts_with(&dir_str))
                .cloned()
                .collect();
            for key in keys_to_remove {
                if let Some(entry) = index.remove(&key) {
                    let _ = std::fs::remove_file(entry.png_path);
                }
            }
        }
    }

    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        let cache_len = self.cache.lock().map(|c| c.len()).unwrap_or(0);
        let index_len = self.index.lock().map(|i| i.len()).unwrap_or(0);
        CacheStats {
            in_memory_entries: cache_len,
            index_entries: index_len,
            max_entries: MAX_CACHE_SIZE,
        }
    }

    /// Sanitize a file path for use as a cache filename
    fn sanitize_path(&self, path: &FilesPath) -> String {
        path.as_path()
            .to_string_lossy()
            .replace(|c: char| !c.is_alphanumeric() && c != '.' && c != '_', "_")
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
        let a = FileIdentity { size: 100, mtime_secs: 1000, mtime_nanos: 0 };
        let b = FileIdentity { size: 100, mtime_secs: 1000, mtime_nanos: 0 };
        let c = FileIdentity { size: 200, mtime_secs: 1000, mtime_nanos: 0 };
        assert!(a.matches(&b));
        assert!(!a.matches(&c));
    }

    #[test]
    fn test_thumbnail_cache_new() {
        let cache = ThumbnailCache::new();
        let stats = cache.stats();
        assert_eq!(stats.in_memory_entries, 0);
        assert_eq!(stats.max_entries, MAX_CACHE_SIZE);
    }

    #[test]
    fn test_sanitize_path() {
        let cache = ThumbnailCache::new();
        let path = FilesPath::new(std::path::PathBuf::from("C:\\Users\\test\\file.txt"));
        let sanitized = cache.sanitize_path(&path);
        assert!(!sanitized.contains('\\'));
        assert!(!sanitized.contains(':'));
    }
}

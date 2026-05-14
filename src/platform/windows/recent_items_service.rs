use crate::core::path_util::FilesPath;
use anyhow::{anyhow, Result};
use std::collections::VecDeque;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

/// Maximum number of recent items to track
const MAX_RECENT_ITEMS: usize = 100;

/// Windows Recent Items Service
/// Reads and manages Windows Recent Items (shortcuts in %APPDATA%\Microsoft\Windows\Recent)
pub struct RecentItemsService {
    recent_items: std::sync::Mutex<VecDeque<RecentItem>>,
    max_items: usize,
}

#[derive(Debug, Clone)]
pub struct RecentItem {
    pub path: FilesPath,
    pub accessed_at: SystemTime,
    pub is_file: bool,
}

impl RecentItemsService {
    pub fn new() -> Self {
        Self {
            recent_items: std::sync::Mutex::new(VecDeque::with_capacity(MAX_RECENT_ITEMS)),
            max_items: MAX_RECENT_ITEMS,
        }
    }

    /// Refresh recent items from the Windows Recent folder
    pub async fn refresh_recent(&self) -> Result<Vec<RecentItem>> {
        let recent_path = if let Some(appdata) = std::env::var_os("APPDATA") {
            PathBuf::from(appdata).join("Microsoft").join("Windows").join("Recent")
        } else {
            return Err(anyhow!("Could not find Recent folder"));
        };

        if !recent_path.exists() {
            return Ok(Vec::new());
        }

        let mut items = VecDeque::new();

        let entries = fs::read_dir(&recent_path)
            .map_err(|e| anyhow!("Failed to read Recent folder: {}", e))?;

        for entry in entries.flatten() {
            let path = entry.path();

            // Skip directories and non-.lnk files
            if !path.is_file() {
                continue;
            }
            if path.extension().map_or(true, |e| e != "lnk") {
                continue;
            }

            // Get file metadata for access time
            if let Ok(metadata) = entry.metadata() {
                let accessed_at = metadata
                    .accessed()
                    .unwrap_or(SystemTime::UNIX_EPOCH);

                // Skip items older than 30 days
                let thirty_days = Duration::from_secs(30 * 24 * 60 * 60);
                if let Ok(duration_since) = accessed_at.duration_since(SystemTime::UNIX_EPOCH) {
                    if duration_since.as_secs() > thirty_days.as_secs() {
                        continue;
                    }
                }

                // Try to resolve the shortcut target
                let target = self.resolve_shortcut(&path);

                items.push_back(RecentItem {
                    path: target.unwrap_or_else(|| FilesPath::new(&path)),
                    accessed_at,
                    is_file: path
                        .as_path()
                        .file_name()
                        .and_then(|n| n.to_str())
                        .map(|n| !n.ends_with(".folder.lnk"))
                        .unwrap_or(true),
                });

                // Limit to max items
                if items.len() > MAX_RECENT_ITEMS {
                    items.pop_front();
                }
            }
        }

        // Sort by access time (most recent first)
        let mut sorted: Vec<_> = items.into_iter().collect();
        sorted.sort_by(|a, b| b.accessed_at.cmp(&a.accessed_at));

        // Update internal cache
        {
            let mut cache = self.recent_items.lock().unwrap();
            cache.clear();
            cache.extend(sorted.clone());
        }

        Ok(sorted)
    }

    /// Resolve a .lnk shortcut to its target path
    fn resolve_shortcut(&self, lnk_path: &PathBuf) -> Option<FilesPath> {
        // For a proper implementation, we would use IShellLink COM
        // to resolve the shortcut. For now, we'll parse it as best we can.
        use std::io::Read;

        // Read the .lnk file - it has a specific format
        // The target path is stored at a specific offset
        if let Ok(mut file) = fs::File::open(lnk_path) {
            let mut buffer = vec![0u8; 1024];
            if let Ok(_) = file.read(&mut buffer) {
                // Look for the target path in the .lnk file
                // This is a simplified approach - a full implementation
                // would properly parse the shell link format
                if let Some(target) = self.find_target_in_lnk(&buffer) {
                    return Some(FilesPath::new(&target));
                }
            }
        }

        // Fallback: return the .lnk path itself
        Some(FilesPath::new(lnk_path))
    }

    /// Try to find the target path in a .lnk file buffer
    fn find_target_in_lnk(&self, buffer: &[u8]) -> Option<String> {
        // The .lnk format stores paths as wide strings (UTF-16LE)
        // Search for drive letters (e.g., C:\) which indicate a path
        let wide_chars: Vec<u16> = buffer
            .chunks(2)
            .filter_map(|chunk| {
                if chunk.len() == 2 {
                    Some(u16::from_le_bytes([chunk[0], chunk[1]]))
                } else {
                    None
                }
            })
            .collect();

        // Look for patterns like "C:\" or "\\" (UNC paths)
        for i in 0..wide_chars.len() {
            // Check for drive letter pattern (e.g., "C:\")
            if i + 2 < wide_chars.len() {
                let c = wide_chars[i] as u8;
                if (c >= b'A' && c <= b'Z') || (c >= b'a' && c <= b'z') {
                    if wide_chars[i + 1] == b':' as u16 && wide_chars[i + 2] == b'\\' as u16 {
                        // Found a drive path - extract until null terminator
                        let end = wide_chars[i..]
                            .iter()
                            .position(|&c| c == 0)
                            .unwrap_or(100);
                        let path_chars = &wide_chars[i..i + end];
                        if let Ok(path) = String::from_utf16(path_chars) {
                            if !path.is_empty() && path.contains('\\') {
                                return Some(path);
                            }
                        }
                    }
                }
            }
        }

        None
    }

    /// Get cached recent items (without refreshing)
    pub fn get_cached_items(&self) -> Vec<RecentItem> {
        let cache = self.recent_items.lock().unwrap();
        cache.iter().cloned().collect()
    }

    /// Search recent items by name pattern
    pub fn search_recent(&self, pattern: &str) -> Vec<RecentItem> {
        let cache = self.recent_items.lock().unwrap();
        cache
            .iter()
            .filter(|item| {
                item.path
                    .as_path()
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.to_lowercase().contains(&pattern.to_lowercase()))
                    .unwrap_or(false)
            })
            .cloned()
            .collect()
    }

    /// Add an item to recent (called when opening files)
    pub fn add_recent(&self, path: FilesPath, is_file: bool) {
        let mut cache = self.recent_items.lock().unwrap();

        // Remove if already exists
        cache.retain(|item| &item.path != &path);

        // Add to front
        cache.push_front(RecentItem {
            path,
            accessed_at: SystemTime::now(),
            is_file,
        });

        // Trim to max size
        while cache.len() > self.max_items {
            cache.pop_back();
        }
    }

    /// Clear all recent items
    pub async fn clear_recent(&self) -> Result<()> {
        let recent_path = if let Some(appdata) = std::env::var_os("APPDATA") {
            PathBuf::from(appdata).join("Microsoft").join("Windows").join("Recent")
        } else {
            return Err(anyhow!("Could not find Recent folder"));
        };

        // Delete all .lnk files in the Recent folder
        if recent_path.exists() {
            if let Ok(entries) = fs::read_dir(&recent_path) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().map_or(false, |e| e == "lnk") {
                        let _ = fs::remove_file(&path);
                    }
                }
            }
        }

        // Clear cache
        let mut cache = self.recent_items.lock().unwrap();
        cache.clear();

        Ok(())
    }
}

impl Default for RecentItemsService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn test_service_creation() {
        let service = RecentItemsService::new();
        assert!(service.get_cached_items().is_empty());
    }

    #[test]
    fn test_add_recent() {
        let service = RecentItemsService::new();
        service.add_recent(FilesPath::new("/tmp/test"), true);
        assert_eq!(service.get_cached_items().len(), 1);
    }
}
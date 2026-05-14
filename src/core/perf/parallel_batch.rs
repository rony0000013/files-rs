//! Parallel metadata batch loading using rayon.
//! For large directories, metadata operations are parallelized
//! across available CPU cores.

use crate::core::path_util::FilesPath;
use crate::storage::storage::{FileMetadata, FileType, StorableItem};
use rayon::prelude::*;
use std::path::Path;

/// Load metadata for many paths in parallel using rayon.
/// Returns items in the same order as input paths.
/// Skips paths that error (logs warning).
pub fn batch_load_metadata(paths: &[FilesPath]) -> Vec<(FilesPath, Option<FileMetadata>)> {
    paths
        .par_iter()
        .map(|path| {
            let meta = load_single_metadata(path);
            (path.clone(), meta)
        })
        .collect()
}

/// Load metadata for many path-like strings in parallel.
pub fn batch_load_metadata_str<P: AsRef<Path> + Send + Sync>(
    paths: &[P],
) -> Vec<(String, Option<FileMetadata>)> {
    paths
        .par_iter()
        .map(|path| {
            let path_str = path.as_ref().to_string_lossy().into_owned();
            let meta = load_single_metadata(&FilesPath::new(path.as_ref()));
            (path_str, meta)
        })
        .collect()
}

/// Load a single file's metadata.
fn load_single_metadata(path: &FilesPath) -> Option<FileMetadata> {
    match std::fs::metadata(path.as_path()) {
        Ok(meta) => {
            let file_type = if meta.is_dir() {
                FileType::Directory
            } else if meta.file_type().is_symlink() {
                FileType::Symlink
            } else {
                FileType::File
            };
            Some(FileMetadata {
                size: meta.len(),
                created: meta.created().ok(),
                modified: meta.modified().ok(),
                file_type,
                is_hidden: false,
                is_system: false,
            })
        }
        Err(e) => {
            tracing::warn!("Failed to load metadata for {:?}: {}", path, e);
            None
        }
    }
}

/// Batch load items with metadata, ready for display.
/// This is the primary API for parallel directory loading.
pub fn batch_load_items(paths: &[FilesPath]) -> Vec<StorableItem> {
    let results = batch_load_metadata(paths);
    results
        .into_iter()
        .filter_map(|(path, meta)| meta.map(|m| StorableItem { path, metadata: m }))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_batch_load_empty() {
        let result = batch_load_metadata(&[]);
        assert!(result.is_empty());
    }

    #[test]
    fn test_batch_load_single_file() {
        let tmp = TempDir::new().unwrap();
        let file_path = tmp.path().join("test.txt");
        std::fs::write(&file_path, "hello").unwrap();

        let paths = vec![FilesPath::new(file_path)];
        let results = batch_load_metadata(&paths);
        assert_eq!(results.len(), 1);
        assert!(results[0].1.is_some());
    }

    #[test]
    fn test_batch_load_missing_file() {
        let paths = vec![FilesPath::new("/nonexistent/path/file.txt")];
        let results = batch_load_metadata(&paths);
        assert_eq!(results.len(), 1);
        assert!(results[0].1.is_none());
    }

    #[test]
    fn test_batch_load_multiple_files() {
        let tmp = TempDir::new().unwrap();
        let mut paths = Vec::new();
        for i in 0..10 {
            let f = tmp.path().join(format!("file_{}.txt", i));
            std::fs::write(&f, format!("content {}", i)).unwrap();
            paths.push(FilesPath::new(f));
        }
        let results = batch_load_metadata(&paths);
        assert_eq!(results.len(), 10);
        assert!(results.iter().all(|(_, m)| m.is_some()));
    }

    #[test]
    fn test_batch_load_items() {
        let tmp = TempDir::new().unwrap();
        let f = tmp.path().join("a.txt");
        std::fs::write(&f, "a").unwrap();
        let items = batch_load_items(&[FilesPath::new(f)]);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].metadata.file_type, FileType::File);
    }

    #[test]
    fn test_batch_load_directory() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir(tmp.path().join("subdir")).unwrap();
        let paths = vec![FilesPath::new(tmp.path().join("subdir"))];
        let results = batch_load_metadata(&paths);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].1.unwrap().file_type, FileType::Directory);
    }

    #[test]
    fn test_rayon_par_bridge_works() {
        let v: Vec<i32> = (0..1000).collect();
        let sum: i32 = v.par_iter().sum();
        assert_eq!(sum, 499500);
    }
}

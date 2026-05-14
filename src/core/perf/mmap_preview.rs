//! Memory-mapped file reading for large preview files.
//! Uses `memmap2` crate for zero-copy file access.

use memmap2::Mmap;
use std::fs::File;
use std::io::Read;
use std::path::Path;

/// A memory-mapped file reader optimized for previewing large files.
pub struct MmapPreviewReader {
    mmap: Mmap,
    path: String,
}

impl MmapPreviewReader {
    /// Memory-map a file for reading. Returns None if the file is too small
    /// (< 1KB) or if mmapping fails.
    pub fn open(path: &Path) -> Result<Self, String> {
        let path_str = path.to_string_lossy().into_owned();

        // Check file size — don't mmap tiny files
        let meta = std::fs::metadata(path).map_err(|e| format!("Failed to stat: {}", e))?;
        if meta.len() < 1024 {
            return Err(format!("File too small for mmap: {} bytes", meta.len()));
        }

        let file = File::open(path).map_err(|e| format!("Failed to open: {}", e))?;
        let mmap = unsafe { Mmap::map(&file).map_err(|e| format!("Failed to mmap: {}", e))? };

        Ok(Self {
            mmap,
            path: path_str,
        })
    }

    /// Read a slice of the file at the given offset.
    pub fn read_at(&self, offset: usize, length: usize) -> &[u8] {
        let end = (offset + length).min(self.mmap.len());
        &self.mmap[offset..end]
    }

    /// Get the entire mmap'd content.
    pub fn as_slice(&self) -> &[u8] {
        &self.mmap
    }

    /// Get the file size.
    pub fn len(&self) -> usize {
        self.mmap.len()
    }

    /// Returns true if the mmap is empty.
    pub fn is_empty(&self) -> bool {
        self.mmap.is_empty()
    }

    /// Get the path.
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Check if this reader is suitable for a file of the given size.
    /// Mmap is beneficial for files > 1MB.
    pub fn is_suitable_for(file_size: u64) -> bool {
        file_size >= 1024 * 1024 // 1 MB
    }
}

/// Read a file for preview, choosing between standard IO and mmap
/// based on file size.
pub fn read_for_preview(path: &Path, max_size: usize) -> Result<Vec<u8>, String> {
    let meta = std::fs::metadata(path).map_err(|e| format!("Failed to stat: {}", e))?;

    if MmapPreviewReader::is_suitable_for(meta.len()) {
        // Use mmap for large files
        let reader = MmapPreviewReader::open(path)?;
        let len = reader.len().min(max_size);
        Ok(reader.read_at(0, len).to_vec())
    } else {
        // Use standard read for smaller files
        let mut file = File::open(path).map_err(|e| format!("Failed to open: {}", e))?;
        let buf_size = meta.len().min(max_size as u64) as usize;
        let mut buf = vec![0u8; buf_size];
        file.read_exact(&mut buf)
            .map_err(|e| format!("Failed to read: {}", e))?;
        Ok(buf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_mmap_small_file_returns_err() {
        let tmp = TempDir::new().unwrap();
        let f = tmp.path().join("small.txt");
        std::fs::write(&f, "hello").unwrap();
        let result = MmapPreviewReader::open(&f);
        assert!(result.is_err());
    }

    #[test]
    fn test_mmap_large_file_succeeds() {
        let tmp = TempDir::new().unwrap();
        let f = tmp.path().join("large.txt");
        let content = vec![0xABu8; 4096]; // 4KB
        std::fs::write(&f, &content).unwrap();
        let reader = MmapPreviewReader::open(&f).unwrap();
        assert_eq!(reader.len(), 4096);
    }

    #[test]
    fn test_mmap_read_at() {
        let tmp = TempDir::new().unwrap();
        let f = tmp.path().join("data.bin");
        let content = (0..2000u8).collect::<Vec<_>>();
        std::fs::write(&f, &content).unwrap();
        let reader = MmapPreviewReader::open(&f).unwrap();
        let slice = reader.read_at(10, 5);
        assert_eq!(slice, &[10, 11, 12, 13, 14]);
    }

    #[test]
    fn test_mmap_read_at_bounds() {
        let tmp = TempDir::new().unwrap();
        let f = tmp.path().join("bounds.bin");
        let content = vec![0xFFu8; 1500];
        std::fs::write(&f, &content).unwrap();
        let reader = MmapPreviewReader::open(&f).unwrap();
        let slice = reader.read_at(1495, 100); // past end
        assert_eq!(slice.len(), 5); // clamped to available
    }

    #[test]
    fn test_mmap_as_slice() {
        let tmp = TempDir::new().unwrap();
        let f = tmp.path().join("slice.bin");
        let content = vec![0x42u8; 2048];
        std::fs::write(&f, &content).unwrap();
        let reader = MmapPreviewReader::open(&f).unwrap();
        assert_eq!(reader.as_slice().len(), 2048);
        assert_eq!(reader.as_slice()[0], 0x42);
    }

    #[test]
    fn test_mmap_is_empty() {
        let tmp = TempDir::new().unwrap();
        let f = tmp.path().join("large_empty.txt");
        // 1024 bytes is the minimum for mmap, but empty won't work
        // This test is just for the is_empty() method on an opened reader
        let content = vec![0u8; 2048];
        std::fs::write(&f, &content).unwrap();
        let reader = MmapPreviewReader::open(&f).unwrap();
        assert!(!reader.is_empty());
    }

    #[test]
    fn test_is_suitable_for() {
        assert!(!MmapPreviewReader::is_suitable_for(1024)); // 1KB
        assert!(MmapPreviewReader::is_suitable_for(1024 * 1024)); // 1MB
        assert!(MmapPreviewReader::is_suitable_for(10 * 1024 * 1024)); // 10MB
    }

    #[test]
    fn test_read_for_preview_small_file() {
        let tmp = TempDir::new().unwrap();
        let f = tmp.path().join("small.txt");
        std::fs::write(&f, "hello world").unwrap();
        let data = read_for_preview(&f, 1024).unwrap();
        assert_eq!(String::from_utf8_lossy(&data), "hello world");
    }

    #[test]
    fn test_read_for_preview_large_file() {
        let tmp = TempDir::new().unwrap();
        let f = tmp.path().join("large_preview.bin");
        let content = vec![0xCDu8; 2 * 1024 * 1024]; // 2MB
        std::fs::write(&f, &content).unwrap();
        let data = read_for_preview(&f, 4096).unwrap();
        assert_eq!(data.len(), 4096);
    }

    #[test]
    fn test_mmap_path() {
        let tmp = TempDir::new().unwrap();
        let f = tmp.path().join("path_test.bin");
        let content = vec![0u8; 2048];
        std::fs::write(&f, &content).unwrap();
        let reader = MmapPreviewReader::open(&f).unwrap();
        assert!(reader.path().contains("path_test.bin"));
    }

    #[test]
    fn test_read_for_preview_nonexistent() {
        let result = read_for_preview(Path::new("/nonexistent/file.bin"), 1024);
        assert!(result.is_err());
    }
}

use crate::core::path_util::FilesPath;
use anyhow::{anyhow, Result};
use std::fs;
use std::io;

pub struct AdsService;

impl AdsService {
    /// Reads content from a specific NTFS Alternate Data Stream
    pub async fn read_stream(path: &FilesPath, stream_name: &str) -> Result<Vec<u8>> {
        let stream_path = format!("{}:{}", path.as_path().to_string_lossy(), stream_name);
        let data = fs::read(stream_path)?;
        Ok(data)
    }

    /// Writes content to a specific NTFS Alternate Data Stream
    pub async fn write_stream(path: &FilesPath, stream_name: &str, data: &[u8]) -> Result<()> {
        let stream_path = format!("{}:{}", path.as_path().to_string_lossy(), stream_name);
        fs::write(stream_path, data)?;
        Ok(())
    }

    /// Lists all streams for a file (requires Win32 API, scaffolded here)
    pub async fn list_streams(path: &FilesPath) -> Result<Vec<String>> {
        // In a full implementation, this would use `FindFirstStreamW` / `FindNextStreamW`
        // For now, we return a stub or use a known common stream if applicable
        Err(anyhow!(
            "Listing NTFS streams requires Win32 FindFirstStreamW implementation"
        ))
    }

    /// Checks if ADS is supported on the current filesystem (NTFS only)
    pub fn is_supported(path: &FilesPath) -> bool {
        // ADS is only supported on NTFS filesystems
        // This is a best-effort check - we try to write and immediately clean up
        let test_stream = format!("{}:hermes_test_stream", path.as_path().to_string_lossy());
        match fs::write(&test_stream, b"test") {
            Ok(_) => {
                let _ = fs::remove_file(&test_stream);
                true
            }
            Err(_) => false,
        }
    }

    /// Gets the stream path format string (for reference)
    pub fn stream_path(path: &FilesPath, stream_name: &str) -> String {
        format!("{}:{}", path.as_path().to_string_lossy(), stream_name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    #[cfg(windows)] // ADS only works on Windows NTFS
    async fn test_write_and_read_stream() -> Result<()> {
        let dir = tempdir()?;
        let file_path = dir.path().join("test.txt");
        std::fs::write(&file_path, "Main content")?;

        let files_path = FilesPath::new(&file_path);
        
        // Write to ADS
        let stream_data = b"Stream content";
        AdsService::write_stream(&files_path, "test_stream", stream_data).await?;

        // Read from ADS
        let read_data = AdsService::read_stream(&files_path, "test_stream").await?;
        assert_eq!(read_data, stream_data);

        Ok(())
    }

    #[tokio::test]
    #[cfg(windows)] // ADS only works on Windows NTFS
    async fn test_read_nonexistent_stream() -> Result<()> {
        let dir = tempdir()?;
        let file_path = dir.path().join("test.txt");
        std::fs::write(&file_path, "Main content")?;

        let files_path = FilesPath::new(&file_path);
        
        // Try to read non-existent stream
        let result = AdsService::read_stream(&files_path, "nonexistent").await;
        assert!(result.is_err());

        Ok(())
    }

    #[test]
    fn test_stream_path_format() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        let files_path = FilesPath::new(&file_path);
        
        let stream_path = AdsService::stream_path(&files_path, "mystream");
        
        // Path should be in format "filename:streamname"
        assert!(stream_path.contains(":mystream"));
    }

    #[test]
    fn test_is_supported_on_ntfs() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        std::fs::write(&file_path, "test").unwrap();
        
        let files_path = FilesPath::new(&file_path);
        
        // On Linux/WSL (typically not NTFS), this should return false
        // On real Windows NTFS, this would return true
        let supported = AdsService::is_supported(&files_path);
        
        // Log the result for debugging
        println!("ADS supported on {:?}: {}", dir.path(), supported);
        
        // This test just verifies the function doesn't panic
        // Actual result depends on filesystem type
    }

    #[tokio::test]
    async fn test_list_streams_returns_error() -> Result<()> {
        let dir = tempdir()?;
        let file_path = dir.path().join("test.txt");
        std::fs::write(&file_path, "content")?;

        let files_path = FilesPath::new(&file_path);
        
        // list_streams should return an error (not implemented yet)
        let result = AdsService::list_streams(&files_path).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("FindFirstStreamW"));
        
        Ok(())
    }

    #[tokio::test]
    #[cfg(windows)] // ADS only works on Windows NTFS
    async fn test_write_multiple_streams() -> Result<()> {
        let dir = tempdir()?;
        let file_path = dir.path().join("test.txt");
        std::fs::write(&file_path, "Main content")?;

        let files_path = FilesPath::new(&file_path);
        
        // Write multiple streams
        AdsService::write_stream(&files_path, "stream1", b"Content 1").await?;
        AdsService::write_stream(&files_path, "stream2", b"Content 2").await?;
        
        // Verify both streams can be read independently
        let data1 = AdsService::read_stream(&files_path, "stream1").await?;
        let data2 = AdsService::read_stream(&files_path, "stream2").await?;
        
        assert_eq!(data1, b"Content 1");
        assert_eq!(data2, b"Content 2");
        
        // Main file content should be unchanged
        let main_content = std::fs::read(&file_path)?;
        assert_eq!(main_content, b"Main content");

        Ok(())
    }
}

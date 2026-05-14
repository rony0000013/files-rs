use crate::core::path_util::FilesPath;
use anyhow::Result;
use std::io;
use tokio::fs;

pub struct FolderSizeService;

impl FolderSizeService {
    /// Recursively calculates the total size of a folder asynchronously.
    pub async fn calculate_size(path: &FilesPath) -> Result<u64> {
        let mut total_size = 0;
        let mut entries = fs::read_dir(path.as_path()).await?;

        while let Some(entry) = entries.next_entry().await? {
            let meta = entry.metadata().await?;
            if meta.is_dir() {
                total_size += Box::pin(Self::calculate_size(&FilesPath::new(entry.path()))).await?;
            } else {
                total_size += meta.len();
            }
        }
        Ok(total_size)
    }
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
    async fn test_calculate_size_single_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        let content = "Hello, World!";
        tokio::fs::write(&file_path, content).await.unwrap();
        
        let path = FilesPath::from(temp_dir.path().to_str().unwrap());
        let size = FolderSizeService::calculate_size(&path).await.unwrap();
        
        assert_eq!(size, content.len() as u64);
    }

    #[tokio::test]
    async fn test_calculate_size_multiple_files() {
        let temp_dir = TempDir::new().unwrap();
        
        // Create multiple files with known sizes
        tokio::fs::write(temp_dir.path().join("file1.txt"), "Hello").await.unwrap();
        tokio::fs::write(temp_dir.path().join("file2.txt"), "World").await.unwrap();
        tokio::fs::write(temp_dir.path().join("file3.txt"), "!").await.unwrap();
        
        let path = FilesPath::from(temp_dir.path().to_str().unwrap());
        let size = FolderSizeService::calculate_size(&path).await.unwrap();
        
        // "Hello" + "World" + "!" = 5 + 5 + 1 = 11 bytes
        assert_eq!(size, 11);
    }

    #[tokio::test]
    async fn test_calculate_size_nested_folders() {
        let temp_dir = TempDir::new().unwrap();
        
        // Create nested directory structure
        let sub_dir = temp_dir.path().join("subdir");
        tokio::fs::create_dir(&sub_dir).await.unwrap();
        
        tokio::fs::write(temp_dir.path().join("root.txt"), "ROOT").await.unwrap();
        tokio::fs::write(sub_dir.join("sub.txt"), "SUB").await.unwrap();
        
        let path = FilesPath::from(temp_dir.path().to_str().unwrap());
        let size = FolderSizeService::calculate_size(&path).await.unwrap();
        
        // "ROOT" (4) + "SUB" (3) = 7 bytes
        assert_eq!(size, 7);
    }

    #[tokio::test]
    async fn test_calculate_size_nonexistent_folder() {
        let temp_dir = TempDir::new().unwrap();
        let nonexistent = temp_dir.path().join("nonexistent_folder");
        
        let path = FilesPath::from(nonexistent.to_str().unwrap());
        let result = FolderSizeService::calculate_size(&path).await;
        
        // Should return an error
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_calculate_size_preserves_temp_dir_on_error() {
        let temp_dir = TempDir::new().unwrap();
        let path = FilesPath::from(temp_dir.path().to_str().unwrap());
        
        // Should succeed and not delete the temp dir
        let size = FolderSizeService::calculate_size(&path).await.unwrap();
        assert_eq!(size, 0);
        
        // Temp dir should still exist after calculation
        assert!(temp_dir.path().exists());
    }

    #[tokio::test]
    async fn test_calculate_size_deeply_nested() {
        let temp_dir = TempDir::new().unwrap();
        
        // Create a deeply nested structure
        let level1 = temp_dir.path().join("level1");
        let level2 = level1.join("level2");
        let level3 = level2.join("level3");
        
        tokio::fs::create_dir_all(&level3).await.unwrap();
        tokio::fs::write(level3.join("deep.txt"), "DEEP").await.unwrap();
        
        let path = FilesPath::from(temp_dir.path().to_str().unwrap());
        let size = FolderSizeService::calculate_size(&path).await.unwrap();
        
        assert_eq!(size, 4); // "DEEP" = 4 bytes
    }
}

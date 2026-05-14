use crate::core::path_util::FilesPath;
use anyhow::{anyhow, Result};

pub struct SecurityService;

impl SecurityService {
    pub async fn get_permissions(path: &FilesPath) -> Result<Vec<u8>> {
        tracing::info!("Fetching security descriptor for {:?}", path.as_path());
        Ok(Vec::new())
    }

    pub async fn set_permissions(path: &FilesPath, _descriptor: &[u8]) -> Result<()> {
        tracing::info!("Setting security descriptor for {:?}", path.as_path());
        Ok(())
    }

    pub async fn can_write(path: &FilesPath) -> bool {
        match std::fs::metadata(path.as_path()) {
            Ok(meta) => meta.permissions().readonly() == false,
            Err(_) => false,
        }
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_permissions_returns_empty() {
        let path = FilesPath::new("/tmp/test");
        let result = SecurityService::get_permissions(&path).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_set_permissions_succeeds() {
        let path = FilesPath::new("/tmp/test");
        let descriptor = vec![0u8; 10];
        let result = SecurityService::set_permissions(&path, &descriptor).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_can_write_nonexistent_path() {
        let path = FilesPath::new("/tmp/nonexistent_file_path_xyz123.txt");
        let can_write = SecurityService::can_write(&path).await;
        assert!(!can_write); // File doesn't exist, so can't write
    }

    #[tokio::test]
    async fn test_can_write_readonly_file() {
        use std::fs;
        use std::io;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("readonly.txt");

        fs::write(&file_path, "test content").unwrap();
        let metadata = fs::metadata(&file_path).unwrap();
        let mut permissions = metadata.permissions();
        permissions.set_readonly(true);
        fs::set_permissions(&file_path, permissions).unwrap();

        let path = FilesPath::new(file_path.to_str().unwrap());
        let can_write = SecurityService::can_write(&path).await;

        // Clean up - make writable again
        let metadata = fs::metadata(&file_path).unwrap();
        let mut permissions = metadata.permissions();
        permissions.set_readonly(false);
        let _ = fs::set_permissions(&file_path, permissions);

        assert!(!can_write);
    }

    #[tokio::test]
    async fn test_can_write_writable_file() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("writable.txt");

        fs::write(&file_path, "test content").unwrap();

        let path = FilesPath::new(file_path.to_str().unwrap());
        let can_write = SecurityService::can_write(&path).await;

        assert!(can_write);
    }
}

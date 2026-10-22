use crate::core::path_util::FilesPath;
use crate::storage::storage::{FileMetadata, FileType, StorableItem};
use anyhow::Result;
use std::fs;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileItem {
    pub name: String,
    pub path: FilesPath,
    pub file_type: FileType,
    pub size: u64,
    pub is_hidden: bool,
    pub is_system: bool,
    pub modified: Option<std::time::SystemTime>,
    pub created: Option<std::time::SystemTime>,
}

pub struct MetadataExtractor;

impl MetadataExtractor {
    pub fn extract(path: &FilesPath) -> Result<StorableItem> {
        let meta = fs::metadata(path.as_path())?;
        let file_type = if meta.is_dir() {
            FileType::Directory
        } else if meta.is_symlink() {
            FileType::Symlink
        } else {
            FileType::File
        };

        Ok(StorableItem {
            path: path.clone(),
            metadata: FileMetadata {
                size: meta.len(),
                created: meta.created().ok(),
                modified: meta.modified().ok(),
                file_type,
                is_hidden: false,
                is_system: false,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use tempfile::tempdir;

    #[test]
    fn test_extract_file() -> Result<()> {
        let dir = tempdir()?;
        let file_path = dir.path().join("test_file.txt");
        File::create(&file_path)?;

        let fs_path = FilesPath::new(file_path);
        let item = MetadataExtractor::extract(&fs_path)?;

        assert_eq!(item.metadata.size, 0);
        assert!(matches!(item.metadata.file_type, FileType::File));
        Ok(())
    }
}

use crate::core::path_util::FilesPath;
use anyhow::Result;
use std::fs;

pub struct BulkRenameService;

impl BulkRenameService {
    pub async fn rename_with_pattern(
        items: &[FilesPath],
        pattern: &str,
        replacement: &str,
    ) -> Result<Vec<(FilesPath, FilesPath)>> {
        let mut results = Vec::new();
        for item in items {
            let old_name = item
                .as_path()
                .file_name()
                .ok_or_else(|| anyhow::anyhow!("Invalid filename"))?
                .to_string_lossy();

            if old_name.contains(pattern) {
                let new_name = old_name.replace(pattern, replacement);
                let mut new_path = item.as_path().to_path_buf();
                new_path.set_file_name(new_name);
                let new_files_path = FilesPath::new(new_path);

                fs::rename(item.as_path(), new_files_path.as_path())?;
                results.push((item.clone(), new_files_path));
            }
        }
        Ok(results)
    }

    /// Renames a single file with pattern replacement
    pub async fn rename_single(
        path: &FilesPath,
        pattern: &str,
        replacement: &str,
    ) -> Result<FilesPath> {
        let old_name = path
            .as_path()
            .file_name()
            .ok_or_else(|| anyhow::anyhow!("Invalid filename"))?
            .to_string_lossy();

        let new_name = old_name.replace(pattern, replacement);
        let mut new_path = path.as_path().to_path_buf();
        new_path.set_file_name(new_name);
        
        fs::rename(path.as_path(), &new_path)?;
        Ok(FilesPath::new(new_path))
    }

    /// Generates new names without actually renaming (dry run)
    pub fn preview_rename(items: &[FilesPath], pattern: &str, replacement: &str) -> Vec<(String, String)> {
        let mut results = Vec::new();
        for item in items {
            if let Some(old_name) = item.as_path().file_name() {
                let old_str = old_name.to_string_lossy();
                if old_str.contains(pattern) {
                    let new_name = old_str.replace(pattern, replacement);
                    results.push((old_str.to_string(), new_name));
                }
            }
        }
        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_rename_single_basic() -> Result<()> {
        let dir = tempdir()?;
        let old_path = dir.path().join("test_old.txt");
        std::fs::write(&old_path, "content")?;

        let files_path = FilesPath::new(&old_path);
        let new_path = BulkRenameService::rename_single(&files_path, "old", "new").await?;

        assert!(!old_path.exists());
        assert!(new_path.as_path().exists());
        assert_eq!(
            new_path.as_path().file_name().unwrap().to_string_lossy(),
            "test_new.txt"
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_rename_single_multiple_occurrences() -> Result<()> {
        let dir = tempdir()?;
        let old_path = dir.path().join("old_file_old_name.txt");
        std::fs::write(&old_path, "content")?;

        let files_path = FilesPath::new(&old_path);
        let new_path = BulkRenameService::rename_single(&files_path, "old", "new").await?;

        assert!(!old_path.exists());
        assert!(new_path.as_path().exists());
        assert_eq!(
            new_path.as_path().file_name().unwrap().to_string_lossy(),
            "new_file_new_name.txt"
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_rename_with_pattern_basic() -> Result<()> {
        let dir = tempdir()?;
        
        // Create multiple test files
        let file1 = dir.path().join("doc_old.txt");
        let file2 = dir.path().join("report_old.txt");
        let file3 = dir.path().join("doc_new.txt"); // This one should NOT be renamed (no "old" after replace)
        
        std::fs::write(&file1, "content1")?;
        std::fs::write(&file2, "content2")?;
        std::fs::write(&file3, "content3")?;

        let items = vec![
            FilesPath::new(&file1),
            FilesPath::new(&file2),
            FilesPath::new(&file3),
        ];

        let results = BulkRenameService::rename_with_pattern(&items, "old", "new").await?;

        // Should have renamed 2 files
        assert_eq!(results.len(), 2);
        
        // Verify old files gone and new files exist
        assert!(!file1.exists());
        assert!(!file2.exists());
        assert!(file3.exists()); // unchanged

        assert_eq!(
            results[0].1.as_path().file_name().unwrap().to_string_lossy(),
            "doc_new.txt"
        );
        assert_eq!(
            results[1].1.as_path().file_name().unwrap().to_string_lossy(),
            "report_new.txt"
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_preview_rename() -> Result<()> {
        let dir = tempdir()?;
        
        let file1 = dir.path().join("test_old.txt");
        let file2 = dir.path().join("test_old_old.txt");
        let file3 = dir.path().join("test_new.txt"); // no match
        
        std::fs::write(&file1, "1")?;
        std::fs::write(&file2, "2")?;
        std::fs::write(&file3, "3")?;

        let items = vec![
            FilesPath::new(&file1),
            FilesPath::new(&file2),
            FilesPath::new(&file3),
        ];

        let preview = BulkRenameService::preview_rename(&items, "old", "new");
        
        assert_eq!(preview.len(), 2);
        assert_eq!(preview[0], ("test_old.txt".to_string(), "test_new.txt".to_string()));
        assert_eq!(preview[1], ("test_old_old.txt".to_string(), "test_new_new.txt".to_string()));
        
        // Files should NOT be renamed (preview mode)
        assert!(file1.exists());
        assert!(file2.exists());
        assert!(file3.exists());
        
        Ok(())
    }

    #[test]
    fn test_preview_rename_empty_items() {
        let items: Vec<FilesPath> = vec![];
        let preview = BulkRenameService::preview_rename(&items, "old", "new");
        assert!(preview.is_empty());
    }

    #[test]
    fn test_preview_rename_no_matches() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("test.txt");
        std::fs::write(&file, "content").unwrap();

        let items = vec![FilesPath::new(&file)];
        let preview = BulkRenameService::preview_rename(&items, "old", "new");
        
        assert!(preview.is_empty());
        // File should be unchanged
        assert!(file.exists());
    }
}

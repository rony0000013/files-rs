use crate::core::path_util::FilesPath;
use anyhow::Result;
use std::fs;

pub struct FolderFlattenService;

impl FolderFlattenService {
    pub async fn flatten(folder: &FilesPath) -> Result<()> {
        Self::flatten_recursive(folder.as_path())
    }

    fn flatten_recursive(folder: &std::path::Path) -> Result<()> {
        let entries = fs::read_dir(folder)?;
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                // Recursively flatten nested directories first
                Self::flatten_recursive(&path)?;
                
                // Now move all contents from this subdirectory to parent
                let sub_entries = fs::read_dir(&path)?;
                for sub_entry in sub_entries {
                    let sub_entry = sub_entry?;
                    let dest = folder.join(sub_entry.file_name());
                    fs::rename(sub_entry.path(), dest)?;
                }
                fs::remove_dir(path)?;
            }
        }
        Ok(())
    }

    /// Flatten with conflict resolution - adds suffix if file exists
    pub async fn flatten_with_conflicts(folder: &FilesPath, suffix: &str) -> Result<Vec<(String, String)>> {
        let mut moved = Vec::new();
        let entries = fs::read_dir(folder.as_path())?;
        
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                let sub_entries = fs::read_dir(&path)?;
                for sub_entry in sub_entries {
                    let sub_entry = sub_entry?;
                    let file_name = sub_entry.file_name();
                    let mut dest = folder.as_path().join(&file_name);
                    
                    // Handle conflicts
                    if dest.exists() {
                        let stem = file_name
                            .to_string_lossy()
                            .rsplit_once('.')
                            .map(|(s, e)| format!("{}_{}.{}", s, suffix, e))
                            .unwrap_or_else(|| format!("{}_{}", file_name.to_string_lossy(), suffix));
                        dest = folder.as_path().join(&stem);
                    }
                    
                    let old_name = file_name.to_string_lossy().to_string();
                    let new_name = dest.file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| old_name.clone());
                    
                    fs::rename(sub_entry.path(), &dest)?;
                    moved.push((old_name, new_name));
                }
                fs::remove_dir(path)?;
            }
        }
        Ok(moved)
    }

    /// Preview what would be flattened (dry run)
    pub fn preview_flatten(folder: &FilesPath) -> Result<Vec<(String, String)>> {
        let mut results = Vec::new();
        let entries = fs::read_dir(folder.as_path())?;
        
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                let dir_name = path.file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                let sub_entries = fs::read_dir(&path)?;
                for sub_entry in sub_entries {
                    let sub_entry = sub_entry?;
                    let file_name = sub_entry.file_name()
                        .to_string_lossy()
                        .to_string();
                    results.push((dir_name.clone(), file_name));
                }
            }
        }
        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_flatten_basic() -> Result<()> {
        let dir = tempdir()?;
        
        // Create a subfolder with files
        let subfolder = dir.path().join("subfolder");
        fs::create_dir(&subfolder)?;
        
        let file1 = subfolder.join("file1.txt");
        let file2 = subfolder.join("file2.txt");
        fs::write(&file1, "content1")?;
        fs::write(&file2, "content2")?;
        
        // Also add a file directly in root (should remain)
        let root_file = dir.path().join("root_file.txt");
        fs::write(&root_file, "root content")?;
        
        let folder = FilesPath::new(dir.path());
        FolderFlattenService::flatten(&folder).await?;
        
        // Subfolder should be gone
        assert!(!subfolder.exists());
        
        // Files should be moved to root
        assert!(dir.path().join("file1.txt").exists());
        assert!(dir.path().join("file2.txt").exists());
        
        // Root file should remain
        assert!(root_file.exists());
        
        Ok(())
    }

    #[tokio::test]
    async fn test_flatten_with_conflicts() -> Result<()> {
        let dir = tempdir()?;
        
        // Create a file that will conflict
        let existing_file = dir.path().join("existing.txt");
        fs::write(&existing_file, "existing content")?;
        
        // Create a subfolder with same-named file
        let subfolder = dir.path().join("subfolder");
        fs::create_dir(&subfolder)?;
        let conflicting_file = subfolder.join("existing.txt");
        fs::write(&conflicting_file, "new content")?;
        
        let folder = FilesPath::new(dir.path());
        let moved = FolderFlattenService::flatten_with_conflicts(&folder, "moved").await?;
        
        // Check the conflict resolution
        assert_eq!(moved.len(), 1);
        assert_eq!(moved[0].0, "existing.txt");
        assert_eq!(moved[0].1, "existing_moved.txt");
        
        // Original file should still exist
        assert!(existing_file.exists());
        
        // New file should be created with suffix
        assert!(dir.path().join("existing_moved.txt").exists());
        
        Ok(())
    }

    #[test]
    fn test_preview_flatten() -> Result<()> {
        let dir = tempdir()?;
        
        // Create a subfolder with files
        let subfolder = dir.path().join("documents");
        fs::create_dir(&subfolder)?;
        fs::write(subfolder.join("file1.txt"), "1")?;
        fs::write(subfolder.join("file2.txt"), "2")?;
        
        // Create another subfolder
        let subfolder2 = dir.path().join("images");
        fs::create_dir(&subfolder2)?;
        fs::write(subfolder2.join("photo.jpg"), "3")?;
        
        let folder = FilesPath::new(dir.path());
        let preview = FolderFlattenService::preview_flatten(&folder)?;
        
        assert_eq!(preview.len(), 3);
        
        // Check that we get source folder name and filename pairs
        let doc_files: Vec<_> = preview.iter()
            .filter(|(src, _)| src == "documents")
            .collect();
        assert_eq!(doc_files.len(), 2);
        
        let image_files: Vec<_> = preview.iter()
            .filter(|(src, _)| src == "images")
            .collect();
        assert_eq!(image_files.len(), 1);
        
        // Files should NOT be actually moved (preview mode)
        assert!(subfolder.exists());
        assert!(subfolder.join("file1.txt").exists());
        
        Ok(())
    }

    #[tokio::test]
    async fn test_flatten_nested_subfolders() -> Result<()> {
        let dir = tempdir()?;
        
        // Create nested subfolders
        let level1 = dir.path().join("level1");
        fs::create_dir(&level1)?;
        let level2 = level1.join("level2");
        fs::create_dir(&level2)?;
        
        fs::write(level2.join("nested_file.txt"), "content")?;
        
        let folder = FilesPath::new(dir.path());
        FolderFlattenService::flatten(&folder).await?;
        
        // Only level1 content should move, level2 should be removed
        assert!(!level2.exists());
        assert!(!level1.exists());
        assert!(dir.path().join("nested_file.txt").exists());
        
        Ok(())
    }
}

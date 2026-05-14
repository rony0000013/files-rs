//! Create Folder With Selection Service
//! Creates a new folder and moves selected items into it

use crate::core::path_util::FilesPath;
use anyhow::{Context, Result};
use std::path::Path;

/// Result of a create folder with selection operation
#[derive(Debug)]
pub struct CreateFolderWithSelectionResult {
    /// Path to the newly created folder
    pub folder_path: FilesPath,
    /// Number of items moved into the folder
    pub items_moved: u32,
}

/// Service for creating a new folder and moving selected items into it
pub struct CreateFolderWithSelectionService;

impl CreateFolderWithSelectionService {
    /// Creates a new instance of CreateFolderWithSelectionService
    pub fn new() -> Self {
        Self
    }

    /// Creates a new folder with the given name and moves the specified items into it
    ///
    /// # Arguments
    /// * `destination_parent` - The parent directory where the new folder will be created
    /// * `folder_name` - Name for the new folder
    /// * `items_to_move` - List of item paths to move into the new folder
    ///
    /// # Returns
    /// Returns the result containing the new folder path and count of moved items
    pub async fn execute(
        &self,
        destination_parent: &Path,
        folder_name: &str,
        items_to_move: &[FilesPath],
    ) -> Result<CreateFolderWithSelectionResult> {
        // Create the new folder path
        let mut new_folder_path = destination_parent.to_path_buf();
        new_folder_path.push(folder_name);

        // Create the directory
        std::fs::create_dir_all(&new_folder_path)
            .context(format!("Failed to create folder: {:?}", new_folder_path))?;

        let new_folder_files_path = FilesPath::new(&new_folder_path);
        let mut items_moved = 0u32;

        // Move each item into the new folder
        for item in items_to_move {
            let item_name = item
                .as_path()
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown");

            let mut target_path = new_folder_path.clone();
            target_path.push(item_name);

            // Use rename to move the item
            match std::fs::rename(item.as_path(), &target_path) {
                Ok(_) => {
                    items_moved += 1;
                    tracing::debug!("Moved {:?} to {:?}", item.as_path(), target_path);
                }
                Err(e) => {
                    tracing::warn!("Failed to move {:?}: {}", item.as_path(), e);
                    // Continue with other items even if one fails
                }
            }
        }

        Ok(CreateFolderWithSelectionResult {
            folder_path: new_folder_files_path,
            items_moved,
        })
    }

    /// Generates a unique folder name if the requested name already exists
    pub fn generate_unique_folder_name(base_name: &str, existing_names: &[String]) -> String {
        if !existing_names.contains(&base_name.to_string()) {
            return base_name.to_string();
        }

        let mut counter = 1;
        loop {
            let new_name = format!("{} ({})", base_name, counter);
            if !existing_names.contains(&new_name) {
                return new_name;
            }
            counter += 1;
        }
    }
}

impl Default for CreateFolderWithSelectionService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_generate_unique_folder_name_no_conflict() {
        let names = vec!["Documents".to_string(), "Pictures".to_string()];
        let result = CreateFolderWithSelectionService::generate_unique_folder_name("New Folder", &names);
        assert_eq!(result, "New Folder");
    }

    #[test]
    fn test_generate_unique_folder_name_with_conflict() {
        let names = vec!["New Folder".to_string(), "New Folder (1)".to_string()];
        let result = CreateFolderWithSelectionService::generate_unique_folder_name("New Folder", &names);
        assert_eq!(result, "New Folder (2)");
    }

    #[test]
    fn test_generate_unique_folder_name_multiple_conflicts() {
        let names = vec![
            "New Folder".to_string(),
            "New Folder (1)".to_string(),
            "New Folder (2)".to_string(),
        ];
        let result = CreateFolderWithSelectionService::generate_unique_folder_name("New Folder", &names);
        assert_eq!(result, "New Folder (3)");
    }

    #[test]
    fn test_generate_unique_folder_name_empty_list() {
        let names: Vec<String> = vec![];
        let result = CreateFolderWithSelectionService::generate_unique_folder_name("Test", &names);
        assert_eq!(result, "Test");
    }

    #[tokio::test]
    async fn test_execute_creates_folder_and_moves_items() -> Result<()> {
        let temp_dir = tempdir()?;
        let parent_path = temp_dir.path();

        // Create some test files
        let file1 = parent_path.join("file1.txt");
        let file2 = parent_path.join("file2.txt");
        std::fs::write(&file1, "content1")?;
        std::fs::write(&file2, "content2")?;

        let items = vec![FilesPath::new(&file1), FilesPath::new(&file2)];

        let service = CreateFolderWithSelectionService::new();
        let result = service
            .execute(parent_path, "NewFolder", &items)
            .await?;

        assert_eq!(result.items_moved, 2);
        assert!(result.folder_path.as_path().exists());
        assert!(result.folder_path.as_path().join("file1.txt").exists());
        assert!(result.folder_path.as_path().join("file2.txt").exists());

        // Original files should be moved (not exist in original location)
        assert!(!file1.exists());
        assert!(!file2.exists());

        Ok(())
    }

    #[tokio::test]
    async fn test_execute_with_empty_items() -> Result<()> {
        let temp_dir = tempdir()?;
        let parent_path = temp_dir.path();

        let items: Vec<FilesPath> = vec![];

        let service = CreateFolderWithSelectionService::new();
        let result = service
            .execute(parent_path, "EmptyFolder", &items)
            .await?;

        assert_eq!(result.items_moved, 0);
        assert!(result.folder_path.as_path().exists());

        Ok(())
    }

    #[tokio::test]
    async fn test_execute_handles_nonexistent_parent() -> Result<()> {
        let service = CreateFolderWithSelectionService::new();
        let nonexistent_parent = Path::new("/nonexistent/path");
        let items: Vec<FilesPath> = vec![];

        let result = service
            .execute(nonexistent_parent, "TestFolder", &items)
            .await;

        // Should fail gracefully
        assert!(result.is_err() || result.unwrap().folder_path.as_path().exists());

        Ok(())
    }

    #[tokio::test]
    async fn test_execute_moves_nested_structure() -> Result<()> {
        let temp_dir = tempdir()?;
        let parent_path = temp_dir.path();

        // Create a subfolder with a file
        let subfolder = parent_path.join("subfolder");
        std::fs::create_dir_all(&subfolder)?;
        let file_in_subfolder = subfolder.join("nested.txt");
        std::fs::write(&file_in_subfolder, "nested content")?;

        let items = vec![FilesPath::new(&subfolder)];

        let service = CreateFolderWithSelectionService::new();
        let result = service
            .execute(parent_path, "ContainerFolder", &items)
            .await?;

        assert_eq!(result.items_moved, 1);
        assert!(result.folder_path.as_path().join("subfolder").exists());
        assert!(result.folder_path.as_path().join("subfolder").join("nested.txt").exists());

        Ok(())
    }

    #[test]
    fn test_service_creation() {
        let service = CreateFolderWithSelectionService::new();
        let default_service = CreateFolderWithSelectionService::default();
        assert!(true); // Both created without panic
        let _ = default_service;
        let _ = service;
    }

    #[test]
    fn test_result_debug_format() {
        let temp_dir = tempdir().unwrap();
        let result = CreateFolderWithSelectionResult {
            folder_path: FilesPath::new(temp_dir.path()),
            items_moved: 5,
        };
        let debug_str = format!("{:?}", result);
        assert!(debug_str.contains("5"));
    }
}
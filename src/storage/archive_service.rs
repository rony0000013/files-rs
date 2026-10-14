use crate::core::path_util::FilesPath;
use anyhow::{anyhow, Result};
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use zip::write::FileOptions;
use zip::ZipWriter;

pub struct ArchiveService;

impl ArchiveService {
    /// Creates a ZIP archive from a list of files/folders
    pub async fn create_zip(source_items: &[FilesPath], destination: &FilesPath) -> Result<()> {
        let path = std::path::PathBuf::from(destination.as_path());
        let file = File::create(&path)?;
        let mut zip = ZipWriter::new(file);
        let options: zip::write::FileOptions<()> = FileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .unix_permissions(0o755);

        for item in source_items {
            let item_path = item.as_path();
            if item_path.is_dir() {
                let walk = jwalk::WalkDir::new(item_path)
                    .into_iter()
                    .filter_map(|e| e.ok());
                for entry in walk {
                    let path = entry.path();
                    let name = path
                        .strip_prefix(item_path)
                        .map_err(|e| anyhow!("Failed to strip prefix: {}", e))?;

                    if path.is_dir() {
                        zip.add_directory(name.to_string_lossy(), options.clone())?;
                    } else {
                        zip.start_file(name.to_string_lossy(), options.clone())?;
                        let mut f = std::fs::File::open(path)?;
                        std::io::copy(&mut f, &mut zip)?;
                    }
                }
            } else {
                zip.start_file(
                    item_path.file_name().unwrap().to_string_lossy(),
                    options.clone(),
                )?;
                let mut f = std::fs::File::open(item_path)?;
                std::io::copy(&mut f, &mut zip)?;
            }
        }
        zip.finish()?;
        Ok(())
    }

    /// Extracts a ZIP archive to a destination folder
    pub async fn extract_zip(archive_path: &FilesPath, destination: &FilesPath) -> Result<()> {
        let file = File::open(archive_path.as_path())?;
        let mut archive = zip::ZipArchive::new(file)?;

        let dest_root = std::fs::canonicalize(destination.as_path())
            .unwrap_or_else(|_| destination.as_path().to_path_buf());

        for i in 0..archive.len() {
            let mut file = archive.by_index(i)?;
            let outpath = std::path::PathBuf::from(destination.as_path()).join(file.mangled_name()?);

            // Zip slip protection: validate the resolved path stays within destination
            let canonical = std::fs::canonicalize(&outpath).unwrap_or_else(|_| outpath.clone());
            if !canonical.starts_with(&dest_root) {
                anyhow::bail!(
                    "Zip slip detected: entry '{}' resolves to '{}' which is outside destination '{}'",
                    file.name()?,
                    canonical.display(),
                    dest_root.display()
                );
            }

            if file.is_dir() {
                std::fs::create_dir_all(&outpath)?;
            } else {
                if let Some(p) = outpath.parent() {
                    if !p.exists() {
                        std::fs::create_dir_all(p)?;
                    }
                }
                let mut outfile = File::create(&outpath)?;
                std::io::copy(&mut file, &mut outfile)?;
            }
        }
        Ok(())
    }

    /// Lists contents of a ZIP archive without extracting
    pub fn list_contents(archive_path: &FilesPath) -> Result<Vec<String>> {
        let file = File::open(archive_path.as_path())?;
        let mut archive = zip::ZipArchive::new(file)?;
        let mut names = Vec::new();

        for i in 0..archive.len() {
            let file = archive.by_index(i)?;
            names.push(file.name()?.to_string());
        }
        Ok(names)
    }

    /// Gets the top-level items in an archive (files/directories in root)
    fn get_top_level_items(archive: &mut zip::ZipArchive<File>) -> Result<Vec<String>> {
        let mut top_level = Vec::new();
        let mut seen = std::collections::HashSet::new();

        for i in 0..archive.len() {
            let file = archive.by_index(i)?;
            let name = file.name()?.to_string();

            // Get first path component
            let top = name.split('/').next().unwrap_or(&name).to_string();
            if !top.is_empty() && !seen.contains(&top) {
                seen.insert(top.clone());
                top_level.push(top);
            }
        }
        Ok(top_level)
    }

    /// Extracts archive "smart" - creates subfolder if multiple top-level items exist
    /// Returns the path where files were extracted
    pub async fn extract_zip_smart(
        archive_path: &FilesPath,
        destination: &FilesPath,
    ) -> Result<FilesPath> {
        // First, open and analyze archive
        let file = File::open(archive_path.as_path())?;
        let mut archive = zip::ZipArchive::new(file)?;

        // Determine if we need a subfolder
        let top_level_items = Self::get_top_level_items(&mut archive)?;

        let actual_destination = if top_level_items.len() > 1 {
            // Multiple items - create subfolder with archive name
            let archive_name = archive_path
                .as_path()
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "extracted".to_string());

            let subfolder_path = destination.as_path().join(&archive_name);
            std::fs::create_dir_all(&subfolder_path)?;
            FilesPath::new(subfolder_path)
        } else {
            destination.clone()
        };

        // Now do the actual extraction
        Self::extract_zip(archive_path, &actual_destination).await?;

        Ok(actual_destination)
    }

    /// Analyzes an archive to determine extraction strategy
    pub fn analyze_archive(archive_path: &FilesPath) -> Result<ArchiveAnalysis> {
        let file = File::open(archive_path.as_path())?;
        let mut archive = zip::ZipArchive::new(file)?;

        let top_level_items = Self::get_top_level_items(&mut archive)?;
        let total_items = archive.len();

        let strategy = if top_level_items.is_empty() {
            ExtractionStrategy::Empty
        } else if top_level_items.len() == 1 {
            let item = &top_level_items[0];
            if item.contains('.') {
                // Single file (has extension)
                ExtractionStrategy::SingleFile
            } else {
                // Single folder
                ExtractionStrategy::SingleFolder
            }
        } else {
            ExtractionStrategy::MultipleItems(top_level_items.len())
        };

        Ok(ArchiveAnalysis {
            total_items,
            top_level_count: top_level_items.len(),
            top_level_names: top_level_items,
            recommended_strategy: strategy,
        })
    }
}

/// Analysis result for an archive
#[derive(Debug, Clone)]
pub struct ArchiveAnalysis {
    pub total_items: usize,
    pub top_level_count: usize,
    pub top_level_names: Vec<String>,
    pub recommended_strategy: ExtractionStrategy,
}

/// Recommended extraction strategy based on archive contents
#[derive(Debug, Clone, PartialEq)]
pub enum ExtractionStrategy {
    /// Archive is empty
    Empty,
    /// Single file directly in archive
    SingleFile,
    /// Single folder directly in archive
    SingleFolder,
    /// Multiple items at root level
    MultipleItems(usize),
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_create_and_extract_zip_file() -> Result<()> {
        let dir = tempdir()?;

        // Create test files
        let file1 = dir.path().join("test1.txt");
        let file2 = dir.path().join("test2.txt");
        std::fs::write(&file1, "Hello World 1")?;
        std::fs::write(&file2, "Hello World 2")?;

        // Create zip
        let zip_path = dir.path().join("output.zip");
        let source_items = vec![FilesPath::new(&file1), FilesPath::new(&file2)];

        ArchiveService::create_zip(&source_items, &FilesPath::new(&zip_path)).await?;

        // Verify zip exists
        assert!(zip_path.exists());

        // Extract to new folder
        let extract_dir = dir.path().join("extracted");
        std::fs::create_dir(&extract_dir)?;

        ArchiveService::extract_zip(&FilesPath::new(&zip_path), &FilesPath::new(&extract_dir))
            .await?;

        // Verify extracted files
        assert!(extract_dir.join("test1.txt").exists());
        assert!(extract_dir.join("test2.txt").exists());
        assert_eq!(
            std::fs::read_to_string(extract_dir.join("test1.txt"))?,
            "Hello World 1"
        );
        assert_eq!(
            std::fs::read_to_string(extract_dir.join("test2.txt"))?,
            "Hello World 2"
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_create_zip_directory() -> Result<()> {
        let dir = tempdir()?;

        // Create a folder structure
        let subfolder = dir.path().join("docs");
        std::fs::create_dir(&subfolder)?;
        std::fs::write(subfolder.join("readme.txt"), "Readme content")?;
        std::fs::write(subfolder.join("notes.txt"), "Notes content")?;

        // Create zip from directory - contents are stored without the folder prefix
        let zip_path = dir.path().join("docs.zip");
        let source_items = vec![FilesPath::new(&subfolder)];

        ArchiveService::create_zip(&source_items, &FilesPath::new(&zip_path)).await?;

        assert!(zip_path.exists());

        // Extract and verify
        let extract_dir = dir.path().join("extracted");
        std::fs::create_dir(&extract_dir)?;

        ArchiveService::extract_zip(&FilesPath::new(&zip_path), &FilesPath::new(&extract_dir))
            .await?;

        // Files are stored without folder prefix, so they go directly to extract_dir
        assert!(extract_dir.join("readme.txt").exists());
        assert!(extract_dir.join("notes.txt").exists());

        Ok(())
    }

    #[test]
    fn test_list_contents() -> Result<()> {
        let dir = tempdir()?;

        // Create test files
        let file1 = dir.path().join("file1.txt");
        let file2 = dir.path().join("file2.txt");
        std::fs::write(&file1, "Content 1")?;
        std::fs::write(&file2, "Content 2")?;

        // Create zip
        let zip_path = dir.path().join("output.zip");
        let source_items = vec![FilesPath::new(&file1), FilesPath::new(&file2)];

        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(ArchiveService::create_zip(
            &source_items,
            &FilesPath::new(&zip_path),
        ))?;

        // List contents
        let contents = ArchiveService::list_contents(&FilesPath::new(&zip_path))?;

        assert_eq!(contents.len(), 2);
        assert!(contents.contains(&"file1.txt".to_string()));
        assert!(contents.contains(&"file2.txt".to_string()));

        Ok(())
    }

    #[tokio::test]
    async fn test_empty_archive() -> Result<()> {
        let dir = tempdir()?;

        // Create empty zip
        let zip_path = dir.path().join("empty.zip");
        ArchiveService::create_zip(&[], &FilesPath::new(&zip_path)).await?;

        assert!(zip_path.exists());

        // Extract empty zip
        let extract_dir = dir.path().join("extracted");
        std::fs::create_dir(&extract_dir)?;

        ArchiveService::extract_zip(&FilesPath::new(&zip_path), &FilesPath::new(&extract_dir))
            .await?;

        // Should have extracted nothing
        let entries: Vec<_> = std::fs::read_dir(&extract_dir)?.collect();
        assert!(entries.is_empty());

        Ok(())
    }

    #[tokio::test]
    async fn test_preserve_directory_structure() -> Result<()> {
        let dir = tempdir()?;

        // Create nested folder structure
        let folder1 = dir.path().join("folder1");
        let folder2 = folder1.join("folder2");
        std::fs::create_dir_all(&folder2)?;

        std::fs::write(folder2.join("deep_file.txt"), "Deep content")?;
        std::fs::write(folder1.join("shallow_file.txt"), "Shallow content")?;

        // Create zip from nested folder
        let zip_path = dir.path().join("nested.zip");
        let source_items = vec![FilesPath::new(&folder1)];

        ArchiveService::create_zip(&source_items, &FilesPath::new(&zip_path)).await?;

        // Extract
        let extract_dir = dir.path().join("extracted");
        std::fs::create_dir(&extract_dir)?;

        ArchiveService::extract_zip(&FilesPath::new(&zip_path), &FilesPath::new(&extract_dir))
            .await?;

        // Verify directory structure - prefix is stripped so files go to root
        // folder2/deep_file.txt becomes folder2/deep_file.txt (prefix folder1 stripped)
        // folder1/shallow_file.txt becomes shallow_file.txt (prefix folder1 stripped)
        assert!(extract_dir.join("shallow_file.txt").exists());
        assert!(extract_dir.join("folder2").join("deep_file.txt").exists());

        Ok(())
    }

    #[tokio::test]
    async fn test_extract_zip_smart_single_file() -> Result<()> {
        let dir = tempdir()?;

        // Create a zip with a single file
        let file1 = dir.path().join("readme.txt");
        std::fs::write(&file1, "Readme content")?;

        let zip_path = dir.path().join("single.zip");
        ArchiveService::create_zip(&[FilesPath::new(&file1)], &FilesPath::new(&zip_path)).await?;

        // Extract with smart method
        let extract_dir = dir.path().join("extracted");
        std::fs::create_dir(&extract_dir)?;

        let result_path = ArchiveService::extract_zip_smart(
            &FilesPath::new(&zip_path),
            &FilesPath::new(&extract_dir),
        )
        .await?;

        // Should extract directly to extract_dir (no subfolder for single file)
        assert!(extract_dir.join("readme.txt").exists());
        assert_eq!(result_path.as_path(), extract_dir);

        Ok(())
    }

    #[tokio::test]
    async fn test_extract_zip_smart_multiple_files() -> Result<()> {
        let dir = tempdir()?;

        // Create a zip with multiple files
        let file1 = dir.path().join("file1.txt");
        let file2 = dir.path().join("file2.txt");
        std::fs::write(&file1, "Content 1")?;
        std::fs::write(&file2, "Content 2")?;

        let zip_path = dir.path().join("multi.zip");
        ArchiveService::create_zip(
            &[FilesPath::new(&file1), FilesPath::new(&file2)],
            &FilesPath::new(&zip_path),
        )
        .await?;

        // Extract with smart method
        let extract_dir = dir.path().join("extracted");
        std::fs::create_dir(&extract_dir)?;

        let result_path = ArchiveService::extract_zip_smart(
            &FilesPath::new(&zip_path),
            &FilesPath::new(&extract_dir),
        )
        .await?;

        // Should create subfolder "multi" (from archive name)
        assert!(result_path.as_path().join("file1.txt").exists());
        assert!(result_path.as_path().join("file2.txt").exists());
        assert!(result_path.as_path().ends_with("multi"));

        Ok(())
    }

    #[test]
    fn test_analyze_archive_single_file() -> Result<()> {
        let dir = tempdir()?;

        // Create a zip with a single file
        let file1 = dir.path().join("readme.txt");
        std::fs::write(&file1, "Readme")?;

        let zip_path = dir.path().join("single.zip");
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(ArchiveService::create_zip(
            &[FilesPath::new(&file1)],
            &FilesPath::new(&zip_path),
        ))?;

        let analysis = ArchiveService::analyze_archive(&FilesPath::new(&zip_path))?;

        assert_eq!(analysis.total_items, 1);
        assert_eq!(analysis.top_level_count, 1);
        assert!(matches!(
            analysis.recommended_strategy,
            ExtractionStrategy::SingleFile
        ));

        Ok(())
    }

    #[test]
    fn test_analyze_archive_multiple_items() -> Result<()> {
        let dir = tempdir()?;

        // Create a zip with multiple files
        let file1 = dir.path().join("file1.txt");
        let file2 = dir.path().join("file2.txt");
        std::fs::write(&file1, "1")?;
        std::fs::write(&file2, "2")?;

        let zip_path = dir.path().join("multi.zip");
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(ArchiveService::create_zip(
            &[FilesPath::new(&file1), FilesPath::new(&file2)],
            &FilesPath::new(&zip_path),
        ))?;

        let analysis = ArchiveService::analyze_archive(&FilesPath::new(&zip_path))?;

        assert_eq!(analysis.total_items, 2);
        assert_eq!(analysis.top_level_count, 2);
        assert!(matches!(
            analysis.recommended_strategy,
            ExtractionStrategy::MultipleItems(2)
        ));

        Ok(())
    }

    #[test]
    fn test_analyze_archive_empty() -> Result<()> {
        let dir = tempdir()?;

        // Create empty zip
        let zip_path = dir.path().join("empty.zip");
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(ArchiveService::create_zip(&[], &FilesPath::new(&zip_path)))?;

        let analysis = ArchiveService::analyze_archive(&FilesPath::new(&zip_path))?;

        assert_eq!(analysis.total_items, 0);
        assert_eq!(analysis.top_level_count, 0);
        assert!(matches!(
            analysis.recommended_strategy,
            ExtractionStrategy::Empty
        ));

        Ok(())
    }
}

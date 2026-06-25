use crate::core::path_util::FilesPath;
use anyhow::Result;
use async_trait::async_trait;
use notify::{Config, Event, EventKind, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use std::time::SystemTime;
use tokio::sync::mpsc;
use tracing;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum FileType {
    File,
    Directory,
    Symlink,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMetadata {
    pub size: u64,
    pub created: Option<SystemTime>,
    pub modified: Option<SystemTime>,
    pub file_type: FileType,
    pub is_hidden: bool,
    pub is_system: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorableItem {
    pub path: FilesPath,
    pub metadata: FileMetadata,
}

#[async_trait]
pub trait StorageService {
    async fn list_directory(&self, path: &FilesPath) -> Result<Vec<StorableItem>>;
    async fn get_metadata(&self, path: &FilesPath) -> Result<StorableItem>;
    async fn read_file(&self, path: &FilesPath) -> Result<Vec<u8>>;
    async fn write_file(&self, path: &FilesPath, data: &[u8]) -> Result<()>;
}

pub struct LocalFileSystem;

#[async_trait]
#[allow(dead_code)]
impl StorageService for LocalFileSystem {
    #[tracing::instrument(skip(self))]
    async fn list_directory(&self, path: &FilesPath) -> Result<Vec<StorableItem>> {
        let mut items = Vec::new();

        // Verify the directory exists and is accessible
        if !path.as_path().is_dir() {
            tracing::warn!("Directory not found or not accessible: {:?}", path);
            return Ok(items); // Return empty rather than erroring
        }

        // Use std::fs::read_dir (not jwalk) for flat listings — much faster on WSL
        // because DirEntry::metadata() reuses the directory handle, halving syscalls.
        match std::fs::read_dir(path.as_path()) {
            Ok(rd) => {
                for entry in rd.filter_map(|e| e.ok()) {
                    let path = FilesPath::new(entry.path());
                    match get_metadata_from_entry(&entry, &path).await {
                        Ok(item) => items.push(item),
                        Err(e) => {
                            tracing::warn!("Skipping entry {:?}: {}", path, e);
                            continue;
                        }
                    }
                }
            }
            Err(e) => {
                tracing::warn!("Failed to read directory {:?}: {}", path, e);
            }
        }
        Ok(items)
    }

    #[tracing::instrument(skip(self))]
    async fn get_metadata(&self, path: &FilesPath) -> Result<StorableItem> {
        let meta = std::fs::metadata(path.as_path())?;
        let file_type = if meta.is_dir() {
            FileType::Directory
        } else if meta.is_symlink() {
            FileType::Symlink
        } else {
            FileType::File
        };

        let mut is_hidden = false;
        let mut is_system = false;

        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;

            let attrs = meta.file_attributes();
            is_hidden = (attrs & windows::Win32::Storage::FileSystem::FILE_ATTRIBUTE_HIDDEN.0) != 0;
            is_system = (attrs & windows::Win32::Storage::FileSystem::FILE_ATTRIBUTE_SYSTEM.0) != 0;
        }

        Ok(StorableItem {
            path: path.clone(),
            metadata: FileMetadata {
                size: meta.len(),
                created: meta.created().ok(),
                modified: meta.modified().ok(),
                file_type,
                is_hidden,
                is_system,
            },
        })
    }

    async fn read_file(&self, path: &FilesPath) -> Result<Vec<u8>> {
        let data = std::fs::read(path.as_path())?;
        Ok(data)
    }

    async fn write_file(&self, path: &FilesPath, data: &[u8]) -> Result<()> {
        std::fs::write(path.as_path(), data)?;
        Ok(())
    }
}

#[allow(dead_code)]
pub struct FileWatcher {
    _watcher: notify::RecommendedWatcher,
}

impl FileWatcher {
    pub fn new(path: FilesPath, tx: mpsc::UnboundedSender<FileEvent>) -> Result<Self> {
        let mut watcher = notify::RecommendedWatcher::new(
            move |res: notify::Result<Event>| match res {
                Ok(event) => {
                    if event.paths.is_empty() {
                        return;
                    }
                    let event_type = match event.kind {
                        EventKind::Create(_) => {
                            Some(FileEvent::Created(FilesPath::new(event.paths[0].clone())))
                        }
                        EventKind::Modify(_) => {
                            Some(FileEvent::Modified(FilesPath::new(event.paths[0].clone())))
                        }
                        EventKind::Remove(_) => {
                            Some(FileEvent::Removed(FilesPath::new(event.paths[0].clone())))
                        }
                        _ => None,
                    };

                    if let Some(e) = event_type {
                        let _ = tx.send(e);
                    }
                }
                Err(e) => println!("watch error: {:?}", e),
            },
            Config::default(),
        )?;

        watcher.watch(path.as_path(), RecursiveMode::Recursive)?;

        Ok(Self { _watcher: watcher })
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum FileEvent {
    Created(FilesPath),
    Modified(FilesPath),
    Removed(FilesPath),
    Renamed(FilesPath, FilesPath),
}

#[async_trait]
pub trait FileOperationService {
    async fn delete(&self, path: &FilesPath) -> Result<()>;
    async fn rename(&self, path: &FilesPath, new_name: &str) -> Result<()>;
    async fn copy(&self, src: &FilesPath, dst: &FilesPath) -> Result<()>;
    async fn move_item(&self, src: &FilesPath, dst: &FilesPath) -> Result<()>;
    async fn create_file(&self, path: &FilesPath, content: Option<&[u8]>) -> Result<()>;
    async fn create_folder(&self, path: &FilesPath) -> Result<()>;
    async fn create_shortcut(&self, target: &FilesPath, shortcut_path: &FilesPath) -> Result<()>;
}

pub struct DefaultFileOperationService;

#[async_trait]
impl FileOperationService for DefaultFileOperationService {
    async fn delete(&self, path: &FilesPath) -> Result<()> {
        tracing::info!("Deleting {:?} via trash...", path.as_path());
        trash::delete(path.as_path())?;
        Ok(())
    }

    async fn rename(&self, path: &FilesPath, new_name: &str) -> Result<()> {
        let mut new_path = path.as_path().to_path_buf();
        new_path.set_file_name(new_name);
        std::fs::rename(path.as_path(), new_path)?;
        Ok(())
    }

    async fn copy(&self, src: &FilesPath, dst: &FilesPath) -> Result<()> {
        tracing::info!("Copying {:?} to {:?}...", src.as_path(), dst.as_path());
        let src_path = src.as_path();
        let dst_path = dst.as_path();
        if src_path.is_dir() {
            // Simple recursive copy for directories (stub for now)
            std::fs::create_dir_all(dst_path)?;
            // Actual recursive copy would go here
        } else {
            std::fs::copy(src_path, dst_path)?;
        }
        Ok(())
    }

    async fn move_item(&self, src: &FilesPath, dst: &FilesPath) -> Result<()> {
        tracing::info!("Moving {:?} to {:?}...", src.as_path(), dst.as_path());
        std::fs::rename(src.as_path(), dst.as_path())?;
        Ok(())
    }

    async fn create_file(&self, path: &FilesPath, content: Option<&[u8]>) -> Result<()> {
        tracing::info!("Creating file {:?}", path.as_path());
        let data = content.unwrap_or(&[]);
        std::fs::write(path.as_path(), data)?;
        Ok(())
    }

    async fn create_folder(&self, path: &FilesPath) -> Result<()> {
        tracing::info!("Creating folder {:?}", path.as_path());
        std::fs::create_dir_all(path.as_path())?;
        Ok(())
    }

    async fn create_shortcut(&self, target: &FilesPath, shortcut_path: &FilesPath) -> Result<()> {
        tracing::info!(
            "Creating shortcut for {:?} at {:?}...",
            target.as_path(),
            shortcut_path.as_path()
        );
        // Stub: .lnk creation requires COM ShellLink, will be implemented in WindowsFileOperationService
        Err(anyhow::anyhow!(
            "Shortcut creation not implemented in DefaultFileOperationService"
        ))
    }
}

#[cfg(windows)]
#[allow(dead_code)]
pub struct WindowsFileOperationService;

#[cfg(windows)]
#[async_trait]
impl FileOperationService for WindowsFileOperationService {
    async fn delete(&self, path: &FilesPath) -> Result<()> {
        tracing::info!("Deleting {:?} via Windows Recycle Bin...", path.as_path());
        trash::delete(path.as_path())?;
        Ok(())
    }

    async fn rename(&self, path: &FilesPath, new_name: &str) -> Result<()> {
        let mut new_path = path.as_path().to_path_buf();
        new_path.set_file_name(new_name);
        std::fs::rename(path.as_path(), new_path)?;
        Ok(())
    }

    async fn copy(&self, src: &FilesPath, dst: &FilesPath) -> Result<()> {
        // Use Default implementation for now
        DefaultFileOperationService.copy(src, dst).await
    }

    async fn move_item(&self, src: &FilesPath, dst: &FilesPath) -> Result<()> {
        // Use Default implementation for now
        DefaultFileOperationService.move_item(src, dst).await
    }

    async fn create_file(&self, path: &FilesPath, content: Option<&[u8]>) -> Result<()> {
        // Use Default implementation for now
        DefaultFileOperationService.create_file(path, content).await
    }

    async fn create_folder(&self, path: &FilesPath) -> Result<()> {
        // Use Default implementation for now
        DefaultFileOperationService.create_folder(path).await
    }

    async fn create_shortcut(&self, target: &FilesPath, shortcut_path: &FilesPath) -> Result<()> {
        tracing::info!(
            "Creating Windows shortcut for {:?} at {:?}...",
            target.as_path(),
            shortcut_path.as_path()
        );
        // Real implementation would use IShellLink
        Err(anyhow::anyhow!(
            "Windows shortcut creation (.lnk) requires COM IShellLink implementation"
        ))
    }
}

/// Optimized metadata extraction from a DirEntry — avoids a separate stat syscall
/// by using the handle already opened during read_dir.
async fn get_metadata_from_entry(
    entry: &std::fs::DirEntry,
    path: &FilesPath,
) -> Result<StorableItem> {
    let meta = entry.metadata()?;
    let file_type = if meta.is_dir() {
        FileType::Directory
    } else if meta.is_symlink() {
        FileType::Symlink
    } else {
        FileType::File
    };

    let mut is_hidden = false;
    let mut is_system = false;

    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;

        // file_attributes() comes from the same metadata() call above — no extra syscall
        let attrs = meta.file_attributes();
        is_hidden = (attrs & windows::Win32::Storage::FileSystem::FILE_ATTRIBUTE_HIDDEN.0) != 0;
        is_system = (attrs & windows::Win32::Storage::FileSystem::FILE_ATTRIBUTE_SYSTEM.0) != 0;
    }

    Ok(StorableItem {
        path: path.clone(),
        metadata: FileMetadata {
            size: meta.len(),
            created: meta.created().ok(),
            modified: meta.modified().ok(),
            file_type,
            is_hidden,
            is_system,
        },
    })
}

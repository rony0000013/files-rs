use crate::core::path_util::FilesPath;
use anyhow::{anyhow, Result};
use std::path::{Path, PathBuf};

pub struct UndoRedoManager {
    history: Vec<Box<dyn UndoableAction>>,
    redo_stack: Vec<Box<dyn UndoableAction>>,
}

pub trait UndoableAction: Send + Sync {
    fn execute(&self) -> Result<()>;
    fn undo(&self) -> Result<()>;
    fn name(&self) -> &str;
}

impl UndoRedoManager {
    pub fn new() -> Self {
        Self {
            history: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    pub fn perform(&mut self, action: Box<dyn UndoableAction>) -> Result<()> {
        action.execute()?;
        self.history.push(action);
        self.redo_stack.clear();
        Ok(())
    }

    /// Records an already-executed filesystem operation (such as an async copy,
    /// move, or trash delete) so the user can undo/redo it later.
    pub fn record_executed(&mut self, action: Box<dyn UndoableAction>) {
        self.history.push(action);
        self.redo_stack.clear();
    }

    pub fn undo(&mut self) -> Result<()> {
        if let Some(mut action) = self.history.pop() {
            match action.undo() {
                Ok(()) => {
                    self.redo_stack.push(action);
                    Ok(())
                }
                Err(e) => {
                    // Push action back to history so it's not lost
                    self.history.push(action);
                    Err(e)
                }
            }
        } else {
            Err(anyhow!("Nothing to undo"))
        }
    }

    pub fn redo(&mut self) -> Result<()> {
        if let Some(mut action) = self.redo_stack.pop() {
            match action.execute() {
                Ok(()) => {
                    self.history.push(action);
                    Ok(())
                }
                Err(e) => {
                    // Push action back to redo_stack so it's not lost
                    self.redo_stack.push(action);
                    Err(e)
                }
            }
        } else {
            Err(anyhow!("Nothing to redo"))
        }
    }

    /// Pops the next undoable action WITHOUT executing it, so the caller can
    /// run the blocking filesystem work off the UI thread and commit the
    /// outcome afterwards via [`Self::commit_undo_success`] /
    /// [`Self::commit_undo_failure`]. Returns `None` when history is empty.
    pub fn pop_undo_action(&mut self) -> Option<Box<dyn UndoableAction>> {
        self.history.pop()
    }

    /// Records a background-executed undo as successful (moves it to redo).
    pub fn commit_undo_success(&mut self, action: Box<dyn UndoableAction>) {
        self.redo_stack.push(action);
    }

    /// Returns a failed background undo to history so it is not lost.
    pub fn commit_undo_failure(&mut self, action: Box<dyn UndoableAction>) {
        self.history.push(action);
    }

    /// Pops the next redoable action WITHOUT executing it; commit via
    /// [`Self::commit_redo_success`] / [`Self::commit_redo_failure`].
    /// Returns `None` when the redo stack is empty.
    pub fn pop_redo_action(&mut self) -> Option<Box<dyn UndoableAction>> {
        self.redo_stack.pop()
    }

    /// Records a background-executed redo as successful (moves it to history).
    pub fn commit_redo_success(&mut self, action: Box<dyn UndoableAction>) {
        self.history.push(action);
    }

    /// Returns a failed background redo to the redo stack so it is not lost.
    pub fn commit_redo_failure(&mut self, action: Box<dyn UndoableAction>) {
        self.redo_stack.push(action);
    }
}

// --- Concrete Actions ---

pub struct RenameAction {
    pub old_path: FilesPath,
    pub new_path: FilesPath,
}

impl UndoableAction for RenameAction {
    fn execute(&self) -> Result<()> {
        std::fs::rename(self.old_path.as_path(), self.new_path.as_path())?;
        Ok(())
    }
    fn undo(&self) -> Result<()> {
        std::fs::rename(self.new_path.as_path(), self.old_path.as_path())?;
        Ok(())
    }
    fn name(&self) -> &str {
        "Rename"
    }
}

pub struct CreateFolderAction {
    pub path: FilesPath,
}

impl UndoableAction for CreateFolderAction {
    fn execute(&self) -> Result<()> {
        std::fs::create_dir_all(self.path.as_path())?;
        Ok(())
    }
    fn undo(&self) -> Result<()> {
        std::fs::remove_dir_all(self.path.as_path())?;
        Ok(())
    }
    fn name(&self) -> &str {
        "Create Folder"
    }
}

/// Undo/Redo record for creating a new file or folder via Add Item.
pub struct CreateItemAction {
    pub path: FilesPath,
    pub is_folder: bool,
}

impl UndoableAction for CreateItemAction {
    fn execute(&self) -> Result<()> {
        let p = self.path.as_path();
        if self.is_folder {
            std::fs::create_dir_all(p)?;
        } else {
            if let Some(parent) = p.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(p)?;
        }
        Ok(())
    }

    fn undo(&self) -> Result<()> {
        let p = self.path.as_path();
        if p.is_dir() {
            std::fs::remove_dir_all(p)?;
        } else if p.symlink_metadata().is_ok() {
            std::fs::remove_file(p)?;
        }
        Ok(())
    }

    fn name(&self) -> &str {
        if self.is_folder {
            "Create Folder"
        } else {
            "Create File"
        }
    }
}

/// Undo/Redo record for Copy/Paste and copy drag-drop transfers.
/// Each pair is `(source_path, created_dest_path)`.
pub struct CopyFilesAction {
    pub pairs: Vec<(PathBuf, PathBuf)>,
}

impl UndoableAction for CopyFilesAction {
    fn execute(&self) -> Result<()> {
        for (src, dst) in &self.pairs {
            if dst.symlink_metadata().is_ok() {
                return Err(anyhow!(
                    "Destination already exists when redoing copy: {}",
                    dst.display()
                ));
            }
            copy_path_recursive(src, dst)?;
        }
        Ok(())
    }

    fn undo(&self) -> Result<()> {
        for (_, dst) in self.pairs.iter().rev() {
            remove_path_sync(dst)?;
        }
        Ok(())
    }

    fn name(&self) -> &str {
        "Copy"
    }
}

/// Undo/Redo record for Cut/Paste and move drag-drop transfers.
/// Each pair is `(original_source_path, moved_dest_path)`.
pub struct MoveFilesAction {
    pub pairs: Vec<(PathBuf, PathBuf)>,
}

impl UndoableAction for MoveFilesAction {
    fn execute(&self) -> Result<()> {
        for (src, dst) in &self.pairs {
            move_path_sync(src, dst)?;
        }
        Ok(())
    }

    fn undo(&self) -> Result<()> {
        for (src, dst) in self.pairs.iter().rev() {
            move_path_sync(dst, src)?;
        }
        Ok(())
    }

    fn name(&self) -> &str {
        "Move"
    }
}

/// Undo/Redo record for moving files/folders to the system Recycle Bin / Trash.
pub struct DeleteToTrashAction {
    pub paths: Vec<PathBuf>,
}

impl UndoableAction for DeleteToTrashAction {
    fn execute(&self) -> Result<()> {
        for p in &self.paths {
            if p.symlink_metadata().is_ok() {
                trash::delete(p).map_err(|e| anyhow!("{e}"))?;
            }
        }
        Ok(())
    }

    fn undo(&self) -> Result<()> {
        for p in self.paths.iter().rev() {
            crate::platform::windows::recycle_bin_service::RecycleBinService::restore_item_sync(
                p,
            )?;
        }
        Ok(())
    }

    fn name(&self) -> &str {
        "Delete"
    }
}

fn remove_path_sync(path: &Path) -> Result<()> {
    let Ok(meta) = path.symlink_metadata() else {
        return Ok(());
    };
    if meta.is_dir() {
        std::fs::remove_dir_all(path)?;
    } else {
        std::fs::remove_file(path)?;
    }
    Ok(())
}

fn copy_path_recursive(src: &Path, dst: &Path) -> Result<()> {
    let meta = std::fs::metadata(src)?;
    if meta.is_dir() {
        std::fs::create_dir_all(dst)?;
        for entry in std::fs::read_dir(src)? {
            let entry = entry?;
            let child_dst = dst.join(entry.file_name());
            copy_path_recursive(&entry.path(), &child_dst)?;
        }
    } else {
        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(src, dst)?;
    }
    Ok(())
}

fn move_path_sync(from: &Path, to: &Path) -> Result<()> {
    if to.symlink_metadata().is_ok() {
        return Err(anyhow!(
            "Target already exists when moving: {}",
            to.display()
        ));
    }
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent)?;
    }
    match std::fs::rename(from, to) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::CrossesDevices => {
            copy_path_recursive(from, to)?;
            remove_path_sync(from)?;
            Ok(())
        }
        Err(e) => Err(e.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestDir(PathBuf);
    impl TestDir {
        fn new(label: &str) -> Self {
            let dir = std::env::temp_dir()
                .join(format!("files_rs_{}_{}", label, uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }
    }
    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn test_undo_redo_copy_files_action() {
        let tmp = TestDir::new("undo_copy");
        let src = tmp.0.join("src_folder");
        std::fs::create_dir_all(src.join("nested")).unwrap();
        std::fs::write(src.join("nested/hello.txt"), b"world").unwrap();
        let dst = tmp.0.join("copied_folder");
        copy_path_recursive(&src, &dst).unwrap();

        let mut mgr = UndoRedoManager::new();
        mgr.record_executed(Box::new(CopyFilesAction {
            pairs: vec![(src.clone(), dst.clone())],
        }));

        assert!(dst.join("nested/hello.txt").exists());
        mgr.undo().unwrap();
        assert!(!dst.exists(), "undo Copy should remove copied destination");
        assert!(src.join("nested/hello.txt").exists(), "source remains intact");

        mgr.redo().unwrap();
        assert_eq!(
            std::fs::read(dst.join("nested/hello.txt")).unwrap(),
            b"world"
        );
    }

    #[test]
    fn test_undo_redo_move_files_action() {
        let tmp = TestDir::new("undo_move");
        let src = tmp.0.join("note.txt");
        let dst = tmp.0.join("sub/note.txt");
        std::fs::write(&src, b"payload").unwrap();
        move_path_sync(&src, &dst).unwrap();

        let mut mgr = UndoRedoManager::new();
        mgr.record_executed(Box::new(MoveFilesAction {
            pairs: vec![(src.clone(), dst.clone())],
        }));

        assert!(!src.exists() && dst.exists());
        mgr.undo().unwrap();
        assert!(src.exists() && !dst.exists());

        mgr.redo().unwrap();
        assert!(!src.exists() && dst.exists());
        assert_eq!(std::fs::read(&dst).unwrap(), b"payload");
    }

    #[test]
    fn test_undo_redo_create_item_action() {
        let tmp = TestDir::new("undo_create");
        let file_p = tmp.0.join("created.txt");
        let mut mgr = UndoRedoManager::new();
        mgr.perform(Box::new(CreateItemAction {
            path: FilesPath::from(file_p.clone()),
            is_folder: false,
        }))
        .unwrap();
        assert!(file_p.exists());
        mgr.undo().unwrap();
        assert!(!file_p.exists());
        mgr.redo().unwrap();
        assert!(file_p.exists());
    }
}

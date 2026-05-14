use crate::core::path_util::FilesPath;
use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::path::PathBuf;

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

    pub fn undo(&mut self) -> Result<()> {
        if let Some(action) = self.history.pop() {
            action.undo()?;
            self.redo_stack.push(action);
            Ok(())
        } else {
            Err(anyhow!("Nothing to undo"))
        }
    }

    pub fn redo(&mut self) -> Result<()> {
        if let Some(action) = self.redo_stack.pop() {
            action.execute()?;
            self.history.push(action);
            Ok(())
        } else {
            Err(anyhow!("Nothing to redo"))
        }
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

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

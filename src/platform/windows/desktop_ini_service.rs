use crate::core::path_util::FilesPath;
use anyhow::Result;
use std::fs;

pub struct DesktopIniService;

impl DesktopIniService {
    /// Reads the desktop.ini file for a folder to find custom icons or localized names
    pub async fn read_folder_customization(path: &FilesPath) -> Result<Option<String>> {
        let ini_path = path.as_path().join(".desktop.ini");
        if !ini_path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(ini_path)?;
        // Simplified: just return the content for now; real impl would parse [CShellClassInfo]
        Ok(Some(content))
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_read_folder_customization_nonexistent() {
        let temp_dir = TempDir::new().unwrap();
        let path = FilesPath::new(temp_dir.path().to_str().unwrap());
        let result = DesktopIniService::read_folder_customization(&path).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_read_folder_customization_with_file() {
        let temp_dir = TempDir::new().unwrap();
        let ini_path = temp_dir.path().join(".desktop.ini");

        let mut file = std::fs::File::create(&ini_path).unwrap();
        writeln!(file, "[.ShellClassInfo]").unwrap();
        writeln!(file, "IconResource=C:\\\\test.ico").unwrap();

        let path = FilesPath::new(temp_dir.path().to_str().unwrap());
        let result = DesktopIniService::read_folder_customization(&path).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_some());
    }
}

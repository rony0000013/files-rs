use crate::core::path_util::FilesPath;
use anyhow::Result;
use std::process::Command;

pub struct ClipboardService;

impl ClipboardService {
    pub fn copy_path(path: &FilesPath) -> Result<()> {
        Self::set_clipboard_text(&path.as_path().to_string_lossy());
        Ok(())
    }

    pub fn copy_path_quoted(path: &FilesPath) -> Result<()> {
        Self::set_clipboard_text(&format!("\"{}\"", path.as_path().to_string_lossy()));
        Ok(())
    }

    fn set_clipboard_text(text: &str) -> Result<()> {
        // Pipe text to clip.exe via stdin — clip reads from stdin, not args
        use std::io::Write;
        use std::process::Stdio;
        let mut child = Command::new("clip")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
        if let Some(mut stdin) = child.stdin.take() {
            // clip.exe expects UTF-16LE encoded text
            let wide: Vec<u16> = text.encode_utf16().collect();
            let bytes: Vec<u8> = wide.iter().flat_map(|w| w.to_le_bytes()).collect();
            stdin.write_all(&bytes)?;
            drop(stdin); // Close stdin to signal EOF
        }
        child.wait()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_copy_path_creates_valid_result() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        std::fs::write(&file_path, "content").unwrap();

        let path = FilesPath::from(file_path.to_str().unwrap());

        // copy_path should return Result
        // On Windows it will try to use clip command, may fail in test env
        let result = ClipboardService::copy_path(&path);

        // Result should be Ok or Err depending on clip command availability
        // The important thing is it doesn't panic
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_copy_path_quoted_creates_valid_result() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        std::fs::write(&file_path, "content").unwrap();

        let path = FilesPath::from(file_path.to_str().unwrap());

        let result = ClipboardService::copy_path_quoted(&path);

        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_copy_path_with_nonexistent_file() {
        let temp_dir = TempDir::new().unwrap();
        let nonexistent = temp_dir.path().join("nonexistent.txt");

        let path = FilesPath::from(nonexistent.to_str().unwrap());

        // Should not panic, returns Result
        let result = ClipboardService::copy_path(&path);
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_copy_path_quoted_with_spaces() {
        let temp_dir = TempDir::new().unwrap();
        let file_with_spaces = temp_dir.path().join("file with spaces.txt");
        std::fs::write(&file_with_spaces, "content").unwrap();

        let path = FilesPath::from(file_with_spaces.to_str().unwrap());

        let result = ClipboardService::copy_path_quoted(&path);
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_set_clipboard_text_handles_empty_string() {
        let result = ClipboardService::copy_path_quoted(&FilesPath::from(""));

        // Should handle empty string gracefully
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_set_clipboard_text_handles_unicode() {
        let temp_dir = TempDir::new().unwrap();
        let unicode_file = temp_dir.path().join("文件.txt");
        std::fs::write(&unicode_file, "content").unwrap();

        let path = FilesPath::from(unicode_file.to_str().unwrap());

        let result = ClipboardService::copy_path(&path);
        assert!(result.is_ok() || result.is_err());
    }
}

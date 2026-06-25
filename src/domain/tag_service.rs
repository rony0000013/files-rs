//! Enhanced Tag Service with ADS and FileFRN support
//! Provides tag persistence via SQLite with optional NTFS Alternate Data Streams.
//! Uses File Reference Numbers (FRN) for robust tracking across renames/moves.

use crate::core::path_util::FilesPath;
use anyhow::{anyhow, Result};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

/// ADS stream name for tag storage
const TAG_STREAM_NAME: &str = ":files";

/// A tag with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tag {
    pub name: String,
    pub color: String,
}

/// A tagged file entry
#[derive(Debug, Clone)]
pub struct TaggedFile {
    pub path: String,
    pub frn: Option<u64>, // File Reference Number
    pub tags: Vec<Tag>,
}

/// Enhanced Tag Service with ADS and FRN support
pub struct TagService {
    conn: Arc<Mutex<Connection>>,
    use_ads: bool, // Write tags to NTFS alternate data streams
}

impl TagService {
    /// Create a new TagService
    /// If use_ads is true, tags will also be written to NTFS alternate data streams (:files)
    /// which allows tags to persist even if the database is lost.
    pub fn new(db_path: &str, use_ads: bool) -> Result<Self> {
        let conn = Connection::open(db_path)?;

        // Create tags table for tag definitions (name, color)
        conn.execute(
            "CREATE TABLE IF NOT EXISTS tags (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL UNIQUE,
                color TEXT NOT NULL
            )",
            [],
        )?;

        // Create file_tags table for file-tag associations
        conn.execute(
            "CREATE TABLE IF NOT EXISTS file_tags (
                id INTEGER PRIMARY KEY,
                path TEXT NOT NULL,
                frn INTEGER,  -- File Reference Number for robust tracking
                tag TEXT NOT NULL,
                UNIQUE(path, tag)
            )",
            [],
        )?;

        // Create index on frn for fast lookups
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_file_tags_frn ON file_tags(frn)",
            [],
        )?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
            use_ads,
        })
    }

    /// Get the File Reference Number (FRN) for a file
    /// FRN is a unique 64-bit identifier assigned by NTFS to every file.
    #[cfg(windows)]
    pub fn get_file_frn(path: &FilesPath) -> Result<Option<u64>> {
        use std::os::windows::ffi::OsStrExt;
        use windows::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
        use windows::Win32::Storage::FileSystem::{
            CreateFileW, GetFileInformationByHandle, FILE_BASIC_INFO, FILE_FLAGS_AND_ATTRIBUTES,
            FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_MODE,
            FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
        };

        let wide_path: Vec<u16> = path
            .as_path()
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        unsafe {
            let handle = CreateFileW(
                windows::core::PCWSTR(wide_path.as_ptr()),
                0u32, // dwDesiredAccess - 0 means query access
                FILE_SHARE_MODE(FILE_SHARE_READ.0 | FILE_SHARE_WRITE.0), // dwShareMode
                std::option::Option::None, // lpSecurityAttributes
                OPEN_EXISTING, // dwCreationDisposition
                FILE_FLAGS_AND_ATTRIBUTES(FILE_FLAG_BACKUP_SEMANTICS.0), // dwFlagsAndAttributes
                None, // hTemplateFile
            );

            let handle = match handle {
                Ok(h) => h,
                Err(_) => return Ok(None),
            };

            // Use GetFileInformationByHandleEx with FileIdInfo to get the actual
            // file reference number (FRN). FILE_BASIC_INFO does NOT contain the FRN —
            // it only has timestamps and FileAttributes. The FRN is in FILE_ID_INFO.FileId.
            use windows::Win32::Storage::FileSystem::{FileIdInfo, GetFileInformationByHandleEx};
            use windows::Win32::Storage::FileSystem::FILE_ID_INFO;

            let mut file_id_info: FILE_ID_INFO = std::mem::zeroed();
            let result = GetFileInformationByHandleEx(
                handle,
                FileIdInfo,
                &mut file_id_info as *mut _ as *mut _,
                std::mem::size_of::<FILE_ID_INFO>() as u32,
            );

            let _ = CloseHandle(handle);

            if result.is_ok() {
                // FILE_ID_INFO.FileId is a 128-bit FILE_ID_128 (16 bytes).
                // Take the lower 64 bits as a compact FRN proxy for tag deduplication.
                let file_id_bytes = file_id_info.FileId.Identifier;
                let frn = u64::from_ne_bytes([
                    file_id_bytes[0], file_id_bytes[1], file_id_bytes[2], file_id_bytes[3],
                    file_id_bytes[4], file_id_bytes[5], file_id_bytes[6], file_id_bytes[7],
                ]);
                Ok(Some(frn))
            } else {
                Ok(None)
            }
        }
    }

    #[cfg(not(windows))]
    pub fn get_file_frn(_path: &FilesPath) -> Result<Option<u64>> {
        Ok(None)
    }

    /// Read tags from NTFS Alternate Data Stream
    /// Reads the ":files" stream to get portable tag data.
    #[cfg(windows)]
    pub fn read_tags_from_ads(path: &FilesPath) -> Result<Vec<String>> {
        use std::fs;
        use std::io::{BufRead, BufReader};

        let ads_path = format!("{}:files", path.as_path().to_string_lossy());
        let tags_path = std::path::Path::new(&ads_path);

        if !tags_path.exists() {
            return Ok(Vec::new());
        }

        let file = fs::File::open(tags_path)?;
        let reader = BufReader::new(file);
        let tags: Vec<String> = reader.lines().filter_map(|line| line.ok()).collect();

        Ok(tags)
    }

    #[cfg(not(windows))]
    pub fn read_tags_from_ads(_path: &FilesPath) -> Result<Vec<String>> {
        Ok(Vec::new())
    }

    /// Write tags to NTFS Alternate Data Stream
    /// Writes to ":files" stream for portable tag storage.
    #[cfg(windows)]
    pub fn write_tags_to_ads(path: &FilesPath, tags: &[String]) -> Result<()> {
        use std::fs;
        use std::io::Write;

        let ads_path = format!("{}:files", path.as_path().to_string_lossy());
        let file = fs::File::create(&ads_path)?;
        let mut writer = std::io::BufWriter::new(file);

        for tag in tags {
            writeln!(writer, "{}", tag)?;
        }

        writer.flush()?;
        Ok(())
    }

    #[cfg(not(windows))]
    pub fn write_tags_to_ads(_path: &FilesPath, _tags: &[String]) -> Result<()> {
        Ok(())
    }

    /// Sync tags from ADS to database
    /// Useful for recovering tags if the database is lost.
    pub fn sync_tags_from_ads(&self, path: &FilesPath) -> Result<()> {
        if !self.use_ads {
            return Ok(());
        }

        let ads_tags = Self::read_tags_from_ads(path)?;
        if ads_tags.is_empty() {
            return Ok(());
        }

        let conn = self.conn.lock().unwrap();
        for tag in ads_tags {
            conn.execute(
                "INSERT OR IGNORE INTO file_tags (path, tag) VALUES (?1, ?2)",
                params![path.as_path().to_string_lossy(), tag],
            )?;
        }
        Ok(())
    }

    /// Create a new tag definition
    pub fn create_tag(&self, name: &str, color: &str) -> Result<String> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR IGNORE INTO tags (name, color) VALUES (?1, ?2)",
            params![name, color],
        )?;
        conn.execute(
            "UPDATE tags SET color = ?2 WHERE name = ?1",
            params![name, color],
        )?;
        Ok(name.to_string())
    }

    /// Delete a tag definition and all its file associations
    pub fn delete_tag(&self, name: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM file_tags WHERE tag = ?1", params![name])?;
        conn.execute("DELETE FROM tags WHERE name = ?1", params![name])?;
        Ok(())
    }

    /// Rename a tag (updates all file associations)
    pub fn rename_tag(&self, old_name: &str, new_name: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let exists: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM tags WHERE name = ?1)",
            params![new_name],
            |row| row.get(0),
        )?;

        if exists {
            conn.execute(
                "UPDATE OR IGNORE file_tags SET tag = ?1 WHERE tag = ?2",
                params![new_name, old_name],
            )?;
            conn.execute("DELETE FROM file_tags WHERE tag = ?1", params![old_name])?;
            conn.execute("DELETE FROM tags WHERE name = ?1", params![old_name])?;
        } else {
            conn.execute(
                "UPDATE tags SET name = ?1 WHERE name = ?2",
                params![new_name, old_name],
            )?;
            conn.execute(
                "UPDATE file_tags SET tag = ?1 WHERE tag = ?2",
                params![new_name, old_name],
            )?;
        }
        Ok(())
    }

    /// Update tag color
    pub fn update_tag_color(&self, name: &str, color: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE tags SET color = ?2 WHERE name = ?1",
            params![name, color],
        )?;
        Ok(())
    }

    /// Add a tag to a file
    pub fn add_tag(&self, path: &FilesPath, tag: &str, color: &str) -> Result<()> {
        // Get FRN for robust tracking
        let frn_opt = Self::get_file_frn(path).ok().flatten().map(|v| v as i64);

        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR IGNORE INTO tags (name, color) VALUES (?1, ?2)",
            params![tag, color],
        )?;
        conn.execute(
            "INSERT OR IGNORE INTO file_tags (path, frn, tag) VALUES (?1, ?2, ?3)",
            params![path.as_path().to_string_lossy(), frn_opt, tag],
        )?;
        drop(conn);

        // Also write to ADS if enabled
        if self.use_ads {
            let current_tags = self.get_tags(path)?;
            let tag_names: Vec<String> = current_tags.into_iter().map(|(n, _)| n).collect();
            Self::write_tags_to_ads(path, &tag_names)?;
        }

        Ok(())
    }

    /// Remove a tag from a file
    pub fn remove_tag(&self, path: &FilesPath, tag: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM file_tags WHERE path = ?1 AND tag = ?2",
            params![path.as_path().to_string_lossy(), tag],
        )?;
        conn.execute(
            "DELETE FROM tags WHERE name = ?1 AND NOT EXISTS (SELECT 1 FROM file_tags WHERE tag = ?1)",
            params![tag],
        )?;
        drop(conn);

        // Also update ADS if enabled
        if self.use_ads {
            let current_tags = self.get_tags(path)?;
            let tag_names: Vec<String> = current_tags.into_iter().map(|(n, _)| n).collect();
            Self::write_tags_to_ads(path, &tag_names)?;
        }

        Ok(())
    }

    /// Get all tags for a file
    pub fn get_tags(&self, path: &FilesPath) -> Result<Vec<(String, String)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT ft.tag, COALESCE(t.color, '#808080') as color
             FROM file_tags ft
             LEFT JOIN tags t ON ft.tag = t.name
             WHERE ft.path = ?1
             ORDER BY ft.tag",
        )?;
        let tags = stmt
            .query_map(params![path.as_path().to_string_lossy()], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(tags)
    }

    /// Get all tags for a file by FRN (useful after file moves)
    pub fn get_tags_by_frn(&self, frn: u64) -> Result<Vec<(String, String)>> {
        let frn_i64 = frn as i64;
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT ft.tag, COALESCE(t.color, '#808080') as color
             FROM file_tags ft
             LEFT JOIN tags t ON ft.tag = t.name
             WHERE ft.frn = ?1
             ORDER BY ft.tag",
        )?;
        let tags = stmt
            .query_map(params![frn_i64], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(tags)
    }

    /// Get all unique tags with their colors and file counts
    pub fn get_all_tags(&self) -> Result<Vec<(String, String, i64)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT t.name, t.color, COUNT(ft.id) as count
             FROM tags t
             LEFT JOIN file_tags ft ON t.name = ft.tag
             GROUP BY t.name
             ORDER BY count DESC, t.name ASC",
        )?;
        let tags = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(tags)
    }

    /// Get all files with a specific tag
    pub fn get_files_with_tag(&self, tag: &str) -> Result<Vec<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT path FROM file_tags WHERE tag = ?1")?;
        let paths = stmt
            .query_map(params![tag], |row| row.get(0))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(paths)
    }

    /// Resolve a file path by FRN (handles file renames/moves)
    pub fn resolve_path_by_frn(&self, frn: u64) -> Result<Option<String>> {
        let frn_i64 = frn as i64;
        let conn = self.conn.lock().unwrap();
        let path: Option<String> = conn
            .query_row(
                "SELECT path FROM file_tags WHERE frn = ?1 LIMIT 1",
                params![frn_i64],
                |row| row.get(0),
            )
            .ok();
        Ok(path)
    }

    /// Update FRN for a file (call after operations that may change FRN)
    pub fn update_file_frn(&self, path: &FilesPath) -> Result<()> {
        let frn_opt = Self::get_file_frn(path).ok().flatten().map(|v| v as i64);
        let path_str = path.as_path().to_string_lossy();

        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE file_tags SET frn = ?1 WHERE path = ?2",
            params![frn_opt, path_str],
        )?;
        Ok(())
    }

    /// Export all tags to ADS for all tagged files (backup)
    pub fn export_all_tags_to_ads(&self) -> Result<usize> {
        if !self.use_ads {
            return Ok(0);
        }

        let paths: Vec<String> = {
            let conn = self.conn.lock().unwrap();
            let mut stmt = conn.prepare("SELECT DISTINCT path FROM file_tags")?;
            let mut rows = stmt.query([])?;
            let mut paths = Vec::new();
            while let Ok(Some(row)) = rows.next().map(|r| r) {
                // Use a block to ensure we get an owned String
                let path_str: String = row.get(0)?;
                paths.push(path_str);
            }
            paths
        };

        let mut count = 0;
        for path_str in paths {
            let path = FilesPath::from(path_str.as_str());
            let tags = self.get_tags(&path)?;
            if !tags.is_empty() {
                let tag_names: Vec<String> = tags.into_iter().map(|(n, _)| n).collect();
                if Self::write_tags_to_ads(&path, &tag_names).is_ok() {
                    count += 1;
                }
            }
        }

        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tag_service_creation() {
        // This would need a real temp directory
        let service = TagService::new(":memory:", false);
        assert!(service.is_ok());
    }

    #[test]
    fn test_create_and_get_tags() {
        let service = TagService::new(":memory:", false).unwrap();
        let path = FilesPath::from("/tmp/test.txt");

        service.create_tag("work", "#FF0000").unwrap();
        service.add_tag(&path, "work", "#FF0000").unwrap();

        let tags = service.get_tags(&path).unwrap();
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].0, "work");
    }

    #[test]
    fn test_ads_read_write() {
        // ADS operations are Windows-specific
        #[cfg(windows)]
        {
            let path = FilesPath::from("C:\\test.txt");
            let tags = vec!["tag1".to_string(), "tag2".to_string()];
            TagService::write_tags_to_ads(&path, &tags).unwrap();
            let read_tags = TagService::read_tags_from_ads(&path).unwrap();
            assert_eq!(read_tags, tags);
        }
    }

    #[test]
    fn test_get_file_frn() {
        // FRN is Windows-specific
        #[cfg(windows)]
        {
            let path = FilesPath::from("C:\\Windows");
            let frn = TagService::get_file_frn(&path);
            // May be None for system files without admin privileges
            assert!(frn.is_ok());
        }
    }
}

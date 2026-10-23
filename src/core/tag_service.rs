use crate::core::database::Database;
use rusqlite::params;
use std::sync::Arc;

pub struct TagService {
    db: Arc<Database>,
}

impl TagService {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    pub fn add_tag(&self, path: &str, tag: &str) -> anyhow::Result<()> {
        let conn = self.db.get_conn();
        let conn = conn
            .lock()
            .map_err(|e| anyhow::anyhow!("Lock error: {}", e))?;
        conn.execute(
            "INSERT OR IGNORE INTO file_tags (path, tag) VALUES (?1, ?2)",
            params![path, tag],
        )?;
        Ok(())
    }

    pub fn remove_tag(&self, path: &str, tag: &str) -> anyhow::Result<()> {
        let conn = self.db.get_conn();
        let conn = conn
            .lock()
            .map_err(|e| anyhow::anyhow!("Lock error: {}", e))?;
        conn.execute(
            "DELETE FROM file_tags WHERE path = ?1 AND tag = ?2",
            params![path, tag],
        )?;
        Ok(())
    }

    pub fn get_tags(&self, path: &str) -> anyhow::Result<Vec<String>> {
        let conn = self.db.get_conn();
        let conn = conn
            .lock()
            .map_err(|e| anyhow::anyhow!("Lock error: {}", e))?;
        let mut stmt = conn.prepare("SELECT tag FROM file_tags WHERE path = ?1")?;
        let tags = stmt
            .query_map(params![path], |row| row.get(0))?
            .collect::<Result<Vec<String>, _>>()?;
        Ok(tags)
    }

    pub fn get_files_with_tag(&self, tag: &str) -> anyhow::Result<Vec<String>> {
        let conn = self.db.get_conn();
        let conn = conn
            .lock()
            .map_err(|e| anyhow::anyhow!("Lock error: {}", e))?;
        let mut stmt = conn.prepare("SELECT path FROM file_tags WHERE tag = ?1")?;
        let paths = stmt
            .query_map(params![tag], |row| row.get(0))?
            .collect::<Result<Vec<String>, _>>()?;
        Ok(paths)
    }

    pub fn delete_tag(&self, tag: &str) -> anyhow::Result<()> {
        let conn = self.db.get_conn();
        let conn = conn
            .lock()
            .map_err(|e| anyhow::anyhow!("Lock error: {}", e))?;
        conn.execute("DELETE FROM file_tags WHERE tag = ?1", params![tag])?;
        Ok(())
    }

    pub fn rename_tag(&self, old_tag: &str, new_tag: &str) -> anyhow::Result<()> {
        let conn = self.db.get_conn();
        let conn = conn
            .lock()
            .map_err(|e| anyhow::anyhow!("Lock error: {}", e))?;
        conn.execute(
            "UPDATE OR IGNORE file_tags SET tag = ?2 WHERE tag = ?1",
            params![old_tag, new_tag],
        )?;
        Ok(())
    }

    pub fn get_all_tags(&self) -> anyhow::Result<Vec<(String, i64)>> {
        let conn = self.db.get_conn();
        let conn = conn
            .lock()
            .map_err(|e| anyhow::anyhow!("Lock error: {}", e))?;
        let mut stmt = conn.prepare(
            "SELECT tag, COUNT(*) as count FROM file_tags GROUP BY tag ORDER BY count DESC, tag ASC"
        )?;
        let tags = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<Result<Vec<(String, i64)>, _>>()?;
        Ok(tags)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn create_test_db() -> Arc<Database> {
        // Use in-memory SQLite for tests
        let db = Database::new(PathBuf::from(":memory:")).unwrap();
        Arc::new(db)
    }

    #[test]
    fn test_get_files_with_tag() {
        let db = create_test_db();
        let service = TagService::new(db);

        // Add tags to files
        service.add_tag("/path/to/file1.txt", "work").unwrap();
        service.add_tag("/path/to/file2.txt", "work").unwrap();
        service.add_tag("/path/to/file3.txt", "personal").unwrap();

        // Verify count for "work" tag
        let work_files = service.get_files_with_tag("work").unwrap();
        assert_eq!(work_files.len(), 2);
        assert!(work_files.contains(&"/path/to/file1.txt".to_string()));
        assert!(work_files.contains(&"/path/to/file2.txt".to_string()));

        // Verify count for "personal" tag
        let personal_files = service.get_files_with_tag("personal").unwrap();
        assert_eq!(personal_files.len(), 1);
        assert!(personal_files.contains(&"/path/to/file3.txt".to_string()));

        // Verify non-existent tag returns empty
        let nonexistent = service.get_files_with_tag("nonexistent").unwrap();
        assert!(nonexistent.is_empty());
    }

    #[test]
    fn test_get_all_tags() {
        let db = create_test_db();
        let service = TagService::new(db);

        // Add tags to multiple files
        service.add_tag("/path/to/file1.txt", "work").unwrap();
        service.add_tag("/path/to/file2.txt", "work").unwrap();
        service.add_tag("/path/to/file3.txt", "personal").unwrap();
        service.add_tag("/path/to/file4.txt", "archive").unwrap();
        service.add_tag("/path/to/file2.txt", "personal").unwrap();

        let all_tags = service.get_all_tags().unwrap();

        // Should have 3 unique tags
        assert_eq!(all_tags.len(), 3);

        // Verify each tag name and count
        let mut work_found = false;
        let mut personal_found = false;
        let mut archive_found = false;
        for (tag, count) in &all_tags {
            match tag.as_str() {
                "work" => {
                    assert_eq!(*count, 2);
                    work_found = true;
                }
                "personal" => {
                    assert_eq!(*count, 2);
                    personal_found = true;
                }
                "archive" => {
                    assert_eq!(*count, 1);
                    archive_found = true;
                }
                _ => {}
            }
        }
        assert!(work_found, "work tag not found");
        assert!(personal_found, "personal tag not found");
        assert!(archive_found, "archive tag not found");
    }

    #[test]
    fn test_get_all_tags_empty() {
        let db = create_test_db();
        let service = TagService::new(db);

        // No tags added
        let all_tags = service.get_all_tags().unwrap();
        assert!(all_tags.is_empty());
    }

    #[test]
    fn test_tag_state_all_selected_have_tag() {
        let db = create_test_db();
        let service = TagService::new(db);

        // Add tag "work" to all files
        service.add_tag("/path/a.txt", "work").unwrap();
        service.add_tag("/path/b.txt", "work").unwrap();
        service.add_tag("/path/c.txt", "work").unwrap();

        // Each file should have the tag
        let tags_a = service.get_tags("/path/a.txt").unwrap();
        let tags_b = service.get_tags("/path/b.txt").unwrap();
        let tags_c = service.get_tags("/path/c.txt").unwrap();

        assert!(tags_a.contains(&"work".to_string()));
        assert!(tags_b.contains(&"work".to_string()));
        assert!(tags_c.contains(&"work".to_string()));

        // Simulate All state: all selected files have the tag
        let selected_tags = vec![
            (0, tags_a.clone()),
            (1, tags_b.clone()),
            (2, tags_c.clone()),
        ];
        let total = selected_tags.len();
        let tagged_count = selected_tags
            .iter()
            .filter(|(_, tags)| tags.contains(&"work".to_string()))
            .count();
        assert_eq!(tagged_count, total, "All items should have 'work' tag");
    }

    #[test]
    fn test_tag_state_some_selected_have_tag() {
        let db = create_test_db();
        let service = TagService::new(db);

        // Add tag "urgent" to only some files
        service.add_tag("/path/a.txt", "urgent").unwrap();
        service.add_tag("/path/c.txt", "urgent").unwrap();
        // /path/b.txt does NOT get "urgent"

        let tags_a = service.get_tags("/path/a.txt").unwrap();
        let tags_b = service.get_tags("/path/b.txt").unwrap();
        let tags_c = service.get_tags("/path/c.txt").unwrap();

        assert!(tags_a.contains(&"urgent".to_string()));
        assert!(!tags_b.contains(&"urgent".to_string()));
        assert!(tags_c.contains(&"urgent".to_string()));

        // Simulate Some state: only some selected files have the tag
        let selected_tags = vec![(0, tags_a), (1, tags_b), (2, tags_c)];
        let total = selected_tags.len();
        let tagged_count = selected_tags
            .iter()
            .filter(|(_, tags)| tags.contains(&"urgent".to_string()))
            .count();
        assert!(
            tagged_count > 0 && tagged_count < total,
            "Some (but not all) items should have 'urgent' tag"
        );
    }

    #[test]
    fn test_tag_state_none_selected_have_tag() {
        let db = create_test_db();
        let service = TagService::new(db);

        // Add tags but NOT "archive" to any
        service.add_tag("/path/a.txt", "work").unwrap();
        service.add_tag("/path/b.txt", "personal").unwrap();

        let tags_a = service.get_tags("/path/a.txt").unwrap();
        let tags_b = service.get_tags("/path/b.txt").unwrap();
        let tags_c = service.get_tags("/path/c.txt").unwrap();

        // None of the selected files should have "archive"
        let selected_tags = vec![(0, tags_a), (1, tags_b), (2, tags_c)];
        let tagged_count = selected_tags
            .iter()
            .filter(|(_, tags)| tags.contains(&"archive".to_string()))
            .count();
        assert_eq!(tagged_count, 0, "No items should have 'archive' tag");
    }

    #[test]
    fn test_get_common_tags_works_correctly() {
        let db = create_test_db();
        let service = TagService::new(db);

        // Setup: 3 files with overlapping tags
        service.add_tag("/path/a.txt", "work").unwrap();
        service.add_tag("/path/a.txt", "urgent").unwrap();
        service.add_tag("/path/b.txt", "work").unwrap();
        service.add_tag("/path/b.txt", "personal").unwrap();
        service.add_tag("/path/c.txt", "work").unwrap();
        service.add_tag("/path/c.txt", "personal").unwrap();

        // Get all tags from the service
        let all_tags = service.get_all_tags().unwrap();

        // We expect: work(3), personal(2), urgent(1)
        assert_eq!(all_tags.len(), 3);

        // Helper to find count by name
        let count_for = |name: &str| -> i64 {
            all_tags
                .iter()
                .find(|(t, _)| t == name)
                .map(|(_, c)| *c)
                .unwrap_or(0)
        };
        assert_eq!(count_for("work"), 3);
        assert_eq!(count_for("personal"), 2);
        assert_eq!(count_for("urgent"), 1);

        // Now simulate get_common_tags_for_selected logic:
        // Selected files: a.txt, b.txt
        let tags_a = service.get_tags("/path/a.txt").unwrap(); // [work, urgent]
        let tags_b = service.get_tags("/path/b.txt").unwrap(); // [work, personal]

        let selected_tags: Vec<(usize, Vec<String>)> = vec![(0, tags_a), (1, tags_b)];
        let total_selected = selected_tags.len();

        for (tag_name, _count) in &all_tags {
            let tagged_count = selected_tags
                .iter()
                .filter(|(_, tags)| tags.contains(tag_name))
                .count();

            match tag_name.as_str() {
                "work" => assert_eq!(
                    tagged_count, total_selected,
                    "work should be on ALL selected"
                ),
                "personal" => assert!(
                    tagged_count > 0 && tagged_count < total_selected,
                    "personal should be on SOME selected (only b.txt)"
                ),
                "urgent" => assert!(
                    tagged_count > 0 && tagged_count < total_selected,
                    "urgent should be on SOME selected (only a.txt)"
                ),
                _ => panic!("Unexpected tag: {}", tag_name),
            }
        }
    }
}

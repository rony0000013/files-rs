use rusqlite::{Connection, Result};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub struct Database {
    conn: Arc<Mutex<Connection>>,
}

impl Database {
    pub fn new(path: PathBuf) -> Result<Self> {
        let conn = Connection::open(path)?;
        
        // Initialize tables
        conn.execute(
            "CREATE TABLE IF NOT EXISTS file_tags (
                path TEXT NOT NULL,
                tag TEXT NOT NULL,
                PRIMARY KEY (path, tag)
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS custom_data (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            )",
            [],
        )?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn get_conn(&self) -> Arc<Mutex<Connection>> {
        self.conn.clone()
    }
}

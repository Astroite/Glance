pub mod dao;
pub mod schema;

use rusqlite::{Connection, Result as SqlResult};
use std::path::{Path, PathBuf};

/// Get the application data directory (%APPDATA%/Glance/)
pub fn app_data_dir() -> PathBuf {
    let data_dir = dirs::data_dir().expect("Failed to get app data directory");
    data_dir.join("Glance")
}

/// Get the database file path
pub fn db_path() -> PathBuf {
    app_data_dir().join("index.sqlite")
}

/// Get the thumbnails directory path
pub fn thumbs_dir() -> PathBuf {
    app_data_dir().join("thumbs")
}

/// Get the logs directory path
pub fn logs_dir() -> PathBuf {
    app_data_dir().join("logs")
}

/// Initialize the application data directory structure
pub fn init_app_data_dir() -> Result<(), std::io::Error> {
    let base = app_data_dir();
    std::fs::create_dir_all(&base)?;
    std::fs::create_dir_all(thumbs_dir().join("240"))?;
    std::fs::create_dir_all(thumbs_dir().join("480"))?;
    std::fs::create_dir_all(thumbs_dir().join("1080"))?;
    std::fs::create_dir_all(logs_dir())?;
    Ok(())
}

/// Create a new SQLite connection with WAL mode enabled
pub fn create_connection(db_path: &Path) -> SqlResult<Connection> {
    let conn = Connection::open(db_path)?;
    conn.execute_batch("PRAGMA journal_mode=WAL;")?;
    conn.execute_batch("PRAGMA foreign_keys=ON;")?;
    Ok(conn)
}

/// Run all pending migrations
pub fn run_migrations(conn: &Connection) -> SqlResult<()> {
    schema::migrate(conn)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn test_db() -> (TempDir, Connection) {
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("test.sqlite");
        let conn = create_connection(&db_path).unwrap();
        (tmp, conn)
    }

    #[test]
    fn test_wal_mode_enabled() {
        let (_tmp, conn) = test_db();
        let mode: String = conn
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))
            .unwrap();
        assert_eq!(mode, "wal");
    }

    #[test]
    fn test_migrations_create_tables() {
        let (_tmp, conn) = test_db();
        run_migrations(&conn).unwrap();

        // Verify tables exist
        let tables: Vec<String> = {
            let mut stmt = conn
                .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
                .unwrap();
            stmt.query_map([], |row| row.get(0))
                .unwrap()
                .collect::<Result<Vec<String>, _>>()
                .unwrap()
        };

        assert!(tables.contains(&"libraries".to_string()));
        assert!(tables.contains(&"photos".to_string()));
        assert!(tables.contains(&"photo_files".to_string()));
        assert!(tables.contains(&"thumbnails".to_string()));
        assert!(tables.contains(&"scan_jobs".to_string()));
    }

    #[test]
    fn test_migrations_idempotent() {
        let (_tmp, conn) = test_db();
        // Run migrations twice - should not error
        run_migrations(&conn).unwrap();
        run_migrations(&conn).unwrap();
    }

    #[test]
    fn test_indexes_created() {
        let (_tmp, conn) = test_db();
        run_migrations(&conn).unwrap();

        let indexes: Vec<String> = {
            let mut stmt = conn
                .prepare("SELECT name FROM sqlite_master WHERE type='index' AND name NOT LIKE 'sqlite_%' ORDER BY name")
                .unwrap();
            stmt.query_map([], |row| row.get(0))
                .unwrap()
                .collect::<Result<Vec<String>, _>>()
                .unwrap()
        };

        assert!(indexes.iter().any(|i| i.contains("photos_timeline")));
        assert!(indexes.iter().any(|i| i.contains("photo_files_photo")));
    }

    #[test]
    fn test_app_data_dir_paths() {
        let base = app_data_dir();
        assert!(base.ends_with("Glance"));
        assert!(thumbs_dir().ends_with("Glance/thumbs"));
        assert!(logs_dir().ends_with("Glance/logs"));
    }
}

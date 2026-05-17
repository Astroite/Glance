use rusqlite::{Connection, Result as SqlResult, params};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Library {
    pub id: i64,
    pub name: String,
    pub root_path: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Photo {
    pub id: i64,
    pub library_id: i64,
    pub taken_at: Option<i64>,
    pub taken_at_src: Option<String>,
    pub camera_make: Option<String>,
    pub camera_model: Option<String>,
    pub lens: Option<String>,
    pub focal_len: Option<f64>,
    pub aperture: Option<f64>,
    pub shutter: Option<f64>,
    pub iso: Option<i64>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub orientation: Option<i64>,
    pub gps_lat: Option<f64>,
    pub gps_lon: Option<f64>,
    pub rating: Option<i64>,
    pub label: Option<String>,
    pub format: String,
    pub display_file_id: Option<i64>,
    pub indexed_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhotoFile {
    pub id: i64,
    pub photo_id: i64,
    pub path: String,
    pub content_hash: String,
    pub file_size: i64,
    pub mtime: i64,
    pub role: String,
    pub status: String,
    pub last_seen_at: i64,
    pub last_scan_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Thumbnail {
    pub photo_id: i64,
    pub source_file_id: i64,
    pub source_hash: String,
    pub tier: i64,
    pub width: i64,
    pub height: i64,
    pub generated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanJob {
    pub id: i64,
    pub library_id: i64,
    pub status: String,
    pub cursor: Option<String>,
    pub started_at: i64,
    pub finished_at: Option<i64>,
    pub added: i64,
    pub updated: i64,
    pub missing: i64,
}

/// Insert a new library
pub fn insert_library(conn: &Connection, name: &str, root_path: &str) -> SqlResult<Library> {
    conn.execute(
        "INSERT INTO libraries (name, root_path) VALUES (?1, ?2)",
        params![name, root_path],
    )?;
    let id = conn.last_insert_rowid();
    get_library(conn, id)
}

/// Get a library by ID
pub fn get_library(conn: &Connection, id: i64) -> SqlResult<Library> {
    conn.query_row(
        "SELECT id, name, root_path, created_at FROM libraries WHERE id = ?1",
        [id],
        |row| {
            Ok(Library {
                id: row.get(0)?,
                name: row.get(1)?,
                root_path: row.get(2)?,
                created_at: row.get(3)?,
            })
        },
    )
}

/// List all libraries
pub fn list_libraries(conn: &Connection) -> SqlResult<Vec<Library>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, root_path, created_at FROM libraries ORDER BY created_at DESC",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(Library {
            id: row.get(0)?,
            name: row.get(1)?,
            root_path: row.get(2)?,
            created_at: row.get(3)?,
        })
    })?;
    rows.collect()
}

/// Insert a new photo
pub fn insert_photo(
    conn: &Connection,
    library_id: i64,
    format: &str,
    taken_at: Option<i64>,
    taken_at_src: Option<&str>,
) -> SqlResult<Photo> {
    conn.execute(
        "INSERT INTO photos (library_id, format, taken_at, taken_at_src) VALUES (?1, ?2, ?3, ?4)",
        params![library_id, format, taken_at, taken_at_src],
    )?;
    let id = conn.last_insert_rowid();
    get_photo(conn, id)
}

/// Get a photo by ID
pub fn get_photo(conn: &Connection, id: i64) -> SqlResult<Photo> {
    conn.query_row(
        "SELECT id, library_id, taken_at, taken_at_src, camera_make, camera_model,
         lens, focal_len, aperture, shutter, iso, width, height, orientation,
         gps_lat, gps_lon, rating, label, format, display_file_id, indexed_at
         FROM photos WHERE id = ?1",
        [id],
        |row| {
            Ok(Photo {
                id: row.get(0)?,
                library_id: row.get(1)?,
                taken_at: row.get(2)?,
                taken_at_src: row.get(3)?,
                camera_make: row.get(4)?,
                camera_model: row.get(5)?,
                lens: row.get(6)?,
                focal_len: row.get(7)?,
                aperture: row.get(8)?,
                shutter: row.get(9)?,
                iso: row.get(10)?,
                width: row.get(11)?,
                height: row.get(12)?,
                orientation: row.get(13)?,
                gps_lat: row.get(14)?,
                gps_lon: row.get(15)?,
                rating: row.get(16)?,
                label: row.get(17)?,
                format: row.get(18)?,
                display_file_id: row.get(19)?,
                indexed_at: row.get(20)?,
            })
        },
    )
}

/// Insert a photo file
pub fn insert_photo_file(
    conn: &Connection,
    photo_id: i64,
    path: &str,
    content_hash: &str,
    file_size: i64,
    mtime: i64,
    role: &str,
    last_scan_id: Option<i64>,
) -> SqlResult<PhotoFile> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    conn.execute(
        "INSERT INTO photo_files (photo_id, path, content_hash, file_size, mtime, role, last_seen_at, last_scan_id)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![photo_id, path, content_hash, file_size, mtime, role, now, last_scan_id],
    )?;
    let id = conn.last_insert_rowid();
    get_photo_file(conn, id)
}

/// Get a photo file by ID
pub fn get_photo_file(conn: &Connection, id: i64) -> SqlResult<PhotoFile> {
    conn.query_row(
        "SELECT id, photo_id, path, content_hash, file_size, mtime, role, status, last_seen_at, last_scan_id
         FROM photo_files WHERE id = ?1",
        [id],
        |row| {
            Ok(PhotoFile {
                id: row.get(0)?,
                photo_id: row.get(1)?,
                path: row.get(2)?,
                content_hash: row.get(3)?,
                file_size: row.get(4)?,
                mtime: row.get(5)?,
                role: row.get(6)?,
                status: row.get(7)?,
                last_seen_at: row.get(8)?,
                last_scan_id: row.get(9)?,
            })
        },
    )
}

/// Find photo file by path
pub fn find_photo_file_by_path(conn: &Connection, path: &str) -> SqlResult<Option<PhotoFile>> {
    let mut stmt = conn.prepare(
        "SELECT id, photo_id, path, content_hash, file_size, mtime, role, status, last_seen_at, last_scan_id
         FROM photo_files WHERE path = ?1",
    )?;
    let mut rows = stmt.query_map([path], |row| {
        Ok(PhotoFile {
            id: row.get(0)?,
            photo_id: row.get(1)?,
            path: row.get(2)?,
            content_hash: row.get(3)?,
            file_size: row.get(4)?,
            mtime: row.get(5)?,
            role: row.get(6)?,
            status: row.get(7)?,
            last_seen_at: row.get(8)?,
            last_scan_id: row.get(9)?,
        })
    })?;
    match rows.next() {
        Some(row) => Ok(Some(row?)),
        None => Ok(None),
    }
}

/// Find photo files by identity (content_hash + file_size)
pub fn find_photo_files_by_identity(
    conn: &Connection,
    content_hash: &str,
    file_size: i64,
) -> SqlResult<Vec<PhotoFile>> {
    let mut stmt = conn.prepare(
        "SELECT id, photo_id, path, content_hash, file_size, mtime, role, status, last_seen_at, last_scan_id
         FROM photo_files WHERE content_hash = ?1 AND file_size = ?2",
    )?;
    let rows = stmt.query_map(params![content_hash, file_size], |row| {
        Ok(PhotoFile {
            id: row.get(0)?,
            photo_id: row.get(1)?,
            path: row.get(2)?,
            content_hash: row.get(3)?,
            file_size: row.get(4)?,
            mtime: row.get(5)?,
            role: row.get(6)?,
            status: row.get(7)?,
            last_seen_at: row.get(8)?,
            last_scan_id: row.get(9)?,
        })
    })?;
    rows.collect()
}

/// Update photo file status
pub fn update_photo_file_status(conn: &Connection, id: i64, status: &str) -> SqlResult<()> {
    conn.execute(
        "UPDATE photo_files SET status = ?1 WHERE id = ?2",
        params![status, id],
    )?;
    Ok(())
}

/// Update photo file last_seen_at
pub fn update_photo_file_last_seen(conn: &Connection, id: i64, last_scan_id: Option<i64>) -> SqlResult<()> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    conn.execute(
        "UPDATE photo_files SET last_seen_at = ?1, last_scan_id = ?2 WHERE id = ?3",
        params![now, last_scan_id, id],
    )?;
    Ok(())
}

/// Insert thumbnail metadata
pub fn insert_thumbnail(
    conn: &Connection,
    photo_id: i64,
    source_file_id: i64,
    source_hash: &str,
    tier: i64,
    width: i64,
    height: i64,
) -> SqlResult<Thumbnail> {
    conn.execute(
        "INSERT OR REPLACE INTO thumbnails (photo_id, source_file_id, source_hash, tier, width, height)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![photo_id, source_file_id, source_hash, tier, width, height],
    )?;
    get_thumbnail(conn, photo_id, tier)
}

/// Get thumbnail metadata
pub fn get_thumbnail(conn: &Connection, photo_id: i64, tier: i64) -> SqlResult<Thumbnail> {
    conn.query_row(
        "SELECT photo_id, source_file_id, source_hash, tier, width, height, generated_at
         FROM thumbnails WHERE photo_id = ?1 AND tier = ?2",
        params![photo_id, tier],
        |row| {
            Ok(Thumbnail {
                photo_id: row.get(0)?,
                source_file_id: row.get(1)?,
                source_hash: row.get(2)?,
                tier: row.get(3)?,
                width: row.get(4)?,
                height: row.get(5)?,
                generated_at: row.get(6)?,
            })
        },
    )
}

/// Create a scan job
pub fn create_scan_job(conn: &Connection, library_id: i64) -> SqlResult<ScanJob> {
    conn.execute(
        "INSERT INTO scan_jobs (library_id, status) VALUES (?1, 'running')",
        [library_id],
    )?;
    let id = conn.last_insert_rowid();
    get_scan_job(conn, id)
}

/// Get a scan job by ID
pub fn get_scan_job(conn: &Connection, id: i64) -> SqlResult<ScanJob> {
    conn.query_row(
        "SELECT id, library_id, status, cursor, started_at, finished_at, added, updated, missing
         FROM scan_jobs WHERE id = ?1",
        [id],
        |row| {
            Ok(ScanJob {
                id: row.get(0)?,
                library_id: row.get(1)?,
                status: row.get(2)?,
                cursor: row.get(3)?,
                started_at: row.get(4)?,
                finished_at: row.get(5)?,
                added: row.get(6)?,
                updated: row.get(7)?,
                missing: row.get(8)?,
            })
        },
    )
}

/// Update scan job status
pub fn update_scan_job_status(
    conn: &Connection,
    id: i64,
    status: &str,
    added: Option<i64>,
    updated: Option<i64>,
    missing: Option<i64>,
) -> SqlResult<()> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    conn.execute(
        "UPDATE scan_jobs SET status = ?1, finished_at = ?2,
         added = COALESCE(?3, added), updated = COALESCE(?4, updated), missing = COALESCE(?5, missing)
         WHERE id = ?6",
        params![status, now, added, updated, missing, id],
    )?;
    Ok(())
}

/// Update scan job cursor
pub fn update_scan_job_cursor(conn: &Connection, id: i64, cursor: &str) -> SqlResult<()> {
    conn.execute(
        "UPDATE scan_jobs SET cursor = ?1 WHERE id = ?2",
        params![cursor, id],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::db;
    use tempfile::TempDir;

    fn test_db() -> (TempDir, Connection) {
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("test.sqlite");
        let conn = db::create_connection(&db_path).unwrap();
        db::run_migrations(&conn).unwrap();
        (tmp, conn)
    }

    #[test]
    fn test_library_crud() {
        let (_tmp, conn) = test_db();

        let lib = insert_library(&conn, "My Photos", "/photos").unwrap();
        assert_eq!(lib.name, "My Photos");
        assert_eq!(lib.root_path, "/photos");

        let fetched = get_library(&conn, lib.id).unwrap();
        assert_eq!(fetched.id, lib.id);

        let libs = list_libraries(&conn).unwrap();
        assert_eq!(libs.len(), 1);
    }

    #[test]
    fn test_photo_crud() {
        let (_tmp, conn) = test_db();
        let lib = insert_library(&conn, "Test", "/test").unwrap();

        let photo = insert_photo(&conn, lib.id, "jpeg", Some(1000), Some("exif")).unwrap();
        assert_eq!(photo.library_id, lib.id);
        assert_eq!(photo.format, "jpeg");
        assert_eq!(photo.taken_at, Some(1000));

        let fetched = get_photo(&conn, photo.id).unwrap();
        assert_eq!(fetched.id, photo.id);
    }

    #[test]
    fn test_photo_file_roles() {
        let (_tmp, conn) = test_db();
        let lib = insert_library(&conn, "Test", "/test").unwrap();
        let photo = insert_photo(&conn, lib.id, "jpeg", None, None).unwrap();

        // Test different roles
        let display = insert_photo_file(
            &conn,
            photo.id,
            "/test/photo.jpg",
            "abc123",
            1000,
            100,
            "display",
            None,
        )
        .unwrap();
        assert_eq!(display.role, "display");
        assert_eq!(display.status, "available");

        let raw = insert_photo_file(
            &conn,
            photo.id,
            "/test/photo.arw",
            "def456",
            5000,
            100,
            "raw",
            None,
        )
        .unwrap();
        assert_eq!(raw.role, "raw");

        let sidecar = insert_photo_file(
            &conn,
            photo.id,
            "/test/photo.xmp",
            "ghi789",
            500,
            100,
            "sidecar",
            None,
        )
        .unwrap();
        assert_eq!(sidecar.role, "sidecar");
    }

    #[test]
    fn test_photo_file_identity_lookup() {
        let (_tmp, conn) = test_db();
        let lib = insert_library(&conn, "Test", "/test").unwrap();
        let photo = insert_photo(&conn, lib.id, "jpeg", None, None).unwrap();

        insert_photo_file(&conn, photo.id, "/test/a.jpg", "hash1", 1000, 100, "display", None).unwrap();
        insert_photo_file(&conn, photo.id, "/test/b.jpg", "hash1", 1000, 100, "duplicate", None).unwrap();

        let files = find_photo_files_by_identity(&conn, "hash1", 1000).unwrap();
        assert_eq!(files.len(), 2);
    }

    #[test]
    fn test_scan_job_lifecycle() {
        let (_tmp, conn) = test_db();
        let lib = insert_library(&conn, "Test", "/test").unwrap();

        let job = create_scan_job(&conn, lib.id).unwrap();
        assert_eq!(job.status, "running");
        assert_eq!(job.added, 0);

        update_scan_job_cursor(&conn, job.id, "photos/2024").unwrap();
        update_scan_job_status(&conn, job.id, "done", Some(10), Some(2), Some(1)).unwrap();

        let updated = get_scan_job(&conn, job.id).unwrap();
        assert_eq!(updated.status, "done");
        assert_eq!(updated.added, 10);
        assert_eq!(updated.updated, 2);
        assert_eq!(updated.missing, 1);
    }

    #[test]
    fn test_thumbnail_metadata() {
        let (_tmp, conn) = test_db();
        let lib = insert_library(&conn, "Test", "/test").unwrap();
        let photo = insert_photo(&conn, lib.id, "jpeg", None, None).unwrap();
        let file = insert_photo_file(&conn, photo.id, "/test/a.jpg", "hash1", 1000, 100, "display", None).unwrap();

        let thumb = insert_thumbnail(&conn, photo.id, file.id, "hash1", 240, 240, 180).unwrap();
        assert_eq!(thumb.tier, 240);
        assert_eq!(thumb.width, 240);

        let fetched = get_thumbnail(&conn, photo.id, 240).unwrap();
        assert_eq!(fetched.source_hash, "hash1");
    }
}

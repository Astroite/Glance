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
    pub display_file_id: Option<i64>,
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
    pub indexed_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhotoFile {
    pub id: i64,
    pub library_id: i64,
    pub photo_id: i64,
    pub path: String,
    pub role: String,
    pub format: String,
    pub content_hash: String,
    pub file_size: i64,
    pub mtime: i64,
    pub status: String,
    pub missing_since: Option<i64>,
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

/// Insert a new photo (no format — format lives on photo_files)
pub fn insert_photo(
    conn: &Connection,
    library_id: i64,
    taken_at: Option<i64>,
    taken_at_src: Option<&str>,
) -> SqlResult<Photo> {
    conn.execute(
        "INSERT INTO photos (library_id, taken_at, taken_at_src) VALUES (?1, ?2, ?3)",
        params![library_id, taken_at, taken_at_src],
    )?;
    let id = conn.last_insert_rowid();
    get_photo(conn, id)
}

/// Get a photo by ID
pub fn get_photo(conn: &Connection, id: i64) -> SqlResult<Photo> {
    conn.query_row(
        "SELECT id, library_id, display_file_id, taken_at, taken_at_src, camera_make, camera_model,
         lens, focal_len, aperture, shutter, iso, width, height, orientation,
         gps_lat, gps_lon, rating, label, indexed_at, updated_at
         FROM photos WHERE id = ?1",
        [id],
        |row| {
            Ok(Photo {
                id: row.get(0)?,
                library_id: row.get(1)?,
                display_file_id: row.get(2)?,
                taken_at: row.get(3)?,
                taken_at_src: row.get(4)?,
                camera_make: row.get(5)?,
                camera_model: row.get(6)?,
                lens: row.get(7)?,
                focal_len: row.get(8)?,
                aperture: row.get(9)?,
                shutter: row.get(10)?,
                iso: row.get(11)?,
                width: row.get(12)?,
                height: row.get(13)?,
                orientation: row.get(14)?,
                gps_lat: row.get(15)?,
                gps_lon: row.get(16)?,
                rating: row.get(17)?,
                label: row.get(18)?,
                indexed_at: row.get(19)?,
                updated_at: row.get(20)?,
            })
        },
    )
}

/// Update photo metadata
pub fn update_photo_metadata(
    conn: &Connection,
    id: i64,
    meta: &crate::core::exif::PhotoMetadata,
) -> SqlResult<()> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    conn.execute(
        "UPDATE photos SET taken_at = COALESCE(?1, taken_at), taken_at_src = COALESCE(?2, taken_at_src),
         camera_make = ?3, camera_model = ?4, lens = ?5, focal_len = ?6, aperture = ?7,
         shutter = ?8, iso = ?9, width = ?10, height = ?11, orientation = ?12,
         gps_lat = ?13, gps_lon = ?14, rating = ?15, label = ?16, updated_at = ?17
         WHERE id = ?18",
        params![
            meta.taken_at,
            meta.taken_at_src,
            meta.camera_make,
            meta.camera_model,
            meta.lens,
            meta.focal_len,
            meta.aperture,
            meta.shutter,
            meta.iso,
            meta.width,
            meta.height,
            meta.orientation,
            meta.gps_lat,
            meta.gps_lon,
            meta.rating,
            meta.label,
            now,
            id,
        ],
    )?;
    Ok(())
}

/// Insert a photo file
pub fn insert_photo_file(
    conn: &Connection,
    library_id: i64,
    photo_id: i64,
    path: &str,
    content_hash: &str,
    file_size: i64,
    mtime: i64,
    role: &str,
    format: &str,
    last_scan_id: Option<i64>,
) -> SqlResult<PhotoFile> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    conn.execute(
        "INSERT INTO photo_files (library_id, photo_id, path, content_hash, file_size, mtime, role, format, last_seen_at, last_scan_id)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![library_id, photo_id, path, content_hash, file_size, mtime, role, format, now, last_scan_id],
    )?;
    let id = conn.last_insert_rowid();
    get_photo_file(conn, id)
}

/// Get a photo file by ID
pub fn get_photo_file(conn: &Connection, id: i64) -> SqlResult<PhotoFile> {
    conn.query_row(
        "SELECT id, library_id, photo_id, path, role, format, content_hash, file_size, mtime, status, missing_since, last_seen_at, last_scan_id
         FROM photo_files WHERE id = ?1",
        [id],
        |row| {
            Ok(PhotoFile {
                id: row.get(0)?,
                library_id: row.get(1)?,
                photo_id: row.get(2)?,
                path: row.get(3)?,
                role: row.get(4)?,
                format: row.get(5)?,
                content_hash: row.get(6)?,
                file_size: row.get(7)?,
                mtime: row.get(8)?,
                status: row.get(9)?,
                missing_since: row.get(10)?,
                last_seen_at: row.get(11)?,
                last_scan_id: row.get(12)?,
            })
        },
    )
}

/// Find photo files by identity (content_hash + file_size), scoped to library
pub fn find_photo_files_by_identity(
    conn: &Connection,
    library_id: i64,
    content_hash: &str,
    file_size: i64,
) -> SqlResult<Vec<PhotoFile>> {
    let mut stmt = conn.prepare(
        "SELECT id, library_id, photo_id, path, role, format, content_hash, file_size, mtime, status, missing_since, last_seen_at, last_scan_id
         FROM photo_files WHERE library_id = ?1 AND content_hash = ?2 AND file_size = ?3",
    )?;
    let rows = stmt.query_map(params![library_id, content_hash, file_size], |row| {
        Ok(PhotoFile {
            id: row.get(0)?,
            library_id: row.get(1)?,
            photo_id: row.get(2)?,
            path: row.get(3)?,
            role: row.get(4)?,
            format: row.get(5)?,
            content_hash: row.get(6)?,
            file_size: row.get(7)?,
            mtime: row.get(8)?,
            status: row.get(9)?,
            missing_since: row.get(10)?,
            last_seen_at: row.get(11)?,
            last_scan_id: row.get(12)?,
        })
    })?;
    rows.collect()
}

/// Find photo file by path within a library
pub fn find_photo_file_by_path(conn: &Connection, library_id: i64, path: &str) -> SqlResult<Option<PhotoFile>> {
    let mut stmt = conn.prepare(
        "SELECT id, library_id, photo_id, path, role, format, content_hash, file_size, mtime, status, missing_since, last_seen_at, last_scan_id
         FROM photo_files WHERE library_id = ?1 AND path = ?2",
    )?;
    let mut rows = stmt.query_map(params![library_id, path], |row| {
        Ok(PhotoFile {
            id: row.get(0)?,
            library_id: row.get(1)?,
            photo_id: row.get(2)?,
            path: row.get(3)?,
            role: row.get(4)?,
            format: row.get(5)?,
            content_hash: row.get(6)?,
            file_size: row.get(7)?,
            mtime: row.get(8)?,
            status: row.get(9)?,
            missing_since: row.get(10)?,
            last_seen_at: row.get(11)?,
            last_scan_id: row.get(12)?,
        })
    })?;
    match rows.next() {
        Some(row) => Ok(Some(row?)),
        None => Ok(None),
    }
}

/// Update photo file status, setting missing_since when marking missing
pub fn update_photo_file_status(conn: &Connection, id: i64, status: &str) -> SqlResult<()> {
    if status == "missing" {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        conn.execute(
            "UPDATE photo_files SET status = ?1, missing_since = ?2 WHERE id = ?3",
            params![status, now, id],
        )?;
    } else {
        conn.execute(
            "UPDATE photo_files SET status = ?1, missing_since = NULL WHERE id = ?2",
            params![status, id],
        )?;
    }
    Ok(())
}

/// Update photo file last_seen_at
pub fn update_photo_file_last_seen(conn: &Connection, id: i64, last_scan_id: Option<i64>) -> SqlResult<()> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    conn.execute(
        "UPDATE photo_files SET last_seen_at = ?1, last_scan_id = ?2, status = 'available', missing_since = NULL WHERE id = ?3",
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

/// Mark scan job as complete
pub fn mark_scan_complete(
    conn: &Connection,
    id: i64,
    added: i64,
    updated: i64,
    missing: i64,
) -> SqlResult<()> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    conn.execute(
        "UPDATE scan_jobs SET status = 'done', finished_at = ?1, added = ?2, updated = ?3, missing = ?4 WHERE id = ?5",
        params![now, added, updated, missing, id],
    )?;
    Ok(())
}

/// Mark scan job as failed
pub fn mark_scan_failed(conn: &Connection, id: i64, error: &str) -> SqlResult<()> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    conn.execute(
        "UPDATE scan_jobs SET status = 'failed', finished_at = ?1 WHERE id = ?2",
        params![now, id],
    )?;
    // Log error but don't store it in DB (scan_jobs has no error column)
    eprintln!("Scan job {} failed: {}", id, error);
    Ok(())
}

/// Mark scan job as paused
pub fn mark_scan_paused(conn: &Connection, id: i64) -> SqlResult<()> {
    conn.execute(
        "UPDATE scan_jobs SET status = 'paused' WHERE id = ?1",
        [id],
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

/// Get running or paused scan jobs for a library
pub fn get_resumable_scan_jobs(conn: &Connection, library_id: i64) -> SqlResult<Vec<ScanJob>> {
    let mut stmt = conn.prepare(
        "SELECT id, library_id, status, cursor, started_at, finished_at, added, updated, missing
         FROM scan_jobs WHERE library_id = ?1 AND status IN ('running', 'paused')
         ORDER BY id DESC",
    )?;
    let rows = stmt.query_map([library_id], |row| {
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
    })?;
    rows.collect()
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

        let photo = insert_photo(&conn, lib.id, Some(1000), Some("exif")).unwrap();
        assert_eq!(photo.library_id, lib.id);
        assert_eq!(photo.taken_at, Some(1000));

        let fetched = get_photo(&conn, photo.id).unwrap();
        assert_eq!(fetched.id, photo.id);
    }

    #[test]
    fn test_photo_file_with_format_and_library_id() {
        let (_tmp, conn) = test_db();
        let lib = insert_library(&conn, "Test", "/test").unwrap();
        let photo = insert_photo(&conn, lib.id, None, None).unwrap();

        let display = insert_photo_file(
            &conn, lib.id, photo.id, "/test/photo.jpg",
            "abc123", 1000, 100, "display", "jpeg", None,
        ).unwrap();
        assert_eq!(display.role, "display");
        assert_eq!(display.format, "jpeg");
        assert_eq!(display.library_id, lib.id);
        assert_eq!(display.status, "available");
        assert!(display.missing_since.is_none());

        let raw = insert_photo_file(
            &conn, lib.id, photo.id, "/test/photo.arw",
            "def456", 5000, 100, "raw", "arw", None,
        ).unwrap();
        assert_eq!(raw.format, "arw");
    }

    #[test]
    fn test_photo_file_identity_lookup_scoped_to_library() {
        let (_tmp, conn) = test_db();
        let lib1 = insert_library(&conn, "Lib1", "/lib1").unwrap();
        let lib2 = insert_library(&conn, "Lib2", "/lib2").unwrap();
        let photo1 = insert_photo(&conn, lib1.id, None, None).unwrap();
        let photo2 = insert_photo(&conn, lib2.id, None, None).unwrap();

        insert_photo_file(&conn, lib1.id, photo1.id, "/lib1/a.jpg", "hash1", 1000, 100, "display", "jpeg", None).unwrap();
        insert_photo_file(&conn, lib2.id, photo2.id, "/lib2/a.jpg", "hash1", 1000, 100, "display", "jpeg", None).unwrap();

        // Same hash in different libraries should not conflict
        let files1 = find_photo_files_by_identity(&conn, lib1.id, "hash1", 1000).unwrap();
        assert_eq!(files1.len(), 1);
        assert_eq!(files1[0].library_id, lib1.id);

        let files2 = find_photo_files_by_identity(&conn, lib2.id, "hash1", 1000).unwrap();
        assert_eq!(files2.len(), 1);
        assert_eq!(files2[0].library_id, lib2.id);
    }

    #[test]
    fn test_missing_since_set_on_mark_missing() {
        let (_tmp, conn) = test_db();
        let lib = insert_library(&conn, "Test", "/test").unwrap();
        let photo = insert_photo(&conn, lib.id, None, None).unwrap();
        let file = insert_photo_file(&conn, lib.id, photo.id, "/test/a.jpg", "hash1", 1000, 100, "display", "jpeg", None).unwrap();

        update_photo_file_status(&conn, file.id, "missing").unwrap();
        let updated = get_photo_file(&conn, file.id).unwrap();
        assert_eq!(updated.status, "missing");
        assert!(updated.missing_since.is_some());
    }

    #[test]
    fn test_missing_since_cleared_on_restore() {
        let (_tmp, conn) = test_db();
        let lib = insert_library(&conn, "Test", "/test").unwrap();
        let photo = insert_photo(&conn, lib.id, None, None).unwrap();
        let file = insert_photo_file(&conn, lib.id, photo.id, "/test/a.jpg", "hash1", 1000, 100, "display", "jpeg", None).unwrap();

        update_photo_file_status(&conn, file.id, "missing").unwrap();
        update_photo_file_status(&conn, file.id, "available").unwrap();
        let updated = get_photo_file(&conn, file.id).unwrap();
        assert_eq!(updated.status, "available");
        assert!(updated.missing_since.is_none());
    }

    #[test]
    fn test_scan_job_lifecycle() {
        let (_tmp, conn) = test_db();
        let lib = insert_library(&conn, "Test", "/test").unwrap();

        let job = create_scan_job(&conn, lib.id).unwrap();
        assert_eq!(job.status, "running");
        assert_eq!(job.added, 0);

        update_scan_job_cursor(&conn, job.id, "photos/2024").unwrap();
        mark_scan_complete(&conn, job.id, 10, 2, 1).unwrap();

        let updated = get_scan_job(&conn, job.id).unwrap();
        assert_eq!(updated.status, "done");
        assert_eq!(updated.added, 10);
        assert_eq!(updated.updated, 2);
        assert_eq!(updated.missing, 1);
        assert!(updated.finished_at.is_some());
    }

    #[test]
    fn test_scan_job_paused_no_finished_at() {
        let (_tmp, conn) = test_db();
        let lib = insert_library(&conn, "Test", "/test").unwrap();

        let job = create_scan_job(&conn, lib.id).unwrap();
        mark_scan_paused(&conn, job.id).unwrap();

        let updated = get_scan_job(&conn, job.id).unwrap();
        assert_eq!(updated.status, "paused");
        assert!(updated.finished_at.is_none());
    }

    #[test]
    fn test_thumbnail_metadata() {
        let (_tmp, conn) = test_db();
        let lib = insert_library(&conn, "Test", "/test").unwrap();
        let photo = insert_photo(&conn, lib.id, None, None).unwrap();
        let file = insert_photo_file(&conn, lib.id, photo.id, "/test/a.jpg", "hash1", 1000, 100, "display", "jpeg", None).unwrap();

        let thumb = insert_thumbnail(&conn, photo.id, file.id, "hash1", 240, 240, 180).unwrap();
        assert_eq!(thumb.tier, 240);
        assert_eq!(thumb.width, 240);

        let fetched = get_thumbnail(&conn, photo.id, 240).unwrap();
        assert_eq!(fetched.source_hash, "hash1");
    }

    #[test]
    fn test_resumable_scan_jobs() {
        let (_tmp, conn) = test_db();
        let lib = insert_library(&conn, "Test", "/test").unwrap();

        let job = create_scan_job(&conn, lib.id).unwrap();
        update_scan_job_cursor(&conn, job.id, "photos/2024/05").unwrap();

        let resumable = get_resumable_scan_jobs(&conn, lib.id).unwrap();
        assert_eq!(resumable.len(), 1);
        assert_eq!(resumable[0].cursor, Some("photos/2024/05".to_string()));

        // Complete it — should no longer be resumable
        mark_scan_complete(&conn, job.id, 5, 0, 0).unwrap();
        let resumable = get_resumable_scan_jobs(&conn, lib.id).unwrap();
        assert_eq!(resumable.len(), 0);
    }
}

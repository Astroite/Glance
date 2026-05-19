use crate::core::db::dao;
use crate::core::db::dao::PhotoFile;
use crate::core::exif;
use crate::core::identity;
use crate::core::scanner::{self, FileRole};
use rusqlite::Connection;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Batch size for transactional commits
const BATCH_SIZE: usize = 200;

/// Result of a scan
#[derive(Debug, Clone, serde::Serialize)]
pub struct ScanResult {
    pub added: i64,
    pub updated: i64,
    pub missing: i64,
}

/// Cancellation token for scan operations
pub type CancellationToken = Arc<AtomicBool>;

/// Create a new cancellation token
pub fn new_cancellation_token() -> CancellationToken {
    Arc::new(AtomicBool::new(false))
}

/// Run a full scan of a library directory with batched transactions and cursor support.
/// If `resume_from` is provided, skips files until past the cursor position.
pub fn run_scan(
    conn: &Connection,
    library_id: i64,
    root_path: &Path,
    cancel: Option<&CancellationToken>,
    resume_job_id: Option<i64>,
) -> Result<ScanResult, String> {
    // Create or resume scan job
    let job = if let Some(job_id) = resume_job_id {
        dao::get_scan_job(conn, job_id).map_err(|e| format!("Failed to get scan job: {}", e))?
    } else {
        dao::create_scan_job(conn, library_id)
            .map_err(|e| format!("Failed to create scan job: {}", e))?
    };

    let cursor = job.cursor.clone();

    // Stream-discover files, group by directory, process in batches
    let mut added = 0i64;
    let mut updated = 0i64;
    let mut batch: Vec<scanner::PhotoCandidate> = Vec::new();
    let mut last_dir: Option<PathBuf> = None;
    let mut skipped_past_cursor = cursor.is_none();

    // Use WalkDir iterator — don't collect all files into memory.
    // sort_by_file_name() ensures cursor's lexicographic comparison is stable
    // across filesystems (NTFS/ext4 default ordering is implementation-defined).
    let walker = walkdir::WalkDir::new(root_path)
        .follow_links(false)
        .sort_by_file_name();

    // We need to group files by (dir, stem) as we walk.
    // Strategy: accumulate files per directory, flush when directory changes.
    let mut dir_files: Vec<scanner::DiscoveredFile> = Vec::new();
    let mut current_dir: Option<PathBuf> = None;

    for entry in walker.into_iter().filter_map(|e| e.ok()) {
        // Check cancellation
        if let Some(token) = cancel {
            if token.load(Ordering::Relaxed) {
                // Mark as paused
                dao::mark_scan_paused(conn, job.id)
                    .map_err(|e| format!("Failed to pause scan: {}", e))?;
                return Err("Scan cancelled".to_string());
            }
        }

        if !entry.file_type().is_file() {
            continue;
        }

        let path = entry.path();
        let discovered = match scanner::classify_file(path) {
            Some(f) => f,
            None => continue,
        };

        let dir = path.parent().map(|p| p.to_path_buf()).unwrap_or_default();

        // Cursor skip: if resuming, skip files whose directory (relative to root)
        // is strictly less than the cursor. Cursor stores the last successfully
        // processed directory as a relative path; equal dirs are reprocessed
        // (UPSERT is idempotent).
        if !skipped_past_cursor {
            if let Some(ref cur) = cursor {
                let rel_dir = dir.strip_prefix(root_path).unwrap_or(&dir).to_string_lossy();
                if rel_dir.as_ref() < cur.as_str() {
                    continue;
                }
                skipped_past_cursor = true;
            }
        }

        // If directory changed, flush the previous directory's files into candidates
        if current_dir.as_ref() != Some(&dir) {
            if let Some(prev_dir) = current_dir.take() {
                flush_dir_files(&prev_dir, &mut dir_files, &mut batch);
            }
            current_dir = Some(dir.clone());
        }

        dir_files.push(discovered);

        // If batch is large enough, process it
        if batch.len() >= BATCH_SIZE {
            let batch_dir = current_dir
                .as_ref()
                .map(|d| d.strip_prefix(root_path).unwrap_or(d).to_string_lossy().to_string())
                .unwrap_or_default();
            process_batch(conn, library_id, job.id, &batch, &mut added, &mut updated)?;
            dao::update_scan_job_cursor(conn, job.id, &batch_dir)
                .map_err(|e| format!("Failed to update cursor: {}", e))?;
            batch.clear();
            last_dir = current_dir.clone();
        }
    }

    // Flush remaining files
    if let Some(dir) = current_dir {
        flush_dir_files(&dir, &mut dir_files, &mut batch);
    }

    // Process final batch
    if !batch.is_empty() {
        let batch_dir = last_dir
            .as_ref()
            .map(|d| d.strip_prefix(root_path).unwrap_or(d).to_string_lossy().to_string())
            .unwrap_or_default();
        process_batch(conn, library_id, job.id, &batch, &mut added, &mut updated)?;
        dao::update_scan_job_cursor(conn, job.id, &batch_dir)
            .map_err(|e| format!("Failed to update cursor: {}", e))?;
    }

    // Mark files not seen in this scan as missing
    let missing = mark_missing_files(conn, library_id, job.id)?;

    // Update scan job status
    dao::mark_scan_complete(conn, job.id, added, updated, missing)
        .map_err(|e| format!("Failed to update scan job: {}", e))?;

    Ok(ScanResult {
        added,
        updated,
        missing,
    })
}

/// Convert accumulated directory files into candidates and add to batch
fn flush_dir_files(
    _dir: &Path,
    dir_files: &mut Vec<scanner::DiscoveredFile>,
    batch: &mut Vec<scanner::PhotoCandidate>,
) {
    if dir_files.is_empty() {
        return;
    }
    let candidates = scanner::group_into_candidates(std::mem::take(dir_files));
    batch.extend(candidates);
}

/// Process a batch of candidates in a single transaction
fn process_batch(
    conn: &Connection,
    library_id: i64,
    scan_job_id: i64,
    batch: &[scanner::PhotoCandidate],
    added: &mut i64,
    updated: &mut i64,
) -> Result<(), String> {
    conn.execute_batch("BEGIN TRANSACTION")
        .map_err(|e| format!("Failed to begin transaction: {}", e))?;

    for candidate in batch {
        match process_candidate(conn, library_id, scan_job_id, candidate) {
            Ok(ProcessOutcome::Added) => *added += 1,
            Ok(ProcessOutcome::Updated) => *updated += 1,
            Ok(ProcessOutcome::Unchanged) => {}
            Err(e) => {
                eprintln!("Error processing {}: {}", candidate.stem, e);
            }
        }
    }

    conn.execute_batch("COMMIT")
        .map_err(|e| format!("Failed to commit transaction: {}", e))?;

    Ok(())
}

enum ProcessOutcome {
    Added,
    Updated,
    Unchanged,
}

/// Process a single photo candidate (RAW+JPEG+XMP group)
fn process_candidate(
    conn: &Connection,
    library_id: i64,
    scan_job_id: i64,
    candidate: &scanner::PhotoCandidate,
) -> Result<ProcessOutcome, String> {
    let roles = scanner::assign_roles(candidate);

    // B11 fix: only use display or raw files for identity lookup, not sidecar
    let mut existing_photo_id: Option<i64> = None;
    let mut existing_files: Vec<PhotoFile> = Vec::new();

    for (file, role) in &roles {
        if *role == FileRole::Sidecar {
            continue;
        }

        let identity = identity::compute_identity(&file.path)
            .map_err(|e| format!("Identity error for {}: {}", file.path.display(), e))?;

        let found = dao::find_photo_files_by_identity(conn, library_id, &identity.content_hash, identity.file_size as i64)
            .map_err(|e| format!("DB error: {}", e))?;

        if !found.is_empty() {
            if existing_photo_id.is_none() {
                existing_photo_id = Some(found[0].photo_id);
            }
            existing_files.extend(found);
        }
    }

    if let Some(photo_id) = existing_photo_id {
        let changed = update_existing_photo(conn, library_id, photo_id, &roles, &existing_files, scan_job_id)?;
        if changed {
            Ok(ProcessOutcome::Updated)
        } else {
            Ok(ProcessOutcome::Unchanged)
        }
    } else {
        create_new_photo(conn, library_id, &roles, scan_job_id)?;
        Ok(ProcessOutcome::Added)
    }
}

/// Determine format string from file extension
fn format_from_extension(ext: &str) -> String {
    match ext.to_lowercase().as_str() {
        "jpg" | "jpeg" => "jpeg".to_string(),
        "png" => "png".to_string(),
        "heic" | "heif" => "heic".to_string(),
        "arw" => "arw".to_string(),
        "cr2" => "cr2".to_string(),
        "cr3" => "cr3".to_string(),
        "nef" => "nef".to_string(),
        "dng" => "dng".to_string(),
        "orf" => "orf".to_string(),
        "rw2" => "rw2".to_string(),
        "raf" => "raf".to_string(),
        "xmp" => "xmp".to_string(),
        other => other.to_string(),
    }
}

/// Create a new photo and its file instances
fn create_new_photo(
    conn: &Connection,
    library_id: i64,
    roles: &[(&scanner::DiscoveredFile, FileRole)],
    scan_job_id: i64,
) -> Result<(), String> {
    let display_file = roles.iter().find(|(_, r)| *r == FileRole::Display).map(|(f, _)| f);
    let raw_file = roles.iter().find(|(_, r)| *r == FileRole::Raw).map(|(f, _)| f);
    let sidecar_file = roles.iter().find(|(_, r)| *r == FileRole::Sidecar).map(|(f, _)| f);

    let mtime = display_file
        .or(raw_file)
        .and_then(|f| std::fs::metadata(&f.path).ok())
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    let mut metadata = if let Some(display) = display_file {
        exif::extract_exif(&display.path, mtime)
    } else if let Some(raw) = raw_file {
        exif::extract_exif(&raw.path, mtime)
    } else {
        exif::PhotoMetadata {
            taken_at: Some(mtime),
            taken_at_src: Some("mtime".to_string()),
            ..Default::default()
        }
    };

    if display_file.is_some() && raw_file.is_some() {
        let raw_metadata = exif::extract_exif(&raw_file.unwrap().path, mtime);
        metadata = exif::merge_metadata(metadata, Some(raw_metadata));
    }

    if let Some(sidecar) = sidecar_file {
        let xmp = exif::extract_xmp(&sidecar.path);
        if let Some(rating) = xmp.rating {
            metadata.rating = Some(rating);
        }
        metadata.label = xmp.label;
    }

    let photo = dao::insert_photo(
        conn,
        library_id,
        metadata.taken_at,
        metadata.taken_at_src.as_deref(),
    )
    .map_err(|e| format!("Failed to insert photo: {}", e))?;

    dao::update_photo_metadata(conn, photo.id, &metadata)
        .map_err(|e| format!("Failed to update photo metadata: {}", e))?;

    // For RAW-only photos (no display file), promote the first RAW to display role
    let has_display = roles.iter().any(|(_, r)| *r == FileRole::Display);
    let mut display_file_id = None;
    let mut raw_count = 0;

    for (file, role) in roles {
        let identity = identity::compute_identity(&file.path)
            .map_err(|e| format!("Identity error: {}", e))?;

        let format = format_from_extension(&file.extension);

        // For RAW-only photos, promote the first RAW to "display" role
        let effective_role = if !has_display && *role == FileRole::Raw && raw_count == 0 {
            raw_count += 1;
            FileRole::Display
        } else {
            if *role == FileRole::Raw {
                raw_count += 1;
            }
            *role
        };

        let file_record = dao::insert_photo_file(
            conn,
            library_id,
            photo.id,
            &file.path.to_string_lossy(),
            &identity.content_hash,
            identity.file_size as i64,
            identity.mtime,
            effective_role.as_str(),
            &format,
            Some(scan_job_id),
        )
        .map_err(|e| format!("Failed to insert photo file: {}", e))?;

        if effective_role == FileRole::Display {
            display_file_id = Some(file_record.id);
        }
    }

    if let Some(display_id) = display_file_id {
        conn.execute(
            "UPDATE photos SET display_file_id = ?1 WHERE id = ?2",
            rusqlite::params![display_id, photo.id],
        )
        .map_err(|e| format!("Failed to set display file: {}", e))?;
    }

    Ok(())
}

/// Update an existing photo with new file instances.
fn update_existing_photo(
    conn: &Connection,
    library_id: i64,
    photo_id: i64,
    roles: &[(&scanner::DiscoveredFile, FileRole)],
    existing_files: &[PhotoFile],
    scan_job_id: i64,
) -> Result<bool, String> {
    let mut changed = false;

    for (file, role) in roles {
        let identity = identity::compute_identity(&file.path)
            .map_err(|e| format!("Identity error: {}", e))?;

        let path_str = file.path.to_string_lossy();

        if let Some(existing) = existing_files.iter().find(|f| f.path == path_str) {
            if existing.content_hash == identity.content_hash && existing.file_size == identity.file_size as i64 {
                dao::update_photo_file_last_seen(conn, existing.id, Some(scan_job_id))
                    .map_err(|e| format!("Failed to update last_seen: {}", e))?;

                if (*role == FileRole::Display || *role == FileRole::Sidecar)
                    && existing.mtime != identity.mtime
                {
                    refresh_metadata(conn, library_id, photo_id, roles)?;
                    changed = true;
                }
            } else {
                dao::update_photo_file_status(conn, existing.id, "missing")
                    .map_err(|e| format!("Failed to mark missing: {}", e))?;

                let format = format_from_extension(&file.extension);
                dao::insert_photo_file(
                    conn,
                    library_id,
                    photo_id,
                    &path_str,
                    &identity.content_hash,
                    identity.file_size as i64,
                    identity.mtime,
                    role.as_str(),
                    &format,
                    Some(scan_job_id),
                )
                .map_err(|e| format!("Failed to insert new file: {}", e))?;

                if *role == FileRole::Display {
                    refresh_metadata(conn, library_id, photo_id, roles)?;
                }
                changed = true;
            }
        } else {
            let format = format_from_extension(&file.extension);
            dao::insert_photo_file(
                conn,
                library_id,
                photo_id,
                &path_str,
                &identity.content_hash,
                identity.file_size as i64,
                identity.mtime,
                role.as_str(),
                &format,
                Some(scan_job_id),
            )
            .map_err(|e| format!("Failed to insert file: {}", e))?;
            changed = true;
        }
    }

    Ok(changed)
}

/// Re-read EXIF/XMP metadata for a photo and update the photos table
fn refresh_metadata(
    conn: &Connection,
    _library_id: i64,
    photo_id: i64,
    roles: &[(&scanner::DiscoveredFile, FileRole)],
) -> Result<(), String> {
    let display_file = roles.iter().find(|(_, r)| *r == FileRole::Display).map(|(f, _)| f);
    let raw_file = roles.iter().find(|(_, r)| *r == FileRole::Raw).map(|(f, _)| f);
    let sidecar_file = roles.iter().find(|(_, r)| *r == FileRole::Sidecar).map(|(f, _)| f);

    let mtime = display_file
        .or(raw_file)
        .and_then(|f| std::fs::metadata(&f.path).ok())
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    let mut metadata = if let Some(display) = display_file {
        exif::extract_exif(&display.path, mtime)
    } else if let Some(raw) = raw_file {
        exif::extract_exif(&raw.path, mtime)
    } else {
        return Ok(());
    };

    if display_file.is_some() && raw_file.is_some() {
        let raw_metadata = exif::extract_exif(&raw_file.unwrap().path, mtime);
        metadata = exif::merge_metadata(metadata, Some(raw_metadata));
    }

    if let Some(sidecar) = sidecar_file {
        let xmp = exif::extract_xmp(&sidecar.path);
        if let Some(rating) = xmp.rating {
            metadata.rating = Some(rating);
        }
        metadata.label = xmp.label;
    }

    dao::update_photo_metadata(conn, photo_id, &metadata)
        .map_err(|e| format!("Failed to refresh metadata: {}", e))?;

    Ok(())
}

/// Mark files not seen in the current scan as missing
fn mark_missing_files(conn: &Connection, library_id: i64, scan_job_id: i64) -> Result<i64, String> {
    let count = conn.execute(
        "UPDATE photo_files SET status = 'missing',
         missing_since = CASE WHEN missing_since IS NULL THEN strftime('%s', 'now') ELSE missing_since END
         WHERE photo_id IN (SELECT id FROM photos WHERE library_id = ?1)
         AND (last_scan_id IS NULL OR last_scan_id < ?2)
         AND status = 'available'",
        rusqlite::params![library_id, scan_job_id],
    )
    .map_err(|e| format!("Failed to mark missing files: {}", e))?;

    Ok(count as i64)
}

/// Relocate a folder - update all paths with old prefix to new prefix
pub fn relocate_folder(
    conn: &Connection,
    library_id: i64,
    old_prefix: &str,
    new_prefix: &str,
) -> Result<i64, String> {
    let count = conn.execute(
        "UPDATE photo_files SET path = ?1 || SUBSTR(path, LENGTH(?2) + 1)
         WHERE path LIKE ?2 || '%'
         AND photo_id IN (SELECT id FROM photos WHERE library_id = ?3)",
        rusqlite::params![new_prefix, old_prefix, library_id],
    )
    .map_err(|e| format!("Failed to relocate folder: {}", e))?;

    Ok(count as i64)
}

/// Relocate a single file - verify identity matches before updating
pub fn relocate_file(
    conn: &Connection,
    photo_file_id: i64,
    new_path: &Path,
) -> Result<(), String> {
    let existing = dao::get_photo_file(conn, photo_file_id)
        .map_err(|e| format!("Failed to get photo file: {}", e))?;

    let new_identity = identity::compute_identity(new_path)
        .map_err(|e| format!("Failed to compute identity: {}", e))?;

    if existing.content_hash != new_identity.content_hash || existing.file_size != new_identity.file_size as i64 {
        return Err("Identity mismatch: new file does not match original".to_string());
    }

    conn.execute(
        "UPDATE photo_files SET path = ?1, status = 'available', missing_since = NULL, mtime = ?2 WHERE id = ?3",
        rusqlite::params![new_path.to_string_lossy(), new_identity.mtime, photo_file_id],
    )
    .map_err(|e| format!("Failed to update path: {}", e))?;

    Ok(())
}

/// Build a HashMap of (dir, stem) -> PhotoCandidate from a directory walk,
/// used by tests and by the old non-streaming path.
pub fn discover_and_group(root: &Path) -> Vec<scanner::PhotoCandidate> {
    let files = scanner::discover_media_files(root);
    scanner::group_into_candidates(files)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::db;
    use std::fs::File;
    use std::io::Write;
    use tempfile::TempDir;

    /// Generate thumbnails for all photos in a library (mirrors commands::generate_thumbnails_for_library)
    fn generate_thumbnails_for_library_test(conn: &Connection, library_id: i64) {
        let thumbs = crate::core::db::thumbs_dir();
        let mut stmt = conn
            .prepare(
                "SELECT p.id, p.orientation, pf.content_hash, pf.path, pf.id, pf.format
                 FROM photos p
                 JOIN photo_files pf ON p.display_file_id = pf.id
                 WHERE p.library_id = ?1
                 AND NOT EXISTS (SELECT 1 FROM thumbnails t WHERE t.photo_id = p.id AND t.tier = 240)",
            )
            .unwrap();

        let rows: Vec<(i64, Option<i64>, String, String, i64, String)> = stmt
            .query_map(rusqlite::params![library_id], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?))
            })
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        for (photo_id, orientation, hash, path, display_file_id, _format) in rows {
            let img_path = Path::new(&path);
            if !img_path.exists() {
                continue;
            }
            let orient = orientation.map(|o| o as u32).unwrap_or(1);
            match crate::core::thumbnail::generate_thumbnails(img_path, &thumbs, &hash, Some(orient)) {
                Ok(results) => {
                    for thumb in results {
                        let _ = dao::insert_thumbnail(conn, photo_id, display_file_id, &hash, thumb.tier as i64, thumb.width as i64, thumb.height as i64);
                    }
                }
                Err(e) => eprintln!("Thumbnail gen failed for {}: {}", photo_id, e),
            }
        }
    }

    fn setup_test_library() -> (TempDir, Connection, i64) {
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("test.sqlite");
        let conn = db::create_connection(&db_path).unwrap();
        db::run_migrations(&conn).unwrap();
        let lib = dao::insert_library(&conn, "Test Library", tmp.path().to_str().unwrap()).unwrap();
        (tmp, conn, lib.id)
    }

    fn create_test_image(dir: &Path, name: &str, content: &[u8]) -> PathBuf {
        let path = dir.join(name);
        let mut file = File::create(&path).unwrap();
        file.write_all(content).unwrap();
        path
    }

    #[test]
    fn test_first_scan_creates_photos() {
        let (tmp, conn, lib_id) = setup_test_library();
        create_test_image(tmp.path(), "photo1.jpg", b"image data 1");
        create_test_image(tmp.path(), "photo2.jpg", b"image data 2");

        let result = run_scan(&conn, lib_id, tmp.path(), None, None).unwrap();
        assert_eq!(result.added, 2);
        assert_eq!(result.updated, 0);
        assert_eq!(result.missing, 0);
    }

    #[test]
    fn test_scan_groups_raw_jpeg() {
        let (tmp, conn, lib_id) = setup_test_library();
        create_test_image(tmp.path(), "photo.jpg", b"jpeg data");
        create_test_image(tmp.path(), "photo.arw", b"raw data");

        let result = run_scan(&conn, lib_id, tmp.path(), None, None).unwrap();
        assert_eq!(result.added, 1);

        let mut stmt = conn.prepare("SELECT format, role FROM photo_files WHERE photo_id = 1").unwrap();
        let files: Vec<(String, String)> = stmt.query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?))
        }).unwrap().filter_map(|r| r.ok()).collect();

        assert_eq!(files.len(), 2);
        let display = files.iter().find(|(_, r)| r == "display").unwrap();
        assert_eq!(display.0, "jpeg");
        let raw = files.iter().find(|(_, r)| r == "raw").unwrap();
        assert_eq!(raw.0, "arw");
    }

    #[test]
    fn test_scan_idempotent() {
        let (tmp, conn, lib_id) = setup_test_library();
        create_test_image(tmp.path(), "photo.jpg", b"image data");

        let result1 = run_scan(&conn, lib_id, tmp.path(), None, None).unwrap();
        assert_eq!(result1.added, 1);

        let result2 = run_scan(&conn, lib_id, tmp.path(), None, None).unwrap();
        assert_eq!(result2.added, 0);
        assert_eq!(result2.updated, 0);
        assert_eq!(result2.missing, 0);
    }

    #[test]
    fn test_scan_detects_missing_files() {
        let (tmp, conn, lib_id) = setup_test_library();
        let path = create_test_image(tmp.path(), "photo.jpg", b"image data");

        run_scan(&conn, lib_id, tmp.path(), None, None).unwrap();
        std::fs::remove_file(&path).unwrap();

        let result = run_scan(&conn, lib_id, tmp.path(), None, None).unwrap();
        assert_eq!(result.missing, 1);
    }

    #[test]
    fn test_cancellation() {
        let (tmp, conn, lib_id) = setup_test_library();
        for i in 0..10 {
            create_test_image(tmp.path(), &format!("photo{}.jpg", i), b"image data");
        }

        let cancel = new_cancellation_token();
        cancel.store(true, Ordering::Relaxed);

        let result = run_scan(&conn, lib_id, tmp.path(), Some(&cancel), None);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("cancelled"));

        // Scan job should be paused
        let jobs = dao::get_resumable_scan_jobs(&conn, lib_id).unwrap();
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].status, "paused");
    }

    #[test]
    fn test_batch_transaction_commit() {
        let (tmp, conn, lib_id) = setup_test_library();
        // Create files with different content so they become separate photos
        for i in 0..5 {
            create_test_image(tmp.path(), &format!("photo{}.jpg", i), format!("image data {}", i).as_bytes());
        }

        let result = run_scan(&conn, lib_id, tmp.path(), None, None).unwrap();
        assert_eq!(result.added, 5);

        // All photos should be in the DB
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM photos WHERE library_id = ?1", [lib_id], |row| row.get(0)).unwrap();
        assert_eq!(count, 5);
    }

    #[test]
    fn test_cursor_updated_during_scan() {
        let (tmp, conn, lib_id) = setup_test_library();
        // Create a subdirectory with files
        let subdir = tmp.path().join("2024");
        std::fs::create_dir(&subdir).unwrap();
        create_test_image(&subdir, "photo1.jpg", b"image data 1");
        create_test_image(&subdir, "photo2.jpg", b"image data 2");

        let result = run_scan(&conn, lib_id, tmp.path(), None, None).unwrap();
        assert_eq!(result.added, 2);

        // Cursor should have been set
        let jobs = dao::get_resumable_scan_jobs(&conn, lib_id).unwrap();
        // Job should be done, not resumable
        assert_eq!(jobs.len(), 0);
    }

    #[test]
    fn test_resume_skips_files_before_cursor() {
        let (tmp, conn, lib_id) = setup_test_library();

        // Three sibling subdirectories, each with one JPEG. With
        // sort_by_file_name() enabled, walkdir visits them as a < b < c.
        for name in &["a", "b", "c"] {
            let sub = tmp.path().join(name);
            std::fs::create_dir(&sub).unwrap();
            create_test_image(&sub, "photo.jpg", format!("data {}", name).as_bytes());
        }

        // First full scan: all three photos added.
        let r1 = run_scan(&conn, lib_id, tmp.path(), None, None).unwrap();
        assert_eq!(r1.added, 3);

        // Resurrect the just-completed job as paused with cursor at relative dir "b".
        let job_id: i64 = conn.query_row(
            "SELECT id FROM scan_jobs WHERE library_id = ?1 ORDER BY id DESC LIMIT 1",
            [lib_id],
            |row| row.get(0),
        ).unwrap();
        conn.execute("UPDATE scan_jobs SET status = 'paused', finished_at = NULL WHERE id = ?1", [job_id]).unwrap();
        dao::update_scan_job_cursor(&conn, job_id, "b").unwrap();

        // Wipe photo state so `added` on resume reflects exactly which dirs were walked.
        // photos.display_file_id -> photo_files.id has no ON DELETE; null it first, then
        // cascade through photos (photo_files.photo_id has ON DELETE CASCADE).
        conn.execute("UPDATE photos SET display_file_id = NULL", []).unwrap();
        conn.execute("DELETE FROM photos", []).unwrap();

        // Resume: cursor="b" must skip dir "a" (rel "a" < "b") and process b, c.
        let r2 = run_scan(&conn, lib_id, tmp.path(), None, Some(job_id)).unwrap();
        assert_eq!(r2.added, 2, "resume should re-add only b and c, skipping a");
    }

    #[test]
    fn test_relocate_file() {
        let (tmp, conn, lib_id) = setup_test_library();
        let content = b"image data for relocation";
        let old_path = create_test_image(tmp.path(), "old_name.jpg", content);

        run_scan(&conn, lib_id, tmp.path(), None, None).unwrap();

        let new_path = tmp.path().join("new_name.jpg");
        std::fs::copy(&old_path, &new_path).unwrap();

        relocate_file(&conn, 1, &new_path).unwrap();

        let file = dao::get_photo_file(&conn, 1).unwrap();
        assert_eq!(file.path, new_path.to_string_lossy());
        assert_eq!(file.status, "available");
    }

    #[test]
    fn test_relocate_folder() {
        let (tmp, conn, lib_id) = setup_test_library();
        let subdir = tmp.path().join("subdir");
        std::fs::create_dir(&subdir).unwrap();
        create_test_image(&subdir, "photo.jpg", b"image data");

        run_scan(&conn, lib_id, tmp.path(), None, None).unwrap();

        let old_prefix = subdir.to_string_lossy().to_string();
        let new_prefix = tmp.path().join("newdir").to_string_lossy().to_string();
        std::fs::create_dir(tmp.path().join("newdir")).unwrap();

        relocate_folder(&conn, lib_id, &old_prefix, &new_prefix).unwrap();

        let file = dao::get_photo_file(&conn, 1).unwrap();
        assert!(file.path.contains("newdir"));
    }

    #[test]
    fn test_missing_since_populated() {
        let (tmp, conn, lib_id) = setup_test_library();
        let path = create_test_image(tmp.path(), "photo.jpg", b"image data");
        run_scan(&conn, lib_id, tmp.path(), None, None).unwrap();

        std::fs::remove_file(&path).unwrap();
        run_scan(&conn, lib_id, tmp.path(), None, None).unwrap();

        let file = dao::get_photo_file(&conn, 1).unwrap();
        assert_eq!(file.status, "missing");
        assert!(file.missing_since.is_some());
    }

    #[test]
    fn test_format_on_photo_files() {
        let (tmp, conn, lib_id) = setup_test_library();
        create_test_image(tmp.path(), "photo.jpg", b"jpeg data");

        run_scan(&conn, lib_id, tmp.path(), None, None).unwrap();

        let file = dao::get_photo_file(&conn, 1).unwrap();
        assert_eq!(file.format, "jpeg");
        assert_eq!(file.library_id, lib_id);
    }

    #[test]
    fn test_b6_jpeg_not_misclassified_as_raw() {
        let (tmp, conn, lib_id) = setup_test_library();
        create_test_image(tmp.path(), "photo.jpg", b"jpeg data 1");
        create_test_image(tmp.path(), "photo.jpeg", b"jpeg data 2");

        run_scan(&conn, lib_id, tmp.path(), None, None).unwrap();

        let mut stmt = conn.prepare("SELECT role FROM photo_files WHERE photo_id = 1").unwrap();
        let roles: Vec<String> = stmt.query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        assert!(roles.contains(&"display".to_string()));
        assert!(!roles.contains(&"raw".to_string()));
    }

    #[test]
    fn test_b11_sidecar_not_used_for_identity_matching() {
        let (tmp, conn, lib_id) = setup_test_library();
        create_test_image(tmp.path(), "photo1.jpg", b"image data 1");
        create_test_image(tmp.path(), "photo1.xmp", b"<xmp>identical</xmp>");
        create_test_image(tmp.path(), "photo2.jpg", b"image data 2");
        create_test_image(tmp.path(), "photo2.xmp", b"<xmp>identical</xmp>");

        let result = run_scan(&conn, lib_id, tmp.path(), None, None).unwrap();
        assert_eq!(result.added, 2);
    }

    /// Integration test: scan a real photo directory end-to-end
    #[test]
    #[ignore = "requires real photo library at E:\\TEMP\\Photos\\Share — run with --ignored locally"]
    fn test_e2e_real_photo_directory() {
        let real_dir = std::path::PathBuf::from(r"E:\TEMP\Photos\Share");
        if !real_dir.exists() {
            eprintln!("Real photo directory not found at {:?}, skipping", real_dir);
            return;
        }

        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("test.sqlite");
        let conn = db::create_connection(&db_path).unwrap();
        db::run_migrations(&conn).unwrap();

        let lib = dao::insert_library(&conn, "Real Photos", real_dir.to_str().unwrap()).unwrap();

        // Step 1: Scan
        let result = run_scan(&conn, lib.id, &real_dir, None, None).unwrap();
        println!("Scan result: added={}, updated={}, missing={}", result.added, result.updated, result.missing);
        assert!(result.added > 0, "Should have found at least one photo");

        // Step 1b: Generate thumbnails (same as library_scan command does)
        generate_thumbnails_for_library_test(&conn, lib.id);

        // Step 2: Verify photos in DB
        let photo_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM photos WHERE library_id = ?1",
            [lib.id],
            |row| row.get(0),
        ).unwrap();
        println!("Photos in DB: {}", photo_count);
        assert_eq!(photo_count, result.added);

        // Step 3: Verify photo_files have correct format and library_id
        let mut stmt = conn.prepare(
            "SELECT format, library_id, role FROM photo_files WHERE library_id = ?1"
        ).unwrap();
        let files: Vec<(String, i64, String)> = stmt.query_map([lib.id], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        }).unwrap().filter_map(|r| r.ok()).collect();

        assert!(!files.is_empty());
        for (fmt, lid, role) in &files {
            assert_eq!(*lid, lib.id, "library_id should match");
            assert_eq!(fmt, "jpeg", "All files should be jpeg format");
            assert_eq!(role, "display", "All files should be display role (no RAW pairs)");
        }
        println!("Photo files: {} entries", files.len());

        // Step 4: Verify thumbnails were generated
        let thumb_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM thumbnails",
            [],
            |row| row.get(0),
        ).unwrap();
        println!("Thumbnails in DB: {}", thumb_count);
        assert!(thumb_count > 0, "Thumbnails should have been generated");
        // Each photo should have 3 tiers (240, 480, 1080)
        assert_eq!(thumb_count, photo_count * 3, "Each photo should have 3 thumbnail tiers");

        // Step 5: Verify thumbnail files exist on disk
        let thumbs_dir = crate::core::db::thumbs_dir();
        let mut stmt = conn.prepare("SELECT source_hash, tier FROM thumbnails").unwrap();
        let thumbs: Vec<(String, i64)> = stmt.query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?))
        }).unwrap().filter_map(|r| r.ok()).collect();

        let mut missing_files = 0;
        for (hash, tier) in &thumbs {
            let path = crate::core::thumbnail::thumbnail_path(&thumbs_dir, *tier as u32, &hash);
            if !path.exists() {
                missing_files += 1;
                eprintln!("Missing thumbnail file: {:?}", path);
            }
        }
        assert_eq!(missing_files, 0, "All thumbnail files should exist on disk");
        println!("All {} thumbnail files verified on disk", thumbs.len());

        // Step 6: Verify EXIF metadata was extracted
        let mut stmt = conn.prepare(
            "SELECT id, taken_at, camera_make, camera_model, orientation FROM photos WHERE library_id = ?1"
        ).unwrap();
        let photos: Vec<(i64, Option<i64>, Option<String>, Option<String>, Option<i64>)> =
            stmt.query_map([lib.id], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?))
            }).unwrap().filter_map(|r| r.ok()).collect();

        let mut with_exif = 0;
        for (id, taken_at, make, model, orient) in &photos {
            if taken_at.is_some() || make.is_some() || model.is_some() {
                with_exif += 1;
            }
            println!("Photo {}: taken_at={:?}, camera={:?} {:?}, orient={:?}",
                id, taken_at, make, model, orient);
        }
        println!("{} of {} photos have EXIF metadata", with_exif, photos.len());

        // Step 7: Verify idempotent re-scan
        let result2 = run_scan(&conn, lib.id, &real_dir, None, None).unwrap();
        println!("Re-scan result: added={}, updated={}, missing={}", result2.added, result2.updated, result2.missing);
        assert_eq!(result2.added, 0, "Re-scan should not add new photos");
        assert_eq!(result2.missing, 0, "Re-scan should not mark any as missing");

        println!("=== E2E integration test PASSED ===");
    }
}

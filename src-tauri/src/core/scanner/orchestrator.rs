use crate::core::db::dao;
use crate::core::db::dao::PhotoFile;
use crate::core::exif;
use crate::core::identity;
use crate::core::scanner::{self, FileRole, MediaType};
use rusqlite::Connection;
use std::path::Path;

/// Result of processing a single file during scan
#[derive(Debug)]
pub struct ScanResult {
    pub added: i64,
    pub updated: i64,
    pub missing: i64,
}

/// Run a full scan of a library directory
pub fn run_scan(conn: &Connection, library_id: i64, root_path: &Path) -> Result<ScanResult, String> {
    // Create scan job
    let job = dao::create_scan_job(conn, library_id)
        .map_err(|e| format!("Failed to create scan job: {}", e))?;

    // Discover media files
    let files = scanner::discover_media_files(root_path);

    // Group into photo candidates
    let candidates = scanner::group_into_candidates(files);

    let mut added = 0i64;
    let mut updated = 0i64;

    for candidate in &candidates {
        match process_candidate(conn, library_id, job.id, candidate) {
            Ok(ProcessOutcome::Added) => added += 1,
            Ok(ProcessOutcome::Updated) => updated += 1,
            Ok(ProcessOutcome::Unchanged) => {}
            Err(e) => {
                eprintln!("Error processing {}: {}", candidate.stem, e);
            }
        }
    }

    // Mark files not seen in this scan as missing
    let missing = mark_missing_files(conn, library_id, job.id)?;

    // Update scan job status
    dao::update_scan_job_status(conn, job.id, "done", Some(added), Some(updated), Some(missing))
        .map_err(|e| format!("Failed to update scan job: {}", e))?;

    Ok(ScanResult {
        added,
        updated,
        missing,
    })
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
    // Get the roles for each file
    let roles = scanner::assign_roles(candidate);

    // Check if any file in this group already exists
    let mut existing_photo_id: Option<i64> = None;
    let mut existing_files: Vec<PhotoFile> = Vec::new();

    for (file, _role) in &roles {
        let identity = identity::compute_identity(&file.path)
            .map_err(|e| format!("Identity error for {}: {}", file.path.display(), e))?;

        let found = dao::find_photo_files_by_identity(conn, &identity.content_hash, identity.file_size as i64)
            .map_err(|e| format!("DB error: {}", e))?;

        if !found.is_empty() {
            if existing_photo_id.is_none() {
                existing_photo_id = Some(found[0].photo_id);
            }
            existing_files.extend(found);
        }
    }

    if let Some(photo_id) = existing_photo_id {
        // Existing photo - update files
        update_existing_photo(conn, photo_id, &roles, &existing_files, scan_job_id)?;
        Ok(ProcessOutcome::Updated)
    } else {
        // New photo - create it
        create_new_photo(conn, library_id, &roles, scan_job_id)?;
        Ok(ProcessOutcome::Added)
    }
}

/// Create a new photo and its file instances
fn create_new_photo(
    conn: &Connection,
    library_id: i64,
    roles: &[(&scanner::DiscoveredFile, FileRole)],
    scan_job_id: i64,
) -> Result<(), String> {
    // Determine format from display file or first raw file
    let format = roles.iter()
        .find(|(_, r)| *r == FileRole::Display)
        .or_else(|| roles.first())
        .map(|(f, _)| match f.media_type {
            MediaType::Display => f.extension.clone(),
            MediaType::Raw => f.extension.clone(),
            MediaType::Sidecar => "unknown".to_string(),
        })
        .unwrap_or_else(|| "unknown".to_string());

    // Extract metadata from display file or first raw
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

    // If we have both display and raw, merge metadata
    if display_file.is_some() && raw_file.is_some() {
        let raw_metadata = exif::extract_exif(&raw_file.unwrap().path, mtime);
        metadata = exif::merge_metadata(metadata, Some(raw_metadata));
    }

    // Read XMP sidecar if present
    if let Some(sidecar) = sidecar_file {
        let xmp = exif::extract_xmp(&sidecar.path);
        if let Some(rating) = xmp.rating {
            metadata.rating = Some(rating);
        }
        metadata.label = xmp.label;
    }

    // Create photo record
    let photo = dao::insert_photo(
        conn,
        library_id,
        &format,
        metadata.taken_at,
        metadata.taken_at_src.as_deref(),
    )
    .map_err(|e| format!("Failed to insert photo: {}", e))?;

    // Update photo metadata
    conn.execute(
        "UPDATE photos SET camera_make = ?1, camera_model = ?2, lens = ?3,
         focal_len = ?4, aperture = ?5, shutter = ?6, iso = ?7,
         width = ?8, height = ?9, orientation = ?10,
         gps_lat = ?11, gps_lon = ?12, rating = ?13, label = ?14
         WHERE id = ?15",
        rusqlite::params![
            metadata.camera_make,
            metadata.camera_model,
            metadata.lens,
            metadata.focal_len,
            metadata.aperture,
            metadata.shutter,
            metadata.iso,
            metadata.width,
            metadata.height,
            metadata.orientation,
            metadata.gps_lat,
            metadata.gps_lon,
            metadata.rating,
            metadata.label,
            photo.id,
        ],
    )
    .map_err(|e| format!("Failed to update photo metadata: {}", e))?;

    // Create photo file records
    let mut display_file_id = None;
    for (file, role) in roles {
        let identity = identity::compute_identity(&file.path)
            .map_err(|e| format!("Identity error: {}", e))?;

        let file_record = dao::insert_photo_file(
            conn,
            photo.id,
            &file.path.to_string_lossy(),
            &identity.content_hash,
            identity.file_size as i64,
            identity.mtime,
            role.as_str(),
            Some(scan_job_id),
        )
        .map_err(|e| format!("Failed to insert photo file: {}", e))?;

        if *role == FileRole::Display {
            display_file_id = Some(file_record.id);
        }
    }

    // Set display_file_id
    if let Some(display_id) = display_file_id {
        conn.execute(
            "UPDATE photos SET display_file_id = ?1 WHERE id = ?2",
            rusqlite::params![display_id, photo.id],
        )
        .map_err(|e| format!("Failed to set display file: {}", e))?;
    }

    Ok(())
}

/// Update an existing photo with new file instances
fn update_existing_photo(
    conn: &Connection,
    photo_id: i64,
    roles: &[(&scanner::DiscoveredFile, FileRole)],
    existing_files: &[PhotoFile],
    scan_job_id: i64,
) -> Result<(), String> {
    for (file, role) in roles {
        let identity = identity::compute_identity(&file.path)
            .map_err(|e| format!("Identity error: {}", e))?;

        let path_str = file.path.to_string_lossy();

        // Check if this exact path already exists
        if let Some(existing) = existing_files.iter().find(|f| f.path == path_str) {
            // Same path - check if content changed
            if existing.content_hash == identity.content_hash && existing.file_size == identity.file_size as i64 {
                // Same content - just update last_seen_at
                dao::update_photo_file_last_seen(conn, existing.id, Some(scan_job_id))
                    .map_err(|e| format!("Failed to update last_seen: {}", e))?;
            } else {
                // Content changed - mark old as missing, add new
                dao::update_photo_file_status(conn, existing.id, "missing")
                    .map_err(|e| format!("Failed to mark missing: {}", e))?;

                dao::insert_photo_file(
                    conn,
                    photo_id,
                    &path_str,
                    &identity.content_hash,
                    identity.file_size as i64,
                    identity.mtime,
                    role.as_str(),
                    Some(scan_job_id),
                )
                .map_err(|e| format!("Failed to insert new file: {}", e))?;
            }
        } else {
            // New path for existing photo
            dao::insert_photo_file(
                conn,
                photo_id,
                &path_str,
                &identity.content_hash,
                identity.file_size as i64,
                identity.mtime,
                role.as_str(),
                Some(scan_job_id),
            )
            .map_err(|e| format!("Failed to insert file: {}", e))?;
        }
    }

    Ok(())
}

/// Mark files not seen in the current scan as missing
fn mark_missing_files(conn: &Connection, library_id: i64, scan_job_id: i64) -> Result<i64, String> {
    let count = conn.execute(
        "UPDATE photo_files SET status = 'missing'
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
    // Get existing file record
    let existing = dao::get_photo_file(conn, photo_file_id)
        .map_err(|e| format!("Failed to get photo file: {}", e))?;

    // Compute identity of new path
    let new_identity = identity::compute_identity(new_path)
        .map_err(|e| format!("Failed to compute identity: {}", e))?;

    // Verify identity matches
    if existing.content_hash != new_identity.content_hash || existing.file_size != new_identity.file_size as i64 {
        return Err("Identity mismatch: new file does not match original".to_string());
    }

    // Update path
    conn.execute(
        "UPDATE photo_files SET path = ?1, status = 'available', mtime = ?2 WHERE id = ?3",
        rusqlite::params![new_path.to_string_lossy(), new_identity.mtime, photo_file_id],
    )
    .map_err(|e| format!("Failed to update path: {}", e))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::db;
    use std::fs::File;
    use std::io::Write;
    use std::path::PathBuf;
    use tempfile::TempDir;

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

        // Create test files
        create_test_image(tmp.path(), "photo1.jpg", b"image data 1");
        create_test_image(tmp.path(), "photo2.jpg", b"image data 2");

        let result = run_scan(&conn, lib_id, tmp.path()).unwrap();
        assert_eq!(result.added, 2);
        assert_eq!(result.updated, 0);
        assert_eq!(result.missing, 0);

        // Verify photos exist
        let photos = dao::get_photo(&conn, 1);
        assert!(photos.is_ok());
    }

    #[test]
    fn test_scan_groups_raw_jpeg() {
        let (tmp, conn, lib_id) = setup_test_library();

        // Create RAW+JPEG pair
        create_test_image(tmp.path(), "photo.jpg", b"jpeg data");
        create_test_image(tmp.path(), "photo.arw", b"raw data");

        let result = run_scan(&conn, lib_id, tmp.path()).unwrap();
        assert_eq!(result.added, 1); // Should be one logical photo

        // Verify photo has both files
        let photo = dao::get_photo(&conn, 1).unwrap();
        assert_eq!(photo.format, "jpg"); // Display file format
    }

    #[test]
    fn test_scan_idempotent() {
        let (tmp, conn, lib_id) = setup_test_library();

        create_test_image(tmp.path(), "photo.jpg", b"image data");

        // First scan
        let result1 = run_scan(&conn, lib_id, tmp.path()).unwrap();
        assert_eq!(result1.added, 1);

        // Second scan - same files
        let result2 = run_scan(&conn, lib_id, tmp.path()).unwrap();
        assert_eq!(result2.added, 0);
        assert_eq!(result2.updated, 1);
        assert_eq!(result2.missing, 0);
    }

    #[test]
    fn test_scan_detects_missing_files() {
        let (tmp, conn, lib_id) = setup_test_library();

        let path = create_test_image(tmp.path(), "photo.jpg", b"image data");

        // First scan
        run_scan(&conn, lib_id, tmp.path()).unwrap();

        // Delete the file
        std::fs::remove_file(&path).unwrap();

        // Second scan
        let result = run_scan(&conn, lib_id, tmp.path()).unwrap();
        assert_eq!(result.missing, 1);
    }

    #[test]
    fn test_relocate_file_success() {
        let (tmp, conn, lib_id) = setup_test_library();

        let content = b"image data for relocation";
        let old_path = create_test_image(tmp.path(), "old_name.jpg", content);

        // Scan to create the photo
        run_scan(&conn, lib_id, tmp.path()).unwrap();

        // Create new path with same content
        let new_path = tmp.path().join("new_name.jpg");
        std::fs::copy(&old_path, &new_path).unwrap();

        // Relocate
        relocate_file(&conn, 1, &new_path).unwrap();

        // Verify
        let file = dao::get_photo_file(&conn, 1).unwrap();
        assert_eq!(file.path, new_path.to_string_lossy());
        assert_eq!(file.status, "available");
    }

    #[test]
    fn test_relocate_file_identity_mismatch() {
        let (tmp, conn, lib_id) = setup_test_library();

        create_test_image(tmp.path(), "photo.jpg", b"original content");

        // Scan to create the photo
        run_scan(&conn, lib_id, tmp.path()).unwrap();

        // Create new path with different content
        let new_path = tmp.path().join("different.jpg");
        let mut file = File::create(&new_path).unwrap();
        file.write_all(b"completely different content").unwrap();

        // Relocate should fail
        let result = relocate_file(&conn, 1, &new_path);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Identity mismatch"));
    }

    #[test]
    fn test_relocate_folder() {
        let (tmp, conn, lib_id) = setup_test_library();

        // Create a subdirectory with files
        let subdir = tmp.path().join("subdir");
        std::fs::create_dir(&subdir).unwrap();
        create_test_image(&subdir, "photo.jpg", b"image data");

        // Scan
        run_scan(&conn, lib_id, tmp.path()).unwrap();

        // Verify original path
        let file = dao::get_photo_file(&conn, 1).unwrap();
        assert!(file.path.contains("subdir"));

        // Relocate folder
        let old_prefix = subdir.to_string_lossy().to_string();
        let new_prefix = tmp.path().join("newdir").to_string_lossy().to_string();
        std::fs::create_dir(tmp.path().join("newdir")).unwrap();

        relocate_folder(&conn, lib_id, &old_prefix, &new_prefix).unwrap();

        // Verify new path
        let file = dao::get_photo_file(&conn, 1).unwrap();
        assert!(file.path.contains("newdir"));
        assert!(!file.path.contains("subdir"));
    }
}

use crate::core::db::dao;
use crate::core::db::thumbs_dir;
use crate::core::raw;
use crate::core::scanner::orchestrator;
use crate::core::thumbnail;
use rusqlite::Connection;
use serde::Serialize;
use std::path::Path;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

/// RAW formats that need embedded JPEG extraction for thumbnails
const RAW_FORMATS: &[&str] = &["arw", "cr2", "cr3", "nef", "orf", "rw2", "dng", "raf"];

/// Check if a format string is a RAW format
pub fn is_raw_format(format: &str) -> bool {
    RAW_FORMATS.contains(&format)
}

/// Application state shared across all Tauri commands.
pub struct AppState {
    pub db: Arc<Mutex<Connection>>,
    pub scan_cancel_tokens: Arc<Mutex<std::collections::HashMap<i64, orchestrator::CancellationToken>>>,
}

#[derive(Serialize)]
pub struct PhotoSummary {
    pub id: i64,
    pub taken_at: Option<i64>,
    pub content_hash: String,
    pub orientation: Option<i64>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub is_missing: bool,
    pub thumbnail_url: String,
}

#[derive(Serialize)]
pub struct TimelinePage {
    pub photos: Vec<PhotoSummary>,
    pub next_cursor: Option<String>,
}

#[derive(Serialize)]
pub struct PhotoDetail {
    pub id: i64,
    pub library_id: i64,
    pub taken_at: Option<i64>,
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
    pub is_missing: bool,
    pub files: Vec<PhotoFileInfo>,
}

#[derive(Serialize)]
pub struct PhotoFileInfo {
    pub id: i64,
    pub path: String,
    pub role: String,
    pub format: String,
    pub status: String,
}

#[tauri::command]
pub fn library_list(state: tauri::State<'_, AppState>) -> Result<Vec<dao::Library>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    dao::list_libraries(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn library_add(state: tauri::State<'_, AppState>, path: String) -> Result<dao::Library, String> {
    let p = Path::new(&path);
    if !p.exists() {
        return Err(format!("Path does not exist: {}", path));
    }
    if !p.is_dir() {
        return Err(format!("Path is not a directory: {}", path));
    }

    let conn = state.db.lock().map_err(|e| e.to_string())?;

    let name = p
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.clone());

    dao::insert_library(&conn, &name, &path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn library_scan(state: tauri::State<'_, AppState>, id: i64) -> Result<dao::ScanJob, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let library = dao::get_library(&conn, id).map_err(|e| format!("Library not found: {}", e))?;

    let root = Path::new(&library.root_path);
    if !root.exists() {
        return Err(format!("Library root path does not exist: {}", library.root_path));
    }

    // Create cancellation token and register it
    let cancel = orchestrator::new_cancellation_token();
    {
        let mut tokens = state.scan_cancel_tokens.lock().map_err(|e| e.to_string())?;
        tokens.insert(id, cancel.clone());
    }

    // Run scan synchronously with batched transactions
    let result = orchestrator::run_scan(&conn, id, root, Some(&cancel), None);

    // Remove cancellation token
    {
        let mut tokens = state.scan_cancel_tokens.lock().map_err(|e| e.to_string())?;
        tokens.remove(&id);
    }

    result?;

    // After scan, generate thumbnails for newly added photos
    generate_thumbnails_for_library(&conn, id)?;

    let job = get_latest_scan_job(&conn, id)?;
    Ok(job)
}

#[tauri::command]
pub fn library_scan_pause(state: tauri::State<'_, AppState>, id: i64) -> Result<(), String> {
    let tokens = state.scan_cancel_tokens.lock().map_err(|e| e.to_string())?;
    if let Some(token) = tokens.get(&id) {
        token.store(true, Ordering::Relaxed);
        Ok(())
    } else {
        Err("No active scan found for this library".to_string())
    }
}

#[tauri::command]
pub fn library_scan_resume(state: tauri::State<'_, AppState>, id: i64) -> Result<dao::ScanJob, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let library = dao::get_library(&conn, id).map_err(|e| format!("Library not found: {}", e))?;

    let root = Path::new(&library.root_path);
    if !root.exists() {
        return Err(format!("Library root path does not exist: {}", library.root_path));
    }

    // Find the paused scan job
    let resumable = dao::get_resumable_scan_jobs(&conn, id)
        .map_err(|e| format!("Failed to get resumable jobs: {}", e))?;
    let job = resumable.into_iter().find(|j| j.status == "paused")
        .ok_or_else(|| "No paused scan job found".to_string())?;

    // Create cancellation token
    let cancel = orchestrator::new_cancellation_token();
    {
        let mut tokens = state.scan_cancel_tokens.lock().map_err(|e| e.to_string())?;
        tokens.insert(id, cancel.clone());
    }

    // Resume scan from cursor
    let result = orchestrator::run_scan(&conn, id, root, Some(&cancel), Some(job.id));

    {
        let mut tokens = state.scan_cancel_tokens.lock().map_err(|e| e.to_string())?;
        tokens.remove(&id);
    }

    result?;

    generate_thumbnails_for_library(&conn, id)?;

    let job = get_latest_scan_job(&conn, id)?;
    Ok(job)
}

fn generate_thumbnails_for_library(conn: &Connection, library_id: i64) -> Result<(), String> {
    let thumbs = thumbs_dir();

    // Find photos without thumbnails, including the display file format
    let mut stmt = conn
        .prepare(
            "SELECT p.id, p.orientation, pf.content_hash, pf.path, pf.id, pf.format
             FROM photos p
             JOIN photo_files pf ON p.display_file_id = pf.id
             WHERE p.library_id = ?1
             AND NOT EXISTS (SELECT 1 FROM thumbnails t WHERE t.photo_id = p.id AND t.tier = 240)",
        )
        .map_err(|e| e.to_string())?;

    let rows: Vec<(i64, Option<i64>, String, String, i64, String)> = stmt
        .query_map(rusqlite::params![library_id], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
            ))
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    for (photo_id, orientation, hash, path, display_file_id, format) in rows {
        let img_path = Path::new(&path);
        if !img_path.exists() {
            continue;
        }

        let orient = orientation.map(|o| o as u32).unwrap_or(1);

        // For RAW display files, extract embedded JPEG first
        let result = if RAW_FORMATS.contains(&format.as_str()) {
            match raw::extract_embedded_jpeg(img_path) {
                Ok(jpeg_bytes) => {
                    thumbnail::generate_thumbnails_from_bytes(&jpeg_bytes, &thumbs, &hash, Some(orient))
                }
                Err(e) => {
                    eprintln!("Failed to extract embedded JPEG from RAW {}: {}", path, e);
                    // Fall back to trying direct open (works for some DNG files)
                    thumbnail::generate_thumbnails(img_path, &thumbs, &hash, Some(orient))
                }
            }
        } else {
            thumbnail::generate_thumbnails(img_path, &thumbs, &hash, Some(orient))
        };

        match result {
            Ok(results) => {
                for (tier, _) in results {
                    let _ = dao::insert_thumbnail(
                        conn,
                        photo_id,
                        display_file_id,
                        &hash,
                        tier as i64,
                        tier as i64,
                        tier as i64,
                    );
                }
            }
            Err(e) => {
                eprintln!("Failed to generate thumbnails for photo {}: {}", photo_id, e);
            }
        }
    }

    Ok(())
}

fn get_latest_scan_job(conn: &Connection, library_id: i64) -> Result<dao::ScanJob, String> {
    conn.query_row(
        "SELECT id, library_id, status, cursor, started_at, finished_at, added, updated, missing
         FROM scan_jobs WHERE library_id = ?1 ORDER BY id DESC LIMIT 1",
        [library_id],
        |row| {
            Ok(dao::ScanJob {
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
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn library_relocate_folder(
    state: tauri::State<'_, AppState>,
    library_id: i64,
    old_prefix: String,
    new_prefix: String,
) -> Result<i64, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    orchestrator::relocate_folder(&conn, library_id, &old_prefix, &new_prefix)
}

#[tauri::command]
pub fn timeline_query(
    state: tauri::State<'_, AppState>,
    library_id: i64,
    cursor: Option<String>,
    limit: Option<i64>,
) -> Result<TimelinePage, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let limit = limit.unwrap_or(200);

    let (cursor_taken_at, cursor_photo_id) = if let Some(ref c) = cursor {
        parse_cursor(c)?
    } else {
        (None, None)
    };

    let sql = if cursor_taken_at.is_some() {
        "SELECT p.id, p.taken_at, p.orientation, p.width, p.height,
                pf.content_hash,
                CASE WHEN pf.status = 'missing' THEN 1 ELSE 0 END as is_missing
         FROM photos p
         LEFT JOIN photo_files pf ON p.display_file_id = pf.id
         WHERE p.library_id = ?1
           AND (p.taken_at < ?2 OR (p.taken_at = ?2 AND p.id < ?3))
         ORDER BY p.taken_at DESC, p.id DESC
         LIMIT ?4"
    } else {
        "SELECT p.id, p.taken_at, p.orientation, p.width, p.height,
                pf.content_hash,
                CASE WHEN pf.status = 'missing' THEN 1 ELSE 0 END as is_missing
         FROM photos p
         LEFT JOIN photo_files pf ON p.display_file_id = pf.id
         WHERE p.library_id = ?1
         ORDER BY p.taken_at DESC, p.id DESC
         LIMIT ?2"
    };

    let mut stmt = conn.prepare(sql).map_err(|e| e.to_string())?;

    let photos: Vec<PhotoSummary> = if let (Some(ta), Some(pid)) = (cursor_taken_at, cursor_photo_id) {
        let rows = stmt
            .query_map(rusqlite::params![library_id, ta, pid, limit], |row| {
                let photo_id: i64 = row.get(0)?;
                let taken_at: Option<i64> = row.get(1)?;
                let orientation: Option<i64> = row.get(2)?;
                let width: Option<i64> = row.get(3)?;
                let height: Option<i64> = row.get(4)?;
                let content_hash: Option<String> = row.get(5)?;
                let is_missing: bool = row.get::<_, i64>(6)? != 0;

                Ok(PhotoSummary {
                    id: photo_id,
                    taken_at,
                    content_hash: content_hash.unwrap_or_default(),
                    orientation,
                    width,
                    height,
                    is_missing,
                    thumbnail_url: format!("asset://thumb/{}/240", photo_id),
                })
            })
            .map_err(|e| e.to_string())?;
        rows.filter_map(|r| r.ok()).collect()
    } else {
        let rows = stmt
            .query_map(rusqlite::params![library_id, limit], |row| {
                let photo_id: i64 = row.get(0)?;
                let taken_at: Option<i64> = row.get(1)?;
                let orientation: Option<i64> = row.get(2)?;
                let width: Option<i64> = row.get(3)?;
                let height: Option<i64> = row.get(4)?;
                let content_hash: Option<String> = row.get(5)?;
                let is_missing: bool = row.get::<_, i64>(6)? != 0;

                Ok(PhotoSummary {
                    id: photo_id,
                    taken_at,
                    content_hash: content_hash.unwrap_or_default(),
                    orientation,
                    width,
                    height,
                    is_missing,
                    thumbnail_url: format!("asset://thumb/{}/240", photo_id),
                })
            })
            .map_err(|e| e.to_string())?;
        rows.filter_map(|r| r.ok()).collect()
    };

    let next_cursor = photos.last().and_then(|p| {
        if photos.len() as i64 >= limit {
            p.taken_at.map(|ta| format!("{}:{}", ta, p.id))
        } else {
            None
        }
    });

    Ok(TimelinePage {
        photos,
        next_cursor,
    })
}

fn parse_cursor(cursor: &str) -> Result<(Option<i64>, Option<i64>), String> {
    let parts: Vec<&str> = cursor.splitn(2, ':').collect();
    if parts.len() != 2 {
        return Err("Invalid cursor format".to_string());
    }
    let taken_at: i64 = parts[0].parse().map_err(|_| "Invalid cursor taken_at")?;
    let photo_id: i64 = parts[1].parse().map_err(|_| "Invalid cursor photo_id")?;
    Ok((Some(taken_at), Some(photo_id)))
}

#[tauri::command]
pub fn photo_detail(state: tauri::State<'_, AppState>, id: i64) -> Result<PhotoDetail, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;

    let photo = dao::get_photo(&conn, id).map_err(|e| format!("Photo not found: {}", e))?;

    // Get all photo files for this photo
    let mut stmt = conn
        .prepare(
            "SELECT id, path, role, format, status FROM photo_files WHERE photo_id = ?1",
        )
        .map_err(|e| e.to_string())?;

    let files: Vec<PhotoFileInfo> = stmt
        .query_map([id], |row| {
            Ok(PhotoFileInfo {
                id: row.get(0)?,
                path: row.get(1)?,
                role: row.get(2)?,
                format: row.get(3)?,
                status: row.get(4)?,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    let is_missing = files.iter().all(|f| f.role != "display" || f.status == "missing");

    Ok(PhotoDetail {
        id: photo.id,
        library_id: photo.library_id,
        taken_at: photo.taken_at,
        camera_make: photo.camera_make,
        camera_model: photo.camera_model,
        lens: photo.lens,
        focal_len: photo.focal_len,
        aperture: photo.aperture,
        shutter: photo.shutter,
        iso: photo.iso,
        width: photo.width,
        height: photo.height,
        orientation: photo.orientation,
        gps_lat: photo.gps_lat,
        gps_lon: photo.gps_lon,
        rating: photo.rating,
        label: photo.label,
        is_missing,
        files,
    })
}

#[tauri::command]
pub fn photo_relocate_file(
    state: tauri::State<'_, AppState>,
    photo_file_id: i64,
    new_path: String,
) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    orchestrator::relocate_file(&conn, photo_file_id, Path::new(&new_path))
}

#[tauri::command]
pub fn thumbnail_url(
    state: tauri::State<'_, AppState>,
    photo_id: i64,
    tier: i64,
) -> Result<String, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let _ = dao::get_photo(&conn, photo_id).map_err(|e| format!("Photo not found: {}", e))?;
    Ok(format!("asset://thumb/{}/{}", photo_id, tier))
}

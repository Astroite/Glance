use crate::core::db::dao;
use crate::core::raw;
use crate::core::scanner::orchestrator;
use crate::core::tasks::{Task, TaskQueue};
use crate::core::thumbnail;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::Connection;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use tauri::Emitter;

pub type DbPool = Pool<SqliteConnectionManager>;

/// RAW formats that need embedded JPEG extraction for thumbnails
const RAW_FORMATS: &[&str] = &["arw", "cr2", "cr3", "nef", "orf", "rw2", "dng", "raf"];

/// Check if a format string is a RAW format
pub fn is_raw_format(format: &str) -> bool {
    RAW_FORMATS.contains(&format)
}

/// Application state shared across all Tauri commands.
pub struct AppState {
    pub db: DbPool,
    pub scan_cancel_tokens: Arc<Mutex<std::collections::HashMap<i64, orchestrator::CancellationToken>>>,
    pub task_queue: Arc<TaskQueue>,
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
    let conn = state.db.get().map_err(|e| format!("Failed to get DB connection: {}", e))?;
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

    let conn = state.db.get().map_err(|e| format!("Failed to get DB connection: {}", e))?;

    let name = p
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.clone());

    dao::insert_library(&conn, &name, &path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn library_scan(state: tauri::State<'_, AppState>, id: i64) -> Result<(), String> {
    let conn = state.db.get().map_err(|e| format!("Failed to get DB connection: {}", e))?;
    let library = dao::get_library(&conn, id).map_err(|e| format!("Library not found: {}", e))?;

    let root = Path::new(&library.root_path);
    if !root.exists() {
        return Err(format!("Library root path does not exist: {}", library.root_path));
    }

    // Reject if a scan is already running for this library
    {
        let tokens = state.scan_cancel_tokens.lock().map_err(|e| e.to_string())?;
        if tokens.contains_key(&id) {
            return Err("A scan is already running for this library".to_string());
        }
    }

    // Create cancellation token and register it
    let cancel = orchestrator::new_cancellation_token();
    {
        let mut tokens = state.scan_cancel_tokens.lock().map_err(|e| e.to_string())?;
        tokens.insert(id, cancel.clone());
    }

    // Enqueue scan on the background IO pool
    state.task_queue.enqueue(Task::ScanLibrary { library_id: id, resume_job_id: None });
    Ok(())
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
pub fn library_scan_resume(state: tauri::State<'_, AppState>, id: i64) -> Result<(), String> {
    let conn = state.db.get().map_err(|e| format!("Failed to get DB connection: {}", e))?;
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

    // Reject if a scan is already running for this library
    {
        let tokens = state.scan_cancel_tokens.lock().map_err(|e| e.to_string())?;
        if tokens.contains_key(&id) {
            return Err("A scan is already running for this library".to_string());
        }
    }

    // Create cancellation token
    let cancel = orchestrator::new_cancellation_token();
    {
        let mut tokens = state.scan_cancel_tokens.lock().map_err(|e| e.to_string())?;
        tokens.insert(id, cancel.clone());
    }

    // Enqueue resume scan on the background IO pool
    state.task_queue.enqueue(Task::ScanLibrary { library_id: id, resume_job_id: Some(job.id) });
    Ok(())
}

/// Find photos in this library that don't yet have all three thumbnail tiers.
fn pending_thumbnail_photo_ids(conn: &Connection, library_id: i64) -> Result<Vec<i64>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT p.id FROM photos p
             LEFT JOIN thumbnails t ON t.photo_id = p.id
             WHERE p.library_id = ?1
             GROUP BY p.id
             HAVING COUNT(t.tier) < 3",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(rusqlite::params![library_id], |row| row.get::<_, i64>(0))
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

/// Generate all three thumbnail tiers for each photo_id, using a fresh pool
/// connection. Invoked from the TaskQueue handler on the CPU pool; errors are
/// logged and skipped — a panic here would kill the worker thread.
pub fn run_thumbnail_prefetch(pool: &DbPool, thumbs: &Path, photo_ids: &[i64]) {
    let conn = match pool.get() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("ThumbnailPrefetch: failed to acquire DB connection: {}", e);
            return;
        }
    };

    for &photo_id in photo_ids {
        let row: rusqlite::Result<(Option<i64>, String, String, i64, String)> = conn.query_row(
            "SELECT p.orientation, pf.content_hash, pf.path, pf.id, pf.format
             FROM photos p
             JOIN photo_files pf ON p.display_file_id = pf.id
             WHERE p.id = ?1",
            [photo_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
        );

        let (orientation, hash, path, display_file_id, format) = match row {
            Ok(r) => r,
            Err(e) => {
                eprintln!("ThumbnailPrefetch: photo {} lookup failed: {}", photo_id, e);
                continue;
            }
        };

        let img_path = Path::new(&path);
        if !img_path.exists() {
            continue;
        }

        let orient = orientation.map(|o| o as u32).unwrap_or(1);

        let result = if RAW_FORMATS.contains(&format.as_str()) {
            match raw::extract_embedded_jpeg(img_path) {
                Ok(jpeg_bytes) => {
                    thumbnail::generate_thumbnails_from_bytes(&jpeg_bytes, thumbs, &hash, Some(orient))
                }
                Err(e) => {
                    eprintln!("ThumbnailPrefetch: embedded JPEG extract failed for {}: {}", path, e);
                    thumbnail::generate_thumbnails(img_path, thumbs, &hash, Some(orient))
                }
            }
        } else {
            thumbnail::generate_thumbnails(img_path, thumbs, &hash, Some(orient))
        };

        match result {
            Ok(thumbs_out) => {
                for thumb in thumbs_out {
                    if let Err(e) = dao::insert_thumbnail(
                        &conn,
                        photo_id,
                        display_file_id,
                        &hash,
                        thumb.tier as i64,
                        thumb.width as i64,
                        thumb.height as i64,
                    ) {
                        eprintln!("ThumbnailPrefetch: insert_thumbnail failed for photo {}: {}", photo_id, e);
                    }
                }
            }
            Err(e) => {
                eprintln!("ThumbnailPrefetch: thumbnail gen failed for photo {}: {}", photo_id, e);
            }
        }
    }
}

/// Shared handle to the TaskQueue, populated after construction.
pub type SharedTaskQueue = Arc<std::sync::OnceLock<Arc<TaskQueue>>>;

/// Construct a handler that only processes ThumbnailPrefetch — used in tests
/// where a Tauri AppHandle is not available.
pub fn make_thumbnail_handler(
    pool: DbPool,
    thumbs: PathBuf,
) -> impl Fn(&Task, &crate::core::tasks::CancellationToken) + Send + Sync + 'static {
    move |task, _cancel| match task {
        Task::ThumbnailPrefetch { photo_ids } => {
            run_thumbnail_prefetch(&pool, &thumbs, photo_ids);
        }
        other => {
            eprintln!("TaskQueue: variant {:?} not yet wired", other);
        }
    }
}

/// Construct the full handler closure used by the lib-wide TaskQueue,
/// including ScanLibrary support with event emission.
pub fn make_task_handler(
    pool: DbPool,
    thumbs: PathBuf,
    app_handle: tauri::AppHandle,
    scan_cancel_tokens: Arc<Mutex<std::collections::HashMap<i64, orchestrator::CancellationToken>>>,
    task_queue_ref: SharedTaskQueue,
) -> impl Fn(&Task, &crate::core::tasks::CancellationToken) + Send + Sync + 'static {
    move |task, _cancel| match task {
        Task::ThumbnailPrefetch { photo_ids } => {
            run_thumbnail_prefetch(&pool, &thumbs, photo_ids);
        }
        Task::ScanLibrary { library_id, resume_job_id } => {
            run_scan_task(&pool, &thumbs, &app_handle, &scan_cancel_tokens, &task_queue_ref, *library_id, *resume_job_id);
        }
        other => {
            eprintln!("TaskQueue: variant {:?} not yet wired", other);
        }
    }
}

/// Execute a library scan on the IO worker, then emit events and enqueue thumbnails.
fn run_scan_task(
    pool: &DbPool,
    _thumbs: &Path,
    app_handle: &tauri::AppHandle,
    scan_cancel_tokens: &Arc<Mutex<std::collections::HashMap<i64, orchestrator::CancellationToken>>>,
    task_queue_ref: &SharedTaskQueue,
    library_id: i64,
    resume_job_id: Option<i64>,
) {
    let conn = match pool.get() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("ScanLibrary: failed to acquire DB connection: {}", e);
            let _ = app_handle.emit("scan-error", serde_json::json!({ "library_id": library_id, "error": e.to_string() }));
            return;
        }
    };

    let library = match dao::get_library(&conn, library_id) {
        Ok(lib) => lib,
        Err(e) => {
            eprintln!("ScanLibrary: library not found: {}", e);
            let _ = app_handle.emit("scan-error", serde_json::json!({ "library_id": library_id, "error": e.to_string() }));
            return;
        }
    };

    let root = Path::new(&library.root_path);
    if !root.exists() {
        let msg = format!("Library root path does not exist: {}", library.root_path);
        eprintln!("ScanLibrary: {}", msg);
        let _ = app_handle.emit("scan-error", serde_json::json!({ "library_id": library_id, "error": msg }));
        return;
    }

    // Get the cancel token for this library
    let cancel_token = {
        let tokens = match scan_cancel_tokens.lock() {
            Ok(t) => t,
            Err(e) => {
                eprintln!("ScanLibrary: failed to lock cancel tokens: {}", e);
                return;
            }
        };
        tokens.get(&library_id).cloned()
    };

    let cancel_ref = cancel_token.as_ref();

    // Run the scan
    let result = orchestrator::run_scan(&conn, library_id, root, cancel_ref, resume_job_id);

    // Clean up cancel token
    {
        if let Ok(mut tokens) = scan_cancel_tokens.lock() {
            tokens.remove(&library_id);
        }
    }

    match result {
        Ok(scan_result) => {
            // Enqueue thumbnail generation for newly scanned photos
            match pending_thumbnail_photo_ids(&conn, library_id) {
                Ok(pending) if !pending.is_empty() => {
                    if let Some(q) = task_queue_ref.get() {
                        q.enqueue(Task::ThumbnailPrefetch { photo_ids: pending });
                    }
                }
                _ => {}
            }

            let job = get_latest_scan_job(&conn, library_id).ok();
            let _ = app_handle.emit("scan-complete", serde_json::json!({
                "library_id": library_id,
                "result": scan_result,
                "job": job,
            }));
        }
        Err(e) if e == "Scan cancelled" => {
            let job = get_latest_scan_job(&conn, library_id).ok();
            let _ = app_handle.emit("scan-paused", serde_json::json!({
                "library_id": library_id,
                "job": job,
            }));
        }
        Err(e) => {
            eprintln!("ScanLibrary: scan failed: {}", e);
            let _ = app_handle.emit("scan-error", serde_json::json!({
                "library_id": library_id,
                "error": e,
            }));
        }
    }
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
    let conn = state.db.get().map_err(|e| format!("Failed to get DB connection: {}", e))?;
    orchestrator::relocate_folder(&conn, library_id, &old_prefix, &new_prefix)
}

#[tauri::command]
pub fn timeline_query(
    state: tauri::State<'_, AppState>,
    library_id: i64,
    cursor: Option<String>,
    limit: Option<i64>,
) -> Result<TimelinePage, String> {
    let conn = state.db.get().map_err(|e| format!("Failed to get DB connection: {}", e))?;
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
    let conn = state.db.get().map_err(|e| format!("Failed to get DB connection: {}", e))?;

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
    let conn = state.db.get().map_err(|e| format!("Failed to get DB connection: {}", e))?;
    orchestrator::relocate_file(&conn, photo_file_id, Path::new(&new_path))
}

#[tauri::command]
pub fn thumbnail_url(
    state: tauri::State<'_, AppState>,
    photo_id: i64,
    tier: i64,
) -> Result<String, String> {
    let conn = state.db.get().map_err(|e| format!("Failed to get DB connection: {}", e))?;
    let _ = dao::get_photo(&conn, photo_id).map_err(|e| format!("Photo not found: {}", e))?;
    Ok(format!("asset://thumb/{}/{}", photo_id, tier))
}

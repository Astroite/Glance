pub mod commands;
pub mod core;
pub mod error;

use commands::{AppState, SharedTaskQueue};
use core::db::{self, thumbs_dir};
use core::tasks::TaskQueue;
use core::thumbnail;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};
use tauri::Manager;

const RAW_FORMATS: &[&str] = &["arw", "cr2", "cr3", "nef", "orf", "rw2", "dng", "raf"];

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // Initialize app data directory
            db::init_app_data_dir().expect("Failed to initialize app data directory");

            // Run migrations once with a throwaway connection (migrations should not run per
            // pooled connection).
            let db_path = db::db_path();
            {
                let conn = db::create_connection(&db_path)
                    .expect("Failed to create database connection");
                db::run_migrations(&conn).expect("Failed to run database migrations");
            }

            // Build pool. Every new connection gets the same pragmas as `create_connection`.
            let manager = SqliteConnectionManager::file(&db_path).with_init(|c| {
                c.execute_batch("PRAGMA journal_mode=WAL;")?;
                c.execute_batch("PRAGMA foreign_keys=ON;")?;
                Ok(())
            });
            let pool = Pool::builder()
                .max_size(10)
                .build(manager)
                .expect("Failed to build SQLite connection pool");

            // Build the background TaskQueue. Worker threads spawn immediately,
            // so the handler must be Send + Sync + 'static — we capture pool
            // and thumbs_dir by move.
            let cpu_workers = std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(4);
            let scan_cancel_tokens = Arc::new(Mutex::new(HashMap::new()));
            let task_queue_ref: SharedTaskQueue = Arc::new(OnceLock::new());
            let handler = commands::make_task_handler(
                pool.clone(),
                thumbs_dir(),
                app.handle().clone(),
                scan_cancel_tokens.clone(),
                task_queue_ref.clone(),
            );
            let queue = Arc::new(TaskQueue::new(3, cpu_workers, handler));
            // Safe: workers only run after TaskQueue::new returns, and they
            // block on the condvar until a task is enqueued.
            let _ = task_queue_ref.set(queue.clone());

            app.manage(AppState {
                db: pool,
                scan_cancel_tokens,
                task_queue: queue,
            });

            Ok(())
        })
        .register_uri_scheme_protocol("asset", |app, request| {
            // Parse asset://thumb/{photoId}/{tier}
            let uri = request.uri();
            let path = uri.path();

            // path is "/thumb/{photoId}/{tier}" (the scheme is stripped)
            let segments: Vec<&str> = path.trim_start_matches('/').split('/').collect();

            if segments.len() < 3 || segments[0] != "thumb" {
                let mut response = tauri::http::Response::new(Vec::new());
                *response.status_mut() = tauri::http::StatusCode::BAD_REQUEST;
                return response;
            }

            let photo_id: i64 = match segments[1].parse() {
                Ok(v) => v,
                Err(_) => {
                    let mut response = tauri::http::Response::new(Vec::new());
                    *response.status_mut() = tauri::http::StatusCode::BAD_REQUEST;
                    return response;
                }
            };

            let tier: u32 = match segments[2].parse() {
                Ok(v) => v,
                Err(_) => {
                    let mut response = tauri::http::Response::new(Vec::new());
                    *response.status_mut() = tauri::http::StatusCode::BAD_REQUEST;
                    return response;
                }
            };

            // Look up the thumbnail in the DB via the shared pool
            let thumbs = thumbs_dir();
            let state = app.app_handle().state::<AppState>();

            let thumb_result = match state.db.get() {
                Ok(conn) => conn
                    .query_row(
                        "SELECT source_hash FROM thumbnails WHERE photo_id = ?1 AND tier = ?2",
                        rusqlite::params![photo_id, tier],
                        |row| row.get::<_, String>(0),
                    )
                    .ok(),
                Err(_) => {
                    let mut response = tauri::http::Response::new(Vec::new());
                    *response.status_mut() = tauri::http::StatusCode::INTERNAL_SERVER_ERROR;
                    return response;
                }
            };

            if let Some(hash) = thumb_result {
                let thumb_path = thumbnail::thumbnail_path(&thumbs, tier, &hash);
                if thumb_path.exists() {
                    match std::fs::read(&thumb_path) {
                        Ok(bytes) => {
                            let mut response = tauri::http::Response::new(bytes);
                            response.headers_mut().insert(
                                "content-type",
                                "image/webp".parse().unwrap(),
                            );
                            return response;
                        }
                        Err(_) => {}
                    }
                }
            }

            // If thumbnail not found in DB, try to find it by looking up the photo's display file hash
            let display_hash_result = match state.db.get() {
                Ok(conn) => conn
                    .query_row(
                        "SELECT pf.content_hash FROM photos p
                         JOIN photo_files pf ON p.display_file_id = pf.id
                         WHERE p.id = ?1",
                        [photo_id],
                        |row| row.get::<_, String>(0),
                    )
                    .ok(),
                Err(_) => None,
            };

            if let Some(hash) = display_hash_result {
                let thumb_path = thumbnail::thumbnail_path(&thumbs, tier, &hash);
                if thumb_path.exists() {
                    match std::fs::read(&thumb_path) {
                        Ok(bytes) => {
                            let mut response = tauri::http::Response::new(bytes);
                            response.headers_mut().insert(
                                "content-type",
                                "image/webp".parse().unwrap(),
                            );
                            return response;
                        }
                        Err(_) => {}
                    }
                }

                // Try to generate on the fly
                if let Ok(conn) = state.db.get() {
                    if let Ok(Some((display_path, display_format))) = conn.query_row(
                        "SELECT pf.path, pf.format FROM photos p
                         JOIN photo_files pf ON p.display_file_id = pf.id
                         WHERE p.id = ?1",
                        [photo_id],
                        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                    ).map(Some) {
                        let img_path = Path::new(&display_path);
                        if img_path.exists() {
                            let orientation: Option<i64> = conn.query_row(
                                "SELECT orientation FROM photos WHERE id = ?1",
                                [photo_id],
                                |row| row.get(0),
                            ).ok();

                            let orient = orientation.map(|o| o as u32).unwrap_or(1);

                            let gen_result = if RAW_FORMATS.contains(&display_format.as_str()) {
                                // RAW file: extract embedded JPEG first
                                match crate::core::raw::extract_embedded_jpeg(img_path) {
                                    Ok(jpeg_bytes) => thumbnail::generate_thumbnails_from_bytes(&jpeg_bytes, &thumbs, &hash, Some(orient)),
                                    Err(_) => thumbnail::generate_thumbnails(img_path, &thumbs, &hash, Some(orient)),
                                }
                            } else {
                                thumbnail::generate_thumbnails(img_path, &thumbs, &hash, Some(orient))
                            };

                            if gen_result.is_ok() {
                                let thumb_path = thumbnail::thumbnail_path(&thumbs, tier, &hash);
                                if let Ok(bytes) = std::fs::read(&thumb_path) {
                                    let mut response = tauri::http::Response::new(bytes);
                                    response.headers_mut().insert(
                                        "content-type",
                                        "image/webp".parse().unwrap(),
                                    );
                                    return response;
                                }
                            }
                        }
                    }
                }
            }

            // Return placeholder
            let placeholder = thumbnail::generate_placeholder(tier);
            let mut response = tauri::http::Response::new(placeholder);
            response.headers_mut().insert(
                "content-type",
                "image/webp".parse().unwrap(),
            );
            response
        })
        .invoke_handler(tauri::generate_handler![
            commands::library_list,
            commands::library_add,
            commands::library_scan,
            commands::library_scan_pause,
            commands::library_scan_resume,
            commands::library_relocate_folder,
            commands::timeline_query,
            commands::photo_detail,
            commands::photo_relocate_file,
            commands::thumbnail_url,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    #[test]
    fn module_structure_compiles() {
        let _ = std::any::type_name::<crate::error::Error>();
    }
}

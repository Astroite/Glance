-- Libraries (MVP single library, library_id always present)
CREATE TABLE IF NOT EXISTS libraries (
    id          INTEGER PRIMARY KEY,
    name        TEXT NOT NULL,
    root_path   TEXT NOT NULL,
    created_at  INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
);

-- Photos (logical entity, deduplicated by identity)
CREATE TABLE IF NOT EXISTS photos (
    id              INTEGER PRIMARY KEY,
    library_id      INTEGER NOT NULL REFERENCES libraries(id),
    taken_at        INTEGER,
    taken_at_src    TEXT,           -- 'exif' | 'mtime'
    camera_make     TEXT,
    camera_model    TEXT,
    lens            TEXT,
    focal_len       REAL,
    aperture        REAL,
    shutter         REAL,           -- seconds
    iso             INTEGER,
    width           INTEGER,
    height          INTEGER,
    orientation     INTEGER,        -- 1..8 EXIF value
    gps_lat         REAL,
    gps_lon         REAL,
    rating          INTEGER,        -- from XMP, 0..5
    label           TEXT,           -- from XMP, color label
    format          TEXT NOT NULL,   -- 'jpeg' | 'heic' | 'arw' etc
    display_file_id INTEGER REFERENCES photo_files(id),
    indexed_at      INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
);

-- Photo files (physical instances, one-to-many with photos)
CREATE TABLE IF NOT EXISTS photo_files (
    id              INTEGER PRIMARY KEY,
    photo_id        INTEGER NOT NULL REFERENCES photos(id) ON DELETE CASCADE,
    path            TEXT NOT NULL UNIQUE,
    content_hash    TEXT NOT NULL,    -- xxh3(head 64KB + tail 64KB + size)
    file_size       INTEGER NOT NULL,
    mtime           INTEGER NOT NULL, -- for change detection only
    role            TEXT NOT NULL,    -- 'display' | 'raw' | 'sidecar' | 'duplicate'
    status          TEXT NOT NULL DEFAULT 'available', -- 'available' | 'missing'
    last_seen_at    INTEGER NOT NULL,
    last_scan_id    INTEGER
);

-- Thumbnail cache metadata (actual path derived from hash)
CREATE TABLE IF NOT EXISTS thumbnails (
    photo_id        INTEGER NOT NULL REFERENCES photos(id) ON DELETE CASCADE,
    source_file_id  INTEGER NOT NULL REFERENCES photo_files(id),
    source_hash     TEXT NOT NULL,
    tier            INTEGER NOT NULL, -- 240 | 480 | 1080
    width           INTEGER NOT NULL,
    height          INTEGER NOT NULL,
    generated_at    INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    PRIMARY KEY (photo_id, tier)
);

-- Scan jobs (supports resumable scans)
CREATE TABLE IF NOT EXISTS scan_jobs (
    id          INTEGER PRIMARY KEY,
    library_id  INTEGER NOT NULL REFERENCES libraries(id),
    status      TEXT NOT NULL,         -- 'running' | 'paused' | 'done' | 'failed'
    cursor      TEXT,                  -- current scan position (relative path)
    started_at  INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    finished_at INTEGER,
    added       INTEGER DEFAULT 0,
    updated     INTEGER DEFAULT 0,
    missing     INTEGER DEFAULT 0
);

-- Indexes
CREATE INDEX IF NOT EXISTS idx_photos_timeline ON photos(library_id, taken_at DESC);
CREATE INDEX IF NOT EXISTS idx_photo_files_photo ON photo_files(photo_id);
CREATE INDEX IF NOT EXISTS idx_photo_files_path ON photo_files(path);
CREATE INDEX IF NOT EXISTS idx_photo_files_identity ON photo_files(content_hash, file_size);
CREATE INDEX IF NOT EXISTS idx_photo_files_status ON photo_files(status);

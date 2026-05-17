-- Libraries
CREATE TABLE IF NOT EXISTS libraries (
    id          INTEGER PRIMARY KEY,
    name        TEXT NOT NULL,
    root_path   TEXT NOT NULL,
    created_at  INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
);

-- Photos (logical timeline entity)
CREATE TABLE IF NOT EXISTS photos (
    id              INTEGER PRIMARY KEY,
    library_id      INTEGER NOT NULL REFERENCES libraries(id),
    display_file_id INTEGER REFERENCES photo_files(id),
    taken_at        INTEGER,
    taken_at_src    TEXT,
    camera_make     TEXT,
    camera_model    TEXT,
    lens            TEXT,
    focal_len       REAL,
    aperture        REAL,
    shutter         REAL,
    iso             INTEGER,
    width           INTEGER,
    height          INTEGER,
    orientation     INTEGER,
    gps_lat         REAL,
    gps_lon         REAL,
    rating          INTEGER,
    label           TEXT,
    indexed_at      INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    updated_at      INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
);

-- Photo files (physical instances)
CREATE TABLE IF NOT EXISTS photo_files (
    id              INTEGER PRIMARY KEY,
    library_id      INTEGER NOT NULL REFERENCES libraries(id),
    photo_id        INTEGER NOT NULL REFERENCES photos(id) ON DELETE CASCADE,
    path            TEXT NOT NULL,
    role            TEXT NOT NULL,
    format          TEXT NOT NULL,
    content_hash    TEXT NOT NULL,
    file_size       INTEGER NOT NULL,
    mtime           INTEGER NOT NULL,
    status          TEXT NOT NULL DEFAULT 'available',
    missing_since   INTEGER,
    last_seen_at    INTEGER NOT NULL,
    last_scan_id    INTEGER,
    UNIQUE(library_id, path)
);

-- Thumbnail cache metadata
CREATE TABLE IF NOT EXISTS thumbnails (
    photo_id        INTEGER NOT NULL REFERENCES photos(id) ON DELETE CASCADE,
    tier            INTEGER NOT NULL,
    source_file_id  INTEGER NOT NULL REFERENCES photo_files(id),
    source_hash     TEXT NOT NULL,
    width           INTEGER NOT NULL,
    height          INTEGER NOT NULL,
    generated_at    INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    PRIMARY KEY (photo_id, tier)
);

-- Scan jobs (supports resumable scans)
CREATE TABLE IF NOT EXISTS scan_jobs (
    id          INTEGER PRIMARY KEY,
    library_id  INTEGER NOT NULL REFERENCES libraries(id),
    status      TEXT NOT NULL,
    cursor      TEXT,
    started_at  INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    finished_at INTEGER,
    added       INTEGER DEFAULT 0,
    updated     INTEGER DEFAULT 0,
    missing     INTEGER DEFAULT 0
);

-- Indexes
CREATE INDEX IF NOT EXISTS idx_photos_timeline ON photos(library_id, taken_at DESC);
CREATE INDEX IF NOT EXISTS idx_photo_files_photo ON photo_files(photo_id);
CREATE INDEX IF NOT EXISTS idx_photo_files_path_prefix ON photo_files(path);
CREATE INDEX IF NOT EXISTS idx_photo_files_identity ON photo_files(library_id, content_hash, file_size);
CREATE INDEX IF NOT EXISTS idx_photo_files_status ON photo_files(library_id, status);

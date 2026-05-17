pub mod orchestrator;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Supported media file extensions (lowercase)
const DISPLAY_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "heic", "heif"];
const RAW_EXTENSIONS: &[&str] = &["arw", "cr2", "cr3", "nef", "orf", "rw2", "dng", "raf"];
const SIDECAR_EXTENSIONS: &[&str] = &["xmp"];

/// Media file type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MediaType {
    Display,  // JPEG, PNG, HEIC - can be shown directly
    Raw,      // RAW files - needs embedded JPEG extraction
    Sidecar,  // XMP sidecar files
}

/// Role of a file in a photo group
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileRole {
    Display,    // Primary display file (JPEG preferred)
    Raw,        // RAW original
    Sidecar,    // XMP metadata sidecar
    Duplicate,  // Duplicate of another file
}

impl FileRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            FileRole::Display => "display",
            FileRole::Raw => "raw",
            FileRole::Sidecar => "sidecar",
            FileRole::Duplicate => "duplicate",
        }
    }
}

/// A discovered media file
#[derive(Debug, Clone)]
pub struct DiscoveredFile {
    pub path: PathBuf,
    pub media_type: MediaType,
    pub stem: String,          // filename without extension
    pub extension: String,     // lowercase extension
}

/// A logical photo candidate (grouped by stem in same directory)
#[derive(Debug, Clone)]
pub struct PhotoCandidate {
    pub directory: PathBuf,
    pub stem: String,
    pub display_file: Option<DiscoveredFile>,
    pub raw_files: Vec<DiscoveredFile>,
    pub sidecar_files: Vec<DiscoveredFile>,
}

/// Scan a directory for media files and group them into photo candidates.
pub fn discover_media_files(root: &Path) -> Vec<DiscoveredFile> {
    let mut files = Vec::new();

    for entry in WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_file() {
            continue;
        }

        let path = entry.path();
        if let Some(discovered) = classify_file(path) {
            files.push(discovered);
        }
    }

    files
}

/// Classify a file by its extension
pub fn classify_file(path: &Path) -> Option<DiscoveredFile> {
    let extension = path.extension()?.to_str()?.to_lowercase();
    let stem = path.file_stem()?.to_str()?.to_string();

    let media_type = if DISPLAY_EXTENSIONS.contains(&extension.as_str()) {
        MediaType::Display
    } else if RAW_EXTENSIONS.contains(&extension.as_str()) {
        MediaType::Raw
    } else if SIDECAR_EXTENSIONS.contains(&extension.as_str()) {
        MediaType::Sidecar
    } else {
        return None;
    };

    Some(DiscoveredFile {
        path: path.to_path_buf(),
        media_type,
        stem,
        extension,
    })
}

/// Group discovered files into photo candidates by directory + stem.
/// Same directory, same stem = one logical photo.
pub fn group_into_candidates(files: Vec<DiscoveredFile>) -> Vec<PhotoCandidate> {
    let mut groups: HashMap<(PathBuf, String), PhotoCandidate> = HashMap::new();

    for file in files {
        let dir = file.path.parent().map(|p| p.to_path_buf()).unwrap_or_default();
        let key = (dir.clone(), file.stem.clone());

        let candidate = groups.entry(key).or_insert_with(|| PhotoCandidate {
            directory: dir,
            stem: file.stem.clone(),
            display_file: None,
            raw_files: Vec::new(),
            sidecar_files: Vec::new(),
        });

        match file.media_type {
            MediaType::Display => {
                // B6 fix: prefer JPEG as display, but never push another display into raw_files.
                // Extra display files become duplicates (stored as pending_displays for later).
                if candidate.display_file.is_none() {
                    candidate.display_file = Some(file);
                } else if file.extension == "jpg" || file.extension == "jpeg" {
                    // JPEG takes priority as display
                    let prev = candidate.display_file.take().unwrap();
                    // Previous display becomes a duplicate sidecar (not raw)
                    candidate.sidecar_files.push(prev);
                    candidate.display_file = Some(file);
                } else {
                    // Non-JPEG display when we already have a display — keep as sidecar reference
                    candidate.sidecar_files.push(file);
                }
            }
            MediaType::Raw => {
                candidate.raw_files.push(file);
            }
            MediaType::Sidecar => {
                candidate.sidecar_files.push(file);
            }
        }
    }

    groups.into_values().collect()
}

/// Assign roles to files within a photo candidate.
/// Returns (file, role) pairs.
pub fn assign_roles(candidate: &PhotoCandidate) -> Vec<(&DiscoveredFile, FileRole)> {
    let mut roles = Vec::new();

    // Display file gets Display role
    if let Some(ref display) = candidate.display_file {
        roles.push((display, FileRole::Display));
    }

    // RAW files get Raw role
    for raw in &candidate.raw_files {
        roles.push((raw, FileRole::Raw));
    }

    // Sidecar files get Sidecar role
    for sidecar in &candidate.sidecar_files {
        roles.push((sidecar, FileRole::Sidecar));
    }

    roles
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::TempDir;

    fn create_test_file(dir: &Path, name: &str) -> PathBuf {
        let path = dir.join(name);
        let mut file = File::create(&path).unwrap();
        file.write_all(b"test content").unwrap();
        path
    }

    #[test]
    fn test_classify_jpeg() {
        let tmp = TempDir::new().unwrap();
        let path = create_test_file(tmp.path(), "photo.jpg");
        let file = classify_file(&path).unwrap();
        assert_eq!(file.media_type, MediaType::Display);
        assert_eq!(file.stem, "photo");
        assert_eq!(file.extension, "jpg");
    }

    #[test]
    fn test_classify_raw() {
        let tmp = TempDir::new().unwrap();
        let path = create_test_file(tmp.path(), "photo.arw");
        let file = classify_file(&path).unwrap();
        assert_eq!(file.media_type, MediaType::Raw);
    }

    #[test]
    fn test_classify_xmp() {
        let tmp = TempDir::new().unwrap();
        let path = create_test_file(tmp.path(), "photo.xmp");
        let file = classify_file(&path).unwrap();
        assert_eq!(file.media_type, MediaType::Sidecar);
    }

    #[test]
    fn test_classify_unsupported() {
        let tmp = TempDir::new().unwrap();
        let path = create_test_file(tmp.path(), "readme.txt");
        assert!(classify_file(&path).is_none());
    }

    #[test]
    fn test_classify_case_insensitive() {
        let tmp = TempDir::new().unwrap();
        let path = create_test_file(tmp.path(), "photo.JPG");
        let file = classify_file(&path).unwrap();
        assert_eq!(file.media_type, MediaType::Display);
        assert_eq!(file.extension, "jpg");
    }

    #[test]
    fn test_group_same_stem() {
        let tmp = TempDir::new().unwrap();
        create_test_file(tmp.path(), "photo.jpg");
        create_test_file(tmp.path(), "photo.arw");
        create_test_file(tmp.path(), "photo.xmp");

        let files = discover_media_files(tmp.path());
        let candidates = group_into_candidates(files);

        assert_eq!(candidates.len(), 1);
        let candidate = &candidates[0];
        assert!(candidate.display_file.is_some());
        assert_eq!(candidate.raw_files.len(), 1);
        assert_eq!(candidate.sidecar_files.len(), 1);
    }

    #[test]
    fn test_group_different_stem() {
        let tmp = TempDir::new().unwrap();
        create_test_file(tmp.path(), "photo1.jpg");
        create_test_file(tmp.path(), "photo2.jpg");

        let files = discover_media_files(tmp.path());
        let candidates = group_into_candidates(files);

        assert_eq!(candidates.len(), 2);
    }

    #[test]
    fn test_group_different_directory() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir(tmp.path().join("a")).unwrap();
        std::fs::create_dir(tmp.path().join("b")).unwrap();
        create_test_file(&tmp.path().join("a"), "photo.jpg");
        create_test_file(&tmp.path().join("b"), "photo.jpg");

        let files = discover_media_files(tmp.path());
        let candidates = group_into_candidates(files);

        assert_eq!(candidates.len(), 2);
    }

    #[test]
    fn test_jpeg_preferred_as_display() {
        let tmp = TempDir::new().unwrap();
        create_test_file(tmp.path(), "photo.heic");
        create_test_file(tmp.path(), "photo.jpg");

        let files = discover_media_files(tmp.path());
        let candidates = group_into_candidates(files);

        assert_eq!(candidates.len(), 1);
        let candidate = &candidates[0];
        // JPEG should be preferred as display
        assert!(candidate.display_file.is_some());
        assert_eq!(candidate.display_file.as_ref().unwrap().extension, "jpg");
    }

    #[test]
    fn test_assign_roles() {
        let tmp = TempDir::new().unwrap();
        create_test_file(tmp.path(), "photo.jpg");
        create_test_file(tmp.path(), "photo.arw");
        create_test_file(tmp.path(), "photo.xmp");

        let files = discover_media_files(tmp.path());
        let candidates = group_into_candidates(files);
        let roles = assign_roles(&candidates[0]);

        assert_eq!(roles.len(), 3);
        assert!(roles.iter().any(|(_, r)| *r == FileRole::Display));
        assert!(roles.iter().any(|(_, r)| *r == FileRole::Raw));
        assert!(roles.iter().any(|(_, r)| *r == FileRole::Sidecar));
    }

    #[test]
    fn test_raw_only_photo() {
        let tmp = TempDir::new().unwrap();
        create_test_file(tmp.path(), "photo.arw");

        let files = discover_media_files(tmp.path());
        let candidates = group_into_candidates(files);

        assert_eq!(candidates.len(), 1);
        let candidate = &candidates[0];
        assert!(candidate.display_file.is_none());
        assert_eq!(candidate.raw_files.len(), 1);
    }
}

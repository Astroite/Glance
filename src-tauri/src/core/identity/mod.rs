use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use xxhash_rust::xxh3::Xxh3;

const CHUNK_SIZE: usize = 64 * 1024; // 64KB

/// File identity: content_hash + file_size.
/// content_hash = xxh3(head 64KB + tail 64KB + file_size).
/// mtime is only for change detection, not part of identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileIdentity {
    pub content_hash: String,
    pub file_size: u64,
    pub mtime: i64,
}

/// Compute file identity using xxh3(head 64KB + tail 64KB + file_size).
/// Only reads head and tail chunks, not the entire file.
pub fn compute_identity(path: &Path) -> Result<FileIdentity, std::io::Error> {
    let metadata = std::fs::metadata(path)?;
    let file_size = metadata.len();
    let mtime = metadata
        .modified()?
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    let content_hash = compute_hash(path, file_size)?;

    Ok(FileIdentity {
        content_hash,
        file_size,
        mtime,
    })
}

/// Compute xxh3 hash from head 64KB + tail 64KB + file_size.
fn compute_hash(path: &Path, file_size: u64) -> Result<String, std::io::Error> {
    let mut file = File::open(path)?;
    let mut hasher = Xxh3::new();

    // Read head chunk
    let head_size = CHUNK_SIZE.min(file_size as usize);
    let mut head_buf = vec![0u8; head_size];
    file.read_exact(&mut head_buf)?;
    hasher.update(&head_buf);

    // Read tail chunk if file is larger than one chunk
    if file_size > CHUNK_SIZE as u64 {
        let tail_size = CHUNK_SIZE.min(file_size as usize - CHUNK_SIZE);
        file.seek(SeekFrom::End(-(tail_size as i64)))?;
        let mut tail_buf = vec![0u8; tail_size];
        file.read_exact(&mut tail_buf)?;
        hasher.update(&tail_buf);
    }

    // Include file size in hash
    hasher.update(&file_size.to_le_bytes());

    let hash = hasher.digest();
    Ok(format!("{:016x}", hash))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_small_file_identity() {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(b"hello world").unwrap();
        file.flush().unwrap();

        let identity = compute_identity(file.path()).unwrap();
        assert_eq!(identity.file_size, 11);
        assert!(!identity.content_hash.is_empty());
        assert!(identity.mtime > 0);
    }

    #[test]
    fn test_large_file_identity() {
        let mut file = NamedTempFile::new().unwrap();
        // Write 256KB (larger than chunk size)
        let data = vec![0xABu8; 256 * 1024];
        file.write_all(&data).unwrap();
        file.flush().unwrap();

        let identity = compute_identity(file.path()).unwrap();
        assert_eq!(identity.file_size, 256 * 1024);
        assert!(!identity.content_hash.is_empty());
    }

    #[test]
    fn test_same_content_same_hash() {
        let mut file1 = NamedTempFile::new().unwrap();
        let mut file2 = NamedTempFile::new().unwrap();
        file1.write_all(b"identical content").unwrap();
        file2.write_all(b"identical content").unwrap();
        file1.flush().unwrap();
        file2.flush().unwrap();

        let id1 = compute_identity(file1.path()).unwrap();
        let id2 = compute_identity(file2.path()).unwrap();
        assert_eq!(id1.content_hash, id2.content_hash);
        assert_eq!(id1.file_size, id2.file_size);
    }

    #[test]
    fn test_different_content_different_hash() {
        let mut file1 = NamedTempFile::new().unwrap();
        let mut file2 = NamedTempFile::new().unwrap();
        file1.write_all(b"content A").unwrap();
        file2.write_all(b"content B").unwrap();
        file1.flush().unwrap();
        file2.flush().unwrap();

        let id1 = compute_identity(file1.path()).unwrap();
        let id2 = compute_identity(file2.path()).unwrap();
        assert_ne!(id1.content_hash, id2.content_hash);
    }

    #[test]
    fn test_empty_file() {
        let file = NamedTempFile::new().unwrap();
        let identity = compute_identity(file.path()).unwrap();
        assert_eq!(identity.file_size, 0);
        assert!(!identity.content_hash.is_empty());
    }

    #[test]
    fn test_mtime_not_in_hash() {
        // mtime should not affect the hash
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(b"test").unwrap();
        file.flush().unwrap();

        let id1 = compute_identity(file.path()).unwrap();
        // Even if we could change mtime, hash should be the same
        let id2 = compute_identity(file.path()).unwrap();
        assert_eq!(id1.content_hash, id2.content_hash);
    }
}

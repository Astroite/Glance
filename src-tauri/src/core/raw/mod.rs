use std::io::Cursor;
use std::path::Path;

use rawler::analyze::extract_preview_pixels;
use rawler::decoders::RawDecodeParams;

/// Extract the largest embedded JPEG preview from a RAW file.
/// Returns ready-to-decode JPEG bytes (consumable by `image::load_from_memory`).
/// Does NOT perform demosaic — only retrieves the camera's embedded preview / full-size
/// JPEG via rawler. Falls back to a raw byte scan if rawler cannot decode the file
/// (unsupported format, corrupt header, etc.).
pub fn extract_embedded_jpeg(path: &Path) -> Result<Vec<u8>, String> {
    if !path.exists() {
        return Err(format!("RAW file does not exist: {}", path.display()));
    }

    let params = RawDecodeParams::default();
    match extract_preview_pixels(path, &params) {
        Ok(img) => {
            let mut buf: Vec<u8> = Vec::new();
            img.write_to(&mut Cursor::new(&mut buf), image::ImageFormat::Jpeg)
                .map_err(|e| format!("Failed to encode RAW preview as JPEG: {}", e))?;
            Ok(buf)
        }
        Err(rawler_err) => fallback_byte_scan(path).map_err(|fb_err| {
            format!(
                "rawler preview extraction failed ({}); fallback byte-scan also failed ({})",
                rawler_err, fb_err
            )
        }),
    }
}

/// Fallback: scan raw bytes for JPEG SOI/EOI markers and return the largest
/// well-formed segment. Used when rawler cannot decode the file at all.
/// Less reliable than rawler (can be fooled by nested EXIF thumbnails) but
/// better than nothing for exotic formats rawler does not support.
fn fallback_byte_scan(path: &Path) -> Result<Vec<u8>, String> {
    let data = std::fs::read(path).map_err(|e| format!("Failed to read RAW file: {}", e))?;

    let jpeg_soi: &[u8] = &[0xFF, 0xD8];
    let jpeg_eoi: &[u8] = &[0xFF, 0xD9];

    let mut best_jpeg: Option<Vec<u8>> = None;
    let mut best_size = 0usize;
    let mut pos = 0;

    while pos < data.len() {
        if let Some(soi_offset) = find_pattern(&data[pos..], jpeg_soi) {
            let abs_start = pos + soi_offset;
            if let Some(eoi_offset) = find_pattern(&data[abs_start + 2..], jpeg_eoi) {
                let abs_end = abs_start + 2 + eoi_offset + 2;
                let jpeg_data = &data[abs_start..abs_end];

                if jpeg_data.len() > 4 && jpeg_data[2] == 0xFF {
                    let jpeg_len = jpeg_data.len();
                    if jpeg_len > best_size {
                        best_size = jpeg_len;
                        best_jpeg = Some(jpeg_data.to_vec());
                    }
                }
                pos = abs_end;
            } else {
                break;
            }
        } else {
            break;
        }
    }

    best_jpeg.ok_or_else(|| "No embedded JPEG found in RAW file".to_string())
}

fn find_pattern(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack.windows(needle.len()).position(|w| w == needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_pattern() {
        let data = b"\x00\x00\xFF\xD8\x00\x00\xFF\xD9";
        let pos = find_pattern(data, &[0xFF, 0xD8]);
        assert_eq!(pos, Some(2));

        let pos = find_pattern(data, &[0xFF, 0xD9]);
        assert_eq!(pos, Some(6));
    }

    #[test]
    fn test_find_pattern_not_found() {
        let data = b"\x00\x00\x00";
        let pos = find_pattern(data, &[0xFF, 0xD8]);
        assert_eq!(pos, None);
    }

    #[test]
    fn test_extract_embedded_jpeg_no_file() {
        let result = extract_embedded_jpeg(Path::new("/nonexistent/file.arw"));
        assert!(result.is_err());
    }

    // The byte-scan fallback should still find a synthetic JPEG block when rawler
    // cannot parse the container. We test the fallback directly here; a full
    // end-to-end test against real CR2/ARW/NEF fixtures is left to a follow-up PR
    // since those fixtures are not yet checked into the repo.
    #[test]
    fn test_fallback_byte_scan_synthetic() {
        let mut data = Vec::new();
        data.extend_from_slice(b"\x00\x01\x02\x03\x04\x05");
        data.extend_from_slice(&[0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x01, 0x02, 0x03, 0x04]);
        data.extend_from_slice(&[0xFF, 0xD9]);
        data.extend_from_slice(b"\x00\x00\x00");

        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), &data).unwrap();

        let jpeg = fallback_byte_scan(tmp.path()).unwrap();
        assert!(!jpeg.is_empty());
        assert_eq!(&jpeg[0..2], &[0xFF, 0xD8]);
        assert_eq!(&jpeg[jpeg.len() - 2..], &[0xFF, 0xD9]);
    }
}

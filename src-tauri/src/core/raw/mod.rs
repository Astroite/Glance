use std::path::Path;

/// Extract the largest embedded JPEG preview from a RAW file.
/// Returns the raw JPEG bytes (ready to be decoded by image::load_from_memory).
/// Does NOT perform demosaic — only scans for embedded JPEG markers.
pub fn extract_embedded_jpeg(path: &Path) -> Result<Vec<u8>, String> {
    let data = std::fs::read(path)
        .map_err(|e| format!("Failed to read RAW file: {}", e))?;

    // Scan for JPEG SOI (0xFFD8) and EOI (0xFFD9) markers
    let jpeg_soi: &[u8] = &[0xFF, 0xD8];
    let jpeg_eoi: &[u8] = &[0xFF, 0xD9];

    let mut best_jpeg: Option<Vec<u8>> = None;
    let mut best_size = 0usize;
    let mut pos = 0;

    while pos < data.len() {
        if let Some(soi_offset) = find_pattern(&data[pos..], jpeg_soi) {
            let abs_start = pos + soi_offset;
            // Look for EOI after this SOI
            if let Some(eoi_offset) = find_pattern(&data[abs_start + 2..], jpeg_eoi) {
                let abs_end = abs_start + 2 + eoi_offset + 2; // +2 for EOI marker itself
                let jpeg_data = &data[abs_start..abs_end];

                // Validate it's a real JPEG (check for APP0/APP1 marker after SOI)
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

/// Find a byte pattern in a slice
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

    #[test]
    fn test_extract_from_synthetic_raw() {
        // Create a synthetic file with an embedded JPEG
        let mut data = Vec::new();
        // Some header bytes
        data.extend_from_slice(b"\x00\x01\x02\x03\x04\x05");
        // Embedded JPEG: SOI + APP0 marker + some data + EOI
        data.extend_from_slice(&[0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x01, 0x02, 0x03, 0x04]);
        data.extend_from_slice(&[0xFF, 0xD9]);
        // Some trailing bytes
        data.extend_from_slice(b"\x00\x00\x00");

        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), &data).unwrap();

        let jpeg = extract_embedded_jpeg(tmp.path()).unwrap();
        assert!(!jpeg.is_empty());
        assert_eq!(&jpeg[0..2], &[0xFF, 0xD8]);
        assert_eq!(&jpeg[jpeg.len() - 2..], &[0xFF, 0xD9]);
    }
}

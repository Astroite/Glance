use chrono::NaiveDateTime;
use exif::{In, Reader, Tag, Value};
use quick_xml::events::Event;
use quick_xml::Reader as XmlReader;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

/// Extracted photo metadata from EXIF
#[derive(Debug, Clone, Default)]
pub struct PhotoMetadata {
    pub taken_at: Option<i64>,
    pub taken_at_src: Option<String>,
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
    pub format: Option<String>,
    pub rating: Option<i64>,
    pub label: Option<String>,
}

/// XMP sidecar metadata
#[derive(Debug, Clone, Default)]
pub struct XmpMetadata {
    pub rating: Option<i64>,
    pub label: Option<String>,
}

/// Extract EXIF metadata from an image file
pub fn extract_exif(path: &Path, mtime: i64) -> PhotoMetadata {
    let mut metadata = PhotoMetadata {
        taken_at_src: Some("mtime".to_string()),
        ..Default::default()
    };

    // Determine format from extension
    metadata.format = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .map(|e| match e.as_str() {
            "jpg" | "jpeg" => "jpeg".to_string(),
            "png" => "png".to_string(),
            "heic" | "heif" => "heic".to_string(),
            "arw" => "arw".to_string(),
            "cr2" => "cr2".to_string(),
            "cr3" => "cr3".to_string(),
            "nef" => "nef".to_string(),
            "dng" => "dng".to_string(),
            other => other.to_string(),
        });

    let file = match File::open(path) {
        Ok(f) => f,
        Err(_) => {
            metadata.taken_at = Some(mtime);
            return metadata;
        }
    };

    let mut buf_reader = BufReader::new(file);
    let reader = Reader::new();
    let exif = match reader.read_from_container(&mut buf_reader) {
        Ok(exif) => exif,
        Err(_) => {
            metadata.taken_at = Some(mtime);
            return metadata;
        }
    };

    // Extract taken_at from DateTimeOriginal
    if let Some(dt) = get_exif_datetime(&exif, Tag::DateTimeOriginal) {
        metadata.taken_at = Some(dt);
        metadata.taken_at_src = Some("exif".to_string());
    } else if let Some(dt) = get_exif_datetime(&exif, Tag::DateTime) {
        metadata.taken_at = Some(dt);
        metadata.taken_at_src = Some("exif".to_string());
    } else {
        metadata.taken_at = Some(mtime);
    }

    // Camera make/model
    metadata.camera_make = get_exif_string(&exif, Tag::Make);
    metadata.camera_model = get_exif_string(&exif, Tag::Model);

    // Lens
    metadata.lens = get_exif_string(&exif, Tag::LensModel)
        .or_else(|| get_exif_string(&exif, Tag::LensMake));

    // Focal length
    metadata.focal_len = get_exif_rational(&exif, Tag::FocalLength);

    // Aperture (FNumber)
    metadata.aperture = get_exif_rational(&exif, Tag::FNumber)
        .or_else(|| get_exif_rational(&exif, Tag::ApertureValue));

    // Shutter speed (ExposureTime in seconds)
    metadata.shutter = get_exif_rational(&exif, Tag::ExposureTime);

    // ISO
    metadata.iso = get_exif_u32(&exif, Tag::PhotographicSensitivity)
        .or_else(|| get_exif_u32(&exif, Tag::ISOSpeed))
        .map(|v| v as i64);

    // Dimensions
    metadata.width = get_exif_u32(&exif, Tag::PixelXDimension).map(|v| v as i64)
        .or_else(|| get_exif_u32(&exif, Tag::ImageWidth).map(|v| v as i64));
    metadata.height = get_exif_u32(&exif, Tag::PixelYDimension).map(|v| v as i64)
        .or_else(|| get_exif_u32(&exif, Tag::ImageLength).map(|v| v as i64));

    // Orientation
    metadata.orientation = get_exif_u16(&exif, Tag::Orientation).map(|v| v as i64);

    // GPS
    if let (Some(lat), Some(lon)) = (
        get_gps_coord(&exif, Tag::GPSLatitude, Tag::GPSLatitudeRef),
        get_gps_coord(&exif, Tag::GPSLongitude, Tag::GPSLongitudeRef),
    ) {
        metadata.gps_lat = Some(lat);
        metadata.gps_lon = Some(lon);
    }

    metadata
}

/// Extract XMP metadata from a sidecar file
pub fn extract_xmp(path: &Path) -> XmpMetadata {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return XmpMetadata::default(),
    };

    parse_xmp(&content)
}

/// Parse XMP XML content for Rating and Label
fn parse_xmp(content: &str) -> XmpMetadata {
    let mut metadata = XmpMetadata::default();
    let mut reader = XmlReader::from_str(content);

    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                // Check attributes for Rating and Label
                for attr in e.attributes().flatten() {
                    let key = String::from_utf8_lossy(attr.key.as_ref());
                    let value = String::from_utf8_lossy(&attr.value);

                    if key.contains("Rating") {
                        if let Ok(rating) = value.parse::<i64>() {
                            if (0..=5).contains(&rating) {
                                metadata.rating = Some(rating);
                            }
                        }
                    } else if key.contains("Label") {
                        metadata.label = Some(value.to_string());
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }

    metadata
}

/// Merge metadata from RAW and JPEG, with JPEG taking precedence for display fields
pub fn merge_metadata(display: PhotoMetadata, raw: Option<PhotoMetadata>) -> PhotoMetadata {
    let mut result = display;

    if let Some(raw_meta) = raw {
        // Fill in missing fields from RAW
        if result.taken_at_src.as_deref() == Some("mtime") && raw_meta.taken_at_src.as_deref() == Some("exif") {
            result.taken_at = raw_meta.taken_at;
            result.taken_at_src = raw_meta.taken_at_src;
        }
        if result.camera_make.is_none() {
            result.camera_make = raw_meta.camera_make;
        }
        if result.camera_model.is_none() {
            result.camera_model = raw_meta.camera_model;
        }
        if result.lens.is_none() {
            result.lens = raw_meta.lens;
        }
        if result.focal_len.is_none() {
            result.focal_len = raw_meta.focal_len;
        }
        if result.aperture.is_none() {
            result.aperture = raw_meta.aperture;
        }
        if result.shutter.is_none() {
            result.shutter = raw_meta.shutter;
        }
        if result.iso.is_none() {
            result.iso = raw_meta.iso;
        }
        if result.gps_lat.is_none() {
            result.gps_lat = raw_meta.gps_lat;
            result.gps_lon = raw_meta.gps_lon;
        }
    }

    result
}

// Helper functions for EXIF extraction

fn get_exif_string(exif: &exif::Exif, tag: Tag) -> Option<String> {
    let field = exif.get_field(tag, In::PRIMARY)?;
    match &field.value {
        Value::Ascii(vec) => {
            let s = String::from_utf8_lossy(&vec[0]);
            let trimmed = s.trim();
            if trimmed.is_empty() { None } else { Some(trimmed.to_string()) }
        }
        _ => None,
    }
}

fn get_exif_datetime(exif: &exif::Exif, tag: Tag) -> Option<i64> {
    let field = exif.get_field(tag, In::PRIMARY)?;
    match &field.value {
        Value::Ascii(vec) => {
            let s = String::from_utf8_lossy(&vec[0]);
            let trimmed = s.trim();
            // EXIF datetime format: "YYYY:MM:DD HH:MM:SS"
            let dt = NaiveDateTime::parse_from_str(trimmed, "%Y:%m:%d %H:%M:%S").ok()?;
            Some(dt.and_utc().timestamp())
        }
        _ => None,
    }
}

fn get_exif_rational(exif: &exif::Exif, tag: Tag) -> Option<f64> {
    let field = exif.get_field(tag, In::PRIMARY)?;
    match &field.value {
        Value::Rational(vec) => {
            let r = vec.first()?;
            if r.denom == 0 { None } else { Some(r.num as f64 / r.denom as f64) }
        }
        Value::SRational(vec) => {
            let r = vec.first()?;
            if r.denom == 0 { None } else { Some(r.num as f64 / r.denom as f64) }
        }
        _ => None,
    }
}

fn get_exif_u32(exif: &exif::Exif, tag: Tag) -> Option<u32> {
    let field = exif.get_field(tag, In::PRIMARY)?;
    match &field.value {
        Value::Long(vec) => vec.first().copied(),
        Value::Short(vec) => vec.first().map(|&v| v as u32),
        Value::SLong(vec) => vec.first().map(|&v| v as u32),
        Value::SShort(vec) => vec.first().map(|&v| v as u32),
        Value::Byte(vec) => vec.first().map(|&v| v as u32),
        _ => None,
    }
}

fn get_exif_u16(exif: &exif::Exif, tag: Tag) -> Option<u16> {
    let field = exif.get_field(tag, In::PRIMARY)?;
    match &field.value {
        Value::Short(vec) => vec.first().copied(),
        Value::Long(vec) => vec.first().map(|&v| v as u16),
        Value::SShort(vec) => vec.first().map(|&v| v as u16),
        Value::SLong(vec) => vec.first().map(|&v| v as u16),
        Value::Byte(vec) => vec.first().map(|&v| v as u16),
        _ => None,
    }
}

fn get_gps_coord(exif: &exif::Exif, coord_tag: Tag, ref_tag: Tag) -> Option<f64> {
    let field = exif.get_field(coord_tag, In::PRIMARY)?;
    let ref_field = exif.get_field(ref_tag, In::PRIMARY)?;

    let coords = match &field.value {
        Value::Rational(vec) => {
            if vec.len() < 3 { return None; }
            vec
        }
        _ => return None,
    };

    let ref_str = match &ref_field.value {
        Value::Ascii(vec) => String::from_utf8_lossy(&vec[0]).to_string(),
        _ => return None,
    };

    let degrees = coords[0].num as f64 / coords[0].denom as f64;
    let minutes = coords[1].num as f64 / coords[1].denom as f64;
    let seconds = coords[2].num as f64 / coords[2].denom as f64;

    let mut coord = degrees + minutes / 60.0 + seconds / 3600.0;
    if ref_str.trim() == "S" || ref_str.trim() == "W" {
        coord = -coord;
    }

    Some(coord)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_xmp_rating() {
        let xmp = r#"<?xml version="1.0" encoding="UTF-8"?>
<rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"
         xmlns:xmp="http://ns.adobe.com/xap/1.0/">
  <rdf:Description rdf:about=""
    xmp:Rating="4" />
</rdf:RDF>"#;

        let metadata = parse_xmp(xmp);
        assert_eq!(metadata.rating, Some(4));
    }

    #[test]
    fn test_parse_xmp_label() {
        let xmp = r#"<?xml version="1.0" encoding="UTF-8"?>
<rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"
         xmlns:xmp="http://ns.adobe.com/xap/1.0/">
  <rdf:Description rdf:about=""
    xmp:Label="Green" />
</rdf:RDF>"#;

        let metadata = parse_xmp(xmp);
        assert_eq!(metadata.label, Some("Green".to_string()));
    }

    #[test]
    fn test_parse_xmp_rating_and_label() {
        let xmp = r#"<?xml version="1.0" encoding="UTF-8"?>
<rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"
         xmlns:xmp="http://ns.adobe.com/xap/1.0/">
  <rdf:Description rdf:about=""
    xmp:Rating="5"
    xmp:Label="Red" />
</rdf:RDF>"#;

        let metadata = parse_xmp(xmp);
        assert_eq!(metadata.rating, Some(5));
        assert_eq!(metadata.label, Some("Red".to_string()));
    }

    #[test]
    fn test_parse_xmp_missing_fields() {
        let xmp = r#"<?xml version="1.0" encoding="UTF-8"?>
<rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
  <rdf:Description rdf:about="" />
</rdf:RDF>"#;

        let metadata = parse_xmp(xmp);
        assert_eq!(metadata.rating, None);
        assert_eq!(metadata.label, None);
    }

    #[test]
    fn test_parse_xmp_invalid_rating() {
        let xmp = r#"<?xml version="1.0" encoding="UTF-8"?>
<rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"
         xmlns:xmp="http://ns.adobe.com/xap/1.0/">
  <rdf:Description rdf:about=""
    xmp:Rating="invalid" />
</rdf:RDF>"#;

        let metadata = parse_xmp(xmp);
        assert_eq!(metadata.rating, None);
    }

    #[test]
    fn test_parse_xmp_out_of_range_rating() {
        let xmp = r#"<?xml version="1.0" encoding="UTF-8"?>
<rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"
         xmlns:xmp="http://ns.adobe.com/xap/1.0/">
  <rdf:Description rdf:about=""
    xmp:Rating="10" />
</rdf:RDF>"#;

        let metadata = parse_xmp(xmp);
        assert_eq!(metadata.rating, None);
    }

    #[test]
    fn test_merge_metadata_fill_from_raw() {
        let display = PhotoMetadata {
            taken_at: Some(1000),
            taken_at_src: Some("mtime".to_string()),
            camera_make: None,
            camera_model: None,
            ..Default::default()
        };

        let raw = PhotoMetadata {
            taken_at: Some(2000),
            taken_at_src: Some("exif".to_string()),
            camera_make: Some("Sony".to_string()),
            camera_model: Some("A7III".to_string()),
            ..Default::default()
        };

        let merged = merge_metadata(display, Some(raw));
        assert_eq!(merged.taken_at, Some(2000));
        assert_eq!(merged.taken_at_src, Some("exif".to_string()));
        assert_eq!(merged.camera_make, Some("Sony".to_string()));
    }

    #[test]
    fn test_merge_metadata_display_preferred() {
        let display = PhotoMetadata {
            taken_at: Some(1000),
            taken_at_src: Some("exif".to_string()),
            camera_make: Some("Canon".to_string()),
            width: Some(1920),
            height: Some(1080),
            ..Default::default()
        };

        let raw = PhotoMetadata {
            taken_at: Some(2000),
            taken_at_src: Some("exif".to_string()),
            camera_make: Some("Sony".to_string()),
            width: Some(6000),
            height: Some(4000),
            ..Default::default()
        };

        let merged = merge_metadata(display, Some(raw));
        // Display values should be preferred
        assert_eq!(merged.camera_make, Some("Canon".to_string()));
        assert_eq!(merged.width, Some(1920));
    }

    #[test]
    fn test_format_detection() {
        assert_eq!(
            detect_format(Path::new("photo.jpg")),
            Some("jpeg".to_string())
        );
        assert_eq!(
            detect_format(Path::new("photo.HEIC")),
            Some("heic".to_string())
        );
        assert_eq!(
            detect_format(Path::new("photo.ARW")),
            Some("arw".to_string())
        );
    }

    fn detect_format(path: &Path) -> Option<String> {
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .map(|e| match e.as_str() {
                "jpg" | "jpeg" => "jpeg".to_string(),
                "png" => "png".to_string(),
                "heic" | "heif" => "heic".to_string(),
                "arw" => "arw".to_string(),
                "cr2" => "cr2".to_string(),
                "cr3" => "cr3".to_string(),
                "nef" => "nef".to_string(),
                "dng" => "dng".to_string(),
                other => other.to_string(),
            })
    }
}

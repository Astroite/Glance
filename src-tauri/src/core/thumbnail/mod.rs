use fast_image_resize::{FilterType, PixelType, Resizer, ResizeOptions};
use fast_image_resize::images::Image as FirImage;
use image::{DynamicImage, GenericImageView, ImageBuffer, Rgb, RgbImage};
use std::path::{Path, PathBuf};

/// Thumbnail tier sizes (short edge in pixels)
pub const TIER_SMALL: u32 = 240;
pub const TIER_MEDIUM: u32 = 480;
pub const TIER_LARGE: u32 = 1080;

/// All thumbnail tiers
pub const TIERS: &[u32] = &[TIER_SMALL, TIER_MEDIUM, TIER_LARGE];

/// WebP quality for lossy encoding (0-100)
pub const WEBP_QUALITY: f32 = 85.0;

/// A generated thumbnail with its real dimensions
pub struct GeneratedThumbnail {
    pub tier: u32,
    pub path: PathBuf,
    pub width: u32,
    pub height: u32,
}

/// Get the thumbnail file path for a given hash and tier
/// Format: {thumbs_dir}/{tier}/{hash[0:2]}/{hash}.webp
pub fn thumbnail_path(thumbs_dir: &Path, tier: u32, hash: &str) -> PathBuf {
    let prefix = if hash.len() >= 2 { &hash[..2] } else { hash };
    thumbs_dir
        .join(tier.to_string())
        .join(prefix)
        .join(format!("{}.webp", hash))
}

/// Generate a placeholder thumbnail for missing files (4:3 ratio)
pub fn generate_placeholder(tier: u32) -> Vec<u8> {
    let width = tier;
    let height = tier * 3 / 4; // 4:3 ratio
    let mut img: RgbImage = ImageBuffer::new(width, height);

    // Fill with a light gray color
    for pixel in img.pixels_mut() {
        *pixel = Rgb([200, 200, 200]);
    }

    // Draw a simple "X" pattern
    let line_width = (width.min(height) / 20).max(2);
    let min_dim = width.min(height) as f64;
    for i in 0..width {
        for j in 0..height {
            let scale_x = i as f64 / width as f64;
            let scale_y = j as f64 / height as f64;
            if (scale_x - scale_y).abs() * min_dim < line_width as f64 {
                img.put_pixel(i, j, Rgb([150, 150, 150]));
            }
            if ((1.0 - scale_x) - scale_y).abs() * min_dim < line_width as f64 {
                img.put_pixel(i, j, Rgb([150, 150, 150]));
            }
        }
    }

    encode_webp_lossy(&DynamicImage::ImageRgb8(img))
}

/// Generate thumbnails for an image at all tiers
pub fn generate_thumbnails(
    image_path: &Path,
    thumbs_dir: &Path,
    hash: &str,
    orientation: Option<u32>,
) -> Result<Vec<GeneratedThumbnail>, String> {
    let img = image::open(image_path)
        .map_err(|e| format!("Failed to open image: {}", e))?;

    // Read ICC profile from source image for color conversion
    let icc_profile = read_icc_profile(image_path);

    // Convert to sRGB if needed
    let img = if let Some(icc) = icc_profile {
        convert_to_srgb(&img, &icc).unwrap_or_else(|| img.clone())
    } else {
        img.clone()
    };

    // Apply orientation
    let img = apply_orientation(img, orientation.unwrap_or(1));

    let mut results = Vec::new();

    for &tier in TIERS {
        let path = thumbnail_path(thumbs_dir, tier, hash);

        // Create parent directory
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create directory: {}", e))?;
        }

        // Resize image using fast_image_resize
        let resized = resize_to_tier_fast(&img, tier);
        let (width, height) = resized.dimensions();

        // Encode to lossy WebP
        let webp_data = encode_webp_lossy(&resized);

        // Write to file
        std::fs::write(&path, webp_data)
            .map_err(|e| format!("Failed to write thumbnail: {}", e))?;

        results.push(GeneratedThumbnail { tier, path, width, height });
    }

    Ok(results)
}

/// Generate thumbnails from raw JPEG bytes (e.g., embedded JPEG from RAW files).
/// The bytes should be a valid JPEG/PNG that `image::load_from_memory` can decode.
pub fn generate_thumbnails_from_bytes(
    image_bytes: &[u8],
    thumbs_dir: &Path,
    hash: &str,
    orientation: Option<u32>,
) -> Result<Vec<GeneratedThumbnail>, String> {
    let img = image::load_from_memory(image_bytes)
        .map_err(|e| format!("Failed to decode image from bytes: {}", e))?;

    let img = apply_orientation(img, orientation.unwrap_or(1));

    let mut results = Vec::new();

    for &tier in TIERS {
        let path = thumbnail_path(thumbs_dir, tier, hash);

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create directory: {}", e))?;
        }

        let resized = resize_to_tier_fast(&img, tier);
        let (width, height) = resized.dimensions();
        let webp_data = encode_webp_lossy(&resized);

        std::fs::write(&path, webp_data)
            .map_err(|e| format!("Failed to write thumbnail: {}", e))?;

        results.push(GeneratedThumbnail { tier, path, width, height });
    }

    Ok(results)
}

/// Resize an image to fit within a tier's short edge using fast_image_resize
fn resize_to_tier_fast(img: &DynamicImage, tier: u32) -> DynamicImage {
    let (width, height) = img.dimensions();
    let short_edge = width.min(height);

    if short_edge <= tier {
        return img.clone();
    }

    let scale = tier as f64 / short_edge as f64;
    let new_width = (width as f64 * scale).round() as u32;
    let new_height = (height as f64 * scale).round() as u32;

    // Convert to RGBA8 for fast_image_resize
    let rgba = img.to_rgba8();
    let src_image = FirImage::from_vec_u8(
        width,
        height,
        rgba.to_vec(),
        PixelType::U8x4,
    ).expect("Failed to create source image");

    let mut dst_image = FirImage::new(new_width, new_height, PixelType::U8x4);

    let mut resizer = Resizer::new();
    let resize_options = ResizeOptions::new()
        .resize_alg(fast_image_resize::ResizeAlg::Convolution(FilterType::Lanczos3));

    resizer.resize(&src_image, &mut dst_image, &resize_options)
        .expect("Failed to resize image");

    // Convert back to DynamicImage
    let raw = dst_image.buffer().to_vec();
    let img_buffer = ImageBuffer::<image::Rgba<u8>, _>::from_raw(new_width, new_height, raw)
        .expect("Failed to create image buffer");
    DynamicImage::ImageRgba8(img_buffer)
}

/// Encode an image to lossy WebP format
fn encode_webp_lossy(img: &DynamicImage) -> Vec<u8> {
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    let encoded = webp::Encoder::from_rgba(rgba.as_raw(), w, h).encode(WEBP_QUALITY);
    if !encoded.is_empty() {
        encoded.to_vec()
    } else {
        Vec::new()
    }
}

/// Read ICC profile from an image file
fn read_icc_profile(path: &Path) -> Option<Vec<u8>> {
    let data = std::fs::read(path).ok()?;
    // Look for ICC profile in JPEG APP2 marker
    if data.len() > 2 && data[0] == 0xFF && data[1] == 0xD8 {
        return extract_jpeg_icc(&data);
    }
    // For PNG, look for iCCP chunk
    if data.len() > 8 && &data[0..8] == b"\x89PNG\r\n\x1a\n" {
        return extract_png_icc(&data);
    }
    None
}

/// Extract ICC profile from JPEG APP2 markers
fn extract_jpeg_icc(data: &[u8]) -> Option<Vec<u8> > {
    let mut i = 2; // Skip SOI
    while i + 4 < data.len() {
        if data[i] != 0xFF {
            break;
        }
        let marker = data[i + 1];
        if marker == 0xDA {
            break; // Start of scan
        }
        let len = ((data[i + 2] as usize) << 8) | (data[i + 3] as usize);
        if marker == 0xE2 && len > 14 {
            // APP2 marker - check for ICC profile
            let header = &data[i + 4..i + 4 + 12];
            if header == b"ICC_PROFILE\0" {
                // Found ICC profile data
                let icc_data = &data[i + 4 + 14..i + 2 + len];
                return Some(icc_data.to_vec());
            }
        }
        i += 2 + len;
    }
    None
}

/// Extract ICC profile from PNG iCCP chunk
fn extract_png_icc(data: &[u8]) -> Option<Vec<u8>> {
    let mut i = 8; // Skip PNG signature
    while i + 8 < data.len() {
        let length = ((data[i] as usize) << 24)
            | ((data[i + 1] as usize) << 16)
            | ((data[i + 2] as usize) << 8)
            | (data[i + 3] as usize);
        let chunk_type = &data[i + 4..i + 8];
        if chunk_type == b"iCCP" {
            // iCCP chunk: name (null-terminated) + compression method + compressed profile
            let chunk_data = &data[i + 8..i + 8 + length];
            // Skip name (find null terminator)
            if let Some(null_pos) = chunk_data.iter().position(|&b| b == 0) {
                let compression = chunk_data.get(null_pos + 1)?;
                if *compression == 0 {
                    // zlib compressed - we'd need to decompress
                    // For now, skip this and let the fallback handle it
                    return None;
                }
            }
        }
        i += 12 + length; // 4 (length) + 4 (type) + data + 4 (crc)
    }
    None
}

/// Convert an image from source ICC profile to sRGB using qcms
fn convert_to_srgb(img: &DynamicImage, icc_data: &[u8]) -> Option<DynamicImage> {
    let input_profile = qcms::Profile::new_from_slice(icc_data, false)?;
    let srgb_profile = qcms::Profile::new_sRGB();

    let transform = qcms::Transform::new(
        &input_profile,
        &srgb_profile,
        qcms::DataType::RGB8,
        qcms::Intent::Perceptual,
    )?;

    // Apply transform in-place on RGB data
    let rgb = img.to_rgb8();
    let (w, h) = rgb.dimensions();
    let mut data = rgb.as_raw().to_vec();
    transform.apply(&mut data);

    let img_buffer = ImageBuffer::<image::Rgb<u8>, _>::from_raw(w, h, data)?;
    Some(DynamicImage::ImageRgb8(img_buffer))
}

/// Apply EXIF orientation to an image
pub fn apply_orientation(img: DynamicImage, orientation: u32) -> DynamicImage {
    match orientation {
        1 => img,
        2 => img.flipv(),
        3 => img.rotate180(),
        4 => img.fliph(),
        5 => img.flipv().rotate90(),
        6 => img.rotate90(),
        7 => img.fliph().rotate90(),
        8 => img.rotate270(),
        _ => img,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Rgb};
    use tempfile::TempDir;

    fn create_test_image(width: u32, height: u32) -> DynamicImage {
        let img: RgbImage = ImageBuffer::from_fn(width, height, |x, y| {
            Rgb([(x % 256) as u8, (y % 256) as u8, 128])
        });
        DynamicImage::ImageRgb8(img)
    }

    #[test]
    fn test_thumbnail_path_format() {
        let thumbs_dir = PathBuf::from("/thumbs");
        let path = thumbnail_path(&thumbs_dir, 240, "abcdef123456");
        assert_eq!(path, PathBuf::from("/thumbs/240/ab/abcdef123456.webp"));
    }

    #[test]
    fn test_thumbnail_path_short_hash() {
        let thumbs_dir = PathBuf::from("/thumbs");
        let path = thumbnail_path(&thumbs_dir, 480, "ab");
        assert_eq!(path, PathBuf::from("/thumbs/480/ab/ab.webp"));
    }

    #[test]
    fn test_resize_to_tier_fast() {
        let img = create_test_image(6000, 4000);
        let resized = resize_to_tier_fast(&img, 240);
        let (w, h) = resized.dimensions();
        // Short edge (4000) should be resized to 240
        assert_eq!(h, 240);
        assert_eq!(w, 360); // 6000 * (240/4000) = 360
    }

    #[test]
    fn test_resize_small_image_unchanged() {
        let img = create_test_image(100, 80);
        let resized = resize_to_tier_fast(&img, 240);
        let (w, h) = resized.dimensions();
        assert_eq!(w, 100);
        assert_eq!(h, 80);
    }

    #[test]
    fn test_apply_orientation() {
        let img = create_test_image(100, 200);

        // Orientation 1: no change
        let result = apply_orientation(img.clone(), 1);
        assert_eq!(result.dimensions(), (100, 200));

        // Orientation 6: rotate 90
        let result = apply_orientation(img.clone(), 6);
        assert_eq!(result.dimensions(), (200, 100));

        // Orientation 3: rotate 180
        let result = apply_orientation(img.clone(), 3);
        assert_eq!(result.dimensions(), (100, 200));
    }

    #[test]
    fn test_encode_webp_lossy() {
        let img = create_test_image(100, 100);
        let data = encode_webp_lossy(&img);
        assert!(!data.is_empty());
        // WebP magic bytes
        assert_eq!(&data[0..4], b"RIFF");
    }

    #[test]
    fn test_placeholder_generation() {
        let data = generate_placeholder(240);
        assert!(!data.is_empty());
        // Should be valid WebP
        assert_eq!(&data[0..4], b"RIFF");
    }

    #[test]
    fn test_placeholder_4_3_ratio() {
        let data = generate_placeholder(240);
        // Decode and check dimensions
        let img = image::load_from_memory(&data).unwrap();
        let (w, h) = img.dimensions();
        assert_eq!(w, 240);
        assert_eq!(h, 180); // 240 * 3/4 = 180
    }

    #[test]
    fn test_generate_thumbnails() {
        let tmp = TempDir::new().unwrap();
        let thumbs_dir = tmp.path().join("thumbs");

        // Create a test image
        let img = create_test_image(2000, 1500);
        let img_path = tmp.path().join("test.jpg");
        img.save(&img_path).unwrap();

        let results = generate_thumbnails(&img_path, &thumbs_dir, "testhash123", Some(1)).unwrap();

        assert_eq!(results.len(), 3);
        for thumb in &results {
            assert!(thumb.path.exists());
            assert!(thumb.path.to_string_lossy().contains(&thumb.tier.to_string()));
            assert!(thumb.width > 0);
            assert!(thumb.height > 0);
        }
    }

    #[test]
    fn test_lossy_encoding() {
        let img = create_test_image(2000, 1500);
        let lossy = encode_webp_lossy(&img);
        assert!(!lossy.is_empty());
        // Should be valid WebP
        assert_eq!(&lossy[0..4], b"RIFF");
    }
}

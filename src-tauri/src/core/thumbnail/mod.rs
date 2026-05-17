use image::codecs::webp::WebPEncoder;
use image::imageops::FilterType;
use image::{DynamicImage, GenericImageView, ImageBuffer, Rgb, RgbImage};
use std::io::Cursor;
use std::path::{Path, PathBuf};

/// Thumbnail tier sizes (short edge in pixels)
pub const TIER_SMALL: u32 = 240;
pub const TIER_MEDIUM: u32 = 480;
pub const TIER_LARGE: u32 = 1080;

/// All thumbnail tiers
pub const TIERS: &[u32] = &[TIER_SMALL, TIER_MEDIUM, TIER_LARGE];

/// Get the thumbnail file path for a given hash and tier
/// Format: {thumbs_dir}/{tier}/{hash[0:2]}/{hash}.webp
pub fn thumbnail_path(thumbs_dir: &Path, tier: u32, hash: &str) -> PathBuf {
    let prefix = if hash.len() >= 2 { &hash[..2] } else { hash };
    thumbs_dir
        .join(tier.to_string())
        .join(prefix)
        .join(format!("{}.webp", hash))
}

/// Generate a placeholder thumbnail for missing files
pub fn generate_placeholder(tier: u32) -> Vec<u8> {
    let size = tier;
    let mut img: RgbImage = ImageBuffer::new(size, size);

    // Fill with a light gray color
    for pixel in img.pixels_mut() {
        *pixel = Rgb([200, 200, 200]);
    }

    // Draw a simple "X" pattern
    let line_width = (size / 20).max(2);
    for i in 0..size {
        for j in 0..size {
            // Diagonal from top-left to bottom-right
            if (i as i32 - j as i32).unsigned_abs() < line_width {
                img.put_pixel(i, j, Rgb([150, 150, 150]));
            }
            // Diagonal from top-right to bottom-left
            if (i as i32 + j as i32 - size as i32).unsigned_abs() < line_width {
                img.put_pixel(i, j, Rgb([150, 150, 150]));
            }
        }
    }

    encode_webp(&DynamicImage::ImageRgb8(img))
}

/// Generate thumbnails for an image at all tiers
pub fn generate_thumbnails(
    image_path: &Path,
    thumbs_dir: &Path,
    hash: &str,
    orientation: Option<u32>,
) -> Result<Vec<(u32, PathBuf)>, String> {
    let img = image::open(image_path)
        .map_err(|e| format!("Failed to open image: {}", e))?;

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

        // Resize image
        let resized = resize_to_tier(&img, tier);

        // Encode to WebP
        let webp_data = encode_webp(&resized);

        // Write to file
        std::fs::write(&path, webp_data)
            .map_err(|e| format!("Failed to write thumbnail: {}", e))?;

        results.push((tier, path));
    }

    Ok(results)
}

/// Resize an image to fit within a tier's short edge
fn resize_to_tier(img: &DynamicImage, tier: u32) -> DynamicImage {
    let (width, height) = img.dimensions();
    let short_edge = width.min(height);

    if short_edge <= tier {
        return img.clone();
    }

    let scale = tier as f64 / short_edge as f64;
    let new_width = (width as f64 * scale).round() as u32;
    let new_height = (height as f64 * scale).round() as u32;

    img.resize_exact(new_width, new_height, FilterType::Lanczos3)
}

/// Apply EXIF orientation to an image
fn apply_orientation(img: DynamicImage, orientation: u32) -> DynamicImage {
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

/// Encode an image to WebP format
fn encode_webp(img: &DynamicImage) -> Vec<u8> {
    use image::ExtendedColorType;
    let rgb = img.to_rgb8();
    let mut cursor = Cursor::new(Vec::new());
    let encoder = WebPEncoder::new_lossless(&mut cursor);
    encoder.encode(rgb.as_raw(), rgb.width(), rgb.height(), ExtendedColorType::Rgb8).unwrap_or(());
    cursor.into_inner()
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
    fn test_resize_to_tier() {
        let img = create_test_image(6000, 4000);
        let resized = resize_to_tier(&img, 240);
        let (w, h) = resized.dimensions();
        // Short edge (4000) should be resized to 240
        assert_eq!(h, 240);
        assert_eq!(w, 360); // 6000 * (240/4000) = 360
    }

    #[test]
    fn test_resize_small_image_unchanged() {
        let img = create_test_image(100, 80);
        let resized = resize_to_tier(&img, 240);
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
    fn test_encode_webp() {
        let img = create_test_image(100, 100);
        let data = encode_webp(&img);
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
    fn test_generate_thumbnails() {
        let tmp = TempDir::new().unwrap();
        let thumbs_dir = tmp.path().join("thumbs");

        // Create a test image
        let img = create_test_image(2000, 1500);
        let img_path = tmp.path().join("test.jpg");
        img.save(&img_path).unwrap();

        let results = generate_thumbnails(&img_path, &thumbs_dir, "testhash123", Some(1)).unwrap();

        assert_eq!(results.len(), 3);
        for (tier, path) in &results {
            assert!(path.exists());
            assert!(path.to_string_lossy().contains(&tier.to_string()));
        }
    }
}

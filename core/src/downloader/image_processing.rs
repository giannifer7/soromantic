use anyhow::{Context, Result};
use image::GenericImageView;
use std::path::Path;

const BLACK_THRESHOLD: u8 = 15;

/// Detects black bars and crops the image in place.
/// Returns `Ok(true)` if the image was cropped, `Ok(false)` if unchanged.
///
/// # Errors
/// Returns error if the image cannot be opened or saved.
pub fn detect_and_crop(path: &Path) -> Result<bool> {
    const MARGIN: u32 = 2;
    let img =
        image::open(path).with_context(|| format!("Failed to open image {}", path.display()))?;
    let (w, h) = img.dimensions();

    // Grayscale for analysis
    let gray = img.to_luma8();

    // Find bounding box of non-black content
    let mut min_x = w;
    let mut max_x = 0;
    let mut min_y = h;
    let mut max_y = 0;

    let mut found_content = false;

    // Iterate pixels - optimization: could skip pixels for speed, but for thumbs (400x225) it's fast enough.
    for y in 0..h {
        for x in 0..w {
            // Unsafe get_pixel is faster but safe is okay here
            let p = gray.get_pixel(x, y)[0];
            if p > BLACK_THRESHOLD {
                if x < min_x {
                    min_x = x;
                }
                if x > max_x {
                    max_x = x;
                }
                if y < min_y {
                    min_y = y;
                }
                if y > max_y {
                    max_y = y;
                }
                found_content = true;
            }
        }
    }

    if !found_content {
        // Image is all black? Don't crop to 0x0
        return Ok(false);
    }

    // Check if crop is significant (ignore 1-2 pixel borders which might be compression noise or just frame edge)
    // If the content touches the edges (within margin), don't crop.
    if min_x <= MARGIN && min_y <= MARGIN && max_x >= w - 1 - MARGIN && max_y >= h - 1 - MARGIN {
        return Ok(false);
    }

    let crop_w = max_x - min_x + 1;
    let crop_h = max_y - min_y + 1;

    tracing::info!(
        "Auto-cropping {}: {w}x{h} -> {crop_w}x{crop_h} (Box: x={min_x}..{max_x}, y={min_y}..{max_y})",
        path.display()
    );

    // Crop the ORIGINAL image
    let mut img = img;
    let cropped = img.crop(min_x, min_y, crop_w, crop_h);

    // Save (overwrite)
    cropped
        .save(path)
        .with_context(|| format!("Failed to save cropped image {}", path.display()))?;

    Ok(true)
}

use anyhow::{Context, Result};
use std::path::Path;

/// Loads a thumbnail image, resizing it to the specified dimensions.
///
/// # Errors
/// Returns error if the file cannot be opened or parsed as an image.
pub fn load_thumbnail(path: &Path, width: u32, height: u32) -> Result<(u32, u32, Vec<u8>)> {
    let img =
        image::open(path).with_context(|| format!("Failed to open image: {}", path.display()))?;
    let img = img.resize(width, height, ::image::imageops::FilterType::Triangle);
    let rgba = img.to_rgba8();
    let w = rgba.width();
    let h = rgba.height();
    let pixels = rgba.into_raw();
    Ok((w, h, pixels))
}

/// Create a thumbnail file from a source image file.
/// Resizes to Fill 400x225 (16:9) using `FilterType::Triangle`.
///
/// # Errors
/// Returns error if reading or writing fails.
pub fn create_thumbnail(src: &Path, dest: &Path) -> Result<()> {
    let img =
        image::open(src).with_context(|| format!("Failed to open image: {}", src.display()))?;
    // Resize to 400x225
    let thumb = img.resize_to_fill(400, 225, image::imageops::FilterType::Triangle);

    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }

    thumb
        .save(dest)
        .with_context(|| format!("Failed to save thumbnail: {}", dest.display()))?;
    Ok(())
}

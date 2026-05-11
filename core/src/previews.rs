use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Ensures that preview frames exist for a given video ID.
/// If not, it runs ffmpeg to generate them.
/// Returns a sorted list of paths to the generated frames.
///
/// # Errors
/// Returns error if video file doesn't exist, ffmpeg fails, or IO errors occur.
pub fn ensure_preview_frames(
    id: i64,
    video_path: &Path,
    runtime_dir: &Path,
) -> Result<Vec<PathBuf>> {
    if !video_path.exists() {
        return Err(anyhow::anyhow!(
            "Video file not found: {}",
            video_path.display()
        ));
    }

    // Create temp dir (use padded ID for consistency)
    let tmp_dir = runtime_dir.join(format!(
        "{id:0width$}",
        width = crate::constants::ui::PAD_WIDTH
    ));
    std::fs::create_dir_all(&tmp_dir).context("Failed to create preview directory")?;

    // Check if frames already exist (simple caching)
    let existing_files = std::fs::read_dir(&tmp_dir).ok().map_or(0, Iterator::count);

    if existing_files == 0 {
        let output_pattern = tmp_dir.join("%03d.jpg");
        let output = Command::new("ffmpeg")
            .arg("-i")
            .arg(video_path)
            .arg("-vf")
            .arg("fps=12,scale=400:-1")
            .arg("-q:v")
            .arg("3") // High quality
            .arg(&output_pattern)
            .output()
            .context("Failed to execute ffmpeg")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("ffmpeg failed: {stderr}"));
        }
    }

    // Read frames
    let mut entries: Vec<_> = std::fs::read_dir(&tmp_dir)
        .context("Failed to read preview directory")?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "jpg"))
        .collect();

    // Sort by filename
    entries.sort();

    Ok(entries)
}

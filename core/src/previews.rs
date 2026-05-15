use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Ensures that preview frames exist for a given video ID.
///
/// If not, it runs ffmpeg to generate them.
/// Uses the configured ffmpeg path instead of PATH lookup.
/// Returns a sorted list of paths to the generated frames.
///
/// # Errors
/// Returns error if video file doesn't exist, the ffmpeg binary is not found, or IO errors occur.
pub fn ensure_preview_frames(
    id: i64,
    video_path: &Path,
    runtime_dir: &Path,
    ffmpeg_path: &Path,
) -> Result<Vec<PathBuf>> {
    tracing::info!(
        "[preview] id={id} video={} runtime={} ffmpeg={}",
        video_path.display(),
        runtime_dir.display(),
        ffmpeg_path.display()
    );

    if !video_path.exists() {
        tracing::warn!("[preview] id={id} video file NOT FOUND: {}", video_path.display());
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
    tracing::info!("[preview] id={id} tmp_dir={}", tmp_dir.display());

    // Check if frames already exist (simple caching)
    let existing_files = std::fs::read_dir(&tmp_dir).ok().map_or(0, Iterator::count);

    if existing_files == 0 {
        let output_pattern = tmp_dir.join("%03d.jpg");
        tracing::info!(
            "[preview] id={id} running ffmpeg: {} -i {} -vf fps=12,scale=400:-1 -q:v 3 {}",
            ffmpeg_path.display(),
            video_path.display(),
            output_pattern.display()
        );
        let output = Command::new(ffmpeg_path)
            .arg("-i")
            .arg(video_path)
            .arg("-vf")
            .arg("fps=12,scale=400:-1")
            .arg("-q:v")
            .arg("3") // High quality
            .arg(&output_pattern)
            .output()
            .with_context(|| format!("Failed to execute ffmpeg at {}", ffmpeg_path.display()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            tracing::error!("[preview] id={id} ffmpeg FAILED: {stderr}");
            return Err(anyhow::anyhow!("ffmpeg failed: {stderr}"));
        }
        tracing::info!("[preview] id={id} ffmpeg completed OK");
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

    tracing::info!("[preview] id={id} extracted {} frames", entries.len());
    Ok(entries)
}
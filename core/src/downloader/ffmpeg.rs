//! FFMPEG wrappers for video processing and HLS downloads.

use anyhow::{Context, Result};
use serde::Deserialize;
use std::io::Read;
use std::path::Path;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

/// Video metadata from ffprobe.
#[derive(Debug, Clone, Default)]
pub struct VideoMeta {
    pub width: i64,
    pub height: i64,
    pub duration: f64,
}

/// Progress callback for HLS downloads (`current_us`, `total_us`).
pub type HlsProgressCallback = Arc<dyn Fn(i64, i64) + Send + Sync>;

/// Extract a snapshot frame from a video file with high quality.
/// # Errors
/// Returns error if ffmpeg fails to extract the snapshot.
/// Helper to ensure parent directory exists and run a command.
async fn prepare_dir_and_run(cmd: &mut Command, dest: &Path) -> Result<()> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let status = cmd.status().await.context("Failed to run ffmpeg")?;

    if !status.success() {
        anyhow::bail!("ffmpeg command failed with status: {status}");
    }

    Ok(())
}

/// Extract a snapshot frame from a video file with high quality.
/// # Errors
/// Returns error if ffmpeg fails to extract the snapshot.
pub async fn extract_snapshot(
    ffmpeg_path: &Path,
    video_path: &Path,
    dest_path: &Path,
) -> Result<()> {
    let mut cmd = Command::new(ffmpeg_path);
    cmd.arg("-i")
        .arg(video_path)
        .arg("-ss")
        .arg(crate::constants::media::SNAPSHOT_OFFSET_SECS) // Take snapshot at offset
        .arg("-vframes")
        .arg("1")
        .arg("-q:v")
        .arg(crate::constants::media::SNAPSHOT_QUALITY) // High quality
        .arg("-y") // Overwrite
        .arg(dest_path)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());

    prepare_dir_and_run(&mut cmd, dest_path)
        .await
        .context("Failed to extract snapshot")
}

/// Download HLS stream using ffmpeg (no progress tracking).
///
/// # Errors
/// Returns error if ffmpeg fails to download the stream.
pub async fn download_hls(ffmpeg_path: &Path, url: &str, dest_path: &Path) -> Result<()> {
    let mut cmd = Command::new(ffmpeg_path);
    cmd.arg("-i")
        .arg(url)
        .arg("-c")
        .arg("copy")
        .arg("-bsf:a")
        .arg("aac_adtstoasc")
        .arg("-f")
        .arg("mp4")
        .arg("-y")
        .arg(dest_path);

    prepare_dir_and_run(&mut cmd, dest_path)
        .await
        .context("Failed to download HLS")
}

/// Download HLS stream with progress tracking.
///
/// Progress is reported via callback with (`current_us`, `total_us`).
/// Uses ffmpeg's `-progress pipe:1` output format.
///
/// # Errors
/// Returns error if ffmpeg fails to download the stream.
pub async fn download_hls_with_progress(
    ffmpeg_path: &Path,
    url: &str,
    dest_path: &Path,
    total_duration_us: Option<i64>,
    on_progress: Option<HlsProgressCallback>,
) -> Result<()> {
    // Ensure parent directory exists
    if let Some(parent) = dest_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let part_path = dest_path.with_extension("mp4.part");

    let mut child = Command::new(ffmpeg_path)
        .arg("-i")
        .arg(url)
        .arg("-c")
        .arg("copy")
        .arg("-bsf:a")
        .arg("aac_adtstoasc")
        .arg("-f")
        .arg("mp4")
        .arg("-y")
        .arg("-progress")
        .arg("pipe:1")
        .arg("-nostats")
        .arg(&part_path)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .context("Failed to spawn ffmpeg for HLS download")?;

    // Read progress from stdout
    if let (Some(stdout), Some(cb), Some(total_us)) =
        (child.stdout.take(), on_progress.as_ref(), total_duration_us)
    {
        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();

        while let Ok(Some(line)) = lines.next_line().await {
            if let Some(current_us) = parse_progress_line(&line) {
                cb(current_us, total_us);
            }
        }
    }

    let status = child.wait().await?;

    if !status.success() {
        let _ = std::fs::remove_file(&part_path);
        anyhow::bail!("ffmpeg HLS download failed with status: {status}");
    }

    // Rename part file to final destination
    std::fs::rename(&part_path, dest_path)?;

    Ok(())
}

/// Parse ffmpeg progress output line for `out_time_us` value.
fn parse_progress_line(line: &str) -> Option<i64> {
    if !line.starts_with("out_time_us=") {
        return None;
    }

    line.strip_prefix("out_time_us=")?
        .trim()
        .parse::<i64>()
        .ok()
}

/// Probe video file for metadata using ffprobe.
///
/// # Errors
/// Returns error if ffprobe fails to probe the file or if JSON parsing fails.
pub async fn probe_video(ffprobe_path: &Path, video_path: &Path) -> Result<VideoMeta> {
    let output = Command::new(ffprobe_path)
        .arg("-v")
        .arg("quiet")
        .arg("-print_format")
        .arg("json")
        .arg("-show_format")
        .arg("-show_streams")
        .arg(video_path)
        .output()
        .await
        .context("Failed to run ffprobe")?;

    if !output.status.success() {
        anyhow::bail!("ffprobe failed with status: {}", output.status);
    }

    parse_ffprobe_output(&output.stdout)
}

/// Probe duration of a media file or URL.
///
/// Returns duration in seconds.
///
/// # Errors
/// Returns error if ffprobe fails to probe the file or if JSON parsing fails.
pub async fn probe_duration(ffprobe_path: &Path, url_or_path: &str) -> Result<f64> {
    #[derive(Deserialize)]
    struct Format {
        duration: Option<String>,
    }

    #[derive(Deserialize)]
    struct ProbeOutput {
        format: Option<Format>,
    }

    let output = Command::new(ffprobe_path)
        .arg("-v")
        .arg("quiet")
        .arg("-print_format")
        .arg("json")
        .arg("-show_format")
        .arg(url_or_path)
        .output()
        .await
        .context("Failed to run ffprobe")?;

    if !output.status.success() {
        anyhow::bail!("ffprobe failed with status: {}", output.status);
    }

    let probe: ProbeOutput = serde_json::from_slice(&output.stdout)?;
    let duration_sec: f64 = probe
        .format
        .and_then(|f| f.duration)
        .and_then(|d| d.parse().ok())
        .ok_or_else(|| anyhow::anyhow!("No duration found in ffprobe output"))?;

    Ok(duration_sec)
}

/// Parse ffprobe JSON output into `VideoMeta`.
fn parse_ffprobe_output(stdout: &[u8]) -> Result<VideoMeta> {
    #[derive(Deserialize)]
    struct Stream {
        codec_type: Option<String>,
        width: Option<i64>,
        height: Option<i64>,
    }

    #[derive(Deserialize)]
    struct Format {
        duration: Option<String>,
    }

    #[derive(Deserialize)]
    struct ProbeOutput {
        streams: Option<Vec<Stream>>,
        format: Option<Format>,
    }

    let probe: ProbeOutput = serde_json::from_slice(stdout)?;

    let video_stream = probe
        .streams
        .unwrap_or_default()
        .into_iter()
        .find(|s| s.codec_type.as_deref() == Some("video"));

    let (width, height) =
        video_stream.map_or((0, 0), |s| (s.width.unwrap_or(0), s.height.unwrap_or(0)));

    let duration: f64 = probe
        .format
        .and_then(|f| f.duration)
        .and_then(|d| d.parse().ok())
        .unwrap_or(0.0);

    Ok(VideoMeta {
        width,
        height,
        duration,
    })
}

/// Check if a file is a valid image (JPEG or PNG).
///
/// Reads file header to check magic bytes.
#[must_use]
pub fn is_valid_image(path: &Path) -> bool {
    let Ok(mut file) = std::fs::File::open(path) else {
        return false;
    };

    // Check file is not empty
    if file.metadata().map_or(true, |meta| meta.len() == 0) {
        return false;
    }

    // Read first 8 bytes for magic number check
    let mut header = [0u8; 8];
    if file.read_exact(&mut header).is_err() {
        return false;
    }

    // JPEG: starts with 0xFF 0xD8
    if header[0] == 0xFF && header[1] == 0xD8 {
        return true;
    }

    // PNG: starts with 0x89 PNG\r\n\x1a\n
    if header.starts_with(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]) {
        return true;
    }

    // Check for HTML (error pages)
    let header_lower: Vec<u8> = header.iter().map(u8::to_ascii_lowercase).collect();
    if header_lower.starts_with(b"<html") || header_lower.starts_with(b"<!doc") {
        return false;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_progress_line_valid() {
        assert_eq!(parse_progress_line("out_time_us=1234567"), Some(1_234_567));
    }

    #[test]
    fn test_parse_progress_line_invalid() {
        assert_eq!(parse_progress_line("progress=continue"), None);
        assert_eq!(parse_progress_line("out_time=00:00:05"), None);
    }

    #[test]
    fn test_is_valid_image_nonexistent() {
        assert!(!is_valid_image(Path::new("/nonexistent/file.jpg")));
    }
}

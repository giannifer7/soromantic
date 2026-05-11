//! Downloader module for managing file downloads.

pub mod core;
pub mod ffmpeg;
pub mod image_processing;
pub mod paths;
pub mod tasks;

// Re-export commonly used items
pub use core::{
    DownloadConfig, DownloadProgressCallback, DownloadResult, download_file_robust,
    download_file_simple,
};
pub use ffmpeg::{
    VideoMeta, download_hls_with_progress, extract_snapshot, is_valid_image, probe_duration,
    probe_video,
};
pub use paths::{FileType, get_download_path, get_file_extension};
pub use tasks::{
    download_cover_workflow, download_previews_for_grid, download_thumbs_for_grid,
    download_video_workflow, generate_fallback_thumbs, is_hls_url,
};

use anyhow::Result;
use std::path::Path;
use std::sync::Arc;

/// Generic progress callback (JSON events for UI)
pub type ProgressCallback = Arc<dyn Fn(serde_json::Value) + Send + Sync>;

/// Download a file from a URL to a local path with optional progress tracking.
///
/// This is a simplified version for backwards compatibility. For robust downloads,
/// use [`download_file_robust`] from the `core` submodule.
///
/// # Errors
/// Returns error if network request fails, server returns error status, or IO errors occur.
pub async fn download_file(
    url: &str,
    dest: &Path,
    on_progress: Option<&ProgressCallback>,
) -> Result<()> {
    let adapter = on_progress.map(|cb| {
        let cb = cb.clone();
        let url = url.to_string();
        Arc::new(move |current, total| {
            cb(serde_json::json!({
                "type": "download_progress",
                "current": current,
                "total": total,
                "url": url
            }));
        }) as DownloadProgressCallback
    });

    core::download_file_simple(url, dest, adapter.as_ref()).await
}

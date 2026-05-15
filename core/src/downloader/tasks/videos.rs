use super::super::{ProgressCallback, download_file, ffmpeg, paths};
use crate::config::ResolvedConfig;
use crate::db::Database;
use anyhow::Result;
use std::path::Path;
use std::sync::Arc;

/// Helper to check if URL is HLS.
#[must_use]
pub fn is_hls_url(url: &str) -> bool {
    url.to_lowercase().contains(".m3u8")
}

/// Helper to check if a file exists and is not empty.
#[must_use]
pub fn check_file_exists(path: &Path) -> bool {
    path.exists() && std::fs::metadata(path).is_ok_and(|m| m.len() > 0)
}

/// Download main video workflow.
///
/// # Errors
/// Returns error if no video sources are found, download fails, or IO errors occur.
pub async fn download_video_workflow(
    db: Arc<Database>,
    page_id: i64,
    config: &ResolvedConfig,
    on_progress: Option<ProgressCallback>,
    video_sources: Vec<(String, i64)>,
) -> Result<()> {
    // 1. Fetch sources (Provided)
    if video_sources.is_empty() {
        anyhow::bail!("No video sources found for page {page_id}");
    }

    // 2. Select resolution based on preferences
    let (video_url, resolution) =
        select_best_source(&video_sources, &config.playback.video_preferences)?;
    tracing::info!("Selected video source for page {page_id}: {resolution}p - {video_url}");

    // Use paths module for proper path generation
    let dest_path = paths::get_download_path(
        config,
        page_id,
        Some(video_url),
        paths::FileType::Video,
        Some(*resolution),
    );

    // Ensure directory exists and check for existing file
    if check_video_exists(&dest_path)? {
        // Ensure DB status is consistent for both page and video_source
        // This is crucial for self-healing: if file exists, we must ensure status is DONE (3)
        // We also probe duration to ensuring metadata is correct.
        let ffprobe_path = &config.ffprobe_path;
        match ffmpeg::probe_duration(ffprobe_path, dest_path.to_string_lossy().as_ref()).await {
            Ok(duration_us) => {
                let duration_sec = duration_us / crate::constants::time::MICROSECONDS_PER_SECOND;
                tracing::info!("Existing video duration: {duration_sec:.2}s");
                if let Err(e) = db
                    .set_video_source_done(page_id, *resolution, duration_sec)
                    .await
                {
                    tracing::warn!("Failed to update existing video source in DB: {e}");
                }
            }
            Err(e) => {
                tracing::warn!("Failed to probe existing video duration: {e}");
                // Still update status
                if let Err(e) = db.set_video_source_done(page_id, *resolution, 0.0).await {
                    tracing::warn!("Failed to update existing video source status in DB: {e}");
                }
            }
        }

        db.set_page_video_status(page_id, crate::constants::status::DONE)
            .await?;
        return Ok(());
    }

    // 3. Download
    notify_start(on_progress.as_ref(), page_id, *resolution);

    if is_hls_url(video_url) {
        process_hls_download(config, video_url, &dest_path, page_id, on_progress.as_ref()).await?;
    } else {
        process_direct_download(video_url, &dest_path, page_id, on_progress.as_ref()).await?;
    }

    tracing::info!("Video download complete: {}", dest_path.display());

    // 4. Update DB - mark as downloaded
    db.set_page_video_status(page_id, crate::constants::status::DONE)
        .await?;
    // Also record in downloads table for path mapping (REMOVED)
    // db.record_download(...) replaced by status in video_sources

    // 5. Probe local file for duration and update DB
    let ffprobe_path = &config.ffprobe_path;
    match ffmpeg::probe_duration(ffprobe_path, dest_path.to_string_lossy().as_ref()).await {
        Ok(duration_us) => {
            let duration_sec = duration_us / crate::constants::time::MICROSECONDS_PER_SECOND;
            tracing::info!("Checked duration: {duration_sec:.2}s");
            // Update DB with confirmed duration and status
            if let Err(e) = db
                .set_video_source_done(page_id, *resolution, duration_sec)
                .await
            {
                tracing::warn!("Failed to update video source in DB: {e}");
            }
        }
        Err(e) => {
            tracing::warn!("Failed to probe local file duration: {e}");
            // Still update status even if duration probe failed
            if let Err(e) = db.set_video_source_done(page_id, *resolution, 0.0).await {
                tracing::warn!("Failed to update video source status in DB: {e}");
            }
        }
    }

    Ok(())
}

fn select_best_source<'a>(
    sources: &'a [(String, i64)],
    preferences: &[i64],
) -> Result<&'a (String, i64)> {
    // Filter logic
    let valid_sources: Vec<_> = sources.iter().filter(|(_url, _)| true).collect();

    if valid_sources.is_empty() {
        anyhow::bail!("No suitable video source found");
    }

    // 1. Try to find a source matching preferences in order
    for &pref in preferences {
        if let Some(source) = valid_sources.iter().find(|(_, res)| *res == pref) {
            return Ok(source);
        }
    }

    // 2. Fallback: select best quality available (highest resolution)
    // Sort by resolution descending
    let mut sorted_sources = valid_sources.clone();
    sorted_sources.sort_by_key(|b| std::cmp::Reverse(b.1));

    Ok(sorted_sources[0])
}

fn check_video_exists(dest_path: &Path) -> Result<bool> {
    if let Some(parent) = dest_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    if check_file_exists(dest_path) {
        tracing::info!("Video already exists: {}", dest_path.display());
        return Ok(true);
    }
    Ok(false)
}

fn notify_start(on_progress: Option<&ProgressCallback>, page_id: i64, resolution: i64) {
    if let Some(cb) = on_progress {
        let msg = format!("Starting download: {resolution}p");
        cb(serde_json::json!({
            "type": "scrape_progress",
            "page_id": page_id,
            "stage": "downloading_video",
            "progress": 0,
            "total": 0,
            "message": msg
        }));
    }
}

async fn process_hls_download(
    config: &ResolvedConfig,
    video_url: &str,
    dest_path: &Path,
    page_id: i64,
    on_progress: Option<&ProgressCallback>,
) -> Result<()> {
    tracing::info!("Starting HLS download via ffmpeg...");

    // Use configured binary paths
    let ffmpeg_path = &config.ffmpeg_path;
    let ffprobe_path = &config.ffprobe_path;

    // Probe duration for progress (seconds -> microseconds for HLS callback)
    let duration_sec = ffmpeg::probe_duration(ffprobe_path, video_url).await.ok();
    #[allow(clippy::cast_possible_truncation)]
    let duration_us =
        duration_sec.map(|s| (s * crate::constants::time::MICROSECONDS_PER_SECOND) as i64);

    // Wrap callback
    let hls_cb: Option<ffmpeg::HlsProgressCallback> = on_progress.map(|cb| {
        let cb = Arc::clone(cb);
        Arc::new(move |current_us: i64, total_us: i64| {
            let pct = if total_us > 0 {
                #[allow(clippy::cast_precision_loss)]
                {
                    current_us as f64 / total_us as f64 * 100.0
                }
            } else {
                0.0
            };
            cb(serde_json::json!({
               "type": "scrape_progress",
               "page_id": page_id,
               "stage": "downloading_video",
               "progress": u64::try_from(current_us).unwrap_or(0),
               "total": u64::try_from(total_us).unwrap_or(0),
               "message": format!("Downloading HLS: {pct:.1}% [{current_us}/{total_us}]")
            }));
        }) as ffmpeg::HlsProgressCallback
    });

    ffmpeg::download_hls_with_progress(ffmpeg_path, video_url, dest_path, duration_us, hls_cb)
        .await?;

    Ok(())
}

async fn process_direct_download(
    video_url: &str,
    dest_path: &Path,
    page_id: i64,
    on_progress: Option<&ProgressCallback>,
) -> Result<()> {
    tracing::info!("Starting direct HTTP download...");

    // Wrap callback to emit scrape_progress structure
    let wrapped_cb = on_progress.map(|cb| {
        let cb = Arc::clone(cb);
        Arc::new(move |json: serde_json::Value| {
            if let Some(current) = json.get("current").and_then(serde_json::Value::as_u64) {
                let total = json
                    .get("total")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(0);
                let pct = if total > 0 {
                    #[allow(clippy::cast_precision_loss)]
                    {
                        current as f64 / total as f64 * 100.0
                    }
                } else {
                    0.0
                };
                #[allow(clippy::cast_precision_loss)]
                let mb_curr = current as f64 / 1024.0 / 1024.0;
                #[allow(clippy::cast_precision_loss)]
                let mb_tot = total as f64 / 1024.0 / 1024.0;

                cb(serde_json::json!({
                   "type": "scrape_progress",
                   "page_id": page_id,
                   "stage": "downloading_video",
                   "progress": current,
                   "total": total,
                   "message": format!("Downloading Video: {pct:.1}% ({mb_curr:.1}/{mb_tot:.1} MB) [{current}/{total}]")
                }));
            }
        }) as ProgressCallback
    });

    download_file(video_url, dest_path, wrapped_cb.as_ref()).await?;
    Ok(())
}
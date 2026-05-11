use super::super::{ProgressCallback, download_file, paths};
use crate::config::ResolvedConfig;
use crate::db::Database;
use crate::media::get_probe;
use anyhow::Result;
use std::sync::Arc;

/// Download cover workflow.
///
/// # Errors
/// Returns error if cover download fails and snapshot fallback also fails.
///
/// # Panics
/// Panics if the generated thumbnail path has no parent directory (which should strictly never happen).
pub async fn download_cover_workflow(
    db: Arc<Database>,
    page_id: i64,
    cover_url: Option<String>,
    config: &ResolvedConfig,
    on_progress: Option<ProgressCallback>,
) -> Result<()> {
    // let url_for_path = cover_url.as_deref().unwrap_or("dummy.jpg"); // Removed
    let cover_path = paths::get_download_path(
        config,
        page_id,
        cover_url.as_deref(),
        paths::FileType::Cover,
        None,
    );

    // Ensure covers directory exists
    if let Some(parent) = cover_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // 1. Try download if URL exists
    if let Some(url) = &cover_url
        && !url.is_empty()
    {
        tracing::info!("Downloading cover for page {page_id}: {url}");
        match download_file(url, &cover_path, on_progress.as_ref()).await {
            Ok(()) => {
                tracing::info!("Cover downloaded successfully: {}", cover_path.display());

                if url.contains("xvideos")
                    && let Err(e) = super::super::image_processing::detect_and_crop(&cover_path)
                {
                    tracing::warn!("Failed to auto-crop cover {url}: {e}");
                }

                db.mark_cover_downloaded(page_id, 1).await?;
                db.set_page_thumb_status(page_id, crate::constants::status::DONE)
                    .await?;
                return Ok(());
            }
            Err(e) => {
                tracing::warn!("Failed to download cover from {url}: {e}");
            }
        }
    }

    // New logic: After cover download (or even if it failed but we might have it from earlier?),
    // Check if we need to generate a thumbnail.
    // If cover exists but thumb does not, create it.
    if cover_path.exists() {
        let thumb_path = paths::get_download_path(
            config,
            page_id,
            Some("dummy_thumb.jpg"), // We just want the base dir logic
            paths::FileType::Thumb,
            None,
        );
        // Ensure filename matches page_id convention (trust paths.rs now)

        if !thumb_path.exists() {
            tracing::info!(
                "Generating thumbnail from cover: {} -> {}",
                cover_path.display(),
                thumb_path.display()
            );
            match crate::images::create_thumbnail(&cover_path, &thumb_path) {
                Ok(()) => {
                    tracing::info!("Thumbnail generated successfully");
                    // Insert into DB
                    let _ = db
                        .set_page_thumb_status(page_id, crate::constants::status::DONE)
                        .await;
                }
                Err(e) => {
                    tracing::error!("Failed to generate thumbnail: {}", e);
                }
            }
        }
    }

    // 2. Fallback: Snapshot from video
    tracing::info!("Attempting snapshot fallback for page {page_id}");

    if let Some(video_path) = db.find_downloaded_video(page_id).await? {
        tracing::info!("Found local video for snapshot: {video_path}");
        match get_probe()
            .extract_thumbnail(&video_path, cover_path.to_string_lossy().as_ref())
            .await
        {
            Ok(()) => {
                tracing::info!("Snapshot created successfully: {}", cover_path.display());
                db.mark_cover_downloaded(page_id, 2).await?;
                // Sync thumb status
                db.set_page_thumb_status(page_id, crate::constants::status::DONE)
                    .await?;
                return Ok(());
            }
            Err(e) => {
                tracing::error!("Failed to extract snapshot: {e}");
            }
        }
    } else {
        tracing::warn!("No local video found for snapshot fallback for page {page_id}");
    }

    Err(anyhow::anyhow!(
        "Failed to download cover or create snapshot"
    ))
}

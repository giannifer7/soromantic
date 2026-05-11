use super::super::{ProgressCallback, download_file, paths};
use crate::config::ResolvedConfig;
use crate::db::{Database, GridItem};
use crate::media::get_probe;
use anyhow::Result;
use std::path::Path;
use std::sync::Arc;

enum DownloadStatus {
    Success,
    Failure,
    Skipped,
}

macro_rules! run_grid_loop {
    ($items:expr, $db:expr, $config:expr, $on_progress:expr, $process_fn:ident) => {{
        let mut success_count = 0;
        let mut fail_count = 0;
        let total = $items.len();
        let mut skip_count = 0;

        for (idx, item) in $items.iter().enumerate() {
            match $process_fn($db, $config, item, idx, total, $on_progress.as_ref()).await {
                Ok(DownloadStatus::Success) => success_count += 1,
                Ok(DownloadStatus::Failure) | Err(_) => fail_count += 1,
                Ok(DownloadStatus::Skipped) => skip_count += 1,
            }
        }

        if skip_count > 0 {
            tracing::debug!("Download: skipped {skip_count} items");
        }

        Ok((success_count, fail_count))
    }};
}

/// Download thumbnails for a list of grid items.
///
/// # Errors
/// Returns error if database queries fail.
pub async fn download_thumbs_for_grid(
    db: Arc<Database>,
    config: &ResolvedConfig,
    grid_items: &[GridItem],
    on_progress: Option<ProgressCallback>,
) -> Result<(usize, usize)> {
    run_grid_loop!(grid_items, &db, config, on_progress, process_thumb_item)
}

async fn process_thumb_item(
    db: &Arc<Database>,
    config: &ResolvedConfig,
    item: &GridItem,
    idx: usize,
    total: usize,
    on_progress: Option<&ProgressCallback>,
) -> Result<DownloadStatus> {
    let Some(thumb_url) = item.image.as_ref() else {
        tracing::debug!("Skipping item {idx}: no thumb URL");
        emit_progress(on_progress, "downloading_thumbs", idx, total, "Thumbnails");
        return Ok(DownloadStatus::Skipped);
    };

    let Some(related_id) = item.related_id else {
        tracing::debug!("Skipping item {idx}: no related_id");
        emit_progress(on_progress, "downloading_thumbs", idx, total, "Thumbnails");
        return Ok(DownloadStatus::Skipped);
    };

    let dest = paths::get_download_path(
        config,
        related_id,
        Some(thumb_url.as_str()),
        paths::FileType::Thumb,
        None,
    );

    if check_file_exists(&dest) {
        return Ok(DownloadStatus::Success);
    }

    let status = match download_file(thumb_url, &dest, None).await {
        Ok(()) => {
            if thumb_url.contains("xvideos")
                && let Err(e) = super::super::image_processing::detect_and_crop(&dest)
            {
                tracing::warn!("Failed to auto-crop {thumb_url}: {e}");
            }
            let _ = db
                .set_page_thumb_status(related_id, crate::constants::status::DONE)
                .await;
            DownloadStatus::Success
        }
        Err(e) => {
            tracing::warn!("Failed to download thumb {thumb_url}: {e}");
            let _ = db
                .set_page_thumb_status(related_id, crate::constants::status::ERROR)
                .await;
            DownloadStatus::Failure
        }
    };

    emit_progress(on_progress, "downloading_thumbs", idx, total, "Thumbnails");
    Ok(status)
}

/// Download preview videos for a list of grid items.
///
/// # Errors
/// Returns error if database queries fail.
pub async fn download_previews_for_grid(
    db: Arc<Database>,
    config: &ResolvedConfig,
    grid_items: &[GridItem],
    on_progress: Option<ProgressCallback>,
) -> Result<(usize, usize)> {
    run_grid_loop!(grid_items, &db, config, on_progress, process_preview_item)
}

async fn process_preview_item(
    db: &Arc<Database>,
    config: &ResolvedConfig,
    item: &GridItem,
    idx: usize,
    total: usize,
    on_progress: Option<&ProgressCallback>,
) -> Result<DownloadStatus> {
    let Some(preview_url) = item.preview_url.as_ref() else {
        return Ok(DownloadStatus::Skipped);
    };
    let Some(related_id) = item.related_id else {
        return Ok(DownloadStatus::Skipped);
    };

    let dest = paths::get_download_path(
        config,
        related_id,
        Some(preview_url.as_str()),
        paths::FileType::Preview,
        None,
    );

    if check_file_exists(&dest) {
        return Ok(DownloadStatus::Success);
    }

    let status = match download_file(preview_url, &dest, None).await {
        Ok(()) => {
            let _ = db
                .set_page_preview_status(related_id, crate::constants::status::DONE)
                .await;
            DownloadStatus::Success
        }
        Err(e) => {
            tracing::warn!("Failed to download preview {preview_url}: {e}");
            let _ = db
                .set_page_preview_status(related_id, crate::constants::status::ERROR)
                .await;
            DownloadStatus::Failure
        }
    };

    emit_progress(on_progress, "downloading_previews", idx, total, "Previews");
    Ok(status)
}

/// Generate fallback thumbnails
///
/// # Errors
/// Returns error if database queries fail.
pub async fn generate_fallback_thumbs(
    db: Arc<Database>,
    config: &ResolvedConfig,
    grid_items: &[GridItem],
) -> Result<usize> {
    let mut generated_count = 0;
    for item in grid_items {
        if process_fallback_item(&db, config, item).await? {
            generated_count += 1;
        }
    }
    Ok(generated_count)
}

async fn process_fallback_item(
    db: &Arc<Database>,
    config: &ResolvedConfig,
    item: &GridItem,
) -> Result<bool> {
    let Some(related_id) = item.related_id else {
        return Ok(false);
    };

    let thumb_path = item.image.as_ref().map_or_else(
        || {
            paths::get_download_path(
                config,
                related_id,
                None, // Use default extension
                paths::FileType::Thumb,
                None,
            )
        },
        |url| {
            paths::get_download_path(
                config,
                related_id,
                Some(url.as_str()),
                paths::FileType::Thumb,
                None,
            )
        },
    );

    if check_file_exists(&thumb_path) {
        return Ok(false);
    }

    let Some(preview_url) = &item.preview_url else {
        return Ok(false);
    };
    let preview_path = paths::get_download_path(
        config,
        related_id,
        Some(preview_url.as_str()),
        paths::FileType::Preview,
        None,
    );

    if preview_path.exists() {
        let probe = get_probe();
        match probe
            .extract_thumbnail(
                preview_path.to_string_lossy().as_ref(),
                thumb_path.to_string_lossy().as_ref(),
            )
            .await
        {
            Ok(()) => {
                update_fallback_db(db, item, related_id, &thumb_path).await;
                return Ok(true);
            }
            Err(e) => {
                tracing::warn!("Failed to extract snapshot for {related_id}: {e}");
            }
        }
    }
    Ok(false)
}

async fn update_fallback_db(
    db: &Arc<Database>,
    _item: &GridItem,
    related_id: i64,
    _thumb_path: &Path,
) {
    // If we successfully created a fallback thumb, just mark the page status as done.
    if let Err(e) = db
        .set_page_thumb_status(related_id, crate::constants::status::DONE)
        .await
    {
        tracing::warn!("Failed to record fallback thumb for {related_id}: {e}");
    }
}

fn check_file_exists(path: &Path) -> bool {
    path.exists() && std::fs::metadata(path).is_ok_and(|m| m.len() > 0)
}

fn emit_progress(
    cb: Option<&ProgressCallback>,
    stage: &str,
    idx: usize,
    total: usize,
    label: &str,
) {
    if let Some(cb) = cb {
        cb(serde_json::json!({
            "type": "scrape_progress",
            "stage": stage,
            "progress": idx + 1,
            "total": total,
            "message": format!("{label}: {curr}/{total}", curr = idx + 1)
        }));
    }
}

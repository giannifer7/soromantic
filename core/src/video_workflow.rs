use crate::db::Database;
use crate::downloader::{
    DownloadConfig, download_previews_for_grid, download_thumbs_for_grid, generate_fallback_thumbs,
};
use crate::model_workflow::WorkflowConfig;
use crate::scripting::ProgressUpdate;
use crate::scripting::glue::{extract_list, extract_opt_string, extract_string, into_object};
use anyhow::{Context, Result};
use rune::runtime::Value;
use std::sync::mpsc;

use rune::from_value;

#[derive(Debug)]
pub struct RuneVideoResult {
    pub title: String,
    pub url: String,
    pub image: Option<String>,
    pub preview_url: Option<String>,
    pub models: Vec<(String, String)>,
    pub featuring: Vec<(String, String)>,
    pub studio: Option<(String, String)>,
    pub grid_boxes: Vec<crate::scraper::ScrapedVideo>,
    pub video_sources: Vec<(String, i64)>, // (url, resolution)
}

impl RuneVideoResult {
    #[allow(clippy::too_many_lines)]
    fn from_rune(value: Value) -> Result<Self> {
        // Calculate Result first to handle potential Result object from Rune
        let value = match from_value::<std::result::Result<Value, Value>>(value.clone()) {
            Ok(res) => match res {
                Ok(v) => v,
                Err(e) => anyhow::bail!("Scraper returned error: {e:?}"),
            },
            Err(_) => value,
        };

        let obj = into_object(value)?;

        let title = extract_string(&obj, "title").context("Extracting title")?;
        let url = extract_string(&obj, "url").context("Extracting url")?;
        let image = extract_opt_string(&obj, "image")?;
        let preview_url = extract_opt_string(&obj, "preview_url")?;

        let models = extract_list(&obj, "models").map_or_else(
            |_| Vec::new(),
            |v_vals| {
                let mut valid_models = Vec::new();
                for item in v_vals {
                    // Try extracting tuple (String, String)
                    if let Ok((name, url)) = rune::from_value::<(String, String)>(item) {
                        valid_models.push((name, url));
                    }
                }
                valid_models
            },
        );

        let featuring = extract_list(&obj, "featuring").map_or_else(
            |_| Vec::new(),
            |v_vals| {
                let mut valid = Vec::new();
                for item in v_vals {
                    if let Ok((name, url)) = rune::from_value::<(String, String)>(item) {
                        valid.push((name, url));
                    }
                }
                valid
            },
        );

        let studio = extract_list(&obj, "studio").map_or_else(
            |_| None,
            |v_vals| {
                if v_vals.is_empty() {
                    return None;
                }
                rune::from_value::<(String, String)>(v_vals[0].clone()).ok()
            },
        );

        let grid_boxes = extract_list(&obj, "grid_boxes").map_or_else(
            |_| Vec::new(),
            |v_vals| {
                let mut valid = Vec::new();
                for item in v_vals {
                    if let Ok(map) = from_value::<rune::runtime::Object>(item) {
                        let title = extract_string(&map, "title").unwrap_or_default();
                        let url = extract_string(&map, "url").unwrap_or_default();
                        let image = extract_opt_string(&map, "image").unwrap_or_default();
                        let preview_url =
                            extract_opt_string(&map, "preview_url").unwrap_or_default();

                        if !url.is_empty() {
                            valid.push(crate::scraper::ScrapedVideo {
                                id: (),
                                title,
                                url,
                                image,
                                local_image: None,
                                preview_url,
                                local_preview: None,
                                finished_videos: 0,
                                failed_videos: 0,
                                related_id: None,
                            });
                        }
                    }
                }
                valid
            },
        );

        if grid_boxes.is_empty() {
            tracing::warn!("WARNING: No grid boxes extracted from Rune!");
        } else {
            tracing::info!("Extracted {} grid boxes from Rune", grid_boxes.len());
        }

        let video_sources = extract_list(&obj, "video_sources").map_or_else(
            |_| Vec::new(),
            |v_vals| {
                let mut valid = Vec::new();
                for item in v_vals {
                    // In Rune, it's a Map/Object
                    if let Ok(map) = from_value::<rune::runtime::Object>(item) {
                        let url = extract_string(&map, "url").unwrap_or_default();
                        let res = map
                            .get("resolution")
                            .map_or(0, |v| from_value::<i64>(v.clone()).unwrap_or(0));
                        if !url.is_empty() {
                            valid.push((url, res));
                        }
                    }
                }
                valid
            },
        );

        Ok(Self {
            title,
            url,
            image,
            preview_url,
            models,
            featuring,
            studio,
            grid_boxes,
            video_sources,
        })
    }

    #[must_use]
    pub fn into_page_data(self) -> crate::scraper::PageData {
        crate::scraper::PageData {
            url: self.url,
            title: Some(self.title),
            image: self.image,
            preview_url: self.preview_url,
            video_sources: self
                .video_sources
                .into_iter()
                .map(|(url, resolution)| crate::scraper::VideoSource {
                    url,
                    resolution,
                    duration: None,
                })
                .collect(),
            models: self.models,
            featuring: self.featuring,
            studio: self.studio,
            grid_boxes: self.grid_boxes,
        }
    }
}

#[derive(Debug, Default)]
#[allow(clippy::struct_excessive_bools)]
pub struct VideoProcessResult {
    pub page_id: i64,
    pub cover_downloaded: bool,
    pub thumb_downloaded: bool,
    pub preview_downloaded: bool,
    pub video_downloaded: bool,
}

pub type WorkflowProgressCallback = crate::model_workflow::WorkflowProgressCallback;

/// Scrape a video page (XV or PV) and save to DB.
///
/// # Errors
///
/// Returns an error if the database operation, download, or scrape processing fails.
#[allow(clippy::too_many_lines)]
pub async fn scrape_and_save_video(
    db: &Database,
    url: &str,
    config: &WorkflowConfig,
    on_progress: Option<&WorkflowProgressCallback>,
) -> Result<VideoProcessResult> {
    // 1. Scrape
    if let Some(cb) = on_progress {
        cb("fetch", &format!("Running scraper for {url}"));
    }

    let page_data = if url.contains("xvideos.com") {
        crate::scraper::xv::fetch_and_parse_xv(url, &config.ffprobe_path).await?
    } else {
        // Rune Script for PV
        let script_path = config.scrapers_dir.join("pv.rn");

        let script_source = tokio::fs::read_to_string(&script_path)
            .await
            .context(format!(
                "Failed to read pv.rn from {}",
                script_path.display()
            ))?;

        let (tx, rx) = mpsc::channel::<ProgressUpdate>();

        // Spawn a thread to forward progress messages to the callback
        let progress_forwarder = crate::scripting::spawn_progress_forwarder(on_progress, rx);

        let url_string = url.to_string();
        // Run the scraper & parse result in the blocking thread
        let rune_result = tokio::task::spawn_blocking(move || {
            crate::scripting::run_scraper_fn(
                &script_source,
                "scrape",
                (url_string,),
                Some(tx),
                |val| RuneVideoResult::from_rune(val).context("Failed to deserialize Rune result"),
            )
        })
        .await??;

        // Drop the sender to close the progress channel
        if let Some(handle) = progress_forwarder {
            let _ = handle.join();
        }

        rune_result.into_page_data()
    };

    if let Some(cb) = on_progress {
        cb(
            "save",
            &format!("Saving {}", page_data.title.as_deref().unwrap_or("video")),
        );
    }

    // 2. Store Page (Robust persistence including video sources)
    let video_sources_clone: Vec<(String, i64)> = page_data
        .video_sources
        .iter()
        .map(|vs| (vs.url.clone(), vs.resolution))
        .collect();

    let page_id = db.store_page(&page_data).await?;

    // 4. Download Assets (Images and Previews)
    #[allow(clippy::cast_precision_loss)]
    let dl_config = DownloadConfig {
        timeout_seconds: 60,
        max_retries: 3,
        interval_seconds: (config.download_delay_ms as f64) / 1000.0,
    };

    let cover_status = crate::model_workflow::download_page_cover(
        db,
        &config.covers_dir,
        page_id,
        page_data.image.as_deref(),
        &dl_config,
    )
    .await?;

    let thumb_status = crate::model_workflow::download_page_thumb(
        db,
        &config.thumbs_dir,
        &config.covers_dir,
        page_id,
        page_data.image.as_deref(), // Use same image for thumb for now if separate not provided
        &dl_config,
    )
    .await?;

    let preview_status = crate::model_workflow::download_page_preview(
        db,
        &config.previews_dir,
        page_id,
        page_data.preview_url.as_deref(),
        &dl_config,
    )
    .await?;

    // 5. Download Video
    let video_progress_cb = on_progress.map(|cb| {
        let cb = std::sync::Arc::clone(cb);
        std::sync::Arc::new(move |json: serde_json::Value| {
            let msg = json.get("message").and_then(|v| v.as_str()).unwrap_or("");
            let stage = json
                .get("stage")
                .and_then(|v| v.as_str())
                .unwrap_or("download");
            cb(stage, msg);
        }) as crate::downloader::ProgressCallback
    });

    let video_download_result = crate::downloader::tasks::download_video_workflow(
        std::sync::Arc::new(db.clone()),
        page_id,
        &db.config,
        video_progress_cb,
        video_sources_clone,
    )
    .await;

    // 6. Download grid assets (thumbnails and previews for related videos)
    if let Ok(Some(page)) = db.get_page(page_id).await
        && !page.grid.is_empty()
    {
        if let Err(e) = download_thumbs_for_grid(
            std::sync::Arc::new(db.clone()),
            &db.config,
            &page.grid,
            on_progress.map(|cb| {
                let cb = std::sync::Arc::clone(cb);
                std::sync::Arc::new(move |json: serde_json::Value| {
                    let msg = json.get("message").and_then(|v| v.as_str()).unwrap_or("");
                    let stage = json
                        .get("stage")
                        .and_then(|v| v.as_str())
                        .unwrap_or("thumb");
                    cb(stage, msg);
                }) as crate::downloader::ProgressCallback
            }),
        )
        .await
        {
            tracing::error!("Thumbnail download failed for grid in {url}: {e}");
        }

        if let Err(e) = download_previews_for_grid(
            std::sync::Arc::new(db.clone()),
            &db.config,
            &page.grid,
            on_progress.map(|cb| {
                let cb = std::sync::Arc::clone(cb);
                std::sync::Arc::new(move |json: serde_json::Value| {
                    let msg = json.get("message").and_then(|v| v.as_str()).unwrap_or("");
                    let stage = json
                        .get("stage")
                        .and_then(|v| v.as_str())
                        .unwrap_or("preview");
                    cb(stage, msg);
                }) as crate::downloader::ProgressCallback
            }),
        )
        .await
        {
            tracing::error!("Preview download failed for grid in {url}: {e}");
        }

        if let Err(e) =
            generate_fallback_thumbs(std::sync::Arc::new(db.clone()), &db.config, &page.grid).await
        {
            tracing::error!("Failed to generate fallback thumbs for grid in {url}: {e}");
        }
    }

    Ok(VideoProcessResult {
        page_id,
        cover_downloaded: cover_status.is_ok(),
        thumb_downloaded: thumb_status.is_ok(),
        preview_downloaded: preview_status.is_ok(),
        video_downloaded: video_download_result.is_ok(),
    })
}

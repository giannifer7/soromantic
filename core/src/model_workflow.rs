//! Model scraper workflow - orchestrates scraping and downloading for model pages.
//!
//! This module replicates the functionality of `scraper_workflow.ml` in Rust,
//! coordinating the Rune scraper with database operations and downloads.

use crate::db::Database;
use crate::downloader::{self, DownloadConfig, DownloadResult, get_file_extension};
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// Configuration for the model scraper workflow.
#[derive(Debug, Clone)]
pub struct WorkflowConfig {
    pub models_dir: PathBuf,
    pub flags_dir: PathBuf,
    pub covers_dir: PathBuf,
    pub thumbs_dir: PathBuf,
    pub previews_dir: PathBuf,
    pub scrapers_dir: PathBuf,
    pub download_delay_ms: u64,
    pub ffmpeg_path: PathBuf,
    pub ffprobe_path: PathBuf,
}

/// Result of processing a single model URL.
#[derive(Debug, Default)]
pub struct ProcessResult {
    pub model_id: i64,
    pub flag_id: Option<i64>,
    pub pages_added: usize,
    pub covers_downloaded: usize,
    pub thumbs_downloaded: usize,
    pub previews_downloaded: usize,
    pub hero_downloaded: bool,
    pub flag_downloaded: bool,
}

use serde::Deserialize;

/// Model info extracted from scraping.
#[derive(Debug, Clone, Deserialize)]
pub struct ModelInfo {
    pub name: String,
    pub url: String,
    pub hero_image: Option<String>,
    pub flag_code: Option<String>,
    pub nationality: Option<String>,
    pub birth_year: Option<i32>,
    pub aliases: Option<String>,
}

/// Scene data extracted from scraping.
#[derive(Debug, Clone, Deserialize)]
pub struct SceneData {
    pub id: String,
    pub url: String,
    pub thumb: Option<String>,
    pub preview: Option<String>,
    pub caption: String,
}

/// Result of scraping a model page.
#[derive(Debug, Deserialize)]
pub struct ScrapeResult {
    pub info: Option<ModelInfo>,
    pub scenes: Vec<SceneData>,
}

/// Pad an ID to 6 digits for use as filename.
#[must_use]
pub fn pad_id(id: i64) -> String {
    format!("{id:0width$}", width = crate::constants::ui::PAD_WIDTH)
}

/// Asset type for download workflow
#[derive(Debug, Clone, Copy)]
enum AssetType {
    Cover,
    Thumb,
    Preview,
}

/// Generic helper to download an asset (cover, thumb, preview)
async fn process_asset_download(
    db: &Database,
    page_id: i64,
    url: Option<&str>,
    dest_dir: &Path,
    config: &DownloadConfig,
    asset_type: AssetType,
) -> Result<DownloadStatus> {
    let Some(url) = url else {
        return Ok(DownloadStatus::NoUrl);
    };

    let ext = get_file_extension(url).unwrap_or_else(|| match asset_type {
        AssetType::Preview => "mp4".to_string(),
        _ => "jpg".to_string(),
    });

    let filename = format!("{}.{}", pad_id(page_id), ext);
    let dest_path = dest_dir.join(&filename);

    if dest_path.exists() {
        update_asset_status(db, page_id, asset_type).await?;
        return Ok(DownloadStatus::FileExists);
    }

    let client = create_client()?;

    match downloader::download_file_robust(&client, url, &dest_path, config, None, None).await? {
        DownloadResult::Success(_) => {
            update_asset_status(db, page_id, asset_type).await?;
            Ok(DownloadStatus::Downloaded)
        }
        DownloadResult::AlreadyExists(_) => Ok(DownloadStatus::AlreadyDone),
        DownloadResult::Failed(msg) => Ok(DownloadStatus::Failed(msg)),
    }
}

async fn update_asset_status(db: &Database, page_id: i64, asset_type: AssetType) -> Result<()> {
    match asset_type {
        AssetType::Cover | AssetType::Thumb => {
            let _ = db
                .set_page_thumb_status(page_id, crate::constants::status::DONE)
                .await;
        }
        AssetType::Preview => {
            let _ = db
                .set_page_preview_status(page_id, crate::constants::status::DONE)
                .await;
        }
    }
    Ok(())
}

/// Create HTTP client with standard headers.
fn create_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        .default_headers({
            let mut headers = reqwest::header::HeaderMap::new();
            headers.insert(
                "Cookie",
                #[allow(clippy::expect_used)]
                "pissvidscookie=1; AGREE=1"
                    .parse()
                    .expect("Static cookie string should parse"),
            );
            headers
        })
        .build()
        .context("Failed to create HTTP client")
}

/// Download a flag icon if it doesn't exist.
///
/// # Errors
///
/// Returns an error if the database operation or download fails.
///
/// Returns the `flag_id` from the database.
pub async fn ensure_flag(
    db: &Database,
    flag_code: &str,
    flags_dir: &Path,
    config: &DownloadConfig,
) -> Result<Option<i64>> {
    let flag_id = db.upsert_nation(flag_code, None).await?;

    let filename = format!("{}.png", pad_id(flag_id));
    let dest_path = flags_dir.join(&filename);

    if !dest_path.exists() {
        let remote_url = format!("https://pissvids.com/assets/img/flags/{flag_code}.png");
        let client = create_client()?;

        match downloader::download_file_robust(&client, &remote_url, &dest_path, config, None, None)
            .await?
        {
            DownloadResult::Success(_) | DownloadResult::AlreadyExists(_) => {
                tracing::info!("Downloaded flag: {}", dest_path.display());
            }
            DownloadResult::Failed(msg) => {
                tracing::warn!("Failed to download flag {flag_code}: {msg}");
            }
        }
    }

    Ok(Some(flag_id))
}

/// Download a model hero image if it doesn't exist.
///
/// # Errors
///
/// Returns an error if the download fails.
pub async fn download_hero_image(
    model_id: i64,
    hero_url: &str,
    models_dir: &Path,
    config: &DownloadConfig,
) -> Result<bool> {
    let ext = get_file_extension(hero_url).unwrap_or_else(|| "jpg".to_string());
    let filename = format!("{}.{}", pad_id(model_id), ext);
    let dest_path = models_dir.join(&filename);

    if dest_path.exists() {
        return Ok(true);
    }

    let client = create_client()?;

    match downloader::download_file_robust(&client, hero_url, &dest_path, config, None, None)
        .await?
    {
        DownloadResult::Success(_) => {
            tracing::info!("Downloaded hero: {}", dest_path.display());
            Ok(true)
        }
        DownloadResult::AlreadyExists(_) => Ok(true),
        DownloadResult::Failed(msg) => {
            tracing::warn!("Failed to download hero image: {msg}");
            Ok(false)
        }
    }
}

/// Status of an individual download attempt.
#[derive(Debug)]
pub enum DownloadStatus {
    Downloaded,
    AlreadyDone,
    FileExists,
    NoUrl,
    Failed(String),
}

impl DownloadStatus {
    #[must_use]
    pub const fn is_ok(&self) -> bool {
        matches!(
            self,
            Self::Downloaded | Self::AlreadyDone | Self::FileExists
        )
    }
}

/// Download a page cover if status is not done.
///
/// # Errors
///
/// Returns an error if the database operation or download fails.
pub async fn download_page_cover(
    db: &Database,
    covers_dir: &Path,
    page_id: i64,
    cover_url: Option<&str>,
    config: &DownloadConfig,
) -> Result<DownloadStatus> {
    // Clean URL (remove query params for cover download)
    let clean_url = cover_url.map(|u| u.split('?').next().unwrap_or(u));

    process_asset_download(db, page_id, clean_url, covers_dir, config, AssetType::Cover).await
}

/// Download a page thumbnail if status is not done.
///
/// # Errors
///
/// Returns an error if the database operation or download fails.
pub async fn download_page_thumb(
    db: &Database,
    thumbs_dir: &Path,
    _covers_dir: &Path,
    page_id: i64,
    thumb_url: Option<&str>,
    config: &DownloadConfig,
) -> Result<DownloadStatus> {
    let scaled_url = thumb_url.map(scale_poster_url);

    process_asset_download(
        db,
        page_id,
        scaled_url.as_deref(),
        thumbs_dir,
        config,
        AssetType::Thumb,
    )
    .await
}

/// Scale a poster URL to thumbnail size by rewriting query params.
fn scale_poster_url(url: &str) -> String {
    url::Url::parse(url).map_or_else(
        |_| url.to_string(),
        |mut parsed| {
            parsed.query_pairs_mut().clear();
            parsed
                .query_pairs_mut()
                .append_pair("method", "resize")
                .append_pair("w", "432")
                .append_pair("height", "244");
            parsed.to_string()
        },
    )
}

/// Download a page preview if it's a video preview.
///
/// # Errors
///
/// Returns an error if the database operation or download fails.
pub async fn download_page_preview(
    db: &Database,
    previews_dir: &Path,
    page_id: i64,
    preview_url: Option<&str>,
    config: &DownloadConfig,
) -> Result<DownloadStatus> {
    process_asset_download(
        db,
        page_id,
        preview_url,
        previews_dir,
        config,
        AssetType::Preview,
    )
    .await
}

/// Progress callback for workflow operations.
pub type WorkflowProgressCallback = crate::scripting::WorkflowProgressCallback;

/// Process a list of scenes, upserting them and downloading assets.
///
/// # Errors
///
/// Returns an error if any database operation or download fails.
pub async fn process_scenes(
    db: &Database,
    model_id: i64,
    scenes: &[SceneData],
    config: &WorkflowConfig,
    download_config: &DownloadConfig,
    on_progress: Option<&WorkflowProgressCallback>,
) -> Result<(usize, usize, usize, usize)> {
    let total = scenes.len();
    let mut pages_added = 0;
    let mut covers_ok = 0;
    let mut thumbs_ok = 0;
    let mut previews_ok = 0;

    for (idx, scene) in scenes.iter().enumerate() {
        if let Some(cb) = on_progress {
            cb(
                "scene",
                &format!("[{}/{}] Processing {}", idx + 1, total, scene.caption),
            );
        }

        // Upsert page
        let page_id = match db.upsert_page(&scene.url, &scene.caption).await {
            Ok(id) => id,
            Err(e) => {
                tracing::warn!("Failed to upsert page {}: {}", scene.url, e);
                continue;
            }
        };

        // Link to model
        let _ = db.link_cast(page_id, model_id, 1).await;
        pages_added += 1;

        // Download cover
        let cover_status = download_page_cover(
            db,
            &config.covers_dir,
            page_id,
            scene.thumb.as_deref(),
            download_config,
        )
        .await?;
        if cover_status.is_ok() {
            covers_ok += 1;
        }

        // Download thumb
        let thumb_status = download_page_thumb(
            db,
            &config.thumbs_dir,
            &config.covers_dir,
            page_id,
            scene.thumb.as_deref(),
            download_config,
        )
        .await?;
        if thumb_status.is_ok() {
            thumbs_ok += 1;
        }

        // Download preview
        let preview_status = download_page_preview(
            db,
            &config.previews_dir,
            page_id,
            scene.preview.as_deref(),
            download_config,
        )
        .await?;
        if preview_status.is_ok() {
            previews_ok += 1;
        }
    }

    Ok((pages_added, covers_ok, thumbs_ok, previews_ok))
}

use crate::scripting::ProgressUpdate;
use rune::runtime::Value;
use std::sync::mpsc;

#[derive(Debug)]
struct RuneModelResult {
    info: Option<ModelInfo>,
    scenes: Vec<SceneData>,
}

use crate::scripting::glue::{extract_opt_int, extract_opt_string, extract_string, into_object};

impl RuneModelResult {
    fn from_rune(value: Value) -> Result<Self> {
        let obj = into_object(value)?;

        let info = match obj.get_value::<_, Value>("info").into_result()? {
            Some(v) => {
                // Try as Option<Value>
                match rune::from_value::<Option<Value>>(v.clone()) {
                    Ok(Some(inner)) => Some(ModelInfo::from_rune(inner)?),
                    Ok(None) => None,
                    Err(_) => Some(ModelInfo::from_rune(v)?),
                }
            }
            None => None,
        };

        // Use helper for list
        let scenes = match crate::scripting::glue::extract_list(&obj, "scenes") {
            Ok(vec_vals) => {
                let mut valid_scenes = Vec::new();
                for item in vec_vals {
                    valid_scenes.push(SceneData::from_rune(item)?);
                }
                valid_scenes
            }
            Err(_) => Vec::new(),
        };

        Ok(Self { info, scenes })
    }
}

impl ModelInfo {
    fn from_rune(value: Value) -> Result<Self> {
        let obj = into_object(value)?;

        Ok(Self {
            name: extract_string(&obj, "name")?,
            url: extract_string(&obj, "url")?,
            hero_image: extract_opt_string(&obj, "hero_image")?,
            flag_code: extract_opt_string(&obj, "flag_code")?,
            nationality: extract_opt_string(&obj, "nationality")?,
            birth_year: extract_opt_int(&obj, "birth_year")?.map(|i| i32::try_from(i).unwrap_or(0)),
            aliases: extract_opt_string(&obj, "aliases")?,
        })
    }
}

impl SceneData {
    fn from_rune(value: Value) -> Result<Self> {
        let obj = into_object(value)?;

        Ok(Self {
            id: extract_string(&obj, "id")?,
            url: extract_string(&obj, "url")?,
            thumb: extract_opt_string(&obj, "thumb")?,
            preview: extract_opt_string(&obj, "preview")?,
            caption: extract_string(&obj, "caption")?,
        })
    }
}

/// Main entry point: scrape and save a model URL using Rune.
///
/// This function:
/// 1. Runs the Rune scraper
/// 2. Upserts model and flag to database
/// 3. Downloads hero image
/// 4. Processes all scenes (upsert + download assets)
///
/// # Errors
///
/// Returns an error if any database operation, download, or scrape processing fails.
pub async fn scrape_and_save_model(
    db: &Database,
    url: &str,
    config: &WorkflowConfig,
    on_progress: Option<&WorkflowProgressCallback>,
) -> Result<ProcessResult> {
    #[allow(clippy::cast_precision_loss)]
    let download_config = DownloadConfig {
        timeout_seconds: crate::config::DEFAULT_DOWNLOAD_TIMEOUT_SECS,
        max_retries: crate::config::DEFAULT_MAX_RETRIES,
        interval_seconds: (config.download_delay_ms as f64) / 1000.0,
    };

    if let Some(cb) = on_progress {
        cb("fetch", &format!("Running scraper for {url}"));
    }

    // 1. Run Rune Script
    let script_source = tokio::fs::read_to_string("assets/scrapers/pv_model.rn")
        .await
        .context("Failed to read pv_model.rn")?;

    let (tx, rx) = mpsc::channel::<ProgressUpdate>();

    // Spawn a thread to forward progress messages to the callback
    // Spawn a thread to forward progress messages to the callback
    let progress_forwarder = crate::scripting::spawn_progress_forwarder(on_progress, rx);

    // Run the scraper & parse result in the blocking thread (Value is not Send)
    let url_string = url.to_string();
    let scrape_result =
        crate::scripting::execute_scraper_blocking(script_source, url_string, tx, |val| {
            RuneModelResult::from_rune(val).context("Failed to deserialize Rune result")
        })
        .await?;

    // Drop the sender to close the progress channel
    if let Some(handle) = progress_forwarder {
        let _ = handle.join();
    }

    let info = scrape_result
        .info
        .context("No model info found in scrape result")?;

    if let Some(cb) = on_progress {
        cb("model", &format!("Processing model: {}", info.name));
    }

    // 2. Upsert flag if present
    let flag_id = if let Some(ref code) = info.flag_code {
        ensure_flag(db, code, &config.flags_dir, &download_config).await?
    } else {
        None
    };
    let flag_downloaded = flag_id.is_some();

    // 3. Upsert model
    // Note: The Rune scraper returns `url` in info, which might be slightly different from input `url`
    // (e.g. if redirected or normalized). We use the one from info.
    let model_id = db
        .upsert_performer(
            &info.name,
            flag_id,
            info.birth_year,
            info.aliases.as_deref(),
            None, // sex
        )
        .await?;

    // 4. Download hero image
    let hero_downloaded = if let Some(ref hero_url) = info.hero_image {
        download_hero_image(model_id, hero_url, &config.models_dir, &download_config).await?
    } else {
        false
    };

    // 5. Process all scenes
    let (pages_added, covers_downloaded, thumbs_downloaded, previews_downloaded) = process_scenes(
        db,
        model_id,
        &scrape_result.scenes,
        config,
        &download_config,
        on_progress,
    )
    .await?;

    Ok(ProcessResult {
        model_id,
        flag_id,
        pages_added,
        covers_downloaded,
        thumbs_downloaded,
        previews_downloaded,
        hero_downloaded,
        flag_downloaded,
    })
}

//! Core file download functionality with retries and error handling.
//!
//! Port of Python `download/core.py` - provides robust file downloading
//! with atomic writes, retry logic, and progress tracking.

use anyhow::{Context, Result};
use futures_util::StreamExt;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

/// Progress callback receiving (`current_bytes`, `total_bytes`).
pub type DownloadProgressCallback = Arc<dyn Fn(u64, u64) + Send + Sync>;

/// Configuration for download operations.
#[derive(Debug, Clone)]
pub struct DownloadConfig {
    /// HTTP request timeout in seconds.
    pub timeout_seconds: u64,
    /// Maximum retry attempts for failed downloads.
    pub max_retries: u32,
    /// Delay between downloads to avoid rate limiting.
    pub interval_seconds: f64,
}

impl Default for DownloadConfig {
    fn default() -> Self {
        Self {
            timeout_seconds: 60,
            max_retries: 3,
            interval_seconds: 0.0,
        }
    }
}

/// Result of a download attempt.
#[derive(Debug)]
pub enum DownloadResult {
    /// Download completed successfully.
    Success(PathBuf),
    /// File already exists (skipped).
    AlreadyExists(PathBuf),
    /// Download failed with error message.
    Failed(String),
}

/// Download a file from URL with retries, atomic writes, and progress tracking.
///
/// Features:
/// - Atomic writes using `.part` file
/// - Exponential backoff retry on 429/5xx errors
/// - Proper handling of 403/404 errors (no retry)
/// - Progress callback support
/// - Database tracking of download status
/// # Errors
/// Returns error if the request fails, or if there is a fatal file I/O error.
pub async fn download_file_robust(
    client: &reqwest::Client,
    url: &str,
    dest_path: &Path,
    config: &DownloadConfig,
    referer: Option<&str>,
    on_progress: Option<&DownloadProgressCallback>,
) -> Result<DownloadResult> {
    // 1. Check if file already exists and is valid
    if dest_path.exists() {
        let meta = std::fs::metadata(dest_path)?;
        if meta.len() > 0 {
            tracing::debug!("File already exists: {:?}", dest_path);
            return Ok(DownloadResult::AlreadyExists(dest_path.to_path_buf()));
        }

        // Empty file - remove and re-download
        tracing::warn!("Found empty file, re-downloading: {:?}", dest_path);
        std::fs::remove_file(dest_path)?;
    }

    // 2. Ensure parent directory exists
    if let Some(parent) = dest_path.parent() {
        std::fs::create_dir_all(parent).context("Failed to create download directory")?;
    }

    // 3. Apply download interval if configured
    if config.interval_seconds > 0.0 {
        tokio::time::sleep(Duration::from_secs_f64(config.interval_seconds)).await;
    }

    tracing::info!("Downloading {} -> {:?}", url, dest_path);

    // 4. Attempt download with retries
    let mut last_error = String::new();

    for attempt in 0..config.max_retries {
        match try_download(client, url, dest_path, config, referer, on_progress).await {
            Ok(path) => {
                tracing::info!("Downloaded: {:?}", path);
                return Ok(DownloadResult::Success(path));
            }
            Err(DownloadError::Forbidden) => {
                return Ok(DownloadResult::Failed(format!("Forbidden (403): {url}")));
            }
            Err(DownloadError::NotFound) => {
                return Ok(DownloadResult::Failed(format!("Not found (404): {url}")));
            }
            Err(DownloadError::Retryable(msg)) => {
                last_error = msg.clone();
                tracing::warn!("{} (attempt {}/{})", msg, attempt + 1, config.max_retries);
                if attempt < config.max_retries - 1 {
                    // Exponential backoff
                    let delay = Duration::from_secs(2u64.pow(attempt));
                    tokio::time::sleep(delay).await;
                }
            }
            Err(DownloadError::Fatal(msg)) => {
                return Ok(DownloadResult::Failed(msg));
            }
        }
    }

    // Max retries exceeded
    let error_msg = if last_error.is_empty() {
        format!("Max retries exceeded for {url}")
    } else {
        last_error
    };
    Ok(DownloadResult::Failed(error_msg))
}

/// Download error classification for retry logic.
enum DownloadError {
    /// 403 Forbidden - don't retry.
    Forbidden,
    /// 404 Not Found - don't retry.
    NotFound,
    /// Retryable error (429, 5xx, timeout, connection error).
    Retryable(String),
    /// Fatal error (file I/O, etc.) - don't retry.
    Fatal(String),
}

/// Attempt a single download.
async fn try_download(
    client: &reqwest::Client,
    url: &str,
    dest_path: &Path,
    config: &DownloadConfig,
    referer: Option<&str>,
    on_progress: Option<&DownloadProgressCallback>,
) -> std::result::Result<PathBuf, DownloadError> {
    // Build request
    let mut request = client.get(url);

    if let Some(ref_url) = referer {
        request = request.header("Referer", ref_url);
    }

    // Send request with timeout
    let response = request
        .timeout(Duration::from_secs(config.timeout_seconds))
        .send()
        .await
        .map_err(|e| {
            if e.is_timeout() {
                DownloadError::Retryable(format!("Timeout downloading {url}"))
            } else if e.is_connect() {
                DownloadError::Retryable(format!("Connection error for {url}: {e}"))
            } else {
                DownloadError::Retryable(format!("Request error for {url}: {e}"))
            }
        })?;

    // Handle HTTP status codes
    let status = response.status();
    match status.as_u16() {
        200 => {}
        403 => return Err(DownloadError::Forbidden),
        404 => return Err(DownloadError::NotFound),
        429 | 500..=599 => {
            return Err(DownloadError::Retryable(format!("HTTP {status} for {url}")));
        }
        _ => {
            return Err(DownloadError::Fatal(format!(
                "Unexpected HTTP {status} for {url}"
            )));
        }
    }

    // Write to part file
    let part_path = dest_path.with_extension(dest_path.extension().map_or_else(
        || "part".to_string(),
        |e| format!("{}.part", e.to_string_lossy()),
    ));

    let total_size = response.content_length().unwrap_or(0);
    let mut stream = response.bytes_stream();

    let mut file = File::create(&part_path).map_err(|e| {
        DownloadError::Fatal(format!(
            "Failed to create file {}: {e}",
            part_path.display()
        ))
    })?;

    let mut downloaded: u64 = 0;
    while let Some(item) = stream.next().await {
        let chunk =
            item.map_err(|e| DownloadError::Retryable(format!("Stream error for {url}: {e}")))?;

        file.write_all(&chunk).map_err(|e| {
            DownloadError::Fatal(format!("Write error for {}: {e}", part_path.display()))
        })?;

        downloaded += chunk.len() as u64;

        if let Some(cb) = on_progress {
            cb(downloaded, total_size);
        }
    }

    // Rename part file to final destination (atomic)
    std::fs::rename(&part_path, dest_path).map_err(|e| {
        DownloadError::Fatal(format!(
            "Failed to rename {} to {}: {e}",
            part_path.display(),
            dest_path.display()
        ))
    })?;

    Ok(dest_path.to_path_buf())
}

/// Simple download without database tracking (for backwards compatibility).
///
/// # Errors
/// Returns error if the request fails, or if there is a file I/O error.
pub async fn download_file_simple(
    url: &str,
    dest: &Path,
    on_progress: Option<&DownloadProgressCallback>,
) -> Result<()> {
    let timeout = Duration::from_mins(1);
    let client = reqwest::Client::builder().timeout(timeout).build()?;

    // Ensure directory exists
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).context("Failed to create download directory")?;
    }

    let response = client.get(url).send().await?;

    if !response.status().is_success() {
        anyhow::bail!("Download failed with status: {}", response.status());
    }

    let total_size = response.content_length().unwrap_or(0);
    let mut stream = response.bytes_stream();

    // Use part file for atomic write
    let part_path = dest.with_extension("part");
    let mut file = File::create(&part_path).context("Failed to create file")?;

    let mut downloaded: u64 = 0;
    while let Some(item) = stream.next().await {
        let chunk = item?;
        file.write_all(&chunk)?;
        downloaded += chunk.len() as u64;

        if let Some(cb) = on_progress {
            cb(downloaded, total_size);
        }
    }

    // Atomic rename
    std::fs::rename(&part_path, dest)?;

    Ok(())
}

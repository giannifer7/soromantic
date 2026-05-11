//! Web scraping modules for video sites.
//!
//! This module provides a unified [`PageData`] type that all scrapers return.

pub mod pv;
pub mod xv;

use anyhow::{Context, Result};
use std::path::Path;

use crate::types::VideoEntry;

/// Unified parsed page data returned by all scrapers.
#[derive(Debug, Clone, Default)]
pub struct PageData {
    pub url: String,
    pub title: Option<String>,
    pub image: Option<String>,
    pub preview_url: Option<String>,
    pub video_sources: Vec<VideoSource>,
    pub models: Vec<(String, String)>,    // (name, url)
    pub featuring: Vec<(String, String)>, // (name, url)
    pub studio: Option<(String, String)>, // (name, url)
    pub grid_boxes: Vec<ScrapedVideo>,
}

pub type ScrapedVideo = VideoEntry<()>;

/// Video source with URL and resolution.
#[derive(Debug, Clone)]
pub struct VideoSource {
    pub url: String,
    pub resolution: i64,
    pub duration: Option<f64>, // Duration in seconds
}

pub const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

/// Fetch page text content with standard user agent.
///
/// # Errors
/// Returns error if network request fails or status is not success.
pub async fn fetch_page_text(url: &str) -> Result<String> {
    let client = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .build()
        .context("Failed to create HTTP client")?;

    let response = client
        .get(url)
        .header("Cookie", "pissvidscookie=1")
        .send()
        .await
        .context("Failed to fetch page")?;

    if !response.status().is_success() {
        anyhow::bail!("HTTP error: {status}", status = response.status());
    }

    response
        .text()
        .await
        .context("Failed to read response body")
}

/// Probe duration for a list of video sources using `ffprobe`.
/// Updates the `duration` field of each source in-place.
pub async fn probe_sources_duration(sources: &mut [VideoSource], ffprobe_path: &Path) {
    for source in sources {
        // Probe duration (returns seconds)
        match crate::downloader::ffmpeg::probe_duration(ffprobe_path, &source.url).await {
            Ok(duration_sec) => {
                source.duration = Some(duration_sec);
            }
            Err(e) => {
                // Log warning but don't fail the whole scrape
                eprintln!("Failed to probe duration for {url}: {e}", url = source.url);
            }
        }
    }
}

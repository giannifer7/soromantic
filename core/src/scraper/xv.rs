//! XV video site scraper.
//!
//! Extracts video metadata, sources, and related videos from XV pages.

use anyhow::{Context, Result};
use regex::Regex;
use scraper::{Html, Selector};
use serde::Deserialize;
use tracing; // Added for logging

// Re-export shared types
pub use super::{PageData, ScrapedVideo, VideoSource};

/// Parsed JSON-LD `VideoObject` data from page.
#[derive(Debug, Clone, Default)]
pub struct JsonLdData {
    pub name: Option<String>,
    pub thumbnail_url: Option<String>,
    pub content_url: Option<String>,
    pub duration: Option<String>, // ISO 8601, e.g. "PT00H07M24S"
}

/// Video URLs extracted from `HTML5Player` script.
#[derive(Debug, Clone, Default)]
pub struct Html5PlayerUrls {
    pub url_low: Option<String>,  // 3gp/low quality
    pub url_high: Option<String>, // mp4/high quality
    pub url_hls: Option<String>,  // HLS m3u8 playlist
}

/// A single HLS stream variant.
#[derive(Debug, Clone)]
pub struct HlsStream {
    pub resolution: u32, // Height in pixels (e.g., 480)
    pub url: String,     // URL to the stream's m3u8
}

// --- JSON-LD extraction ---

#[derive(Deserialize)]
struct JsonLdRaw {
    name: Option<String>,
    #[serde(rename = "thumbnailUrl")]
    thumbnail_url: Option<serde_json::Value>, // Can be string or array
    #[serde(rename = "contentUrl")]
    content_url: Option<String>,
    duration: Option<String>,
}

/// Extract `VideoObject` data from JSON-LD script tag.
///
/// # Errors
/// Returns error if script tag is missing or JSON parsing fails.
pub fn extract_json_ld(html: &Html) -> Result<JsonLdData> {
    let selector = Selector::parse(r#"script[type="application/ld+json"]"#)
        .map_err(|e| anyhow::anyhow!("Selector error: {e:?}"))?;

    let script = html
        .select(&selector)
        .next()
        .context("No JSON-LD script found")?;

    let text = script.text().collect::<String>();
    let raw: JsonLdRaw = serde_json::from_str(&text).context("JSON-LD parse error")?;

    // Handle thumbnailUrl which can be a list
    let thumbnail_url = match raw.thumbnail_url {
        Some(serde_json::Value::String(s)) => {
            if s.contains(".mp4") || s.contains(".webm") {
                None
            } else {
                Some(s)
            }
        }
        Some(serde_json::Value::Array(arr)) => arr.first().and_then(|v| {
            v.as_str()
                .filter(|s| !s.contains(".mp4") && !s.contains(".webm"))
                .map(String::from)
        }),
        _ => None,
    };

    Ok(JsonLdData {
        name: raw.name,
        thumbnail_url,
        content_url: raw.content_url,
        duration: raw.duration,
    })
}

// --- HTML5 Player URL extraction ---

/// Extract video URLs from `HTML5Player` JavaScript.
#[must_use]
pub fn extract_html5player_urls(html_content: &str) -> Html5PlayerUrls {
    fn extract_url(content: &str, method: &str) -> Option<String> {
        let pattern = format!(r"html5player\.{method}\('([^']+)'\)");
        Regex::new(&pattern)
            .ok()
            .and_then(|re| re.captures(content))
            .and_then(|caps| caps.get(1))
            .map(|m| m.as_str().to_string())
    }

    Html5PlayerUrls {
        url_low: extract_url(html_content, "setVideoUrlLow"),
        url_high: extract_url(html_content, "setVideoUrlHigh"),
        url_hls: extract_url(html_content, "setVideoHLS"),
    }
}

// --- Related videos extraction ---

/// Extract `video_related` array from inline JavaScript.
///
/// # Errors
/// Returns error if `video_related` variable is missing or malformed.
pub fn extract_video_related(html_content: &str) -> Result<Vec<serde_json::Value>> {
    let re = Regex::new(r"var\s+video_related\s*=\s*(\[[^\]]*\])\s*;")?;

    let caps = re
        .captures(html_content)
        .context("video_related not found")?;
    let js_array = caps.get(1).context("No capture group")?.as_str();

    // Try parsing directly
    if let Ok(arr) = serde_json::from_str::<Vec<serde_json::Value>>(js_array) {
        return Ok(arr);
    }

    // Clean up trailing commas and retry
    let cleaned = Regex::new(r",\s*([}\]])")?.replace_all(js_array, "$1");
    serde_json::from_str(&cleaned).context("Failed to parse video_related")
}

// --- Uploader and models extraction ---

/// Return type for `extract_uploader_and_models`:
/// where uploader is (name, url) and models is Vec<(name, url)>.
pub type UploaderModels = (Option<(String, String)>, Vec<(String, String)>);

/// Extract uploader and models from `.video-tags-list`.
#[must_use]
pub fn extract_uploader_and_models(html: &Html) -> UploaderModels {
    let mut uploader: Option<(String, String)> = None;
    let mut models: Vec<(String, String)> = Vec::new();

    let tags_selector = Selector::parse(".video-tags-list").ok();
    let uploader_selector = Selector::parse("li.main-uploader a").ok();
    let name_selector = Selector::parse(".name").ok();
    let model_selector = Selector::parse("li.model a").ok();

    let (Some(tags_sel), Some(up_sel), Some(name_sel), Some(model_sel)) = (
        tags_selector,
        uploader_selector,
        name_selector,
        model_selector,
    ) else {
        return (uploader, models);
    };

    let Some(tags_list) = html.select(&tags_sel).next() else {
        return (uploader, models);
    };

    // Helper closure to extract name and URL
    let extract = |el: scraper::ElementRef| -> Option<(String, String)> {
        let href = el.value().attr("href")?;
        let name_node = el.select(&name_sel).next()?;
        let name = name_node.text().collect::<String>().trim().to_string();
        let url = format!("https://www.xvideos.com{href}");
        Some((name, url))
    };

    // Extract uploader
    if let Some(uploader_el) = tags_list.select(&up_sel).next() {
        uploader = extract(uploader_el);
    }

    // Extract models
    for model_el in tags_list.select(&model_sel) {
        if let Some(data) = extract(model_el) {
            models.push(data);
        }
    }

    (uploader, models)
}

// --- HLS parsing ---

/// Parse HLS master m3u8 to extract available streams.
pub fn parse_hls_master(m3u8_content: &str, base_url: &str) -> Vec<HlsStream> {
    tracing::debug!("Parsing HLS master playlist from base URL: {base_url}");
    tracing::trace!("HLS manifest content:\n{}", m3u8_content);

    let mut streams = Vec::new();
    let re = Regex::new(r"RESOLUTION=\d+x(\d+)").ok();

    let lines: Vec<&str> = m3u8_content.lines().collect();

    // Iterate over pairs of lines (metadata + URL)
    for window in lines.windows(2) {
        let (info_line, url_line) = (window[0], window[1]);

        if !info_line.starts_with("#EXT-X-STREAM-INF:") {
            continue;
        }

        // Parse resolution using regex
        // Logic: Regex must match -> capture group 1 must exist -> parsing to u32 must succeed
        let Some(resolution) = re
            .as_ref()
            .and_then(|r| r.captures(info_line))
            .and_then(|c| c.get(1))
            .and_then(|m| m.as_str().parse::<u32>().ok())
        else {
            tracing::warn!("Failed to parse resolution from line: {info_line}");
            continue;
        };

        let stream_url = url_line.trim();
        let full_url = if stream_url.starts_with("http") {
            stream_url.to_string()
        } else {
            // Make URL absolute
            let base = base_url.rsplit_once('/').map_or(base_url, |(b, _)| b);
            format!("{base}/{stream_url}")
        };

        tracing::debug!(
            "Found HLS variant: {resolution}p -> {full_url} (from relative: {stream_url})"
        );

        streams.push(HlsStream {
            resolution,
            url: full_url,
        });
    }

    // Sort by resolution descending
    streams.sort_by_key(|b| std::cmp::Reverse(b.resolution));

    tracing::info!(
        "Parsed {} HLS variants: [{}]",
        streams.len(),
        streams
            .iter()
            .map(|s| format!("{p}p", p = s.resolution))
            .collect::<Vec<_>>()
            .join(", ")
    );

    streams
}

// --- Duration parsing ---

/// Parse ISO 8601 duration to seconds. e.g. "PT00H07M24S" -> 444
#[must_use]
pub fn parse_duration(iso_duration: &str) -> Option<u32> {
    let re = Regex::new(r"PT(?:(\d+)H)?(?:(\d+)M)?(?:(\d+)S)?").ok()?;
    let caps = re.captures(iso_duration)?;

    let hours: u32 = caps
        .get(1)
        .and_then(|m| m.as_str().parse().ok())
        .unwrap_or(0);
    let minutes: u32 = caps
        .get(2)
        .and_then(|m| m.as_str().parse().ok())
        .unwrap_or(0);
    let seconds: u32 = caps
        .get(3)
        .and_then(|m| m.as_str().parse().ok())
        .unwrap_or(0);

    Some(hours * 3600 + minutes * 60 + seconds)
}

// --- Grid boxes ---

/// Build grid boxes from related videos JSON array.
#[must_use]
pub fn build_grid_boxes(related: &[serde_json::Value]) -> Vec<super::ScrapedVideo> {
    related
        .iter()
        .filter_map(|item| {
            let obj = item.as_object()?;

            // Make URL absolute (XV uses relative URLs like /video.xxx/title)
            let rel_url = obj.get("u").and_then(|v| v.as_str()).unwrap_or("");
            let abs_url = if rel_url.starts_with('/') {
                format!("https://www.xvideos.com{rel_url}")
            } else {
                rel_url.to_string()
            };

            // Extract title (tf = full title, t = truncated)
            let raw_title = obj
                .get("tf")
                .or_else(|| obj.get("t"))
                .and_then(|v| v.as_str());

            if abs_url.is_empty() {
                return None;
            }

            Some(super::ScrapedVideo {
                id: (),
                title: raw_title
                    .map(|s| html_escape::decode_html_entities(s).to_string())
                    .unwrap_or_default(),
                url: abs_url,
                image: obj
                    .get("il")
                    .or_else(|| obj.get("i"))
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty() && !s.contains(".mp4") && !s.contains(".webm"))
                    .map(String::from),
                preview_url: obj.get("ipu").and_then(|v| v.as_str()).map(String::from),
                ..Default::default()
            })
        })
        .collect()
}

// --- Video sources ---

/// Extract resolution from URL filename (e.g., "`video_360p.mp4`" -> 360, "hls-480p.m3u8" -> 480).
fn parse_resolution_from_url(url: &str) -> Option<i64> {
    // First try numeric pattern like "360p", "720p"
    if let Ok(re) = Regex::new(r"(\d+)p")
        && let Some(caps) = re.captures(url)
        && let Some(m) = caps.get(1)
        && let Ok(res) = m.as_str().parse::<i64>()
    {
        return Some(res);
    }

    // Fall back to quality labels
    let url_lower = url.to_lowercase();
    if url_lower.contains("_sd") || url_lower.contains("mp4_sd") {
        return Some(360); // SD = Standard Definition = 360p
    }
    if url_lower.contains("video_low") || url_lower.contains("_low") {
        return Some(240); // Low quality
    }
    if url_lower.contains("_hd") || url_lower.contains("mp4_hd") {
        return Some(720); // HD = 720p
    }

    None
}

#[must_use]
pub fn build_video_sources(player_urls: &Html5PlayerUrls) -> Vec<VideoSource> {
    let mut sources = Vec::new();

    // Helper to add source with parsed or fallback resolution
    let add_source = |sources: &mut Vec<VideoSource>, url: String, fallback_res: i64| {
        let resolution = parse_resolution_from_url(&url).unwrap_or(fallback_res);
        tracing::debug!(
            "Adding initial source: {url} -> {resolution}p (fallback: {fallback_res}p)"
        );
        sources.push(VideoSource {
            url,
            resolution,
            duration: None,
        });
    };

    // Parse actual resolutions from URLs instead of hardcoding
    if let Some(url) = player_urls.url_high.clone() {
        add_source(&mut sources, url, 720);
    }
    if let Some(url) = player_urls.url_hls.clone() {
        add_source(&mut sources, url, 480);
    }
    if let Some(url) = player_urls.url_low.clone() {
        add_source(&mut sources, url, 360);
    }

    sources
}

// --- Main parser ---

/// Parse XV page HTML into structured `PageData`.
///
/// # Errors
/// Returns error if parsing fails.
pub fn parse_page_xv(html_content: &str, base_url: &str) -> Result<PageData> {
    let html = Html::parse_document(html_content);

    // Extract JSON-LD data
    let json_ld = extract_json_ld(&html).unwrap_or_default();

    // Extract related videos
    let related = extract_video_related(html_content).unwrap_or_default();
    let grid_boxes = build_grid_boxes(&related);

    // Extract video sources
    let player_urls = extract_html5player_urls(html_content);
    let video_sources = build_video_sources(&player_urls);

    // Extract uploader and models
    let (uploader, models) = extract_uploader_and_models(&html);

    Ok(PageData {
        url: base_url.to_string(),
        title: json_ld
            .name
            .map(|s| html_escape::decode_html_entities(&s).to_string()),
        image: json_ld.thumbnail_url,
        preview_url: None,
        video_sources,
        models,
        featuring: Vec::new(),
        studio: uploader,
        grid_boxes,
    })
}

// --- Async fetch + parse ---

/// Fetch and parse an XV page.
///
/// This is the main entry point for scraping - it handles HTTP fetch and parsing.
///
/// # Errors
/// Returns error if network request fails or parsing fails.
#[allow(clippy::too_many_lines)]
pub async fn fetch_and_parse_xv(url: &str, ffprobe_path: &std::path::Path) -> Result<PageData> {
    let client = reqwest::Client::builder()
        .user_agent(super::USER_AGENT)
        .build()
        .context("Failed to create HTTP client")?;

    let html_content = super::fetch_page_text(url).await?;

    let mut data = parse_page_xv(&html_content, url)?;

    // Expand HLS sources
    // We collect new sources to replace the original list
    tracing::debug!(
        "Initial sources before HLS expansion: {sources:?}",
        sources = data.video_sources
    );
    let mut final_sources = Vec::new();

    for source in data.video_sources.drain(..) {
        if source.url.contains(".m3u8") {
            // Fetch HLS manifest
            tracing::info!(
                "Fetching HLS manifest: {url} (initial resolution label: {res}p)",
                url = source.url,
                res = source.resolution
            );
            match client.get(&source.url).send().await {
                Ok(resp) => {
                    if resp.status().is_success() {
                        if let Ok(m3u8_text) = resp.text().await {
                            let variants = parse_hls_master(&m3u8_text, &source.url);
                            if variants.is_empty() {
                                // No variants found? Keep original (maybe it's a direct stream or parsing failed)
                                tracing::warn!(
                                    "No variants parsed from HLS manifest, keeping original"
                                );
                                final_sources.push(source);
                            } else {
                                tracing::info!(
                                    "Found {count} variants in HLS, replacing initial {res}p source",
                                    count = variants.len(),
                                    res = source.resolution
                                );
                                for variant in variants {
                                    tracing::debug!(
                                        "Adding variant to final sources: {res}p -> {url}",
                                        res = variant.resolution,
                                        url = variant.url
                                    );
                                    final_sources.push(VideoSource {
                                        url: variant.url.clone(),
                                        resolution: i64::from(variant.resolution),
                                        duration: None, // Will be probed below
                                    });
                                }
                            }
                        } else {
                            tracing::warn!("Failed to read HLS manifest text");
                            final_sources.push(source);
                        }
                    } else {
                        tracing::warn!(
                            "Failed to fetch HLS manifest: {status}",
                            status = resp.status()
                        );
                        final_sources.push(source);
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to request HLS manifest: {e}");
                    final_sources.push(source);
                }
            }
        } else {
            final_sources.push(source);
        }
    }
    data.video_sources = final_sources;

    tracing::info!(
        "Final sources after HLS expansion: [{}]",
        data.video_sources
            .iter()
            .map(|s| format!("{p}p", p = s.resolution))
            .collect::<Vec<_>>()
            .join(", ")
    );

    // Probe duration for video sources
    super::probe_sources_duration(&mut data.video_sources, ffprobe_path).await;

    Ok(data)
}

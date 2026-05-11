//! PV (pissvids/analvids) video site scraper.
//!
//! Extracts video metadata, sources, and related videos from PV pages.

use anyhow::Result;
use scraper::{Html, Selector};

// Re-export shared types
pub use super::{PageData, ScrapedVideo, VideoSource};

/// Type alias for models/featuring tuple lists.
type ModelsFeats = (Vec<(String, String)>, Vec<(String, String)>);

// --- Helper functions ---

use crate::utils::norm_url;

/// Extract metadata from a `.card-scene` div.
fn extract_scene(
    scene: &scraper::ElementRef,
    base_url: &str,
) -> Result<Option<super::ScrapedVideo>> {
    let text_a_sel = Selector::parse(".card-scene__text a")
        .map_err(|e| anyhow::anyhow!("Invalid selector: {e:?}"))?;
    let view_a_sel = Selector::parse(".card-scene__view a")
        .map_err(|e| anyhow::anyhow!("Invalid selector: {e:?}"))?;
    let img_sel = Selector::parse(".card-scene__view a img")
        .map_err(|e| anyhow::anyhow!("Invalid selector: {e:?}"))?;

    let text_a = scene.select(&text_a_sel).next();
    let view_a = scene.select(&view_a_sel).next();
    let img = scene.select(&img_sel).next();

    let image = img
        .and_then(|e| {
            e.value()
                .attr("data-src")
                .or_else(|| e.value().attr("src"))
                .filter(|s| !s.contains(".mp4") && !s.contains(".webm") && !s.starts_with("data:"))
        })
        .map(|s| s.replace("&amp;", "&"))
        .map(|s| norm_url(base_url, &s))
        .filter(|s| !s.is_empty());

    // Fallback: If thumb is missing or base64, check for casting data
    // (Casting logic removed in favor of downloader fallback)
    // We kept the thumb variable as Option<String> above.

    let preview_url = view_a
        .and_then(|e| e.value().attr("data-preview"))
        .map(|s| s.replace("&amp;", "&"))
        .map(|s| norm_url(base_url, &s))
        .filter(|s| !s.is_empty());

    let url_opt = text_a
        .and_then(|e| e.value().attr("href"))
        .map(|s| norm_url(base_url, s))
        .filter(|s| !s.is_empty());

    let Some(url) = url_opt else {
        return Ok(None);
    };

    let title = text_a
        .map(|e| e.text().collect::<String>().trim().to_string())
        .map(|s| html_escape::decode_html_entities(&s).to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_default();

    Ok(Some(super::ScrapedVideo {
        id: (),
        title,
        url,
        image,
        preview_url,
        ..Default::default()
    }))
}

/// Extract page title from HTML.
/// Removes the site trailer (e.g. " - Pissvids.com") from the end.
/// Extract page title from HTML.
/// Removes the site trailer (e.g. " - Pissvids.com") from the end.
fn get_title(html: &Html, title_trailer: &str) -> Result<Option<String>> {
    let title_sel =
        Selector::parse("title").map_err(|e| anyhow::anyhow!("Invalid selector: {e:?}"))?;
    let Some(el) = html.select(&title_sel).next() else {
        return Ok(None);
    };
    let raw_title = el.text().collect::<String>().trim().to_string();
    if raw_title.is_empty() {
        return Ok(None);
    }
    // Strip trailer from end
    let title = if raw_title.ends_with(title_trailer) {
        raw_title[..raw_title.len() - title_trailer.len()].to_string()
    } else {
        raw_title
    };
    Ok(Some(html_escape::decode_html_entities(&title).to_string()))
}

/// Extract model and featuring links from `.watch__title`.
/// Extract model and featuring links from `.watch__title`.
fn get_text_with_links(html: &Html) -> Result<ModelsFeats> {
    let mut models = Vec::new();
    let feats = Vec::new(); // PV doesn't seem to have "featuring" separately

    let watch_title_sel =
        Selector::parse(".watch__title").map_err(|e| anyhow::anyhow!("Invalid selector: {e:?}"))?;
    let a_sel = Selector::parse("a").map_err(|e| anyhow::anyhow!("Invalid selector: {e:?}"))?;

    if let Some(watch_title) = html.select(&watch_title_sel).next() {
        for a in watch_title.select(&a_sel) {
            if let Some(href) = a.value().attr("href") {
                let name = a.text().collect::<String>().trim().to_string();
                if !href.is_empty() && !name.is_empty() {
                    models.push((name, href.to_string()));
                }
            }
        }
    }

    Ok((models, feats))
}

/// Extract studio information from `.genres-list`.
/// Extract studio information from `.genres-list`.
fn get_studios(html: &Html) -> Result<Option<(String, String)>> {
    let genres_sel = Selector::parse("div.genres-list")
        .map_err(|e| anyhow::anyhow!("Invalid selector: {e:?}"))?;
    let a_sel = Selector::parse("a").map_err(|e| anyhow::anyhow!("Invalid selector: {e:?}"))?;

    let Some(genres_div) = html.select(&genres_sel).next() else {
        return Ok(None);
    };
    let Some(a) = genres_div.select(&a_sel).next() else {
        return Ok(None);
    };
    let Some(url) = a.value().attr("href").map(|u| u.trim().to_string()) else {
        return Ok(None);
    };
    let name = a.text().collect::<String>().trim().to_string();

    if url.is_empty() || name.is_empty() {
        return Ok(None);
    }
    Ok(Some((name, url)))
}

/// Extract video poster and sources from `<video>` tag.
/// Extract video poster and sources from `<video>` tag.
fn get_video_data(html: &Html) -> Result<(Option<String>, Vec<VideoSource>)> {
    let video_sel =
        Selector::parse("video").map_err(|e| anyhow::anyhow!("Invalid selector: {e:?}"))?;
    let source_sel =
        Selector::parse("source").map_err(|e| anyhow::anyhow!("Invalid selector: {e:?}"))?;

    let mut image = None;
    let mut sources = Vec::new();

    if let Some(video) = html.select(&video_sel).next() {
        // Get poster
        if let Some(poster) = video.value().attr("data-poster")
            && !poster.is_empty()
        {
            image = Some(poster.to_string());
        }

        // Get sources
        for source in video.select(&source_sel) {
            let src = source.value().attr("src");
            let size = source.value().attr("size");

            if let (Some(src), Some(size)) = (src, size)
                && let Ok(mut res) = size.parse::<i64>()
            {
                // Map 576p to 480p (non-standard resolution)
                if res == crate::constants::media::RESOLUTION_NON_STD_576 {
                    res = crate::constants::media::RESOLUTION_STD_480;
                }
                sources.push(VideoSource {
                    url: src.to_string(),
                    resolution: res,
                    duration: None,
                });
            }
        }
    }

    sources.sort_by_key(|s| s.resolution);
    Ok((image, sources))
}

/// Extract grid boxes from all `.card-scene` divs.
/// Extract grid boxes from all `.card-scene` divs.
fn extract_all_scenes(html: &Html, base_url: &str) -> Result<Vec<super::ScrapedVideo>> {
    let scene_sel = Selector::parse("div.card-scene")
        .map_err(|e| anyhow::anyhow!("Invalid selector: {e:?}"))?;
    html.select(&scene_sel)
        .map(|scene| extract_scene(&scene, base_url))
        .filter_map(|res| match res {
            Ok(opt) => opt.map(Ok),
            Err(e) => Some(Err(e)),
        })
        .collect()
}

// --- Main parser ---

/// Default title trailer for PV sites.
pub const DEFAULT_TITLE_TRAILER: &str = " - Pissvids.com";

/// Parse PV page HTML into structured `PageData`.
///
/// # Errors
/// Returns error if parsing fails.
pub fn parse_page_pv(html_content: &str, base_url: &str) -> Result<PageData> {
    parse_page_pv_with_trailer(html_content, base_url, DEFAULT_TITLE_TRAILER)
}

/// Parse PV page HTML with custom title trailer.
///
/// # Errors
/// Returns error if parsing fails.
pub fn parse_page_pv_with_trailer(
    html_content: &str,
    base_url: &str,
    title_trailer: &str,
) -> Result<PageData> {
    let html = Html::parse_document(html_content);

    let title = get_title(&html, title_trailer)?;
    let (models, featuring) = get_text_with_links(&html)?;
    let (image, video_sources) = get_video_data(&html)?;
    let studio = get_studios(&html)?;
    let grid_boxes = extract_all_scenes(&html, base_url)?;

    Ok(PageData {
        url: base_url.to_string(),
        title,
        image,
        preview_url: None,
        video_sources,
        models,
        featuring,
        studio,
        grid_boxes,
    })
}

// --- Async fetch + parse ---

/// Fetch and parse a PV page.
///
/// # Errors
/// Returns error if network request fails or parsing fails.
pub async fn fetch_and_parse_pv(url: &str, ffprobe_path: &std::path::Path) -> Result<PageData> {
    fetch_and_parse_pv_with_trailer(url, DEFAULT_TITLE_TRAILER, ffprobe_path).await
}

/// Fetch and parse a PV page with custom title trailer.
///
/// # Errors
/// Returns error if network request fails or parsing fails.
pub async fn fetch_and_parse_pv_with_trailer(
    url: &str,
    title_trailer: &str,
    ffprobe_path: &std::path::Path,
) -> Result<PageData> {
    let html_content = super::fetch_page_text(url).await?;

    let mut data = parse_page_pv_with_trailer(&html_content, url, title_trailer)?;

    // Probe duration for video sources
    super::probe_sources_duration(&mut data.video_sources, ffprobe_path).await;

    Ok(data)
}

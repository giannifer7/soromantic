use anyhow::{Context, Result};
use reqwest::header::{HeaderMap, HeaderValue, COOKIE, USER_AGENT};
use soromantic_core::config::ResolvedConfig;
use soromantic_core::constants::status;
use soromantic_core::db::Database;
use soromantic_core::scraper::pv;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    println!("Starting thumbnail downloader (with page repair)...");

    // 1. Load config
    let config = load_config().await?;
    let db = Arc::new(Database::new(config.clone()).await?);
    println!("Database connected: {:?}", config.db_path);

    // 2. Load all pages (id, url) from DB
    println!("Loading all pages from DB...");
    let pages = get_all_pages(&db).await?;
    println!("Found {} pages in DB", pages.len());

    // 3. Load missing thumbnail IDs set for fast lookup
    let missing_ids = get_missing_thumb_ids(&db).await?;
    println!(
        "Found {} missing thumbnails that need to be found",
        missing_ids.len()
    );

    // 4. Setup HTTP client with headers
    let client = create_client()?;

    let pages_dir = config.download_dir.join("pages");
    if !pages_dir.exists() {
        fs::create_dir_all(&pages_dir).await?;
    }

    println!("Processing pages in {:?}...", pages_dir);

    let mut pages_processed = 0;
    let mut pages_downloaded = 0;
    let mut thumbs_found = 0;
    let mut thumbs_downloaded = 0;
    let mut failures = 0;

    for (id, url) in pages {
        pages_processed += 1;
        if pages_processed % 50 == 0 {
            // println!("Processed {} pages...", pages_processed);
        }

        let base_url_clean = url.trim_end_matches('/');
        let mut page_num = 1;

        loop {
            // Determine URL for this page
            let current_url = if page_num == 1 {
                base_url_clean.to_string()
            } else {
                format!("{}/{}", base_url_clean, page_num)
            };

            // Determine file path for this specific page (M indexed from 0)
            let filename = format!("{:06}-{}.html", id, page_num - 1);
            let path = pages_dir.join(&filename);

            // Check/Download HTML
            let mut html_content = String::new();
            let mut need_parse = true;

            // Check if local file exists
            if path.exists() {
                match fs::read_to_string(&path).await {
                    Ok(c) if !c.is_empty() => html_content = c,
                    _ => {
                        tracing::info!(
                            "Page {} #{} local file invalid, re-downloading...",
                            id,
                            page_num
                        );
                    }
                }
            }

            if html_content.is_empty() {
                match download_page_html(&client, &current_url).await {
                    Ok(content) => {
                        // Save ALL pages
                        if let Err(e) = fs::write(&path, &content).await {
                            tracing::error!("Failed to save page {} #{}: {}", id, page_num, e);
                        }
                        html_content = content;
                        pages_downloaded += 1;
                        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Failed to download page {} #{} ({}): {}",
                            id,
                            page_num,
                            current_url,
                            e
                        );
                        failures += 1;
                        need_parse = false;
                        // If page 1 fails, break. Subsequent pages might exist but usually not if page 1 is gone.
                        if page_num == 1 {
                            break;
                        }
                    }
                }
            }

            // Parse & Extract Thumbs
            if need_parse {
                match process_page_content(&html_content, &missing_ids, &client, &db, &config).await
                {
                    Ok(count) => {
                        thumbs_found += count;
                        thumbs_downloaded += count;
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Failed to process thumbs for page {} #{}: {}",
                            id,
                            page_num,
                            e
                        );
                    }
                }

                // Check for next page
                if has_next_page(&html_content) {
                    page_num += 1;
                } else {
                    break;
                }
            } else {
                break;
            }
        }
    }

    println!("\nProcess complete.");
    println!("Pages processed: {}", pages_processed);
    println!("Pages downloaded: {}", pages_downloaded);
    println!(
        "Thumbs found: {}, Downloaded: {}",
        thumbs_found, thumbs_downloaded
    );
    println!("Failures: {}", failures);

    Ok(())
}

fn has_next_page(html: &str) -> bool {
    let document = scraper::Html::parse_document(html);
    let selector = scraper::Selector::parse("i.bi-chevron-double-right").unwrap();
    document.select(&selector).next().is_some()
}

async fn get_all_pages(db: &Database) -> Result<Vec<(i64, String)>> {
    let pages: Vec<(i64, String)> = sqlx::query_as("SELECT id, url FROM pages_full")
        .fetch_all(&db.pool)
        .await?
        .into_iter()
        .collect();
    Ok(pages)
}

async fn get_missing_thumb_ids(db: &Database) -> Result<HashSet<i64>> {
    let query = format!(
        "SELECT id FROM pages WHERE thumb_status != {}",
        status::DONE
    );
    let ids: HashSet<i64> = sqlx::query(&query)
        .fetch_all(&db.pool)
        .await?
        .into_iter()
        .map(|row| {
            use sqlx::Row;
            row.get::<i64, _>("id")
        })
        .collect();
    Ok(ids)
}

async fn download_page_html(client: &reqwest::Client, url: &str) -> Result<String> {
    let resp = client.get(url).send().await?;
    if !resp.status().is_success() {
        anyhow::bail!("HTTP {}", resp.status());
    }
    let text = resp.text().await?;
    Ok(text)
}

async fn process_page_content(
    content: &str,
    missing_ids: &HashSet<i64>,
    client: &reqwest::Client,
    db: &Database,
    config: &ResolvedConfig,
) -> Result<usize> {
    // Basic heuristics
    if !content.contains("pissvids.com") && !content.contains("Start Parsing") {
        return Ok(0);
    }

    let base_url = "https://pissvids.com";
    let page_data = match pv::parse_page_pv(content, base_url) {
        Ok(data) => data,
        Err(_) => return Ok(0),
    };

    let mut downloaded_count = 0;

    for item in page_data.grid_boxes {
        if let Some(id) = extract_id_from_url(&item.url) {
            if missing_ids.contains(&id) {
                if let Some(img_url) = item.image {
                    if !img_url.is_empty()
                        && download_thumb(id, &img_url, client, config).await.is_ok()
                    {
                        match db.set_page_thumb_status(id, status::DONE).await {
                            Ok(_) => {
                                println!("Downloaded thumb for page {}: {}", id, img_url);
                                downloaded_count += 1;
                            }
                            Err(e) => tracing::error!("Failed to update DB for {}: {}", id, e),
                        }
                    }
                }
            }
        }
    }
    Ok(downloaded_count)
}

fn extract_id_from_url(url: &str) -> Option<i64> {
    if let Some(start) = url.find("/watch/") {
        let remainder = &url[start + 7..];
        let id_str: String = remainder
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect();
        if !id_str.is_empty() {
            return id_str.parse::<i64>().ok();
        }
    }
    None
}

async fn download_thumb(
    page_id: i64,
    url: &str,
    client: &reqwest::Client,
    config: &ResolvedConfig,
) -> Result<()> {
    let dest = soromantic_core::downloader::paths::get_download_path(
        config,
        page_id,
        Some(url),
        soromantic_core::downloader::paths::FileType::Thumb,
        None,
    );

    if dest.exists() && std::fs::metadata(&dest)?.len() > 0 {
        return Ok(());
    }

    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).await?;
    }

    let resp = client.get(url).send().await?;
    if !resp.status().is_success() {
        anyhow::bail!("HTTP {}", resp.status());
    }

    let bytes = resp.bytes().await?;
    fs::write(&dest, bytes).await?;

    let dest_clone = dest.clone();
    tokio::task::spawn_blocking(move || {
        if let Err(e) = soromantic_core::downloader::image_processing::detect_and_crop(&dest_clone)
        {
            tracing::warn!("Failed to auto-crop {}: {}", dest_clone.display(), e);
        }
    })
    .await?;

    Ok(())
}

fn create_client() -> Result<reqwest::Client> {
    let mut headers = HeaderMap::new();
    headers.insert(
        USER_AGENT,
        HeaderValue::from_static("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
    );
    headers.insert(
        COOKIE,
        HeaderValue::from_static("pissvidscookie=1; AGREE=1; kt_rt_popunder=1"),
    );

    reqwest::Client::builder()
        .default_headers(headers)
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .context("Failed to build HTTP client")
}

async fn load_config() -> Result<ResolvedConfig> {
    let config_path = PathBuf::from("config.toml");
    let path_str = config_path.to_str();
    use soromantic_core::config::ConfigStatus;
    match soromantic_core::config::load_config(path_str)? {
        ConfigStatus::Loaded(config) => Ok(*config),
        ConfigStatus::Created(path) => {
            anyhow::bail!(
                "Created new config at {:?}, please configure it first.",
                path
            );
        }
    }
}

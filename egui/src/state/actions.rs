use super::MyApp;
use eframe::egui;
use soromantic_core::db::LibraryItem;

impl MyApp {
    /// Trigger batch scrape from configured file path (native Rust)
    pub fn trigger_batch_scrape(&mut self) {
        let file_path = self.batch_list_path.clone();
        tracing::info!("Triggering batch scrape from config path: {:?}", file_path);

        self.scrape.show_scrape_window = false;

        // Read URLs from file
        let urls = match soromantic_core::config::load_batch_list(&file_path) {
            Ok(urls) => urls,
            Err(e) => {
                tracing::error!("Failed to read/parse batch file: {}", e);
                return;
            }
        };

        if urls.is_empty() {
            tracing::warn!("No URLs found in batch file");
            return;
        }

        tracing::info!("Enqueueing {} URLs from batch file", urls.len());
        self.trigger_scrape_urls(urls);
    }

    /// Trigger repair to download missing assets (native Rust)
    pub fn trigger_repair(&mut self, ctx: &egui::Context) {
        if self.scrape.repair_triggered {
            return;
        }
        self.scrape.repair_triggered = true;

        let db = self.db.clone();
        let batch_manager = self.scrape.batch_manager.clone();
        let ctx = ctx.clone();

        tracing::info!("Triggering native Rust repair to download missing assets...");

        // For now, just refresh the library - full repair would re-download missing files
        // This is a placeholder; full implementation would iterate pages and call download workflows
        self.rt_handle.spawn(async move {
            // Just check how many items exist - using small batch for count
            if let Ok((all_items, _)) = db.get_library_paginated(0, 10000, true).await {
                let count = all_items.len();
                tracing::info!("Repair: found {} library items", count);
                // In a full implementation, we'd check each item's thumbnails/videos
                // and enqueue downloads for missing ones using batch_manager
                let _ = batch_manager; // Suppress unused warning
            }
            ctx.request_repaint();
        });
    }

    /// Trigger scrape for URL(s) - supports multiple URLs (one per line)
    pub fn trigger_scrape(&mut self) {
        let input = self.scrape.scrape_url.clone();

        let mut urls = Vec::new();

        for line in input.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            let path = std::path::Path::new(line);
            if path.exists() && path.is_file() {
                match soromantic_core::config::load_batch_list(path) {
                    Ok(batch_urls) => {
                        tracing::info!(
                            "Loaded {} URLs from batch file: {}",
                            batch_urls.len(),
                            line
                        );
                        urls.extend(batch_urls);
                    }
                    Err(e) => {
                        tracing::error!("Failed to load batch file {}: {}", line, e);
                    }
                }
            } else {
                urls.push(line.to_string());
            }
        }

        if urls.is_empty() {
            return;
        }

        self.scrape.show_scrape_window = false;
        self.scrape.scrape_url.clear();

        self.trigger_scrape_urls(urls);
    }

    /// Trigger batch scrape for multiple URLs
    pub fn trigger_scrape_urls(&mut self, urls: Vec<String>) {
        if urls.is_empty() {
            return;
        }

        let count = urls.len();
        self.scrape.scrape_status = format!("Queued {count} URLs...");

        if let Ok(mut mgr) = self.scrape.batch_manager.lock() {
            for url in urls {
                mgr.enqueue(url);
            }
            mgr.start(); // Ensure worker is running
        } else {
            tracing::error!("Failed to lock batch manager");
        }
    }

    /// Play videos for selected items
    pub fn play_items(&self, ids: Vec<i64>, _context_items: &[LibraryItem]) {
        tracing::info!("DEBUG: play_items called with {} ids", ids.len());
        if ids.is_empty() {
            return;
        }

        let db = self.db.clone();
        let mpv = self.mpv.clone();
        self.rt_handle.spawn(async move {
            match db.get_playlist(&ids).await {
                Ok(playlist) => {
                    if playlist.is_empty() {
                        tracing::warn!("No videos found for selection");
                    } else {
                        tracing::info!("Playing {} items", playlist.len());
                        // Probe runtime
                        let _ = tokio::runtime::Handle::current();
                        tracing::info!("Runtime handle check passed");

                        if let Err(e) = mpv.play_playlist(&playlist) {
                            tracing::error!("Failed to play playlist: {}", e);
                        }
                    }
                }
                Err(e) => tracing::error!("Failed to generate playlist: {}", e),
            }
        });
    }
}

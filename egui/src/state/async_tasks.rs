use super::MyApp;
use crate::data::{PendingImage, PreviewFrames};
use eframe::egui;

use std::path::PathBuf;

impl MyApp {
    /// Request async image loading in background thread
    pub fn request_image_load(&mut self, _ctx: &egui::Context, id: i64, path: PathBuf) {
        if self.images.loading_ids.contains(&id) {
            return;
        }
        self.images.loading_ids.insert(id);
        tracing::debug!("Requesting image load for id {}: {:?}", id, path);

        let pending = self.images.pending_images.clone();

        let thumb_w = self.ui_config.thumbnail_width;
        let thumb_h = self.ui_config.thumbnail_height;

        // Use Tokio's blocking thread pool for IO tasks
        self.rt_handle.spawn_blocking(move || {
            match soromantic_core::images::load_thumbnail(&path, thumb_w, thumb_h) {
                Ok((w, h, pixels)) => {
                    pending.lock().push(PendingImage {
                        id,
                        image: egui::ColorImage::from_rgba_unmultiplied(
                            [
                                usize::try_from(w).unwrap_or(0),
                                usize::try_from(h).unwrap_or(0),
                            ],
                            &pixels,
                        ),
                        is_preview: false,
                    });
                }
                Err(e) => {
                    tracing::error!("Failed to load image {:?}: {}", path, e);
                    // Send a 1x1 error placeholder to clear the loading state
                    // Use a subtle red or transparent pixel
                    let fallback = egui::ColorImage::from_rgba_unmultiplied(
                        [1, 1],
                        &[255, 0, 0, 255], // Red
                    );
                    pending.lock().push(PendingImage {
                        id,
                        image: fallback,
                        is_preview: false,
                    });
                }
            }
        });
    }

    /// Load remaining library items in background after initial fast startup
    /// Called once on first frame - fetches full library from DB asynchronously
    /// Also writes cache file for instant startup on next run
    /// Load just the library count (fast startup)
    pub fn load_library_count(&mut self, _ctx: &egui::Context) {
        if self.grid.library_fully_loaded {
            return;
        }
        self.grid.library_fully_loaded = true;

        let db = self.db.clone();
        let pending = self.grid.pending_library_data.clone();

        // We only need the count, so limit=0 is fine (get_library_paginated returns (items, count))
        // But get_library_paginated uses the limit for the items query.
        // If we pass 0, we get 0 items and the correct total count.

        self.rt_handle.spawn(async move {
            // Fetch just the count (limit 0)
            // skip_count = false because we WANT the count
            match db.get_library_paginated(0, 0, false).await {
                Ok((_, total_count)) => {
                    tracing::info!("Library count loaded: {}", total_count);
                    *pending.lock() = Some((Vec::new(), total_count, 0));
                }
                Err(e) => {
                    tracing::error!("Failed to load library count: {}", e);
                }
            }
        });
    }

    /// Load a window of items centered on a specific item index (Optimized: sends to background worker)
    pub fn load_window(&mut self, _ctx: &egui::Context, center_item_index: usize) {
        let skip_count = !self.grid.library_dirty;
        if let Err(e) = self.grid.pagination_tx.send((center_item_index, skip_count)) {
            tracing::error!("Failed to send pagination request: {}", e);
        }
    }

    /// Process pending images from background thread.
    ///
    /// Uploads thumbnails first (cheap per-texture), then preview frames,
    /// up to `texture_upload_limit` per frame to avoid GPU stutter.
    /// No sorting is needed — we do two linear passes over the pending queue.
    pub fn process_pending_images(&mut self, ctx: &egui::Context) {
        let mut guard = self.images.pending_images.lock();
        let len = guard.len();
        if len == 0 {
            return;
        }

        let limit = self.images.texture_upload_limit.min(len);
        let mut pending = Vec::with_capacity(limit);

        // Pass 1: drain thumbnails (is_preview = false) first — they are lighter
        guard.retain(|img| {
            if pending.len() >= limit || img.is_preview {
                true // keep
            } else {
                pending.push(img.clone());
                false // remove
            }
        });

        // Pass 2: fill remaining slots with preview frames
        if pending.len() < limit {
            guard.retain(|img| {
                if pending.len() >= limit {
                    true // keep
                } else {
                    pending.push(img.clone());
                    false // remove
                }
            });
        }

        let remaining = guard.len();
        drop(guard);

        // If we didn't drain everything, ensure we repaint next frame
        if remaining > 0 {
            ctx.request_repaint();
        }

        for img in pending {
            tracing::debug!("Processing pending image for id {}", img.id);
            // Check for sentinel (completion signal)
            if img.image.width() == 0 {
                if let Some(frames) = self.images.preview_cache.get_mut(&img.id) {
                    frames.ready = true;
                }
                continue;
            }

            let texture = ctx.load_texture(
                if img.is_preview {
                    use std::time::{SystemTime, UNIX_EPOCH};
                    let dur = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_nanos();
                    format!("preview_{}_{}", img.id, dur)
                } else {
                    format!("thumb_{}", img.id)
                },
                img.image,
                egui::TextureOptions::LINEAR,
            );

            if img.is_preview {
                // Add to preview frames
                self.images.preview_cache
                    .entry(img.id)
                    .or_default()
                    .frames
                    .push(texture);
            } else {
                // Actually LoadedImage is imported in mod.rs, but submodules don't inherit imports.
                // But `self.images.textures` is `HashMap<i64, LoadedImage>`, so compiler knows the type.
                // I need to construct `LoadedImage { texture }`.
                // `LoadedImage` is imported in `mod.rs`. Use `crate::data::LoadedImage` to be safe/clear.
                self.images.textures
                    .insert(img.id, crate::data::LoadedImage { texture });
                self.images.loading_ids.remove(&img.id);
            }
        }
    }

    /// Request async preview extraction
    pub fn request_preview(
        &mut self,
        _ctx: &egui::Context,
        id: i64,
        preview_url: String,
        runtime_dir: PathBuf,
    ) {
        if self.images.preview_cache.contains_key(&id) {
            return;
        }

        // Mark as started
        self.images.preview_cache.insert(id, PreviewFrames::default());

        let pending = self.images.pending_images.clone();

        let file_path = PathBuf::from(preview_url); // It's already a local path from the DB query alias

        self.rt_handle.spawn_blocking(move || {
            let frames =
                soromantic_core::previews::ensure_preview_frames(id, &file_path, &runtime_dir);

            match frames {
                Ok(paths) => {
                    for path in paths {
                        if let Ok(img) = image::open(&path) {
                            let rgba = img.to_rgba8();
                            pending.lock().push(PendingImage {
                                id,
                                image: egui::ColorImage::from_rgba_unmultiplied(
                                    [
                                        usize::try_from(rgba.width()).unwrap_or(0),
                                        usize::try_from(rgba.height()).unwrap_or(0),
                                    ],
                                    rgba.as_flat_samples().as_slice(),
                                ),
                                is_preview: true,
                            });
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to generate previews for {}: {}", id, e);
                }
            }

            // Send Sentinel (Ready Signal) - ALWAYS send this
            pending.lock().push(PendingImage {
                id,
                image: egui::ColorImage::new([0, 0], egui::Color32::TRANSPARENT),
                is_preview: true,
            });
        });
    }

    pub fn check_async_results(&mut self) {
        // Check page data
        {
            let mut guard = self.nav.pending_page_data.lock();
            if guard.is_some() {
                self.nav.active_page = guard.take();
            }
        }

        // Check search results
        {
            let mut guard = self.nav.pending_search_results.lock();
            if !guard.is_empty() {
                self.nav.browser_results = std::mem::take(&mut *guard);
            }
        }

        // Check library refresh
        {
            let mut guard = self.grid.pending_library_data.lock();
            if guard.is_some()
                && let Some((new_items, total_count, offset)) = guard.take()
            {
                self.grid.items = new_items;
                self.grid.total_items = self.grid.items.len();
                // Only update count if not skipped (-1)
                if total_count != -1 {
                    self.grid.library_total_count = total_count;
                    self.grid.library_dirty = false;
                }
                self.grid.window_start_offset = offset;

                if offset == 0 {
                    self.update_library_cache(&self.grid.items);
                }

                tracing::info!(
                    "Library refreshed. Window size: {}, Offset: {}, Total in DB: {}",
                    self.grid.total_items,
                    offset,
                    total_count
                );
            }
        }

        // Check models
        {
            let mut guard = self.model_studio.pending_models.lock();
            if guard.is_some() {
                self.model_studio.models = guard.take();
            }
        }

        // Check studios
        {
            let mut guard = self.model_studio.pending_studios.lock();
            if guard.is_some() {
                self.model_studio.studios = guard.take();
            }
        }

        // Check model_studio_items
        {
            let mut guard = self.model_studio.pending_model_studio_items.lock();
            if guard.is_some()
                && let Some((items, count, urls, page)) = guard.take()
            {
                self.model_studio.model_studio_items = Some(items);
                // Only update count if not skipped (-1)
                if count != -1 {
                    self.model_studio.model_studio_total_count = count;
                    self.grid.library_dirty = false; // Details also count as "library" scope for dirty tracking
                }
                self.model_studio.model_studio_urls = urls;
                self.model_studio.loaded_model_studio_page = page; // Sync loaded page with RESULT page
                self.model_studio.is_loading_model_studio = false; // Mark loading as complete
            }
        }
    }

    /// Update the on-disk cache with the first 30 items
    fn update_library_cache(&self, items: &[soromantic_core::db::LibraryItem]) {
        if items.is_empty() {
            return;
        }

        let cache_items: Vec<_> = items.iter().take(30).cloned().collect();
        let cache_dir = self.cache_dir.clone();

        self.rt_handle.spawn_blocking(move || {
            if let Err(e) = soromantic_core::cache::write_library_cache(&cache_dir, &cache_items) {
                tracing::error!("Failed to write library cache: {}", e);
            }
        });
    }
}

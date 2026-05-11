use crate::state::MyApp;
use crate::ui::draw_ui;
use eframe::egui;

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.run_frame(ctx);
    }
}

impl MyApp {
    #[allow(clippy::too_many_lines)]
    pub fn run_frame(&mut self, ctx: &egui::Context) {
        // Measure Time to First Frame — one-time setup
        if !self.first_frame_rendered {
            self.first_frame_rendered = true;
            let ttff = self.startup_time.elapsed();
            eprintln!("Time to First Frame: {ttff:.2?}");
            if std::env::var("RUST_LOG").is_ok() {
                tracing::info!("Time to First Frame: {:.2?}", ttff);
            }

            // Force dark visuals (fixes light-mode flash on some platforms)
            ctx.set_visuals(egui::Visuals::dark());

            // Disable shadows globally — they're expensive and unwanted
            let no_shadow = egui::Shadow {
                offset: egui::vec2(0.0, 0.0),
                blur: 0.0,
                spread: 0.0,
                color: egui::Color32::TRANSPARENT,
            };
            ctx.style_mut(|style| {
                style.visuals.window_shadow = no_shadow;
                style.visuals.popup_shadow = no_shadow;
            });
        }

        self.process_pending_images(ctx);
        self.process_server_events(ctx);

        // Animate scrape progress bar toward target (or pulse if indeterminate)
        let animate = self.animate_scrape_progress(ctx);
        if animate || !self.scrape.active_scrapes.is_empty() {
            ctx.request_repaint();
        }

        // Global Drag & Drop Handler
        // Supports: Multiple files, Direct Text/URL drops
        let mut new_dropped_urls = Vec::new();

        // 1. Handle Dropped Files
        if !ctx.input(|i| i.raw.dropped_files.is_empty()) {
            let dropped_files = ctx.input(|i| i.raw.dropped_files.clone());
            for file in dropped_files {
                if let Some(path) = file.path {
                    tracing::info!("File dropped: {:?}", path);
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        for line in content.lines() {
                            let trimmed = line.trim();
                            if !trimmed.is_empty() && !trimmed.starts_with('#') {
                                new_dropped_urls.push(trimmed.to_string());
                            }
                        }
                    } else {
                        tracing::error!("Failed to read dropped file: {:?}", path);
                    }
                }
            }
        }

        // 2. Handle Dropped Text (Direct URLs)
        // dropped_text is not available in RawInput in this version of egui/eframe.
        // We rely on pasting for text for now. (dropped_files handles file paths)

        // 3. Process accumulated URLs
        if !new_dropped_urls.is_empty() {
            self.open_scrape_window();

            // Append to existing text if any, with newline separator
            let cleaned_input = self.scrape.scrape_url.trim();
            if !cleaned_input.is_empty() {
                self.scrape.scrape_url.push('\n');
            }

            let append_count = new_dropped_urls.len();
            self.scrape.scrape_url.push_str(&new_dropped_urls.join("\n"));

            tracing::info!("Added {} dropped URLs to scrape list", append_count);
        }

        // Global Playback Shortcuts — forward to MPV when no widget has keyboard focus.
        if !ctx.wants_keyboard_input() {
            for event in &ctx.input(|i| i.events.clone()) {
                match event {
                    egui::Event::Text(text) => {
                        if text == "q" {
                            let _ = self.mpv.send_command(r#"{"command": ["stop"]}"#);
                        } else {
                            let cmd = format!(r#"{{"command": ["keypress", "{text}"]}}"#);
                            let _ = self.mpv.send_command(&cmd);
                        }
                    }
                    egui::Event::Key {
                        key,
                        pressed: true,
                        repeat: false,
                        ..
                    } => {
                        match key {
                            egui::Key::ArrowLeft => {
                                let _ = self.mpv.send_command(r#"{"command": ["keypress", "LEFT"]}"#);
                            }
                            egui::Key::ArrowRight => {
                                let _ = self.mpv.send_command(r#"{"command": ["keypress", "RIGHT"]}"#);
                            }
                            egui::Key::ArrowUp => {
                                let _ = self.mpv.send_command(r#"{"command": ["keypress", "UP"]}"#);
                            }
                            egui::Key::ArrowDown => {
                                let _ = self.mpv.send_command(r#"{"command": ["keypress", "DOWN"]}"#);
                            }
                            egui::Key::Escape => {
                                let _ = self.mpv.send_command(r#"{"command": ["set", "fullscreen", "no"]}"#);
                            }
                            egui::Key::Enter => {
                                let _ = self.mpv.send_command(r#"{"command": ["keypress", "ENTER"]}"#);
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
        }

        draw_ui(ctx, self);
    }
}

impl MyApp {
    /// Smoothly animate the scrape progress bar toward its target value.
    /// Returns `true` if a repaint is needed for the next animation step.
    fn animate_scrape_progress(&mut self, ctx: &egui::Context) -> bool {
        let Some(sp) = &mut self.scrape.scrape_progress else {
            return false;
        };

        let dt = ctx.input(|i| i.stable_dt).min(0.1);

        if sp.total > 0 {
            // Determinate: interpolate toward target
            #[allow(clippy::cast_precision_loss)]
            let target = sp.progress as f32 / sp.total as f32;
            let diff = target - sp.displayed_progress;
            if diff.abs() > 0.001 {
                sp.displayed_progress = (diff * dt).mul_add(5.0, sp.displayed_progress);
                return true;
            }
            sp.displayed_progress = target;
        } else {
            // Indeterminate: pulse between 0.1 and 0.9 over ~2 seconds
            #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
            {
                let val = (ctx.input(|i| i.time) * std::f64::consts::PI).sin() as f32;
                sp.displayed_progress = 0.1 + 0.8 * (val + 1.0) / 2.0;
            }
            return true;
        }
        false
    }

    /// Drain all pending server events and dispatch to per-type handlers.
    fn process_server_events(&mut self, ctx: &egui::Context) {
        while let Ok(event) = self.server_events.try_recv() {
            match event {
                crate::server::InternalEvent::BatchEvent(val) => {
                    if let Some(obj) = val.as_object()
                        && let Some(type_str) = obj.get("type").and_then(|v| v.as_str())
                    {
                        match type_str {
                            "scrape_start" => self.on_scrape_start(ctx, obj),
                            "batch_queued" => self.on_batch_queued(obj),
                            "scrape_progress" => self.on_scrape_progress(obj),
                            "grid_page" => self.on_grid_page(ctx, obj),
                            "download_started" => self.on_download_started(),
                            "scrape_complete" => self.on_scrape_complete(ctx, obj),
                            "batch_progress_update" => self.on_batch_progress_update(ctx, obj),
                            "batch_complete" => self.on_batch_complete(ctx),
                            "error" | "scrape_failed" => self.on_scrape_error(ctx, obj),
                            _ => {}
                        }
                        ctx.request_repaint();
                    }
                }
            }
        }
    }

    fn on_scrape_start(&mut self, _ctx: &egui::Context, obj: &serde_json::Map<String, serde_json::Value>) {
        let url = obj.get("url").and_then(|v| v.as_str()).unwrap_or("unknown");
        self.scrape.scrape_status = format!("Scraping started: {url}");
        self.scrape.scrape_title = url.to_string();
        if let Some(page_id) = obj.get("page_id").and_then(serde_json::Value::as_i64) {
            self.scrape.active_scrapes.insert(page_id);
            self.scrape.failed_scrapes.remove(&page_id);
        }
    }

    fn on_batch_queued(&mut self, obj: &serde_json::Map<String, serde_json::Value>) {
        if let (Some(enqueued), Some(_total)) = (
            obj.get("enqueued").and_then(serde_json::Value::as_u64),
            obj.get("total").and_then(serde_json::Value::as_u64),
        ) {
            if let Some(bp) = &mut self.scrape.batch_progress {
                if bp.current < bp.total {
                    bp.total += usize::try_from(enqueued).unwrap_or(0);
                    self.scrape.scrape_status = format!("Batch updated: {}/{}", bp.current, bp.total);
                } else {
                    *bp = crate::state::BatchProgress {
                        current: 0,
                        total: usize::try_from(enqueued).unwrap_or(0),
                    };
                    self.scrape.scrape_status = format!("Batch queued: {enqueued}");
                }
            } else {
                self.scrape.batch_progress = Some(crate::state::BatchProgress {
                    current: 0,
                    total: usize::try_from(enqueued).unwrap_or(0),
                });
                self.scrape.scrape_status = format!("Batch queued: {enqueued}");
            }
        }
    }

    fn on_scrape_progress(&mut self, obj: &serde_json::Map<String, serde_json::Value>) {
        let page_id = obj.get("page_id").and_then(serde_json::Value::as_i64).unwrap_or(0);
        let stage = obj.get("stage").and_then(|v| v.as_str()).unwrap_or("unknown");
        let progress = obj.get("progress").and_then(serde_json::Value::as_u64).unwrap_or(0);
        let total = obj.get("total").and_then(serde_json::Value::as_u64).unwrap_or(0);
        let message = obj.get("message").and_then(|v| v.as_str()).unwrap_or("").to_string();

        let current_displayed = self.scrape.scrape_progress.as_ref().map_or(0.0, |existing| {
            if existing.stage == stage { existing.displayed_progress } else { 0.0 }
        });

        self.scrape.scrape_progress = Some(crate::state::ScrapeProgress {
            page_id, stage: stage.to_string(), progress, total, message: message.clone(), displayed_progress: current_displayed,
        });
        self.scrape.scrape_status = message;
    }

    fn on_grid_page(&mut self, ctx: &egui::Context, obj: &serde_json::Map<String, serde_json::Value>) {
        if let Some(page) = obj.get("page_num").and_then(serde_json::Value::as_u64) {
            self.scrape.scrape_status = format!("Scraping page {page}...");
            if let Some(title) = obj.get("title").and_then(|v| v.as_str()) {
                self.scrape.scrape_title = title.to_string();
            }
            if let Some(page_id) = obj.get("page_id").and_then(serde_json::Value::as_i64) {
                self.scrape.active_scrapes.insert(page_id);
                if self.nav.view_mode == crate::state::ViewMode::Related(page_id) {
                    self.refresh_page(ctx, page_id);
                }
            }
        }
    }

    fn on_download_started(&mut self) {
        self.scrape.scrape_status = "Downloading main video...".to_string();
    }

    fn on_scrape_complete(&mut self, ctx: &egui::Context, obj: &serde_json::Map<String, serde_json::Value>) {
        self.scrape.scrape_status = "Scrape complete!".to_string();
        self.scrape.scrape_progress = None;

        if let Some(bp) = &self.scrape.batch_progress
            && bp.current >= bp.total
            && bp.total > 0
        {
            tracing::info!("Batch scrape complete");
            self.scrape.scrape_status = format!("Batch complete: {} pages scraped", bp.total);
        }

        self.refresh_library(ctx);

        if let Some(page_id) = obj.get("page_id").and_then(serde_json::Value::as_i64) {
            self.scrape.active_scrapes.remove(&page_id);
            self.scrape.failed_scrapes.remove(&page_id);

            let mut refresh_id = None;
            if self.nav.view_mode == crate::state::ViewMode::Related(page_id) {
                refresh_id = Some(page_id);
            } else if let Some(page) = &mut self.nav.active_page
                && let Some(item) = page.grid.iter_mut().find(|i| i.related_id == Some(page_id))
            {
                item.finished_videos = item.finished_videos.max(1);
                refresh_id = Some(page.id);
            }

            if let Some(rid) = refresh_id {
                self.refresh_page(ctx, rid);
            } else {
                tracing::info!("Scrape complete for page {}", page_id);
            }
        } else {
            tracing::warn!("Scrape complete but no page_id found");
        }
    }

    fn on_batch_progress_update(&mut self, ctx: &egui::Context, obj: &serde_json::Map<String, serde_json::Value>) {
        if let (Some(current), Some(total)) = (
            obj.get("current").and_then(serde_json::Value::as_u64),
            obj.get("total").and_then(serde_json::Value::as_u64),
        ) {
            let url = obj.get("url").and_then(|v| v.as_str()).unwrap_or("...");
            let display_url = if url.len() > 40 { format!("...{}", &url[url.len() - 37..]) } else { url.to_string() };

            self.scrape.batch_progress = Some(crate::state::BatchProgress {
                current: usize::try_from(current).unwrap_or(0),
                total: usize::try_from(total).unwrap_or(0),
            });
            self.scrape.scrape_status = format!("Batch {current}/{total}: {display_url}");
            self.refresh_library(ctx);
        }
    }

    fn on_batch_complete(&mut self, ctx: &egui::Context) {
        tracing::info!("Batch processing finished");
        self.scrape.scrape_status = "Batch processing finished".to_string();
        if let Some(bp) = &mut self.scrape.batch_progress {
            bp.current = bp.total;
        }
        self.refresh_library(ctx);
    }

    fn on_scrape_error(&mut self, ctx: &egui::Context, obj: &serde_json::Map<String, serde_json::Value>) {
        if let Some(err) = obj.get("error").and_then(|v| v.as_str()) {
            self.scrape.scrape_status = format!("Error: {err}");
        }
        if let Some(page_id) = obj.get("page_id").and_then(serde_json::Value::as_i64) {
            self.scrape.active_scrapes.remove(&page_id);
            self.scrape.failed_scrapes.insert(page_id);

            if let Some(page) = &self.nav.active_page
                && page.grid.iter().any(|item| item.related_id == Some(page_id))
            {
                self.refresh_page(ctx, page.id);
            }
        }
    }
}

use super::{MyApp, ViewMode};
use eframe::egui;
use soromantic_core::db::LibraryItem;

impl MyApp {
    pub const fn open_scrape_window(&mut self) {
        self.scrape.show_scrape_window = true;
        self.scrape.focus_scrape_input = true;
    }

    pub fn close_scrape_window(&mut self) {
        self.scrape.show_scrape_window = false;
        self.scrape.focus_scrape_input = false;
        self.scrape.scrape_url.clear();
    }

    pub fn set_view_mode(&mut self, mode: ViewMode) {
        if matches!(mode, ViewMode::Library) {
            self.nav.active_page = None;
            self.nav.navigation_stack.clear();
        }
        self.nav.view_mode = mode;
        self.nav.focused_index = None; // Disable auto-focus to avoid stealing focus
        self.grid.current_page = 0; // Reset pagination
    }

    pub fn open_page(&mut self, ctx: &egui::Context, id: i64) {
        // Push current page to stack if valid
        if let Some(page) = &self.nav.active_page {
            self.nav.navigation_stack.push(page.clone());
        }

        self.nav.view_mode = ViewMode::Related(id);
        self.nav.active_page = None;
        self.nav.focused_index = None;
        self.grid.current_page = 0;

        self.refresh_page(ctx, id);
    }

    pub fn back(&mut self, ctx: &egui::Context) {
        if let Some(page) = self.nav.navigation_stack.pop() {
            // Restore previous page from stack
            self.nav.view_mode = ViewMode::Related(page.id);
            self.nav.active_page = Some(page);
            self.nav.focused_index = None;
            self.grid.current_page = 0;
            ctx.request_repaint();
        } else {
            // Stack empty, go to library
            self.set_view_mode(ViewMode::Library);
        }
    }

    pub fn refresh_page(&mut self, ctx: &egui::Context, id: i64) {
        let db = self.db.clone();
        let pending = self.nav.pending_page_data.clone();
        let ctx = ctx.clone();

        self.rt_handle.spawn(async move {
            if let Ok(Some(page)) = db.get_page(id).await {
                *pending.lock() = Some(page);
                ctx.request_repaint();
            }
        });
    }

    pub fn perform_search(&mut self, ctx: &egui::Context, query: String) {
        if query.trim().len() < 2 {
            return;
        }

        let db = self.db.clone();
        let pending = self.nav.pending_search_results.clone();
        let ctx = ctx.clone();
        let limit = self.ui_config.search_limit;

        self.rt_handle.spawn(async move {
            if let Ok(results) = db.search_pages(&query, limit).await {
                *pending.lock() = results;
                ctx.request_repaint();
            }
        });
    }

    pub fn refresh_library(&mut self, ctx: &egui::Context) {
        let db = self.db.clone();
        let pending = self.grid.pending_library_data.clone();
        let ctx = ctx.clone();
        let batch_size = self.ui_config.db_batch_size;

        self.rt_handle.spawn(async move {
            if let Ok((all_items, total_count)) =
                db.get_library_paginated(0, batch_size, false).await
            {
                // Filter to items with finished videos
                let filtered: Vec<LibraryItem> = all_items
                    .into_iter()
                    .filter(|item| item.finished_videos > 0)
                    .collect();
                *pending.lock() = Some((filtered, total_count, 0));
                ctx.request_repaint();
            }
        });
    }

    /// Ensure the current library window covers the active page.
    /// Triggers background load if current page center is outside current window.
    pub fn reconcile_library_window(&mut self, ctx: &egui::Context) {
        if !matches!(self.nav.view_mode, ViewMode::Library) {
            return;
        }

        let items_per_page = self.grid.items_per_page;
        if items_per_page == 0 {
            return;
        }

        let current_page_start = self.grid.current_page * items_per_page;
        let current_page_center = current_page_start + (items_per_page / 2);

        let window_start = self.grid.window_start_offset;
        let window_end = window_start + self.grid.items.len();

        // If the center of the current page is not in our loaded window, we need to shift.
        let contained = current_page_center >= window_start && current_page_center < window_end;

        if !contained {
            // The pagination watch channel coalesces — sending the same request
            // repeatedly is harmless, so we always send when the window needs shifting.
            tracing::info!(
                "Reconciling library window: page_center={} outside window [{}, {}). Loading.",
                current_page_center, window_start, window_end
            );
            self.load_window(ctx, current_page_center);
        }
    }
}

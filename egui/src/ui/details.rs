use crate::state::MyApp;
use crate::ui::grid;
use anyhow::Result;
use eframe::egui;
use std::future::Future;
use std::pin::Pin;

pub fn draw_model_details(ctx: &egui::Context, app: &mut MyApp, model_name: &str) {
    draw_details_generic(
        ctx,
        app,
        model_name,
        "model",
        move |db, name, offset, limit, skip_count| {
            Box::pin(async move {
                db.get_videos_by_performer_name_paginated(name, offset, limit, skip_count)
                    .await
            })
        },
    );
}

pub fn draw_studio_details(ctx: &egui::Context, app: &mut MyApp, studio_name: &str) {
    draw_details_generic(
        ctx,
        app,
        studio_name,
        "studio",
        move |db, name, offset, limit, skip_count| {
            Box::pin(async move {
                db.get_videos_by_studio_name_paginated(name, offset, limit, skip_count)
                    .await
            })
        },
    );
}

fn draw_details_generic<F>(
    ctx: &egui::Context,
    app: &mut MyApp,
    name: &str,
    _kind: &str, // "model" or "studio" used for cache key/validation if needed
    fetcher: F,
) where
    F: Fn(
            std::sync::Arc<soromantic_core::db::Database>,
            String,
            i64,
            i64,
            bool, // skip_count
        ) -> Pin<
            Box<
                dyn Future<
                        Output = Result<(Vec<soromantic_core::db::LibraryItem>, i64, Vec<String>)>,
                    > + Send,
            >,
        >
        + Send
        + 'static
        + Copy,
{
    egui::CentralPanel::default()
        .frame(
            egui::Frame::central_panel(&ctx.style())
                .inner_margin(egui::Margin::symmetric(8.0, 0.0)),
        )
        .show(ctx, |ui| {
            // Calculate metrics first (always needed for layout and pagination)
            let metrics = grid::calculate_grid_metrics(ctx, ui);
            let items_per_page = metrics.cols * metrics.rows;
            app.grid.items_per_page = items_per_page; // Sync for footer

            // Header: Show associated URLs -> MOVED TO FOOTER
            // if !app.model_studio.model_studio_urls.is_empty() {
            //     ui.horizontal_wrapped(|ui| {
            //         ui.label("Sources:");
            //         for url in &app.model_studio.model_studio_urls {
            //             ui.hyperlink(url);
            //             ui.add_space(8.0);
            //         }
            //     });
            //     ui.separator();
            // }
            // ui.separator();

            // Check if we have data for the current ID/Page
            // We take the items out to avoid borrow checker issues (E0502)
            let items_opt = app.model_studio.model_studio_items.take();
            let mut reload = false;
            let mut show_spinner = false;

            if let Some(items) = items_opt {
                let total_count = usize::try_from(app.model_studio.model_studio_total_count).unwrap_or(0);

                // Handle Input (Selection, Navigation, etc.)
                ui.add_space(10.0);

                // Pagination is controlled by the footer now

                // Handle Input (Selection, Navigation, etc.)
                grid::handle_grid_input(ctx, ui, app, &items, total_count, items_per_page);

                ui.add_space(10.0);

                grid::draw_grid(
                    ctx,
                    app,
                    ui,
                    &items,
                    &items,
                    0, // Offset is 0 relative to this slice
                    metrics.cols,
                    None::<fn(&mut egui::Ui, usize, f32, &mut MyApp)>,
                );

                // If not reloading, put the items back (we always keep them until new ones arrive)
                app.model_studio.model_studio_items = Some(items);

                // Check if we need to load a new page due to pagination
                if app.model_studio.current_model_studio_page != app.model_studio.loaded_model_studio_page {
                    reload = true; // Use existing flag or just trigger below
                }
            } else {
                // Initial load (items is None)
                reload = true;
                show_spinner = true;
            }

            if reload {
                if show_spinner {
                    ui.vertical_centered(|ui| {
                        ui.add_space(50.0);
                        ui.spinner();
                        ui.label("Loading videos...");
                    });
                }
                // We have items (old page), but we are loading new ones.
                // Optional: Show a small spinner overlay or status in footer?
                // For now, just keep showing old items (smooth transition).

                // Trigger Load
                if !app.model_studio.is_loading_model_studio {
                    app.model_studio.is_loading_model_studio = true; // Mark as loading

                    let db = app.db.clone();
                    let pending = app.model_studio.pending_model_studio_items.clone();
                    let ctx = ctx.clone();
                    let page = app.model_studio.current_model_studio_page;
                    // Use dynamic items_per_page for limit, not fixed batch size
                    let limit = i64::try_from(items_per_page.max(1)).unwrap_or(20);
                    let offset = i64::try_from(page).unwrap_or(0).saturating_mul(limit);
                    let name = name.to_string();
                    // fetcher is Copy, so it's captured automatically by move closure without re-binding

                    let skip_count = offset > 0 && !app.grid.library_dirty; // For simple logic, only count on first page or if dirty

                    app.rt_handle.spawn(async move {
                        match fetcher(db, name, offset, limit, skip_count).await {
                            Ok((items, count, urls)) => {
                                *pending.lock() = Some((items, count, urls, page));
                            }
                            Err(e) => {
                                tracing::error!("Failed to fetch details: {}", e);
                            }
                        }
                        ctx.request_repaint();
                    });
                }
            }
        });
}

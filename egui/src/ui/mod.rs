use crate::state::{FocusScope, MyApp, ViewMode};

pub mod details;
pub mod footer;
pub mod grid;
pub mod lists;
pub mod related;

pub fn draw_ui(ctx: &eframe::egui::Context, app: &mut MyApp) {
    app.check_async_results();

    // TAB handling for Library view (Grid ↔ Footer)
    if matches!(app.nav.view_mode, ViewMode::Library) && !app.scrape.show_scrape_window {
        let typing = ctx.wants_keyboard_input();
        if !typing && ctx.input(|i| i.key_pressed(eframe::egui::Key::Tab)) {
            app.nav.focus_scope = match app.nav.focus_scope {
                FocusScope::Grid => FocusScope::Footer,
                _ => FocusScope::Grid,
            };
        }
    }

    // Footer must be drawn BEFORE CentralPanel to reserve space
    if app.ui_config.show_footer {
        footer::draw_footer(app, ctx);
    }

    match app.nav.view_mode {
        ViewMode::Library => {
            // Draw Library Grid
            eframe::egui::CentralPanel::default()
                .frame(
                    eframe::egui::Frame::central_panel(&ctx.style())
                        .inner_margin(eframe::egui::Margin::symmetric(8.0, 0.0)),
                )
                .show(ctx, |ui| {
                    // Calculate metrics inside UI where 'ui' is available
                    let metrics = grid::calculate_grid_metrics(ctx, ui);
                    let items_per_page = metrics.cols * metrics.rows;
                    app.grid.items_per_page = items_per_page;

                    // Clone items once — pass &items to all functions so the borrow
                    // checker can separate &mut app from the local &[LibraryItem] borrow.
                    let items = app.grid.items.clone();

                    grid::handle_grid_input(
                        ctx,
                        ui,
                        app,
                        &items,
                        usize::try_from(app.grid.library_total_count).unwrap_or(0),
                        items_per_page,
                    );

                    app.reconcile_library_window(ctx);

                    let (global_start, _global_end) = grid::update_pagination(
                        app,
                        usize::try_from(app.grid.library_total_count).unwrap_or(0),
                        items_per_page,
                    );

                    let window_offset = app.grid.window_start_offset;
                    let local_start = global_start.saturating_sub(window_offset);
                    let local_end = (local_start + items_per_page).min(items.len());

                    let render_slice = if local_start < items.len() {
                        &items[local_start..local_end]
                    } else {
                        &[]
                    };

                    grid::preload_items(ctx, app, &items);

                    ui.add_space(20.0);

                    grid::draw_grid(
                        ctx,
                        app,
                        ui,
                        render_slice,
                        &items,
                        global_start,
                        metrics.cols,
                        None::<fn(&mut eframe::egui::Ui, usize, f32, &mut MyApp)>,
                    );
                });
        }
        ViewMode::Related(_) => {
            related::draw_related(ctx, app);
        }
        ViewMode::Models => {
            lists::draw_models(ctx, app);
        }
        ViewMode::Studios => {
            lists::draw_studios(ctx, app);
        }
        ViewMode::ModelDetails(ref name) => {
            let name = name.clone();
            details::draw_model_details(ctx, app, &name);
        }
        ViewMode::StudioDetails(ref name) => {
            let name = name.clone();
            details::draw_studio_details(ctx, app, &name);
        }
    }
}

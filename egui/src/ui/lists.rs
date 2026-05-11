use crate::state::{MyApp, ViewMode};
use eframe::egui;

pub fn draw_models(ctx: &egui::Context, app: &mut MyApp) {
    egui::CentralPanel::default().show(ctx, |ui| {
        if let Some(models) = &app.model_studio.models {
            ui.heading(format!("Models ({})", models.len()));
            ui.separator();

            egui::ScrollArea::vertical().show(ui, |ui| {
                egui::Grid::new("models_grid")
                    .striped(true)
                    .spacing(egui::vec2(20.0, 10.0))
                    .show(ui, |ui| {
                        for model in models {
                            if ui
                                .link(egui::RichText::new(&model.name).size(16.0))
                                .clicked()
                            {
                                app.nav.view_mode = ViewMode::ModelDetails(model.name.clone());
                                app.model_studio.model_studio_items = None;
                                app.model_studio.current_model_studio_page = 0;
                                app.model_studio.model_studio_total_count = 0;
                            }
                            ui.label(format!("{} videos", model.count));

                            // Link to source if available?
                            // ui.hyperlink(&model.url);

                            ui.end_row();
                        }
                    });
            });
        } else {
            ui.vertical_centered(|ui| {
                ui.add_space(50.0);
                ui.spinner();
                ui.label("Loading models...");
            });

            // Trigger load if not running
            // Use -1 as special ID for loading models
            if app.images.loading_ids.insert(-1) {
                let db = app.db.clone();
                let pending = app.model_studio.pending_models.clone();
                let ctx = ctx.clone();

                app.rt_handle.spawn(async move {
                    match db.get_all_performers().await {
                        Ok(data) => {
                            *pending.lock() = Some(data);
                        }
                        Err(e) => {
                            tracing::error!("Failed to fetch models: {}", e);
                        }
                    }
                    ctx.request_repaint();
                });
            }
        }
    });
}

pub fn draw_studios(ctx: &egui::Context, app: &mut MyApp) {
    egui::CentralPanel::default().show(ctx, |ui| {
        if let Some(studios) = &app.model_studio.studios {
            ui.heading(format!("Studios ({})", studios.len()));
            ui.separator();

            egui::ScrollArea::vertical().show(ui, |ui| {
                egui::Grid::new("studios_grid")
                    .striped(true)
                    .spacing(egui::vec2(20.0, 10.0))
                    .show(ui, |ui| {
                        for studio in studios {
                            if ui
                                .link(egui::RichText::new(&studio.name).size(16.0))
                                .clicked()
                            {
                                app.nav.view_mode = ViewMode::StudioDetails(studio.name.clone());
                                app.model_studio.model_studio_items = None;
                                app.model_studio.current_model_studio_page = 0;
                                app.model_studio.model_studio_total_count = 0;
                            }
                            ui.label(format!("{} videos", studio.count));
                            ui.end_row();
                        }
                    });
            });
        } else {
            ui.vertical_centered(|ui| {
                ui.add_space(50.0);
                ui.spinner();
                ui.label("Loading studios...");
            });

            // Use -2 as special ID for loading studios
            if app.images.loading_ids.insert(-2) {
                let db = app.db.clone();
                let pending = app.model_studio.pending_studios.clone();
                let ctx = ctx.clone();

                app.rt_handle.spawn(async move {
                    match db.get_all_studios().await {
                        Ok(data) => {
                            *pending.lock() = Some(data);
                        }
                        Err(e) => {
                            tracing::error!("Failed to fetch studios: {}", e);
                        }
                    }
                    ctx.request_repaint();
                });
            }
        }
    });
}

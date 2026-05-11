use super::actions::Action;
use super::layout::LayoutMetrics;
use crate::state::MyApp;
use eframe::egui;
use soromantic_core::db::LibraryItem;

pub fn render_panel(
    ui: &mut egui::Ui,
    app: &MyApp,
    actions: &mut Vec<Action>,
    metrics: &LayoutMetrics,
    nav_items: &[LibraryItem],
    current_nav_id: i64,
    search_query: &str,
) {
    if metrics.wide_layout {
        render_wide(
            ui,
            app,
            actions,
            metrics,
            nav_items,
            current_nav_id,
            search_query,
        );
    } else {
        render_narrow(
            ui,
            app,
            actions,
            metrics,
            nav_items,
            current_nav_id,
            search_query,
        );
    }
}

fn render_wide(
    ui: &mut egui::Ui,
    app: &MyApp,
    actions: &mut Vec<Action>,
    metrics: &LayoutMetrics,
    nav_items: &[LibraryItem],
    current_nav_id: i64,
    search_query: &str,
) {
    ui.horizontal(|ui| {
        // Image (1 column)
        ui.allocate_ui(egui::vec2(metrics.col_width, metrics.row_height), |ui| {
            ui.vertical_centered(|ui| {
                let thumb_size = egui::vec2(metrics.col_width, metrics.item_height);
                render_main_image(ui, app, actions, thumb_size);
            });
        });

        ui.add_space(metrics.spacing);

        // Panel (remaining columns)
        let panel_cols = metrics.cols - 1;
        let panel_width = metrics.col_width.mul_add(
            f32::from(u8::try_from(panel_cols).unwrap_or(1)),
            metrics.spacing * f32::from(u8::try_from(panel_cols.saturating_sub(1)).unwrap_or(0)),
        );

        ui.allocate_ui(egui::vec2(panel_width, metrics.row_height), |ui| {
            ui.vertical(|ui| {
                render_title_block(ui, app);
                ui.add_space(3.0);

                // 3 columns: Buttons | Search/Filter | Nav List
                ui.horizontal(|ui| {
                    // Column 1: Buttons (fixed width)
                    ui.allocate_ui(egui::vec2(110.0, metrics.row_height - 50.0), |ui| {
                        render_buttons_column(ui, app, actions);
                    });

                    ui.separator();

                    // Column 2: Search/Filter (fixed width)
                    ui.allocate_ui(egui::vec2(160.0, metrics.row_height - 50.0), |ui| {
                        render_search_column(ui, actions, search_query);
                    });

                    ui.separator();

                    // Column 3: Nav List (remaining width, ~9 items visible)
                    ui.vertical(|ui| {
                        let height = metrics.row_height - 55.0;
                        render_nav_list(
                            ui,
                            actions,
                            nav_items,
                            current_nav_id,
                            "nav_scroll",
                            height,
                        );
                    });
                });
            });
        });
    });
}

fn render_narrow(
    ui: &mut egui::Ui,
    app: &MyApp,
    actions: &mut Vec<Action>,
    metrics: &LayoutMetrics,
    nav_items: &[LibraryItem],
    current_nav_id: i64,
    search_query: &str,
) {
    // Image
    ui.vertical_centered(|ui| {
        let thumb_size = egui::vec2(metrics.col_width, metrics.item_height);
        render_main_image(ui, app, actions, thumb_size);
    });

    ui.add_space(8.0);

    // Panel (full width, below)
    ui.vertical(|ui| {
        render_title_block(ui, app);
        ui.add_space(4.0);

        ui.horizontal(|ui| {
            if ui
                .add(
                    egui::Button::new(egui::RichText::new("⬅ Back").size(16.0))
                        .min_size(egui::vec2(80.0, 26.0)),
                )
                .clicked()
            {
                actions.push(Action::Back);
            }
            if let Some(page) = &app.nav.active_page
                && ui
                    .add(
                        egui::Button::new(egui::RichText::new("▶ Play").size(16.0))
                            .min_size(egui::vec2(80.0, 26.0)),
                    )
                    .clicked()
            {
                actions.push(Action::Grid(crate::ui::grid::GridAction::Play(page.id)));
            }
        });

        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Search:").size(15.0));
            let mut tmp_search = search_query.to_string();
            let response = ui.add(
                egui::TextEdit::singleline(&mut tmp_search)
                    .desired_width(200.0)
                    .font(egui::TextStyle::Body),
            );
            if response.changed() {
                actions.push(Action::Search(tmp_search));
            }
        });
        ui.add_space(4.0);

        // List (vertical, limited height)
        render_nav_list(
            ui,
            actions,
            nav_items,
            current_nav_id,
            "nav_scroll_narrow",
            55.0,
        );
    });
}

fn render_main_image(ui: &mut egui::Ui, app: &MyApp, actions: &mut Vec<Action>, size: egui::Vec2) {
    if let Some(page) = &app.nav.active_page {
        let page_id = page.id;
        let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());

        // Draw thumbnail (no preview for main page - would need back-reference)
        if let Some(texture) = app.images.textures.get(&page_id) {
            let mut mesh = egui::Mesh::with_texture(texture.texture.id());
            mesh.add_rect_with_uv(
                rect,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );
            ui.painter().add(mesh);
        } else {
            ui.painter()
                .rect_filled(rect, 5.0, egui::Color32::from_rgb(50, 50, 50));
        }

        if response.clicked() {
            actions.push(Action::Grid(crate::ui::grid::GridAction::Play(page_id)));
        }
    }
}

fn render_title_block(ui: &mut egui::Ui, app: &MyApp) {
    if let Some(page) = &app.nav.active_page {
        ui.add(egui::Label::new(egui::RichText::new(&page.title).strong().size(16.0)).truncate());
        // ID + Studio on same row
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(format!("ID: {}", page.id))
                    .size(14.0)
                    .color(egui::Color32::GRAY),
            );
            if let Some(studio) = &page.studio {
                ui.label(
                    egui::RichText::new(format!("• {studio}"))
                        .size(14.0)
                        .color(egui::Color32::LIGHT_BLUE),
                );
            }
        });
    }
}

fn render_buttons_column(ui: &mut egui::Ui, app: &MyApp, actions: &mut Vec<Action>) {
    ui.vertical(|ui| {
        if ui
            .add(
                egui::Button::new(egui::RichText::new("⬅ Back").size(15.0))
                    .min_size(egui::vec2(100.0, 24.0)),
            )
            .clicked()
        {
            actions.push(Action::Back);
        }
        ui.add_space(3.0);
        if let Some(page) = &app.nav.active_page
            && ui
                .add(
                    egui::Button::new(egui::RichText::new("▶ Play").size(15.0))
                        .min_size(egui::vec2(100.0, 24.0)),
                )
                .clicked()
        {
            actions.push(Action::Grid(crate::ui::grid::GridAction::Play(page.id)));
        }
    });
}

fn render_search_column(ui: &mut egui::Ui, actions: &mut Vec<Action>, search_query: &str) {
    ui.vertical(|ui| {
        ui.label(egui::RichText::new("Search:").size(14.0));
        let mut tmp_search = search_query.to_string();
        let response = ui.add(egui::TextEdit::singleline(&mut tmp_search).desired_width(150.0));
        if response.changed() {
            actions.push(Action::Search(tmp_search));
        }
        ui.add_space(5.0);
        ui.label(
            egui::RichText::new("Filter:")
                .size(14.0)
                .color(egui::Color32::DARK_GRAY),
        );
        // Placeholder for future filter
    });
}

fn render_nav_list(
    ui: &mut egui::Ui,
    actions: &mut Vec<Action>,
    nav_items: &[LibraryItem],
    current_nav_id: i64,
    salt: &str,
    height: f32,
) {
    egui::ScrollArea::vertical()
        .id_salt(salt)
        .max_height(height)
        .show(ui, |ui| {
            for item in nav_items {
                let is_selected = item.id == current_nav_id;
                let response =
                    ui.selectable_label(is_selected, egui::RichText::new(&item.title).size(14.0));
                if is_selected {
                    response.scroll_to_me(Some(egui::Align::Center));
                }
                if response.clicked() {
                    actions.push(Action::SetFocusScope(crate::state::FocusScope::NavList));
                    actions.push(Action::Grid(crate::ui::grid::GridAction::OpenPage(item.id)));
                }
            }
        });
}

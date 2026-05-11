mod actions;
mod data;
mod header;
mod input;
mod layout;

pub use actions::Action;

use crate::state::MyApp;
use eframe::egui;

pub fn draw_related(ctx: &egui::Context, app: &mut MyApp) {
    let mut actions = Vec::new();

    // 1. Handle Global Shortcuts (Tab cycling)
    input::handle_global_shortcuts(ctx, app, &mut actions);

    // 2. Pre-Load Images
    data::preload_images(ctx, app);

    // 3. Prepare Data
    let (grid_items, nav_items, current_nav_id) = data::prepare_data(app);
    let search_query = app.nav.browser_search.clone();

    // 4. Handle Context-Specific Input
    input::handle_nav_list_input(ctx, app, &nav_items, current_nav_id, &mut actions);
    input::handle_header_buttons_input(ctx, app, &mut actions);

    // 5. Render UI
    egui::CentralPanel::default()
        .frame(
            eframe::egui::Frame::central_panel(&ctx.style())
                .inner_margin(eframe::egui::Margin::symmetric(8.0, 0.0)),
        )
        .show(ctx, |ui| {
            ui.add_space(20.0);

            // Calculate Layout
            let metrics = layout::calculate_layout(ui);

            // Render Header
            header::render_panel(
                ui,
                app,
                &mut actions,
                &metrics,
                &nav_items,
                current_nav_id,
                &search_query,
            );

            ui.add_space(8.0);

            // Render Grid
            // Calculate slice for pagination
            let items_per_page = metrics.cols * metrics.grid_rows;
            crate::ui::grid::handle_grid_input(
                ctx,
                ui,
                app,
                &grid_items,
                grid_items.len(),
                items_per_page,
            );
            let (start, end) =
                crate::ui::grid::update_pagination(app, grid_items.len(), items_per_page);
            let prefetch_end = (end + items_per_page).min(grid_items.len());

            let render_slice = if start < grid_items.len() {
                grid_items[start..end.min(grid_items.len())].to_vec()
            } else {
                Vec::new()
            };

            if end < prefetch_end {
                let prefetch_slice = grid_items[end..prefetch_end].to_vec();
                crate::ui::grid::preload_items(ctx, app, &prefetch_slice);
            }

            crate::ui::grid::draw_grid(
                ctx,
                app,
                ui,
                &render_slice,
                &grid_items,
                start,
                metrics.cols,
                None::<fn(&mut egui::Ui, usize, f32, &mut MyApp)>,
            );
        });

    // 6. Handle Actions
    for action in actions {
        match action {
            Action::Back => app.back(ctx),
            Action::Search(q) => {
                app.nav.browser_search.clone_from(&q);
                app.perform_search(ctx, q);
            }
            Action::SetFocusScope(scope) => {
                app.nav.focus_scope = scope;
            }
            Action::NavTo(id) => {
                app.open_page(ctx, id);
            }
            Action::HeaderButton(action) => {
                use crate::state::HeaderAction;
                match action {
                    HeaderAction::Back => app.back(ctx), // Back
                    HeaderAction::Play => {
                        // Play current page
                        if let Some(page) = &app.nav.active_page {
                            app.play_items(vec![page.id], &[]);
                        }
                    }
                }
            }
            Action::Grid(a) => match a {
                crate::ui::grid::GridAction::Play(id) => app.play_items(vec![id], &[]),
                crate::ui::grid::GridAction::OpenPage(id) => app.open_page(ctx, id),
                _ => {}
            },
        }
    }
}

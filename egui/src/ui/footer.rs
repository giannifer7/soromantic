use crate::state::{FocusScope, MyApp};
use eframe::egui::{self, RichText};

pub fn draw_footer(app: &mut MyApp, ctx: &egui::Context) {
    let (total_items, current_page, is_main_library) = match &app.nav.view_mode {
        crate::state::ViewMode::Library => (
            usize::try_from(app.grid.library_total_count).unwrap_or(0),
            app.grid.current_page,
            true,
        ),
        crate::state::ViewMode::Related(_) => (app.grid.total_items, app.grid.current_page, true),
        crate::state::ViewMode::ModelDetails(_) | crate::state::ViewMode::StudioDetails(_) => (
            usize::try_from(app.model_studio.model_studio_total_count).unwrap_or(0),
            app.model_studio.current_model_studio_page,
            false,
        ),
        _ => (0, 0, false),
    };

    handle_footer_input(app, ctx, total_items);

    // DYNAMIC FOOTER SIZING
    let min_footer_req = if app.scrape.show_scrape_window { 300.0 } else { 50.0 };
    let grid_h = crate::ui::grid::predict_grid_height(ctx, min_footer_req);
    let screen_h = ctx.input(|i| i.screen_rect.height());
    let target_footer_h = (screen_h - grid_h).max(min_footer_req);

    egui::TopBottomPanel::bottom("footer")
        .frame(egui::Frame::central_panel(&ctx.style()).inner_margin(0.0))
        .min_height(target_footer_h)
        .show(ctx, |ui| {
            if app.scrape.show_scrape_window {
                draw_scrape_inline_panel(ui, app, ctx);
                ui.separator();
            }

            draw_progress_section(ui, app);

            ui.horizontal(|ui| {
                ui.add_space(10.0);
                draw_footer_controls(ui, app, ctx, total_items, current_page, is_main_library);
            });
        });
}

fn handle_footer_input(app: &mut MyApp, ctx: &egui::Context, total_items: usize) {
    let typing = ctx.wants_keyboard_input();
    if !typing && app.nav.focus_scope == FocusScope::Footer && !app.scrape.show_scrape_window {
        if ctx.input(|i| i.key_pressed(egui::Key::ArrowRight) || i.key_pressed(egui::Key::L)) {
            app.nav.footer_focus = app.nav.footer_focus.next();
        }
        if ctx.input(|i| i.key_pressed(egui::Key::ArrowLeft) || i.key_pressed(egui::Key::H)) {
            app.nav.footer_focus = app.nav.footer_focus.prev();
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
            use crate::state::FooterAction;
            let total_pages =
                (total_items + app.grid.items_per_page - 1).max(1) / app.grid.items_per_page.max(1);

            // Determine which page variable to update
            let is_main = matches!(
                app.nav.view_mode,
                crate::state::ViewMode::Library | crate::state::ViewMode::Related(_)
            );
            let current_page = if is_main {
                app.grid.current_page
            } else {
                app.model_studio.current_model_studio_page
            };

            match app.nav.footer_focus {
                FooterAction::Library => { /* Library - no-op for now */ }
                FooterAction::Scrape => app.open_scrape_window(),
                FooterAction::Repair => {
                    app.scrape.repair_triggered = false;
                    app.trigger_repair(ctx);
                }
                FooterAction::PlaySelected => {
                    let ids: Vec<i64> = app.nav.selected_ids.iter().copied().collect();
                    app.play_items(ids, &[]);
                    app.nav.selected_ids.clear();
                }
                FooterAction::FirstPage => {
                    if is_main {
                        app.grid.current_page = 0;
                    } else {
                        app.model_studio.current_model_studio_page = 0;
                    }
                }
                FooterAction::PrevPage => {
                    if current_page > 0 {
                        if is_main {
                            app.grid.current_page -= 1;
                        } else {
                            app.model_studio.current_model_studio_page -= 1;
                        }
                    }
                }
                FooterAction::NextPage => {
                    if current_page < total_pages.saturating_sub(1) {
                        if is_main {
                            app.grid.current_page += 1;
                        } else {
                            app.model_studio.current_model_studio_page += 1;
                        }
                    }
                }
                FooterAction::LastPage => {
                    let last = total_pages.saturating_sub(1);
                    if is_main {
                        app.grid.current_page = last;
                    } else {
                        app.model_studio.current_model_studio_page = last;
                    }
                }
            }
        }
    }
}

fn draw_progress_section(ui: &mut egui::Ui, app: &MyApp) {
    let show_progress = app.scrape.batch_progress.is_some() || app.scrape.scrape_progress.is_some();
    let needed_height = if show_progress { 50.0 } else { 0.0 };

    if show_progress {
        ui.allocate_ui_with_layout(
            egui::vec2(ui.available_width(), needed_height),
            egui::Layout::top_down(egui::Align::Center),
            |ui| {
                if !app.scrape.scrape_title.is_empty() {
                    ui.label(egui::RichText::new(&app.scrape.scrape_title).strong().size(14.0));
                }

                let both_active = app.scrape.batch_progress.is_some() && app.scrape.scrape_progress.is_some();
                if both_active {
                    ui.columns(2, |cols| {
                        if let Some(bp) = &app.scrape.batch_progress {
                            cols[0].vertical_centered(|ui| draw_batch_progress_bar(ui, bp));
                        }
                        if let Some(sp) = &app.scrape.scrape_progress {
                            cols[1].vertical_centered(|ui| draw_scrape_progress_bar(ui, sp));
                        }
                    });
                } else if let Some(bp) = &app.scrape.batch_progress {
                    draw_batch_progress_bar(ui, bp);
                } else if let Some(sp) = &app.scrape.scrape_progress {
                    draw_scrape_progress_bar(ui, sp);
                }
            },
        );
        ui.add_space(5.0);
    } else {
        ui.add_space(5.0);
        ui.vertical_centered(|ui| {
            ui.label(egui::RichText::new(&app.scrape.scrape_status).color(egui::Color32::GRAY));
        });
    }
    ui.add_space(5.0);
}

fn draw_batch_progress_bar(ui: &mut egui::Ui, bp: &crate::state::BatchProgress) {
    let progress = if bp.total > 0 {
        #[allow(clippy::cast_precision_loss)]
        let p = bp.current as f32 / bp.total as f32;
        p
    } else {
        0.0
    };
    let text = format!("Batch: {}/{} Pages", bp.current, bp.total);
    ui.add(egui::ProgressBar::new(progress).text(text).animate(true));
}

fn draw_scrape_progress_bar(ui: &mut egui::Ui, sp: &crate::state::ScrapeProgress) {
    let text = if sp.message.is_empty() {
        format!("{} ({}/{})", sp.stage, sp.progress, sp.total)
    } else {
        sp.message.clone()
    };
    ui.add(
        egui::ProgressBar::new(sp.displayed_progress)
            .text(text)
            .animate(true),
    );
}

fn draw_footer_controls(
    ui: &mut egui::Ui,
    app: &mut MyApp,
    ctx: &egui::Context,
    total_items: usize,
    current_page: usize,
    is_main_library: bool,
) {
    // Left side: title and buttons
    ui.label(RichText::new("Soromantic").size(18.0).strong());
    ui.separator();

    if let Some(focused) = app.nav.focused_index {
        let focused_id = match app.nav.view_mode {
            crate::state::ViewMode::Library => app.grid.items.get(focused).map(|i| i.id),
            crate::state::ViewMode::Related(_) => app
                .nav
                .active_page
                .as_ref()
                .and_then(|p| p.grid.get(focused).and_then(|g| g.related_id)),
            _ => None,
        };
        if let Some(id) = focused_id {
            ui.label(RichText::new(format!("ID: {id}")).color(egui::Color32::GRAY));
            ui.separator();
        }
    }

    if ui.button("Library").clicked() {
        app.nav.view_mode = crate::state::ViewMode::Library;
        tracing::info!("Library clicked");
    }
    if ui.button("Models").clicked() {
        app.nav.view_mode = crate::state::ViewMode::Models;
        tracing::info!("Models clicked");
    }
    if ui.button("Studios").clicked() {
        app.nav.view_mode = crate::state::ViewMode::Studios;
        tracing::info!("Studios clicked");
    }
    if ui.button("Scrape").clicked() {
        app.open_scrape_window();
    }
    if ui.button("Repair").clicked() {
        app.scrape.repair_triggered = false;
        app.trigger_repair(ctx);
    }

    if ui.button("Play Selected").clicked() {
        let ids: Vec<i64> = app.nav.selected_ids.iter().copied().collect();
        app.play_items(ids, &[]);
        app.nav.selected_ids.clear();
    }

    // Left: Status / Back Button
    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
        // Show Back Button execution logic if clicked
        let mut go_back = false;
        if matches!(
            app.nav.view_mode,
            crate::state::ViewMode::ModelDetails(_) | crate::state::ViewMode::StudioDetails(_)
        ) {
            if ui.button("⬅ Back").clicked() {
                go_back = true;
            }
            ui.separator();

            // Show Sources next to Back button
            if !app.model_studio.model_studio_urls.is_empty() {
                ui.label("Sources:");
                for url in &app.model_studio.model_studio_urls {
                    ui.hyperlink(url);
                    ui.add_space(8.0);
                }
                ui.separator();
            }
        }

        if go_back {
            match app.nav.view_mode {
                crate::state::ViewMode::ModelDetails(_) => {
                    app.nav.view_mode = crate::state::ViewMode::Models;
                }
                crate::state::ViewMode::StudioDetails(_) => {
                    app.nav.view_mode = crate::state::ViewMode::Studios;
                }
                _ => {}
            }
            app.model_studio.model_studio_items = None;
            app.model_studio.current_model_studio_page = 0;
            app.model_studio.model_studio_total_count = 0;
        }

        ui.label(format!("Total: {total_items}"));
    });

    // Right side: pagination
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        draw_pagination_controls(ui, app, total_items, current_page, is_main_library);
    });
}

fn draw_pagination_controls(
    ui: &mut egui::Ui,
    app: &mut MyApp,
    total_items: usize,
    current_page: usize,
    is_main_library: bool,
) {
    ui.add_space(10.0); // Right padding

    let total_pages = (total_items + app.grid.items_per_page - 1).max(1) / app.grid.items_per_page.max(1);

    // Last Page
    if ui.button(">>|").clicked() {
        let last = total_pages.saturating_sub(1);
        if is_main_library {
            app.grid.current_page = last;
        } else {
            app.model_studio.current_model_studio_page = last;
        }
    }

    // Next Page
    if ui.button("▶").clicked() && current_page < total_pages.saturating_sub(1) {
        if is_main_library {
            app.grid.current_page += 1;
        } else {
            app.model_studio.current_model_studio_page += 1;
        }
    }

    ui.label(format!("{}/{}", current_page + 1, total_pages.max(1)));

    // Previous Page
    if ui.button("◀").clicked() && current_page > 0 {
        if is_main_library {
            app.grid.current_page -= 1;
        } else {
            app.model_studio.current_model_studio_page -= 1;
        }
    }

    // First Page
    if ui.button("|<<").clicked() {
        if is_main_library {
            app.grid.current_page = 0;
        } else {
            app.model_studio.current_model_studio_page = 0;
        }
    }
}

fn draw_scrape_inline_panel(ui: &mut egui::Ui, app: &mut MyApp, ctx: &egui::Context) {
    ui.vertical_centered(|ui| {
        ui.add_space(8.0);
        ui.label(RichText::new("Enter URL(s) to scrape (one per line):").size(16.0));
        ui.add_space(20.0);

        // Multi-line text input for pasting multiple URLs
        let text_edit = egui::TextEdit::multiline(&mut app.scrape.scrape_url)
            .desired_width(800.0) // Limit width for better aesthetics
            .desired_rows(8) // More rows
            .font(egui::TextStyle::Monospace) // Monospace for URLs
            .margin(egui::vec2(10.0, 10.0));
        let response = ui.add(text_edit);

        // Auto-focus logic
        if app.scrape.focus_scrape_input {
            response.request_focus();
            app.scrape.focus_scrape_input = false; // Only focus once
        }

        // Handle middle-click paste (X11 primary selection)
        if response.has_focus()
            && ctx.input(|i| i.pointer.button_clicked(egui::PointerButton::Middle))
        {
            // Fallback: try native Wayland then local arboard (X11)
            #[cfg(target_os = "linux")]
            {
                use wl_clipboard_rs::paste::{ClipboardType, MimeType, Seat, get_contents};

                // Try native Wayland (libwayland-client)
                let wayland_text =
                    get_contents(ClipboardType::Primary, Seat::Unspecified, MimeType::Text)
                        .ok()
                        .and_then(|(mut pipe, _)| {
                            let mut contents = String::new();
                            std::io::Read::read_to_string(&mut pipe, &mut contents).ok()?;
                            Some(contents.trim().to_string())
                        })
                        .filter(|s| !s.is_empty());

                if let Some(text) = wayland_text {
                    app.scrape.scrape_url.push_str(&text);
                } else {
                    // All native Wayland failed, try local arboard (X11 fallback)
                    if let Ok(mut clipboard) = arboard::Clipboard::new() {
                        use arboard::{GetExtLinux, LinuxClipboardKind};
                        if let Ok(text) = clipboard
                            .get()
                            .clipboard(LinuxClipboardKind::Primary)
                            .text()
                        {
                            app.scrape.scrape_url.push_str(&text);
                        }
                    }
                }
            }
            #[cfg(not(target_os = "linux"))]
            {
                if let Ok(mut clipboard) = arboard::Clipboard::new() {
                    if let Ok(text) = clipboard.get_text() {
                        app.scrape.scrape_url.push_str(&text);
                    }
                }
            }
        }

        // Handle Escape to close
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            app.close_scrape_window();
        }

        ui.add_space(20.0);

        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(20.0);
                let button_size = egui::vec2(100.0, 32.0);

                // Use Ctrl+Enter to submit (Enter adds newline in multiline)
                let ctrl_enter = ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::Enter));
                if ui
                    .add_sized(
                        button_size,
                        egui::Button::new(RichText::new("Scrape").size(15.0)),
                    )
                    .clicked()
                    || ctrl_enter
                {
                    app.trigger_scrape();
                }

                ui.add_space(16.0);

                if ui
                    .add_sized(
                        button_size,
                        egui::Button::new(RichText::new("Cancel").size(15.0)),
                    )
                    .clicked()
                {
                    app.close_scrape_window();
                }
            });
        });

        ui.add_space(12.0);
    });
}

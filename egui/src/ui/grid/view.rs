// egui/src/ui/grid/views.rs
use super::actions::GridAction;
use crate::state::MyApp;
use eframe::egui::{self, RichText};
use soromantic_core::db::LibraryItem;
use std::cell::RefCell;
use std::path::PathBuf;

pub struct GridMetrics {
    pub cols: usize,
    pub rows: usize,
}

pub const GRID_TOP_MARGIN: f32 = 20.0;
/// Buffer below grid rows to prevent footer overlap.
pub const GRID_BOTTOM_BUFFER: f32 = 20.0;
const TITLE_HEIGHT: f32 = 24.0;
const TITLE_GAP: f32 = 5.0;
const ROW_GAP: f32 = 8.0;
const ASPECT_RATIO: f32 = 16.0 / 9.0;

/// Shared grid sizing computation used by both `calculate_grid_metrics` and `predict_grid_height`.
struct GridSizing {
    cols: usize,
    rows: usize,
    row_h: f32,
}

impl GridSizing {
    fn compute(available_w: f32, available_h: f32, spacing: f32, overhead: f32) -> Self {
        let cols = if available_w >= 1200.0 { 4 }
            else if available_w >= 900.0 { 3 }
            else if available_w >= 600.0 { 2 }
            else { 1 };

        #[allow(clippy::cast_precision_loss)]
        let cols_f = if cols < 4 { cols as f32 } else { 4.0 };
        let item_w = spacing.mul_add(-(cols_f - 1.0), available_w) / cols_f;
        let item_h = item_w / ASPECT_RATIO;
        let row_h = item_h + TITLE_GAP + TITLE_HEIGHT + ROW_GAP;

        let avail = (available_h - overhead).max(0.0);
        let rows = if avail >= row_h * 3.0 { 3 }
            else if avail >= row_h * 2.0 { 2 }
            else { 1 };

        Self { cols, rows, row_h }
    }
}

pub fn calculate_grid_metrics(_ctx: &egui::Context, ui: &egui::Ui) -> GridMetrics {
    let sizing = GridSizing::compute(
        ui.available_width(),
        ui.available_height(),
        ui.spacing().item_spacing.x,
        GRID_TOP_MARGIN + GRID_BOTTOM_BUFFER,
    );
    GridMetrics { cols: sizing.cols, rows: sizing.rows }
}

pub fn predict_grid_height(ctx: &egui::Context, min_footer_height: f32) -> f32 {
    let screen_rect = ctx.input(|i| i.screen_rect);
    let sizing = GridSizing::compute(
        screen_rect.width() - 16.0,  // CentralPanel inner margin
        screen_rect.height(),
        8.0,                         // standard spacing
        GRID_TOP_MARGIN + min_footer_height,
    );
    #[allow(clippy::cast_precision_loss)]
    let rows_f = if sizing.rows < 4 { sizing.rows as f32 } else { 4.0 };
    GRID_TOP_MARGIN + rows_f * sizing.row_h + GRID_BOTTOM_BUFFER
}

pub fn preload_items(ctx: &egui::Context, app: &mut MyApp, items: &[LibraryItem]) {
    // Collect ALL items that need loading (in the current window)
    // We must check ALL of them to ensure we prioritize the visible ones,
    // otherwise the `take(50)` cut-off might starve the actual view if the window is large.
    let mut items_to_load: Vec<(usize, i64, PathBuf)> = items
        .iter()
        .enumerate()
        .filter(|(_, item)| {
            !app.images.textures.contains_key(&item.id) && !app.images.loading_ids.contains(&item.id)
        })
        .filter_map(|(idx, item)| {
            item.local_image
                .as_ref()
                .map(|local_path| (idx, item.id, PathBuf::from(local_path)))
        })
        .collect();

    if items_to_load.is_empty() {
        return;
    }

    // Calculate priority based on visibility
    // Visible items should be processed FIRST.
    // Window-relative visible range:
    let global_start = app.grid.current_page * app.grid.items_per_page;
    let window_start = app.grid.window_start_offset;
    let visible_start = global_start.saturating_sub(window_start);
    let visible_end = visible_start + app.grid.items_per_page;

    // Sort: Visible items first (distance 0), then by distance from visible range
    items_to_load.sort_by_key(|(idx, _, _)| {
        if *idx >= visible_start && *idx < visible_end {
            0 // High priority
        } else if *idx < visible_start {
            // Buffer above (distance)
            visible_start - idx
        } else {
            // Buffer below (distance)
            idx - visible_end
        }
    });

    // NOW take the limit, after sorting
    for (_, id, path) in items_to_load.into_iter().take(50) {
        app.request_image_load(ctx, id, path);
    }
}

#[allow(clippy::too_many_arguments)]
pub fn draw_grid<F>(
    ctx: &egui::Context,
    app: &mut MyApp,
    ui: &mut egui::Ui,
    render_items: &[LibraryItem],
    all_context_items: &[LibraryItem],
    offset: usize,
    cols: usize,
    header: Option<F>,
) where
    F: FnOnce(&mut egui::Ui, usize, f32, &mut MyApp),
{
    let actions = RefCell::new(Vec::new());

    // Preload visible items
    preload_items(ctx, app, render_items);

    // Cols passed explicitly to avoid nested container layout issues

    // Calculate item width for Header usage
    let spacing = ui.spacing().item_spacing.x;
    let avail_w = ui.available_width();
    let cols_f = f32::from(u8::try_from(cols).unwrap_or(1));
    let item_w = spacing.mul_add(-(cols_f - 1.0), avail_w) / cols_f;

    // Render Header (Top Row) within the same geometry context
    if let Some(h) = header {
        h(ui, cols, item_w, app);
        ui.add_space(8.0); // Standard gap
    }

    let mut current_idx = offset;

    ui.vertical_centered(|ui| {
        for chunk in render_items.chunks(cols) {
            ui.columns(cols, |columns| {
                for (col_idx, item) in chunk.iter().enumerate() {
                    let item_idx = current_idx;
                    current_idx += 1;

                    columns[col_idx].vertical_centered(|ui| {
                        draw_grid_item(
                            ui,
                            ctx,
                            app,
                            item,
                            item_idx,
                            item_idx.saturating_sub(offset),
                            &actions,
                        );
                    });
                }
            });
            ui.add_space(8.0);
        }
    });

    // Apply deferred actions
    for action in actions.into_inner() {
        match action {
            GridAction::ToggleSelection(id, idx) => {
                app.toggle_selection(id);
                app.nav.last_clicked_index = Some(idx);
            }
            GridAction::SelectRange(start, _end) => {
                // Use existing last_clicked_index as anchor
                // Default to 0 if none set
                let anchor = app.nav.last_clicked_index.unwrap_or(0);
                app.select_range(anchor, start, all_context_items);

                // Update last clicked to current target
                app.nav.last_clicked_index = Some(start);
            }
            GridAction::Play(id) => {
                if app.nav.selected_ids.contains(&id) && !app.nav.selected_ids.is_empty() {
                    let ids: Vec<i64> = app.nav.selected_ids.iter().copied().collect();
                    app.play_items(ids, all_context_items);
                } else {
                    app.play_items(vec![id], all_context_items);
                }
                app.nav.selected_ids.clear();
            }
            GridAction::Focus(idx) => {
                // Only update visual focus here
                // We do NOT update last_clicked_index to avoid clobbering it before SelectRange runs.
                // However, for pure clicks (OpenPage), we might want to?
                // Actually, let's leave it to specific actions if possible.
                // But OpenPage also needs it?
                app.nav.focused_index = Some(idx);
            }
            GridAction::RequestPreview(id, path, runtime_dir) => {
                app.request_preview(ctx, id, path, runtime_dir);
            }
            GridAction::OpenPage(id) => {
                app.open_page(ctx, id);
                // Also update last clicked
                if let Some(idx) = app.grid.items.iter().position(|i| i.id == id) {
                    app.nav.last_clicked_index = Some(idx);
                }
            }
        }
    }
}

fn draw_grid_item(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    app: &mut MyApp,
    item: &LibraryItem,
    item_idx: usize,
    visual_idx: usize,
    actions: &RefCell<Vec<GridAction>>,
) {
    let w = ui.available_width();
    let h = w / ASPECT_RATIO;
    let is_selected = app.nav.selected_ids.contains(&item.id);
    let is_focused = app.nav.focused_index == Some(item_idx);
    let is_hovered_id = ui.make_persistent_id(format!("img_{}", item.id));
    let (rect, response) = ui.allocate_exact_size(egui::vec2(w, h), egui::Sense::click());

    // 1. Thumbnail
    draw_thumbnail(ui, app, item, visual_idx, rect);
    // 2. Preview overlay
    let show_preview = draw_preview_overlay(ui, ctx, app, item, &response, is_hovered_id, actions);
    // 3. Borders
    draw_item_borders(ui, &response, is_selected, is_focused);
    // 4. Input
    handle_grid_clicks(ui, app, item, item_idx, &response, actions);
    // 5. Caption: title + status LED
    ui.add_space(TITLE_GAP);
    draw_grid_caption(ui, ctx, app, item, is_selected, w);

    if show_preview
        && let Some(frames) = app.images.preview_cache.get(&item.id)
        && frames.ready && !frames.frames.is_empty()
    {
        ctx.request_repaint();
    }
}

/// Render the thumbnail texture, grid-cached texture, or a dark placeholder.
fn draw_thumbnail(
    ui: &egui::Ui,
    app: &mut MyApp,
    item: &LibraryItem,
    visual_idx: usize,
    rect: egui::Rect,
) {
    if let Some(loaded) = app.images.textures.get(&item.id) {
        app.grid.grid_cache.insert(visual_idx, loaded.texture.clone());
        let mut mesh = egui::Mesh::with_texture(loaded.texture.id());
        mesh.add_rect_with_uv(
            rect,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
        ui.painter().add(mesh);
    } else if let Some(cached) = app.grid.grid_cache.get(&visual_idx) {
        let mut mesh = egui::Mesh::with_texture(cached.id());
        mesh.add_rect_with_uv(
            rect,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
        ui.painter().add(mesh);
    } else {
        ui.painter().rect_filled(rect, 5.0, egui::Color32::from_rgb(50, 50, 50));
    }
}

/// Request preview on hover; render animated preview frames if ready.
fn draw_preview_overlay(
    ui: &egui::Ui,
    ctx: &egui::Context,
    app: &MyApp,
    item: &LibraryItem,
    response: &egui::Response,
    is_hovered_id: egui::Id,
    actions: &RefCell<Vec<GridAction>>,
) -> bool {
    let hovered = response.hovered();
    if hovered
        && let Some(preview_path) = &item.local_preview
    {
        actions.borrow_mut().push(GridAction::RequestPreview(
            item.id, preview_path.clone(), app.previews_dir.clone(),
        ));
    }

    if let Some(frames) = app.images.preview_cache.get(&item.id) {
        let should_show = hovered && frames.ready && !frames.frames.is_empty();
        let opacity = ctx.animate_bool(is_hovered_id.with("preview_fade"), should_show);
        if opacity > 0.0 && !frames.frames.is_empty() {
            let speed = 12.0;
            #[allow(clippy::cast_precision_loss)]
            let len = frames.frames.len() as f64;
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let idx = ((ctx.input(|i| i.time).abs() * speed).rem_euclid(len)) as usize;
            let mut mesh = egui::Mesh::with_texture(frames.frames[idx].id());
            mesh.add_rect_with_uv(
                response.rect,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE.gamma_multiply(opacity),
            );
            ui.painter().add(mesh);
        }
        return should_show;
    }
    false
}

/// Draw selection (blue) and focus (white) borders around the item rect.
fn draw_item_borders(ui: &egui::Ui, response: &egui::Response, is_selected: bool, is_focused: bool) {
    if is_selected {
        ui.painter().rect_stroke(
            response.rect, 5.0,
            egui::Stroke::new(3.0, egui::Color32::from_rgb(100, 150, 255)),
        );
    }
    if is_focused {
        ui.painter().rect_stroke(
            response.rect.expand(2.0), 5.0,
            egui::Stroke::new(1.0, egui::Color32::WHITE),
        );
        response.scroll_to_me(Some(egui::Align::Center));
    }
}

/// Translate mouse clicks into deferred grid actions.
fn handle_grid_clicks(
    ui: &egui::Ui,
    app: &mut MyApp,
    item: &LibraryItem,
    item_idx: usize,
    response: &egui::Response,
    actions: &RefCell<Vec<GridAction>>,
) {
    if response.clicked() {
        app.nav.focus_scope = crate::state::FocusScope::Grid;
        if ui.input(|i| i.modifiers.shift) {
            actions.borrow_mut().push(GridAction::SelectRange(item_idx, item_idx));
        } else {
            actions.borrow_mut().push(GridAction::ToggleSelection(item.id, item_idx));
        }
        actions.borrow_mut().push(GridAction::Focus(item_idx));
    }
    if response.secondary_clicked() {
        actions.borrow_mut().push(GridAction::OpenPage(item.id));
    }
    if response.double_clicked() {
        actions.borrow_mut().push(GridAction::Play(item.id));
    }
}

/// Render the item title and status LED below the thumbnail.
fn draw_grid_caption(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    app: &MyApp,
    item: &LibraryItem,
    is_selected: bool,
    max_w: f32,
) {
    const LED_WIDTH: f32 = 20.0;
    let (response, painter) =
        ui.allocate_painter(egui::vec2(max_w, TITLE_HEIGHT), egui::Sense::hover());
    let rect = response.rect;

    // Title
    let title_rect = egui::Rect::from_min_size(rect.min, egui::vec2((max_w - LED_WIDTH).max(0.0), rect.height()));
    ui.allocate_new_ui(eframe::egui::UiBuilder::new().max_rect(title_rect), |ui| {
        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
            ui.add(egui::Label::new(RichText::new(&item.title).strong().size(14.0)).truncate());
        });
    });

    // Status LED
    let led_center = egui::pos2(rect.max.x - 10.0, rect.center().y);
    let color = led_color(app, item, is_selected, ctx);
    painter.circle_filled(led_center, 4.5, color);
}

/// Determine the status LED color for a grid item.
fn led_color(app: &MyApp, item: &LibraryItem, is_selected: bool, ctx: &egui::Context) -> egui::Color32 {
    if is_selected {
        egui::Color32::from_rgb(0, 100, 255)
    } else if app.scrape.active_scrapes.contains(&item.id) {
        let flash = (ctx.input(|i| i.time) * 6.0).sin() > 0.0;
        if flash { egui::Color32::from_rgb(50, 150, 255) } else { egui::Color32::from_rgb(0, 100, 200) }
    } else if app.scrape.failed_scrapes.contains(&item.id) || item.failed_videos > 0 {
        egui::Color32::from_rgb(255, 128, 0)
    } else if item.finished_videos > 0 {
        egui::Color32::GREEN
    } else {
        egui::Color32::GRAY
    }
}

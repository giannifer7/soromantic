// egui/src/ui/grid/input.rs
use super::view::calculate_grid_metrics;
use crate::state::MyApp;
use eframe::egui;
use soromantic_core::db::LibraryItem;

pub fn handle_grid_input(
    ctx: &egui::Context,
    ui: &egui::Ui,
    app: &mut MyApp,
    items: &[LibraryItem],
    total_count: usize,
    items_per_page: usize,
) {
    let total_items = total_count;
    let metrics = calculate_grid_metrics(ctx, ui);
    let cols = metrics.cols;

    // Check if we are typing in a text field (Search, etc.)
    // If so, we should not process single-letter hotkeys (H, J, K, L, G, etc.)
    if ctx.wants_keyboard_input() {
        return;
    }

    handle_focus_entry(ctx, app, total_items, items_per_page);

    if app.nav.focused_index.is_some() && app.nav.focus_scope == crate::state::FocusScope::Grid {
        handle_navigation(ctx, app, total_items, items_per_page, cols);
        handle_item_interaction(ctx, app, items);
    }

    handle_scrolling(ctx, app, total_items, items_per_page);
    handle_global_shortcuts(ctx, app, items);
    handle_escape(ctx, app);
}

fn handle_focus_entry(
    ctx: &egui::Context,
    app: &mut MyApp,
    total_items: usize,
    items_per_page: usize,
) {
    // 1. Enter Grid Focus ('g' or 'Tab')
    if app.nav.focused_index.is_none()
        && ctx.input(|i| i.key_pressed(egui::Key::G) || i.key_pressed(egui::Key::Tab))
    {
        // Focus the first item on the current page
        let start_idx = app.grid.current_page * items_per_page;
        let target = start_idx.min(total_items.saturating_sub(1));
        app.nav.focused_index = Some(target);
    }
}

fn handle_navigation(
    ctx: &egui::Context,
    app: &mut MyApp,
    total_items: usize,
    items_per_page: usize,
    cols: usize,
) {
    let next_page = if ctx
        .input(|i| i.key_pressed(egui::Key::ArrowRight) || i.key_pressed(egui::Key::L))
    {
        let next = app
            .nav
            .focused_index
            .map_or(0, |i| (i + 1).min(total_items.saturating_sub(1)));
        app.nav.focused_index = Some(next);
        Some(next / items_per_page.max(1))
    } else if ctx.input(|i| i.key_pressed(egui::Key::ArrowLeft) || i.key_pressed(egui::Key::H)) {
        let prev = app.nav.focused_index.map_or(0, |i| i.saturating_sub(1));
        app.nav.focused_index = Some(prev);
        Some(prev / items_per_page.max(1))
    } else if ctx.input(|i| i.key_pressed(egui::Key::ArrowDown) || i.key_pressed(egui::Key::J)) {
        let next = app
            .nav
            .focused_index
            .map_or(0, |i| (i + cols).min(total_items.saturating_sub(1)));
        app.nav.focused_index = Some(next);
        Some(next / items_per_page.max(1))
    } else if ctx.input(|i| i.key_pressed(egui::Key::ArrowUp) || i.key_pressed(egui::Key::K)) {
        let prev = app.nav.focused_index.map_or(0, |i| i.saturating_sub(cols));
        app.nav.focused_index = Some(prev);
        Some(prev / items_per_page.max(1))
    } else if ctx.input(|i| i.key_pressed(egui::Key::Home)) {
        let next = 0;
        app.nav.focused_index = Some(next);
        Some(0)
    } else if ctx.input(|i| i.key_pressed(egui::Key::End)) {
        let next = total_items.saturating_sub(1);
        app.nav.focused_index = Some(next);
        Some(next / items_per_page.max(1))
    } else {
        None
    };

    if let Some(page) = next_page {
        if matches!(
            app.nav.view_mode,
            crate::state::ViewMode::Library | crate::state::ViewMode::Related(_)
        ) {
            app.grid.current_page = page;
        } else if matches!(
            app.nav.view_mode,
            crate::state::ViewMode::ModelDetails(_) | crate::state::ViewMode::StudioDetails(_)
        ) {
            // Only trigger reload causing None if page actually changed
            if app.model_studio.current_model_studio_page != page {
                app.model_studio.current_model_studio_page = page;
                // do NOT clear model_studio_items here, so we don't get a black frame
                // details.rs will detect page mismatch and trigger reload
            }
        }
    }
}

fn handle_item_interaction(ctx: &egui::Context, app: &mut MyApp, items: &[LibraryItem]) {
    // Helper to get local item from global index (Sliding Window aware)
    let get_local_item = |app: &MyApp, global_idx: usize| -> Option<&LibraryItem> {
        if global_idx >= app.grid.window_start_offset {
            let local_idx = global_idx - app.grid.window_start_offset;
            items.get(local_idx)
        } else {
            None
        }
    };

    // Enter to Play
    if ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
        if !app.nav.selected_ids.is_empty() {
            // For multi-selection, we might need valid items?
            // If selected items are not in current page, we might have issues playing them?
            // But existing logic seemingly used `app.nav.selected_ids`.
            // `app.play_items` likely expects full item objects.
            // If items are not loaded, we can't play them easily without fetching.
            // But for now, let's assume valid selection is visible.
            let ids: Vec<i64> = app.nav.selected_ids.iter().copied().collect();
            // We can only pass the items we HAVE.
            // If we select cross-page, `items` slice won't have them all.
            // This is a limitation of on-demand without persistent item cache.
            // For now, pass what we have.
            app.play_items(ids, items);
            app.nav.selected_ids.clear();
        } else if let Some(idx) = app.nav.focused_index
            && let Some(item) = get_local_item(app, idx)
        {
            app.play_items(vec![item.id], items);
            app.nav.selected_ids.clear();
        }
    }

    if ctx.input(|i| i.key_pressed(egui::Key::Space))
        && let Some(focused) = app.nav.focused_index
    {
        if ctx.input(|i| i.modifiers.shift) {
            let anchor = app.nav.last_clicked_index.unwrap_or(0);
            app.select_range(anchor, focused, items);
            app.nav.last_clicked_index = Some(focused);
        } else if let Some(item) = get_local_item(app, focused) {
            app.toggle_selection(item.id);
            app.nav.last_clicked_index = Some(focused);
        }
    }

    // 'r' to Open Related View
    if ctx.input(|i| i.key_pressed(egui::Key::R))
        && let Some(focused) = app.nav.focused_index
        && let Some(item) = get_local_item(app, focused)
    {
        app.open_page(ctx, item.id);
    }
}

fn handle_scrolling(
    ctx: &egui::Context,
    app: &mut MyApp,
    total_items: usize,
    items_per_page: usize,
) {
    // Scroll Wheel (Page Navigation)
    let scroll_delta = ctx.input(|i| i.raw_scroll_delta.y);
    let total_pages = (total_items + items_per_page - 1) / items_per_page.max(1);

    let current_page = if matches!(
        app.nav.view_mode,
        crate::state::ViewMode::Library | crate::state::ViewMode::Related(_)
    ) {
        app.grid.current_page
    } else {
        app.model_studio.current_model_studio_page
    };

    let mut new_page = current_page;
    let mut changed = false;

    // Threshold for page change (Low = Sensitive)
    if scroll_delta.abs() > 4.0 {
        if scroll_delta > 0.0 {
            // Scroll up - previous page
            if current_page > 0 {
                new_page -= 1;
                changed = true;
            }
        } else {
            // Scroll down - next page
            if current_page + 1 < total_pages {
                new_page += 1;
                changed = true;
            }
        }
    }

    if changed {
        // Update Focus if it was set
        if app.nav.focused_index.is_some() {
            let new_focus = (new_page * items_per_page).min(total_items.saturating_sub(1));
            app.nav.focused_index = Some(new_focus);
        }

        // Apply page change
        if matches!(
            app.nav.view_mode,
            crate::state::ViewMode::Library | crate::state::ViewMode::Related(_)
        ) {
            app.grid.current_page = new_page;

            ctx.request_repaint();
        } else if matches!(
            app.nav.view_mode,
            crate::state::ViewMode::ModelDetails(_) | crate::state::ViewMode::StudioDetails(_)
        ) {
            app.model_studio.current_model_studio_page = new_page;
            // do NOT clear model_studio_items here, so we don't get a black frame
            // details.rs will detect page mismatch and trigger reload
        }
    }
}

fn handle_global_shortcuts(ctx: &egui::Context, app: &mut MyApp, items: &[LibraryItem]) {
    // Ctrl+A
    if ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::A)) {
        app.select_all(items);
    }
    // Invert Selection
    if ctx.input(|i| {
        i.key_pressed(egui::Key::I) && !i.modifiers.ctrl && !i.modifiers.shift && !i.modifiers.alt
    }) {
        app.invert_selection(items);
    }

    // 's' to Scrape
    if ctx.input(|i| i.key_pressed(egui::Key::S)) {
        let urls_to_scrape: Vec<String> = if app.nav.selected_ids.is_empty() {
            // No selection - use focused item
            app.nav.focused_index
                .and_then(|idx| items.get(idx))
                .map(|item| vec![item.url.clone()])
                .unwrap_or_default()
        } else {
            // Scrape all selected items
            items
                .iter()
                .filter(|item| app.nav.selected_ids.contains(&item.id))
                .map(|item| item.url.clone())
                .collect()
        };

        if urls_to_scrape.len() == 1 {
            // Single URL - use individual scrape
            app.scrape.scrape_url = urls_to_scrape.into_iter().next().unwrap_or_default();
            app.trigger_scrape();
        } else if !urls_to_scrape.is_empty() {
            // Multiple URLs - use batch API for proper tracking
            app.trigger_scrape_urls(urls_to_scrape);
        }
    }
}

fn handle_escape(ctx: &egui::Context, app: &mut MyApp) {
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        app.nav.selected_ids.clear();
        app.nav.focused_index = None;
    }
}

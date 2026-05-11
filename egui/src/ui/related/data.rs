use crate::state::{MyApp, ViewMode};
use eframe::egui;
use soromantic_core::db::LibraryItem;

pub fn preload_images(ctx: &egui::Context, app: &mut MyApp) {
    let mut to_load = Vec::new();
    if let Some(page) = &app.nav.active_page {
        // Check Main Image
        if !app.images.textures.contains_key(&page.id)
            && !app.images.loading_ids.contains(&page.id)
            && let Some(path) = &page.local_image
        {
            to_load.push((page.id, std::path::PathBuf::from(path)));
        }
        // Check Grid Images (Limit to reasonable count e.g. 12)
        for g in page.grid.iter().take(12) {
            if let Some(rid) = g.related_id
                && !app.images.textures.contains_key(&rid)
                && !app.images.loading_ids.contains(&rid)
                && let Some(path) = &g.local_image
            {
                to_load.push((rid, std::path::PathBuf::from(path)));
            }
        }
    }
    for (id, path) in to_load {
        app.request_image_load(ctx, id, path);
    }
}

pub fn prepare_data(app: &MyApp) -> (Vec<LibraryItem>, Vec<LibraryItem>, i64) {
    // Extract grid items
    let grid_items: Vec<LibraryItem> = app.nav.active_page.as_ref().map_or(Vec::new(), |page| {
        page.grid
            .iter()
            .filter_map(|g| {
                g.related_id.map(|id| LibraryItem {
                    id,
                    title: g.title.clone(),
                    url: g.url.clone(),
                    image: g.image.clone(),
                    local_image: g.local_image.clone(),
                    finished_videos: g.finished_videos,
                    failed_videos: g.failed_videos,
                    local_preview: g.local_preview.clone(),
                    related_id: None,
                    ..Default::default()
                })
            })
            .collect()
    });

    // Extract Navigator Items
    let nav_items: Vec<LibraryItem> = if app.nav.browser_search.is_empty() {
        app.grid.items.clone()
    } else {
        app.nav.browser_results.clone()
    };

    let current_nav_id = if let ViewMode::Related(id) = app.nav.view_mode {
        id
    } else {
        0
    };

    (grid_items, nav_items, current_nav_id)
}

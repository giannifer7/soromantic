use crate::state::MyApp;

pub fn update_pagination(
    app: &mut MyApp,
    total_items: usize,
    items_per_page: usize,
) -> (usize, usize) {
    app.grid.items_per_page = items_per_page;
    app.grid.total_items = total_items;

    let start_idx = app.grid.current_page * app.grid.items_per_page;
    let start_idx = start_idx.min(app.grid.total_items);
    let end_idx = (start_idx + app.grid.items_per_page).min(app.grid.total_items);

    (start_idx, end_idx)
}

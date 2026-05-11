use eframe::egui;

pub struct LayoutMetrics {
    pub spacing: f32,
    pub cols: usize,
    pub col_width: f32,
    pub item_height: f32,
    pub row_height: f32,
    pub wide_layout: bool,
    pub grid_rows: usize,
}

pub fn calculate_layout(ui: &egui::Ui) -> LayoutMetrics {
    let spacing = ui.spacing().item_spacing.x;
    let avail_width = ui.available_width();
    let avail_height = ui.available_height();
    let cols = if avail_width >= 1200.0 {
        4
    } else if avail_width >= 900.0 {
        3
    } else if avail_width >= 600.0 {
        2
    } else {
        1
    };

    let cols_f = f32::from(u8::try_from(cols).unwrap_or(1));
    let col_width = spacing.mul_add(-(cols_f - 1.0), avail_width) / cols_f;
    let item_height = (col_width / (16.0 / 9.0)).ceil();
    let row_height = item_height + 24.0 + 5.0 + 8.0;

    // ROW 1: Main Image + Panel
    // Wide layout (cols >= 2): Image | Panel side-by-side
    // Narrow layout (cols == 1): Image above, Panel below

    let wide_layout = cols >= 2;

    // Calculate header height based on layout mode
    let header_height = if wide_layout {
        row_height // Image and panel side by side
    } else {
        // Narrow: image + gap + title + buttons_row + search_row + nav_list + gaps
        // image + 8 + 20 + 5 + 25 + 5 + 25 + 5 + 60 + 8 = image + 161
        item_height + 8.0 + 20.0 + 5.0 + 25.0 + 5.0 + 25.0 + 5.0 + 60.0 + 8.0
    };

    // Calculate available height for grid rows
    let grid_available_height = (avail_height - header_height - 8.0).max(0.0);

    let mut grid_rows = 1;
    if grid_available_height >= row_height * 3.0 {
        grid_rows = 3;
    } else if grid_available_height >= row_height * 2.0 {
        grid_rows = 2;
    }

    LayoutMetrics {
        spacing,
        cols,
        col_width,
        item_height,
        row_height,
        wide_layout,
        grid_rows,
    }
}

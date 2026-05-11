//! Grid rendering state: pagination, library items window, grid texture cache.

use super::{PendingLibraryData, LIBRARY_BATCH_SIZE};
use soromantic_core::db::LibraryItem;
use std::collections::HashMap;

/// State for the library grid: items, pagination, texture cache.
pub struct GridState {
    /// Number of items that fit on one page (cols × rows).
    pub items_per_page: usize,
    /// Number of items in the current loaded window (len of `items`).
    pub total_items: usize,
    /// Current page index (0-based).
    pub current_page: usize,
    /// Total number of items in the database (for pagination math).
    pub library_total_count: i64,
    /// Global index into the full library where the `items` window starts.
    pub window_start_offset: usize,
    /// Whether the library count is stale and needs refresh.
    pub library_dirty: bool,
    /// The currently loaded window of library items.
    pub items: Vec<LibraryItem>,
    /// Texture cache keyed by visual index within the current window
    /// (smooths pagination transitions before real textures arrive).
    pub grid_cache: HashMap<usize, eframe::egui::TextureHandle>,
    /// Channel to send pagination requests to the background worker.
    pub pagination_tx: tokio::sync::watch::Sender<(usize, bool)>,

    /// Pending library data from the pagination worker.
    pub pending_library_data: PendingLibraryData,
    /// Whether the full library count has been loaded after initial fast startup.
    pub library_fully_loaded: bool,
}

impl std::fmt::Debug for GridState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GridState")
            .field("items_per_page", &self.items_per_page)
            .field("total_items", &self.total_items)
            .field("current_page", &self.current_page)
            .field("library_total_count", &self.library_total_count)
            .field("window_start_offset", &self.window_start_offset)
            .field("library_dirty", &self.library_dirty)
            .field("items", &self.items.len())
            .field("grid_cache", &format!("{} entries", self.grid_cache.len()))
            .field("library_fully_loaded", &self.library_fully_loaded)
            .finish_non_exhaustive()
    }
}

impl GridState {
    /// Block size for the sliding window (aligns to [`LIBRARY_BATCH_SIZE`]).
    #[must_use]
    pub const fn block_size() -> usize {
        LIBRARY_BATCH_SIZE
    }
}

use crate::db::{Database, LibraryItem};
use std::path::Path;

/// Load initial library items for startup.
///
/// Strategy:
/// 1. Try loading from disk cache (fastest, < 10ms).
/// 2. If no cache, fall back to fast DB query (no complex JOINs, ~50ms).
///    Loads up to 30 items to populate the initial view.
/// 3. Returns the list of items to display immediately.
pub async fn load_initial_items(db: &Database, cache_dir: &Path) -> Vec<LibraryItem> {
    if let Some(cached) = crate::cache::load_library_cache(cache_dir) {
        if std::env::var("RUST_LOG").is_ok() {
            tracing::info!("Fast startup from cache: {} items", cached.len());
        }
        cached
    } else {
        // No cache - use paginated query (skip_count=true for speed)
        if std::env::var("RUST_LOG").is_ok() {
            tracing::info!("No cache, using paginated DB query");
        }
        db.get_library_paginated(0, 30, true)
            .await
            .map(|(items, _)| items)
            .unwrap_or_default()
    }
}

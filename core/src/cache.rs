use crate::db::LibraryItem;
use anyhow::{Context, Result};
use std::path::Path;

/// Load the startup cache from disk.
/// Returns `Some(items)` if successful, `None` if file missing or error.
pub fn load_library_cache(cache_dir: &Path) -> Option<Vec<LibraryItem>> {
    let cache_path = cache_dir.join("library_cache.json");

    if !cache_path.exists() {
        return None;
    }

    match std::fs::read_to_string(&cache_path) {
        Ok(json) => match serde_json::from_str::<Vec<LibraryItem>>(&json) {
            Ok(items) => {
                tracing::debug!(
                    "Loaded {} items from cache at {:?}",
                    items.len(),
                    cache_path
                );
                Some(items)
            }
            Err(e) => {
                tracing::warn!("Failed to parse library cache: {}", e);
                None
            }
        },
        Err(e) => {
            tracing::warn!("Failed to read library cache file: {}", e);
            None
        }
    }
}

/// Write the first 30 library items to disk for fast startup.
/// Ensures the cache stays small and fast.
///
/// # Errors
/// Returns error if JSON serialization fails or if the cache file cannot be written.
pub fn write_library_cache(cache_dir: &Path, items: &[LibraryItem]) -> Result<()> {
    if items.is_empty() {
        return Ok(());
    }

    let cache_items: Vec<_> = items.iter().take(30).collect();
    let cache_path = cache_dir.join("library_cache.json");

    let json = serde_json::to_string(&cache_items).context("Failed to serialize library cache")?;
    std::fs::write(&cache_path, json).context("Failed to write library cache file")?;

    tracing::debug!("Updated library cache at {:?}", cache_path);
    Ok(())
}

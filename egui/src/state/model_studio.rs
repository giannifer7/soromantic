//! Model/Studio detail state: performer/studio lists and detail-page items.

use super::PendingModelStudioItems;
use parking_lot::Mutex;
use soromantic_core::db::{LibraryItem, PerformerItem, StudioItem};
use std::sync::Arc;

/// State for performer and studio detail views.
#[derive(Debug)]
pub struct ModelStudioState {
    /// Pending performer list from async query.
    pub pending_models: Arc<Mutex<Option<Vec<PerformerItem>>>>,
    /// Pending studio list from async query.
    pub pending_studios: Arc<Mutex<Option<Vec<StudioItem>>>>,
    /// Cached performer list.
    pub models: Option<Vec<PerformerItem>>,
    /// Cached studio list.
    pub studios: Option<Vec<StudioItem>>,
    /// Items for the current model/studio detail grid.
    pub model_studio_items: Option<Vec<LibraryItem>>,
    /// Pending model/studio detail items from async query.
    pub pending_model_studio_items: PendingModelStudioItems,
    /// Current page index for model/studio detail view.
    pub current_model_studio_page: usize,
    /// Which page the currently loaded items correspond to.
    pub loaded_model_studio_page: usize,
    /// Total item count for the current model/studio detail view.
    pub model_studio_total_count: i64,
    /// Metadata URLs associated with the current model/studio.
    pub model_studio_urls: Vec<String>,
    /// Whether a model/studio detail load is in-flight.
    pub is_loading_model_studio: bool,
}

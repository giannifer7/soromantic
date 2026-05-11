//! Image state: texture cache, preview cache, pending upload queue.

use crate::data::{LoadedImage, PendingImage, PreviewFrames};
use parking_lot::Mutex;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// State for image loading and texture management.
pub struct ImageState {
    /// Texture cache: item id → GPU texture handle.
    pub textures: HashMap<i64, LoadedImage>,
    /// Preview frame cache: item id → sequence of preview textures.
    pub preview_cache: HashMap<i64, PreviewFrames>,
    /// IDs currently being loaded (prevents duplicate requests).
    pub loading_ids: HashSet<i64>,
    /// Queue of images waiting to be uploaded to the GPU from background threads.
    pub pending_images: Arc<Mutex<Vec<PendingImage>>>,
    /// Maximum number of textures to upload per frame (prevents stutter).
    pub texture_upload_limit: usize,
}

impl std::fmt::Debug for ImageState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImageState")
            .field("textures", &format!("{} entries", self.textures.len()))
            .field("preview_cache", &format!("{} entries", self.preview_cache.len()))
            .field("loading_ids", &self.loading_ids)
            .field("texture_upload_limit", &self.texture_upload_limit)
            .finish_non_exhaustive()
    }
}

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoEntry<ID> {
    pub id: ID,
    pub title: String,
    pub url: String,
    pub image: Option<String>,
    pub local_image: Option<String>,
    pub preview_url: Option<String>,
    pub local_preview: Option<String>,
    pub finished_videos: i64,
    pub failed_videos: i64,
    pub related_id: Option<i64>,
}

impl<ID> Default for VideoEntry<ID>
where
    ID: Default,
{
    fn default() -> Self {
        Self {
            id: Default::default(),
            title: String::new(),
            url: String::new(),
            image: None,
            local_image: None,
            preview_url: None,
            local_preview: None,
            finished_videos: 0,
            failed_videos: 0,
            related_id: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaylistItem {
    pub path: String,
    pub title: String,
    pub intervals: Option<Vec<(f64, f64)>>, // Start, End (in seconds)
}

use crate::types::VideoEntry;

pub type LibraryItem = VideoEntry<i64>;

#[derive(Debug, Clone)]
pub struct PageData {
    pub id: i64,
    pub url: String,
    pub title: String,
    pub image: Option<String>,
    pub local_image: Option<String>,
    pub cover_status: i64,
    pub studio: Option<String>,
    pub local_video_path: Option<String>,
    pub thumb_status: i64,
    pub preview_status: i64,
    pub video_status: i64,
    // FK fields
    pub studio_id: Option<i64>,
    pub site_id: Option<i64>,
    pub grid: Vec<GridItem>,
}

pub type GridItem = VideoEntry<Option<i64>>;

#[derive(Debug, Clone)]
pub struct VideoSource {
    pub local_path: String,
    pub resolution: Option<i64>,
    pub start_time: Option<f64>,
    pub stop_time: Option<f64>,
}

// ============================================================
// Rompla schema types
// ============================================================

/// A performer (model) from the performers table.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PerformerItem {
    pub id: i64,
    pub name: String,
    pub star: i32,
    pub sex: i32,
    pub birth_year: Option<i32>,
    pub aliases: Option<String>,
    pub thumb_status: i64,
    pub nation_id: Option<i64>,
    /// Video count across all linked pages
    pub count: i64,
}

/// A studio from the studios table.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StudioItem {
    pub id: i64,
    pub name: String,
    pub url: String,
    pub count: i64,
}

/// A nation (country flag) from the nations table.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NationItem {
    pub id: i64,
    pub code: String,
    pub name: Option<String>,
    pub flag_status: i64,
}

/// A site from the sites table.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SiteItem {
    pub id: i64,
    pub name: String,
    pub url_prefix: Option<String>,
    pub scraper: Option<String>,
}

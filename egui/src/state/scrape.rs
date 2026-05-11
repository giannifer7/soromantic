//! Scrape state: scrape window UI, progress tracking, batch manager.

use super::{BatchProgress, ScrapeProgress};
use soromantic_core::batch::BatchManager;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};

/// State for scraping: window visibility, progress, batch tracking.
pub struct ScrapeState {
    /// Whether the inline scrape panel is visible.
    pub show_scrape_window: bool,
    /// Request focus on the scrape URL text input (one-shot flag).
    pub focus_scrape_input: bool,
    /// URL(s) entered by the user (newline-separated).
    pub scrape_url: String,
    /// Current status message shown in the footer.
    pub scrape_status: String,
    /// Title of the page currently being scraped.
    pub scrape_title: String,
    /// Set of page IDs currently being scraped.
    pub active_scrapes: HashSet<i64>,
    /// Set of page IDs whose scrape failed.
    pub failed_scrapes: HashSet<i64>,
    /// Batch progress (current / total pages).
    pub batch_progress: Option<BatchProgress>,
    /// Single-scrape progress with animation state.
    pub scrape_progress: Option<ScrapeProgress>,
    /// Whether repair has been triggered (prevents double-trigger).
    pub repair_triggered: bool,
    /// The batch manager that handles scraping workflows.
    pub batch_manager: Arc<Mutex<BatchManager>>,
}

impl std::fmt::Debug for ScrapeState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ScrapeState")
            .field("show_scrape_window", &self.show_scrape_window)
            .field("scrape_url", &self.scrape_url)
            .field("scrape_status", &self.scrape_status)
            .field("active_scrapes", &self.active_scrapes)
            .field("failed_scrapes", &self.failed_scrapes)
            .field("batch_progress", &self.batch_progress)
            .field("scrape_progress", &self.scrape_progress)
            .field("repair_triggered", &self.repair_triggered)
            .finish_non_exhaustive()
    }
}

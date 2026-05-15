use parking_lot::Mutex;
use std::path::PathBuf;
use std::sync::Arc;

use soromantic_core::db::{Database, LibraryItem};
use soromantic_core::mpv::MpvClient;

pub mod grid;
pub mod images;
pub mod model_studio;
pub mod nav;
pub mod scrape;

pub use grid::GridState;
pub use images::ImageState;
pub use model_studio::ModelStudioState;
pub use nav::NavigationState;
pub use scrape::ScrapeState;

pub mod actions;
pub mod async_tasks;
pub mod init;
pub mod navigation;
pub mod selection;

pub use soromantic_core::ui::ViewMode;

/// Type alias for pending library data: (items, `total_count`, `window_start_offset`)
pub type PendingLibraryData = Arc<Mutex<Option<(Vec<LibraryItem>, i64, usize)>>>;
/// Type alias for pending model/studio details data: (items, `total_count`, `metadata_urls`, `page_index`)
pub type PendingModelStudioItems = Arc<Mutex<Option<(Vec<LibraryItem>, i64, Vec<String>, usize)>>>;

/// Global constant for library data window size.
pub const LIBRARY_BATCH_SIZE: usize = 350;
/// Global constant for model/studio detail window size.
pub const MODEL_STUDIO_BATCH_SIZE: usize = 100;

#[derive(PartialEq, Eq, Clone, Copy, Debug, Default)]
pub enum FocusScope {
    #[default]
    Grid,
    NavList,
    Search,
    HeaderButtons,
    Footer,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum FooterAction {
    #[default]
    Library,
    Scrape,
    Repair,
    PlaySelected,
    FirstPage,
    PrevPage,
    NextPage,
    LastPage,
}

impl FooterAction {
    #[must_use]
    pub const fn next(&self) -> Self {
        match self {
            Self::Library => Self::Scrape,
            Self::Scrape => Self::Repair,
            Self::Repair => Self::PlaySelected,
            Self::PlaySelected => Self::FirstPage,
            Self::FirstPage => Self::PrevPage,
            Self::PrevPage => Self::NextPage,
            Self::NextPage | Self::LastPage => Self::LastPage,
        }
    }

    #[must_use]
    pub const fn prev(&self) -> Self {
        match self {
            Self::Library | Self::Scrape => Self::Library,
            Self::Repair => Self::Scrape,
            Self::PlaySelected => Self::Repair,
            Self::FirstPage => Self::PlaySelected,
            Self::PrevPage => Self::FirstPage,
            Self::NextPage => Self::PrevPage,
            Self::LastPage => Self::NextPage,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum HeaderAction {
    #[default]
    Back,
    Play,
}

impl HeaderAction {
    #[must_use]
    pub const fn next(&self) -> Self {
        match self {
            Self::Back | Self::Play => Self::Play,
        }
    }

    #[must_use]
    pub const fn prev(&self) -> Self {
        match self {
            Self::Back | Self::Play => Self::Back,
        }
    }
}

#[derive(Clone, Debug)]
pub struct BatchProgress {
    pub current: usize,
    pub total: usize,
}

#[derive(Clone, Debug)]
pub struct ScrapeProgress {
    pub page_id: i64,
    pub stage: String,
    pub progress: u64,
    pub total: u64,
    pub message: String,
    pub displayed_progress: f32,
}

/// Central application state.
///
/// Shared infrastructure fields stay here; domain state is delegated to sub-structs.
pub struct MyApp {
    // ── Infrastructure (shared across all sub-systems) ──
    pub mpv: Arc<MpvClient>,
    pub db: Arc<Database>,
    pub cache_dir: PathBuf,
    pub previews_dir: PathBuf,
    pub frames_dir: PathBuf,
    pub ffmpeg_path: PathBuf,
    pub batch_list_path: PathBuf,
    pub ui_config: soromantic_core::config::UIConfig,
    pub playback_config: soromantic_core::config::PlaybackConfig,
    pub startup_time: std::time::Instant,
    pub first_frame_rendered: bool,
    pub server_events: std::sync::mpsc::Receiver<crate::server::InternalEvent>,
    pub rt_handle: tokio::runtime::Handle,

    // ── Domain sub-structs ──
    pub grid: GridState,
    pub images: ImageState,
    pub nav: NavigationState,
    pub scrape: ScrapeState,
    pub model_studio: ModelStudioState,
}

use super::{FocusScope, GridState, ImageState, ModelStudioState, MyApp, NavigationState, ScrapeState, ViewMode};
use parking_lot::Mutex;
use soromantic_core::db::{Database, LibraryItem};
use soromantic_core::mpv::MpvClient;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

impl MyApp {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        items: Vec<LibraryItem>,
        mpv: Arc<MpvClient>,
        db: Arc<Database>,
        cache_dir: PathBuf,
        previews_dir: PathBuf,
        batch_list_path: &std::path::Path,
        event_rx: std::sync::mpsc::Receiver<crate::server::InternalEvent>,
        ui_config: soromantic_core::config::UIConfig,
        playback_config: soromantic_core::config::PlaybackConfig,
        batch_manager: Arc<std::sync::Mutex<soromantic_core::batch::BatchManager>>,
        startup_time: std::time::Instant,
        rt_handle: tokio::runtime::Handle,
        texture_upload_limit: usize,
        ctx: eframe::egui::Context,
    ) -> Self {
        // Initialize Pagination Background Worker
        let (pag_tx, pag_rx) = tokio::sync::watch::channel((0usize, false));
        let pending_lib = Arc::new(Mutex::new(None));

        // Copy values from ui_config before it's moved into MyApp
        let items_per_page = ui_config.items_per_page;

        spawn_pagination_worker(&rt_handle, db.clone(), pending_lib.clone(), pag_rx, ctx);

        Self {
            // ── Infrastructure ──
            mpv,
            db,
            cache_dir,
            previews_dir,
            batch_list_path: batch_list_path.to_path_buf(),
            ui_config,
            playback_config,
            startup_time,
            first_frame_rendered: false,
            server_events: event_rx,
            rt_handle,

            // ── Domain sub-structs ──
            grid: GridState {
                items_per_page,
                total_items: 0,
                current_page: 0,
                library_total_count: 0,
                window_start_offset: 0,
                library_dirty: true,
                items,
                grid_cache: HashMap::new(),
                pagination_tx: pag_tx,
                pending_library_data: pending_lib,
                library_fully_loaded: false,
            },
            images: ImageState {
                textures: HashMap::new(),
                preview_cache: HashMap::new(),
                loading_ids: HashSet::new(),
                pending_images: Arc::new(Mutex::new(Vec::new())),
                texture_upload_limit,
            },
            nav: NavigationState {
                view_mode: ViewMode::Library,
                focus_scope: FocusScope::default(),
                active_page: None,
                navigation_stack: Vec::new(),
                browser_search: String::new(),
                browser_results: Vec::new(),
                focused_index: Some(0), // Auto-focus first item at startup
                last_clicked_index: None,
                selected_ids: HashSet::new(),
                footer_focus: super::FooterAction::default(),
                header_focus: super::HeaderAction::default(),
                pending_page_data: Arc::new(Mutex::new(None)),
                pending_search_results: Arc::new(Mutex::new(Vec::new())),
            },
            scrape: ScrapeState {
                show_scrape_window: false,
                focus_scrape_input: false,
                scrape_url: String::new(),
                scrape_status: "Ready".to_string(),
                scrape_title: String::new(),
                active_scrapes: HashSet::new(),
                failed_scrapes: HashSet::new(),
                batch_progress: None,
                scrape_progress: None,
                repair_triggered: false,
                batch_manager,
            },
            model_studio: ModelStudioState {
                pending_models: Arc::new(Mutex::new(None)),
                pending_studios: Arc::new(Mutex::new(None)),
                models: None,
                studios: None,
                model_studio_items: None,
                pending_model_studio_items: Arc::new(parking_lot::Mutex::new(None)),
                current_model_studio_page: 0,
                loaded_model_studio_page: 0,
                model_studio_total_count: 0,
                model_studio_urls: Vec::new(),
                is_loading_model_studio: false,
            },
        }
    }
}

fn spawn_pagination_worker(
    rt_handle: &tokio::runtime::Handle,
    db: Arc<Database>,
    pending_lib: crate::state::PendingLibraryData,
    mut pag_rx: tokio::sync::watch::Receiver<(usize, bool)>,
    ctx: eframe::egui::Context,
) {
    rt_handle.spawn(async move {
        loop {
            // Get latest requested center item index immediately (and mark seen)
            let (requested_index, skip_count) = *pag_rx.borrow_and_update();

            let batch_size_i64 = i64::try_from(crate::state::LIBRARY_BATCH_SIZE).unwrap_or(350);
            let block_index = i64::try_from(requested_index).unwrap_or(0) / batch_size_i64;
            let offset = block_index * batch_size_i64;

            match db
                .get_library_paginated(offset, batch_size_i64, skip_count)
                .await
            {
                Ok((items, total_count)) => {
                    let filtered: Vec<_> = items
                        .into_iter()
                        .filter(|item| item.finished_videos > 0)
                        .collect();

                    *pending_lib.lock() =
                        Some((filtered, total_count, usize::try_from(offset).unwrap_or(0)));

                    ctx.request_repaint();
                }
                Err(e) => {
                    tracing::error!("Failed to load block for index {}: {}", requested_index, e);
                }
            }

            // Wait for NEXT change notification
            if pag_rx.changed().await.is_err() {
                break; // Sender dropped
            }
        }
    });
}

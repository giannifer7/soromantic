#![deny(warnings)]

use std::sync::Arc;

use anyhow::Context;
use eframe::egui;
use gumdrop::Options;
use soromantic_core::config::{ConfigStatus, load_config};
use soromantic_core::db::{Database, LibraryItem};
use soromantic_core::mpv::MpvClient;

pub mod app;
mod data;
pub mod server;
pub mod state;
mod ui;

use state::MyApp;

#[derive(Debug, Options)]
struct AppOptions {
    #[options(help = "print help message")]
    help: bool,

    #[options(help = "path to configuration file")]
    config: Option<String>,

    #[options(help = "path to batch list file (overrides config)")]
    batch_file: Option<String>,
}

/// Run the application.
///
/// # Errors
/// Returns error if runtime, database, or UI initialization fails.
///
/// # Panics
/// Panics if configuration loading, MPV daemon startup, or Tokio runtime build fails.
#[allow(clippy::too_many_lines)]
pub fn run() -> anyhow::Result<()> {
    // Parse CLI args
    let opts = AppOptions::parse_args_default_or_exit();

    let startup_start = std::time::Instant::now();

    // Only initialize logging if RUST_LOG is set, to minimize overhead
    // Initialize logging default if not set, to ensure rich tracing is active
    if std::env::var("RUST_LOG").is_err() {
        // SAFETY: Safe because this is the main thread at startup, before other threads are spawned.
        unsafe {
            std::env::set_var("RUST_LOG", "info");
        }
    }
    tracing_subscriber::fmt::init();

    // Load config
    let mut cfg = match load_config(opts.config.as_deref()).context("Failed to load config")? {
        ConfigStatus::Loaded(c) => *c,
        ConfigStatus::Created(path) => {
            eprintln!("-------------------------------------------------------");
            eprintln!("Configuration created at: {}", path.display());
            eprintln!("Please review and edit the file to customize your settings.");
            eprintln!("Run the application again to start.");
            eprintln!("-------------------------------------------------------");
            return Ok(());
        }
    };

    // ── Config validation ──
    // Clamp to safe ranges to prevent division-by-zero or excessive GPU uploads
    cfg.ui.items_per_page = cfg.ui.items_per_page.clamp(1, 500);
    if cfg.ui.texture_upload_limit_egui == 0 {
        cfg.ui.texture_upload_limit_egui = 50; // sensible default
    } else {
        cfg.ui.texture_upload_limit_egui = cfg.ui.texture_upload_limit_egui.clamp(1, 200);
    }
    // ── End config validation ──

    // CLI overrides
    if let Some(batch_path) = opts.batch_file {
        if std::env::var("RUST_LOG").is_ok() {
            tracing::info!("Overriding batch list path from CLI: {}", batch_path);
        }
        cfg.batch_list_path = std::path::PathBuf::from(batch_path);
    }

    tracing::info!("── config paths ──");
    tracing::info!("  data (download_dir) : {}", cfg.download_dir.display());
    tracing::info!("  db_path             : {}", cfg.db_path.display());
    tracing::info!("  cache_dir           : {}", cfg.cache_dir.display());
    tracing::info!("  frames_dir          : {}", cfg.frames_dir.display());
    tracing::info!("  previews_dir        : {}", cfg.previews_dir.display());
    tracing::info!("  thumbs_dir          : {}", cfg.thumbs_dir.display());
    tracing::info!("  videos_dir          : {}", cfg.videos_dir.display());
    tracing::info!("  scrapers_dir        : {}", cfg.scrapers_dir.display());
    tracing::info!("  scripts_dir         : {}", cfg.scripts_dir.display());
    tracing::info!("  ffmpeg              : {}", cfg.ffmpeg_path.display());
    tracing::info!("  ffprobe             : {}", cfg.ffprobe_path.display());
    if let Some(ref orig) = cfg.orig_db_path {
        tracing::info!("  orig_db_path        : {}", orig.display());
    }
    tracing::info!("── end config ──");

    // Initialize mpv client
    let mpv = Arc::new(MpvClient::new_unix(
        cfg.mpv_socket.to_string_lossy().to_string(),
        cfg.timeouts.mpv_socket_connect,
        cfg.timeouts.mpv_socket_command,
    ));
    mpv.start_daemon(vec![])
        .context("Failed to start mpv daemon")?;

    if std::env::var("RUST_LOG").is_ok() {
        tracing::info!("mpv daemon started");
    }

    // Initialize Tokio runtime for async tasks
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("Failed to build Tokio runtime")?;

    // Connect to database
    let db = Arc::new(
        rt.block_on(async { Database::new(cfg.clone()).await })
            .context("Failed to connect to database")?,
    );

    let items: Vec<LibraryItem> = rt.block_on(async {
        soromantic_core::startup::load_initial_items(&db, &cfg.cache_dir).await
    });

    let startup_duration = startup_start.elapsed();
    eprintln!("Startup took: {startup_duration:.2?} (Ready to show window)");

    if std::env::var("RUST_LOG").is_ok() {
        tracing::info!("Startup took: {:.2?}", startup_duration);
        tracing::info!("Fast startup: loaded {} items", items.len());
        let preview_count = items.iter().filter(|i| i.local_preview.is_some()).count();
        tracing::info!(
            "Items with preview: {preview_count}/{}",
            items.len()
        );
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_maximized(true)
            .with_title("Soromantic"),
        ..Default::default()
    };

    // Event channel for BatchManager -> UI
    let (event_tx, event_rx) = std::sync::mpsc::channel::<crate::server::InternalEvent>();

    // Initialize BatchManager with callback that sends to the channel
    let on_progress = Arc::new(move |json: serde_json::Value| {
        let _ = event_tx.send(crate::server::InternalEvent::BatchEvent(json));
    }) as soromantic_core::downloader::ProgressCallback;

    let batch_manager = Arc::new(std::sync::Mutex::new(
        soromantic_core::batch::BatchManager::new(
            db.clone(),
            cfg.clone(),
            Some(on_progress),
            rt.handle().clone(),
        ),
    ));

    let cache_dir = cfg.cache_dir.clone();
    let rt_handle = rt.handle().clone();

    eframe::run_native(
        "Soromantic",
        options,
        Box::new(move |cc| {
            Ok(Box::new(MyApp::new(
                items.clone(),
                mpv.clone(),
                db.clone(),
                cache_dir,
                cfg.previews_dir.clone(),
                cfg.frames_dir.clone(),
                cfg.ffmpeg_path.clone(),
                &cfg.batch_list_path,
                event_rx,
                cfg.ui.clone(),
                cfg.playback.clone(),
                batch_manager,
                startup_start,
                rt_handle,
                cfg.ui.texture_upload_limit_egui,
                cc.egui_ctx.clone(),
            )))
        }),
    )
    .map_err(|e| anyhow::anyhow!("Eframe error: {e}"))?;

    Ok(())
}

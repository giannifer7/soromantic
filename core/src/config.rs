//! Configuration loader for soromantic server.
//! Reads from ~/.config/soromantic/config.toml

use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::PathBuf;

pub const DEFAULT_DB_BUSY_TIMEOUT_MS: u64 = 5000;
pub const DEFAULT_DB_RETRY_INTERVAL_SECS: f64 = 1.0;
pub const DEFAULT_HTTP_TIMEOUT_SECS: f64 = 5.0;
pub const DEFAULT_MP_CONNECT_TIMEOUT_SECS: f64 = 5.0;
pub const DEFAULT_MP_COMMAND_TIMEOUT_SECS: f64 = 2.0;

pub const DEFAULT_ITEMS_PER_PAGE: usize = 50;
pub const DEFAULT_DB_BATCH_SIZE: i64 = 500;
pub const DEFAULT_THUMBNAIL_WIDTH: u32 = 400;
pub const DEFAULT_THUMBNAIL_HEIGHT: u32 = 225;
pub const DEFAULT_SEARCH_LIMIT: i64 = 50;

pub const DEFAULT_DOWNLOAD_DELAY_MS: u64 = 0;
pub const DEFAULT_DOWNLOAD_TIMEOUT_SECS: u64 = 60;
pub const DEFAULT_MAX_RETRIES: u32 = 3;

/// Get platform-specific default config path
fn get_default_config_path() -> PathBuf {
    directories::ProjectDirs::from("", "", "soromantic").map_or_else(
        || PathBuf::from("config.toml"),
        |dirs| dirs.config_dir().join("config.toml"),
    )
}

#[derive(Debug, Deserialize, Clone, Copy)]
#[serde(default)]
pub struct TimeoutsConfig {
    pub mpv_socket_connect: f64,
    pub mpv_socket_command: f64,
    pub http_request: f64,
    pub db_connect: f64,
    pub db_busy: u64,
    pub db_retry_interval: f64,
}

impl Default for TimeoutsConfig {
    fn default() -> Self {
        Self {
            mpv_socket_connect: DEFAULT_MP_CONNECT_TIMEOUT_SECS,
            mpv_socket_command: DEFAULT_MP_COMMAND_TIMEOUT_SECS,
            http_request: DEFAULT_HTTP_TIMEOUT_SECS,
            db_connect: DEFAULT_HTTP_TIMEOUT_SECS,
            db_busy: DEFAULT_DB_BUSY_TIMEOUT_MS,
            db_retry_interval: DEFAULT_DB_RETRY_INTERVAL_SECS,
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
#[serde(default)]
pub struct PlaybackConfig {
    pub video_preferences: Vec<i64>,
    /// Seek amount in seconds when using mouse wheel on the seek bar.
    #[serde(default = "default_wheel_seek_amount")]
    pub wheel_seek_amount: f64,
    /// Default volume on app startup (0-100).
    #[serde(default = "default_initial_volume")]
    pub initial_volume: f64,
    /// Volume adjustment step (0-100).
    #[serde(default = "default_volume_step")]
    pub volume_step: f64,
}

const fn default_wheel_seek_amount() -> f64 {
    5.0
}

const fn default_initial_volume() -> f64 {
    70.0
}

const fn default_volume_step() -> f64 {
    5.0
}

impl Default for PlaybackConfig {
    fn default() -> Self {
        Self {
            video_preferences: vec![480, 720],
            wheel_seek_amount: default_wheel_seek_amount(),
            initial_volume: default_initial_volume(),
            volume_step: default_volume_step(),
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
#[serde(default)]
pub struct UIConfig {
    pub items_per_page: usize,
    pub db_batch_size: i64,
    pub thumbnail_width: u32,
    pub thumbnail_height: u32,
    pub search_limit: i64,
    #[serde(default = "default_texture_upload_limit_egui")]
    pub texture_upload_limit_egui: usize,
    #[serde(default = "default_texture_upload_limit_fltk")]
    pub texture_upload_limit_fltk: usize,
    #[serde(default = "default_show_footer")]
    pub show_footer: bool,
}

const fn default_texture_upload_limit_egui() -> usize {
    50
}

const fn default_texture_upload_limit_fltk() -> usize {
    5
}

const fn default_show_footer() -> bool {
    true
}

impl Default for UIConfig {
    fn default() -> Self {
        Self {
            items_per_page: DEFAULT_ITEMS_PER_PAGE,
            db_batch_size: DEFAULT_DB_BATCH_SIZE,
            thumbnail_width: DEFAULT_THUMBNAIL_WIDTH,
            thumbnail_height: DEFAULT_THUMBNAIL_HEIGHT,
            search_limit: DEFAULT_SEARCH_LIMIT,
            texture_upload_limit_egui: default_texture_upload_limit_egui(),
            texture_upload_limit_fltk: default_texture_upload_limit_fltk(),
            show_footer: default_show_footer(),
        }
    }
}

/// Rompla-compatible [paths] section.
///
/// When `data` is set, all subdirectories (thumbs, covers, videos,
/// previews, flags) are re-derived from it, matching rompla's
/// behavior. Individual overrides still take priority.
#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct PathsConfig {
    /// Base data directory — all media subdirs derive from this when set.
    pub data: Option<String>,
    /// Cache directory (defaults to XDG cache).
    pub cache: Option<String>,
    /// Frame extraction cache (defaults to cache/frames).
    pub frames_cache: Option<String>,
    /// Scrapers directory.
    pub scrapers: Option<String>,
    /// Scripts directory.
    pub scripts: Option<String>,
    /// Original database path for migration tools.
    pub orig_database: Option<String>,
    /// Database path override.
    pub database: Option<String>,
    /// Schema file path.
    pub schema: Option<String>,
    // Individual subdirectory overrides
    pub thumbs: Option<String>,
    pub covers: Option<String>,
    pub videos: Option<String>,
    pub previews: Option<String>,
    pub models: Option<String>,
    pub performers: Option<String>,
    pub flags: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct Config {
    pub paths: PathsConfig,
    pub runtime: RuntimeConfig,
    pub timeouts: TimeoutsConfig,
    pub ui: UIConfig,
    pub playback: PlaybackConfig,
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct RuntimeConfig {
    pub output_dir: String,
    pub db_path: String,
    pub download_dir: String,
    pub cache_dir: Option<String>,
    /// Enable verbose debug output (matches rompla's `ROMPLA_DEBUG` env var).
    #[serde(default)]
    pub debug: bool,
    #[serde(default = "default_batch_list_path")]
    pub batch_list_path: String,
    #[serde(default = "default_ffmpeg_binary")]
    pub ffmpeg_binary: String,
    #[serde(default = "default_ffprobe_binary")]
    pub ffprobe_binary: String,

    // New configurable paths
    pub thumbs_dir: Option<String>,
    pub covers_dir: Option<String>,
    pub videos_dir: Option<String>,
    pub previews_dir: Option<String>,
    pub frames_dir: Option<String>,
    pub models_dir: Option<String>,
    pub flags_dir: Option<String>,
    pub scrapers_dir: Option<String>,

    // Scraper settings
    pub download_delay_ms: Option<u64>,
}

fn default_batch_list_path() -> String {
    "scraped_urls.txt".to_string()
}

fn default_ffmpeg_binary() -> String {
    "ffmpeg".to_string()
}

fn default_ffprobe_binary() -> String {
    "ffprobe".to_string()
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        // Use platform-specific local data dir
        // Linux: ~/.local/share/soromantic
        // Windows: %LOCALAPPDATA%/soromantic (data_local_dir)
        let base_dir = directories::ProjectDirs::from("", "", "soromantic").map_or_else(
            || PathBuf::from("output"),
            |dirs| dirs.data_local_dir().to_path_buf(),
        );

        let base_dir_str = base_dir.to_string_lossy().to_string();

        Self {
            output_dir: base_dir_str,
            db_path: base_dir
                .join("db")
                .join("data.db")
                .to_string_lossy()
                .to_string(),
            download_dir: base_dir.join("downloads").to_string_lossy().to_string(),
            cache_dir: None,
            debug: false,
            batch_list_path: default_batch_list_path(),
            ffmpeg_binary: default_ffmpeg_binary(),
            ffprobe_binary: default_ffprobe_binary(),
            thumbs_dir: None,
            covers_dir: None,
            videos_dir: None,
            previews_dir: None,
            frames_dir: None,
            models_dir: None,
            flags_dir: None,
            scrapers_dir: Some(
                base_dir
                    .join("assets")
                    .join("scrapers")
                    .to_string_lossy()
                    .to_string(),
            ),
            download_delay_ms: None,
        }
    }
}

/// Expand ~ and environment variables in a path string
fn expand_path(path: &str) -> PathBuf {
    let expanded = path.strip_prefix("~/").map_or_else(
        || PathBuf::from(shellexpand::tilde(path).into_owned()),
        |stripped| {
            directories::UserDirs::new().map_or_else(
                || PathBuf::from(path),
                |user_dirs| user_dirs.home_dir().join(stripped),
            )
        },
    );

    // Make relative paths absolute from current dir
    if expanded.is_relative() {
        std::env::current_dir().unwrap_or_default().join(expanded)
    } else {
        expanded
    }
}

/// Resolved configuration with absolute paths
#[derive(Debug, Clone)]
pub struct ResolvedConfig {
    pub config_file_path: Option<PathBuf>,
    pub db_path: PathBuf,
    pub download_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub frontend_dir: PathBuf,
    pub mpv_socket: PathBuf,
    pub batch_list_path: PathBuf,
    pub ffmpeg_path: PathBuf,
    pub ffprobe_path: PathBuf,
    pub timeouts: TimeoutsConfig,
    pub ui: UIConfig,
    pub playback: PlaybackConfig,

    // Resolved specific dirs
    pub thumbs_dir: PathBuf,
    pub covers_dir: PathBuf,
    pub videos_dir: PathBuf,
    pub previews_dir: PathBuf,
    pub frames_dir: PathBuf,
    pub models_dir: PathBuf,
    pub flags_dir: PathBuf,
    pub scrapers_dir: PathBuf,
    pub scripts_dir: PathBuf,
    pub orig_db_path: Option<PathBuf>,
    pub download_delay_ms: u64,
}

impl ResolvedConfig {
    #[must_use]
    #[allow(clippy::too_many_lines)]
    pub fn from_config(config: Config, config_path: Option<PathBuf>) -> Self {
        // ── Determine data_dir (paths.data > runtime.download_dir) ──
        let data_dir = config
            .paths
            .data
            .as_ref()
            .map_or_else(|| expand_path(&config.runtime.download_dir), |p| expand_path(p));

        // ── Cache dir (paths.cache > runtime.cache_dir > XDG cache/rompla) ──
        // Rompla: cacheDir defaults to getCacheHome() = XDG_CACHE_HOME/rompla
        let xdg_cache_home = || -> PathBuf {
            std::env::var("XDG_CACHE_HOME")
                .ok().map_or_else(|| {
                    directories::BaseDirs::new().map_or_else(|| PathBuf::from("/tmp"), |b| b.home_dir().join(".cache"))
                }, PathBuf::from)
                .join("rompla")
        };
        let cache_dir = config
            .paths
            .cache
            .as_ref()
            .or(config.runtime.cache_dir.as_ref())
            .map_or_else(xdg_cache_home, |p| expand_path(p));

        // ── Helper: derive a subdir (paths.<name> > runtime.<name>_dir > data_dir/<name>) ──
        let resolve_subdir = |paths_val: &Option<String>,
                              runtime_val: &Option<String>,
                              default_name: &str|
         -> PathBuf {
            paths_val
                .as_ref()
                .or(runtime_val.as_ref())
                .map_or_else(|| data_dir.join(default_name), |p| expand_path(p))
        };

        // db_path: paths.database > runtime.db_path; when data is set, re-derive
        let db_path = if let Some(db) = &config.paths.database {
            expand_path(db)
        } else if config.paths.data.is_some() {
            // Rompla: when data is set, database = data / "data.db"
            data_dir.join("data.db")
        } else {
            expand_path(&config.runtime.db_path)
        };

        let thumbs_dir = resolve_subdir(&config.paths.thumbs, &config.runtime.thumbs_dir, "thumbs");
        let covers_dir = resolve_subdir(&config.paths.covers, &config.runtime.covers_dir, "covers");
        let videos_dir = resolve_subdir(&config.paths.videos, &config.runtime.videos_dir, "videos");
        let previews_dir =
            resolve_subdir(&config.paths.previews, &config.runtime.previews_dir, "previews");
        let models_dir = resolve_subdir(&config.paths.models, &config.runtime.models_dir, "models");
        let flags_dir = resolve_subdir(&config.paths.flags, &config.runtime.flags_dir, "flags");

        // ── Frames dir (paths.frames_cache > runtime.frames_dir > XDG cache/rompla/frames) ──
        // Rompla: framesDir defaults to getCacheHome()/frames, independent of cacheDir.
        // We replicate that: XDG_CACHE_HOME/rompla/frames (or ~/.cache/rompla/frames).
        let xdg_cache_frames = || -> PathBuf {
            let xdg = std::env::var("XDG_CACHE_HOME")
                .ok().map_or_else(|| {
                    directories::BaseDirs::new().map_or_else(|| PathBuf::from("/tmp"), |b| b.home_dir().join(".cache"))
                }, PathBuf::from);
            xdg.join("rompla").join("frames")
        };
        let frames_dir = config
            .paths
            .frames_cache
            .as_ref()
            .or(config.runtime.frames_dir.as_ref())
            .map_or_else(xdg_cache_frames, |p| expand_path(p));

        // ── Scrapers dir (paths.scrapers > runtime.scrapers_dir > data_dir/scrapers) ──
        let scrapers_dir = config
            .paths
            .scrapers
            .as_ref()
            .or(config.runtime.scrapers_dir.as_ref())
            .map_or_else(|| data_dir.join("scrapers"), |p| expand_path(p));

        // ── Scripts dir (paths.scripts > XDG data/assets/scripts > data_dir/scripts) ──
        let scripts_dir = config
            .paths
            .scripts
            .as_ref()
            .map_or_else(
                || {
                    directories::ProjectDirs::from("", "", "soromantic").map_or_else(
                        || data_dir.join("scripts"),
                        |dirs| dirs.data_dir().join("assets").join("scripts"),
                    )
                },
                |p| expand_path(p),
            );

        // ── Original database path for migration tools ──
        let orig_db_path = config.paths.orig_database.as_ref().map(|p| expand_path(p));

        let download_delay_ms = config.runtime.download_delay_ms.unwrap_or(0);

        Self {
            config_file_path: config_path,
            db_path,
            download_dir: data_dir.clone(),
            cache_dir,
            frontend_dir: PathBuf::from("frontend/dist"),
            #[cfg(unix)]
            mpv_socket: data_dir.join(".mpv-socket"),
            #[cfg(windows)]
            mpv_socket: PathBuf::from(format!(r"\\.\pipe\soromantic-mpv-{}", std::process::id())),
            batch_list_path: expand_path(&config.runtime.batch_list_path),
            ffmpeg_path: if config
                .runtime
                .ffmpeg_binary
                .contains(std::path::MAIN_SEPARATOR)
            {
                expand_path(&config.runtime.ffmpeg_binary)
            } else {
                PathBuf::from(config.runtime.ffmpeg_binary)
            },
            ffprobe_path: if config
                .runtime
                .ffprobe_binary
                .contains(std::path::MAIN_SEPARATOR)
            {
                expand_path(&config.runtime.ffprobe_binary)
            } else {
                PathBuf::from(config.runtime.ffprobe_binary)
            },
            timeouts: config.timeouts,
            ui: config.ui,
            playback: config.playback,
            thumbs_dir,
            covers_dir,
            videos_dir,
            previews_dir,
            frames_dir,
            models_dir,
            flags_dir,
            scrapers_dir,
            scripts_dir,
            orig_db_path,
            download_delay_ms,
        }
    }
}

/// Status of configuration loading
pub enum ConfigStatus {
    Loaded(Box<ResolvedConfig>),
    Created(PathBuf),
}

static CONFIG_CACHE: std::sync::Mutex<Option<ResolvedConfig>> = std::sync::Mutex::new(None);

/// Load configuration from file.
///
/// # Errors
/// Returns error if reading or parsing the config file fails, or if writing the
/// default config fails.
///
/// # Panics
/// Panics if the global configuration cache lock is poisoned.
///
/// Returns `Loaded(config)` if found, or `Created(path)` if a new default was created.
#[allow(clippy::expect_used)]
pub fn load_config(config_path: Option<&str>) -> Result<ConfigStatus> {
    // 1. Check Cache
    {
        let cache = CONFIG_CACHE.lock().expect("Config cache lock poisoned");
        if let Some(cfg) = &*cache {
            // println!("Config: Returning cached config.");
            return Ok(ConfigStatus::Loaded(Box::new(cfg.clone())));
        }
    }

    let path = config_path.map_or_else(
        || {
            // Check local paths first
            let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            let local_config = cwd.join("config.toml");
            let parent_config = cwd.join("../config.toml");

            if local_config.exists() {
                println!("Config: Found local config: {}", local_config.display());
                local_config
            } else if parent_config.exists() {
                parent_config
            } else {
                get_default_config_path()
            }
        },
        expand_path,
    );

    tracing::info!("Reading config file: {:?}", path);

    if path.exists() {
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read config file: {}", path.display()))?;
        let cfg_raw = toml::from_str(&content)
            .with_context(|| format!("Failed to parse config file: {}", path.display()))?;

        let resolved = ResolvedConfig::from_config(cfg_raw, Some(path));

        // Update Cache
        *CONFIG_CACHE.lock().expect("Config cache lock poisoned") = Some(resolved.clone());

        Ok(ConfigStatus::Loaded(Box::new(resolved)))
    } else {
        // Auto-create config if it doesn't exist
        if let Some(parent) = path.parent()
            && let Err(e) = std::fs::create_dir_all(parent)
        {
            tracing::warn!(
                "Failed to create config directory {}: {e}",
                parent.display()
            );
        }

        let default_config_content = include_str!("../../config.toml.example");
        if let Err(e) = std::fs::write(&path, default_config_content) {
            // If we fail to write, we can't really "Created", so error out
            anyhow::bail!("Failed to write default config to {}: {e}", path.display());
        }

        tracing::info!("Created default config file at {}", path.display());

        // We generally don't cache "Created" status immediately unless we reload it,
        // but `soromantic_init` usually reloads it.
        // For simplicity, we won't cache the *default* config here, forcing a reload
        // if called again (which will then hit the Loaded path).
        Ok(ConfigStatus::Created(path))
    }
}

/// Ensure all configured directories exist on startup.
///
/// Mirrors rompla's `ensureDirectories` proc. Creates every directory
/// the application expects, so individual download/cache operations
/// don't have to handle missing directories.
///
/// # Errors
/// Returns error if a directory cannot be created.
pub fn ensure_directories(config: &ResolvedConfig) -> Result<()> {
    let dirs: &[(&PathBuf, &str)] = &[
        (&config.download_dir, "download_dir"),
        (&config.cache_dir, "cache_dir"),
        (&config.thumbs_dir, "thumbs_dir"),
        (&config.covers_dir, "covers_dir"),
        (&config.videos_dir, "videos_dir"),
        (&config.previews_dir, "previews_dir"),
        (&config.frames_dir, "frames_dir"),
        (&config.models_dir, "models_dir"),
        (&config.flags_dir, "flags_dir"),
        (&config.scrapers_dir, "scrapers_dir"),
    ];

    for (dir, name) in dirs {
        if !dir.as_os_str().is_empty() && !dir.exists() {
            std::fs::create_dir_all(dir)
                .with_context(|| format!("Failed to create {name}: {}", dir.display()))?;
        }
    }

    // Create the db directory (db_path's parent)
    if let Some(parent) = config.db_path.parent()
        && !parent.as_os_str().is_empty()
        && !parent.exists()
    {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create db path parent: {}", parent.display()))?;
    }

    Ok(())
}

/// Load batch list from file, filtering comments and empty lines.
///
/// # Errors
/// Returns error if reading the batch file fails.
pub fn load_batch_list(path: &std::path::Path) -> Result<Vec<String>> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read batch file: {}", path.display()))?;

    Ok(content
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_example_config_is_valid() {
        let content = include_str!("../../config.toml.example");
        let cfg: Config = toml::from_str(content)
            .expect("config.toml.example should be valid TOML and match Config struct");

        // Sanity checks to ensure playback additions and debug flag parse correctly
        assert_eq!(cfg.ui.items_per_page, 50);
        assert!((cfg.playback.wheel_seek_amount - 5.0).abs() < f64::EPSILON);
        assert!((cfg.playback.initial_volume - 70.0).abs() < f64::EPSILON);
        assert!((cfg.playback.volume_step - 5.0).abs() < f64::EPSILON);
        assert!(!cfg.runtime.debug);
    }
}
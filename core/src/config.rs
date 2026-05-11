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
}

impl Default for PlaybackConfig {
    fn default() -> Self {
        Self {
            video_preferences: vec![480, 720],
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

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct Config {
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
            batch_list_path: default_batch_list_path(),
            ffmpeg_binary: default_ffmpeg_binary(),
            ffprobe_binary: default_ffprobe_binary(),
            thumbs_dir: None,
            covers_dir: None,
            videos_dir: None,
            previews_dir: None,
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
    pub models_dir: PathBuf,
    pub flags_dir: PathBuf,
    pub scrapers_dir: PathBuf,
    pub download_delay_ms: u64,
}

impl ResolvedConfig {
    #[must_use]
    pub fn from_config(config: Config, config_path: Option<PathBuf>) -> Self {
        let download_dir = expand_path(&config.runtime.download_dir);

        // Cache dir defaults to download_dir/cache if not explicitly set
        let cache_dir = config
            .runtime
            .cache_dir
            .as_ref()
            .map_or_else(|| download_dir.join("cache"), |p| expand_path(p));

        let thumbs_dir = config
            .runtime
            .thumbs_dir
            .as_ref()
            .map_or_else(|| download_dir.join("thumbs"), |p| expand_path(p));

        let covers_dir = config
            .runtime
            .covers_dir
            .as_ref()
            .map_or_else(|| download_dir.join("covers"), |p| expand_path(p));

        let videos_dir = config
            .runtime
            .videos_dir
            .as_ref()
            .map_or_else(|| download_dir.join("videos"), |p| expand_path(p));

        let previews_dir = config
            .runtime
            .previews_dir
            .as_ref()
            .map_or_else(|| download_dir.join("previews"), |p| expand_path(p));

        let models_dir = config
            .runtime
            .models_dir
            .as_ref()
            .map_or_else(|| download_dir.join("models"), |p| expand_path(p));

        let flags_dir = config
            .runtime
            .flags_dir
            .as_ref()
            .map_or_else(|| download_dir.join("flags"), |p| expand_path(p));

        let scrapers_dir = config
            .runtime
            .scrapers_dir
            .as_ref()
            .map_or_else(|| download_dir.join("scrapers"), |p| expand_path(p));

        let download_delay_ms = config.runtime.download_delay_ms.unwrap_or(0);

        Self {
            config_file_path: config_path,
            db_path: expand_path(&config.runtime.db_path),
            download_dir: download_dir.clone(),
            cache_dir,
            frontend_dir: PathBuf::from("frontend/dist"),
            #[cfg(unix)]
            mpv_socket: download_dir.join(".mpv-socket"),
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
            models_dir,
            flags_dir,
            scrapers_dir,
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

        // Sanity check one value to ensure we parsed what we thought we parsed
        assert_eq!(cfg.ui.items_per_page, 50);
    }
}

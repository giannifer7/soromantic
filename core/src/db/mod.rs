mod commands;
mod models;
mod queries;

pub use models::*;

use anyhow::Result;
use sqlx::ConnectOptions;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions};
use std::path::Path;
use std::str::FromStr;
use std::time::Duration;

use crate::config::ResolvedConfig;

#[derive(Clone)]
pub struct Database {
    pub pool: SqlitePool,
    pub(crate) config: ResolvedConfig,
}

impl Database {
    /// Open an existing Rompla database. Does NOT run migrations —
    /// the database is expected to already have the Rompla schema.
    ///
    /// # Errors
    /// Returns error if database connection fails or the schema is missing.
    pub async fn new(config: ResolvedConfig) -> Result<Self> {
        let path = &config.db_path;
        let busy_timeout = Duration::from_millis(config.timeouts.db_busy);

        if let Some(parent) = path.parent()
            && !parent.exists()
        {
            std::fs::create_dir_all(parent)?;
        }

        let db_url = format!("sqlite://{}", path.to_string_lossy());

        let mut opts = SqliteConnectOptions::from_str(&db_url)?
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .busy_timeout(busy_timeout);

        opts = opts.log_statements(log::LevelFilter::Debug);

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(opts)
            .await?;

        // Run PRAGMAs matching Rompla's openDb
        sqlx::query("PRAGMA journal_mode=WAL")
            .execute(&pool)
            .await?;
        sqlx::query("PRAGMA synchronous=NORMAL")
            .execute(&pool)
            .await?;
        sqlx::query("PRAGMA busy_timeout=200")
            .execute(&pool)
            .await?;

        // Verify the Rompla schema exists
        let table_exists: (i64,) = sqlx::query_as(
            "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='sites'",
        )
        .fetch_one(&pool)
        .await?;

        if table_exists.0 == 0 {
            anyhow::bail!(
                "Rompla schema not found in database. Expected 'sites' table. \
                 Run the Rompla app first to initialize the database."
            );
        }

        let db = Self { pool, config };

        // Sync sites from config (upsert configured scrapers)
        if let Err(e) = db.sync_sites() {
            tracing::warn!("Failed to sync sites: {e}");
        }

        Ok(db)
    }

    /// Upsert all sites from config into the sites table.
    /// Mirrors Rompla's `syncSites` proc.
    ///
    /// # Errors
    /// Returns error if database query fails.
    pub const fn sync_sites(&self) -> Result<()> {
        // If no sites are configured, nothing to sync
        // Sites come from the scraper configuration — for now this is a no-op
        // but the method exists for future use when scrapers are registered
        Ok(())
    }

    /// Convert an absolute path to a relative one based on configuration.
    /// Returns the original string if it doesn't match any configured root.
    #[must_use]
    pub fn relativize_path(&self, path: &str) -> String {
        let p = Path::new(path);
        if !p.is_absolute() {
            return path.to_string();
        }

        // Try to match specific roots first (more specific -> less specific)
        let roots = [
            (&self.config.thumbs_dir, "thumbs"),
            (&self.config.covers_dir, "covers"),
            (&self.config.videos_dir, "videos"),
            (&self.config.previews_dir, "previews"),
            (&self.config.frames_dir, "frames"),
            (&self.config.models_dir, "models"),
            (&self.config.flags_dir, "flags"),
        ];

        for (root, prefix) in roots {
            if let Ok(rel) = p.strip_prefix(root) {
                return format!("{prefix}/{rel}", rel = rel.to_string_lossy());
            }
        }

        path.to_string()
    }

    /// Convert a relative path to absolute based on configuration prefix.
    /// If path is already absolute, return as is.
    #[must_use]
    pub fn absolutize_path(&self, path: &str) -> String {
        let p = Path::new(path);
        if p.is_absolute() {
            return path.to_string();
        }

        // Check prefixes
        if let Some(rest) = path.strip_prefix("thumbs/") {
            return self
                .config
                .thumbs_dir
                .join(rest)
                .to_string_lossy()
                .to_string();
        }
        if let Some(rest) = path.strip_prefix("covers/") {
            return self
                .config
                .covers_dir
                .join(rest)
                .to_string_lossy()
                .to_string();
        }
        if let Some(rest) = path.strip_prefix("videos/") {
            return self
                .config
                .videos_dir
                .join(rest)
                .to_string_lossy()
                .to_string();
        }
        if let Some(rest) = path.strip_prefix("previews/") {
            return self
                .config
                .previews_dir
                .join(rest)
                .to_string_lossy()
                .to_string();
        }
        if let Some(rest) = path.strip_prefix("frames/") {
            return self
                .config
                .frames_dir
                .join(rest)
                .to_string_lossy()
                .to_string();
        }
        if let Some(rest) = path.strip_prefix("models/") {
            return self
                .config
                .models_dir
                .join(rest)
                .to_string_lossy()
                .to_string();
        }
        if let Some(rest) = path.strip_prefix("flags/") {
            return self
                .config
                .flags_dir
                .join(rest)
                .to_string_lossy()
                .to_string();
        }

        // Unknown prefix? Treat as relative to download_dir
        self.config
            .download_dir
            .join(path)
            .to_string_lossy()
            .to_string()
    }
}

use crate::db::Database;
use crate::db::models::VideoSource;
use crate::downloader::paths;
use anyhow::Result;

use crate::types::PlaylistItem;

impl Database {
    /// Get available videos for a page.
    ///
    /// # Errors
    /// Returns error if database query fails.
    pub async fn get_videos(&self, page_id: i64) -> Result<Vec<VideoSource>> {
        use futures_util::StreamExt;
        use sqlx::Row;

        let mut rows = sqlx::query(
            r"
            SELECT resolution, start_time, stop_time
            FROM video_sources
            WHERE page_id = ? AND status = ?
            ",
        )
        .bind(page_id)
        .bind(crate::constants::status::DONE)
        .fetch(&self.pool);

        let mut videos = Vec::new();
        while let Some(row_res) = rows.next().await {
            let row = row_res?;
            let resolution: Option<i64> = row.try_get(0)?;

            if let Some(path) = self.resolve_video_path(page_id, resolution) {
                videos.push(VideoSource {
                    local_path: path,
                    resolution,
                    start_time: row.try_get(1)?,
                    stop_time: row.try_get(2)?,
                });
            }
        }
        Ok(videos)
    }

    /// Generate a playlist for the given page IDs.
    /// Fetches local video paths and titles, applying resolution preferences.
    ///
    /// # Errors
    /// Returns error if database query fails.
    pub async fn get_playlist(&self, ids: &[i64]) -> Result<Vec<PlaylistItem>> {
        use futures_util::StreamExt;
        use sqlx::Row;

        if ids.is_empty() {
            return Ok(Vec::new());
        }

        // 1. Get Videos
        let videos_map = self.get_videos_batched(ids).await?;
        println!(
            "DB: get_videos_batched returned {} entries",
            videos_map.len()
        );

        // 2. Get Titles
        let placeholders: Vec<String> = ids.iter().map(|_| "?".to_string()).collect();
        let query_str = format!(
            "SELECT id, title FROM pages WHERE id IN ({})",
            placeholders.join(",")
        );

        let mut query = sqlx::query(&query_str);
        for id in ids {
            query = query.bind(id);
        }

        let mut rows = query.fetch(&self.pool);

        let mut page_titles: std::collections::HashMap<i64, String> =
            std::collections::HashMap::new();
        while let Some(row_res) = rows.next().await {
            let row = row_res?;
            let id: i64 = row.try_get(0)?;
            let title: String = row.try_get(1)?;
            page_titles.insert(id, title);
        }

        // 3. Build Playlist
        let mut playlist = Vec::new();
        let prefs = &self.config.playback.video_preferences;

        for id in ids {
            if let Some(videos) = videos_map.get(id) {
                // Find best video based on preferences
                // Try to find exact match for each preference in order
                let best_video = prefs
                    .iter()
                    .find_map(|&pref| videos.iter().find(|v| v.resolution == Some(pref)))
                    .or_else(|| videos.first()); // Fallback to first available

                if let Some(video) = best_video {
                    let title = page_titles
                        .get(id)
                        .cloned()
                        .unwrap_or_else(|| format!("Video {id}"));

                    // Use intervals from the specific video source
                    let intervals = match (video.start_time, video.stop_time) {
                        (Some(start), Some(stop)) if start > 0.0 || stop > 0.0 => {
                            Some(vec![(start, stop)])
                        }
                        _ => None,
                    };

                    println!(
                        "DB: Adding to playlist: {} (path={})",
                        title, video.local_path
                    );
                    playlist.push(PlaylistItem {
                        path: video.local_path.clone(),
                        title,
                        intervals,
                    });
                } else {
                    println!("DB: No suitable video found for ID {id}");
                }
            } else {
                println!("DB: No videos map entry for ID {id}");
            }
        }

        Ok(playlist)
    }

    /// Find a locally downloaded video path for a page.
    /// Returns the path if found, or `None`.
    ///
    /// # Errors
    /// Returns error if database query fails.
    pub async fn find_downloaded_video(&self, page_id: i64) -> Result<Option<String>> {
        let videos = self.get_videos(page_id).await?;
        Ok(videos.into_iter().next().map(|v| v.local_path))
    }

    /// Helper to resolve video path and check existence.
    fn resolve_video_path(&self, page_id: i64, resolution: Option<i64>) -> Option<String> {
        let path = paths::get_download_path(
            &self.config,
            page_id,
            None,
            paths::FileType::Video,
            resolution,
        );
        if path.exists() {
            Some(path.to_string_lossy().to_string())
        } else {
            None
        }
    }
}

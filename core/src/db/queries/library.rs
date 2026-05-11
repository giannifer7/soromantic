use crate::db::Database;
use crate::db::models::{GridItem, LibraryItem, PageData, VideoSource};
use crate::downloader::paths;
use anyhow::Result;

impl Database {
    /// Get page URL by ID using `pages_full` view.
    ///
    /// # Errors
    /// Returns error if database query fails.
    pub async fn get_page_url(&self, id: i64) -> Result<Option<String>> {
        let url: Option<String> = sqlx::query_scalar("SELECT url FROM pages_full WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(url)
    }

    /// Get library data with pagination — uses `pages_full` view.
    ///
    /// # Errors
    /// Returns error if database query fails.
    #[allow(clippy::too_many_lines)]
    #[tracing::instrument(skip(self))]
    pub async fn get_library_paginated(
        &self,
        offset: i64,
        limit: i64,
        skip_count: bool,
    ) -> Result<(Vec<LibraryItem>, i64)> {
        use sqlx::Row;

        let start = std::time::Instant::now();

        // 1. Get total count
        let total_count: i64 = if skip_count {
            -1
        } else {
            sqlx::query_scalar(
                "SELECT COUNT(*) FROM pages_full p WHERE p.title IS NOT NULL AND p.thumb_status = ? AND p.video_status = ?",
            )
            .bind(crate::constants::status::DONE)
            .bind(crate::constants::status::DONE)
            .fetch_one(&self.pool)
            .await?
        };

        // 2. Get subset of pages
        let pages_query = r"
            SELECT p.id, p.title, p.thumb_status, p.preview_status
            FROM pages_full p
            WHERE p.title IS NOT NULL AND p.thumb_status = ? AND p.video_status = ?
            ORDER BY p.id DESC
            LIMIT ? OFFSET ?
        ";

        let rows: Vec<sqlx::sqlite::SqliteRow> = sqlx::query(pages_query)
            .bind(crate::constants::status::DONE)
            .bind(crate::constants::status::DONE)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?;

        if rows.is_empty() {
            return Ok((Vec::new(), total_count));
        }

        let page_ids: Vec<i64> = rows.iter().map(|r| r.get::<i64, _>(0)).collect();

        fn place(n: usize) -> String {
            if n == 0 {
                return "NULL".to_string();
            }
            (0..n).map(|_| "?").collect::<Vec<_>>().join(",")
        }

        // 3a. Batch fetch Video Stats
        let stats_query_str = format!(
            r"
            SELECT vs.page_id,
                   SUM(CASE WHEN vs.status = ? THEN 1 ELSE 0 END) as finished_count,
                   0 as failed_count
            FROM video_sources vs
            WHERE vs.page_id IN ({})
            GROUP BY vs.page_id
            ",
            place(page_ids.len())
        );

        let mut query = sqlx::query(&stats_query_str).bind(crate::constants::status::DONE);
        for id in &page_ids {
            query = query.bind(id);
        }

        let mut stats_map: std::collections::HashMap<i64, (i64, i64)> =
            std::collections::HashMap::new();

        let stats_rows: Vec<sqlx::sqlite::SqliteRow> = query.fetch_all(&self.pool).await?;
        for row in stats_rows {
            let id: i64 = row.get(0);
            let finished: i64 = row.get(1);
            let failed: i64 = row.get(2);
            stats_map.insert(id, (finished, failed));
        }

        // 4. Assemble items
        let mut items = Vec::new();
        for row in rows {
            let id: i64 = row.get(0);
            let (finished, failed) = stats_map.get(&id).unwrap_or(&(0, 0));
            items.push(self.map_to_library_item(&row, *finished, *failed)?);
        }

        let elapsed_ms: u64 = start.elapsed().as_millis().try_into().unwrap_or(u64::MAX);
        tracing::info!(
            elapsed_ms,
            count = items.len(),
            total = total_count,
            offset,
            limit,
            "get_library_paginated"
        );
        Ok((items, total_count))
    }

    /// Get page data with grid items.
    ///
    /// # Errors
    /// Returns error if database query fails.
    pub async fn get_page(&self, page_id: i64) -> Result<Option<PageData>> {
        let Some(mut page) = self.get_page_info(page_id).await? else {
            return Ok(None);
        };
        page.grid = self.get_related(page_id).await?;
        Ok(Some(page))
    }

    /// Get page metadata only (no grid items). Uses `pages_full` view.
    ///
    /// # Errors
    /// Returns error if database query fails.
    pub async fn get_page_info(&self, page_id: i64) -> Result<Option<PageData>> {
        use sqlx::Row;

        let row: Option<sqlx::sqlite::SqliteRow> = sqlx::query(
            r"
            SELECT p.id, p.url, COALESCE(p.title, p.url) as title,
                   p.cover_status,
                   (SELECT s.name FROM studios s WHERE s.id = p.studio_id) as studio,
                   (SELECT vs.resolution
                    FROM video_sources vs
                    WHERE vs.page_id = p.id AND vs.status = ?
                    LIMIT 1) as resolution,
                   p.thumb_status,
                   p.preview_status,
                   p.video_status,
                   p.studio_id,
                   p.site_id
            FROM pages_full p
            WHERE p.id = ?
            ",
        )
        .bind(crate::constants::status::DONE)
        .bind(page_id)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => {
                let id: i64 = row.try_get(0)?;
                let thumb_status: i64 = row.try_get::<Option<i64>, _>(6)?.unwrap_or(0);
                let local_image = if thumb_status == crate::constants::status::DONE {
                    Some(self.absolutize_path(&format!(
                        "thumbs/{id:0width$}.jpg",
                        width = crate::constants::ui::PAD_WIDTH
                    )))
                } else {
                    None
                };

                let resolution: Option<i64> = row.try_get(5)?;
                let local_video_path = resolution.map(|res| {
                    paths::get_download_path(
                        &self.config,
                        id,
                        None,
                        paths::FileType::Video,
                        Some(res),
                    )
                    .to_string_lossy()
                    .to_string()
                });

                Ok(Some(PageData {
                    id,
                    url: row.try_get(1)?,
                    title: row.try_get(2)?,
                    image: None,
                    local_image,
                    cover_status: row.try_get(3)?,
                    studio: row.try_get(4)?,
                    local_video_path,
                    thumb_status,
                    preview_status: row.try_get::<Option<i64>, _>(7)?.unwrap_or(0),
                    video_status: row.try_get::<Option<i64>, _>(8)?.unwrap_or(0),
                    studio_id: row.try_get(9)?,
                    site_id: row.try_get(10)?,
                    grid: Vec::new(),
                }))
            }
            None => Ok(None),
        }
    }

    /// Get related items with pagination.
    ///
    /// # Errors
    /// Returns error if database query fails.
    pub async fn get_related_paginated(
        &self,
        source_id: i64,
        offset: i64,
        limit: i64,
    ) -> Result<(Vec<LibraryItem>, i64)> {
        let related_ids: Vec<i64> = sqlx::query_scalar(
            "SELECT target_id FROM page_relations WHERE source_id = ? ORDER BY target_id LIMIT ? OFFSET ?",
        )
        .bind(source_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        let total_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM page_relations WHERE source_id = ?")
                .bind(source_id)
                .fetch_one(&self.pool)
                .await?;

        if related_ids.is_empty() {
            return Ok((Vec::new(), total_count));
        }

        let items = self.get_library_items_by_page_ids(&related_ids).await?;
        Ok((items, total_count))
    }

    /// Get all related items as `GridItems`.
    ///
    /// # Errors
    /// Returns error if database query fails.
    pub async fn get_related(&self, source_id: i64) -> Result<Vec<GridItem>> {
        let (items, _) = self.get_related_paginated(source_id, 0, 500).await?;
        Ok(items
            .into_iter()
            .map(|item| GridItem {
                id: Some(item.id),
                title: item.title,
                url: item.url,
                image: item.image,
                local_image: item.local_image,
                preview_url: item.preview_url,
                local_preview: item.local_preview,
                related_id: Some(item.id),
                finished_videos: item.finished_videos,
                failed_videos: item.failed_videos,
            })
            .collect())
    }

    /// Get page ID by URL (from `pages_full` view for full URL lookup).
    ///
    /// # Errors
    /// Returns error if database query fails.
    pub async fn get_page_id_by_url(&self, url: &str) -> Result<Option<i64>> {
        let id_opt = sqlx::query_scalar("SELECT id FROM pages_full WHERE url = ?")
            .bind(url)
            .fetch_optional(&self.pool)
            .await?;
        Ok(id_opt)
    }

    /// Search pages by title using `pages_full` view.
    ///
    /// # Errors
    /// Returns error if database query fails.
    pub async fn search_pages(&self, query: &str, limit: i64) -> Result<Vec<LibraryItem>> {
        use futures_util::StreamExt;
        use sqlx::Row;

        let pattern = format!("%{query}%");
        let mut rows = sqlx::query(
            r"
            SELECT p.id, p.title,
                   p.thumb_status,
                   p.preview_status,
                   (SELECT COUNT(*) FROM video_sources vs
                    WHERE vs.page_id = p.id AND vs.status = ?) as finished_videos,
                   (CASE WHEN p.video_status = ? THEN 1 ELSE 0 END) as failed_videos
            FROM pages_full p
            WHERE p.title LIKE ? AND p.video_status = ?
            ORDER BY p.id DESC
            LIMIT ?
            ",
        )
        .bind(crate::constants::status::DONE)
        .bind(crate::constants::status::ERROR)
        .bind(pattern)
        .bind(crate::constants::status::DONE)
        .bind(limit)
        .fetch(&self.pool);

        let mut items = Vec::new();
        while let Some(row_res) = rows.next().await {
            let row = row_res?;
            let finished: i64 = row.try_get(4)?;
            let failed: i64 = row.try_get(5)?;
            items.push(self.map_to_library_item(&row, finished, failed)?);
        }
        Ok(items)
    }

    /// Get available videos for multiple pages.
    ///
    /// # Errors
    /// Returns error if database query fails.
    pub async fn get_videos_batched(
        &self,
        page_ids: &[i64],
    ) -> Result<std::collections::HashMap<i64, Vec<VideoSource>>> {
        use futures_util::StreamExt;
        use sqlx::Row;

        if page_ids.is_empty() {
            return Ok(std::collections::HashMap::new());
        }

        let placeholders: Vec<String> = page_ids.iter().map(|_| "?".to_string()).collect();
        let query_str = format!(
            r"
            SELECT vs.page_id, vs.resolution, vs.start_time, vs.stop_time
            FROM video_sources vs
            WHERE vs.page_id IN ({}) AND vs.status = ?
            ",
            placeholders.join(",")
        );

        let mut query = sqlx::query(&query_str);
        for id in page_ids {
            query = query.bind(id);
        }
        query = query.bind(crate::constants::status::DONE);

        let mut rows = query.fetch(&self.pool);

        let mut result: std::collections::HashMap<i64, Vec<VideoSource>> =
            std::collections::HashMap::new();

        while let Some(row_res) = rows.next().await {
            let row = row_res?;
            let page_id: i64 = row.try_get(0)?;
            let resolution: Option<i64> = row.try_get(1)?;

            let path = paths::get_download_path(
                &self.config,
                page_id,
                None,
                paths::FileType::Video,
                resolution,
            );

            let video = VideoSource {
                local_path: path.to_string_lossy().to_string(),
                resolution,
                start_time: row.try_get(2)?,
                stop_time: row.try_get(3)?,
            };
            result.entry(page_id).or_default().push(video);
        }

        Ok(result)
    }

    /// Get library items for a specific list of page IDs.
    /// The returned items match the order of `page_ids`.
    ///
    /// # Errors
    /// Returns error if database query fails.
    pub async fn get_library_items_by_page_ids(
        &self,
        page_ids: &[i64],
    ) -> Result<Vec<LibraryItem>> {
        use sqlx::Row;

        if page_ids.is_empty() {
            return Ok(Vec::new());
        }

        let values_list = page_ids
            .iter()
            .map(|id| format!("({id})"))
            .collect::<Vec<_>>()
            .join(",");

        let query_str = format!(
            r"
            WITH input_ids(id) AS (VALUES {values_list})
            SELECT p.id, p.title, p.thumb_status, p.preview_status, p.video_status
            FROM pages p
            JOIN input_ids i ON p.id = i.id
            "
        );

        let rows = sqlx::query(&query_str).fetch_all(&self.pool).await?;

        let mut row_map: std::collections::HashMap<i64, sqlx::sqlite::SqliteRow> =
            rows.into_iter().map(|r| (r.get::<i64, _>(0), r)).collect();

        let mut items = Vec::with_capacity(page_ids.len());
        for id in page_ids {
            if let Some(row) = row_map.remove(id) {
                let video_status: i64 = row.try_get::<Option<i64>, _>(4)?.unwrap_or(0);
                let finished = i64::from(video_status == crate::constants::status::DONE);
                let failed = 0;

                items.push(self.map_to_library_item(&row, finished, failed)?);
            }
        }

        Ok(items)
    }
}

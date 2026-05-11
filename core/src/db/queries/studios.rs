use crate::db::Database;
use crate::db::models::{LibraryItem, StudioItem};
use anyhow::Result;

impl Database {
    const STUDIOS_BASE_SELECT: &'static str = r"
        SELECT s.id, s.name, s.url, COUNT(DISTINCT p.id) as count
        FROM studios s
        LEFT JOIN studio_links sl ON s.id = sl.studio_id
        LEFT JOIN pages p ON sl.page_id = p.id
    ";

    const STUDIOS_GROUP_ORDER: &'static str = r"
        GROUP BY s.id
        ORDER BY s.name ASC
    ";

    /// Get studios with pagination and optional search.
    ///
    /// # Errors
    /// Returns error if database query fails.
    pub async fn get_studios_paginated(
        &self,
        offset: i64,
        limit: i64,
        search_query: Option<String>,
    ) -> Result<(Vec<StudioItem>, i64)> {
        let search_filter = search_query.as_ref().map_or_else(String::new, |q| {
            format!("AND s.name LIKE '%{}%'", q.replace('\'', "''"))
        });

        let count_sql =
            format!("SELECT COUNT(DISTINCT s.id) FROM studios s WHERE 1=1 {search_filter}");

        let total = sqlx::query_scalar(&count_sql).fetch_one(&self.pool).await?;

        let query = format!(
            "{} WHERE 1=1 {} {} LIMIT ? OFFSET ?",
            Self::STUDIOS_BASE_SELECT,
            search_filter,
            Self::STUDIOS_GROUP_ORDER
        );

        let mut rows = sqlx::query(&query)
            .bind(limit)
            .bind(offset)
            .fetch(&self.pool);

        let items = Self::fetch_studios_from_stream(&mut rows).await?;
        Ok((items, total))
    }

    /// Get all studios (non-paginated).
    ///
    /// # Errors
    /// Returns error if database query fails.
    pub async fn get_all_studios(&self) -> Result<Vec<StudioItem>> {
        let query = format!(
            "{} {}",
            Self::STUDIOS_BASE_SELECT,
            Self::STUDIOS_GROUP_ORDER
        );

        let mut rows = sqlx::query(&query).fetch(&self.pool);

        Self::fetch_studios_from_stream(&mut rows).await
    }

    async fn fetch_studios_from_stream(
        rows: &mut futures_util::stream::BoxStream<
            '_,
            Result<sqlx::sqlite::SqliteRow, sqlx::Error>,
        >,
    ) -> Result<Vec<StudioItem>> {
        use futures_util::StreamExt;
        use sqlx::Row;

        let mut items = Vec::new();
        while let Some(row_res) = rows.next().await {
            let row = row_res?;
            items.push(StudioItem {
                id: row.try_get(0)?,
                name: row.try_get(1)?,
                url: row.try_get(2)?,
                count: row.try_get(3)?,
            });
        }
        Ok(items)
    }

    /// Get videos by studio ID with pagination.
    ///
    /// # Errors
    /// Returns error if database query fails.
    pub async fn get_videos_by_studio_paginated(
        &self,
        studio_id: i64,
        offset: i64,
        limit: i64,
        skip_count: bool,
    ) -> Result<(Vec<LibraryItem>, i64)> {
        let total_count: i64 = if skip_count {
            -1
        } else {
            sqlx::query_scalar("SELECT COUNT(*) FROM studio_links WHERE studio_id = ?")
                .bind(studio_id)
                .fetch_one(&self.pool)
                .await?
        };

        let ids: Vec<i64> = sqlx::query_scalar(
            r"
            SELECT DISTINCT page_id
            FROM studio_links
            WHERE studio_id = ?
            ORDER BY page_id DESC
            LIMIT ? OFFSET ?
            ",
        )
        .bind(studio_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        let items = self.get_library_items_by_page_ids(&ids).await?;
        Ok((items, total_count))
    }

    /// Get videos by studio name with pagination, including studio URLs.
    ///
    /// # Errors
    /// Returns error if database queries fail.
    pub async fn get_videos_by_studio_name_paginated(
        &self,
        studio_name: String,
        offset: i64,
        limit: i64,
        skip_count: bool,
    ) -> Result<(Vec<LibraryItem>, i64, Vec<String>)> {
        use futures_util::StreamExt;
        use sqlx::Row;

        // 1. Get studio IDs by name
        let mut studio_rows = sqlx::query("SELECT id, url FROM studios WHERE name = ?")
            .bind(&studio_name)
            .fetch(&self.pool);

        let mut studio_ids = Vec::new();
        let mut urls = Vec::new();
        while let Some(row_res) = studio_rows.next().await {
            let row = row_res?;
            studio_ids.push(row.try_get::<i64, _>(0)?);
            if let Ok(Some(url)) = row.try_get::<Option<String>, _>(1)
                && !url.is_empty()
            {
                urls.push(url);
            }
        }
        urls.sort();
        urls.dedup();

        if studio_ids.is_empty() {
            return Ok((Vec::new(), 0, Vec::new()));
        }

        let placeholders: Vec<String> = studio_ids.iter().map(|_| "?".to_string()).collect();
        let in_clause = placeholders.join(",");

        // 2. Get total count
        let total_count: i64 = if skip_count {
            -1
        } else {
            let count_sql = format!(
                "SELECT COUNT(DISTINCT page_id) FROM studio_links WHERE studio_id IN ({in_clause})"
            );
            let mut count_query = sqlx::query_scalar(&count_sql);
            for id in &studio_ids {
                count_query = count_query.bind(id);
            }
            count_query.fetch_one(&self.pool).await?
        };

        // 3. Fetch page IDs with pagination
        let fetch_sql = format!(
            r"
            SELECT page_id
            FROM studio_links
            WHERE studio_id IN ({in_clause})
            GROUP BY page_id
            ORDER BY page_id DESC
            LIMIT ? OFFSET ?
            "
        );
        let mut fetch_query = sqlx::query_scalar(&fetch_sql);
        for id in &studio_ids {
            fetch_query = fetch_query.bind(id);
        }
        fetch_query = fetch_query.bind(limit).bind(offset);

        let ids: Vec<i64> = fetch_query.fetch_all(&self.pool).await?;
        let items = self.get_library_items_by_page_ids(&ids).await?;

        Ok((items, total_count, urls))
    }
}

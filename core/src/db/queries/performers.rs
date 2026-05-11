use crate::db::Database;
use crate::db::models::{LibraryItem, PerformerItem};
use anyhow::Result;

impl Database {
    const PERFORMERS_BASE_SELECT: &'static str = r"
        SELECT perf.id, perf.name, perf.star, perf.sex,
               perf.birth_year, perf.aliases, perf.thumb_status, perf.nation_id,
               COUNT(DISTINCT p.id) as count
        FROM performers perf
        LEFT JOIN cast c ON perf.id = c.performer_id
        LEFT JOIN pages p ON c.page_id = p.id
    ";

    const PERFORMERS_GROUP_ORDER: &'static str = r"
        GROUP BY perf.id
        ORDER BY perf.name ASC
    ";

    /// Get performers with pagination and optional search.
    ///
    /// # Errors
    /// Returns error if database query fails.
    pub async fn get_performers_paginated(
        &self,
        offset: i64,
        limit: i64,
        search_query: Option<String>,
    ) -> Result<(Vec<PerformerItem>, i64)> {
        let search_filter = search_query.as_ref().map_or_else(String::new, |q| {
            format!("AND perf.name LIKE '%{}%'", q.replace('\'', "''"))
        });

        let count_sql = format!(
            "SELECT COUNT(DISTINCT perf.id) FROM performers perf WHERE 1=1 {search_filter}"
        );

        let total = sqlx::query_scalar(&count_sql).fetch_one(&self.pool).await?;

        let query = format!(
            "{} WHERE 1=1 {} {} LIMIT ? OFFSET ?",
            Self::PERFORMERS_BASE_SELECT,
            search_filter,
            Self::PERFORMERS_GROUP_ORDER
        );

        let mut rows = sqlx::query(&query)
            .bind(limit)
            .bind(offset)
            .fetch(&self.pool);

        let items = Self::fetch_performers_from_stream(&mut rows).await?;
        Ok((items, total))
    }

    /// Get all performers (non-paginated).
    ///
    /// # Errors
    /// Returns error if database query fails.
    pub async fn get_all_performers(&self) -> Result<Vec<PerformerItem>> {
        let query = format!(
            "{} {}",
            Self::PERFORMERS_BASE_SELECT,
            Self::PERFORMERS_GROUP_ORDER
        );

        let mut rows = sqlx::query(&query).fetch(&self.pool);

        Self::fetch_performers_from_stream(&mut rows).await
    }

    async fn fetch_performers_from_stream(
        rows: &mut futures_util::stream::BoxStream<
            '_,
            Result<sqlx::sqlite::SqliteRow, sqlx::Error>,
        >,
    ) -> Result<Vec<PerformerItem>> {
        use futures_util::StreamExt;
        use sqlx::Row;

        let mut items = Vec::new();
        while let Some(row_res) = rows.next().await {
            let row = row_res?;
            items.push(PerformerItem {
                id: row.try_get(0)?,
                name: row.try_get(1)?,
                star: row.try_get::<Option<i32>, _>(2)?.unwrap_or(0),
                sex: row.try_get::<Option<i32>, _>(3)?.unwrap_or(0),
                birth_year: row.try_get(4)?,
                aliases: row.try_get(5)?,
                thumb_status: row.try_get::<Option<i64>, _>(6)?.unwrap_or(0),
                nation_id: row.try_get(7)?,
                count: row.try_get(8)?,
            });
        }
        Ok(items)
    }

    /// Get videos by performer ID with pagination.
    ///
    /// # Errors
    /// Returns error if database query fails.
    pub async fn get_videos_by_performer_paginated(
        &self,
        performer_id: i64,
        offset: i64,
        limit: i64,
        skip_count: bool,
    ) -> Result<(Vec<LibraryItem>, i64)> {
        let total_count: i64 = if skip_count {
            -1
        } else {
            sqlx::query_scalar("SELECT COUNT(*) FROM cast WHERE performer_id = ?")
                .bind(performer_id)
                .fetch_one(&self.pool)
                .await?
        };

        let ids: Vec<i64> = sqlx::query_scalar(
            r"
            SELECT DISTINCT page_id
            FROM cast
            WHERE performer_id = ?
            ORDER BY page_id DESC
            LIMIT ? OFFSET ?
            ",
        )
        .bind(performer_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        let items = self.get_library_items_by_page_ids(&ids).await?;
        Ok((items, total_count))
    }

    /// Get videos by performer name with pagination, including performer URLs.
    /// Mirrors the old `get_videos_by_taxonomy_name_paginated` for the Rompla schema.
    ///
    /// # Errors
    /// Returns error if database queries fail.
    pub async fn get_videos_by_performer_name_paginated(
        &self,
        performer_name: String,
        offset: i64,
        limit: i64,
        skip_count: bool,
    ) -> Result<(Vec<LibraryItem>, i64, Vec<String>)> {
        use futures_util::StreamExt;
        use sqlx::Row;

        // 1. Get performer IDs by name
        let mut perf_rows = sqlx::query("SELECT id FROM performers WHERE name = ?")
            .bind(&performer_name)
            .fetch(&self.pool);

        let mut performer_ids = Vec::new();
        while let Some(row_res) = perf_rows.next().await {
            let row = row_res?;
            performer_ids.push(row.try_get::<i64, _>(0)?);
        }

        if performer_ids.is_empty() {
            return Ok((Vec::new(), 0, Vec::new()));
        }

        // 1b. Get performer URLs
        let placeholders: Vec<String> = performer_ids.iter().map(|_| "?".to_string()).collect();
        let in_clause = placeholders.join(",");

        let url_sql =
            format!("SELECT DISTINCT url FROM performer_urls WHERE performer_id IN ({in_clause})");
        let mut url_query = sqlx::query_scalar(&url_sql);
        for id in &performer_ids {
            url_query = url_query.bind(id);
        }
        let urls: Vec<String> = url_query.fetch_all(&self.pool).await?;

        // 2. Get total count
        let total_count: i64 = if skip_count {
            -1
        } else {
            let count_sql = format!(
                "SELECT COUNT(DISTINCT page_id) FROM cast WHERE performer_id IN ({in_clause})"
            );
            let mut count_query = sqlx::query_scalar(&count_sql);
            for id in &performer_ids {
                count_query = count_query.bind(id);
            }
            count_query.fetch_one(&self.pool).await?
        };

        // 3. Fetch page IDs with pagination
        let fetch_sql = format!(
            r"
            SELECT page_id
            FROM cast
            WHERE performer_id IN ({in_clause})
            GROUP BY page_id
            ORDER BY page_id DESC
            LIMIT ? OFFSET ?
            "
        );
        let mut fetch_query = sqlx::query_scalar(&fetch_sql);
        for id in &performer_ids {
            fetch_query = fetch_query.bind(id);
        }
        fetch_query = fetch_query.bind(limit).bind(offset);

        let ids: Vec<i64> = fetch_query.fetch_all(&self.pool).await?;
        let items = self.get_library_items_by_page_ids(&ids).await?;

        Ok((items, total_count, urls))
    }
}

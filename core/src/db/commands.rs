use super::Database;
use anyhow::Result;
use sqlx::Row;

impl Database {
    /// Store a scraped page to the database.
    ///
    /// This creates/updates the page record and inserts related pages and video sources.
    /// Uses Rompla schema: studios + `studio_links`, performers + cast.
    /// Returns the page ID.
    ///
    /// # Errors
    /// Returns error if database queries fail.
    #[allow(clippy::too_many_lines)]
    pub async fn store_page(&self, data: &crate::scraper::PageData) -> Result<i64> {
        // Normalize URL (strip trailing slash)
        let url = data.url.trim_end_matches('/');

        // Upsert page record — uses pages_full-compatible upsert with _orphaned_ site
        // For now we store full URLs with site_id = 0 (orphaned)
        sqlx::query(
            r"
            INSERT INTO pages (site_id, url, title)
            VALUES (0, ?, ?)
            ON CONFLICT(site_id, url) DO UPDATE SET
                title = excluded.title
            ",
        )
        .bind(url)
        .bind(data.title.as_deref())
        .execute(&self.pool)
        .await?;

        // Get the page ID
        let row = sqlx::query("SELECT id FROM pages WHERE site_id = 0 AND url = ?")
            .bind(url)
            .fetch_optional(&self.pool)
            .await?;

        let page_id: i64 = match row {
            Some(r) => r.try_get(0)?,
            None => anyhow::bail!("Failed to get page ID after insert"),
        };

        // Only update video_sources if we actually found some.
        if !data.video_sources.is_empty() {
            sqlx::query("DELETE FROM video_sources WHERE page_id = ?")
                .bind(page_id)
                .execute(&self.pool)
                .await?;

            // Insert video sources
            for vs in &data.video_sources {
                sqlx::query(
                    "INSERT OR IGNORE INTO video_sources (page_id, resolution, duration) VALUES (?, ?, ?)",
                )
                .bind(page_id)
                .bind(vs.resolution)
                .bind(vs.duration)
                .execute(&self.pool)
                .await?;
            }
        }

        // 1. Resolve and Store Studio
        let mut studio_id: Option<i64> = None;
        if let Some((studio_name, studio_url)) = &data.studio {
            studio_id = Some(self.upsert_studio(studio_name, studio_url).await?);

            if let Some(s_id) = studio_id {
                sqlx::query(
                    "INSERT OR IGNORE INTO studio_links (page_id, studio_id) VALUES (?, ?)",
                )
                .bind(page_id)
                .bind(s_id)
                .execute(&self.pool)
                .await?;
            }
        }

        // 2. Resolve and Store Models (performers)
        for (model_name, _model_url) in &data.models {
            let performer_id = self.upsert_performer_by_name(model_name).await?;

            sqlx::query(
                "INSERT OR IGNORE INTO cast (page_id, performer_id, starring) VALUES (?, ?, 1)",
            )
            .bind(page_id)
            .bind(performer_id)
            .execute(&self.pool)
            .await?;
        }

        // 2b. Featuring performers (starring=0)
        for (feat_name, _feat_url) in &data.featuring {
            let performer_id = self.upsert_performer_by_name(feat_name).await?;

            sqlx::query(
                "INSERT OR IGNORE INTO cast (page_id, performer_id, starring) VALUES (?, ?, 0)",
            )
            .bind(page_id)
            .bind(performer_id)
            .execute(&self.pool)
            .await?;
        }

        // 3. Resolve and Store Related Pages
        let mut related_id_vec = Vec::new();
        for gb in &data.grid_boxes {
            if gb.url.is_empty() {
                continue;
            }
            let related_url = gb.url.trim_end_matches('/');

            sqlx::query(
                "INSERT INTO pages (site_id, url, title, thumb_status) VALUES (0, ?, ?, ?) ON CONFLICT(site_id, url) DO NOTHING",
            )
            .bind(related_url)
            .bind(gb.title.as_str())
            .bind(crate::constants::status::PENDING)
            .execute(&self.pool)
            .await?;

            if let Some(r_id) =
                sqlx::query_scalar::<_, i64>("SELECT id FROM pages WHERE site_id = 0 AND url = ?")
                    .bind(related_url)
                    .fetch_optional(&self.pool)
                    .await?
            {
                related_id_vec.push(r_id);
            }
        }

        // 3b. Populate page_relations table (used by get_related queries)
        sqlx::query("DELETE FROM page_relations WHERE source_id = ?")
            .bind(page_id)
            .execute(&self.pool)
            .await?;
        for r_id in &related_id_vec {
            sqlx::query(
                "INSERT OR IGNORE INTO page_relations (source_id, target_id) VALUES (?, ?)",
            )
            .bind(page_id)
            .bind(r_id)
            .execute(&self.pool)
            .await?;
        }

        // 4. Update page with studio FK
        if let Some(s_id) = studio_id {
            sqlx::query("UPDATE pages SET studio_id = ? WHERE id = ?")
                .bind(s_id)
                .bind(page_id)
                .execute(&self.pool)
                .await?;
        }

        Ok(page_id)
    }

    /// Mark cover as downloaded (status 1) or snapshot (status 2).
    ///
    /// # Errors
    /// Returns error if database update fails.
    pub async fn mark_cover_downloaded(&self, page_id: i64, status: i32) -> Result<()> {
        sqlx::query("UPDATE pages SET cover_status = ? WHERE id = ?")
            .bind(status)
            .bind(page_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Mark video as downloaded (update status on page).
    ///
    /// # Errors
    /// Returns error if database update fails.
    pub async fn set_page_video_status(&self, page_id: i64, status: i64) -> Result<()> {
        sqlx::query("UPDATE pages SET video_status = ? WHERE id = ?")
            .bind(status)
            .bind(page_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Update video source duration by `page_id` and resolution.
    ///
    /// # Errors
    /// Returns error if database update fails.
    pub async fn set_video_source_done(
        &self,
        page_id: i64,
        resolution: i64,
        duration: f64,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE video_sources SET duration = ?, status = ? WHERE page_id = ? AND resolution = ?",
        )
        .bind(duration)
        .bind(crate::constants::status::DONE)
        .bind(page_id)
        .bind(resolution)
        .execute(&self.pool)
        .await?;

        // Also update the page video_status to DONE
        sqlx::query("UPDATE pages SET video_status = ? WHERE id = ?")
            .bind(crate::constants::status::DONE)
            .bind(page_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    /// Update page thumbnail status.
    ///
    /// # Errors
    /// Returns error if database update fails.
    pub async fn set_page_thumb_status(&self, page_id: i64, status: i64) -> Result<()> {
        sqlx::query("UPDATE pages SET thumb_status = ? WHERE id = ?")
            .bind(status)
            .bind(page_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Update page preview status.
    ///
    /// # Errors
    /// Returns error if database update fails.
    pub async fn set_page_preview_status(&self, page_id: i64, status: i64) -> Result<()> {
        sqlx::query("UPDATE pages SET preview_status = ? WHERE id = ?")
            .bind(status)
            .bind(page_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // ============================================================
    // Rompla: nations (replaces flags)
    // ============================================================

    /// Upsert a nation and return its ID.
    ///
    /// # Errors
    /// Returns error if database queries fail.
    pub async fn upsert_nation(&self, code: &str, name: Option<&str>) -> Result<i64> {
        sqlx::query(
            r"
            INSERT INTO nations (code, name)
            VALUES (?, ?)
            ON CONFLICT(code) DO UPDATE SET
                name = COALESCE(excluded.name, nations.name)
            ",
        )
        .bind(code)
        .bind(name)
        .execute(&self.pool)
        .await?;

        let row = sqlx::query("SELECT id FROM nations WHERE code = ?")
            .bind(code)
            .fetch_one(&self.pool)
            .await?;

        Ok(row.try_get(0)?)
    }

    // ============================================================
    // Rompla: performers (replaces taxonomies type=1)
    // ============================================================

    /// Upsert a performer by name (atomic upsert on UNIQUE name).
    /// Returns the performer ID.
    ///
    /// # Errors
    /// Returns error if database query fails.
    pub async fn upsert_performer_by_name(&self, name: &str) -> Result<i64> {
        sqlx::query("INSERT INTO performers (name) VALUES (?) ON CONFLICT(name) DO NOTHING")
            .bind(name)
            .execute(&self.pool)
            .await?;

        let row = sqlx::query("SELECT id FROM performers WHERE name = ?")
            .bind(name)
            .fetch_one(&self.pool)
            .await?;

        Ok(row.try_get(0)?)
    }

    /// Upsert a performer with extended metadata and return its ID.
    ///
    /// # Errors
    /// Returns error if database queries fail.
    #[allow(clippy::too_many_arguments)]
    pub async fn upsert_performer(
        &self,
        name: &str,
        nation_id: Option<i64>,
        birth_year: Option<i32>,
        aliases: Option<&str>,
        sex: Option<i32>,
    ) -> Result<i64> {
        // Insert with metadata; ON CONFLICT(name) DO UPDATE for mutable fields
        sqlx::query(
            r"
            INSERT INTO performers (name, nation_id, birth_year, aliases, sex)
            VALUES (?, ?, ?, ?, ?)
            ON CONFLICT(name) DO UPDATE SET
                nation_id = COALESCE(excluded.nation_id, performers.nation_id),
                birth_year = COALESCE(excluded.birth_year, performers.birth_year),
                aliases = COALESCE(excluded.aliases, performers.aliases),
                sex = COALESCE(excluded.sex, performers.sex)
            ",
        )
        .bind(name)
        .bind(nation_id)
        .bind(birth_year)
        .bind(aliases)
        .bind(sex.unwrap_or(0))
        .execute(&self.pool)
        .await?;

        let row = sqlx::query("SELECT id FROM performers WHERE name = ?")
            .bind(name)
            .fetch_one(&self.pool)
            .await?;

        Ok(row.try_get(0)?)
    }

    /// Link a page to a performer via the cast table.
    ///
    /// # Errors
    /// Returns error if database queries fail.
    pub async fn link_cast(&self, page_id: i64, performer_id: i64, starring: i32) -> Result<()> {
        sqlx::query(
            r"
            INSERT OR IGNORE INTO cast (page_id, performer_id, starring)
            VALUES (?, ?, ?)
            ",
        )
        .bind(page_id)
        .bind(performer_id)
        .bind(starring)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    // ============================================================
    // Rompla: studios (replaces taxonomies type=2)
    // ============================================================

    /// Upsert a studio and return its ID.
    ///
    /// # Errors
    /// Returns error if database queries fail.
    pub async fn upsert_studio(&self, name: &str, url: &str) -> Result<i64> {
        sqlx::query("INSERT INTO studios (name, url) VALUES (?, ?) ON CONFLICT DO NOTHING")
            .bind(name)
            .bind(url)
            .execute(&self.pool)
            .await?;

        let row = sqlx::query("SELECT id FROM studios WHERE url = ? OR name = ? LIMIT 1")
            .bind(url)
            .bind(name)
            .fetch_one(&self.pool)
            .await?;

        Ok(row.try_get(0)?)
    }

    /// Link a page to a studio via the `studio_links` table.
    ///
    /// # Errors
    /// Returns error if database queries fail.
    pub async fn link_studio(&self, page_id: i64, studio_id: i64) -> Result<()> {
        sqlx::query(
            r"
            INSERT OR IGNORE INTO studio_links (page_id, studio_id)
            VALUES (?, ?)
            ",
        )
        .bind(page_id)
        .bind(studio_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    // ============================================================
    // Page operations
    // ============================================================

    /// Upsert a page (for scraped scenes) and return its ID.
    ///
    /// # Errors
    /// Returns error if database queries fail.
    pub async fn upsert_page(&self, url: &str, title: &str) -> Result<i64> {
        let url = url.trim_end_matches('/');

        sqlx::query(
            r"
            INSERT INTO pages (site_id, url, title)
            VALUES (0, ?, ?)
            ON CONFLICT(site_id, url) DO UPDATE SET
                title = excluded.title
            ",
        )
        .bind(url)
        .bind(title)
        .execute(&self.pool)
        .await?;

        let row = sqlx::query("SELECT id FROM pages WHERE site_id = 0 AND url = ?")
            .bind(url)
            .fetch_one(&self.pool)
            .await?;

        Ok(row.try_get(0)?)
    }
}

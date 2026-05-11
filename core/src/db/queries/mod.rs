pub mod library;
pub mod media;
pub mod performers;
pub mod studios;

use crate::db::Database;
use crate::db::models::LibraryItem;
use sqlx::Row;

impl Database {
    pub(crate) fn map_to_library_item(
        &self,
        row: &sqlx::sqlite::SqliteRow,
        finished: i64,
        failed: i64,
    ) -> Result<LibraryItem, sqlx::Error> {
        let id: i64 = row.try_get(0)?;
        let title: String = row.try_get(1)?;
        let thumb_status: i64 = row.try_get::<Option<i64>, _>(2)?.unwrap_or(0);
        let preview_status: i64 = row.try_get::<Option<i64>, _>(3)?.unwrap_or(0);

        let local_image = if thumb_status == crate::constants::status::DONE {
            let p = self.absolutize_path(&format!(
                "thumbs/{id:0width$}.jpg",
                width = crate::constants::ui::PAD_WIDTH
            ));
            tracing::debug!("Item {id} generated path: {p}");
            Some(p)
        } else {
            tracing::debug!(
                "Item {id} has thumb_status {thumb_status}, expected {}",
                crate::constants::status::DONE
            );
            None
        };

        let local_preview = if preview_status == crate::constants::status::DONE {
            Some(self.absolutize_path(&format!(
                "previews/{id:0width$}.mp4",
                width = crate::constants::ui::PAD_WIDTH
            )))
        } else {
            None
        };

        Ok(LibraryItem {
            id,
            title,
            url: String::new(),
            image: None,
            local_image,
            finished_videos: finished,
            failed_videos: failed,
            local_preview,
            ..Default::default()
        })
    }
}


import logging
import sqlite3
from pathlib import Path
from soromantic_utils.common import get_db_connection, get_download_dir

def sync_thumbs():
    logging.basicConfig(level=logging.INFO)
    log = logging.getLogger("sync_thumbs")
    
    thumbs_dir = get_download_dir() / "thumbs"
    if not thumbs_dir.exists():
        log.error(f"Thumbs directory not found: {thumbs_dir}")
        return

    with get_db_connection() as conn:
        cur = conn.cursor()
        
        # Find pages with thumb_status != 3
        cur.execute("SELECT id FROM pages WHERE thumb_status != 3")
        rows = cur.fetchall()
        log.info(f"Checking {len(rows)} pages for existing thumbnails...")
        
        synced_count = 0
        for (pid,) in rows:
            thumb_path = thumbs_dir / f"{pid:06}.jpg"
            if thumb_path.exists():
                cur.execute("UPDATE pages SET thumb_status = 3 WHERE id = ?", (pid,))
                synced_count += 1
                if synced_count % 100 == 0:
                    log.info(f"Synced {synced_count} thumbnails...")
                    conn.commit()
        
        conn.commit()
        log.info(f"Sync complete. Updated {synced_count} pages to thumb_status = 3.")

if __name__ == "__main__":
    sync_thumbs()

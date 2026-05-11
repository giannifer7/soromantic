
import sqlite3
import logging
from pathlib import Path

# Configure logging
logging.basicConfig(level=logging.INFO, format="%(asctime)s - %(levelname)s - %(message)s")
log = logging.getLogger(__name__)

DB_PATH = Path("/mnt/sda3/porn/pyssvids/db/data.db")

def normalize_relations():
    if not DB_PATH.exists():
        log.error(f"Database not found at {DB_PATH}")
        return

    log.info(f"Connecting to {DB_PATH}...")
    conn = sqlite3.connect(DB_PATH)
    cur = conn.cursor()

    try:
        # 1. Fetch all pages with related_ids
        log.info("Fetching pages with related_ids...")
        cur.execute("SELECT id, related_ids FROM pages WHERE related_ids IS NOT NULL AND related_ids != ''")
        rows = cur.fetchall()

        total_relations = 0
        pages_processed = 0
        batch_data = []
        batch_size = 1000

        # 2. Parse and Prepare Inserts
        cur.execute("DELETE FROM page_relations") # Clear existing to be safe/idempotent
        conn.commit()

        log.info(f"Processing {len(rows)} pages...")

        for row in rows:
            source_id = row[0]
            related_str = row[1]
            
            # Parse ",1,2,3," format
            try:
                # filter(None) removes empty strings from split like ['', '1', '2', '']
                target_ids = [int(x) for x in related_str.split(',') if x.strip()]
            except ValueError:
                log.warning(f"Skipping malformed related_ids for page {source_id}: {related_str}")
                continue

            for target_id in target_ids:
                batch_data.append((source_id, target_id))
                total_relations += 1

            pages_processed += 1
            
            if len(batch_data) >= batch_size:
                cur.executemany("INSERT OR IGNORE INTO page_relations (source_id, target_id) VALUES (?, ?)", batch_data)
                batch_data = []
                conn.commit()
                if pages_processed % 1000 == 0:
                    log.info(f"Processed {pages_processed} pages, {total_relations} relations...")

        # Final batch
        if batch_data:
            cur.executemany("INSERT OR IGNORE INTO page_relations (source_id, target_id) VALUES (?, ?)", batch_data)
            conn.commit()

        log.info(f"Migration Complete. Processed {pages_processed} pages. Total relations created: {total_relations}")

    except Exception as e:
        log.error(f"Migration failed: {e}")
        conn.rollback()
    finally:
        conn.close()

if __name__ == "__main__":
    normalize_relations()

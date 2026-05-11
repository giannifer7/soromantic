import logging
import sys
from collections import defaultdict
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

from soromantic_utils.common import get_db_connection, load_config


@dataclass
class Config:
    verbose: bool = False
    batch_size: int = 1000
    video_preferences: list[int] = field(default_factory=lambda: [480, 720, 1080, 360])


def get_best_resolution(resolutions: list[int], preferences: list[int]) -> int | None:
    if not resolutions:
        return None
    
    available = set(resolutions)
    
    # Try preferred resolutions in order
    for pref in preferences:
        if pref in available:
            return pref
            
    # Fallback to max resolution if none of the preferences match
    # (Though typically one would match if preferences covers all bases, but good to be safe)
    return max(resolutions)


def migrate(config: Config, log: logging.Logger) -> None:
    log.info("Starting migration: add_has_video_column")
    
    with get_db_connection() as conn:
        cursor = conn.cursor()
        
        # 1. Check/Add Column
        cursor.execute("PRAGMA table_info(pages)")
        columns = {row[1] for row in cursor.fetchall()}
        
        if "has_video" not in columns:
            log.info("Adding 'has_video' column to pages table...")
            cursor.execute("ALTER TABLE pages ADD COLUMN has_video TEXT")
        else:
            log.info("'has_video' column already exists.")

        # 2. Fetch all pages and their video sources
        log.info("Fetching video sources...")
        # We only care about pages that actually HAVE video sources.
        # Pages without video sources will just have NULL has_video, which is default.
        # But wait, default for existing rows in SQLite ADD COLUMN is NULL? Yes.
        # So we only need to update rows that have video sources.
        
        cursor.execute("SELECT page_id, resolution FROM video_sources WHERE resolution IS NOT NULL")
        sources = cursor.fetchall()
        
        page_resolutions: dict[int, list[int]] = defaultdict(list)
        for page_id, res in sources:
            page_resolutions[page_id].append(res)
            
        log.info(f"Found {len(sources)} video sources across {len(page_resolutions)} pages.")
        
        # 3. Calculate best resolution for each page
        updates: list[tuple[str, int]] = []
        for page_id, res_list in page_resolutions.items():
            best = get_best_resolution(res_list, config.video_preferences)
            if best is not None:
                updates.append((str(best), page_id))
                
        # 4. Batch Update
        log.info(f"Updating {len(updates)} pages...")
        
        total_batches = (len(updates) + config.batch_size - 1) // config.batch_size
        
        for i in range(0, len(updates), config.batch_size):
            batch = updates[i : i + config.batch_size]
            cursor.executemany("UPDATE pages SET has_video = ? WHERE id = ?", batch)
            
            if (i // config.batch_size) % 10 == 0:
                log.info(f"Processed batch {i // config.batch_size + 1}/{total_batches}")
                
        conn.commit()
        log.info("Migration complete.")


def main() -> None:
    logging.basicConfig(
        level=logging.INFO,
        stream=sys.stderr,
        format="%(asctime)s - %(levelname)s - %(message)s"
    )
    log = logging.getLogger(__name__)

    raw_config = load_config()
    
    # Extract preferences from config part
    prefs = raw_config.get("playback", {}).get("video_preferences")
    
    config = Config()
    if prefs:
        config.video_preferences = prefs
        
    log.info(f"Using video preferences: {config.video_preferences}")

    try:
        migrate(config, log)
    except Exception:
        log.exception("Migration failed")
        sys.exit(1)


if __name__ == "__main__":
    main()

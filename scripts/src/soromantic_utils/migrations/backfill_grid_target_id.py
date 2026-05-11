#!/usr/bin/env python3
"""
Backfill script to populate target_page_id in grid_boxes.
Matches grid_boxes.related (URL) with pages.url to get the id.
"""

import sqlite3
import sys
import os

from soromantic_utils.common import get_db_path, load_config

def backfill(db_path: str) -> None:
    print(f"Connecting to database: {db_path}")
    conn = sqlite3.connect(db_path)
    cursor = conn.cursor()

    # Get total count of records to update
    cursor.execute("SELECT COUNT(*) FROM grid_boxes WHERE related IS NOT NULL AND target_page_id IS NULL")
    total = cursor.fetchone()[0]
    print(f"Found {total} records to backfill.")

    if total == 0:
        print("Nothing to backfill.")
        conn.close()
        return

    # Perform the update in batches for better performance and progress reporting
    batch_size = 5000
    updated_total = 0

    while True:
        # We use a subquery for the update. 
        # SQLite 3.33+ supports UPDATE FROM, but subquery is more compatible.
        cursor.execute(f"""
            UPDATE grid_boxes
            SET target_page_id = (
                SELECT id FROM pages WHERE pages.url = grid_boxes.related
            )
            WHERE id IN (
                SELECT id FROM grid_boxes 
                WHERE related IS NOT NULL AND target_page_id IS NULL 
                LIMIT {batch_size}
            )
        """)
        
        affected = cursor.rowcount
        if affected == 0:
            break
            
        updated_total += affected
        conn.commit()
        print(f"  Updated {updated_total}/{total}...")

    print("Backfill complete.")
    conn.close()

def main() -> int:
    config = load_config()
    db_path = get_db_path(config)

    if not db_path:
        print("Error: Could not determine database path from config.")
        return 1

    if not os.path.exists(db_path):
        print(f"Error: Database file does not exist at {db_path}")
        return 1

    backfill(db_path)
    return 0

if __name__ == "__main__":
    sys.exit(main())


import argparse
from pathlib import Path
from soromantic_utils.common import get_db_connection, load_config

def migrate_db(config: dict | None = None):
    print("Migrating duration to REAL...")
    
    with get_db_connection(config) as conn:
        cursor = conn.cursor()
        
        # Check current schema to see if migration is needed?
        # Only if duration is TEXT. But SQLite PRAGMA table_info gives type.
        cursor.execute("PRAGMA table_info(video_sources)")
        cols = {row[1]: row[2] for row in cursor.fetchall()}
        
        if cols.get("duration") == "REAL":
            print("duration column is already REAL. Skipping.")
            return

        print("Converting video_sources.duration from TEXT to REAL...")

        # 1. Rename old table
        cursor.execute("ALTER TABLE video_sources RENAME TO video_sources_old")
        
        # 2. Create new table
        cursor.execute("""
            CREATE TABLE video_sources (
                id INTEGER PRIMARY KEY,
                page_id INTEGER,
                url TEXT,
                resolution INTEGER,
                duration REAL,
                codec TEXT,
                UNIQUE(page_id, url)
            )
        """)
        
        # 3. Copy data
        cursor.execute("SELECT id, page_id, url, resolution, duration, codec FROM video_sources_old")
        rows = cursor.fetchall()
        
        count = 0
        for row in rows:
            vid_id, page_id, url, res, _, codec = row
            # Discard inaccurate legacy duration (set to None)
            dur_real = None
            
            cursor.execute(
                "INSERT INTO video_sources (id, page_id, url, resolution, duration, codec) VALUES (?, ?, ?, ?, ?, ?)",
                (vid_id, page_id, url, res, dur_real, codec)
            )
            count += 1
            
        print(f"Migrated {count} rows.")
            
        # 4. Recreate indexes
        cursor.execute("CREATE INDEX IF NOT EXISTS idx_video_sources_page_id ON video_sources(page_id)")
        cursor.execute("CREATE INDEX IF NOT EXISTS idx_video_sources_url ON video_sources(url)")
        
        # 5. Drop old table
        cursor.execute("DROP TABLE video_sources_old")
        
        conn.commit()

    print("Migration complete.")

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Migrate video_sources.duration to REAL")
    parser.add_argument("--db-path", type=Path, help="Path to sqlite database")
    
    args = parser.parse_args()
    
    config = None
    if args.db_path:
        config = {"runtime": {"db_path": str(args.db_path)}}
    else:
        config = load_config()
    
    migrate_db(config)

import sqlite3
import argparse
from pathlib import Path

def migrate(db_path: Path):
    print(f"Migrating database at {db_path}...")
    conn = sqlite3.connect(db_path)
    cursor = conn.cursor()

    try:
        # Check if url column exists
        cursor.execute("PRAGMA table_info(video_sources)")
        columns = [row[1] for row in cursor.fetchall()]
        
        if "url" not in columns:
            print("Migration already applied: 'url' column not found in video_sources.")
            return

        print("Removing 'url' column from video_sources...")
        
        cursor.execute("BEGIN TRANSACTION")

        # Create new table without url
        cursor.execute("""
            CREATE TABLE video_sources_new (
                id INTEGER PRIMARY KEY,
                page_id INTEGER,
                resolution INTEGER,
                duration REAL,
                codec TEXT,
                start_time REAL DEFAULT 0.0,
                stop_time REAL DEFAULT 0.0,
                status INTEGER DEFAULT 0,
                UNIQUE(page_id, resolution)
            )
        """)

        # Copy data, deduplicating by (page_id, resolution)
        # We group by page_id, resolution to pick one row per pair.
        # SQLite's GROUP BY behavior picks an arbitrary row, which is fine for deduplication here.
        cursor.execute("""
            INSERT INTO video_sources_new (page_id, resolution, duration, codec, start_time, stop_time, status)
            SELECT page_id, resolution, duration, codec, start_time, stop_time, status
            FROM video_sources
            GROUP BY page_id, resolution
        """)

        cursor.execute("DROP TABLE video_sources")
        cursor.execute("ALTER TABLE video_sources_new RENAME TO video_sources")

        cursor.execute("CREATE INDEX idx_video_sources_page_id ON video_sources(page_id)")
        cursor.execute("CREATE INDEX idx_video_sources_page_status ON video_sources(page_id, status)")

        conn.commit()
        print("Migration successful.")

    except Exception as e:
        conn.rollback()
        print(f"Migration failed: {e}")
        raise
    finally:
        conn.close()

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Remove url column from video_sources")
    parser.add_argument("db_path", type=Path, help="Path to sqlite database")
    args = parser.parse_args()
    
    migrate(args.db_path)

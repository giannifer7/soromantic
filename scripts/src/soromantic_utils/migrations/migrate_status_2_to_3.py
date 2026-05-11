import sqlite3
import argparse
from pathlib import Path

def migrate(db_path: Path):
    print(f"Migrating video_sources status at {db_path}...")
    conn = sqlite3.connect(db_path)
    cursor = conn.cursor()

    try:
        # Check counts before
        cursor.execute("SELECT count(*) FROM video_sources WHERE status = 2")
        count = cursor.fetchone()[0]
        print(f"Found {count} rows with status = 2.")

        if count == 0:
            print("No rows to update.")
            return

        cursor.execute("BEGIN TRANSACTION")

        # Update status 2 -> 3
        cursor.execute("UPDATE video_sources SET status = 3 WHERE status = 2")
        
        updated_count = cursor.rowcount
        print(f"Updated {updated_count} rows to status 3.")

        conn.commit()
        print("Migration successful.")

    except Exception as e:
        conn.rollback()
        print(f"Migration failed: {e}")
        raise
    finally:
        conn.close()

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Migrate video_sources status 2 to 3")
    parser.add_argument("db_path", type=Path, help="Path to sqlite database")
    args = parser.parse_args()
    
    migrate(args.db_path)

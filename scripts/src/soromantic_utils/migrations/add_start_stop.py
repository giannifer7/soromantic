
import argparse
from pathlib import Path
from soromantic_utils.common import get_db_connection, load_config

def migrate_db(config: dict | None = None):
    print("Migrating database...")

    with get_db_connection(config) as conn:
        cursor = conn.cursor()

        # Check if columns exist
        cursor.execute("PRAGMA table_info(pages)")
        columns = [row[1] for row in cursor.fetchall()]

        if "start_time" not in columns:
            print("Adding start_time column...")
            cursor.execute("ALTER TABLE pages ADD COLUMN start_time REAL")
        else:
            print("start_time column already exists.")

        if "stop_time" not in columns:
            print("Adding stop_time column...")
            cursor.execute("ALTER TABLE pages ADD COLUMN stop_time REAL")
        else:
            print("stop_time column already exists.")

        conn.commit()

    print("Migration complete.")

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Migrate Soromantic database schema")
    parser.add_argument("--db-path", type=Path, help="Path to sqlite database")
    
    args = parser.parse_args()
    
    if args.db_path:
        # Construct a config dict that satisfies common.get_db_path expectations
        config = {"runtime": {"db_path": str(args.db_path)}}
    else:
        config = load_config()
    
    migrate_db(config)

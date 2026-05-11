import sqlite3
from soromantic_utils.common import get_db_connection, load_config

def migrate_intervals():
    print("Migrating intervals from Pages to Video Sources...")
    config = load_config()
    
    with get_db_connection(config) as conn:
        cursor = conn.cursor()
        
        # 1. Add columns to video_sources if they don't exist
        print("Adding columns to video_sources...")
        try:
            cursor.execute("ALTER TABLE video_sources ADD COLUMN start_time REAL DEFAULT 0.0")
        except sqlite3.OperationalError:
            pass # Already exists
            
        try:
            cursor.execute("ALTER TABLE video_sources ADD COLUMN stop_time REAL DEFAULT 0.0")
        except sqlite3.OperationalError:
            pass # Already exists
            
        # 2. Copy data from pages to video_sources
        print("Copying interval data...")
        # We update video_sources matching the page_id
        # Set start_time = pages.start_time, stop_time = pages.stop_time
        # But only where pages have values
        
        # SQLite update with join support is limited, using subquery approach
        cursor.execute("""
            UPDATE video_sources 
            SET start_time = (SELECT start_time FROM pages WHERE pages.id = video_sources.page_id),
                stop_time = (SELECT stop_time FROM pages WHERE pages.id = video_sources.page_id)
            WHERE page_id IN (SELECT id FROM pages WHERE start_time > 0 OR stop_time > 0)
        """)
        
        print(f"Rows updated: {cursor.rowcount}")
        
        # 3. Drop columns from pages
        # Requires SQLite 3.35+, strictly check or try. 
        # If older, we need to recreate the table, which is risky in a script.
        # Let's assume modern sqlite for linux user.
        print("Dropping columns from pages...")
        try:
            cursor.execute("ALTER TABLE pages DROP COLUMN start_time")
            cursor.execute("ALTER TABLE pages DROP COLUMN stop_time")
        except sqlite3.OperationalError as e:
            print(f"Could not drop columns (might need newer SQLite): {e}")
            print("Please drop 'start_time' and 'stop_time' from 'pages' manually or update SQLite.")
            return

        conn.commit()
        print("Migration complete. Schema updated.")

if __name__ == "__main__":
    migrate_intervals()

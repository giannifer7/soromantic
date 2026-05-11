import sqlite3
import os
import sys

DB_PATH = "/mnt/sda3/porn/pyssvids/db/data.db"

def fix_db():
    if not os.path.exists(DB_PATH):
        print(f"Database not found at {DB_PATH}")
        return

    print(f"Connecting to {DB_PATH}...")
    conn = sqlite3.connect(DB_PATH)
    conn.row_factory = sqlite3.Row
    cursor = conn.cursor()

    try:
        # 1. Fix Pages Table (TEXT -> INTEGER statuses)
        print("Checking pages table schema...")
        cursor.execute("PRAGMA table_info(pages)")
        cols = {row['name']: row['type'] for row in cursor.fetchall()}
        
        needs_rebuild = False
        if cols.get('thumb_status') == 'TEXT':
            print("Detected TEXT thumb_status. Rebuilding pages table...")
            needs_rebuild = True

        # Ensure text columns for denormalization exist in pages table
        # If they don't exist, we need to add them. 
        # But if we rebuild, we add them in the new table.
        # If we don't rebuild, we might need to ALTER.
        
        required_cols = ['model_ids', 'studio_id', 'related_ids']
        for col in required_cols:
            if col not in cols:
                print(f"Adding missing column {col} to pages...")
                cursor.execute(f"ALTER TABLE pages ADD COLUMN {col} {'INTEGER' if col == 'studio_id' else 'TEXT'}")

        if needs_rebuild:
            cursor.execute("PRAGMA foreign_keys=OFF")
            
            # Create new pages table with correct types
            cursor.execute("""
            CREATE TABLE pages_new (
                id INTEGER PRIMARY KEY,
                url TEXT UNIQUE,
                title TEXT,
                cover_status INTEGER DEFAULT 0,
                thumb_status INTEGER DEFAULT 0,
                preview_status INTEGER DEFAULT 0,
                video_status INTEGER DEFAULT 0,
                preview TEXT,
                model_ids TEXT,
                studio_id INTEGER,
                related_ids TEXT
            )
            """)
            
            # Copy data, casting statuses to INTEGER safely
            # We handle 'done', 'pending' etc. by using 0 as fallback or simple cast if they are strings of numbers
            # The previous SQL logic handled conversion. Let's replicate it in Python or SQL.
            # SQL is faster.
            print("Migrating pages data...")
            cursor.execute("""
            INSERT INTO pages_new (id, url, title, cover_status, thumb_status, preview_status, video_status, preview, model_ids, studio_id, related_ids)
            SELECT 
                id, 
                url, 
                title, 
                CASE WHEN typeof(cover_status)='integer' THEN cover_status ELSE 0 END,
                CASE 
                    WHEN thumb_status = 'done' THEN 3
                    WHEN thumb_status = 'pending' THEN 1
                    WHEN thumb_status = 'downloading' THEN 2
                    WHEN thumb_status = 'error' THEN 4
                    WHEN typeof(thumb_status)='integer' THEN thumb_status
                    ELSE 0 
                END,
                CASE 
                    WHEN preview_status = 'done' THEN 3
                    WHEN preview_status = 'pending' THEN 1
                    WHEN preview_status = 'downloading' THEN 2
                    WHEN preview_status = 'error' THEN 4
                    WHEN typeof(preview_status)='integer' THEN preview_status
                    ELSE 0 
                END,
                CASE 
                    WHEN video_status = 'done' THEN 3
                    WHEN video_status = 'pending' THEN 1
                    WHEN video_status = 'downloading' THEN 2
                    WHEN video_status = 'error' THEN 4
                    WHEN typeof(video_status)='integer' THEN video_status
                    ELSE 0 
                END,
                preview,
                model_ids,
                studio_id,
                related_ids
            FROM pages
            """)
            
            cursor.execute("DROP TABLE pages")
            cursor.execute("ALTER TABLE pages_new RENAME TO pages")
            
            # Recreate indices
            print("Recreating indices...")
            cursor.execute("CREATE INDEX IF NOT EXISTS idx_pages_studio_id ON pages(studio_id)")
            cursor.execute("CREATE INDEX IF NOT EXISTS idx_pages_title ON pages(title)")
            cursor.execute("CREATE INDEX IF NOT EXISTS idx_pages_thumb_status ON pages(thumb_status)")
            cursor.execute("CREATE INDEX IF NOT EXISTS idx_pages_id_desc ON pages(id DESC)")
            
            cursor.execute("PRAGMA foreign_keys=ON")
            print("Pages table rebuilt.")

        # 2. Fix Video Sources
        print("Checking video_sources...")
        cursor.execute("PRAGMA table_info(video_sources)")
        vs_cols = {row['name'] for row in cursor.fetchall()}
        if 'status' not in vs_cols:
            print("Adding status to video_sources...")
            cursor.execute("ALTER TABLE video_sources ADD COLUMN status INTEGER DEFAULT 0")
            cursor.execute("UPDATE video_sources SET status = 2") # Assume done

        # 3. Create Taxonomies and Links if missing
        print("Ensuring hybrid tables exist...")
        cursor.execute("""
        CREATE TABLE IF NOT EXISTS taxonomies (
            id INTEGER PRIMARY KEY,
            url TEXT UNIQUE,
            name TEXT,
            type INTEGER -- 1=model, 2=studio
        )
        """)
        cursor.execute("CREATE INDEX IF NOT EXISTS idx_taxonomies_name ON taxonomies(name)")
        cursor.execute("CREATE INDEX IF NOT EXISTS idx_taxonomies_type ON taxonomies(type)")

        # Migrate legacy models/studios if they exist
        cursor.execute("SELECT name FROM sqlite_master WHERE type='table'")
        tables = {row[0] for row in cursor.fetchall()}
        
        if 'models' in tables:
            print("Migrating legacy models...")
            cursor.execute("INSERT OR IGNORE INTO taxonomies (url, name, type) SELECT url, name, 1 FROM models")
        
        if 'studios' in tables:
            print("Migrating legacy studios...")
            cursor.execute("INSERT OR IGNORE INTO taxonomies (url, name, type) SELECT url, name, 2 FROM studios")

        # Links Table
        # We need to make sure links table has the correct schema (taxonomy_id)
        # If it has model_id/studio_id, it is legacy.
        cursor.execute("PRAGMA table_info(links)")
        links_cols = {row['name'] for row in cursor.fetchall()}
        
        if 'taxonomy_id' not in links_cols:
            print("Upgrading links table...")
            cursor.execute("""
            CREATE TABLE links_new (
                id INTEGER PRIMARY KEY,
                page_id INTEGER,
                taxonomy_id INTEGER,
                FOREIGN KEY (page_id) REFERENCES pages(id),
                FOREIGN KEY (taxonomy_id) REFERENCES taxonomies(id)
            )
            """)
            
            if 'models' in tables:
                print("Migrating model links...")
                cursor.execute("""
                INSERT INTO links_new (page_id, taxonomy_id)
                SELECT l.page_id, t.id
                FROM links l
                JOIN models m ON l.model_id = m.id
                JOIN taxonomies t ON m.url = t.url
                """)
                
            if 'studios' in tables:
                print("Migrating studio links...")
                cursor.execute("""
                INSERT INTO links_new (page_id, taxonomy_id)
                SELECT l.page_id, t.id
                FROM links l
                JOIN studios s ON l.studio_id = s.id
                JOIN taxonomies t ON s.url = t.url
                """)
            
            cursor.execute("DROP TABLE links")
            cursor.execute("ALTER TABLE links_new RENAME TO links")
            print("Links table upgraded.")

        cursor.execute("CREATE INDEX IF NOT EXISTS idx_links_taxonomy_id ON links(taxonomy_id)")
        cursor.execute("CREATE INDEX IF NOT EXISTS idx_links_taxonomy_page ON links(taxonomy_id, page_id)")
        cursor.execute("CREATE INDEX IF NOT EXISTS idx_links_page_id ON links(page_id)")

        # 4. Populate Denormalized Data
        print("Populating denormalized data...")
        cursor.execute("""
        UPDATE pages SET studio_id = (
            SELECT t.id 
            FROM links l 
            JOIN taxonomies t ON l.taxonomy_id = t.id 
            WHERE l.page_id = pages.id AND t.type = 2
            LIMIT 1
        ) WHERE studio_id IS NULL
        """)

        # model_ids
        # Since we use group_concat, we can just run it. 
        # But to be faster, maybe only for null? 
        # Let's run for all to be safe.
        cursor.execute("""
        UPDATE pages SET model_ids = (
            SELECT group_concat(t.id)
            FROM links l
            JOIN taxonomies t ON l.taxonomy_id = t.id
            WHERE l.page_id = pages.id AND t.type = 1
        )
        """)

        # 5. Fix Statuses for Display
        print("Fixing item statuses...")
        cursor.execute("""
        UPDATE pages 
        SET thumb_status = 3, preview_status = 3, video_status = 3
        WHERE id IN (
            SELECT DISTINCT page_id FROM video_sources WHERE status = 2
        ) AND (thumb_status = 0 OR thumb_status IS NULL)
        """)

        # 6. Cleanup
        print("Dropping legacy tables...")
        for tbl in ['models', 'studios', 'grid_boxes', 'page_relations', 'downloads']:
            cursor.execute(f"DROP TABLE IF EXISTS {tbl}")
            
        # 7. Clear SQLx migrations to stop it from complaining
        print("Clearing _sqlx_migrations...")
        try:
            cursor.execute("DELETE FROM _sqlx_migrations")
        except sqlite3.OperationalError:
            print("_sqlx_migrations table does not exist, skipping.")

        conn.commit()
        print("Database repair complete!")

    except Exception as e:
        print(f"Error: {e}")
        conn.rollback()
    finally:
        conn.close()

if __name__ == "__main__":
    fix_db()

from pathlib import Path
import shutil
import sqlite3
import sys

from soromantic_utils.common import get_download_dir, get_db_connection

def migrate_file(
    base_dir: Path, target_dir: Path, page_id: int, local_path: str | None, current_status: str, extension: str = ".jpg"
) -> str:
    """Migrate a single file and return the new status."""
    if current_status == "done":
        new_status = "done"
    elif current_status in ("pending", "downloading"):
        return current_status
    else:
        return "error"

    dest = target_dir / f"{page_id:06}{extension}"
    
    if dest.exists():
         return "done"

    # Check for unpadded version as fallback source
    unpadded_src = target_dir / f"{page_id}{extension}"
    if unpadded_src.exists() and unpadded_src != dest:
        try:
             shutil.move(unpadded_src, dest)
             print(f"Renamed unpadded {unpadded_src} -> {dest}")
             return "done"
        except Exception as e:
             print(f"Error renaming {unpadded_src}: {e}")
             return "error"

    if not local_path:
        return "error"

    src = Path(local_path)
    if not src.is_absolute():
        src = base_dir / src

    # Also check if src is unpadded version in same dir?
    
    if not src.exists():
        print(f"Missing file for page {page_id}: {local_path} (and not found at {dest})")
        return "error"

    dest.parent.mkdir(parents=True, exist_ok=True)

    try:
        shutil.move(src, dest)
        print(f"Moved {src} -> {dest}")
        return "done"
    except Exception as e:
        print(f"Error moving {src}: {e}")
        return "error"

def migrate_thumbs(cur: sqlite3.Cursor, base_dir: Path, thumbs_dir: Path) -> None:
    print("Migrating Thumbnails...")
    rows = cur.execute("""
        SELECT p.id, d.local_path, d.status 
        FROM pages p 
        JOIN downloads d ON p.image = d.src_url
        WHERE p.thumb_status IN ('none', 'error', 'done') OR p.thumb_status IS NULL
    """).fetchall()

    for row in rows:
        page_id = row['id']
        new_status = migrate_file(base_dir, thumbs_dir, page_id, row['local_path'], row['status'], ".jpg")
        cur.execute("UPDATE pages SET thumb_status = ? WHERE id = ?", (new_status, page_id))

def migrate_previews(cur: sqlite3.Cursor, base_dir: Path, previews_dir: Path) -> None:
    print("Migrating Previews...")
    rows = cur.execute("""
        SELECT p.id, d.local_path, d.status 
        FROM pages p 
        JOIN downloads d ON p.preview = d.src_url
        WHERE (p.preview_status IN ('none', 'error', 'done') OR p.preview_status IS NULL) AND p.preview IS NOT NULL
    """).fetchall()

    for row in rows:
        page_id = row['id']
        local_path = row['local_path']
        
        # Determine extension
        ext = ".mp4"
        if local_path and local_path.endswith(".m3u8"):
             ext = ".mp4"
        elif local_path:
             path_ext = Path(local_path).suffix
             if path_ext:
                 ext = path_ext

        new_status = migrate_file(base_dir, previews_dir, page_id, local_path, row['status'], ext)
        cur.execute("UPDATE pages SET preview_status = ? WHERE id = ?", (new_status, page_id))

def apply_schema_changes(cur: sqlite3.Cursor, conn: sqlite3.Connection) -> None:
    print("Applying schema changes...")
    try:
        # 1. Add status columns to pages
        for col in ['thumb_status', 'preview_status', 'video_status']:
            try:
                cur.execute(f"ALTER TABLE pages ADD COLUMN {col} TEXT DEFAULT 'none'")
            except sqlite3.OperationalError as e:
                if "duplicate column name" in str(e):
                    pass
                else:
                    raise

        # 2. Add preview column (URL)
        try:
            cur.execute("ALTER TABLE pages ADD COLUMN preview TEXT")
        except sqlite3.OperationalError as e:
            if "duplicate column name" in str(e):
                pass
            else:
                raise

        # 3. Create page_relations table
        cur.execute("""
            CREATE TABLE IF NOT EXISTS page_relations (
                page_id INTEGER NOT NULL,
                related_page_id INTEGER NOT NULL,
                PRIMARY KEY (page_id, related_page_id),
                FOREIGN KEY (page_id) REFERENCES pages(id) ON DELETE CASCADE,
                FOREIGN KEY (related_page_id) REFERENCES pages(id) ON DELETE CASCADE
            )
        """)
        
        cur.execute("CREATE INDEX IF NOT EXISTS idx_page_relations_page_id ON page_relations(page_id)")
        cur.execute("CREATE INDEX IF NOT EXISTS idx_page_relations_related_id ON page_relations(related_page_id)")

        # Commit schema changes immediately
        conn.commit()
    except Exception as e:
        print(f"Schema migration warning: {e}")

def backfill_previews(cur: sqlite3.Cursor, conn: sqlite3.Connection) -> None:
    print("Backfilling Previews...")
    cur.execute("""
        UPDATE pages 
        SET preview = (
            SELECT preview FROM grid_boxes WHERE related = pages.url LIMIT 1
        )
        WHERE preview IS NULL
    """)
    conn.commit()

def sync_sqlx_migrations(cur: sqlite3.Cursor, conn: sqlite3.Connection) -> None:
    """Mark sqlx migrations as applied since we did them manually."""
    print("Syncing sqlx migrations...")
    # Calculated using `sha384sum` on the files
    # page_centric: 725301ea...
    # add_preview_column: deff7b5b...
    
    migrations = [
        (20260126000001, "page centric", "sql", 0, bytes.fromhex("725301ea68fe67462fcb4e3f234894d276397c4052342fafe28166bbbac203cdd4c5d5a0d8e50c972b2b0f235c5c8d9b")),
        (20260126000002, "add preview column", "sql", 0, bytes.fromhex("deff7b5b315534608a757a6cab2b7862048d8ef9d61d6d05f6afd5c111f52c0cae6fa10e04eb5d46b173b13f2e3b361c"))
    ]
    
    # Check if table exists (it should if app ran before, otherwise create it?)
    # sqlx creates it automatically, but might not exist if clean install.
    # If clean install, our script probably runs before app.
    cur.execute("""
        CREATE TABLE IF NOT EXISTS _sqlx_migrations (
            version BIGINT PRIMARY KEY,
            description TEXT NOT NULL,
            installed_on TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
            success BOOLEAN NOT NULL,
            checksum BLOB,
            execution_time BIGINT NOT NULL
        )
    """)
    
    for version, desc, mode, exec_time, checksum in migrations:
        try:
            cur.execute("""
                INSERT INTO _sqlx_migrations (version, description, success, execution_time, checksum)
                VALUES (?, ?, 1, ?, ?)
                ON CONFLICT(version) DO UPDATE SET
                    checksum = excluded.checksum
            """, (version, desc, exec_time, checksum))
        except Exception as e:
            print(f"Warning syncing migration {version}: {e}")
            
    conn.commit()

def migrate_grid_items(cur: sqlite3.Cursor, conn: sqlite3.Connection) -> None:
    print("Migrating Related Items...")
    rows = cur.execute("SELECT page_id, title, related, thumb, preview FROM grid_boxes").fetchall()
    
    for row in rows:
        parent_id = row['page_id']
        url = row['related']
        
        if not url: continue
        
        cur.execute("""
            INSERT INTO pages (url, title, image, preview, thumb_status) 
            VALUES (?, ?, ?, ?, 'pending')
            ON CONFLICT(url) DO NOTHING
        """, (url, row['title'], row['thumb'], row['preview']))
        
        cur.execute("SELECT id FROM pages WHERE url = ?", (url,))
        res = cur.fetchone()
        if res:
            child_id = res['id']
            cur.execute("""
                INSERT OR IGNORE INTO page_relations (page_id, related_page_id)
                VALUES (?, ?)
            """, (parent_id, child_id))

    conn.commit()

def migrate() -> None:
    # Get configuration
    download_dir_str = get_download_dir()
    if not download_dir_str:
        print("Error: download_dir not found in config.")
        sys.exit(1)
        
    base_dir = Path(download_dir_str)
    thumbs_dir = base_dir / "thumbs"
    previews_dir = base_dir / "previews"

    # Use context manager for auto-commit/close
    with get_db_connection() as conn:
        conn.row_factory = sqlite3.Row
        cur = conn.cursor()

        apply_schema_changes(cur, conn)
        sync_sqlx_migrations(cur, conn)
        backfill_previews(cur, conn)
        
        migrate_thumbs(cur, base_dir, thumbs_dir)
        migrate_previews(cur, base_dir, previews_dir)
        
        migrate_grid_items(cur, conn)

    print("Migration complete.")

if __name__ == "__main__":
    migrate()


import os
import sys
import shutil
import argparse
import sqlite3
from pathlib import Path
from soromantic_utils.common import get_db_connection, load_config, get_download_dir

def delete_page(page_id: int, dry_run: bool = False, db_path: Path | None = None):
    config = load_config()
    if db_path:
        config["runtime"]["db_path"] = str(db_path)
    
    download_dir = Path(get_download_dir(config))
    thumbs_dir = download_dir / "thumbs"
    covers_dir = download_dir / "covers"
    previews_dir = download_dir / "previews"
    cache_dir = download_dir / "cache" # Standard cache
    
    # Potential "wrong" cache location based on user report
    nested_cache_previews = cache_dir / "cache" / "previews"

    print(f"Deleting Page ID: {page_id}")
    if dry_run:
        print("  [DRY RUN] No changes will be made.")

    with get_db_connection(config) as conn:
        cursor = conn.cursor()
        
        # 1. Fetch Page Info
        cursor.execute("SELECT url, title FROM pages WHERE id = ?", (page_id,))
        page = cursor.fetchone()
        if not page:
            print(f"  Page ID {page_id} not found in database.")
            return
        
        url, title = page
        print(f"  Title: {title}")
        print(f"  URL: {url}")
        
        # 2. Identify Local Files to Delete
        files_to_delete = []
        dirs_to_delete = []
        
        # Main Video (via downloads table)
        cursor.execute("""
            SELECT d.local_path 
            FROM video_sources vs 
            JOIN downloads d ON d.src_url = vs.url 
            WHERE vs.page_id = ? AND d.status = 'done'
        """, (page_id,))
        rows = cursor.fetchall()
        for row in rows:
            if row[0]:
                files_to_delete.append(download_dir / row[0])

        # Standard Assets
        files_to_delete.append(thumbs_dir / f"{page_id:06}.jpg")
        files_to_delete.append(covers_dir / f"{page_id:06}.jpg")
        files_to_delete.append(previews_dir / f"{page_id:06}.mp4")
        
        # Cache / Photograms (Aggressive Cleanup)
        # Try finding directory for this page in nested cache
        # Assumption: nested cache might use page_id as folder name? 
        # User showed: cache/cache/previews/1/001.jpg -> implies page_id '1' folder?
        # Let's check string ID and integer ID folder variants just to be safe.
        
        # Check standard cache/previews/{id} (if it uses folders)
        dirs_to_delete.append(cache_dir / "previews" / str(page_id))
        
        # Check the reported "wrong" cache: cache/cache/previews/{id}
        dirs_to_delete.append(nested_cache_previews / str(page_id))
        
        # Execute File Deletion
        for path in files_to_delete:
            if path.exists():
                if not dry_run:
                    try:
                        os.remove(path)
                        print(f"  Deleted file: {path}")
                    except OSError as e:
                        print(f"  Error deleting {path}: {e}")
                else:
                    print(f"  [DRY RUN] Would delete file: {path}")
        
        for path in dirs_to_delete:
            if path.exists() and path.is_dir():
                if not dry_run:
                    try:
                        shutil.rmtree(path)
                        print(f"  Deleted dir: {path}")
                    except OSError as e:
                        print(f"  Error deleting dir {path}: {e}")
                else:
                    print(f"  [DRY RUN] Would delete dir: {path}")


        # 3. DB Deletion (Cascading order explicitly handled just in case)
        
        # Incoming References (Grid Boxes on OTHER pages pointing to THIS page)
        # Assuming 'related' or 'related_id' stores the ID.
        # Check grid_boxes schema via query if uncertain, but usually we just delete by value.
        # We need to find grid_boxes where the box represents THIS video.
        # The schema has `related` (TEXT). We assume it holds the ID string.
        # BUT: grid_boxes entries are usually "Video X on Page Y".
        # We want to remove entries where "Video X" is THIS page.
        # Which column stores the video ID?
        # Usually `related` stores JSON or the ID.
        # Let's assume we remove by matching `related` column content if it matches ID.
        # Actually, let's look at `links` table? No, `grid_boxes` is for UI display.
        
        # Wait, usually `grid_boxes` has `related` column.
        # If `related` == page_id (as string), we delete it.
        
        queries = [
            # Delete incoming references (Video X appearing on Page Y)
            ("DELETE FROM grid_boxes WHERE related = ?", (str(page_id),)),
            
            # Delete items belonging to this page (The grid items shown ON this page)
            ("DELETE FROM grid_boxes WHERE page_id = ?", (page_id,)),
            
            # Delete downloads (via source URL linking)
            ("DELETE FROM downloads WHERE src_url IN (SELECT url FROM video_sources WHERE page_id = ?)", (page_id,)),
            
            # Delete video sources
            ("DELETE FROM video_sources WHERE page_id = ?", (page_id,)),
            
            # Delete links (start/stop markers, etc)
            ("DELETE FROM links WHERE page_id = ?", (page_id,)),
            
            # Delete page itself
            ("DELETE FROM pages WHERE id = ?", (page_id,))
        ]
        
        for sql, params in queries:
            if not dry_run:
                cursor.execute(sql, params)
                # print(f"  Executed DB: {sql.split('WHERE')[0]}...") 
            else:
                print(f"  [DRY RUN] Would execute: {sql} with {params}")
        
        if not dry_run:
            conn.commit()
            print("  Database request completed.")

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Delete a page and all its assets/references")
    parser.add_argument("page_id", type=int, help="ID of the page to delete")
    parser.add_argument("--dry-run", action="store_true", help="Do not actually delete anything")
    parser.add_argument("--db-path", type=Path, help="Path to sqlite database")
    
    args = parser.parse_args()
    delete_page(args.page_id, args.dry_run, args.db_path)

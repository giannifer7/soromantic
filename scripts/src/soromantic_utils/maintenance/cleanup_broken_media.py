import os
import sys
from pathlib import Path

# Add the current directory to sys.path to find soromantic_utils
sys.path.append(str(Path(__file__).parent.parent.parent))

from soromantic_utils.common import get_db_path, get_db_connection, get_download_dir

def cleanup_broken_media():
    db_path = get_db_path()
    download_dir = get_download_dir()
    
    if not download_dir:
        print("Error: Could not determine download_dir from config.")
        return

    print(f"Using database: {db_path}")
    print(f"Download directory: {download_dir}")

    with get_db_connection() as conn:
        cursor = conn.cursor()

        # 1. Identify broken thumbnails in downloads table
        # Files in thumbs/ with .mp4 extension
        cursor.execute("""
            SELECT src_url, local_path 
            FROM downloads 
            WHERE status = 'done' 
            AND local_path LIKE 'thumbs/%.mp4'
        """)
        broken_thumbs = cursor.fetchall()

        if not broken_thumbs:
            print("No broken thumbnails found in downloads table.")
        else:
            print(f"Found {len(broken_thumbs)} broken thumbnails.")
            for src_url, rel_path in broken_thumbs:
                abs_path = os.path.join(download_dir, rel_path)
                
                # Delete file if exists
                if os.path.exists(abs_path):
                    print(f"Deleting file: {abs_path}")
                    try:
                        os.remove(abs_path)
                    except Exception as e:
                        print(f"Failed to delete {abs_path}: {e}")
                
                # Delete database record
                print(f"Deleting DB record for: {src_url}")
                cursor.execute("DELETE FROM downloads WHERE src_url = ?", (src_url,))

        # 2. Identify and clear broken thumbnails in grid_boxes table
        # These are URLs that are actually video previews or page URLs incorrectly assigned as thumbs
        cursor.execute("""
            UPDATE grid_boxes 
            SET thumb = NULL 
            WHERE thumb LIKE '%.mp4%' 
               OR (thumb LIKE 'https://pissvids.com/%' AND thumb LIKE '%/None')
        """)
        updated_grid = cursor.rowcount
        print(f"Updated {updated_grid} grid_boxes entries (set thumb to NULL).")

        conn.commit()
        print("Cleanup complete.")

if __name__ == "__main__":
    cleanup_broken_media()

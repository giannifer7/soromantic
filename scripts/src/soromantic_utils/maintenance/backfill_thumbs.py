#!/usr/bin/env python3
import os
import sys
from PIL import Image, ImageOps

from soromantic_utils.common import get_db_connection, get_download_dir

def main() -> None:
    print("Starting thumbnail backfill...")
    
    # Establish connection using context manager from common
    # We might need config, but get_db_connection loads it if None
    with get_db_connection() as conn:
        cursor = conn.cursor()
        
        # Get directories from config
        # Note: common.py has get_download_dir, but we might need thumbs/covers specifically if they are configurable.
        # But common.py doesn't expose get_thumbs_dir explicitly yet, only get_download_dir.
        # core/src/config.rs defaults thumbs/covers to download_dir/thumbs, download_dir/covers.
        # Let's assume standard layout relative to download_dir for now, 
        # as replicating the full config logic from Rust might be overkill if common.py handles basic paths.
        
        # Check if we can get the full config.
        from soromantic_utils.common import load_config, expand_path
        config = load_config()
        
        # Resolve paths similar to Rust
        download_dir = get_download_dir(config)
        if not download_dir:
            print("Error: Could not determine download directory.")
            sys.exit(1)
            
        runtime_cfg = config.get("runtime", {})
        
        thumbs_dir_raw = runtime_cfg.get("thumbs_dir")
        thumbs_dir = expand_path(thumbs_dir_raw) if thumbs_dir_raw else os.path.join(download_dir, "thumbs")
        
        covers_dir_raw = runtime_cfg.get("covers_dir")
        covers_dir = expand_path(covers_dir_raw) if covers_dir_raw else os.path.join(download_dir, "covers")
        
        if not os.path.exists(thumbs_dir):
            os.makedirs(thumbs_dir)
            
        print(f"Using thumbs dir: {thumbs_dir}")
        print(f"Using covers dir: {covers_dir}")

        print("Scanning for pages needing thumbnails...")
        cursor.execute("SELECT id, url, image FROM pages")
        pages = cursor.fetchall()
        
        count = 0
        generated = 0
        errors = 0
        
        for page in pages:
            page_id = page[0]
            page_url = page[1]
            # page[2] is image url
            
            thumb_filename = f"{page_id:06}.jpg"
            thumb_path = os.path.join(thumbs_dir, thumb_filename)
            
            if os.path.exists(thumb_path):
                continue
                
            # Check if cover exists
            cursor.execute("SELECT local_path FROM downloads WHERE src_url = ? AND status = 'done'", (page[2],))
            row = cursor.fetchone()
            
            cover_path = None
            if row:
                path = row[0]
                # Check for absolute path
                if os.path.isabs(path):
                    if os.path.exists(path):
                        cover_path = path
                else:
                    # Attempt to resolve relative path
                    # 1. relative to CWD? No.
                    # 2. relative to download_dir?
                    cand = os.path.join(download_dir, path)
                    if os.path.exists(cand):
                        cover_path = cand
            
            # Fallback to standard location
            if not cover_path:
                std_cover = os.path.join(covers_dir, f"{page_id:06}.jpg")
                if os.path.exists(std_cover):
                    cover_path = std_cover
            
            if cover_path and os.path.exists(cover_path):
                count += 1
                try:
                    print(f"Generating thumb for {page_id} from {cover_path}")
                    img = Image.open(cover_path)
                    # Resize to fill 400x225
                    thumb = ImageOps.fit(img, (400, 225), method=Image.Resampling.BILINEAR)
                    thumb.save(thumb_path, quality=90)
                    
                    # Update DB
                    thumb_url = f"generated:thumb:{page_id}"
                    
                    # 1. Insert/Update downloads
                    # Store ABSOLUTE path to match Rust behavior?
                    cursor.execute("""
                        INSERT OR REPLACE INTO downloads (src_url, local_path, status, last_modified)
                        VALUES (?, ?, 'done', datetime('now'))
                    """, (thumb_url, thumb_path))
                    
                    # 2. Insert self-ref grid_box
                    cursor.execute("""
                        INSERT INTO grid_boxes (page_id, thumb, title, related)
                        VALUES (?, ?, 'Thumbnail', ?)
                    """, (page_id, thumb_url, page_url))
                    
                    generated += 1
                    conn.commit()
                    
                except Exception as e:
                    print(f"Error processing {page_id}: {e}")
                    errors += 1
            else:
                pass

        print(f"Finished. Checked {len(pages)} pages.")
        print(f"Generated {generated} thumbs.")
        print(f"Errors: {errors}")

if __name__ == "__main__":
    main()

import sqlite3
import os
from pathlib import Path
from soromantic_utils.common import get_db_connection, load_config, get_download_dir

def repair_sources():
    config = load_config()
    db_path = config['runtime']['db_path']
    download_dir = Path(get_download_dir(config))
    videos_dir = download_dir / "videos"

    print(f"Scanning {videos_dir} for orphaned files...")

    # 1. Get Target Pages
    with get_db_connection(config) as conn:
        cursor = conn.cursor()
        
        # Same query as before: specific studios + missing duration
        query = """
        SELECT DISTINCT p.id, p.url, p.title
        FROM pages p 
        JOIN links l ON l.page_id = p.id AND l.rel_type = 'studio'
        JOIN studios s ON l.studio_id = s.id
        LEFT JOIN video_sources vs ON p.id = vs.page_id 
        WHERE s.url IN (
            'https://pissvids.com/studios/angelo-godshack-original',
            'https://www.xvideos.com/angelogodshack',
            'https://pissvids.com/studios/giorgio-grandi'
        ) 
        AND (vs.id IS NULL OR vs.duration IS NULL);
        """
        
        cursor.execute(query)
        rows = cursor.fetchall()
        print(f"Found {len(rows)} pages with missing duration/sources.")

        repaired_count = 0
        missing_count = 0

        for row in rows:
            page_id, page_url, page_title = row
            
            # 2. Look for file
            # Format: {videos_dir}/{res}/{0000id}.mp4
            # We check known resolutions
            found_path = None
            resolution = 0
            
            # Zero-pad ID to 6 digits (based on observation 008146.mp4)
            padded_id = f"{page_id:06d}"
            filename = f"{padded_id}.mp4"

            # Check subdirectories
            for res_dir in videos_dir.iterdir():
                if res_dir.is_dir():
                    candidate = res_dir / filename
                    if candidate.exists():
                        found_path = candidate
                        try:
                            resolution = int(res_dir.name)
                        except:
                            resolution = 480 # fallback
                        break
            
            # Also check root videos dir (legacy?)
            if not found_path:
                candidate = videos_dir / filename
                if candidate.exists():
                    found_path = candidate
                    resolution = 480

            if found_path:
                # 3. Repair
                # print(f"Found match: {found_path}")
                
                # Insert fake source if needed
                # We use a file:// URL so it works with probe_local_durations? 
                # Or relative path if that's what downloads uses.
                
                # Actually probe_local_durations joins on downloads.src_url = video_sources.url
                # So we make a fake URL key.
                fake_url = f"file://repair/{padded_id}"
                
                # Check if source exists
                cursor.execute("SELECT id FROM video_sources WHERE page_id = ?", (page_id,))
                if not cursor.fetchone():
                    cursor.execute("""
                        INSERT INTO video_sources (page_id, url, resolution, duration)
                        VALUES (?, ?, ?, NULL)
                    """, (page_id, fake_url, resolution))
                
                # Check if download exists
                cursor.execute("SELECT id FROM downloads WHERE src_url = ?", (fake_url,))
                if not cursor.fetchone():
                    # Rel path
                    try:
                        rel_path = found_path.relative_to(download_dir)
                    except:
                        rel_path = found_path # fallback

                    cursor.execute("""
                        INSERT INTO downloads (src_url, local_path, status)
                        VALUES (?, ?, 'done')
                    """, (fake_url, str(rel_path)))
                
                repaired_count += 1
            else:
                # print(f"Missing file for ID {page_id}")
                missing_count += 1

        conn.commit()
        print(f"Repaired {repaired_count} pages (linked to local files).")
        print(f"Still missing files for {missing_count} pages.")

if __name__ == "__main__":
    repair_sources()

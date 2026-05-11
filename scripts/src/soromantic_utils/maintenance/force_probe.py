import sqlite3
from pathlib import Path
from soromantic_utils.common import get_db_connection, load_config, get_download_dir
from soromantic_utils.media import get_video_duration


def force_probe():
    config = load_config()
    download_dir = Path(get_download_dir(config))
    videos_dir = download_dir / "videos"
    ffprobe_path = config.get("runtime", {}).get("ffprobe_binary", "ffprobe")

    print("Scanning DB for sources with missing duration...")

    with get_db_connection(config) as conn:
        conn.execute("PRAGMA busy_timeout = 30000") # 30s timeout
        cursor = conn.cursor()
        
        # Select sources with NULL duration
        query = """
        SELECT vs.id, vs.page_id, vs.url
        FROM video_sources vs
        WHERE vs.duration IS NULL
        """
        
        cursor.execute(query)
        rows = cursor.fetchall()
        print(f"Found {len(rows)} sources to probe.")

        updated_count = 0
        missing_file_count = 0
        error_count = 0

        for row in rows:
            vs_id, page_id, url = row
            
            # Find file: 00{page_id}.mp4 (padded to 6 digits)
            padded_id = f"{page_id:06d}"
            filename = f"{padded_id}.mp4"
            found_path = None
            
            candidates = list(videos_dir.rglob(filename))
            if candidates:
                found_path = candidates[0]
            else:
                if missing_file_count < 5:
                    print(f"DEBUG: Could not find {filename} in {videos_dir}")
                missing_file_count += 1
                continue

            try:
                duration = get_video_duration(found_path, ffprobe_path)
                cursor.execute("UPDATE video_sources SET duration = ? WHERE id = ?", (duration, vs_id))
                updated_count += 1
                if updated_count % 50 == 0:
                    print(f"Updated {updated_count}...")
            except Exception as e:
                if error_count < 5:
                    print(f"Error probing {found_path}: {e}")
                error_count += 1

        conn.commit()
        print(f"Finished. Updated {updated_count} durations. {missing_file_count} files not found. {error_count} probe errors.")

if __name__ == "__main__":
    force_probe()

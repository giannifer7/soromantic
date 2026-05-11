# scripts/src/soromantic_utils/maintenance/probe_local_durations.py
import argparse
from pathlib import Path
from soromantic_utils.common import get_db_connection, load_config, get_download_dir
from soromantic_utils.media import get_video_duration

def probe_and_update(config: dict | None = None):
    print("Scanning for downloaded videos to update duration...")
    
    if config is None:
        config = load_config()
    
    download_dir = get_download_dir(config)
    ffprobe_path = config.get("runtime", {}).get("ffprobe_binary", "ffprobe")
    print(f"Using download dir: {download_dir}")

    with get_db_connection(config) as conn:
        cursor = conn.cursor()
        
        # Select sources that have a local file (joined with downloads)
        query = """
            SELECT vs.id, vs.url, d.local_path
            FROM video_sources vs
            JOIN downloads d ON d.src_url = vs.url
            WHERE d.local_path IS NOT NULL AND d.status = 'done'
        """
        cursor.execute(query)
        rows = cursor.fetchall()
        
        updated_count = 0
        failed_count = 0
        
        print(f"Found {len(rows)} local files linked in DB.")
        
        for row in rows:
            vs_id, url, local_path_str = row
            local_path = Path(local_path_str)
            
            # Resolve relative path
            if not local_path.is_absolute():
                if download_dir:
                    local_path = download_dir / local_path
            
            if not local_path.exists():
                print(f"Warning: File not found: {local_path}")
                failed_count += 1
                continue
                
            # Probe
            try:
                duration = get_video_duration(local_path, ffprobe_path)
                cursor.execute("UPDATE video_sources SET duration = ? WHERE id = ?", (duration, vs_id))
                updated_count += 1
            except Exception as e:
                print(f"Failed to probe: {local_path} (ID: {vs_id}): {e}")
                failed_count += 1
                
        conn.commit()
        print(f"Updated {updated_count} durations. {failed_count} failed.")

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Probe local files and update duration in DB")
    parser.add_argument("--db-path", type=Path, help="Path to sqlite database")
    
    args = parser.parse_args()
    
    config = None
    if args.db_path:
        config = {"runtime": {"db_path": str(args.db_path)}}
    else:
        config = load_config()
    
    probe_and_update(config)

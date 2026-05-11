
import argparse
import sys
import math
from pathlib import Path
from soromantic_utils.common import get_db_connection, load_config

def format_iso_duration(seconds: float) -> str:
    """Convert seconds to ISO 8601 duration string (e.g. PT01H05M30S)."""
    if seconds is None:
        return "None"
        
    seconds_int = int(round(seconds))
    hours = seconds_int // 3600
    minutes = (seconds_int % 3600) // 60
    secs = seconds_int % 60
    
    parts = ["PT"]
    if hours > 0:
        parts.append(f"{hours}H")
    if minutes > 0 or hours > 0: # Show 0M if hours exist but mins are 0? Standard usually omits 0 values but let's be simple.
        parts.append(f"{minutes}M")
    parts.append(f"{secs}S")
    
    return "".join(parts)

def get_duration(video_id: int, config: dict | None = None):
    with get_db_connection(config) as conn:
        cursor = conn.cursor()
        cursor.execute("SELECT duration FROM video_sources WHERE id = ?", (video_id,))
        row = cursor.fetchone()
        
        if row:
            duration = row[0]
            if duration is not None:
                iso = format_iso_duration(duration)
                print(f"Duration (seconds): {duration}")
                print(f"Duration (ISO): {iso}")
            else:
                print("Duration: None")
            return

        # Fallback: Try looking up by page_id
        cursor.execute("SELECT id, resolution, duration FROM video_sources WHERE page_id = ?", (video_id,))
        rows = cursor.fetchall()
        
        if rows:
            print(f"ID {video_id} is a Page ID. Found {len(rows)} video sources:")
            for row in rows:
                vs_id, res, dur = row
                iso = format_iso_duration(dur) if dur is not None else "None"
                print(f"  - Source ID: {vs_id} ({res}p): {dur}s ({iso})")
        else:
            print(f"Video source with ID {video_id} not found, and no sources found for Page ID {video_id}.", file=sys.stderr)
            sys.exit(1)

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Get duration for a video source ID")
    parser.add_argument("video_id", type=int, help="ID of the video source")
    parser.add_argument("--db-path", type=Path, help="Path to sqlite database")
    
    args = parser.parse_args()
    
    config = None
    if args.db_path:
        config = {"runtime": {"db_path": str(args.db_path)}}
    else:
        config = load_config()
    
    get_duration(args.video_id, config)

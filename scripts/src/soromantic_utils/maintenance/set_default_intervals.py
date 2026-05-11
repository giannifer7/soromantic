# scripts/src/soromantic_utils/migrations/set_default_intervals.py
import argparse
from dataclasses import dataclass
from pathlib import Path
from soromantic_utils.common import get_db_connection, load_config


@dataclass
class StudioConfig:
    studio_url: str
    start: float  # Absolute start time (seconds)
    end: float    # Seconds from end (e.g. 10.0 means duration - 10.0)

# --- CONFIGURATION ---
# Populate this list before running the migration
STUDIO_CONFIGS: list[StudioConfig] = [
    StudioConfig(
        studio_url="https://pissvids.com/studios/angelo-godshack-original",
        start=15.0,
        end=6.0
    ),
    StudioConfig(
        studio_url="https://www.xvideos.com/angelogodshack",
        start=15.0,
        end=6.0
    ),
    StudioConfig(
        studio_url="https://pissvids.com/studios/giorgio-grandi",
        start=2.0,
        end=1.0
    ),
]
# ---------------------


def apply_defaults(config: dict | None = None, studio_filter: str | None = None):
    print("Applying default intervals...")


    
    with get_db_connection(config) as conn:
        cursor = conn.cursor()
        
        for studio in STUDIO_CONFIGS:
            print(f"Processing studio: {studio.studio_url}")

            if studio_filter and studio_filter not in studio.studio_url:
                print(f"  Skipping (does not match filter: {studio_filter})")
                continue
            
            # Find pages, video source, and download path
            # We prefer downloaded videos to get real duration
            query = """
                SELECT p.id, p.title, vs.id, vs.duration
                FROM pages p
                JOIN links l ON l.page_id = p.id AND l.rel_type = 'studio'
                JOIN studios s ON l.studio_id = s.id
                JOIN video_sources vs ON vs.page_id = p.id
                WHERE s.url = ?
                GROUP BY p.id
            """
            
            cursor.execute(query, (studio.studio_url,))
            rows = cursor.fetchall()
            
            updated_count = 0
            skipped_count = 0
            
            for row in rows:
                page_id, title, vs_id, db_duration = row
                
                if db_duration is not None and not isinstance(db_duration, float):
                    # Try to convert if it's a string (which it might be due to previous updates)
                    try:
                        db_duration = float(db_duration)
                    except (ValueError, TypeError):
                         # raise TypeError(f"Expected float for duration, got {type(db_duration)}: {db_duration}")
                         pass # handle below

                duration = db_duration

                if duration is None:
                    # Verbose logging only if strictly needed
                    skipped_count += 1
                    continue
                
                start_time = studio.start
                stop_time = duration - studio.end
                
                # Sanity checks
                if stop_time < start_time:
                    stop_time = start_time # 0 length
                
                if start_time > duration:
                    start_time = duration
                    stop_time = duration
                
                if stop_time < 0:
                     stop_time = 0;

                cursor.execute(
                    "UPDATE video_sources SET start_time = ?, stop_time = ? WHERE id = ?",
                    (start_time, stop_time, vs_id)
                )
                updated_count += 1
            
            print(f"  Updated {updated_count} videos (Skipped {skipped_count}) for studio")
            
        conn.commit()

    print("Defaults applied.")

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Apply default start/stop times for studios")
    parser.add_argument("--db-path", type=Path, help="Path to sqlite database")
    parser.add_argument("--studio", type=str, help="Filter by studio URL part (e.g. 'angelo')")
    
    args = parser.parse_args()
    
    config = None
    if args.db_path:
        config = {"runtime": {"db_path": str(args.db_path)}}
    else:
        config = load_config()
    
    apply_defaults(config, studio_filter=args.studio)

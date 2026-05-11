
import os
import sys
import argparse
from pathlib import Path
from soromantic_utils.common import get_db_connection, load_config, get_download_dir
from soromantic_utils.media import get_video_resolution

def get_target_resolution(config: dict) -> int:
    """Get the preferred video resolution from config."""
    preferences = config.get("playback", {}).get("video_preferences", [])
    if not preferences:
        print("No video_preferences found in config. Aborting.", file=sys.stderr)
        sys.exit(1)
    return preferences[0]

def get_xvideos_rows(cursor, download_dir: Path) -> list:
    """Query database for Xvideos downloads."""
    query = """
        SELECT DISTINCT d.local_path, p.url
        FROM downloads d
        JOIN video_sources vs ON d.src_url = vs.url
        JOIN pages p ON vs.page_id = p.id
        WHERE p.url LIKE '%xvideos%' AND d.status = 'done'
    """
    cursor.execute(query)
    return cursor.fetchall()



def process_video(local_path: Path, page_url: str, target_res: int, dry_run: bool) -> tuple[bool, bool]:
    """
    Check video resolution and delete if mismatches.
    Returns (was_cleaned, has_error).
    """
    if not local_path.exists():
        return False, False

    resolution = get_video_resolution(local_path)
    if resolution is None:
        print(f"Could not determine resolution: {local_path}")
        return False, True
    
    width, height = resolution
    
    if height != target_res:
        print(f"Mismatch: {local_path} is {height}p (Target: {target_res}p)")
        
        if dry_run:
            print(f"  [DRY RUN] Would delete file and add {page_url} to rescrape list")
            return True, False
            
        try:
            os.remove(local_path)
            print(f"  Deleted: {local_path}")

            return True, False
        except OSError as e:
            print(f"  Error deleting: {e}")
            return False, True
            
    return False, False

def save_rescrape_list(pages: set, output_file: Path):
    """Save unique page URLs to file."""
    if not pages:
        print("\nNo resolution mismatches found.")
        return
        
    with open(output_file, "w") as f:
        for url in sorted(pages):
            f.write(f"{url}\n")
            
    print(f"\nSummary:")
    print(f"  {len(pages)} URLs written to {output_file.absolute()}")
    print("  Use these URLs to re-scrape and download correct versions.")

def clean_resolutions(dry_run: bool = False, db_path: Path | None = None):
    config = load_config()
    if db_path:
        config["runtime"]["db_path"] = str(db_path)
    
    target_res = get_target_resolution(config)
    print(f"Target Resolution: {target_res}p")
    
    download_dir = Path(get_download_dir(config))
    pages_to_rescrape = set()
    cleaned_count = 0
    error_count = 0
    
    with get_db_connection(config) as conn:
        cursor = conn.cursor()
        rows = get_xvideos_rows(cursor, download_dir)
        print(f"Scanning {len(rows)} videos...")
        
        for rel_path, page_url in rows:
            local_path = download_dir / rel_path
            cleaned, error = process_video(local_path, page_url, target_res, dry_run)
            
            if cleaned:
                pages_to_rescrape.add(page_url)
                cleaned_count += 1
            if error:
                error_count += 1

    print(f"  Total processed: {len(rows)}")
    print(f"  Cleaned/Identified: {cleaned_count}")
    print(f"  Errors: {error_count}")
    
    save_rescrape_list(pages_to_rescrape, Path("batch_rescrape.txt"))

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Clean videos with incorrect resolutions")
    parser.add_argument("--dry-run", action="store_true", help="Scan only, do not delete files")
    parser.add_argument("--db-path", type=Path, help="Path to sqlite database")
    
    args = parser.parse_args()
    clean_resolutions(args.dry_run, args.db_path)

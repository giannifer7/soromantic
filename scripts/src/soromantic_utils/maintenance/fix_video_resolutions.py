#!/usr/bin/env python3
"""
Repair video_sources table by extracting correct resolutions from URLs.

This fixes the bug where URLs like "video_360p.mp4" were incorrectly labeled as 720p.
"""
import re
import sqlite3
import sys
from pathlib import Path


def parse_resolution_from_url(url: str) -> int | None:
    """Extract resolution from URL filename (e.g., 'video_360p.mp4' -> 360)."""
    match = re.search(r'(\d+)p', url)
    return int(match.group(1)) if match else None


def repair_video_sources(db_path: str, dry_run: bool = True):
    """Fix mislabeled video source resolutions."""
    conn = sqlite3.connect(db_path)
    cursor = conn.cursor()
    
    # Find all mismatched entries
    cursor.execute("""
        SELECT page_id, url, resolution 
        FROM video_sources
        WHERE url LIKE '%p%'
    """)
    
    updates = []
    for page_id, url, current_res in cursor.fetchall():
        parsed_res = parse_resolution_from_url(url)
        if parsed_res and parsed_res != current_res:
            updates.append((parsed_res, page_id, url))
            if dry_run:
                print(f"Would update: {url[:80]}... | {current_res}p -> {parsed_res}p")

    
    if not updates:
        print("✓ No mismatched resolutions found!")
        return
    
    print(f"\nFound {len(updates)} entries with incorrect resolutions")
    
    if dry_run:
        print("\nRun with --apply to actually fix the database")
        return
    
    # Apply updates
    cursor.executemany(
        "UPDATE video_sources SET resolution = ? WHERE page_id = ? AND url = ?",
        updates
    )
    conn.commit()
    print(f"✓ Fixed {len(updates)} video source entries!")
    
    conn.close()


if __name__ == "__main__":
    db_path = "/mnt/sda3/porn/pyssvids/db/data.db"
    
    if not Path(db_path).exists():
        print(f"Error: Database not found at {db_path}")
        sys.exit(1)
    
    dry_run = "--apply" not in sys.argv
    
    if dry_run:
        print("=== DRY RUN MODE (no changes will be made) ===\n")
    else:
        print("=== APPLYING FIXES ===\n")
    
    repair_video_sources(db_path, dry_run=dry_run)

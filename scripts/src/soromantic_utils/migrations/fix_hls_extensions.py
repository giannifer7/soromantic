#!/usr/bin/env python3
"""
Fix HLS file extensions from .m3u8 to .mp4 in persistent storage and DB.
"""

import os
import sqlite3
from pathlib import Path

import soromantic_utils.common as common


def main():
    db_path = common.get_db_path()
    if not os.path.exists(db_path):
        print(f"Error: Database not found at {db_path}")
        return

    print(f"Using database: {db_path}")
    conn = sqlite3.connect(db_path)
    cursor = conn.cursor()

    try:
        # Find all downloads with .m3u8 local path
        cursor.execute("SELECT id, src_url, local_path FROM downloads WHERE local_path LIKE '%.m3u8'")
        rows = cursor.fetchall()

        print(f"Found {len(rows)} files to check/rename.")

        fixed_count = 0
        skipped_count = 0

        for row in rows:
            record_id, src_url, local_path_str = row

            if not local_path_str:
                skipped_count += 1
                continue

            old_path = Path(local_path_str)
            new_path = old_path.with_suffix(".mp4")

            # Check file existence
            if not old_path.exists():
                print(f"Checking {old_path}...")
                if new_path.exists():
                    print("  -> File already renamed manually? Updating DB only.")
                    cursor.execute(
                        "UPDATE downloads SET local_path = ? WHERE id = ?", (str(new_path), record_id)
                    )
                    fixed_count += 1
                else:
                    print("  -> File missing. Skipping.")
                    skipped_count += 1
                continue

            # File exists, verify it is truly a video (magic check?)
            # Or just blindly rename since we know HLS downloads were saved as M3U8 but contain MP4 data

            print(f"Renaming: {old_path.name} -> {new_path.name}")
            try:
                os.rename(old_path, new_path)

                # Update DB
                cursor.execute("UPDATE downloads SET local_path = ? WHERE id = ?", (str(new_path), record_id))
                fixed_count += 1

            except OSError as e:
                print(f"  -> Rename failed: {e}")
                skipped_count += 1

        conn.commit()
        print(f"\nDone. Fixed {fixed_count} records. Skipped {skipped_count}.")

    finally:
        conn.close()


if __name__ == "__main__":
    main()

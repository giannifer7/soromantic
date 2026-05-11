#!/usr/bin/env python3
"""Fix swapped studio URL/name fields for XV-scraped entries.

The scraper_xv.py had a bug where uploader was stored as (name, url)
instead of (url, name). This script identifies affected studios
(where URL looks like a name and name looks like a URL) and swaps them.
"""

import sqlite3
import sys
from pathlib import Path

from soromantic_utils.common import get_db_path, load_config


def is_url(s: str) -> bool:
    """Check if string looks like a URL."""
    return s.startswith("http://") or s.startswith("https://")


def fix_swapped_studios(db_path: Path) -> None:
    """Find and fix studios where URL and name are swapped."""
    conn = sqlite3.connect(db_path)
    cursor = conn.cursor()

    # Find studios where:
    # - 'url' column doesn't start with http (it's actually a name)
    # - 'name' column starts with http (it's actually a URL)
    cursor.execute("""
        SELECT id, url, name FROM studios
        WHERE url NOT LIKE 'http%' AND name LIKE 'http%'
    """)

    swapped = cursor.fetchall()
    if not swapped:
        print("No swapped studios found.")
        conn.close()
        return

    print(f"Found {len(swapped)} studios with swapped URL/name:")
    for studio_id, wrong_url, wrong_name in swapped:
        correct_url = wrong_name  # The 'name' field has the URL
        correct_name = wrong_url  # The 'url' field has the name
        print(f"  [{studio_id}] '{correct_name}' -> {correct_url}")

        # Check if a studio with the correct URL already exists
        cursor.execute("SELECT id FROM studios WHERE url = ?", (correct_url,))
        existing = cursor.fetchone()

        if existing:
            existing_id = existing[0]
            print(f"    Merging into existing studio {existing_id}")
            # Update all links to point to the existing studio
            cursor.execute("UPDATE links SET studio_id = ? WHERE studio_id = ?", (existing_id, studio_id))
            # Delete the duplicate
            cursor.execute("DELETE FROM studios WHERE id = ?", (studio_id,))
        else:
            # No conflict, just swap the fields
            cursor.execute(
                "UPDATE studios SET url = ?, name = ? WHERE id = ?", (correct_url, correct_name, studio_id)
            )

    conn.commit()
    conn.close()
    print(f"\nFixed {len(swapped)} studios.")


def main() -> None:
    """Entry point."""
    if len(sys.argv) > 1:
        db_path = Path(sys.argv[1])
    else:
        config = load_config()
        db_path = Path(get_db_path(config))

    if not db_path.exists():
        print(f"Error: Database not found at {db_path}")
        sys.exit(1)

    print(f"Fixing swapped studios in {db_path}...")
    fix_swapped_studios(db_path)
    print("Done!")


if __name__ == "__main__":
    main()

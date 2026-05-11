#!/usr/bin/env python3
"""Fix HTML entities in database titles.

This script finds and fixes HTML entities (like &excl; -> !) in:
- pages.title
- grid_boxes.title
"""

import argparse
import html
import sys
from pathlib import Path

from soromantic_utils.common import get_db_connection


def fix_html_entities(config: dict | None = None) -> None:
    """Decode HTML entities in all title columns."""
    
    with get_db_connection(config) as conn:
        cursor = conn.cursor()

        # Fix pages.title
        cursor.execute("SELECT id, title FROM pages WHERE title IS NOT NULL")
        pages_fixed = 0
        for row_id, title in cursor.fetchall():
            decoded = html.unescape(title)
            if decoded != title:
                cursor.execute("UPDATE pages SET title = ? WHERE id = ?", (decoded, row_id))
                pages_fixed += 1
                print(f"  pages[{row_id}]: {title[:50]} -> {decoded[:50]}")

        # Fix grid_boxes.title
        cursor.execute("SELECT id, title FROM grid_boxes WHERE title IS NOT NULL")
        grid_fixed = 0
        for row_id, title in cursor.fetchall():
            decoded = html.unescape(title)
            if decoded != title:
                cursor.execute("UPDATE grid_boxes SET title = ? WHERE id = ?", (decoded, row_id))
                grid_fixed += 1
                print(f"  grid_boxes[{row_id}]: {title[:50]} -> {decoded[:50]}")

        conn.commit()

    print(f"\nFixed {pages_fixed} pages and {grid_fixed} grid_boxes")


def main() -> None:
    """Entry point."""
    parser = argparse.ArgumentParser(description="Fix HTML entities in database titles")
    parser.add_argument("db_path", nargs="?", type=Path, help="Path to sqlite database")
    
    args = parser.parse_args()
    
    config = None
    if args.db_path:
        config = {"runtime": {"db_path": str(args.db_path)}}

    print("Fixing HTML entities...")
    fix_html_entities(config)
    print("Done!")


if __name__ == "__main__":
    main()

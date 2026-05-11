#!/usr/bin/env python3
"""
Migration script to add new database indexes for query optimization.

This adds indexes that improve JOIN performance for the optimized queries.
Safe to run multiple times (uses IF NOT EXISTS).
"""

import sqlite3
import sys

from soromantic_utils.common import get_db_path, load_config

NEW_INDEXES = [
    "CREATE INDEX IF NOT EXISTS idx_video_sources_url ON video_sources(url)",
    "CREATE INDEX IF NOT EXISTS idx_pages_title ON pages(title)",
]


def migrate(db_path: str) -> None:
    print(f"Connecting to database: {db_path}")
    conn = sqlite3.connect(db_path)
    cursor = conn.cursor()

    for sql in NEW_INDEXES:
        print(f"  Running: {sql}")
        cursor.execute(sql)

    conn.commit()
    print("Migration complete.")
    conn.close()


def main() -> int:
    config = load_config()
    db_path = get_db_path(config)

    if not db_path:
        print("Error: Could not determine database path from config.")
        return 1

    migrate(db_path)
    return 0


if __name__ == "__main__":
    sys.exit(main())

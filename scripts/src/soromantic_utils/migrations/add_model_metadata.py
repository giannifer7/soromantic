"""Add model metadata columns and flags table."""

import logging
import sys
from dataclasses import dataclass
from typing import Any

from soromantic_utils.common import get_db_connection, load_config


@dataclass
class Config:
    """Migration configuration."""

    verbose: bool = False


def add_columns_if_missing(
    log: logging.Logger,
    cursor: Any,
    table: str,
    columns: list[tuple[str, str]],
) -> int:
    """Add columns to table if they don't exist.

    Args:
        log: Logger instance
        cursor: Database cursor
        table: Table name
        columns: List of (column_name, column_type) tuples

    Returns:
        Number of columns added
    """
    cursor.execute(f"PRAGMA table_info({table})")
    existing = {row[1] for row in cursor.fetchall()}

    added = 0
    for col_name, col_type in columns:
        if col_name not in existing:
            log.info("Adding column %s.%s (%s)", table, col_name, col_type)
            cursor.execute(f"ALTER TABLE {table} ADD COLUMN {col_name} {col_type}")
            added += 1
        else:
            log.debug("Column %s.%s already exists", table, col_name)

    return added


def create_flags_table(log: logging.Logger, cursor: Any) -> bool:
    """Create flags table if it doesn't exist.

    Args:
        log: Logger instance
        cursor: Database cursor

    Returns:
        True if table was created, False if it already existed
    """
    cursor.execute(
        "SELECT name FROM sqlite_master WHERE type='table' AND name='flags'"
    )
    if cursor.fetchone():
        log.debug("Table flags already exists")
        return False

    log.info("Creating flags table")
    cursor.execute("""
        CREATE TABLE flags (
            id INTEGER PRIMARY KEY,
            code TEXT UNIQUE NOT NULL,
            name TEXT
        )
    """)
    cursor.execute("CREATE INDEX IF NOT EXISTS idx_flags_code ON flags(code)")
    return True


def main() -> None:
    """Run the migration."""
    logging.basicConfig(
        level=logging.INFO,
        stream=sys.stderr,
        format="%(levelname)s: %(message)s",
    )
    log = logging.getLogger(__name__)

    config = load_config()

    with get_db_connection(config) as conn:
        cursor = conn.cursor()

        # Create flags table
        flags_created = create_flags_table(log, cursor)

        # Add model metadata columns to taxonomies
        # These are only populated for type=1 (model) entries
        # - flag_id references flags table (padded ID used as filename)
        # - hero_image stores the remote URL (padded taxonomy ID used as filename)
        columns_to_add: list[tuple[str, str]] = [
            ("hero_image", "TEXT"),
            ("flag_id", "INTEGER"),
            ("nationality", "TEXT"),
            ("birth_year", "INTEGER"),
            ("aliases", "TEXT"),
            ("thumb_status", "INTEGER DEFAULT 0"),
        ]

        added = add_columns_if_missing(log, cursor, "taxonomies", columns_to_add)

        conn.commit()

        changes = added + (1 if flags_created else 0)
        if changes > 0:
            log.info("Migration complete: %d change(s)", changes)
        else:
            log.info("Migration complete: no changes needed")


if __name__ == "__main__":
    main()

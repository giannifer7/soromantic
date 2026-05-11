"""
Offline Page Processor

Processes downloaded HTML pages to extract metadata and related videos,
updating the database without making network requests.
"""

import logging
import sqlite3
import sys
from dataclasses import dataclass, field
from pathlib import Path

from soromantic_utils.common import get_db_connection, get_download_dir, load_config
from soromantic_utils.maintenance.rescrape_related import (
    extract_page_title,
    extract_related_urls,
    get_pagination_base_url,
    process_page_update,
)


@dataclass
class Config:
    """Configuration for offline processing."""

    base_url_map: dict[int, str] = field(default_factory=dict)
    batch_size: int = 100


def setup_logging(name: str = __name__) -> logging.Logger:
    """Setup and return a configured logger."""
    logging.basicConfig(
        level=logging.INFO,
        stream=sys.stderr,
        format="%(asctime)s - %(levelname)s - %(message)s",
    )
    return logging.getLogger(name)


def parse_page_file(file_path: Path) -> tuple[int, str | None, list[tuple[str, str, str]]]:
    """
    Parse a single HTML file interactively.
    Returns (page_id, title, related_videos).
    Note: related_videos urls are normalized if base_url is known or inferred.
    Here we assume absolute URLs are in the HTML or we need the original URL to normalize.
    """
    try:
        # Filename format: {id:06}-{page}.html
        # e.g. 008508-2.html
        stem = file_path.stem
        parts = stem.split("-")
        if len(parts) != 2:
            return 0, None, []

        page_id = int(parts[0])
        html = file_path.read_text(encoding="utf-8")

        # We construct a dummy base URL or try to find it?
        # extract_related_urls needs a base_url for normalization.
        # Since we don't have the original URL handy in the filename,
        # we might produce relative URLs if the HTML has them.
        # However, the site usually has absolute URLs or consistent relative ones.
        # Let's pass a placeholder base_url and see if we can get away with it,
        # OR better: load the mapping of ID -> URL from DB once at start.

        # Actually, extracting this inside the loop is slow if we do DB query per file.
        # We will pass the base_url map in config.

        return page_id, html, []
    except Exception as e:
        print(f"Error parsing {file_path}: {e}", file=sys.stderr)
        return 0, None, []


def process_files(
    files: list[Path], url_map: dict[int, str]
) -> dict[int, tuple[str | None, list[tuple[str, str, str]]]]:
    """
    Process a list of files and aggregate results by page_id.
    Returns dict: page_id -> (best_title, all_related_items)
    """
    results: dict[int, tuple[str | None, list[tuple[str, str, str]]]] = {}

    for file_path in files:
        try:
            stem = file_path.stem
            parts = stem.split("-")
            pid = int(parts[0])
        except (ValueError, IndexError):
            continue

        base_url = url_map.get(pid)
        if not base_url:
            # log.warning("No URL found for page %s, skipping.", pid)
            continue

        # Ensure base URL is pagination base (strip trailing slash/suffix if needed)
        # reusing logic from rescrape_related
        clean_base = get_pagination_base_url(base_url)

        html = file_path.read_text(encoding="utf-8", errors="replace")

        title = extract_page_title(html)
        related = extract_related_urls(html, clean_base)

        if pid not in results:
            results[pid] = (title, related)
        else:
            current_title, current_related = results[pid]
            # Prefer non-None title, or longer title?
            new_title = title if title else current_title
            current_related.extend(related)
            results[pid] = (new_title, current_related)

    return results


def main() -> None:
    """Main entry point."""
    log = setup_logging()
    app_config = load_config()

    download_root = get_download_dir(app_config)
    if not download_root:
        log.error("Download dir not configured.")
        sys.exit(1)

    pages_dir = download_root / "pages"
    if not pages_dir.exists():
        log.error("Pages directory not found: %s", pages_dir)
        sys.exit(1)

    all_files = list(pages_dir.glob("*.html"))
    if not all_files:
        log.info("No HTML files found to process.")
        return

    log.info("Found %s files to process.", len(all_files))

    # Pre-fetch URLs for all page IDs found in files
    pids = set()
    for f in all_files:
        try:
            pids.add(int(f.stem.split("-")[0]))
        except ValueError:
            pass

    if not pids:
        log.info("No valid page IDs found in filenames.")
        return

    log.info("Resolving URLs for %s unique page IDs...", len(pids))

    url_map: dict[int, str] = {}
    with get_db_connection() as conn:
        cur = conn.cursor()
        # Fetch URLs in bulk. SQLite limits variable number, so chunk it.
        # well, 999 variables usually.
        pids_list = list(pids)
        chunk_size = 900
        for i in range(0, len(pids_list), chunk_size):
            chunk = pids_list[i : i + chunk_size]
            placeholders = ",".join("?" * len(chunk))
            cur.execute(f"SELECT id, url FROM pages WHERE id IN ({placeholders})", chunk)
            for row in cur.fetchall():
                url_map[row[0]] = row[1]

    log.info("Loaded %s URLs.", len(url_map))

    # Process files
    # We can do this in memory since file count is likely < 100k and they are small HTMLs.
    # But batching db updates is smart.

    # Process files
    log.info("Parsing files...")
    # This could be parallelized via ThreadPoolExecutor for CPU bound BS4 parsing
    # but simple loop is fine for <1000 files.
    # Let's keep it simple first.

    processed_data = process_files(all_files, url_map)

    log.info("Extracted data for %s pages. Updating database...", len(processed_data))

    updated_count = update_database(processed_data, log)

    log.info("Offline processing complete. Updated %s pages.", updated_count)


def update_database(
    processed_data: dict[int, tuple[str | None, list[tuple[str, str, str]]]],
    log: logging.Logger,
) -> int:
    """Update database with processed data."""
    updated_count = 0
    target_ids = list(processed_data.keys())

    with get_db_connection() as conn:
        cur = conn.cursor()
        start = 0
        batch_size = 100

        while start < len(target_ids):
            batch_ids = target_ids[start : start + batch_size]

            placeholders = ",".join("?" * len(batch_ids))
            # Including title column for consistency with new rescrape logic
            cur.execute(
                f"SELECT id, url, related_ids, title FROM pages WHERE id IN ({placeholders})",
                batch_ids,
            )
            db_rows = cur.fetchall()

            # db_rows maps to the 'chunk' expected by process_page_update
            # logic in rescrape_related expects: list[tuple[int, str, str | None]] (size 3)
            # but we updated process_page_update to read row[3] (title) if present.
            # So passing the 4-element tuple is actually correct/required now.

            for row in db_rows:
                pid = row[0]
                if pid in processed_data:
                    data = processed_data[pid]
                    if process_page_update(cur, db_rows, pid, data, log):
                        updated_count += 1

            conn.commit()
            start += batch_size
            log.info("Processed DB batch %s/%s...", start, len(target_ids))

    return updated_count


if __name__ == "__main__":
    main()

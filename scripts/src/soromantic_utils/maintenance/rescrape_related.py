"""
Rescrape Related Videos Script

This script rescrapes pages that are missing relationship data (related_ids).
It performs a deep scan of pages 2-4 to find related videos and updates the database.
"""

import asyncio
import logging
import random
import sqlite3
import sys
from dataclasses import dataclass
from urllib.parse import urljoin

import httpx
from bs4 import BeautifulSoup

from soromantic_utils.common import get_db_connection, get_download_dir
from pathlib import Path


@dataclass
class Config:
    """Configuration for the rescraper."""

    user_agent: str = (
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) "
        "AppleWebKit/537.36 (KHTML, like Gecko) "
        "Chrome/120.0.0.0 Safari/537.36"
    )
    batch_size: int = 5
    execution_limit: int = 2  # Only process this many pages in this run
    timeout: float = 30.0


def setup_logging(name: str = __name__) -> logging.Logger:
    """Setup and return a configured logger."""
    logging.basicConfig(
        level=logging.INFO,
        stream=sys.stderr,
        format="%(asctime)s - %(levelname)s - %(message)s",
    )
    return logging.getLogger(name)


def norm_url(base: str, href: str | None) -> str:
    """Normalize a URL relative to a base."""
    if not href:
        return ""
    return urljoin(base, href)


def get_pagination_base_url(url: str) -> str:
    """
    Derive the base URL for pagination.
    We assume simplistic pagination where numbers are appended to the base URL.
    """
    clean_url = url.rstrip("/")
    return clean_url


async def fetch_page(
    client: httpx.AsyncClient, url: str, config: Config, log: logging.Logger
) -> str | None:
    """Fetch a URL with the given client, handling 404s and timeouts."""
    try:
        resp = await client.get(url, timeout=config.timeout, follow_redirects=True)
    except httpx.ConnectError as e:
        log.warning("Connection error for %s: %s", url, e)
        return None
    except httpx.TimeoutException:
        log.warning("Timeout for %s", url)
        return None
    except httpx.TooManyRedirects:
        log.warning("Too many redirects for %s", url)
        return None

    match resp.status_code:
        case 404:
            return None
        case 200:
            return resp.text
        case _:
            resp.raise_for_status()
            return resp.text


def extract_related_urls(html: str, base_url: str) -> list[tuple[str, str, str]]:
    """Extract related video URLs, titles, and images from the HTML content."""
    soup = BeautifulSoup(html, "html.parser")
    related: list[tuple[str, str, str]] = []

    for scene in soup.select("div.card-scene"):
        if not (text_a := scene.select_one(".card-scene__text a")):
            continue

        if not (href := text_a.get("href")):
            continue

        if isinstance(href, list):
            # This case should be rare for 'href' but safe to handle for mypy
            href = href[0]

        full_url = norm_url(base_url, href)
        title = text_a.get_text(strip=True)

        # Extract image
        image_url = ""
        # Selector matches pv.rs: .card-scene__view a img
        if img := scene.select_one(".card-scene__view a img"):
            raw_src = img.get("data-src") or img.get("src")
            if isinstance(raw_src, list):
                raw_src = raw_src[0]
            if raw_src and not raw_src.startswith("data:"):
                # Take first URL if it's a comma-separated list
                image_url = norm_url(base_url, raw_src.split(",")[0].strip())

        related.append((full_url, title, image_url))

    return related


def extract_page_title(html: str) -> str | None:
    """Extract and clean the page title from HTML."""
    soup = BeautifulSoup(html, "html.parser")
    if t_tag := soup.find("title"):
        raw = t_tag.get_text(strip=True)
        if " - Pissvids.com" in raw:
            return raw.replace(" - Pissvids.com", "")
        return raw
    return None


async def process_single_page(
    client: httpx.AsyncClient, url: str, config: Config, log: logging.Logger
) -> tuple[str | None, list[tuple[str, str, str]]]:
    """
    Fetches pagination (pages 2-4) for a single video page
    and returns (page_title, found_related_videos).
    """
    base_url = get_pagination_base_url(url)
    tasks = []

    for p in range(2, 5):
        # Pagination pattern: base_url + "/" + p
        p_url = f"{base_url}/{p}"
        tasks.append(fetch_page(client, p_url, config, log))

    responses = await asyncio.gather(*tasks)

    all_related: list[tuple[str, str, str]] = []
    page_title: str | None = None

    for html in responses:
        if html:
            if not page_title:
                page_title = extract_page_title(html)

            found = extract_related_urls(html, base_url)
            all_related.extend(found)

    return page_title, all_related


async def process_batch(
    client: httpx.AsyncClient,
    batch: list[tuple[int, str, str | None]],
    config: Config,
    log: logging.Logger,
) -> dict[int, tuple[str | None, list[tuple[str, str, str]]]]:
    """
    Process a batch of pages, returning a map of page_id
    to (title, list of new related (url, title, image)).
    """
    tasks = []
    meta = []

    for pid, url, _ in batch:
        if not url.startswith("http"):
            log.warning("Skipping invalid URL for page %s: %s", pid, url)
            continue

        tasks.append(process_single_page(client, url, config, log))
        meta.append(pid)

    results = await asyncio.gather(*tasks)

    mapped_results: dict[int, tuple[str | None, list[tuple[str, str, str]]]] = {}
    for i, (p_title, related_items) in enumerate(results):
        if related_items or p_title:
            pid = meta[i]
            mapped_results[pid] = (p_title, related_items)

    return mapped_results


def resolve_or_create_page(cur: sqlite3.Cursor, url: str, title: str, image: str) -> int:
    """Finds a page ID by URL or creates a new pending page."""
    # Note: 'image' arg is currently unused in DB as table lacks column,
    # so we log it to a csv for later processing.
    cur.execute("SELECT id, title FROM pages WHERE url = ?", (url,))

    rid = 0
    match cur.fetchone():
        case None:
            cur.execute(
                (
                    "INSERT INTO pages "
                    "(url, title, thumb_status, preview_status, video_status) "
                    "VALUES (?, ?, 1, 1, 1)"
                ),
                (url, title),
            )
            if (row_id := cur.lastrowid) is None:
                raise ValueError("Insert failed, no row ID returned")
            rid = row_id
        case (row_id, current_title):
            rid = row_id
            if not current_title and title:
                cur.execute("UPDATE pages SET title = ? WHERE id = ?", (title, rid))
        case _:
            raise ValueError("Unexpected database result shape")

    csv_path = "/home/g4/_prj/soromantic/missing_thumbs.csv"
    if rid > 0:
        # Check if thumbnail already exists
        thumb_path = get_download_dir() / "thumbs" / f"{rid:06}.jpg"
        if thumb_path.exists():
            return int(rid)

        # print(f"RESCRAPE_DEBUG: rid={rid}, image='{image}'", file=sys.stderr)

    if image and rid > 0:
        try:
            with open(csv_path, "a", encoding="utf-8") as f:
                f.write(f"{rid}|{image}\n")
        except OSError:
            pass

    return int(rid)


def update_page_relations(
    cur: sqlite3.Cursor,
    page_id: int,
    new_items: list[tuple[str, str, str]],
    _ignored_existing_str: str | None,
) -> bool:
    """Updates the page_relations table, adding new items."""
    # 1. Get existing relations from table
    cur.execute("SELECT target_id FROM page_relations WHERE source_id = ?", (page_id,))
    current_ids = {row[0] for row in cur.fetchall()}

    new_relations = []

    for r_url, r_title, r_image in new_items:
        r_url = r_url.rstrip("/")
        if "xvideos.com" in r_url:
            continue

        rid = resolve_or_create_page(cur, r_url, r_title, r_image)

        if rid not in current_ids and rid != page_id:
            new_relations.append((page_id, rid))
            current_ids.add(rid)

    if new_relations:
        cur.executemany(
            "INSERT OR IGNORE INTO page_relations (source_id, target_id) VALUES (?, ?)",
            new_relations,
        )
        return True

    return False


def process_page_update(
    cur: sqlite3.Cursor,
    chunk: list[tuple[int, str, str | None]],
    page_id: int,
    data: tuple[str | None, list[tuple[str, str, str]]],
    log: logging.Logger,
) -> bool:
    """
    Helper to find original row and update page relations and title.
    Returns True if updated, False otherwise.
    """
    new_title, new_items = data

    original_row = next((r for r in chunk if r[0] == page_id), None)
    if not original_row:
        return False

    updated = False

    # Update title if missing in DB and present in scan
    # original_row is (id, url, related_ids), so we don't have title there?
    # Wait, the SELECT only gets related_ids. We need to select title to check.
    # actually, we can just blind update if new_title is present,
    # or we can trust the DB query if we modify it to include title.

    # Let's modify the query in main() to select title as well.
    # original_row[0]=id, [1]=url, [2]=related_ids, [3]=title (if we add it)

    # Assuming we update SELECT to: id, url, related_ids, title
    current_title = original_row[3] if len(original_row) > 3 else ""

    if not current_title and new_title:
        cur.execute("UPDATE pages SET title = ? WHERE id = ?", (new_title, page_id))
        log.info("Updated title for page %s to '%s'", page_id, new_title)
        updated = True

    existing_ids_str = original_row[2]

    if update_page_relations(cur, page_id, new_items, existing_ids_str):
        log.info("Updated page %s with %s new relations", page_id, len(new_items))
        updated = True

    return updated


async def run_rescraping(
    rows: list[tuple[int, str, str | None]], config: Config, log: logging.Logger
) -> None:
    """Run the main rescraping loop."""
    chunks = [rows[i : i + config.batch_size] for i in range(0, len(rows), config.batch_size)]

    with get_db_connection() as conn:
        cur = conn.cursor()
        total_updated = 0
        pages_processed = 0

        async with httpx.AsyncClient(headers={"User-Agent": config.user_agent}) as client:
            for i, chunk in enumerate(chunks):
                if pages_processed >= config.execution_limit:
                    log.info(
                        "Reached execution limit of %s pages. Stopping.", config.execution_limit
                    )
                    break

                log.info("Processing batch %s/%s...", i + 1, len(chunks))

                batch_results = await process_batch(client, chunk, config, log)

                batch_updated_count = 0
                for page_id, data in batch_results.items():
                    if process_page_update(cur, chunk, page_id, data, log):
                        batch_updated_count += 1

                conn.commit()
                total_updated += batch_updated_count
                pages_processed += len(chunk)

                # Rate limiting
                await asyncio.sleep(1.5 + random.random() * 2.0)

        log.info("Updated %s pages with deep relations.", total_updated)


def main() -> None:
    """Main entry point."""
    log = setup_logging()
    config = Config()

    with get_db_connection() as conn:
        cur = conn.cursor()
        log.info("Fetching pages to check pagination for (only those missing relations)...")
        # We limit the fetch to a bit more than execution limit just to be sure
        # we have candidates. But actually let's fetch all candidates, we limit
        # processing in run_rescraping.
        cur.execute(
            "SELECT id, url, related_ids, title FROM pages "
            "WHERE url NOT LIKE '%xvideos.com%' "
            "AND url LIKE 'http%' "
            "AND (related_ids IS NULL OR related_ids = '') "
            "LIMIT ?",
            (config.execution_limit * 2,),
        )

        rows: list[tuple[int, str, str | None]] = cur.fetchall()
        log.info("Found %s pages to process (filtering from limited set).", len(rows))

    if not rows:
        log.info("No pages found needing update.")
        return

    try:
        asyncio.run(run_rescraping(rows, config, log))
    except KeyboardInterrupt:
        log.info("Interrupted by user.")


if __name__ == "__main__":
    main()

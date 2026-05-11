"""
Download Pages Script

Downloads pagination pages (2-4) for related video scraping
and saves them to disk to avoid repeated network requests.
"""

import asyncio
import logging
import random
import sys
from dataclasses import dataclass
from pathlib import Path

import httpx
from soromantic_utils.common import get_db_connection, get_download_dir, load_config


@dataclass
class Config:
    """Configuration for the downloader."""

    user_agent: str = (
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) "
        "AppleWebKit/537.36 (KHTML, like Gecko) "
        "Chrome/120.0.0.0 Safari/537.36"
    )
    batch_size: int = 10
    execution_limit: int = 10000  # Increased limit for full run
    timeout: float = 30.0
    pages_to_fetch: tuple[int, ...] = (1, 2, 3)


def setup_logging(name: str = __name__) -> logging.Logger:
    """Setup and return a configured logger."""
    logging.basicConfig(
        level=logging.INFO,
        stream=sys.stderr,
        format="%(asctime)s - %(levelname)s - %(message)s",
    )
    return logging.getLogger(name)


def get_pagination_base_url(url: str) -> str:
    """Derive the base URL for pagination."""
    return url.rstrip("/")


async def fetch_and_save(
    client: httpx.AsyncClient,
    url: str,
    output_path: Path,
    config: Config,
    log: logging.Logger,
) -> bool:
    """Fetch URL and save to path. Returns True if downloaded, False otherwise."""
    if output_path.exists():
        # log.info("Skipping existing: %s", output_path.name)
        return False

    try:
        resp = await client.get(url, timeout=config.timeout, follow_redirects=True)
        resp.raise_for_status()
    except (httpx.HTTPError, httpx.TimeoutException) as e:
        log.warning("Error fetching %s: %s", url, e)
        return False

    try:
        output_path.parent.mkdir(parents=True, exist_ok=True)
        output_path.write_text(resp.text, encoding="utf-8")
        return True
    except OSError as e:
        log.error("Failed to write %s: %s", output_path, e)
        return False


async def process_page(
    client: httpx.AsyncClient,
    row: tuple[int, str],
    save_dir: Path,
    config: Config,
    log: logging.Logger,
) -> int:
    """Process a single video page, downloading its pagination pages."""
    pid, url = row
    base_url = get_pagination_base_url(url)
    downloaded_count = 0

    for p in config.pages_to_fetch:
        # e.g. .../pages/000123-2.html
        filename = f"{pid:06}-{p}.html"
        file_path = save_dir / filename
        if p == 1:
            p_url = base_url
        else:
            p_url = f"{base_url}/{p}"

        if await fetch_and_save(client, p_url, file_path, config, log):
            downloaded_count += 1
            # increased delay to avoid overwhelming server
            await asyncio.sleep(2.0 + random.random())

    return downloaded_count


async def run_downloader(
    rows: list[tuple[int, str]],
    save_dir: Path,
    config: Config,
    log: logging.Logger,
) -> None:
    """Run the main downloader loop."""
    chunks = [rows[i : i + config.batch_size] for i in range(0, len(rows), config.batch_size)]

    total_downloaded = 0
    pages_processed = 0

    async with httpx.AsyncClient(headers={"User-Agent": config.user_agent}) as client:
        for i, chunk in enumerate(chunks):
            if pages_processed >= config.execution_limit:
                log.info("Reached execution limit. Stopping.")
                break

            log.info(
                "Processing batch %s/%s (Pages processed: %s)...",
                i + 1,
                len(chunks),
                pages_processed,
            )

            tasks = [process_page(client, row, save_dir, config, log) for row in chunk]
            results = await asyncio.gather(*tasks)

            batch_downloaded = sum(results)
            total_downloaded += batch_downloaded
            pages_processed += len(chunk)

            if batch_downloaded > 0:
                log.info("Downloaded %s new files in this batch.", batch_downloaded)
                # Generous rate limit between batches
                await asyncio.sleep(5.0 + random.random() * 2.0)

    log.info("Total files downloaded: %s", total_downloaded)


def main() -> None:
    """Main entry point."""
    log = setup_logging()
    config = Config()
    app_config = load_config()

    download_root = get_download_dir(app_config)
    if not download_root:
        log.error("Download dir not configured.")
        sys.exit(1)

    pages_dir = download_root / "pages"
    log.info("Saving pages to: %s", pages_dir)

    with get_db_connection() as conn:
        cur = conn.cursor()
        log.info("Fetching target pages from database...")
        cur.execute(
            "SELECT id, url FROM pages "
            "WHERE url NOT LIKE '%xvideos.com%' "
            "AND url LIKE 'http%' "
            "AND (related_ids IS NULL OR related_ids = '') "
            "LIMIT ?",
            (config.execution_limit * 2,),
        )
        rows: list[tuple[int, str]] = cur.fetchall()
        log.info("Found %s candidates.", len(rows))

    if not rows:
        log.info("No pages found.")
        return

    try:
        asyncio.run(run_downloader(rows, pages_dir, config, log))
    except KeyboardInterrupt:
        log.info("Interrupted by user.")


if __name__ == "__main__":
    main()

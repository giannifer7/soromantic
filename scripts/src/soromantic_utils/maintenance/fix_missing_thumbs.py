import sys
import logging
from dataclasses import dataclass
from pathlib import Path

from soromantic_utils.common import get_db_connection, get_download_dir


@dataclass
class Config:
    thumbs_dir: Path


def find_missing_thumbs(
    conn: "sqlite3.Connection",
    config: Config,
    log: logging.Logger,
) -> list[int]:
    """Find pages with thumb_status=3 but no file on disk."""
    cursor = conn.cursor()
    cursor.execute("SELECT id FROM pages WHERE thumb_status = 3")
    pages: list[tuple[int]] = cursor.fetchall()

    log.info("Found %d pages with thumb_status=3", len(pages))

    missing: list[int] = []
    for (page_id,) in pages:
        filename = f"{page_id:06d}"
        found = any(
            (config.thumbs_dir / f"{filename}{ext}").exists()
            for ext in [".jpg", ".png", ".jpeg"]
        )
        if not found:
            missing.append(page_id)

    return missing


def reset_thumb_status(
    conn: "sqlite3.Connection",
    page_ids: list[int],
    log: logging.Logger,
) -> None:
    """Reset thumb_status to 0 for given page IDs."""
    cursor = conn.cursor()
    for page_id in page_ids:
        cursor.execute("UPDATE pages SET thumb_status = 0 WHERE id = ?", (page_id,))
    conn.commit()
    log.info("Reset thumb_status for %d pages", len(page_ids))


def main() -> None:
    logging.basicConfig(
        level=logging.INFO,
        stream=sys.stderr,
        format="%(levelname)s: %(message)s",
    )
    log = logging.getLogger(__name__)

    download_dir = get_download_dir()
    config = Config(thumbs_dir=Path(download_dir) / "thumbs")

    log.info("Scanning for missing thumbnail files in %s", config.thumbs_dir)

    with get_db_connection() as conn:
        missing = find_missing_thumbs(conn, config, log)

        if not missing:
            log.info("All thumbnails present. Nothing to fix!")
            return

        log.info("Found %d pages with missing thumb files", len(missing))

        reset_thumb_status(conn, missing, log)

        if len(missing) <= 20:
            log.info("IDs: %s", missing)
        else:
            log.info("First 10 IDs: %s ...", missing[:10])

        log.info("Re-run the scraper to download them.")


if __name__ == "__main__":
    main()

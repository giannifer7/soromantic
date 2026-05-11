
import asyncio
import logging
import random
import sqlite3
import sys
from dataclasses import dataclass
from pathlib import Path

import httpx
from soromantic_utils.common import get_db_connection, get_download_dir

@dataclass
class Config:
    """Configuration for the thumbnail downloader."""
    user_agent: str = (
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) "
        "AppleWebKit/537.36 (KHTML, like Gecko) "
        "Chrome/120.0.0.0 Safari/537.36"
    )
    concurrency_limit: int = 5
    timeout: float = 30.0
    csv_path: Path = Path("/home/g4/_prj/soromantic/missing_thumbs.csv")

def setup_logging() -> logging.Logger:
    logging.basicConfig(
        level=logging.INFO,
        format="%(asctime)s - %(levelname)s - %(message)s",
        stream=sys.stderr,
    )
    return logging.getLogger("thumb_downloader")

async def download_thumb(
    client: httpx.AsyncClient,
    pid: int,
    url: str,
    save_path: Path,
    semaphore: asyncio.Semaphore,
    log: logging.Logger,
) -> bool:
    async with semaphore:
        try:
            resp = await client.get(url, follow_redirects=True)
            if resp.status_code == 200:
                save_path.write_bytes(resp.content)
                return True
            log.warning(f"Failed to download {url}: Status {resp.status_code}")
        except Exception as e:
            log.warning(f"Error downloading {url}: {e}")
        
        return False

async def run_download(config: Config, log: logging.Logger):
    if not config.csv_path.exists():
        log.error(f"CSV not found: {config.csv_path}")
        return

    # 1. Load and deduplicate
    targets: dict[int, str] = {}
    with open(config.csv_path, "r", encoding="utf-8") as f:
        for line in f:
            if "|" in line:
                try:
                    pid_str, url = line.strip().split("|", 1)
                    pid = int(pid_str)
                    if pid not in targets:
                        targets[pid] = url
                except ValueError:
                    continue
    
    if not targets:
        log.info("No targets found in CSV.")
        return
    
    log.info(f"Loaded {len(targets)} unique thumbnail targets.")

    thumbs_dir = get_download_dir() / "thumbs"
    thumbs_dir.mkdir(parents=True, exist_ok=True)

    semaphore = asyncio.Semaphore(config.concurrency_limit)
    async with httpx.AsyncClient(headers={"User-Agent": config.user_agent}, timeout=config.timeout) as client:
        tasks = []
        for pid, url in targets.items():
            save_path = thumbs_dir / f"{pid:06}.jpg"
            if save_path.exists():
                continue
            tasks.append(download_thumb(client, pid, url, save_path, semaphore, log))
        
        if not tasks:
            log.info("All thumbnails already exist.")
            return

        log.info(f"Downloading {len(tasks)} thumbnails...")
        results = await asyncio.gather(*tasks)
        
        downloaded_count = sum(1 for r in results if r)
        log.info(f"Downloaded {downloaded_count} thumbnails.")

        # 2. Update Database
        if downloaded_count > 0:
            with get_db_connection() as conn:
                cur = conn.cursor()
                updated_ids = [pid for pid, success in zip(targets.keys(), results) if success]
                
                # Update in batches
                batch_size = 100
                for i in range(0, len(updated_ids), batch_size):
                    batch = updated_ids[i:i+batch_size]
                    placeholders = ",".join(["?"] * len(batch))
                    cur.execute(f"UPDATE pages SET thumb_status = 3 WHERE id IN ({placeholders})", batch)
                
                conn.commit()
                log.info(f"Updated {len(updated_ids)} pages to thumb_status = 3.")

def main():
    log = setup_logging()
    config = Config()
    asyncio.run(run_download(config, log))

if __name__ == "__main__":
    main()

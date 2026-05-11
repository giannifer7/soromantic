"""
Scan videos/480 directory for bad files and create a re-scrape list.

Bad files are:
- Files with .part extension (incomplete downloads)
- Files with resolution != 480p

For each bad file:
1. Delete the file
2. Write the corresponding pissvids page URL to batch_fix_pv.txt

Usage:
    # Dry run (default - shows what would be deleted)
    uv run fix_bad_480p

    # Actually delete and create batch file
    uv run fix_bad_480p --confirm
"""

import argparse
import os
from pathlib import Path

from soromantic_utils.common import get_db_connection, get_download_dir, load_config
from soromantic_utils.media import get_video_resolution


def fix_bad_480p(confirm: bool = False, db_path: Path | None = None) -> None:
    config = load_config()
    if db_path:
        config["runtime"]["db_path"] = str(db_path)

    download_dir_str = get_download_dir(config)
    if not download_dir_str:
        print("Error: download_dir not configured")
        return

    download_dir = Path(download_dir_str)
    videos_480_dir = download_dir / "videos" / "480"

    if not videos_480_dir.exists():
        print(f"Error: Directory does not exist: {videos_480_dir}")
        return

    dry_run = not confirm
    if dry_run:
        print("=" * 60)
        print("DRY RUN MODE - No changes will be made")
        print("Run with --confirm to actually delete")
        print("=" * 60)
    else:
        print("=" * 60)
        print("DESTRUCTIVE MODE - Changes will be permanent!")
        print("=" * 60)

    print(f"\nScanning: {videos_480_dir}")

    # Build mapping from local_path to page_url
    path_to_page_url: dict[str, str] = {}

    with get_db_connection(config) as conn:
        cursor = conn.cursor()

        # Query for pissvids pages with downloaded videos
        cursor.execute(
            """
            SELECT d.local_path, p.url
            FROM downloads d
            JOIN video_sources vs ON d.src_url = vs.url
            JOIN pages p ON vs.page_id = p.id
            WHERE (p.url LIKE '%://pissvids.com/%' OR p.url LIKE '%://www.pissvids.com/%')
              AND d.status = 'done'
              AND d.local_path IS NOT NULL
            """
        )

        for local_path, page_url in cursor.fetchall():
            path_to_page_url[local_path] = page_url

    print(f"Found {len(path_to_page_url)} videos with page mappings in database")

    # Scan the directory
    bad_files: list[tuple[Path, str, str]] = []  # (path, reason, page_url)
    unmapped_files: list[tuple[Path, str]] = []  # (path, reason)

    for file_path in videos_480_dir.iterdir():
        if not file_path.is_file():
            continue

        reason = None

        # Check for .part extension
        if file_path.suffix == ".part":
            reason = "incomplete (.part)"
        else:
            # Check resolution
            resolution = get_video_resolution(file_path)
            if resolution is None:
                reason = "unreadable (probe failed)"
            else:
                _, height = resolution
                if height != 480:
                    reason = f"wrong resolution ({height}p)"

        if reason:
            # Find the relative path for database lookup
            rel_path = file_path.relative_to(download_dir)
            page_url = path_to_page_url.get(str(rel_path))

            if page_url:
                bad_files.append((file_path, reason, page_url))
            else:
                unmapped_files.append((file_path, reason))

    print(f"\nBad files found: {len(bad_files)}")
    for path, reason, url in bad_files[:20]:
        print(f"  [{reason}] {path.name}")
    if len(bad_files) > 20:
        print(f"  ... and {len(bad_files) - 20} more")

    if unmapped_files:
        print(f"\nUnmapped bad files (no DB entry): {len(unmapped_files)}")
        for path, reason in unmapped_files[:10]:
            print(f"  [{reason}] {path.name}")
        if len(unmapped_files) > 10:
            print(f"  ... and {len(unmapped_files) - 10} more")

    if dry_run:
        print("\n" + "=" * 60)
        print("DRY RUN COMPLETE - Run with --confirm to delete and create batch")
        print("=" * 60)
        return

    # Execute deletions
    print("\nDeleting bad files...")
    deleted_count = 0
    page_urls: set[str] = set()

    for file_path, reason, page_url in bad_files:
        try:
            os.remove(file_path)
            deleted_count += 1
            page_urls.add(page_url)
        except OSError as e:
            print(f"  Error deleting {file_path}: {e}")

    # Also delete unmapped files (orphans)
    for file_path, reason in unmapped_files:
        try:
            os.remove(file_path)
            deleted_count += 1
        except OSError as e:
            print(f"  Error deleting {file_path}: {e}")

    print(f"  Deleted {deleted_count} files")

    # Write batch file
    output_file = Path("batch_fix_pv.txt")
    if page_urls:
        with open(output_file, "w") as f:
            for url in sorted(page_urls):
                f.write(f"{url}\n")

        print(f"\nWrote {len(page_urls)} URLs to {output_file.absolute()}")
        print("Use this file to re-scrape the affected pages.")
    else:
        print("\nNo page URLs to write.")

    print("\n" + "=" * 60)
    print("COMPLETE")
    print("=" * 60)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(
        description="Fix bad 480p videos by deleting and creating re-scrape list"
    )
    parser.add_argument(
        "--confirm",
        action="store_true",
        help="Actually delete (default is dry-run)",
    )
    parser.add_argument(
        "--db-path",
        type=Path,
        help="Path to sqlite database (overrides config)",
    )

    args = parser.parse_args()
    fix_bad_480p(args.confirm, args.db_path)

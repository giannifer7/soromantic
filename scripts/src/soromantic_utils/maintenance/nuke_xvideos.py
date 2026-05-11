"""
Completely remove all xvideos.com data from database and filesystem.

This script identifies all pages originating from xvideos.com and removes:
- All associated files (videos, thumbnails, covers, previews, cache)
- All database records (pages, video_sources, links, grid_boxes, downloads)
- Orphaned models and studios that have no remaining links

Usage:
    # Dry run (default - shows what would be deleted)
    uv run nuke_xvideos

    # Actually delete everything
    uv run nuke_xvideos --confirm
"""

import argparse
import os
import shutil
from pathlib import Path

from soromantic_utils.common import get_db_connection, get_download_dir, load_config


def nuke_xvideos(confirm: bool = False, db_path: Path | None = None) -> None:
    config = load_config()
    if db_path:
        config["runtime"]["db_path"] = str(db_path)

    download_dir_str = get_download_dir(config)
    if not download_dir_str:
        print("Error: download_dir not configured")
        return

    download_dir = Path(download_dir_str)
    thumbs_dir = download_dir / "thumbs"
    covers_dir = download_dir / "covers"
    previews_dir = download_dir / "previews"
    cache_dir = download_dir / "cache"
    nested_cache_previews = cache_dir / "cache" / "previews"

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

    with get_db_connection(config) as conn:
        cursor = conn.cursor()

        # 1. Find all xvideos pages (precise domain matching)
        cursor.execute(
            """
            SELECT id, url, title FROM pages 
            WHERE url LIKE '%://xvideos.com/%' 
               OR url LIKE '%://www.xvideos.com/%'
            """
        )
        pages = cursor.fetchall()

        if not pages:
            print("\nNo xvideos.com pages found in database.")
            return

        print(f"\nFound {len(pages)} xvideos.com pages to delete:\n")

        page_ids = []
        for page_id, url, title in pages:
            page_ids.append(page_id)
            display_title = title or "(no title)"
            print(f"  [{page_id}] {display_title[:60]}..." if len(display_title) > 60 else f"  [{page_id}] {display_title}")

        # 2. Collect files to delete
        files_to_delete: list[Path] = []
        dirs_to_delete: list[Path] = []

        for page_id in page_ids:
            # Video files via downloads table
            cursor.execute(
                """
                SELECT d.local_path
                FROM video_sources vs
                JOIN downloads d ON d.src_url = vs.url
                WHERE vs.page_id = ? AND d.local_path IS NOT NULL
                """,
                (page_id,),
            )
            for (local_path,) in cursor.fetchall():
                if local_path:
                    files_to_delete.append(download_dir / local_path)

            # Standard assets
            files_to_delete.append(thumbs_dir / f"{page_id:06}.jpg")
            files_to_delete.append(covers_dir / f"{page_id:06}.jpg")
            files_to_delete.append(previews_dir / f"{page_id:06}.mp4")

            # Cache directories
            dirs_to_delete.append(cache_dir / "previews" / str(page_id))
            dirs_to_delete.append(nested_cache_previews / str(page_id))

        # Filter to existing files/dirs
        existing_files = [f for f in files_to_delete if f.exists()]
        existing_dirs = [d for d in dirs_to_delete if d.exists() and d.is_dir()]

        print(f"\nFiles to delete: {len(existing_files)}")
        for f in existing_files[:20]:  # Show first 20
            print(f"  {f}")
        if len(existing_files) > 20:
            print(f"  ... and {len(existing_files) - 20} more")

        print(f"\nDirectories to delete: {len(existing_dirs)}")
        for d in existing_dirs[:10]:
            print(f"  {d}")
        if len(existing_dirs) > 10:
            print(f"  ... and {len(existing_dirs) - 10} more")

        # 3. Count DB records to delete
        page_id_placeholders = ",".join("?" * len(page_ids))

        cursor.execute(
            f"SELECT COUNT(*) FROM grid_boxes WHERE page_id IN ({page_id_placeholders})",
            page_ids,
        )
        grid_boxes_by_page = cursor.fetchone()[0]

        cursor.execute(
            f"SELECT COUNT(*) FROM grid_boxes WHERE related IN ({page_id_placeholders})",
            [str(pid) for pid in page_ids],
        )
        grid_boxes_by_related = cursor.fetchone()[0]

        cursor.execute(
            f"SELECT COUNT(*) FROM video_sources WHERE page_id IN ({page_id_placeholders})",
            page_ids,
        )
        video_sources_count = cursor.fetchone()[0]

        cursor.execute(
            f"SELECT COUNT(*) FROM links WHERE page_id IN ({page_id_placeholders})",
            page_ids,
        )
        links_count = cursor.fetchone()[0]

        # Downloads count
        cursor.execute(
            f"""
            SELECT COUNT(*) FROM downloads
            WHERE src_url IN (SELECT url FROM video_sources WHERE page_id IN ({page_id_placeholders}))
            """,
            page_ids,
        )
        downloads_count = cursor.fetchone()[0]

        print("\nDatabase records to delete:")
        print(f"  pages: {len(pages)}")
        print(f"  video_sources: {video_sources_count}")
        print(f"  downloads: {downloads_count}")
        print(f"  links: {links_count}")
        print(f"  grid_boxes (by page_id): {grid_boxes_by_page}")
        print(f"  grid_boxes (by related): {grid_boxes_by_related}")

        if dry_run:
            print("\n" + "=" * 60)
            print("DRY RUN COMPLETE - Run with --confirm to delete")
            print("=" * 60)
            return

        # 4. Execute deletions
        print("\nDeleting files...")
        deleted_files = 0
        for f in existing_files:
            try:
                os.remove(f)
                deleted_files += 1
            except OSError as e:
                print(f"  Error deleting {f}: {e}")

        print(f"  Deleted {deleted_files} files")

        print("Deleting directories...")
        deleted_dirs = 0
        for d in existing_dirs:
            try:
                shutil.rmtree(d)
                deleted_dirs += 1
            except OSError as e:
                print(f"  Error deleting {d}: {e}")

        print(f"  Deleted {deleted_dirs} directories")

        print("\nDeleting database records...")

        # Delete grid_boxes by related (incoming references)
        cursor.execute(
            f"DELETE FROM grid_boxes WHERE related IN ({page_id_placeholders})",
            [str(pid) for pid in page_ids],
        )
        print(f"  Deleted {cursor.rowcount} grid_boxes (by related)")

        # Delete grid_boxes by page_id
        cursor.execute(
            f"DELETE FROM grid_boxes WHERE page_id IN ({page_id_placeholders})",
            page_ids,
        )
        print(f"  Deleted {cursor.rowcount} grid_boxes (by page_id)")

        # Delete downloads
        cursor.execute(
            f"""
            DELETE FROM downloads
            WHERE src_url IN (SELECT url FROM video_sources WHERE page_id IN ({page_id_placeholders}))
            """,
            page_ids,
        )
        print(f"  Deleted {cursor.rowcount} downloads")

        # Delete video_sources
        cursor.execute(
            f"DELETE FROM video_sources WHERE page_id IN ({page_id_placeholders})",
            page_ids,
        )
        print(f"  Deleted {cursor.rowcount} video_sources")

        # Delete links
        cursor.execute(
            f"DELETE FROM links WHERE page_id IN ({page_id_placeholders})",
            page_ids,
        )
        print(f"  Deleted {cursor.rowcount} links")

        # Delete pages
        cursor.execute(
            f"DELETE FROM pages WHERE id IN ({page_id_placeholders})",
            page_ids,
        )
        print(f"  Deleted {cursor.rowcount} pages")

        # Clean up orphaned models (no remaining links)
        cursor.execute(
            """
            DELETE FROM models
            WHERE id NOT IN (SELECT DISTINCT model_id FROM links WHERE model_id IS NOT NULL)
            """
        )
        print(f"  Deleted {cursor.rowcount} orphaned models")

        # Clean up orphaned studios (no remaining links)
        cursor.execute(
            """
            DELETE FROM studios
            WHERE id NOT IN (SELECT DISTINCT studio_id FROM links WHERE studio_id IS NOT NULL)
            """
        )
        print(f"  Deleted {cursor.rowcount} orphaned studios")

        conn.commit()

        print("\n" + "=" * 60)
        print("DELETION COMPLETE")
        print("=" * 60)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(
        description="Remove all xvideos.com data from database and filesystem"
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
    nuke_xvideos(args.confirm, args.db_path)

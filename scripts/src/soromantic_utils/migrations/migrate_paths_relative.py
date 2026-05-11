#!/usr/bin/env python3

import os

from soromantic_utils.common import expand_path, get_db_connection, get_download_dir, load_config


# pylint: disable=too-many-locals
def migrate(dry_run=False):
    config = load_config()
    db_path = config.get("runtime", {}).get("db_path")
    print(f"Migrating database: {db_path}")
    if dry_run:
        print("DRY RUN: No changes will be committed.")

    # Resolve all configured roots
    download_dir = get_download_dir(config)

    runtime = config.get("runtime", {})
    thumbs_dir_cfg = runtime.get("thumbs_dir")
    covers_dir_cfg = runtime.get("covers_dir")
    videos_dir_cfg = runtime.get("videos_dir")
    previews_dir_cfg = runtime.get("previews_dir")

    # If cfg is None, default to download_dir/subdir
    thumbs_root = expand_path(thumbs_dir_cfg) if thumbs_dir_cfg else os.path.join(download_dir, "thumbs")
    covers_root = expand_path(covers_dir_cfg) if covers_dir_cfg else os.path.join(download_dir, "covers")
    videos_root = expand_path(videos_dir_cfg) if videos_dir_cfg else os.path.join(download_dir, "videos")
    previews_root = (
        expand_path(previews_dir_cfg) if previews_dir_cfg else os.path.join(download_dir, "previews")
    )

    # We will try to match longest prefix first
    roots = [
        (thumbs_root, "thumbs"),
        (covers_root, "covers"),
        (videos_root, "videos"),
        (previews_root, "previews"),
    ]
    roots.sort(key=lambda x: len(x[0]), reverse=True)

    print("Configured roots:")
    for path, name in roots:
        print(f"  {name}: {path}")

    with get_db_connection(config) as conn:
        cursor = conn.cursor()

        # 1. Update downloads table
        print("\nChecking 'downloads' table...")
        cursor.execute("SELECT id, local_path FROM downloads WHERE local_path IS NOT NULL")
        rows = cursor.fetchall()

        updated_count = 0
        for row_id, path in rows:
            if not os.path.isabs(path):
                continue  # Already relative

            new_path = None
            for root, prefix in roots:
                if path.startswith(root):
                    # Remove root + separator
                    rel = path[len(root) :].lstrip(os.sep)
                    new_path = f"{prefix}/{rel}"
                    break

            if new_path:
                if dry_run:
                    print(f"  [DRY] Would update {row_id}: {path} -> {new_path}")
                else:
                    cursor.execute("UPDATE downloads SET local_path = ? WHERE id = ?", (new_path, row_id))
                updated_count += 1
            else:
                print(f"WARNING: Path not in any configured root: {path}")

        print(f"Updated {updated_count} rows in downloads.")

        if not dry_run:
            conn.commit()
            print("Changes committed.")
        else:
            print("Dry run complete. No changes made.")


if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser(description="Migrate absolute paths to relative paths in DB.")
    parser.add_argument("--dry-run", action="store_true", help="Print what would happen without modifying DB")
    args = parser.parse_args()

    migrate(dry_run=args.dry_run)

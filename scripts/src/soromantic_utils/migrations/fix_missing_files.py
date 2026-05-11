import os
import shutil
import sqlite3

from soromantic_utils.common import get_db_path, get_download_dir

DB_PATH = get_db_path()
DOWNLOAD_DIR = get_download_dir()

if not DOWNLOAD_DIR:
    print("Error: Could not determine download_dir from config")
    sys.exit(1)

# Assuming standard structure inside download directory or matching previous hardcoded paths relative to it?
# Previous: /mnt/sda3/porn/pyssvids/videos/576
# Config seems to point to /mnt/sda3/porn/pyssvids/downloads usually.
# If download_dir is ".../downloads", then videos might be sibling or child.
# But usually `download_dir` is where downloads go.
# Let's assume structure: `download_dir/videos/576` etc OR `download_dir/...`
# `output/downloads`...
# I'll Assume `get_download_dir` returns the root where `videos/` live or `download_dir` IS the root.
# If `download_dir` ends in `downloads`, maybe `videos` is inside.
DIR_576 = os.path.join(DOWNLOAD_DIR, "videos", "576")
DIR_480 = os.path.join(DOWNLOAD_DIR, "videos", "480")

if not os.path.exists(DB_PATH):
    print(f"Error: Database not found at {DB_PATH}")
    sys.exit(1)

conn = sqlite3.connect(DB_PATH)
c = conn.cursor()

# Get list of files physically in 576
if not os.path.exists(DIR_576):
    print(f"Directory {DIR_576} does not exist. Nothing to do.")
    sys.exit(0)

files_in_576 = os.listdir(DIR_576)
print(f"Found {len(files_in_576)} files in {DIR_576}: {files_in_576}")

for filename in files_in_576:
    file_path_576 = os.path.join(DIR_576, filename)
    file_path_480 = os.path.join(DIR_480, filename)

    # Check if DB expects this file in 480
    expected_path_480 = file_path_480

    c.execute("SELECT COUNT(*) FROM downloads WHERE local_path = ?", (expected_path_480,))
    count = c.fetchone()[0]

    if count > 0:
        print(f"File {filename} is expected in 480 according to DB. Moving...")
        if not os.path.exists(DIR_480):
            os.makedirs(DIR_480)

        shutil.move(file_path_576, file_path_480)
        print(f"Moved {filename} to {DIR_480}")
    else:
        print(f"File {filename} is NOT linked to 480 in DB. Checking if it's linked to 576...")
        c.execute("SELECT COUNT(*) FROM downloads WHERE local_path = ?", (file_path_576,))
        count_576 = c.fetchone()[0]
        if count_576 > 0:
            print(
                f"File {filename} is correctly linked to 576 in DB. (Wait, I thought we migrated everything?)"
            )
            # If we migrated everything, this shouldn't happen.
            # Unless the migration failed or only covered regex matches?
            # Migration used `UPDATE downloads SET local_path = REPLACE(local_path, '/videos/576/',
            # '/videos/480/')`.
            # So if the path in DB was /videos/576/..., it is now /videos/480/...
        else:
            print(f"File {filename} is orphaned (not in DB as 480 or 576).")

# Also check for empty DB entries that point to 576 but file is missing
# (Not part of this fix, but good info)

conn.close()

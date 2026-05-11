import os
import sqlite3

from soromantic_utils.common import get_db_path

DB_PATH = get_db_path()

if not os.path.exists(DB_PATH):
    print(f"Error: Database not found at {DB_PATH}")
    sys.exit(1)

conn = sqlite3.connect(DB_PATH)
c = conn.cursor()

# 1. Update resolution in video_sources
print("Updating video_sources resolution from 576 to 480...")
c.execute("UPDATE video_sources SET resolution = 480 WHERE resolution = 576")
print(f"Updated {c.rowcount} rows in video_sources.")

# 2. Update local_path in downloads
print("Updating downloads local_path from matching 576 pattern...")
# Find downloads associated with previously 576p videos (now 480p) or just fix paths containing /576/
# The safest is to target downloads where src_url matches what was 576p, but since we updated them,
# we can just fix any path containing /videos/576/ to /videos/480/ provided the file exists.
# Or simpler: Update ALL paths with /videos/576/ to /videos/480/
c.execute("SELECT src_url, local_path FROM downloads WHERE local_path LIKE '%/videos/576/%'")
rows = c.fetchall()

updated_paths = 0
for src_url, local_path in rows:
    new_path = local_path.replace("/videos/576/", "/videos/480/")
    print(f"Updating path: {local_path} -> {new_path}")
    c.execute("UPDATE downloads SET local_path = ? WHERE src_url = ?", (new_path, src_url))
    updated_paths += 1

print(f"Updated {updated_paths} paths in downloads.")

conn.commit()
conn.close()
print("Migration completed successfully.")

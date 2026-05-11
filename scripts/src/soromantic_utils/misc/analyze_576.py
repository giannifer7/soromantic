import os
import sqlite3

from soromantic_utils.common import get_db_path

DB_PATH = get_db_path()

if not os.path.exists(DB_PATH):
    print(f"Error: Database not found at {DB_PATH}")
    sys.exit(1)

conn = sqlite3.connect(DB_PATH)
c = conn.cursor()

# Count 576p entries
query_count = "SELECT COUNT(*) FROM video_sources WHERE resolution = 576"
c.execute(query_count)
count_576 = c.fetchone()[0]

print(f"Total 576p entries in video_sources: {count_576}")

if count_576 > 0:
    # Check linked downloads
    query_downloads = """
    SELECT d.local_path, vs.url
    FROM video_sources vs
    JOIN downloads d ON d.src_url = vs.url
    WHERE vs.resolution = 576
    LIMIT 10
    """
    c.execute(query_downloads)
    rows = c.fetchall()

    print("\nSample paths for 576p entries:")
    for path, _url in rows:
        print(f"Path: {path}")

conn.close()

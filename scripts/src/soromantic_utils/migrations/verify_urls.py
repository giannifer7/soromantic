import os
import sqlite3

from soromantic_utils.common import get_db_path

DB_PATH = get_db_path()

if not os.path.exists(DB_PATH):
    print(f"Error: Database not found at {DB_PATH}")
    sys.exit(1)

conn = sqlite3.connect(DB_PATH)
c = conn.cursor()

# Find pages that have multiple video sources
query = """
SELECT page_id, resolution, url
FROM video_sources
WHERE page_id IN (
    SELECT page_id FROM video_sources GROUP BY page_id HAVING COUNT(*) > 1
)
ORDER BY page_id
LIMIT 50
"""

c.execute(query)
rows = c.fetchall()

if not rows:
    print("No pages found with multiple video sources.")
else:
    print(f"Found {len(rows)} entries:")
    current_page = None
    for row in rows:
        page_id, res, url = row
        if page_id != current_page:
            print("-" * 40)
            print(f"Page {page_id}:")
            current_page = page_id

        print(f"  Res: {res} | URL: {url}")

conn.close()

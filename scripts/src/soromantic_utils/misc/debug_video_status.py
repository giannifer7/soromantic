import sqlite3

from soromantic_utils.common import get_db_path


def debug_status():
    db_path = get_db_path()
    print(f"Opening database: {db_path}")

    conn = sqlite3.connect(db_path)
    cursor = conn.cursor()

    target_id = 2113
    print(f"Inspecting Page {target_id}...")

    try:
        # Get Page info
        cursor.execute("SELECT id, url, title FROM pages WHERE id = ?", (target_id,))
        page = cursor.fetchone()
        if not page:
            print(f"Page {target_id} NOT FOUND in pages table!")
            return

        print(f"Page found: {page}")

        # Get Video Sources
        cursor.execute("SELECT url, resolution FROM video_sources WHERE page_id = ?", (target_id,))
        sources = cursor.fetchall()
        print(f"Found {len(sources)} video sources in 'video_sources' table.")

        for src_url, res in sources:
            print(f"  Source: {src_url} ({res}p)")

            # Check Downloads table exact match
            cursor.execute("SELECT local_path, status FROM downloads WHERE src_url = ?", (src_url,))
            row = cursor.fetchone()
            if row:
                print(f"    -> Downloads Match! Status='{row[1]}', Path='{row[0]}'")
            else:
                print("    -> NO Match in downloads table for exact URL.")
                # Check fuzzy match?
                cursor.execute(
                    "SELECT src_url, status FROM downloads WHERE src_url LIKE ?",
                    (src_url.split("?")[0] + "%",),
                )
                fuzzy = cursor.fetchall()
                if fuzzy:
                    print("    -> BUT found fuzzy matches:")
                    for f in fuzzy:
                        print(f"       - {f[0]} (Status: {f[1]})")

    except Exception as e:
        print(f"Error: {e}")
    finally:
        conn.close()


if __name__ == "__main__":
    debug_status()

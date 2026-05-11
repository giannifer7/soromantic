import sqlite3

from soromantic_utils.common import get_db_path


def fix_slashes():
    db_path = get_db_path()
    print(f"Opening database: {db_path}")

    conn = sqlite3.connect(db_path)
    cursor = conn.cursor()

    try:
        # Check broken links (strict equality fails)
        cursor.execute("""
            SELECT COUNT(*) FROM grid_boxes gb
            LEFT JOIN pages p ON p.url = gb.related
            WHERE gb.related IS NOT NULL AND gb.related != '' AND p.id IS NULL
        """)
        broken_count = cursor.fetchone()[0]
        print(f"Broken links (strict match failed): {broken_count}")

        # Check traversable links (RTRIM match succeeds)
        cursor.execute("""
            SELECT COUNT(*) FROM grid_boxes gb
            JOIN pages p ON p.url = RTRIM(gb.related, '/')
            WHERE gb.related != p.url
        """)
        fixable_count = cursor.fetchone()[0]
        print(f"Fixable links (trailing slash mismatch): {fixable_count}")

        if fixable_count > 0:
            print("Applying fix for trailing slashes...")
            # SQLite doesn't support JOIN in UPDATE easily, but we can use subquery or pure RTRIM logic
            # Since we know p.url IS rtrim(gb.related, '/'), we can just update gb.related
            # to RTRIM(gb.related, '/')
            # ensuring we only do it where it actually changes (LIKE '%/')
            cursor.execute("UPDATE grid_boxes SET related = RTRIM(related, '/') WHERE related LIKE '%/'")
            print(f"Updated {cursor.rowcount} rows.")
            conn.commit()

            # Re-check broken links
            cursor.execute("""
                SELECT COUNT(*) FROM grid_boxes gb
                LEFT JOIN pages p ON p.url = gb.related
                WHERE gb.related IS NOT NULL AND gb.related != '' AND p.id IS NULL
            """)
            new_broken = cursor.fetchone()[0]
            print(f"Remaining broken links: {new_broken}")
        else:
            print("No trailing slash mismatches found.")

    except Exception as e:
        print(f"Error: {e}")
        conn.rollback()
    finally:
        conn.close()


if __name__ == "__main__":
    fix_slashes()

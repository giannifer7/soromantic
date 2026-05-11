import sqlite3
import argparse
from soromantic_utils import common

def main():
    parser = argparse.ArgumentParser(description="Deduplicate links table and add UNIQUE constraint")
    parser.add_argument("--dry-run", action="store_true", help="Print what would be deleted without actually deleting")
    args = parser.parse_args()

    # Use the context manager to get the connection
    with common.get_db_connection() as conn:
        cursor = conn.cursor()

        print("Checking for duplicate entries in 'links' table...")

        # Find duplicates
        query = """
        SELECT page_id, taxonomy_id, COUNT(*) as count
        FROM links
        GROUP BY page_id, taxonomy_id
        HAVING count > 1
        """
        cursor.execute(query)
        duplicates = cursor.fetchall()

        if not duplicates:
            print("No duplicates found. The 'links' table is clean.")
        else:
            print(f"Found {len(duplicates)} pairs with duplicate entries.")
            
            total_deleted = 0
            for page_id, tax_id, count in duplicates:
                print(f"  - Page {page_id} <-> Tax {tax_id}: {count} copies")
                
                # Keep the row with the lowest ID (assuming 'id' column exists, otherwise use rowid)
                # Let's check schema for ID column? 
                # Usually links tables might just be (page_id, taxonomy_id). 
                # If so, we use rowid.
                
                cleanup_query = """
                DELETE FROM links 
                WHERE rowid NOT IN (
                    SELECT MIN(rowid) 
                    FROM links 
                    WHERE page_id = ? AND taxonomy_id = ?
                )
                AND page_id = ? AND taxonomy_id = ?
                """
                
                if not args.dry_run:
                    cursor.execute(cleanup_query, (page_id, tax_id, page_id, tax_id))
                    total_deleted += cursor.rowcount
                else:
                    total_deleted += (count - 1)

            print(f"\\nTotal duplicate rows {'would be ' if args.dry_run else ''}deleted: {total_deleted}")

        if not args.dry_run:
            # Add UNIQUE index if not exists
            print("\\nEnsuring UNIQUE constraint...")
            try:
                cursor.execute("CREATE UNIQUE INDEX IF NOT EXISTS idx_links_unique ON links(page_id, taxonomy_id)")
                print("✓ UNIQUE index 'idx_links_unique' created/verified.")
            except sqlite3.OperationalError as e:
                print(f"⚠ Could not create unique index: {e}")
            
            conn.commit()
            print("✓ Changes committed.")
        else:
            print("\\n[Dry Run] No changes committed.")

if __name__ == "__main__":
    main()

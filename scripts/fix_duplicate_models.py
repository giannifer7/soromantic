import sqlite3
import os
import sys

# DB Path from config
DB_PATH = "/mnt/sda3/porn/pyssvids/db/data.db"

def main():
    if not os.path.exists(DB_PATH):
        print(f"Database not found at {DB_PATH}")
        sys.exit(1)

    conn = sqlite3.connect(DB_PATH)
    cursor = conn.cursor()

    try:
        # Find duplicates (same name, same type)
        cursor.execute("""
            SELECT name, type, COUNT(*) as c
            FROM taxonomies
            GROUP BY name, type
            HAVING c > 1
        """)
        duplicates = cursor.fetchall()

        print(f"Found {len(duplicates)} duplicate sets.")

        for name, tax_type, count in duplicates:
            print(f"Processing duplicate: {name} (Type: {tax_type}, Count: {count})")
            
            # Get all entries for this duplicate set
            cursor.execute("""
                SELECT id, url FROM taxonomies
                WHERE name = ? AND type = ?
                ORDER BY id ASC
            """, (name, tax_type))
            entries = cursor.fetchall()
            
            # Strategy:
            # 1. Prefer pyssvids.com URL if available.
            # 2. Otherwise prefer the one with the lowest ID (first inserted).
            
            target_id = None
            target_url = None
            ids_to_merge = []

            # Check for pyssvids preference
            for eid, eurl in entries:
                if "pissvids.com" in eurl or "pyssvids.com" in eurl:
                    if target_id is None:
                        target_id = eid
                        target_url = eurl
            
            # Fallback to first ID if no priority URL found
            if target_id is None:
                target_id = entries[0][0]
                target_url = entries[0][1]

            # Collect IDs to remove
            for eid, eurl in entries:
                if eid != target_id:
                    ids_to_merge.append(eid)

            print(f"  -> Keeping ID: {target_id} ({target_url})")
            print(f"  -> Merging IDs: {ids_to_merge}")

            for old_id in ids_to_merge:
                # Reassign links
                cursor.execute("""
                    UPDATE OR IGNORE links SET taxonomy_id = ? WHERE taxonomy_id = ?
                """, (target_id, old_id))
                
                # Delete duplicate links (if UPDATE caused conflicts due to OR IGNORE, we might have leftover links to old_id? 
                # No, OR IGNORE on UPDATE means if (page_id, new_tax_id) exists, we skip updating.
                # So we might have some links still pointing to old_id that are redundant.
                # We should delete them safely.)
                cursor.execute("""
                    DELETE FROM links WHERE taxonomy_id = ?
                """, (old_id,))

                # Delete the taxonomy entry
                cursor.execute("DELETE FROM taxonomies WHERE id = ?", (old_id,))
                print(f"     Merged and deleted ID {old_id}")

        conn.commit()
        print("Cleanup complete.")

    except Exception as e:
        print(f"Error: {e}")
        conn.rollback()
    finally:
        conn.close()

if __name__ == "__main__":
    main()

import sqlite3
import os
import sys

MAIN_DB = "/mnt/sda3/porn/pyssvids/db/data.db"
BACKUP_DB = "/mnt/sda3/porn/pyssvids/db-bak/db/data.db"

def restore_relations():
    if not os.path.exists(MAIN_DB):
        print(f"Main DB not found at {MAIN_DB}")
        return
    if not os.path.exists(BACKUP_DB):
        print(f"Backup DB not found at {BACKUP_DB}")
        return

    print(f"Connecting to Main: {MAIN_DB}")
    main_conn = sqlite3.connect(MAIN_DB)
    main_cur = main_conn.cursor()

    print(f"Connecting to Backup: {BACKUP_DB}")
    back_conn = sqlite3.connect(BACKUP_DB)
    back_conn.row_factory = sqlite3.Row
    back_cur = back_conn.cursor()

    # 1. Load Backup Grid Boxes
    print("Reading grid_boxes from backup...")
    back_cur.execute("SELECT page_id, related FROM grid_boxes WHERE related IS NOT NULL AND related != ''")
    rows = back_cur.fetchall()
    
    # 2. Group by page_id -> [related_url]
    relations_map = {} # page_id -> set(related_url)
    for row in rows:
        pid = row['page_id']
        url = row['related'].strip().rstrip('/')
        if pid not in relations_map:
            relations_map[pid] = set()
        relations_map[pid].add(url)
    
    print(f"Found relations for {len(relations_map)} pages.")

    # 3. Resolve URLs to IDs in Main DB
    # Fetch all pages url->id map for fast lookup
    print("Loading page map from Main DB...")
    main_cur.execute("SELECT id, url FROM pages")
    url_to_id = {}
    for row in main_cur.fetchall():
        u = row[1].strip().rstrip('/')
        url_to_id[u] = row[0]
    
    print(f"Loaded {len(url_to_id)} pages from Main DB.")

    # 4. Construct related_ids and Update
    print("updating relations...")
    updated_count = 0
    
    for page_id, related_urls in relations_map.items():
        # Check if source page exists in main db (id might be same or different? IDs should be stable ideally)
        # Actually, if we rebuilt tables, IDs *might* have changed if we didn't preserve them exactly.
        # But we did `INSERT INTO ... SELECT id, ...`, so IDs should be preserved.
        # However, to be safe, let's look up the source page URL in backup, then find its ID in main.
        
        # Determine Source Page ID in Main DB
        # We have `page_id` from Backup. Does it match `page_id` in Main?
        # Let's verify a few. Or just assume IDs are stable if we didn't re-scrape.
        # Restoring from backup usually implies same IDs if data was just copied.
        # But let's be robust: Get Source URL from Backup Page ID -> Find Main ID.
        
        back_cur.execute("SELECT url FROM pages WHERE id = ?", (page_id,))
        source_row = back_cur.fetchone()
        if not source_row:
            continue
            
        source_url = source_row['url'].strip().rstrip('/')
        if source_url not in url_to_id:
            continue
            
        target_main_id = url_to_id[source_url]
        
        # Resolve related URLs
        related_ids = []
        for r_url in related_urls:
            if r_url in url_to_id:
                related_ids.append(url_to_id[r_url])
        
        if not related_ids:
            continue
            
        # Serialize: ",1,2,3,"
        related_ids.sort()
        ids_str = "," + ",".join(map(str, related_ids)) + ","
        
        main_cur.execute("UPDATE pages SET related_ids = ? WHERE id = ?", (ids_str, target_main_id))
        updated_count += 1
        
        if updated_count % 1000 == 0:
            print(f"Updated {updated_count} pages...")
            main_conn.commit()

    main_conn.commit()
    print(f"Finished restoring relations for {updated_count} pages.")
    main_conn.close()
    back_conn.close()

if __name__ == "__main__":
    restore_relations()

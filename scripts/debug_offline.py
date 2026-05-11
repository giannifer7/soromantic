
import logging
import sys
from pathlib import Path
from soromantic_utils.common import get_db_connection, load_config, get_download_dir
from soromantic_utils.maintenance.rescrape_related import (
    extract_page_title,
    extract_related_urls,
    get_pagination_base_url,
    process_page_update,
)

def main():
    logging.basicConfig(level=logging.INFO)
    log = logging.getLogger("debug")
    
    pages_dir = get_download_dir() / "pages"
    all_files = list(pages_dir.glob("*.html"))
    print(f"Total files in pages dir: {len(all_files)}")
    
    if not all_files:
        return

    test_file = all_files[0]
    print(f"Testing with file: {test_file}")
    
    pid = int(test_file.stem.split("-")[0])
    print(f"Deduced PID: {pid}")
    
    with get_db_connection() as conn:
        cur = conn.cursor()
        cur.execute("SELECT id, url FROM pages WHERE id = ?", (pid,))
        row = cur.fetchone()
        if not row:
            print(f"ERROR: PID {pid} not found in database!")
            return
        
        base_url = row[1]
        print(f"Found base URL in DB: {base_url}")
        
        clean_base = get_pagination_base_url(base_url)
        html = test_file.read_text(encoding="utf-8", errors="replace")
        
        title = extract_page_title(html)
        related = extract_related_urls(html, clean_base)
        print(f"Found {len(related)} related items in file.")
        
        if related:
            print(f"First related item: {related[0]}")
            
        cur.execute("SELECT id, url, related_ids, title FROM pages WHERE id = ?", (pid,))
        db_row = cur.fetchone()
        print(f"Current DB state for {pid}: {db_row}")
        
        # Test the update call
        processed_data = {pid: (title, related)}
        print("Executing process_page_update...")
        # Note: we need to wrap db_row in a list to mimic chunk
        res = process_page_update(cur, [db_row], pid, processed_data[pid], log)
        print(f"process_page_update returned: {res}")
        
if __name__ == "__main__":
    main()

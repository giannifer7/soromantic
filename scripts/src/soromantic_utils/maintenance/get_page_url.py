
import argparse
import sys
from pathlib import Path
from soromantic_utils.common import get_db_connection, load_config

def get_url(page_id: int, config: dict | None = None):
    with get_db_connection(config) as conn:
        cursor = conn.cursor()
        cursor.execute("SELECT url FROM pages WHERE id = ?", (page_id,))
        row = cursor.fetchone()
        
        if row:
            print(row[0])
        else:
            print(f"Page with ID {page_id} not found.", file=sys.stderr)
            sys.exit(1)

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Get URL for a page ID")
    parser.add_argument("page_id", type=int, help="ID of the page")
    parser.add_argument("--db-path", type=Path, help="Path to sqlite database")
    
    args = parser.parse_args()
    
    config = None
    if args.db_path:
        config = {"runtime": {"db_path": str(args.db_path)}}
    else:
        config = load_config()
    
    get_url(args.page_id, config)

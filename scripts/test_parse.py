
import sys
from pathlib import Path
from bs4 import BeautifulSoup
from soromantic_utils.maintenance.rescrape_related import extract_related_urls, extract_page_title

def test_parse(file_path: Path):
    print(f"Testing file: {file_path}")
    html = file_path.read_text(encoding="utf-8", errors="replace")
    
    title = extract_page_title(html)
    print(f"Title: {title}")
    
    # Placeholder base URL
    base_url = "https://pissvids.com"
    related = extract_related_urls(html, base_url)
    
    print(f"Found {len(related)} related items.")
    for i, (url, r_title, img) in enumerate(related[:5]):
        print(f"  {i+1}. Title: {r_title}")
        print(f"     URL: {url}")
        print(f"     IMG: {img}")

if __name__ == "__main__":
    if len(sys.argv) < 2:
        # Try to pick a file automatically
        pages_dir = Path("/mnt/sda3/porn/pyssvids/pages")
        files = list(pages_dir.glob("*.html"))
        if files:
            test_parse(files[0])
        else:
            print("No files found in pages dir.")
    else:
        test_parse(Path(sys.argv[1]))

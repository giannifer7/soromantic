import os
import sqlite3

from PIL import Image

from soromantic_utils.common import expand_path, get_db_path

# Pixel intensity threshold for "black" (0-255)
BLACK_THRESHOLD = 15


def detect_and_crop(image_path):
    try:
        with Image.open(image_path) as img:
            # Convert to grayscale for analysis
            gray = img.convert("L")

            # Create a mask: 0 (black) if pixel < threshold, 255 (white) otherwise
            # This handles "noisy black" often found in compression artifacts
            mask = gray.point(lambda p: 255 if p > BLACK_THRESHOLD else 0).convert("1")

            # Get bounding box of non-black regions
            bbox = mask.getbbox()

            if not bbox:
                # Image is entirely black?
                return False

            w, h = img.size
            left, top, right, bottom = bbox

            # Check if crop is significant (e.g. > 2 pixels changed)
            if left <= 2 and top <= 2 and right >= w - 2 and bottom >= h - 2:
                return False  # Virtually no black bars

            print(
                f"Cropping {os.path.basename(image_path)}: {w}x{h} -> "
                f"{right - left}x{bottom - top} (Box: {bbox})"
            )

            # Crop and save
            # We must crop the ORIGINAL image, not grayscale
            cropped = img.crop(bbox)

            # Save. We need to close the file handle first? `with Image.open` handles input.
            # But writing to same path might be tricky on some OS if open.
            # Loading entire image into memory ensures we can write back.
            cropped.load()
            cropped.save(image_path, quality=95)

            return True

    except Exception as e:
        print(f"Error processing {image_path}: {e}")
        return False


def main():
    print("Starting smart thumbnail cropping for xvideos...")
    db_path = get_db_path()

    if not os.path.exists(db_path):
        print(f"Error: Database not found at {db_path}")
        return

    conn = sqlite3.connect(db_path)
    cursor = conn.cursor()

    # Select downloaded thumbnails from xvideos
    # We filter by src_url content.
    # Also checking local_path extension roughly helps, but Image.open handles format.
    query = """
        SELECT local_path 
        FROM downloads 
        WHERE src_url LIKE '%xvideos%' 
          AND status = 'done'
          AND local_path IS NOT NULL
    """

    cursor.execute(query)
    rows = cursor.fetchall()

    print(f"Found {len(rows)} xvideos downloads. Scanning for black bars...")

    count = 0
    fixed = 0

    for (path_str,) in rows:
        path = expand_path(path_str)
        if path and os.path.exists(path):
            # Only process image files (skip .mp4 video previews if they somehow got in
            # downloads table without extension filter)
            # Actually downloads table mixes videos and images.
            # We should check extension or assume if xvideos and it's a thumbnail...
            # But xvideos downloads BOTH video and thumb?
            # Usually distinct entries.
            # Let's check extension.
            ext = os.path.splitext(path)[1].lower()
            if ext not in [".jpg", ".jpeg", ".png", ".webp"]:
                continue

            if detect_and_crop(path):
                fixed += 1

            count += 1
            if count % 100 == 0:
                print(f"Scanned {count} images...")

    print(f"Finished. Scanned {count} images. Fixed {fixed} thumbnails.")
    conn.close()


if __name__ == "__main__":
    main()

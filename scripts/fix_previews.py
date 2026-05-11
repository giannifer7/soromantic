import os
import shutil
import re
from pathlib import Path

# Paths
PREVIEWS_DIR = Path("/mnt/sda3/porn/pyssvids/previews")
CACHE_PREVIEWS_DIR = Path("/mnt/sda3/porn/pyssvids/cache/previews")

def normalize_mp4s():
    """Normalize MP4 filenames in PREVIEWS_DIR to 6-digit zero-padded."""
    print("Scanning MP4s in previews dir...")
    count = 0
    for file_path in PREVIEWS_DIR.glob("*.mp4"):
        name = file_path.name
        # Match digits before .mp4
        match = re.match(r"^(\d+)\.mp4$", name)
        if match:
            digits = match.group(1)
            print(f"Checking {name}, digits={digits}, len={len(digits)}") # DEBUG
            if len(digits) != 6:
                # Pad/Normalize to 6 digits
                try:
                    val = int(digits)
                    new_digits = f"{val:06}"
                except ValueError:
                    print(f"Skipping non-integer name: {name}")
                    continue
                    
                if new_digits == digits:
                    continue

                new_name = f"{new_digits}.mp4"
                new_path = PREVIEWS_DIR / new_name
                
                # Check collision
                if new_path.exists() and new_path != file_path:
                    # Check if they are likely same file (collisions)
                    if new_path.stat().st_size == file_path.stat().st_size:
                        print(f"Removing duplicate {name} (keeping {new_name})")
                        file_path.unlink()
                        count += 1
                    else:
                        print(f"WARNING: Cannot rename {name} to {new_name}, target exists and differs.")
                elif new_path != file_path:
                    print(f"Renaming {name} -> {new_name}")
                    file_path.rename(new_path)
                    count += 1
    print(f"Renamed/Removed {count} MP4 files.")

def remove_item(path):
    if path.is_dir():
        shutil.rmtree(path)
    else:
        path.unlink()

def move_and_normalize_dirs():
    """
    1. Move dirs from PREVIEWS_DIR to CACHE_PREVIEWS_DIR.
    2. Normalize all dirs in CACHE_PREVIEWS_DIR to 6-digit zero-padded.
    """
    print("Moving and normalizing directories...")
    
    # Ensure cache dir exists
    CACHE_PREVIEWS_DIR.mkdir(parents=True, exist_ok=True)
    
    # 1. Move dirs from previews/ to cache/previews/
    moved_count = 0
    for item in PREVIEWS_DIR.iterdir():
        if item.is_dir():
            # It's a photogram dir
            target_path = CACHE_PREVIEWS_DIR / item.name
            if target_path.exists():
                print(f"Merging {item.name} into cache...")
                # Merge logic: move contents, remove source dir
                for subfile in item.iterdir():
                    dest = target_path / subfile.name
                    if dest.exists():
                        remove_item(subfile)
                    else:
                        shutil.move(str(subfile), str(dest))
                # Remove now empty source dir
                try:
                    item.rmdir()
                except OSError as e:
                    print(f"Warning: Could not remove {item}: {e}")
            else:
                print(f"Moving {item.name} -> cache/")
                shutil.move(str(item), str(target_path))
            moved_count += 1
    print(f"Moved {moved_count} directories to cache.")

    # 2. Normalize dirs in cache/previews/
    renamed_count = 0
    for dir_path in CACHE_PREVIEWS_DIR.iterdir():
        if dir_path.is_dir():
            name = dir_path.name
            if name.isdigit():
                try:
                    val = int(name)
                    new_name = f"{val:06}"
                except ValueError:
                    continue
                    
                if new_name == name:
                    continue
                    
                new_path = CACHE_PREVIEWS_DIR / new_name
                
                if new_path.exists() and new_path != dir_path:
                    print(f"Merging {name} into {new_name}...")
                    # Merge
                    for subfile in dir_path.iterdir():
                        dest = new_path / subfile.name
                        if dest.exists():
                             remove_item(subfile)
                        else:
                            shutil.move(str(subfile), str(dest))
                    try:
                        dir_path.rmdir() # remove old
                        renamed_count += 1
                    except OSError as e:
                        print(f"Warning: Could not remove old dir {dir_path}: {e}")
                elif new_path != dir_path:
                    print(f"Renaming dir {name} -> {new_name}")
                    dir_path.rename(new_path)
                    renamed_count += 1
    print(f"Renamed {renamed_count} directories.")

if __name__ == "__main__":
    normalize_mp4s()
    move_and_normalize_dirs()

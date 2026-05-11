import json
import subprocess
from pathlib import Path

from soromantic_utils.common import get_download_dir

VIDEO_EXTS = {".mp4", ".mkv", ".mov", ".avi", ".webm"}


def probe_video(path: Path):
    cmd = [
        "ffprobe",
        "-v",
        "error",
        "-select_streams",
        "v:0",
        "-show_entries",
        "stream=width,height",
        "-show_entries",
        "format=duration",
        "-of",
        "json",
        str(path),
    ]

    result = subprocess.run(cmd, capture_output=True, text=True, check=True)
    data = json.loads(result.stdout)

    stream = data["streams"][0]
    duration = float(data["format"]["duration"])

    return {
        "file": path.name,
        "width": stream["width"],
        "height": stream["height"],
        "duration": duration,
    }


def scan_dir(dir_path: Path):
    for path in dir_path.iterdir():
        if path.suffix.lower() in VIDEO_EXTS:
            try:
                yield probe_video(path)
            except Exception as e:
                print(f"Failed to probe {path}: {e}")


if __name__ == "__main__":
    download_dir = get_download_dir()
    if not download_dir:
        print("Error: Could not determine download_dir from config")
        sys.exit(1)

    # Assuming the previous path "output/downloads/videos/None" was some debug artifact.
    # We should probably scan the whole videos dir or accept an argument.
    # The original was: Path("output/downloads/videos/None")
    # For now, let's scan the video dir itself if it exists, or let user specify.
    # Let's use the download_dir joined with 'videos' if customary, or just download_dir.
    # Based on config.rs: download_dir is usually "output/downloads".

    target_dir = Path(download_dir) / "videos"
    if not target_dir.exists():
        target_dir = Path(download_dir)  # Fallback to root download dir

    print(f"Scanning {target_dir}...")
    if target_dir.exists():
        for info in scan_dir(target_dir):
            print(info)
    else:
        print(f"Directory {target_dir} does not exist.")


import subprocess
import json
from pathlib import Path

def get_video_duration(path: Path, ffprobe_path: str) -> float:
    """
    Get the duration of a video file in seconds using ffprobe.
    Raises exception if probing fails.
    """
    if not path.exists():
        raise FileNotFoundError(f"Video file not found: {path}")

    cmd = [
        ffprobe_path,
        "-v", "quiet",
        "-print_format", "json",
        "-show_format",
        str(path)
    ]
    
    result = subprocess.run(cmd, capture_output=True, text=True, check=True)
    data = json.loads(result.stdout)
    
    if "format" not in data or "duration" not in data["format"]:
        raise ValueError(f"No duration found in ffprobe output for {path}")
        
    return float(data["format"]["duration"])



def get_video_streams(path: Path) -> list[dict] | None:
    """
    Get video streams info from a video file using ffprobe.
    Returns None if the file doesn't exist or ffprobe fails.
    """
    if not path.exists():
        return None

    try:
        cmd = [
            "ffprobe",
            "-v", "quiet",
            "-print_format", "json",
            "-show_streams",
            "-select_streams", "v",
            str(path)
        ]
        
        result = subprocess.run(cmd, capture_output=True, text=True, check=True)
        data = json.loads(result.stdout)
        
        return data.get("streams", [])
        
    except (subprocess.CalledProcessError, json.JSONDecodeError, ValueError) as e:
        print(f"Error probing streams {path}: {e}")
        return None



def get_video_resolution(path: Path) -> tuple[int, int] | None:
    """
    Get video resolution (width, height) from a video file.
    Returns None if the file doesn't exist or probing fails.
    """
    streams = get_video_streams(path)
    if not streams:
        return None
        
    # Since get_video_streams selects only video streams, we can check the first one
    # or iterate to find one with valid dimensions.
    for stream in streams:
        w = stream.get("width")
        h = stream.get("height")
        if w is not None and h is not None:
            return (int(w), int(h))
            
    return None

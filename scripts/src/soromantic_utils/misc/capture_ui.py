import os
import shutil
import subprocess
import sys
import time


def check_command(cmd):
    if not shutil.which(cmd):
        print(f"Error: '{cmd}' not found. Please install it.")
        sys.exit(1)


def get_project_root():
    return os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def start_application(project_root):
    print("Starting app with 'just run-iced'...")
    return subprocess.Popen(
        ["just", "run-iced"], cwd=project_root, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL
    )


def focus_window():
    if shutil.which("wmctrl"):
        print("Attempting to focus 'Soromantic Iced' window...")
        subprocess.run(["wmctrl", "-a", "Soromantic Iced"], check=False)
        time.sleep(0.5)


def capture_snapshot(output_path):
    # Capture command: -a: Active window, -b: Background, -n: Non-notify, -o: Output
    cmd = ["spectacle", "-a", "-b", "-n", "-o", output_path]
    print(f"Capturing screenshot: {' '.join(cmd)}")
    
    # Run spectacle
    process = subprocess.run(cmd, capture_output=False, check=False)
    
    if os.environ.get("XDG_SESSION_TYPE") == "wayland":
        if shutil.which("wl-copy"):
            print("Wayland detected. Using wl-copy for clipboard...")
            with open(output_path, "rb") as f:
                subprocess.run(["wl-copy", "--type", "image/png"], stdin=f, check=False)
    return process


def copy_to_clipboard(output_path):
    if os.environ.get("XDG_SESSION_TYPE") == "wayland":
        if shutil.which("wl-copy"):
            print("Wayland detected. Using wl-copy for clipboard...")
            with open(output_path, "rb") as f:
                subprocess.run(["wl-copy", "--type", "image/png"], stdin=f, check=False)
            print("Snapshot copied to CLIPBOARD (Wayland).")
        else:
            print("Warning: Wayland detected but 'wl-copy' not found.")
    elif shutil.which("xclip"):
        with open(output_path, "rb") as f:
            subprocess.run(["xclip", "-selection", "clipboard", "-t", "image/png"], stdin=f, check=False)
        print("Snapshot copied to CLIPBOARD (X11/xclip).")


def stop_application(app_proc):
    print("Closing application...")
    if app_proc:
        app_proc.terminate()
        try:
            app_proc.wait(timeout=2)
        except subprocess.TimeoutExpired:
            app_proc.kill()


def run():
    check_command("spectacle")
    project_root = get_project_root()
    app_proc = start_application(project_root)

    try:
        print("Waiting for application to start (5s)...")
        time.sleep(5)
        focus_window()

        output_path = "/tmp/soromantic_snapshot.png"
        result = capture_snapshot(output_path)

        if result.returncode == 0:
            print(f"Success! Snapshot saved to {output_path}")
            copy_to_clipboard(output_path)
        else:
            print("Error capturing screenshot.")

    finally:
        stop_application(app_proc)


if __name__ == "__main__":
    run()

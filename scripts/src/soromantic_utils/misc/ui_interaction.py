import os
import shutil
import signal
import subprocess
import sys
import time


def check_command(cmd):
    if not shutil.which(cmd):
        print(f"Error: '{cmd}' not found. Please install it.")
        sys.exit(1)


def run_ydotool_cmd(args):
    """Run a ydotool command with a timeout."""
    cmd = ["ydotool"] + args
    print(f"Running: {' '.join(cmd)}")
    try:
        subprocess.run(cmd, check=True, timeout=2)
        time.sleep(0.3)
    except subprocess.TimeoutExpired:
        print(f"Warning: ydotool command timed out: {args}")
    except subprocess.CalledProcessError as e:
        print(f"Warning: ydotool command failed: {e}")


def start_app(project_root):
    """Start the application in a new session."""
    print("Starting app with 'just run-iced'...")
    return subprocess.Popen(
        ["just", "run-iced"],
        cwd=project_root,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        preexec_fn=os.setsid,
    )


def focus_window():
    """Attempt to focus the application window."""
    if shutil.which("wmctrl"):
        print("Attempting to focus 'Soromantic Iced' window...")
        try:
            subprocess.run(["wmctrl", "-a", "Soromantic Iced"], timeout=2)
            time.sleep(1)
        except Exception as e:
            print(f"Focus attempt failed: {e}")


def perform_interactions():
    """Execute the sequence of UI interactions."""
    print("Starting UI interaction sequence...")
    # Tab navigation
    for i in range(3):
        print(f"  Tab {i + 1}/3")
        run_ydotool_cmd(["key", "15:1", "15:0"])

    print("  Enter")
    run_ydotool_cmd(["key", "28:1", "28:0"])

    time.sleep(2)  # Wait for animation/action


def capture_screenshot(output_path):
    """Capture a screenshot using spectacle."""
    cmd = ["spectacle", "-a", "-b", "-n", "-o", output_path]
    print(f"Capturing screenshot: {' '.join(cmd)}")
    try:
        result = subprocess.run(cmd, timeout=5)
        if result.returncode == 0:
            print(f"Success! Snapshot saved to {output_path}")
            return True
        else:
            print("Error capturing screenshot.")
            return False
    except subprocess.TimeoutExpired:
        print("Screenshot capture timed out.")
        return False


def handle_clipboard(output_path):
    """Copy the screenshot to clipboard if on Wayland."""
    if os.environ.get("XDG_SESSION_TYPE") == "wayland" and shutil.which("wl-copy"):
        try:
            with open(output_path, "rb") as f:
                subprocess.run(["wl-copy", "--type", "image/png"], stdin=f, timeout=2)
            print("Snapshot copied to CLIPBOARD.")
        except Exception as e:
            print(f"Clipboard copy failed: {e}")


def cleanup_app(app_proc):
    """Terminate the application process group."""
    print("Closing application...")
    if app_proc:
        try:
            os.killpg(os.getpgid(app_proc.pid), signal.SIGTERM)
            try:
                app_proc.wait(timeout=3)
            except subprocess.TimeoutExpired:
                os.killpg(os.getpgid(app_proc.pid), signal.SIGKILL)
        except ProcessLookupError:
            pass  # Already gone


def run():
    check_command("ydotool")
    check_command("spectacle")

    project_root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    app_proc = None

    # Set socket path for ydotool (daemon likely running as root)
    if os.path.exists("/tmp/.ydotool_socket"):
        os.environ["YDOTOOL_SOCKET"] = "/tmp/.ydotool_socket"
        print("Using ydotool socket: /tmp/.ydotool_socket")

    try:
        app_proc = start_app(project_root)
        print("Waiting for application to start (5s)...")
        time.sleep(5)

        focus_window()
        perform_interactions()

        output_path = "/tmp/soromantic_automation.png"
        if capture_screenshot(output_path):
            handle_clipboard(output_path)

    except Exception as e:
        print(f"An error occurred: {e}")

    finally:
        if app_proc:
            cleanup_app(app_proc)


if __name__ == "__main__":
    run()

"""
Visual testing script for Soromantic.
Captures screenshots of Egui and Iced versions for comparison.
"""

import collections.abc
import contextlib
import os
import platform
import shutil
import signal
import subprocess
import time

try:
    import pyautogui
except ImportError:
    pass


def get_platform_app_command(is_iced: bool = False) -> list[str]:
    """Returns the command to run the application based on the platform."""
    # Prioritize 'just' if available on any platform
    if shutil.which("just"):
        return ["just", "run-iced"] if is_iced else ["just", "run"]

    pkg = "soromantic-iced" if is_iced else "soromantic"
    cmd = ["cargo", "run", "-p", pkg]

    if platform.system() == "Windows":
        cmd.append("--release")

    return cmd


def check_dependencies() -> None:
    """Check for system dependencies (screencapture tools) on Linux."""
    if platform.system() != "Linux":
        return

    if not (shutil.which("scrot") or shutil.which("maim") or shutil.which("spectacle")):
        print("Warning: No screenshot tool found. PyAutoGUI screenshot might fail.")


def kill_app(proc: subprocess.Popen[bytes]) -> None:
    """Terminates the application subprocess safely."""
    print("Closing application...")
    if platform.system() == "Windows":
        subprocess.call(["taskkill", "/F", "/T", "/PID", str(proc.pid)])
        return

    try:
        os.killpg(os.getpgid(proc.pid), signal.SIGTERM)
        proc.wait(timeout=3)
    except OSError:
        try:
            os.killpg(os.getpgid(proc.pid), signal.SIGKILL)
        except OSError:
            pass


@contextlib.contextmanager
def start_app(cmd: list[str]) -> collections.abc.Iterator[subprocess.Popen[bytes]]:
    """Starts the application subprocess as a context manager."""
    print(f"Starting app: {' '.join(cmd)}")
    root_dir = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

    proc = subprocess.Popen(
        cmd,
        cwd=root_dir,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        env=os.environ.copy(),
        start_new_session=(platform.system() != "Windows"),
    )

    try:
        yield proc
    finally:
        kill_app(proc)


def try_wmctrl_focus(name_hint: str) -> bool:
    """Attempts to focus window using wmctrl."""
    if not shutil.which("wmctrl"):
        return False

    print(f"Attempting to focus via wmctrl: {name_hint}")
    subprocess.run(["wmctrl", "-a", name_hint], check=False)
    return True


def get_screen_resolution() -> tuple[int, int]:
    """Get screen resolution, trying pyautogui then xrandr."""
    # Try pyautogui if available
    try:
        return pyautogui.size()
    except Exception:  # pylint: disable=broad-exception-caught
        pass

    if shutil.which("xrandr"):
        try:
            output = subprocess.check_output(["xrandr"], text=True)
            for line in output.splitlines():
                if "*" in line:
                    # Found active mode, e.g., "   1920x1080     60.00*+"
                    parts = line.strip().split()
                    if parts and "x" in parts[0]:
                        w, h = parts[0].split("x")
                        return int(w), int(h)
        except (OSError, ValueError, subprocess.SubprocessError):
            pass

    return 1920, 1080


def try_ydotool_focus() -> bool:
    """Attempts to focus via ydotool click."""
    if not shutil.which("ydotool"):
        return False

    width, height = get_screen_resolution()
    cx, cy = width // 2, height // 2

    print(f"Attempting to focus via ydotool click (Center: {cx}x{cy})...")
    try:
        subprocess.run(
            ["ydotool", "mousemove", "-x", str(cx), "-y", str(cy)], stdout=subprocess.DEVNULL, check=False
        )
        subprocess.run(["ydotool", "click", "0xC0"], stdout=subprocess.DEVNULL, check=False)
        return True
    except OSError as e:
        print(f"ydotool focus failed: {e}")
        return False


def ensure_window_focus(name_hint: str) -> None:
    """Orchestrates focus attempts on Linux."""
    if platform.system() == "Windows":
        return

    if try_wmctrl_focus(name_hint):
        time.sleep(1)
        return

    if try_ydotool_focus():
        time.sleep(1)


def capture_with_spectacle(output_path: str) -> bool:
    """Fallback capture using spectacle."""
    if not shutil.which("spectacle"):
        return False

    print("Attempting fallback to spectacle...")
    try:
        subprocess.run(["spectacle", "-a", "-b", "-n", "-o", output_path], check=True)
        return True
    except subprocess.CalledProcessError as ex:
        print(f"Spectacle fallback failed: {ex}")
        return False


def capture_screenshot(output_path: str) -> bool:
    """Captures a screenshot, trying PyAutoGUI then system tools."""
    print(f"Capturing screenshot to {output_path}...")

    # Allow some time for focus to settle
    time.sleep(1)

    try:
        pyautogui.screenshot().save(output_path)
        print("Success!")
        return True
    except Exception as e:  # pylint: disable=broad-exception-caught
        # We catch broad exception because pyautogui can raise diverse errors
        # depending on the platform/failure mode (ImageNotFound, PyAutoGUIException, etc.)
        # and we want to fall back reliably.
        print(f"Error capturing screenshot with PyAutoGUI: {e}")

    if platform.system() == "Linux":
        return capture_with_spectacle(output_path)

    return False


def run_test(is_iced: bool, output_path: str, name_hint: str) -> None:
    """Run a single test case."""
    cmd = get_platform_app_command(is_iced)

    with start_app(cmd):
        print("Waiting 8s for app launch...")
        time.sleep(8)

        ensure_window_focus(name_hint)
        capture_screenshot(output_path)


def main() -> None:
    """Main entry point."""
    check_dependencies()

    # Setup ydotool socket
    if os.path.exists("/tmp/.ydotool_socket"):
        os.environ["YDOTOOL_SOCKET"] = "/tmp/.ydotool_socket"
        print("Using ydotool socket.")

    # Egui
    print("--- Capturing Reference (Egui) ---")
    run_test(is_iced=False, output_path="ref_egui_py.png", name_hint="Soromantic")

    time.sleep(2)

    # Iced
    print("--- Capturing Target (Iced) ---")
    run_test(is_iced=True, output_path="ref_iced_py.png", name_hint="Soromantic Iced")

    print("\nDone. Screenshots saved to pwd.")


if __name__ == "__main__":
    main()

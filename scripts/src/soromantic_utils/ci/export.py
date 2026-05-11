import argparse
import shutil
import subprocess
import sys
from pathlib import Path

from soromantic_utils.common import run_podman


class TemporaryContainer:
    def __init__(self, image: str, name: str) -> None:
        self.image = image
        self.name = name

    def __enter__(self) -> str:
        # Clean up any existing container
        subprocess.run(["podman", "rm", self.name], capture_output=True, check=False)

        print(f"Creating container {self.name} from {self.image}...")
        try:
            run_podman(["create", "--name", self.name, self.image])
        except subprocess.CalledProcessError:
            print(f"Error: Could not create container from {self.image}. Did the build fail?")
            sys.exit(1)
        return self.name

    def __exit__(self, exc_type: object, exc_value: object, traceback: object) -> None:
        subprocess.run(["podman", "rm", self.name], capture_output=True, check=False)


def resolve_profile(target: str, profile_arg: str | None) -> str:
    if profile_arg and profile_arg != "default":
        return profile_arg

    # Defaults
    if target in ["musl", "void-musl", "alpine-musl"]:
        return "release-debug"
    # All others default to release
    return "release"


def export_windows_native(root_dir: Path, dist_dir: Path, profile: str) -> None:
    """
    Export logic for windows-native target (local build).
    """
    dist_dir.mkdir(parents=True, exist_ok=True)

    # Windows native builds locally.
    # If profile is release, it's in target/release
    # If profile is release-debug, it's in target/release-debug

    src_dir = root_dir / "egui" / "target" / profile
    exe_path = src_dir / "soromantic.exe"

    if exe_path.exists():
        dest = dist_dir / "soromantic-windows-native.exe"
        shutil.copy2(exe_path, dest)
        print(f"✓ Exported ({dest})")
    else:
        print(f"Error: No binary found at {exe_path}")
        sys.exit(1)


def export_artifacts(target: str, profile: str) -> None:
    """
    Main export logic.
    profile must be resolved (not 'default').
    """
    # scripts/src/soromantic_utils/ci/export.py -> .../soromantic
    root_dir = Path(__file__).resolve().parents[4]
    dist_dir = root_dir / "dist" / target

    print(f"Exporting artifacts for {target} (profile: {profile})...")

    if target == "windows-native":
        export_windows_native(root_dir, dist_dir, profile)
        return

    # Container export logic
    dist_dir.mkdir(parents=True, exist_ok=True)
    container_name = f"tmp-export-{target}"
    image = f"soromantic:{target}"

    with TemporaryContainer(image, container_name):
        # Determine container source path based on profile
        # Workspace root is /app, so artifacts are in /app/target
        container_target_dir = "/app/target"

        match target:
            case "windows":
                # pylint: disable=line-too-long
                src = (
                    f"{container_name}:{container_target_dir}/x86_64-pc-windows-gnu/{profile}/soromantic.exe"
                )
                run_podman(["cp", src, str(dist_dir)])
                print(f"✓ Exported soromantic.exe to dist/{target}/")

            case "fedora":
                src = f"{container_name}:{container_target_dir}/generate-rpm"
                run_podman(["cp", src, str(dist_dir)])
                print(f"✓ Exported RPMs to dist/{target}/")

            case "glibc":
                # Binary
                src = f"{container_name}:{container_target_dir}/{profile}/soromantic"
                run_podman(["cp", src, str(dist_dir)])
                # Debian packages
                src_deb = f"{container_name}:{container_target_dir}/debian"
                run_podman(["cp", src_deb, str(dist_dir)])

                # Move debs from subfolder to target folder
                debian_subdir = dist_dir / "debian"
                if debian_subdir.exists():
                    for deb in debian_subdir.glob("*.deb"):
                        dest = dist_dir / deb.name
                        if dest.exists():
                            dest.unlink()
                        shutil.move(str(deb), dest)
                    try:
                        debian_subdir.rmdir()
                    except OSError:
                        pass
                print(f"✓ Exported binary and .deb to dist/{target}/")

            case "alpine-musl":
                # Cross-compiled artifact path (restored)
                src = f"{container_name}:{container_target_dir}/x86_64-unknown-linux-musl/{profile}/soromantic"
                run_podman(["cp", src, str(dist_dir)])
                print(f"✓ Exported binary to dist/{target}/")

            case _:
                # Default for musl (void), artix, ubuntu
                src = f"{container_name}:{container_target_dir}/{profile}/soromantic"
                run_podman(["cp", src, str(dist_dir)])
                print(f"✓ Exported binary to dist/{target}/")


def main() -> None:
    parser = argparse.ArgumentParser(description="Export Soromantic artifacts")
    parser.add_argument("target", help="Export target")
    parser.add_argument(
        "profile", nargs="?", default="default", help="Export profile (default: based on target)"
    )

    args = parser.parse_args()
    target: str = args.target
    profile_arg: str | None = args.profile

    # Resolve profile here, as caller's concern
    profile = resolve_profile(target, profile_arg)

    export_artifacts(target, profile)


if __name__ == "__main__":
    main()

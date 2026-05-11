import argparse
import os
from pathlib import Path

from soromantic_utils.ci.export import export_artifacts, resolve_profile
from soromantic_utils.common import run_command


def main() -> None:
    parser = argparse.ArgumentParser(description="Build Soromantic artifacts")
    parser.add_argument("target", nargs="?", default="local", help="Build target (default: local)")
    parser.add_argument(
        "profile", nargs="?", default="default", help="Build profile (default: based on target)"
    )

    args = parser.parse_args()
    target: str = args.target
    profile_arg: str | None = args.profile

    # Handle shortcut: 'just build release' means 'just build local release'
    # If target looks like a profile name, treat it as a profile for local build
    profile_names = {"release", "dev", "release-debug", "debug"}
    if target in profile_names:
        profile_arg = target
        target = "local"

    # scripts/src/soromantic_utils/ci/build.py -> .../soromantic
    root_dir = Path(__file__).resolve().parents[4]

    print(f"Build Target: {target}, Profile: {profile_arg}")

    # Resolve profile using the logic in export.py
    profile = resolve_profile(target, profile_arg)
    print(f"Resolved Profile: {profile}")

    match target:
        case "windows-native":
            # Windows native build
            egui_dir = root_dir / "egui"
            env = os.environ.copy()
            env["CARGO_TARGET_DIR"] = "target"

            # Mapping profile to cargo args
            cargo_args = ["cargo", "build"]
            if profile == "release":
                cargo_args.append("--release")
            else:
                cargo_args.extend(["--profile", profile])

            run_command(cargo_args, cwd=egui_dir, env=env)
            print("✓ Windows native build complete")

            # Auto-export
            export_artifacts(target, profile)

        case "local":
            # Local build
            cargo_args = ["cargo", "build", "--bin", "soromantic"]
            if profile == "release":
                cargo_args.append("--release")
            else:
                cargo_args.extend(["--profile", profile])

            run_command(cargo_args, cwd=root_dir)
            print(f"✓ Local build complete: target/{profile}/soromantic")

            # No formal export for local target usually

        case _:
            # Container build (glibc, musl, windows, fedora, artix, ubuntu)
            cmd = [
                "podman",
                "build",
                "--target",
                target,
                "--build-arg",
                f"PROFILE={profile}",
                "-t",
                f"soromantic:{target}",
                "-f",
                "Containerfile",
                ".",
            ]
            run_command(cmd, cwd=root_dir)
            print(f"✓ Container build complete for {target}")

            # Auto-export
            export_artifacts(target, profile)


if __name__ == "__main__":
    main()

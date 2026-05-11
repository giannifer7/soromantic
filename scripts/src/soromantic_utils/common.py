import os
import sqlite3
import subprocess
import sys
import tomllib
from contextlib import contextmanager
from pathlib import Path
from typing import Any, Generator

from platformdirs import user_config_dir

DEFAULT_CONFIG_FILE = Path(user_config_dir("soromantic")) / "config.toml"


def run_command(
    cmd: list[str],
    cwd: Path | str | None = None,
    env: dict[str, str] | None = None,
    check: bool = True,
) -> None:
    print(f"Running: {' '.join(cmd)}")
    subprocess.run(cmd, cwd=cwd, env=env, check=check)


def run_podman(cmd: list[str]) -> None:
    subprocess.run(["podman"] + cmd, check=True)


def expand_path(path_input: str | Path | None) -> Path | None:
    if not path_input:
        return None
    # os.path.expandvars is still useful for environment variable expansion
    # pathlib doesn't implement it.
    expanded_str = os.path.expandvars(str(path_input))
    return Path(expanded_str).expanduser().resolve()


def load_config() -> dict[str, Any]:
    # Priority: ENV var -> Project config.toml -> User config.toml -> Fail
    local_config = Path(__file__).parents[3] / "config.toml"

    env_config = os.environ.get("SOROMANTIC_CONFIG")
    config_path: Path | None = None

    if env_config:
        config_path = expand_path(env_config)

    if not config_path:
        if local_config.exists():
            config_path = local_config.resolve()
        else:
            config_path = DEFAULT_CONFIG_FILE

    if not config_path or not config_path.exists():
        # Predictable behavior: if I expect a config and it's not there, that's notable.
        # But maybe the user hasn't created it yet.
        if config_path:
            print(f"Note: Config file not found at {config_path}. Using defaults.")
        else:
            print("Note: Config path resolution failed. Using defaults.")
        return {}

    with config_path.open("rb") as f:
        return tomllib.load(f)


def get_db_path(config: dict[str, Any] | None = None) -> Path:
    if config is None:
        config = load_config()

    # Try [runtime] db_path first (based on core/src/config.rs)
    path_val = config.get("runtime", {}).get("db_path")

    if path_val:
        expanded = expand_path(path_val)
        if expanded:
            return expanded

    # Validation
    if not path_val:
        print("Error: 'db_path' not found in [runtime] section of config.")
        sys.exit(1)

    # It should have been a string if it was in config and triggered path_val
    # If expand_path returned None (unlikely if path_val is truthy), we fallback
    print(f"Error: Could not resolve db_path: {path_val}")
    sys.exit(1)


def get_download_dir(config: dict[str, Any] | None = None) -> Path | None:
    if config is None:
        config = load_config()
    path_val = config.get("runtime", {}).get("download_dir")
    if path_val:
        return expand_path(path_val)
    return None


def get_db_connection(config: dict[str, Any] | None = None) -> Any:
    """
    Context manager for database connection.
    Yields a sqlite3 connection.
    """

    db_path = get_db_path(config)

    @contextmanager
    def _connection() -> Generator[sqlite3.Connection, None, None]:
        # sqlite3.connect accepts Path objects in Python 3.7+
        conn = sqlite3.connect(db_path)
        try:
            yield conn
        finally:
            conn.close()

    return _connection()

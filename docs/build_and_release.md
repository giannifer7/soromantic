# Build and Release Guide

We have unified the project's build automation using `just`. This ensures that CI/CD pipelines use the exact same commands as local development.

## Core Concepts

-   **Justfile**: The single source of truth for all build logic.
-   **Containers**: Linux builds use Podman containers defined in `Containerfile`.
-   **Native**: Windows builds (and local native builds) use `nushell` via `just build-native`.
-   **Mise**: manages the development environment (Python, paths) without Nix.

## Recipes

### Building
```bash
# Build for a specific target (glibc, musl, windows, fedora, artix, ubuntu)
# Uses Podman containers
just build glibc

# Build natively on the current Windows host (requires Nushell)
just build windows-native
```

### Exporting Artifacts
```bash
# Extract build artifacts from the container to dist/<target>/
just export glibc

# Move native build artifacts to dist/windows-native/ (requires Nushell)
just export windows-native
```

## CI/CD Workflows

### CI (`.github/workflows/ci.yml`)
Runs on every push/PR.
-   Installs `just` and `podman`.
-   Runs `just build {{target}}` and `just export {{target}}`.
-   Matrix: `glibc`, `musl`, `windows`, `fedora`, `artix`, `ubuntu`.

### Release (`.github/workflows/release.yml`)
Runs on `v*` tags.
-   **Linux**: Same as CI (Podman + Just).
-   **Windows**: Runs on `windows-latest` runner.
    -   Installs `nushell` via `ustrus/setup-nu`.
    -   Runs `just build-native` and `just export-native`.

## Environment
-   `mise.toml`: Configures Python virtualenv and PATH.
-   `config.fish`: Auto-loads local completions from `completions/`.

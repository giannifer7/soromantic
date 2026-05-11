# Linux Development Setup

This guide details how to set up the development environment for Soromantic on Linux (Arch and Debian/Ubuntu).

## Dependencies

We use **[Mise](https://mise.jdx.dev/)** to manage our development environment (Rust, Python, Node, etc.) ensuring a reproducible setup without polluting your global system.

### Arch Linux
Install the base development tools and `just`:
```bash
paru -S mise just podman nushell ffmpeg mpv base-devel uv mold sccache
```

Then enable sccache by uncommenting this section in `.cargo/config.toml`:
```toml
[build]
rustc-wrapper = "sccache"
```


### Void Linux
```bash
# Install dependencies
sudo xbps-install -S base-devel podman nushell ffmpeg mpv curl just uv mold

# Install Mise (not in official repos)
curl https://mise.run | sh
echo 'eval "$(~/.local/bin/mise activate bash)"' >> ~/.bashrc
source ~/.bashrc

# Install sccache
cargo install sccache
```

### Debian / Ubuntu
```bash
# Install dependencies
sudo apt update
sudo apt install build-essential curl podman ffmpeg mpv mold

# Install Just
curl --proto '=https' --tlsv1.2 -sSf https://just.systems/install.sh | bash -s -- --to /usr/local/bin

# Install uv (Python package manager)
curl -LsSf https://astral.sh/uv/install.sh | sh

# Install Nushell (required for justfile recipes)
# Option 1: Via Cargo (if you have Rust already)
# cargo install nu
# Option 2: Download binary (recommended for speed)
curl -L https://github.com/nushell/nushell/releases/download/0.90.1/nu-0.90.1-x86_64-unknown-linux-gnu.tar.gz | tar -xz
sudo mv nu-*-x86_64-unknown-linux-gnu/nu /usr/local/bin/
rm -rf nu-*-x86_64-unknown-linux-gnu*

# Install Mise
curl https://mise.run | sh
echo 'eval "$(~/.local/bin/mise activate bash)"' >> ~/.bashrc
source ~/.bashrc

# Install sccache
cargo install sccache
```

## Setup Environment

Enter the project directory and `mise` will automatically set up the environment (or prompt you to install tools):
```bash
mise install
```

### Python Development
If you need to work on the Python scripts (in `scripts/`), navigate to that directory and use `uv` to sync dependencies (including `ruff`, `mypy`, `pylint`):
```bash
cd scripts
uv sync
```

Before running any build commands, enable sccache by uncommenting this section in `.cargo/config.toml`:
```toml
[build]
rustc-wrapper = "sccache"
```

## Development Commands

-   **Run Dev**: `just run`
-   **Build Release**: `just build`
-   **Package for AUR**: `just aur`
-   **Check Code**: `just check`

# Soromantic

A romantic video library manager (desktop native).

## Installation

Downloads are available on the [GitHub Releases page](https://github.com/giannifer7/soromantic/releases).

### Prerequisites
Soromantic requires `mpv` and `ffmpeg` to be installed on your system.

**Arch Linux**
```bash
sudo pacman -S mpv ffmpeg
```

**Debian / Ubuntu**
```bash
sudo apt install mpv ffmpeg
```

**Windows**
The **Installer Script** (see below) will automatically handle these dependencies for you.
Manual installation: [Chocolatey](https://chocolatey.org/) is recommended (`choco install mpv ffmpeg`).

### Arch Linux

**Option 1: Pre-built Binary (.pkg.tar.zst)**
Download the file ending in `.pkg.tar.zst` (e.g., `soromantic-0.1.0-x86_64.pkg.tar.zst`) and install it:
```bash
sudo pacman -U soromantic-*.pkg.tar.zst
```
*   **Pros**: Fastest install.
*   **Cons**: Depends on system library versions matching the build environment.

**Option 2: Build from Source (PKGBUILD)**
Download the source tarball (e.g., `soromantic-0.1.0-x86_64.tar.gz`) which contains the `PKGBUILD`, extract it, and build:
```bash
tar -xvf soromantic-*.tar.gz
cd soromantic-0.1.0-x86_64
makepkg -si
```
*   **Pros**: Compiles against your exact system libraries (maximum compatibility).
*   **Cons**: Takes longer to install (compile time).

### Debian / Ubuntu
Download the `.deb` package from the releases page and install it:
```bash
sudo dpkg -i soromantic_x.x.x_amd64.deb
```

### Windows
**Automatic Install (Recommended):**
Open PowerShell and run:
```powershell
irm https://raw.githubusercontent.com/giannifer7/soromantic/main/windows/install.ps1 | iex
```
This will:
1.  Install Chocolatey (if missing).
2.  Install `mpv` and `ffmpeg` dependencies.
3.  Download and install Soromantic to `%LOCALAPPDATA%`.
4.  Create a Start Menu shortcut.

**Manual Install:**
Download `soromantic-windows-x86_64.zip` from the releases page, extract it, and run `soromantic.exe`. (Requires `mpv` and `ffmpeg` in PATH).

## Configuration

Soromantic stores its configuration in `config.toml`, located in standard platform-specific directories:

- **Linux**: `~/.config/soromantic/config.toml`
- **Windows**: `%LOCALAPPDATA%\soromantic\config.toml` (e.g., `C:\Users\Name\AppData\Local\soromantic\config.toml`)

**Automatic Initialization**:
On the first run, if no configuration file is found, Soromantic will **automatically create** one with default settings and comments explaining each option.

To customize your experience (e.g., change download paths, timeouts, or UI preferences), simply edit this file after running the application once.

## Development Setup

If you want to build from source or contribute:

### Dependencies

We use **[Mise](https://mise.jdx.dev/)** to manage our development environment (Rust, Python, Node, etc.).

Please see the detailed setup guides for your platform:

-   **[Linux Setup Guide](docs/linux_setup.md)** (Arch, Ubuntu)
-   **[Windows Setup Guide](docs/windows_setup.md)** (Chocolatey, VS Build Tools, Developer Shell)

### Quick Start (Once Setup)

1.  Enter the project directory and install tools:
    ```bash
    mise install
    ```

### Building & Verification

For detailed instructions on building, testing, and the release process (including Podman containers and native builds), please see **[build_and_release.md](docs/build_and_release.md)**.

#### Native Builds (Windows CI/Local)
Build natively on your Windows machine (requires `nushell`):
```bash
# Builds using your local rust toolchain
just build windows-native
just export windows-native
```

### Windows Sandbox Verification
To test the Windows build in a completely clean environment:
1.  Build the release (`just build windows`).
2.  Run `windows/verification.wsb` (Requires Windows Sandbox enabled).
    - Checks for missing DLLs and validates the installer logic.

<!-- antigravity-dummy-edit: testing background command stability -->

# Windows Development Setup

This guide details how to set up a robust development environment for Soromantic on Windows.

## 1. Install Core Dependencies
We recommend using **Chocolatey** to install the base tools:
```powershell
choco install mise just podman mpv ffmpeg cmake llvm strawberryperl uutils-coreutils
```

Also install `uv` (Python package manager):
```powershell
winget install --id astral-sh.uv
```
*   **mise**: Manages language runtimes (Rust, Python, Node).
*   **just**: Task runner for build commands.
*   **podman**: Container engine for cross-compilation.
*   **mpv/ffmpeg**: Runtime dependencies for video playback.
*   **cmake/llvm/strawberryperl**: Build dependencies required by `libsql` and other C-binding crates.
*   **uutils-coreutils**: Provides unix-like commands (like `cp`) required by some build scripts.

## 2. Install Nushell (via Winget)
Nushell is required for the native build scripts (`just build-native`).
```powershell
# Install to user scope (recommended)
winget install nushell

# OR Machine scope installation (Run as admin)
winget install nushell --scope machine
```

## 3. Install Visual Studio Build Tools
Soromantic depends on local C++ compilers (MSVC) for linking.

1.  Download **[Visual Studio 2022 Build Tools](https://visualstudio.microsoft.com/downloads/)**.
2.  Run the installer.
3.  Select the **"Desktop development with C++"** workload.
4.  In the details pane, ensure the following are checked:
    *   MSVC v143 - VS 2022 C++ x64/x86 build tools
    *   Windows 11 SDK (or Windows 10 SDK)

## 4. Developer Environment (Crucial)

> [!IMPORTANT]
> You **MUST** run all build commands (`cargo build`, `just run`) inside the **"Developer PowerShell for VS 2022"**.
>
> Standard PowerShell windows often lack the C compiler (`cl.exe`) and linker (`link.exe`) required for dependencies like `libsql-ffi`.

### Automating the Developer Shell (Windows Terminal)
To avoid manually opening "Developer PowerShell" every time, add this profile to your **Windows Terminal** settings (open `settings.json`):

```json
{
    "name": "Dev Nushell",
    "commandline": "powershell.exe -ExecutionPolicy Bypass -NoExit -Command \"&{Import-Module 'C:\\\\Program Files\\\\Microsoft Visual Studio\\\\2022\\\\Community\\\\Common7\\\\Tools\\\\Microsoft.VisualStudio.DevShell.dll'; Enter-VsDevShell -VsInstallPath 'C:\\\\Program Files\\\\Microsoft Visual Studio\\\\2022\\\\Community' -SkipAutomaticLocation -DevCmdArguments '-arch=x64 -host_arch=x64'}; nu\"",
    "icon": "ms-appx:///ProfileIcons/{0caa0dad-35be-5f56-a8ff-afceeeaa6101}.png",
    "hidden": false
}
```
*Note: Adjust the path if you use VS Professional (`Enterprise` or `Professional` instead of `Community`).*

## 5. Final Setup
Enter the project directory and run:
```bash
mise install
```
This will configure the Rust toolchain and Python environment locally.

### Python Development
If you need to work on the Python scripts (in `scripts/`), navigate to that directory and use `uv` to sync dependencies (including `ruff`, `mypy`, `pylint`):
```bash
cd scripts
uv sync
```

## 6. Enable sccache (Build Caching)

Uncomment the sccache section in `.cargo/config.toml`:
```toml
[build]
rustc-wrapper = "sccache"
```

Then install sccache:
```powershell
cargo install sccache
```

**Tip:** For faster development builds, use `just build-release-debug` (~1 min) instead of `cargo build --release` (~3-5 min).

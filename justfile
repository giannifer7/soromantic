# justfile for soromantic

# Default: List commands
default:
    @just --list
    
# --- Setup & Install ---

# Setup Rust environment
setup:
    cargo build
    @echo "✓ Setup complete! Run 'just dev' or 'just run' to start."

# Update dependencies
update:
    cargo update

# --- Development ---

# Start app in dev mode
run:
    RUST_LOG="info,eframe=debug,winit=debug,glutin=debug" cargo run --bin soromantic

# Start app in release mode (optimized)
run-release:
    RUST_LOG=info cargo run --bin soromantic --release

# Start app with release-debug profile (fast build, optimized, with symbols)
run-release-debug:
    RUST_LOG=info cargo run --bin soromantic --profile release-debug

# --- soromantic-fltk (FLTK version) ---

# Start FLTK app with optional profile (default: dev)
# Usage: just run-fltk [profile]
# Examples: just run-fltk
#           just run-fltk release
#           just run-fltk release-debug
run-fltk PROFILE='dev':
    FLTK_BACKEND=wayland RUST_LOG=info cargo run -p soromantic-fltk {{ if PROFILE == "dev" { "" } else if PROFILE == "release" { "--release" } else { "--profile " + PROFILE } }}

# Build FLTK app with optional profile (default: dev)
# Usage: just build-fltk [profile]
build-fltk PROFILE='dev':
    cargo build -p soromantic-fltk {{ if PROFILE == "dev" { "" } else if PROFILE == "release" { "--release" } else { "--profile " + PROFILE } }}

# --- OCaml SDL ---

# Build OCaml SDL app
build-ocaml:
    cargo build --release -p soromantic-core
    cd ocaml-sdl && dune build bin/main.exe

# Run OCaml SDL app
run-ocaml:
    cargo build --release -p soromantic-core
    cd ocaml-sdl && dune exec bin/main.exe

# --- Raycaml (Raylib version) ---

# Build Raycaml app
build-raycaml:
    cargo build --release -p soromantic-core
    cd raycaml && dune build bin/main.exe

# Run Raycaml app
run-raycaml:
    cargo build --release -p soromantic-core
    cd raycaml && dune exec bin/main.exe

# --- Raycaml Pure (OCaml 5 / Eio version) ---

# Build Raycaml Pure app
build-raycaml-pure:
    cd raycaml-pure && dune build bin/main.exe

# Run Raycaml Pure app
run-raycaml-pure:
    cd raycaml-pure && dune exec bin/main.exe

# --- Development ---

# Watch for changes and auto-restart (hot reload)
watch:
    RUST_LOG=info cargo watch -c -w egui/src -w core/src -x 'run --bin soromantic'



# Build AUR package (requires cargo-aur)
aur:
    cd egui && CARGO_TARGET_DIR=target cargo aur

aur-release: aur

# Build Deb package (requires cargo-deb)
deb:
    cd egui && CARGO_TARGET_DIR=target cargo deb


# --- Thumbnail Downloader ---

# Run the thumbnail downloader tool
thumb-dl:
    RUST_LOG=info cargo run -p thumb-dl

# --- Quality & Testing ---

# Run Rust checks (clippy)
check:
    cargo clippy --workspace -- -D warnings
    cargo fmt --check

# Auto-fix code (format)
fix:
    cargo fmt

# Run tests
test:
    cargo test --workspace

# Find code duplicates
duplicates TARGET='.':
    npx jscpd {{TARGET}}

# Build project.
# Usage: just build (defaults to local cargo build)
#        just build <target>
# Possible targets: glibc, void-musl, alpine-musl, windows, fedora, artix, ubuntu, windows-native
build TARGET='local' PROFILE='default':
    uv run --project scripts python -m soromantic_utils.ci.build {{TARGET}} {{PROFILE}}

# Build and export artifacts for a TARGET
# Usage: just export <target>
export TARGET PROFILE='default': (build TARGET PROFILE)
    uv run --project scripts python -m soromantic_utils.ci.export {{TARGET}} {{PROFILE}}

# Run the exported binary for a TARGET
# Usage: just try <target>
try TARGET:
    ./dist/{{TARGET}}/soromantic


# Run a migration script by name/module
# Usage: just migration add_start_stop
migration NAME:
    uv run --project scripts python -m soromantic_utils.migrations.{{NAME}}

# Run a maintenance script by name/module
# Usage: just maintenance xxx
maintenance NAME *ARGS:
    uv run --project scripts python -m soromantic_utils.maintenance.{{NAME}} {{ARGS}}

# Clean artifacts
clean:
    cargo clean
    git clean -fdX
    @echo "✓ Cleaned all build artifacts"

# Git sync (commit & push)
save MESSAGE: fix
    git add .
    git commit -m "{{MESSAGE}}"
    git push

# Tag a version and push to trigger release workflow
release VERSION:
    git tag {{VERSION}}
    git push origin {{VERSION}}

# Re-trigger release for a specific tag (moves the tag to current HEAD)
re-release VERSION:
    @echo "Re-releasing {{VERSION}}..."
    -git push --delete origin {{VERSION}}
    -git tag -d {{VERSION}}
    git tag {{VERSION}}
    git push origin {{VERSION}}


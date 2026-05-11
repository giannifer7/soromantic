# justfile for soromantic — romantic video library manager

default:
    @just --list

# ── Development ──

setup:
    cargo build
    @echo "✓ Setup complete. Run 'just run' to start."

update:
    cargo update

run:
    RUST_LOG="info,eframe=debug,winit=debug,glutin=debug" cargo run --bin soromantic

run-release:
    RUST_LOG=info cargo run --bin soromantic --release

run-release-debug:
    RUST_LOG=info cargo run --bin soromantic --profile release-debug

watch:
    RUST_LOG=info cargo watch -c -w egui/src -w core/src -x 'run --bin soromantic'

# ── Packages ──

aur:
    cd egui && CARGO_TARGET_DIR=target cargo aur

deb:
    cd egui && CARGO_TARGET_DIR=target cargo deb

# ── Thumbnail Downloader ──

thumb-dl:
    RUST_LOG=info cargo run -p thumb-dl

# ── Quality & Testing ──

check:
    cargo clippy --workspace -- -D warnings
    cargo fmt --check

fix:
    cargo fmt

test:
    cargo test --workspace

# ── CI / Build ──

build TARGET='local' PROFILE='default':
    uv run --project scripts python -m soromantic_utils.ci.build {{TARGET}} {{PROFILE}}

export TARGET PROFILE='default': (build TARGET PROFILE)
    uv run --project scripts python -m soromantic_utils.ci.export {{TARGET}} {{PROFILE}}

try TARGET:
    ./dist/{{TARGET}}/soromantic

# ── Maintenance ──

migration NAME:
    uv run --project scripts python -m soromantic_utils.migrations.{{NAME}}

maintenance NAME *ARGS:
    uv run --project scripts python -m soromantic_utils.maintenance.{{NAME}} {{ARGS}}

# ── Git ──

save MESSAGE: fix
    git add .
    git commit -m "{{MESSAGE}}"
    git push

release VERSION:
    git tag {{VERSION}}
    git push origin {{VERSION}}

re-release VERSION:
    @echo "Re-releasing {{VERSION}}..."
    -git push --delete origin {{VERSION}}
    -git tag -d {{VERSION}}
    git tag {{VERSION}}
    git push origin {{VERSION}}

clean:
    cargo clean
    git clean -fdX
    @echo "✓ Cleaned all build artifacts"

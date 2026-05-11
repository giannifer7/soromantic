# syntax=docker/dockerfile:1

##############################
# Base toolchain image
##############################
FROM ghcr.io/rust-lang/rust:nightly-slim AS base

# Workaround for "Invalid cross-device link" when rustup updates in Docker
ENV RUSTUP_PERMIT_COPY_RENAME=true

RUN apt-get update && apt-get install -y \
    build-essential \
    pkg-config \
    curl \
    libxcb-render0-dev \
    libxcb-shape0-dev \
    libxcb-xfixes0-dev \
    libxkbcommon-dev \
    libwayland-dev \
    fontconfig \
    ffmpeg \
    mpv \
    fonts-dejavu \
 && rm -rf /var/lib/apt/lists/*

RUN cargo install cargo-chef cargo-deb

WORKDIR /app

##############################
# Cargo dependency planning
##############################
FROM base AS planner

COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY core/Cargo.toml core/Cargo.toml
COPY egui/Cargo.toml egui/Cargo.toml

# Create dummy source files for cargo chef metadata calculation
RUN mkdir -p core/src egui/src && \
    touch core/src/lib.rs && \
    touch egui/src/lib.rs egui/src/main.rs

RUN cargo chef prepare --recipe-path recipe.json

##############################
# Dependency cache
##############################
FROM base AS cacher

COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

# Stage 3: Linux (Glibc) - Debian based, produces binary + .deb
FROM base AS glibc

COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY core/ core/
COPY egui/ egui/
COPY config.toml.example config.toml.example

COPY --from=cacher /app/target /app/target
COPY --from=cacher /usr/local/cargo /usr/local/cargo

WORKDIR /app/egui
ENV CARGO_TARGET_DIR=/app/target

ARG PROFILE=release
RUN cargo build --profile ${PROFILE}
RUN cargo deb

# Stage 4: Linux (Void Musl) - Void based, produces binary
FROM ghcr.io/void-linux/void-musl:latest AS void-musl

RUN xbps-install -Sy \
 && xbps-install -y \
    base-devel \
    rust \
    cargo \
    wayland-devel \
    libxkbcommon-devel \
    libxcb-devel \
    fontconfig-devel \
    pkg-config \
    mpv \
    ffmpeg \
    dejavu-fonts-ttf \
    mesa-dri

WORKDIR /app

# Improve backtraces for musl
ENV RUSTFLAGS="-C force-frame-pointers=yes"

COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY core/ core/
COPY egui/ egui/
COPY config.toml.example config.toml.example

WORKDIR /app/egui
ARG PROFILE=release
RUN cargo build --profile ${PROFILE}

# Stage 4b: Linux (Alpine Musl) - Cross-compile from Debian for stability
FROM base AS alpine-musl

RUN apt-get update && apt-get install -y \
    musl-tools \
    musl-dev \
 && rm -rf /var/lib/apt/lists/*

RUN rustup target add x86_64-unknown-linux-musl

WORKDIR /app

# Configure target-specific flags to enforce static linking ONLY for the target
# This avoids breaking proc-macros (which run on the glibc host)
ENV CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_RUSTFLAGS="-C target-feature=+crt-static -C linker=musl-gcc -C force-frame-pointers=yes"

COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY core/ core/
COPY egui/ egui/
COPY config.toml.example config.toml.example

WORKDIR /app/egui
ARG PROFILE=release
RUN cargo build --profile ${PROFILE} --target x86_64-unknown-linux-musl

# Stage 5: Windows (MinGW) - produces .exe
FROM base AS windows

RUN apt-get update && apt-get install -y \
    mingw-w64 \
    wine \
 && rm -rf /var/lib/apt/lists/*

RUN rustup target add x86_64-pc-windows-gnu

WORKDIR /app

COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY core/ core/
COPY egui/ egui/
COPY config.toml.example config.toml.example

RUN mkdir -p .cargo && \
    printf '[target.x86_64-pc-windows-gnu]\nlinker = "x86_64-w64-mingw32-gcc"\nar = "x86_64-w64-mingw32-gcc-ar"\n' \
      > .cargo/config.toml

WORKDIR /app/egui
ARG PROFILE=release
RUN cargo build --profile ${PROFILE} --target x86_64-pc-windows-gnu

# Stage 6: Fedora (RPM)
FROM fedora:40 AS fedora

RUN dnf install -y \
    gcc \
    rust \
    cargo \
    openssl-devel \
    libxcb-devel \
    libxkbcommon-devel \
    wayland-devel \
    mesa-libGL-devel \
    fontconfig-devel \
    mpv \
    ffmpeg \
    dejavu-fonts-all \
    rpm-build \
 && dnf clean all

RUN cargo install cargo-generate-rpm

WORKDIR /app

COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY core/ core/
COPY egui/ egui/
COPY config.toml.example config.toml.example

WORKDIR /app/egui
ARG PROFILE=release
RUN cargo build --profile ${PROFILE}
RUN cargo generate-rpm

# Stage 7: Artix Linux (Runital) - Verification
FROM artixlinux/artixlinux AS artix

RUN pacman -Sy --noconfirm \
    base-devel \
    rust \
    wayland \
    libxkbcommon \
    libxcb \
    fontconfig \
    pkgconf \
    mpv \
    ffmpeg \
    ttf-dejavu \
    mesa

WORKDIR /app

COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY core/ core/
COPY egui/ egui/
COPY config.toml.example config.toml.example

WORKDIR /app/egui
ARG PROFILE=release
RUN cargo build --profile ${PROFILE}

# Stage 8: Ubuntu (Rolling) - Verification
FROM ubuntu:rolling AS ubuntu
ENV DEBIAN_FRONTEND=noninteractive

RUN apt-get update && apt-get install -y \
    build-essential \
    curl \
    libxcb-render0-dev \
    libxcb-shape0-dev \
    libxcb-xfixes0-dev \
    libxkbcommon-dev \
    libwayland-dev \
    libssl-dev \
    pkg-config \
    mpv \
    ffmpeg \
    fonts-dejavu \
 && rm -rf /var/lib/apt/lists/*

# Install Rust
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

WORKDIR /app

COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY core/ core/
COPY egui/ egui/
COPY config.toml.example config.toml.example

WORKDIR /app/egui
ARG PROFILE=release
RUN cargo build --profile ${PROFILE}

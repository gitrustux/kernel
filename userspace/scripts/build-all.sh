#!/bin/bash
# Copyright 2025 The Rustux Authors
#
# Use of this source code is governed by a MIT-style
# license that can be found in the LICENSE file or at
# https://opensource.org/licenses/MIT

set -e

# Build script for Rustux userspace SDK

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
USERSPACE_DIR="$(dirname "$SCRIPT_DIR")"
PROJECT_ROOT="$(dirname "$USERSPACE_DIR")"

# Architecture to build for
ARCH="${ARCH:-x86_64}"
TARGET="${ARCH}-unknown-none-gnu"

echo "Building Rustux userspace for $ARCH..."

# Set Rust target
case "$ARCH" in
    x86_64)
        TARGET="x86_64-unknown-none-gnu"
        RUST_TARGET="x86_64-unknown-linux-gnu"
        ;;
    aarch64)
        TARGET="aarch64-unknown-none-gnu"
        RUST_TARGET="aarch64-unknown-linux-gnu"
        ;;
    riscv64)
        TARGET="riscv64-unknown-none-gnu"
        RUST_TARGET="riscv64-unknown-linux-gnu"
        ;;
    *)
        echo "Unknown architecture: $ARCH"
        exit 1
        ;;
esac

echo "Target: $TARGET"

# Build libsys
echo "Building libsys..."
cd "$USERSPACE_DIR/libsys"
cargo build --release --target "$RUST_TARGET" || cargo build --release

# Build libipc
echo "Building libipc..."
cd "$USERSPACE_DIR/libipc"
cargo build --release --target "$RUST_TARGET" || cargo build --release

# Build librt
echo "Building librt..."
cd "$USERSPACE_DIR/librt"
cargo build --release --target "$RUST_TARGET" || cargo build --release

# Build libc-rx
echo "Building libc-rx..."
cd "$USERSPACE_DIR/libc-rx"
cargo build --release --target "$RUST_TARGET" || cargo build --release

# Build crt
echo "Building crt..."
cd "$USERSPACE_DIR/crt"
cargo build --release --target "$RUST_TARGET" || cargo build --release

# Build test programs
echo "Building test programs..."
cd "$USERSPACE_DIR/tests/hello"
cargo build --release --target "$RUST_TARGET" || cargo build --release

echo "Build complete!"

# Create rootfs
ROOTFS_DIR="$USERSPACE_DIR/rootfs-$ARCH"
echo "Creating rootfs at $ROOTFS_DIR..."
mkdir -p "$ROOTFS_DIR/bin"

# Copy libraries
cp "$USERSPACE_DIR/libsys/target/release/libsys.a" "$ROOTFS_DIR/lib/"
cp "$USERSPACE_DIR/libipc/target/release/libipc.a" "$ROOTFS_DIR/lib/"
cp "$USERSPACE_DIR/librt/target/release/librt.a" "$ROOTFS_DIR/lib/"
cp "$USERSPACE_DIR/libc-rx/target/release/libc.a" "$ROOTFS_DIR/lib/"
cp "$USERSPACE_DIR/crt/target/release/libcrt0.a" "$ROOTFS_DIR/lib/"

# Copy test programs
cp "$USERSPACE_DIR/tests/hello/target/release/hello" "$ROOTFS_DIR/bin/"

echo "Rootfs created at $ROOTFS_DIR"
echo "Done!"

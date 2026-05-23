#!/usr/bin/env bash
set -eu

# Build shpool from source for the current platform and store it in the sshr
# bin directory. Run this on each target platform to populate the binary cache.
#
# On Linux, builds a statically-linked musl binary for maximum portability.
# Requires: cargo (and musl-tools on Linux for static linking)

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
BIN_DIR="$SCRIPT_DIR/bin"
mkdir -p "$BIN_DIR"

OS="$(uname -s | tr '[:upper:]' '[:lower:]')"
ARCH="$(uname -m)"

# Normalize arch names
case "$ARCH" in
    amd64) ARCH="x86_64" ;;
    arm64) ARCH="aarch64" ;;
esac

TARGET="shpool-${OS}-${ARCH}"

if ! command -v cargo >/dev/null 2>&1; then
    echo "Error: cargo is required to build shpool" >&2
    exit 1
fi

TMPDIR="$(mktemp -d)"
trap 'rm -rf "$TMPDIR"' EXIT

echo "Cloning shpool..."
git clone --depth 1 https://github.com/shell-pool/shpool.git "$TMPDIR/shpool"
cd "$TMPDIR/shpool"

if [ "$OS" = "linux" ]; then
    # Build statically with musl for portability across Linux distros
    MUSL_TARGET="${ARCH}-unknown-linux-musl"
    echo "Building shpool for ${MUSL_TARGET} (static)..."
    rustup target add "$MUSL_TARGET" 2>/dev/null || true
    cargo build --release --target "$MUSL_TARGET"
    cp "target/$MUSL_TARGET/release/shpool" "$BIN_DIR/$TARGET"
else
    echo "Building shpool for ${OS}-${ARCH}..."
    cargo build --release
    cp target/release/shpool "$BIN_DIR/$TARGET"
fi

chmod +x "$BIN_DIR/$TARGET"
echo "Built: $BIN_DIR/$TARGET"
ls -lh "$BIN_DIR/$TARGET"
file "$BIN_DIR/$TARGET"

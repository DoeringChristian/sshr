shpool_repo := "https://github.com/shell-pool/shpool.git"
shpool_dir := "shpool/bin"

# Build sshr
build:
    cargo build --release

# Build shpool for all supported targets and place in shpool/bin/
shpool-all: (shpool "x86_64-unknown-linux-musl" "linux-x86_64") (shpool "aarch64-unknown-linux-musl" "linux-aarch64") (shpool-native "darwin-aarch64")

# Build shpool for a cross-compilation target (requires cargo-zigbuild and zig)
shpool rust_target name:
    #!/usr/bin/env bash
    set -eu
    out="{{shpool_dir}}/shpool-{{name}}"
    if [ -f "$out" ]; then
        echo "shpool-{{name}} already exists, skipping (use 'just shpool-force' to rebuild all)"
        exit 0
    fi
    tmpdir=$(mktemp -d)
    trap 'rm -rf "$tmpdir"' EXIT
    echo "Building shpool for {{name}} ({{rust_target}})..."
    git clone --depth 1 {{shpool_repo}} "$tmpdir/shpool"
    cd "$tmpdir/shpool"
    rustup target add {{rust_target}} 2>/dev/null || true
    cargo zigbuild --release --target {{rust_target}}
    mkdir -p "{{justfile_directory()}}/{{shpool_dir}}"
    cp "target/{{rust_target}}/release/shpool" "{{justfile_directory()}}/$out"
    chmod +x "{{justfile_directory()}}/$out"
    echo "Built: $out"

# Build shpool natively (for current platform, e.g. macOS)
shpool-native name:
    #!/usr/bin/env bash
    set -eu
    out="{{shpool_dir}}/shpool-{{name}}"
    if [ -f "$out" ]; then
        echo "shpool-{{name}} already exists, skipping (use 'just shpool-force' to rebuild all)"
        exit 0
    fi
    tmpdir=$(mktemp -d)
    trap 'rm -rf "$tmpdir"' EXIT
    echo "Building shpool for {{name}} (native)..."
    git clone --depth 1 {{shpool_repo}} "$tmpdir/shpool"
    cd "$tmpdir/shpool"
    cargo build --release
    mkdir -p "{{justfile_directory()}}/{{shpool_dir}}"
    cp "target/release/shpool" "{{justfile_directory()}}/$out"
    chmod +x "{{justfile_directory()}}/$out"
    echo "Built: $out"

# Force rebuild all shpool binaries
shpool-force:
    rm -f {{shpool_dir}}/shpool-*
    just shpool-all

# Clean all build artifacts
clean:
    cargo clean
    rm -f {{shpool_dir}}/shpool-*

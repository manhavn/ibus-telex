#!/usr/bin/env bash
# Build the engine: ./build.sh [--debug]
#
# The release binary ends up in target/release/ibus-telex; install it with
# ./install.sh.
set -euo pipefail

ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
PROFILE=release

for arg in "$@"; do
    case "$arg" in
        --debug) PROFILE=debug ;;
        -h | --help)
            sed -n '2,6p' "$ROOT/build.sh" | sed 's/^# \{0,1\}//'
            exit 0
            ;;
        *)
            echo "build.sh: unknown option $arg" >&2
            exit 2
            ;;
    esac
done

command -v cargo >/dev/null || {
    echo "build.sh: cargo not found - install Rust from https://rustup.rs" >&2
    exit 1
}

if [ "$PROFILE" = release ]; then
    echo "==> cargo build --release"
    cargo build --release --manifest-path "$ROOT/Cargo.toml"
    BIN="$ROOT/target/release/ibus-telex"
else
    echo "==> cargo build"
    cargo build --manifest-path "$ROOT/Cargo.toml"
    BIN="$ROOT/target/debug/ibus-telex"
fi

echo
echo "built: $BIN"
echo "next : ./install.sh"

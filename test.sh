#!/usr/bin/env bash
# Check the source: formatting, lints and the test suite.
#
#   ./test.sh            everything
#   ./test.sh --quick    tests only
#
# The protocol test starts the engine on the session bus and needs a running
# IBus session; it skips itself when there is none.
set -euo pipefail

ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
QUICK=0

for arg in "$@"; do
    case "$arg" in
        --quick) QUICK=1 ;;
        -h | --help)
            sed -n '2,8p' "$ROOT/test.sh" | sed 's/^# \{0,1\}//'
            exit 0
            ;;
        *)
            echo "test.sh: unknown option $arg" >&2
            exit 2
            ;;
    esac
done

command -v cargo >/dev/null || {
    echo "test.sh: cargo not found - install Rust from https://rustup.rs" >&2
    exit 1
}

if [ "$QUICK" -eq 0 ]; then
    echo "==> cargo fmt --check"
    cargo fmt --manifest-path "$ROOT/Cargo.toml" -- --check
    echo "==> cargo clippy --all-targets"
    cargo clippy --all-targets --manifest-path "$ROOT/Cargo.toml"
fi

echo "==> cargo test"
cargo test --manifest-path "$ROOT/Cargo.toml"

echo
echo "all good"

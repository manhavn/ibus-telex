#!/usr/bin/env bash
# Build the engine.
#
#   ./build.sh                  the host architecture, dynamically linked
#   ./build.sh --debug          same, without optimisation
#   ./build.sh --amd64          release build for amd64  (static)
#   ./build.sh --arm64          release build for arm64  (static)
#   ./build.sh --all-arch       both of them
#   ./build.sh --target aarch64-unknown-linux-musl
#
# amd64/arm64 are the artifacts a release is made of: musl targets, statically
# linked, so they run on any Linux of that architecture whatever its libc is -
# which is also what makes the arm64 build possible on an amd64 machine, since
# no cross C toolchain is involved (see .cargo/config.toml).
set -euo pipefail

ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
PROFILE=release
TARGETS=()
AMD64=x86_64-unknown-linux-musl
ARM64=aarch64-unknown-linux-musl

usage() {
    sed -n '2,12p' "$ROOT/build.sh" | sed 's/^# \{0,1\}//'
}

while [ $# -gt 0 ]; do
    case "$1" in
        --debug) PROFILE=debug ;;
        --release) PROFILE=release ;;
        --amd64) TARGETS+=("$AMD64") ;;
        --arm64) TARGETS+=("$ARM64") ;;
        --all-arch)
            TARGETS+=("$AMD64" "$ARM64")
            ;;
        --target)
            shift
            [ $# -ge 1 ] || {
                echo "build.sh: --target needs a triple" >&2
                exit 2
            }
            TARGETS+=("$1")
            ;;
        -h | --help)
            usage
            exit 0
            ;;
        *)
            echo "build.sh: unknown option $1" >&2
            usage >&2
            exit 2
            ;;
    esac
    shift
done

command -v cargo >/dev/null || {
    echo "build.sh: cargo not found - install Rust from https://rustup.rs" >&2
    exit 1
}

# Every target except the host one has to be installed into the toolchain
# first; the musl ones are self contained, so that is all it takes.
ensure_target() {
    local triple=$1
    rustup target list --installed 2>/dev/null | grep -qx "$triple" && return 0
    echo "==> rustup target add $triple"
    rustup target add "$triple"
}

if [ "${#TARGETS[@]}" -eq 0 ]; then
    echo "==> cargo build --$PROFILE (host)"
    if [ "$PROFILE" = release ]; then
        cargo build --release --manifest-path "$ROOT/Cargo.toml"
    else
        cargo build --manifest-path "$ROOT/Cargo.toml"
    fi
    BIN=$ROOT/target/$PROFILE/ibus-telex
    echo
    echo "built: $BIN"
    echo "next : ./install.sh"
    exit 0
fi

for triple in "${TARGETS[@]}"; do
    ensure_target "$triple"
    echo "==> cargo build --release --target $triple"
    if [ "$PROFILE" = release ]; then
        cargo build --release --target "$triple" --manifest-path "$ROOT/Cargo.toml"
    else
        cargo build --target "$triple" --manifest-path "$ROOT/Cargo.toml"
    fi
    bin=$ROOT/target/$triple/$PROFILE/ibus-telex
    printf '    %-46s %s\n' "$bin" "$(file -b "$bin" | cut -d, -f1-2)"
done

echo
echo "next: ./package.sh --all-arch    # .deb and .rpm for both architectures"

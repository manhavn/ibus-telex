#!/usr/bin/env bash
# One shot: build, install into ~/.local, register the input source with
# GNOME/IBus and restart the daemon.
#
#   ./install.sh                build, install, add the input source
#   ./install.sh --no-source    do not touch org.gnome.desktop.input-sources
#   ./install.sh --skip-build   install the binary that is already built
#
# Everything it changes lives under ~/.local, ~/.config and ~/.cache, and
# ./uninstall.sh undoes all of it.  No root needed - do not run it with sudo.
set -euo pipefail

ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
SKIP_BUILD=0
ADD_SOURCE=1
SERVICE=org.freedesktop.IBus.session.GNOME.service
SOURCE_KEY=org.gnome.desktop.input-sources

usage() {
    sed -n '2,11p' "$ROOT/install.sh" | sed 's/^# \{0,1\}//'
}

for arg in "$@"; do
    case "$arg" in
        --skip-build) SKIP_BUILD=1 ;;
        --no-source) ADD_SOURCE=0 ;;
        -h | --help)
            usage
            exit 0
            ;;
        *)
            echo "install.sh: unknown option $arg" >&2
            usage >&2
            exit 2
            ;;
    esac
done

if [ "$(id -u)" -eq 0 ]; then
    echo "install.sh: run this as your normal user, not with sudo" >&2
    echo "            (it installs into your own ~/.local)" >&2
    exit 1
fi

command -v cargo >/dev/null || {
    echo "install.sh: cargo not found - install Rust from https://rustup.rs" >&2
    exit 1
}
command -v ibus >/dev/null || {
    echo "install.sh: ibus not found - install it first (sudo apt install ibus)" >&2
    exit 1
}

if [ "$SKIP_BUILD" -eq 0 ]; then
    echo "==> building"
    cargo build --release --manifest-path "$ROOT/Cargo.toml"
fi

BIN="$ROOT/target/release/ibus-telex"
[ -x "$BIN" ] || {
    echo "install.sh: no binary at $BIN (run ./build.sh)" >&2
    exit 1
}

# A running engine holds the installed binary open, which would make the copy
# fail; the daemon starts a fresh one when it needs it.
if pkill -f "$HOME/.local/bin/ibus-telex --ibus" 2>/dev/null; then
    echo "==> stopped the running engine"
    sleep 0.5
fi

echo "==> installing into ~/.local and registering with IBus"
"$BIN" install

if [ "$ADD_SOURCE" -eq 1 ] && command -v gsettings >/dev/null; then
    CURRENT=$(gsettings get "$SOURCE_KEY" sources 2>/dev/null || true)
    case "$CURRENT" in
        *"'Telex'"*) echo "==> input source already registered" ;;
        "")
            echo "!! gsettings did not return the input sources, skipping" >&2
            ;;
        *)
            BACKUP="${XDG_CACHE_HOME:-$HOME/.cache}/ibus-telex/input-sources.bak"
            mkdir -p -- "$(dirname -- "$BACKUP")"
            printf '%s\n' "$CURRENT" >"$BACKUP"
            echo "==> adding the Telex input source (previous value in $BACKUP)"
            if "$BIN" sources --append >"$BACKUP.new" &&
                gsettings set "$SOURCE_KEY" sources "$(cat "$BACKUP.new")"; then
                rm -f -- "$BACKUP.new"
                AFTER=$(gsettings get "$SOURCE_KEY" sources)
                case "$AFTER" in
                    *"'Telex'"*) ;;
                    *)
                        echo "!! the input sources came out wrong, putting back:" >&2
                        echo "   $CURRENT" >&2
                        gsettings set "$SOURCE_KEY" sources "$CURRENT"
                        ;;
                esac
            else
                echo "!! could not set the input sources; add 'Vietnamese (Telex)'" >&2
                echo "   by hand in Settings -> Keyboard -> Input Sources" >&2
            fi
            ;;
    esac
fi

# The daemon only picks a new binary up when it starts the engine again.
if systemctl --user is-active --quiet "$SERVICE" 2>/dev/null; then
    echo "==> restarting $SERVICE"
    systemctl --user restart "$SERVICE"
elif command -v ibus >/dev/null; then
    echo "==> restarting ibus-daemon"
    ibus restart >/dev/null 2>&1 || true
fi

echo
if "$BIN" doctor; then :; fi

cat <<'EOF'

Done.
  switch to it   Super+Space, or:  ibus engine Telex
  check          ibus-telex doctor
  remove         ./uninstall.sh
EOF

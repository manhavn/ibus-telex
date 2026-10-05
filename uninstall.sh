#!/usr/bin/env bash
# Remove the engine again: the input source, the component file, the systemd
# drop-in and the binary.  Undoes ./install.sh.
#
#   ./uninstall.sh [--keep-source]
set -euo pipefail

ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
SERVICE=org.freedesktop.IBus.session.GNOME.service
SOURCE_KEY=org.gnome.desktop.input-sources
KEEP_SOURCE=0

for arg in "$@"; do
    case "$arg" in
        --keep-source) KEEP_SOURCE=1 ;;
        -h | --help)
            sed -n '2,5p' "$ROOT/uninstall.sh" | sed 's/^# \{0,1\}//'
            exit 0
            ;;
        *)
            echo "uninstall.sh: unknown option $arg" >&2
            exit 2
            ;;
    esac
done

if [ "$(id -u)" -eq 0 ]; then
    echo "uninstall.sh: run this as your normal user, not with sudo" >&2
    exit 1
fi

BIN="$ROOT/target/release/ibus-telex"
[ -x "$BIN" ] || BIN=$(command -v ibus-telex || true)

# Take the input source out of the GNOME list again.
if [ "$KEEP_SOURCE" -eq 0 ] && [ -n "$BIN" ] && command -v gsettings >/dev/null; then
    CURRENT=$(gsettings get "$SOURCE_KEY" sources 2>/dev/null || true)
    case "$CURRENT" in
        *"'Telex'"*)
            BACKUP="${XDG_CACHE_HOME:-$HOME/.cache}/ibus-telex/input-sources.bak"
            mkdir -p -- "$(dirname -- "$BACKUP")"
            printf '%s\n' "$CURRENT" >"$BACKUP"
            echo "==> removing the Telex input source (previous value in $BACKUP)"
            gsettings set "$SOURCE_KEY" sources "$("$BIN" sources --remove)"
            ;;
    esac
fi

# A running engine must go before its binary can be replaced or removed.
if pkill -f "$HOME/.local/bin/ibus-telex --ibus" 2>/dev/null; then
    echo "==> stopped the running engine"
    sleep 0.5
fi

if [ -n "$BIN" ] && [ -x "$BIN" ]; then
    echo "==> removing the component, the drop-in and the binary"
    "$BIN" uninstall
else
    echo "==> removing the files by hand"
    rm -f -- "${XDG_DATA_HOME:-$HOME/.local/share}/ibus/component/telex.xml"
    rm -f -- "${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user/$SERVICE.d/telex.conf"
    rm -f -- "$HOME/.local/bin/ibus-telex"
    systemctl --user daemon-reload 2>/dev/null || true
fi

# The daemon caches the component list; without this it keeps the engine.
rm -f -- "${XDG_CACHE_HOME:-$HOME/.cache}/ibus/bus/registry"

if systemctl --user is-active --quiet "$SERVICE" 2>/dev/null; then
    echo "==> restarting $SERVICE"
    systemctl --user restart "$SERVICE"
elif command -v ibus >/dev/null; then
    ibus restart >/dev/null 2>&1 || true
fi

echo
echo "Removed.  If Gnome still shows the input source, log out and back in."

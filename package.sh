#!/usr/bin/env bash
# Build installable packages for a release.
#
#   ./package.sh                 .deb and .rpm (the .rpm uses rpmbuild, or
#                                falls back to podman with a Fedora image)
#   ./package.sh --deb           only the .deb
#   ./package.sh --rpm           only the .rpm
#   ./package.sh --no-container  never fall back to a container
#   ./package.sh --skip-build    package the binary that is already built
#
# The packages install the engine system wide - /usr/bin/ibus-telex and
# /usr/share/ibus/component/telex.xml - which is the directory IBus reads
# anyway, so `apt install ./ibus-telex_*.deb` or `dnf install ./ibus-telex-*.rpm`
# is the whole installation.  The input source is per user and is still added
# once by hand; the package prints how.
#
# Artifacts end up in dist/, together with SHA256SUMS for the release page.
set -euo pipefail

ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
NAME=ibus-telex
DIST=$ROOT/dist
BIN=$ROOT/target/release/$NAME

WANT_DEB=0
WANT_RPM=0
SKIP_BUILD=0
CONTAINER=1

for arg in "$@"; do
    case "$arg" in
        --deb) WANT_DEB=1 ;;
        --rpm) WANT_RPM=1 ;;
        --skip-build) SKIP_BUILD=1 ;;
        --no-container) CONTAINER=0 ;;
        -h | --help)
            sed -n '2,18p' "$ROOT/package.sh" | sed 's/^# \{0,1\}//'
            exit 0
            ;;
        *)
            echo "package.sh: unknown option $arg" >&2
            exit 2
            ;;
    esac
done

# No explicit choice means "everything that is possible".
if [ "$WANT_DEB" -eq 0 ] && [ "$WANT_RPM" -eq 0 ]; then
    WANT_DEB=1
    WANT_RPM=1
fi

log() { echo "==> $*"; }
warn() { echo "!!  $*" >&2; }

command -v cargo >/dev/null || {
    echo "package.sh: cargo not found - install Rust from https://rustup.rs" >&2
    exit 1
}

if [ "$SKIP_BUILD" -eq 0 ]; then
    log "building the release binary"
    cargo build --release --manifest-path "$ROOT/Cargo.toml"
fi
[ -x "$BIN" ] || {
    echo "package.sh: no binary at $BIN (run ./build.sh)" >&2
    exit 1
}

VERSION=$(sed -n 's/^version *= *"\(.*\)"/\1/p' "$ROOT/Cargo.toml" | head -1)
[ -n "$VERSION" ] || {
    echo "package.sh: cannot read the version from Cargo.toml" >&2
    exit 1
}
DEB_ARCH=$(dpkg --print-architecture 2>/dev/null || echo amd64)
RPM_ARCH=$(uname -m)
[ -n "${DEBEMAIL:-}" ] && maintainer="${DEBFULLNAME:-$(git -C "$ROOT" config user.name || echo ibus-telex)} <$DEBEMAIL>"
maintainer=${maintainer:-"$(git -C "$ROOT" config user.name 2>/dev/null || echo 'ibus-telex developers') <$(git -C "$ROOT" config user.email 2>/dev/null || echo 'ibus-telex@localhost')>"}
YEAR=$(date +%Y)

# The oldest libc the binary needs, straight out of its symbol table - a
# package that claims less will simply not start on the target machine.
GLIBC=$(objdump -T "$BIN" 2>/dev/null | grep -o 'GLIBC_[0-9.]*' | sort -uV | tail -1 | sed 's/^GLIBC_//' || true)
if [ -n "$GLIBC" ]; then
    DEB_LIBC="libc6 (>= $GLIBC)"
    log "binary needs glibc $GLIBC"
else
    DEB_LIBC=libc6
fi

SHORT_DESC="Vietnamese input method engine for IBus (Telex/VNI)"
LONG_DESC="Vietnamese input method for IBus, written in Rust: Telex and VNI, tone
marks, đ, ư, ơ, ă, â, ê, ô, spell checking and automatic restore of words
that are not Vietnamese.  It speaks the IBus D-Bus protocol directly, so it
needs neither libibus nor any other C library beyond libc.
.
Add the input source once per user account under
Settings -> Keyboard -> Input Sources -> Vietnamese -> Telex, then switch to
it with Super+Space."

POST_INSTALL_TEXT="
Add the input source once per user account:

    Settings -> Keyboard -> Input Sources -> Vietnamese -> Telex (Vietnamese)

Without a desktop settings dialog, the same thing from a terminal:

    ibus-telex sources --append     # prints the value to put in
    gsettings set org.gnome.desktop.input-sources sources '<that value>'

Then switch to it with Super+Space.
"

# only clear what is about to be rebuilt, so building one format does not
# throw away the artifacts of the other
mkdir -p "$DIST"
rm -f "$DIST"/*.deb "$DIST"/*.rpm "$DIST"/SHA256SUMS

# ---------------------------------------------------------------- .deb
make_deb() {
    local stage=$DIST/pkg/deb
    log "building the .deb"
    mkdir -p "$stage/DEBIAN" \
        "$stage/usr/bin" \
        "$stage/usr/share/ibus/component" \
        "$stage/usr/share/doc/$NAME"

    install -Dm755 "$BIN" "$stage/usr/bin/$NAME"
    "$BIN" component --exec "/usr/bin/$NAME" >"$stage/usr/share/ibus/component/telex.xml"
    install -Dm644 "$ROOT/README.md" "$stage/usr/share/doc/$NAME/README.md"
    git -C "$ROOT" log --no-decorate --date=short --pretty='%h %ad %s' 2>/dev/null |
        gzip -9n >"$stage/usr/share/doc/$NAME/changelog.gz" || true

    cat >"$stage/usr/share/doc/$NAME/copyright" <<EOF
Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/
Upstream-Name: $NAME

Files: *
Copyright: $YEAR $maintainer
License: GPL-3.0-or-later
 On Debian systems the full text of the GNU General Public License version 3
 can be found in /usr/share/common-licenses/GPL-3.
EOF

    # shipped files are 0644/0755, not whatever the umask left behind
    chmod -R u=rwX,go=rX "$stage/usr"

    local size indented
    size=$(du -sk --exclude=DEBIAN "$stage" | cut -f1)
    # every line after the first one of a control Description has to start
    # with a space, and a lone "." is a paragraph break
    indented=$(printf '%s\n' "$LONG_DESC" | sed 's/^/ /')

    cat >"$stage/DEBIAN/control" <<EOF
Package: $NAME
Version: $VERSION
Section: utils
Priority: optional
Architecture: $DEB_ARCH
Depends: $DEB_LIBC, libgcc-s1, ibus (>= 1.5.0)
Recommends: ibus
Installed-Size: $size
Maintainer: $maintainer
Description: $SHORT_DESC
$indented
EOF

    cat >"$stage/DEBIAN/postinst" <<EOF
#!/bin/sh
set -e
if command -v ibus >/dev/null 2>&1; then
    ibus write-cache --system >/dev/null 2>&1 || true
fi
echo "$POST_INSTALL_TEXT"
exit 0
EOF

    cat >"$stage/DEBIAN/postrm" <<'EOF'
#!/bin/sh
set -e
if command -v ibus >/dev/null 2>&1; then
    ibus write-cache --system >/dev/null 2>&1 || true
fi
exit 0
EOF

    chmod 755 "$stage/DEBIAN/postinst" "$stage/DEBIAN/postrm"
    dpkg-deb --build --root-owner-group "$stage" "$DIST/${NAME}_${VERSION}_${DEB_ARCH}.deb" >/dev/null
    rm -rf "$DIST/pkg/deb"
    echo "    $DIST/${NAME}_${VERSION}_${DEB_ARCH}.deb"
}

# ---------------------------------------------------------------- .rpm
write_spec() {
    local top=$1
    mkdir -p "$top/SPECS" "$top/SOURCES" "$top/BUILD" "$top/RPMS" "$top/SRPMS"
    install -Dm755 "$BIN" "$top/SOURCES/$NAME"
    "$BIN" component --exec "/usr/bin/$NAME" >"$top/SOURCES/telex.xml"
    install -Dm644 "$ROOT/README.md" "$top/SOURCES/README.md"

    cat >"$top/SPECS/$NAME.spec" <<EOF
Name:           $NAME
Version:        $VERSION
Release:        1
Summary:        $SHORT_DESC
License:        GPL-3.0-or-later
Requires:       ibus >= 1.5.0
Source0:        $NAME
Source1:        telex.xml
Source2:        README.md
BuildArch:      $RPM_ARCH

%description
$LONG_DESC

%prep

%build

%install
rm -rf %{buildroot}
install -Dm755 %{SOURCE0} %{buildroot}%{_bindir}/$NAME
install -Dm644 %{SOURCE1} %{buildroot}%{_datadir}/ibus/component/telex.xml
install -Dm644 %{SOURCE2} %{buildroot}%{_docdir}/$NAME/README.md

%post
if command -v ibus >/dev/null 2>&1; then
    ibus write-cache --system >/dev/null 2>&1 || :
fi
echo "$POST_INSTALL_TEXT"

%postun
if command -v ibus >/dev/null 2>&1; then
    ibus write-cache --system >/dev/null 2>&1 || :
fi

%files
%{_bindir}/$NAME
%{_datadir}/ibus/component/telex.xml
%{_docdir}/$NAME/README.md

%changelog
* $(LC_ALL=C date '+%a %b %d %Y') $maintainer - $VERSION-1
- $VERSION
EOF
}

make_rpm() {
    local top=$DIST/rpmbuild
    log "building the .rpm"
    write_spec "$top"

    if command -v rpmbuild >/dev/null; then
        rpmbuild -bb --define "_topdir $top" "$top/SPECS/$NAME.spec" >/dev/null
    elif [ "$CONTAINER" -eq 1 ] && command -v podman >/dev/null; then
        log "no rpmbuild here, using podman with a fedora image"
        podman run --rm -v "$top:/dist:rw" fedora:latest bash -c '
            set -e
            dnf -y install rpm-build >/dev/null
            rpmbuild -bb --define "_topdir /dist" /dist/SPECS/'"$NAME"'.spec >/dev/null
        '
    else
        warn "no rpmbuild and no podman: skipping the .rpm"
        warn "install rpm (apt install rpm) or podman, or use --deb"
        return 1
    fi

    local rpm
    rpm=$(find "$top/RPMS" -name '*.rpm' -print -quit)
    [ -n "$rpm" ] || {
        warn "the rpm build produced nothing"
        return 1
    }
    cp -- "$rpm" "$DIST/"
    rm -rf "$top"
    echo "    $DIST/$(basename "$rpm")"
}

status=0
if [ "$WANT_DEB" -eq 1 ]; then
    command -v dpkg-deb >/dev/null || {
        warn "dpkg-deb not found: skipping the .deb"
        status=1
    }
    if command -v dpkg-deb >/dev/null; then make_deb; fi
fi
if [ "$WANT_RPM" -eq 1 ]; then
    make_rpm || true
fi

# ------------------------------------------------------------- checksums
mapfile -t artifacts < <(find "$DIST" -maxdepth 1 -type f \( -name '*.deb' -o -name '*.rpm' \) | sort)
if [ "${#artifacts[@]}" -eq 0 ]; then
    warn "nothing was built"
    exit 1
fi
(cd "$DIST" && sha256sum "${artifacts[@]##*/}" >SHA256SUMS)

echo
log "artifacts in $DIST"
ls -1 "$DIST" | grep -v pkg | sed 's/^/    /'

deb=$(ls "$DIST" | grep '\.deb$' | head -1 || true)
rpm=$(ls "$DIST" | grep '\.rpm$' | head -1 || true)
cat <<EOF

Upload them, for example:

    gh release create v$VERSION --title "v$VERSION" --notes-from-tag dist/*.deb dist/*.rpm dist/SHA256SUMS

Anyone can then install with:
EOF
[ -n "$deb" ] && echo "    sudo apt install ./$deb"
[ -n "$rpm" ] && echo "    sudo dnf install ./$rpm"

exit "$status"

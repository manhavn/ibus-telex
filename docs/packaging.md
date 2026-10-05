# Packages

```sh
./package.sh                # .deb and .rpm into dist/
./package.sh --deb          # only the .deb
./package.sh --rpm          # only the .rpm
```

Artifacts land in `dist/` next to a `SHA256SUMS` file:

```
dist/ibus-telex_1.0.0_amd64.deb
dist/ibus-telex-1.0.0-1.x86_64.rpm
dist/SHA256SUMS
```

Anyone can then install with

```sh
sudo apt install ./ibus-telex_1.0.0_amd64.deb
sudo dnf install ./ibus-telex-1.0.0-1.x86_64.rpm
```

## What is in them

```
/usr/bin/ibus-telex                       the engine
/usr/share/ibus/component/telex.xml       the IBus component
/usr/share/doc/ibus-telex/                README, copyright, changelog
```

A package installs system wide, which is the directory IBus reads by
default - so unlike `install.sh` there is no `IBUS_COMPONENT_PATH` drop-in
and no per-user component directory, and adding a file to
`/usr/share/ibus/component` is enough for the daemon to notice it (it
compares the directory's mtime).  The `postinst` runs `ibus write-cache
--system` as a belt and braces measure.

The **input source** is per user account and cannot be part of a package:
the `postinst` prints how to add it (`Settings -> Keyboard -> Input
Sources`, or `ibus-telex sources --append` plus `gsettings`).

## What the packages depend on

`Depends: libc6 (>= …), libgcc-s1, ibus (>= 1.5.0)`.  The libc version is
not guessed: `package.sh` reads the highest `GLIBC_x.y` symbol version out
of the binary with `objdump` and puts that in.  Nothing else is needed -
the engine speaks the IBus D-Bus protocol itself and does not link libibus,
GLib or GTK.

That also means the packages only run on a libc at least as new as the one
they were built against (glibc 2.39 from Ubuntu 26.10 as of writing, so
Ubuntu 24.04 and newer, Debian 13 and newer).  Building for something older
means compiling in a container of that distribution:

```sh
podman run --rm -v "$PWD:/src" -w /src ubuntu:22.04 bash -c '
    apt-get update && apt-get install -y curl build-essential pkg-config
    curl -sSf https://sh.rustup.rs | sh -s -- -y
    . "$HOME/.cargo/env"
    ./package.sh --deb
'
```

## Building the .rpm without rpm (what `package.sh` does)

`rpmbuild` is not part of a Debian/Ubuntu system, so when it is missing
`package.sh` builds the rpm inside a Fedora container with podman:

```sh
podman run --rm -v "$PWD/dist/rpmbuild:/dist" fedora:latest bash -c '
    dnf -y install rpm-build && rpmbuild -bb --define "_topdir /dist" /dist/SPECS/ibus-telex.spec
'
```

`--no-container` turns that fallback off, which is what you want on a
machine that has rpmbuild or no podman.

## Release

```sh
./test.sh                   # make sure it is good
./package.sh                # build both packages
gh release create v1.0.0 --title v1.0.0 --notes-from-tag \
    dist/*.deb dist/*.rpm dist/SHA256SUMS
```

`package.sh` prints exactly that line with the version filled in, plus the
`apt install` / `dnf install` commands for the release notes.

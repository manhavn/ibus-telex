# Packages

```sh
./package.sh                # .deb and .rpm for this machine's architecture
./package.sh --all-arch     # amd64 and arm64 both
./package.sh --arch amd64,arm64
./package.sh --deb          # only the .deb
./package.sh --rpm          # only the .rpm
```

Artifacts land in `dist/` next to a `SHA256SUMS` file:

```
dist/ibus-telex_1.0.0_amd64.deb
dist/ibus-telex_1.0.0_arm64.deb
dist/ibus-telex-1.0.0-1.x86_64.rpm
dist/ibus-telex-1.0.0-1.aarch64.rpm
dist/SHA256SUMS
```

Anyone can then install with

```sh
sudo apt install ./ibus-telex_1.0.0_amd64.deb      # or _arm64.deb
sudo dnf install ./ibus-telex-1.0.0-1.x86_64.rpm   # or .aarch64.rpm
```

## Both architectures

The binaries that go into a package come from the **musl** targets, so they
are statically linked:

* they run on any Linux of that architecture, whatever libc it has - the
  packages need `ibus` and nothing else, no `libc6 (>= …)`;
* the arm64 package can be built on an amd64 machine, because no cross C
  toolchain is involved: `rustup target add aarch64-unknown-linux-musl` and
  the LLD that ships with Rust do the linking (`.cargo/config.toml`).

```sh
./build.sh --all-arch       # both binaries, target/{x86_64,aarch64}-unknown-linux-musl
```

`./build.sh` on its own still builds the host architecture the normal way
(dynamically linked, for development and `./install.sh`).

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

`Depends: ibus (>= 1.5.0)` - and nothing else, since the binaries are
static.  The engine speaks the IBus D-Bus protocol itself; it does not link
libibus, GLib or GTK.

If you ever build a package from a *dynamically* linked binary instead,
`package.sh` reads the highest `GLIBC_x.y` symbol version out of it with
`objdump` and puts that in the dependency rather than guessing.

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

# ibus-telex

A Vietnamese input method engine for IBus, written in Rust, meant to be used
like `ibus-unikey` - Telex and VNI, tone marks, `đ`, `ư`, `ơ`, `ă`, `â`, `ê`,
`ô`, spell checking, auto restore of non-Vietnamese words, and the same
options in the input method panel.

Two things make it different from the C++ engines:

* **No C dependency at all.**  It speaks the `org.freedesktop.IBus.Engine`
  D-Bus protocol directly with [`zbus`](https://docs.rs/zbus); no `libibus`,
  no GLib, no `libibus-1.0-dev` needed to build or run it.  The protocol was
  taken from a live engine (`gdbus introspect` on ibus-unikey) rather than
  from documentation.
* **The Wayland problems are addressed on purpose**, see
  [`docs/wayland.md`](docs/wayland.md).

Typing behaviour was reverse engineered from ibus-unikey 0.7.0 and is checked
against it case by case, see [`docs/unikey-parity.md`](docs/unikey-parity.md).

## Install

```sh
cargo build --release
./target/release/ibus-telex install          # ~/.local, no root needed
ibus-telex sources --append                  # prints the gsettings value
gsettings set org.gnome.desktop.input-sources sources "$(~/.local/bin/ibus-telex sources --append)"
```

`install` copies the binary to `~/.local/bin/ibus-telex`, writes the component
to `~/.local/share/ibus/component/telex.xml`, points IBus at that directory
(IBus only reads `/usr/share/ibus/component` by default, see below) and
restarts the daemon.  Add `--add-source` to also register the GNOME input
source in one go.

Then switch with <kbd>Super</kbd>+<kbd>Space</kbd>, or directly:

```sh
ibus engine Telex
```

Requirements: Rust 1.75+, IBus 1.5.x (tested against 1.5.34 on Ubuntu
26.10 / GNOME Wayland).  Nothing else - not even the `libibus` development
headers.

### System wide install

Packages normally put the binary and the component in the system
directories, which needs root:

```sh
sudo install -m755 target/release/ibus-telex /usr/local/bin/ibus-telex
sudo install -m644 data/telex.xml /usr/share/ibus/component/telex.xml
sudo ibus write-cache && ibus restart
```

## Options

The options are the ones Unikey has; defaults match too.  Change them from
the IBus input method panel (the engine registers them with
`RegisterProperties`) or in `~/.config/ibus-telex/config`:

| option         | default | effect                                        |
|----------------|---------|-----------------------------------------------|
| `method`       | telex   | `telex` or `vni`                              |
| `spell_check`  | true    | only accept syllables that exist in Vietnamese |
| `auto_restore` | true    | restore the keystrokes of a non-Vietnamese word |
| `modern_style` | false   | `hoà`, `thuý` instead of `hòa`, `thúy`        |
| `free_marking` | true    | tone key may come before the final consonant  |
| `standalone_w` | true    | a lone `w` produces `ư` (Telex only)          |

## Commands

```
ibus-telex [--ibus]              run the engine (this is what ibus-daemon starts)
ibus-telex --xml                 engine list, printed for ibus-daemon
ibus-telex install [--add-source]
ibus-telex uninstall
ibus-telex doctor                check the session, the install, the options
ibus-telex sources --append      new value for org.gnome.desktop.input-sources
ibus-telex --help | --version
```

`IBUS_TELEX_DEBUG=1` makes the engine log what it does to stderr.

## How it works

```
src/
  vn/            the typing engine
    mod.rs       keystroke log -> rendered word, options, actions
    tables.rs    onsets and rhymes (generated, see docs/unikey-parity.md)
  ibus/          the D-Bus side
    engine.rs    org.freedesktop.IBus.Engine + Factory + Service objects
    object.rs    IBusText / IBusProperty / IBusPropList serialisation
    keys.rs      key values and modifier masks
  cli.rs         install, doctor, --xml, ...
tools/unikey/    the reference harness used to derive and verify behaviour
```

Every keypress is appended to a log of *keystrokes* and the word is rendered
from that log, which is how Unikey behaves: re-typing a modifier cancels the
mark it produced (`ass` -> `as`, `aww` -> `aw`), `Backspace` deletes one
*rendered* character (`tiếng` -> `tiến`, but `tiế` -> `ti`), and a word that
is not valid Vietnamese is committed as the raw keystrokes (`tiengs`
pre-edits as `tiéng` and commits as `tiengs`).  Spelling checking is what
makes the second half work: **a word that is not valid Vietnamese is
committed exactly as typed**, so `kof` stays `kof` (`cò` is the Vietnamese
word - `k` is only written before i/y/e/ê), never `kò`.

One engine object is created per input context, exactly like libibus engines;
the factory hands out `/org/freedesktop/IBus/Engine/<n>`.

## Tests

```sh
cargo test
```

* `tests/parity.rs` replays 252 captured ibus-unikey cases and requires an
  exact match.
* `tests/protocol.rs` starts the engine on a real bus and drives it through
  the calls `ibus-daemon` makes, checking the pre-edit, the commits, control
  key pass-through, focus handling and password fields.
* `tests/typing.rs` covers VNI and the options the corpus cannot reach.

## Why IBus picks up the component

IBus only reads `/usr/share/ibus/component` unless `IBUS_COMPONENT_PATH`
says otherwise, and it caches the directories it scanned in
`~/.cache/ibus/bus/registry`, so a component directory that did not exist
when the cache was written stays invisible.  `install` therefore writes a
systemd user drop-in
(`~/.config/systemd/user/org.freedesktop.IBus.session.GNOME.service.d/telex.conf`)
that extends `IBUS_COMPONENT_PATH`, drops the stale cache and restarts the
daemon.  `uninstall` undoes all of it.

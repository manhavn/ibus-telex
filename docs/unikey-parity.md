# Behaviour, as measured against ibus-unikey

"Like ibus-unikey" is not a claim, it is a test: this engine is compared
against the real thing, case by case, and the tests keep it that way.

## How the reference was captured

`tools/unikey/drive.py` connects to the running ibus-daemon (through libibus,
the same library a client application uses), asks ibus-unikey's factory for an
engine object, feeds it key events and records every `UpdatePreeditText` and
`CommitText` signal.  A model of an editor turns those into "what the user
would see":

* a pre-edit update replaces the pre-edit,
* a commit appends to the text and clears the pre-edit,
* a key the engine declines is inserted by the client,
* every case ends with a space so the word is committed, and that trailing
  space is not counted.

`tools/unikey/battery*.txt` hold 268 such key sequences: Vietnamese words,
tone placement, every modifier key, `Backspace`, `Escape`, `Delete`,
punctuation, uppercase, non-Vietnamese words, control keys.  The captures are
in `tools/unikey/data`; `tests/parity_corpus.tsv` (252 cases, the ones this
engine is expected to match) is generated from them.

The syllable tables (`src/vn/tables.rs`) are not hand written either: 839
probes of the form `onset + rhyme + tone key` were fed to Unikey, and an
onset or rhyme counts as valid when Unikey applies the diacritic instead of
restoring the raw keystrokes.  That is where the 28 onsets and 175 rhymes
come from, and it is why `ieng` is invalid while `iêng` is valid.

Running the same 268 captures against this engine gives 12 differences, all
of them deliberate.  No other case differs.

## The rules that were derived

* **Modifiers.** `s f r x j` set the tone, `z` removes it, doubling vowels
  give `â ê ô`, `w` gives `ă`/`ơ`/`ư` (and horns both vowels of `uo`),
  `[`/`]` give `ơ`/`ư`, `dd` gives `đ`.  VNI uses `1-5` for tones, `6 7 8`
  for `â ê ô`/`ơ ư`/`ă`, `9` for `đ`, `0` to remove a tone.
* **Re-typing a modifier cancels it** and turns *that* keystroke into a
  literal: `ass` -> `as`, `aaa` -> `aa`, `aww` -> `aw`, `ddd` -> `dd`,
  `ooo` -> `oo`, `[ [` -> `[`.
* **Diacritics are optimistic, commits are not.**  `tiengs` pre-edits as
  `tiéng` but commits as `tiengs`: a word that is not valid Vietnamese is
  restored to the raw keystrokes (`spell_check` + `auto_restore`).  This is
  what makes typing an English word with the IME on behave.
* **Tone placement.**  A vowel with its own diacritic takes the mark (`ươ`
  takes it on the second); with a final consonant the last vowel of the
  nucleus takes it; otherwise the first (`hóa`, `thúy`), except that a
  three vowel nucleus takes the middle (`khuýa`) and `modern_style` moves it
  to the second of `oa`, `oe`, `uy` (`hoà`, `thuý`).
* **Backspace works on what is rendered**, not on the keystrokes: `tiếng` ->
  `tiến` (the tone survives), `tiế` -> `ti` (the tone sat on the deleted
  vowel), `aw` -> `` (one character, two keys).
* **One junk character is tolerated.**  A `z` with no tone to remove, or a
  `[`/`]` with no vowel to horn, is kept as text without making the word
  invalid: `aaz` commits `âz`, `ơ]` commits `ơ]`.  A *second* character of
  that kind does invalidate it again: `aazz` commits `aazz`, `aazk` commits
  `aazk`.

## Deliberate differences

Measured with the harness; "Unikey" is what ibus-unikey 0.7.0 does.

| keys | Unikey | here | why |
|------|--------|------|-----|
| `t i e @focusout` | `` (word lost) | `tie` | focus loss commits the word, see [wayland.md](wayland.md) §4 |
| `a s @focusout` | `` | `á` | same |
| `d d @focusout` | `` | `đ` | same |
| `t i e e n g s @focusout` | `` | `tiếng` | same |
| `t i e n g s DEL` | `tiéng` | `tiengs` | `Delete` commits the word before the application sees the key (§7), and the commit then runs the auto-restore check like any other commit |
| `z a s` | `zá` | `zas` | Unikey accepts `z` as an onset by accident; here an unknown onset blocks the tone mark |
| `z e s`, `z o s`, `Z a s`, `z e e s` | `zé`, `zó`, `Zá`, `zế` | `zes`, `zos`, `Zas`, `zees` | same |
| `u w ]` | `]` | `ư]` | Unikey's `]` handler swallows a preceding `ư`; here `]` after a vowel is plain text |
| `w ]` | `]` | `ư]` | same |

One more difference that is *not* visible in the text: with an empty buffer
Unikey handles the space key itself (`CommitText(" ")`, return `true`), this
engine declines it (`return false`) and lets the application insert the
space.  The resulting text is the same; the difference shows up only in the
`handled` flag, and declining is one D-Bus round trip cheaper.

## Caveats

* **VNI is not oracle checked.**  The reference engine was captured in its
  default Telex mode; VNI is implemented from the method's definition, so it
  is covered by `tests/typing.rs` but not by the parity corpus.  Flipping
  `org.freedesktop.ibus.engine.unikey input-method` and re-running `drive.py`
  would close that gap.
* **Unikey's quirks are inherited where they are not harmful.**  The syllable
  tables come from measuring Unikey, so things it accepts by accident (for
  instance `ưn` as a rhyme) are accepted here too.  Only the two quirks that
  change the text (`z` as an onset, `]` swallowing `ư`) are rejected.
* **Only the Telex defaults were measured.**  Unikey's `spell-check`,
  `auto-restore-non-vn`, `free-marking` and `modern-style` were on/off as
  listed in the README; the corpus does not cover the other combinations,
  `tests/typing.rs` does.

## Regenerating

```sh
cd tools/unikey
./drive.py Unikey Unikey battery1.txt --json data/battery_golden.json   # needs ibus-unikey
./drive.py Unikey Unikey battery2.txt --json data/battery2_golden.json
... battery3 ... battery7
./gen_probe.py && ./drive.py Unikey Unikey data/probe_battery.txt --json data/probe_golden.json
cd ../..
python3 tools/unikey/gen_tables.py src/vn/tables.rs
python3 tools/unikey/gen_corpus.py tests/parity_corpus.tsv
cargo test
```

`gen_tables.py` and `gen_corpus.py` read only the captures in
`tools/unikey/data`, so the tables and the corpus can be rebuilt byte for
byte without an IBus install; only `drive.py` needs a live Unikey.

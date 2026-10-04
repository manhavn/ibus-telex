# Reference harness

Scripts that capture how ibus-unikey behaves, so this engine's behaviour and
its syllable tables can be derived from measurements instead of guesses.
Read [`../../docs/unikey-parity.md`](../../docs/unikey-parity.md) first.

## `drive.py` - capture a live engine

```
drive.py <bus name suffix> <engine name> <battery file> [--json out.json]
```

Connects to the running ibus-daemon with libibus (`IBus.Bus()`), asks the
factory for `CreateEngine`, and for every battery line: creates a fresh
engine object, sends the keys, and records the signals.  A trailing space is
appended to each line so the word is committed.

Battery syntax (space separated):

```
a b c          one character per token, `A` means the key value for A
Sx             shift + x
BACK RETURN ESC DEL TAB LEFT RIGHT HOME END PGUP PGDN SPACE
@focusout @focusin @reset @disable @enable
# comments
```

`--json` writes one record per line with the keys, the `handled` flags, the
commit chunks, the final pre-edit and every signal.

## `gen_probe.py` - probe syllable validity

Writes `data/probe_battery.txt` and `data/probe_manifest.json`: every
candidate onset and every nucleus+coda combination followed by a tone key.
Feed the battery to an engine with `drive.py`, and a rhyme is valid when the
engine applies the diacritic instead of restoring the keystrokes.

## `gen_tables.py`, `gen_corpus.py` - build artefacts

```
gen_tables.py <path to src/vn/tables.rs>
gen_corpus.py <path to tests/parity_corpus.tsv>
```

Both read only files in `data/`, so they run without IBus installed.
`gen_corpus.py` leaves out the cases listed in `docs/unikey-parity.md`
("Deliberate differences").

## data/

| file | what it is |
|------|------------|
| `battery*_golden.json` | the 268 captured cases, `{keys, word}` |
| `probe_manifest.json` | which onset/rhyme each probe belongs to |
| `probe_golden.json` | the 839 probe results |
| `probe_battery.txt` | the generated probe battery |

The captures are trimmed to the fields the generators need.

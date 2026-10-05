#!/usr/bin/python3
"""Turn the Unikey validity probe into src/vn/tables.rs.

Usage: gen_tables.py <path to src/vn/tables.rs>
Captures are read from ./data (see README.md).

Three things come out of the probes:

  * which onsets exist,
  * which onsets may be followed by which vowel - `k` is only used before
    i/y/e/ê in Vietnamese and Unikey enforces that while typing, so probing
    an onset with a single vowel is not enough,
  * which rhymes (nucleus + coda) exist.

An onset, an onset/vowel pair or a rhyme counts as valid when Unikey applies
the diacritic instead of restoring the raw keystrokes.
"""
import json, os, sys

HERE = os.path.dirname(os.path.abspath(__file__))
DATA = os.path.join(HERE, "data")

gold = json.load(open(os.path.join(DATA, "probe_golden.json")))
man = json.load(open(os.path.join(DATA, "probe_manifest.json")))
assert len(gold) == len(man), (len(gold), len(man))

TONED = {
    'a': 'aáàảãạ', 'ă': 'ăắằẳẵặ', 'â': 'âấầẩẫậ', 'e': 'eéèẻẽẹ',
    'ê': 'êếềểễệ', 'i': 'iíìỉĩị', 'o': 'oóòỏõọ', 'ô': 'ôốồỗộ', 'ơ': 'ơớờởỡợ',
    'u': 'uúùủũụ', 'ư': 'ưứừửữự', 'y': 'yýỳỷỹỵ',
}
UNMAP = {}
for base, forms in TONED.items():
    for f in forms:
        UNMAP[f] = base
        UNMAP[f.upper()] = base

# `w`/`[`/`]` are Telex modifiers, not onsets; `z` is a Unikey quirk we do not model.
EXCLUDE_ONSETS = {'w', 'z'}
# The vowel characters, in the order the probes were made.
VOWEL_ORDER = ['a', 'ă', 'â', 'e', 'ê', 'i', 'o', 'ô', 'ơ', 'u', 'ư', 'y']

onset_probe, onset_vowels, rhymes = {}, {}, {}
for g, m in zip(gold, man):
    raw = m['probe']
    assert g['keys'].replace(' ', '') == raw, (g['keys'], raw)
    transformed = g['word'] != raw
    if m['kind'] == 'onset':
        onset_probe[m['on']] = transformed
    elif m['kind'] == 'onset_vowel':
        onset_vowels.setdefault(m['on'], set())
        if transformed:
            onset_vowels[m['on']].add(m['vchar'])
    else:
        key = m['nuc'] + m['coda']
        if transformed:
            if not all(c in UNMAP if c in 'aăâeêioôơuưy' else c.isascii() for c in key):
                print('!! unknown char in', key, file=sys.stderr)
                continue
            rhymes[key] = True

# An onset exists when it can be followed by at least one vowel.
onsets = sorted(
    {'đ' if on == 'dd' else on for on, vs in onset_vowels.items()
     if vs and on not in EXCLUDE_ONSETS}
)
matrix = {
    ('đ' if on == 'dd' else on): ''.join(v for v in VOWEL_ORDER if v in vs)
    for on, vs in onset_vowels.items()
    if vs and on not in EXCLUDE_ONSETS
}
rhymes = sorted(rhymes)


def prefixes(words):
    out = set()
    for w in words:
        for i in range(1, len(w) + 1):
            out.add(w[:i])
    out.discard('')
    return sorted(out)


o_pre = prefixes(onsets)
r_pre = prefixes(rhymes)


def arr(name, items, doc):
    body = "\n".join('    "%s",' % s for s in items)
    return "#[rustfmt::skip]\n/// %s\npub const %s: &[&str] = &[\n%s\n];\n" % (doc, name, body)


src = '''//! Vietnamese syllable tables.
//!
//! Derived empirically from ibus-unikey 0.7.0 (Telex, spell check on) by
//! driving the real engine over D-Bus: an onset, an onset/vowel pair or a
//! rhyme is valid when Unikey applies the diacritic instead of restoring the
//! raw keystrokes.
//!
//! Regenerate with the harness in tools/unikey (see docs/unikey-parity.md).

#![allow(clippy::all)]

'''
src += arr('ONSETS', onsets, 'Valid syllable onsets (empty onset included).')
src += "\n"
src += arr('ONSET_PREFIXES', o_pre, 'Prefixes of valid onsets.')
src += "\n"
src += arr('RHYMES', rhymes, 'Valid rhymes: nucleus + coda, tone-less, lower case.')
src += "\n"
src += arr('RHYME_PREFIXES', r_pre, 'Prefixes of valid rhymes.')
src += "\n"
src += "#[rustfmt::skip]\n"
src += "/// First vowel each onset may be followed by, e.g. `k` only before i/y/e/ê.\n"
src += "pub const ONSET_VOWELS: &[(&str, &str)] = &[\n"
for on in sorted(matrix):
    if on:
        src += '    ("%s", "%s"),\n' % (on, matrix[on])
src += "];\n"
src += '''
/// Valid codas.
pub const CODAS: &[&str] = &["c", "ch", "m", "n", "ng", "nh", "p", "t"];

/// Prefixes of valid codas.
pub const CODA_PREFIXES: &[&str] = &["c", "ch", "m", "n", "ng", "nh", "p", "t"];

/// Is `s` a well-formed onset?
pub fn is_onset(s: &str) -> bool {
    s.is_empty() || ONSETS.binary_search(&s).is_ok()
}

/// Can `s` still become a well-formed onset?
pub fn is_onset_prefix(s: &str) -> bool {
    ONSET_PREFIXES.binary_search(&s).is_ok()
}

/// Is `s` a well-formed coda?
pub fn is_valid_coda(s: &str) -> bool {
    s.is_empty() || CODAS.binary_search(&s).is_ok()
}

/// Can `s` still become a well-formed coda?
pub fn is_coda_prefix(s: &str) -> bool {
    s.is_empty() || CODA_PREFIXES.binary_search(&s).is_ok()
}

/// Is `s` a well-formed rhyme (nucleus + coda, without the tone mark)?
pub fn is_rhyme(s: &str) -> bool {
    RHYMES.binary_search(&s).is_ok()
}

/// May `onset` be followed by a nucleus that starts with `vowel`?
pub fn onset_allows(onset: &str, vowel: char) -> bool {
    if onset.is_empty() {
        return true;
    }
    match ONSET_VOWELS.binary_search_by(|(o, _)| o.cmp(&onset)) {
        Ok(i) => ONSET_VOWELS[i].1.contains(vowel),
        Err(_) => false,
    }
}
'''

open(sys.argv[1], 'w').write(src)
print('onsets=%d rhymes=%d' % (len(onsets), len(rhymes)))
print('onsets:', onsets)
print('k allows:', matrix.get('k'))
print('qu allows:', matrix.get('qu'))
print('gi allows:', matrix.get('gi'))
print('ngh allows:', matrix.get('ngh'))

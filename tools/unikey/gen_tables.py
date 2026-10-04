#!/usr/bin/python3
"""Turn the Unikey validity probe into src/vn/tables.rs.

Usage: gen_tables.py <path to src/vn/tables.rs>
Captures are read from ./data (see README.md).
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

onsets, rhymes = [], []
for g, m in zip(gold, man):
    raw = m['probe']
    assert g['keys'].replace(' ', '') == raw, (g['keys'], raw)
    transformed = g['word'] != raw
    if m['kind'] == 'onset':
        if transformed and m['on'] not in EXCLUDE_ONSETS:
            onsets.append({'dd': 'đ'}.get(m['on'], m['on']))
    else:
        key = m['nuc'] + m['coda']
        if transformed:
            # vowels must be known Vietnamese letters; codas are plain ASCII
            if not all(c in UNMAP if c in 'aăâeêioôơuưy' else c.isascii() for c in key):
                print('!! unknown char in', key, file=sys.stderr)
                continue
            rhymes.append(key)

onsets = sorted(set(onsets))
rhymes = sorted(set(rhymes))


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
//! driving the real engine over D-Bus: an onset/rhyme is valid when Unikey
//! applies the diacritic instead of restoring the raw keystrokes.
//!
//! Regenerate with the harness in the repository (see docs/unikey-parity.md).

#![allow(clippy::all)]

'''
src += arr('ONSETS', onsets, 'Valid syllable onsets (empty onset included).')
src += "\n"
src += arr('ONSET_PREFIXES', o_pre, 'Prefixes of valid onsets.')
src += "\n"
src += arr('RHYMES', rhymes, 'Valid rhymes: nucleus + coda, tone-less, lower case.')
src += "\n"
src += arr('RHYME_PREFIXES', r_pre, 'Prefixes of valid rhymes.')
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
'''

open(sys.argv[1], 'w').write(src)
print('onsets=%d rhymes=%d onset_prefixes=%d rhyme_prefixes=%d'
      % (len(onsets), len(rhymes), len(o_pre), len(r_pre)))
print('onsets:', onsets)
missing = [('a', ), ]
print('rhymes sample:', rhymes[:40])

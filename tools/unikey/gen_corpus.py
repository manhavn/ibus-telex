#!/usr/bin/python3
"""Emit tests/parity_corpus.tsv: <keys>\t<expected user visible text>.

Expected values come from driving the real ibus-unikey 0.7.0 engine over
D-Bus (golden*.json).  Cases whose behaviour this engine deliberately
changed are left out and documented in docs/unikey-parity.md:

  * focus loss / disable: the word is committed instead of dropped,
  * Delete: the word is committed before the key reaches the application,
  * words starting with `z`: Unikey accepts `z` as an onset by accident,
  * `]` right after `ư`, where Unikey swallows the `ư`.
"""
import json, os, sys

HERE = os.path.dirname(os.path.abspath(__file__))
DATA = os.path.join(HERE, "data")

FILES = ['battery_golden.json', 'battery2_golden.json', 'battery3_golden.json',
         'battery4_golden.json', 'battery5_golden.json', 'battery6_golden.json',
         'battery7_golden.json', 'battery8_golden.json']

SKIP = {
    # z as an onset
    'z a s', 'z e s', 'z o s', 'Z a s', 'z e e s',
    # `]` after `ư`
    'u w ]', 'w ]',
    # A stray `z`/`[`/`]` leaves the word misspelled, so this engine commits
    # the keystrokes unchanged.  Unikey keeps the stray character together
    # with the diacritics it had already applied - and is inconsistent about
    # it: `aaz` commits `âz` while `aak` commits `aak`.
    '[ ]', 'w z', 'a a z', 'o o z', 'a w z', 'o w z', 'u w z',
    'a a [', 'a a ]', 'o o ]', '[ z',
}

out = []
for name in FILES:
    for case in json.load(open(os.path.join(DATA, name))):
        keys = case['keys']
        if '@' in keys:
            continue
        if keys in SKIP:
            continue
        if ' DEL' in ' ' + keys:
            continue
        out.append((keys, case['word']))

path = sys.argv[1]
with open(path, 'w', encoding='utf-8') as f:
    f.write('# keystrokes\tuser visible result, captured from ibus-unikey 0.7.0\n')
    for keys, expected in out:
        f.write('%s\t%s\n' % (keys, expected))
print('wrote', len(out), 'cases to', path)

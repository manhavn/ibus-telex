#!/usr/bin/python3
"""Emit a battery probing validity of onsets and rhymes, plus the manifest."""
import json, os, sys

HERE = os.path.dirname(os.path.abspath(__file__))
DATA = os.path.join(HERE, "data")
os.makedirs(DATA, exist_ok=True)

ONONSETS = ["", "b", "c", "ch", "d", "dd", "g", "gh", "gi", "h", "k", "kh",
            "l", "m", "n", "ng", "ngh", "nh", "p", "ph", "q", "qu", "r", "s",
            "t", "th", "tr", "v", "x", "f", "j", "w", "z", "bl", "br", "cr",
            "dr", "gr", "pl", "pr", "sl", "sp", "st", "kl", "ngg", "qqu"]

# Telex keystrokes for each nucleus -> display form
NUCLEI = [
    ("a", "a"), ("aw", "ă"), ("aa", "â"), ("e", "e"), ("ee", "ê"),
    ("i", "i"), ("o", "o"), ("oo", "ô"), ("ow", "ơ"), ("u", "u"),
    ("uw", "ư"), ("y", "y"), ("ai", "ai"), ("ao", "ao"), ("au", "au"),
    ("ay", "ay"), ("eo", "eo"), ("eeu", "êu"), ("ia", "ia"), ("iee", "iê"),
    ("iu", "iu"), ("oa", "oa"), ("oaw", "oă"), ("oe", "oe"), ("oi", "oi"),
    ("ooi", "ôi"), ("owi", "ơi"), ("ua", "ua"), ("uaa", "uâ"), ("uee", "uê"),
    ("ui", "ui"), ("uoo", "uô"), ("uow", "ươ"), ("uy", "uy"), ("uya", "uya"),
    ("uyee", "uyê"), ("uwu", "ưu"), ("uwa", "ưa"), ("uwi", "ưi"),
    ("ieu", "ieu"), ("ieeu", "iêu"), ("yeeu", "yêu"), ("uooi", "uôi"),
    ("uowi", "ươi"), ("uowu", "ươu"), ("oai", "oai"), ("oay", "oay"),
    ("oeo", "oeo"), ("uaay", "uây"), ("uyu", "uyu"), ("uyeeu", "uyêu"),
    ("aau", "âu"), ("aay", "ây"), ("yee", "yê"), ("uou", "uou"),
    ("oao", "oao"), ("uye", "uye"), ("uay", "uay"), ("aai", "âi"),
    ("eei", "êi"), ("oou", "ôu"),
]
CODAS = ["", "c", "ch", "m", "n", "ng", "nh", "p", "t", "k", "b", "d", "g"]

lines, manifest = [], []
for on in ONONSETS:
    probe = on + "as"          # onset + 'a' + tone key
    lines.append(" ".join(probe))
    manifest.append({"kind": "onset", "on": on, "probe": probe})

# Onsets are not free to combine with every vowel: `k` is only used before
# i/y/e/ê in Vietnamese and Unikey enforces that while typing.  One probe per
# onset and vowel finds those rules.
VOWELS = [("a", "a"), ("aw", "ă"), ("aa", "â"), ("e", "e"), ("ee", "ê"),
          ("i", "i"), ("o", "o"), ("oo", "ô"), ("ow", "ơ"), ("u", "u"),
          ("uw", "ư"), ("y", "y")]
for on in ONONSETS:
    for keys, char in VOWELS:
        probe = on + keys + "s"
        lines.append(" ".join(probe))
        manifest.append({"kind": "onset_vowel", "on": on, "vokey": keys,
                         "vchar": char, "probe": probe})
for keys, disp in NUCLEI:
    for coda in CODAS:
        probe = "b" + keys + coda + "s"
        lines.append(" ".join(probe))
        manifest.append({"kind": "rhyme", "nuc_keys": keys, "nuc": disp,
                         "coda": coda, "probe": probe})

open(os.path.join(DATA, "probe_battery.txt"), "w").write("\n".join(lines) + "\n")
json.dump(manifest, open(os.path.join(DATA, "probe_manifest.json"), "w"), ensure_ascii=False)
print(len(lines), 'probes')

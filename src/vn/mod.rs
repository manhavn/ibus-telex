//! Vietnamese typing engine.
//!
//! The engine keeps a log of the *keystrokes* of the word being typed and
//! re-renders the word from that log.  The behaviour was reverse engineered
//! from ibus-unikey 0.7.0 by driving the real engine over D-Bus:
//!
//! * a keypress may be consumed as a *modifier* (tone mark, `w`, doubling,
//!   `dd`) instead of being appended to the word;
//! * re-typing the modifier that produced the current mark cancels the mark
//!   and turns *that* keystroke into a plain literal character
//!   (`ass` -> `as`, `aaa` -> `aa`, `ddd` -> `dd`, `aww` -> `aw`);
//! * diacritics are applied optimistically while typing, but at a word
//!   boundary a syllable that is not valid Vietnamese is *restored to the
//!   raw keystrokes* (`tiengs` pre-edits as `tiéng` and commits as
//!   `tiengs`) - this is what makes typing English words with the IME on
//!   behave sanely;
//! * `Backspace` removes one *rendered* character and the word is
//!   re-derived from what is left, so the tone mark survives deleting the
//!   final consonant (`tiếng` -> `tiến`) but a tone sitting on the deleted
//!   vowel disappears with it (`tiế` -> `ti`).

pub mod tables;

use std::fmt;

/// Input method flavour.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Method {
    Telex,
    Vni,
}

impl Method {
    pub fn as_str(self) -> &'static str {
        match self {
            Method::Telex => "telex",
            Method::Vni => "vni",
        }
    }
}

/// Tone mark.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Tone {
    #[default]
    None,
    Acute,
    Grave,
    Hook,
    Tilde,
    Dot,
}

/// Vowel diacritic.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Diac {
    Circumflex,
    Breve,
    Horn,
}

/// Engine options, mirroring the ones Unikey exposes in the panel.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Options {
    pub method: Method,
    /// Only accept syllables that exist in Vietnamese.
    pub spell_check: bool,
    /// Restore the raw keystrokes when the word is not a valid syllable.
    pub auto_restore: bool,
    /// `oà`/`uý` instead of `òa`/`úy`.
    pub modern_style: bool,
    /// The tone key may be typed before the final consonant.
    pub free_marking: bool,
    /// A lone `w` produces `ư`.
    pub standalone_w: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            method: Method::Telex,
            spell_check: true,
            auto_restore: true,
            modern_style: false,
            free_marking: true,
            standalone_w: true,
        }
    }
}

/// A keystroke in the word buffer.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Key {
    ch: char,
    /// Set when the keystroke was demoted from a modifier to a literal
    /// character by re-typing that modifier; it must never be treated as a
    /// modifier again.
    literal: bool,
}

/// The word currently being typed.
#[derive(Clone, Debug, Default)]
pub struct Word {
    keys: Vec<Key>,
}

/// Maximum number of keystrokes kept for one word.
pub const MAX_KEYS: usize = 64;

/// Result of rendering a word.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Rendered {
    /// Text as it should be shown in the pre-edit.
    pub text: String,
    /// Whether the word is a well-formed Vietnamese syllable.
    pub valid: bool,
    /// Whether a vowel diacritic or a tone mark was applied.
    pub marked: bool,
}

impl fmt::Display for Rendered {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.text)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Phase {
    Onset,
    Nucleus,
    Coda,
    /// One trailing character that the syllable parser has no use for but
    /// which does not make the word wrong (`aaz`, `ơ]`).  A second one does.
    Junk,
    Tail,
}

/// A rendered character together with the transformation it carries.
#[derive(Clone, Copy, Debug)]
struct Unit {
    /// ASCII base letter, lower case; `đ` is kept as a base character of the
    /// onset.
    base: char,
    upper: bool,
    diac: Option<Diac>,
    phase: Phase,
}

impl Unit {
    fn plain(ch: char, phase: Phase) -> Unit {
        Unit {
            base: ch.to_ascii_lowercase(),
            upper: ch.is_uppercase(),
            diac: None,
            phase,
        }
    }

    /// Lower case composed character, ignoring the tone mark.
    fn char_no_tone(&self) -> char {
        compose(self.base, self.diac).unwrap_or(self.base)
    }
}

/// The keystroke that acted as the most recent modifier.
#[derive(Clone, Copy, Debug)]
struct LastMod {
    ch: char,
}

struct Parsed {
    units: Vec<Unit>,
    tone: Tone,
    /// Index into `units` of the vowel carrying the tone mark.
    tone_at: Option<usize>,
    broken: bool,
    last_mod: Option<LastMod>,
}

impl Default for Parsed {
    fn default() -> Self {
        Parsed {
            units: Vec::new(),
            tone: Tone::None,
            tone_at: None,
            broken: false,
            last_mod: None,
        }
    }
}

fn is_vowel(ch: char) -> bool {
    matches!(ch, 'a' | 'e' | 'i' | 'o' | 'u' | 'y')
}

/// Telex tone keys.
fn telex_tone(ch: char) -> Option<Tone> {
    match ch {
        's' => Some(Tone::Acute),
        'f' => Some(Tone::Grave),
        'r' => Some(Tone::Hook),
        'x' => Some(Tone::Tilde),
        'j' => Some(Tone::Dot),
        _ => None,
    }
}

/// VNI tone keys.
fn vni_tone(ch: char) -> Option<Tone> {
    match ch {
        '1' => Some(Tone::Acute),
        '2' => Some(Tone::Grave),
        '3' => Some(Tone::Hook),
        '4' => Some(Tone::Tilde),
        '5' => Some(Tone::Dot),
        _ => None,
    }
}

fn tone_of(method: Method, ch: char) -> Option<Tone> {
    match method {
        Method::Telex => telex_tone(ch),
        Method::Vni => vni_tone(ch),
    }
}

fn tone_key(method: Method, tone: Tone) -> Option<char> {
    let telex = match tone {
        Tone::Acute => 's',
        Tone::Grave => 'f',
        Tone::Hook => 'r',
        Tone::Tilde => 'x',
        Tone::Dot => 'j',
        Tone::None => return None,
    };
    let vni = match tone {
        Tone::Acute => '1',
        Tone::Grave => '2',
        Tone::Hook => '3',
        Tone::Tilde => '4',
        Tone::Dot => '5',
        Tone::None => return None,
    };
    Some(match method {
        Method::Telex => telex,
        Method::Vni => vni,
    })
}

/// Composed vowel character for a base letter plus diacritic.
fn compose(base: char, diac: Option<Diac>) -> Option<char> {
    Some(match (base, diac) {
        (b, None) => b,
        ('a', Some(Diac::Circumflex)) => 'â',
        ('a', Some(Diac::Breve)) => 'ă',
        ('e', Some(Diac::Circumflex)) => 'ê',
        ('o', Some(Diac::Circumflex)) => 'ô',
        ('o', Some(Diac::Horn)) => 'ơ',
        ('u', Some(Diac::Horn)) => 'ư',
        _ => return None,
    })
}

/// Toned forms of the twelve Vietnamese vowel letters.
const TONED: [(char, [char; 6]); 12] = [
    ('a', ['a', 'á', 'à', 'ả', 'ã', 'ạ']),
    ('ă', ['ă', 'ắ', 'ằ', 'ẳ', 'ẵ', 'ặ']),
    ('â', ['â', 'ấ', 'ầ', 'ẩ', 'ẫ', 'ậ']),
    ('e', ['e', 'é', 'è', 'ẻ', 'ẽ', 'ẹ']),
    ('ê', ['ê', 'ế', 'ề', 'ể', 'ễ', 'ệ']),
    ('i', ['i', 'í', 'ì', 'ỉ', 'ĩ', 'ị']),
    ('o', ['o', 'ó', 'ò', 'ỏ', 'õ', 'ọ']),
    ('ô', ['ô', 'ố', 'ồ', 'ổ', 'ỗ', 'ộ']),
    ('ơ', ['ơ', 'ớ', 'ờ', 'ở', 'ỡ', 'ợ']),
    ('u', ['u', 'ú', 'ù', 'ủ', 'ũ', 'ụ']),
    ('ư', ['ư', 'ứ', 'ừ', 'ử', 'ữ', 'ự']),
    ('y', ['y', 'ý', 'ỳ', 'ỷ', 'ỹ', 'ỵ']),
];

fn tone_index(tone: Tone) -> usize {
    match tone {
        Tone::None => 0,
        Tone::Acute => 1,
        Tone::Grave => 2,
        Tone::Hook => 3,
        Tone::Tilde => 4,
        Tone::Dot => 5,
    }
}

fn tone_from_index(i: usize) -> Tone {
    match i {
        1 => Tone::Acute,
        2 => Tone::Grave,
        3 => Tone::Hook,
        4 => Tone::Tilde,
        5 => Tone::Dot,
        _ => Tone::None,
    }
}

/// Apply a tone mark to a vowel letter.
fn apply_tone(ch: char, tone: Tone) -> char {
    let idx = tone_index(tone);
    if idx == 0 {
        return ch;
    }
    for (base, forms) in TONED.iter() {
        if *base == ch {
            return forms[idx];
        }
    }
    ch
}

/// Analyse a rendered character: base letter, case, diacritic and tone mark.
fn analyse(ch: char) -> Option<(char, bool, Option<Diac>, Tone)> {
    let upper = ch.is_uppercase();
    let lower = ch.to_lowercase().next().unwrap_or(ch);
    if lower == 'đ' {
        return Some(('đ', upper, None, Tone::None));
    }
    for (base, forms) in TONED.iter() {
        for (i, f) in forms.iter().enumerate() {
            if *f == lower {
                let diac = match base {
                    'â' => Some(Diac::Circumflex),
                    'ă' => Some(Diac::Breve),
                    'ê' => Some(Diac::Circumflex),
                    'ô' => Some(Diac::Circumflex),
                    'ơ' => Some(Diac::Horn),
                    'ư' => Some(Diac::Horn),
                    _ => None,
                };
                let ascii = match base {
                    'â' | 'ă' => 'a',
                    'ê' => 'e',
                    'ô' | 'ơ' => 'o',
                    'ư' => 'u',
                    other => *other,
                };
                return Some((ascii, upper, diac, tone_from_index(i)));
            }
        }
    }
    None
}

impl Word {
    pub fn new() -> Word {
        Word { keys: Vec::new() }
    }

    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    pub fn clear(&mut self) {
        self.keys.clear();
    }

    /// Raw keystrokes, as they would have to be typed again.
    pub fn raw(&self) -> String {
        self.keys.iter().map(|k| k.ch).collect()
    }

    /// Render the word with the given options.
    pub fn render(&self, opts: &Options) -> Rendered {
        let p = self.parse(opts);
        let text = render_text(&p);
        let valid = self.is_valid(&p);
        let marked = p.tone != Tone::None || p.units.iter().any(|u| u.diac.is_some());
        Rendered {
            text,
            valid,
            marked,
        }
    }

    /// Text that must be committed at a word boundary.
    ///
    /// A word that is not valid Vietnamese is restored to its raw keystrokes
    /// when spell checking and auto restore are enabled - this is what makes
    /// typing English words with the IME enabled behave sanely.
    pub fn commit_text(&self, opts: &Options) -> String {
        let r = self.render(opts);
        if opts.spell_check && opts.auto_restore && r.marked && !r.valid {
            self.raw()
        } else {
            r.text
        }
    }

    /// Feed a character key.
    pub fn push(&mut self, ch: char, opts: &Options) {
        if self.keys.len() >= MAX_KEYS {
            self.keys.clear();
        }
        let lower = ch.to_ascii_lowercase();
        if let Some(last) = self.parse(opts).last_mod {
            if last.ch == lower && is_cancellable(ch, opts) {
                // The modifier keystroke that is being cancelled becomes a
                // literal character; the keystroke that cancelled it is
                // dropped altogether.
                if let Some(k) = self
                    .keys
                    .iter_mut()
                    .rev()
                    .find(|k| k.ch.to_ascii_lowercase() == lower && !k.literal)
                {
                    k.literal = true;
                }
                return;
            }
        }
        self.keys.push(Key { ch, literal: false });
    }

    /// Remove one rendered character; returns false when there is nothing
    /// to delete.
    pub fn backspace(&mut self, opts: &Options) -> bool {
        if self.keys.is_empty() {
            return false;
        }
        let p = self.parse(opts);
        let (mut onset, mut nucleus, mut coda, mut tail) = split(&p, true);
        if !tail.is_empty() {
            tail.pop();
        } else if !coda.is_empty() {
            coda.pop();
        } else if !nucleus.is_empty() {
            nucleus.pop();
        } else if !onset.is_empty() {
            onset.pop();
        } else {
            return false;
        }
        self.keys = rebuild_keys(&onset, &nucleus, &coda, &tail, opts);
        true
    }

    fn is_valid(&self, p: &Parsed) -> bool {
        if p.broken {
            return false;
        }
        let (onset, nucleus, coda, _) = split(p, false);
        let onset = onset.to_lowercase();
        let rhyme = format!("{}{}", nucleus.to_lowercase(), coda.to_lowercase());
        tables::is_onset(&onset) && !rhyme.is_empty() && tables::is_rhyme(&rhyme)
    }

    fn parse(&self, opts: &Options) -> Parsed {
        let mut p = Parsed::default();
        let mut phase = Phase::Onset;
        let mut last_mod: Option<LastMod> = None;

        for (ki, k) in self.keys.iter().enumerate() {
            let ch = k.ch;
            let lower = ch.to_ascii_lowercase();
            let last_key = ki + 1 == self.keys.len();

            if !k.literal && phase != Phase::Tail {
                match opts.method {
                    Method::Telex => {
                        // `dd` -> đ
                        if lower == 'd' {
                            if let Some(u) = p.units.last_mut() {
                                if u.phase == Phase::Onset && u.base == 'd' && u.diac.is_none() {
                                    u.base = 'đ';
                                    u.upper |= ch.is_uppercase();
                                    last_mod = Some(LastMod { ch: 'd' });
                                    continue;
                                }
                            }
                        }
                        // a lone `w` produces ư
                        if lower == 'w'
                            && opts.standalone_w
                            && phase == Phase::Onset
                            && p.units.is_empty()
                        {
                            push_vowel(&mut p, 'u', ch.is_uppercase(), Some(Diac::Horn));
                            phase = Phase::Nucleus;
                            last_mod = Some(LastMod { ch: 'w' });
                            continue;
                        }
                        // `[` -> ơ, `]` -> ư, but only while the nucleus is
                        // still empty; after a vowel they are plain text.
                        if (lower == '[' || lower == ']')
                            && !p.units.iter().any(|u| u.phase == Phase::Nucleus)
                        {
                            let base = if lower == '[' { 'o' } else { 'u' };
                            push_vowel(&mut p, base, false, Some(Diac::Horn));
                            phase = Phase::Nucleus;
                            last_mod = Some(LastMod { ch: lower });
                            continue;
                        }
                        // `w` bends or horns the preceding vowel
                        if lower == 'w' && phase != Phase::Onset {
                            if let Some(unit) = p.units.last().copied() {
                                if unit.phase == Phase::Nucleus && unit.diac.is_none() {
                                    let d = match unit.base {
                                        'a' => Some(Diac::Breve),
                                        'o' | 'u' => Some(Diac::Horn),
                                        _ => None,
                                    };
                                    if let Some(d) = d {
                                        let n = p.units.len();
                                        p.units[n - 1].diac = Some(d);
                                        // `uo` + w -> ươ
                                        if unit.base == 'o' && n >= 2 {
                                            let prev = p.units[n - 2];
                                            if prev.phase == Phase::Nucleus
                                                && prev.base == 'u'
                                                && prev.diac.is_none()
                                            {
                                                p.units[n - 2].diac = Some(Diac::Horn);
                                            }
                                        }
                                        last_mod = Some(LastMod { ch: 'w' });
                                        continue;
                                    }
                                }
                            }
                        }
                        // `z` removes the tone mark
                        if lower == 'z' && p.tone != Tone::None {
                            p.tone = Tone::None;
                            p.tone_at = None;
                            last_mod = Some(LastMod { ch: 'z' });
                            continue;
                        }
                        // A `z` with nothing to remove stays in the text but
                        // is not part of the syllable, so it does not make an
                        // otherwise valid word invalid - Unikey behaves the
                        // same way (`aaz` commits `âz`, `aak` commits `aak`).
                        if lower == 'z' && phase != Phase::Junk {
                            push_tail(&mut p, ch);
                            phase = Phase::Junk;
                            continue;
                        }
                    }
                    Method::Vni => {
                        // 6 circumflex, 7 horn, 8 breve, 9 đ, 0 delete tone
                        if matches!(lower, '6' | '7' | '8') {
                            if let Some(unit) = p.units.last().copied() {
                                if unit.phase == Phase::Nucleus && unit.diac.is_none() {
                                    let d = match (lower, unit.base) {
                                        ('6', 'a') | ('6', 'e') | ('6', 'o') => {
                                            Some(Diac::Circumflex)
                                        }
                                        ('7', 'o') | ('7', 'u') => Some(Diac::Horn),
                                        ('8', 'a') => Some(Diac::Breve),
                                        _ => None,
                                    };
                                    if let Some(d) = d {
                                        let n = p.units.len();
                                        p.units[n - 1].diac = Some(d);
                                        if lower == '7' && unit.base == 'o' && n >= 2 {
                                            let prev = p.units[n - 2];
                                            if prev.phase == Phase::Nucleus
                                                && prev.base == 'u'
                                                && prev.diac.is_none()
                                            {
                                                p.units[n - 2].diac = Some(Diac::Horn);
                                            }
                                        }
                                        last_mod = Some(LastMod { ch: lower });
                                        continue;
                                    }
                                }
                            }
                        }
                        if lower == '9' {
                            if let Some(u) = p.units.last_mut() {
                                if u.phase == Phase::Onset && u.base == 'd' && u.diac.is_none() {
                                    u.base = 'đ';
                                    last_mod = Some(LastMod { ch: '9' });
                                    continue;
                                }
                            }
                        }
                        if lower == '0' && p.tone != Tone::None {
                            p.tone = Tone::None;
                            p.tone_at = None;
                            last_mod = Some(LastMod { ch: '0' });
                            continue;
                        }
                        // see the `z` case above
                        if lower == '0' && phase != Phase::Junk {
                            push_tail(&mut p, ch);
                            phase = Phase::Junk;
                            continue;
                        }
                    }
                }
                // tone keys
                if let Some(tone) = tone_of(opts.method, lower) {
                    let has_nucleus = p.units.iter().any(|u| u.phase == Phase::Nucleus);
                    if has_nucleus && (opts.free_marking || last_key) && tone_allowed(&p) {
                        p.tone = tone;
                        p.tone_at = None;
                        last_mod = Some(LastMod { ch: lower });
                        continue;
                    }
                }
            }

            // --- ordinary characters --------------------------------------
            match phase {
                Phase::Tail => push_tail(&mut p, ch),
                Phase::Junk => {
                    // a second character the parser cannot place makes the
                    // word invalid again, so `aaz` stays `âz` but `aazz`
                    // and `aazk` are restored to the keystrokes
                    p.broken = true;
                    phase = Phase::Tail;
                    push_tail(&mut p, ch);
                    last_mod = None;
                }
                Phase::Onset => {
                    if is_vowel(lower) {
                        push_vowel(&mut p, lower, ch.is_uppercase(), None);
                        phase = Phase::Nucleus;
                    } else if ch.is_ascii_alphabetic() {
                        let mut candidate: String = p
                            .units
                            .iter()
                            .filter(|u| u.phase == Phase::Onset)
                            .map(|u| u.char_no_tone())
                            .collect();
                        candidate.push(lower);
                        if tables::is_onset_prefix(&candidate) {
                            p.units.push(Unit::plain(ch, Phase::Onset));
                        } else {
                            p.broken = true;
                            phase = Phase::Tail;
                            push_tail(&mut p, ch);
                        }
                    } else {
                        p.broken = true;
                        phase = Phase::Tail;
                        push_tail(&mut p, ch);
                    }
                }
                Phase::Nucleus | Phase::Coda => {
                    let doubling = !k.literal
                        && is_vowel(lower)
                        && phase == Phase::Nucleus
                        && matches!(lower, 'a' | 'e' | 'o')
                        && p.units.last().is_some_and(|u| {
                            u.phase == Phase::Nucleus && u.base == lower && u.diac.is_none()
                        });
                    if doubling {
                        let n = p.units.len();
                        p.units[n - 1].diac = Some(Diac::Circumflex);
                        last_mod = Some(LastMod { ch: lower });
                    } else if is_vowel(lower) && phase == Phase::Nucleus {
                        push_vowel(&mut p, lower, ch.is_uppercase(), None);
                        last_mod = None;
                    } else if ch.is_ascii_alphabetic() {
                        let mut coda: String = p
                            .units
                            .iter()
                            .filter(|u| u.phase == Phase::Coda)
                            .map(|u| u.char_no_tone())
                            .collect();
                        coda.push(lower);
                        if tables::is_coda_prefix(&coda) {
                            p.units.push(Unit::plain(ch, Phase::Coda));
                            phase = Phase::Coda;
                            last_mod = None;
                        } else {
                            p.broken = true;
                            phase = Phase::Tail;
                            push_tail(&mut p, ch);
                            last_mod = None;
                        }
                    } else {
                        // A `[` or `]` that found no vowel to horn is kept as
                        // plain text without invalidating the word, just like
                        // a `z` that had no tone to remove.
                        let harmless = opts.method == Method::Telex
                            && matches!(lower, '[' | ']')
                            && phase != Phase::Junk;
                        if !harmless {
                            p.broken = true;
                        }
                        phase = if harmless { Phase::Junk } else { Phase::Tail };
                        push_tail(&mut p, ch);
                        last_mod = None;
                    }
                }
            }
        }

        if !p.broken {
            move_glide(&mut p);
            p.tone_at = tone_position(&p, opts);
        }
        p.last_mod = last_mod;
        p
    }
}

/// Whether re-typing this keystroke cancels the mark it produced.
fn is_cancellable(ch: char, opts: &Options) -> bool {
    let lower = ch.to_ascii_lowercase();
    match opts.method {
        Method::Telex => matches!(
            lower,
            'w' | 's' | 'f' | 'r' | 'x' | 'j' | 'a' | 'e' | 'o' | 'd' | '[' | ']'
        ),
        Method::Vni => matches!(
            lower,
            '1' | '2' | '3' | '4' | '5' | '6' | '7' | '8' | '9' | '0'
        ),
    }
}

fn push_vowel(p: &mut Parsed, base: char, upper: bool, diac: Option<Diac>) {
    p.units.push(Unit {
        base,
        upper,
        diac,
        phase: Phase::Nucleus,
    });
}

fn push_tail(p: &mut Parsed, ch: char) {
    p.units.push(Unit::plain(ch, Phase::Tail));
}

fn tone_allowed(p: &Parsed) -> bool {
    if p.broken {
        return false;
    }
    let onset: String = p
        .units
        .iter()
        .filter(|u| u.phase == Phase::Onset)
        .map(|u| u.char_no_tone())
        .collect();
    let coda: String = p
        .units
        .iter()
        .filter(|u| u.phase == Phase::Coda)
        .map(|u| u.char_no_tone())
        .collect();
    if !tables::is_onset(&onset) {
        return false;
    }
    if !coda.is_empty() && !tables::is_valid_coda(&coda) {
        return false;
    }
    p.units.iter().any(|u| u.phase == Phase::Nucleus)
}

/// `gi` and `qu` keep their glide in the onset (`giá`, `quá`).
fn move_glide(p: &mut Parsed) {
    let onset: String = p
        .units
        .iter()
        .filter(|u| u.phase == Phase::Onset)
        .map(|u| u.char_no_tone())
        .collect();
    let vowels = p.units.iter().filter(|u| u.phase == Phase::Nucleus).count();
    if vowels < 2 {
        return;
    }
    let first = match p.units.iter().find(|u| u.phase == Phase::Nucleus) {
        Some(u) => u.base,
        None => return,
    };
    let move_it = (onset == "g" && first == 'i') || (onset == "q" && first == 'u');
    if !move_it {
        return;
    }
    let idx = p
        .units
        .iter()
        .position(|u| u.phase == Phase::Nucleus)
        .unwrap();
    p.units[idx].phase = Phase::Onset;
}

/// Index of the vowel that carries the tone mark.
fn tone_position(p: &Parsed, opts: &Options) -> Option<usize> {
    let idxs: Vec<usize> = (0..p.units.len())
        .filter(|&i| p.units[i].phase == Phase::Nucleus)
        .collect();
    if idxs.is_empty() {
        return None;
    }
    // A vowel with a diacritic takes the mark; `ươ` takes it on the second one.
    if let Some(&i) = idxs.iter().rev().find(|&&i| p.units[i].diac.is_some()) {
        return Some(i);
    }
    if p.units.iter().any(|u| u.phase == Phase::Coda) {
        return idxs.last().copied();
    }
    match idxs.len() {
        1 => Some(idxs[0]),
        2 => {
            let first = p.units[idxs[0]].char_no_tone();
            let second = p.units[idxs[1]].char_no_tone();
            let modern_pair = matches!((first, second), ('o', 'a') | ('o', 'e') | ('u', 'y'));
            if opts.modern_style && modern_pair {
                Some(idxs[1])
            } else {
                Some(idxs[0])
            }
        }
        3 => Some(idxs[1]),
        _ => idxs.last().copied(),
    }
}

/// Split the rendered units into onset, nucleus, coda and literal tail.
fn split(p: &Parsed, toned: bool) -> (String, String, String, String) {
    let mut onset = String::new();
    let mut nucleus = String::new();
    let mut coda = String::new();
    let mut tail = String::new();
    for (i, u) in p.units.iter().enumerate() {
        let mut ch = u.char_no_tone();
        if toned && Some(i) == p.tone_at {
            ch = apply_tone(ch, p.tone);
        }
        if u.upper {
            ch = ch.to_uppercase().next().unwrap_or(ch);
        }
        match u.phase {
            Phase::Onset => onset.push(ch),
            Phase::Nucleus => nucleus.push(ch),
            Phase::Coda => coda.push(ch),
            Phase::Junk | Phase::Tail => tail.push(ch),
        }
    }
    (onset, nucleus, coda, tail)
}

fn render_text(p: &Parsed) -> String {
    let (on, nu, co, ta) = split(p, true);
    let mut out = String::with_capacity(on.len() + nu.len() + co.len() + ta.len() + 4);
    out.push_str(&on);
    out.push_str(&nu);
    out.push_str(&co);
    out.push_str(&ta);
    out
}

/// Rebuild the keystroke log that renders exactly to the given parts.
///
/// Used by `Backspace`, which operates on the rendered word: the surviving
/// text (tone marks included) is turned back into the keystrokes that would
/// type it.
fn rebuild_keys(onset: &str, nucleus: &str, coda: &str, tail: &str, opts: &Options) -> Vec<Key> {
    let mut keys: Vec<Key> = Vec::new();
    let mut tone = Tone::None;
    for ch in onset.chars().chain(nucleus.chars()).chain(coda.chars()) {
        match analyse(ch) {
            Some((base, upper, diac, t)) => {
                if t != Tone::None {
                    tone = t;
                }
                push_strokes(&mut keys, base, upper, diac, opts);
            }
            None => keys.push(Key { ch, literal: false }),
        }
    }
    for ch in tail.chars() {
        keys.push(Key { ch, literal: true });
    }
    if let Some(k) = tone_key(opts.method, tone) {
        // `z` removes a mark, it must never be used to rebuild one
        keys.push(Key {
            ch: k,
            literal: false,
        });
    }
    keys
}

/// Emit the keystrokes that type a single rendered character.
fn push_strokes(keys: &mut Vec<Key>, base: char, upper: bool, diac: Option<Diac>, opts: &Options) {
    let render_case = |c: char| if upper { c.to_ascii_uppercase() } else { c };
    let mut push = |c: char, literal: bool| keys.push(Key { ch: c, literal });
    if base == 'đ' {
        push(render_case('d'), false);
        match opts.method {
            Method::Telex => push(render_case('d'), false),
            Method::Vni => push('9', false),
        }
        return;
    }
    push(render_case(base), false);
    match (opts.method, diac) {
        (_, None) => {}
        (Method::Telex, Some(Diac::Circumflex)) => push(render_case(base), false),
        (Method::Telex, Some(Diac::Breve | Diac::Horn)) => push('w', false),
        (Method::Vni, Some(Diac::Circumflex)) => push('6', false),
        (Method::Vni, Some(Diac::Breve)) => push('8', false),
        (Method::Vni, Some(Diac::Horn)) => push('7', false),
    }
}

/// Whether this character can extend the word buffer.
pub fn is_word_char(ch: char, opts: &Options) -> bool {
    if ch.is_ascii_alphanumeric() {
        return true;
    }
    matches!(opts.method, Method::Telex) && matches!(ch, '[' | ']')
}

/// Whether this character ends the word (and is committed together with it).
pub fn is_terminator(ch: char) -> bool {
    ch == ' ' || (ch.is_ascii_punctuation() && !matches!(ch, '[' | ']'))
}

/// Non-printing keys the engine reacts to.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SpecialKey {
    /// Return, Tab, Home/End, arrows, PageUp/PageDown, Escape, Delete:
    /// commit what has been typed and let the application see the key.
    Navigation,
    /// Backspace.
    Backspace,
}

/// What the caller has to tell the input context after a keypress.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Action {
    /// Key not handled; the application should process it normally.
    Pass,
    /// Pre-edit changed; re-read [`Typing::display`].
    Update,
    /// Commit this text; the pre-edit is empty afterwards.  `consumed` tells
    /// whether the keystroke itself was swallowed.
    Commit { text: String, consumed: bool },
}

/// The typing state of one input context.
#[derive(Clone, Debug)]
pub struct Typing {
    word: Word,
    opts: Options,
}

impl Typing {
    pub fn new(opts: Options) -> Typing {
        Typing {
            word: Word::new(),
            opts,
        }
    }

    pub fn options(&self) -> &Options {
        &self.opts
    }

    pub fn set_options(&mut self, opts: Options) {
        self.opts = opts;
    }

    /// Text currently shown as pre-edit (empty when there is none).
    pub fn display(&self) -> String {
        if self.word.is_empty() {
            String::new()
        } else {
            self.word.render(&self.opts).text
        }
    }

    pub fn has_preedit(&self) -> bool {
        !self.word.is_empty() && !self.display().is_empty()
    }

    /// Text to commit at a word boundary, applying auto restore.
    pub fn commit_text(&self) -> String {
        self.word.commit_text(&self.opts)
    }

    pub fn clear(&mut self) {
        self.word.clear();
    }

    /// Handle a printable character.
    pub fn key_char(&mut self, ch: char) -> Action {
        if is_word_char(ch, &self.opts) {
            self.word.push(ch, &self.opts);
            Action::Update
        } else if is_terminator(ch) {
            if !self.has_preedit() {
                return Action::Pass;
            }
            let mut text = self.commit_text();
            text.push(ch);
            self.word.clear();
            Action::Commit {
                text,
                consumed: true,
            }
        } else {
            Action::Pass
        }
    }

    /// Handle one of the keys that end a word without being committed
    /// themselves, plus Backspace.
    pub fn key_special(&mut self, key: SpecialKey) -> Action {
        match key {
            SpecialKey::Navigation => {
                if !self.has_preedit() {
                    return Action::Pass;
                }
                let text = self.commit_text();
                self.word.clear();
                Action::Commit {
                    text,
                    consumed: false,
                }
            }
            SpecialKey::Backspace => {
                if !self.has_preedit() {
                    self.word.clear();
                    return Action::Pass;
                }
                if !self.word.backspace(&self.opts) {
                    self.word.clear();
                    return Action::Pass;
                }
                Action::Update
            }
        }
    }

    /// Commit a pending word, e.g. when focus is lost.
    pub fn flush(&mut self) -> Option<String> {
        if !self.has_preedit() {
            self.word.clear();
            return None;
        }
        let text = self.commit_text();
        self.word.clear();
        Some(text)
    }

    /// Drop a pending word without committing it.
    pub fn discard(&mut self) {
        self.word.clear();
    }
}

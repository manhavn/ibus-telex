//! Unit tests for the typing engine.
//!
//! The Unikey parity corpus covers Telex with the default options; these
//! tests cover the other input method and the options Unikey exposes.

use ibus_telex::vn::{Action, Method, Options, SpecialKey, Typing};

/// Type `keys` and return everything the input context would end up with.
fn type_text(keys: &str, opts: Options) -> String {
    let mut typing = Typing::new(opts);
    let mut out = String::new();
    for ch in keys.chars() {
        match typing.key_char(ch) {
            Action::Pass => out.push(ch),
            Action::Update => {}
            Action::Commit { text, .. } => out.push_str(&text),
        }
    }
    out.push_str(&typing.display());
    out
}

/// Pre-edit after typing `keys`.
fn preedit(keys: &str, opts: Options) -> String {
    let mut typing = Typing::new(opts);
    for ch in keys.chars() {
        typing.key_char(ch);
    }
    typing.display()
}

fn vni() -> Options {
    Options {
        method: Method::Vni,
        ..Options::default()
    }
}

fn modern() -> Options {
    Options {
        modern_style: true,
        ..Options::default()
    }
}

fn no_spell_check() -> Options {
    Options {
        spell_check: false,
        ..Options::default()
    }
}

#[test]
fn telex_words() {
    assert_eq!(type_text("tieengs", Options::default()), "tiếng");
    assert_eq!(type_text("Tieengs", Options::default()), "Tiếng");
    assert_eq!(type_text("VIEETJ", Options::default()), "VIỆT");
    assert_eq!(type_text("nuowcs", Options::default()), "nước");
    assert_eq!(type_text("dduowngf", Options::default()), "đường");
}

#[test]
fn tone_placement_old_and_modern() {
    // Unikey's default (and the corpus) is the traditional placement.
    assert_eq!(type_text("hoas", Options::default()), "hóa");
    assert_eq!(type_text("thuys", Options::default()), "thúy");
    // `oà`, `uý` instead of `òa`, `úy`.
    assert_eq!(type_text("hoaf", modern()), "hoà");
    assert_eq!(type_text("hoas", modern()), "hoá");
    assert_eq!(type_text("thuys", modern()), "thuý");
    assert_eq!(type_text("thuyr", modern()), "thuỷ");
    // A vowel with its own diacritic always carries the mark.
    assert_eq!(type_text("tuowngs", Options::default()), "tướng");
    assert_eq!(type_text("tieengs", Options::default()), "tiếng");
    // `qu` and `gi` keep their glide in the onset.
    assert_eq!(type_text("quas", Options::default()), "quá");
    assert_eq!(type_text("gias", Options::default()), "giá");
}

#[test]
fn vni_method() {
    assert_eq!(type_text("a1", vni()), "á");
    assert_eq!(type_text("a61", vni()), "ấ");
    assert_eq!(type_text("a81", vni()), "ắ");
    assert_eq!(type_text("o71", vni()), "ớ");
    assert_eq!(type_text("u71", vni()), "ứ");
    assert_eq!(type_text("d9", vni()), "đ");
    // the diacritic key follows the vowel it changes, like `w` in Telex
    assert_eq!(type_text("d9uo7ng2", vni()), "đường");
    assert_eq!(type_text("tie6ng1", vni()), "tiếng");
    assert_eq!(type_text("vie6t5", vni()), "việt");
    // `0` removes the tone mark
    assert_eq!(type_text("a10", vni()), "a");
}

/// A misspelled word must stay exactly as typed - no diacritic, no tone mark,
/// no half conversion.
#[test]
fn misspelled_words_stay_as_typed() {
    let opts = Options::default();
    // `k` is only written before i/y/e/ê in Vietnamese: `kof` is not `kò`,
    // it is not a word at all, so it stays `kof`.
    for (keys, want) in [
        ("kof", "kof"),
        ("kas", "kas"),
        ("kuw ", "kuw "),
        ("kom", "kom"),
        ("kaf", "kaf"),
        ("kas ", "kas "),
    ] {
        assert_eq!(type_text(keys, opts), want, "{keys} must not be converted");
    }
    // words that are Vietnamese do get their tone marks
    for (keys, want) in [
        ("kis", "kí"),
        ("kys", "ký"),
        ("kif", "kì"),
        ("kysf", "kỳ"),
        ("kes", "ké"),
        ("kees", "kế"),
        ("kins", "kín"),
        ("cas", "cá"),
        ("cof", "cò"),
    ] {
        assert_eq!(type_text(keys, opts), want, "{keys} must be Vietnamese");
    }
    // a stray character that is not part of Vietnamese takes the whole
    // conversion back
    for (keys, want) in [
        ("aaz ", "aaz "),
        ("ooz ", "ooz "),
        ("awz ", "awz "),
        ("tiez ", "tiez "),
        ("tiengs ", "tiengs "),
        ("hello ", "hello "),
    ] {
        assert_eq!(type_text(keys, opts), want, "{keys} must stay as typed");
    }
    // ... but spelling `đ` is not a diacritic and stays
    assert_eq!(type_text("dd", opts), "đ");
    assert_eq!(type_text("ddi", opts), "đi");

    // The pre-edit is optimistic while typing, exactly like Unikey's; it is
    // the commit that refuses to hand a misspelled word over converted.
    let mut typing = Typing::new(opts);
    for c in "kuw".chars() {
        typing.key_char(c);
    }
    assert_eq!(typing.display(), "kư");
    assert_eq!(typing.flush().as_deref(), Some("kuw"));
}

#[test]
fn spell_check_can_be_switched_off() {
    // Invalid word: restored to the keystrokes with spell checking on ...
    assert_eq!(type_text("ook ", Options::default()), "ook ");
    // ... and kept as typed with it off.
    assert_eq!(type_text("ook ", no_spell_check()), "ôk ");
}

#[test]
fn free_marking_can_be_switched_off() {
    // The tone key may sit before the final consonant only when free marking
    // is on: `taosn` is `táo` + `n` with it, a broken word without it.
    assert_eq!(preedit("taosn", Options::default()), "taón");
    let strict = Options {
        free_marking: false,
        ..Options::default()
    };
    assert_eq!(preedit("taosn", strict), "taosn");
    // at the end of the word the tone key is always accepted
    assert_eq!(preedit("toans", strict), "toán");
}

#[test]
fn standalone_w_is_optional() {
    assert_eq!(preedit("w", Options::default()), "ư");
    let no_standalone = Options {
        standalone_w: false,
        ..Options::default()
    };
    assert_eq!(preedit("w", no_standalone), "w");
    // `uw` still produces ư, and `[`/`]` still work.
    assert_eq!(preedit("uw", no_standalone), "ư");
    assert_eq!(preedit("[", no_standalone), "ơ");
}

#[test]
fn backspace_removes_one_rendered_character() {
    let mut typing = Typing::new(Options::default());
    for c in "tieengs".chars() {
        typing.key_char(c);
    }
    assert_eq!(typing.display(), "tiếng");
    typing.key_special(SpecialKey::Backspace);
    // the tone survives deleting the final consonant
    assert_eq!(typing.display(), "tiến");
    typing.key_special(SpecialKey::Backspace);
    assert_eq!(typing.display(), "tiế");
    typing.key_special(SpecialKey::Backspace);
    // deleting the vowel the tone sat on removes the tone with it
    assert_eq!(typing.display(), "ti");
    // `aw` renders as one character, so one backspace deletes both keys
    let mut typing = Typing::new(Options::default());
    typing.key_char('a');
    typing.key_char('w');
    assert_eq!(typing.display(), "ă");
    typing.key_special(SpecialKey::Backspace);
    assert_eq!(typing.display(), "");
}

#[test]
fn retyping_a_modifier_cancels_it() {
    for (keys, want) in [
        ("aaa", "aa"),
        ("ass", "as"),
        ("aww", "aw"),
        ("ooo", "oo"),
        ("ddd", "dd"),
        ("uww", "uw"),
        // `z` removes the tone mark of the word
        ("asz", "a"),
        ("tieengsz", "tiêng"),
    ] {
        assert_eq!(preedit(keys, Options::default()), want, "preedit of {keys}");
    }
}

#[test]
fn navigation_commits_and_passes_the_key_through() {
    let mut typing = Typing::new(Options::default());
    for c in "vieetj".chars() {
        typing.key_char(c);
    }
    match typing.key_special(SpecialKey::Navigation) {
        Action::Commit { text, consumed } => {
            assert_eq!(text, "việt");
            assert!(!consumed, "the application still gets the key");
        }
        other => panic!("expected a commit, got {other:?}"),
    }
    assert_eq!(typing.display(), "");
}

#[test]
fn an_empty_word_lets_the_space_through() {
    let mut typing = Typing::new(Options::default());
    assert_eq!(typing.key_char(' '), Action::Pass);
}

#[test]
fn focus_loss_commits_a_pending_word() {
    let mut typing = Typing::new(Options::default());
    for c in "dd".chars() {
        typing.key_char(c);
    }
    assert_eq!(typing.flush().as_deref(), Some("đ"));
    assert_eq!(typing.flush(), None);
}

/// `discard` is the opposite of `flush`; the engine itself commits on Reset.
#[test]
fn discard_drops_a_pending_word() {
    let mut typing = Typing::new(Options::default());
    for c in "dd".chars() {
        typing.key_char(c);
    }
    typing.discard();
    assert_eq!(typing.display(), "");
}

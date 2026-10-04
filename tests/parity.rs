//! Behaviour parity with ibus-unikey.
//!
//! `parity_corpus.tsv` holds key sequences whose result was captured by
//! driving the real ibus-unikey 0.7.0 engine over D-Bus (see
//! docs/unikey-parity.md).  Every line is replayed through this engine the
//! same way an input context would and the text the user would end up with
//! has to match exactly.

use ibus_telex::vn::{Action, Options, SpecialKey, Typing};

fn feed(t: &mut Typing, ch: char, out: &mut String) {
    match t.key_char(ch) {
        // The application handles keys the engine declines.
        Action::Pass => out.push(ch),
        Action::Update => {}
        Action::Commit { text, .. } => out.push_str(&text),
    }
}

/// Replay one corpus line: type the keys, then a space to close the word.
fn replay(keys: &str) -> String {
    let mut typing = Typing::new(Options::default());
    let mut out = String::new();
    for token in keys.split(' ') {
        let (shift, base) = match token.strip_prefix('S') {
            Some(rest) if rest.chars().count() == 1 => (true, rest),
            _ => (false, token),
        };
        match base {
            "BACK" => {
                if let Action::Commit { text, .. } = typing.key_special(SpecialKey::Backspace) {
                    out.push_str(&text);
                }
            }
            "ESC" | "RET" | "TAB" | "LEFT" | "RIGHT" | "UP" | "DOWN" | "HOME" | "END" | "PGUP"
            | "PGDN" | "DEL" | "INSERT" => {
                if let Action::Commit { text, .. } = typing.key_special(SpecialKey::Navigation) {
                    out.push_str(&text);
                }
            }
            "SPACE" | "space" => feed(&mut typing, ' ', &mut out),
            other => {
                let ch = other.chars().next().expect("token");
                let ch = if shift { ch.to_ascii_uppercase() } else { ch };
                feed(&mut typing, ch, &mut out);
            }
        }
    }
    feed(&mut typing, ' ', &mut out);
    // the harness strips the single trailing space that closed the word
    if out.ends_with(' ') {
        out.pop();
    }
    out
}

#[test]
fn matches_ibus_unikey() {
    let corpus = include_str!("parity_corpus.tsv");
    let mut failures = Vec::new();
    let mut checked = 0;
    for (n, line) in corpus.lines().enumerate() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (keys, expected) = line.split_once('\t').expect("corpus line has two columns");
        checked += 1;
        let got = replay(keys);
        if got != expected {
            failures.push(format!(
                "line {}: {keys}\n    expected {:?}\n    got      {:?}",
                n + 1,
                expected,
                got
            ));
        }
    }
    assert!(checked > 200, "corpus looks truncated ({checked} cases)");
    assert!(
        failures.is_empty(),
        "{} of {} cases differ from ibus-unikey:\n{}",
        failures.len(),
        checked,
        failures.join("\n")
    );
}

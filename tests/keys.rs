//! Modifier policy.
//!
//! A keystroke must only be treated as an application shortcut when the user
//! is actually asking for one.  The state word carries a lot of other bits -
//! Num Lock alone sets `MOD2` on every single keystroke - and an engine that
//! is too eager stops typing altogether: with Num Lock on (the default on
//! many keyboards) nothing would reach the pre-edit.
//!
//! ibus-table, the reference engine shipped with IBus, only ever checks
//! `CONTROL_MASK` and `MOD1_MASK`.

use ibus_telex::ibus::keys;

#[test]
fn state_bits_that_must_not_block_typing() {
    let harmless = [
        keys::SHIFT_MASK, // typing upper case
        keys::LOCK_MASK,  // Caps Lock
        keys::MOD2_MASK,  // Num Lock - set on every key while it is on
        keys::MOD3_MASK,  // Scroll Lock on some layouts
        keys::MOD5_MASK,  // AltGr
    ];
    for mask in harmless {
        assert!(
            !keys::is_shortcut(mask),
            "{mask:#x} must not be treated as a shortcut"
        );
    }
    // A completely ordinary desktop: Num Lock on, Caps Lock on.
    assert!(!keys::is_shortcut(
        keys::MOD2_MASK | keys::LOCK_MASK | keys::SHIFT_MASK
    ));
}

#[test]
fn keypad_keys_are_not_their_ascii_equivalents() {
    // With Num Lock on the keypad sends its own keysyms: `KP_5` is 0xffb5,
    // not `5`.  An engine that does not translate them never sees a keypad
    // digit, the application inserts it - in front of the pre-edit.
    assert_eq!(keys::char_from_keyval(0xffb5), Some('5'));
    assert_eq!(keys::char_from_keyval(0xffb0), Some('0'));
    assert_eq!(keys::char_from_keyval(0xffb9), Some('9'));
    assert_eq!(keys::char_from_keyval(0xffad), Some('-'));
    assert_eq!(keys::char_from_keyval(0xffab), Some('+'));
    assert_eq!(keys::char_from_keyval(0xffae), Some('.'));
    assert_eq!(keys::char_from_keyval(0xffaf), Some('/'));
    assert_eq!(keys::char_from_keyval(0xffbd), Some('='));
    // the number row is unchanged
    assert_eq!(keys::char_from_keyval('5' as u32), Some('5'));
}

#[test]
fn keypad_without_num_lock_is_navigation() {
    for kv in [
        keys::KEY_KP_HOME,
        keys::KEY_KP_LEFT,
        keys::KEY_KP_UP,
        keys::KEY_KP_RIGHT,
        keys::KEY_KP_DOWN,
        keys::KEY_KP_PAGE_UP,
        keys::KEY_KP_PAGE_DOWN,
        keys::KEY_KP_END,
        keys::KEY_KP_INSERT,
        keys::KEY_KP_DELETE,
    ] {
        assert!(keys::is_navigation(kv), "{kv:#x} must be navigation");
    }
    assert!(!keys::is_navigation(0xffb5), "KP_5 is a digit");
}

#[test]
fn application_shortcuts_still_pass_through() {
    let shortcuts = [
        keys::CONTROL_MASK,
        keys::MOD1_MASK,
        keys::MOD4_MASK,
        keys::SUPER_MASK,
    ];
    for mask in shortcuts {
        assert!(
            keys::is_shortcut(mask),
            "{mask:#x} must be treated as a shortcut"
        );
    }
    // Control+Shift+key is still a shortcut.
    assert!(keys::is_shortcut(keys::CONTROL_MASK | keys::SHIFT_MASK));
    // Key releases are never ours.
    assert!(keys::is_release(keys::RELEASE_MASK));
}

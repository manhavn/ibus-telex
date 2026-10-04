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

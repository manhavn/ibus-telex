//! Key values and modifier masks used by the IBus protocol.
//!
//! The values match `IBusModifierType` and `IBus.keyval` as used by
//! libibus 1.5.x on X11/Wayland.

pub const SHIFT_MASK: u32 = 1 << 0;
pub const LOCK_MASK: u32 = 1 << 1;
pub const CONTROL_MASK: u32 = 1 << 2;
pub const MOD1_MASK: u32 = 1 << 3;
pub const MOD2_MASK: u32 = 1 << 4;
pub const MOD3_MASK: u32 = 1 << 5;
pub const MOD4_MASK: u32 = 1 << 6;
pub const MOD5_MASK: u32 = 1 << 7;
pub const HANDLED_MASK: u32 = 1 << 24;
pub const FORWARD_MASK: u32 = 1 << 25;
pub const SUPER_MASK: u32 = 1 << 26;
pub const HYPER_MASK: u32 = 1 << 27;
pub const META_MASK: u32 = 1 << 28;
pub const RELEASE_MASK: u32 = 1 << 30;

/// The modifiers that mean the user is asking for an application shortcut.
///
/// Note what is *not* here: `MOD2` is Num Lock and is set on every keystroke
/// while it is on, `MOD5` is AltGr, `MOD3` shows up as Scroll Lock, and
/// `LOCK`/`SHIFT` are just typing.  Treating any of those as a shortcut
/// makes the engine hand every key back to the application, which is what
/// Num Lock on a normal desktop used to do.  ibus-table, the reference
/// engine, only ever looks at Control and Alt.
const SHORTCUT: u32 = CONTROL_MASK | MOD1_MASK | MOD4_MASK | SUPER_MASK | HYPER_MASK | META_MASK;

/// Capabilities the input context may announce.
pub const CAP_PREEDIT_TEXT: u32 = 1 << 0;
pub const CAP_AUXILIARY_TEXT: u32 = 1 << 1;
pub const CAP_LOOKUP_TABLE: u32 = 1 << 2;
pub const CAP_FOCUS: u32 = 1 << 3;
pub const CAP_PROPERTY: u32 = 1 << 4;
pub const CAP_SURROUNDING_TEXT: u32 = 1 << 5;
pub const CAP_SYNC_PROCESS_KEY: u32 = 1 << 7;

/// Pre-edit focus modes.
pub const PREEDIT_CLEAR: u32 = 0;
pub const PREEDIT_COMMIT: u32 = 1;

/// Input purposes we keep our hands off.
pub const PURPOSE_PASSWORD: u32 = 8;
pub const PURPOSE_PIN: u32 = 9;

pub const KEY_BACKSPACE: u32 = 0xff08;
pub const KEY_TAB: u32 = 0xff09;
pub const KEY_RETURN: u32 = 0xff0d;
pub const KEY_KP_ENTER: u32 = 0xff8d;
pub const KEY_ESCAPE: u32 = 0xff1b;
pub const KEY_HOME: u32 = 0xff50;
pub const KEY_LEFT: u32 = 0xff51;
pub const KEY_UP: u32 = 0xff52;
pub const KEY_RIGHT: u32 = 0xff53;
pub const KEY_DOWN: u32 = 0xff54;
pub const KEY_PAGE_UP: u32 = 0xff55;
pub const KEY_PAGE_DOWN: u32 = 0xff56;
pub const KEY_END: u32 = 0xff57;
pub const KEY_INSERT: u32 = 0xff63;
pub const KEY_DELETE: u32 = 0xffff;

/// True when the keystroke carries a modifier that makes it a shortcut for
/// the application (Control, Alt, Super, Hyper, Meta).
pub fn is_shortcut(state: u32) -> bool {
    state & SHORTCUT != 0
}

pub fn is_release(state: u32) -> bool {
    state & RELEASE_MASK != 0
}

pub fn shift_pressed(state: u32) -> bool {
    state & SHIFT_MASK != 0
}

/// Keys that make the engine commit the pending word and then let the
/// application handle the key.
pub fn is_navigation(keyval: u32) -> bool {
    matches!(
        keyval,
        KEY_TAB
            | KEY_RETURN
            | KEY_KP_ENTER
            | KEY_ESCAPE
            | KEY_HOME
            | KEY_LEFT
            | KEY_UP
            | KEY_RIGHT
            | KEY_DOWN
            | KEY_PAGE_UP
            | KEY_PAGE_DOWN
            | KEY_END
            | KEY_INSERT
            | KEY_DELETE
    )
}

/// Translate a key value into the character it produces, if it is one we
/// can type with.
pub fn char_from_keyval(keyval: u32) -> Option<char> {
    // Latin-1 keysyms are their Unicode code point.
    if (0x20..=0x7e).contains(&keyval) {
        char::from_u32(keyval)
    } else {
        None
    }
}

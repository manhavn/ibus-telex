//! Serialisation of the `IBus*` container objects.
//!
//! IBus smuggles its GObject types over D-Bus as structs of the shape
//! `(s a{sv} <fields...>)` where the first member is the type name and the
//! second is the attachment dictionary.  The signatures below were taken
//! from a live `org.freedesktop.IBus.Engine` instance (ibus-unikey 0.7.0):
//!
//! ```text
//! IBusText       (sa{sv}sv)
//! IBusAttrList   (sa{sv}av)
//! IBusAttribute  (sa{sv}uuuu)
//! IBusPropList   (sa{sv}av)
//! IBusProperty   (sa{sv}suvsvbbuvv)
//! ```

use std::collections::HashMap;

use zbus::zvariant::Value;

/// The attachment dictionary of an IBus object; always empty here.
pub type Attachments = HashMap<String, Value<'static>>;

/// `IBusAttrType`
const ATTR_UNDERLINE: u32 = 1;
/// `IBusAttrUnderline`
const UNDERLINE_SINGLE: u32 = 1;

/// `IBusPropType`
pub const PROP_NORMAL: u32 = 0;
pub const PROP_TOGGLE: u32 = 1;
pub const PROP_RADIO: u32 = 2;
pub const PROP_MENU: u32 = 3;
pub const PROP_SEPARATOR: u32 = 4;

/// `IBusPropState`
pub const PROP_STATE_UNCHECKED: u32 = 0;
pub const PROP_STATE_CHECKED: u32 = 1;

fn attrs() -> Attachments {
    Attachments::new()
}

/// An `IBusText` with a single underline attribute covering all of `text`,
/// which is how Unikey draws its pre-edit.
pub fn text(text: &str) -> Value<'static> {
    let len = text.chars().count() as u32;
    let attr_list = if len == 0 {
        ("IBusAttrList", attrs(), Vec::<Value<'static>>::new())
    } else {
        let attr = (
            "IBusAttribute",
            attrs(),
            ATTR_UNDERLINE,
            UNDERLINE_SINGLE,
            0u32,
            len,
        );
        ("IBusAttrList", attrs(), vec![Value::new(attr)])
    };
    Value::new(("IBusText", attrs(), text.to_owned(), Value::new(attr_list)))
}

/// A plain `IBusText` without attributes, used for labels.
fn label(text: &str) -> Value<'static> {
    let attr_list = ("IBusAttrList", attrs(), Vec::<Value<'static>>::new());
    Value::new(("IBusText", attrs(), text.to_owned(), Value::new(attr_list)))
}

/// An `IBusPropList` holding `props`.
pub fn prop_list(props: Vec<Value<'static>>) -> Value<'static> {
    Value::new(("IBusPropList", attrs(), props))
}

/// Build one `IBusProperty`.
pub fn property(
    key: &str,
    kind: u32,
    label_text: &str,
    tooltip: &str,
    state: u32,
    sub_props: Value<'static>,
    sensitive: bool,
) -> Value<'static> {
    Value::new((
        "IBusProperty",
        attrs(),
        key.to_owned(),
        kind,
        label(label_text),
        String::new(),
        label(tooltip),
        sensitive,
        true,
        state,
        sub_props,
        label(""),
    ))
}

/// A toggle property.
pub fn toggle(key: &str, label_text: &str, tooltip: &str, on: bool) -> Value<'static> {
    property(
        key,
        PROP_TOGGLE,
        label_text,
        tooltip,
        if on {
            PROP_STATE_CHECKED
        } else {
            PROP_STATE_UNCHECKED
        },
        prop_list(Vec::new()),
        true,
    )
}

/// A radio entry; used inside a menu.
pub fn radio(key: &str, label_text: &str, selected: bool) -> Value<'static> {
    property(
        key,
        PROP_RADIO,
        label_text,
        "",
        if selected {
            PROP_STATE_CHECKED
        } else {
            PROP_STATE_UNCHECKED
        },
        prop_list(Vec::new()),
        true,
    )
}

/// A menu holding radio entries.
pub fn menu(key: &str, label_text: &str, entries: Vec<Value<'static>>) -> Value<'static> {
    property(
        key,
        PROP_MENU,
        label_text,
        "",
        PROP_STATE_UNCHECKED,
        prop_list(entries),
        true,
    )
}

/// A separator entry.
pub fn separator() -> Value<'static> {
    property(
        "",
        PROP_SEPARATOR,
        "",
        "",
        PROP_STATE_UNCHECKED,
        prop_list(Vec::new()),
        true,
    )
}

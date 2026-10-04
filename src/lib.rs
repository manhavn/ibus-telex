//! `ibus-telex` - a Vietnamese input method engine for IBus.
//!
//! The crate is split into
//!
//! * [`vn`] - the typing engine (Telex and VNI) that turns keystrokes into
//!   Vietnamese text, and
//! * [`ibus`] - the D-Bus side that speaks the `org.freedesktop.IBus.Engine`
//!   protocol directly, so the engine needs neither libibus nor any C
//!   dependency.

pub mod cli;
pub mod config;
pub mod ibus;
pub mod vn;

/// Name of the well known bus name the engine requests.
pub const BUS_NAME: &str = "org.freedesktop.IBus.Telex";

/// Engine (input source) name shown in the IBus panel.
pub const ENGINE_NAME: &str = "Telex";

/// Path of the component file inside the user's data dir.
pub const COMPONENT_FILE: &str = "ibus/component/telex.xml";

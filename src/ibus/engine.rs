//! The `org.freedesktop.IBus.Engine` object and its factory.
//!
//! IBus engines are ordinary D-Bus services on the private bus named by
//! `IBUS_ADDRESS`: the engine process owns `org.freedesktop.IBus.<Engine>`,
//! exports a factory at `/org/freedesktop/IBus/Factory` and lets the daemon
//! create one engine object per input context through `CreateEngine`.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use parking_lot::Mutex;

use zbus::fdo::{RequestNameFlags, RequestNameReply};
use zbus::object_server::ObjectServer;
use zbus::zvariant::{OwnedObjectPath, Value};
use zbus::Connection;

use super::{keys, object};
use crate::config::Config;
use crate::vn::{Action, SpecialKey, Typing};
use crate::{BUS_NAME, ENGINE_NAME};

pub const ENGINE_IFACE: &str = "org.freedesktop.IBus.Engine";
pub const FACTORY_IFACE: &str = "org.freedesktop.IBus.Factory";
pub const SERVICE_IFACE: &str = "org.freedesktop.IBus.Service";
pub const FACTORY_PATH: &str = "/org/freedesktop/IBus/Factory";

/// Options shared by every engine object of this process.
pub struct Shared {
    pub config: Mutex<Config>,
}

impl Shared {
    pub fn new() -> Arc<Shared> {
        Arc::new(Shared {
            config: Mutex::new(Config::load()),
        })
    }
}

struct State {
    typing: Typing,
    caps: u32,
    purpose: u32,
    hints: u32,
    enabled: bool,
    focused: bool,
    cursor: (i32, i32, i32, i32),
}

impl State {
    /// Passwords and PINs never get Vietnamese input, and a password field
    /// is exactly where a leaking pre-edit hurts most.
    fn is_secret(&self) -> bool {
        matches!(self.purpose, keys::PURPOSE_PASSWORD | keys::PURPOSE_PIN)
    }
}

/// One engine object, i.e. one input context.
pub struct Engine {
    conn: Connection,
    path: OwnedObjectPath,
    shared: Arc<Shared>,
    state: Mutex<State>,
}

impl Engine {
    fn new(conn: Connection, path: OwnedObjectPath, shared: Arc<Shared>) -> Engine {
        let options = shared.config.lock().options;
        Engine {
            conn,
            path,
            shared,
            state: Mutex::new(State {
                typing: Typing::new(options),
                caps: 0,
                purpose: 0,
                hints: 0,
                enabled: false,
                focused: false,
                cursor: (0, 0, 0, 0),
            }),
        }
    }

    fn signal_path(&self) -> &str {
        self.path.as_str()
    }

    /// Show or hide the pre-edit.
    ///
    /// The focus mode is `COMMIT`, like Unikey publishes: when the input
    /// context loses focus with a word half typed, the *client* commits the
    /// pre-edit text it is holding.  That is the only copy that still counts
    /// at that moment - a commit this engine sends after the context has
    /// been unfocused never reaches it, which is why committing from here
    /// left the word on the floor whenever the user clicked another window.
    async fn emit_preedit(&self, text: &str) {
        let cursor = text.chars().count() as u32;
        let body = (
            object::text(text),
            cursor,
            !text.is_empty(),
            keys::PREEDIT_COMMIT,
        );
        if let Err(e) = self
            .conn
            .emit_signal(
                None::<&str>,
                self.signal_path(),
                ENGINE_IFACE,
                "UpdatePreeditText",
                &body,
            )
            .await
        {
            warn(&format!("UpdatePreeditText failed: {e}"));
        }
    }

    async fn emit_commit(&self, text: &str) {
        debug_event(&format!("committing {text:?}"));
        if let Err(e) = self
            .conn
            .emit_signal(
                None::<&str>,
                self.signal_path(),
                ENGINE_IFACE,
                "CommitText",
                &(object::text(text),),
            )
            .await
        {
            warn(&format!("CommitText failed: {e}"));
        }
        self.emit_preedit("").await;
    }

    fn properties(&self) -> Value<'static> {
        let o = {
            let st = self.state.lock();
            *st.typing.options()
        };
        object::prop_list(vec![
            object::menu(
                "input-method",
                "Input method",
                vec![
                    object::radio(
                        "input-method:telex",
                        "Telex",
                        o.method == crate::vn::Method::Telex,
                    ),
                    object::radio(
                        "input-method:vni",
                        "VNI",
                        o.method == crate::vn::Method::Vni,
                    ),
                ],
            ),
            object::separator(),
            object::toggle(
                "spell-check",
                "Spell check",
                "Only accept syllables that exist in Vietnamese",
                o.spell_check,
            ),
            object::toggle(
                "modern-style",
                "Modern tone style",
                "oà, uý instead of òa, úy",
                o.modern_style,
            ),
            object::toggle(
                "free-marking",
                "Free tone marking",
                "Allow the tone key before the final consonant",
                o.free_marking,
            ),
            object::toggle(
                "standalone-w",
                "Standalone W as ư",
                "Telex only: a lone w produces ư",
                o.standalone_w,
            ),
        ])
    }

    async fn emit_properties(&self) {
        let props = self.properties();
        if let Err(e) = self
            .conn
            .emit_signal(
                None::<&str>,
                self.signal_path(),
                ENGINE_IFACE,
                "RegisterProperties",
                &(props,),
            )
            .await
        {
            warn(&format!("RegisterProperties failed: {e}"));
        }
    }

    async fn emit_property(&self, prop: Value<'static>) {
        if let Err(e) = self
            .conn
            .emit_signal(
                None::<&str>,
                self.signal_path(),
                ENGINE_IFACE,
                "UpdateProperty",
                &(prop,),
            )
            .await
        {
            warn(&format!("UpdateProperty failed: {e}"));
        }
    }

    /// Turn the outcome of the typing engine into D-Bus traffic.
    async fn apply(&self, action: Action) -> bool {
        match action {
            Action::Pass => false,
            Action::Update => {
                let text = {
                    let st = self.state.lock();
                    st.typing.display()
                };
                self.emit_preedit(&text).await;
                true
            }
            Action::Commit { text, consumed } => {
                self.emit_commit(&text).await;
                consumed
            }
        }
    }

    /// Refresh this engine's options from the shared configuration.
    fn sync_options(&self) {
        let options = self.shared.config.lock().options;
        let mut st = self.state.lock();
        if *st.typing.options() != options {
            st.typing.set_options(options);
        }
    }

    async fn activate_property(&self, name: &str, _state: u32) {
        let changed: Vec<Value<'static>>;
        {
            let mut cfg = self.shared.config.lock();
            let o = &mut cfg.options;
            match name {
                "input-method:telex" => o.method = crate::vn::Method::Telex,
                "input-method:vni" => o.method = crate::vn::Method::Vni,
                "spell-check" => o.spell_check = !o.spell_check,
                "modern-style" => o.modern_style = !o.modern_style,
                "free-marking" => o.free_marking = !o.free_marking,
                "standalone-w" => o.standalone_w = !o.standalone_w,
                _ => return,
            }
            let o = *o;
            let _ = cfg.save();
            let mut st = self.state.lock();
            // A pending word was rendered with the old settings.
            st.typing.discard();
            st.typing.set_options(o);
            changed = match name {
                "input-method:telex" | "input-method:vni" => vec![
                    object::radio(
                        "input-method:telex",
                        "Telex",
                        o.method == crate::vn::Method::Telex,
                    ),
                    object::radio(
                        "input-method:vni",
                        "VNI",
                        o.method == crate::vn::Method::Vni,
                    ),
                ],
                "spell-check" => vec![object::toggle(
                    "spell-check",
                    "Spell check",
                    "Only accept syllables that exist in Vietnamese",
                    o.spell_check,
                )],
                "modern-style" => vec![object::toggle(
                    "modern-style",
                    "Modern tone style",
                    "oà, uý instead of òa, úy",
                    o.modern_style,
                )],
                "free-marking" => vec![object::toggle(
                    "free-marking",
                    "Free tone marking",
                    "Allow the tone key before the final consonant",
                    o.free_marking,
                )],
                "standalone-w" => vec![object::toggle(
                    "standalone-w",
                    "Standalone W as ư",
                    "Telex only: a lone w produces ư",
                    o.standalone_w,
                )],
                _ => Vec::new(),
            };
        }
        self.emit_preedit("").await;
        for prop in changed {
            self.emit_property(prop).await;
        }
    }

    /// Forget the word and hide the pre-edit.
    ///
    /// Used when the client is the one that commits: it holds the pre-edit
    /// and commits it itself, so the engine only has to drop its own copy.
    async fn drop_preedit(&self) {
        {
            let mut st = self.state.lock();
            st.typing.discard();
        }
        self.emit_preedit("").await;
    }

    /// Commit the word if there is one, and do nothing otherwise.
    ///
    /// Used when the user reaches for something else - a shortcut, Alt, the
    /// overview - so the half typed word is finished instead of left hanging.
    async fn commit_pending(&self) {
        let text = {
            let mut st = self.state.lock();
            st.typing.flush()
        };
        if let Some(text) = text {
            self.emit_commit(&text).await;
        }
    }

    /// The key handling proper; `process_key_event` wraps it with logging.
    async fn handle_key(&self, keyval: u32, keycode: u32, state: u32) -> bool {
        if keys::is_release(state) {
            return false;
        }
        if keys::is_shortcut(state) || keys::is_modifier_key(keyval) {
            // Never swallow application shortcuts (Ctrl+C, Alt+Tab, ...), but
            // do finish the word first: the user is reaching for something
            // else, and a half typed word must not be left hanging.
            self.commit_pending().await;
            return false;
        }
        let _ = keycode;
        self.sync_options();

        if keyval == keys::KEY_BACKSPACE {
            let action = {
                let mut st = self.state.lock();
                st.typing.key_special(SpecialKey::Backspace)
            };
            return self.apply(action).await;
        }
        if keys::is_navigation(keyval) {
            let action = {
                let mut st = self.state.lock();
                st.typing.key_special(SpecialKey::Navigation)
            };
            return self.apply(action).await;
        }

        let mut ch = match keys::char_from_keyval(keyval) {
            Some(c) => c,
            None => return false,
        };
        if keys::shift_pressed(state) && ch.is_ascii_lowercase() {
            ch = ch.to_ascii_uppercase();
        }
        let action = {
            let mut st = self.state.lock();
            if st.is_secret() {
                return false;
            }
            st.typing.key_char(ch)
        };
        self.apply(action).await
    }
}

#[zbus::interface(name = "org.freedesktop.IBus.Engine")]
impl Engine {
    async fn process_key_event(&self, keyval: u32, keycode: u32, state: u32) -> bool {
        let handled = self.handle_key(keyval, keycode, state).await;
        debug_key(keyval, state, handled);
        handled
    }

    async fn set_cursor_location(&self, x: i32, y: i32, w: i32, h: i32) {
        self.state.lock().cursor = (x, y, w, h);
    }

    async fn set_capabilities(&self, caps: u32) {
        self.state.lock().caps = caps;
    }

    async fn set_surrounding_text(&self, text: Value<'_>, cursor_pos: u32, anchor_pos: u32) {
        // Surrounding text is deliberately ignored: the engine never issues
        // DeleteSurroundingText, which is the one thing that goes badly wrong
        // with clients that report stale text (common on Wayland).
        let _ = (text, cursor_pos, anchor_pos);
    }

    async fn focus_in(&self) {
        debug_event("FocusIn");
        self.state.lock().focused = true;
    }

    async fn focus_in_id(&self, object_path: String, client: String) {
        debug_event(&format!("FocusInId {object_path} {client}"));
        let _ = (object_path, client);
        self.focus_in().await;
    }

    async fn focus_out(&self) {
        debug_event("FocusOut");
        {
            let mut st = self.state.lock();
            st.focused = false;
        }
        // The client commits the pre-edit (the mode says COMMIT); here the
        // engine only drops its own copy so the next word starts clean.
        self.drop_preedit().await;
    }

    async fn focus_out_id(&self, object_path: String) {
        debug_event(&format!("FocusOutId {object_path}"));
        let _ = object_path;
        self.focus_out().await;
    }

    async fn reset(&self) {
        debug_event("Reset");
        // Clear, do not commit: this is what the client sends when it clears
        // the pre-edit on its own (a mouse click at another position in the
        // text field, or a key it handles itself), and what every other
        // engine does with it - ibus-table's reset() is documented as "clear
        // the preëdit".
        //
        // Committing here was a mistake: on a window switch the reset arrives
        // while the pre-edit is still pending, and the commit it produced was
        // delivered to the *newly focused* window, so the word showed up
        // twice.  What protects a half typed word across a focus change is
        // the COMMIT focus mode on the pre-edit (see `emit_preedit`), which
        // makes the client commit the text it is holding.
        self.drop_preedit().await;
    }

    async fn enable(&self) {
        debug_event("Enable");
        self.state.lock().enabled = true;
        self.emit_properties().await;
        self.emit_preedit("").await;
    }

    async fn disable(&self) {
        debug_event("Disable");
        {
            let mut st = self.state.lock();
            st.enabled = false;
        }
        self.drop_preedit().await;
    }

    async fn set_content_type(&self, purpose: u32, hints: u32) {
        debug_event(&format!("SetContentType purpose={purpose} hints={hints}"));
        let secret = matches!(purpose, keys::PURPOSE_PASSWORD | keys::PURPOSE_PIN);
        let was_secret = {
            let mut st = self.state.lock();
            let was = st.is_secret();
            st.purpose = purpose;
            st.hints = hints;
            was
        };
        if secret && !was_secret {
            let text = {
                let mut st = self.state.lock();
                st.typing.flush()
            };
            if let Some(text) = text {
                self.emit_commit(&text).await;
            }
        }
    }

    async fn get_content_type(&self) -> (u32, u32) {
        let st = self.state.lock();
        (st.purpose, st.hints)
    }

    async fn property_activate(&self, name: String, state: u32) {
        self.activate_property(&name, state).await;
    }

    async fn property_show(&self, name: String) {
        let _ = name;
    }

    async fn property_hide(&self, name: String) {
        let _ = name;
    }

    async fn candidate_clicked(&self, index: u32, button: u32, state: u32) {
        let _ = (index, button, state);
    }

    async fn page_up(&self) {}

    async fn page_down(&self) {}

    async fn cursor_up(&self) {}

    async fn cursor_down(&self) {}

    async fn process_hand_writing_event(&self, coordinates: Vec<f64>) {
        let _ = coordinates;
    }

    async fn cancel_hand_writing(&self, n_strokes: u32) {
        let _ = n_strokes;
    }

    async fn panel_extension_received(&self, event: Value<'_>) {
        let _ = event;
    }

    async fn panel_extension_register_keys(&self, data: Value<'_>) {
        let _ = data;
    }

    #[zbus(property)]
    async fn content_type(&self) -> (u32, u32) {
        let st = self.state.lock();
        (st.purpose, st.hints)
    }

    #[zbus(property, name = "FocusId")]
    async fn focus_id(&self) -> (bool,) {
        let st = self.state.lock();
        (st.focused,)
    }
}

/// `org.freedesktop.IBus.Service`, which the daemon calls to free an engine
/// object again.
pub struct Service {
    conn: Connection,
    path: OwnedObjectPath,
}

#[zbus::interface(name = "org.freedesktop.IBus.Service")]
impl Service {
    async fn destroy(&self) -> bool {
        debug_event(&format!("Destroy {}", self.path.as_str()));
        let server = self.conn.object_server();
        let _ = server.remove::<Service, _>(&self.path).await;
        let _ = server.remove::<Engine, _>(&self.path).await;
        true
    }
}

/// The component factory: hands out one [`Engine`] per input context.
pub struct Factory {
    conn: Connection,
    shared: Arc<Shared>,
    next: AtomicU64,
}

impl Factory {
    pub fn new(conn: Connection, shared: Arc<Shared>) -> Factory {
        Factory {
            conn,
            shared,
            next: AtomicU64::new(1),
        }
    }
}

#[zbus::interface(name = "org.freedesktop.IBus.Factory")]
impl Factory {
    async fn create_engine(&self, name: String) -> zbus::fdo::Result<OwnedObjectPath> {
        if !matches!(name.as_str(), ENGINE_NAME | "ibus-telex" | "IBusTelex") {
            return Err(zbus::fdo::Error::Failed(format!("unknown engine: {name}")));
        }
        let id = self.next.fetch_add(1, Ordering::SeqCst);
        let path = OwnedObjectPath::try_from(format!("/org/freedesktop/IBus/Engine/{id}"))
            .map_err(|e| zbus::fdo::Error::Failed(format!("bad path: {e}")))?;
        let engine = Engine::new(self.conn.clone(), path.clone(), self.shared.clone());
        let service = Service {
            conn: self.conn.clone(),
            path: path.clone(),
        };
        self.register(&path, engine, service).await?;
        Ok(path)
    }
}

impl Factory {
    async fn register(
        &self,
        path: &OwnedObjectPath,
        engine: Engine,
        service: Service,
    ) -> zbus::fdo::Result<()> {
        let server: &ObjectServer = self.conn.object_server();
        server
            .at(path, engine)
            .await
            .map_err(|e| zbus::fdo::Error::Failed(format!("cannot export engine: {e}")))?;
        server
            .at(path, service)
            .await
            .map_err(|e| zbus::fdo::Error::Failed(format!("cannot export service: {e}")))?;
        Ok(())
    }
}

fn warn(msg: &str) {
    if std::env::var_os("IBUS_TELEX_DEBUG").is_some() {
        eprintln!("ibus-telex: {msg}");
    }
}

/// Log the calls the daemon makes, under the same switch as the keys.
fn debug_event(msg: &str) {
    if std::env::var_os("IBUS_TELEX_DEBUG").is_some() {
        eprintln!("ibus-telex: {msg}");
    }
}

/// Log every key the daemon offers, with the modifier state.
///
/// This is the evidence needed to tell "the engine never got the key" from
/// "the engine declined it" - the two look identical from the outside.
fn debug_key(keyval: u32, state: u32, handled: bool) {
    if std::env::var_os("IBUS_TELEX_DEBUG").is_none() {
        return;
    }
    let key = match keys::char_from_keyval(keyval) {
        Some(c) => format!("{c:?}"),
        None => format!("{keyval:#x}"),
    };
    eprintln!(
        "ibus-telex: key {key} state={state:#010x} -> {}",
        if handled {
            "consumed"
        } else {
            "passed through"
        }
    );
}

/// Connect to the IBus bus.
///
/// The daemon passes the private bus address in `IBUS_ADDRESS`; when the
/// engine is started by hand we fall back to asking the `ibus` command and
/// finally to the session bus.
async fn connect() -> zbus::Result<Connection> {
    if let Some(addr) = std::env::var_os("IBUS_ADDRESS") {
        let addr = addr.to_string_lossy().into_owned();
        if !addr.is_empty() {
            return zbus::connection::Builder::address(addr.as_str())?
                .build()
                .await;
        }
    }
    match zbus::connection::Builder::ibus()?.build().await {
        Ok(conn) => Ok(conn),
        Err(e) => {
            warn(&format!(
                "cannot connect to the IBus bus ({e}), using the session bus"
            ));
            zbus::connection::Builder::session()?.build().await
        }
    }
}

/// Run the engine until the bus goes away.
pub async fn run() -> zbus::Result<()> {
    let conn = connect().await?;
    let flags = RequestNameFlags::DoNotQueue | RequestNameFlags::AllowReplacement;
    let reply = conn.request_name_with_flags(BUS_NAME, flags).await?;
    if reply != RequestNameReply::PrimaryOwner {
        // Another instance got there first (the daemon can start the engine
        // twice when several sessions share a bus): step aside instead of
        // stealing the name, which used to leave a dead engine behind.
        warn(&format!("{BUS_NAME} is already owned ({reply:?}), exiting"));
        return Ok(());
    }

    let shared = Shared::new();
    let factory = Factory::new(conn.clone(), shared);
    conn.object_server().at(FACTORY_PATH, factory).await?;
    if std::env::var_os("IBUS_TELEX_DEBUG").is_some() {
        eprintln!("ibus-telex: serving {BUS_NAME} on {FACTORY_PATH}");
    }
    // Park the task; the connection keeps the process alive.
    std::future::pending::<()>().await;
    Ok(())
}

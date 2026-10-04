//! End-to-end test of the D-Bus protocol.
//!
//! The engine is started as a child process on a real bus (the session bus
//! stands in for the private IBus bus) and then driven with exactly the calls
//! `ibus-daemon` makes: create the engine through the factory, feed key
//! events, watch the signals that come back.

use std::process::{Child, Command};
use std::time::{Duration, Instant};

use futures_lite::StreamExt;
use ibus_telex::ibus::engine::{ENGINE_IFACE, FACTORY_IFACE, FACTORY_PATH, SERVICE_IFACE};
use ibus_telex::{BUS_NAME, ENGINE_NAME};
use zbus::message::Message;
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};
use zbus::{Connection, MessageStream, Proxy};

const PREEDIT: &str = "UpdatePreeditText";
const COMMIT: &str = "CommitText";
const REGISTER: &str = "RegisterProperties";

const NO_MODIFIER: u32 = 0;
const CONTROL_MASK: u32 = 1 << 2;
const PASSWORD: u32 = 8;

/// Kills the engine when the test ends, however it ends.
struct EngineProcess(Child);

impl Drop for EngineProcess {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn keyval(c: char) -> u32 {
    c as u32
}

/// `IBusText` travels as `(name, attachments, text, attributes)` inside a
/// variant; pull the text out of it.
fn ibus_text(value: &OwnedValue) -> Option<String> {
    fn text_of(value: &Value<'_>) -> Option<String> {
        match value {
            Value::Value(inner) => text_of(inner),
            Value::Structure(s) => match s.fields().get(2) {
                Some(Value::Str(t)) => Some(t.as_str().to_owned()),
                _ => None,
            },
            _ => None,
        }
    }
    text_of(value)
}

fn record(message: &Message, seen: &mut Vec<(String, String)>) {
    let header = message.header();
    if header.interface().map(|i| i.as_str()) != Some(ENGINE_IFACE) {
        return;
    }
    let Some(name) = header.member().map(|m| m.as_str().to_owned()) else {
        return;
    };
    let text = match name.as_str() {
        PREEDIT => message
            .body()
            .deserialize::<(OwnedValue, u32, bool, u32)>()
            .ok()
            .and_then(|(t, _, _, _)| ibus_text(&t))
            .unwrap_or_default(),
        COMMIT => message
            .body()
            .deserialize::<(OwnedValue,)>()
            .ok()
            .and_then(|(t,)| ibus_text(&t))
            .unwrap_or_default(),
        _ => String::new(),
    };
    seen.push((name, text));
}

/// Read everything the engine has sent so far.
async fn drain(stream: &mut MessageStream, seen: &mut Vec<(String, String)>) {
    loop {
        let got = futures_lite::future::or(async { Some(stream.next().await) }, async {
            async_io::Timer::after(Duration::from_millis(80)).await;
            None
        })
        .await;
        match got {
            Some(Some(Ok(message))) => record(&message, seen),
            _ => break,
        }
    }
}

async fn wait_for_engine(conn: &Connection) -> zbus::Result<Proxy<'static>> {
    let dbus = Proxy::new_owned(
        conn.clone(),
        "org.freedesktop.DBus",
        "/org/freedesktop/DBus",
        "org.freedesktop.DBus",
    )
    .await?;
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let owned: bool = dbus
            .call("NameHasOwner", &(BUS_NAME,))
            .await
            .unwrap_or(false);
        if owned {
            break;
        }
        if Instant::now() > deadline {
            panic!("the engine never took {BUS_NAME}");
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    Proxy::new_owned(conn.clone(), BUS_NAME, FACTORY_PATH, FACTORY_IFACE).await
}

fn spawn_engine(addr: &str) -> EngineProcess {
    let exe = env!("CARGO_BIN_EXE_ibus-telex");
    let child = Command::new(exe)
        .env("IBUS_ADDRESS", addr)
        .env("IBUS_TELEX_DEBUG", "1")
        .spawn()
        .expect("spawn engine");
    EngineProcess(child)
}

#[test]
fn engine_serves_the_ibus_protocol() {
    let addr = match std::env::var("DBUS_SESSION_BUS_ADDRESS") {
        Ok(a) if !a.is_empty() => a,
        _ => {
            eprintln!("skipping: no session bus to run the engine on");
            return;
        }
    };
    let _guard = spawn_engine(&addr);
    zbus::block_on(async move {
        let conn = zbus::connection::Builder::address(addr.as_str())
            .unwrap()
            .build()
            .await
            .unwrap();
        let factory = wait_for_engine(&conn).await.unwrap();
        let path: OwnedObjectPath = factory
            .call("CreateEngine", &(ENGINE_NAME,))
            .await
            .expect("CreateEngine");
        assert_eq!(path.as_str(), "/org/freedesktop/IBus/Engine/1");
        let engine = Proxy::new_owned(conn.clone(), BUS_NAME, path.clone(), ENGINE_IFACE)
            .await
            .unwrap();

        // The bus only delivers signals a connection asked for.
        let rule = zbus::MatchRule::builder()
            .msg_type(zbus::message::Type::Signal)
            .interface(ENGINE_IFACE)
            .unwrap()
            .build();
        zbus::fdo::DBusProxy::new(&conn)
            .await
            .unwrap()
            .add_match_rule(rule)
            .await
            .unwrap();

        let mut stream = MessageStream::from(&conn);
        let mut seen: Vec<(String, String)> = Vec::new();

        let _: () = engine.call("Enable", &()).await.unwrap();
        drain(&mut stream, &mut seen).await;
        assert!(
            seen.iter().any(|(name, _)| name == REGISTER),
            "Enable must register the panel properties, got {seen:?}"
        );

        seen.clear();
        // "tieeng" -> pre-edit "tiêng"
        for c in ['t', 'i', 'e', 'e', 'n', 'g'] {
            let handled: bool = engine
                .call("ProcessKeyEvent", &(keyval(c), 0u32, NO_MODIFIER))
                .await
                .unwrap();
            assert!(handled, "{c} must be consumed");
        }
        drain(&mut stream, &mut seen).await;
        let preedits: Vec<&String> = seen
            .iter()
            .filter(|(n, _)| n == PREEDIT)
            .map(|(_, t)| t)
            .collect();
        assert_eq!(
            preedits.last().map(|s| s.as_str()),
            Some("tiêng"),
            "pre-edit while typing: {preedits:?}"
        );

        seen.clear();
        let handled: bool = engine
            .call("ProcessKeyEvent", &(keyval('s'), 0u32, NO_MODIFIER))
            .await
            .unwrap();
        assert!(handled);
        drain(&mut stream, &mut seen).await;
        assert!(
            seen.iter().any(|(n, t)| n == PREEDIT && t == "tiếng"),
            "the tone key must update the pre-edit, got {seen:?}"
        );

        seen.clear();
        let handled: bool = engine
            .call("ProcessKeyEvent", &(keyval(' '), 0u32, NO_MODIFIER))
            .await
            .unwrap();
        assert!(handled, "the space that ends a word is consumed");
        drain(&mut stream, &mut seen).await;
        assert!(
            seen.iter().any(|(n, t)| n == COMMIT && t == "tiếng "),
            "the space must commit the word, got {seen:?}"
        );
        assert!(
            seen.iter().any(|(n, t)| n == PREEDIT && t.is_empty()),
            "the pre-edit must be hidden after committing, got {seen:?}"
        );

        // an invalid syllable is restored to the raw keystrokes
        seen.clear();
        for c in ['t', 'i', 'e', 'n', 'g', 's'] {
            let _: bool = engine
                .call("ProcessKeyEvent", &(keyval(c), 0u32, NO_MODIFIER))
                .await
                .unwrap();
        }
        let _: bool = engine
            .call("ProcessKeyEvent", &(keyval(' '), 0u32, NO_MODIFIER))
            .await
            .unwrap();
        drain(&mut stream, &mut seen).await;
        assert!(
            seen.iter().any(|(n, t)| n == COMMIT && t == "tiengs "),
            "an invalid word is restored, got {seen:?}"
        );

        // application shortcuts are never swallowed
        for c in ['c', 'a', 'v'] {
            let handled: bool = engine
                .call("ProcessKeyEvent", &(keyval(c), 0u32, CONTROL_MASK))
                .await
                .unwrap();
            assert!(!handled, "Ctrl+{c} must go to the application");
        }

        // Num Lock sets MOD2 on every keystroke while it is on; that used to
        // make the engine hand every key back to the application, so nothing
        // was ever typed.
        const MOD2: u32 = 1 << 4;
        seen.clear();
        for c in ['t', 'i', 'e', 'e', 'n', 'g', 's'] {
            let handled: bool = engine
                .call("ProcessKeyEvent", &(keyval(c), 0u32, MOD2))
                .await
                .unwrap();
            assert!(handled, "{c} with Num Lock on must still be typed");
        }
        drain(&mut stream, &mut seen).await;
        let last_preedit = seen
            .iter()
            .filter(|(name, _)| name == PREEDIT)
            .map(|(_, text)| text.as_str())
            .next_back();
        assert_eq!(
            last_preedit,
            Some("tiếng"),
            "pre-edit with Num Lock on, got {seen:?}"
        );
        let _: bool = engine
            .call("ProcessKeyEvent", &(keyval(' '), 0u32, MOD2))
            .await
            .unwrap();
        drain(&mut stream, &mut seen).await;
        assert!(
            seen.iter().any(|(n, t)| n == COMMIT && t == "tiếng "),
            "Num Lock must not stop committing, got {seen:?}"
        );

        // losing focus commits instead of dropping the word
        seen.clear();
        for c in ['d', 'd'] {
            let _: bool = engine
                .call("ProcessKeyEvent", &(keyval(c), 0u32, NO_MODIFIER))
                .await
                .unwrap();
        }
        let _: () = engine.call("FocusOut", &()).await.unwrap();
        drain(&mut stream, &mut seen).await;
        assert!(
            seen.iter().any(|(n, t)| n == COMMIT && t == "đ"),
            "FocusOut must commit the pending word, got {seen:?}"
        );

        // password fields are left alone
        let _: () = engine
            .call("SetContentType", &(PASSWORD, 0u32))
            .await
            .unwrap();
        let handled: bool = engine
            .call("ProcessKeyEvent", &(keyval('a'), 0u32, NO_MODIFIER))
            .await
            .unwrap();
        assert!(!handled, "password fields must not get Vietnamese input");
        let _: () = engine.call("SetContentType", &(0u32, 0u32)).await.unwrap();

        // and the engine object can be destroyed again
        let service = Proxy::new_owned(conn.clone(), BUS_NAME, path, SERVICE_IFACE)
            .await
            .unwrap();
        let destroyed: bool = service.call("Destroy", &()).await.expect("Destroy");
        assert!(destroyed);
    });
}

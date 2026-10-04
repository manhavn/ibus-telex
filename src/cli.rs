//! Command line interface: run the engine, install it, check the setup.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use crate::config::{bin_dir, cache_dir, config_dir, data_dir, Config};
use crate::{BUS_NAME, COMPONENT_FILE, ENGINE_NAME};

const VERSION: &str = env!("CARGO_PKG_VERSION");

const HELP: &str = "\
ibus-telex - Vietnamese input method engine for IBus (Telex/VNI)

USAGE:
    ibus-telex [--ibus]            run the engine (started by ibus-daemon)
    ibus-telex --xml               print the engine list, used by ibus-daemon
    ibus-telex install [--add-source]
                                   install into ~/.local and register in IBus
    ibus-telex uninstall           remove it again
    ibus-telex doctor              check the setup and report problems
    ibus-telex --help | --version

ENVIRONMENT:
    IBUS_ADDRESS        bus to connect to (set by ibus-daemon)
    IBUS_TELEX_DEBUG    print engine diagnostics to stderr
";

/// Entry point; returns the process exit code.
pub fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut cmd: Option<&str> = None;
    let mut add_source = false;
    let mut print_xml = false;
    let mut run = false;

    for arg in &args {
        match arg.as_str() {
            "--xml" => print_xml = true,
            "--ibus" | "--daemon" => run = true,
            "--help" | "-h" | "help" => {
                print!("{HELP}");
                return ExitCode::SUCCESS;
            }
            "--version" | "-V" | "version" => {
                println!("ibus-telex {VERSION}");
                return ExitCode::SUCCESS;
            }
            "--add-source" => add_source = true,
            other if other.starts_with('-') => {
                eprintln!("ibus-telex: unknown option {other}\n\n{HELP}");
                return ExitCode::FAILURE;
            }
            other => cmd = Some(other),
        }
    }

    match cmd {
        None => {
            if print_xml {
                print_engines_xml();
                return ExitCode::SUCCESS;
            }
            let _ = run;
            run_engine()
        }
        Some("install") => install(add_source),
        Some("uninstall") => uninstall(),
        Some("doctor") => doctor(),
        Some("sources") => print_sources_append(),
        Some("xml") => {
            print_engines_xml();
            ExitCode::SUCCESS
        }
        Some(other) => {
            eprintln!("ibus-telex: unknown command {other}\n\n{HELP}");
            ExitCode::FAILURE
        }
    }
}

fn run_engine() -> ExitCode {
    match crate::ibus::block_on(crate::ibus::run()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("ibus-telex: {e}");
            ExitCode::FAILURE
        }
    }
}

/// `<engines>` list, printed for `ibus-daemon` when it scans the component.
fn print_engines_xml() {
    println!(
        "\
<engines>
  <engine>
    <name>{name}</name>
    <longname>{name} (Vietnamese)</longname>
    <description>Vietnamese input method: Telex and VNI, with the same options as Unikey. Tone marks, đ, ư, ơ, ă, â, ê, ô and spell checking are handled while typing.</description>
    <language>vi</language>
    <license>GPL-3.0-or-later</license>
    <author>ibus-telex</author>
    <icon>ibus-keyboard</icon>
    <layout>us</layout>
    <layout_variant></layout_variant>
    <layout_option></layout_option>
    <hotkeys></hotkeys>
    <symbol></symbol>
    <rank>1</rank>
  </engine>
</engines>",
        name = ENGINE_NAME
    );
}

fn component_xml(exec: &Path) -> String {
    let exec = exec.display();
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<!-- installed by ibus-telex install -->
<component>
  <name>{BUS_NAME}</name>
  <description>Vietnamese input method (Telex, VNI)</description>
  <exec>{exec} --ibus</exec>
  <version>{VERSION}</version>
  <author>ibus-telex</author>
  <license>GPL-3.0-or-later</license>
  <textdomain>ibus-telex</textdomain>
  <engines exec="{exec} --xml" />
</component>
"#
    )
}

fn component_path() -> Option<PathBuf> {
    Some(data_dir()?.join(COMPONENT_FILE))
}

/// IBus only reads components from `/usr/share/ibus/component` unless
/// `IBUS_COMPONENT_PATH` says otherwise (man ibus: write-cache), so a
/// user-local install has to point the daemon at the user directory.  This
/// drop-in is the piece that makes a rootless install work; the system
/// directory is kept in the list so nothing else disappears.
fn dropin_path() -> Option<PathBuf> {
    Some(
        config_dir()?
            .join("systemd/user")
            .join(format!("{IBUS_SERVICE}.d"))
            .join("telex.conf"),
    )
}

const IBUS_SERVICE: &str = "org.freedesktop.IBus.session.GNOME.service";

/// Returns the path and whether the file had to be written, so that a
/// re-install does not make systemd complain about a changed unit.
fn write_dropin() -> std::io::Result<(PathBuf, bool)> {
    let path = dropin_path()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "$HOME is not set"))?;
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let comp_dir = data_dir()
        .map(|d| d.join("ibus/component").display().to_string())
        .unwrap_or_default();
    let content = format!(
        "[Service]\n\
         # added by `ibus-telex install`: IBus looks for components in\n\
         # {comp_dir} in addition to the system directory.\n\
         Environment=IBUS_COMPONENT_PATH=/usr/share/ibus/component:{comp_dir}\n"
    );
    if fs::read_to_string(&path).is_ok_and(|old| old == content) {
        return Ok((path, false));
    }
    fs::write(&path, content)?;
    Ok((path, true))
}

/// Drop the daemon's component cache.
///
/// IBus remembers the component directories it scanned in
/// `~/.cache/ibus/bus/registry` and only rescans when one of *those* paths
/// changes - a directory that was not there when the cache was written stays
/// invisible forever otherwise.
fn invalidate_registry_cache() {
    let Some(path) = cache_dir().map(|d| d.join("ibus/bus/registry")) else {
        return;
    };
    if path.exists() {
        match fs::remove_file(&path) {
            Ok(()) => println!("dropped the stale component cache {}", path.display()),
            Err(e) => eprintln!("ibus-telex: cannot remove {}: {e}", path.display()),
        }
    }
}

fn daemon_reload() {
    let _ = std::process::Command::new("systemctl")
        .args(["--user", "daemon-reload"])
        .status();
}

/// Restart the daemon so it picks the component up.
fn reload_ibus() {
    let systemd = std::process::Command::new("systemctl")
        .args(["--user", "is-active", IBUS_SERVICE])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "active")
        .unwrap_or(false);
    if systemd {
        daemon_reload();
        match std::process::Command::new("systemctl")
            .args(["--user", "restart", IBUS_SERVICE])
            .status()
        {
            Ok(s) if s.success() => println!("restarted {IBUS_SERVICE}"),
            _ => eprintln!("ibus-telex: could not restart {IBUS_SERVICE}, run `ibus restart`"),
        }
    } else if let Ok(s) = std::process::Command::new("ibus").arg("restart").status() {
        if s.success() {
            println!("restarted ibus-daemon");
        }
    } else {
        eprintln!("ibus-telex: restart IBus by hand (`ibus restart`) or log out");
    }
}

/// Ask the daemon whether it knows our engine.
fn daemon_knows_engine() -> bool {
    std::process::Command::new("ibus")
        .arg("list-engine")
        .output()
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .any(|l| l.trim_start().starts_with(ENGINE_NAME))
        })
        .unwrap_or(false)
}

fn install(add_source: bool) -> ExitCode {
    let exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("ibus-telex: cannot find my own path: {e}");
            return ExitCode::FAILURE;
        }
    };
    let target = match bin_dir() {
        Some(dir) => dir.join("ibus-telex"),
        None => {
            eprintln!("ibus-telex: $HOME is not set");
            return ExitCode::FAILURE;
        }
    };
    let target = fs::canonicalize(&target).unwrap_or(target);

    if !same_file(&exe, &target) {
        if let Some(dir) = target.parent() {
            if let Err(e) = fs::create_dir_all(dir) {
                eprintln!("ibus-telex: cannot create {}: {e}", dir.display());
                return ExitCode::FAILURE;
            }
        }
        if let Err(e) = fs::copy(&exe, &target) {
            eprintln!(
                "ibus-telex: cannot install {} -> {}: {e}",
                exe.display(),
                target.display()
            );
            return ExitCode::FAILURE;
        }
        let _ = fs::set_permissions(&target, fs::Permissions::from_mode(0o755));
        println!("installed {}", target.display());
    } else {
        println!("already installed at {}", target.display());
    }

    let path = match component_path() {
        Some(p) => p,
        None => {
            eprintln!("ibus-telex: $HOME is not set");
            return ExitCode::FAILURE;
        }
    };
    if let Some(dir) = path.parent() {
        if let Err(e) = fs::create_dir_all(dir) {
            eprintln!("ibus-telex: cannot create {}: {e}", dir.display());
            return ExitCode::FAILURE;
        }
    }
    if let Err(e) = fs::write(&path, component_xml(&target)) {
        eprintln!("ibus-telex: cannot write {}: {e}", path.display());
        return ExitCode::FAILURE;
    }
    println!("installed {}", path.display());

    let mut dropin_changed = false;
    match write_dropin() {
        Ok((dropin, changed)) => {
            dropin_changed = changed;
            if changed {
                println!("installed {}", dropin.display());
            }
        }
        Err(e) => eprintln!("ibus-telex: could not write the systemd drop-in: {e}"),
    }
    // Keep systemd's view of the unit files in step; an earlier install may
    // have rewritten the drop-in.
    daemon_reload();
    if dropin_changed || !daemon_knows_engine() {
        invalidate_registry_cache();
        reload_ibus();
    } else {
        // Re-installing over a working setup must not disturb the session.
        println!("IBus already knows the {ENGINE_NAME} engine.");
    }

    if add_source {
        add_input_source();
    } else {
        println!(
            "\nTell your desktop to use the input source (or pick it in\n\
             Settings -> Keyboard -> Input Sources):\n\
             \x20   gsettings set org.gnome.desktop.input-sources sources \\\n\
             \x20       \"$(ibus-telex sources --append)\""
        );
    }
    if !daemon_knows_engine() {
        eprintln!(
            "\nIBus does not list {ENGINE_NAME} yet. Log out and back in, or run\n\
             `systemctl --user restart {IBUS_SERVICE}`."
        );
    }
    ExitCode::SUCCESS
}

/// `ibus-telex sources --append` prints the new gsettings value with the
/// engine appended to the current input sources.
pub fn print_sources_append() -> ExitCode {
    let current = gsettings_get_sources();
    let mut sources = current.clone();
    let entry = format!("('ibus','{}')", ENGINE_NAME);
    if !sources.contains(&entry) {
        sources.push(entry);
    }
    println!("[{}]", sources.join(", "));
    ExitCode::SUCCESS
}

fn gsettings_get_sources() -> Vec<String> {
    let out = std::process::Command::new("gsettings")
        .args(["get", "org.gnome.desktop.input-sources", "sources"])
        .output();
    let text = match out {
        Ok(o) => String::from_utf8_lossy(&o.stdout).into_owned(),
        Err(_) => return vec!["('xkb', 'us')".to_owned()],
    };
    let text = text.trim();
    let inner = text.trim_start_matches('[').trim_end_matches(']');
    inner
        .split("), (")
        .map(|s| s.trim().trim_matches(|c| c == '(' || c == ')').to_owned())
        .filter(|s| !s.is_empty())
        .map(|s| format!("({s})"))
        .collect()
}

fn add_input_source() {
    let value = format!("[{}]", {
        let current = gsettings_get_sources();
        let mut sources = current;
        let entry = format!("('ibus','{}')", ENGINE_NAME);
        if !sources.contains(&entry) {
            sources.push(entry);
        }
        sources.join(", ")
    });
    let status = std::process::Command::new("gsettings")
        .args(["set", "org.gnome.desktop.input-sources", "sources", &value])
        .status();
    match status {
        Ok(s) if s.success() => println!("added input source {ENGINE_NAME} ({value})"),
        _ => eprintln!(
            "ibus-telex: could not update the input sources, \
             add them by hand in Settings -> Keyboard"
        ),
    }
}

fn uninstall() -> ExitCode {
    if let Some(path) = component_path() {
        if path.exists() {
            match fs::remove_file(&path) {
                Ok(()) => println!("removed {}", path.display()),
                Err(e) => {
                    eprintln!("ibus-telex: cannot remove {}: {e}", path.display());
                    return ExitCode::FAILURE;
                }
            }
        }
    }
    if let Some(dropin) = dropin_path() {
        if dropin.exists() {
            let _ = fs::remove_file(&dropin);
            println!("removed {}", dropin.display());
            let _ = std::process::Command::new("systemctl")
                .args(["--user", "daemon-reload"])
                .status();
        }
    }
    if let Some(target) = bin_dir().map(|d| d.join("ibus-telex")) {
        if target.exists() {
            let _ = fs::remove_file(&target);
            println!("removed {}", target.display());
        }
    }
    println!("restart IBus (ibus restart) or log out to finish");
    ExitCode::SUCCESS
}

fn same_file(a: &Path, b: &Path) -> bool {
    match (fs::canonicalize(a), fs::canonicalize(b)) {
        (Ok(x), Ok(y)) => x == y,
        _ => false,
    }
}

fn doctor() -> ExitCode {
    let mut problems = 0;

    let session = std::env::var("XDG_SESSION_TYPE").unwrap_or_else(|_| "unknown".into());
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_else(|_| "unknown".into());
    println!("session type     : {session}");
    println!("desktop          : {desktop}");

    match std::env::var("IBUS_ADDRESS") {
        Ok(a) if !a.is_empty() => println!("IBUS_ADDRESS     : {a}"),
        _ => {
            let bus_file = std::env::var("XDG_CONFIG_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|_| crate::config::home().unwrap_or_default().join(".config"))
                .join("ibus/bus");
            let found = fs::read_dir(&bus_file)
                .map(|rd| rd.filter_map(|e| e.ok()).any(|e| e.path().is_file()))
                .unwrap_or(false);
            if found {
                println!("IBUS_ADDRESS     : not set (found an ibus bus file)");
            } else {
                problems += 1;
                println!(
                    "IBUS_ADDRESS     : not set and no bus file in {}",
                    bus_file.display()
                );
            }
        }
    }

    match component_path() {
        Some(p) if p.exists() => println!("component file   : {}", p.display()),
        Some(p) => {
            problems += 1;
            println!(
                "component file   : missing ({}) - run `ibus-telex install`",
                p.display()
            );
        }
        None => {
            problems += 1;
            println!("component file   : $HOME is not set");
        }
    }

    match dropin_path() {
        Some(p) if p.exists() => println!("component path   : {}", p.display()),
        Some(_) if std::path::Path::new("/usr/share/ibus/component/telex.xml").exists() => {
            println!("component path   : system-wide install")
        }
        Some(p) => {
            problems += 1;
            println!(
                "component path   : missing ({}) - run `ibus-telex install`",
                p.display()
            );
        }
        None => {}
    }

    if component_path().is_some_and(|p| p.exists()) {
        if daemon_knows_engine() {
            println!("ibus knows us    : yes");
        } else {
            problems += 1;
            println!("ibus knows us    : no - restart IBus or log out and back in");
        }
    }

    for key in ["GTK_IM_MODULE", "QT_IM_MODULE", "XMODIFIERS"] {
        if let Ok(v) = std::env::var(key) {
            println!("{key:<17}: {v}");
        }
    }

    // Things that go wrong specifically on Wayland.
    if session == "wayland" {
        println!("\nWayland notes:");
        println!("  * Electron/Chromium apps only see input methods when started with");
        println!("    --enable-wayland-ime (or --ozone-platform-hint=auto).");
        println!("  * GNOME Shell draws the pre-edit itself; nothing to configure here.");
        println!("  * If an application ignores the input method entirely, check that");
        println!("    GTK_IM_MODULE/QT_IM_MODULE are not forced to 'wayland'.");
    }

    let cfg = Config::load();
    println!("\noptions          : {:?}", cfg.options);
    if problems == 0 {
        println!("\nno problems found");
        ExitCode::SUCCESS
    } else {
        println!("\n{problems} problem(s) found");
        ExitCode::FAILURE
    }
}

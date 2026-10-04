//! Engine options and their on-disk representation.
//!
//! The options are the same ones Unikey exposes in the IBus panel; they are
//! stored in a small `key=value` file under `$XDG_CONFIG_HOME/ibus-telex/`.

use std::fmt::Write as _;
use std::fs;
use std::path::PathBuf;

use crate::vn::{Method, Options};

/// Configuration of the engine.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Config {
    pub options: Options,
}

impl Config {
    /// Load the configuration, falling back to the defaults.
    pub fn load() -> Config {
        let mut cfg = Config::default();
        let path = match Self::path() {
            Some(p) => p,
            None => return cfg,
        };
        let data = match fs::read_to_string(&path) {
            Ok(d) => d,
            Err(_) => return cfg,
        };
        for line in data.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let (key, value) = match line.split_once('=') {
                Some(kv) => kv,
                None => continue,
            };
            let value = value.trim();
            match key.trim() {
                "method" => {
                    cfg.options.method = match value {
                        "vni" => Method::Vni,
                        _ => Method::Telex,
                    }
                }
                "spell_check" => cfg.options.spell_check = parse_bool(value),
                "auto_restore" => cfg.options.auto_restore = parse_bool(value),
                "modern_style" => cfg.options.modern_style = parse_bool(value),
                "free_marking" => cfg.options.free_marking = parse_bool(value),
                "standalone_w" => cfg.options.standalone_w = parse_bool(value),
                _ => {}
            }
        }
        cfg
    }

    /// Store the configuration; failures are not fatal.
    pub fn save(&self) -> std::io::Result<()> {
        let path = match Self::path() {
            Some(p) => p,
            None => return Ok(()),
        };
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let o = &self.options;
        let mut out = String::new();
        let _ = writeln!(out, "# ibus-telex configuration");
        let _ = writeln!(out, "method={}", o.method.as_str());
        let _ = writeln!(out, "spell_check={}", o.spell_check);
        let _ = writeln!(out, "auto_restore={}", o.auto_restore);
        let _ = writeln!(out, "modern_style={}", o.modern_style);
        let _ = writeln!(out, "free_marking={}", o.free_marking);
        let _ = writeln!(out, "standalone_w={}", o.standalone_w);
        fs::write(path, out)
    }

    fn path() -> Option<PathBuf> {
        Some(config_dir()?.join("config"))
    }
}

fn parse_bool(value: &str) -> bool {
    matches!(value, "1" | "true" | "yes" | "on")
}

/// `$XDG_CONFIG_HOME` or `~/.config`.
pub fn config_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("XDG_CONFIG_HOME") {
        if !dir.is_empty() {
            return Some(PathBuf::from(dir));
        }
    }
    Some(home()?.join(".config"))
}

/// `$XDG_DATA_HOME` or `~/.local/share`.
pub fn data_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("XDG_DATA_HOME") {
        if !dir.is_empty() {
            return Some(PathBuf::from(dir));
        }
    }
    Some(home()?.join(".local/share"))
}

/// `$XDG_CACHE_HOME` or `~/.cache`.
pub fn cache_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("XDG_CACHE_HOME") {
        if !dir.is_empty() {
            return Some(PathBuf::from(dir));
        }
    }
    Some(home()?.join(".cache"))
}

/// `$XDG_BIN_HOME` or `~/.local/bin`.
pub fn bin_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("XDG_BIN_HOME") {
        if !dir.is_empty() {
            return Some(PathBuf::from(dir));
        }
    }
    Some(home()?.join(".local/bin"))
}

pub fn home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

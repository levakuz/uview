//! The user's settings file (ADR-0016): where it is, and the theme it chooses. Optional, and
//! never written by uview.

use std::path::{Path, PathBuf};
use std::str::FromStr;

use ratatui::style::Color;
use serde::Deserialize;

use crate::ui::theme::Theme;

/// Where the settings file is, as `env` reads the environment: `UVIEW_CONFIG_FILE`, or
/// `uview/config.toml` where uv keeps `uv.toml`: under `%APPDATA%` on Windows, else under
/// `$XDG_CONFIG_HOME` or `~/.config`. `None` when none of them is set.
pub fn config_path(env: impl Fn(&str) -> Option<String>, windows: bool) -> Option<PathBuf> {
    let set = |key| env(key).filter(|value| !value.is_empty());
    if let Some(file) = set("UVIEW_CONFIG_FILE") {
        return Some(PathBuf::from(file));
    }
    let dir = if windows {
        PathBuf::from(set("APPDATA")?)
    } else {
        // A relative XDG_CONFIG_HOME is to be ignored, as the spec says.
        set("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .filter(|dir| dir.is_absolute())
            .or_else(|| set("HOME").map(|home| Path::new(&home).join(".config")))?
    };
    Some(dir.join("uview").join("config.toml"))
}

/// What the settings file says.
#[derive(Debug, Default, Deserialize, PartialEq)]
pub struct Config {
    /// The built-in theme to start from; asked of the terminal when unset.
    pub theme: Option<Base>,
    #[serde(default)]
    pub colors: Colors,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Base {
    Dark,
    Light,
}

impl Base {
    pub fn theme(self) -> Theme {
        match self {
            Base::Dark => Theme::DARK,
            Base::Light => Theme::LIGHT,
        }
    }
}

/// A colour for each role the file sets, as written: a name such as `lightred`, `reset`, an
/// index `0`–`255` or `#rrggbb`.
#[derive(Debug, Default, Deserialize, PartialEq)]
pub struct Colors {
    pub text: Option<String>,
    pub accent: Option<String>,
    pub muted: Option<String>,
    pub warning: Option<String>,
    pub error: Option<String>,
    pub success: Option<String>,
}

/// The keys the file may have, at its top and in `[colors]`.
const KEYS: [&str; 2] = ["theme", "colors"];
const ROLES: [&str; 6] = ["text", "accent", "muted", "warning", "error", "success"];

/// The settings `text` holds, with each key that means nothing, which is told and passed over,
/// or what keeps it from being read at all.
pub fn parse(text: &str) -> Result<(Config, Vec<String>), String> {
    let table: toml::Table = text
        .parse()
        .map_err(|e: toml::de::Error| e.message().to_string())?;
    let mut unknown: Vec<String> = table
        .keys()
        .filter(|key| !KEYS.contains(&key.as_str()))
        .map(|key| format!("{key:?} means nothing"))
        .collect();
    if let Some(colors) = table.get("colors").and_then(toml::Value::as_table) {
        unknown.extend(
            colors
                .keys()
                .filter(|key| !ROLES.contains(&key.as_str()))
                .map(|key| format!("{key:?} is no role in [colors]")),
        );
    }
    let config = table
        .try_into()
        .map_err(|e: toml::de::Error| e.message().to_string())?;
    Ok((config, unknown))
}

/// `base` with each colour `colors` sets in place of its own, and what could not be read: a
/// colour that does not parse keeps the theme's.
pub fn themed(base: Theme, colors: &Colors) -> (Theme, Vec<String>) {
    let mut theme = base;
    let mut problems = Vec::new();
    let roles = [
        ("text", &colors.text, &mut theme.text),
        ("accent", &colors.accent, &mut theme.accent),
        ("muted", &colors.muted, &mut theme.muted),
        ("warning", &colors.warning, &mut theme.warning),
        ("error", &colors.error, &mut theme.error),
        ("success", &colors.success, &mut theme.success),
    ];
    for (role, value, color) in roles {
        let Some(value) = value else { continue };
        match Color::from_str(value) {
            Ok(parsed) => *color = parsed,
            Err(_) => problems.push(format!("{value:?} is no colour, for {role}")),
        }
    }
    (theme, problems)
}

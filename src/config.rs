//! The user's settings file (ADR-0016): where it is, and the theme it chooses. Optional, and
//! written by uview only when asked, as a template, and never over a file that is there.

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

/// A settings file with every line commented out, each role at its colour in `theme`: what
/// `C` writes, to be edited.
pub fn template(theme: &Theme) -> String {
    let color = |color: Color| color.to_string().to_lowercase();
    format!(
        r##"# uview's settings. Every line is optional: uncomment one to change it, and start uview
# again to see it. https://github.com/levakuz/uview#colours

# The built-in theme to start from, "dark" or "light". Unset, uview asks the terminal for its
# background.
# theme = "light"

# A colour for any role: a name ("red", "lightred", "darkgray"), "reset" for the terminal's own,
# a number from 0 to 255, or "#rrggbb". Each is at the colour uview draws it with now.
[colors]
# text = "{}"    # the project's name, the tabs, the keys
# accent = "{}"    # what has the focus, what is running
# muted = "{}"    # hints, placeholders, borders without the focus
# warning = "{}"    # a stale uv.lock, a package behind
# error = "{}"
# success = "{}"    # notices, the active tab
"##,
        color(theme.text),
        color(theme.accent),
        color(theme.muted),
        color(theme.warning),
        color(theme.error),
        color(theme.success),
    )
}

/// `path` as the status line shows it: `~` for the home directory `home`.
pub fn shown(path: &Path, home: Option<&str>) -> String {
    match home.and_then(|home| path.strip_prefix(home).ok()) {
        Some(below) => format!("~/{}", below.display()),
        None => path.display().to_string(),
    }
}

/// Writes `contents` to `path`, and the directories above it, unless a file is there already:
/// then it fails with `ErrorKind::AlreadyExists` and leaves the file alone.
pub async fn write_new(path: &Path, contents: &str) -> std::io::Result<()> {
    use tokio::io::AsyncWriteExt;
    if let Some(dir) = path.parent() {
        tokio::fs::create_dir_all(dir).await?;
    }
    let mut file = tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .await?;
    file.write_all(contents.as_bytes()).await?;
    // tokio writes in the background: without this, a reader can find the file empty.
    file.flush().await
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

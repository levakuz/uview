use std::path::PathBuf;

use ratatui::style::Color;
use uview::config::{Base, Colors, Config, config_path, parse, themed};
use uview::ui::theme::Theme;

/// What xterm shows each of the 16 ANSI colours as: a palette to check contrast against. A
/// terminal with a palette of its own shows them otherwise.
fn xterm(color: Color) -> Option<(u8, u8, u8)> {
    Some(match color {
        Color::Black => (0x00, 0x00, 0x00),
        Color::Red => (0xcd, 0x00, 0x00),
        Color::Green => (0x00, 0xcd, 0x00),
        Color::Yellow => (0xcd, 0xcd, 0x00),
        Color::Blue => (0x00, 0x00, 0xee),
        Color::Magenta => (0xcd, 0x00, 0xcd),
        Color::Cyan => (0x00, 0xcd, 0xcd),
        Color::Gray => (0xe5, 0xe5, 0xe5),
        Color::DarkGray => (0x7f, 0x7f, 0x7f),
        Color::LightRed => (0xff, 0x00, 0x00),
        Color::LightGreen => (0x00, 0xff, 0x00),
        Color::LightYellow => (0xff, 0xff, 0x00),
        Color::LightBlue => (0x5c, 0x5c, 0xff),
        Color::LightMagenta => (0xff, 0x00, 0xff),
        Color::LightCyan => (0x00, 0xff, 0xff),
        Color::White => (0xff, 0xff, 0xff),
        _ => return None,
    })
}

/// WCAG's relative luminance.
fn luminance((r, g, b): (u8, u8, u8)) -> f64 {
    let channel = |c: u8| {
        let c = f64::from(c) / 255.0;
        if c <= 0.039_28 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b)
}

/// WCAG's contrast ratio of two colours, from 1 to 21.
fn contrast(a: (u8, u8, u8), b: (u8, u8, u8)) -> f64 {
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

/// The roles of `theme` that are text, then muted: text must reach 4.5, as WCAG asks of body
/// text; muted, what is there to be read past, 3.
fn roles(theme: &Theme) -> [(&'static str, Color, f64); 6] {
    [
        ("text", theme.text, 4.5),
        ("accent", theme.accent, 4.5),
        ("warning", theme.warning, 4.5),
        ("error", theme.error, 4.5),
        ("success", theme.success, 4.5),
        ("muted", theme.muted, 3.0),
    ]
}

#[test]
fn each_built_in_theme_reads_on_its_background() -> Result<(), String> {
    for (theme, background) in [
        (Theme::DARK, (0x00, 0x00, 0x00)),
        (Theme::LIGHT, (0xff, 0xff, 0xff)),
    ] {
        for (role, color, least) in roles(&theme) {
            // The terminal's own text colour reads on its own background.
            if color == Color::Reset {
                continue;
            }
            let rgb = xterm(color).ok_or_else(|| {
                format!("{role} is {color:?}, which is not one of the 16 ANSI colours")
            })?;
            let ratio = contrast(rgb, background);
            assert!(
                ratio >= least,
                "{role} ({color:?}) on {background:?}: {ratio:.2}, below {least}"
            );
        }
    }
    Ok(())
}

#[test]
fn the_config_file_is_where_uv_keeps_its_own() {
    let env = |pairs: &'static [(&'static str, &'static str)]| {
        move |key: &str| {
            pairs
                .iter()
                .find(|(k, _)| *k == key)
                .map(|(_, v)| (*v).to_string())
        }
    };
    assert_eq!(
        config_path(env(&[("HOME", "/home/jane")]), false),
        Some(PathBuf::from("/home/jane/.config/uview/config.toml"))
    );
    // `/home/jane/cfg` is absolute only where the test runs on Unix.
    #[cfg(unix)]
    assert_eq!(
        config_path(
            env(&[
                ("HOME", "/home/jane"),
                ("XDG_CONFIG_HOME", "/home/jane/cfg")
            ]),
            false
        ),
        Some(PathBuf::from("/home/jane/cfg/uview/config.toml"))
    );
    // A relative XDG_CONFIG_HOME is ignored, as the spec says.
    assert_eq!(
        config_path(
            env(&[("HOME", "/home/jane"), ("XDG_CONFIG_HOME", "cfg")]),
            false
        ),
        Some(PathBuf::from("/home/jane/.config/uview/config.toml"))
    );
    assert_eq!(
        config_path(env(&[("APPDATA", r"C:\Users\jane\AppData\Roaming")]), true),
        Some(
            PathBuf::from(r"C:\Users\jane\AppData\Roaming")
                .join("uview")
                .join("config.toml")
        )
    );
    assert_eq!(
        config_path(
            env(&[
                ("HOME", "/home/jane"),
                ("UVIEW_CONFIG_FILE", "/etc/uview.toml")
            ]),
            false
        ),
        Some(PathBuf::from("/etc/uview.toml"))
    );
    assert_eq!(config_path(env(&[]), false), None);
}

#[test]
fn the_file_chooses_a_theme_and_overrides_its_colours() -> Result<(), String> {
    let (config, unknown) = parse(
        r##"# Mine.
theme = "light"

[colors]
accent = "bright magenta"
success = "#2e7d32"
muted = "244"
"##,
    )?;
    assert!(unknown.is_empty());
    assert_eq!(config.theme, Some(Base::Light));
    let (theme, problems) = themed(Base::Light.theme(), &config.colors);
    assert!(problems.is_empty());
    assert_eq!(
        theme,
        Theme {
            accent: Color::LightMagenta,
            success: Color::Rgb(0x2e, 0x7d, 0x32),
            muted: Color::Indexed(244),
            ..Theme::LIGHT
        }
    );
    assert_eq!(parse("")?, (Config::default(), Vec::new()));
    Ok(())
}

#[test]
fn what_cannot_be_read_is_said_and_the_rest_kept() -> Result<(), String> {
    // A colour that does not parse keeps the theme's.
    let colors = Colors {
        error: Some("blu".into()),
        warning: Some("blue".into()),
        ..Colors::default()
    };
    let (theme, problems) = themed(Theme::DARK, &colors);
    assert_eq!(problems, [r#""blu" is no colour, for error"#]);
    assert_eq!(
        theme,
        Theme {
            warning: Color::Blue,
            ..Theme::DARK
        }
    );
    // A key that means nothing is told, and the rest of the file still counts.
    let (config, unknown) =
        parse("theme = \"light\"\n[colours]\naccent = \"blue\"\n[colors]\naccnet = \"red\"\n")?;
    assert_eq!(config.theme, Some(Base::Light));
    assert_eq!(
        unknown,
        [
            r#""colours" means nothing"#,
            r#""accnet" is no role in [colors]"#
        ]
    );
    // What keeps the file from being read at all is told too.
    assert!(parse("theme = \"solarized\"\n").is_err());
    assert!(parse("theme = \n").is_err());
    Ok(())
}

#[test]
fn the_template_changes_nothing_until_a_line_is_uncommented() -> Result<(), String> {
    use uview::config::template;
    let current = Theme {
        success: Color::Rgb(0x2e, 0x7d, 0x32),
        ..Theme::LIGHT
    };
    let text = template(&current);
    // As written, every line is a comment.
    assert_eq!(parse(&text)?, (Config::default(), Vec::new()));
    // Uncommented, each role is at the colour it was written from, and nothing is unknown.
    let uncommented: String = text
        .lines()
        .map(|line| {
            line.strip_prefix("# ")
                .filter(|l| l.contains(" = "))
                .unwrap_or(line)
        })
        .collect::<Vec<_>>()
        .join("\n");
    let (config, unknown) = parse(&uncommented)?;
    assert!(unknown.is_empty());
    assert_eq!(config.theme, Some(Base::Light));
    let (theme, problems) = themed(Theme::DARK, &config.colors);
    assert!(problems.is_empty());
    assert_eq!(theme, current);
    Ok(())
}

#[tokio::test]
async fn a_settings_file_that_is_there_is_never_written_over()
-> Result<(), Box<dyn std::error::Error>> {
    use uview::config::write_new;
    let dir = std::env::temp_dir().join(format!("uview-config-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let path = dir.join("uview").join("config.toml");
    write_new(&path, "theme = \"light\"\n").await?;
    assert_eq!(std::fs::read_to_string(&path)?, "theme = \"light\"\n");
    let again = write_new(&path, "theme = \"dark\"\n").await;
    assert!(again.is_err_and(|e| e.kind() == std::io::ErrorKind::AlreadyExists));
    assert_eq!(std::fs::read_to_string(&path)?, "theme = \"light\"\n");
    std::fs::remove_dir_all(&dir)?;
    Ok(())
}

#[test]
fn shift_c_asks_for_a_settings_file_in_the_tables_and_the_tree() {
    use crossterm::event::{KeyCode, KeyEvent};
    use uview::message::{Action, Input};
    use uview::ui::keymap::{Mode, action_for};
    let c = Input::Key(KeyEvent::from(KeyCode::Char('C')));
    for mode in [Mode::Table, Mode::Tree] {
        assert_eq!(action_for(mode, &c), Some(Action::WriteConfig), "{mode:?}");
    }
    // In an input, it is typed.
    assert!(matches!(
        action_for(Mode::Input, &c),
        Some(Action::Nav(uview::message::NavAction::Type(_)))
    ));
}

#[test]
fn the_file_is_shown_from_home() {
    use std::path::Path;
    use uview::config::shown;
    let path = Path::new("/home/jane/.config/uview/config.toml");
    assert_eq!(
        shown(path, Some("/home/jane")),
        "~/.config/uview/config.toml"
    );
    assert_eq!(
        shown(path, Some("/home/jo")),
        "/home/jane/.config/uview/config.toml"
    );
    assert_eq!(shown(path, None), "/home/jane/.config/uview/config.toml");
}

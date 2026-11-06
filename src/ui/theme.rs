//! The colours of the UI, by what they say rather than what they are. Both built-in themes use
//! only the 16 ANSI colours, so a terminal's own palette shows them and one limited to 16 colours
//! loses nothing; the user's config file can override any role (ADR-0016).

use std::sync::OnceLock;

use ratatui::style::Color;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Theme {
    /// What matters most: the project's name, the tabs, the keys in the footer.
    pub text: Color,
    /// What has the focus, and what is running.
    pub accent: Color,
    /// What is there to be read past: hints, placeholders, borders without the focus.
    pub muted: Color,
    /// Something to do: a stale uv.lock, a package behind, an input being edited.
    pub warning: Color,
    pub error: Color,
    /// What went well, and where you are: notices, the active tab, the help's headings.
    pub success: Color,
}

impl Theme {
    /// For a dark background.
    pub const DARK: Self = Self {
        text: Color::Reset,
        accent: Color::Cyan,
        muted: Color::DarkGray,
        warning: Color::Yellow,
        // Red is too dark on black.
        error: Color::LightRed,
        success: Color::Green,
    };

    /// For a light background, where yellow, cyan and green barely show.
    pub const LIGHT: Self = Self {
        text: Color::Reset,
        accent: Color::Blue,
        muted: Color::DarkGray,
        warning: Color::Magenta,
        error: Color::Red,
        success: Color::Blue,
    };
}

static THEME: OnceLock<Theme> = OnceLock::new();

/// Makes `theme` the one every view draws with. Only the first call counts: the theme is chosen
/// once, before the first frame.
pub fn set_theme(theme: Theme) {
    let _ = THEME.set(theme);
}

/// The theme the views draw with: the one set at startup, or the dark one.
pub fn theme() -> &'static Theme {
    THEME.get().unwrap_or(&Theme::DARK)
}

use ratatui::{
    Frame,
    layout::Rect,
    style::Stylize,
    text::Line,
    widgets::{Block, Paragraph},
};

use crate::ui::components::shared::popup::{Popup, PopupSize};
use crate::ui::keymap::{self, Mode};

/// The bottom line: the keys that work in `mode`, each with a word on what it does.
pub fn render_footer(frame: &mut Frame, area: Rect, mode: Mode) {
    let mut spans = Vec::new();
    for (keys, hint) in keymap::bindings(mode).flat_map(keymap::Binding::hints) {
        if !spans.is_empty() {
            spans.push("  ".into());
        }
        spans.push(keys.white());
        spans.push(format!(" {hint}").dark_gray());
    }
    frame.render_widget(Line::from(spans), area);
}

/// Every binding, by where it works, in a popup over `area`, `scroll` rows down. A `scroll` past
/// the last row is brought back to it.
pub fn render_help(frame: &mut Frame, area: Rect, scroll: &mut u16) {
    let groups = keymap::groups();
    let keys_width = groups
        .iter()
        .flat_map(|(_, bindings)| bindings.iter())
        .map(|binding| binding.help().0.chars().count())
        .max()
        .unwrap_or(0);
    let mut lines = Vec::new();
    for (place, bindings) in groups {
        lines.push(Line::from(place).green().bold());
        for (keys, help) in bindings.iter().map(keymap::Binding::help) {
            lines.push(Line::from(vec![
                format!("  {keys:keys_width$}  ").bold(),
                help.into(),
            ]));
        }
    }
    let width = lines.iter().map(Line::width).max().unwrap_or(0) + 2;
    let rows = u16::try_from(lines.len()).unwrap_or(u16::MAX);
    let popup = Popup::new(PopupSize {
        width: u16::try_from(width).unwrap_or(u16::MAX),
        height: rows.saturating_add(2).min(area.height),
    })
    .render(frame, area);
    let hidden = rows.saturating_sub(popup.height.saturating_sub(2));
    *scroll = (*scroll).min(hidden);
    let title = if hidden > 0 {
        "Keys (↑↓ to scroll)"
    } else {
        "Keys"
    };
    frame.render_widget(
        Paragraph::new(lines)
            .block(Block::bordered().title(title))
            .scroll((*scroll, 0)),
        popup,
    );
}

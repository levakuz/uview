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
    for (keys, hint) in keymap::bindings(mode).filter_map(keymap::Binding::hint) {
        if !spans.is_empty() {
            spans.push("  ".into());
        }
        spans.push(keys.white());
        spans.push(format!(" {hint}").dark_gray());
    }
    frame.render_widget(Line::from(spans), area);
}

/// Every binding, by where it works, in a popup over `area`.
/// ponytail: no scrolling. The bindings fit 80x24 with a row to spare; scroll when they do not.
pub fn render_help(frame: &mut Frame, area: Rect) {
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
    let height = lines.len() + 2;
    let popup = Popup::new(PopupSize {
        width: u16::try_from(width).unwrap_or(u16::MAX),
        height: u16::try_from(height).unwrap_or(u16::MAX),
    })
    .render(frame, area);
    frame.render_widget(
        Paragraph::new(lines).block(Block::bordered().title("Keys")),
        popup,
    );
}

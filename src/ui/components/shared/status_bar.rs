use std::time::Duration;

use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Style, Stylize},
    text::Line,
    widgets::Paragraph,
};

use crate::state::Message;
use crate::ui::theme::theme;

/// `message` on a single line: uv's stderr spans several, so they are flattened, and the
/// widget clips at the width.
fn render_message(frame: &mut Frame, area: Rect, message: &Message) {
    let (text, style) = match message {
        Message::Error(msg) => (
            format!("Error: {msg}"),
            Style::new().fg(theme().error).bold(),
        ),
        Message::Info(msg) => (msg.clone(), Style::new().fg(theme().success)),
    };
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    frame.render_widget(Paragraph::new(text).style(style), area);
}

/// How long the spinner shows each of its frames.
pub const SPINNER_FRAME: Duration = Duration::from_millis(80);

/// The frame of a spinner that has turned for `elapsed`.
pub fn spinner(elapsed: Duration) -> char {
    const FRAMES: [char; 10] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
    FRAMES[(elapsed.as_millis() / SPINNER_FRAME.as_millis() % 10) as usize]
}

/// The bottom line: what is running, if anything, then the latest message.
pub fn render_status_line(
    frame: &mut Frame,
    mut area: Rect,
    activity: Option<&str>,
    message: Option<&Message>,
) {
    if let Some(activity) = activity {
        let activity = Line::raw(activity).fg(theme().accent);
        let [shown, rest] = Layout::horizontal([
            Constraint::Length(activity.width() as u16),
            Constraint::Min(0),
        ])
        .spacing(2)
        .areas(area);
        frame.render_widget(activity, shown);
        area = rest;
    }
    if let Some(message) = message {
        render_message(frame, area, message);
    }
}

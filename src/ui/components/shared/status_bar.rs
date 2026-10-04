use std::time::Duration;

use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Style, Stylize},
    text::Line,
    widgets::Paragraph,
};

/// The latest error or notice, shown on a single line at the bottom of the screen.
pub enum StatusMessage {
    Error(String),
    Info(String),
}

impl StatusMessage {
    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let (text, style) = match self {
            StatusMessage::Error(msg) => (format!("Error: {msg}"), Style::new().red().bold()),
            StatusMessage::Info(msg) => (msg.clone(), Style::new().green()),
        };
        // uv's stderr spans several lines; flatten it, and let the widget clip at the width.
        let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
        frame.render_widget(Paragraph::new(text).style(style), area);
    }
}

/// The frame of a spinner that has turned for `elapsed`.
pub fn spinner(elapsed: Duration) -> char {
    const FRAMES: [char; 10] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
    FRAMES[(elapsed.as_millis() / 80 % 10) as usize]
}

/// The bottom line: on the left, what is running, if anything, then the latest message; on the
/// right, `context` in the default colour. `context` gets at most half the width and is cut from
/// the left, so its most specific end stays visible.
pub fn render_status_line(
    frame: &mut Frame,
    area: Rect,
    activity: Option<&str>,
    message: Option<&StatusMessage>,
    context: &str,
) {
    let context = Line::raw(context).right_aligned();
    let width = context.width().min(usize::from(area.width / 2)) as u16;
    let [mut left, right] = Layout::horizontal([Constraint::Min(0), Constraint::Length(width)])
        .spacing(1)
        .areas(area);
    if let Some(activity) = activity {
        let activity = Line::raw(activity).cyan();
        let [shown, rest] = Layout::horizontal([
            Constraint::Length(activity.width() as u16),
            Constraint::Min(0),
        ])
        .spacing(2)
        .areas(left);
        frame.render_widget(activity, shown);
        left = rest;
    }
    if let Some(message) = message {
        message.render(frame, left);
    }
    frame.render_widget(context, right);
}

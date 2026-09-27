use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::Style,
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

/// The bottom line: the latest message, if any, on the left, and `context` on the right in
/// the default colour. `context` gets at most half the width and is cut from the left, so its
/// most specific end stays visible.
pub fn render_status_line(
    frame: &mut Frame,
    area: Rect,
    message: Option<&StatusMessage>,
    context: &str,
) {
    let context = Line::raw(context).right_aligned();
    let width = context.width().min(usize::from(area.width / 2)) as u16;
    let [left, right] = Layout::horizontal([Constraint::Min(0), Constraint::Length(width)])
        .spacing(1)
        .areas(area);
    if let Some(message) = message {
        message.render(frame, left);
    }
    frame.render_widget(context, right);
}

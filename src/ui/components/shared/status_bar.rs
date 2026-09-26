use ratatui::{Frame, layout::Rect, style::Style, widgets::Paragraph};

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

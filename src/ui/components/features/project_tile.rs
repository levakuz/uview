use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::Style,
    text::Line,
    widgets::{Block, Paragraph, Wrap},
};

/// Draws the project tile at the top of `column`, as tall as the wrapped `root` needs, and
/// returns the rest of the column.
pub fn render_project_tile(frame: &mut Frame, column: Rect, root: &str) -> Rect {
    let inner_width = column.width.saturating_sub(2).max(1);
    let rows = u16::try_from(Line::raw(root).width())
        .unwrap_or(u16::MAX)
        .div_ceil(inner_width)
        .max(1);
    let [tile, rest] = Layout::vertical([
        Constraint::Length(rows.saturating_add(2)),
        Constraint::Min(0),
    ])
    .areas(column);
    let block = Block::bordered()
        .title("Project")
        .border_style(Style::new().dark_gray());
    frame.render_widget(
        Paragraph::new(root).wrap(Wrap { trim: false }).block(block),
        tile,
    );
    rest
}

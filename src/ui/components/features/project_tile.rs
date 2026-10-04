use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Style, Stylize},
    text::{Line, Text},
    widgets::{Block, Paragraph, Wrap},
};

use crate::state::{Loadable, ProjectState};

/// Draws the project tile at the top of `column`, as tall as its wrapped text needs, and
/// returns the rest of the column. The project's name and version are its title once uv has
/// read them. If it could not, what uv said shows dimmed above the root.
pub fn render_project_tile(frame: &mut Frame, column: Rect, project: &ProjectState) -> Rect {
    let title = match &project.info {
        // White, as the tabs are: a title is otherwise as dim as the border.
        Loadable::Loaded(info) => Line::from(vec![
            " ".into(),
            info.name.as_str().white().bold(),
            format!(" {} ", info.version).white(),
        ]),
        _ => Line::raw("Project"),
    };
    let mut text = Text::default();
    if let Loadable::Failed(reason) = &project.info {
        // The first line says what is wrong; what follows it would crowd out the uv log.
        text.push_line(Line::raw(reason.lines().next().unwrap_or_default()).dark_gray());
    }
    text.push_line(project.root.as_str());
    let block = Block::bordered()
        .title(title)
        .border_style(Style::new().dark_gray());
    let paragraph = Paragraph::new(text).wrap(Wrap { trim: false }).block(block);
    // Counts the block's top and bottom, but not its sides.
    let rows = paragraph.line_count(column.width.saturating_sub(2));
    let [tile, rest] = Layout::vertical([
        Constraint::Length(u16::try_from(rows).unwrap_or(u16::MAX)),
        Constraint::Min(0),
    ])
    .areas(column);
    frame.render_widget(paragraph, tile);
    rest
}

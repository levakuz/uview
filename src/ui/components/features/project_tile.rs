use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Style, Stylize},
    text::{Line, Text},
    widgets::{Block, Paragraph, Wrap},
};

use crate::domain::LockState;
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
    if let Some(line) = lock_line(&project.lock) {
        text.push_line(line);
    }
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

/// What the tile says of uv.lock, once a reload has checked it: dim when there is nothing to
/// do, yellow with the key to press when there is.
fn lock_line(lock: &Loadable<LockState>) -> Option<Line<'static>> {
    let Loadable::Loaded(state) = lock else {
        return None;
    };
    Some(match state {
        LockState::Fresh => Line::raw("uv.lock up to date").dark_gray(),
        LockState::Stale => Line::raw("uv.lock out of date — press l").yellow(),
        LockState::Missing => Line::raw("no uv.lock — press l").yellow(),
        LockState::Unknown => Line::raw("uv.lock not checked: it needs the network").dark_gray(),
    })
}

use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Style, Stylize},
    text::{Line, Text},
    widgets::{Block, Paragraph, Wrap},
};

use crate::domain::{EnvInfo, LockState};
use crate::state::{Loadable, ProjectState};
use crate::ui::theme::theme;

/// Draws the project tile at the top of `column`, as tall as its wrapped text needs, and
/// returns the rest of the column. The project's name and version are its title once uv has
/// read them. If it could not, what uv said shows dimmed above the root. `packages` is how many
/// the environment holds, once listed.
pub fn render_project_tile(
    frame: &mut Frame,
    column: Rect,
    project: &ProjectState,
    packages: Option<usize>,
) -> Rect {
    let title = match &project.info {
        // White, as the tabs are: a title is otherwise as dim as the border.
        Loadable::Loaded(info) => Line::from(vec![
            " ".into(),
            info.name.as_str().fg(theme().text).bold(),
            format!(" {} ", info.version).fg(theme().text),
        ]),
        _ => Line::raw("Project"),
    };
    let mut text = Text::default();
    if let Loadable::Failed(reason) = &project.info {
        // The first line says what is wrong; what follows it would crowd out the uv log.
        text.push_line(Line::raw(reason.lines().next().unwrap_or_default()).fg(theme().muted));
    }
    text.push_line(project.root.as_str());
    text.push_line(environment_line(&project.environment, packages));
    if let Some(line) = lock_line(&project.lock) {
        text.push_line(line);
    }
    let block = Block::bordered()
        .title(title)
        .border_style(Style::new().fg(theme().muted));
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
        LockState::Fresh => Line::raw("uv.lock up to date").fg(theme().muted),
        LockState::Stale => Line::raw("uv.lock out of date — press l").fg(theme().warning),
        LockState::Missing => Line::raw("no uv.lock — press l").fg(theme().warning),
        LockState::Unknown => {
            Line::raw("uv.lock not checked: it needs the network").fg(theme().muted)
        }
    })
}

/// The environment as the tile says it, dim: `.venv · Python 3.11.15 · 48 packages · 41.2 MiB`.
/// Quiet whatever happens: `…` while it loads, `no environment` without one, `size unknown`
/// when it could not be measured.
fn environment_line(
    environment: &Loadable<Option<EnvInfo>>,
    packages: Option<usize>,
) -> Line<'static> {
    let info = match environment {
        Loadable::Loaded(Some(info)) => info,
        Loadable::Loaded(None) => return Line::raw("no environment").fg(theme().muted),
        _ => return Line::raw("…").fg(theme().muted),
    };
    let mut parts = vec![info.shown_path.clone()];
    parts.extend(
        info.python
            .as_ref()
            .map(|python| format!("Python {python}")),
    );
    parts.extend(packages.map(|n| match n {
        1 => "1 package".to_string(),
        n => format!("{n} packages"),
    }));
    parts.push(info.size.map_or("size unknown".into(), human_size));
    Line::raw(parts.join(" · ")).fg(theme().muted)
}

/// `bytes` in binary units with one decimal, as `41.2 MiB`.
pub fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut size = bytes as f64;
    let mut unit = 0;
    while size >= 1024.0 && unit + 1 < UNITS.len() {
        size /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{size:.1} {}", UNITS[unit])
    }
}

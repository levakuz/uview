use ratatui::{
    Frame,
    layout::Rect,
    style::{Style, Stylize},
    text::Line,
    widgets::{Block, Paragraph},
};

use crate::runtime::jobs::{JobsState, LogLine};

/// The uv log: each job's label, then what its `uv` wrote, the newest line at the bottom.
pub fn render_uv_log(frame: &mut Frame, area: Rect, jobs: &JobsState) {
    let block = Block::bordered()
        .title("uv")
        .border_style(Style::new().dark_gray());
    let log = jobs.log();
    let shown = usize::from(block.inner(area).height);
    let lines: Vec<Line> = log
        .range(log.len().saturating_sub(shown)..)
        .map(|line| match line {
            LogLine::Job(label) => Line::raw(label.as_str()).cyan(),
            LogLine::Output(text) => Line::raw(text.as_str()),
        })
        .collect();
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

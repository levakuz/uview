use ratatui::{
    Frame,
    layout::Rect,
    style::{Style, Stylize},
    text::Line,
    widgets::{Block, Paragraph},
};

use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::state::{JobsState, LogLine};
use crate::ui::theme::theme;

/// The uv log: each job's label, then what its `uv` wrote, the newest line at the bottom. A line
/// wider than the log wraps. Until `uv` has written anything, a hint says what will appear.
pub fn render_uv_log(frame: &mut Frame, area: Rect, jobs: &JobsState) {
    let block = Block::bordered()
        .title("uv")
        .border_style(Style::new().fg(theme().muted));
    let inner = block.inner(area);
    let (shown, width) = (usize::from(inner.height), usize::from(inner.width));
    let log = jobs.log();
    let lines: Vec<Line> = if log.is_empty() {
        vec![Line::raw("uv output will appear here").fg(theme().muted)]
    } else {
        // From the newest line back, until the log is full.
        let mut rows: Vec<Line> = Vec::new();
        for line in log.iter().rev() {
            if rows.len() >= shown {
                break;
            }
            let (text, style) = match line {
                LogLine::Job(label) => (label.as_str(), Style::new().fg(theme().accent)),
                LogLine::Output(text) => (text.as_str(), Style::new()),
            };
            for row in wrap(text, width).into_iter().rev() {
                rows.push(Line::styled(row, style));
            }
        }
        rows.truncate(shown);
        rows.reverse();
        rows
    };
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

/// `text` in rows at most `width` wide: broken at spaces where it can, inside a word where it
/// must. Every row keeps the text's leading indent, unless that would take half the width.
pub fn wrap(text: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let body = text.trim_start();
    let mut indent = &text[..text.len() - body.len()];
    if indent.width() * 2 > width {
        indent = "";
    }
    let mut rows = Vec::new();
    let mut row = indent.to_string();
    for word in body.split(' ').filter(|word| !word.is_empty()) {
        let started = row.len() > indent.len();
        if row.width() + usize::from(started) + word.width() <= width {
            if started {
                row.push(' ');
            }
            row.push_str(word);
            continue;
        }
        if started {
            rows.push(std::mem::replace(&mut row, indent.to_string()));
        }
        // Too wide even for a row of its own: broken where the row ends.
        for c in word.chars() {
            if row.len() > indent.len() && row.width() + c.width().unwrap_or(0) > width {
                rows.push(std::mem::replace(&mut row, indent.to_string()));
            }
            row.push(c);
        }
    }
    rows.push(row);
    rows
}

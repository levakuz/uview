use std::collections::HashSet;

use ratatui::{
    Frame,
    layout::Rect,
    style::{Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, Borders, List, ListState, Paragraph, Wrap},
};

/// A row of a tree: where it sits, and how it reads.
pub trait TreeRow {
    /// How deep it is: 0 for a root, one more than its parent below it.
    fn depth(&self) -> usize;
    /// What tells it from its siblings, from one load of the rows to the next.
    fn key(&self) -> &str;
    fn line(&self) -> Line<'static>;
}

/// A tree of rows it does not own, handed as a flat list in order, each with its depth. It keeps
/// only the view: which rows are folded, which is highlighted, and the scroll. Rows are known by
/// their path of keys from the root, so a reload that moves them keeps both.
#[derive(Default)]
pub struct TreeView {
    folded: HashSet<String>,
    /// The highlighted row's path.
    selected: Option<String>,
    state: ListState,
}

impl TreeView {
    /// Each row's path: the keys from its root down to it.
    fn paths<T: TreeRow>(rows: &[T]) -> Vec<String> {
        let mut above: Vec<String> = Vec::new();
        rows.iter()
            .map(|row| {
                above.truncate(row.depth());
                let path = match above.last() {
                    Some(parent) => format!("{parent}/{}", row.key()),
                    None => row.key().to_string(),
                };
                above.push(path.clone());
                path
            })
            .collect()
    }

    fn has_children<T: TreeRow>(rows: &[T], i: usize) -> bool {
        rows.get(i + 1)
            .zip(rows.get(i))
            .is_some_and(|(next, row)| next.depth() > row.depth())
    }

    /// The rows shown, as indexes into `rows`: none below a folded one.
    pub fn visible<T: TreeRow>(&self, rows: &[T]) -> Vec<usize> {
        let paths = Self::paths(rows);
        let mut hidden_below: Option<usize> = None;
        let mut shown = Vec::new();
        for (i, row) in rows.iter().enumerate() {
            if hidden_below.is_some_and(|depth| row.depth() > depth) {
                continue;
            }
            hidden_below = None;
            shown.push(i);
            if self.folded.contains(&paths[i]) && Self::has_children(rows, i) {
                hidden_below = Some(row.depth());
            }
        }
        shown
    }

    /// The highlighted row, as an index into `rows`: the one last highlighted if it is still
    /// shown, else the first.
    pub fn selected<T: TreeRow>(&self, rows: &[T]) -> Option<usize> {
        let shown = self.visible(rows);
        let paths = Self::paths(rows);
        self.selected
            .as_ref()
            .and_then(|path| shown.iter().copied().find(|&i| &paths[i] == path))
            .or_else(|| shown.first().copied())
    }

    fn select<T: TreeRow>(&mut self, rows: &[T], i: usize) {
        self.selected = Self::paths(rows).get(i).cloned();
    }

    /// Moves the highlight by `step` shown rows, stopping at either end.
    fn step<T: TreeRow>(&mut self, rows: &[T], forward: bool) {
        let shown = self.visible(rows);
        let Some(at) = self
            .selected(rows)
            .and_then(|i| shown.iter().position(|&s| s == i))
        else {
            return;
        };
        let next = if forward {
            (at + 1).min(shown.len().saturating_sub(1))
        } else {
            at.saturating_sub(1)
        };
        if let Some(&i) = shown.get(next) {
            self.select(rows, i);
        }
    }

    pub fn up<T: TreeRow>(&mut self, rows: &[T]) {
        self.step(rows, false);
    }

    pub fn down<T: TreeRow>(&mut self, rows: &[T]) {
        self.step(rows, true);
    }

    /// Unfolds the highlighted row.
    pub fn expand<T: TreeRow>(&mut self, rows: &[T]) {
        if let Some(i) = self.selected(rows) {
            self.folded.remove(&Self::paths(rows)[i]);
        }
    }

    /// Folds the highlighted row, or, when it is folded or has nothing to fold, moves to its
    /// parent.
    pub fn collapse<T: TreeRow>(&mut self, rows: &[T]) {
        let Some(i) = self.selected(rows) else {
            return;
        };
        let paths = Self::paths(rows);
        if Self::has_children(rows, i) && !self.folded.contains(&paths[i]) {
            self.folded.insert(paths[i].clone());
            return;
        }
        let depth = rows[i].depth();
        if let Some(parent) = (0..i).rev().find(|&p| rows[p].depth() < depth) {
            self.select(rows, parent);
        }
    }

    pub fn expand_all(&mut self) {
        self.folded.clear();
    }

    /// Folds every row below the roots that has rows under it, leaving the roots and their
    /// children shown.
    pub fn collapse_all<T: TreeRow>(&mut self, rows: &[T]) {
        let paths = Self::paths(rows);
        self.folded = (0..rows.len())
            .filter(|&i| rows[i].depth() > 0 && Self::has_children(rows, i))
            .map(|i| paths[i].clone())
            .collect();
        // A highlighted row now hidden gives way to its shown ancestor.
        if let Some(i) = self.selected(rows) {
            self.select(rows, i);
        }
    }

    /// Draws the shown rows in a bordered box titled `title`, each indented by its depth and
    /// marked `▾` when unfolded or `▸` when folded if it has rows under it; or `placeholder`
    /// when there are none.
    pub fn render<T: TreeRow>(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        rows: &[T],
        title: Line<'static>,
        placeholder: Text<'static>,
    ) {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(Style::new().cyan());
        if rows.is_empty() {
            let paragraph = Paragraph::new(placeholder)
                .wrap(Wrap { trim: false })
                .block(block);
            frame.render_widget(paragraph, area);
            return;
        }
        let paths = Self::paths(rows);
        let shown = self.visible(rows);
        let selected = self.selected(rows);
        if let Some(i) = selected {
            self.select(rows, i);
        }
        let items: Vec<Line> = shown
            .iter()
            .map(|&i| {
                let marker = match (Self::has_children(rows, i), self.folded.contains(&paths[i])) {
                    (false, _) => "  ",
                    (true, false) => "▾ ",
                    (true, true) => "▸ ",
                };
                let mut spans = vec![Span::raw("  ".repeat(rows[i].depth())), marker.dark_gray()];
                spans.extend(rows[i].line().spans);
                Line::from(spans)
            })
            .collect();
        self.state
            .select(selected.and_then(|i| shown.iter().position(|&s| s == i)));
        let list = List::new(items)
            .block(block)
            .highlight_style(Style::new().reversed());
        frame.render_stateful_widget(list, area, &mut self.state);
    }
}

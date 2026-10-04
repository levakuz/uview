use std::marker::PhantomData;

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::Text;
use ratatui::widgets::{Block, Borders, Paragraph, Row, Table, TableState, Wrap};

pub trait TableRow {
    fn columns() -> Vec<&'static str>;
    fn cells(&self) -> Vec<String>;
    /// What identifies the row from one load of the rows to the next, such as a package name.
    fn key(&self) -> &str;
}

/// A table of `T` rows it does not own: each call is handed the rows as they are now. It keeps
/// only the view: the selection and the scroll offset.
pub struct SharedTable<T: TableRow> {
    state: TableState,
    /// The key of the selected row, which the selection follows when the rows change.
    selected_key: Option<String>,
    title: String,
    focused: bool,
    rows: PhantomData<fn(&T)>,
}

impl<T: TableRow> SharedTable<T> {
    pub fn new(title: &str) -> Self {
        Self {
            state: TableState::default(),
            selected_key: None,
            title: title.to_string(),
            focused: false,
            rows: PhantomData,
        }
    }

    /// The selected row among `rows`.
    pub fn selected<'a>(&self, rows: &'a [T]) -> Option<&'a T> {
        rows.get(self.index(rows)?)
    }

    /// Where the selection is among `rows`: on the row it was on, wherever that is now. Failing
    /// that, in the same place, the first row until one is chosen, and the last when the rows
    /// have shrunk below it.
    fn index(&self, rows: &[T]) -> Option<usize> {
        let last = rows.len().checked_sub(1)?;
        let key = self.selected_key.as_deref();
        let same_row = key.and_then(|key| rows.iter().position(|row| row.key() == key));
        Some(same_row.unwrap_or_else(|| self.state.selected().unwrap_or(0).min(last)))
    }

    fn select(&mut self, rows: &[T], index: usize) {
        self.state.select(Some(index));
        self.selected_key = rows.get(index).map(|row| row.key().to_string());
    }

    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    /// Selects the next of `rows`, wrapping around after the last.
    pub fn next(&mut self, rows: &[T]) {
        if let Some(i) = self.index(rows) {
            self.select(rows, (i + 1) % rows.len());
        }
    }

    /// Selects the previous of `rows`, wrapping around before the first.
    pub fn previous(&mut self, rows: &[T]) {
        if let Some(i) = self.index(rows) {
            self.select(rows, i.checked_sub(1).unwrap_or(rows.len() - 1));
        }
    }

    /// Draws `rows` under the header, or `placeholder` when there are none.
    pub fn render(&mut self, frame: &mut Frame, area: Rect, rows: &[T], placeholder: Text) {
        // The highlight follows the selection as `selected` sees it, and from now on, so does its
        // row's key.
        match self.index(rows) {
            Some(i) => self.select(rows, i),
            None => self.state.select(None),
        }
        let header_cells = T::columns();
        let num_cols = header_cells.len() as u16;
        let header = Row::new(header_cells).style(Style::new().bold());

        let table_rows: Vec<Row> = rows.iter().map(|row| Row::new(row.cells())).collect();

        let widths: Vec<Constraint> = (0..num_cols)
            .map(|_| Constraint::Percentage(100 / num_cols))
            .collect();

        let border_style = if self.focused {
            Style::new().cyan()
        } else {
            Style::new().dark_gray()
        };

        let highlight_style = if self.focused {
            Style::new().reversed()
        } else {
            Style::new()
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .title(self.title.as_str())
            .border_style(border_style);
        let [_, below_header] =
            Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(block.inner(area));
        let table = Table::new(table_rows, &widths)
            .header(header)
            .block(block)
            .row_highlight_style(highlight_style);

        frame.render_stateful_widget(table, area, &mut self.state);
        if rows.is_empty() {
            let placeholder = Paragraph::new(placeholder).wrap(Wrap { trim: false });
            frame.render_widget(placeholder, below_header);
        }
    }
}

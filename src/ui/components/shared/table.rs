use std::marker::PhantomData;

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::Text;
use ratatui::widgets::{Block, Borders, Paragraph, Row, Table, TableState, Wrap};

pub trait TableRow {
    fn columns() -> Vec<&'static str>;
    fn cells(&self) -> Vec<String>;
}

/// A table of `T` rows it does not own: each call is handed the rows as they are now. It keeps
/// only the view: the selection and the scroll offset.
pub struct SharedTable<T: TableRow> {
    state: TableState,
    title: String,
    focused: bool,
    rows: PhantomData<fn(&T)>,
}

impl<T: TableRow> SharedTable<T> {
    pub fn new(title: &str) -> Self {
        Self {
            state: TableState::default(),
            title: title.to_string(),
            focused: false,
            rows: PhantomData,
        }
    }

    /// The selected row among `rows`.
    pub fn selected<'a>(&self, rows: &'a [T]) -> Option<&'a T> {
        rows.get(self.index(rows.len())?)
    }

    /// Where the selection is among `len` rows: the first until one is chosen, and the last when
    /// the rows have shrunk below it.
    fn index(&self, len: usize) -> Option<usize> {
        let last = len.checked_sub(1)?;
        Some(self.state.selected().unwrap_or(0).min(last))
    }

    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    /// Selects the next of `len` rows, wrapping around after the last.
    pub fn next(&mut self, len: usize) {
        if let Some(i) = self.index(len) {
            self.state.select(Some((i + 1) % len));
        }
    }

    /// Selects the previous of `len` rows, wrapping around before the first.
    pub fn previous(&mut self, len: usize) {
        if let Some(i) = self.index(len) {
            self.state.select(Some(i.checked_sub(1).unwrap_or(len - 1)));
        }
    }

    /// Draws `rows` under the header, or `placeholder` when there are none.
    pub fn render(&mut self, frame: &mut Frame, area: Rect, rows: &[T], placeholder: Text) {
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

        // The highlight follows the selection as `selected` sees it.
        self.state.select(self.index(rows.len()));
        frame.render_stateful_widget(table, area, &mut self.state);
        if rows.is_empty() {
            let placeholder = Paragraph::new(placeholder).wrap(Wrap { trim: false });
            frame.render_widget(placeholder, below_header);
        }
    }
}

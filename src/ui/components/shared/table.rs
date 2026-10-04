use ratatui::Frame;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::Style;
use ratatui::widgets::{Block, Borders, Row, Table, TableState};

pub trait TableRow {
    fn columns() -> Vec<&'static str>;
    fn cells(&self) -> Vec<String>;
}

pub struct SharedTable<T: TableRow> {
    items: Vec<T>,
    state: TableState,
    title: String,
    focused: bool,
}

impl<T: TableRow> SharedTable<T> {
    pub fn new(title: &str) -> Self {
        Self {
            items: Vec::new(),
            state: TableState::default(),
            title: title.to_string(),
            focused: false,
        }
    }

    pub fn selected(&self) -> Option<&T> {
        self.items.get(self.state.selected()?)
    }

    /// Replaces the rows, keeping the selected index where it still exists.
    pub fn set_items(&mut self, items: Vec<T>) {
        self.items = items;
        let selected = match self.items.len() {
            0 => None,
            len => Some(self.state.selected().unwrap_or(0).min(len - 1)),
        };
        self.state.select(selected);
    }

    pub fn find<F: Fn(&T) -> bool>(&self, pred: F) -> Option<&T> {
        self.items.iter().find(|item| pred(item))
    }

    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    pub fn next(&mut self) {
        if self.items.is_empty() {
            return;
        }
        let i = match self.state.selected() {
            Some(i) => (i + 1) % self.items.len(),
            None => 0,
        };
        self.state.select(Some(i));
    }

    pub fn previous(&mut self) {
        if self.items.is_empty() {
            return;
        }
        let i = match self.state.selected() {
            Some(i) => {
                if i == 0 {
                    self.items.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        let header_cells = T::columns();
        let num_cols = header_cells.len() as u16;
        let header = Row::new(header_cells).style(Style::new().bold());

        let rows: Vec<Row> = self
            .items
            .iter()
            .map(|item| Row::new(item.cells()))
            .collect();

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

        let table = Table::new(rows, &widths)
            .header(header)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(self.title.as_str())
                    .border_style(border_style),
            )
            .row_highlight_style(highlight_style);

        frame.render_stateful_widget(table, area, &mut self.state);
    }
}

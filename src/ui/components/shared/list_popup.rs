use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    widgets::{Block, List, ListState},
};

use crate::ui::components::shared::popup::{Popup, PopupSize};

/// A titled list of choices in a popup, one of them highlighted.
pub struct ListPopup {
    popup: Popup,
    title: String,
    items: Vec<String>,
    state: ListState,
}

impl ListPopup {
    /// Opens with the first item highlighted.
    pub fn new(title: &str, items: Vec<String>) -> Self {
        let widest = items.iter().map(|item| item.chars().count()).max();
        let width = widest.unwrap_or(0).max(title.chars().count()) + 4;
        Self {
            // The items inside a border.
            popup: Popup::new(PopupSize {
                width: u16::try_from(width).unwrap_or(u16::MAX),
                height: u16::try_from(items.len() + 2).unwrap_or(u16::MAX),
            }),
            title: title.to_string(),
            items,
            state: ListState::default().with_selected(Some(0)),
        }
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        let popup_area = self.popup.render(frame, area);
        let list = List::new(self.items.clone())
            .block(Block::bordered().title(self.title.clone()))
            .highlight_style(Style::default().reversed());
        frame.render_stateful_widget(list, popup_area, &mut self.state);
    }

    pub fn up(&mut self) {
        self.state.select_previous();
    }

    pub fn down(&mut self) {
        // `select_next` runs past the end until the next render clamps it.
        if self.selected().is_some_and(|i| i + 1 < self.items.len()) {
            self.state.select_next();
        }
    }

    /// The index of the highlighted item.
    pub fn selected(&self) -> Option<usize> {
        self.state.selected().filter(|&i| i < self.items.len())
    }
}

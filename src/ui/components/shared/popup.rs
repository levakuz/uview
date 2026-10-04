use ratatui::{
    Frame,
    layout::{Constraint, Flex, Layout, Rect},
    widgets::Clear,
};

pub struct PopupSize {
    pub width: u16,
    pub height: u16,
}

pub struct Popup {
    size: PopupSize,
}

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let [area] = Layout::horizontal([Constraint::Length(width)])
        .flex(Flex::Center)
        .areas(area);
    let [area] = Layout::vertical([Constraint::Length(height)])
        .flex(Flex::Center)
        .areas(area);
    area
}

impl Popup {
    pub fn new(size: PopupSize) -> Self {
        Self { size }
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect) -> Rect {
        let popup_area = centered_rect(self.size.width, self.size.height, area);

        frame.render_widget(Clear, popup_area);
        popup_area
    }
}

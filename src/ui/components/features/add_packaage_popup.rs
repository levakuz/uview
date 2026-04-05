use ratatui::{Frame, layout::Rect};

use crate::{
    events::Action,
    ui::components::shared::{
        input::UviewInput,
        popup::{Popup, PopupSize},
    },
};

pub struct AddPackagePopup {
    popup: Popup,
    input: UviewInput,
}

impl<'a> AddPackagePopup {
    pub fn new() -> Self {
        let input = UviewInput::default();

        Self {
            popup: Popup::new(PopupSize {
                width: 40,
                height: 10,
            }),
            input,
        }
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        let popup_area = self.popup.render(frame, area);
        self.input.render(frame, popup_area);
    }

    pub fn is_editing(&self) -> bool {
        self.input.is_editing()
    }

    pub fn handle_event(&mut self, action: &Action) -> Option<Action> {
        self.input.handle_event(action);
        return self.popup.handle_event(action);
    }
}

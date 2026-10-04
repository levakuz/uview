use ratatui::{Frame, layout::Rect};

use crate::{
    events::Action,
    ui::components::shared::{
        input::UviewInput,
        popup::{Popup, PopupSize},
    },
};

pub struct PinVersionPopup {
    popup: Popup,
    input: UviewInput,
}

impl PinVersionPopup {
    pub fn new() -> Self {
        let input = UviewInput::default().with_title("Version (e.g. ==1.2.3, >=2.0, <3.0)");

        Self {
            popup: Popup::new(PopupSize {
                width: 50,
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
        if let Action::ConfirmInput = action {
            if self.input.is_editing() {
                let constraint = self.input.take_value();
                // Enter alone reaches an empty field from a table row; it must not run `uv add`.
                if constraint.trim().is_empty() {
                    return None;
                }
                self.input.handle_event(&Action::StopEditing);
                return Some(Action::PinVersion(constraint));
            }
            return None;
        }
        self.input.handle_event(action);
        self.popup.handle_event(action)
    }
}

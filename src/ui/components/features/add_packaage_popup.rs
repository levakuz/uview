use ratatui::{Frame, layout::Rect};

use crate::{
    message::NavAction,
    ui::components::shared::{
        input::UviewInput,
        popup::{Popup, PopupSize},
    },
};

pub struct AddPackagePopup {
    popup: Popup,
    input: UviewInput,
}

impl AddPackagePopup {
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

    /// Opens the input, empty.
    pub fn open(&mut self) {
        self.input.start_editing();
    }

    /// Handles a key while the input is open.
    pub fn update(&mut self, nav: &NavAction) {
        match nav {
            // Not wired to `uv add` yet (#8): Enter only clears the field.
            NavAction::Confirm => {
                self.input.take_value();
            }
            NavAction::Back => self.input.stop_editing(),
            NavAction::Type(key) => self.input.key(*key),
            // While an input is open the keymap sends only Confirm, Back and Type.
            _ => {}
        }
    }
}

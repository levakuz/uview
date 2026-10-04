use ratatui::{Frame, layout::Rect};

use crate::{
    message::NavAction,
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

    /// Opens the input, empty.
    pub fn open(&mut self) {
        self.input.start_editing();
    }

    /// Handles a key while the input is open. Returns the version once Enter confirms one,
    /// which also closes the input.
    pub fn update(&mut self, nav: &NavAction) -> Option<String> {
        match nav {
            NavAction::Confirm => {
                let constraint = self.input.take_value();
                // Enter alone reaches an empty field from a table row; it must not run `uv add`.
                if constraint.trim().is_empty() {
                    return None;
                }
                self.input.stop_editing();
                Some(constraint)
            }
            NavAction::Back => {
                self.input.stop_editing();
                None
            }
            NavAction::Type(key) => {
                self.input.key(*key);
                None
            }
            // While an input is open the keymap sends only Confirm, Back and Type.
            _ => None,
        }
    }
}

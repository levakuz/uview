use ratatui::{Frame, layout::Rect};

use crate::{
    message::NavAction,
    ui::components::shared::{
        input::UviewInput,
        popup::{Popup, PopupSize},
    },
};

/// A popup with one input: Enter hands back what was typed, Esc cancels.
pub struct InputPopup {
    popup: Popup,
    input: UviewInput,
}

impl InputPopup {
    pub fn new(title: &str, width: u16) -> Self {
        Self {
            // One line to type on, inside a border.
            popup: Popup::new(PopupSize { width, height: 3 }),
            input: UviewInput::default().with_title(title),
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

    /// Opens the input with `value` typed in, titled `title`: what was typed came back with a
    /// problem, which the title says.
    pub fn reopen(&mut self, title: &str, value: &str) {
        self.input = UviewInput::default().with_title(title).with_value(value);
        self.input.start_editing();
    }

    /// Handles a key while the input is open. Returns the text once Enter confirms some,
    /// which also closes the input.
    pub fn update(&mut self, nav: &NavAction) -> Option<String> {
        match nav {
            NavAction::Confirm => {
                let text = self.input.take_value();
                // An empty field confirms nothing: Enter alone reaches the version input from a
                // table row, and it must not run `uv add`.
                if text.trim().is_empty() {
                    return None;
                }
                self.input.stop_editing();
                Some(text)
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

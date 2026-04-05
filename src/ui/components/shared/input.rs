use ratatui::{
    Frame,
    crossterm::event::Event,
    layout::Rect,
    style::{Color, Style, Stylize},
    text::{Line, ToSpan},
    widgets::{Block, List, Paragraph},
};
use tui_input::Input;
use tui_input::backend::crossterm::EventHandler;

use crate::events::Action;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum InputMode {
    #[default]
    Normal,
    Editing,
}

#[derive(Debug)]
pub struct UviewInput {
    /// Current value of the input box
    input: Input,
    /// Current input mode
    input_mode: InputMode,
    /// History of recorded messages
    messages: Vec<String>,
    /// Title shown on the input box border
    title: String,
}

impl Default for UviewInput {
    fn default() -> Self {
        Self {
            input: Input::default(),
            input_mode: InputMode::default(),
            messages: Vec::new(),
            title: "Input".to_string(),
        }
    }
}

impl UviewInput {
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    fn start_editing(&mut self) {
        self.input_mode = InputMode::Editing
    }

    fn stop_editing(&mut self) {
        self.input_mode = InputMode::Normal
    }

    fn push_message(&mut self) {
        self.messages.push(self.input.value_and_reset());
    }

    /// Takes the current input value, clearing the box.
    pub fn take_value(&mut self) -> String {
        self.input.value_and_reset()
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        self.render_input(frame, area);
    }

    pub fn is_editing(&self) -> bool {
        self.input_mode == InputMode::Editing
    }

    pub fn handle_event(&mut self, action: &Action) -> Option<Action> {
        match self.input_mode {
            InputMode::Normal => match action {
                Action::Editing => {
                    self.start_editing();
                    return None;
                }
                _ => return None,
            },
            InputMode::Editing => match action {
                Action::ConfirmInput => {
                    self.push_message();
                    return None;
                }
                Action::StopEditing => {
                    self.stop_editing();
                    return None;
                }
                Action::ButtonPressed(event) => {
                    self.input.handle_event(&Event::Key(*event));
                    return None;
                }
                _ => {
                    return None;
                }
            },
        }
    }

    fn render_help_message(&self, frame: &mut Frame, area: Rect) {
        let help_message = Line::from_iter(match self.input_mode {
            InputMode::Normal => [
                "Press ".to_span(),
                "q".bold(),
                " to exit, ".to_span(),
                "e".bold(),
                " to start editing.".to_span(),
            ],
            InputMode::Editing => [
                "Press ".to_span(),
                "Esc".bold(),
                " to stop editing, ".to_span(),
                "Enter".bold(),
                " to record the message".to_span(),
            ],
        });
        frame.render_widget(help_message, area);
    }

    fn render_input(&self, frame: &mut Frame, area: Rect) {
        // keep 2 for borders and 1 for cursor
        let width = area.width.max(3) - 3;
        let scroll = self.input.visual_scroll(width as usize);
        let style = match self.input_mode {
            InputMode::Normal => Style::default(),
            InputMode::Editing => Color::Yellow.into(),
        };
        let input = Paragraph::new(self.input.value())
            .style(style)
            .scroll((0, scroll as u16))
            .block(Block::bordered().title(self.title.clone()));
        frame.render_widget(input, area);

        if self.input_mode == InputMode::Editing {
            // Ratatui hides the cursor unless it's explicitly set. Position the  cursor past the
            // end of the input text and one line down from the border to the input line
            let x = self.input.visual_cursor().max(scroll) - scroll + 1;
            frame.set_cursor_position((area.x + x as u16, area.y + 1))
        }
    }

    fn render_messages(&self, frame: &mut Frame, area: Rect) {
        let messages = self
            .messages
            .iter()
            .enumerate()
            .map(|(i, message)| format!("{}: {}", i, message));
        let messages = List::new(messages).block(Block::bordered().title("Messages"));
        frame.render_widget(messages, area);
    }
}

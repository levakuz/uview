use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};

use crate::message::{Action, Command, DepsAction, Event, Input, NavAction};
use crate::ui::Component;
use crate::ui::components::widgets::dependencies_tables_tabs::DependenciesTablesTabs;

pub struct DependenciesPage {
    dependencies_tabs: DependenciesTablesTabs,
}

impl DependenciesPage {
    pub fn new() -> Self {
        Self {
            dependencies_tabs: DependenciesTablesTabs::new(),
        }
    }

    /// Whether an input is open, so that keys are typed rather than acted on.
    pub fn is_editing(&self) -> bool {
        self.dependencies_tabs.is_editing()
    }

    /// The page's keymap: what `input` means here, if anything.
    pub fn action_for(&self, input: &Input) -> Option<Action> {
        let Input::Key(key) = input;
        if self.is_editing() {
            // Every key but Enter and Esc goes into the field.
            return Some(Action::Nav(match key.code {
                KeyCode::Enter => NavAction::Confirm,
                KeyCode::Esc => NavAction::Back,
                _ => NavAction::Type(*key),
            }));
        }
        // Esc and a key read together arrive as Alt+key: outside an input it would act on the
        // list the Esc was closing. Inputs keep Alt, which they use for editing.
        if key.modifiers.contains(KeyModifiers::ALT) {
            return None;
        }
        Some(match key.code {
            KeyCode::Char('q') => Action::Quit,
            KeyCode::Esc => Action::Nav(NavAction::Back),
            KeyCode::Up | KeyCode::Char('k') => Action::Nav(NavAction::Up),
            KeyCode::Down | KeyCode::Char('j') => Action::Nav(NavAction::Down),
            KeyCode::Char(' ') | KeyCode::Enter => Action::Nav(NavAction::Select),
            KeyCode::Tab | KeyCode::Char('[') | KeyCode::Char(']') => {
                Action::Deps(DepsAction::SwitchTable)
            }
            KeyCode::Char('p') => Action::Deps(DepsAction::OpenAdd),
            _ => return None,
        })
    }

    /// Hands what a command produced to the tables.
    pub fn apply(&mut self, event: &Event) {
        self.dependencies_tabs.apply(event);
    }
}

impl Component for DependenciesPage {
    fn render(&mut self, frame: &mut Frame, area: Rect) {
        let [left, _] =
            Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
                .areas(area);
        self.dependencies_tabs.render(frame, left);
    }

    fn update(&mut self, action: &Action) -> Vec<Command> {
        self.dependencies_tabs.update(action)
    }
}

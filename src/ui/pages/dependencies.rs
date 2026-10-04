use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::Frame;
use ratatui::layout::Rect;

use crate::message::{Action, Command, DepsAction, Input, NavAction};
use crate::state::AppState;
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
        // Stops whatever uv is doing, wherever the focus is.
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return Some(Action::Cancel);
        }
        if self.is_editing() {
            // Every other key but Enter and Esc goes into the field.
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
            KeyCode::Char('s') => Action::Deps(DepsAction::Sync),
            KeyCode::Char('r') => Action::Deps(DepsAction::Refresh),
            _ => return None,
        })
    }
}

impl Component for DependenciesPage {
    fn render(&mut self, frame: &mut Frame, area: Rect, state: &AppState) {
        self.dependencies_tabs.render(frame, area, &state.deps);
    }

    fn update(&mut self, action: &Action, state: &AppState) -> Vec<Command> {
        self.dependencies_tabs.update(action, &state.deps)
    }
}

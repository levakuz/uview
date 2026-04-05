use crossterm::event::KeyCode;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use tokio::sync::mpsc;

use crate::events::Action;
use crate::ui::Component;
use crate::ui::components::widgets::dependencies_tables_tabs::DependenciesTablesTabs;

pub struct DependenciesPage {
    dependencies_tabs: DependenciesTablesTabs,
}

impl DependenciesPage {
    pub fn new(tx: mpsc::UnboundedSender<Action>) -> Self {
        let dependencies_tabs = DependenciesTablesTabs::new(tx);
        Self { dependencies_tabs }
    }
}

impl Component for DependenciesPage {
    fn render(&mut self, frame: &mut Frame, area: Rect) {
        let [left, _] =
            Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
                .areas(area);
        self.dependencies_tabs.render(frame, left);
    }

    fn handle_event(&mut self, action: &Action) -> Option<Action> {
        match action {
            Action::ButtonPressed(key) => match key.code {
                KeyCode::Esc => Some(Action::StopEditing),
                KeyCode::Enter => {
                    let confirm_input_event = Action::ConfirmInput;
                    if self.dependencies_tabs.is_editing() {
                        return self.dependencies_tabs.handle_event(&confirm_input_event);
                    }
                    Some(confirm_input_event)
                }
                KeyCode::Char('q') => {
                    if self.dependencies_tabs.is_editing() {
                        return self.dependencies_tabs.handle_event(action);
                    }
                    Some(Action::Quit)
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    if self.dependencies_tabs.is_editing() {
                        return self.dependencies_tabs.handle_event(action);
                    }
                    Some(Action::MoveUp)
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if self.dependencies_tabs.is_editing() {
                        return self.dependencies_tabs.handle_event(action);
                    }
                    Some(Action::MoveDown)
                }
                KeyCode::Tab => {
                    if self.dependencies_tabs.is_editing() {
                        return self.dependencies_tabs.handle_event(action);
                    }
                    Some(Action::SwitchTable)
                }
                KeyCode::Char(' ') => {
                    if self.dependencies_tabs.is_editing() {
                        return self.dependencies_tabs.handle_event(action);
                    }
                    Some(Action::SelectItem)
                }
                KeyCode::Char(']') => {
                    if self.dependencies_tabs.is_editing() {
                        return self.dependencies_tabs.handle_event(action);
                    }
                    Some(Action::SwitchTable)
                }
                KeyCode::Char('[') => {
                    if self.dependencies_tabs.is_editing() {
                        return self.dependencies_tabs.handle_event(action);
                    }
                    Some(Action::SwitchTable)
                }
                KeyCode::Char('p') => {
                    if self.dependencies_tabs.is_editing() {
                        return self.dependencies_tabs.handle_event(action);
                    }
                    Some(Action::OpenPopup)
                }
                KeyCode::Char('e') => {
                    if self.dependencies_tabs.is_editing() {
                        return self.dependencies_tabs.handle_event(action);
                    }
                    Some(Action::Editing)
                }
                _ => {
                    if self.dependencies_tabs.is_editing() {
                        return self.dependencies_tabs.handle_event(action);
                    }
                    self.dependencies_tabs.handle_event(action)
                }
            },
            _ => {
                if self.dependencies_tabs.is_editing() {
                    return self.dependencies_tabs.handle_event(action);
                }
                self.dependencies_tabs.handle_event(action)
            }
        }
    }
}

use ratatui::Frame;
use ratatui::layout::Rect;

use crate::message::{Action, Command};
use crate::state::AppState;
use crate::ui::Component;
use crate::ui::components::widgets::dependencies_tables_tabs::DependenciesTablesTabs;
use crate::ui::keymap::Mode;

pub struct DependenciesPage {
    dependencies_tabs: DependenciesTablesTabs,
}

impl DependenciesPage {
    pub fn new() -> Self {
        Self {
            dependencies_tabs: DependenciesTablesTabs::new(),
        }
    }

    /// Where the focus is, which decides what keys mean.
    pub fn mode(&self) -> Mode {
        self.dependencies_tabs.mode()
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

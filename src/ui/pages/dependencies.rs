use ratatui::Frame;
use ratatui::layout::Rect;

use crate::message::{Action, Command, DepsAction};
use crate::state::AppState;
use crate::ui::Component;
use crate::ui::components::features::version_popup::VersionPopup;
use crate::ui::components::widgets::dependencies_tables_tabs::DependenciesTablesTabs;
use crate::ui::keymap::Mode;

pub struct DependenciesPage {
    dependencies_tabs: DependenciesTablesTabs,
    /// The project's version, on `v`.
    version_popup: VersionPopup,
}

impl DependenciesPage {
    pub fn new() -> Self {
        Self {
            dependencies_tabs: DependenciesTablesTabs::new(),
            version_popup: VersionPopup::default(),
        }
    }

    /// Where the focus is, which decides what keys mean.
    pub fn mode(&self) -> Mode {
        self.version_popup
            .mode()
            .unwrap_or_else(|| self.dependencies_tabs.mode())
    }
}

impl Component for DependenciesPage {
    fn render(&mut self, frame: &mut Frame, area: Rect, state: &AppState) {
        self.dependencies_tabs.render(frame, area, &state.deps);
        self.version_popup.render(frame, area, &state.project);
    }

    fn update(&mut self, action: &Action, state: &AppState) -> Vec<Command> {
        if self.version_popup.mode().is_some() {
            return self.version_popup.update(action).into_iter().collect();
        }
        // Bound in the table alone, so nothing else is open.
        if let Action::Deps(DepsAction::OpenVersion) = action {
            self.version_popup.open(&state.project);
            return vec![Command::PreviewVersions];
        }
        self.dependencies_tabs.update(action, &state.deps)
    }
}

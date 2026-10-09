use ratatui::{Frame, layout::Rect};

use crate::{
    domain::{Bump, VersionChange, version_problem},
    message::{Action, Command, NavAction},
    state::{Loadable, ProjectState},
    ui::{
        components::{features::input_popup::InputPopup, shared::list_popup::ListPopup},
        keymap::Mode,
    },
};

enum Step {
    /// The bumps, each with the version it makes, then "Set version…".
    Choose(ListPopup),
    Type(InputPopup),
}

/// `v`: the project's version, to bump or to set.
#[derive(Default)]
pub struct VersionPopup {
    step: Option<Step>,
}

impl VersionPopup {
    pub fn open(&mut self, project: &ProjectState) {
        let title = match &project.info {
            Loadable::Loaded(info) => format!("Version {}", info.version),
            _ => "Version".into(),
        };
        self.step = Some(Step::Choose(ListPopup::new(&title, items(project))));
    }

    /// Where the focus is while open: the list, or the input.
    pub fn mode(&self) -> Option<Mode> {
        match self.step.as_ref()? {
            Step::Choose(_) => Some(Mode::Actions),
            Step::Type(_) => Some(Mode::Input),
        }
    }

    /// Draws it with the versions as they are now: they arrive after it opens.
    pub fn render(&mut self, frame: &mut Frame, area: Rect, project: &ProjectState) {
        match &mut self.step {
            Some(Step::Choose(list)) => {
                list.set_items(items(project));
                list.render(frame, area);
            }
            Some(Step::Type(input)) => input.render(frame, area),
            None => {}
        }
    }

    /// Moves on with `action`. Returns the change once chosen; Esc gives up.
    pub fn update(&mut self, action: &Action) -> Option<Command> {
        let Action::Nav(nav) = action else {
            return None;
        };
        let (next, command) = match self.step.take()? {
            Step::Choose(_) if *nav == NavAction::Back => (None, None),
            Step::Choose(mut list) => match nav {
                NavAction::Select => match list.selected().and_then(|i| Bump::ALL.get(i)) {
                    Some(bump) => (None, Some(Command::SetVersion(VersionChange::Bump(*bump)))),
                    None => {
                        let mut input = InputPopup::new("Version (e.g. 1.0.0, 2.0.0rc1)", 50);
                        input.open();
                        (Some(Step::Type(input)), None)
                    }
                },
                NavAction::Up => {
                    list.up();
                    (Some(Step::Choose(list)), None)
                }
                NavAction::Down => {
                    list.down();
                    (Some(Step::Choose(list)), None)
                }
                _ => (Some(Step::Choose(list)), None),
            },
            Step::Type(mut input) => match input.update(nav).map(|v| v.trim().to_string()) {
                Some(version) => match version_problem(&version) {
                    Some(problem) => {
                        input.reopen(problem, &version);
                        (Some(Step::Type(input)), None)
                    }
                    None => (None, Some(Command::SetVersion(VersionChange::To(version)))),
                },
                None => (input.is_editing().then_some(Step::Type(input)), None),
            },
        };
        self.step = next;
        command
    }
}

/// Each bump with the version it makes, as far as uv has said, then "Set version…".
fn items(project: &ProjectState) -> Vec<String> {
    let mut items: Vec<String> = Bump::ALL
        .iter()
        .enumerate()
        .map(|(i, bump)| match &project.bumps {
            Loadable::Loaded(versions) => match versions.get(i) {
                Some(version) => format!("{} → {version}", bump.as_str()),
                None => bump.as_str().to_string(),
            },
            // uv could not say, as for a dynamic version: the bump would fail alike.
            Loadable::Failed(_) => bump.as_str().to_string(),
            Loadable::NotLoaded | Loadable::Loading { .. } => format!("{} → …", bump.as_str()),
        })
        .collect();
    items.push("Set version…".into());
    items
}

use ratatui::{Frame, layout::Rect};

use crate::{
    domain::EnvInfo,
    message::{Action, Command, NavAction},
    state::{Loadable, ProjectState},
    ui::{components::shared::list_popup::ListPopup, keymap::Mode},
};

enum Step {
    /// What to do with the environment.
    Menu(ListPopup),
    /// The Pythons to make it with.
    Pythons(ListPopup),
    /// Deleting: Delete or Keep.
    Confirm(ListPopup),
}

/// `e`: the project's environment, to make it anew with another Python, empty it, or delete it.
#[derive(Default)]
pub struct EnvironmentPopup {
    step: Option<Step>,
}

impl EnvironmentPopup {
    pub fn open(&mut self, project: &ProjectState) {
        let (title, rows) = menu(environment(project));
        self.step = Some(Step::Menu(ListPopup::new(&title, rows)));
    }

    pub fn mode(&self) -> Option<Mode> {
        self.step.as_ref().map(|_| Mode::Actions)
    }

    /// Draws it with the environment and the Pythons as they are now.
    pub fn render(&mut self, frame: &mut Frame, area: Rect, project: &ProjectState) {
        match &mut self.step {
            Some(Step::Menu(list)) => {
                let (title, rows) = menu(environment(project));
                list.set_title(&title);
                list.set_items(rows);
                list.render(frame, area);
            }
            Some(Step::Pythons(list)) => {
                list.set_items(python_rows(project));
                list.render(frame, area);
            }
            Some(Step::Confirm(list)) => list.render(frame, area),
            None => {}
        }
    }

    /// Moves on with `action`, and returns what to run once chosen; Esc closes it.
    pub fn update(&mut self, action: &Action, project: &ProjectState) -> Vec<Command> {
        let Action::Nav(nav) = action else {
            return Vec::new();
        };
        let Some(step) = &mut self.step else {
            return Vec::new();
        };
        let list = match step {
            // The rows the keys move over are the Pythons uv has listed so far.
            Step::Pythons(list) => {
                list.set_items(python_rows(project));
                list
            }
            Step::Menu(list) | Step::Confirm(list) => list,
        };
        match nav {
            NavAction::Up => list.up(),
            NavAction::Down => list.down(),
            NavAction::Back => self.step = None,
            NavAction::Select => {
                let chosen = list.selected();
                match (&*step, chosen) {
                    // The Pythons are listed as their list opens.
                    (Step::Menu(_), Some(0)) => {
                        let list = ListPopup::new("Make it with", python_rows(project));
                        self.step = Some(Step::Pythons(list));
                        return vec![Command::ListPythons];
                    }
                    (Step::Menu(_), Some(1)) => {
                        self.step = None;
                        return vec![Command::ResetEnv];
                    }
                    (Step::Menu(_), Some(2)) => {
                        let path =
                            environment(project).map_or(String::new(), |e| e.shown_path.clone());
                        let title = format!("Delete {path}? s makes it again");
                        let options = vec!["Delete".into(), format!("Keep {path}")];
                        self.step = Some(Step::Confirm(ListPopup::new(&title, options)));
                    }
                    (Step::Pythons(_), Some(i)) => {
                        if let Loadable::Loaded(pythons) = &project.pythons
                            && let Some(python) = pythons.get(i)
                        {
                            let python = python.clone();
                            self.step = None;
                            return vec![Command::RecreateEnv { python }];
                        }
                    }
                    (Step::Confirm(_), Some(0)) => {
                        self.step = None;
                        return vec![Command::DeleteEnv];
                    }
                    (Step::Confirm(_), _) => self.step = None,
                    _ => {}
                }
            }
            _ => {}
        }
        Vec::new()
    }
}

/// The environment, when there is one, as the last reload found it.
fn environment(project: &ProjectState) -> Option<&EnvInfo> {
    match &project.environment {
        Loadable::Loaded(Some(info)) => Some(info),
        _ => None,
    }
}

/// The menu's title and rows: making the environment with a Python, then, when there is one,
/// emptying and deleting it.
fn menu(environment: Option<&EnvInfo>) -> (String, Vec<String>) {
    match environment {
        Some(info) => {
            let mut title = format!("Environment {}", info.shown_path);
            if let Some(python) = &info.python {
                title += &format!(" · Python {python}");
            }
            let rows = vec![
                "Make it anew with another Python…".into(),
                "Empty it, then install the project again".into(),
                "Delete it".into(),
            ];
            (title, rows)
        }
        None => (
            "No environment".into(),
            vec!["Make it with a Python…".into()],
        ),
    }
}

/// Each Python uv can use, installed or to download, the environment's own marked; or why uv
/// could not list them.
fn python_rows(project: &ProjectState) -> Vec<String> {
    let current = environment(project).and_then(|e| e.python.clone());
    match &project.pythons {
        Loadable::Loaded(pythons) if pythons.is_empty() => vec!["uv knows no Python".into()],
        Loadable::Loaded(pythons) => pythons
            .iter()
            .map(|p| {
                let mut row = format!("{} {}", p.implementation, p.version);
                if !matches!(p.variant.as_str(), "" | "default") {
                    row += &format!(" {}", p.variant);
                }
                row += if p.path.is_some() {
                    "  installed"
                } else {
                    "  download"
                };
                if current.as_deref() == Some(p.version.as_str()) && p.path.is_some() {
                    row += "  current";
                }
                row
            })
            .collect(),
        Loadable::Failed(reason) => vec![format!(
            "uv could not list them: {}",
            reason.lines().next().unwrap_or_default()
        )],
        _ => vec!["…".into()],
    }
}

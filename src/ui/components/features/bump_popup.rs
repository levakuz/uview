use ratatui::{Frame, layout::Rect};

use crate::{
    domain::{
        Bump, Bumped, PyprojectDependency, bump_constraint, bump_targets, declared_floor,
        normalize_name,
    },
    message::{Action, Command, NavAction},
    state::{DepsState, Loadable},
    ui::components::shared::list_popup::ListPopup,
};

enum Step {
    /// Each bump with the release it reaches.
    Tiers(ListPopup),
    /// The new version falls outside the declared range: write the moved one, or keep it.
    Confirm { list: ListPopup, constraint: String },
}

struct Open {
    name: String,
    /// The constraint pyproject.toml declares, marker and all.
    declared: String,
    /// The version the bumps go up from: the installed one, else the declared lower bound.
    current: Option<String>,
    step: Step,
}

/// `b`: bump a declared package to its next patch, minor or major release.
#[derive(Default)]
pub struct BumpPopup {
    open: Option<Open>,
}

impl BumpPopup {
    /// Opens on `dep`, measured from the version `deps` has installed of it, and returns the
    /// command that lists its versions.
    pub fn open(&mut self, dep: &PyprojectDependency, deps: &DepsState) -> Command {
        let wanted = normalize_name(&dep.name);
        let installed = deps
            .env
            .rows()
            .iter()
            .find(|e| normalize_name(&e.name) == wanted)
            .map(|e| e.version.clone());
        let current = installed.or_else(|| declared_floor(&dep.version_constraint));
        let title = match &current {
            Some(current) => format!("Bump {} from {current}", dep.name),
            None => format!("Bump {}", dep.name),
        };
        let open = Open {
            name: dep.name.clone(),
            declared: dep.version_constraint.clone(),
            current,
            step: Step::Tiers(ListPopup::new(&title, Vec::new())),
        };
        let rows = rows(&open, deps);
        let mut open = open;
        if let Step::Tiers(list) = &mut open.step {
            list.set_items(rows);
        }
        self.open = Some(open);
        Command::ListVersions {
            name: dep.name.clone(),
        }
    }

    pub fn is_open(&self) -> bool {
        self.open.is_some()
    }

    /// Draws it with the versions as they are now: they arrive after it opens.
    pub fn render(&mut self, frame: &mut Frame, area: Rect, deps: &DepsState) {
        let Some(open) = &mut self.open else {
            return;
        };
        let items = rows(open, deps);
        match &mut open.step {
            Step::Tiers(list) => {
                list.set_items(items);
                list.render(frame, area);
            }
            Step::Confirm { list, .. } => list.render(frame, area),
        }
    }

    /// Moves on with `action`, and returns the pin once there is one to write; Esc gives up.
    pub fn update(&mut self, action: &Action, deps: &DepsState) -> Option<Command> {
        let open = self.open.as_mut()?;
        let list = match &mut open.step {
            Step::Tiers(list) | Step::Confirm { list, .. } => list,
        };
        match action {
            Action::Nav(NavAction::Up) => list.up(),
            Action::Nav(NavAction::Down) => list.down(),
            Action::Nav(NavAction::Back) => self.open = None,
            Action::Nav(NavAction::Select) => match &open.step {
                Step::Tiers(list) => {
                    let row = list.selected()?;
                    let target = targets(open, deps)?.get(row)?.clone()?;
                    match bump_constraint(&open.declared, &target)? {
                        Bumped::To(constraint) => {
                            let name = open.name.clone();
                            self.open = None;
                            return Some(Command::Pin { name, constraint });
                        }
                        Bumped::OutOfRange { old, new } => {
                            let options = vec![format!("Write {new}"), format!("Keep {old}")];
                            let title = format!("{target} is outside {old}");
                            open.step = Step::Confirm {
                                list: ListPopup::new(&title, options),
                                constraint: new,
                            };
                        }
                    }
                }
                Step::Confirm { list, constraint } => {
                    let write = list.selected() == Some(0);
                    let pin = write.then(|| Command::Pin {
                        name: open.name.clone(),
                        constraint: constraint.clone(),
                    });
                    self.open = None;
                    return pin;
                }
            },
            _ => {}
        }
        None
    }
}

/// The release each bump reaches, once the versions have loaded and the package has a version
/// to go up from.
fn targets(open: &Open, deps: &DepsState) -> Option<[Option<String>; 3]> {
    let current = open.current.as_deref()?;
    match &deps.versions {
        Some((listed, Loadable::Loaded(versions))) if *listed == open.name => {
            Some(bump_targets(current, versions))
        }
        _ => None,
    }
}

/// Each bump with the release it reaches, as far as the index has said; or why there is none.
fn rows(open: &Open, deps: &DepsState) -> Vec<String> {
    if open.declared.trim_start().starts_with('@') {
        return vec!["Installed from a URL: there is no release to bump to".into()];
    }
    if open.current.is_none() {
        return vec!["Not installed, and no version declared: use Pin to version".into()];
    }
    if let Some((listed, Loadable::Failed(reason))) = &deps.versions
        && *listed == open.name
    {
        return vec![format!("Could not list versions: {reason}")];
    }
    let targets = targets(open, deps);
    Bump::ALL
        .iter()
        .enumerate()
        .map(|(i, bump)| match &targets {
            Some(targets) => match targets.get(i).cloned().flatten() {
                Some(target) => format!("{} → {target}", bump.as_str()),
                None => format!("{}: up to date", bump.as_str()),
            },
            None => format!("{} → …", bump.as_str()),
        })
        .collect()
}

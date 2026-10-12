use ratatui::{Frame, layout::Rect};

use crate::{
    domain::VersionChoice,
    message::{Action, NavAction},
    state::{DepsState, Loadable},
    ui::components::shared::list_popup::ListPopup,
};

/// What was chosen in the version list.
#[derive(Debug, PartialEq)]
pub enum Picked {
    /// "Type a constraint…": the constraint input, as before the list.
    Typed,
    Version(String),
}

/// Pin to version: "Type a constraint…", then the versions of the package, newest first, as its
/// index lists them.
#[derive(Default)]
pub struct VersionPicker {
    /// The package it is open for, and the list.
    open: Option<(String, ListPopup)>,
}

impl VersionPicker {
    pub fn open(&mut self, name: &str, deps: &DepsState) {
        let list = ListPopup::new(&format!("Pin {name}"), rows(name, deps));
        self.open = Some((name.to_string(), list));
    }

    pub fn is_open(&self) -> bool {
        self.open.is_some()
    }

    /// Draws it with the versions as they are now: they arrive after it opens.
    pub fn render(&mut self, frame: &mut Frame, area: Rect, deps: &DepsState) {
        if let Some((name, list)) = &mut self.open {
            list.set_items(rows(name, deps));
            list.render(frame, area);
        }
    }

    /// Moves on with `action`, and says what was chosen once something is; Esc closes it.
    pub fn update(&mut self, action: &Action, deps: &DepsState) -> Option<Picked> {
        let (name, list) = self.open.as_mut()?;
        match action {
            Action::Nav(NavAction::Up) => list.up(),
            Action::Nav(NavAction::Down) => list.down(),
            Action::Nav(NavAction::Back) => self.open = None,
            Action::Nav(NavAction::Select) => {
                let picked = match list.selected()? {
                    0 => Picked::Typed,
                    // A row past the first is a version only once they have loaded.
                    row => Picked::Version(versions(name, deps)?.get(row - 1)?.version.clone()),
                };
                self.open = None;
                return Some(picked);
            }
            _ => {}
        }
        None
    }
}

/// The versions of `name` in `deps`, once they have loaded.
fn versions<'a>(name: &str, deps: &'a DepsState) -> Option<&'a [VersionChoice]> {
    match &deps.versions {
        Some((listed, Loadable::Loaded(versions))) if listed == name => Some(versions),
        _ => None,
    }
}

/// "Type a constraint…", then a row per version as far as the index has said, or why it could
/// not.
fn rows(name: &str, deps: &DepsState) -> Vec<String> {
    let mut rows = vec!["Type a constraint…".to_string()];
    match &deps.versions {
        Some((listed, Loadable::Loaded(versions))) if listed == name => {
            if versions.is_empty() {
                rows.push("No versions to choose from".into());
            }
            rows.extend(versions.iter().map(row));
        }
        Some((listed, Loadable::Failed(reason))) if listed == name => {
            rows.push(format!("Could not list versions: {reason}"));
        }
        // Not yet listed, or listed for another package.
        _ => rows.push("…".into()),
    }
    rows
}

/// How the list shows `version`: marked a pre-release, or with the Python it needs.
fn row(version: &VersionChoice) -> String {
    let mut row = version.version.clone();
    if version.pre {
        row += "  pre";
    }
    if let Some(needs) = &version.needs_python {
        row += &format!("  needs Python {needs}");
    }
    row
}

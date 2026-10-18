use ratatui::{Frame, layout::Rect};

use crate::{
    domain::{VersionChoice, VersionList, constraint_choices},
    message::{Action, NavAction},
    state::{DepsState, Loadable},
    ui::components::shared::list_popup::ListPopup,
};

/// What was chosen in the version list.
#[derive(Debug, PartialEq)]
pub enum Picked {
    /// The row above the versions: "Type a constraint…" when pinning, "Latest" when adding.
    First,
    /// A version, with the operator chosen for it, such as `>=1.17.0`.
    Constraint(String),
}

/// Where the list is: choosing the version, then how to pin it.
enum Step {
    Versions(ListPopup),
    /// The constraints the version can take, as the list shows them.
    Operator {
        list: ListPopup,
        constraints: Vec<String>,
    },
}

/// The row above the versions.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum First {
    /// "Type a constraint…", when pinning.
    TypeConstraint,
    /// "Latest stable version", when adding: no version, for uv to take the newest it can.
    Latest,
}

/// The list as it is open: for which package, under which title, with which first row.
struct Open {
    name: String,
    title: String,
    first: First,
    step: Step,
}

/// A row of its own, then the versions of a package, newest first, as its index lists them;
/// then, for the version chosen, `==`, `>=` and the other operators.
#[derive(Default)]
pub struct VersionPicker {
    open: Option<Open>,
}

impl VersionPicker {
    /// Opens on the versions of `name`, titled `title`, with `first` the row above them.
    pub fn open(&mut self, name: &str, title: &str, first: First, deps: &DepsState) {
        let list = ListPopup::new(title, rows(name, first, deps));
        self.open = Some(Open {
            name: name.to_string(),
            title: title.to_string(),
            first,
            step: Step::Versions(list),
        });
    }

    pub fn is_open(&self) -> bool {
        self.open.is_some()
    }

    /// Draws it with the versions as they are now: they arrive after it opens.
    pub fn render(&mut self, frame: &mut Frame, area: Rect, deps: &DepsState) {
        let Some(open) = &mut self.open else {
            return;
        };
        match &mut open.step {
            Step::Versions(list) => {
                // PyPI's summary of it, once known, follows the title.
                let title = match listed(&open.name, deps).and_then(|l| l.info.as_ref()) {
                    Some(info) => match &info.summary {
                        Some(summary) => format!("{} — {summary}", open.title),
                        None => open.title.clone(),
                    },
                    None => open.title.clone(),
                };
                list.set_title(&title);
                list.set_items(rows(&open.name, open.first, deps));
                list.render(frame, area);
            }
            Step::Operator { list, .. } => list.render(frame, area),
        }
    }

    /// Moves on with `action`, and says what was chosen once something is; Esc closes it.
    pub fn update(&mut self, action: &Action, deps: &DepsState) -> Option<Picked> {
        let Open { name, step, .. } = self.open.as_mut()?;
        let list = match step {
            Step::Versions(list) | Step::Operator { list, .. } => list,
        };
        match action {
            Action::Nav(NavAction::Up) => list.up(),
            Action::Nav(NavAction::Down) => list.down(),
            Action::Nav(NavAction::Back) => self.open = None,
            Action::Nav(NavAction::Select) => match step {
                Step::Versions(list) => match list.selected()? {
                    0 => {
                        self.open = None;
                        return Some(Picked::First);
                    }
                    // A row past the first is a version only once they have loaded.
                    row => {
                        let version = listed(name, deps)?.versions.get(row - 1)?.version.clone();
                        *step = operators(name, &version);
                    }
                },
                Step::Operator { list, constraints } => {
                    let constraint = constraints.get(list.selected()?)?.clone();
                    self.open = None;
                    return Some(Picked::Constraint(constraint));
                }
            },
            _ => {}
        }
        None
    }
}

/// The second step, for `version` of `name`: each constraint it can take, with what it means,
/// `==` highlighted.
fn operators(name: &str, version: &str) -> Step {
    let choices = constraint_choices(version);
    let width = choices
        .iter()
        .map(|(c, _)| c.chars().count())
        .max()
        .unwrap_or(0);
    let rows = choices
        .iter()
        .map(|(constraint, means)| format!("{constraint:width$}  {means}"))
        .collect();
    Step::Operator {
        list: ListPopup::new(&format!("Pin {name} to {version}"), rows),
        constraints: choices.into_iter().map(|(c, _)| c).collect(),
    }
}

/// The versions of `name` in `deps`, once they have loaded.
fn listed<'a>(name: &str, deps: &'a DepsState) -> Option<&'a VersionList> {
    match &deps.versions {
        Some((listed, Loadable::Loaded(versions))) if listed == name => Some(versions),
        _ => None,
    }
}

/// `first`, then a row per version as far as the index has said, or why it could not.
fn rows(name: &str, first: First, deps: &DepsState) -> Vec<String> {
    let first = match (
        first,
        listed(name, deps).and_then(VersionList::latest_stable),
    ) {
        (First::TypeConstraint, _) => "Type a constraint…".to_string(),
        (First::Latest, Some(latest)) => format!("Latest stable version ({latest})"),
        (First::Latest, None) => "Latest stable version".to_string(),
    };
    let mut rows = vec![first];
    match &deps.versions {
        Some((listed, Loadable::Loaded(list))) if listed == name => {
            if list.versions.is_empty() {
                rows.push("No versions to choose from".into());
            }
            rows.extend(list.versions.iter().map(row));
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

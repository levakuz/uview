use ratatui::{Frame, layout::Rect};

use crate::{
    domain::{DeclaredGroup, Membership, PyprojectDependency, group_name_problem},
    message::{Action, Command, DepsAction, NavAction},
    state::DepsState,
    ui::{
        components::{features::input_popup::InputPopup, shared::list_popup::ListPopup},
        keymap::Mode,
    },
};

const TITLE: &str = "Dependency groups";

enum Step {
    /// "New group…", then each group with its package count.
    List(ListPopup),
    /// One group: Rename… and Delete group, then its packages.
    Details {
        group: String,
        list: ListPopup,
    },
    /// Deleting a group with packages: Delete or Keep.
    Confirm {
        group: String,
        list: ListPopup,
    },
    NewName(InputPopup),
    Rename {
        group: String,
        input: InputPopup,
    },
}

/// `g`: the project's dependency groups, to look into, create, rename or delete. What it does
/// is a sequence of commands the runtime already runs, one after another.
#[derive(Default)]
pub struct GroupsPopup {
    step: Option<Step>,
}

impl GroupsPopup {
    pub fn open(&mut self, deps: &DepsState) {
        self.step = Some(Step::List(ListPopup::new(TITLE, list_rows(deps))));
    }

    /// Where the focus is while open: a list, or an input.
    /// Where the focus is while open: the list or a group, where `n`, `r` and `d` act; the
    /// question of a deletion; or an input.
    pub fn mode(&self) -> Option<Mode> {
        match self.step.as_ref()? {
            Step::List(_) | Step::Details { .. } => Some(Mode::Groups),
            Step::Confirm { .. } => Some(Mode::Actions),
            Step::NewName(_) | Step::Rename { .. } => Some(Mode::Input),
        }
    }

    /// Draws it with the groups as they are now: a reload may land while it is open.
    pub fn render(&mut self, frame: &mut Frame, area: Rect, deps: &DepsState) {
        match &mut self.step {
            Some(Step::List(list)) => {
                list.set_items(list_rows(deps));
                list.render(frame, area);
            }
            Some(Step::Details { group, list }) => {
                list.set_items(detail_rows(group, deps));
                list.render(frame, area);
            }
            Some(Step::Confirm { list, .. }) => list.render(frame, area),
            Some(Step::NewName(input) | Step::Rename { input, .. }) => input.render(frame, area),
            None => {}
        }
    }

    /// Moves on with `action`, and returns the commands to run once there are some. Esc goes
    /// back a step from the details, and closes it from the list.
    pub fn update(&mut self, action: &Action, deps: &DepsState) -> Vec<Command> {
        let Some(step) = self.step.take() else {
            return Vec::new();
        };
        let (next, commands) = match (step, action) {
            (Step::List(_), Action::Nav(NavAction::Back)) => (None, Vec::new()),
            (Step::Details { .. } | Step::Confirm { .. }, Action::Nav(NavAction::Back)) => (
                Some(Step::List(ListPopup::new(TITLE, list_rows(deps)))),
                Vec::new(),
            ),
            // n, from the list or a group's details, and "New group…", first in the list.
            (Step::List(_) | Step::Details { .. }, Action::Deps(DepsAction::New)) => (
                Some(Step::NewName(input("New group (e.g. lint)"))),
                Vec::new(),
            ),
            (Step::List(list), Action::Nav(NavAction::Select)) => match highlighted(&list, deps) {
                Some(group) => {
                    let rows = detail_rows(&group.name, deps);
                    let list = ListPopup::new(&details_title(group, deps), rows);
                    (
                        Some(Step::Details {
                            group: group.name.clone(),
                            list,
                        }),
                        Vec::new(),
                    )
                }
                None if list.selected() == Some(0) => (
                    Some(Step::NewName(input("New group (e.g. lint)"))),
                    Vec::new(),
                ),
                None => (Some(Step::List(list)), Vec::new()),
            },
            // d and r on a group in the list, as Delete group and Rename… do.
            (Step::List(list), Action::Deps(DepsAction::Remove)) => {
                match highlighted(&list, deps) {
                    Some(group) => delete(group, deps, Step::List(list)),
                    None => (Some(Step::List(list)), Vec::new()),
                }
            }
            (Step::List(list), Action::Deps(DepsAction::Rename)) => {
                match highlighted(&list, deps) {
                    Some(group) => rename(group, Step::List(list)),
                    None => (Some(Step::List(list)), Vec::new()),
                }
            }
            // And on the group whose details are open.
            (
                Step::Details { group, list },
                Action::Deps(DepsAction::Remove | DepsAction::Rename),
            ) => {
                let declared = deps.groups.iter().find(|g| g.name == group).cloned();
                let back = Step::Details { group, list };
                match declared {
                    Some(declared) if matches!(action, Action::Deps(DepsAction::Remove)) => {
                        delete(&declared, deps, back)
                    }
                    Some(declared) => rename(&declared, back),
                    None => (Some(back), Vec::new()),
                }
            }
            (Step::Details { group, list }, Action::Nav(NavAction::Select)) => {
                let declared = deps.groups.iter().find(|g| g.name == group).cloned();
                match (list.selected(), declared) {
                    // A group named elsewhere offers no actions.
                    (
                        _,
                        Some(DeclaredGroup {
                            named_by: Some(_), ..
                        }),
                    )
                    | (_, None) => (Some(Step::Details { group, list }), Vec::new()),
                    (Some(0), Some(declared)) => rename(&declared, Step::Details { group, list }),
                    (Some(1), Some(declared)) => {
                        delete(&declared, deps, Step::Details { group, list })
                    }
                    _ => (Some(Step::Details { group, list }), Vec::new()),
                }
            }
            (Step::Confirm { group, list }, Action::Nav(NavAction::Select)) => {
                let commands = if list.selected() == Some(0) {
                    deletion(&group, deps)
                } else {
                    Vec::new()
                };
                (None, commands)
            }
            (Step::NewName(mut input), Action::Nav(nav)) => match input.update(nav) {
                Some(name) => match group_name_problem(name.trim(), &deps.groups) {
                    Some(problem) => {
                        input.reopen(problem, name.trim());
                        (Some(Step::NewName(input)), Vec::new())
                    }
                    None => (
                        None,
                        vec![Command::CreateGroup {
                            name: name.trim().to_string(),
                        }],
                    ),
                },
                None => (
                    input.is_editing().then_some(Step::NewName(input)),
                    Vec::new(),
                ),
            },
            (Step::Rename { group, mut input }, Action::Nav(nav)) => match input.update(nav) {
                Some(name) => match group_name_problem(name.trim(), &deps.groups) {
                    Some(problem) => {
                        input.reopen(problem, name.trim());
                        (Some(Step::Rename { group, input }), Vec::new())
                    }
                    None => (None, renaming(&group, name.trim())),
                },
                None => (
                    input.is_editing().then_some(Step::Rename { group, input }),
                    Vec::new(),
                ),
            },
            (step, Action::Nav(NavAction::Up)) => (Some(moved(step, true)), Vec::new()),
            (step, Action::Nav(NavAction::Down)) => (Some(moved(step, false)), Vec::new()),
            (step, _) => (Some(step), Vec::new()),
        };
        self.step = next;
        commands
    }
}

/// The group highlighted in the list, which has "New group…" above the groups.
fn highlighted<'a>(list: &ListPopup, deps: &'a DepsState) -> Option<&'a DeclaredGroup> {
    deps.groups.get(list.selected()?.checked_sub(1)?)
}

/// Asks for `group`'s new name, unless something names it; `back` is the step to stay on then.
fn rename(group: &DeclaredGroup, back: Step) -> (Option<Step>, Vec<Command>) {
    if group.named_by.is_some() {
        return (Some(back), Vec::new());
    }
    let title = format!("Rename {} to", group.name);
    let rename = Step::Rename {
        group: group.name.clone(),
        input: input(&title),
    };
    (Some(rename), Vec::new())
}

/// `step` with its list's highlight moved up or down.
fn moved(mut step: Step, up: bool) -> Step {
    if let Step::List(list) | Step::Details { list, .. } | Step::Confirm { list, .. } = &mut step {
        if up {
            list.up();
        } else {
            list.down();
        }
    }
    step
}

fn input(title: &str) -> InputPopup {
    let mut input = InputPopup::new(title, 50);
    input.open();
    input
}

/// The packages declared in `group`.
fn members<'a>(group: &str, deps: &'a DepsState) -> Vec<&'a PyprojectDependency> {
    let membership = Membership::Group(group.to_string());
    deps.pyproject
        .rows()
        .iter()
        .filter(|d| d.groups.contains(&membership))
        .collect()
}

/// Deleting `group`: at once when empty, after a question when it has packages, not at all when
/// something names it. `back` is the step to stay on when it cannot go.
fn delete(group: &DeclaredGroup, deps: &DepsState, back: Step) -> (Option<Step>, Vec<Command>) {
    if group.named_by.is_some() {
        return (Some(back), Vec::new());
    }
    let count = members(&group.name, deps).len();
    if count == 0 {
        return (
            None,
            vec![Command::DeleteGroup {
                name: group.name.clone(),
            }],
        );
    }
    let packages = if count == 1 { "package" } else { "packages" };
    let title = format!(
        "Delete {} and remove its {count} {packages} from it?",
        group.name
    );
    let options = vec!["Delete".into(), format!("Keep {}", group.name)];
    let confirm = Step::Confirm {
        group: group.name.clone(),
        list: ListPopup::new(&title, options),
    };
    (Some(confirm), Vec::new())
}

/// Removes every package from `group` alone, then deletes the group left empty.
fn deletion(group: &str, deps: &DepsState) -> Vec<Command> {
    members(group, deps)
        .into_iter()
        .map(|d| Command::RemoveFromProject {
            name: d.name.clone(),
            groups: vec![Membership::Group(group.to_string())],
        })
        .chain([Command::DeleteGroup {
            name: group.to_string(),
        }])
        .collect()
}

/// Renames `group` to `new` in one job, which edits pyproject.toml in place: moving the
/// packages one at a time could fail part-way and lose them.
fn renaming(group: &str, new: &str) -> Vec<Command> {
    vec![Command::RenameGroup {
        from: group.to_string(),
        to: new.to_string(),
    }]
}

/// "New group…", then each group with its package count, and what names it when something does.
fn list_rows(deps: &DepsState) -> Vec<String> {
    let width = deps
        .groups
        .iter()
        .map(|g| g.name.chars().count())
        .max()
        .unwrap_or(0);
    let mut rows = vec!["New group…".to_string()];
    rows.extend(deps.groups.iter().map(|group| {
        let mut row = format!("{:width$}  {}", group.name, count(&group.name, deps));
        if let Some(named_by) = &group.named_by {
            row += &format!("  · {named_by}");
        }
        row
    }));
    if deps.groups.is_empty() {
        rows.push("No dependency groups yet".into());
    }
    rows
}

fn count(group: &str, deps: &DepsState) -> String {
    match members(group, deps).len() {
        0 => "empty".into(),
        1 => "1 package".into(),
        n => format!("{n} packages"),
    }
}

fn details_title(group: &DeclaredGroup, deps: &DepsState) -> String {
    format!("{} — {}", group.name, count(&group.name, deps))
}

/// Rename… and Delete group, or why neither, then the group's packages with their constraints.
fn detail_rows(group: &str, deps: &DepsState) -> Vec<String> {
    let named_by = deps
        .groups
        .iter()
        .find(|g| g.name == group)
        .and_then(|g| g.named_by.clone());
    let mut rows = match named_by {
        // Neither renamed nor deleted.
        Some(named_by) => vec![format!("Kept: {named_by}")],
        None => vec!["Rename…".into(), "Delete group".into()],
    };
    rows.extend(
        members(group, deps)
            .iter()
            .map(|d| format!("  {}", d.requirement())),
    );
    rows
}

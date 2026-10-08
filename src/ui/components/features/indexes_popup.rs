use ratatui::{Frame, layout::Rect};

use crate::{
    domain::{Index, index_name_problem, index_url_problem},
    message::{Action, Command, DepsAction, NavAction},
    ui::{
        components::{features::input_popup::InputPopup, shared::list_popup::ListPopup},
        keymap::Mode,
    },
};

/// Where `i` is: the list of the project's indexes, then removing one or adding one.
enum Step {
    /// The indexes as the list shows them, then "Add index…".
    List {
        list: ListPopup,
        indexes: Vec<Index>,
    },
    Remove {
        index: Index,
        list: ListPopup,
    },
    Name {
        input: InputPopup,
        existing: Vec<Index>,
    },
    Url {
        name: String,
        input: InputPopup,
    },
    Kind {
        name: String,
        url: String,
        list: ListPopup,
    },
}

const WIDTH: u16 = 60;

/// The project's indexes, to add one or remove one.
#[derive(Default)]
pub struct IndexesPopup {
    step: Option<Step>,
}

impl IndexesPopup {
    /// Opens on `indexes`, the project's as last loaded.
    pub fn open(&mut self, indexes: &[Index]) {
        let mut rows: Vec<String> = indexes.iter().map(row).collect();
        let title = if rows.is_empty() {
            rows.push("PyPI is the only index".into());
            "Package indexes"
        } else {
            "Package indexes · d removes"
        };
        rows.push("Add index…".into());
        self.step = Some(Step::List {
            list: ListPopup::new(title, rows),
            indexes: indexes.to_vec(),
        });
    }

    /// Where the focus is while open: a list, or an input.
    pub fn mode(&self) -> Option<Mode> {
        match self.step.as_ref()? {
            Step::Name { .. } | Step::Url { .. } => Some(Mode::Input),
            _ => Some(Mode::Actions),
        }
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        match &mut self.step {
            Some(Step::List { list, .. } | Step::Remove { list, .. } | Step::Kind { list, .. }) => {
                list.render(frame, area)
            }
            Some(Step::Name { input, .. } | Step::Url { input, .. }) => input.render(frame, area),
            None => {}
        }
    }

    /// Moves on with `action`. Returns the command once there is one to run; Esc anywhere
    /// gives up.
    pub fn update(&mut self, action: &Action) -> Option<Command> {
        // d on an index in the list removes it at once.
        if let (Action::Deps(DepsAction::Remove), Some(Step::List { list, indexes })) =
            (action, &self.step)
        {
            let index = list.selected().and_then(|i| indexes.get(i)).cloned()?;
            self.step = None;
            return Some(Command::RemoveIndex(index));
        }
        let Action::Nav(nav) = action else {
            return None;
        };
        let (next, command) = match self.step.take()? {
            Step::List { .. } | Step::Remove { .. } | Step::Kind { .. }
                if *nav == NavAction::Back =>
            {
                (None, None)
            }
            Step::List { mut list, indexes } => match nav {
                NavAction::Select => match list.selected() {
                    Some(i) if i < indexes.len() => {
                        let index = indexes[i].clone();
                        let label = index.name.clone().unwrap_or_else(|| index.url.clone());
                        let options = vec![format!("Remove index {label}"), "Keep it".into()];
                        let list = ListPopup::new(&label, options);
                        (Some(Step::Remove { index, list }), None)
                    }
                    // The last row adds; "PyPI is the only index" does nothing.
                    Some(i) if i == indexes.len().max(1) => {
                        let input = popup("Index name (e.g. pytorch)");
                        let existing = indexes;
                        (Some(Step::Name { input, existing }), None)
                    }
                    _ => (Some(Step::List { list, indexes }), None),
                },
                _ => {
                    moved(&mut list, nav);
                    (Some(Step::List { list, indexes }), None)
                }
            },
            Step::Remove { index, mut list } => match nav {
                NavAction::Select if list.selected() == Some(0) => {
                    (None, Some(Command::RemoveIndex(index)))
                }
                NavAction::Select => (None, None),
                _ => {
                    moved(&mut list, nav);
                    (Some(Step::Remove { index, list }), None)
                }
            },
            Step::Name {
                mut input,
                existing,
            } => match input.update(nav).map(|name| name.trim().to_string()) {
                Some(name) => match index_name_problem(&name, &existing) {
                    Some(problem) => {
                        input.reopen(problem, &name);
                        (Some(Step::Name { input, existing }), None)
                    }
                    None => {
                        let input = popup("URL (e.g. https://download.pytorch.org/whl/cpu)");
                        (Some(Step::Url { name, input }), None)
                    }
                },
                None => (
                    input.is_editing().then_some(Step::Name { input, existing }),
                    None,
                ),
            },
            Step::Url { name, mut input } => {
                match input.update(nav).map(|u| u.trim().to_string()) {
                    Some(url) => match index_url_problem(&url) {
                        Some(problem) => {
                            input.reopen(problem, &url);
                            (Some(Step::Url { name, input }), None)
                        }
                        None => {
                            let kinds = vec![
                                "Extra: searched alongside PyPI".into(),
                                "Default: replaces PyPI; needs every package".into(),
                            ];
                            let list = ListPopup::new("Kind of index", kinds);
                            (Some(Step::Kind { name, url, list }), None)
                        }
                    },
                    None => (
                        input.is_editing().then_some(Step::Url { name, input }),
                        None,
                    ),
                }
            }
            Step::Kind {
                name,
                url,
                mut list,
            } => match nav {
                NavAction::Select => {
                    let default = list.selected() == Some(1);
                    (None, Some(Command::AddIndex { name, url, default }))
                }
                _ => {
                    moved(&mut list, nav);
                    (Some(Step::Kind { name, url, list }), None)
                }
            },
        };
        self.step = next;
        command
    }
}

/// How the list shows `index`: its kind, its name, then its URL, which a narrow pane cuts.
fn row(index: &Index) -> String {
    let name = index.name.as_deref().unwrap_or("(no name)");
    // `explicit`, the longest kind, sets the column.
    format!("{:<8}  {name}  {}", index.kind.to_string(), index.url)
}

/// An open input titled `title`.
fn popup(title: &str) -> InputPopup {
    let mut input = InputPopup::new(title, WIDTH);
    input.open();
    input
}

fn moved(list: &mut ListPopup, nav: &NavAction) {
    match nav {
        NavAction::Up => list.up(),
        NavAction::Down => list.down(),
        _ => {}
    }
}

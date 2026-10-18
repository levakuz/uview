use ratatui::{Frame, layout::Rect};

use crate::{
    domain::bare_name,
    message::{Action, NavAction},
    state::{DepsState, Loadable},
    ui::{
        components::{features::input_popup::InputPopup, shared::list_popup::ListPopup},
        keymap::Mode,
    },
};

/// What the search asks for.
#[derive(Debug, PartialEq)]
pub enum Searched {
    /// Look up `name`'s versions and description.
    Find(String),
    /// Go on to add the package found: `requirement` is the name as typed, extras and all.
    Add { name: String, requirement: String },
}

enum Step {
    Input(InputPopup),
    /// The package looked up, shown as its lookup goes.
    Card {
        name: String,
        requirement: String,
        list: ListPopup,
    },
}

/// `/`: look a package up by its exact name, then add it.
#[derive(Default)]
pub struct SearchPopup {
    step: Option<Step>,
}

impl SearchPopup {
    pub fn open(&mut self) {
        let mut input = InputPopup::new("Look up a package (e.g. httpx)", 50);
        input.open();
        self.step = Some(Step::Input(input));
    }

    /// Where the focus is while open: the input, or the card.
    pub fn mode(&self) -> Option<Mode> {
        match self.step.as_ref()? {
            Step::Input(_) => Some(Mode::Input),
            Step::Card { .. } => Some(Mode::Actions),
        }
    }

    /// Draws it with what the lookup has found so far.
    pub fn render(&mut self, frame: &mut Frame, area: Rect, deps: &DepsState) {
        match &mut self.step {
            Some(Step::Input(input)) => input.render(frame, area),
            Some(Step::Card { name, list, .. }) => {
                let (title, rows) = card(name, deps);
                list.set_title(&title);
                list.set_items(rows);
                list.render(frame, area);
            }
            None => {}
        }
    }

    /// Moves on with `action`, and says what to do once there is something; Esc closes it.
    pub fn update(&mut self, action: &Action, deps: &DepsState) -> Option<Searched> {
        let Action::Nav(nav) = action else {
            return None;
        };
        match self.step.as_mut()? {
            Step::Input(input) => {
                let Some(typed) = input.update(nav) else {
                    // Esc closed the input, and the search with it.
                    if !input.is_editing() {
                        self.step = None;
                    }
                    return None;
                };
                let Some((name, requirement)) = bare_name(&typed) else {
                    input.reopen("A name alone, e.g. httpx or flask[dotenv]", typed.trim());
                    return None;
                };
                let (title, rows) = card(&name, deps);
                self.step = Some(Step::Card {
                    name: name.clone(),
                    requirement,
                    list: ListPopup::new(&title, rows),
                });
                Some(Searched::Find(name))
            }
            Step::Card {
                name,
                requirement,
                list,
            } => match nav {
                NavAction::Up => {
                    list.up();
                    None
                }
                NavAction::Down => {
                    list.down();
                    None
                }
                NavAction::Back => {
                    self.step = None;
                    None
                }
                // The first row adds, once the package is found.
                NavAction::Select if list.selected() == Some(0) && found(name, deps) => {
                    let add = Searched::Add {
                        name: name.clone(),
                        requirement: requirement.clone(),
                    };
                    self.step = None;
                    Some(add)
                }
                _ => None,
            },
        }
    }
}

fn found(name: &str, deps: &DepsState) -> bool {
    matches!(&deps.versions, Some((listed, Loadable::Loaded(_))) if listed == name)
}

/// The card's title and rows: "Add …" first, then what is known of the package.
fn card(name: &str, deps: &DepsState) -> (String, Vec<String>) {
    match &deps.versions {
        Some((listed, Loadable::Loaded(list))) if listed == name => {
            let title = match list.latest_stable() {
                Some(latest) => format!("{name} {latest}"),
                None => name.to_string(),
            };
            let mut rows = vec![format!("Add {name}…")];
            if let Some(info) = &list.info {
                rows.extend(info.summary.clone());
                rows.extend(
                    info.requires_python
                        .as_ref()
                        .map(|python| format!("Requires Python {python}")),
                );
                rows.extend(info.homepage.clone());
            }
            rows.push(format!("From {}", list.index));
            (title, rows)
        }
        Some((listed, Loadable::Failed(reason))) if listed == name => {
            (name.to_string(), vec![reason.clone()])
        }
        _ => (format!("Looking up {name}…"), vec!["…".into()]),
    }
}

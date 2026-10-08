use ratatui::{Frame, layout::Rect};

use crate::{
    domain::Source,
    message::{Action, NavAction},
    ui::{
        components::{features::input_popup::InputPopup, shared::list_popup::ListPopup},
        keymap::Mode,
    },
};

/// Where `P` is in asking for a package from somewhere other than the default index: which
/// source, then its fields, one popup at a time.
enum Step {
    Choose(ListPopup),
    IndexPackage(InputPopup),
    IndexUrl { package: String, input: InputPopup },
    GitUrl(InputPopup),
    Path(InputPopup),
    Editable { path: String, list: ListPopup },
}

const WIDTH: u16 = 60;

/// The questions `P` asks. It hands back the source once every field is in.
#[derive(Default)]
pub struct AddFromSource {
    step: Option<Step>,
}

impl AddFromSource {
    pub fn open(&mut self) {
        let sources = ["Index URL", "Git", "Local path"]
            .map(String::from)
            .to_vec();
        self.step = Some(Step::Choose(ListPopup::new("Add from", sources)));
    }

    /// Where the focus is while open: a list, or an input.
    pub fn mode(&self) -> Option<Mode> {
        match self.step.as_ref()? {
            Step::Choose(_) | Step::Editable { .. } => Some(Mode::Actions),
            _ => Some(Mode::Input),
        }
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        match &mut self.step {
            Some(Step::Choose(list) | Step::Editable { list, .. }) => list.render(frame, area),
            Some(
                Step::IndexPackage(input)
                | Step::IndexUrl { input, .. }
                | Step::GitUrl(input)
                | Step::Path(input),
            ) => input.render(frame, area),
            None => {}
        }
    }

    /// Moves on with `action`. Returns the source once its last field is in; Esc anywhere
    /// gives up.
    pub fn update(&mut self, action: &Action) -> Option<Source> {
        let Action::Nav(nav) = action else {
            return None;
        };
        let step = self.step.take()?;
        let (next, source) = match step {
            Step::Choose(list) | Step::Editable { list, .. } if *nav == NavAction::Back => {
                drop(list);
                (None, None)
            }
            Step::Choose(list) => match nav {
                NavAction::Select => (Some(after_choosing(list.selected())), None),
                _ => (Some(Step::Choose(moved(list, nav))), None),
            },
            Step::Editable { path, mut list } => match nav {
                NavAction::Select => {
                    let editable = list.selected() == Some(0);
                    (None, Some(Source::Path { path, editable }))
                }
                _ => {
                    list = moved(list, nav);
                    (Some(Step::Editable { path, list }), None)
                }
            },
            Step::IndexPackage(mut input) => match input.update(nav) {
                Some(package) => {
                    let input = popup("Index URL (e.g. https://download.pytorch.org/whl/cpu)");
                    (Some(Step::IndexUrl { package, input }), None)
                }
                None => (
                    input.is_editing().then_some(Step::IndexPackage(input)),
                    None,
                ),
            },
            Step::IndexUrl { package, mut input } => match input.update(nav) {
                Some(url) => checked(
                    Source::Index {
                        package: package.clone(),
                        url: url.trim().to_string(),
                    },
                    |source, problem| {
                        let Source::Index { url, .. } = source else {
                            return None;
                        };
                        let mut input = popup("");
                        input.reopen(problem, url);
                        Some(Step::IndexUrl {
                            package: package.clone(),
                            input,
                        })
                    },
                ),
                None => (
                    input
                        .is_editing()
                        .then_some(Step::IndexUrl { package, input }),
                    None,
                ),
            },
            Step::GitUrl(mut input) => match input.update(nav) {
                Some(url) => checked(
                    Source::Git {
                        url: url.trim().to_string(),
                    },
                    |source, problem| {
                        let Source::Git { url } = source else {
                            return None;
                        };
                        let mut input = popup("");
                        input.reopen(problem, url);
                        Some(Step::GitUrl(input))
                    },
                ),
                None => (input.is_editing().then_some(Step::GitUrl(input)), None),
            },
            Step::Path(mut input) => match input.update(nav) {
                Some(path) => {
                    let options = vec!["Editable".into(), "Not editable".into()];
                    let list = ListPopup::new("Install it", options);
                    let path = path.trim().to_string();
                    (Some(Step::Editable { path, list }), None)
                }
                None => (input.is_editing().then_some(Step::Path(input)), None),
            },
        };
        self.step = next;
        source
    }
}

/// The step after the source list: the first field of the chosen source.
fn after_choosing(chosen: Option<usize>) -> Step {
    match chosen {
        Some(0) => Step::IndexPackage(popup("Package (e.g. torch)")),
        Some(1) => Step::GitUrl(popup(
            "Git URL, @ref optional (e.g. https://github.com/o/r@v1)",
        )),
        _ => Step::Path(popup("Path (e.g. ../mylib)")),
    }
}

/// An open input titled `title`.
fn popup(title: &str) -> InputPopup {
    let mut input = InputPopup::new(title, WIDTH);
    input.open();
    input
}

/// `list` with its highlight moved by `nav`.
fn moved(mut list: ListPopup, nav: &NavAction) -> ListPopup {
    match nav {
        NavAction::Up => list.up(),
        NavAction::Down => list.down(),
        _ => {}
    }
    list
}

/// The source, if nothing is wrong with it; else the step `again` makes to ask once more, the
/// problem as its title.
fn checked(
    source: Source,
    again: impl FnOnce(&Source, &str) -> Option<Step>,
) -> (Option<Step>, Option<Source>) {
    match source.problem() {
        None => (None, Some(source)),
        Some(problem) => (again(&source, problem), None),
    }
}

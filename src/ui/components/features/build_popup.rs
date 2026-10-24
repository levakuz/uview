use ratatui::Frame;
use ratatui::layout::Rect;

use crate::domain::{Artifact, PublishTarget};
use crate::message::{Action, Command, NavAction};
use crate::state::AppState;
use crate::ui::components::features::project_tile::human_size;
use crate::ui::components::shared::list_popup::ListPopup;
use crate::ui::keymap::Mode;

/// What a build wrote, shown once it has succeeded, and publishing it: where to, then whether.
#[derive(Default)]
pub struct BuildPopup {
    /// How many builds had succeeded when it last opened.
    seen: u64,
    step: Option<Step>,
}

enum Step {
    /// The files, after **Publish…** when there are any.
    Built(ListPopup),
    /// The indexes to publish to.
    Targets(ListPopup, Vec<PublishTarget>),
    /// The files, the target and the credentials, then **Publish** and **Cancel**. The files
    /// are those shown, whatever a build that lands meanwhile writes.
    Confirm(ListPopup, PublishTarget, Vec<Artifact>),
    /// Says why there is nothing to publish.
    Nothing(ListPopup),
}

impl BuildPopup {
    pub fn mode(&self) -> Option<Mode> {
        self.step.as_ref().map(|_| Mode::Actions)
    }

    /// Opens on the indexes to publish the last build to, or says that nothing is built yet.
    pub fn open_publish(&mut self, state: &AppState) {
        self.step = Some(if state.project.built.is_empty() {
            let row = "Nothing built yet: B builds the project first".to_string();
            Step::Nothing(ListPopup::new("Publish", vec![row]))
        } else {
            targets(state)
        });
    }

    /// Opens, when a build has succeeded since it last did and `idle` says nothing else is
    /// open, then draws it.
    pub fn render(&mut self, frame: &mut Frame, area: Rect, state: &AppState, idle: bool) {
        let builds = state.project.builds;
        if self.step.is_none() && idle && builds != self.seen {
            self.seen = builds;
            let built = &state.project.built;
            let publish = (!built.is_empty()).then(|| "Publish…".to_string());
            let rows = publish.into_iter().chain(file_rows(built)).collect();
            self.step = Some(Step::Built(ListPopup::new("Built", rows)));
        }
        match &mut self.step {
            Some(
                Step::Built(list)
                | Step::Targets(list, _)
                | Step::Confirm(list, ..)
                | Step::Nothing(list),
            ) => list.render(frame, area),
            None => {}
        }
    }

    /// Moves on with `action`, and returns the upload once confirmed; Esc closes it.
    pub fn update(&mut self, action: &Action, state: &AppState) -> Vec<Command> {
        let (Some(step), Action::Nav(nav)) = (&mut self.step, action) else {
            return Vec::new();
        };
        let list = match step {
            Step::Built(list)
            | Step::Targets(list, _)
            | Step::Confirm(list, ..)
            | Step::Nothing(list) => list,
        };
        match nav {
            NavAction::Up => list.up(),
            NavAction::Down => list.down(),
            NavAction::Back => self.step = None,
            NavAction::Select => {
                let chosen = list.selected();
                match step {
                    Step::Built(_) if chosen == Some(0) && !state.project.built.is_empty() => {
                        self.step = Some(targets(state));
                    }
                    Step::Targets(_, targets) => {
                        if let Some(target) = chosen.and_then(|i| targets.get(i)).cloned() {
                            self.step = Some(confirm(state, target));
                        }
                    }
                    // The files and credentials above the two choices are only to read.
                    Step::Confirm(_, target, files) => {
                        match chosen.map(|i| i.checked_sub(files.len() + 1)) {
                            Some(Some(0)) => {
                                let command = Command::Publish {
                                    target: target.clone(),
                                    files: files.clone(),
                                };
                                self.step = None;
                                return vec![command];
                            }
                            Some(Some(1)) => self.step = None,
                            _ => {}
                        }
                    }
                    Step::Built(_) | Step::Nothing(_) => self.step = None,
                }
            }
            _ => {}
        }
        Vec::new()
    }
}

/// The list of indexes to publish to: each one's name, then where it uploads.
fn targets(state: &AppState) -> Step {
    let targets = PublishTarget::all(&state.deps.indexes);
    let width = targets
        .iter()
        .map(|t| t.label().chars().count())
        .max()
        .unwrap_or(0);
    let rows = targets
        .iter()
        .map(|t| format!("{:<width$}  {}", t.label(), t.upload_url()))
        .collect();
    Step::Targets(ListPopup::new("Publish to", rows), targets)
}

/// What publishing to `target` uploads, and with what, then **Publish**, highlighted.
fn confirm(state: &AppState, target: PublishTarget) -> Step {
    let files = state.project.built.clone();
    let mut rows = file_rows(&files);
    rows.push(match state.project.credentials {
        Some(source) => format!("Credentials from {source}"),
        None => "No credentials found: set UV_PUBLISH_TOKEN".into(),
    });
    rows.extend(["Publish".into(), "Cancel".into()]);
    let title = format!("Publish to {} · {}", target.label(), target.upload_url());
    let mut list = ListPopup::new(&title, rows);
    list.select(files.len() + 1);
    Step::Confirm(list, target, files)
}

/// Each file's name, padded so the sizes line up, then its size.
fn file_rows(built: &[Artifact]) -> Vec<String> {
    if built.is_empty() {
        return vec!["uv wrote no new file to dist/".into()];
    }
    let names: Vec<String> = built.iter().map(Artifact::name).collect();
    let width = names.iter().map(|n| n.chars().count()).max().unwrap_or(0);
    names
        .iter()
        .zip(built)
        .map(|(name, a)| format!("{name:<width$}  {}", human_size(a.size)))
        .collect()
}

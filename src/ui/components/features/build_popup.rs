use ratatui::Frame;
use ratatui::layout::Rect;

use crate::domain::Artifact;
use crate::message::{Action, Command, NavAction};
use crate::state::ProjectState;
use crate::ui::components::features::project_tile::human_size;
use crate::ui::components::shared::list_popup::ListPopup;
use crate::ui::keymap::Mode;

/// What a build wrote, shown once it has succeeded.
#[derive(Default)]
pub struct BuildPopup {
    /// How many builds had succeeded when it last opened.
    seen: u64,
    list: Option<ListPopup>,
}

impl BuildPopup {
    pub fn mode(&self) -> Option<Mode> {
        self.list.as_ref().map(|_| Mode::Actions)
    }

    /// Opens, when a build has succeeded since it last did and `idle` says nothing else is
    /// open, then draws it.
    pub fn render(&mut self, frame: &mut Frame, area: Rect, project: &ProjectState, idle: bool) {
        if self.list.is_none() && idle && project.builds != self.seen {
            self.seen = project.builds;
            self.list = Some(ListPopup::new("Built", rows(&project.built)));
        }
        if let Some(list) = &mut self.list {
            list.render(frame, area);
        }
    }

    /// Moves over the files; Enter or Esc closes it.
    pub fn update(&mut self, action: &Action) -> Vec<Command> {
        if let (Some(list), Action::Nav(nav)) = (&mut self.list, action) {
            match nav {
                NavAction::Up => list.up(),
                NavAction::Down => list.down(),
                NavAction::Select | NavAction::Back => self.list = None,
                _ => {}
            }
        }
        Vec::new()
    }
}

/// Each file's name, padded so the sizes line up, then its size.
fn rows(built: &[Artifact]) -> Vec<String> {
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

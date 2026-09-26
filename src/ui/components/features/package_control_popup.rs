use ratatui::{
    Frame,
    layout::Rect,
    widgets::{Block, List, ListState},
};

use crate::{
    events::Action,
    ui::components::shared::popup::{Popup, PopupSize},
};

enum PackageControlOptions {
    Update,
    PinToVersion,
    Remove,
    Cancel,
}

impl PackageControlOptions {
    fn label(&self) -> &'static str {
        match self {
            PackageControlOptions::Update => "Update",
            PackageControlOptions::PinToVersion => "Pin to version",
            PackageControlOptions::Remove => "Remove",
            PackageControlOptions::Cancel => "Cancel",
        }
    }
}

pub struct PackageControlPopup<'a> {
    popup: Popup,
    list: List<'a>,
    list_state: ListState,
}

impl PackageControlPopup<'_> {
    pub fn new() -> Self {
        let block = Block::bordered().title("Package Control");
        let list_state = ListState::default().with_selected(Some(0));
        let list = List::new([
            PackageControlOptions::PinToVersion.label(),
            PackageControlOptions::Update.label(),
            PackageControlOptions::Remove.label(),
            PackageControlOptions::Cancel.label(),
        ])
        .block(block)
        .highlight_style(ratatui::style::Style::default().reversed());

        Self {
            popup: Popup::new(PopupSize {
                width: 40,
                height: 10,
            }),
            list,
            list_state,
        }
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        let popup_area = self.popup.render(frame, area);
        frame.render_stateful_widget(self.list.clone(), popup_area, &mut self.list_state);
    }

    pub fn handle_event(&mut self, action: &Action) -> Option<Action> {
        match action {
            Action::MoveUp => {
                if let Some(selected) = self.list_state.selected() {
                    if selected > 0 {
                        self.list_state.select(Some(selected - 1));
                    }
                }
            }
            Action::MoveDown => {
                if let Some(selected) = self.list_state.selected() {
                    self.list_state.select(Some((selected + 1).min(3)));
                }
            }
            Action::SelectItem => match self.list_state.selected() {
                Some(0) => return Some(Action::OpenPinPopup),
                Some(1) => return Some(Action::UpgradeEnvDep),
                Some(2) => return Some(Action::RemovePackage),
                Some(3) => return Some(Action::ClosePopup),
                _ => {}
            },
            _ => {
                return self.popup.handle_event(action);
            }
        }
        self.popup.handle_event(action)
    }
}

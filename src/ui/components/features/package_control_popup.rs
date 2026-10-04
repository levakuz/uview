use ratatui::{
    Frame,
    layout::Rect,
    widgets::{Block, List, ListState},
};

use crate::ui::components::shared::popup::{Popup, PopupSize};

/// What the actions list offers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PackageControlOption {
    Update,
    PinToVersion,
    Remove,
    Cancel,
}

/// The options in the order the list shows them.
const OPTIONS: [PackageControlOption; 4] = [
    PackageControlOption::PinToVersion,
    PackageControlOption::Update,
    PackageControlOption::Remove,
    PackageControlOption::Cancel,
];

impl PackageControlOption {
    fn label(&self) -> &'static str {
        match self {
            PackageControlOption::Update => "Update",
            PackageControlOption::PinToVersion => "Pin to version",
            PackageControlOption::Remove => "Remove",
            PackageControlOption::Cancel => "Cancel",
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
        let list = List::new(OPTIONS.map(|option| option.label()))
            .block(block)
            .highlight_style(ratatui::style::Style::default().reversed());

        Self {
            // The four options inside a border.
            popup: Popup::new(PopupSize {
                width: 30,
                height: OPTIONS.len() as u16 + 2,
            }),
            list,
            list_state,
        }
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        let popup_area = self.popup.render(frame, area);
        frame.render_stateful_widget(self.list.clone(), popup_area, &mut self.list_state);
    }

    pub fn up(&mut self) {
        if let Some(selected) = self.list_state.selected()
            && selected > 0
        {
            self.list_state.select(Some(selected - 1));
        }
    }

    pub fn down(&mut self) {
        if let Some(selected) = self.list_state.selected() {
            self.list_state
                .select(Some((selected + 1).min(OPTIONS.len() - 1)));
        }
    }

    /// The highlighted option.
    pub fn selected(&self) -> Option<PackageControlOption> {
        OPTIONS.get(self.list_state.selected()?).copied()
    }
}

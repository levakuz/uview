use ratatui::{Frame, layout::Rect};

use crate::ui::components::shared::list_popup::ListPopup;

/// What the actions list offers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PackageControlOption {
    Update,
    PinToVersion,
    /// For a declared package: its next patch, minor or major release.
    Bump,
    /// For a package in one group.
    MoveToGroup,
    AddToGroup,
    /// For a package in several groups, which is not moved: leaving one group should not take
    /// it out of the others.
    RemoveFromGroup,
    Remove,
    Cancel,
}

impl PackageControlOption {
    fn label(&self) -> &'static str {
        match self {
            PackageControlOption::Update => "Update",
            PackageControlOption::PinToVersion => "Pin to version",
            PackageControlOption::Bump => "Bump",
            PackageControlOption::MoveToGroup => "Move to group",
            PackageControlOption::AddToGroup => "Add to group",
            PackageControlOption::RemoveFromGroup => "Remove from group",
            PackageControlOption::Remove => "Remove",
            PackageControlOption::Cancel => "Cancel",
        }
    }
}

pub struct PackageControlPopup {
    list: ListPopup,
    options: Vec<PackageControlOption>,
}

impl PackageControlPopup {
    /// The actions for a package declared in `groups` memberships; none for one that is only
    /// installed. The group actions sit between Pin and Update and Remove.
    pub fn new(groups: usize) -> Self {
        use PackageControlOption::*;
        let group_actions: &[PackageControlOption] = match groups {
            0 => &[],
            1 => &[MoveToGroup, AddToGroup],
            _ => &[AddToGroup, RemoveFromGroup],
        };
        // Bump is for a declared package, as the group actions are.
        let version_actions: &[PackageControlOption] = match groups {
            0 => &[PinToVersion],
            _ => &[PinToVersion, Bump],
        };
        let options: Vec<_> = version_actions
            .iter()
            .chain(&[Update])
            .chain(group_actions)
            .chain(&[Remove, Cancel])
            .copied()
            .collect();
        let labels = options.iter().map(|o| o.label().to_string()).collect();
        Self {
            list: ListPopup::new("Package Control", labels),
            options,
        }
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        self.list.render(frame, area);
    }

    pub fn up(&mut self) {
        self.list.up();
    }

    pub fn down(&mut self) {
        self.list.down();
    }

    /// The highlighted option.
    pub fn selected(&self) -> Option<PackageControlOption> {
        self.options.get(self.list.selected()?).copied()
    }
}

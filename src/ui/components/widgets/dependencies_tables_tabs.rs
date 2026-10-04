use crate::{
    domain::{EnvDependency, PyprojectDependency},
    message::{Action, Command, DepsAction, NavAction},
    state::{DepsState, Loadable},
    ui::components::{
        features::{
            input_popup::InputPopup,
            package_control_popup::{PackageControlOption, PackageControlPopup},
        },
        shared::table::SharedTable,
    },
};
use ratatui::{Frame, symbols, widgets::Tabs};
use ratatui::{
    layout::{Margin, Rect},
    style::{Style, Stylize},
    text::Text,
};
#[derive(PartialEq)]
enum ActiveTable {
    Pyproject,
    Environment,
}

/// The two dependency tables behind tabs, and the popups acting on them. It reads the rows from
/// `DepsState` and keeps only which tab, row and popup are showing.
pub struct DependenciesTablesTabs {
    pyproject_table: SharedTable<PyprojectDependency>,
    env_table: SharedTable<EnvDependency>,
    active_table: ActiveTable,
    package_control_popup: PackageControlPopup<'static>,
    show_package_control_popup: bool,
    /// The package the actions list was opened for. Its actions use this, not the selection
    /// when they run, which a reload can move to another row.
    target: Option<String>,
    /// The popups with an input are shown exactly while they are being typed into.
    package_add_popup: InputPopup,
    pin_version_popup: InputPopup,
}

impl DependenciesTablesTabs {
    pub fn new() -> Self {
        let package_control_popup = PackageControlPopup::new();
        let package_add_popup = InputPopup::new("Package (e.g. requests, httpx>=0.27)", 50);
        let pin_version_popup = InputPopup::new("Version (e.g. ==1.2.3, >=2.0, <3.0)", 50);
        // Only the active table is drawn, so both are styled as focused. The unfocused style
        // is for showing them side by side, which #16 may choose. The tabs on the top border
        // name the tables, so they have no title of their own.
        let mut pyproject_table = SharedTable::new("");
        pyproject_table.set_focused(true);
        let mut env_table = SharedTable::new("");
        env_table.set_focused(true);

        Self {
            pyproject_table,
            env_table,
            active_table: ActiveTable::Pyproject,
            package_control_popup,
            show_package_control_popup: false,
            target: None,
            package_add_popup,
            pin_version_popup,
        }
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect, deps: &DepsState) {
        self.render_content(frame, area, deps);
        self.render_tabs(frame, area);
        if self.show_package_control_popup {
            self.package_control_popup.render(frame, area);
        }
        if self.package_add_popup.is_editing() {
            self.package_add_popup.render(frame, area);
        }
        if self.pin_version_popup.is_editing() {
            self.pin_version_popup.render(frame, area);
        }
    }

    pub fn is_editing(&self) -> bool {
        self.package_add_popup.is_editing() || self.pin_version_popup.is_editing()
    }

    fn render_tabs(&mut self, frame: &mut Frame, area: Rect) {
        let selected_tab = match self.active_table {
            ActiveTable::Pyproject => 0,
            ActiveTable::Environment => 1,
        };
        // `Tabs` paints its base style over the whole area it is given, border included, so the
        // white goes on the titles and the divider themselves.
        let tabs = Tabs::new(vec!["pyproject.toml".white(), "Environment".white()])
            .highlight_style(Style::default().green().bold())
            .select(selected_tab)
            .divider(symbols::DOT.white())
            .padding(" ", " ");
        // On the table's top border, inside its corners, until #16 gives the tabs their own row.
        frame.render_widget(tabs, area.inner(Margin::new(1, 0)));
    }
    fn render_content(&mut self, frame: &mut Frame, area: Rect, deps: &DepsState) {
        match self.active_table {
            ActiveTable::Pyproject => {
                let placeholder = placeholder(
                    &deps.pyproject,
                    "No dependencies yet. Press p to add one.",
                    "Could not read pyproject.toml",
                );
                let rows = deps.pyproject.rows();
                self.pyproject_table.render(frame, area, rows, placeholder);
            }
            ActiveTable::Environment => {
                let placeholder = placeholder(
                    &deps.env,
                    "Nothing installed yet. Press s to sync.",
                    "Could not list the environment",
                );
                self.env_table
                    .render(frame, area, deps.env.rows(), placeholder);
            }
        }
    }

    fn selected_name(&self, deps: &DepsState) -> Option<String> {
        match self.active_table {
            ActiveTable::Pyproject => self
                .pyproject_table
                .selected(deps.pyproject.rows())
                .map(|d| d.name.clone()),
            ActiveTable::Environment => self
                .env_table
                .selected(deps.env.rows())
                .map(|d| d.name.clone()),
        }
    }

    fn switch_table(&mut self) {
        self.active_table = match self.active_table {
            ActiveTable::Pyproject => ActiveTable::Environment,
            ActiveTable::Environment => ActiveTable::Pyproject,
        };
    }

    pub fn update(&mut self, action: &Action, deps: &DepsState) -> Vec<Command> {
        // An open input takes every key.
        if self.package_add_popup.is_editing() {
            if let Action::Nav(nav) = action
                && let Some(requirement) = self.package_add_popup.update(nav)
            {
                return vec![Command::Add { requirement }];
            }
            return Vec::new();
        }
        if self.pin_version_popup.is_editing() {
            if let Action::Nav(nav) = action
                && let Some(constraint) = self.pin_version_popup.update(nav)
                && let Some(name) = self.target.clone()
            {
                return vec![Command::Pin { name, constraint }];
            }
            return Vec::new();
        }
        match action {
            Action::Nav(NavAction::Up) => {
                if self.show_package_control_popup {
                    self.package_control_popup.up();
                } else if self.active_table == ActiveTable::Pyproject {
                    self.pyproject_table.previous(deps.pyproject.rows());
                } else {
                    self.env_table.previous(deps.env.rows());
                }
            }
            Action::Nav(NavAction::Down) => {
                if self.show_package_control_popup {
                    self.package_control_popup.down();
                } else if self.active_table == ActiveTable::Pyproject {
                    self.pyproject_table.next(deps.pyproject.rows());
                } else {
                    self.env_table.next(deps.env.rows());
                }
            }
            Action::Nav(NavAction::Select) => {
                if self.show_package_control_popup {
                    return self.choose(deps);
                }
                // Without a row the actions would have nothing to act on.
                let Some(name) = self.selected_name(deps) else {
                    return Vec::new();
                };
                self.target = Some(name);
                // A fresh popup, so it never opens with Remove still selected.
                self.package_control_popup = PackageControlPopup::new();
                self.show_package_control_popup = true;
            }
            Action::Nav(NavAction::Back) => self.show_package_control_popup = false,
            Action::Deps(DepsAction::SwitchTable) => {
                // The control popup acts on the row it was opened for.
                if !self.show_package_control_popup {
                    self.switch_table();
                }
            }
            Action::Deps(DepsAction::OpenAdd) => self.package_add_popup.open(),
            Action::Deps(DepsAction::Sync) => return vec![Command::Sync],
            Action::Deps(DepsAction::Refresh) => return vec![Command::Reload],
            // Confirm and Type only mean something in an input; Quit and Cancel belong to the app.
            Action::Nav(NavAction::Confirm | NavAction::Type(_))
            | Action::Quit
            | Action::Cancel => {}
        }
        Vec::new()
    }

    /// Runs the option highlighted in the actions list on the package it was opened for.
    fn choose(&mut self, deps: &DepsState) -> Vec<Command> {
        self.show_package_control_popup = false;
        let Some(name) = self.target.clone() else {
            return Vec::new();
        };
        match self.package_control_popup.selected() {
            Some(PackageControlOption::PinToVersion) => {
                self.pin_version_popup.open();
                Vec::new()
            }
            Some(PackageControlOption::Update) => vec![Command::Upgrade { name }],
            Some(PackageControlOption::Remove) => self.remove(&name, deps).into_iter().collect(),
            Some(PackageControlOption::Cancel) | None => Vec::new(),
        }
    }

    /// Removes `name` from wherever the active table lists it, if it still does.
    fn remove(&self, name: &str, deps: &DepsState) -> Option<Command> {
        match self.active_table {
            ActiveTable::Pyproject => {
                deps.pyproject
                    .rows()
                    .iter()
                    .find(|d| d.name == name)
                    .map(|d| Command::RemoveFromProject {
                        name: d.name.clone(),
                        groups: d.groups.clone(),
                    })
            }
            // Environment only: pyproject.toml is left alone. Confirmation and its warnings
            // come with #12.
            ActiveTable::Environment => {
                deps.env
                    .rows()
                    .iter()
                    .find(|d| d.name == name)
                    .map(|d| Command::Uninstall {
                        name: d.name.clone(),
                    })
            }
        }
    }
}

/// What a table shows in place of rows: how its load is going, that it loaded nothing, or why
/// it failed.
fn placeholder<T>(loadable: &Loadable<Vec<T>>, empty: &'static str, failed: &str) -> Text<'static> {
    match loadable {
        Loadable::NotLoaded => Text::default(),
        Loadable::Loading { since } => match since.elapsed().as_secs() {
            // A slow load says how long it has taken.
            waited @ 2.. => Text::from(format!("Loading… {waited}s")).dark_gray(),
            _ => Text::from("Loading…").dark_gray(),
        },
        Loadable::Loaded(_) => Text::from(empty).dark_gray(),
        Loadable::Failed(reason) => Text::from(format!("{failed}: {reason}")).red(),
    }
}

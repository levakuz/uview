use crate::{
    controllers::dependencies::{
        get_env_package_info, load_pyproject_deps, pin_dep_version, remove_dep_from_pyproject,
        upgrade_env_dep,
    },
    events::{Action, EnvDependency, PyprojectDependency},
    ui::components::{
        features::{
            add_packaage_popup::AddPackagePopup, package_control_popup::PackageControlPopup,
            pin_version_popup::PinVersionPopup,
        },
        shared::table::SharedTable,
    },
};
use ratatui::{Frame, symbols, widgets::Tabs};
use ratatui::{
    layout::Rect,
    style::{Color, Style},
};
use tokio::sync::mpsc;
#[derive(PartialEq)]
enum ActiveTable {
    Pyproject,
    Environment,
}

pub struct DependenciesTablesTabs {
    pyproject_table: SharedTable<PyprojectDependency>,
    env_table: SharedTable<EnvDependency>,
    active_table: ActiveTable,
    package_control_popup: PackageControlPopup<'static>,
    show_package_control_popup: bool,
    package_add_popup: AddPackagePopup,
    show_add_package_popup: bool,
    pin_version_popup: PinVersionPopup,
    show_pin_version_popup: bool,
    tx: mpsc::UnboundedSender<Action>,
}

impl DependenciesTablesTabs {
    pub fn new(tx: mpsc::UnboundedSender<Action>) -> Self {
        let package_control_popup = PackageControlPopup::new();
        let package_add_popup = AddPackagePopup::new();
        let pin_version_popup = PinVersionPopup::new();
        let mut pyproject_table = SharedTable::new(" pyproject.toml ");
        pyproject_table.set_focused(true);
        let mut env_table = SharedTable::new(" Environment ");
        env_table.set_focused(true);

        Self {
            pyproject_table,
            env_table,
            active_table: ActiveTable::Pyproject,
            package_control_popup,
            show_package_control_popup: false,
            tx,
            package_add_popup,
            show_add_package_popup: false,
            pin_version_popup,
            show_pin_version_popup: false,
        }
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        self.render_content(frame, area);
        self.render_tabs(frame, area);
        if self.show_package_control_popup {
            self.package_control_popup.render(frame, area);
        }
        if self.show_add_package_popup {
            self.package_add_popup.render(frame, area);
        }
        if self.show_pin_version_popup {
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
        let tabs = Tabs::new(vec!["pyproject.toml", "Environment"])
            .style(Color::White)
            .highlight_style(Style::default().green().bold())
            .select(selected_tab)
            .divider(symbols::DOT)
            .padding(" ", " ");
        frame.render_widget(tabs, area);
    }
    fn render_content(&mut self, frame: &mut Frame, area: Rect) {
        match self.active_table {
            ActiveTable::Pyproject => self.pyproject_table.render(frame, area),
            ActiveTable::Environment => self.env_table.render(frame, area),
        }
    }

    fn switch_table(&mut self) {
        self.active_table = match self.active_table {
            ActiveTable::Pyproject => ActiveTable::Environment,
            ActiveTable::Environment => ActiveTable::Pyproject,
        };
    }

    pub fn handle_event(&mut self, action: &Action) -> Option<Action> {
        if let Action::StopEditing = action {
            if self.package_add_popup.is_editing() {
                self.package_add_popup.handle_event(action);
                self.show_add_package_popup = false;
                return None;
            }
            if self.pin_version_popup.is_editing() {
                self.pin_version_popup.handle_event(action);
                self.show_pin_version_popup = false;
                return None;
            }
        }
        if self.package_add_popup.is_editing() {
            return self.package_add_popup.handle_event(action);
        }
        if self.pin_version_popup.is_editing() {
            return self.pin_version_popup.handle_event(action);
        }
        match action {
            Action::MoveUp => {
                if self.show_package_control_popup {
                    return self.package_control_popup.handle_event(action);
                }
                if self.active_table == ActiveTable::Pyproject {
                    self.pyproject_table.previous();
                } else {
                    self.env_table.previous();
                }
            }
            Action::MoveDown => {
                if self.show_package_control_popup {
                    return self.package_control_popup.handle_event(action);
                }
                if self.active_table == ActiveTable::Pyproject {
                    self.pyproject_table.next();
                } else {
                    self.env_table.next();
                }
            }
            Action::SwitchTable => {
                self.switch_table();
            }
            Action::LoadedPyprojectDeps(deps) => {
                self.pyproject_table.set_items(deps.clone());
            }
            Action::LoadedEnvDeps(deps) => {
                self.env_table.set_items(deps.clone());
            }
            Action::OpenPopup => {
                self.show_add_package_popup = true;
                self.package_add_popup.handle_event(&Action::Editing);
            }
            Action::OpenPinPopup => {
                self.show_package_control_popup = false;
                self.show_pin_version_popup = true;
                self.pin_version_popup.handle_event(&Action::Editing);
            }
            Action::SelectItem => {
                if self.show_package_control_popup {
                    return self.package_control_popup.handle_event(action);
                }
                if self.show_add_package_popup {
                    return self.package_add_popup.handle_event(action);
                }
                self.show_package_control_popup = true;
            }
            Action::RemovePackage => {
                let tx = self.tx.clone();
                if self.active_table == ActiveTable::Pyproject {
                    if let Some(dep) = self.pyproject_table.selected().cloned() {
                        let name = dep.name.clone();
                        let groups = dep.groups.clone();
                        tokio::spawn(async move {
                            if let Err(e) = remove_dep_from_pyproject(&name, groups).await {
                                let _ = tx.send(Action::Error(e.to_string()));
                            }
                        });
                    }
                } else if let Some(dep) = self.env_table.selected().cloned() {
                    let name = dep.name.clone();
                    tokio::spawn(async move {
                        // TODO
                        if let Err(e) =
                            remove_dep_from_pyproject(&name, vec!["main".to_string()]).await
                        {
                            let _ = tx.send(Action::Error(e.to_string()));
                        }
                    });
                }
                self.show_package_control_popup = false;
                return None;
            }
            Action::UpgradeEnvDep => {
                let name = match self.active_table {
                    ActiveTable::Pyproject => {
                        self.pyproject_table.selected().map(|d| d.name.clone())
                    }
                    ActiveTable::Environment => self.env_table.selected().map(|d| d.name.clone()),
                };
                if let Some(name) = name {
                    let tx = self.tx.clone();
                    tokio::spawn(async move {
                        match upgrade_env_dep(&name).await {
                            Ok(()) => send_env_refresh(&tx, &name).await,
                            Err(e) => {
                                let _ = tx.send(Action::Error(e.to_string()));
                            }
                        }
                    });
                }
                self.show_package_control_popup = false;
                return None;
            }
            Action::PinVersion(constraint) => {
                let name = match self.active_table {
                    ActiveTable::Pyproject => {
                        self.pyproject_table.selected().map(|d| d.name.clone())
                    }
                    ActiveTable::Environment => self.env_table.selected().map(|d| d.name.clone()),
                };
                if let Some(name) = name {
                    let constraint = constraint.clone();
                    let tx = self.tx.clone();
                    tokio::spawn(async move {
                        if let Err(e) = pin_dep_version(&name, &constraint).await {
                            let _ = tx.send(Action::Error(e.to_string()));
                            return;
                        }
                        // Reload rather than patch the row, so the table shows what uv wrote.
                        let _ = tx.send(match load_pyproject_deps().await {
                            Ok(deps) => Action::LoadedPyprojectDeps(deps),
                            Err(e) => Action::Error(format!("pyproject.toml: {e}")),
                        });
                        send_env_refresh(&tx, &name).await;
                    });
                }
                self.pin_version_popup.handle_event(&Action::StopEditing);
                self.show_pin_version_popup = false;
                return None;
            }
            Action::UpdatedEnvDep(dep) => {
                let name = dep.name.clone();
                self.env_table
                    .update_item_where(|d| d.name == name, dep.clone());
            }
            Action::ClosePopup => {
                self.show_package_control_popup = false;
                self.show_add_package_popup = false;
                self.show_pin_version_popup = false;
                return None;
            }
            _ => {
                if self.show_add_package_popup {
                    return self.package_add_popup.handle_event(action);
                }
                if self.show_pin_version_popup {
                    return self.pin_version_popup.handle_event(action);
                }
                if self.show_package_control_popup {
                    return self.package_control_popup.handle_event(action);
                }
            }
        }
        None
    }
}

/// Sends the refreshed environment entry for `name`. Silent when the package is not
/// installed — e.g. an extra that a plain sync leaves out — since the mutation itself succeeded.
async fn send_env_refresh(tx: &mpsc::UnboundedSender<Action>, name: &str) {
    match get_env_package_info(name).await {
        Ok(Some(dep)) => {
            let _ = tx.send(Action::UpdatedEnvDep(dep));
        }
        Ok(None) => {}
        Err(e) => {
            let _ = tx.send(Action::Error(e.to_string()));
        }
    }
}

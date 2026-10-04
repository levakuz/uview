use crate::{
    controllers::dependencies::{
        get_env_package_info, load_env_deps, load_pyproject_deps, pin_dep_version,
        remove_dep_from_pyproject, uninstall_from_env, upgrade_env_dep,
    },
    events::{Action, EnvDependency, PyprojectDependency},
    ui::components::{
        features::{
            add_packaage_popup::AddPackagePopup, package_control_popup::PackageControlPopup,
            pin_version_popup::PinVersionPopup,
        },
        shared::table::SharedTable,
    },
    uv::RealUvClient,
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
    /// The package the actions list was opened for. Its actions use this, not the selection
    /// when they run, which a reload can move to another row.
    target: Option<String>,
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
            target: None,
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

    fn selected_name(&self) -> Option<String> {
        match self.active_table {
            ActiveTable::Pyproject => self.pyproject_table.selected().map(|d| d.name.clone()),
            ActiveTable::Environment => self.env_table.selected().map(|d| d.name.clone()),
        }
    }

    fn switch_table(&mut self) {
        self.active_table = match self.active_table {
            ActiveTable::Pyproject => ActiveTable::Environment,
            ActiveTable::Environment => ActiveTable::Pyproject,
        };
    }

    pub fn handle_event(&mut self, action: &Action) -> Option<Action> {
        // Results update the tables whatever has focus: an open input must not swallow them.
        match action {
            Action::LoadedPyprojectDeps(deps) => {
                self.pyproject_table.set_items(deps.clone());
                return None;
            }
            Action::LoadedEnvDeps(deps) => {
                self.env_table.set_items(deps.clone());
                return None;
            }
            Action::UpdatedEnvDep(dep) => {
                let name = dep.name.clone();
                self.env_table
                    .update_item_where(|d| d.name == name, dep.clone());
                return None;
            }
            _ => {}
        }
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
                // The control popup acts on the row it was opened for.
                if !self.show_package_control_popup {
                    self.switch_table();
                }
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
                // Without a row the actions would have nothing to act on.
                self.target = Some(self.selected_name()?);
                // A fresh popup, so it never opens with Remove still selected.
                self.package_control_popup = PackageControlPopup::new();
                self.show_package_control_popup = true;
            }
            Action::RemovePackage => {
                let tx = self.tx.clone();
                if self.active_table == ActiveTable::Pyproject {
                    if let Some(PyprojectDependency { name, groups, .. }) = self
                        .pyproject_table
                        .find(|d| Some(&d.name) == self.target.as_ref())
                        .cloned()
                    {
                        tokio::spawn(async move {
                            let _ = tx.send(
                                match remove_dep_from_pyproject(&RealUvClient, &name, groups).await
                                {
                                    Ok(()) => Action::Notice(format!(
                                        "Removed {name} from pyproject.toml"
                                    )),
                                    Err(e) => Action::Error(e.to_string()),
                                },
                            );
                            // Also after a failure: `uv remove` may already have edited
                            // pyproject.toml, and a stale row would replay that membership.
                            reload_tables(&tx).await;
                        });
                    }
                } else if let Some(dep) = self
                    .env_table
                    .find(|d| Some(&d.name) == self.target.as_ref())
                    .cloned()
                {
                    // Environment only: pyproject.toml is left alone. Confirmation and its
                    // warnings come with #12.
                    let name = dep.name;
                    tokio::spawn(async move {
                        let _ = tx.send(match uninstall_from_env(&RealUvClient, &name).await {
                            Ok(()) => {
                                Action::Notice(format!("Uninstalled {name} from the environment"))
                            }
                            Err(e) => Action::Error(e.to_string()),
                        });
                        // Drop the row, so it cannot be "uninstalled" again as a no-op.
                        reload_tables(&tx).await;
                    });
                }
                self.show_package_control_popup = false;
                return None;
            }
            Action::UpgradeEnvDep => {
                if let Some(name) = self.target.clone() {
                    let tx = self.tx.clone();
                    tokio::spawn(async move {
                        match upgrade_env_dep(&RealUvClient, &name).await {
                            Ok(()) => {
                                let _ = tx.send(Action::Notice(format!("Updated {name}")));
                                send_env_refresh(&tx, &name).await;
                            }
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
                if let Some(name) = self.target.clone() {
                    let constraint = constraint.clone();
                    let tx = self.tx.clone();
                    tokio::spawn(async move {
                        // Sent first, so an error from the reload below replaces it.
                        let _ = tx.send(
                            match pin_dep_version(&RealUvClient, &name, &constraint).await {
                                Ok(pinned) => Action::Notice(format!("Pinned {pinned}")),
                                Err(e) => Action::Error(e.to_string()),
                            },
                        );
                        // Reload rather than patch rows, so the tables show what uv wrote,
                        // including after a failure part-way through the memberships.
                        reload_tables(&tx).await;
                    });
                }
                self.pin_version_popup.handle_event(&Action::StopEditing);
                self.show_pin_version_popup = false;
                return None;
            }
            Action::ClosePopup | Action::StopEditing => {
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
    match get_env_package_info(&RealUvClient, name).await {
        Ok(Some(dep)) => {
            let _ = tx.send(Action::UpdatedEnvDep(dep));
        }
        Ok(None) => {}
        Err(e) => {
            let _ = tx.send(Action::Error(e.to_string()));
        }
    }
}

/// Re-reads both tables from disk after a mutation. Uses the same loaders as startup.
async fn reload_tables(tx: &mpsc::UnboundedSender<Action>) {
    let _ = tx.send(match load_pyproject_deps().await {
        Ok(deps) => Action::LoadedPyprojectDeps(deps),
        Err(e) => Action::Error(format!("pyproject.toml: {e}")),
    });
    let _ = tx.send(match load_env_deps(&RealUvClient).await {
        Ok(deps) => Action::LoadedEnvDeps(deps),
        Err(e) => Action::Error(e.to_string()),
    });
}

use std::fmt;

use crossterm::event::KeyEvent;
use serde::Deserialize;

#[derive(Clone, Debug)]
pub enum Action {
    Quit,
    MoveUp,
    MoveDown,
    SwitchTable,
    SelectItem,
    OpenPopup,
    ClosePopup,
    LoadedPyprojectDeps(Vec<PyprojectDependency>),
    LoadedEnvDeps(Vec<EnvDependency>),
    RemovePackage,
    UpgradeEnvDep,
    UpdatedEnvDep(EnvDependency),
    OpenPinPopup,
    PinVersion(String),
    Editing,
    ConfirmInput,
    StopEditing,
    ButtonPressed(KeyEvent),
    Error(String),
    Notice(String),
}

/// Where a dependency is declared in pyproject.toml.
#[derive(Clone, Debug, PartialEq)]
pub enum Membership {
    /// `[project.dependencies]`
    Project,
    /// `[dependency-groups.<name>]`
    Group(String),
    /// `[project.optional-dependencies.<name>]`
    Extra(String),
}

impl fmt::Display for Membership {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Membership::Project => write!(f, "main"),
            Membership::Group(name) => write!(f, "{name}"),
            Membership::Extra(name) => write!(f, "extra:{name}"),
        }
    }
}

#[derive(Clone, Debug)]
pub struct PyprojectDependency {
    pub name: String,
    pub groups: Vec<Membership>,
    pub version_constraint: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct EnvDependency {
    pub name: String,
    pub version: String,
}

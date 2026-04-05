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
}

#[derive(Clone, Debug)]
pub struct PyprojectDependency {
    pub name: String,
    pub groups: Vec<String>,
    pub version_constraint: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct EnvDependency {
    pub name: String,
    pub version: String,
}

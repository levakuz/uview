//! The messages that move through the app, split by direction of travel (ADR-0001).

use crossterm::event::KeyEvent;

use crate::domain::{EnvDependency, Membership, PyprojectDependency};

/// What the terminal sends.
#[derive(Debug)]
pub enum Input {
    Key(KeyEvent),
}

/// What the user means. A page's keymap turns input into an action, which flows down the
/// component tree.
#[derive(Debug)]
pub enum Action {
    Quit,
    Nav(NavAction),
    Deps(DepsAction),
}

/// Moving, choosing and typing, which any page understands.
#[derive(Debug)]
pub enum NavAction {
    Up,
    Down,
    /// Choose the highlighted row or item.
    Select,
    /// Close a popup or cancel input.
    Back,
    /// Accept what was typed.
    Confirm,
    /// A key typed into an input.
    Type(KeyEvent),
}

/// Intent only the dependencies page understands.
#[derive(Debug)]
pub enum DepsAction {
    SwitchTable,
    OpenAdd,
}

/// An effect a component asks for. Components return these and never perform them; the
/// runtime does.
#[derive(Debug, PartialEq)]
pub enum Command {
    /// Read both tables again.
    Reload,
    /// Add a requirement such as `httpx>=0.27` to the main dependencies, then sync.
    Add {
        requirement: String,
    },
    /// Remove a package from every membership in pyproject.toml, then sync.
    RemoveFromProject {
        name: String,
        groups: Vec<Membership>,
    },
    /// Uninstall a package from the environment only.
    Uninstall {
        name: String,
    },
    Upgrade {
        name: String,
    },
    Pin {
        name: String,
        constraint: String,
    },
}

/// What a command produced. The app shows notices and errors and hands the rest to the page.
#[derive(Debug)]
pub enum Event {
    PyprojectLoaded(Vec<PyprojectDependency>),
    EnvLoaded(Vec<EnvDependency>),
    EnvUpdated(EnvDependency),
    Notice(String),
    Error(String),
}

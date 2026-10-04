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
    /// Stop every job: the running ones and those waiting behind them.
    Cancel,
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
    Sync,
}

/// An effect a component asks for. Components return these and never perform them; the
/// runtime does.
#[derive(Debug, PartialEq)]
pub enum Command {
    /// Read both tables again.
    Reload,
    /// Bring the environment exactly in line with the lockfile: plain `uv sync`.
    Sync,
    /// Add a requirement such as `httpx>=0.27` to the main dependencies with `uv add`.
    Add {
        requirement: String,
    },
    /// Remove a package from every membership in pyproject.toml with `uv remove`.
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

/// Names a job: a command the runtime has accepted, from then until it finishes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JobId(pub u64);

/// What a command produced. The app shows notices, errors and jobs, and hands the rest to the
/// page.
#[derive(Debug)]
pub enum Event {
    PyprojectLoaded(Vec<PyprojectDependency>),
    EnvLoaded(Vec<EnvDependency>),
    Notice(String),
    Error(String),
    /// A command became a job, named by `label`. It may wait behind a running mutation.
    JobQueued {
        id: JobId,
        label: String,
    },
    JobStarted(JobId),
    /// A line the job's `uv` wrote to stderr, where it reports its progress.
    JobOutput(JobId, String),
    /// The job is over: done, failed or cancelled.
    JobFinished(JobId),
}

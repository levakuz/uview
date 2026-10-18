//! The messages that move through the app, split by direction of travel (ADR-0001).

use crossterm::event::KeyEvent;

use crate::domain::{
    DeclaredGroup, EnvDependency, Index, Membership, ProjectInfo, PyprojectDependency, Source,
    VersionChange, VersionList,
};

/// What the terminal sends.
#[derive(Debug)]
pub enum Input {
    Key(KeyEvent),
}

/// What the user means. The keymap turns input into an action, which flows down the component
/// tree.
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    Quit,
    /// Stop every job: the running ones and those waiting behind them.
    Cancel,
    /// Open or close the help.
    Help,
    Nav(NavAction),
    Deps(DepsAction),
}

/// Moving, choosing and typing, which any page understands.
#[derive(Clone, Debug, PartialEq)]
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
#[derive(Clone, Debug, PartialEq)]
pub enum DepsAction {
    SwitchTable,
    OpenAdd,
    /// Add a package from an index, git or a path, rather than the default index.
    OpenAddFromSource,
    /// Show the project's indexes, to add or remove one.
    OpenIndexes,
    /// Show the project's dependency groups, to create, rename or delete one.
    OpenGroups,
    /// Look a package up by name, to add it.
    OpenSearch,
    /// Show the project's version, to bump it or set it.
    OpenVersion,
    /// Bump the selected package to its next patch, minor or major release.
    Bump,
    /// Update the selected package, as its actions list's Update does.
    Update,
    /// Remove the selected package, as its actions list's Remove does.
    Remove,
    Sync,
    Refresh,
}

/// An effect a component asks for. Components return these and never perform them; the
/// runtime does.
#[derive(Debug, PartialEq)]
pub enum Command {
    /// Read both tables, and the project's name and version, again.
    Reload,
    /// Bring the environment exactly in line with the lockfile: plain `uv sync`.
    Sync,
    /// Add a requirement such as `httpx>=0.27` to `membership` with `uv add`.
    Add {
        requirement: String,
        membership: Membership,
    },
    /// Add `requirement` to `to` with `uv add`, then remove `name` from `from` with `uv remove`.
    Move {
        name: String,
        requirement: String,
        from: Membership,
        to: Membership,
    },
    /// Add a package from `source` to `membership` with `uv add`.
    AddFrom {
        source: Source,
        membership: Membership,
    },
    /// Find out what each bump would make the project's version, writing nothing.
    PreviewVersions,
    /// Read the versions `name` can be pinned to from its index.
    ListVersions {
        name: String,
    },
    /// Change the project's version with `uv version`.
    SetVersion(VersionChange),
    /// Add an index to `[[tool.uv.index]]`, then `uv lock`. A default one replaces PyPI.
    AddIndex {
        name: String,
        url: String,
        default: bool,
    },
    /// Remove an index from `[[tool.uv.index]]`, then `uv lock`.
    RemoveIndex(Index),
    /// Add the empty dependency group `name` to pyproject.toml, then `uv lock`.
    CreateGroup {
        name: String,
    },
    /// Delete the empty dependency group `name` from pyproject.toml, then `uv lock`.
    DeleteGroup {
        name: String,
    },
    /// Remove a package from the given memberships in pyproject.toml with `uv remove`.
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
    /// The pyproject table's rows, or why they could not be read, from a reload that started at
    /// `generation`.
    PyprojectLoaded {
        generation: u64,
        loaded: Result<Vec<PyprojectDependency>, String>,
        /// The groups pyproject.toml declares empty, which have no rows.
        empty_groups: Vec<String>,
        /// Those of `empty_groups` that could be deleted.
        deletable_groups: Vec<String>,
        /// The project's `[[tool.uv.index]]` entries, in file order.
        indexes: Vec<Index>,
        /// The project's dependency groups, in file order.
        groups: Vec<DeclaredGroup>,
    },
    /// The Environment table's rows, or why they could not be listed, likewise.
    EnvLoaded {
        generation: u64,
        loaded: Result<Vec<EnvDependency>, String>,
    },
    /// The project's name and version, or what uv said when it could not read them.
    /// From a reload that started at `generation`, like the tables'.
    ProjectLoaded {
        generation: u64,
        loaded: Result<ProjectInfo, String>,
    },
    /// What each bump would make the version, in `Bump::ALL`'s order, or why uv could not say.
    VersionsPreviewed(Result<Vec<String>, String>),
    /// The versions `name` can be pinned to, newest first, or why the index could not say.
    VersionsListed {
        name: String,
        loaded: Result<VersionList, String>,
    },
    /// A mutation has ended, however it did: what the views show of the project may be stale.
    ProjectMutated,
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

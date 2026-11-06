//! The messages that move through the app, split by direction of travel (ADR-0001).

use std::path::PathBuf;

use crossterm::event::KeyEvent;

use crate::domain::{
    Artifact, DeclaredGroup, EnvDependency, EnvInfo, Index, LockState, Membership, ProjectInfo,
    PublishTarget, PyprojectDependency, PythonOption, Source, TreeNode, VersionChange, VersionList,
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
    /// Write a settings file to edit, unless there is one.
    WriteConfig,
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
    /// Show the project's environment, to make it anew, empty it or delete it.
    OpenEnvironment,
    /// Build the project's sdist and wheel.
    Build,
    /// Publish what the last build wrote.
    Publish,
    /// Lock the project, `upgrade` moving every package to its newest allowed version.
    Lock {
        upgrade: bool,
    },
    /// Look a package up by name, to add it.
    OpenSearch,
    /// Show the project's version, to bump it or set it.
    OpenVersion,
    /// Bump the selected package to its next patch, minor or major release.
    Bump,
    /// The tab before the active one.
    SwitchTableBack,
    /// On the tree: unfold the highlighted package, or fold it or go to its parent.
    Expand,
    Collapse,
    /// On the tree: unfold everything, or fold everything below the direct dependencies.
    ExpandAll,
    CollapseAll,
    /// On the tree: show or hide which packages are behind, which asks the index.
    Outdated,
    /// On the tree: show what pulls the highlighted package in.
    Why,
    /// Update the selected package, as its actions list's Update does.
    Update,
    /// Remove the selected package, as its actions list's Remove does; in a list, delete the
    /// highlighted item.
    Remove,
    /// In the list of groups, rename the highlighted one.
    Rename,
    /// In the list of groups, add one.
    New,
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
    /// Lock the project with `uv lock`, or move every package to its newest allowed version
    /// with `uv lock --upgrade`.
    Lock {
        upgrade: bool,
    },
    /// Write `contents` to the settings file at `path`, unless a file is there.
    WriteConfig {
        path: PathBuf,
        contents: String,
    },
    /// Say so if the uv on PATH is too old for uview.
    CheckUv,
    /// Build the project's sdist and wheel with `uv build`.
    Build,
    /// Upload `files` to `target` with `uv publish`.
    Publish {
        target: PublishTarget,
        files: Vec<Artifact>,
    },
    /// List every Python uv can find or download.
    ListPythons,
    /// Make the environment anew with `python`, and install the project.
    RecreateEnv {
        python: PythonOption,
    },
    /// Empty the environment, then install the project in it again.
    ResetEnv,
    /// Delete the environment's directory.
    DeleteEnv,
    /// Read the dependency tree again, each package that is behind marked when `outdated`.
    LoadTree {
        outdated: bool,
    },
    /// Read what pulls `name` in.
    WhyInstalled {
        name: String,
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
    /// The dependency tree, from a job that started at `generation`; `outdated` when it marks
    /// what is behind.
    TreeLoaded {
        generation: u64,
        outdated: bool,
        loaded: Result<Vec<TreeNode>, String>,
    },
    /// What pulls `name` in, as a tree up to the project.
    WhyLoaded {
        name: String,
        loaded: Result<Vec<TreeNode>, String>,
    },
    /// The project's environment, or `None` when there is none, from a reload that started at
    /// `generation`.
    EnvInfoLoaded {
        generation: u64,
        info: Option<EnvInfo>,
    },
    /// The files a build wrote to `dist/`.
    Built(Vec<Artifact>),
    /// Every Python uv can find or download, or what uv said when it could not list them.
    PythonsListed(Result<Vec<PythonOption>, String>),
    /// Whether uv.lock matches pyproject.toml, from a reload that started at `generation`.
    LockChecked {
        generation: u64,
        state: LockState,
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

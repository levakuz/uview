use std::path::{Component, Path, PathBuf};

use crate::domain::{Bump, LockState, ProjectInfo, TreeNode, VersionChange};
use crate::uv::{UvClient, UvError, args, parse};

/// The nearest directory at or above `start` that holds a pyproject.toml: where uv, started
/// from `start`, looks for its project. A relative `start` is taken from the working
/// directory, and `..` is resolved lexically.
pub fn find_project_root(start: &Path) -> Option<PathBuf> {
    let mut dir = PathBuf::new();
    for part in std::path::absolute(start).ok()?.components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir => {
                dir.pop();
            }
            part => dir.push(part),
        }
    }
    dir.ancestors()
        .find(|dir| dir.join("pyproject.toml").is_file())
        .map(Path::to_path_buf)
}

/// Why uv would refuse to manage the project at `root`, or `None` if it would not.
pub fn refusal(root: &Path) -> Option<String> {
    let path = root.join("pyproject.toml");
    // ponytail: an unreadable or malformed file is left for the pyproject table to report.
    let toml: toml::Table = std::fs::read_to_string(&path).ok()?.parse().ok()?;
    let uv = toml.get("tool").and_then(|tool| tool.get("uv"));
    let managed = uv
        .and_then(|uv| uv.get("managed"))
        .and_then(toml::Value::as_bool);
    if managed == Some(false) {
        Some(format!(
            "{path:?} marks the project as unmanaged (`[tool.uv] managed = false`)."
        ))
    } else if toml.contains_key("project") || uv.and_then(|uv| uv.get("workspace")).is_some() {
        None
    } else {
        Some(format!(
            "{path:?} has no [project] table, so uv will not manage it.\n\
             Add one, or run uview inside a Python project."
        ))
    }
}

/// The root of the uv workspace the project rooted at `project` belongs to: the directory whose
/// environment uv syncs for it. A project that declares a workspace is its own root. Otherwise
/// uv looks only at the nearest pyproject.toml above: its directory is the root if its
/// workspace's `members` match the project and its `exclude` does not. A plain project in
/// between, an unreadable file, or no workspace at all leaves the project on its own.
pub fn workspace_root(project: &Path) -> PathBuf {
    if workspace_globs(project).is_none()
        && let Some(above) = project
            .ancestors()
            .skip(1)
            .find(|dir| dir.join("pyproject.toml").is_file())
        && let Some((members, exclude)) = workspace_globs(above)
        && matches_any(&members, above, project)
        && !matches_any(&exclude, above, project)
    {
        return above.to_path_buf();
    }
    project.to_path_buf()
}

/// Where the project's files are: resolved once, at startup, and handed to whatever needs one.
#[derive(Clone, Debug)]
pub struct ProjectPaths {
    /// The project's root, where `uv` runs.
    pub root: PathBuf,
    /// Its pyproject.toml.
    pub pyproject: PathBuf,
    /// The environment `uv sync` uses for it, which `uv pip` must be told.
    pub env: PathBuf,
}

impl ProjectPaths {
    /// The paths of the project at `root`, its environment `configured` as
    /// `UV_PROJECT_ENVIRONMENT` says, if it says.
    pub fn at(root: PathBuf, configured: Option<&str>) -> Self {
        Self {
            pyproject: root.join("pyproject.toml"),
            env: project_environment(&root, configured),
            root,
        }
    }
}

/// The environment uv syncs for `project`: `configured`, the value of
/// `UV_PROJECT_ENVIRONMENT`, or else `.venv`. A relative path is taken from the workspace root.
pub fn project_environment(project: &Path, configured: Option<&str>) -> PathBuf {
    // Empty means unset to uv.
    let env = configured.filter(|env| !env.is_empty()).unwrap_or(".venv");
    workspace_root(project).join(env)
}

/// The `members` and `exclude` globs of the workspace the pyproject.toml in `dir` declares, if
/// it declares one.
fn workspace_globs(dir: &Path) -> Option<(Vec<String>, Vec<String>)> {
    let toml: toml::Table = std::fs::read_to_string(dir.join("pyproject.toml"))
        .ok()?
        .parse()
        .ok()?;
    let workspace = toml.get("tool")?.get("uv")?.get("workspace")?;
    let globs = |key: &str| -> Vec<String> {
        workspace
            .get(key)
            .and_then(toml::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(toml::Value::as_str)
            .map(String::from)
            .collect()
    };
    Some((globs("members"), globs("exclude")))
}

/// Whether one of `globs`, taken from `root`, matches `path`, as uv matches them: `*` crosses
/// `/`, and a glob that does not parse matches nothing.
fn matches_any(globs: &[String], root: &Path, path: &Path) -> bool {
    let root = glob::Pattern::escape(&root.to_string_lossy());
    globs.iter().any(|glob| {
        let pattern = Path::new(&root).join(glob);
        glob::Pattern::new(&pattern.to_string_lossy()).is_ok_and(|p| p.matches_path(path))
    })
}

/// The oldest uv uview works with: `uv venv --clear`, which emptying the environment runs, came
/// in 0.8.0.
pub const MIN_UV: &str = "0.8.0";

/// What to tell the user when the uv on PATH is older than `MIN_UV`, or `None`: also when uv
/// cannot be run, which every other call reports.
pub async fn uv_too_old(uv: &impl UvClient) -> Option<String> {
    let found = parse::uv_version(&uv.run(&args::UV_VERSION).await.ok()?)?;
    let least: pep440_rs::Version = std::str::FromStr::from_str(MIN_UV).ok()?;
    (found < least).then(|| {
        format!(
            "uv {found} is older than uview needs, {MIN_UV}: update it, as with `uv self update`"
        )
    })
}

/// The project's name and version, as `uv version` reports them.
pub async fn load_project_info(uv: &impl UvClient) -> Result<ProjectInfo, UvError> {
    parse::version(&uv.run(&args::VERSION).await?)
}

/// The versions each bump would make, in `Bump::ALL`'s order. Nothing is written.
pub async fn preview_bumps(uv: &impl UvClient) -> Result<Vec<String>, UvError> {
    let mut versions = Vec::new();
    for bump in Bump::ALL {
        let args = args::version_args(&VersionChange::Bump(bump), true);
        let out = uv
            .run(&args.iter().map(String::as_str).collect::<Vec<_>>())
            .await?;
        versions.push(parse::version(&out)?.version);
    }
    Ok(versions)
}

/// Whether uv.lock matches pyproject.toml, as uv says without the network. A cancel is not an
/// answer.
pub async fn check_lock(uv: &impl UvClient) -> Result<LockState, UvError> {
    match uv.run(&args::LOCK_CHECK).await {
        Err(UvError::Cancelled) => Err(UvError::Cancelled),
        checked => Ok(parse::lock_state(&checked)),
    }
}

/// Locks the project with `uv lock`, or with `uv lock --upgrade` to move every package to the
/// newest version its constraints allow.
pub async fn lock(uv: &impl UvClient, upgrade: bool) -> Result<(), UvError> {
    if upgrade {
        uv.run(&args::LOCK_UPGRADE).await?;
    } else {
        uv.run(&args::LOCK).await?;
    }
    Ok(())
}

/// The project's dependency tree as uv.lock holds it, each package that is behind marked when
/// `outdated`, which asks the index.
pub async fn load_tree(uv: &impl UvClient, outdated: bool) -> Result<Vec<TreeNode>, UvError> {
    let out = if outdated {
        uv.run(&args::TREE_OUTDATED).await?
    } else {
        uv.run(&args::TREE).await?
    };
    Ok(parse::tree(&out))
}

/// What pulls `name` in, as a tree from it up to the project.
pub async fn why_installed(uv: &impl UvClient, name: &str) -> Result<Vec<TreeNode>, UvError> {
    Ok(parse::tree(&uv.run(&args::why_args(name)).await?))
}

/// Makes `change` to the project's version, and returns the new one.
pub async fn set_version(uv: &impl UvClient, change: &VersionChange) -> Result<String, UvError> {
    let args = args::version_args(change, false);
    let out = uv
        .run(&args.iter().map(String::as_str).collect::<Vec<_>>())
        .await?;
    Ok(parse::version(&out)?.version)
}

/// Creates a project in `dir` with `uv init`, named `name`, or after `dir`.
pub async fn init_project(
    uv: &impl UvClient,
    dir: &Path,
    name: Option<&str>,
) -> Result<(), UvError> {
    let dir = dir.to_string_lossy();
    uv.run(&args::init_args(&dir, name)).await?;
    Ok(())
}

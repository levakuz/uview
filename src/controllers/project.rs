use std::path::{Component, Path, PathBuf};

use crate::domain::ProjectInfo;
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

/// The project's name and version, as `uv version` reports them.
pub async fn load_project_info(uv: &impl UvClient) -> Result<ProjectInfo, UvError> {
    parse::version(&uv.run(&args::VERSION).await?)
}

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

/// The project's name and version, as `uv version` reports them.
pub async fn load_project_info(uv: &impl UvClient) -> Result<ProjectInfo, UvError> {
    parse::version(&uv.run(&args::VERSION).await?)
}

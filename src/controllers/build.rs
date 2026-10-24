//! Building the project's distributions with `uv build`, and finding what it wrote.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::controllers::project::{ProjectPaths, workspace_root};
use crate::domain::Artifact;
use crate::uv::{UvClient, UvError, args};

/// Where `uv build` writes for the project at `root`: `dist/` at its workspace root, which is
/// the project's own root unless it is a workspace member.
pub fn dist_dir(root: &Path) -> PathBuf {
    workspace_root(root).join("dist")
}

/// Whether the pyproject.toml `text` has no `[project]`, being a virtual workspace root, where
/// plain `uv build` builds an empty `unknown` package.
pub fn is_virtual_root(text: &str) -> bool {
    text.parse::<toml::Table>()
        .is_ok_and(|toml| !toml.contains_key("project"))
}

/// The distributions in `dist`, wheels and sdists, modified at `since` or after, by name, with
/// their sizes. The `.gitignore` uv writes there is no distribution.
pub fn built_since(dist: &Path, since: SystemTime) -> Vec<Artifact> {
    let Ok(entries) = std::fs::read_dir(dist) else {
        return Vec::new();
    };
    let mut built: Vec<Artifact> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            if !(name.ends_with(".whl") || name.ends_with(".tar.gz")) {
                return None;
            }
            let metadata = entry.metadata().ok()?;
            (metadata.is_file() && metadata.modified().ok()? >= since).then(|| Artifact {
                path: entry.path(),
                size: metadata.len(),
            })
        })
        .collect();
    built.sort_by(|a, b| a.path.cmp(&b.path));
    built
}

/// Builds the project's sdist and wheel, every member's at a virtual workspace root, and
/// returns the files it wrote.
pub async fn build(uv: &impl UvClient, paths: &ProjectPaths) -> Result<Vec<Artifact>, UvError> {
    let virtual_root = tokio::fs::read_to_string(&paths.pyproject)
        .await
        .is_ok_and(|text| is_virtual_root(&text));
    // Whole seconds: a filesystem may keep modification times no finer.
    let now = SystemTime::now();
    let since = now
        .duration_since(UNIX_EPOCH)
        .map_or(now, |d| UNIX_EPOCH + Duration::from_secs(d.as_secs()));
    uv.run(args::build_args(virtual_root)).await?;
    Ok(built_since(&dist_dir(&paths.root), since))
}

/// What the status line says of a build that wrote `built` to `dist`, as shown from the root.
pub fn built_notice(built: &[Artifact], dist: &str) -> String {
    let names: Vec<String> = built.iter().map(Artifact::name).collect();
    match names.len() {
        0 => format!("uv build wrote nothing new to {dist}/"),
        1 | 2 => format!("Built {} in {dist}/", names.join(", ")),
        n => format!("Built {n} files in {dist}/"),
    }
}

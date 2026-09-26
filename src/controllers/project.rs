use std::path::{Path, PathBuf};

/// The nearest directory at or above `start` that holds a pyproject.toml — the project uv
/// itself acts on from there.
pub fn find_project_root(start: &Path) -> Option<PathBuf> {
    start
        .ancestors()
        .find(|dir| dir.join("pyproject.toml").is_file())
        .map(Path::to_path_buf)
}

//! uview's own edits of pyproject.toml, for what uv has no command for (ADR-0009, ADR-0010).

use std::path::Path;

use crate::uv::{UvClient, UvError, args};

pub const PYPROJECT: &str = "./pyproject.toml";

/// Rewrites the `pyproject` file with `edit`, then relocks: uv.lock follows what the file
/// declares. If `edit` refuses, nothing is written; if uv cannot lock, the file is put back as
/// it was.
pub async fn edit_then_lock(
    uv: &impl UvClient,
    pyproject: &Path,
    edit: impl FnOnce(&str) -> Result<String, String>,
) -> Result<(), UvError> {
    let io = |e: std::io::Error| UvError::Pyproject(e.to_string());
    let before = tokio::fs::read_to_string(pyproject).await.map_err(io)?;
    let after = edit(&before).map_err(UvError::Pyproject)?;
    tokio::fs::write(pyproject, after).await.map_err(io)?;
    if let Err(e) = uv.run(&args::LOCK).await {
        tokio::fs::write(pyproject, before).await.map_err(io)?;
        return Err(e);
    }
    Ok(())
}

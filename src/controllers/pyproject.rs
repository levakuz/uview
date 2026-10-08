//! uview's own edits of pyproject.toml, for what uv has no command for (ADR-0009, ADR-0010).

use std::path::Path;

use crate::uv::{UvClient, UvError, args};

pub const PYPROJECT: &str = "./pyproject.toml";

/// Rewrites the `pyproject` file with `edit`, then relocks: uv.lock follows what the file
/// declares. If `edit` refuses, nothing is written; if uv cannot lock, the file is put back as
/// it was, and the error says so. Why uv could not is in the uv log: its first line, which is
/// all the status line has room for, rarely says.
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
        return Err(match e {
            UvError::Failed { .. } => UvError::Pyproject(
                "left as it was, since uv could not lock the project with the change; the uv log \
                 says why"
                    .into(),
            ),
            other => other,
        });
    }
    Ok(())
}

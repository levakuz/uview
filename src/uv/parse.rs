//! `uv` output, read without spawning or awaiting anything.

use super::UvError;
use crate::domain::LockState;
use crate::domain::{EnvDependency, ProjectInfo};

/// The packages in `uv pip list --format json` output.
pub fn pip_list(stdout: &str) -> Result<Vec<EnvDependency>, UvError> {
    serde_json::from_str(stdout).map_err(|e| UvError::Parse(e.to_string()))
}

/// The project's name and version in `uv version --output-format json` output.
pub fn version(stdout: &str) -> Result<ProjectInfo, UvError> {
    serde_json::from_str(stdout).map_err(|e| UvError::Parse(e.to_string()))
}

/// What `uv lock --check` said of uv.lock. Its exit status tells only fresh from not; why not is
/// in its words, read here alone so that a change in them breaks one place.
pub fn lock_state(checked: &Result<String, UvError>) -> LockState {
    match checked {
        Ok(_) => LockState::Fresh,
        Err(UvError::Failed { stderr, .. }) if stderr.contains("needs to be updated") => {
            LockState::Stale
        }
        Err(UvError::Failed { stderr, .. }) if stderr.contains("Unable to find lockfile") => {
            LockState::Missing
        }
        Err(_) => LockState::Unknown,
    }
}

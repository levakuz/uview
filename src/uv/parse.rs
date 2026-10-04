//! `uv` output, read without spawning or awaiting anything.

use super::UvError;
use crate::domain::{EnvDependency, ProjectInfo};

/// The packages in `uv pip list --format json` output.
pub fn pip_list(stdout: &str) -> Result<Vec<EnvDependency>, UvError> {
    serde_json::from_str(stdout).map_err(|e| UvError::Parse(e.to_string()))
}

/// The project's name and version in `uv version --output-format json` output.
pub fn version(stdout: &str) -> Result<ProjectInfo, UvError> {
    serde_json::from_str(stdout).map_err(|e| UvError::Parse(e.to_string()))
}

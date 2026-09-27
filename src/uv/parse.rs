//! `uv` output, read without spawning or awaiting anything.

use super::UvError;
use crate::events::EnvDependency;

/// The packages in `uv pip list --format json` output.
pub fn pip_list(stdout: &str) -> Result<Vec<EnvDependency>, UvError> {
    serde_json::from_str(stdout).map_err(|e| UvError::Parse(e.to_string()))
}

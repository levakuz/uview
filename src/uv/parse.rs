//! `uv` output, read without spawning or awaiting anything.

use super::UvError;
use crate::domain::{EnvDependency, ProjectInfo};
use crate::domain::{LockState, PythonOption, TreeNode};

/// The packages in `uv pip list --format json` output.
pub fn pip_list(stdout: &str) -> Result<Vec<EnvDependency>, UvError> {
    serde_json::from_str(stdout).map_err(|e| UvError::Parse(e.to_string()))
}

/// The project's name and version in `uv version --output-format json` output.
pub fn version(stdout: &str) -> Result<ProjectInfo, UvError> {
    serde_json::from_str(stdout).map_err(|e| UvError::Parse(e.to_string()))
}

/// The version `uv --version` names, as in `uv 0.8.9 (68c0bf8a2 2025-08-11)`.
pub fn uv_version(stdout: &str) -> Option<pep440_rs::Version> {
    let version = stdout.split_whitespace().nth(1)?;
    std::str::FromStr::from_str(version).ok()
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
        Err(UvError::Failed { stderr, .. })
            if stderr.contains("network was disabled")
                || stderr.contains("Network connectivity is disabled") =>
        {
            LockState::Offline
        }
        Err(UvError::Failed { stderr, .. }) => LockState::Unknown(first_line(stderr)),
        Err(e) => LockState::Unknown(e.to_string()),
    }
}

/// The first line uv wrote that says something, without its `error:` or `×`.
fn first_line(stderr: &str) -> String {
    let line = stderr
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or_default();
    let said = line
        .strip_prefix("error:")
        .or_else(|| line.strip_prefix('×'));
    said.unwrap_or(line).trim().to_string()
}

/// The packages of `uv tree`'s output, in order, each at its depth. A line is drawn as
/// `│   ` or four spaces per level above it, then `├── ` or `└── `, then `name[extras] vX.Y`
/// and any of `(extra: …)`, `(group: …)`, `(latest: v…)` and `(*)`. A root has no prefix. The
/// closing note on `(*)` is not a package. uv has no machine-readable tree, so this reads its
/// drawing, here alone.
pub fn tree(stdout: &str) -> Vec<TreeNode> {
    stdout.lines().filter_map(tree_line).collect()
}

fn tree_line(line: &str) -> Option<TreeNode> {
    let line = line.trim_end();
    let (depth, rest) = match ["├── ", "└── "]
        .iter()
        .filter_map(|connector| line.find(connector).map(|at| (at, connector.len())))
        .min()
    {
        Some((at, len)) => (line[..at].chars().count() / 4 + 1, &line[at + len..]),
        None => (0, line),
    };
    let mut words = rest.split_whitespace();
    let named = words.next()?;
    let version = words.next()?.strip_prefix('v')?;
    if !version.starts_with(|c: char| c.is_ascii_digit()) {
        return None;
    }
    let (name, extras) = match named.split_once('[') {
        Some((name, extras)) => (
            name,
            extras
                .trim_end_matches(']')
                .split(',')
                .map(|e| e.trim().to_string())
                .filter(|e| !e.is_empty())
                .collect(),
        ),
        None => (named, Vec::new()),
    };
    let mut node = TreeNode {
        depth,
        name: name.to_string(),
        version: version.to_string(),
        extras,
        ..TreeNode::default()
    };
    // The rest is notes in parentheses, each of which may hold spaces.
    let rest: Vec<&str> = words.collect();
    for note in rest
        .join(" ")
        .split('(')
        .map(|n| n.trim().trim_end_matches(')'))
    {
        match note {
            "" => {}
            "*" => node.repeated = true,
            note => match note.strip_prefix("latest: v") {
                Some(latest) => node.latest = Some(latest.to_string()),
                None => node.notes.push(note.to_string()),
            },
        }
    }
    Some(node)
}

/// The Pythons of `uv python list --output-format json`.
pub fn python_list(stdout: &str) -> Result<Vec<PythonOption>, UvError> {
    serde_json::from_str(stdout).map_err(|e| UvError::Parse(e.to_string()))
}

use std::fmt;

use crate::events::{EnvDependency, PyprojectDependency};
use color_eyre::Result;

/// Why a `uv` invocation failed.
#[derive(Debug)]
pub enum UvError {
    /// `uv` could not be started at all.
    Spawn(std::io::Error),
    /// `uv` ran and exited unsuccessfully.
    Failed {
        command: String,
        status: std::process::ExitStatus,
        stderr: String,
    },
    /// `uv` succeeded but its output could not be read.
    Parse(String),
}

impl fmt::Display for UvError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UvError::Spawn(e) if e.kind() == std::io::ErrorKind::NotFound => {
                write!(f, "`uv` was not found — is it installed and on PATH?")
            }
            UvError::Spawn(e) => write!(f, "could not run `uv`: {e}"),
            UvError::Failed {
                command,
                status,
                stderr,
            } => write!(f, "`uv {command}` failed ({status}): {stderr}"),
            UvError::Parse(msg) => write!(f, "could not read `uv` output: {msg}"),
        }
    }
}

impl std::error::Error for UvError {}

/// Runs `uv` with `args` and returns its stdout. Every `uv` call goes through here.
async fn run_uv(args: &[&str]) -> Result<String, UvError> {
    let output = tokio::process::Command::new("uv")
        .args(args)
        .output()
        .await
        .map_err(UvError::Spawn)?;
    if !output.status.success() {
        return Err(UvError::Failed {
            command: args.join(" "),
            status: output.status,
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

pub async fn load_pyproject_deps() -> Result<Vec<PyprojectDependency>> {
    let content = tokio::fs::read_to_string("./pyproject.toml").await?;
    let value: toml::Value = toml::from_str(&content)?;

    let mut deps: Vec<PyprojectDependency> = Vec::new();
    let mut index: std::collections::HashMap<String, usize> = std::collections::HashMap::new();

    let mut add = |name: String, group: String, version_constraint: String| {
        if let Some(&i) = index.get(&name) {
            deps[i].groups.push(group);
        } else {
            index.insert(name.clone(), deps.len());
            deps.push(PyprojectDependency {
                name,
                groups: vec![group],
                version_constraint,
            });
        }
    };

    // [project.dependencies] → group "main"
    if let Some(project_deps) = value
        .get("project")
        .and_then(|p| p.get("dependencies"))
        .and_then(|d| d.as_array())
    {
        for dep in project_deps {
            if let Some(s) = dep.as_str() {
                let (name, constraint) = parse_dep_string(s);
                add(name, "main".to_string(), constraint);
            }
        }
    }

    // [dependency-groups.*] → group = key
    if let Some(groups) = value.get("dependency-groups").and_then(|g| g.as_table()) {
        for (group_name, group_deps) in groups {
            if let Some(arr) = group_deps.as_array() {
                for dep in arr {
                    if let Some(s) = dep.as_str() {
                        let (name, constraint) = parse_dep_string(s);
                        add(name, group_name.clone(), constraint);
                    }
                }
            }
        }
    }

    // [project.optional-dependencies.*] → group = key
    if let Some(opt_deps) = value
        .get("project")
        .and_then(|p| p.get("optional-dependencies"))
        .and_then(|o| o.as_table())
    {
        for (group_name, group_deps) in opt_deps {
            if let Some(arr) = group_deps.as_array() {
                for dep in arr {
                    if let Some(s) = dep.as_str() {
                        let (name, constraint) = parse_dep_string(s);
                        add(name, group_name.clone(), constraint);
                    }
                }
            }
        }
    }

    Ok(deps)
}

async fn remove_dep_from_group(name: &str, group: &str) -> Result<(), UvError> {
    if group == "main" {
        return Ok(());
    }
    run_uv(&["remove", name, "--group", group]).await?;
    Ok(())
}

pub async fn remove_dep_from_pyproject(name: &str, groups: Vec<String>) -> Result<(), UvError> {
    for group in groups {
        remove_dep_from_group(name, &group).await?;
    }
    run_uv(&["remove", name]).await?;
    Ok(())
}

pub async fn sync_env() -> Result<(), UvError> {
    run_uv(&["sync"]).await?;
    Ok(())
}

pub async fn get_env_package_info(name: &str) -> Result<EnvDependency, UvError> {
    let stdout = run_uv(&["pip", "show", name]).await?;
    let field = |key: &str| {
        stdout
            .lines()
            .find_map(|l| l.strip_prefix(key))
            .map(str::to_string)
            .ok_or_else(|| UvError::Parse(format!("no {key:?} line for {name}")))
    };
    Ok(EnvDependency {
        name: field("Name: ")?,
        version: field("Version: ")?,
    })
}

pub async fn upgrade_env_dep(name: &str) -> Result<(), UvError> {
    run_uv(&["lock", "--upgrade-package", name]).await?;
    sync_env().await
}

/// Pins a dependency to a version constraint, e.g. "==1.2.3", ">=2.0", "<3.0".
/// A bare version number (no operator) is treated as an exact pin, and an
/// empty constraint falls back to the latest resolvable version.
pub async fn pin_dep_version(name: &str, constraint: &str) -> Result<(), UvError> {
    let constraint = constraint.trim();
    let requirement = if constraint.is_empty() {
        name.to_string()
    } else if constraint.starts_with(['=', '>', '<', '!', '~']) {
        format!("{name}{constraint}")
    } else {
        format!("{name}=={constraint}")
    };
    run_uv(&["add", requirement.as_str()]).await?;
    Ok(())
}

pub async fn load_env_deps() -> Result<Vec<EnvDependency>, UvError> {
    let stdout = run_uv(&["pip", "list", "--format", "json"]).await?;
    serde_json::from_str(&stdout).map_err(|e| UvError::Parse(e.to_string()))
}

/// Splits a PEP 508 dependency string into (name, version_constraint).
/// e.g. "httpx>=0.28.1" → ("httpx", ">=0.28.1")
///      "requests"       → ("requests", "")
pub fn parse_dep_string(s: &str) -> (String, String) {
    let s = s.trim();
    if let Some(pos) = s.find(['>', '<', '=', '!', '~']) {
        let name = s[..pos].trim_end().to_string();
        let constraint = s[pos..].trim_start().to_string();
        (name, constraint)
    } else {
        (s.to_string(), String::new())
    }
}

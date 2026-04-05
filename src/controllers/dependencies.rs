use std::{fmt, panic};

use crate::events::{EnvDependency, PyprojectDependency};
use color_eyre::Result;

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

async fn remove_dep_from_group(name: &str, group: &str) -> Result<(), String> {
    if group == "main" {
        return Ok(());
    }
    let output = tokio::process::Command::new("uv")
        .args(["remove", name, "--group", group])
        .output()
        .await;

    match output {
        Ok(output) => {
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                panic!("uv remove failed: {}", stderr);
            }
            Ok(())
        }
        Err(e) => panic!("uv remove failed: {}", e),
    }
}

pub async fn remove_dep_from_pyproject(name: &str, groups: Vec<String>) -> Result<(), String> {
    for group in groups {
        remove_dep_from_group(name, &group).await?;
    }
    let output = tokio::process::Command::new("uv")
        .args(["remove", name])
        .output()
        .await;

    match output {
        Ok(output) => {
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                panic!("uv remove failed: {}", stderr);
            }
            Ok(())
        }
        Err(e) => panic!("uv remove failed: {}", e),
    }
}

pub async fn sync_env() -> Result<(), String> {
    let output = tokio::process::Command::new("uv")
        .args(["sync"])
        .output()
        .await;

    match output {
        Ok(output) => {
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                panic!("uv sync failed: {}", stderr);
            }
            Ok(())
        }
        Err(e) => panic!("uv sync failed: {}", e),
    }
}

#[derive(Debug)]
pub enum DependencyError {
    NotFound(String),
}

impl fmt::Display for DependencyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DependencyError::NotFound(msg) => write!(f, "Not found: {}", msg),
        }
    }
}
impl std::error::Error for DependencyError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            _ => None,
        }
    }
}

pub async fn get_env_package_info(name: &str) -> Result<EnvDependency, DependencyError> {
    let output = tokio::process::Command::new("uv")
        .args(["pip", "show", name])
        .output()
        .await;

    match output {
        Ok(output) => {
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                panic!("uv pip show failed: {}", stderr);
            }
            let stdout = String::from_utf8_lossy(&output.stdout);
            let lines: Vec<&str> = stdout.lines().collect();
            let ok = lines
                .iter()
                .find(|l| l.starts_with("Name: "))
                .and_then(|l| l.strip_prefix("Name: "));
            let name = match ok {
                Some(name) => name.to_string(),
                None => return Err(DependencyError::NotFound("Name not found".to_string())),
            };
            let ok = lines
                .iter()
                .find(|l| l.starts_with("Version: "))
                .and_then(|l| l.strip_prefix("Version: "));

            let version = match ok {
                Some(version) => version.to_string(),
                None => return Err(DependencyError::NotFound("Version not found".to_string())),
            };

            Ok(EnvDependency { name, version })
        }
        Err(e) => panic!("uv pip show failed: {}", e),
    }
}

pub async fn upgrade_env_dep(name: &str) -> Result<(), String> {
    let output = tokio::process::Command::new("uv")
        .args(["lock", "--upgrade-package", name])
        .output()
        .await;

    match output {
        Ok(output) => {
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                panic!("uv upgrade failed: {}", stderr);
            }
        }
        Err(e) => panic!("uv upgrade failed: {}", e),
    }
    sync_env().await
}

/// Pins a dependency to a version constraint, e.g. "==1.2.3", ">=2.0", "<3.0".
/// A bare version number (no operator) is treated as an exact pin, and an
/// empty constraint falls back to the latest resolvable version.
pub async fn pin_dep_version(name: &str, constraint: &str) -> Result<(), String> {
    let constraint = constraint.trim();
    let requirement = if constraint.is_empty() {
        name.to_string()
    } else if constraint.starts_with(['=', '>', '<', '!', '~']) {
        format!("{name}{constraint}")
    } else {
        format!("{name}=={constraint}")
    };

    let output = tokio::process::Command::new("uv")
        .args(["add", &requirement])
        .output()
        .await;

    match output {
        Ok(output) => {
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                panic!("uv add failed: {}", stderr);
            }
            Ok(())
        }
        Err(e) => panic!("uv add failed: {}", e),
    }
}

pub async fn load_env_deps() -> Result<Vec<EnvDependency>> {
    let output = tokio::process::Command::new("uv")
        .args(["pip", "list", "--format", "json"])
        .output()
        .await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(color_eyre::eyre::eyre!("uv pip list failed: {}", stderr));
    }

    let deps: Vec<EnvDependency> = serde_json::from_slice(&output.stdout)?;
    Ok(deps)
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

use std::fmt;

use crate::events::{EnvDependency, Membership, PyprojectDependency};
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
pub async fn run_uv(args: &[&str]) -> Result<String, UvError> {
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

    let mut add = |name: String, group: Membership, version_constraint: String| {
        if let Some(&i) = index.get(&name) {
            // A package listed twice in one section (e.g. per-Python markers) is still one
            // membership: a second `uv remove` for it would fail after the first.
            if !deps[i].groups.contains(&group) {
                deps[i].groups.push(group);
            }
        } else {
            index.insert(name.clone(), deps.len());
            deps.push(PyprojectDependency {
                name,
                groups: vec![group],
                version_constraint,
            });
        }
    };

    // [project.dependencies]
    if let Some(project_deps) = value
        .get("project")
        .and_then(|p| p.get("dependencies"))
        .and_then(|d| d.as_array())
    {
        for dep in project_deps {
            if let Some(s) = dep.as_str() {
                let (name, constraint) = parse_dep_string(s);
                add(name, Membership::Project, constraint);
            }
        }
    }

    // [dependency-groups.*]
    if let Some(groups) = value.get("dependency-groups").and_then(|g| g.as_table()) {
        for (group_name, group_deps) in groups {
            if let Some(arr) = group_deps.as_array() {
                for dep in arr {
                    if let Some(s) = dep.as_str() {
                        let (name, constraint) = parse_dep_string(s);
                        add(name, Membership::Group(group_name.clone()), constraint);
                    }
                }
            }
        }
    }

    // [project.optional-dependencies.*]
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
                        add(name, Membership::Extra(group_name.clone()), constraint);
                    }
                }
            }
        }
    }

    Ok(deps)
}

/// `uv remove` / `uv add` arguments acting on one membership. `--no-sync`: the caller
/// syncs once, inexactly, afterwards; each implicit sync would be exact and uninstall
/// whatever the lockfile does not list, such as `uv pip install`ed tools.
pub fn membership_args<'a>(
    verb: &'a str,
    target: &'a str,
    membership: &'a Membership,
) -> Vec<&'a str> {
    let mut args = vec![verb, "--no-sync", target];
    match membership {
        Membership::Project => {}
        Membership::Group(name) => args.extend(["--group", name.as_str()]),
        Membership::Extra(name) => args.extend(["--optional", name.as_str()]),
    }
    args
}

/// Brings the environment up to date without uninstalling packages the lockfile does not
/// list. A package just removed from pyproject.toml therefore stays installed; the
/// Environment table can uninstall it.
async fn sync_inexact() -> Result<(), UvError> {
    run_uv(&["sync", "--inexact"]).await?;
    Ok(())
}

pub async fn remove_dep_from_pyproject(
    name: &str,
    memberships: Vec<Membership>,
) -> Result<(), UvError> {
    for membership in &memberships {
        run_uv(&membership_args("remove", name, membership)).await?;
    }
    sync_inexact().await
}

/// The environment `uv pip` must act on: the project's, as `uv sync` resolves it. Left
/// implicit, `uv pip` follows an activated virtualenv or conda env instead.
fn project_env() -> String {
    std::env::var("UV_PROJECT_ENVIRONMENT").unwrap_or_else(|_| ".venv".to_string())
}

/// `uv pip uninstall` arguments that remove `name` from `env` only.
pub fn uninstall_args<'a>(name: &'a str, env: &'a str) -> [&'a str; 5] {
    ["pip", "uninstall", "--python", env, name]
}

/// Uninstalls `name` from the project environment only; pyproject.toml and uv.lock are
/// untouched. `uv pip` rather than `uv run pip`: a uv environment has no pip of its own,
/// so `uv run pip` would run whichever pip is first on PATH.
pub async fn uninstall_from_env(name: &str) -> Result<(), UvError> {
    run_uv(&uninstall_args(name, &project_env())).await?;
    Ok(())
}

/// The installed entry for `name`, or `None` if it is not in the environment.
pub async fn get_env_package_info(name: &str) -> Result<Option<EnvDependency>, UvError> {
    let wanted = normalize(name);
    Ok(load_env_deps()
        .await?
        .into_iter()
        .find(|d| normalize(&d.name) == wanted))
}

/// PEP 503 name normalization, ignoring extras, so `Flask_SQLAlchemy` matches
/// `flask-sqlalchemy` and a declared `flask[dotenv]` matches an installed `flask`.
pub fn normalize(name: &str) -> String {
    let name = name.split('[').next().unwrap_or(name).trim();
    name.split(['-', '_', '.'])
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
        .to_lowercase()
}

pub async fn upgrade_env_dep(name: &str) -> Result<(), UvError> {
    // `--inexact`: a plain sync uninstalls everything that is not in the lockfile.
    run_uv(&["sync", "--inexact", "--upgrade-package", name]).await?;
    Ok(())
}

/// The requirement that pins `name` to a version constraint, e.g. "==1.2.3", ">=2.0",
/// "<3.0". A bare version number (no operator) is treated as an exact pin, and an
/// empty constraint falls back to the latest resolvable version. The declared entry's
/// marker is kept: uv rewrites an entry in place only when the marker matches, and
/// otherwise adds a second, unconditional one.
pub fn pin_requirement(name: &str, constraint: &str, marker: Option<&str>) -> String {
    let constraint = constraint.trim();
    let requirement = if constraint.is_empty() {
        name.to_string()
    } else if constraint.starts_with(['=', '>', '<', '!', '~']) {
        format!("{name}{constraint}")
    } else {
        format!("{name}=={constraint}")
    };
    match marker {
        Some(marker) => format!("{requirement}; {marker}"),
        None => requirement,
    }
}

/// The entry declaring `name` in pyproject.toml, if there is one.
async fn declaration(name: &str) -> Option<PyprojectDependency> {
    let wanted = normalize(name);
    load_pyproject_deps()
        .await
        .ok()?
        .into_iter()
        .find(|d| normalize(&d.name) == wanted)
}

/// Pins `name` in every membership it is declared in and returns the requirement written.
/// An undeclared package is added to `[project.dependencies]`; if pyproject.toml cannot be
/// read, uv reports why.
pub async fn pin_dep_version(name: &str, constraint: &str) -> Result<String, UvError> {
    let declared = declaration(name).await;
    let marker = declared
        .as_ref()
        .and_then(|d| d.version_constraint.split_once(';'))
        .map(|(_, marker)| marker.trim());
    let requirement = pin_requirement(name, constraint, marker);
    let memberships = declared.map_or_else(|| vec![Membership::Project], |d| d.groups);
    for membership in &memberships {
        run_uv(&membership_args("add", &requirement, membership)).await?;
    }
    sync_inexact().await?;
    Ok(requirement)
}

pub async fn load_env_deps() -> Result<Vec<EnvDependency>, UvError> {
    let env = project_env();
    let stdout = run_uv(&["pip", "list", "--format", "json", "--python", env.as_str()]).await?;
    serde_json::from_str(&stdout).map_err(|e| UvError::Parse(e.to_string()))
}

/// Splits a PEP 508 dependency string into (name, version_constraint).
/// e.g. "httpx>=0.28.1"                   → ("httpx", ">=0.28.1")
///      "requests"                        → ("requests", "")
///      "tomli; python_version < '3.11'"  → ("tomli", "; python_version < '3.11'")
/// A marker (`;`) or a direct reference (`@`) ends the name, so operators inside a
/// marker are never mistaken for a version.
pub fn parse_dep_string(s: &str) -> (String, String) {
    let s = s.trim();
    if let Some(pos) = s.find([';', '@', '>', '<', '=', '!', '~']) {
        let name = s[..pos].trim_end().to_string();
        let constraint = s[pos..].trim_start().to_string();
        (name, constraint)
    } else {
        (s.to_string(), String::new())
    }
}

use std::path::Path;

use crate::domain::{EnvDependency, Membership, PyprojectDependency, Source, normalize_name};
use crate::uv::{UvClient, UvError, args, parse};
use color_eyre::Result;

/// The dependencies the `pyproject` file declares.
pub async fn load_pyproject_deps(pyproject: &Path) -> Result<Vec<PyprojectDependency>> {
    parse_pyproject_deps(&tokio::fs::read_to_string(pyproject).await?)
}

/// The dependencies `pyproject` declares, one row per package: the main ones in file order,
/// then the dependency groups and then the extras, each by name. A package in several places is
/// one row with each of them; its first declaration gives the constraint and extras.
pub fn parse_pyproject_deps(pyproject: &str) -> Result<Vec<PyprojectDependency>> {
    let value: toml::Value = toml::from_str(pyproject)?;

    let mut deps: Vec<PyprojectDependency> = Vec::new();
    let mut index: std::collections::HashMap<String, usize> = std::collections::HashMap::new();

    let mut add = |(name, extras, version_constraint): (String, Vec<String>, String),
                   group: Membership| {
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
                extras,
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
                add(parse_dep_string(s), Membership::Project);
            }
        }
    }

    // [dependency-groups.*]
    if let Some(groups) = value.get("dependency-groups").and_then(|g| g.as_table()) {
        for (group_name, group_deps) in groups {
            if let Some(arr) = group_deps.as_array() {
                for dep in arr {
                    if let Some(s) = dep.as_str() {
                        add(parse_dep_string(s), Membership::Group(group_name.clone()));
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
                        add(parse_dep_string(s), Membership::Extra(group_name.clone()));
                    }
                }
            }
        }
    }

    Ok(deps)
}

/// Syncs the environment, uninstalling what the lockfile does not list.
pub async fn sync_exact(uv: &impl UvClient) -> Result<(), UvError> {
    uv.run(&args::SYNC).await?;
    Ok(())
}

/// Adds `requirement` to `membership` with `uv add`, which also installs it.
pub async fn add_dep(
    uv: &impl UvClient,
    requirement: &str,
    membership: &Membership,
) -> Result<(), UvError> {
    uv.run(&args::membership_args("add", requirement, membership))
        .await?;
    Ok(())
}

/// Adds a package from `source` to `membership` with `uv add`.
pub async fn add_from(
    uv: &impl UvClient,
    source: &Source,
    membership: &Membership,
) -> Result<(), UvError> {
    let args = args::add_from_args(source, membership);
    uv.run(&args.iter().map(String::as_str).collect::<Vec<_>>())
        .await?;
    Ok(())
}

/// Removes `name` from every membership in pyproject.toml with `uv remove`, which also syncs
/// exactly: the package leaves the environment too, with whatever else the lockfile no longer
/// lists.
pub async fn remove_dep_from_pyproject(
    uv: &impl UvClient,
    name: &str,
    memberships: Vec<Membership>,
) -> Result<(), UvError> {
    for membership in &memberships {
        uv.run(&args::remove_args(name, membership)).await?;
    }
    uv.run(&args::SYNC).await?;
    Ok(())
}

/// The requirements declaring `name` in `membership`, as written in the pyproject.toml `text`:
/// extras, marker and all, and each of them when the package is listed more than once.
pub fn declarations(text: &str, name: &str, membership: &Membership) -> Vec<String> {
    let Ok(value) = toml::from_str::<toml::Value>(text) else {
        return Vec::new();
    };
    let section = match membership {
        Membership::Project => value.get("project").and_then(|p| p.get("dependencies")),
        Membership::Group(group) => value.get("dependency-groups").and_then(|g| g.get(group)),
        Membership::Extra(extra) => value
            .get("project")
            .and_then(|p| p.get("optional-dependencies"))
            .and_then(|o| o.get(extra)),
    };
    let wanted = normalize(name);
    section
        .and_then(toml::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(toml::Value::as_str)
        .filter(|dep| normalize(&parse_dep_string(dep).0) == wanted)
        .map(String::from)
        .collect()
}

/// Moves `name` from `from` to `to`, as `from` declares it; `requirement`, what its row shows,
/// stands in if the file cannot say. Added first: a failed `uv add` changes nothing, while a
/// failed remove after it leaves the package in both, still declared, which the `Err` of the
/// inner result says.
pub async fn move_dep(
    uv: &impl UvClient,
    pyproject: &Path,
    name: &str,
    requirement: &str,
    from: &Membership,
    to: &Membership,
) -> Result<Result<(), UvError>, UvError> {
    let text = tokio::fs::read_to_string(pyproject)
        .await
        .unwrap_or_default();
    let mut declared = declarations(&text, name, from);
    if declared.is_empty() {
        declared.push(requirement.to_string());
    }
    for requirement in &declared {
        add_dep(uv, requirement, to).await?;
    }
    Ok(remove_dep_from_pyproject(uv, name, vec![from.clone()]).await)
}

/// Uninstalls `name` from the project environment only; pyproject.toml and uv.lock are
/// untouched. `uv pip` rather than `uv run pip`: a uv environment has no pip of its own,
/// so `uv run pip` would run whichever pip is first on PATH.
pub async fn uninstall_from_env(uv: &impl UvClient, name: &str, env: &Path) -> Result<(), UvError> {
    uv.run(&args::uninstall_args(name, &env.to_string_lossy()))
        .await?;
    Ok(())
}

/// PEP 503 name normalization, ignoring extras, so `Flask_SQLAlchemy` matches
/// `flask-sqlalchemy` and a declared `flask[dotenv]` matches an installed `flask`.
pub fn normalize(name: &str) -> String {
    normalize_name(name.split('[').next().unwrap_or(name).trim())
}

pub async fn upgrade_env_dep(uv: &impl UvClient, name: &str) -> Result<(), UvError> {
    uv.run(&args::upgrade_args(name)).await?;
    Ok(())
}

/// The entry declaring `name` in pyproject.toml, if there is one.
async fn declaration(pyproject: &Path, name: &str) -> Option<PyprojectDependency> {
    let wanted = normalize(name);
    load_pyproject_deps(pyproject)
        .await
        .ok()?
        .into_iter()
        .find(|d| normalize(&d.name) == wanted)
}

/// Pins `name` in every membership it is declared in and returns the requirement written.
/// An undeclared package is added to `[project.dependencies]`; if pyproject.toml cannot be
/// read, uv reports why.
pub async fn pin_dep_version(
    uv: &impl UvClient,
    pyproject: &Path,
    name: &str,
    constraint: &str,
) -> Result<String, UvError> {
    let declared = declaration(pyproject, name).await;
    let marker = declared
        .as_ref()
        .and_then(|d| d.version_constraint.split_once(';'))
        .map(|(_, marker)| marker.trim());
    let extras = declared.as_ref().map_or(&[][..], |d| &d.extras);
    let requirement = args::pin_requirement(name, extras, constraint, marker);
    let memberships = declared.map_or_else(|| vec![Membership::Project], |d| d.groups);
    for membership in &memberships {
        uv.run(&args::membership_args("add", &requirement, membership))
            .await?;
    }
    Ok(requirement)
}

/// The packages installed in `env`. A project never synced has no environment yet: that is
/// nothing installed, not an error, and uv's advice to run `uv venv` would be the wrong step.
pub async fn load_env_deps(uv: &impl UvClient, env: &Path) -> Result<Vec<EnvDependency>, UvError> {
    if !env.exists() {
        return Ok(Vec::new());
    }
    parse::pip_list(&uv.run(&args::pip_list_args(&env.to_string_lossy())).await?)
}

/// Splits a PEP 508 dependency string into (name, extras, version_constraint).
/// e.g. "httpx>=0.28.1"                   → ("httpx", [], ">=0.28.1")
///      "flask[dotenv, async]>=3.0"       → ("flask", ["dotenv", "async"], ">=3.0")
///      "tomli; python_version < '3.11'"  → ("tomli", [], "; python_version < '3.11'")
/// Extras (`[`), a marker (`;`) or a direct reference (`@`) end the name, so operators
/// inside a marker are never mistaken for a version.
pub fn parse_dep_string(s: &str) -> (String, Vec<String>, String) {
    let s = s.trim();
    let end = s
        .find(['[', ';', '@', '>', '<', '=', '!', '~'])
        .unwrap_or(s.len());
    let name = s[..end].trim_end().to_string();
    let (extras, rest) = match s[end..].strip_prefix('[') {
        Some(inner) => {
            let (list, rest) = inner.split_once(']').unwrap_or((inner, ""));
            let extras = list
                .split(',')
                .map(str::trim)
                .filter(|e| !e.is_empty())
                .map(String::from)
                .collect();
            (extras, rest)
        }
        None => (Vec::new(), &s[end..]),
    };
    (name, extras, rest.trim_start().to_string())
}

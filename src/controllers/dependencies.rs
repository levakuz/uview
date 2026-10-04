use crate::domain::{EnvDependency, Membership, PyprojectDependency};
use crate::uv::{UvClient, UvError, args, parse};
use color_eyre::Result;

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

async fn sync_inexact(uv: &impl UvClient) -> Result<(), UvError> {
    uv.run(&args::SYNC_INEXACT).await?;
    Ok(())
}

/// Adds `requirement` to `[project.dependencies]`, then syncs once, inexactly.
pub async fn add_dep(uv: &impl UvClient, requirement: &str) -> Result<(), UvError> {
    uv.run(&args::membership_args(
        "add",
        requirement,
        &Membership::Project,
    ))
    .await?;
    sync_inexact(uv).await
}

pub async fn remove_dep_from_pyproject(
    uv: &impl UvClient,
    name: &str,
    memberships: Vec<Membership>,
) -> Result<(), UvError> {
    for membership in &memberships {
        uv.run(&args::membership_args("remove", name, membership))
            .await?;
    }
    sync_inexact(uv).await
}

/// The environment `uv pip` must act on: the project's, as `uv sync` resolves it. Left
/// implicit, `uv pip` follows an activated virtualenv or conda env instead.
fn project_env() -> String {
    std::env::var("UV_PROJECT_ENVIRONMENT")
        .ok()
        // Empty means unset to `uv sync`, but `--python ""` would mean no `--python` at all.
        .filter(|env| !env.is_empty())
        .unwrap_or_else(|| ".venv".to_string())
}

/// Uninstalls `name` from the project environment only; pyproject.toml and uv.lock are
/// untouched. `uv pip` rather than `uv run pip`: a uv environment has no pip of its own,
/// so `uv run pip` would run whichever pip is first on PATH.
pub async fn uninstall_from_env(uv: &impl UvClient, name: &str) -> Result<(), UvError> {
    uv.run(&args::uninstall_args(name, &project_env())).await?;
    Ok(())
}

/// The installed entry for `name`, or `None` if it is not in the environment.
pub async fn get_env_package_info(
    uv: &impl UvClient,
    name: &str,
) -> Result<Option<EnvDependency>, UvError> {
    let wanted = normalize(name);
    Ok(load_env_deps(uv)
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

pub async fn upgrade_env_dep(uv: &impl UvClient, name: &str) -> Result<(), UvError> {
    uv.run(&args::upgrade_args(name)).await?;
    Ok(())
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
pub async fn pin_dep_version(
    uv: &impl UvClient,
    name: &str,
    constraint: &str,
) -> Result<String, UvError> {
    let declared = declaration(name).await;
    let marker = declared
        .as_ref()
        .and_then(|d| d.version_constraint.split_once(';'))
        .map(|(_, marker)| marker.trim());
    let requirement = args::pin_requirement(name, constraint, marker);
    let memberships = declared.map_or_else(|| vec![Membership::Project], |d| d.groups);
    for membership in &memberships {
        uv.run(&args::membership_args("add", &requirement, membership))
            .await?;
    }
    sync_inexact(uv).await?;
    Ok(requirement)
}

pub async fn load_env_deps(uv: &impl UvClient) -> Result<Vec<EnvDependency>, UvError> {
    parse::pip_list(&uv.run(&args::pip_list_args(&project_env())).await?)
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

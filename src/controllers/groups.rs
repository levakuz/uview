//! Dependency groups themselves, which uv has no command for: deleting one left empty, as
//! `uv remove` leaves `dev = []` behind (ADR-0009), and creating an empty one (ADR-0013).

use std::path::Path;

use toml_edit::{DocumentMut, Item, Value};

use crate::controllers::pyproject::edit_then_lock;
use crate::domain::{DeclaredGroup, group_name_problem};
use crate::uv::{UvClient, UvError};

/// `pyproject` without the group `name`, if the group is empty and nothing else names it:
/// `tool.uv.default-groups`, or another group's `{ include-group = "name" }`. uv refuses to sync
/// with a default group that is not defined.
pub fn without_empty_group(pyproject: &str, name: &str) -> Option<String> {
    let mut doc: DocumentMut = pyproject.parse().ok()?;
    let groups = doc.get("dependency-groups")?.as_table_like()?;
    if !groups.get(name)?.as_array()?.is_empty() || named_by(&doc, name).is_some() {
        return None;
    }
    let groups = doc.get_mut("dependency-groups")?.as_table_like_mut()?;
    groups.remove(name);
    // An emptied table would leave a bare `[dependency-groups]` header.
    if groups.is_empty() {
        doc.remove("dependency-groups");
    }
    Some(doc.to_string())
}

/// What names the group `name` besides its own table, if anything: `tool.uv.default-groups`,
/// or another group's `{ include-group = "name" }`.
fn named_by(doc: &DocumentMut, name: &str) -> Option<String> {
    let default = doc
        .get("tool")
        .and_then(|t| t.get("uv"))
        .and_then(|uv| uv.get("default-groups"))
        .and_then(Item::as_array)
        .is_some_and(|defaults| defaults.iter().any(|g| g.as_str() == Some(name)));
    if default {
        return Some("named in tool.uv.default-groups".into());
    }
    let groups = doc.get("dependency-groups")?.as_table_like()?;
    groups
        .iter()
        .find(|(_, group)| {
            group.as_array().is_some_and(|deps| {
                deps.iter().any(|dep| {
                    dep.as_inline_table()
                        .and_then(|t| t.get("include-group"))
                        .and_then(Value::as_str)
                        == Some(name)
                })
            })
        })
        .map(|(other, _)| format!("included by group {other}"))
}

/// The groups `pyproject` declares, in file order, each with what names it elsewhere.
pub fn declared_groups(pyproject: &str) -> Vec<DeclaredGroup> {
    let Ok(doc) = pyproject.parse::<DocumentMut>() else {
        return Vec::new();
    };
    let Some(groups) = doc.get("dependency-groups").and_then(Item::as_table_like) else {
        return Vec::new();
    };
    groups
        .iter()
        .map(|(name, _)| DeclaredGroup {
            name: name.to_string(),
            named_by: named_by(&doc, name),
        })
        .collect()
}

/// `pyproject` with the empty group `name` added to `[dependency-groups]`, made if missing,
/// unless uv would refuse the name or a group has it already.
pub fn with_group(pyproject: &str, name: &str) -> Result<String, String> {
    if let Some(problem) = group_name_problem(name, &declared_groups(pyproject)) {
        return Err(problem.to_string());
    }
    let mut doc: DocumentMut = pyproject.parse().map_err(|e| format!("{e}"))?;
    let groups = doc
        .entry("dependency-groups")
        .or_insert(Item::Table(toml_edit::Table::new()))
        .as_table_like_mut()
        .ok_or("dependency-groups is not a table")?;
    groups.insert(name, toml_edit::value(toml_edit::Array::new()));
    Ok(doc.to_string())
}

/// Adds the empty group `name` to the `pyproject` file, then relocks (ADR-0013).
pub async fn create_group(uv: &impl UvClient, pyproject: &Path, name: &str) -> Result<(), UvError> {
    edit_then_lock(uv, pyproject, |before| with_group(before, name)).await
}

/// The groups the `pyproject` file declares, in file order.
pub async fn load_groups(pyproject: &Path) -> Vec<DeclaredGroup> {
    tokio::fs::read_to_string(pyproject)
        .await
        .map(|text| declared_groups(&text))
        .unwrap_or_default()
}

/// The groups `pyproject` declares with no packages. They have no row in the pyproject table.
pub fn declared_empty(pyproject: &str) -> Vec<String> {
    let Ok(doc) = pyproject.parse::<DocumentMut>() else {
        return Vec::new();
    };
    let Some(groups) = doc.get("dependency-groups").and_then(Item::as_table_like) else {
        return Vec::new();
    };
    groups
        .iter()
        .filter(|(_, deps)| deps.as_array().is_some_and(|deps| deps.is_empty()))
        .map(|(name, _)| name.to_string())
        .collect()
}

/// The groups the `pyproject` file declares empty, and those of them that could be deleted.
pub async fn empty_groups(pyproject: &Path) -> (Vec<String>, Vec<String>) {
    let Ok(pyproject) = tokio::fs::read_to_string(pyproject).await else {
        return (Vec::new(), Vec::new());
    };
    let empty = declared_empty(&pyproject);
    let deletable = empty
        .iter()
        .filter(|name| without_empty_group(&pyproject, name).is_some())
        .cloned()
        .collect();
    (empty, deletable)
}

/// Deletes the empty group `name` from the `pyproject` file, then relocks, since uv.lock lists
/// the project's groups. If uv cannot lock, the file is put back as it was.
pub async fn delete_group(uv: &impl UvClient, pyproject: &Path, name: &str) -> Result<(), UvError> {
    edit_then_lock(uv, pyproject, |before| {
        without_empty_group(before, name)
            .ok_or_else(|| format!("group `{name}` is no longer empty, or something else names it"))
    })
    .await
}

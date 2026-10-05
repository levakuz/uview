//! Deleting a dependency group that has been left empty. uv has no command for it: `uv remove`
//! leaves `dev = []` behind, so this is the one edit uview makes to pyproject.toml itself
//! (ADR-0009).

use std::path::Path;

use toml_edit::{DocumentMut, Item, Value};

use crate::uv::{UvClient, UvError, args};

pub const PYPROJECT: &str = "./pyproject.toml";

/// `pyproject` without the group `name`, if the group is empty and nothing else names it:
/// `tool.uv.default-groups`, or another group's `{ include-group = "name" }`. uv refuses to sync
/// with a default group that is not defined.
pub fn without_empty_group(pyproject: &str, name: &str) -> Option<String> {
    let mut doc: DocumentMut = pyproject.parse().ok()?;
    let groups = doc.get("dependency-groups")?.as_table_like()?;
    if !groups.get(name)?.as_array()?.is_empty() {
        return None;
    }
    let included = groups.iter().any(|(_, group)| {
        group.as_array().is_some_and(|deps| {
            deps.iter().any(|dep| {
                dep.as_inline_table()
                    .and_then(|t| t.get("include-group"))
                    .and_then(Value::as_str)
                    == Some(name)
            })
        })
    });
    let default = doc
        .get("tool")
        .and_then(|t| t.get("uv"))
        .and_then(|uv| uv.get("default-groups"))
        .and_then(Item::as_array)
        .is_some_and(|defaults| defaults.iter().any(|g| g.as_str() == Some(name)));
    if included || default {
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

/// The groups pyproject.toml declares empty, and those of them that could be deleted.
pub async fn empty_groups() -> (Vec<String>, Vec<String>) {
    let Ok(pyproject) = tokio::fs::read_to_string(PYPROJECT).await else {
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
    let io = |e: std::io::Error| UvError::Pyproject(e.to_string());
    let before = tokio::fs::read_to_string(pyproject).await.map_err(io)?;
    let Some(after) = without_empty_group(&before, name) else {
        return Err(UvError::Pyproject(format!(
            "group `{name}` is no longer empty, or something else names it"
        )));
    };
    tokio::fs::write(pyproject, after).await.map_err(io)?;
    if let Err(e) = uv.run(&args::LOCK).await {
        tokio::fs::write(pyproject, before).await.map_err(io)?;
        return Err(e);
    }
    Ok(())
}

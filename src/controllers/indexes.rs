//! The project's package indexes, `[[tool.uv.index]]`. uv has no command to add or remove one
//! without adding a package, so uview edits the table itself (ADR-0010).

use std::path::Path;

use toml_edit::{ArrayOfTables, DocumentMut, Item, Table, Value};

use crate::controllers::pyproject::edit_then_lock;
use crate::domain::{Index, IndexKind, index_problem};
use crate::uv::{UvClient, UvError};

/// The `[[tool.uv.index]]` entries of `pyproject`, in file order.
pub fn list_indexes(pyproject: &str) -> Vec<Index> {
    let Ok(doc) = pyproject.parse::<DocumentMut>() else {
        return Vec::new();
    };
    let Some(indexes) = index_tables(&doc) else {
        return Vec::new();
    };
    indexes
        .iter()
        .filter_map(|table| {
            let flag = |key| table.get(key).and_then(Item::as_bool) == Some(true);
            Some(Index {
                name: table.get("name").and_then(Item::as_str).map(String::from),
                url: table.get("url").and_then(Item::as_str)?.to_string(),
                kind: if flag("default") {
                    IndexKind::Default
                } else if flag("explicit") {
                    IndexKind::Explicit
                } else {
                    IndexKind::Extra
                },
            })
        })
        .collect()
}

fn index_tables(doc: &DocumentMut) -> Option<&ArrayOfTables> {
    doc.get("tool")?
        .get("uv")?
        .get("index")?
        .as_array_of_tables()
}

/// `pyproject` with an index named `name` at `url` appended. A default index takes over from
/// the one before it: uv accepts two, and which it then uses is not said.
pub fn with_index(pyproject: &str, name: &str, url: &str, default: bool) -> Result<String, String> {
    if let Some(problem) = index_problem(name, url, &list_indexes(pyproject)) {
        return Err(problem.to_string());
    }
    let mut doc: DocumentMut = pyproject.parse().map_err(|e| format!("{e}"))?;
    let uv = implicit_table(implicit_table(doc.as_table_mut(), "tool")?, "uv")?;
    let indexes = uv
        .entry("index")
        .or_insert(Item::ArrayOfTables(ArrayOfTables::new()))
        .as_array_of_tables_mut()
        .ok_or("tool.uv.index is not a list of tables")?;
    if default {
        for index in indexes.iter_mut() {
            index.remove("default");
        }
    }
    let mut index = Table::new();
    index.insert("name", toml_edit::value(name));
    index.insert("url", toml_edit::value(url));
    if default {
        index.insert("default", toml_edit::value(true));
    }
    indexes.push(index);
    Ok(doc.to_string())
}

/// The table `key` of `table`, made if missing without a header of its own.
fn implicit_table<'a>(table: &'a mut Table, key: &str) -> Result<&'a mut Table, String> {
    let item = table.entry(key).or_insert_with(|| {
        let mut made = Table::new();
        made.set_implicit(true);
        Item::Table(made)
    });
    item.as_table_mut()
        .ok_or_else(|| format!("{key} is not a table"))
}

/// `pyproject` without `index`. One that `[tool.uv.sources]` pins packages to stays: uv could
/// not resolve them.
pub fn without_index(pyproject: &str, index: &Index) -> Result<String, String> {
    let mut doc: DocumentMut = pyproject.parse().map_err(|e| format!("{e}"))?;
    if let Some(name) = &index.name {
        let pinned = pinned_to(&doc, name);
        if !pinned.is_empty() {
            return Err(format!(
                "{} {} pinned to {name}",
                pinned.join(", "),
                if pinned.len() == 1 { "is" } else { "are" }
            ));
        }
    }
    let indexes = doc
        .get_mut("tool")
        .and_then(|t| t.get_mut("uv"))
        .and_then(|uv| uv.get_mut("index"))
        .and_then(Item::as_array_of_tables_mut)
        .ok_or("the project has no indexes")?;
    let position = indexes
        .iter()
        .position(|t| {
            t.get("name").and_then(Item::as_str) == index.name.as_deref()
                && t.get("url").and_then(Item::as_str) == Some(index.url.as_str())
        })
        .ok_or("the index is no longer there")?;
    indexes.remove(position);
    Ok(doc.to_string())
}

/// The packages `[tool.uv.sources]` pins to the index `name`, as a table or in a list of them.
fn pinned_to(doc: &DocumentMut, name: &str) -> Vec<String> {
    let Some(sources) = doc
        .get("tool")
        .and_then(|t| t.get("uv"))
        .and_then(|uv| uv.get("sources"))
        .and_then(Item::as_table_like)
    else {
        return Vec::new();
    };
    let names_it = |source: &dyn toml_edit::TableLike| {
        source.get("index").and_then(Item::as_str) == Some(name)
    };
    sources
        .iter()
        .filter(|(_, source)| match source {
            Item::Value(Value::Array(list)) => list
                .iter()
                .filter_map(Value::as_inline_table)
                .any(|t| t.get("index").and_then(Value::as_str) == Some(name)),
            other => other.as_table_like().is_some_and(names_it),
        })
        .map(|(package, _)| package.to_string())
        .collect()
}

/// The indexes the `pyproject` file declares now.
pub async fn load_indexes(pyproject: &Path) -> Vec<Index> {
    tokio::fs::read_to_string(pyproject)
        .await
        .map(|pyproject| list_indexes(&pyproject))
        .unwrap_or_default()
}

/// Adds an index to the `pyproject` file, then relocks.
pub async fn add_index(
    uv: &impl UvClient,
    pyproject: &Path,
    name: &str,
    url: &str,
    default: bool,
) -> Result<(), UvError> {
    edit_then_lock(uv, pyproject, |before| {
        with_index(before, name, url, default)
    })
    .await
}

/// Removes `index` from the `pyproject` file, then relocks.
pub async fn remove_index(
    uv: &impl UvClient,
    pyproject: &Path,
    index: &Index,
) -> Result<(), UvError> {
    edit_then_lock(uv, pyproject, |before| without_index(before, index)).await
}

//! The versions a package can be pinned to, read from the index uv would take it from. uv has
//! no command to list them (ADR-0012).

use std::collections::BTreeMap;
use std::path::Path;
use std::str::FromStr;

use pep440_rs::{Operator, Version, VersionSpecifiers};

use crate::controllers::indexes::{list_indexes, pinned_index};
use crate::domain::{IndexKind, VersionChoice};
use crate::index::parse::{File, files};
use crate::index::{IndexClient, IndexError};

/// PyPI's Simple API: the default index when the project names none.
pub const PYPI: &str = "https://pypi.org/simple";

/// The indexes to ask for `package`, in the order uv would take it from them: the index
/// `[tool.uv.sources]` pins it to; else the project's indexes that are not explicit, in order,
/// then the default, which is PyPI unless an index replaces it. uv takes the first that has it.
// ponytail: indexes from uv.toml and UV_INDEX / UV_DEFAULT_INDEX are not read; read them when a
// project's indexes live there.
pub fn indexes_for(pyproject: &str, package: &str) -> Vec<String> {
    let indexes = list_indexes(pyproject);
    if let Some(pinned) = pinned_index(pyproject, package) {
        return indexes
            .into_iter()
            .filter(|i| i.name.as_deref() == Some(pinned.as_str()))
            .map(|i| i.url)
            .collect();
    }
    let default = indexes
        .iter()
        .find(|i| i.kind == IndexKind::Default)
        .map_or(PYPI.to_string(), |i| i.url.clone());
    indexes
        .into_iter()
        .filter(|i| i.kind == IndexKind::Extra)
        .map(|i| i.url)
        .chain([default])
        .collect()
}

/// The versions of `package` to choose from, newest first, from the first index in uv's order
/// that has it. `pyproject` is the project's file, read for its indexes and its
/// `requires-python`.
pub async fn list_versions(
    index: &impl IndexClient,
    pyproject: &Path,
    package: &str,
) -> Result<Vec<VersionChoice>, String> {
    let text = tokio::fs::read_to_string(pyproject)
        .await
        .map_err(|e| format!("pyproject.toml: {e}"))?;
    let asked = indexes_for(&text, package);
    for url in &asked {
        match index.project_page(url, package).await {
            Ok(page) => {
                let files = files(&page, package).map_err(|e| format!("{url}: {e}"))?;
                return Ok(choices(files, requires_python(&text).as_deref()));
            }
            Err(IndexError::NotFound) => continue,
            Err(IndexError::Status(code)) if ignores(&text, url, code) => continue,
            Err(e) => return Err(format!("{url}: {e}")),
        }
    }
    Err(format!("{package} is not on {}", asked.join(", ")))
}

/// Whether uv would go on past the index at `url` when it answers `code`, as for a package it
/// lacks: the codes its `ignore-error-codes` lists, or 403 from the PyTorch index, which answers
/// that for a package it does not have.
pub fn ignores(pyproject: &str, url: &str, code: u16) -> bool {
    let listed = pyproject.parse::<toml::Table>().ok().and_then(|doc| {
        doc.get("tool")?
            .get("uv")?
            .get("index")?
            .as_array()?
            .iter()
            .find(|index| index.get("url").and_then(toml::Value::as_str) == Some(url))?
            .get("ignore-error-codes")?
            .as_array()
            .cloned()
    });
    match listed {
        Some(codes) => codes
            .iter()
            .any(|c| c.as_integer() == Some(i64::from(code))),
        None => code == 403 && url.contains("download.pytorch.org"),
    }
}

/// The project's `requires-python`, if it declares one.
fn requires_python(pyproject: &str) -> Option<String> {
    let doc: toml::Table = pyproject.parse().ok()?;
    doc.get("project")?
        .get("requires-python")?
        .as_str()
        .map(String::from)
}

/// The releases in `files`, newest first: one per version, without those whose every file is
/// yanked, each marked a pre-release or as needing a newer Python than `project_python`, the
/// project's `requires-python`, allows. A version that is not PEP 440 is left out.
pub fn choices(files: Vec<File>, project_python: Option<&str>) -> Vec<VersionChoice> {
    let oldest_python = project_python.and_then(lowest_python);
    let mut releases: BTreeMap<Version, Vec<File>> = BTreeMap::new();
    for file in files.into_iter().filter(|f| !f.yanked) {
        if let Ok(version) = Version::from_str(&file.version) {
            releases.entry(version).or_default().push(file);
        }
    }
    releases
        .into_iter()
        .rev()
        .map(|(version, files)| {
            let fits = |file: &File| match (&oldest_python, &file.requires_python) {
                (Some(python), Some(needs)) => {
                    VersionSpecifiers::from_str(needs).map_or(true, |needs| needs.contains(python))
                }
                _ => true,
            };
            let needs_python = if files.iter().any(fits) {
                None
            } else {
                files.iter().find_map(|f| f.requires_python.clone())
            };
            VersionChoice {
                version: version.to_string(),
                pre: version.any_prerelease(),
                needs_python,
            }
        })
        .collect()
}

/// The oldest Python the specifier `requires_python` admits, from its lower bounds: `>=3.11`
/// gives 3.11. uv resolves for that oldest one, so a release must support it.
fn lowest_python(requires_python: &str) -> Option<Version> {
    VersionSpecifiers::from_str(requires_python)
        .ok()?
        .iter()
        .filter(|s| {
            matches!(
                s.operator(),
                Operator::GreaterThanEqual
                    | Operator::GreaterThan
                    | Operator::Equal
                    | Operator::TildeEqual
            )
        })
        .map(|s| s.version().clone())
        .max()
}

//! The versions a package can be pinned to, read from the index uv would take it from. uv has
//! no command to list them (ADR-0012).

use std::collections::BTreeMap;
use std::path::Path;
use std::str::FromStr;

use pep440_rs::{Operator, Version, VersionSpecifiers};

use crate::controllers::uv_settings::{PYPI, UvSettings, ignores, indexes_for};
use crate::domain::{VersionChoice, VersionList, redacted};
use crate::index::parse::{File, files};
use crate::index::{IndexClient, IndexError};

/// The versions of `package` to choose from, newest first, from the first index in uv's order
/// that has it, and PyPI's description of it when PyPI is that index; it is read alongside, and
/// a failure to read it leaves the versions be. `settings` are where uv takes its indexes from;
/// `pyproject` is the project's file, read for its sources and its `requires-python`. An index
/// is named without the credentials its URL may carry.
pub async fn list_versions(
    index: &impl IndexClient,
    settings: &UvSettings,
    pyproject: &Path,
    package: &str,
) -> Result<VersionList, String> {
    let text = tokio::fs::read_to_string(pyproject)
        .await
        .map_err(|e| format!("pyproject.toml: {e}"))?;
    let asked = indexes_for(settings, &text, package);
    for url in &asked {
        let (page, info) = if url == PYPI {
            let (page, info) = tokio::join!(
                index.project_page(url, package),
                index.pypi_project(package)
            );
            (page, info.ok())
        } else {
            (index.project_page(url, package).await, None)
        };
        match page {
            Ok(page) => {
                let files = files(&page, package).map_err(|e| format!("{}: {e}", redacted(url)))?;
                return Ok(VersionList {
                    versions: choices(files, requires_python(&text).as_deref()),
                    index: redacted(url),
                    info,
                });
            }
            Err(IndexError::NotFound) => continue,
            Err(IndexError::Status(code)) if ignores(settings, url, code) => continue,
            Err(e) => return Err(format!("{}: {e}", redacted(url))),
        }
    }
    let asked: Vec<String> = asked.iter().map(|url| redacted(url)).collect();
    Err(format!("{package} is not on {}", asked.join(", ")))
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

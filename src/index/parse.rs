//! What an index's project page lists, read from JSON (PEP 691) or HTML (PEP 503). Pure.

use serde::Deserialize;

use crate::domain::{PackageInfo, normalize_name};
use crate::index::{IndexError, Page};

/// One file of a release, as the page lists it.
#[derive(Clone, Debug, PartialEq)]
pub struct File {
    pub version: String,
    pub yanked: bool,
    /// The Python versions the file supports, as a PEP 440 specifier, if the page says.
    pub requires_python: Option<String>,
}

#[derive(Deserialize)]
struct JsonPage {
    files: Vec<JsonFile>,
}

#[derive(Deserialize)]
struct JsonFile {
    filename: String,
    /// `false`, or the reason it was yanked; PEP 691 allows `true` too.
    #[serde(default)]
    yanked: serde_json::Value,
    #[serde(rename = "requires-python")]
    requires_python: Option<String>,
}

/// The files of the project `name` that `page` lists, each with the version its file name
/// carries. A file whose name carries none uview can read, such as an egg, is left out.
pub fn files(page: &Page, name: &str) -> Result<Vec<File>, IndexError> {
    let name = normalize_name(name);
    if page.json {
        let page: JsonPage =
            serde_json::from_str(&page.body).map_err(|e| IndexError::Parse(e.to_string()))?;
        Ok(page
            .files
            .into_iter()
            .filter_map(|file| {
                Some(File {
                    version: version_in(&file.filename, &name)?,
                    yanked: !matches!(
                        file.yanked,
                        serde_json::Value::Bool(false) | serde_json::Value::Null
                    ),
                    requires_python: file.requires_python.filter(|r| !r.is_empty()),
                })
            })
            .collect())
    } else {
        Ok(anchors(&page.body)
            .filter_map(|anchor| {
                Some(File {
                    version: version_in(&anchor.text, &name)?,
                    yanked: anchor.attribute("data-yanked").is_some(),
                    requires_python: anchor
                        .attribute("data-requires-python")
                        .map(|r| unescape(&r))
                        .filter(|r| !r.is_empty()),
                })
            })
            .collect())
    }
}

/// The version in the file name `file` of the project `name`, already normalized: a wheel's
/// second field, or what follows the name in a source archive.
pub fn version_in(file: &str, name: &str) -> Option<String> {
    if let Some(stem) = file.strip_suffix(".whl") {
        return stem.split('-').nth(1).map(String::from);
    }
    let stem = [".tar.gz", ".zip", ".tar.bz2", ".tgz", ".tar.xz"]
        .iter()
        .find_map(|ext| file.strip_suffix(ext))?;
    // The name may hold dashes itself, so each dash is tried as the one before the version.
    stem.match_indices('-')
        .map(|(i, _)| i)
        .find(|&i| normalize_name(&stem[..i]) == name)
        .map(|i| stem[i + 1..].to_string())
        .filter(|version| !version.is_empty())
}

/// A link in an HTML page: its attributes as written, and its text.
struct Anchor<'a> {
    attributes: &'a str,
    text: String,
}

impl Anchor<'_> {
    /// The value of the attribute `name`, empty if it has none; `None` if it is absent.
    fn attribute(&self, name: &str) -> Option<String> {
        let at = self
            .attributes
            .match_indices(name)
            .map(|(i, _)| i)
            .find(|&i| {
                // A whole attribute name, not the end of a longer one.
                self.attributes[..i].ends_with(char::is_whitespace)
                    && !self.attributes[i + name.len()..]
                        .starts_with(|c: char| c.is_alphanumeric() || c == '-')
            })?;
        let rest = self.attributes[at + name.len()..].trim_start();
        let Some(value) = rest.strip_prefix('=') else {
            return Some(String::new());
        };
        let value = value.trim_start();
        let quote = value.chars().next().filter(|c| *c == '"' || *c == '\'');
        Some(match quote {
            Some(q) => value[1..].split(q).next().unwrap_or_default().to_string(),
            None => value
                .split_whitespace()
                .next()
                .unwrap_or_default()
                .to_string(),
        })
    }
}

/// The links of `html`, in order. PEP 503 pages are flat lists of `<a …>file</a>`, so this reads
/// them as text rather than parse HTML.
fn anchors(html: &str) -> impl Iterator<Item = Anchor<'_>> {
    html.split("<a ").skip(1).filter_map(|rest| {
        let (attributes, after) = rest.split_once('>')?;
        let (text, _) = after.split_once("</a>")?;
        Some(Anchor {
            attributes,
            text: unescape(text.trim()),
        })
    })
}

/// `text` with the HTML entities a Simple API page uses decoded.
fn unescape(text: &str) -> String {
    text.replace("&gt;", ">")
        .replace("&lt;", "<")
        .replace("&quot;", "\"")
        .replace("&#x27;", "'")
        .replace("&#39;", "'")
        .replace("&amp;", "&")
}

#[derive(Deserialize)]
struct PypiJson {
    info: PypiInfo,
}

#[derive(Deserialize)]
struct PypiInfo {
    summary: Option<String>,
    requires_python: Option<String>,
    home_page: Option<String>,
    project_urls: Option<std::collections::HashMap<String, String>>,
}

/// What PyPI's JSON API says of a project, from its answer `body`. Empty strings, which PyPI
/// sends for what a project left out, count as nothing.
pub fn pypi_project(body: &str) -> Result<PackageInfo, IndexError> {
    let json: PypiJson =
        serde_json::from_str(body).map_err(|e| IndexError::Parse(e.to_string()))?;
    let given = |value: Option<String>| {
        value
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
    };
    let urls = json.info.project_urls.unwrap_or_default();
    let homepage = [
        "Homepage",
        "homepage",
        "Home",
        "Documentation",
        "Source",
        "Repository",
    ]
    .iter()
    .find_map(|key| given(urls.get(*key).cloned()))
    .or_else(|| given(json.info.home_page));
    Ok(PackageInfo {
        summary: given(json.info.summary),
        requires_python: given(json.info.requires_python),
        homepage,
    })
}

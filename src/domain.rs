use std::fmt;

use serde::Deserialize;

/// Where a dependency is declared in pyproject.toml.
#[derive(Clone, Debug, PartialEq)]
pub enum Membership {
    /// `[project.dependencies]`
    Project,
    /// `[dependency-groups.<name>]`
    Group(String),
    /// `[project.optional-dependencies.<name>]`
    Extra(String),
}

impl fmt::Display for Membership {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Membership::Project => write!(f, "main"),
            Membership::Group(name) => write!(f, "{name}"),
            Membership::Extra(name) => write!(f, "extra:{name}"),
        }
    }
}

#[derive(Clone, Debug)]
pub struct PyprojectDependency {
    pub name: String,
    pub groups: Vec<Membership>,
    /// The extras the declaration asks for, as in `flask[dotenv]`.
    pub extras: Vec<String>,
    pub version_constraint: String,
}

impl PyprojectDependency {
    /// The requirement as declared: name, extras, then version, marker or URL, as in
    /// `flask[dotenv]>=3.0; python_version >= '3.9'`.
    pub fn requirement(&self) -> String {
        let mut requirement = self.name.clone();
        if !self.extras.is_empty() {
            requirement += &format!("[{}]", self.extras.join(","));
        }
        // A URL needs the space: `pkg @ https://…`.
        if self.version_constraint.starts_with(['@', ';']) {
            requirement.push(' ');
        }
        requirement + &self.version_constraint
    }
}

/// Where a package added with `P` comes from, other than the default index.
#[derive(Clone, Debug, PartialEq)]
pub enum Source {
    /// `package`, from the index at `url` and no other.
    Index { package: String, url: String },
    /// A git repository; `@ref` at the end of `url` picks a branch, tag or commit.
    Git { url: String },
    /// A project on disk.
    Path { path: String, editable: bool },
}

impl Source {
    /// Why uv should not be asked to add this, if anything is wrong with it.
    pub fn problem(&self) -> Option<&'static str> {
        match self {
            Source::Index { package, url } => {
                if package.trim().is_empty() {
                    Some("Name the package")
                } else if !url.starts_with("https://") && !url.starts_with("http://") {
                    Some("The index URL starts with https://")
                } else if has_credentials(url) {
                    Some("No credentials in the URL; uv reads them itself")
                } else {
                    None
                }
            }
            Source::Git { url } => {
                let url = url.strip_prefix("git+").unwrap_or(url);
                if !["https://", "http://", "ssh://", "file://"]
                    .iter()
                    .any(|scheme| url.starts_with(scheme))
                {
                    Some("The git URL starts with https://, ssh:// or file://")
                } else if url.starts_with("http") && has_credentials(url) {
                    Some("No credentials in the URL; git reads them itself")
                } else {
                    None
                }
            }
            Source::Path { path, .. } if path.trim().is_empty() => Some("Type a path"),
            Source::Path { .. } => None,
        }
    }

    /// What is being added, for the progress label and the notice.
    pub fn label(&self) -> String {
        match self {
            Source::Index { package, url } => format!("{package} from {url}"),
            Source::Git { url } => url.clone(),
            Source::Path { path, .. } => path.clone(),
        }
    }
}

/// One of the project's `[[tool.uv.index]]` entries.
#[derive(Clone, Debug, PartialEq)]
pub struct Index {
    /// `None` for an index `uv add --index <url>` added without one.
    pub name: Option<String>,
    pub url: String,
    pub kind: IndexKind,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum IndexKind {
    /// Searched before PyPI, which is still searched.
    Extra,
    /// Replaces PyPI.
    Default,
    /// Used only by the packages `[tool.uv.sources]` pins to it.
    Explicit,
}

impl fmt::Display for IndexKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IndexKind::Extra => write!(f, "extra"),
            IndexKind::Default => write!(f, "default"),
            IndexKind::Explicit => write!(f, "explicit"),
        }
    }
}

/// Why an index named `name` at `url` should not be added beside `existing`, if anything is
/// wrong with it.
pub fn index_problem(name: &str, url: &str, existing: &[Index]) -> Option<&'static str> {
    index_name_problem(name, existing).or_else(|| index_url_problem(url))
}

/// Why `name` will not do for a new index beside `existing`, as uv checks it.
pub fn index_name_problem(name: &str, existing: &[Index]) -> Option<&'static str> {
    if name.is_empty() {
        Some("Name the index")
    } else if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    {
        Some("A name is letters, digits, -, _ and . only")
    } else if existing.iter().any(|i| i.name.as_deref() == Some(name)) {
        Some("An index has that name already")
    } else {
        None
    }
}

/// Why `url` will not do for an index. uv takes a mistyped URL for a path, so it would not
/// catch one.
pub fn index_url_problem(url: &str) -> Option<&'static str> {
    if !url.starts_with("https://") && !url.starts_with("http://") {
        Some("The URL starts with https://")
    } else if has_credentials(url) {
        Some("No credentials in the URL; uv reads them itself")
    } else {
        None
    }
}

/// Whether the part of `url` before its path holds a user or a token, as in
/// `https://token@host/`.
fn has_credentials(url: &str) -> bool {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    rest.split('/')
        .next()
        .is_some_and(|host| host.contains('@'))
}

/// The name uv files the index at `url` under: its host and path, each run of other
/// characters a dash, e.g. `download-pytorch-org-whl-cpu`. Each URL gets its own name: by the
/// host alone, a second index on that host would take over the first's packages.
pub fn index_name(url: &str) -> String {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    rest.to_lowercase()
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

#[derive(Clone, Debug, Deserialize)]
pub struct EnvDependency {
    pub name: String,
    pub version: String,
}

/// The project's name and version, as `uv version` reports them.
#[derive(Debug, Deserialize)]
pub struct ProjectInfo {
    /// Normalized: `My_Project` is `my-project`.
    #[serde(rename = "package_name")]
    pub name: String,
    pub version: String,
}

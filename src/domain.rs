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

use std::fmt;
use std::str::FromStr;

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

/// A step up from the project's version, as `uv version --bump` takes it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Bump {
    Patch,
    Minor,
    Major,
}

impl Bump {
    /// In the order the version list offers them.
    pub const ALL: [Bump; 3] = [Bump::Patch, Bump::Minor, Bump::Major];

    pub fn as_str(self) -> &'static str {
        match self {
            Bump::Patch => "patch",
            Bump::Minor => "minor",
            Bump::Major => "major",
        }
    }
}

/// How to change the project's version.
#[derive(Clone, Debug, PartialEq)]
pub enum VersionChange {
    Bump(Bump),
    /// To a version typed in.
    To(String),
}

/// Why `version` will not do, if it plainly will not. uv checks the rest of PEP 440, and writes
/// nothing when it refuses.
pub fn version_problem(version: &str) -> Option<&'static str> {
    if !version.starts_with(|c: char| c.is_ascii_digit()) {
        Some("A version starts with a number, e.g. 1.0.0")
    } else if !version
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '+' | '!' | '_' | '-'))
    {
        Some("Letters, digits and . + ! _ - only")
    } else {
        None
    }
}

/// `name` as PEP 503 normalizes a project name: lower case, each run of `-`, `_` and `.` one
/// dash. Indexes and uv compare names this way.
pub fn normalize_name(name: &str) -> String {
    name.split(['-', '_', '.'])
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
        .to_lowercase()
}

/// A version a package can be pinned to, as the version list shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct VersionChoice {
    pub version: String,
    /// An alpha, beta, release candidate or dev release.
    pub pre: bool,
    /// What its files need when none supports the project's oldest Python, e.g. `>=3.12`.
    pub needs_python: Option<String>,
}

/// The constraints a pin to `version` can take, each with what it means, `==` first. One that
/// PEP 440 does not allow for this version, such as `~=` on `1` or on a local version, is left out.
pub fn constraint_choices(version: &str) -> Vec<(String, String)> {
    let mut choices = vec![
        (format!("=={version}"), "exactly this version".to_string()),
        (format!(">={version}"), "this version or newer".to_string()),
    ];
    // A release series needs at least two parts, and means little for a pre-release.
    if let Ok(parsed) = pep440_rs::Version::from_str(version)
        && !parsed.any_prerelease()
        && let Some((_, series)) = parsed.release().split_last()
        && let Some((&last, head)) = series.split_last()
    {
        let join = |parts: &[u64]| {
            parts
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(".")
        };
        let next = join(&[head, &[last + 1]].concat());
        let series = join(series);
        choices.push((
            format!("~={version}"),
            format!("compatible: >={version}, <{next}"),
        ));
        choices.push((format!("=={series}.*"), format!("any {series}.x release")));
    }
    choices.extend(
        [
            (">", "newer than this"),
            ("<=", "this version or older"),
            ("<", "older than this"),
            ("!=", "any version but this"),
        ]
        .map(|(op, means)| (format!("{op}{version}"), means.to_string())),
    );
    choices.retain(|(constraint, _)| pep440_rs::VersionSpecifier::from_str(constraint).is_ok());
    choices
}

/// `requirement` as a project name and that name with its extras, when it is nothing more: no
/// version, marker or URL. `flask[dotenv]` gives `("flask", "flask[dotenv]")`.
pub fn bare_name(requirement: &str) -> Option<(String, String)> {
    let requirement = requirement.trim();
    let (name, extras) = match requirement.split_once('[') {
        Some((name, extras)) => (name, Some(extras)),
        None => (requirement, None),
    };
    let name_is_bare = !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
    let extras_are_bare = extras.is_none_or(|extras| {
        extras
            .strip_suffix(']')
            .is_some_and(|inner| !inner.contains(['[', ']']))
    });
    (name_is_bare && extras_are_bare).then(|| (name.to_string(), requirement.to_string()))
}

/// The newest release of each bump above `current`, in `Bump::ALL`'s order: a patch keeps the
/// major and minor, a minor keeps the major, a major takes any. `None` where nothing is newer.
/// `versions` comes without yanked releases; pre-releases count only when `current` is one, and
/// a release that needs a newer Python than the project supports does not count.
pub fn bump_targets(current: &str, versions: &[VersionChoice]) -> [Option<String>; 3] {
    let Ok(current) = pep440_rs::Version::from_str(current) else {
        return [None, None, None];
    };
    let part = |v: &pep440_rs::Version, i: usize| v.release().get(i).copied().unwrap_or(0);
    let mut targets: [Option<pep440_rs::Version>; 3] = [None, None, None];
    for choice in versions.iter().filter(|c| c.needs_python.is_none()) {
        let Ok(version) = pep440_rs::Version::from_str(&choice.version) else {
            continue;
        };
        if version <= current || (version.any_prerelease() && !current.any_prerelease()) {
            continue;
        }
        let same_major = part(&version, 0) == part(&current, 0);
        let same_minor = same_major && part(&version, 1) == part(&current, 1);
        for (target, fits) in targets.iter_mut().zip([same_minor, same_major, true]) {
            if fits && target.as_ref().is_none_or(|t| version > *t) {
                *target = Some(version.clone());
            }
        }
    }
    targets.map(|t| t.map(|v| v.to_string()))
}

/// The version a declared constraint starts from: the highest of its lower bounds, as `0.27`
/// in `>=0.27,<1`. `None` for a constraint with none, a URL, or none at all.
pub fn declared_floor(constraint: &str) -> Option<String> {
    use pep440_rs::Operator::*;
    specifiers(constraint)?
        .into_iter()
        .filter(|s| {
            matches!(
                s.operator(),
                Equal | ExactEqual | TildeEqual | GreaterThan | GreaterThanEqual
            )
        })
        .map(|s| s.version().clone())
        .max()
        .map(|v| v.to_string())
}

/// The specifiers of a declared constraint, without its marker; `None` for a URL or one PEP 440
/// does not read.
fn specifiers(constraint: &str) -> Option<Vec<pep440_rs::VersionSpecifier>> {
    let specs = constraint.split(';').next().unwrap_or_default().trim();
    if specs.starts_with('@') {
        return None;
    }
    specs
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| pep440_rs::VersionSpecifier::from_str(s).ok())
        .collect()
}

/// What a bump writes.
#[derive(Debug, PartialEq)]
pub enum Bumped {
    /// The constraint, its operators kept and the new version in place of the old.
    To(String),
    /// The new version falls outside the declared range: the range that would replace it, of
    /// the same width, for the user to confirm.
    OutOfRange { old: String, new: String },
}

/// The constraint a bump of a package declared as `declared` to `new` writes, without the
/// marker, which the pin path keeps. Lower bounds take the new version; a range whose upper
/// bound would exclude it moves that bound past it, keeping its number of parts: `>=1.15,<1.16`
/// bumped to 1.17.0 is `>=1.17.0,<1.18`. Exclusions stay. `None` for a URL, or a constraint PEP
/// 440 does not read.
pub fn bump_constraint(declared: &str, new: &str) -> Option<Bumped> {
    use pep440_rs::Operator::*;
    let target = pep440_rs::Version::from_str(new).ok()?;
    let specs = specifiers(declared)?;
    if specs.is_empty() {
        return Some(Bumped::To(format!(">={new}")));
    }
    // `parts` of the new version, the last moved up one when `past`.
    let cut = |parts: usize, past: bool| {
        let mut release: Vec<u64> = (0..parts)
            .map(|i| target.release().get(i).copied().unwrap_or(0))
            .collect();
        if past && let Some(last) = release.last_mut() {
            *last += 1;
        }
        release
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(".")
    };
    let rewrite = |move_upper: bool| -> String {
        specs
            .iter()
            .map(|spec| {
                let parts = spec.version().release().len();
                match spec.operator() {
                    Equal | ExactEqual | TildeEqual | GreaterThan | GreaterThanEqual => {
                        format!("{}{new}", spec.operator())
                    }
                    EqualStar => format!("=={}.*", cut(parts, false)),
                    LessThan if move_upper => format!("<{}", cut(parts, true)),
                    LessThanEqual if move_upper => format!("<={}", cut(parts, false)),
                    _ => spec.to_string(),
                }
            })
            .collect::<Vec<_>>()
            .join(",")
    };
    let admits = |constraint: &str| {
        pep440_rs::VersionSpecifiers::from_str(constraint).is_ok_and(|s| s.contains(&target))
    };
    let kept = rewrite(false);
    if admits(&kept) {
        return Some(Bumped::To(kept));
    }
    let moved = rewrite(true);
    admits(&moved).then(|| Bumped::OutOfRange {
        old: declared
            .split(';')
            .next()
            .unwrap_or_default()
            .trim()
            .to_string(),
        new: moved,
    })
}

/// What PyPI says of a project beyond its versions.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PackageInfo {
    pub summary: Option<String>,
    /// The Python its latest release needs, as a PEP 440 specifier.
    pub requires_python: Option<String>,
    pub homepage: Option<String>,
}

/// A package's versions as its index lists them, and what PyPI says of it when PyPI is that
/// index.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct VersionList {
    /// Newest first.
    pub versions: Vec<VersionChoice>,
    /// The Simple API URL of the index that answered.
    pub index: String,
    /// Only for a package taken from PyPI: another index's project may share its name there.
    pub info: Option<PackageInfo>,
}

impl VersionList {
    /// The version uv takes with none asked for: the newest that is no pre-release and suits
    /// the project's Python.
    pub fn latest_stable(&self) -> Option<&str> {
        self.versions
            .iter()
            .find(|v| !v.pre && v.needs_python.is_none())
            .map(|v| v.version.as_str())
    }
}

/// A `[dependency-groups]` group as pyproject.toml declares it.
#[derive(Clone, Debug, PartialEq)]
pub struct DeclaredGroup {
    pub name: String,
    /// What names it elsewhere, so that it can be neither deleted nor renamed: uv would refuse
    /// a default group or an included one that is not defined.
    pub named_by: Option<String>,
}

/// Why `name` will not do for a new group beside `existing`, as uv checks a group's name: it
/// starts and ends with a letter or digit, holds only those and `-`, `_`, `.`, and is compared
/// normalized, so `Dev_Tools` is taken when `dev-tools` is.
pub fn group_name_problem(name: &str, existing: &[DeclaredGroup]) -> Option<&'static str> {
    let ends = |c: Option<char>| c.is_some_and(|c| c.is_ascii_alphanumeric());
    if name.is_empty() {
        Some("Name the group")
    } else if !ends(name.chars().next())
        || !ends(name.chars().last())
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    {
        Some("Letters, digits, - _ and . only, starting and ending with a letter or digit")
    } else if existing
        .iter()
        .any(|g| normalize_name(&g.name) == normalize_name(name))
    {
        Some("A group has that name already")
    } else {
        None
    }
}

/// Whether uv.lock matches what pyproject.toml declares, as `uv lock --check` says without the
/// network.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LockState {
    Fresh,
    /// pyproject.toml asks for what the lock does not hold: `uv lock` would change it.
    Stale,
    Missing,
    /// uv could not tell without the network, as for a package not in its cache.
    Unknown,
}

//! The indexes uv would ask for a package, from wherever uv reads them (ADR-0019): the
//! environment, then the project's uv.toml or `[tool.uv]`, the user's uv.toml and the system's,
//! merged as uv merges them.

use std::path::{Path, PathBuf};

use crate::controllers::indexes::pinned_index;

/// PyPI's Simple API: the default index when nothing names another.
pub const PYPI: &str = "https://pypi.org/simple";

/// The variables uv takes indexes from.
const VARIABLES: [&str; 4] = [
    "UV_INDEX",
    "UV_DEFAULT_INDEX",
    "UV_INDEX_URL",
    "UV_EXTRA_INDEX_URL",
];

/// What uv takes its indexes from: the environment's index variables, and the settings tables
/// that count, most binding first. A uv.toml's table is the whole file; a pyproject.toml's, its
/// `[tool.uv]`.
#[derive(Debug, Default)]
pub struct UvSettings {
    /// Each of `VARIABLES` that is set, with its value.
    pub env: Vec<(&'static str, String)>,
    pub files: Vec<toml::Table>,
}

impl UvSettings {
    /// As uv reads them for the project whose workspace root is `workspace`. `user` is the
    /// directory the user's settings go in, where uview's go too; `env` reads the environment.
    // ponytail: UV_ISOLATED, UV_NO_SYSTEM_CONFIG and `no-index` are not read; read them when
    // someone sets them.
    pub fn read(
        workspace: &Path,
        user: Option<&Path>,
        env: impl Fn(&str) -> Option<String>,
        windows: bool,
    ) -> Self {
        let set = |key: &str| env(key).filter(|value| !value.is_empty());
        Self {
            env: VARIABLES
                .iter()
                .filter_map(|&key| Some((key, set(key)?)))
                .collect(),
            files: files(workspace, user, &set, windows),
        }
    }

    fn var(&self, key: &str) -> Option<&str> {
        self.env
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, value)| value.as_str())
    }
}

/// The settings `text` holds, if it parses: all of a uv.toml, or a pyproject.toml's
/// `[tool.uv]`, which it may not have.
pub fn table(text: &str, pyproject: bool) -> Option<toml::Table> {
    let doc: toml::Table = text.parse().ok()?;
    if !pyproject {
        return Some(doc);
    }
    doc.get("tool")?.get("uv")?.as_table().cloned()
}

/// The settings tables uv reads, most binding first: the file UV_CONFIG_FILE names alone, none
/// under UV_NO_CONFIG, else the project's, the user's and the system's.
fn files(
    workspace: &Path,
    user: Option<&Path>,
    set: &impl Fn(&str) -> Option<String>,
    windows: bool,
) -> Vec<toml::Table> {
    let read = |path: &Path, pyproject| table(&std::fs::read_to_string(path).ok()?, pyproject);
    if set("UV_NO_CONFIG").is_some() {
        return Vec::new();
    }
    if let Some(file) = set("UV_CONFIG_FILE") {
        return read(Path::new(&file), false).into_iter().collect();
    }
    // The nearest directory with a uv.toml, or a pyproject.toml that has a [tool.uv]; a uv.toml
    // beside one wins over it.
    let project = workspace.ancestors().find_map(|dir| {
        read(&dir.join("uv.toml"), false).or_else(|| read(&dir.join("pyproject.toml"), true))
    });
    let user = user.and_then(|dir| read(&dir.join("uv").join("uv.toml"), false));
    let system = system_files(set, windows)
        .iter()
        .find_map(|path| read(path, false));
    [project, user, system].into_iter().flatten().collect()
}

/// Where the system's uv.toml may be, in the order uv looks: the first that is there counts.
fn system_files(set: &impl Fn(&str) -> Option<String>, windows: bool) -> Vec<PathBuf> {
    let uv_toml = |dir: PathBuf| dir.join("uv").join("uv.toml");
    if windows {
        let data = set("PROGRAMDATA")
            .map(PathBuf::from)
            .or_else(|| set("SYSTEMDRIVE").map(|drive| Path::new(&drive).join("ProgramData")));
        return data.map(uv_toml).into_iter().collect();
    }
    let xdg = set("XDG_CONFIG_DIRS").unwrap_or_else(|| "/etc/xdg".into());
    xdg.split(':')
        .filter(|dir| Path::new(dir).is_absolute())
        .map(|dir| uv_toml(PathBuf::from(dir)))
        .chain([PathBuf::from("/etc/uv/uv.toml")])
        .collect()
}

/// An index as a settings table or the environment gives it.
struct Entry {
    name: Option<String>,
    url: String,
    default: bool,
    explicit: bool,
}

/// The `[[index]]` tables of `table`, in order.
fn index_tables(table: &toml::Table) -> impl Iterator<Item = &toml::Table> {
    table
        .get("index")
        .and_then(toml::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(toml::Value::as_table)
}

fn entries(table: &toml::Table) -> impl Iterator<Item = Entry> {
    index_tables(table).filter_map(|index| {
        let flag = |key| index.get(key).and_then(toml::Value::as_bool) == Some(true);
        Some(Entry {
            name: index
                .get("name")
                .and_then(toml::Value::as_str)
                .map(String::from),
            url: index.get("url")?.as_str()?.to_string(),
            default: flag("default"),
            explicit: flag("explicit"),
        })
    })
}

/// The strings of `table`'s `key`, a list of them or one.
fn strings(table: &toml::Table, key: &str) -> Vec<String> {
    match table.get(key) {
        Some(toml::Value::String(one)) => vec![one.clone()],
        Some(toml::Value::Array(list)) => list
            .iter()
            .filter_map(toml::Value::as_str)
            .map(String::from)
            .collect(),
        _ => Vec::new(),
    }
}

/// The default index `table` sets, if it sets one: `Some(None)` when it is explicit, which
/// leaves no default, not even PyPI.
fn default_of(table: &toml::Table) -> Option<Option<String>> {
    if let Some(index) = entries(table).find(|i| i.default) {
        return Some((!index.explicit).then_some(index.url));
    }
    strings(table, "index-url").into_iter().next().map(Some)
}

/// The indexes to ask for `package`, in the order uv would take it from them. The index
/// `[tool.uv.sources]` in `pyproject` pins it to, alone. Else, ahead of the default, the indexes
/// UV_INDEX names, then those of the settings files that are not explicit, then the legacy
/// `extra-index-url`s. The default is UV_DEFAULT_INDEX or UV_INDEX_URL, else the first a file
/// sets, else PyPI. uv takes the first that has it.
pub fn indexes_for(settings: &UvSettings, pyproject: &str, package: &str) -> Vec<String> {
    let defined: Vec<Entry> = settings.files.iter().flat_map(entries).collect();
    // A URL, `name=url`, or the name of an index a file defines.
    let given = |value: &str| -> Option<Entry> {
        let (name, url) = match value.split_once('=') {
            Some((name, url)) if !name.contains([':', '/']) => (Some(name), url.to_string()),
            _ if !value.contains("://") => {
                let index = defined.iter().find(|i| i.name.as_deref() == Some(value))?;
                (Some(value), index.url.clone())
            }
            _ => (None, value.to_string()),
        };
        Some(Entry {
            name: name.map(String::from),
            url,
            default: false,
            explicit: false,
        })
    };
    let words = |key| {
        settings
            .var(key)
            .into_iter()
            .flat_map(str::split_whitespace)
    };
    let from_env: Vec<Entry> = words("UV_INDEX").filter_map(given).collect();
    let all = || from_env.iter().chain(&defined);
    if let Some(pinned) = pinned_index(pyproject, package) {
        return all()
            .find(|i| i.name.as_deref() == Some(pinned.as_str()))
            .map(|i| trimmed(&i.url))
            .into_iter()
            .collect();
    }
    let default = settings
        .var("UV_DEFAULT_INDEX")
        .and_then(given)
        .map(|i| i.url)
        .or_else(|| settings.var("UV_INDEX_URL").map(String::from))
        .map(Some)
        .or_else(|| settings.files.iter().find_map(default_of))
        .unwrap_or_else(|| Some(PYPI.to_string()));
    let ahead = all()
        .filter(|i| !i.default && !i.explicit)
        .map(|i| i.url.clone())
        .chain(words("UV_EXTRA_INDEX_URL").map(String::from))
        .chain(
            settings
                .files
                .iter()
                .flat_map(|t| strings(t, "extra-index-url")),
        );
    let mut asked: Vec<String> = Vec::new();
    for url in ahead.chain(default) {
        let url = trimmed(&url);
        if !asked.contains(&url) {
            asked.push(url);
        }
    }
    asked
}

/// `url` without the trailing slash, as uv compares them.
fn trimmed(url: &str) -> String {
    url.trim_end_matches('/').to_string()
}

/// Whether uv would go on past the index at `url` when it answers `code`, as for a package it
/// lacks: the codes its `ignore-error-codes` lists, or 403 from the PyTorch index, which answers
/// that for a package it does not have.
pub fn ignores(settings: &UvSettings, url: &str, code: u16) -> bool {
    let listed = settings
        .files
        .iter()
        .flat_map(index_tables)
        .find(|index| {
            index
                .get("url")
                .and_then(toml::Value::as_str)
                .is_some_and(|u| trimmed(u) == trimmed(url))
        })
        .and_then(|index| index.get("ignore-error-codes")?.as_array().cloned());
    match listed {
        Some(codes) => codes
            .iter()
            .any(|c| c.as_integer() == Some(i64::from(code))),
        None => code == 403 && url.contains("download.pytorch.org"),
    }
}

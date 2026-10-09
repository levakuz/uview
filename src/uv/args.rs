//! `uv` argument lists, built without spawning or awaiting anything.

use crate::domain::{Membership, Source, VersionChange, index_name};

/// Brings the environment exactly in line with the lockfile: packages it does not list, such
/// as `uv pip install`ed ones, are uninstalled.
pub const SYNC: [&str; 1] = ["sync"];
pub const LOCK: [&str; 1] = ["lock"];

/// `uv add` arguments adding from `source` to `membership`. An index is named, so that uv pins
/// the package to it in `[tool.uv.sources]`; a bare `--index` would add it for every package.
pub fn add_from_args(source: &Source, membership: &Membership) -> Vec<String> {
    let mut args = vec!["add".to_string()];
    match source {
        Source::Index { package, url } => {
            args.extend([package.trim().to_string(), "--index".into()]);
            args.push(format!("{}={url}", index_name(url)));
        }
        // Without `git+`, uv takes an https URL for an archive.
        Source::Git { url } if url.starts_with("git+") => args.push(url.clone()),
        Source::Git { url } => args.push(format!("git+{url}")),
        Source::Path { path, editable } => {
            args.push(path.clone());
            if *editable {
                args.push("--editable".into());
            }
        }
    }
    match membership {
        Membership::Project => {}
        Membership::Group(name) => args.extend(["--group".into(), name.clone()]),
        Membership::Extra(name) => args.extend(["--optional".into(), name.clone()]),
    }
    args
}

/// `uv remove` / `uv add` arguments acting on one membership. Each syncs the environment as it
/// ends, as in a shell: `uv add` installs what it needs, and `uv remove` also uninstalls what
/// the lockfile no longer lists.
pub fn membership_args<'a>(
    verb: &'a str,
    target: &'a str,
    membership: &'a Membership,
) -> Vec<&'a str> {
    let mut args = vec![verb, target];
    match membership {
        Membership::Project => {}
        Membership::Group(name) => args.extend(["--group", name.as_str()]),
        Membership::Extra(name) => args.extend(["--optional", name.as_str()]),
    }
    args
}

/// Upgrades `name` within its constraints, and syncs exactly.
pub fn upgrade_args(name: &str) -> [&str; 3] {
    ["sync", "--upgrade-package", name]
}

/// `uv init` arguments that create a project in `dir`, named `name`, or after `dir` as uv names
/// it.
pub fn init_args<'a>(dir: &'a str, name: Option<&'a str>) -> Vec<&'a str> {
    let mut args = vec!["init"];
    if let Some(name) = name {
        args.extend(["--name", name]);
    }
    args.push(dir);
    args
}

/// Reads the project's name and version as JSON. Given no new version, `uv version` neither
/// locks nor syncs.
pub const VERSION: [&str; 3] = ["version", "--output-format", "json"];

/// `uv version` arguments that make `change`, or with `dry_run` only say what it would make.
/// Either way uv answers with the new version as JSON.
pub fn version_args(change: &VersionChange, dry_run: bool) -> Vec<String> {
    let mut args = vec!["version".to_string()];
    match change {
        VersionChange::Bump(bump) => args.extend(["--bump".into(), bump.as_str().into()]),
        VersionChange::To(version) => args.push(version.clone()),
    }
    if dry_run {
        args.push("--dry-run".into());
    }
    args.extend(["--output-format".into(), "json".into()]);
    args
}

/// `uv pip list` arguments that read the packages installed in `env` as JSON. Quietly, so that
/// uv writes nothing but errors: given an environment other than the `.venv` in the working
/// directory, as for a workspace member, uv would also name it on stderr.
pub fn pip_list_args(env: &str) -> [&str; 7] {
    [
        "pip", "list", "--quiet", "--format", "json", "--python", env,
    ]
}

/// `uv pip uninstall` arguments that remove `name` from `env` only.
pub fn uninstall_args<'a>(name: &'a str, env: &'a str) -> [&'a str; 5] {
    ["pip", "uninstall", "--python", env, name]
}

/// The requirement that pins `name`, with its `extras`, to a version constraint, e.g. "==1.2.3", ">=2.0",
/// "<3.0". A bare version number (no operator) is treated as an exact pin, and an
/// empty constraint falls back to the latest resolvable version. The declared entry's
/// marker is kept: uv rewrites an entry in place only when the marker matches, and
/// otherwise adds a second, unconditional one.
pub fn pin_requirement(
    name: &str,
    extras: &[String],
    constraint: &str,
    marker: Option<&str>,
) -> String {
    let name = if extras.is_empty() {
        name.to_string()
    } else {
        format!("{name}[{}]", extras.join(","))
    };
    let constraint = constraint.trim();
    let requirement = if constraint.is_empty() {
        name.to_string()
    } else if constraint.starts_with(['=', '>', '<', '!', '~']) {
        format!("{name}{constraint}")
    } else {
        format!("{name}=={constraint}")
    };
    match marker {
        Some(marker) => format!("{requirement}; {marker}"),
        None => requirement,
    }
}

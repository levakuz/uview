//! `uv` argument lists, built without spawning or awaiting anything.

use crate::domain::Membership;

/// Brings the environment exactly in line with the lockfile: packages it does not list, such
/// as `uv pip install`ed ones, are uninstalled.
pub const SYNC: [&str; 1] = ["sync"];

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

/// Reads the project's name and version as JSON. Given no new version, `uv version` neither
/// locks nor syncs.
pub const VERSION: [&str; 3] = ["version", "--output-format", "json"];

/// `uv pip list` arguments that read the packages installed in `env` as JSON. Quietly: when
/// `env` is not the `.venv` in the working directory, as for a workspace member, uv names it
/// on stderr, which would put a line in the uv log on every reload. Errors still come through.
pub fn pip_list_args(env: &str) -> [&str; 7] {
    [
        "pip", "list", "--quiet", "--format", "json", "--python", env,
    ]
}

/// `uv pip uninstall` arguments that remove `name` from `env` only.
pub fn uninstall_args<'a>(name: &'a str, env: &'a str) -> [&'a str; 5] {
    ["pip", "uninstall", "--python", env, name]
}

/// The requirement that pins `name` to a version constraint, e.g. "==1.2.3", ">=2.0",
/// "<3.0". A bare version number (no operator) is treated as an exact pin, and an
/// empty constraint falls back to the latest resolvable version. The declared entry's
/// marker is kept: uv rewrites an entry in place only when the marker matches, and
/// otherwise adds a second, unconditional one.
pub fn pin_requirement(name: &str, constraint: &str, marker: Option<&str>) -> String {
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

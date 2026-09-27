//! `uv` argument lists, built without spawning or awaiting anything.

use crate::events::Membership;

/// Brings the environment up to date without uninstalling packages the lockfile does not
/// list. A package just removed from pyproject.toml therefore stays installed; the
/// Environment table can uninstall it.
pub const SYNC_INEXACT: [&str; 2] = ["sync", "--inexact"];

/// `uv remove` / `uv add` arguments acting on one membership. `--no-sync`: the caller
/// syncs once, inexactly, afterwards; each implicit sync would be exact and uninstall
/// whatever the lockfile does not list, such as `uv pip install`ed tools.
pub fn membership_args<'a>(
    verb: &'a str,
    target: &'a str,
    membership: &'a Membership,
) -> Vec<&'a str> {
    let mut args = vec![verb, "--no-sync", target];
    match membership {
        Membership::Project => {}
        Membership::Group(name) => args.extend(["--group", name.as_str()]),
        Membership::Extra(name) => args.extend(["--optional", name.as_str()]),
    }
    args
}

/// Upgrades `name` within its constraints, inexactly for the same reason as [`SYNC_INEXACT`].
pub fn upgrade_args(name: &str) -> [&str; 4] {
    ["sync", "--inexact", "--upgrade-package", name]
}

/// `uv pip list` arguments that read the packages installed in `env` as JSON.
pub fn pip_list_args(env: &str) -> [&str; 6] {
    ["pip", "list", "--format", "json", "--python", env]
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

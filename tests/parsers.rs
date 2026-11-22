//! What uview reads, from files on disk and from uv's output, against fixtures in
//! `tests/fixtures/`. The uv output was captured from uv 0.8.9.

use uview::controllers::dependencies::parse_pyproject_deps;
use uview::domain::{Membership, PyprojectDependency};
use uview::uv::args::{LOCK, SYNC, VERSION, upgrade_args};
use uview::uv::{UvError, parse};

/// `(name, groups, extras, constraint)` of each row, for comparing at a glance.
type Row = (String, Vec<Membership>, Vec<String>, String);

fn rows(deps: Vec<PyprojectDependency>) -> Vec<Row> {
    deps.into_iter()
        .map(|d| (d.name, d.groups, d.extras, d.version_constraint))
        .collect()
}

fn row(name: &str, groups: &[Membership], extras: &[&str], constraint: &str) -> Row {
    (
        name.into(),
        groups.to_vec(),
        extras.iter().map(ToString::to_string).collect(),
        constraint.into(),
    )
}

fn group(name: &str) -> Membership {
    Membership::Group(name.into())
}

#[test]
fn the_main_dependencies_are_read_with_extras_and_markers() -> color_eyre::Result<()> {
    let deps = parse_pyproject_deps(include_str!("fixtures/pyproject/main_only.toml"))?;
    assert_eq!(
        rows(deps),
        [
            row("flask", &[Membership::Project], &["dotenv"], ">=3"),
            row("typing-extensions", &[Membership::Project], &[], ""),
            row(
                "tomli",
                &[Membership::Project],
                &[],
                "; python_version < '3.11'"
            ),
        ]
    );
    Ok(())
}

#[test]
fn groups_and_extras_are_read_and_a_package_in_several_is_one_row() -> color_eyre::Result<()> {
    let deps = parse_pyproject_deps(include_str!("fixtures/pyproject/all_kinds.toml"))?;
    assert_eq!(
        rows(deps),
        [
            // Main first; then the groups, by name; then the extras. Its first declaration
            // gives a package's constraint and extras.
            row(
                "httpx",
                &[
                    Membership::Project,
                    group("dev"),
                    Membership::Extra("cli".into())
                ],
                &[],
                ">=0.27"
            ),
            row("pydantic", &[Membership::Project], &[], "~=2.7"),
            row("pytest", &[group("dev")], &[], ">=8"),
            row("ruff", &[group("lint")], &[], "==0.6.9"),
            row("click", &[Membership::Extra("cli".into())], &[], ">=8"),
        ]
    );
    Ok(())
}

#[test]
fn a_project_without_dependencies_has_no_rows_and_bad_toml_is_an_error() -> color_eyre::Result<()> {
    assert!(
        parse_pyproject_deps(include_str!("fixtures/pyproject/no_dependencies.toml"))?.is_empty()
    );
    assert!(parse_pyproject_deps("[project\nname = ").is_err());
    Ok(())
}

#[test]
fn uv_pip_list_is_read_as_uv_writes_it() -> Result<(), UvError> {
    let installed = parse::pip_list(include_str!("fixtures/uv/pip_list.json"))?;
    let names: Vec<(&str, &str)> = installed
        .iter()
        .map(|d| (d.name.as_str(), d.version.as_str()))
        .collect();
    // An editable install carries a field uview has no use for; it is skipped.
    assert_eq!(names.len(), 10);
    assert_eq!(names[2], ("flask", "3.1.3"));
    assert_eq!(names[6], ("mylib", "0.1.0"));
    assert!(parse::pip_list("[]")?.is_empty());
    assert!(matches!(
        parse::pip_list("error: No virtual environment found"),
        Err(UvError::Parse(_))
    ));
    Ok(())
}

#[test]
fn uv_version_is_read_as_uv_writes_it() -> Result<(), UvError> {
    let info = parse::version(include_str!("fixtures/uv/version.json"))?;
    assert_eq!(
        (info.name.as_str(), info.version.as_str()),
        ("acme-api", "0.1.0")
    );
    assert!(matches!(
        parse::version("acme-api 0.1.0"),
        Err(UvError::Parse(_))
    ));
    Ok(())
}

#[test]
fn the_fixed_argvs_are_as_uv_takes_them() {
    // Every group and extra: plain `uv sync` would uninstall those of the others.
    assert_eq!(SYNC, ["sync", "--all-groups", "--all-extras"]);
    assert_eq!(LOCK, ["lock"]);
    assert_eq!(VERSION, ["version", "--output-format", "json"]);
    assert_eq!(
        upgrade_args("six"),
        [
            "sync",
            "--all-groups",
            "--all-extras",
            "--upgrade-package",
            "six"
        ]
    );
}

#[test]
fn uv_lock_check_tells_fresh_stale_missing_and_unknown() {
    use std::process::ExitStatus;
    use uview::domain::LockState;
    use uview::uv::args::{LOCK_CHECK, LOCK_UPGRADE};
    let failed = |stderr: &str| -> Result<String, UvError> {
        Err(UvError::Failed {
            command: "lock --check --offline".into(),
            status: ExitStatus::default(),
            stderr: stderr.into(),
        })
    };
    // As uv 0.8.9 words them.
    assert_eq!(parse::lock_state(&Ok(String::new())), LockState::Fresh);
    assert_eq!(
        parse::lock_state(&failed(
            "Resolved 3 packages in 8ms\\nThe lockfile at `uv.lock` needs to be updated, but `--locked` was provided. To update the lockfile, run `uv lock`."
        )),
        LockState::Stale
    );
    assert_eq!(
        parse::lock_state(&failed(
            "error: Unable to find lockfile at `uv.lock`. To create a lockfile, run `uv lock` or `uv sync`."
        )),
        LockState::Missing
    );
    assert_eq!(
        parse::lock_state(&failed(
            "  × No solution found when resolving dependencies:\\n      hint: Packages were unavailable because the network was disabled."
        )),
        LockState::Offline
    );
    // Any other failure says what uv said.
    assert_eq!(
        parse::lock_state(&failed(
            "error: Failed to parse `pyproject.toml`\n  Caused by: TOML parse error at line 3"
        )),
        LockState::Unknown("Failed to parse `pyproject.toml`".into())
    );
    assert_eq!(LOCK_CHECK, ["lock", "--check", "--offline"]);
    assert_eq!(LOCK_UPGRADE, ["lock", "--upgrade"]);
}

#[test]
fn uv_tree_is_read_from_its_drawing() {
    use uview::domain::TreeNode;
    let nodes = parse::tree(include_str!("fixtures/uv/tree.txt"));
    let shape: Vec<(usize, &str)> = nodes.iter().map(|n| (n.depth, n.name.as_str())).collect();
    assert_eq!(
        shape[..4],
        [(0, "acme-api"), (1, "httpx"), (2, "anyio"), (3, "idna")]
    );
    assert_eq!(nodes.len(), 19);
    let httpx = &nodes[1];
    assert_eq!(
        (httpx.version.as_str(), httpx.extras.as_slice()),
        ("0.27.0", ["http2".to_string()].as_slice())
    );
    let h2 = nodes.iter().find(|n| n.name == "h2");
    assert_eq!(
        h2.map(|n| n.notes.clone()),
        Some(vec!["extra: http2".to_string()])
    );
    let pytest = nodes.iter().find(|n| n.name == "pytest");
    assert_eq!(
        pytest.map(|n| (n.depth, n.notes.clone())),
        Some((1, vec!["group: dev".to_string()]))
    );
    // --outdated marks what is behind.
    let outdated = parse::tree(include_str!("fixtures/uv/tree_outdated.txt"));
    assert_eq!(outdated[1].latest.as_deref(), Some("0.28.1"));
    assert_eq!(outdated[2].latest, None);
    // Turned over: a repeated subtree, and uv's closing note left out.
    let why = parse::tree(include_str!("fixtures/uv/tree_why.txt"));
    assert_eq!(
        why.last(),
        Some(&TreeNode {
            depth: 1,
            name: "httpx".into(),
            version: "0.27.0".into(),
            repeated: true,
            ..TreeNode::default()
        })
    );
    assert_eq!(why.len(), 5);
    assert_eq!(why[3].extras, ["http2".to_string()]);
    assert!(parse::tree("").is_empty());
}

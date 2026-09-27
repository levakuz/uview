use std::collections::VecDeque;
use std::sync::{Mutex, PoisonError};

use uview::controllers::dependencies::{
    get_env_package_info, normalize, parse_dep_string, remove_dep_from_pyproject,
};
use uview::events::Membership;
use uview::uv::args::{membership_args, pin_requirement, uninstall_args};
use uview::uv::{RealUvClient, UvClient, UvError};

/// Stands in for `uv`: answers each call with the next canned result, or empty stdout once
/// they run out, and records the argv it received.
#[derive(Default)]
struct FakeUvClient {
    results: Mutex<VecDeque<Result<String, UvError>>>,
    calls: Mutex<Vec<Vec<String>>>,
}

impl FakeUvClient {
    fn returning(results: impl IntoIterator<Item = Result<String, UvError>>) -> Self {
        Self {
            results: Mutex::new(results.into_iter().collect()),
            ..Self::default()
        }
    }

    fn calls(&self) -> Vec<Vec<String>> {
        self.calls
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl UvClient for FakeUvClient {
    async fn run(&self, args: &[&str]) -> Result<String, UvError> {
        self.calls
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(args.iter().map(ToString::to_string).collect());
        self.results
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .pop_front()
            .unwrap_or_else(|| Ok(String::new()))
    }
}

#[test]
fn test_parse_dep_string() {
    assert_eq!(
        parse_dep_string("httpx>=0.28.1"),
        ("httpx".into(), ">=0.28.1".into())
    );
    assert_eq!(parse_dep_string("requests"), ("requests".into(), "".into()));
    assert_eq!(
        parse_dep_string("numpy==1.26.0"),
        ("numpy".into(), "==1.26.0".into())
    );
    assert_eq!(
        parse_dep_string("flask~=3.0"),
        ("flask".into(), "~=3.0".into())
    );
    // Operators inside a marker or a URL are not the version's.
    assert_eq!(
        parse_dep_string("tomli; python_version < '3.11'"),
        ("tomli".into(), "; python_version < '3.11'".into())
    );
    assert_eq!(
        parse_dep_string("pkg @ https://example.com/pkg-1.0.tar.gz"),
        ("pkg".into(), "@ https://example.com/pkg-1.0.tar.gz".into())
    );
}

#[test]
fn pin_requirement_normalizes_constraints() {
    assert_eq!(pin_requirement("httpx", "1.2.3", None), "httpx==1.2.3");
    assert_eq!(pin_requirement("httpx", " >=2.0 ", None), "httpx>=2.0");
    assert_eq!(pin_requirement("flask", "~=3.0", None), "flask~=3.0");
    assert_eq!(pin_requirement("httpx", "", None), "httpx");
    assert_eq!(
        pin_requirement("tomli", "2.0.1", Some("python_version < '3.11'")),
        "tomli==2.0.1; python_version < '3.11'"
    );
}

#[tokio::test]
async fn failed_uv_call_returns_error_instead_of_panicking() {
    // `uv` exits 1 for an unknown package; without `uv` on PATH this is a spawn error.
    assert!(
        RealUvClient
            .run(&["pip", "show", "uview-no-such-package"])
            .await
            .is_err()
    );
}

#[tokio::test]
async fn successful_uv_call_returns_stdout() {
    match RealUvClient.run(&["--version"]).await {
        Err(UvError::Spawn(_)) => {} // `uv` is not installed here; nothing to check.
        other => assert!(other.is_ok_and(|out| out.starts_with("uv "))),
    }
}

#[test]
fn membership_args_pick_the_flag_for_each_membership() {
    let main_group = Membership::Group("main".into());
    assert_eq!(
        membership_args("remove", "httpx", &Membership::Project),
        ["remove", "--no-sync", "httpx"]
    );
    // A dependency group that happens to be named `main` is still a group.
    assert_eq!(
        membership_args("remove", "pytest", &main_group),
        ["remove", "--no-sync", "pytest", "--group", "main"]
    );
    assert_eq!(
        membership_args("add", "flask==3.0", &Membership::Extra("web".into())),
        ["add", "--no-sync", "flask==3.0", "--optional", "web"]
    );
}

#[test]
fn uninstall_targets_the_given_environment_explicitly() {
    assert_eq!(
        uninstall_args("six", ".venv"),
        ["pip", "uninstall", "--python", ".venv", "six"]
    );
}

#[test]
fn normalize_ignores_case_separators_and_extras() {
    assert_eq!(normalize("Flask_SQLAlchemy"), normalize("flask-sqlalchemy"));
    assert_eq!(normalize("zope.interface"), normalize("zope-interface"));
    assert_eq!(normalize("flask[dotenv]"), normalize("Flask"));
    assert_ne!(normalize("flask"), normalize("flask-cors"));
}

#[tokio::test]
async fn remove_runs_uv_remove_per_membership_then_one_inexact_sync() -> Result<(), UvError> {
    let uv = FakeUvClient::default();
    let memberships = vec![Membership::Project, Membership::Group("dev".into())];
    remove_dep_from_pyproject(&uv, "httpx", memberships).await?;
    assert_eq!(
        uv.calls(),
        [
            vec!["remove", "--no-sync", "httpx"],
            vec!["remove", "--no-sync", "httpx", "--group", "dev"],
            vec!["sync", "--inexact"],
        ]
    );
    Ok(())
}

#[tokio::test]
async fn env_lookup_reads_pip_list_json_by_normalized_name() -> Result<(), UvError> {
    let pip_list = r#"[{"name": "Flask-SQLAlchemy", "version": "3.1.1"}]"#;
    let uv = FakeUvClient::returning([Ok(pip_list.to_string())]);
    let found = get_env_package_info(&uv, "flask_sqlalchemy").await?;
    assert_eq!(found.map(|dep| dep.version), Some("3.1.1".to_string()));
    assert_eq!(uv.calls()[0][..4], ["pip", "list", "--format", "json"]);
    Ok(())
}

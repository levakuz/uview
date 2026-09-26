use uview::controllers::dependencies::{
    UvError, membership_args, normalize, parse_dep_string, pin_requirement, run_uv, uninstall_args,
};
use uview::events::Membership;

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
        run_uv(&["pip", "show", "uview-no-such-package"])
            .await
            .is_err()
    );
}

#[tokio::test]
async fn successful_uv_call_returns_stdout() {
    match run_uv(&["--version"]).await {
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

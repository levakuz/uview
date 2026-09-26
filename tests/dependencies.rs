use uview::controllers::dependencies::{UvError, parse_dep_string, pin_requirement, run_uv};

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
}

#[test]
fn pin_requirement_normalizes_constraints() {
    assert_eq!(pin_requirement("httpx", "1.2.3"), "httpx==1.2.3");
    assert_eq!(pin_requirement("httpx", " >=2.0 "), "httpx>=2.0");
    assert_eq!(pin_requirement("flask", "~=3.0"), "flask~=3.0");
    assert_eq!(pin_requirement("httpx", ""), "httpx");
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

use uview::controllers::dependencies::{get_env_package_info, parse_dep_string};

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

#[tokio::test]
async fn failed_uv_call_returns_error_instead_of_panicking() {
    // `uv` exits 1 for an unknown package; without `uv` on PATH this is a spawn error.
    assert!(get_env_package_info("uview-no-such-package").await.is_err());
}

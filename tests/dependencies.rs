use uview::controllers::dependencies::parse_dep_string;

#[test]
fn test_parse_dep_string() {
    assert_eq!(parse_dep_string("httpx>=0.28.1"), ("httpx".into(), ">=0.28.1".into()));
    assert_eq!(parse_dep_string("requests"), ("requests".into(), "".into()));
    assert_eq!(parse_dep_string("numpy==1.26.0"), ("numpy".into(), "==1.26.0".into()));
    assert_eq!(parse_dep_string("flask~=3.0"), ("flask".into(), "~=3.0".into()));
}

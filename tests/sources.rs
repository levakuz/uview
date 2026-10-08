use uview::domain::{Membership, Source, index_name};
use uview::uv::args::add_from_args;

fn index(package: &str, url: &str) -> Source {
    Source::Index {
        package: package.into(),
        url: url.into(),
    }
}

fn git(url: &str) -> Source {
    Source::Git { url: url.into() }
}

#[test]
fn each_source_becomes_its_uv_add() {
    let torch = index("torch", "https://download.pytorch.org/whl/cpu");
    assert_eq!(
        add_from_args(&torch, &Membership::Project),
        [
            "add",
            "torch",
            "--index",
            "download-pytorch-org-whl-cpu=https://download.pytorch.org/whl/cpu"
        ]
    );
    // Without `git+` uv would take the URL for an archive; `@ref` is left to uv.
    assert_eq!(
        add_from_args(
            &git("https://github.com/org/repo@v1.0"),
            &Membership::Group("dev".into())
        ),
        [
            "add",
            "git+https://github.com/org/repo@v1.0",
            "--group",
            "dev"
        ]
    );
    assert_eq!(
        add_from_args(
            &git("git+ssh://git@github.com/org/repo"),
            &Membership::Project
        ),
        ["add", "git+ssh://git@github.com/org/repo"]
    );
    let path = |editable| Source::Path {
        path: "../mylib".into(),
        editable,
    };
    assert_eq!(
        add_from_args(&path(true), &Membership::Project),
        ["add", "../mylib", "--editable"]
    );
    assert_eq!(
        add_from_args(&path(false), &Membership::Project),
        ["add", "../mylib"]
    );
}

#[test]
fn each_index_url_gets_its_own_name() {
    assert_eq!(
        index_name("https://download.pytorch.org/whl/cpu"),
        "download-pytorch-org-whl-cpu"
    );
    assert_eq!(
        index_name("http://Localhost:8080/simple/"),
        "localhost-8080-simple"
    );
    // Two indexes on one host keep apart: one name would move the first's packages.
    assert_ne!(
        index_name("https://example.com/a/simple/"),
        index_name("https://example.com/b/simple/")
    );
}

#[test]
fn a_source_uv_should_not_get_is_refused() {
    assert_eq!(index("torch", "https://example.com/simple").problem(), None);
    assert!(index("", "https://example.com/simple").problem().is_some());
    assert!(index("torch", "example.com/simple").problem().is_some());
    // A secret never reaches pyproject.toml.
    assert!(
        index("torch", "https://user:token@example.com/simple")
            .problem()
            .is_some()
    );
    assert!(git("https://token@github.com/org/repo").problem().is_some());
    assert!(git("github.com/org/repo").problem().is_some());
    // A user name in an ssh URL is no secret, and an `@ref` is not in the host.
    assert_eq!(git("ssh://git@github.com/org/repo").problem(), None);
    assert_eq!(git("https://github.com/org/repo@main").problem(), None);
    let path = |path: &str| Source::Path {
        path: path.into(),
        editable: true,
    };
    assert!(path(" ").problem().is_some());
    assert_eq!(path("../mylib").problem(), None);
}

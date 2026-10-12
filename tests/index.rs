//! Reading package indexes: the pages, the versions in them, and which index uv would ask.
//! `packaging.json` is PyPI's JSON page and `sympy.html` the PyTorch CPU index's HTML page, both
//! captured on 2026-10-12.

use std::error::Error;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::PathBuf;

use uview::controllers::versions::{PYPI, choices, indexes_for, list_versions};
use uview::domain::VersionChoice;
use uview::index::parse::{files, version_in};
use uview::index::{IndexClient, IndexError, Page, RealIndexClient, project_url};

fn json(body: &str) -> Page {
    Page {
        json: true,
        body: body.into(),
    }
}

fn html(body: &str) -> Page {
    Page {
        json: false,
        body: body.into(),
    }
}

#[test]
fn a_file_name_carries_its_version() {
    assert_eq!(
        version_in("packaging-24.2-py3-none-any.whl", "packaging"),
        Some("24.2".into())
    );
    // A source archive's name may hold dashes, and is matched as normalized.
    assert_eq!(
        version_in("typing-extensions-4.12.2.tar.gz", "typing-extensions"),
        Some("4.12.2".into())
    );
    assert_eq!(
        version_in("Typing_Extensions-4.1.0.zip", "typing-extensions"),
        Some("4.1.0".into())
    );
    // Neither an egg nor another project's file.
    assert_eq!(version_in("sympy-0.7.1-py2.7.egg", "sympy"), None);
    assert_eq!(version_in("other-1.0.tar.gz", "sympy"), None);
}

#[test]
fn a_json_page_lists_each_file_with_yanked_and_requires_python() -> Result<(), IndexError> {
    let files = files(
        &json(include_str!("fixtures/index/packaging.json")),
        "packaging",
    )?;
    assert_eq!(files.len(), 108);
    // PyPI gives the reason a file was yanked, not `true`.
    let yanked: Vec<&str> = files
        .iter()
        .filter(|f| f.yanked)
        .map(|f| f.version.as_str())
        .collect();
    assert_eq!(yanked, ["20.6", "20.6"]);
    assert!(
        files
            .iter()
            .any(|f| f.requires_python.as_deref() == Some(">=3.8"))
    );
    Ok(())
}

#[test]
fn an_html_page_is_read_from_its_links() -> Result<(), IndexError> {
    let files = files(&html(include_str!("fixtures/index/sympy.html")), "sympy")?;
    assert!(files.iter().any(|f| f.version == "1.13.3"));
    assert!(files.iter().filter(|f| f.yanked).count() >= 4);
    // `&gt;=3.8` in the page is `>=3.8`.
    assert!(
        files
            .iter()
            .any(|f| f.requires_python.as_deref() == Some(">=3.8"))
    );
    assert!(files.iter().all(|f| !f.version.ends_with(".egg")));
    // Not a page uview reads.
    assert!(matches!(files_of_bad_json(), Err(IndexError::Parse(_))));
    Ok(())
}

fn files_of_bad_json() -> Result<Vec<uview::index::parse::File>, IndexError> {
    files(&json("<html>"), "sympy")
}

#[test]
fn versions_are_newest_first_without_yanked_and_marked() -> Result<(), IndexError> {
    let page = json(include_str!("fixtures/index/packaging.json"));
    let listed = choices(files(&page, "packaging")?, Some(">=3.7"));
    let versions: Vec<&str> = listed.iter().map(|v| v.version.as_str()).collect();
    // PEP 440's order, not the page's: a release candidate comes after its final release.
    assert_eq!(
        versions[..7],
        [
            "26.3", "26.2", "26.1", "26.0", "26.0rc3", "26.0rc2", "26.0rc1"
        ]
    );
    assert!(listed[..4].iter().all(|v| !v.pre));
    assert!(listed[4..7].iter().all(|v| v.pre));
    assert!(!versions.contains(&"20.6"));
    // Releases for Python 3.8 and newer cannot serve a project that supports 3.7.
    let needs = |version: &str| {
        listed
            .iter()
            .find(|v| v.version == version)
            .and_then(|v| v.needs_python.clone())
    };
    assert_eq!(needs("24.2"), Some(">=3.8".into()));
    assert_eq!(needs("21.3"), None);
    // Without a requires-python, nothing is marked.
    let files = files(
        &json(include_str!("fixtures/index/packaging.json")),
        "packaging",
    )?;
    assert!(
        choices(files, None)
            .iter()
            .all(|v| v.needs_python.is_none())
    );
    Ok(())
}

const PYPROJECT: &str = r#"[project]
name = "acme-api"
requires-python = ">=3.11"
dependencies = ["torch", "six", "internal-lib"]

[[tool.uv.index]]
name = "pytorch"
url = "https://download.pytorch.org/whl/cpu"

[[tool.uv.index]]
name = "corp"
url = "https://corp.example.com/simple"
explicit = true

[tool.uv.sources]
internal-lib = { index = "corp" }
"#;

#[test]
fn uv_s_order_decides_which_index_to_ask() {
    // Pinned: that index alone.
    assert_eq!(
        indexes_for(PYPROJECT, "Internal_Lib"),
        ["https://corp.example.com/simple"]
    );
    // Else the extras in order, then PyPI; the explicit one is skipped.
    assert_eq!(
        indexes_for(PYPROJECT, "six"),
        ["https://download.pytorch.org/whl/cpu", PYPI]
    );
    // A default index replaces PyPI.
    let replaced = PYPROJECT.replace(
        "url = \"https://download.pytorch.org/whl/cpu\"",
        "url = \"https://download.pytorch.org/whl/cpu\"\ndefault = true",
    );
    assert_eq!(
        indexes_for(&replaced, "six"),
        ["https://download.pytorch.org/whl/cpu"]
    );
    assert_eq!(indexes_for("[project]\nname = \"p\"\n", "six"), [PYPI]);
}

/// Serves pages by URL, answers others "no such package", and fails when told to.
struct Pages {
    pages: Vec<(String, Page)>,
    fails: bool,
}

impl IndexClient for Pages {
    async fn project_page(&self, index: &str, name: &str) -> Result<Page, IndexError> {
        if self.fails {
            return Err(IndexError::Status(401));
        }
        let url = project_url(index, name);
        self.pages
            .iter()
            .find(|(served, _)| *served == url)
            .map(|(_, page)| page.clone())
            .ok_or(IndexError::NotFound)
    }
}

fn scratch_pyproject(test: &str) -> Result<PathBuf, Box<dyn Error>> {
    let dir = std::env::temp_dir().join(format!("uview-index-{test}-{}", std::process::id()));
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("pyproject.toml");
    std::fs::write(&path, PYPROJECT)?;
    Ok(path)
}

#[tokio::test]
async fn the_first_index_that_has_the_package_answers() -> Result<(), Box<dyn Error>> {
    let pyproject = scratch_pyproject("first")?;
    let six = r#"{"files": [
        {"filename": "six-1.16.0-py2.py3-none-any.whl", "yanked": false, "requires-python": null},
        {"filename": "six-1.17.0-py2.py3-none-any.whl", "yanked": false}
    ]}"#;
    // The PyTorch index has no six, so PyPI answers.
    let index = Pages {
        pages: vec![(format!("{PYPI}/six/"), json(six))],
        fails: false,
    };
    assert_eq!(
        list_versions(&index, &pyproject, "six").await?,
        [
            VersionChoice {
                version: "1.17.0".into(),
                pre: false,
                needs_python: None
            },
            VersionChoice {
                version: "1.16.0".into(),
                pre: false,
                needs_python: None
            },
        ]
    );
    let missing = list_versions(&index, &pyproject, "nosuch").await;
    assert!(missing.is_err_and(|e| e.contains("nosuch is not on")));
    // A failure is said, with the index that failed.
    let locked = Pages {
        pages: Vec::new(),
        fails: true,
    };
    let failed = list_versions(&locked, &pyproject, "six").await;
    assert!(failed.is_err_and(|e| e.contains("download.pytorch.org") && e.contains("credentials")));
    std::fs::remove_dir_all(pyproject.parent().ok_or("no parent")?)?;
    Ok(())
}

/// A one-thread HTTP server on localhost that answers each request with the response `answer`
/// gives its path and Accept header, and stops after `requests`.
fn serve(requests: usize, answer: fn(&str, &str) -> String) -> Result<String, Box<dyn Error>> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let url = format!("http://{}", listener.local_addr()?);
    std::thread::spawn(move || {
        for stream in listener.incoming().take(requests).flatten() {
            let mut reader = BufReader::new(&stream);
            let mut lines = Vec::new();
            let mut line = String::new();
            while reader.read_line(&mut line).is_ok_and(|n| n > 2) {
                lines.push(std::mem::take(&mut line));
            }
            let path = lines
                .first()
                .and_then(|l| l.split_whitespace().nth(1))
                .unwrap_or_default()
                .to_string();
            let accept = lines
                .iter()
                .find(|l| l.to_lowercase().starts_with("accept:"))
                .cloned()
                .unwrap_or_default();
            let _ = (&stream).write_all(answer(&path, &accept).as_bytes());
        }
    });
    Ok(url)
}

fn response(status: &str, content_type: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

#[tokio::test]
async fn the_real_client_asks_for_json_and_takes_html_or_a_status() -> Result<(), Box<dyn Error>> {
    let url = serve(4, |path, accept| match path {
        "/simple/six/" if accept.contains("application/vnd.pypi.simple.v1+json") => response(
            "200 OK",
            "application/vnd.pypi.simple.v1+json",
            r#"{"files": []}"#,
        ),
        "/simple/sympy/" => response("200 OK", "text/html", "<a href=\"x\">sympy-1.0.tar.gz</a>"),
        "/simple/private/" => response("401 Unauthorized", "text/plain", ""),
        _ => response("404 Not Found", "text/plain", ""),
    })?;
    let client = RealIndexClient::new();
    let index = format!("{url}/simple/");
    let page = client.project_page(&index, "Six").await?;
    assert!(page.json && page.body.contains("files"));
    let page = client.project_page(&index, "sympy").await?;
    assert!(!page.json && page.body.contains("sympy-1.0"));
    assert!(matches!(
        client.project_page(&index, "private").await,
        Err(IndexError::Status(401))
    ));
    assert!(matches!(
        client.project_page(&index, "nosuch").await,
        Err(IndexError::NotFound)
    ));
    Ok(())
}

#[test]
fn uv_goes_past_the_codes_an_index_answers_for_a_missing_package() {
    use uview::controllers::versions::ignores;
    let pytorch = "https://download.pytorch.org/whl/cpu";
    assert!(ignores(PYPROJECT, pytorch, 403));
    assert!(!ignores(PYPROJECT, pytorch, 401));
    assert!(!ignores(PYPROJECT, "https://corp.example.com/simple", 403));
    // An index's own list replaces the default.
    let listed = PYPROJECT.replace(
        "url = \"https://corp.example.com/simple\"",
        "url = \"https://corp.example.com/simple\"\nignore-error-codes = [401, 403]",
    );
    assert!(ignores(&listed, "https://corp.example.com/simple", 401));
}

#[test]
fn a_version_can_be_pinned_with_each_operator_pep_440_allows_for_it() {
    use uview::domain::constraint_choices;
    let constraints = |version: &str| -> Vec<String> {
        constraint_choices(version)
            .into_iter()
            .map(|(constraint, _)| constraint)
            .collect()
    };
    assert_eq!(
        constraints("0.28.1"),
        [
            "==0.28.1", ">=0.28.1", "~=0.28.1", "==0.28.*", ">0.28.1", "<=0.28.1", "<0.28.1",
            "!=0.28.1"
        ]
    );
    assert_eq!(
        constraint_choices("0.28.1")[2].1,
        "compatible: >=0.28.1, <0.29"
    );
    // One part has no series; a pre-release gets no series either.
    assert!(
        !constraints("2")
            .iter()
            .any(|c| c.starts_with("~=") || c.ends_with(".*"))
    );
    assert!(!constraints("1.0rc1").iter().any(|c| c.starts_with("~=")));
    // A local version, as the PyTorch index serves, keeps only what PEP 440 allows it.
    let local = constraints("2.4.0+cpu");
    assert!(local.contains(&"==2.4.0+cpu".to_string()));
    assert!(!local.contains(&"~=2.4.0+cpu".to_string()));
}

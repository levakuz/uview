//! Reading package indexes: the pages, the versions in them, and which index uv would ask.
//! `packaging.json` is PyPI's JSON page and `sympy.html` the PyTorch CPU index's HTML page, both
//! captured on 2026-10-12.

use std::error::Error;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::PathBuf;

use uview::controllers::uv_settings::{PYPI, UvSettings, indexes_for, table};
use uview::controllers::versions::{choices, list_versions};
use uview::domain::{PackageInfo, VersionChoice};
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

/// The settings of a project whose pyproject.toml is `pyproject`, and nothing else.
fn settings_of(pyproject: &str) -> UvSettings {
    UvSettings {
        env: Vec::new(),
        files: table(pyproject, true).into_iter().collect(),
    }
}

#[test]
fn uv_s_order_decides_which_index_to_ask() {
    // Pinned: that index alone.
    assert_eq!(
        indexes_for(&settings_of(PYPROJECT), PYPROJECT, "Internal_Lib"),
        ["https://corp.example.com/simple"]
    );
    // Else the extras in order, then PyPI; the explicit one is skipped.
    assert_eq!(
        indexes_for(&settings_of(PYPROJECT), PYPROJECT, "six"),
        ["https://download.pytorch.org/whl/cpu", PYPI]
    );
    // A default index replaces PyPI.
    let replaced = PYPROJECT.replace(
        "url = \"https://download.pytorch.org/whl/cpu\"",
        "url = \"https://download.pytorch.org/whl/cpu\"\ndefault = true",
    );
    assert_eq!(
        indexes_for(&settings_of(&replaced), &replaced, "six"),
        ["https://download.pytorch.org/whl/cpu"]
    );
    assert_eq!(
        indexes_for(&UvSettings::default(), "[project]\nname = \"p\"\n", "six"),
        [PYPI]
    );
}

/// `text`, as a uv.toml.
fn uv_toml(text: &str) -> toml::Table {
    table(text, false).unwrap_or_default()
}

#[test]
fn the_environment_and_every_settings_file_count_in_uv_s_order() {
    let settings = UvSettings {
        env: vec![(
            "UV_INDEX",
            "mirror=https://mirror.example/simple corp".into(),
        )],
        files: vec![
            // The project's, then the user's.
            uv_toml(
                r#"extra-index-url = ["https://legacy.example/simple"]
[[index]]
name = "corp"
url = "https://corp.example/simple"
explicit = true
[[index]]
url = "https://project.example/simple/"
"#,
            ),
            uv_toml(
                r#"index-url = "https://user-default.example/simple"
[[index]]
url = "https://user.example/simple"
"#,
            ),
        ],
    };
    // UV_INDEX first, by URL or by the name of one a file defines, explicit or not; then the
    // files' indexes, the project's ahead of the user's; then the legacy extras; then the
    // default a file sets, in place of PyPI.
    assert_eq!(
        indexes_for(&settings, "", "six"),
        [
            "https://mirror.example/simple",
            "https://corp.example/simple",
            "https://project.example/simple",
            "https://user.example/simple",
            "https://legacy.example/simple",
            "https://user-default.example/simple",
        ]
    );
    // The environment's default is the default.
    let mut defaulted = settings;
    defaulted.env = vec![(
        "UV_DEFAULT_INDEX",
        "https://env-default.example/simple".into(),
    )];
    assert_eq!(
        indexes_for(&defaulted, "", "six")
            .last()
            .map(String::as_str),
        Some("https://env-default.example/simple")
    );
    // A pin by name, to an index the settings define.
    let pinned = "[tool.uv.sources]\nsix = { index = \"corp\" }\n";
    assert_eq!(
        indexes_for(&defaulted, pinned, "six"),
        ["https://corp.example/simple"]
    );
    // A default index that is also explicit leaves no default, not even PyPI.
    let explicit = UvSettings {
        env: Vec::new(),
        files: vec![uv_toml(
            "[[index]]\nname = \"corp\"\nurl = \"https://corp.example/simple\"\ndefault = true\nexplicit = true\n",
        )],
    };
    assert!(indexes_for(&explicit, "", "six").is_empty());
}

#[test]
fn the_settings_files_are_found_where_uv_looks() -> Result<(), Box<dyn Error>> {
    let root = std::env::temp_dir().join(format!("uview-uv-settings-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (workspace, user, system) = (root.join("repo/app"), root.join("home"), root.join("etc"));
    for dir in [&workspace, &user.join("uv"), &system.join("uv")] {
        std::fs::create_dir_all(dir)?;
    }
    let index = |url: &str| format!("[[index]]\nurl = \"{url}\"\n");
    // The project's pyproject.toml has no [tool.uv], so the uv.toml above it counts.
    std::fs::write(
        workspace.join("pyproject.toml"),
        "[project]\nname = \"app\"\n",
    )?;
    std::fs::write(
        root.join("repo/uv.toml"),
        index("https://repo.example/simple"),
    )?;
    std::fs::write(
        user.join("uv/uv.toml"),
        index("https://user.example/simple"),
    )?;
    std::fs::write(
        system.join("uv/uv.toml"),
        index("https://system.example/simple"),
    )?;
    let system_dirs = system.to_string_lossy().to_string();
    let read = |pairs: Vec<(&'static str, String)>| {
        let system_dirs = system_dirs.clone();
        let env = move |key: &str| {
            let found = pairs.iter().find(|(k, _)| *k == key);
            found.map(|(_, v)| v.clone()).or_else(|| {
                // Where the system's settings are said to be, on each platform.
                let system = ["XDG_CONFIG_DIRS", "PROGRAMDATA"].contains(&key);
                system.then(|| system_dirs.clone())
            })
        };
        let settings = UvSettings::read(&workspace, Some(&user), env, cfg!(windows));
        indexes_for(&settings, "", "six")
    };
    assert_eq!(
        read(Vec::new()),
        [
            "https://repo.example/simple",
            "https://user.example/simple",
            "https://system.example/simple",
            PYPI
        ]
    );
    // A [tool.uv] in the project's pyproject.toml is nearer; a uv.toml beside it wins over it.
    std::fs::write(
        workspace.join("pyproject.toml"),
        format!(
            "[tool.uv]\n{}",
            index("https://pyproject.example/simple").replace("[[index]]", "[[tool.uv.index]]")
        ),
    )?;
    assert_eq!(read(Vec::new())[0], "https://pyproject.example/simple");
    std::fs::write(
        workspace.join("uv.toml"),
        index("https://beside.example/simple"),
    )?;
    assert_eq!(read(Vec::new())[0], "https://beside.example/simple");
    // UV_CONFIG_FILE stands for every file; UV_NO_CONFIG leaves none.
    let only = root.join("only.toml");
    std::fs::write(&only, index("https://only.example/simple"))?;
    assert_eq!(
        read(vec![("UV_CONFIG_FILE", only.to_string_lossy().to_string())]),
        ["https://only.example/simple", PYPI]
    );
    assert_eq!(read(vec![("UV_NO_CONFIG", "1".into())]), [PYPI]);
    std::fs::remove_dir_all(&root)?;
    Ok(())
}

#[test]
fn credentials_in_an_index_url_are_never_drawn() {
    use uview::domain::redacted;
    assert_eq!(
        redacted("https://jane:s3cret@corp.example.com/simple/"),
        "https://****@corp.example.com/simple/"
    );
    assert_eq!(
        redacted("https://token@corp.example.com"),
        "https://****@corp.example.com"
    );
    // An @ past the host is part of the path.
    assert_eq!(
        redacted("https://corp.example.com/simple/@scope"),
        "https://corp.example.com/simple/@scope"
    );
    assert_eq!(redacted(PYPI), PYPI);
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

    /// PyPI describes every project, so a package taken from another index shows whether the
    /// description is kept off it.
    async fn pypi_project(&self, name: &str) -> Result<PackageInfo, IndexError> {
        Ok(PackageInfo {
            summary: Some(format!("PyPI's {name}")),
            ..PackageInfo::default()
        })
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
    // The PyTorch index has no six, so PyPI answers, and describes it.
    let index = Pages {
        pages: vec![
            (format!("{PYPI}/six/"), json(six)),
            (
                "https://download.pytorch.org/whl/cpu/torch/".into(),
                json(r#"{"files": [{"filename": "torch-2.4.0-cp312-none-any.whl"}]}"#),
            ),
        ],
        fails: false,
    };
    let six = list_versions(&index, &settings_of(PYPROJECT), &pyproject, "six").await?;
    assert_eq!(six.index, PYPI);
    assert_eq!(
        six.info.and_then(|info| info.summary).as_deref(),
        Some("PyPI's six")
    );
    // torch comes from the PyTorch index: what PyPI says of a torch is kept off it.
    let torch = list_versions(&index, &settings_of(PYPROJECT), &pyproject, "torch").await?;
    assert_eq!(torch.index, "https://download.pytorch.org/whl/cpu");
    assert_eq!(torch.info, None);
    assert_eq!(
        six.versions,
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
    let missing = list_versions(&index, &settings_of(PYPROJECT), &pyproject, "nosuch").await;
    assert!(missing.is_err_and(|e| e.contains("nosuch is not on")));
    // A failure is said, with the index that failed.
    let locked = Pages {
        pages: Vec::new(),
        fails: true,
    };
    let failed = list_versions(&locked, &settings_of(PYPROJECT), &pyproject, "six").await;
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
    use uview::controllers::uv_settings::ignores;
    let pytorch = "https://download.pytorch.org/whl/cpu";
    assert!(ignores(&settings_of(PYPROJECT), pytorch, 403));
    assert!(!ignores(&settings_of(PYPROJECT), pytorch, 401));
    assert!(!ignores(
        &settings_of(PYPROJECT),
        "https://corp.example.com/simple",
        403
    ));
    // An index's own list replaces the default.
    let listed = PYPROJECT.replace(
        "url = \"https://corp.example.com/simple\"",
        "url = \"https://corp.example.com/simple\"\nignore-error-codes = [401, 403]",
    );
    assert!(ignores(
        &settings_of(&listed),
        "https://corp.example.com/simple",
        401
    ));
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

#[test]
fn only_a_name_alone_asks_for_its_version() {
    use uview::domain::bare_name;
    assert_eq!(bare_name(" six "), Some(("six".into(), "six".into())));
    assert_eq!(
        bare_name("flask[dotenv]"),
        Some(("flask".into(), "flask[dotenv]".into()))
    );
    for typed in [
        "httpx>=0.27",
        "pkg @ git+https://example.com/pkg",
        "tomli; python_version < '3.11'",
        "flask[dotenv",
        "",
    ] {
        assert_eq!(bare_name(typed), None, "{typed}");
    }
}

#[test]
fn pypi_describes_a_project_in_its_json() -> Result<(), IndexError> {
    use uview::index::parse::pypi_project;
    let info = pypi_project(include_str!("fixtures/index/httpx_pypi.json"))?;
    assert_eq!(
        info.summary.as_deref(),
        Some("The next generation HTTP client.")
    );
    assert_eq!(info.requires_python.as_deref(), Some(">=3.8"));
    assert!(info.homepage.is_some_and(|url| url.starts_with("https://")));
    // What a project left out comes as an empty string, and is nothing.
    let bare = pypi_project(
        r#"{"info": {"summary": "", "requires_python": "", "home_page": null, "project_urls": null}}"#,
    )?;
    assert_eq!(bare, PackageInfo::default());
    Ok(())
}

#[tokio::test]
async fn the_real_client_reads_pypi_s_json_api() -> Result<(), Box<dyn Error>> {
    let url = serve(2, |path, _| match path {
        "/pypi/typing-extensions/json" => response(
            "200 OK",
            "application/json",
            r#"{"info": {"summary": "Backported types", "requires_python": ">=3.9", "home_page": "", "project_urls": {"Source": "https://example.com/te"}}}"#,
        ),
        _ => response("404 Not Found", "text/plain", ""),
    })?;
    let client = RealIndexClient::with_pypi_json(&format!("{url}/pypi"));
    // Asked for by its normalized name.
    let info = client.pypi_project("Typing_Extensions").await?;
    assert_eq!(info.summary.as_deref(), Some("Backported types"));
    assert_eq!(info.homepage.as_deref(), Some("https://example.com/te"));
    assert!(matches!(
        client.pypi_project("nosuch").await,
        Err(IndexError::NotFound)
    ));
    Ok(())
}

#[tokio::test]
async fn the_index_asked_is_named_without_its_credentials() -> Result<(), Box<dyn Error>> {
    let pyproject = scratch_pyproject("credentials")?;
    let corp = "https://jane:s3cret@corp.example/simple";
    let settings = UvSettings {
        env: vec![("UV_DEFAULT_INDEX", corp.into())],
        files: Vec::new(),
    };
    let six = r#"{"files": [{"filename": "six-1.17.0-py2.py3-none-any.whl"}]}"#;
    let index = Pages {
        pages: vec![(format!("{corp}/six/"), json(six))],
        fails: false,
    };
    // The From row, and what says the package is not there.
    let listed = list_versions(&index, &settings, &pyproject, "six").await?;
    assert_eq!(listed.index, "https://****@corp.example/simple");
    let missing = list_versions(&index, &settings, &pyproject, "nosuch").await;
    assert_eq!(
        missing,
        Err("nosuch is not on https://****@corp.example/simple".into())
    );
    std::fs::remove_dir_all(pyproject.parent().ok_or("no parent")?)?;
    Ok(())
}

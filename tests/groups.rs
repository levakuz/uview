use std::error::Error;
use std::path::PathBuf;
use std::sync::Mutex;

use uview::controllers::groups::{declared_empty, delete_group, without_empty_group};
use uview::uv::{UvClient, UvError};

const PYPROJECT: &str = r#"[project]
name = "demo"
dependencies = []

# Tools for the checks.
[dependency-groups]
dev = [
    "pytest>=8",
]
lint = []
"#;

#[test]
fn the_empty_groups_are_found() {
    assert_eq!(declared_empty(PYPROJECT), ["lint"]);
    assert!(declared_empty("[project]\nname = \"demo\"\n").is_empty());
}

#[test]
fn an_empty_group_is_deleted_and_the_rest_is_kept_as_written() {
    let after = without_empty_group(PYPROJECT, "lint");
    let expected = PYPROJECT.replace("lint = []\n", "");
    assert_eq!(after.as_deref(), Some(expected.as_str()));
}

#[test]
fn the_last_group_takes_its_table_with_it() {
    let only_empty = "[project]\nname = \"demo\"\n\n[dependency-groups]\nlint = []\n";
    assert_eq!(
        without_empty_group(only_empty, "lint").as_deref(),
        Some("[project]\nname = \"demo\"\n")
    );
}

#[test]
fn a_group_with_packages_or_named_elsewhere_is_not_deleted() {
    assert_eq!(without_empty_group(PYPROJECT, "dev"), None);
    assert_eq!(without_empty_group(PYPROJECT, "test"), None);
    // uv will not sync with a default group that is not defined.
    let default = format!("{PYPROJECT}\n[tool.uv]\ndefault-groups = [\"lint\"]\n");
    assert_eq!(without_empty_group(&default, "lint"), None);
    let included = PYPROJECT.replace(
        "    \"pytest>=8\",\n",
        "    \"pytest>=8\",\n    { include-group = \"lint\" },\n",
    );
    assert_eq!(without_empty_group(&included, "lint"), None);
}

/// Answers every call, failing if `fails`, and records what it was asked to run.
struct Lock {
    fails: bool,
    calls: Mutex<Vec<String>>,
}

impl UvClient for Lock {
    async fn run(&self, args: &[&str]) -> Result<String, UvError> {
        if let Ok(mut calls) = self.calls.lock() {
            calls.push(args.join(" "));
        }
        if self.fails {
            Err(UvError::Parse("no".into()))
        } else {
            Ok(String::new())
        }
    }
}

/// A pyproject.toml of its own under the system's temporary directory.
fn scratch(test: &str) -> Result<PathBuf, Box<dyn Error>> {
    let dir = std::env::temp_dir().join(format!("uview-{test}-{}", std::process::id()));
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("pyproject.toml");
    std::fs::write(&path, PYPROJECT)?;
    Ok(path)
}

#[tokio::test]
async fn deleting_a_group_relocks() -> Result<(), Box<dyn Error>> {
    let path = scratch("relocks")?;
    let uv = Lock {
        fails: false,
        calls: Mutex::default(),
    };
    delete_group(&uv, &path, "lint").await?;
    assert!(!std::fs::read_to_string(&path)?.contains("lint"));
    assert_eq!(*uv.calls.lock().map_err(|e| e.to_string())?, ["lock"]);
    std::fs::remove_dir_all(path.parent().ok_or("no parent")?)?;
    Ok(())
}

#[tokio::test]
async fn a_failed_lock_puts_the_group_back() -> Result<(), Box<dyn Error>> {
    let path = scratch("restores")?;
    let uv = Lock {
        fails: true,
        calls: Mutex::default(),
    };
    assert!(delete_group(&uv, &path, "lint").await.is_err());
    assert_eq!(std::fs::read_to_string(&path)?, PYPROJECT);
    std::fs::remove_dir_all(path.parent().ok_or("no parent")?)?;
    Ok(())
}

const WITH_REFERENCES: &str = r#"[project]
name = "demo"
dependencies = []

[dependency-groups]
test = ["pytest>=8"]
dev = [{ include-group = "test" }, "ruff"]
docs = []

[tool.uv]
default-groups = ["docs"]
"#;

#[test]
fn the_groups_are_listed_in_file_order_with_what_names_them() {
    use uview::controllers::groups::declared_groups;
    use uview::domain::DeclaredGroup;
    let group = |name: &str, named_by: Option<&str>| DeclaredGroup {
        name: name.into(),
        named_by: named_by.map(String::from),
    };
    assert_eq!(
        declared_groups(WITH_REFERENCES),
        [
            group("test", Some("included by group dev")),
            group("dev", None),
            group("docs", Some("named in tool.uv.default-groups")),
        ]
    );
    assert!(declared_groups("[project]\nname = \"demo\"\n").is_empty());
}

#[test]
fn an_empty_group_is_added_and_the_rest_kept_as_written() -> Result<(), String> {
    use uview::controllers::groups::with_group;
    let after = with_group(PYPROJECT, "docs")?;
    assert_eq!(
        after,
        PYPROJECT.replace("lint = []\n", "lint = []\ndocs = []\n")
    );
    // Without a [dependency-groups] table, one is made.
    let bare = "[project]\nname = \"demo\"\n";
    assert_eq!(
        with_group(bare, "lint")?,
        format!("{bare}\n[dependency-groups]\nlint = []\n")
    );
    // A name uv would refuse, or one taken as uv compares them.
    for refused in ["my group", "-lint", "lint-", "", "Lint", "LINT"] {
        assert!(with_group(PYPROJECT, refused).is_err(), "{refused}");
    }
    Ok(())
}

#[tokio::test]
async fn creating_a_group_relocks_and_a_failed_lock_puts_the_file_back()
-> Result<(), Box<dyn Error>> {
    use uview::controllers::groups::create_group;
    let path = scratch("create")?;
    let ok = Lock {
        fails: false,
        calls: Mutex::default(),
    };
    create_group(&ok, &path, "docs").await?;
    assert!(std::fs::read_to_string(&path)?.contains("docs = []"));
    let failing = Lock {
        fails: true,
        calls: Mutex::default(),
    };
    let before = std::fs::read_to_string(&path)?;
    assert!(create_group(&failing, &path, "site").await.is_err());
    assert_eq!(std::fs::read_to_string(&path)?, before);
    std::fs::remove_dir_all(path.parent().ok_or("no parent")?)?;
    Ok(())
}

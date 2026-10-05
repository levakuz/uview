use std::error::Error;
use std::path::PathBuf;
use std::sync::Mutex;

use uview::controllers::groups::{delete_group, without_empty_group};
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

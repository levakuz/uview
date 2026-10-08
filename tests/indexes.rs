use std::error::Error;
use std::sync::Mutex;

use uview::controllers::indexes::{
    add_index, list_indexes, remove_index, with_index, without_index,
};
use uview::domain::{Index, IndexKind};
use uview::uv::{UvClient, UvError};

const BARE: &str = "[project]\nname = \"demo\"\ndependencies = []\n";

const WITH_INDEXES: &str = r#"[project]
name = "demo"
dependencies = ["six"]

# Mirrors.
[[tool.uv.index]]
url = "https://example.com/unnamed/simple/"

[[tool.uv.index]]
name = "corp"
url = "https://corp.example.com/simple/"
default = true

[[tool.uv.index]]
name = "pinned"
url = "https://example.com/pinned/simple/"
explicit = true

[tool.uv.sources]
six = [{ index = "pinned", marker = "sys_platform == 'linux'" }]
"#;

fn index(name: Option<&str>, url: &str, kind: IndexKind) -> Index {
    Index {
        name: name.map(String::from),
        url: url.into(),
        kind,
    }
}

#[test]
fn the_indexes_are_listed_in_file_order() {
    assert_eq!(
        list_indexes(WITH_INDEXES),
        [
            index(
                None,
                "https://example.com/unnamed/simple/",
                IndexKind::Extra
            ),
            index(
                Some("corp"),
                "https://corp.example.com/simple/",
                IndexKind::Default
            ),
            index(
                Some("pinned"),
                "https://example.com/pinned/simple/",
                IndexKind::Explicit
            ),
        ]
    );
    assert!(list_indexes(BARE).is_empty());
}

#[test]
fn an_extra_index_is_appended_and_the_rest_kept() -> Result<(), String> {
    let after = with_index(BARE, "torch", "https://download.pytorch.org/whl/cpu", false)?;
    assert_eq!(
        after,
        format!(
            "{BARE}\n[[tool.uv.index]]\nname = \"torch\"\nurl = \"https://download.pytorch.org/whl/cpu\"\n"
        )
    );
    Ok(())
}

#[test]
fn a_new_default_takes_over_from_the_old() -> Result<(), String> {
    let after = with_index(
        WITH_INDEXES,
        "mirror",
        "https://mirror.example.com/simple/",
        true,
    )?;
    let kinds: Vec<IndexKind> = list_indexes(&after).iter().map(|i| i.kind).collect();
    assert_eq!(
        kinds,
        [
            IndexKind::Extra,
            IndexKind::Extra,
            IndexKind::Explicit,
            IndexKind::Default
        ]
    );
    // Only corp's `default` line went; the comment and the sources are as written.
    assert!(after.contains("# Mirrors."));
    assert!(after.contains("six = [{ index = \"pinned\", marker = \"sys_platform == 'linux'\" }]"));
    Ok(())
}

#[test]
fn a_bad_index_is_refused_before_anything_is_written() {
    let refused = |name, url| with_index(WITH_INDEXES, name, url, false).is_err();
    assert!(refused("corp", "https://other.example.com/simple/"));
    assert!(refused("my index", "https://other.example.com/simple/"));
    assert!(refused("other", "other.example.com/simple"));
    assert!(refused(
        "other",
        "https://user:pw@other.example.com/simple/"
    ));
}

#[test]
fn an_index_is_removed_unless_a_package_is_pinned_to_it() -> Result<(), String> {
    let unnamed = index(
        None,
        "https://example.com/unnamed/simple/",
        IndexKind::Extra,
    );
    let after = without_index(WITH_INDEXES, &unnamed)?;
    // The comment right above the table goes with it; the rest is as written.
    assert_eq!(
        after,
        WITH_INDEXES.replace(
            "# Mirrors.\n[[tool.uv.index]]\nurl = \"https://example.com/unnamed/simple/\"\n\n",
            ""
        )
    );
    let pinned = index(
        Some("pinned"),
        "https://example.com/pinned/simple/",
        IndexKind::Explicit,
    );
    assert_eq!(
        without_index(WITH_INDEXES, &pinned),
        Err("six is pinned to pinned".into())
    );
    // Pinned as a table, not in a list.
    let as_table = WITH_INDEXES.replace(
        "six = [{ index = \"pinned\", marker = \"sys_platform == 'linux'\" }]",
        "six = { index = \"pinned\" }",
    );
    assert!(without_index(&as_table, &pinned).is_err());
    Ok(())
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
            Err(UvError::Failed {
                command: "lock".into(),
                status: std::process::ExitStatus::default(),
                stderr: "× No solution found when resolving dependencies".into(),
            })
        } else {
            Ok(String::new())
        }
    }
}

#[tokio::test]
async fn an_edit_relocks_and_a_failed_lock_puts_the_file_back() -> Result<(), Box<dyn Error>> {
    let dir = std::env::temp_dir().join(format!("uview-indexes-{}", std::process::id()));
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("pyproject.toml");
    std::fs::write(&path, WITH_INDEXES)?;
    let ok = Lock {
        fails: false,
        calls: Mutex::default(),
    };
    add_index(
        &ok,
        &path,
        "torch",
        "https://download.pytorch.org/whl/cpu",
        false,
    )
    .await?;
    assert_eq!(list_indexes(&std::fs::read_to_string(&path)?).len(), 4);
    assert_eq!(*ok.calls.lock().map_err(|e| e.to_string())?, ["lock"]);
    let failing = Lock {
        fails: true,
        calls: Mutex::default(),
    };
    let before = std::fs::read_to_string(&path)?;
    let unnamed = index(
        None,
        "https://example.com/unnamed/simple/",
        IndexKind::Extra,
    );
    // Said in full: the file is as it was, and uv's own words are in the log.
    let failed = remove_index(&failing, &path, &unnamed).await;
    assert_eq!(
        failed.map_err(|e| e.to_string()),
        Err(
            "pyproject.toml: left as it was, since uv could not lock the project with the \
             change; the uv log says why"
                .into()
        )
    );
    assert_eq!(std::fs::read_to_string(&path)?, before);
    // A refused edit runs nothing.
    let pinned = index(
        Some("pinned"),
        "https://example.com/pinned/simple/",
        IndexKind::Explicit,
    );
    assert!(remove_index(&ok, &path, &pinned).await.is_err());
    assert_eq!(ok.calls.lock().map_err(|e| e.to_string())?.len(), 1);
    std::fs::remove_dir_all(&dir)?;
    Ok(())
}

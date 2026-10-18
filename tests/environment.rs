use std::error::Error;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crossterm::event::{KeyCode, KeyEvent};
use uview::controllers::environment::{
    apparent_size, delete_env, list_pythons, load_env_info, python_version, recreate,
    refusal_to_delete, relative_to, reset,
};
use uview::controllers::project::ProjectPaths;
use uview::domain::{EnvInfo, PythonOption};
use uview::message::{Command, Input};
use uview::state::{AppState, Loadable};
use uview::ui::Component;
use uview::ui::keymap::{self, Mode};
use uview::ui::pages::dependencies::DependenciesPage;
use uview::uv::{UvClient, UvError, parse};

#[test]
fn the_environment_is_shown_from_the_projects_root() {
    let root = Path::new("/home/jane/work/api");
    assert_eq!(relative_to(&root.join(".venv"), root), ".venv");
    assert_eq!(
        relative_to(Path::new("/home/jane/.venv"), root),
        "../../.venv"
    );
    assert_eq!(
        relative_to(Path::new("/opt/envs/api"), root),
        "../../../../opt/envs/api"
    );
    assert_eq!(relative_to(root, root), ".");
}

#[test]
fn the_python_is_read_from_pyvenv_cfg() {
    let uv = "home = /usr/bin\nimplementation = CPython\nversion_info = 3.11.15\nuv = 0.8.9\n";
    assert_eq!(python_version(uv).as_deref(), Some("3.11.15"));
    // What `python -m venv` writes.
    assert_eq!(
        python_version("home = /usr/bin\nversion = 3.12.4\n").as_deref(),
        Some("3.12.4")
    );
    assert_eq!(python_version("home = /usr/bin\n"), None);
}

/// A directory of its own under the system's temporary directory, empty.
fn scratch(test: &str) -> Result<PathBuf, Box<dyn Error>> {
    let dir = std::env::temp_dir().join(format!("uview-env-{test}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// An environment at `root/.venv`, holding `pyvenv.cfg` and a 1000-byte package.
fn venv(root: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let env = root.join(".venv");
    let package = env.join("lib/python3.11/site-packages/six");
    std::fs::create_dir_all(&package)?;
    std::fs::write(env.join("pyvenv.cfg"), "version_info = 3.11.15\n")?;
    std::fs::write(package.join("six.py"), vec![b'x'; 1000])?;
    Ok(env)
}

#[test]
fn the_size_counts_files_and_not_what_symlinks_point_to() -> Result<(), Box<dyn Error>> {
    let root = scratch("size")?;
    let env = venv(&root)?;
    let cfg = std::fs::metadata(env.join("pyvenv.cfg"))?.len();
    #[cfg(unix)]
    {
        let big = root.join("python");
        std::fs::write(&big, vec![b'x'; 100_000])?;
        std::fs::create_dir_all(env.join("bin"))?;
        std::os::unix::fs::symlink(&big, env.join("bin/python"))?;
        std::os::unix::fs::symlink(&root, env.join("bin/loop"))?;
    }
    assert_eq!(apparent_size(&env)?, 1000 + cfg);
    std::fs::remove_dir_all(&root)?;
    Ok(())
}

#[tokio::test]
async fn the_tile_learns_the_path_the_python_and_the_size() -> Result<(), Box<dyn Error>> {
    let root = scratch("info")?;
    let paths = ProjectPaths::at(root.clone(), None);
    assert_eq!(load_env_info(&paths).await, None);
    // A directory that is no environment is not measured.
    std::fs::create_dir_all(root.join(".venv"))?;
    assert_eq!(load_env_info(&paths).await.and_then(|info| info.size), None);
    let env = venv(&root)?;
    let cfg = std::fs::metadata(env.join("pyvenv.cfg"))?.len();
    assert_eq!(
        load_env_info(&paths).await,
        Some(EnvInfo {
            shown_path: ".venv".into(),
            python: Some("3.11.15".into()),
            size: Some(1000 + cfg),
        })
    );
    std::fs::remove_dir_all(&root)?;
    Ok(())
}

#[tokio::test]
async fn only_an_environment_outside_the_project_root_is_deleted() -> Result<(), Box<dyn Error>> {
    let root = scratch("delete")?;
    std::fs::write(root.join("pyproject.toml"), "[project]\nname = \"demo\"\n")?;
    // No pyvenv.cfg: not an environment, whatever UV_PROJECT_ENVIRONMENT names.
    let plain = root.join("src");
    std::fs::create_dir_all(&plain)?;
    assert!(refusal_to_delete(&plain, &root).is_some());
    assert!(delete_env(&plain, &root).await.is_err());
    assert!(plain.is_dir());
    // A pyvenv.cfg in the root itself, or above it, does not make the project deletable.
    std::fs::write(root.join("pyvenv.cfg"), "version_info = 3.11.15\n")?;
    assert!(refusal_to_delete(&root, &root).is_some());
    assert!(refusal_to_delete(&root, &plain).is_some());
    let env = venv(&root)?;
    assert_eq!(refusal_to_delete(&env, &root), None);
    delete_env(&env, &root).await?;
    assert!(!env.exists());
    assert!(root.join("pyproject.toml").is_file());
    std::fs::remove_dir_all(&root)?;
    Ok(())
}

/// Answers every call, and records what it was asked to run.
#[derive(Default)]
struct Recorder {
    calls: Mutex<Vec<String>>,
}

impl UvClient for Recorder {
    async fn run(&self, args: &[&str]) -> Result<String, UvError> {
        if let Ok(mut calls) = self.calls.lock() {
            calls.push(args.join(" "));
        }
        Ok(String::new())
    }
}

fn python(key: &str, version: &str, implementation: &str, variant: &str) -> PythonOption {
    PythonOption {
        key: key.into(),
        version: version.into(),
        implementation: implementation.into(),
        variant: variant.into(),
        path: None,
    }
}

#[test]
fn a_python_is_pinned_as_uv_python_pin_takes_it() {
    let cpython = python("k", "3.12.11", "cpython", "default");
    assert_eq!(cpython.request(), "3.12.11");
    assert_eq!(
        python("k", "3.13.6", "cpython", "freethreaded").request(),
        "3.13.6t"
    );
    assert_eq!(
        python("k", "3.10.16", "pypy", "default").request(),
        "pypy@3.10.16"
    );
    // Older uv says no variant.
    assert_eq!(python("k", "3.12.11", "cpython", "").request(), "3.12.11");
}

#[tokio::test]
async fn recreating_syncs_with_the_python_and_resetting_clears_first() -> Result<(), Box<dyn Error>>
{
    let root = scratch("recreate")?;
    let uv = Recorder::default();
    let chosen = python(
        "cpython-3.12.11-macos-aarch64-none",
        "3.12.11",
        "cpython",
        "default",
    );
    assert!(!recreate(&uv, &chosen, &root).await?);
    reset(&uv, Path::new("/home/jane/api/.venv")).await?;
    // A .python-version would bring the old Python back on the next sync.
    std::fs::write(root.join(".python-version"), "3.11\n")?;
    assert!(recreate(&uv, &chosen, &root).await?);
    assert_eq!(
        *uv.calls.lock().map_err(|e| e.to_string())?,
        [
            "sync --python cpython-3.12.11-macos-aarch64-none",
            "venv --clear /home/jane/api/.venv",
            "sync",
            "sync --python cpython-3.12.11-macos-aarch64-none",
            "python pin 3.12.11",
        ]
    );
    std::fs::remove_dir_all(&root)?;
    Ok(())
}

/// Answers `uv python list` with the fixture, each Python twice, as under two paths.
struct Twice;

impl UvClient for Twice {
    async fn run(&self, _: &[&str]) -> Result<String, UvError> {
        let mut pythons: Vec<serde_json::Value> =
            serde_json::from_str(include_str!("fixtures/uv/python_list.json"))
                .map_err(|e| UvError::Parse(e.to_string()))?;
        pythons.extend(pythons.clone());
        Ok(serde_json::Value::Array(pythons).to_string())
    }
}

#[tokio::test]
async fn a_python_reached_by_several_paths_is_listed_once() -> Result<(), Box<dyn Error>> {
    assert_eq!(list_pythons(&Twice).await?.len(), 4);
    Ok(())
}

#[test]
fn uvs_python_list_is_read() -> Result<(), Box<dyn Error>> {
    let pythons = parse::python_list(include_str!("fixtures/uv/python_list.json"))?;
    let read: Vec<(&str, &str, bool)> = pythons
        .iter()
        .map(|p| (p.version.as_str(), p.variant.as_str(), p.path.is_some()))
        .collect();
    assert_eq!(
        read,
        [
            ("3.13.6", "default", false),
            ("3.13.6", "freethreaded", false),
            ("3.12.11", "default", true),
            ("3.11.15", "default", true),
        ]
    );
    assert!(parse::python_list("error: no").is_err());
    Ok(())
}

/// The page with an environment at `.venv`, or none, as the app holds it.
struct Screen {
    page: DependenciesPage,
    state: AppState,
}

impl Screen {
    fn new(environment: Option<EnvInfo>) -> Self {
        let mut state = AppState::default();
        state.project.environment = Loadable::Loaded(environment);
        Self {
            page: DependenciesPage::new(),
            state,
        }
    }

    fn with_venv() -> Self {
        Self::new(Some(EnvInfo {
            shown_path: ".venv".into(),
            python: Some("3.11.15".into()),
            size: None,
        }))
    }

    fn press(&mut self, code: KeyCode) -> Vec<Command> {
        let input = Input::Key(KeyEvent::from(code));
        match keymap::action_for(self.page.mode(), &input) {
            Some(action) => self.page.update(&action, &self.state),
            None => Vec::new(),
        }
    }
}

#[test]
fn e_lists_the_pythons_and_makes_the_environment_with_the_one_chosen() -> Result<(), Box<dyn Error>>
{
    let mut screen = Screen::with_venv();
    assert!(screen.press(KeyCode::Char('e')).is_empty());
    assert_eq!(screen.page.mode(), Mode::Actions);
    assert_eq!(screen.press(KeyCode::Enter), [Command::ListPythons]);
    let pythons = parse::python_list(include_str!("fixtures/uv/python_list.json"))?;
    screen.state.project.pythons = Loadable::Loaded(pythons.clone());
    screen.press(KeyCode::Down);
    screen.press(KeyCode::Down);
    assert_eq!(
        screen.press(KeyCode::Enter),
        [Command::RecreateEnv {
            python: pythons[2].clone()
        }]
    );
    assert_eq!(screen.page.mode(), Mode::Table);
    Ok(())
}

#[test]
fn a_python_list_still_coming_or_failed_chooses_nothing() {
    let mut screen = Screen::with_venv();
    screen.press(KeyCode::Char('e'));
    screen.press(KeyCode::Enter);
    screen.state.project.pythons = Loadable::NotLoaded;
    assert!(screen.press(KeyCode::Enter).is_empty());
    screen.state.project.pythons = Loadable::Failed("error: broken python".into());
    assert!(screen.press(KeyCode::Enter).is_empty());
    screen.press(KeyCode::Esc);
    assert_eq!(screen.page.mode(), Mode::Table);
}

#[test]
fn e_empties_the_environment_or_deletes_it_once_asked() {
    let mut screen = Screen::with_venv();
    screen.press(KeyCode::Char('e'));
    screen.press(KeyCode::Down);
    assert_eq!(screen.press(KeyCode::Enter), [Command::ResetEnv]);

    screen.press(KeyCode::Char('e'));
    screen.press(KeyCode::Down);
    screen.press(KeyCode::Down);
    assert!(screen.press(KeyCode::Enter).is_empty());
    assert_eq!(screen.press(KeyCode::Enter), [Command::DeleteEnv]);

    // Keep it, or Esc, deletes nothing.
    screen.press(KeyCode::Char('e'));
    screen.press(KeyCode::Down);
    screen.press(KeyCode::Down);
    screen.press(KeyCode::Enter);
    screen.press(KeyCode::Down);
    assert!(screen.press(KeyCode::Enter).is_empty());
    assert_eq!(screen.page.mode(), Mode::Table);
}

#[test]
fn without_an_environment_e_only_makes_one() {
    let mut screen = Screen::new(None);
    screen.press(KeyCode::Char('e'));
    screen.press(KeyCode::Down);
    screen.press(KeyCode::Down);
    assert_eq!(screen.press(KeyCode::Enter), [Command::ListPythons]);
}

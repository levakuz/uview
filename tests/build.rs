use std::error::Error;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{Terminal, backend::TestBackend};
use uview::controllers::build::{build, built_notice, built_since, dist_dir, is_virtual_root};
use uview::controllers::project::ProjectPaths;
use uview::domain::Artifact;
use uview::message::{Command, Input};
use uview::state::{AppState, Loadable};
use uview::ui::Component;
use uview::ui::keymap::{self, Mode};
use uview::ui::pages::dependencies::DependenciesPage;
use uview::uv::{UvClient, UvError};

/// A directory of its own under the system's temporary directory, empty.
fn scratch(test: &str) -> Result<PathBuf, Box<dyn Error>> {
    let dir = std::env::temp_dir().join(format!("uview-build-{test}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

#[test]
fn only_a_root_without_a_project_builds_every_member() {
    assert!(!is_virtual_root("[project]\nname = \"demo\"\n"));
    assert!(!is_virtual_root(
        "[project]\nname = \"demo\"\n[tool.uv.workspace]\nmembers = [\"packages/*\"]\n"
    ));
    assert!(is_virtual_root(
        "[tool.uv.workspace]\nmembers = [\"packages/*\"]\n"
    ));
    // uv reports a file it cannot read; plain `uv build` lets it.
    assert!(!is_virtual_root("[project"));
}

#[test]
fn a_members_dist_is_its_workspace_roots() -> Result<(), Box<dyn Error>> {
    let root = scratch("dist")?;
    std::fs::write(
        root.join("pyproject.toml"),
        "[tool.uv.workspace]\nmembers = [\"packages/*\"]\n",
    )?;
    let member = root.join("packages/a");
    std::fs::create_dir_all(&member)?;
    std::fs::write(member.join("pyproject.toml"), "[project]\nname = \"a\"\n")?;
    assert_eq!(dist_dir(&member), root.join("dist"));
    assert_eq!(dist_dir(&root), root.join("dist"));
    std::fs::remove_dir_all(&root)?;
    Ok(())
}

#[test]
fn only_files_written_since_the_build_began_are_its() -> Result<(), Box<dyn Error>> {
    let dist = scratch("since")?;
    let since = SystemTime::now() - Duration::from_secs(60);
    let old = dist.join("demo-0.1.0.tar.gz");
    std::fs::write(&old, "old")?;
    std::fs::File::options()
        .write(true)
        .open(&old)?
        .set_modified(since - Duration::from_secs(3600))?;
    std::fs::write(dist.join("demo-0.2.0.tar.gz"), "sdist")?;
    std::fs::write(dist.join("demo-0.2.0-py3-none-any.whl"), "wheel!")?;
    std::fs::create_dir_all(dist.join("subdir"))?;
    // What uv writes to a dist/ it makes.
    std::fs::write(dist.join(".gitignore"), "*")?;
    assert_eq!(
        built_since(&dist, since),
        [
            Artifact {
                path: dist.join("demo-0.2.0-py3-none-any.whl"),
                size: 6
            },
            Artifact {
                path: dist.join("demo-0.2.0.tar.gz"),
                size: 5
            },
        ]
    );
    assert!(built_since(&dist.join("missing"), since).is_empty());
    std::fs::remove_dir_all(&dist)?;
    Ok(())
}

/// Writes a wheel to `dist` when asked to build, as uv would, and records what it ran.
struct Builder {
    dist: PathBuf,
    calls: Mutex<Vec<String>>,
}

impl UvClient for Builder {
    async fn run(&self, args: &[&str]) -> Result<String, UvError> {
        if let Ok(mut calls) = self.calls.lock() {
            calls.push(args.join(" "));
        }
        std::fs::create_dir_all(&self.dist).map_err(|e| UvError::Parse(e.to_string()))?;
        std::fs::write(self.dist.join("demo-0.1.0-py3-none-any.whl"), "wheel")
            .map_err(|e| UvError::Parse(e.to_string()))?;
        Ok(String::new())
    }
}

#[tokio::test]
async fn building_returns_what_uv_wrote_and_a_virtual_root_builds_all() -> Result<(), Box<dyn Error>>
{
    let root = scratch("build")?;
    std::fs::write(root.join("pyproject.toml"), "[project]\nname = \"demo\"\n")?;
    let uv = Builder {
        dist: root.join("dist"),
        calls: Mutex::default(),
    };
    let paths = ProjectPaths::at(root.clone(), None);
    let built = build(&uv, &paths).await?;
    assert_eq!(
        built,
        [Artifact {
            path: root.join("dist/demo-0.1.0-py3-none-any.whl"),
            size: 5
        }]
    );
    std::fs::write(
        root.join("pyproject.toml"),
        "[tool.uv.workspace]\nmembers = [\"packages/*\"]\n",
    )?;
    build(&uv, &paths).await?;
    assert_eq!(
        *uv.calls.lock().map_err(|e| e.to_string())?,
        ["build", "build --all-packages"]
    );
    std::fs::remove_dir_all(&root)?;
    Ok(())
}

fn artifact(name: &str, size: u64) -> Artifact {
    Artifact {
        path: Path::new("/home/jane/api/dist").join(name),
        size,
    }
}

#[test]
fn the_notice_names_two_files_and_counts_more() {
    let sdist = artifact("demo-0.1.0.tar.gz", 3);
    let wheel = artifact("demo-0.1.0-py3-none-any.whl", 2);
    assert_eq!(
        built_notice(&[sdist.clone(), wheel.clone()], "dist"),
        "Built demo-0.1.0.tar.gz, demo-0.1.0-py3-none-any.whl in dist/"
    );
    assert_eq!(
        built_notice(&[sdist.clone(), wheel, sdist], "../../dist"),
        "Built 3 files in ../../dist/"
    );
    assert_eq!(
        built_notice(&[], "dist"),
        "uv build wrote nothing new to dist/"
    );
}

/// The page as the app holds it.
struct Screen {
    page: DependenciesPage,
    state: AppState,
}

impl Screen {
    fn new() -> Self {
        let mut state = AppState::default();
        state.deps.pyproject = Loadable::Loaded(Vec::new());
        Self {
            page: DependenciesPage::new(),
            state,
        }
    }

    fn press(&mut self, code: KeyCode) -> Vec<Command> {
        let input = Input::Key(KeyEvent::from(code));
        match keymap::action_for(self.page.mode(), &input) {
            Some(action) => self.page.update(&action, &self.state),
            None => Vec::new(),
        }
    }

    fn drawn(&mut self) -> Result<String, Box<dyn Error>> {
        let mut terminal = Terminal::new(TestBackend::new(60, 12))?;
        terminal.draw(|frame| self.page.render(frame, frame.area(), &self.state))?;
        Ok(terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect())
    }

    /// A build that wrote `built`, landed as the app writes it.
    fn built(&mut self, built: Vec<Artifact>) {
        self.state.project.built = built;
        self.state.project.builds += 1;
    }
}

#[test]
fn b_builds_at_once_in_the_table_and_the_tree() {
    let mut screen = Screen::new();
    assert_eq!(screen.press(KeyCode::Char('B')), [Command::Build]);
    screen.press(KeyCode::Tab);
    screen.press(KeyCode::Tab);
    assert_eq!(screen.page.mode(), Mode::Tree);
    assert_eq!(screen.press(KeyCode::Char('B')), [Command::Build]);
}

#[test]
fn a_build_that_lands_lists_its_files_once() -> Result<(), Box<dyn Error>> {
    let mut screen = Screen::new();
    screen.drawn()?;
    assert_eq!(screen.page.mode(), Mode::Table);
    screen.built(vec![
        artifact("demo-0.1.0-py3-none-any.whl", 2048),
        artifact("demo-0.1.0.tar.gz", 3174),
    ]);
    let drawn = screen.drawn()?;
    assert_eq!(screen.page.mode(), Mode::Actions);
    assert!(drawn.contains("demo-0.1.0-py3-none-any.whl  2.0 KiB"));
    assert!(drawn.contains("demo-0.1.0.tar.gz            3.1 KiB"));
    assert!(screen.press(KeyCode::Esc).is_empty());
    screen.drawn()?;
    assert_eq!(screen.page.mode(), Mode::Table);
    Ok(())
}

#[test]
fn a_build_waits_for_an_open_popup_to_close() -> Result<(), Box<dyn Error>> {
    let mut screen = Screen::new();
    screen.press(KeyCode::Char('e'));
    screen.built(Vec::new());
    screen.drawn()?;
    // Esc closes the environment's popup, not the build's.
    screen.press(KeyCode::Esc);
    assert_eq!(screen.page.mode(), Mode::Table);
    assert!(screen.drawn()?.contains("uv wrote no new file to dist/"));
    screen.press(KeyCode::Enter);
    assert_eq!(screen.page.mode(), Mode::Table);
    Ok(())
}

use std::error::Error;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{Terminal, backend::TestBackend};
use uview::controllers::indexes::list_indexes;
use uview::controllers::publish::{credentials, publish, published_notice};
use uview::domain::{Artifact, Index, IndexKind, PublishTarget};
use uview::message::{Command, Input};
use uview::state::{AppState, Loadable};
use uview::ui::Component;
use uview::ui::keymap::{self, Mode};
use uview::ui::pages::dependencies::DependenciesPage;
use uview::uv::args::publish_args;
use uview::uv::{UvClient, UvError, scrub};

#[test]
fn credentials_are_named_by_where_uv_takes_them_from() {
    let env = |pairs: &'static [(&'static str, &'static str)]| {
        move |key: &str| {
            pairs
                .iter()
                .find(|(k, _)| *k == key)
                .map(|(_, v)| (*v).to_string())
        }
    };
    assert_eq!(
        credentials(env(&[
            ("UV_PUBLISH_TOKEN", "pypi-x"),
            ("UV_PUBLISH_PASSWORD", "p")
        ])),
        Some("UV_PUBLISH_TOKEN")
    );
    assert_eq!(
        credentials(env(&[
            ("UV_PUBLISH_TOKEN", ""),
            ("UV_PUBLISH_PASSWORD", "p")
        ])),
        Some("UV_PUBLISH_PASSWORD")
    );
    assert_eq!(
        credentials(env(&[("UV_KEYRING_PROVIDER", "subprocess")])),
        Some("keyring")
    );
    assert_eq!(
        credentials(env(&[("UV_KEYRING_PROVIDER", "disabled")])),
        None
    );
    assert_eq!(credentials(env(&[])), None);
}

#[test]
fn a_credential_never_reaches_a_line() {
    let secrets = vec!["pypi-AgEIcHlwaS5vcmc".to_string()];
    assert_eq!(
        scrub("Authorization: pypi-AgEIcHlwaS5vcmc rejected", &secrets),
        "Authorization: **** rejected"
    );
    assert_eq!(scrub("Uploading demo.whl", &[]), "Uploading demo.whl");
}

fn corp(publish_url: Option<&str>) -> Index {
    Index {
        name: Some("corp".into()),
        url: "https://corp.example.com/simple/".into(),
        kind: IndexKind::Explicit,
        publish_url: publish_url.map(String::from),
    }
}

#[test]
fn an_index_with_a_publish_url_is_a_target() {
    let indexes = list_indexes(
        r#"[[tool.uv.index]]
name = "corp"
url = "https://corp.example.com/simple/"
publish-url = "https://corp.example.com/upload/"
explicit = true

[[tool.uv.index]]
name = "mirror"
url = "https://mirror.example.com/simple/"
"#,
    );
    assert_eq!(
        indexes[0].publish_url.as_deref(),
        Some("https://corp.example.com/upload/")
    );
    assert_eq!(indexes[1].publish_url, None);
    assert_eq!(
        PublishTarget::all(&indexes),
        [
            PublishTarget::PyPi,
            PublishTarget::TestPyPi,
            PublishTarget::Index {
                name: "corp".into(),
                publish_url: "https://corp.example.com/upload/".into()
            },
        ]
    );
}

#[test]
fn each_target_is_told_to_uv_its_own_way() {
    let files = [PathBuf::from("/home/jane/api/dist/demo-0.1.0.tar.gz")];
    let corp = PublishTarget::Index {
        name: "corp".into(),
        publish_url: "https://corp.example.com/upload/".into(),
    };
    assert_eq!(
        publish_args(&PublishTarget::PyPi, &files),
        ["publish", "/home/jane/api/dist/demo-0.1.0.tar.gz"]
    );
    assert_eq!(
        publish_args(&PublishTarget::TestPyPi, &files),
        [
            "publish",
            "--publish-url",
            "https://test.pypi.org/legacy/",
            "/home/jane/api/dist/demo-0.1.0.tar.gz"
        ]
    );
    assert_eq!(
        publish_args(&corp, &files),
        [
            "publish",
            "--index",
            "corp",
            "/home/jane/api/dist/demo-0.1.0.tar.gz"
        ]
    );
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

fn artifact(name: &str) -> Artifact {
    Artifact {
        path: Path::new("/home/jane/api/dist").join(name),
        size: 2048,
    }
}

#[tokio::test]
async fn exactly_the_built_files_are_published() -> Result<(), Box<dyn Error>> {
    let uv = Recorder::default();
    let files = [
        artifact("demo-0.1.0.tar.gz"),
        artifact("demo-0.1.0-py3-none-any.whl"),
    ];
    publish(&uv, &PublishTarget::TestPyPi, &files).await?;
    assert_eq!(
        *uv.calls.lock().map_err(|e| e.to_string())?,
        ["publish --publish-url https://test.pypi.org/legacy/ \
          /home/jane/api/dist/demo-0.1.0.tar.gz /home/jane/api/dist/demo-0.1.0-py3-none-any.whl"]
    );
    assert_eq!(
        published_notice(&files, &PublishTarget::TestPyPi),
        "Published demo-0.1.0.tar.gz, demo-0.1.0-py3-none-any.whl to TestPyPI"
    );
    assert_eq!(
        published_notice(
            &[files[0].clone(), files[1].clone(), files[0].clone()],
            &PublishTarget::PyPi
        ),
        "Published 3 files to PyPI"
    );
    Ok(())
}

/// The page as the app holds it, with a corp index that uploads.
struct Screen {
    page: DependenciesPage,
    state: AppState,
}

impl Screen {
    fn new() -> Self {
        let mut state = AppState::default();
        state.deps.pyproject = Loadable::Loaded(Vec::new());
        state.deps.indexes = vec![corp(Some("https://corp.example.com/upload/")), corp(None)];
        state.project.credentials = Some("UV_PUBLISH_TOKEN");
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
        let mut terminal = Terminal::new(TestBackend::new(90, 14))?;
        terminal.draw(|frame| self.page.render(frame, frame.area(), &self.state))?;
        Ok(terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect())
    }

    /// A build that wrote an sdist and a wheel, landed and shown.
    fn built(&mut self) -> Result<String, Box<dyn Error>> {
        self.state.project.built = vec![
            artifact("demo-0.1.0-py3-none-any.whl"),
            artifact("demo-0.1.0.tar.gz"),
        ];
        self.state.project.builds += 1;
        self.drawn()
    }
}

#[test]
fn publish_after_a_build_asks_where_then_publishes_the_built_files() -> Result<(), Box<dyn Error>> {
    let mut screen = Screen::new();
    assert!(screen.built()?.contains("Publish…"));
    // Publish…, then where to.
    assert!(screen.press(KeyCode::Enter).is_empty());
    let drawn = screen.drawn()?;
    assert!(drawn.contains("PyPI      https://upload.pypi.org/legacy/"));
    assert!(drawn.contains("TestPyPI  https://test.pypi.org/legacy/"));
    assert!(drawn.contains("corp      https://corp.example.com/upload/"));
    screen.press(KeyCode::Down);
    assert!(screen.press(KeyCode::Enter).is_empty());
    let drawn = screen.drawn()?;
    assert!(drawn.contains("Publish to TestPyPI · https://test.pypi.org/legacy/"));
    assert!(drawn.contains("demo-0.1.0.tar.gz            2.0 KiB"));
    assert!(drawn.contains("Credentials from UV_PUBLISH_TOKEN"));
    // Publish is highlighted.
    assert_eq!(
        screen.press(KeyCode::Enter),
        [Command::Publish {
            target: PublishTarget::TestPyPi,
            files: screen.state.project.built.clone(),
        }]
    );
    assert_eq!(screen.page.mode(), Mode::Table);
    Ok(())
}

#[test]
fn cancel_or_a_file_row_publishes_nothing() -> Result<(), Box<dyn Error>> {
    let mut screen = Screen::new();
    screen.built()?;
    screen.press(KeyCode::Enter);
    screen.press(KeyCode::Enter);
    screen.drawn()?;
    // The credentials row, above Publish, is only to read.
    screen.press(KeyCode::Up);
    assert!(screen.press(KeyCode::Enter).is_empty());
    assert_eq!(screen.page.mode(), Mode::Actions);
    screen.press(KeyCode::Down);
    screen.press(KeyCode::Down);
    assert!(screen.press(KeyCode::Enter).is_empty());
    assert_eq!(screen.page.mode(), Mode::Table);
    Ok(())
}

#[test]
fn u_publishes_the_last_build_or_says_there_is_none() -> Result<(), Box<dyn Error>> {
    let mut screen = Screen::new();
    screen.state.project.credentials = None;
    screen.press(KeyCode::Char('U'));
    assert!(
        screen
            .drawn()?
            .contains("Nothing built yet: B builds the project first")
    );
    screen.press(KeyCode::Enter);
    assert_eq!(screen.page.mode(), Mode::Table);
    screen.built()?;
    screen.press(KeyCode::Esc);
    screen.press(KeyCode::Char('U'));
    screen.press(KeyCode::Enter);
    let drawn = screen.drawn()?;
    assert!(drawn.contains("Publish to PyPI · https://upload.pypi.org/legacy/"));
    assert!(drawn.contains("No credentials found: set UV_PUBLISH_TOKEN"));
    assert_eq!(
        screen.press(KeyCode::Enter),
        [Command::Publish {
            target: PublishTarget::PyPi,
            files: screen.state.project.built.clone(),
        }]
    );
    Ok(())
}

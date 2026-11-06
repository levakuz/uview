mod app;
mod config;
mod controllers;
mod domain;
mod index;
mod message;
mod runtime;
mod state;
mod ui;
mod uv;

use std::io::IsTerminal;
use std::path::{Path, PathBuf};

use color_eyre::Result;
use tokio_util::sync::CancellationToken;

use crate::config::Config;
use crate::ui::theme::{Theme, set_theme};
use crate::uv::{RealUvClient, UvClient, UvError};

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    let root = match project_root().await {
        Ok(Some(root)) => root,
        // Offered a project and declined it: there is nothing more to say.
        Ok(None) => std::process::exit(1),
        Err(message) => {
            eprintln!("uview: {message}");
            std::process::exit(1);
        }
    };
    // Every path from here on is the project's, handed on; uview's working directory is left
    // as it was started in (ADR-0011).
    let configured = std::env::var("UV_PROJECT_ENVIRONMENT").ok();
    let paths = controllers::project::ProjectPaths::at(root, configured.as_deref());
    let config = config::config_path(|key| std::env::var(key).ok(), cfg!(windows));
    let (theme, problem) = choose_theme(config.as_deref());
    set_theme(theme);
    let mut app = app::App::new(paths, config, problem);
    app.run().await
}

/// The theme to draw with, from the settings file at `path` and the terminal, and what is wrong
/// with the file, if anything: a file uview cannot use is reported, never fatal.
fn choose_theme(path: Option<&Path>) -> (Theme, Option<String>) {
    let name = path
        .and_then(Path::file_name)
        .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
    let (config, mut problems) = match path.map(std::fs::read_to_string) {
        Some(Ok(text)) => match config::parse(&text) {
            Ok(read) => read,
            Err(e) => (Config::default(), vec![e]),
        },
        // No file is the usual case.
        Some(Err(e)) if e.kind() == std::io::ErrorKind::NotFound => (Config::default(), Vec::new()),
        Some(Err(e)) => (Config::default(), vec![e.to_string()]),
        None => (Config::default(), Vec::new()),
    };
    let base = config.theme.map_or_else(background, config::Base::theme);
    let (theme, colors) = config::themed(base, &config.colors);
    problems.extend(colors);
    let problem = (!problems.is_empty()).then(|| format!("{name}: {}", problems.join("; ")));
    (theme, problem)
}

/// The built-in theme for the terminal's background, as the terminal tells it before the app
/// takes the screen over; dark when it does not answer.
fn background() -> Theme {
    use terminal_colorsaurus::{QueryOptions, ThemeMode, theme_mode};
    if !std::io::stdout().is_terminal() {
        return Theme::DARK;
    }
    match theme_mode(QueryOptions::default()) {
        Ok(ThemeMode::Light) => Theme::LIGHT,
        _ => Theme::DARK,
    }
}

/// The project uv would act on from here, or why there is none. With none at all, offers to
/// create one, and `None` means the offer was declined. Paths print with `{:?}`, so a directory
/// name cannot send escape sequences to the terminal.
async fn project_root() -> Result<Option<PathBuf>, String> {
    let cwd =
        std::env::current_dir().map_err(|e| format!("cannot read the current directory: {e}"))?;
    // uv starts from UV_PROJECT when it is set, and so does uview; RealUvClient then drops it.
    let start = match std::env::var_os("UV_PROJECT") {
        Some(project) if !project.is_empty() => cwd.join(project),
        _ => cwd,
    };
    let root = match controllers::project::find_project_root(&start) {
        Some(root) => root,
        None => match offer_init(&start).await? {
            Some(root) => root,
            None => return Ok(None),
        },
    };
    match controllers::project::refusal(&root) {
        Some(reason) => Err(reason),
        None => Ok(Some(root)),
    }
}

/// Offers to create a project in `dir`, which has none, with `uv init`, and returns its root.
/// The questions are asked on the terminal before the app takes it over; without one to ask
/// on, it says why there is no project instead.
async fn offer_init(dir: &Path) -> Result<Option<PathBuf>, String> {
    let missing = format!("no pyproject.toml in {dir:?} or any parent directory.");
    if !(std::io::stdin().is_terminal() && std::io::stderr().is_terminal()) {
        return Err(format!(
            "{missing}\nRun uview inside a Python project, or create one with `uv init`."
        ));
    }
    eprintln!("uview: {missing}");
    let create = ask(&format!("Create a uv project in {dir:?}? [y/N] "))
        .is_some_and(|answer| matches!(answer.to_lowercase().as_str(), "y" | "yes"));
    if !create {
        return Ok(None);
    }
    let default = dir.file_name().map_or(String::new(), |name| {
        format!(" [{}]", name.to_string_lossy())
    });
    let Some(name) = ask(&format!("Project name{default}: ")) else {
        return Ok(None);
    };
    let name = Some(name.as_str()).filter(|name| !name.is_empty());
    match controllers::project::init_project(&Echoing(RealUvClient::new(dir)), dir, name).await {
        Ok(()) => controllers::project::find_project_root(dir)
            .map(Some)
            .ok_or_else(|| format!("`uv init` left no pyproject.toml in {dir:?}.")),
        // uv has said why, just above.
        Err(UvError::Failed { .. }) => Err("no project was created.".into()),
        Err(e) => Err(e.to_string()),
    }
}

/// Asks `question` on stderr and returns the answer, trimmed, or `None` at the end of input.
fn ask(question: &str) -> Option<String> {
    eprint!("{question}");
    let mut answer = String::new();
    match std::io::stdin().read_line(&mut answer) {
        Ok(0) | Err(_) => None,
        Ok(_) => Some(answer.trim().to_string()),
    }
}

/// Runs `uv` as a shell would, before the app takes the terminal: each line it writes to stderr
/// is printed as it comes.
struct Echoing(RealUvClient);

impl UvClient for Echoing {
    async fn run(&self, args: &[&str]) -> Result<String, UvError> {
        let echo = |line: &str| eprintln!("{line}");
        self.0
            .run_streaming(args, &CancellationToken::new(), &echo)
            .await
    }
}

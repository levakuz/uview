//! Benchmarks, not checks: they print what they measure and assert nothing about it. Run them on
//! a release build, as docs/performance.md describes:
//!
//! ```text
//! cargo test --release --test performance -- --ignored --nocapture
//! ```

use std::error::Error;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use ratatui::{Terminal, backend::TestBackend};
use uview::controllers::project::ProjectPaths;
use uview::domain::{EnvDependency, Membership, PyprojectDependency};
use uview::index::RealIndexClient;
use uview::message::{Command, Event};
use uview::runtime::jobs::JobRunner;
use uview::state::{AppState, Loadable};
use uview::ui::Component;
use uview::ui::pages::dependencies::DependenciesPage;
use uview::uv::RealUvClient;

/// The page with `n` rows in each table.
fn state_with(n: usize) -> AppState {
    let mut state = AppState::default();
    state.deps.pyproject = Loadable::Loaded(
        (0..n)
            .map(|i| PyprojectDependency {
                name: format!("package-number-{i}"),
                groups: vec![Membership::Project],
                extras: Vec::new(),
                version_constraint: ">=1.2.3".into(),
            })
            .collect(),
    );
    state.deps.env = Loadable::Loaded(
        (0..n)
            .map(|i| EnvDependency {
                name: format!("package-number-{i}"),
                version: "1.2.3".into(),
            })
            .collect(),
    );
    state
}

#[test]
#[ignore = "a benchmark; see docs/performance.md"]
fn frame_time() -> Result<(), Box<dyn Error>> {
    for n in [200, 1000] {
        let state = state_with(n);
        let mut page = DependenciesPage::new();
        // The page's 60% of a 110x30 terminal, as the app lays it out.
        let mut terminal = Terminal::new(TestBackend::new(66, 28))?;
        let frames = 1000;
        let start = Instant::now();
        for _ in 0..frames {
            terminal.draw(|frame| page.render(frame, frame.area(), &state))?;
        }
        println!("{n:>5} rows: {:?} a frame", start.elapsed() / frames);
    }
    Ok(())
}

#[tokio::test]
#[ignore = "a benchmark; see docs/performance.md"]
async fn reload_time() -> Result<(), Box<dyn Error>> {
    let Some(root) = std::env::var_os("UVIEW_BENCH_PROJECT").map(PathBuf::from) else {
        println!("UVIEW_BENCH_PROJECT is not set: no project to reload");
        return Ok(());
    };
    for run in 1..=3 {
        let (events, mut received) = tokio::sync::mpsc::unbounded_channel();
        let mut runner = JobRunner::new(
            Arc::new(RealUvClient::new(&root)),
            Arc::new(RealIndexClient::new()),
            ProjectPaths::at(root.clone(), None),
            events,
        );
        let start = Instant::now();
        runner.execute(Command::Reload);
        let mut line = format!("reload {run}:");
        loop {
            let Some(event) =
                tokio::time::timeout(Duration::from_secs(60), received.recv()).await?
            else {
                break;
            };
            let what = match event {
                Event::PyprojectLoaded { .. } => "pyproject",
                Event::EnvLoaded { .. } => "environment",
                Event::ProjectLoaded { .. } => "version",
                Event::LockChecked { .. } => "lock",
                Event::TreeLoaded { .. } => "tree",
                Event::EnvInfoLoaded { .. } => "size",
                Event::JobFinished(_) => "done",
                _ => continue,
            };
            line += &format!(" {what} {}ms", start.elapsed().as_millis());
            if what == "done" {
                break;
            }
        }
        println!("{line}");
    }
    Ok(())
}

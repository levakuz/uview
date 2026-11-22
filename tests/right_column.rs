use std::error::Error;

use ratatui::{
    Terminal,
    backend::TestBackend,
    layout::Rect,
    style::{Color, Modifier},
};
use uview::domain::ProjectInfo;
use uview::message::{Event, JobId};
use uview::state::{JobsState, Loadable, ProjectState};
use uview::ui::components::features::project_tile::render_project_tile;
use uview::ui::components::features::uv_log::{render_uv_log, wrap};

fn rows(terminal: &Terminal<TestBackend>) -> Vec<String> {
    let buffer = terminal.backend().buffer();
    buffer
        .content()
        .chunks(usize::from(buffer.area.width))
        .map(|row| row.iter().map(|cell| cell.symbol()).collect())
        .collect()
}

fn project(info: Loadable<ProjectInfo>) -> ProjectState {
    let mut project = ProjectState::new("/home/jane/billing-api".into());
    project.info = info;
    project
}

#[test]
fn the_project_tile_wraps_the_root_and_leaves_the_rest_to_the_log() -> Result<(), Box<dyn Error>> {
    let mut terminal = Terminal::new(TestBackend::new(16, 9))?;
    let mut rest = Rect::default();
    let project = project(Loadable::NotLoaded);
    terminal.draw(|frame| rest = render_project_tile(frame, frame.area(), &project, None))?;
    // The environment's line is a dim … until a reload has looked.
    assert_eq!(
        rows(&terminal)[..5],
        [
            "┌Project───────┐",
            "│/home/jane/bil│",
            "│ling-api      │",
            "│…             │",
            "└──────────────┘",
        ]
    );
    assert_eq!(rest, Rect::new(0, 5, 16, 4));
    Ok(())
}

#[test]
fn the_project_tile_is_titled_with_the_projects_name_and_version() -> Result<(), Box<dyn Error>> {
    let mut terminal = Terminal::new(TestBackend::new(24, 5))?;
    let project = project(Loadable::Loaded(ProjectInfo {
        name: "billing-api".into(),
        version: "0.4.2".into(),
    }));
    terminal.draw(|frame| {
        render_project_tile(frame, frame.area(), &project, None);
    })?;
    assert_eq!(
        rows(&terminal)[..4],
        [
            "┌ billing-api 0.4.2 ───┐",
            "│/home/jane/billing-api│",
            "│…                     │",
            "└──────────────────────┘",
        ]
    );
    let buffer = terminal.backend().buffer();
    assert!(buffer[(2, 0)].modifier.contains(Modifier::BOLD));
    assert!(!buffer[(14, 0)].modifier.contains(Modifier::BOLD));
    assert_eq!(
        (buffer[(2, 0)].fg, buffer[(14, 0)].fg),
        (Color::Reset, Color::Reset)
    );
    Ok(())
}

#[test]
fn the_project_tile_says_what_uv_said_when_it_could_not_read_the_version()
-> Result<(), Box<dyn Error>> {
    let mut terminal = Terminal::new(TestBackend::new(27, 10))?;
    let mut rest = Rect::default();
    let project = project(Loadable::Failed(
        "error: We cannot get or set dynamic project versions in: pyproject.toml\nhint: more"
            .into(),
    ));
    terminal.draw(|frame| rest = render_project_tile(frame, frame.area(), &project, None))?;
    // Wrapped at words, the first line takes a row more than its width alone needs.
    assert_eq!(
        rows(&terminal)[..8],
        [
            "┌Project──────────────────┐",
            "│error: We cannot get or  │",
            "│set dynamic project      │",
            "│versions in:             │",
            "│pyproject.toml           │",
            "│/home/jane/billing-api   │",
            "│…                        │",
            "└─────────────────────────┘",
        ]
    );
    assert_eq!(terminal.backend().buffer()[(1, 1)].fg, Color::DarkGray);
    assert_eq!(rest, Rect::new(0, 8, 27, 2));
    Ok(())
}

#[test]
fn an_empty_uv_log_says_what_will_appear() -> Result<(), Box<dyn Error>> {
    let mut terminal = Terminal::new(TestBackend::new(30, 4))?;
    terminal.draw(|frame| render_uv_log(frame, frame.area(), &JobsState::default(), &mut 0))?;
    assert_eq!(rows(&terminal)[1], "│uv output will appear here  │");
    assert_eq!(terminal.backend().buffer()[(1, 1)].fg, Color::DarkGray);
    Ok(())
}

#[test]
fn the_uv_log_keeps_its_newest_lines_in_view() -> Result<(), Box<dyn Error>> {
    let mut jobs = JobsState::default();
    jobs.apply(&Event::JobQueued {
        id: JobId(1),
        label: "Adding numpy".into(),
    });
    for line in ["Resolved 9 packages", "Downloading numpy", "+ numpy==2.5.3"] {
        jobs.apply(&Event::JobOutput(JobId(1), line.into()));
    }

    let mut terminal = Terminal::new(TestBackend::new(24, 6))?;
    terminal.draw(|frame| render_uv_log(frame, frame.area(), &jobs, &mut 0))?;
    assert_eq!(
        rows(&terminal),
        [
            "┌uv────────────────────┐",
            "│Adding numpy          │",
            "│Resolved 9 packages   │",
            "│Downloading numpy     │",
            "│+ numpy==2.5.3        │",
            "└──────────────────────┘",
        ]
    );
    assert_eq!(terminal.backend().buffer()[(1, 1)].fg, Color::Cyan);

    // Two rows: the oldest lines go first.
    let mut terminal = Terminal::new(TestBackend::new(24, 4))?;
    terminal.draw(|frame| render_uv_log(frame, frame.area(), &jobs, &mut 0))?;
    assert_eq!(
        rows(&terminal)[1..3],
        ["│Downloading numpy     │", "│+ numpy==2.5.3        │"]
    );
    Ok(())
}

#[test]
fn a_long_uv_line_wraps_under_its_own_indent() -> Result<(), Box<dyn Error>> {
    let mut jobs = JobsState::default();
    jobs.apply(&Event::JobQueued {
        id: JobId(1),
        label: "Adding nosuchpackage-zzz".into(),
    });
    for line in [
        "  × No solution found when resolving dependencies:",
        "  ╰─▶ Because nosuchpackage-zzz was not found in the package registry and",
        "      your project depends on nosuchpackage-zzz, we can conclude that",
    ] {
        jobs.apply(&Event::JobOutput(JobId(1), line.into()));
    }
    // The log's width at 80 columns.
    let mut terminal = Terminal::new(TestBackend::new(32, 15))?;
    terminal.draw(|frame| render_uv_log(frame, frame.area(), &jobs, &mut 0))?;
    assert_eq!(
        rows(&terminal),
        [
            "┌uv────────────────────────────┐",
            "│Adding nosuchpackage-zzz      │",
            "│  × No solution found when    │",
            "│  resolving dependencies:     │",
            "│  ╰─▶ Because                 │",
            "│  nosuchpackage-zzz was not   │",
            "│  found in the package        │",
            "│  registry and                │",
            "│      your project depends on │",
            "│      nosuchpackage-zzz, we   │",
            "│      can conclude that       │",
            "│                              │",
            "│                              │",
            "│                              │",
            "└──────────────────────────────┘",
        ]
    );
    // Short of rows, the newest are kept, wrapped as they are.
    let mut terminal = Terminal::new(TestBackend::new(32, 4))?;
    terminal.draw(|frame| render_uv_log(frame, frame.area(), &jobs, &mut 0))?;
    assert_eq!(
        rows(&terminal)[1..3],
        [
            "│      nosuchpackage-zzz, we   │",
            "│      can conclude that       │"
        ]
    );
    Ok(())
}

#[test]
fn a_word_wider_than_the_log_is_broken_inside() {
    assert_eq!(
        wrap("see https://example.com/a/very/long/path", 12),
        ["see", "https://exam", "ple.com/a/ve", "ry/long/path"]
    );
    // An indent that would take half the width is dropped.
    assert_eq!(wrap("        a b", 10), ["a b"]);
    assert_eq!(wrap("", 10), [""]);
}

#[test]
fn the_project_tile_says_what_uv_lock_needs() -> Result<(), Box<dyn Error>> {
    use uview::domain::LockState;
    let line = |lock: Loadable<LockState>| -> Result<(String, Option<Color>), Box<dyn Error>> {
        let mut project = project(Loadable::NotLoaded);
        project.lock = lock;
        let mut terminal = Terminal::new(TestBackend::new(48, 6))?;
        terminal.draw(|frame| {
            render_project_tile(frame, frame.area(), &project, None);
        })?;
        // Below the root and the environment's line.
        let row = rows(&terminal)[3]
            .trim_matches(|c| c == '│' || c == ' ')
            .to_string();
        Ok((row, Some(terminal.backend().buffer()[(1, 3)].fg)))
    };
    assert_eq!(
        line(Loadable::Loaded(LockState::Fresh))?.0,
        "uv.lock up to date"
    );
    let (stale, colour) = line(Loadable::Loaded(LockState::Stale))?;
    assert_eq!(stale, "uv.lock out of date — press l");
    assert_eq!(colour, Some(Color::Yellow));
    assert_eq!(
        line(Loadable::Loaded(LockState::Missing))?.0,
        "no uv.lock — press l"
    );
    assert_eq!(
        line(Loadable::Loaded(LockState::Offline))?.0,
        "uv.lock not checked: it needs the network"
    );
    assert_eq!(
        line(Loadable::Loaded(LockState::Unknown("no pyproject".into())))?.0,
        "uv.lock not checked: no pyproject"
    );
    // Until a reload has checked, nothing is said: the environment's row is followed by the
    // border.
    assert!(line(Loadable::NotLoaded)?.0.starts_with('└'));
    Ok(())
}

#[test]
fn the_project_tile_says_where_the_environment_is_and_how_big() -> Result<(), Box<dyn Error>> {
    use uview::domain::EnvInfo;
    let line =
        |environment: Loadable<Option<EnvInfo>>, packages| -> Result<String, Box<dyn Error>> {
            let mut project = project(Loadable::NotLoaded);
            project.environment = environment;
            let mut terminal = Terminal::new(TestBackend::new(60, 5))?;
            terminal.draw(|frame| {
                render_project_tile(frame, frame.area(), &project, packages);
            })?;
            Ok(rows(&terminal)[2]
                .trim_matches(|c| c == '│' || c == ' ')
                .to_string())
        };
    let venv = |size| EnvInfo {
        shown_path: "../../.venv".into(),
        python: Some("3.11.15".into()),
        size,
    };
    assert_eq!(
        line(Loadable::Loaded(Some(venv(Some(43_200_000)))), Some(48))?,
        "../../.venv · Python 3.11.15 · 48 packages · 41.2 MiB"
    );
    // Quiet whatever happens.
    assert_eq!(
        line(Loadable::Loaded(Some(venv(None))), None)?,
        "../../.venv · Python 3.11.15 · size unknown"
    );
    assert_eq!(line(Loadable::Loaded(None), None)?, "no environment");
    assert_eq!(line(Loadable::NotLoaded, None)?, "…");
    Ok(())
}

#[test]
fn the_uv_log_scrolls_back_as_far_as_it_goes() -> Result<(), Box<dyn Error>> {
    let mut jobs = JobsState::default();
    jobs.apply(&Event::JobQueued {
        id: JobId(1),
        label: "Locking".into(),
    });
    for n in 1..=6 {
        jobs.apply(&Event::JobOutput(JobId(1), format!("line {n}")));
    }
    // Seven rows of log in a box of three.
    let mut terminal = Terminal::new(TestBackend::new(30, 5))?;
    let mut back = 0;
    terminal.draw(|frame| render_uv_log(frame, frame.area(), &jobs, &mut back))?;
    let drawn = rows(&terminal);
    assert!(drawn[0].starts_with("┌uv (shift+↑↓ to scroll)"));
    assert!(drawn[3].contains("line 6"));
    back = 2;
    terminal.draw(|frame| render_uv_log(frame, frame.area(), &jobs, &mut back))?;
    assert!(rows(&terminal)[3].contains("line 4"));
    // No further than the first row.
    back = 99;
    terminal.draw(|frame| render_uv_log(frame, frame.area(), &jobs, &mut back))?;
    assert_eq!(back, 4);
    assert!(rows(&terminal)[1].contains("Locking"));
    Ok(())
}

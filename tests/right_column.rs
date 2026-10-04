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
use uview::ui::components::features::uv_log::render_uv_log;

fn rows(terminal: &Terminal<TestBackend>) -> Vec<String> {
    let buffer = terminal.backend().buffer();
    buffer
        .content()
        .chunks(usize::from(buffer.area.width))
        .map(|row| row.iter().map(|cell| cell.symbol()).collect())
        .collect()
}

fn project(info: Loadable<ProjectInfo>) -> ProjectState {
    ProjectState {
        root: "/home/jane/billing-api".into(),
        info,
    }
}

#[test]
fn the_project_tile_wraps_the_root_and_leaves_the_rest_to_the_log() -> Result<(), Box<dyn Error>> {
    let mut terminal = Terminal::new(TestBackend::new(16, 8))?;
    let mut rest = Rect::default();
    let project = project(Loadable::NotLoaded);
    terminal.draw(|frame| rest = render_project_tile(frame, frame.area(), &project))?;
    assert_eq!(
        rows(&terminal)[..4],
        [
            "┌Project───────┐",
            "│/home/jane/bil│",
            "│ling-api      │",
            "└──────────────┘",
        ]
    );
    assert_eq!(rest, Rect::new(0, 4, 16, 4));
    Ok(())
}

#[test]
fn the_project_tile_is_titled_with_the_projects_name_and_version() -> Result<(), Box<dyn Error>> {
    let mut terminal = Terminal::new(TestBackend::new(24, 4))?;
    let project = project(Loadable::Loaded(ProjectInfo {
        name: "billing-api".into(),
        version: "0.4.2".into(),
    }));
    terminal.draw(|frame| {
        render_project_tile(frame, frame.area(), &project);
    })?;
    assert_eq!(
        rows(&terminal)[..3],
        [
            "┌ billing-api 0.4.2 ───┐",
            "│/home/jane/billing-api│",
            "└──────────────────────┘",
        ]
    );
    let buffer = terminal.backend().buffer();
    assert!(buffer[(2, 0)].modifier.contains(Modifier::BOLD));
    assert!(!buffer[(14, 0)].modifier.contains(Modifier::BOLD));
    assert_eq!(
        (buffer[(2, 0)].fg, buffer[(14, 0)].fg),
        (Color::White, Color::White)
    );
    Ok(())
}

#[test]
fn the_project_tile_says_what_uv_said_when_it_could_not_read_the_version()
-> Result<(), Box<dyn Error>> {
    let mut terminal = Terminal::new(TestBackend::new(27, 9))?;
    let mut rest = Rect::default();
    let project = project(Loadable::Failed(
        "error: We cannot get or set dynamic project versions in: pyproject.toml\nhint: more"
            .into(),
    ));
    terminal.draw(|frame| rest = render_project_tile(frame, frame.area(), &project))?;
    // Wrapped at words, the first line takes a row more than its width alone needs.
    assert_eq!(
        rows(&terminal)[..7],
        [
            "┌Project──────────────────┐",
            "│error: We cannot get or  │",
            "│set dynamic project      │",
            "│versions in:             │",
            "│pyproject.toml           │",
            "│/home/jane/billing-api   │",
            "└─────────────────────────┘",
        ]
    );
    assert_eq!(terminal.backend().buffer()[(1, 1)].fg, Color::DarkGray);
    assert_eq!(rest, Rect::new(0, 7, 27, 2));
    Ok(())
}

#[test]
fn an_empty_uv_log_says_what_will_appear() -> Result<(), Box<dyn Error>> {
    let mut terminal = Terminal::new(TestBackend::new(30, 4))?;
    terminal.draw(|frame| render_uv_log(frame, frame.area(), &JobsState::default()))?;
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
    terminal.draw(|frame| render_uv_log(frame, frame.area(), &jobs))?;
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
    terminal.draw(|frame| render_uv_log(frame, frame.area(), &jobs))?;
    assert_eq!(
        rows(&terminal)[1..3],
        ["│Downloading numpy     │", "│+ numpy==2.5.3        │"]
    );
    Ok(())
}

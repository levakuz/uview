use std::error::Error;

use ratatui::{Terminal, backend::TestBackend, layout::Rect, style::Color};
use uview::message::{Event, JobId};
use uview::runtime::jobs::JobsState;
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

#[test]
fn the_project_tile_wraps_the_root_and_leaves_the_rest_to_the_log() -> Result<(), Box<dyn Error>> {
    let mut terminal = Terminal::new(TestBackend::new(16, 8))?;
    let mut rest = Rect::default();
    terminal
        .draw(|frame| rest = render_project_tile(frame, frame.area(), "/home/jane/billing-api"))?;
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

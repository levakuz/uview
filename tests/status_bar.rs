use ratatui::{Terminal, backend::TestBackend};
use uview::state::Message;
use uview::ui::components::shared::status_bar::render_status_line;

#[test]
fn error_is_one_labelled_line_clipped_to_width() -> Result<(), Box<dyn std::error::Error>> {
    let mut terminal = Terminal::new(TestBackend::new(30, 1))?;
    // Multi-line like uv's stderr: must be flattened, not cut off at the first newline.
    let message = Message::Error("failed:\n  Caused by: something long enough to clip".into());
    terminal.draw(|frame| render_status_line(frame, frame.area(), None, Some(&message)))?;
    let line: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect();
    assert_eq!(line, "Error: failed: Caused by: some");
    Ok(())
}

#[test]
fn a_running_job_comes_before_the_message() -> Result<(), Box<dyn std::error::Error>> {
    let mut terminal = Terminal::new(TestBackend::new(40, 1))?;
    let message = Message::Info("Removed six".into());
    terminal.draw(|frame| {
        let activity = Some("⠋ Adding httpx");
        render_status_line(frame, frame.area(), activity, Some(&message))
    })?;
    let cells = terminal.backend().buffer().content();
    let line: String = cells.iter().map(|cell| cell.symbol()).collect();
    assert_eq!(line, format!("{:<40}", "⠋ Adding httpx  Removed six"));
    assert_eq!(cells[0].fg, ratatui::style::Color::Cyan);
    Ok(())
}

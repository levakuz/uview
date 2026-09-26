use ratatui::{Terminal, backend::TestBackend};
use uview::ui::components::shared::status_bar::StatusMessage;

#[test]
fn error_is_one_labelled_line_clipped_to_width() -> Result<(), Box<dyn std::error::Error>> {
    let mut terminal = Terminal::new(TestBackend::new(30, 1))?;
    // Multi-line like uv's stderr: must be flattened, not cut off at the first newline.
    let message =
        StatusMessage::Error("failed:\n  Caused by: something long enough to clip".into());
    terminal.draw(|frame| message.render(frame, frame.area()))?;
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

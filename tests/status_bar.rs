use ratatui::{Terminal, backend::TestBackend};
use uview::ui::components::shared::status_bar::{StatusMessage, render_status_line};

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

#[test]
fn project_stays_beside_a_message_with_its_tail_visible() -> Result<(), Box<dyn std::error::Error>>
{
    let mut terminal = Terminal::new(TestBackend::new(30, 1))?;
    let message = StatusMessage::Info("Removed six".into());
    let project = "/Users/jane/services/billing-api";
    terminal.draw(|frame| render_status_line(frame, frame.area(), Some(&message), project))?;
    let cells = terminal.backend().buffer().content();
    let line: String = cells.iter().map(|cell| cell.symbol()).collect();
    assert_eq!(line, "Removed six    ces/billing-api");
    // The default colour: a palette grey can be the background colour itself.
    assert_eq!(cells[29].fg, ratatui::style::Color::Reset);
    Ok(())
}

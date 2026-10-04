use ratatui::buffer::Buffer;
use ratatui::style::{Color, Modifier};
use ratatui::{Terminal, backend::TestBackend};
use uview::message::{Action, DepsAction};
use uview::ui::components::widgets::dependencies_tables_tabs::DependenciesTablesTabs;

fn tabs() -> DependenciesTablesTabs {
    DependenciesTablesTabs::new()
}

fn render(
    tabs: &mut DependenciesTablesTabs,
    width: u16,
) -> Result<Buffer, Box<dyn std::error::Error>> {
    let mut terminal = Terminal::new(TestBackend::new(width, 4))?;
    terminal.draw(|frame| tabs.render(frame, frame.area()))?;
    Ok(terminal.backend().buffer().clone())
}

#[test]
fn tabs_leave_the_table_its_own_colours() -> Result<(), Box<dyn std::error::Error>> {
    let mut tabs = tabs();
    let buffer = render(&mut tabs, 40)?;
    let top: String = (0..40).map(|x| buffer[(x, 0)].symbol()).collect();
    assert_eq!(top, "┌ pyproject.toml • Environment ────────┐");
    // The border keeps the focused table's colour on both sides of the tabs...
    for x in [0, 35, 39] {
        assert_eq!(buffer[(x, 0)].fg, Color::Cyan, "x = {x}");
    }
    // ...the divider and the inactive tab are white, and the active tab green and bold.
    assert_eq!(buffer[(17, 0)].fg, Color::White);
    assert_eq!(buffer[(19, 0)].fg, Color::White);
    assert_eq!(buffer[(2, 0)].fg, Color::Green);
    assert!(buffer[(2, 0)].modifier.contains(Modifier::BOLD));
    // The header keeps the terminal's default colour.
    assert_eq!(buffer[(1, 1)].symbol(), "N");
    assert_eq!(buffer[(1, 1)].fg, Color::Reset);

    tabs.update(&Action::Deps(DepsAction::SwitchTable));
    let buffer = render(&mut tabs, 40)?;
    assert_eq!(buffer[(0, 0)].fg, Color::Cyan);
    assert_eq!(buffer[(2, 0)].fg, Color::White);
    assert_eq!(buffer[(19, 0)].fg, Color::Green);
    Ok(())
}

#[test]
fn tabs_cut_short_keep_the_right_corner() -> Result<(), Box<dyn std::error::Error>> {
    let buffer = render(&mut tabs(), 30)?;
    assert_eq!(buffer[(29, 0)].symbol(), "┐");
    Ok(())
}

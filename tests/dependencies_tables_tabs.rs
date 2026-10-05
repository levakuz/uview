use ratatui::buffer::Buffer;
use ratatui::style::{Color, Modifier};
use ratatui::{Terminal, backend::TestBackend};
use std::time::{Duration, Instant};

use uview::domain::{EnvDependency, Membership, PyprojectDependency};
use uview::message::{Action, DepsAction};
use uview::state::{DepsState, Loadable};
use uview::ui::components::shared::table::TableRow;
use uview::ui::components::widgets::dependencies_tables_tabs::DependenciesTablesTabs;

fn tabs() -> DependenciesTablesTabs {
    DependenciesTablesTabs::new()
}

fn render(
    tabs: &mut DependenciesTablesTabs,
    width: u16,
) -> Result<Buffer, Box<dyn std::error::Error>> {
    let mut terminal = Terminal::new(TestBackend::new(width, 4))?;
    terminal.draw(|frame| tabs.render(frame, frame.area(), &DepsState::default()))?;
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

    tabs.update(
        &Action::Deps(DepsAction::SwitchTable),
        &DepsState::default(),
    );
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

#[test]
fn the_first_row_is_highlighted_before_any_key() -> Result<(), Box<dyn std::error::Error>> {
    let mut deps = DepsState::default();
    deps.env = Loadable::Loaded(vec![
        EnvDependency {
            name: "six".into(),
            version: "1.17.0".into(),
        },
        EnvDependency {
            name: "idna".into(),
            version: "3.10".into(),
        },
    ]);
    let mut tabs = tabs();
    tabs.update(&Action::Deps(DepsAction::SwitchTable), &deps);
    let mut terminal = Terminal::new(TestBackend::new(30, 5))?;
    terminal.draw(|frame| tabs.render(frame, frame.area(), &deps))?;
    let buffer = terminal.backend().buffer();
    // Below the border and the header: six, highlighted, then idna.
    assert!(buffer[(1, 2)].modifier.contains(Modifier::REVERSED));
    assert!(!buffer[(1, 3)].modifier.contains(Modifier::REVERSED));
    Ok(())
}

/// What the pyproject table shows below its header when its rows are `pyproject`.
fn first_line(
    pyproject: Loadable<Vec<PyprojectDependency>>,
) -> Result<String, Box<dyn std::error::Error>> {
    let mut deps = DepsState::default();
    deps.pyproject = pyproject;
    let mut terminal = Terminal::new(TestBackend::new(60, 5))?;
    terminal.draw(|frame| tabs().render(frame, frame.area(), &deps))?;
    let buffer = terminal.backend().buffer();
    Ok((1..59)
        .map(|x| buffer[(x, 2)].symbol())
        .collect::<String>()
        .trim_end()
        .to_string())
}

#[test]
fn each_load_state_shows_differently() -> Result<(), Box<dyn std::error::Error>> {
    // Nothing has asked for the rows yet.
    assert_eq!(first_line(Loadable::NotLoaded)?, "");
    let now = Instant::now();
    assert_eq!(first_line(Loadable::Loading { since: now })?, "Loading…");
    let since = now
        .checked_sub(Duration::from_secs(5))
        .ok_or("clock too young")?;
    assert_eq!(first_line(Loadable::Loading { since })?, "Loading… 5s");
    assert_eq!(
        first_line(Loadable::Loaded(Vec::new()))?,
        "No dependencies yet. Press p to add one."
    );
    assert_eq!(
        first_line(Loadable::Failed("No such file or directory".into()))?,
        "Could not read pyproject.toml: No such file or directory"
    );
    let six = PyprojectDependency {
        name: "six".into(),
        groups: vec![Membership::Project],
        extras: Vec::new(),
        version_constraint: ">=1.17".into(),
    };
    assert!(first_line(Loadable::Loaded(vec![six]))?.starts_with("six"));
    Ok(())
}

#[test]
fn columns_fit_their_cells_and_sit_on_the_left() -> Result<(), Box<dyn std::error::Error>> {
    let dep = |name: &str, constraint: &str| PyprojectDependency {
        name: name.into(),
        groups: vec![Membership::Project],
        extras: Vec::new(),
        version_constraint: constraint.into(),
    };
    let mut deps = DepsState::default();
    deps.pyproject = Loadable::Loaded(vec![
        dep("typing-extensions", ">=4.16.0"),
        dep("tomli", ">=2.0; python_version < '3.11'"),
    ]);
    let mut terminal = Terminal::new(TestBackend::new(48, 5))?;
    terminal.draw(|frame| tabs().render(frame, frame.area(), &deps))?;
    let buffer = terminal.backend().buffer();
    let row = |y| (1..47).map(|x| buffer[(x, y)].symbol()).collect::<String>();
    // Name fits the longest name, Group fits "Group", a long constraint stops at a third of the
    // width, and the spare room trails on the right.
    assert_eq!(row(1), "Name                Group   Constraint        ");
    assert_eq!(row(2), "typing-extensions   main    >=4.16.0          ");
    assert_eq!(row(3), "tomli               main    >=2.0; python_ve  ");
    Ok(())
}

#[test]
fn extras_follow_the_name_and_a_long_list_is_cut_short() {
    let dep = |extras: &[&str]| PyprojectDependency {
        name: "uvicorn".into(),
        groups: vec![Membership::Project],
        extras: extras.iter().map(ToString::to_string).collect(),
        version_constraint: String::new(),
    };
    assert_eq!(dep(&[]).cells()[0], "uvicorn");
    assert_eq!(dep(&["standard"]).cells()[0], "uvicorn[standard]");
    assert_eq!(
        dep(&["standard", "socks", "watchfiles", "httptools"]).cells()[0],
        "uvicorn[standard, socks, ...]"
    );
}

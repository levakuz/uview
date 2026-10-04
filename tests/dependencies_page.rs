use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend};
use uview::domain::{Membership, PyprojectDependency};
use uview::message::{Command, Event, Input};
use uview::ui::Component;
use uview::ui::pages::dependencies::DependenciesPage;

fn package(name: &str) -> PyprojectDependency {
    PyprojectDependency {
        name: name.into(),
        groups: vec![Membership::Project],
        version_constraint: String::new(),
    }
}

/// A page whose pyproject table lists `names`.
fn page_with(names: &[&str]) -> DependenciesPage {
    let mut page = DependenciesPage::new();
    page.apply(&Event::PyprojectLoaded(
        names.iter().map(|name| package(name)).collect(),
    ));
    page
}

/// Presses `code` as the app does: the page's keymap turns it into an action, and `update`
/// returns the commands that asks for. Nothing runs them: no runtime, no `uv`.
fn press(page: &mut DependenciesPage, code: KeyCode) -> Vec<Command> {
    match page.action_for(&Input::Key(KeyEvent::from(code))) {
        Some(action) => page.update(&action),
        None => Vec::new(),
    }
}

#[test]
fn enter_and_space_open_the_actions_and_run_the_highlighted_one() {
    for key in [KeyCode::Char(' '), KeyCode::Enter] {
        let mut page = page_with(&["six"]);
        // On a row: open the actions list. In the list: run Pin to version, which opens the
        // version input.
        assert!(press(&mut page, key).is_empty());
        assert!(press(&mut page, key).is_empty());
        assert!(page.is_editing(), "{key:?}");
    }
}

#[test]
fn a_typed_version_is_pinned() {
    let mut page = page_with(&["six"]);
    press(&mut page, KeyCode::Enter);
    press(&mut page, KeyCode::Enter);
    press(&mut page, KeyCode::Char('1'));
    assert_eq!(
        press(&mut page, KeyCode::Enter),
        [Command::Pin {
            name: "six".into(),
            constraint: "1".into()
        }]
    );
    assert!(!page.is_editing());
}

#[test]
fn p_adds_the_typed_requirement() {
    let mut page = page_with(&["six"]);
    press(&mut page, KeyCode::Char('p'));
    // Enter on an empty field adds nothing and leaves the input open.
    assert!(press(&mut page, KeyCode::Enter).is_empty());
    assert!(page.is_editing());
    for c in "httpx>=0.27".chars() {
        press(&mut page, KeyCode::Char(c));
    }
    assert_eq!(
        press(&mut page, KeyCode::Enter),
        [Command::Add {
            requirement: "httpx>=0.27".into()
        }]
    );
    assert!(!page.is_editing());
}

#[test]
fn enter_alone_cannot_pin() {
    let mut page = page_with(&["six"]);
    // The version input opens empty: a third Enter, as from a held key, runs nothing.
    for _ in 0..3 {
        assert!(press(&mut page, KeyCode::Enter).is_empty());
    }
    // A version abandoned with Esc does not come back for the next Enter, Enter, Enter.
    press(&mut page, KeyCode::Char('1'));
    press(&mut page, KeyCode::Esc);
    for _ in 0..3 {
        assert!(press(&mut page, KeyCode::Enter).is_empty());
    }
}

#[test]
fn a_table_without_rows_opens_no_actions() -> Result<(), Box<dyn std::error::Error>> {
    let mut page = DependenciesPage::new();
    press(&mut page, KeyCode::Enter);
    let mut terminal = Terminal::new(TestBackend::new(80, 20))?;
    terminal.draw(|frame| page.render(frame, frame.area()))?;
    let screen: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect();
    assert!(!screen.contains("Package Control"));
    Ok(())
}

#[test]
fn the_actions_act_on_the_row_they_were_opened_for() {
    let mut page = page_with(&["anyio", "httpx", "requests"]);
    press(&mut page, KeyCode::Down);
    press(&mut page, KeyCode::Enter);
    // A reload lands while the list is open for httpx, and the selected row is now requests.
    page.apply(&Event::PyprojectLoaded(vec![
        package("httpx"),
        package("requests"),
    ]));
    press(&mut page, KeyCode::Down);
    press(&mut page, KeyCode::Down);
    assert_eq!(
        press(&mut page, KeyCode::Enter),
        [Command::RemoveFromProject {
            name: "httpx".into(),
            groups: vec![Membership::Project]
        }]
    );
}

#[test]
fn a_load_that_lands_while_typing_is_kept() {
    let mut page = page_with(&["six"]);
    press(&mut page, KeyCode::Enter);
    press(&mut page, KeyCode::Enter);
    page.apply(&Event::PyprojectLoaded(vec![
        package("six"),
        package("idna"),
    ]));
    press(&mut page, KeyCode::Esc);
    // idna arrived while the version input was open.
    press(&mut page, KeyCode::Down);
    press(&mut page, KeyCode::Enter);
    press(&mut page, KeyCode::Down);
    assert_eq!(
        press(&mut page, KeyCode::Enter),
        [Command::Upgrade {
            name: "idna".into()
        }]
    );
}

#[test]
fn alt_keys_do_nothing_outside_an_input() {
    let page = page_with(&["six"]);
    let alt_enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::ALT);
    assert!(page.action_for(&Input::Key(alt_enter)).is_none());
}

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend};
use uview::domain::{Membership, PyprojectDependency};
use uview::message::{Action, Command, Input};
use uview::state::AppState;
use uview::ui::Component;
use uview::ui::pages::dependencies::DependenciesPage;

fn package(name: &str) -> PyprojectDependency {
    PyprojectDependency {
        name: name.into(),
        groups: vec![Membership::Project],
        version_constraint: String::new(),
    }
}

/// The page and what the app knows, as the app holds them.
struct Screen {
    page: DependenciesPage,
    state: AppState,
}

impl Screen {
    /// A screen whose pyproject table lists `names`.
    fn with(names: &[&str]) -> Self {
        let mut state = AppState::default();
        state.deps.pyproject = names.iter().map(|name| package(name)).collect();
        Self {
            page: DependenciesPage::new(),
            state,
        }
    }

    /// Presses `code` as the app does: the page's keymap turns it into an action, and `update`
    /// returns the commands that asks for. Nothing runs them: no runtime, no `uv`.
    fn press(&mut self, code: KeyCode) -> Vec<Command> {
        match self.page.action_for(&Input::Key(KeyEvent::from(code))) {
            Some(action) => self.page.update(&action, &self.state),
            None => Vec::new(),
        }
    }
}

#[test]
fn enter_and_space_open_the_actions_and_run_the_highlighted_one() {
    for key in [KeyCode::Char(' '), KeyCode::Enter] {
        let mut screen = Screen::with(&["six"]);
        // On a row: open the actions list. In the list: run Pin to version, which opens the
        // version input.
        assert!(screen.press(key).is_empty());
        assert!(screen.press(key).is_empty());
        assert!(screen.page.is_editing(), "{key:?}");
    }
}

#[test]
fn a_typed_version_is_pinned() {
    let mut screen = Screen::with(&["six"]);
    screen.press(KeyCode::Enter);
    screen.press(KeyCode::Enter);
    screen.press(KeyCode::Char('1'));
    assert_eq!(
        screen.press(KeyCode::Enter),
        [Command::Pin {
            name: "six".into(),
            constraint: "1".into()
        }]
    );
    assert!(!screen.page.is_editing());
}

#[test]
fn p_adds_the_typed_requirement() {
    let mut screen = Screen::with(&["six"]);
    screen.press(KeyCode::Char('p'));
    // Enter on an empty field adds nothing and leaves the input open.
    assert!(screen.press(KeyCode::Enter).is_empty());
    assert!(screen.page.is_editing());
    for c in "httpx>=0.27".chars() {
        screen.press(KeyCode::Char(c));
    }
    assert_eq!(
        screen.press(KeyCode::Enter),
        [Command::Add {
            requirement: "httpx>=0.27".into()
        }]
    );
    assert!(!screen.page.is_editing());
}

#[test]
fn enter_alone_cannot_pin() {
    let mut screen = Screen::with(&["six"]);
    // The version input opens empty: a third Enter, as from a held key, runs nothing.
    for _ in 0..3 {
        assert!(screen.press(KeyCode::Enter).is_empty());
    }
    // A version abandoned with Esc does not come back for the next Enter, Enter, Enter.
    screen.press(KeyCode::Char('1'));
    screen.press(KeyCode::Esc);
    for _ in 0..3 {
        assert!(screen.press(KeyCode::Enter).is_empty());
    }
}

#[test]
fn a_table_without_rows_opens_no_actions() -> Result<(), Box<dyn std::error::Error>> {
    let mut screen = Screen::with(&[]);
    screen.press(KeyCode::Enter);
    let mut terminal = Terminal::new(TestBackend::new(80, 20))?;
    terminal.draw(|frame| screen.page.render(frame, frame.area(), &screen.state))?;
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
    let mut screen = Screen::with(&["anyio", "httpx", "requests"]);
    screen.press(KeyCode::Down);
    screen.press(KeyCode::Enter);
    // A reload lands while the list is open for httpx, and the selected row is now requests.
    screen.state.deps.pyproject = vec![package("httpx"), package("requests")];
    screen.press(KeyCode::Down);
    screen.press(KeyCode::Down);
    assert_eq!(
        screen.press(KeyCode::Enter),
        [Command::RemoveFromProject {
            name: "httpx".into(),
            groups: vec![Membership::Project]
        }]
    );
}

#[test]
fn a_load_that_lands_while_typing_is_kept() {
    let mut screen = Screen::with(&["six"]);
    screen.press(KeyCode::Enter);
    screen.press(KeyCode::Enter);
    screen.state.deps.pyproject = vec![package("six"), package("idna")];
    screen.press(KeyCode::Esc);
    // idna arrived while the version input was open.
    screen.press(KeyCode::Down);
    screen.press(KeyCode::Enter);
    screen.press(KeyCode::Down);
    assert_eq!(
        screen.press(KeyCode::Enter),
        [Command::Upgrade {
            name: "idna".into()
        }]
    );
}

#[test]
fn alt_keys_do_nothing_outside_an_input() {
    let screen = Screen::with(&["six"]);
    let alt_enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::ALT);
    assert!(screen.page.action_for(&Input::Key(alt_enter)).is_none());
}

#[test]
fn ctrl_c_cancels_even_in_an_input() {
    let mut screen = Screen::with(&["six"]);
    let ctrl_c = Input::Key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
    assert!(matches!(
        screen.page.action_for(&ctrl_c),
        Some(Action::Cancel)
    ));
    screen.press(KeyCode::Char('p'));
    assert!(screen.page.is_editing());
    assert!(matches!(
        screen.page.action_for(&ctrl_c),
        Some(Action::Cancel)
    ));
}

#[test]
fn s_syncs_outside_an_input() {
    let mut screen = Screen::with(&["six"]);
    assert_eq!(screen.press(KeyCode::Char('s')), [Command::Sync]);
    // In an input, it is typed.
    screen.press(KeyCode::Char('p'));
    assert!(screen.press(KeyCode::Char('s')).is_empty());
    assert!(screen.page.is_editing());
}

#[test]
fn a_selection_past_the_rows_falls_back_to_the_last() {
    let mut screen = Screen::with(&["anyio", "httpx", "requests"]);
    screen.press(KeyCode::Up);
    // A reload leaves fewer rows than the selection was down.
    screen.state.deps.pyproject = vec![package("anyio"), package("httpx")];
    screen.press(KeyCode::Enter);
    screen.press(KeyCode::Down);
    screen.press(KeyCode::Down);
    assert_eq!(
        screen.press(KeyCode::Enter),
        [Command::RemoveFromProject {
            name: "httpx".into(),
            groups: vec![Membership::Project]
        }]
    );
}

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend};
use uview::domain::{Membership, PyprojectDependency};
use uview::message::{Action, Command, Input, NavAction};
use uview::state::{AppState, Loadable};
use uview::ui::Component;
use uview::ui::keymap::{self, Mode};
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
        state.deps.pyproject = Loadable::Loaded(names.iter().map(|name| package(name)).collect());
        Self {
            page: DependenciesPage::new(),
            state,
        }
    }

    /// What `key` means where the page has the focus, as the app reads it.
    fn action(&self, key: KeyEvent) -> Option<Action> {
        keymap::action_for(self.page.mode(), &Input::Key(key))
    }

    /// Presses `code` as the app does: the keymap turns it into an action, and `update` returns
    /// the commands that asks for. Nothing runs them: no runtime, no `uv`.
    fn press(&mut self, code: KeyCode) -> Vec<Command> {
        match self.action(KeyEvent::from(code)) {
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
        assert_eq!(screen.page.mode(), Mode::Input, "{key:?}");
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
    assert_eq!(screen.page.mode(), Mode::Table);
}

#[test]
fn p_adds_the_typed_requirement() {
    let mut screen = Screen::with(&["six"]);
    screen.press(KeyCode::Char('p'));
    // Enter on an empty field adds nothing and leaves the input open.
    assert!(screen.press(KeyCode::Enter).is_empty());
    assert_eq!(screen.page.mode(), Mode::Input);
    for c in "httpx>=0.27".chars() {
        screen.press(KeyCode::Char(c));
    }
    assert_eq!(
        screen.press(KeyCode::Enter),
        [Command::Add {
            requirement: "httpx>=0.27".into()
        }]
    );
    assert_eq!(screen.page.mode(), Mode::Table);
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
    screen.state.deps.pyproject = Loadable::Loaded(vec![package("httpx"), package("requests")]);
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
    screen.state.deps.pyproject = Loadable::Loaded(vec![package("six"), package("idna")]);
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
fn alt_and_ctrl_keys_do_nothing_outside_an_input() {
    let screen = Screen::with(&["six"]);
    let alt_enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::ALT);
    assert!(screen.action(alt_enter).is_none());
    // Ctrl+P is not p.
    let ctrl_p = KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL);
    assert!(screen.action(ctrl_p).is_none());
}

#[test]
fn ctrl_c_cancels_even_in_an_input() {
    let mut screen = Screen::with(&["six"]);
    let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
    assert_eq!(screen.action(ctrl_c), Some(Action::Cancel));
    screen.press(KeyCode::Char('p'));
    assert_eq!(screen.page.mode(), Mode::Input);
    assert_eq!(screen.action(ctrl_c), Some(Action::Cancel));
    // A plain c is typed.
    assert!(matches!(
        screen.action(KeyEvent::from(KeyCode::Char('c'))),
        Some(Action::Nav(NavAction::Type(_)))
    ));
}

#[test]
fn s_syncs_outside_an_input() {
    let mut screen = Screen::with(&["six"]);
    assert_eq!(screen.press(KeyCode::Char('s')), [Command::Sync]);
    // In an input, it is typed.
    screen.press(KeyCode::Char('p'));
    assert!(screen.press(KeyCode::Char('s')).is_empty());
    assert_eq!(screen.page.mode(), Mode::Input);
}

#[test]
fn a_selection_past_the_rows_falls_back_to_the_last() {
    let mut screen = Screen::with(&["anyio", "httpx", "requests"]);
    screen.press(KeyCode::Up);
    // A reload leaves fewer rows than the selection was down.
    screen.state.deps.pyproject = Loadable::Loaded(vec![package("anyio"), package("httpx")]);
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

#[test]
fn r_refreshes_outside_an_input() {
    let mut screen = Screen::with(&["six"]);
    assert_eq!(screen.press(KeyCode::Char('r')), [Command::Reload]);
    // In an input, it is typed.
    screen.press(KeyCode::Char('p'));
    assert!(screen.press(KeyCode::Char('r')).is_empty());
}

#[test]
fn the_selection_stays_on_its_package_across_a_reload() {
    let mut screen = Screen::with(&["anyio", "httpx", "requests"]);
    screen.press(KeyCode::Down);
    // A reload adds a package above the selected httpx.
    screen.state.deps.pyproject = Loadable::Loaded(vec![
        package("aiohttp"),
        package("anyio"),
        package("httpx"),
        package("requests"),
    ]);
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

#[test]
fn the_actions_list_takes_only_its_own_keys() {
    let mut screen = Screen::with(&["six"]);
    screen.press(KeyCode::Enter);
    assert_eq!(screen.page.mode(), Mode::Actions);
    // The table's keys do nothing while the list is open, as its footer says.
    for key in ['p', 's', 'r', '['] {
        assert!(screen.press(KeyCode::Char(key)).is_empty(), "{key}");
        assert_eq!(screen.page.mode(), Mode::Actions, "{key}");
    }
    screen.press(KeyCode::Esc);
    assert_eq!(screen.page.mode(), Mode::Table);
}

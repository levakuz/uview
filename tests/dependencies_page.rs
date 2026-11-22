use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend};
use uview::domain::{Bump, Membership, PyprojectDependency, Source, VersionChange};
use uview::message::{Action, Command, Input, NavAction};
use uview::state::{AppState, Loadable};
use uview::ui::Component;
use uview::ui::keymap::{self, Mode};
use uview::ui::pages::dependencies::DependenciesPage;

fn package(name: &str) -> PyprojectDependency {
    PyprojectDependency {
        name: name.into(),
        groups: vec![Membership::Project],
        extras: Vec::new(),
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
        // On a row: open the actions list. In the list: run Pin to version, which lists the
        // versions. In that list: "Type a constraint…", which opens the input.
        assert!(screen.press(key).is_empty());
        assert_eq!(
            screen.press(key),
            [Command::ListVersions { name: "six".into() }]
        );
        assert!(screen.press(key).is_empty());
        assert_eq!(screen.page.mode(), Mode::Input, "{key:?}");
    }
}

#[test]
fn a_typed_version_is_pinned() {
    let mut screen = Screen::with(&["six"]);
    for _ in 0..3 {
        screen.press(KeyCode::Enter);
    }
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

impl Screen {
    /// Opens the add input and types `requirement` into it.
    fn type_requirement(&mut self, requirement: &str) {
        self.press(KeyCode::Char('p'));
        for c in requirement.chars() {
            self.press(KeyCode::Char(c));
        }
    }
}

#[test]
fn p_adds_the_typed_requirement_to_main_by_default() {
    let mut screen = Screen::with(&["six"]);
    screen.press(KeyCode::Char('p'));
    // Enter on an empty field adds nothing and leaves the input open.
    assert!(screen.press(KeyCode::Enter).is_empty());
    assert_eq!(screen.page.mode(), Mode::Input);
    for c in "httpx>=0.27".chars() {
        screen.press(KeyCode::Char(c));
    }
    // Enter moves on to the group, main highlighted.
    assert!(screen.press(KeyCode::Enter).is_empty());
    assert_eq!(screen.page.mode(), Mode::Actions);
    assert_eq!(
        screen.press(KeyCode::Enter),
        [Command::Add {
            requirement: "httpx>=0.27".into(),
            membership: Membership::Project,
        }]
    );
    assert_eq!(screen.page.mode(), Mode::Table);
}

#[test]
fn the_group_list_offers_the_projects_groups_then_a_new_one() {
    let mut screen = Screen::with(&["six"]);
    if let Loadable::Loaded(rows) = &mut screen.state.deps.pyproject {
        rows.push(PyprojectDependency {
            groups: vec![
                Membership::Group("test".into()),
                Membership::Group("dev".into()),
            ],
            ..package("pytest")
        });
    }
    // main, dev, test: the second is dev.
    screen.type_requirement("ruff");
    screen.press(KeyCode::Enter);
    // "Latest stable version".
    screen.press(KeyCode::Enter);
    screen.press(KeyCode::Down);
    assert_eq!(
        screen.press(KeyCode::Enter),
        [Command::Add {
            requirement: "ruff".into(),
            membership: Membership::Group("dev".into()),
        }]
    );
    // Past the last group: a new one, typed in.
    screen.type_requirement("mypy");
    screen.press(KeyCode::Enter);
    // "Latest stable version".
    screen.press(KeyCode::Enter);
    for _ in 0..5 {
        screen.press(KeyCode::Down);
    }
    assert!(screen.press(KeyCode::Enter).is_empty());
    assert_eq!(screen.page.mode(), Mode::Input);
    for c in "lint".chars() {
        screen.press(KeyCode::Char(c));
    }
    assert_eq!(
        screen.press(KeyCode::Enter),
        [Command::Add {
            requirement: "mypy".into(),
            membership: Membership::Group("lint".into()),
        }]
    );
    assert_eq!(screen.page.mode(), Mode::Table);
}

#[test]
fn esc_while_choosing_a_group_adds_nothing() {
    let mut screen = Screen::with(&["six"]);
    screen.type_requirement("httpx");
    screen.press(KeyCode::Enter);
    // "Latest stable version".
    screen.press(KeyCode::Enter);
    screen.press(KeyCode::Esc);
    assert_eq!(screen.page.mode(), Mode::Table);
    // Nor from the new group's input.
    screen.type_requirement("httpx");
    screen.press(KeyCode::Enter);
    // "Latest stable version".
    screen.press(KeyCode::Enter);
    screen.press(KeyCode::Down);
    screen.press(KeyCode::Enter);
    screen.press(KeyCode::Esc);
    assert_eq!(screen.page.mode(), Mode::Table);
    // The next add starts afresh.
    screen.type_requirement("ruff");
    screen.press(KeyCode::Enter);
    // "Latest stable version".
    screen.press(KeyCode::Enter);
    assert_eq!(
        screen.press(KeyCode::Enter),
        [Command::Add {
            requirement: "ruff".into(),
            membership: Membership::Project,
        }]
    );
}

#[test]
fn enter_alone_cannot_pin() {
    let mut screen = Screen::with(&["six"]);
    // The version input opens empty: one more Enter, as from a held key, pins nothing.
    let pins = |screen: &mut Screen| {
        (0..4)
            .flat_map(|_| screen.press(KeyCode::Enter))
            .any(|command| matches!(command, Command::Pin { .. }))
    };
    assert!(!pins(&mut screen));
    // A version abandoned with Esc does not come back for the next Enters.
    screen.press(KeyCode::Char('1'));
    screen.press(KeyCode::Esc);
    assert!(!pins(&mut screen));
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
    // Pin, Bump, Update, Move to group, Add to group, Remove.
    for _ in 0..5 {
        screen.press(KeyCode::Down);
    }
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
    // idna arrived while the version input was open. Pin, Bump, Update.
    screen.press(KeyCode::Down);
    screen.press(KeyCode::Enter);
    screen.press(KeyCode::Down);
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
    // Pin, Bump, Update, Move to group, Add to group, Remove.
    for _ in 0..5 {
        screen.press(KeyCode::Down);
    }
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
    // Pin, Bump, Update, Move to group, Add to group, Remove.
    for _ in 0..5 {
        screen.press(KeyCode::Down);
    }
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

/// A screen whose pyproject table lists six in main, and pytest in `pytest_groups`.
fn with_pytest_in(pytest_groups: &[&str]) -> Screen {
    let mut screen = Screen::with(&["six"]);
    if let Loadable::Loaded(rows) = &mut screen.state.deps.pyproject {
        rows.push(PyprojectDependency {
            groups: pytest_groups
                .iter()
                .map(|g| Membership::Group(g.to_string()))
                .collect(),
            version_constraint: ">=8".into(),
            ..package("pytest")
        });
    }
    screen
}

impl Screen {
    /// Opens the actions list on the `row`th row and runs its `option`th option.
    fn run_option(&mut self, row: usize, option: usize) -> Vec<Command> {
        for _ in 0..row {
            self.press(KeyCode::Down);
        }
        self.press(KeyCode::Enter);
        for _ in 0..option {
            self.press(KeyCode::Down);
        }
        self.press(KeyCode::Enter)
    }
}

#[test]
fn a_package_in_one_group_moves_to_another() {
    // pytest in dev; the groups other than dev: main, test.
    let mut screen = with_pytest_in(&["dev", "test"]);
    if let Loadable::Loaded(rows) = &mut screen.state.deps.pyproject {
        rows[1].groups = vec![Membership::Group("dev".into())];
        rows.push(PyprojectDependency {
            groups: vec![Membership::Group("test".into())],
            ..package("coverage")
        });
    }
    // Pin, Bump, Update, Move to group.
    assert!(screen.run_option(1, 3).is_empty());
    assert_eq!(screen.page.mode(), Mode::Actions);
    screen.press(KeyCode::Down);
    assert_eq!(
        screen.press(KeyCode::Enter),
        [Command::Move {
            name: "pytest".into(),
            requirement: "pytest>=8".into(),
            from: Membership::Group("dev".into()),
            to: Membership::Group("test".into()),
        }]
    );
}

#[test]
fn a_package_in_one_group_can_be_added_to_another_too() {
    // six is in main alone. Pin, Bump, Update, Move to group, Add to group; main is left out.
    let mut screen = with_pytest_in(&["dev"]);
    screen.run_option(0, 4);
    assert_eq!(
        screen.press(KeyCode::Enter),
        [Command::Add {
            requirement: "six".into(),
            membership: Membership::Group("dev".into()),
        }]
    );
}

#[test]
fn a_package_in_several_groups_is_added_to_or_removed_from_one() {
    let mut screen = with_pytest_in(&["dev", "test"]);
    // Pin, Bump, Update, Add to group: main is the one group it is not in.
    screen.run_option(1, 3);
    assert_eq!(
        screen.press(KeyCode::Enter),
        [Command::Add {
            requirement: "pytest>=8".into(),
            membership: Membership::Project,
        }]
    );
    // Remove from group: the list is its groups.
    screen.run_option(0, 4);
    screen.press(KeyCode::Down);
    assert_eq!(
        screen.press(KeyCode::Enter),
        [Command::RemoveFromProject {
            name: "pytest".into(),
            groups: vec![Membership::Group("test".into())],
        }]
    );
}

#[test]
fn an_installed_package_has_no_group_actions() {
    let mut screen = Screen::with(&["six"]);
    screen.state.deps.env = Loadable::Loaded(vec![uview::domain::EnvDependency {
        name: "six".into(),
        version: "1.17.0".into(),
    }]);
    screen.press(KeyCode::Tab);
    // Pin, Update, Remove: uninstall, as before; Bump is for a declared package.
    assert_eq!(
        screen.run_option(0, 2),
        [Command::Uninstall { name: "six".into() }]
    );
}

#[test]
fn removing_a_groups_last_package_offers_to_delete_the_group() {
    let mut screen = with_pytest_in(&["dev"]);
    // A group already empty is not asked about.
    screen.state.deps.deletable_groups = vec!["lint".into()];
    // pytest: Pin, Bump, Update, Move to group, Add to group, Remove.
    assert_eq!(
        screen.run_option(1, 5),
        [Command::RemoveFromProject {
            name: "pytest".into(),
            groups: vec![Membership::Group("dev".into())],
        }]
    );
    assert_eq!(screen.page.mode(), Mode::Table);
    // The reload after it: dev is left empty.
    screen.state.deps.pyproject = Loadable::Loaded(vec![package("six")]);
    screen.state.deps.deletable_groups = vec!["dev".into(), "lint".into()];
    // The question takes the next key, here Enter on Delete group dev.
    assert_eq!(
        screen.press(KeyCode::Enter),
        [Command::DeleteGroup { name: "dev".into() }]
    );
    // Asked once, and never about lint.
    assert!(screen.press(KeyCode::Char('r')) == [Command::Reload]);
}

#[test]
fn keep_it_leaves_the_empty_group() {
    let mut screen = with_pytest_in(&["dev"]);
    screen.run_option(1, 5);
    screen.state.deps.deletable_groups = vec!["dev".into()];
    screen.press(KeyCode::Down);
    assert!(screen.press(KeyCode::Enter).is_empty());
    assert_eq!(screen.page.mode(), Mode::Table);
}

#[test]
fn an_empty_group_is_offered_too() {
    let mut screen = Screen::with(&["six"]);
    // docs has no package, so no row.
    screen.state.deps.empty_groups = vec!["docs".into()];
    screen.type_requirement("mkdocs");
    screen.press(KeyCode::Enter);
    // "Latest stable version".
    screen.press(KeyCode::Enter);
    screen.press(KeyCode::Down);
    assert_eq!(
        screen.press(KeyCode::Enter),
        [Command::Add {
            requirement: "mkdocs".into(),
            membership: Membership::Group("docs".into()),
        }]
    );
}

impl Screen {
    fn type_text(&mut self, text: &str) {
        for c in text.chars() {
            self.press(KeyCode::Char(c));
        }
    }
}

#[test]
fn shift_p_adds_a_package_pinned_to_an_index() {
    let mut screen = Screen::with(&["six"]);
    screen.press(KeyCode::Char('P'));
    assert_eq!(screen.page.mode(), Mode::Actions);
    // Index URL is first: the package, then the URL, then the group.
    screen.press(KeyCode::Enter);
    screen.type_text("torch");
    screen.press(KeyCode::Enter);
    screen.type_text("https://download.pytorch.org/whl/cpu");
    assert!(screen.press(KeyCode::Enter).is_empty());
    assert_eq!(screen.page.mode(), Mode::Actions);
    assert_eq!(
        screen.press(KeyCode::Enter),
        [Command::AddFrom {
            source: Source::Index {
                package: "torch".into(),
                url: "https://download.pytorch.org/whl/cpu".into(),
            },
            membership: Membership::Project,
        }]
    );
    assert_eq!(screen.page.mode(), Mode::Table);
}

#[test]
fn a_git_url_with_a_problem_is_asked_for_again_as_typed() {
    let mut screen = Screen::with(&["six"]);
    screen.press(KeyCode::Char('P'));
    screen.press(KeyCode::Down);
    screen.press(KeyCode::Enter);
    screen.type_text("github.com/org/repo");
    // Refused: the input stays open with what was typed, to be fixed rather than retyped.
    assert!(screen.press(KeyCode::Enter).is_empty());
    assert_eq!(screen.page.mode(), Mode::Input);
    screen.press(KeyCode::Home);
    screen.type_text("https://");
    screen.press(KeyCode::Enter);
    assert_eq!(
        screen.press(KeyCode::Enter),
        [Command::AddFrom {
            source: Source::Git {
                url: "https://github.com/org/repo".into(),
            },
            membership: Membership::Project,
        }]
    );
}

#[test]
fn a_local_path_is_editable_unless_said_otherwise() {
    let mut screen = Screen::with(&["six"]);
    screen.press(KeyCode::Char('P'));
    screen.press(KeyCode::Down);
    screen.press(KeyCode::Down);
    screen.press(KeyCode::Enter);
    screen.type_text("../mylib");
    screen.press(KeyCode::Enter);
    // Editable is highlighted; then main.
    screen.press(KeyCode::Enter);
    assert_eq!(
        screen.press(KeyCode::Enter),
        [Command::AddFrom {
            source: Source::Path {
                path: "../mylib".into(),
                editable: true,
            },
            membership: Membership::Project,
        }]
    );
}

#[test]
fn esc_gives_up_on_a_source_at_any_step() {
    let mut screen = Screen::with(&["six"]);
    screen.press(KeyCode::Char('P'));
    screen.press(KeyCode::Esc);
    assert_eq!(screen.page.mode(), Mode::Table);
    screen.press(KeyCode::Char('P'));
    screen.press(KeyCode::Enter);
    screen.type_text("torch");
    screen.press(KeyCode::Enter);
    screen.press(KeyCode::Esc);
    assert_eq!(screen.page.mode(), Mode::Table);
    // Plain p still adds from the default index at once.
    screen.press(KeyCode::Char('p'));
    assert_eq!(screen.page.mode(), Mode::Input);
}

#[test]
fn u_and_d_update_and_remove_the_selected_row_in_one_key() {
    let mut screen = with_pytest_in(&["dev", "test"]);
    screen.press(KeyCode::Down);
    // As Enter, then Update or Remove, would.
    assert_eq!(
        screen.press(KeyCode::Char('u')),
        [Command::Upgrade {
            name: "pytest".into()
        }]
    );
    assert_eq!(
        screen.press(KeyCode::Char('d')),
        [Command::RemoveFromProject {
            name: "pytest".into(),
            groups: vec![
                Membership::Group("dev".into()),
                Membership::Group("test".into())
            ],
        }]
    );
    // In the Environment table, d uninstalls and leaves pyproject.toml alone.
    screen.state.deps.env = Loadable::Loaded(vec![uview::domain::EnvDependency {
        name: "six".into(),
        version: "1.17.0".into(),
    }]);
    screen.press(KeyCode::Tab);
    assert_eq!(
        screen.press(KeyCode::Char('u')),
        [Command::Upgrade { name: "six".into() }]
    );
    assert_eq!(
        screen.press(KeyCode::Char('d')),
        [Command::Uninstall { name: "six".into() }]
    );
}

#[test]
fn u_and_d_do_nothing_without_a_row_or_over_a_popup() {
    let mut screen = Screen::with(&[]);
    assert!(screen.press(KeyCode::Char('u')).is_empty());
    assert!(screen.press(KeyCode::Char('d')).is_empty());
    // Over the actions list.
    let mut screen = Screen::with(&["six"]);
    screen.press(KeyCode::Enter);
    assert!(screen.press(KeyCode::Char('d')).is_empty());
    assert_eq!(screen.page.mode(), Mode::Actions);
    screen.press(KeyCode::Esc);
    // In an input they are typed: "ud" is the requirement.
    screen.press(KeyCode::Char('p'));
    screen.press(KeyCode::Char('u'));
    assert!(screen.press(KeyCode::Char('d')).is_empty());
    screen.press(KeyCode::Enter);
    // "Latest stable version".
    screen.press(KeyCode::Enter);
    assert_eq!(
        screen.press(KeyCode::Enter),
        [Command::Add {
            requirement: "ud".into(),
            membership: Membership::Project,
        }]
    );
}

fn corp() -> uview::domain::Index {
    uview::domain::Index {
        name: Some("corp".into()),
        url: "https://corp.example.com/simple/".into(),
        kind: uview::domain::IndexKind::Default,
        publish_url: None,
    }
}

#[test]
fn i_adds_an_index_extra_unless_said_otherwise() {
    let mut screen = Screen::with(&["six"]);
    screen.press(KeyCode::Char('i'));
    assert_eq!(screen.page.mode(), Mode::Actions);
    // "PyPI is the only index" does nothing; "Add index…" asks for the name and the URL.
    assert!(screen.press(KeyCode::Enter).is_empty());
    assert_eq!(screen.page.mode(), Mode::Actions);
    screen.press(KeyCode::Down);
    screen.press(KeyCode::Enter);
    screen.type_text("pytorch");
    screen.press(KeyCode::Enter);
    screen.type_text("https://download.pytorch.org/whl/cpu");
    screen.press(KeyCode::Enter);
    // Extra is highlighted.
    assert_eq!(
        screen.press(KeyCode::Enter),
        [Command::AddIndex {
            name: "pytorch".into(),
            url: "https://download.pytorch.org/whl/cpu".into(),
            default: false,
        }]
    );
    assert_eq!(screen.page.mode(), Mode::Table);
}

#[test]
fn a_bad_name_or_url_is_asked_for_again_and_default_can_be_chosen() {
    let mut screen = Screen::with(&["six"]);
    screen.state.deps.indexes = vec![corp()];
    screen.press(KeyCode::Char('i'));
    screen.press(KeyCode::Down);
    screen.press(KeyCode::Enter);
    // corp is taken.
    screen.type_text("corp");
    assert!(screen.press(KeyCode::Enter).is_empty());
    assert_eq!(screen.page.mode(), Mode::Input);
    screen.type_text("2");
    screen.press(KeyCode::Enter);
    // A token in the URL never gets as far as pyproject.toml.
    screen.type_text("https://token@mirror.example.com/simple/");
    assert!(screen.press(KeyCode::Enter).is_empty());
    assert_eq!(screen.page.mode(), Mode::Input);
    for _ in 0.."https://token@mirror.example.com/simple/".len() {
        screen.press(KeyCode::Backspace);
    }
    screen.type_text("https://mirror.example.com/simple/");
    screen.press(KeyCode::Enter);
    screen.press(KeyCode::Down);
    assert_eq!(
        screen.press(KeyCode::Enter),
        [Command::AddIndex {
            name: "corp2".into(),
            url: "https://mirror.example.com/simple/".into(),
            default: true,
        }]
    );
}

#[test]
fn an_index_in_the_list_can_be_removed_or_kept() {
    let mut screen = Screen::with(&["six"]);
    screen.state.deps.indexes = vec![corp()];
    screen.press(KeyCode::Char('i'));
    screen.press(KeyCode::Enter);
    assert_eq!(screen.press(KeyCode::Enter), [Command::RemoveIndex(corp())]);
    screen.press(KeyCode::Char('i'));
    screen.press(KeyCode::Enter);
    screen.press(KeyCode::Down);
    assert!(screen.press(KeyCode::Enter).is_empty());
    assert_eq!(screen.page.mode(), Mode::Table);
    // Esc closes the list.
    screen.press(KeyCode::Char('i'));
    screen.press(KeyCode::Esc);
    assert_eq!(screen.page.mode(), Mode::Table);
}

#[test]
fn d_in_the_list_of_indexes_removes_the_highlighted_one_at_once() {
    let mut screen = Screen::with(&["six"]);
    let other = uview::domain::Index {
        name: None,
        url: "https://mirror.example.com/simple/".into(),
        kind: uview::domain::IndexKind::Extra,
        publish_url: None,
    };
    screen.state.deps.indexes = vec![corp(), other.clone()];
    screen.press(KeyCode::Char('i'));
    screen.press(KeyCode::Down);
    assert_eq!(
        screen.press(KeyCode::Char('d')),
        [Command::RemoveIndex(other)]
    );
    assert_eq!(screen.page.mode(), Mode::Table);
    // On "Add index…" there is nothing to remove, and the list stays.
    screen.press(KeyCode::Char('i'));
    screen.press(KeyCode::Down);
    screen.press(KeyCode::Down);
    assert!(screen.press(KeyCode::Char('d')).is_empty());
    assert_eq!(screen.page.mode(), Mode::Actions);
    screen.press(KeyCode::Esc);
    // In the group list, d does nothing either.
    screen.type_requirement("ruff");
    screen.press(KeyCode::Enter);
    // "Latest stable version".
    screen.press(KeyCode::Enter);
    assert!(screen.press(KeyCode::Char('d')).is_empty());
    assert_eq!(screen.page.mode(), Mode::Actions);
}

#[test]
fn v_asks_uv_for_the_next_versions_and_bumps_with_one_more_key() {
    let mut screen = Screen::with(&["six"]);
    assert_eq!(screen.press(KeyCode::Char('v')), [Command::PreviewVersions]);
    assert_eq!(screen.page.mode(), Mode::Actions);
    screen.press(KeyCode::Down);
    assert_eq!(
        screen.press(KeyCode::Enter),
        [Command::SetVersion(VersionChange::Bump(Bump::Minor))]
    );
    assert_eq!(screen.page.mode(), Mode::Table);
}

#[test]
fn a_typed_version_is_checked_before_uv_runs() {
    let mut screen = Screen::with(&["six"]);
    screen.press(KeyCode::Char('v'));
    for _ in 0..3 {
        screen.press(KeyCode::Down);
    }
    screen.press(KeyCode::Enter);
    assert_eq!(screen.page.mode(), Mode::Input);
    screen.type_text("v1.0");
    // Refused: open again with what was typed.
    assert!(screen.press(KeyCode::Enter).is_empty());
    assert_eq!(screen.page.mode(), Mode::Input);
    screen.press(KeyCode::Home);
    screen.press(KeyCode::Delete);
    assert_eq!(
        screen.press(KeyCode::Enter),
        [Command::SetVersion(VersionChange::To("1.0".into()))]
    );
    // Esc closes the list without a change.
    screen.press(KeyCode::Char('v'));
    assert!(screen.press(KeyCode::Esc).is_empty());
    assert_eq!(screen.page.mode(), Mode::Table);
}

#[test]
fn each_bump_shows_the_version_it_makes_once_uv_has_said() -> Result<(), Box<dyn std::error::Error>>
{
    let mut screen = Screen::with(&["six"]);
    screen.state.project.info = Loadable::Loaded(uview::domain::ProjectInfo {
        name: "acme-api".into(),
        version: "0.4.2".into(),
    });
    screen.press(KeyCode::Char('v'));
    let text = |screen: &mut Screen| -> Result<String, Box<dyn std::error::Error>> {
        let mut terminal = Terminal::new(TestBackend::new(48, 12))?;
        terminal.draw(|frame| screen.page.render(frame, frame.area(), &screen.state))?;
        Ok(terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect())
    };
    let waiting = text(&mut screen)?;
    assert!(waiting.contains("Version 0.4.2") && waiting.contains("minor → …"));
    screen.state.project.bumps =
        Loadable::Loaded(vec!["0.4.3".into(), "0.5.0".into(), "1.0.0".into()]);
    let shown = text(&mut screen)?;
    for row in [
        "patch → 0.4.3",
        "minor → 0.5.0",
        "major → 1.0.0",
        "Set version…",
    ] {
        assert!(shown.contains(row), "{row}");
    }
    Ok(())
}

fn version(version: &str, pre: bool, needs_python: Option<&str>) -> uview::domain::VersionChoice {
    uview::domain::VersionChoice {
        version: version.into(),
        pre,
        needs_python: needs_python.map(String::from),
    }
}

impl Screen {
    /// What the page draws, as text, in the left pane's width at 80 columns.
    fn drawn(&mut self) -> Result<String, Box<dyn std::error::Error>> {
        let mut terminal = Terminal::new(TestBackend::new(48, 16))?;
        terminal.draw(|frame| self.page.render(frame, frame.area(), &self.state))?;
        Ok(terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect())
    }
}

#[test]
fn pin_to_version_lists_the_versions_and_pins_the_one_chosen()
-> Result<(), Box<dyn std::error::Error>> {
    let mut screen = Screen::with(&["six"]);
    screen.press(KeyCode::Enter);
    assert_eq!(
        screen.press(KeyCode::Enter),
        [Command::ListVersions { name: "six".into() }]
    );
    // Until the index answers.
    let waiting = screen.drawn()?;
    assert!(waiting.contains("Pin six") && waiting.contains("Type a constraint…"));
    assert!(waiting.contains('…'));
    screen.state.deps.versions = Some((
        "six".into(),
        Loadable::Loaded(listed(vec![
            version("2.0.0b1", true, None),
            version("1.17.0", false, Some(">=3.12")),
            version("1.16.0", false, None),
        ])),
    ));
    let listed = screen.drawn()?;
    for row in ["2.0.0b1  pre", "1.17.0  needs Python >=3.12", "1.16.0"] {
        assert!(listed.contains(row), "{row}");
    }
    // The third row is 1.17.0; then how to pin it, `==` first, so Enter twice pins it exactly.
    screen.press(KeyCode::Down);
    screen.press(KeyCode::Down);
    assert!(screen.press(KeyCode::Enter).is_empty());
    let operators = screen.drawn()?;
    assert!(operators.contains("Pin six to 1.17.0"));
    assert!(operators.contains("==1.17.0  exactly this version"));
    assert!(operators.contains("~=1.17.0  compatible: >=1.17.0, <1.18"));
    assert_eq!(
        screen.press(KeyCode::Enter),
        [Command::Pin {
            name: "six".into(),
            constraint: "==1.17.0".into()
        }]
    );
    assert_eq!(screen.page.mode(), Mode::Table);
    // The second operator is `>=`.
    for _ in 0..2 {
        screen.press(KeyCode::Enter);
    }
    screen.press(KeyCode::Down);
    screen.press(KeyCode::Down);
    screen.press(KeyCode::Enter);
    screen.press(KeyCode::Down);
    assert_eq!(
        screen.press(KeyCode::Enter),
        [Command::Pin {
            name: "six".into(),
            constraint: ">=1.17.0".into()
        }]
    );
    Ok(())
}

#[test]
fn a_list_that_failed_or_is_for_another_package_offers_typing_alone()
-> Result<(), Box<dyn std::error::Error>> {
    let mut screen = Screen::with(&["six"]);
    // Versions of another package, from an earlier list, are not six's.
    screen.state.deps.versions = Some((
        "idna".into(),
        Loadable::Loaded(listed(vec![version("3.10", false, None)])),
    ));
    screen.press(KeyCode::Enter);
    screen.press(KeyCode::Enter);
    assert!(!screen.drawn()?.contains("3.10"));
    // Nothing to choose past "Type a constraint…" while it waits.
    screen.press(KeyCode::Down);
    assert!(screen.press(KeyCode::Enter).is_empty());
    assert_eq!(screen.page.mode(), Mode::Actions);
    screen.state.deps.versions = Some((
        "six".into(),
        Loadable::Failed("no answer: timed out".into()),
    ));
    assert!(
        screen
            .drawn()?
            .contains("Could not list versions: no answer")
    );
    // Typing still works.
    screen.press(KeyCode::Up);
    screen.press(KeyCode::Enter);
    assert_eq!(screen.page.mode(), Mode::Input);
    screen.press(KeyCode::Esc);
    assert_eq!(screen.page.mode(), Mode::Table);
    Ok(())
}

#[test]
fn p_with_a_name_alone_asks_which_version_then_how_then_the_group()
-> Result<(), Box<dyn std::error::Error>> {
    let mut screen = Screen::with(&["six"]);
    screen.type_requirement("flask[dotenv]");
    // The versions are those of flask; the extras stay with the requirement.
    assert_eq!(
        screen.press(KeyCode::Enter),
        [Command::ListVersions {
            name: "flask".into()
        }]
    );
    let drawn = screen.drawn()?;
    assert!(drawn.contains("Add flask") && drawn.contains("Latest stable version"));
    screen.state.deps.versions = Some((
        "flask".into(),
        Loadable::Loaded(listed(vec![
            version("3.1.3", false, None),
            version("3.1.2", false, None),
        ])),
    ));
    // 3.1.3, then `>=`, then main.
    screen.press(KeyCode::Down);
    screen.press(KeyCode::Enter);
    screen.press(KeyCode::Down);
    assert!(screen.press(KeyCode::Enter).is_empty());
    assert!(screen.drawn()?.contains("Add to group"));
    assert_eq!(
        screen.press(KeyCode::Enter),
        [Command::Add {
            requirement: "flask[dotenv]>=3.1.3".into(),
            membership: Membership::Project,
        }]
    );
    assert_eq!(screen.page.mode(), Mode::Table);
    Ok(())
}

#[test]
fn latest_leaves_the_version_to_uv_and_a_typed_one_skips_the_list() {
    let mut screen = Screen::with(&["six"]);
    screen.type_requirement("ruff");
    screen.press(KeyCode::Enter);
    screen.press(KeyCode::Enter);
    assert_eq!(
        screen.press(KeyCode::Enter),
        [Command::Add {
            requirement: "ruff".into(),
            membership: Membership::Project,
        }]
    );
    // Typed with a version: straight to the group.
    screen.type_requirement("httpx>=0.27");
    assert!(screen.press(KeyCode::Enter).is_empty());
    assert_eq!(
        screen.press(KeyCode::Enter),
        [Command::Add {
            requirement: "httpx>=0.27".into(),
            membership: Membership::Project,
        }]
    );
}

#[test]
fn esc_in_the_version_list_gives_up_the_add() {
    let mut screen = Screen::with(&["six"]);
    screen.type_requirement("ruff");
    screen.press(KeyCode::Enter);
    screen.press(KeyCode::Esc);
    assert_eq!(screen.page.mode(), Mode::Table);
    // Pin to version afterwards is a pin, not the add's leftover.
    screen.press(KeyCode::Enter);
    screen.press(KeyCode::Enter);
    screen.state.deps.versions = Some((
        "six".into(),
        Loadable::Loaded(listed(vec![version("1.17.0", false, None)])),
    ));
    screen.press(KeyCode::Down);
    screen.press(KeyCode::Enter);
    assert_eq!(
        screen.press(KeyCode::Enter),
        [Command::Pin {
            name: "six".into(),
            constraint: "==1.17.0".into()
        }]
    );
}

/// A screen whose pyproject table declares `name` as `constraint`, with `installed` in the
/// environment, and the index's versions of it already listed.
fn bump_screen(name: &str, constraint: &str, installed: &str, versions: &[&str]) -> Screen {
    let mut screen = Screen::with(&[]);
    screen.state.deps.pyproject = Loadable::Loaded(vec![PyprojectDependency {
        version_constraint: constraint.into(),
        ..package(name)
    }]);
    screen.state.deps.env = Loadable::Loaded(vec![uview::domain::EnvDependency {
        name: name.into(),
        version: installed.into(),
    }]);
    screen.state.deps.versions = Some((
        name.into(),
        Loadable::Loaded(listed(
            versions.iter().map(|v| version(v, false, None)).collect(),
        )),
    ));
    screen
}

#[test]
fn b_bumps_to_the_release_chosen_keeping_the_operator() -> Result<(), Box<dyn std::error::Error>> {
    let mut screen = bump_screen(
        "httpx",
        "==0.25.0",
        "0.25.0",
        &["1.0.0", "0.28.1", "0.25.2", "0.25.0"],
    );
    assert_eq!(
        screen.press(KeyCode::Char('b')),
        [Command::ListVersions {
            name: "httpx".into()
        }]
    );
    let drawn = screen.drawn()?;
    for row in [
        "Bump httpx from 0.25.0",
        "patch → 0.25.2",
        "minor → 0.28.1",
        "major → 1.0.0",
    ] {
        assert!(drawn.contains(row), "{row}");
    }
    screen.press(KeyCode::Down);
    assert_eq!(
        screen.press(KeyCode::Enter),
        [Command::Pin {
            name: "httpx".into(),
            constraint: "==0.28.1".into()
        }]
    );
    assert_eq!(screen.page.mode(), Mode::Table);
    Ok(())
}

#[test]
fn a_range_the_bump_leaves_is_written_only_once_confirmed() -> Result<(), Box<dyn std::error::Error>>
{
    let mut screen = bump_screen("six", ">=1.15,<1.16", "1.15.0", &["1.17.0", "1.15.0"]);
    screen.press(KeyCode::Char('b'));
    // minor → 1.17.0, outside the range.
    screen.press(KeyCode::Down);
    assert!(screen.press(KeyCode::Enter).is_empty());
    let drawn = screen.drawn()?;
    assert!(drawn.contains("1.17.0 is outside >=1.15,<1.16"));
    assert!(drawn.contains("Write >=1.17.0,<1.18") && drawn.contains("Keep >=1.15,<1.16"));
    assert_eq!(
        screen.press(KeyCode::Enter),
        [Command::Pin {
            name: "six".into(),
            constraint: ">=1.17.0,<1.18".into()
        }]
    );
    // Keep writes nothing.
    screen.press(KeyCode::Char('b'));
    screen.press(KeyCode::Down);
    screen.press(KeyCode::Enter);
    screen.press(KeyCode::Down);
    assert!(screen.press(KeyCode::Enter).is_empty());
    assert_eq!(screen.page.mode(), Mode::Table);
    Ok(())
}

#[test]
fn up_to_date_does_nothing_and_an_undeclared_package_is_not_bumped()
-> Result<(), Box<dyn std::error::Error>> {
    let mut screen = bump_screen("six", ">=1.17", "1.17.0", &["1.17.0"]);
    screen.press(KeyCode::Char('b'));
    assert!(screen.drawn()?.contains("patch: up to date"));
    assert!(screen.press(KeyCode::Enter).is_empty());
    screen.press(KeyCode::Esc);
    // In the Environment table, a package only installed has nothing to bump.
    screen.state.deps.env = Loadable::Loaded(vec![uview::domain::EnvDependency {
        name: "idna".into(),
        version: "3.10".into(),
    }]);
    screen.press(KeyCode::Tab);
    assert!(screen.press(KeyCode::Char('b')).is_empty());
    assert_eq!(screen.page.mode(), Mode::Table);
    Ok(())
}

#[test]
fn bump_is_in_the_actions_of_a_declared_package() {
    let mut screen = bump_screen("httpx", "==0.25.0", "0.25.0", &["0.25.2", "0.25.0"]);
    // Pin, Bump.
    assert_eq!(
        screen.run_option(0, 1),
        [Command::ListVersions {
            name: "httpx".into()
        }]
    );
    assert_eq!(
        screen.press(KeyCode::Enter),
        [Command::Pin {
            name: "httpx".into(),
            constraint: "==0.25.2".into()
        }]
    );
}

/// The versions an index listed, from PyPI with no description.
fn listed(versions: Vec<uview::domain::VersionChoice>) -> uview::domain::VersionList {
    uview::domain::VersionList {
        versions,
        index: "https://pypi.org/simple".into(),
        info: None,
    }
}

/// The versions of httpx as PyPI lists them, with its description.
fn httpx_from_pypi() -> uview::domain::VersionList {
    uview::domain::VersionList {
        versions: vec![
            version("1.0.dev6", true, None),
            version("0.28.1", false, None),
            version("0.28.0", false, None),
        ],
        index: "https://pypi.org/simple".into(),
        info: Some(uview::domain::PackageInfo {
            summary: Some("The next generation HTTP client.".into()),
            requires_python: Some(">=3.8".into()),
            homepage: Some("https://github.com/encode/httpx".into()),
        }),
    }
}

#[test]
fn p_shows_what_pypi_says_and_the_latest_stable_version() -> Result<(), Box<dyn std::error::Error>>
{
    let mut screen = Screen::with(&["six"]);
    screen.type_requirement("httpx");
    screen.press(KeyCode::Enter);
    screen.state.deps.versions = Some(("httpx".into(), Loadable::Loaded(httpx_from_pypi())));
    let mut terminal = Terminal::new(TestBackend::new(80, 16))?;
    terminal.draw(|frame| screen.page.render(frame, frame.area(), &screen.state))?;
    let drawn: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect();
    assert!(drawn.contains("Add httpx — The next generation HTTP client."));
    // The pre-release above it is not what uv takes unasked.
    assert!(drawn.contains("Latest stable version (0.28.1)"));
    Ok(())
}

#[test]
fn slash_looks_a_package_up_and_adds_it() -> Result<(), Box<dyn std::error::Error>> {
    let mut screen = Screen::with(&["six"]);
    screen.press(KeyCode::Char('/'));
    assert_eq!(screen.page.mode(), Mode::Input);
    screen.type_text("httpx");
    assert_eq!(
        screen.press(KeyCode::Enter),
        [Command::ListVersions {
            name: "httpx".into()
        }]
    );
    assert!(screen.drawn()?.contains("Looking up httpx…"));
    screen.state.deps.versions = Some(("httpx".into(), Loadable::Loaded(httpx_from_pypi())));
    let mut terminal = Terminal::new(TestBackend::new(80, 16))?;
    terminal.draw(|frame| screen.page.render(frame, frame.area(), &screen.state))?;
    let card: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect();
    for row in [
        "httpx 0.28.1",
        "Add httpx…",
        "The next generation HTTP client.",
        "Requires Python >=3.8",
        "https://github.com/encode/httpx",
        "From https://pypi.org/simple",
    ] {
        assert!(card.contains(row), "{row}");
    }
    // Add: on into the add, the latest stable version highlighted, then the group.
    assert!(screen.press(KeyCode::Enter).is_empty());
    assert!(screen.press(KeyCode::Enter).is_empty());
    assert_eq!(
        screen.press(KeyCode::Enter),
        [Command::Add {
            requirement: "httpx".into(),
            membership: Membership::Project,
        }]
    );
    Ok(())
}

#[test]
fn a_search_takes_a_name_alone_and_says_when_there_is_none()
-> Result<(), Box<dyn std::error::Error>> {
    let mut screen = Screen::with(&["six"]);
    screen.press(KeyCode::Char('/'));
    screen.type_text("httpx>=1");
    // Refused, kept as typed.
    assert!(screen.press(KeyCode::Enter).is_empty());
    assert_eq!(screen.page.mode(), Mode::Input);
    // Esc closes the search.
    screen.press(KeyCode::Esc);
    assert_eq!(screen.page.mode(), Mode::Table);
    screen.press(KeyCode::Char('/'));
    screen.type_text("nosuch");
    screen.press(KeyCode::Enter);
    screen.state.deps.versions = Some((
        "nosuch".into(),
        Loadable::Failed("nosuch is not on https://pypi.org/simple".into()),
    ));
    assert!(screen.drawn()?.contains("nosuch is not on"));
    // Nothing to add.
    assert!(screen.press(KeyCode::Enter).is_empty());
    assert_eq!(screen.page.mode(), Mode::Actions);
    screen.press(KeyCode::Esc);
    assert_eq!(screen.page.mode(), Mode::Table);
    Ok(())
}

/// six in main; pytest in dev and test; ruff in dev; lint empty; docs empty and a default group.
fn groups_screen() -> Screen {
    let mut screen = Screen::with(&[]);
    let dep = |name: &str, constraint: &str, groups: &[&str]| PyprojectDependency {
        version_constraint: constraint.into(),
        groups: groups
            .iter()
            .map(|g| match *g {
                "main" => Membership::Project,
                g => Membership::Group(g.into()),
            })
            .collect(),
        ..package(name)
    };
    screen.state.deps.pyproject = Loadable::Loaded(vec![
        dep("six", "", &["main"]),
        dep("pytest", ">=8", &["dev", "test"]),
        dep("ruff", "==0.6.9", &["dev"]),
    ]);
    let group = |name: &str, named_by: Option<&str>| uview::domain::DeclaredGroup {
        name: name.into(),
        named_by: named_by.map(String::from),
    };
    screen.state.deps.groups = vec![
        group("dev", None),
        group("test", None),
        group("lint", None),
        group("docs", Some("named in tool.uv.default-groups")),
    ];
    screen
}

#[test]
fn g_lists_the_groups_and_d_deletes_an_empty_one_at_once() -> Result<(), Box<dyn std::error::Error>>
{
    let mut screen = groups_screen();
    screen.press(KeyCode::Char('g'));
    assert_eq!(screen.page.mode(), Mode::Groups);
    let mut terminal = Terminal::new(TestBackend::new(80, 16))?;
    terminal.draw(|frame| screen.page.render(frame, frame.area(), &screen.state))?;
    let drawn: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect();
    for row in [
        "dev   2 packages",
        "test  1 package",
        "lint  empty",
        "docs  empty  · named in tool.uv.default-groups",
        "New group…",
    ] {
        assert!(drawn.contains(row), "{row}");
    }
    // "New group…" is first; lint is the third group.
    for _ in 0..3 {
        screen.press(KeyCode::Down);
    }
    assert_eq!(
        screen.press(KeyCode::Char('d')),
        [Command::DeleteGroup {
            name: "lint".into()
        }]
    );
    // docs is a default group: it stays.
    screen.press(KeyCode::Char('g'));
    for _ in 0..4 {
        screen.press(KeyCode::Down);
    }
    assert!(screen.press(KeyCode::Char('d')).is_empty());
    assert_eq!(screen.page.mode(), Mode::Groups);
    Ok(())
}

#[test]
fn a_group_with_packages_is_deleted_only_once_confirmed() -> Result<(), Box<dyn std::error::Error>>
{
    let mut screen = groups_screen();
    screen.press(KeyCode::Char('g'));
    // dev's details: its actions, then its packages.
    screen.press(KeyCode::Down);
    screen.press(KeyCode::Enter);
    let details = screen.drawn()?;
    for row in [
        "dev — 2 packages",
        "Rename…",
        "Delete group",
        "pytest>=8",
        "ruff==0.6.9",
    ] {
        assert!(details.contains(row), "{row}");
    }
    screen.press(KeyCode::Down);
    assert!(screen.press(KeyCode::Enter).is_empty());
    assert!(
        screen
            .drawn()?
            .contains("Delete dev and remove its 2 packages from it?")
    );
    // Each package leaves dev alone, then dev goes.
    assert_eq!(
        screen.press(KeyCode::Enter),
        [
            Command::RemoveFromProject {
                name: "pytest".into(),
                groups: vec![Membership::Group("dev".into())],
            },
            Command::RemoveFromProject {
                name: "ruff".into(),
                groups: vec![Membership::Group("dev".into())],
            },
            Command::DeleteGroup { name: "dev".into() },
        ]
    );
    // A reload that sees dev empty before it is deleted asks nothing.
    screen.state.deps.deletable_groups = vec!["dev".into()];
    assert_eq!(screen.press(KeyCode::Char('r')), [Command::Reload]);
    Ok(())
}

#[test]
fn keep_and_esc_leave_the_groups_as_they_are() {
    let mut screen = groups_screen();
    screen.press(KeyCode::Char('g'));
    screen.press(KeyCode::Down);
    screen.press(KeyCode::Enter);
    screen.press(KeyCode::Down);
    screen.press(KeyCode::Enter);
    screen.press(KeyCode::Down);
    assert!(screen.press(KeyCode::Enter).is_empty());
    assert_eq!(screen.page.mode(), Mode::Table);
    // Esc goes back from the details to the list, then closes.
    screen.press(KeyCode::Char('g'));
    screen.press(KeyCode::Down);
    screen.press(KeyCode::Enter);
    screen.press(KeyCode::Esc);
    assert_eq!(screen.page.mode(), Mode::Groups);
    screen.press(KeyCode::Esc);
    assert_eq!(screen.page.mode(), Mode::Table);
}

#[test]
fn a_rename_is_one_job() {
    let mut screen = groups_screen();
    screen.press(KeyCode::Char('g'));
    screen.press(KeyCode::Down);
    screen.press(KeyCode::Enter);
    screen.press(KeyCode::Enter);
    assert_eq!(screen.page.mode(), Mode::Input);
    // test is taken, as uv compares names.
    screen.type_text("Test");
    assert!(screen.press(KeyCode::Enter).is_empty());
    assert_eq!(screen.page.mode(), Mode::Input);
    for _ in 0..4 {
        screen.press(KeyCode::Backspace);
    }
    screen.type_text("tools");
    // One job, which edits the file in place: nothing can be lost part-way.
    assert_eq!(
        screen.press(KeyCode::Enter),
        [Command::RenameGroup {
            from: "dev".into(),
            to: "tools".into(),
        }]
    );
    // An empty group is renamed the same way.
    screen.press(KeyCode::Char('g'));
    for _ in 0..3 {
        screen.press(KeyCode::Down);
    }
    screen.press(KeyCode::Enter);
    screen.press(KeyCode::Enter);
    screen.type_text("linters");
    assert_eq!(
        screen.press(KeyCode::Enter),
        [Command::RenameGroup {
            from: "lint".into(),
            to: "linters".into(),
        }]
    );
}

#[test]
fn a_new_group_is_created_empty_and_a_named_one_offers_nothing()
-> Result<(), Box<dyn std::error::Error>> {
    let mut screen = groups_screen();
    screen.press(KeyCode::Char('g'));
    // "New group…" is first.
    screen.press(KeyCode::Enter);
    screen.type_text("my group");
    // Refused, as uv would.
    assert!(screen.press(KeyCode::Enter).is_empty());
    for _ in 0..8 {
        screen.press(KeyCode::Backspace);
    }
    screen.type_text("site");
    assert_eq!(
        screen.press(KeyCode::Enter),
        [Command::CreateGroup {
            name: "site".into()
        }]
    );
    // docs is named by default-groups: its details say why, and do nothing.
    screen.press(KeyCode::Char('g'));
    for _ in 0..4 {
        screen.press(KeyCode::Down);
    }
    screen.press(KeyCode::Enter);
    assert!(
        screen
            .drawn()?
            .contains("Kept: named in tool.uv.default-groups")
    );
    assert!(screen.press(KeyCode::Enter).is_empty());
    assert_eq!(screen.page.mode(), Mode::Groups);
    Ok(())
}

#[test]
fn n_r_and_d_act_from_the_list_of_groups_and_from_a_group_s_details() {
    let mut screen = groups_screen();
    screen.press(KeyCode::Char('g'));
    // n: a new group, wherever the highlight is.
    screen.press(KeyCode::Down);
    screen.press(KeyCode::Char('n'));
    assert_eq!(screen.page.mode(), Mode::Input);
    screen.type_text("site");
    assert_eq!(
        screen.press(KeyCode::Enter),
        [Command::CreateGroup {
            name: "site".into()
        }]
    );
    // r on lint, the third group.
    screen.press(KeyCode::Char('g'));
    for _ in 0..3 {
        screen.press(KeyCode::Down);
    }
    screen.press(KeyCode::Char('r'));
    screen.type_text("linters");
    assert_eq!(
        screen.press(KeyCode::Enter),
        [Command::RenameGroup {
            from: "lint".into(),
            to: "linters".into(),
        }]
    );
    // r on docs, named by default-groups, does nothing; nor on "New group…".
    screen.press(KeyCode::Char('g'));
    assert!(screen.press(KeyCode::Char('r')).is_empty());
    for _ in 0..4 {
        screen.press(KeyCode::Down);
    }
    assert!(screen.press(KeyCode::Char('r')).is_empty());
    assert_eq!(screen.page.mode(), Mode::Groups);
    screen.press(KeyCode::Esc);
    // From test's details, d asks first, as it has a package.
    screen.press(KeyCode::Char('g'));
    screen.press(KeyCode::Down);
    screen.press(KeyCode::Down);
    screen.press(KeyCode::Enter);
    assert!(screen.press(KeyCode::Char('d')).is_empty());
    assert_eq!(
        screen.press(KeyCode::Enter),
        [
            Command::RemoveFromProject {
                name: "pytest".into(),
                groups: vec![Membership::Group("test".into())],
            },
            Command::DeleteGroup {
                name: "test".into()
            },
        ]
    );
}

#[test]
fn r_and_n_outside_the_list_of_groups_keep_their_meaning() {
    // r still reloads from the table, and n there means nothing.
    let mut screen = groups_screen();
    assert_eq!(screen.press(KeyCode::Char('r')), [Command::Reload]);
    assert!(screen.press(KeyCode::Char('n')).is_empty());
    // In a package's actions list, neither does anything.
    screen.press(KeyCode::Enter);
    assert!(screen.press(KeyCode::Char('r')).is_empty());
    assert!(screen.press(KeyCode::Char('n')).is_empty());
    assert_eq!(screen.page.mode(), Mode::Actions);
}

#[test]
fn l_locks_and_shift_l_upgrades_every_package_in_the_lock() {
    let mut screen = Screen::with(&["six"]);
    assert_eq!(
        screen.press(KeyCode::Char('l')),
        [Command::Lock { upgrade: false }]
    );
    assert_eq!(
        screen.press(KeyCode::Char('L')),
        [Command::Lock { upgrade: true }]
    );
    // Typed, in an input.
    screen.press(KeyCode::Char('p'));
    assert!(screen.press(KeyCode::Char('l')).is_empty());
}

/// A screen whose tree is acme-api ─ httpx[http2] (declared) ─ anyio ─ idna, httpx ─ certifi.
fn tree_screen() -> Screen {
    let mut screen = Screen::with(&["httpx"]);
    let node = |depth: usize, name: &str| uview::domain::TreeNode {
        depth,
        name: name.into(),
        version: "1.0".into(),
        ..uview::domain::TreeNode::default()
    };
    screen.state.deps.tree = Loadable::Loaded(vec![
        node(0, "acme-api"),
        node(1, "httpx"),
        node(2, "anyio"),
        node(3, "idna"),
        node(2, "certifi"),
    ]);
    // Tab, Tab: the Tree tab.
    screen.press(KeyCode::Tab);
    screen.press(KeyCode::Tab);
    screen
}

#[test]
fn the_tree_tab_shows_the_tree_unfolded_and_folds() -> Result<(), Box<dyn std::error::Error>> {
    let mut screen = tree_screen();
    assert_eq!(screen.page.mode(), Mode::Tree);
    let drawn = screen.drawn()?;
    for row in [
        "▾ acme-api 1.0",
        "▾ httpx 1.0",
        "▾ anyio 1.0",
        "idna 1.0",
        "certifi 1.0",
    ] {
        assert!(drawn.contains(row), "{row}");
    }
    // httpx folded: its subtree goes.
    screen.press(KeyCode::Down);
    screen.press(KeyCode::Left);
    let folded = screen.drawn()?;
    assert!(folded.contains("▸ httpx 1.0") && !folded.contains("anyio"));
    screen.press(KeyCode::Right);
    assert!(screen.drawn()?.contains("anyio"));
    // All folded, then all unfolded.
    screen.press(KeyCode::Char('-'));
    assert!(!screen.drawn()?.contains("anyio"));
    screen.press(KeyCode::Char('+'));
    assert!(screen.drawn()?.contains("idna"));
    Ok(())
}

#[test]
fn o_asks_for_what_is_behind_and_w_for_what_pulls_a_package_in()
-> Result<(), Box<dyn std::error::Error>> {
    let mut screen = tree_screen();
    assert_eq!(
        screen.press(KeyCode::Char('o')),
        [Command::LoadTree { outdated: true }]
    );
    screen.state.deps.tree_outdated = true;
    assert_eq!(
        screen.press(KeyCode::Char('o')),
        [Command::LoadTree { outdated: false }]
    );
    // w on idna: what pulls it in, in place of the tree, until Esc.
    for _ in 0..3 {
        screen.press(KeyCode::Down);
    }
    assert_eq!(
        screen.press(KeyCode::Char('w')),
        [Command::WhyInstalled {
            name: "idna".into()
        }]
    );
    let why = |depth: usize, name: &str| uview::domain::TreeNode {
        depth,
        name: name.into(),
        version: "1.0".into(),
        ..uview::domain::TreeNode::default()
    };
    screen.state.deps.why = Some((
        "idna".into(),
        Loadable::Loaded(vec![why(0, "idna"), why(1, "anyio"), why(2, "httpx")]),
    ));
    let drawn = screen.drawn()?;
    assert!(drawn.contains("Why idna") && !drawn.contains("certifi"));
    screen.press(KeyCode::Esc);
    let drawn = screen.drawn()?;
    assert!(drawn.contains("Tree") && drawn.contains("certifi"));
    Ok(())
}

#[test]
fn package_actions_work_on_the_tree_for_a_declared_package() {
    let mut screen = tree_screen();
    // httpx is declared: Enter opens its actions; d removes it from pyproject.toml.
    screen.press(KeyCode::Down);
    assert!(screen.press(KeyCode::Enter).is_empty());
    assert_eq!(screen.page.mode(), Mode::Actions);
    screen.press(KeyCode::Esc);
    assert_eq!(
        screen.press(KeyCode::Char('d')),
        [Command::RemoveFromProject {
            name: "httpx".into(),
            groups: vec![Membership::Project],
        }]
    );
    // anyio is not: Enter and d do nothing, u updates it within the constraints.
    screen.press(KeyCode::Down);
    assert!(screen.press(KeyCode::Enter).is_empty());
    assert_eq!(screen.page.mode(), Mode::Tree);
    assert!(screen.press(KeyCode::Char('d')).is_empty());
    assert!(screen.press(KeyCode::Char('b')).is_empty());
    assert_eq!(
        screen.press(KeyCode::Char('u')),
        [Command::Upgrade {
            name: "anyio".into()
        }]
    );
}

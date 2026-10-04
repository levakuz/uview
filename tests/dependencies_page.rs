use crossterm::event::{KeyCode, KeyEvent};
use tokio::sync::mpsc;
use uview::events::{Action, Membership, PyprojectDependency};
use uview::ui::Component;
use uview::ui::pages::dependencies::DependenciesPage;

fn page() -> DependenciesPage {
    DependenciesPage::new(mpsc::unbounded_channel().0)
}

/// A pyproject load with one package. The tests stop short of running an action on it:
/// actions spawn uv onto tokio, and these tests have no runtime.
fn one_package() -> Action {
    Action::LoadedPyprojectDeps(vec![PyprojectDependency {
        name: "six".into(),
        groups: vec![Membership::Project],
        version_constraint: String::new(),
    }])
}

fn page_with_a_package() -> DependenciesPage {
    let mut page = page();
    page.handle_event(&one_package());
    page
}

/// Presses `code` and feeds each returned action back into the page, as `App::dispatch`
/// does. Returns those actions in order.
fn press(page: &mut DependenciesPage, code: KeyCode) -> Vec<Action> {
    let mut emitted = Vec::new();
    let mut next = page.handle_event(&Action::ButtonPressed(KeyEvent::from(code)));
    while let Some(action) = next {
        next = page.handle_event(&action);
        emitted.push(action);
    }
    emitted
}

#[test]
fn enter_outside_an_input_does_what_space_does() {
    for key in [KeyCode::Char(' '), KeyCode::Enter] {
        let mut page = page_with_a_package();
        // On a row: open the actions list.
        assert!(matches!(
            press(&mut page, key).as_slice(),
            [Action::SelectItem]
        ));
        // In the list: run the highlighted action, Pin to version.
        assert!(matches!(
            press(&mut page, key).as_slice(),
            [Action::SelectItem, Action::OpenPinPopup]
        ));
    }
}

#[test]
fn enter_alone_cannot_pin() {
    let mut page = page_with_a_package();
    press(&mut page, KeyCode::Enter);
    press(&mut page, KeyCode::Enter);
    // The version input is open and empty: a third Enter, as from a held key, runs nothing.
    assert!(press(&mut page, KeyCode::Enter).is_empty());
    // A version abandoned with Esc does not come back for the next Enter, Enter, Enter.
    press(&mut page, KeyCode::Char('1'));
    press(&mut page, KeyCode::Esc);
    press(&mut page, KeyCode::Enter);
    press(&mut page, KeyCode::Enter);
    assert!(press(&mut page, KeyCode::Enter).is_empty());
}

#[test]
fn enter_in_the_version_input_confirms_the_version() {
    // No package is loaded, so the pin is emitted but runs nothing.
    let mut page = page();
    page.handle_event(&Action::OpenPinPopup);
    press(&mut page, KeyCode::Char('1'));
    assert!(matches!(
        press(&mut page, KeyCode::Enter).as_slice(),
        [Action::PinVersion(version)] if version == "1"
    ));
}

#[test]
fn a_table_without_rows_opens_no_actions() {
    let mut page = page();
    press(&mut page, KeyCode::Enter);
    // Had the list opened, this Enter would run Pin to version.
    assert!(matches!(
        press(&mut page, KeyCode::Enter).as_slice(),
        [Action::SelectItem]
    ));
}

#[test]
fn a_load_that_lands_while_typing_is_kept() {
    let mut page = page();
    page.handle_event(&Action::OpenPinPopup);
    page.handle_event(&one_package());
    press(&mut page, KeyCode::Esc);
    // The row arrived while the input was open, so the actions list opens on it.
    press(&mut page, KeyCode::Enter);
    assert!(matches!(
        press(&mut page, KeyCode::Enter).as_slice(),
        [Action::SelectItem, Action::OpenPinPopup]
    ));
}

use std::collections::HashSet;
use std::error::Error;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend, style::Color};
use uview::message::{Action, Input, NavAction};
use uview::ui::components::features::help::{render_footer, render_help};
use uview::ui::keymap::{self, Mode};

const MODES: [Mode; 4] = [Mode::Table, Mode::Actions, Mode::Input, Mode::Help];

fn rows(terminal: &Terminal<TestBackend>) -> Vec<String> {
    let buffer = terminal.backend().buffer();
    buffer
        .content()
        .chunks(usize::from(buffer.area.width))
        .map(|row| row.iter().map(|cell| cell.symbol()).collect())
        .collect()
}

/// The key the footer or the help calls `name`.
fn key(name: &str) -> KeyEvent {
    let code = match name {
        "↑" => KeyCode::Up,
        "↓" => KeyCode::Down,
        "enter" => KeyCode::Enter,
        "esc" => KeyCode::Esc,
        "tab" => KeyCode::Tab,
        "shift+↑" => return KeyEvent::new(KeyCode::Up, KeyModifiers::SHIFT),
        "shift+↓" => return KeyEvent::new(KeyCode::Down, KeyModifiers::SHIFT),
        "pgup" => KeyCode::PageUp,
        "pgdn" => KeyCode::PageDown,
        "space" => KeyCode::Char(' '),
        "ctrl+c" => return KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
        name => name.chars().next().map_or(KeyCode::Null, KeyCode::Char),
    };
    KeyEvent::from(code)
}

fn press(mode: Mode, key: KeyEvent) -> Option<Action> {
    keymap::action_for(mode, &Input::Key(key))
}

#[test]
fn every_key_named_does_one_thing_where_it_is_named() {
    for mode in MODES {
        let mut named = HashSet::new();
        for binding in keymap::bindings(mode) {
            let (keys, _) = binding.help();
            for name in keys.split(' ') {
                // A key bound twice would only ever do what its first binding says.
                assert!(named.insert(name.to_string()), "{name} twice in {mode:?}");
                assert!(press(mode, key(name)).is_some(), "{name} in {mode:?}");
            }
        }
    }
}

#[test]
fn the_footer_lists_the_keys_that_work_where_the_focus_is() -> Result<(), Box<dyn Error>> {
    let footer = |mode| -> Result<Terminal<TestBackend>, Box<dyn Error>> {
        let mut terminal = Terminal::new(TestBackend::new(80, 1))?;
        terminal.draw(|frame| render_footer(frame, frame.area(), mode))?;
        Ok(terminal)
    };
    let table = footer(Mode::Table)?;
    assert_eq!(
        rows(&table)[0].trim_end(),
        "enter actions  tab switch  p add  u update  d remove  s sync  r reload  ? help"
    );
    // The keys stand out from what they do.
    let buffer = table.backend().buffer();
    assert_eq!(
        (buffer[(0, 0)].fg, buffer[(6, 0)].fg),
        (Color::Reset, Color::DarkGray)
    );
    assert_eq!(
        rows(&footer(Mode::Actions)?)[0].trim_end(),
        "enter run  esc close  ? help"
    );
    // The Tree tab names the tree's own keys; the table's still work there.
    assert_eq!(
        rows(&footer(Mode::Tree)?)[0].trim_end(),
        "←→ fold  +- fold all  o outdated  w why  esc back  ? help"
    );
    // The list of groups names what it is for.
    assert_eq!(
        rows(&footer(Mode::Groups)?)[0].trim_end(),
        "enter open  n new  r rename  d delete  esc back  ? help"
    );
    assert_eq!(
        rows(&footer(Mode::Input)?)[0].trim_end(),
        "enter confirm  esc cancel"
    );
    assert_eq!(rows(&footer(Mode::Help)?)[0].trim_end(), "esc close");
    Ok(())
}

#[test]
fn the_help_lists_every_binding_by_where_it_works() -> Result<(), Box<dyn Error>> {
    // The screen above the footer, at 80x24: the top, then each scroll as far as it goes.
    let mut terminal = Terminal::new(TestBackend::new(80, 23))?;
    let mut scroll = 0;
    terminal.draw(|frame| render_help(frame, frame.area(), &mut scroll))?;
    let mut rows = rows(&terminal);
    assert!(rows[0].contains("Keys (↑↓ to scroll)"));
    scroll = u16::MAX;
    terminal.draw(|frame| render_help(frame, frame.area(), &mut scroll))?;
    // Brought back to the last row, and no further however far it is asked to go.
    assert!(scroll > 0);
    let last = scroll;
    scroll += 5;
    terminal.draw(|frame| render_help(frame, frame.area(), &mut scroll))?;
    assert_eq!(scroll, last);
    for at in 1..=last {
        scroll = at;
        terminal.draw(|frame| render_help(frame, frame.area(), &mut scroll))?;
        rows.extend(self::rows(&terminal));
    }
    let text = rows.join("\n");
    for (place, bindings) in keymap::groups() {
        assert!(text.contains(place), "{place}");
        for binding in bindings {
            assert!(text.contains(binding.help().1), "{}", binding.help().1);
        }
    }
    // The keys in a column, then what they do.
    assert!(
        rows.iter().any(
            |row| row.contains("│  enter space      Open the actions for the selected package")
        )
    );
    assert!(
        rows.iter()
            .any(|row| row.contains("│  ctrl+c           Cancel"))
    );
    Ok(())
}

#[test]
fn question_mark_opens_the_help_and_esc_question_mark_or_q_close_it() {
    assert_eq!(press(Mode::Table, key("?")), Some(Action::Help));
    // However the terminal reports the Shift that types it.
    let shifted = KeyEvent::new(KeyCode::Char('?'), KeyModifiers::SHIFT);
    assert_eq!(press(Mode::Actions, shifted), Some(Action::Help));
    // In an input, it is typed.
    assert!(matches!(
        press(Mode::Input, key("?")),
        Some(Action::Nav(NavAction::Type(_)))
    ));
    for name in ["esc", "?", "q"] {
        assert_eq!(press(Mode::Help, key(name)), Some(Action::Help), "{name}");
    }
    // Over the help, the page's keys do nothing, and Ctrl+C still cancels.
    assert_eq!(press(Mode::Help, key("p")), None);
    assert_eq!(press(Mode::Help, key("ctrl+c")), Some(Action::Cancel));
}

#[test]
fn shift_tells_the_logs_arrows_from_the_selections() {
    let up = |modifiers| press(Mode::Table, KeyEvent::new(KeyCode::Up, modifiers));
    assert_eq!(up(KeyModifiers::SHIFT), Some(Action::LogBack));
    assert_eq!(up(KeyModifiers::NONE), Some(Action::Nav(NavAction::Up)));
    // For a character, Shift is the terminal's business: `?` and `B` come with or without it.
    let help = KeyEvent::new(KeyCode::Char('?'), KeyModifiers::SHIFT);
    assert_eq!(press(Mode::Table, help), Some(Action::Help));
}

#[test]
fn altgr_on_windows_is_the_character_it_types() {
    use uview::ui::keymap::typed;
    // `[` on a German layout, as Windows reports it.
    let altgr = KeyEvent::new(
        KeyCode::Char('['),
        KeyModifiers::CONTROL | KeyModifiers::ALT,
    );
    assert_eq!(typed(altgr, true), KeyEvent::from(KeyCode::Char('[')));
    // Elsewhere Ctrl+Alt is no AltGr: Esc and a key read together arrive with Alt.
    assert_eq!(typed(altgr, false), altgr);
    // Only for characters, and Ctrl alone stays Ctrl.
    let arrow = KeyEvent::new(KeyCode::Up, KeyModifiers::CONTROL | KeyModifiers::ALT);
    assert_eq!(typed(arrow, true), arrow);
    let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
    assert_eq!(typed(ctrl_c, true), ctrl_c);
}

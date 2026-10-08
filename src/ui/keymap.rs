//! What each key means, by where the focus is. One table, read by the key handling, the
//! footer and the help alike, so what they say cannot drift from what the keys do.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::message::{Action, DepsAction, Input, NavAction};

/// Where the focus is, which decides what keys mean.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// A table, with nothing open over it.
    Table,
    /// The actions list.
    Actions,
    /// An input, into which every key without a binding is typed.
    Input,
    /// The help overlay.
    Help,
}

/// A key as a binding names it: its code, and whether Ctrl is held. Shift is ignored, so `?`
/// matches however the terminal reports it.
pub struct Key {
    code: KeyCode,
    ctrl: bool,
}

impl Key {
    const fn new(code: KeyCode) -> Self {
        Self { code, ctrl: false }
    }

    const fn char(c: char) -> Self {
        Self::new(KeyCode::Char(c))
    }

    const fn ctrl(c: char) -> Self {
        Self {
            code: KeyCode::Char(c),
            ctrl: true,
        }
    }

    /// Whether `event` is this key. With Alt held, no key is: Esc and a key read together
    /// arrive as Alt+key, and would act on whatever the Esc was closing. In an input, Alt+key
    /// is typed, for the input's own editing.
    fn matches(&self, event: &KeyEvent) -> bool {
        event.code == self.code
            && event.modifiers.contains(KeyModifiers::CONTROL) == self.ctrl
            && !event.modifiers.contains(KeyModifiers::ALT)
    }

    /// How the footer and the help name it. crossterm's names differ by platform, Enter being
    /// Return on macOS.
    fn name(&self) -> String {
        let name = match self.code {
            KeyCode::Up => "↑".to_string(),
            KeyCode::Down => "↓".to_string(),
            KeyCode::Enter => "enter".to_string(),
            KeyCode::Esc => "esc".to_string(),
            KeyCode::Tab => "tab".to_string(),
            KeyCode::Char(' ') => "space".to_string(),
            KeyCode::Char(c) => c.to_string(),
            code => code.to_string(),
        };
        if self.ctrl {
            format!("ctrl+{name}")
        } else {
            name
        }
    }
}

/// Keys, what each does, and how the footer and the help describe them.
pub struct Binding {
    keys: &'static [(Key, Action)],
    /// A word for the footer, or `None` to leave the binding to the help.
    hint: Option<&'static str>,
    /// What it does, for the help.
    help: &'static str,
}

impl Binding {
    fn action_for(&self, event: &KeyEvent) -> Option<Action> {
        self.keys
            .iter()
            .find(|(key, _)| key.matches(event))
            .map(|(_, action)| action.clone())
    }

    /// What the footer shows for it, if anything: the first key of each thing it does, run
    /// together, such as `↑↓` for moving, and its hint.
    pub fn hint(&self) -> Option<(String, &'static str)> {
        let hint = self.hint?;
        let mut done: Vec<&Action> = Vec::new();
        let mut keys = String::new();
        for (key, action) in self.keys {
            if !done.contains(&action) {
                done.push(action);
                keys += &key.name();
            }
        }
        Some((keys, hint))
    }

    /// What the help shows for it: every key, and what it does.
    pub fn help(&self) -> (String, &'static str) {
        let keys: Vec<String> = self.keys.iter().map(|(key, _)| key.name()).collect();
        (keys.join(" "), self.help)
    }
}

const UP: Action = Action::Nav(NavAction::Up);
const DOWN: Action = Action::Nav(NavAction::Down);
const SELECT: Action = Action::Nav(NavAction::Select);
const BACK: Action = Action::Nav(NavAction::Back);
const SWITCH: Action = Action::Deps(DepsAction::SwitchTable);

const MOVE_KEYS: [(Key, Action); 4] = [
    (Key::new(KeyCode::Up), UP),
    (Key::new(KeyCode::Down), DOWN),
    (Key::char('k'), UP),
    (Key::char('j'), DOWN),
];
const SELECT_KEYS: [(Key, Action); 2] =
    [(Key::new(KeyCode::Enter), SELECT), (Key::char(' '), SELECT)];

const TABLE: &[Binding] = &[
    Binding {
        keys: &MOVE_KEYS,
        hint: Some("move"),
        help: "Move the selection",
    },
    Binding {
        keys: &SELECT_KEYS,
        hint: Some("actions"),
        help: "Open the actions for the selected package",
    },
    Binding {
        keys: &[
            (Key::new(KeyCode::Tab), SWITCH),
            (Key::char('['), SWITCH),
            (Key::char(']'), SWITCH),
        ],
        hint: Some("switch"),
        help: "Switch between the pyproject.toml and Environment tables",
    },
    // One row of the help, which fills 80x24.
    Binding {
        keys: &[
            (Key::char('p'), Action::Deps(DepsAction::OpenAdd)),
            (Key::char('P'), Action::Deps(DepsAction::OpenAddFromSource)),
        ],
        hint: Some("add"),
        help: "Add a package (p), or from an index, git or a path (P)",
    },
    // One row of the help, which fills 80x24, and left out of the footer, which would no
    // longer fit 80 columns.
    Binding {
        keys: &[
            (Key::char('u'), Action::Deps(DepsAction::Update)),
            (Key::char('d'), Action::Deps(DepsAction::Remove)),
        ],
        hint: None,
        help: "Update (u) or remove (d) the selected package",
    },
    Binding {
        keys: &[(Key::char('s'), Action::Deps(DepsAction::Sync))],
        hint: Some("sync"),
        help: "Sync the environment with uv sync",
    },
    Binding {
        keys: &[(Key::char('r'), Action::Deps(DepsAction::Refresh))],
        hint: Some("reload"),
        help: "Read both tables again",
    },
    Binding {
        keys: &[(Key::new(KeyCode::Esc), BACK)],
        hint: None,
        help: "Dismiss the message on the bottom line",
    },
];

const ACTIONS: &[Binding] = &[
    Binding {
        keys: &MOVE_KEYS,
        hint: Some("move"),
        help: "Move the highlight",
    },
    Binding {
        keys: &SELECT_KEYS,
        hint: Some("run"),
        help: "Run the highlighted action",
    },
    Binding {
        keys: &[(Key::new(KeyCode::Esc), BACK)],
        hint: Some("close"),
        help: "Close the list",
    },
];

const INPUT: &[Binding] = &[
    Binding {
        keys: &[(Key::new(KeyCode::Enter), Action::Nav(NavAction::Confirm))],
        hint: Some("confirm"),
        help: "Confirm what you typed",
    },
    Binding {
        keys: &[(Key::new(KeyCode::Esc), BACK)],
        hint: Some("cancel"),
        help: "Close the input",
    },
];

/// Wherever letters are not typed.
const OUTSIDE_INPUTS: &[Binding] = &[
    Binding {
        keys: &[(Key::char('?'), Action::Help)],
        hint: Some("help"),
        help: "Show this help",
    },
    Binding {
        keys: &[(Key::char('q'), Action::Quit)],
        hint: Some("quit"),
        help: "Quit, once whatever uv is running has stopped",
    },
];

const ANYWHERE: &[Binding] = &[Binding {
    keys: &[(Key::ctrl('c'), Action::Cancel)],
    hint: None,
    help: "Cancel what uv is running, and what waits behind it",
}];

/// The help itself, which `?` and `q` close rather than open or quit.
const HELP: &[Binding] = &[Binding {
    keys: &[
        (Key::new(KeyCode::Esc), Action::Help),
        (Key::char('?'), Action::Help),
        (Key::char('q'), Action::Help),
    ],
    hint: Some("close"),
    help: "Close the help",
}];

/// The bindings in force in `mode`, in the order the footer lists them. Where two bind a key,
/// the first wins.
pub fn bindings(mode: Mode) -> impl Iterator<Item = &'static Binding> {
    let lists: [&'static [Binding]; 3] = match mode {
        Mode::Table => [TABLE, OUTSIDE_INPUTS, ANYWHERE],
        Mode::Actions => [ACTIONS, OUTSIDE_INPUTS, ANYWHERE],
        Mode::Input => [INPUT, ANYWHERE, &[]],
        Mode::Help => [HELP, ANYWHERE, &[]],
    };
    lists.into_iter().flatten()
}

/// What `input` means in `mode`, if anything.
pub fn action_for(mode: Mode, input: &Input) -> Option<Action> {
    let Input::Key(key) = input;
    bindings(mode)
        .find_map(|binding| binding.action_for(key))
        // In an input, every other key goes into the field.
        .or_else(|| (mode == Mode::Input).then_some(Action::Nav(NavAction::Type(*key))))
}

/// Every binding, by where it works, as the help lists them.
pub fn groups() -> [(&'static str, &'static [Binding]); 5] {
    [
        ("In a table", TABLE),
        ("In the actions list", ACTIONS),
        ("In an input, where other keys are typed", INPUT),
        ("Outside an input", OUTSIDE_INPUTS),
        ("Anywhere", ANYWHERE),
    ]
}

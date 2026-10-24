//! What each key means, by where the focus is. One table, read by the key handling, the
//! footer and the help alike, so what they say cannot drift from what the keys do.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::message::{Action, DepsAction, Input, NavAction};

/// Where the focus is, which decides what keys mean.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// A table, with nothing open over it.
    Table,
    /// The actions list, or another list to choose from.
    Actions,
    /// The list of dependency groups, or a group's details, where `n`, `r` and `d` act.
    Groups,
    /// The Tree tab, where the table's keys work and the tree's own too.
    Tree,
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
            KeyCode::Left => "←".to_string(),
            KeyCode::Right => "→".to_string(),
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
    /// The footer's word for it, or one per thing it does; none leaves it to the help.
    hints: &'static [&'static str],
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

    /// What the footer shows for it: with one hint, the first key of each thing it does run
    /// together, such as `↑↓` for moving; with one hint per thing it does, each its own key, such
    /// as `u update` and `d remove`. A capital whose small letter is shown, such as `P` beside
    /// `p`, is left to the help.
    pub fn hints(&self) -> Vec<(String, &'static str)> {
        let mut shown: Vec<(&Action, String)> = Vec::new();
        for (key, action) in self.keys {
            let shifted = matches!(key.code, KeyCode::Char(c) if c.is_ascii_uppercase()
                && shown.iter().any(|(_, name)| name.contains(c.to_ascii_lowercase())));
            if !shifted && !shown.iter().any(|(done, _)| *done == action) {
                shown.push((action, key.name()));
            }
        }
        match self.hints {
            [] => Vec::new(),
            [hint] => vec![(shown.into_iter().map(|(_, name)| name).collect(), *hint)],
            hints => shown
                .into_iter()
                .zip(hints)
                .map(|((_, name), hint)| (name, *hint))
                .collect(),
        }
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
        hints: &[],
        help: "Move the selection",
    },
    Binding {
        keys: &SELECT_KEYS,
        hints: &["actions"],
        help: "Open the actions for the selected package",
    },
    Binding {
        keys: &[(Key::new(KeyCode::Tab), SWITCH), (Key::char(']'), SWITCH)],
        hints: &["switch"],
        help: "Switch to the next tab: pyproject.toml, Environment, Tree",
    },
    Binding {
        keys: &[(Key::char('['), Action::Deps(DepsAction::SwitchTableBack))],
        hints: &[],
        help: "Switch to the tab before",
    },
    // One row of the help, which fills 80x24.
    Binding {
        keys: &[
            (Key::char('p'), Action::Deps(DepsAction::OpenAdd)),
            (Key::char('P'), Action::Deps(DepsAction::OpenAddFromSource)),
        ],
        hints: &["add"],
        help: "Add a package (p), or from an index, git or a path (P)",
    },
    // One row of the help, which fills 80x24.
    Binding {
        keys: &[
            (Key::char('u'), Action::Deps(DepsAction::Update)),
            (Key::char('d'), Action::Deps(DepsAction::Remove)),
        ],
        hints: &["update", "remove"],
        help: "Update (u) or remove (d) the selected package",
    },
    // One row of the help, which fills 80x24.
    Binding {
        keys: &[
            (Key::char('s'), Action::Deps(DepsAction::Sync)),
            (Key::char('r'), Action::Deps(DepsAction::Refresh)),
        ],
        hints: &["sync", "reload"],
        help: "Sync with uv sync (s), or read both tables again (r)",
    },
    Binding {
        keys: &[(Key::char('b'), Action::Deps(DepsAction::Bump))],
        hints: &[],
        help: "Bump the package to its next patch, minor or major",
    },
    Binding {
        keys: &[(Key::char('v'), Action::Deps(DepsAction::OpenVersion))],
        hints: &[],
        help: "Bump the project's version, or set it",
    },
    // One row of the help.
    Binding {
        keys: &[
            (
                Key::char('l'),
                Action::Deps(DepsAction::Lock { upgrade: false }),
            ),
            (
                Key::char('L'),
                Action::Deps(DepsAction::Lock { upgrade: true }),
            ),
        ],
        hints: &[],
        help: "Lock with uv lock (l), or upgrade every package in it (L)",
    },
    Binding {
        keys: &[(Key::char('e'), Action::Deps(DepsAction::OpenEnvironment))],
        hints: &[],
        help: "Make the environment anew, empty it or delete it",
    },
    Binding {
        keys: &[
            (Key::char('B'), Action::Deps(DepsAction::Build)),
            (Key::char('U'), Action::Deps(DepsAction::Publish)),
        ],
        hints: &[],
        help: "Build with uv build (B), or publish the last build (U)",
    },
    Binding {
        keys: &[(Key::char('g'), Action::Deps(DepsAction::OpenGroups))],
        hints: &[],
        help: "Manage the dependency groups: create, rename, delete",
    },
    Binding {
        keys: &[(Key::char('/'), Action::Deps(DepsAction::OpenSearch))],
        hints: &[],
        help: "Look a package up by its name, to add it",
    },
    Binding {
        keys: &[(Key::char('i'), Action::Deps(DepsAction::OpenIndexes))],
        hints: &[],
        help: "Show the project's package indexes, to add or remove one",
    },
    Binding {
        keys: &[(Key::new(KeyCode::Esc), BACK)],
        hints: &[],
        help: "Dismiss the message on the bottom line",
    },
];

const ACTIONS: &[Binding] = &[
    Binding {
        keys: &MOVE_KEYS,
        hints: &[],
        help: "Move the highlight",
    },
    Binding {
        keys: &SELECT_KEYS,
        hints: &["run"],
        help: "Run the highlighted action",
    },
    Binding {
        keys: &[(Key::new(KeyCode::Esc), BACK)],
        hints: &["close"],
        help: "Close the list",
    },
    Binding {
        keys: &[(Key::char('d'), Action::Deps(DepsAction::Remove))],
        hints: &[],
        help: "Delete the highlighted index, in the list of indexes",
    },
];

const TREE: &[Binding] = &[
    Binding {
        keys: &[
            (Key::new(KeyCode::Left), Action::Deps(DepsAction::Collapse)),
            (Key::new(KeyCode::Right), Action::Deps(DepsAction::Expand)),
        ],
        hints: &["fold"],
        help: "Fold, or go to the parent (←); unfold (→)",
    },
    Binding {
        keys: &[
            (Key::char('+'), Action::Deps(DepsAction::ExpandAll)),
            (Key::char('-'), Action::Deps(DepsAction::CollapseAll)),
        ],
        hints: &["fold all"],
        help: "Unfold all (+), or fold all but direct dependencies (-)",
    },
    Binding {
        keys: &[(Key::char('o'), Action::Deps(DepsAction::Outdated))],
        hints: &["outdated"],
        help: "Show or hide which packages are behind, asking the index",
    },
    Binding {
        keys: &[(Key::char('w'), Action::Deps(DepsAction::Why))],
        hints: &["why"],
        help: "Show what pulls the highlighted package in",
    },
    Binding {
        keys: &[(Key::new(KeyCode::Esc), BACK)],
        hints: &["back"],
        help: "Back from what pulls a package in to the tree",
    },
];

const GROUPS: &[Binding] = &[
    Binding {
        keys: &MOVE_KEYS,
        hints: &[],
        help: "Move the highlight",
    },
    Binding {
        keys: &SELECT_KEYS,
        hints: &["open"],
        help: "Open the highlighted group, or New group…",
    },
    // One row of the help each, and each in the footer: these are what the list is for.
    Binding {
        keys: &[(Key::char('n'), Action::Deps(DepsAction::New))],
        hints: &["new"],
        help: "Add a group",
    },
    Binding {
        keys: &[(Key::char('r'), Action::Deps(DepsAction::Rename))],
        hints: &["rename"],
        help: "Rename the highlighted group, or the open one",
    },
    Binding {
        keys: &[(Key::char('d'), Action::Deps(DepsAction::Remove))],
        hints: &["delete"],
        help: "Delete the highlighted group, or the open one",
    },
    Binding {
        keys: &[(Key::new(KeyCode::Esc), BACK)],
        hints: &["back"],
        help: "Back from a group to the list, or close the list",
    },
];

const INPUT: &[Binding] = &[
    Binding {
        keys: &[(Key::new(KeyCode::Enter), Action::Nav(NavAction::Confirm))],
        hints: &["confirm"],
        help: "Confirm what you typed",
    },
    Binding {
        keys: &[(Key::new(KeyCode::Esc), BACK)],
        hints: &["cancel"],
        help: "Close the input",
    },
];

/// Wherever letters are not typed.
const OUTSIDE_INPUTS: &[Binding] = &[
    Binding {
        keys: &[(Key::char('?'), Action::Help)],
        hints: &["help"],
        help: "Show this help",
    },
    Binding {
        keys: &[(Key::char('q'), Action::Quit)],
        hints: &[],
        help: "Quit, once whatever uv is running has stopped",
    },
];

const ANYWHERE: &[Binding] = &[Binding {
    keys: &[(Key::ctrl('c'), Action::Cancel)],
    hints: &[],
    help: "Cancel what uv is running, and what waits behind it",
}];

/// The help itself, which `?` and `q` close rather than open or quit. Its title says when it
/// scrolls.
const HELP: &[Binding] = &[
    Binding {
        keys: &MOVE_KEYS,
        hints: &[],
        help: "Scroll the help",
    },
    Binding {
        keys: &[
            (Key::new(KeyCode::Esc), Action::Help),
            (Key::char('?'), Action::Help),
            (Key::char('q'), Action::Help),
        ],
        hints: &["close"],
        help: "Close the help",
    },
];

/// The bindings in force in `mode`, in the order the footer lists them. Where two bind a key,
/// the first wins.
pub fn bindings(mode: Mode) -> impl Iterator<Item = &'static Binding> {
    let lists: [&'static [Binding]; 4] = match mode {
        Mode::Table => [TABLE, OUTSIDE_INPUTS, ANYWHERE, &[]],
        Mode::Tree => [TREE, TABLE, OUTSIDE_INPUTS, ANYWHERE],
        Mode::Actions => [ACTIONS, OUTSIDE_INPUTS, ANYWHERE, &[]],
        Mode::Groups => [GROUPS, OUTSIDE_INPUTS, ANYWHERE, &[]],
        Mode::Input => [INPUT, ANYWHERE, &[], &[]],
        Mode::Help => [HELP, ANYWHERE, &[], &[]],
    };
    lists.into_iter().flatten()
}

/// The bindings the footer names in `mode`: all of them, but on the Tree tab only the tree's
/// own, as the table's would not fit beside them.
pub fn footer(mode: Mode) -> Box<dyn Iterator<Item = &'static Binding>> {
    match mode {
        Mode::Tree => Box::new(TREE.iter().chain(OUTSIDE_INPUTS).chain(ANYWHERE)),
        mode => Box::new(bindings(mode)),
    }
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
pub fn groups() -> [(&'static str, &'static [Binding]); 7] {
    [
        ("In a table", TABLE),
        ("On the Tree tab, besides the table's", TREE),
        ("In the actions list", ACTIONS),
        ("In the list of groups", GROUPS),
        ("In an input, where other keys are typed", INPUT),
        ("Outside an input", OUTSIDE_INPUTS),
        ("Anywhere", ANYWHERE),
    ]
}

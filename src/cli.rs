//! What uview is asked on its command line, answered before it takes the terminal over.

/// uview's version, as Cargo.toml has it: the one that `--version`, the help and the Python
/// package show (ADR-0017).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// What the arguments ask for.
#[derive(Debug, PartialEq)]
pub enum Invocation {
    /// No arguments: the app.
    Run,
    /// Print this, then exit: the version or the usage.
    Print(String),
    /// Print this to stderr, then exit with status 2.
    Misuse(String),
}

/// What `args`, the arguments after the program's name, ask for.
pub fn invocation(args: &[String]) -> Invocation {
    match args {
        [] => Invocation::Run,
        [arg] if arg == "--version" || arg == "-V" => Invocation::Print(format!("uview {VERSION}")),
        [arg] if arg == "--help" || arg == "-h" => Invocation::Print(usage()),
        [arg, ..] => Invocation::Misuse(format!(
            "uview: unexpected argument {arg:?}\n\nUsage: uview [--help | --version]"
        )),
    }
}

fn usage() -> String {
    format!(
        "uview {VERSION}: a terminal UI for uv, the Python package manager

Usage: uview [--help | --version]

Run it in a Python project, or below one: it works on the project uv would.
Press ? in it for every key. Colours: ~/.config/uview/config.toml, which C writes.

Options:
  -h, --help     Print this help
  -V, --version  Print the version"
    )
}

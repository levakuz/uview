# 0016. Keep the user's settings in a TOML file where uv keeps its own

- **Status:** Accepted (2026-11-06)
- **Implemented by:** #33

## Context

#33 gives uview a dark and a light theme, and users want to choose the colours themselves.
That needs a place for settings that outlive a run, which uview has never had. Where it is and
what it looks like are hard to change once people have files of their own.

Tools put such files in several places. uv, Helix, starship, Alacritty and gitui use
`~/.config/<tool>/` on macOS as well as Linux. lazygit uses `~/Library/Application Support/`
on macOS, which its users often cannot find. Older tools use a dotfile in the home directory.
The formats vary as much: TOML for uv, ruff, Cargo, Helix and Alacritty, JSON with comments for
VS Code, Zed and Windows Terminal, YAML for lazygit.

The theme also has to suit the terminal's background, which most terminals tell when asked
(OSC 11), and some say in `COLORFGBG`.

## Decision

We will read one optional settings file, which uview never writes, from where uv keeps
`uv.toml`: `$XDG_CONFIG_HOME/uview/config.toml`, else `~/.config/uview/config.toml` on macOS
and Linux, and `%APPDATA%\uview\config.toml` on Windows. `UVIEW_CONFIG_FILE` names another
file instead.

It is TOML. `theme` chooses the built-in theme, `dark` or `light`, and `[colors]` sets any
role's colour as ratatui parses it: a name, `reset`, `0`–`255` or `#rrggbb`. A key that means
nothing, or a colour that does not parse, is reported on the status line; uview starts anyway,
with the theme's colour for anything it could not read.

Without `theme`, uview asks the terminal for its background before it takes over the screen,
with `terminal-colorsaurus`, and starts dark if the terminal does not answer.

## Consequences

- uview's users edit `pyproject.toml` and `uv.toml` daily: the file is in the format, and next
  to the file, they already know. TOML takes comments, which a file edited by hand needs.
- No new dependency to read it: `toml` and `serde` were there. Querying the background adds
  `terminal-colorsaurus`.
- Asking the terminal costs a round trip at startup, which can reach its one-second timeout
  over a slow connection to a terminal that does not answer. `theme` in the file skips it.
- Editors complete JSON from a schema, and TOML only with Taplo; a schema can be published
  later without changing the format.

## Alternatives considered

- **`~/.uview`.** One place on every system, but against XDG and away from uv's file.
- **JSON.** The most common format, but strict JSON takes no comments and no trailing comma,
  and the dialect that does would need a parser of its own.
- **`COLORFGBG` alone.** No dependency, but Terminal.app and VS Code do not set it, so their
  light themes would need the file.

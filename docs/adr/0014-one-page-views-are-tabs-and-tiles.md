# 0014. One page: new views are tabs and tiles, not pages

- **Status:** Accepted (2026-10-18)
- **Implemented by:** #26 (closed as not planned in favour of this decision)

## Context

`ARCHITECTURE.md` §13.8 planned a stack of pages, `Vec<Box<dyn Component>>`, with a home page
listing the sections (`pages/home.rs`), and #26 asked for it: navigation between pages, state
that survives it, global keys above the stack. The sections coming next are a dependency tree
(#27) and the project's environment (#29), then Python versions, the cache and scripts.

The app already shows everything about a project at once: the dependency tables on the left,
the project tile and the uv log on the right, and every action as a key or a popup over that
screen. A page per section would hide the tables behind a menu, and add navigation keys and a
"back" that the user has no use for while working in one project.

## Decision

We will keep one page. A new view joins the screen that exists: as another tab of the tables
block on the left, as the dependency tree does (`pyproject.toml • Environment • Tree`), or in
the right column, in the project tile or a tab of it, as the environment's details do. There is
no home page and no page stack. `App` keeps holding the one page, and the keys stay global as
they are, since every one already works on that page.

## Consequences

- No navigation to learn: Tab, `[` and `]` already move between the tables' tabs.
- Each view lives where the user is already looking, beside the uv log and the project.
- §13.8 of `ARCHITECTURE.md` and step 9 of its migration path change accordingly; `pages/`
  keeps its one page.
- A view that needs the whole screen would need this revisited; none does so far.

## Alternatives considered

- **A stack of pages with a home page**, as planned. Free "back" and room for any number of
  screens, at the cost of hiding the tables behind navigation for views that fit beside them.
- **One page per section, always alive.** As above, with state kept; more machinery still.

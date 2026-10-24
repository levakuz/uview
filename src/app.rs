use std::sync::Arc;
use std::time::{Duration, Instant};

use color_eyre::Result;
use crossterm::event::{self, KeyEventKind};
use ratatui::DefaultTerminal;
use ratatui::layout::{Constraint, Layout};
use tokio::sync::mpsc;

use crate::controllers::project::ProjectPaths;
use crate::index::RealIndexClient;
use crate::message::{Action, Command, Event, Input, NavAction};
use crate::runtime::jobs::JobRunner;
use crate::state::{AppState, Loadable, Message, ProjectState};
use crate::ui::Component;
use crate::ui::components::features::help::{render_footer, render_help};
use crate::ui::components::features::project_tile::render_project_tile;
use crate::ui::components::features::uv_log::render_uv_log;
use crate::ui::components::shared::status_bar::{render_status_line, spinner};
use crate::ui::keymap::{self, Mode};
use crate::ui::pages::dependencies::DependenciesPage;
use crate::uv::RealUvClient;

pub struct App {
    page: DependenciesPage,
    should_quit: bool,
    /// Whether the help is open, over every page.
    help: bool,
    /// How far the help is scrolled down, in rows.
    help_scroll: u16,
    /// What the app knows. Only `receive` and Esc change it.
    state: AppState,
    /// Runs the commands the page returns.
    runner: JobRunner<RealUvClient, RealIndexClient>,
    /// Where the runner reports what commands produced.
    events_rx: mpsc::UnboundedReceiver<Event>,
}

impl App {
    pub fn new(paths: ProjectPaths) -> Self {
        let (events_tx, events_rx) = mpsc::unbounded_channel();
        Self {
            page: DependenciesPage::new(),
            should_quit: false,
            help: false,
            help_scroll: 0,
            state: AppState {
                project: ProjectState::new(paths.root.display().to_string()),
                ..AppState::default()
            },
            runner: JobRunner::new(
                Arc::new(RealUvClient::new(&paths.root)),
                Arc::new(RealIndexClient::new()),
                paths,
                events_tx,
            ),
            events_rx,
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        // Also installs a panic hook that restores the terminal; must stay after color_eyre::install.
        let mut terminal = ratatui::init();
        let since = Instant::now();
        self.state.deps.pyproject = Loadable::Loading { since };
        self.state.deps.env = Loadable::Loading { since };
        self.state.project.info = Loadable::Loading { since };
        self.runner.execute(Command::Reload);

        // Restore whatever the loop returns: an I/O error must not leave the terminal raw.
        let result = self.event_loop(&mut terminal);
        ratatui::restore();
        result
    }

    fn event_loop(&mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        let clock = Instant::now();
        loop {
            let mode = self.mode();
            terminal.draw(|frame| {
                let [screen, footer] =
                    Layout::vertical([Constraint::Min(0), Constraint::Length(1)])
                        .areas(frame.area());
                let [main, status] =
                    Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).areas(screen);
                // The tables get the larger share: their names should not be cut.
                let [left, right] =
                    Layout::horizontal([Constraint::Percentage(60), Constraint::Percentage(40)])
                        .areas(main);
                self.page.render(frame, left, &self.state);
                let packages = match &self.state.deps.env {
                    Loadable::Loaded(rows) => Some(rows.len()),
                    _ => None,
                };
                let log = render_project_tile(frame, right, &self.state.project, packages);
                render_uv_log(frame, log, &self.state.jobs);
                let activity = self
                    .state
                    .jobs
                    .summary()
                    .map(|summary| format!("{} {summary}", spinner(clock.elapsed())));
                render_status_line(
                    frame,
                    status,
                    activity.as_deref(),
                    self.state.message.as_ref(),
                );
                render_footer(frame, footer, mode);
                if self.help {
                    render_help(frame, screen, &mut self.help_scroll);
                }
            })?;

            if event::poll(Duration::from_millis(50))?
                && let event::Event::Key(key) = event::read()?
                && key.kind == KeyEventKind::Press
                && let Some(action) = keymap::action_for(mode, &Input::Key(key))
            {
                self.dispatch(action);
            }

            // Take in what commands produced
            while let Ok(event) = self.events_rx.try_recv() {
                self.receive(event);
            }

            if self.should_quit && self.state.jobs.is_idle() {
                break;
            }
        }
        Ok(())
    }

    /// Where the focus is: the help when it is open, else wherever the page has it.
    fn mode(&self) -> Mode {
        if self.help {
            Mode::Help
        } else {
            self.page.mode()
        }
    }

    /// Hands `action` to the page and the commands it returns to the runner. Components
    /// return commands, never actions, so nothing comes back into dispatch.
    fn dispatch(&mut self, action: Action) {
        match action {
            // Quits once the jobs have stopped, so a `uv` interrupted mid-way can put back what
            // it changed.
            Action::Quit => {
                self.runner.cancel();
                self.should_quit = true;
            }
            Action::Cancel => self.runner.cancel(),
            Action::Help => {
                self.help = !self.help;
                self.help_scroll = 0;
            }
            Action::Nav(NavAction::Up) if self.help => {
                self.help_scroll = self.help_scroll.saturating_sub(1);
            }
            Action::Nav(NavAction::Down) if self.help => self.help_scroll += 1,
            action => {
                // Esc closes any popup and also dismisses the message.
                if let Action::Nav(NavAction::Back) = action {
                    self.state.message = None;
                }
                for command in self.page.update(&action, &self.state) {
                    self.runner.execute(command);
                }
            }
        }
    }

    /// Puts what a command produced into the state, where every view reads it.
    fn receive(&mut self, event: Event) {
        match event {
            event @ (Event::PyprojectLoaded { .. }
            | Event::EnvLoaded { .. }
            | Event::TreeLoaded { .. }) => self.state.deps.apply(event),
            Event::ProjectLoaded { generation, loaded } => {
                self.state.project.apply_info(generation, loaded);
            }
            Event::VersionsPreviewed(loaded) => self.state.project.bumps = loaded.into(),
            Event::WhyLoaded { name, loaded } => {
                self.state.deps.why = Some((name, loaded.into()));
            }
            Event::EnvInfoLoaded { generation, info } => {
                self.state.project.apply_environment(generation, info);
            }
            Event::PythonsListed(loaded) => self.state.project.pythons = loaded.into(),
            Event::LockChecked { generation, state } => {
                self.state.project.apply_lock(generation, state);
            }
            Event::VersionsListed { name, loaded } => {
                self.state.deps.versions = Some((name, loaded.into()));
            }
            // What the tables show may be stale. Not worth reading again on the way out.
            Event::ProjectMutated => {
                // The next bumps may differ; `v` asks again.
                self.state.project.bumps = Loadable::NotLoaded;
                if !self.should_quit {
                    self.runner.execute(Command::Reload);
                }
            }
            Event::Notice(msg) => self.state.message = Some(Message::Info(msg)),
            Event::Error(msg) => self.state.message = Some(Message::Error(msg)),
            Event::JobQueued { .. }
            | Event::JobStarted(_)
            | Event::JobOutput(..)
            | Event::JobFinished(_) => self.state.jobs.apply(&event),
        }
    }
}

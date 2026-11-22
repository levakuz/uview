use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use color_eyre::Result;
use crossterm::event::{self, KeyEventKind};
use ratatui::DefaultTerminal;
use ratatui::layout::{Constraint, Layout};
use tokio::sync::mpsc;

use crate::config::template;
use crate::controllers::project::ProjectPaths;
use crate::controllers::publish::credentials;
use crate::index::RealIndexClient;
use crate::message::{Action, Command, Event, Input, NavAction};
use crate::runtime::jobs::JobRunner;
use crate::state::{AppState, Loadable, Message, ProjectState};
use crate::ui::Component;
use crate::ui::components::features::help::{render_footer, render_help};
use crate::ui::components::features::project_tile::render_project_tile;
use crate::ui::components::features::uv_log::render_uv_log;
use crate::ui::components::shared::status_bar::{SPINNER_FRAME, render_status_line, spinner};
use crate::ui::keymap::{self, Mode};
use crate::ui::pages::dependencies::DependenciesPage;
use crate::ui::theme::theme;
use crate::uv::RealUvClient;

/// How many rows of the uv log a key scrolls.
const LOG_STEP: usize = 5;

pub struct App {
    page: DependenciesPage,
    should_quit: bool,
    /// Whether the help is open, over every page.
    help: bool,
    /// How far the help is scrolled down, in rows.
    help_scroll: u16,
    /// What the app knows. Only `receive`, Esc and a settings file with nowhere to go change it.
    state: AppState,
    /// Where the settings file is, or would be; `None` with nowhere to put one.
    config: Option<PathBuf>,
    /// How many rows the uv log is scrolled back from its newest.
    log_back: usize,
    /// Runs the commands the page returns.
    runner: JobRunner<RealUvClient, RealIndexClient>,
    /// Where the runner reports what commands produced.
    events_rx: mpsc::UnboundedReceiver<Event>,
}

impl App {
    /// The app for the project at `paths`, with the settings file at `config`, if there is a
    /// place for one; `problem` says what was wrong with it, until a later message replaces it.
    pub fn new(paths: ProjectPaths, config: Option<PathBuf>, problem: Option<String>) -> Self {
        let (events_tx, events_rx) = mpsc::unbounded_channel();
        let mut project = ProjectState::new(paths.root.display().to_string());
        project.credentials = credentials(|key| std::env::var(key).ok());
        Self {
            page: DependenciesPage::new(),
            should_quit: false,
            help: false,
            help_scroll: 0,
            state: AppState {
                project,
                message: problem.map(Message::Error),
                ..AppState::default()
            },
            config,
            log_back: 0,
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
        // Drawn only when something changed: a key, a resize or an event. While a job runs, the
        // spinner turns, and the `Loading…` of a table counts its seconds, so it is drawn on time.
        let mut changed = true;
        let mut drawn = Instant::now();
        loop {
            let mode = self.mode();
            let animating = !self.state.jobs.is_idle() && drawn.elapsed() >= SPINNER_FRAME;
            if changed || animating {
                changed = false;
                drawn = Instant::now();
                self.draw(terminal, mode, clock)?;
            }

            if event::poll(Duration::from_millis(50))? {
                match event::read()? {
                    event::Event::Key(key) if key.kind == KeyEventKind::Press => {
                        changed = true;
                        if let Some(action) = keymap::action_for(mode, &Input::Key(key)) {
                            self.dispatch(action);
                        }
                    }
                    event::Event::Resize(..) => changed = true,
                    _ => {}
                }
            }

            // Take in what commands produced
            while let Ok(event) = self.events_rx.try_recv() {
                self.receive(event);
                changed = true;
            }

            if self.should_quit && self.state.jobs.is_idle() {
                break;
            }
        }
        Ok(())
    }

    fn draw(&mut self, terminal: &mut DefaultTerminal, mode: Mode, clock: Instant) -> Result<()> {
        terminal.draw(|frame| {
            let [screen, footer] =
                Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).areas(frame.area());
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
            render_uv_log(frame, log, &self.state.jobs, &mut self.log_back);
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
            // With nothing to cancel, whoever pressed it most likely wants out.
            Action::Cancel if self.state.jobs.is_idle() => {
                let hint = "Nothing is running to cancel; q quits".to_string();
                self.state.message = Some(Message::Info(hint));
            }
            Action::Cancel => self.runner.cancel(),
            Action::LogBack => self.log_back += LOG_STEP,
            Action::LogForward => self.log_back = self.log_back.saturating_sub(LOG_STEP),
            Action::WriteConfig => match &self.config {
                Some(path) => self.runner.execute(Command::WriteConfig {
                    path: path.clone(),
                    contents: template(theme()),
                }),
                None => {
                    let reason = "no HOME, XDG_CONFIG_HOME or UVIEW_CONFIG_FILE to put it under";
                    let message = format!("Nowhere to write a settings file: {reason}");
                    self.state.message = Some(Message::Error(message));
                }
            },
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
            Event::Built(artifacts) => {
                self.state.project.built = artifacts;
                self.state.project.builds += 1;
            }
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
            // A new job's output is what to see: back to the newest.
            Event::JobQueued { .. } => {
                self.log_back = 0;
                self.state.jobs.apply(&event);
            }
            Event::JobStarted(_) | Event::JobOutput(..) | Event::JobFinished(_) => {
                self.state.jobs.apply(&event)
            }
        }
    }
}

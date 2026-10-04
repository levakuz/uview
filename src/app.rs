use std::sync::Arc;
use std::time::{Duration, Instant};

use color_eyre::Result;
use crossterm::event::{self, KeyEventKind};
use ratatui::DefaultTerminal;
use ratatui::layout::{Constraint, Layout};
use tokio::sync::mpsc;

use crate::message::{Action, Command, Event, Input, NavAction};
use crate::runtime::jobs::JobRunner;
use crate::state::{AppState, Loadable, Message, ProjectState};
use crate::ui::Component;
use crate::ui::components::features::project_tile::render_project_tile;
use crate::ui::components::features::uv_log::render_uv_log;
use crate::ui::components::shared::status_bar::{render_status_line, spinner};
use crate::ui::pages::dependencies::DependenciesPage;
use crate::uv::RealUvClient;

pub struct App {
    page: DependenciesPage,
    should_quit: bool,
    /// What the app knows. Only `receive` and Esc change it.
    state: AppState,
    /// Runs the commands the page returns.
    runner: JobRunner<RealUvClient>,
    /// Where the runner reports what commands produced.
    events_rx: mpsc::UnboundedReceiver<Event>,
}

impl App {
    pub fn new(project: String) -> Self {
        let (events_tx, events_rx) = mpsc::unbounded_channel();
        Self {
            page: DependenciesPage::new(),
            should_quit: false,
            state: AppState {
                project: ProjectState { root: project },
                ..AppState::default()
            },
            runner: JobRunner::new(Arc::new(RealUvClient), events_tx),
            events_rx,
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        // Also installs a panic hook that restores the terminal; must stay after color_eyre::install.
        let mut terminal = ratatui::init();
        let since = Instant::now();
        self.state.deps.pyproject = Loadable::Loading { since };
        self.state.deps.env = Loadable::Loading { since };
        self.runner.execute(Command::Reload);

        // Restore whatever the loop returns: an I/O error must not leave the terminal raw.
        let result = self.event_loop(&mut terminal);
        ratatui::restore();
        result
    }

    fn event_loop(&mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        let clock = Instant::now();
        loop {
            terminal.draw(|frame| {
                let [main, status] = Layout::vertical([Constraint::Min(0), Constraint::Length(1)])
                    .areas(frame.area());
                let [left, right] =
                    Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
                        .areas(main);
                self.page.render(frame, left, &self.state);
                let log = render_project_tile(frame, right, &self.state.project.root);
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
            })?;

            if event::poll(Duration::from_millis(50))?
                && let event::Event::Key(key) = event::read()?
                && key.kind == KeyEventKind::Press
                && let Some(action) = self.page.action_for(&Input::Key(key))
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
            event @ (Event::PyprojectLoaded { .. } | Event::EnvLoaded { .. }) => {
                self.state.deps.apply(event)
            }
            // What the tables show may be stale. Not worth reading again on the way out.
            Event::ProjectMutated => {
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

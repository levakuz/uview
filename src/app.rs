use std::sync::Arc;
use std::time::{Duration, Instant};

use color_eyre::Result;
use crossterm::event::{self, KeyEventKind};
use ratatui::DefaultTerminal;
use ratatui::layout::{Constraint, Layout};
use tokio::sync::mpsc;

use crate::message::{Action, Command, Event, Input, NavAction};
use crate::runtime::jobs::{JobRunner, JobsState};
use crate::ui::Component;
use crate::ui::components::features::project_tile::render_project_tile;
use crate::ui::components::features::uv_log::render_uv_log;
use crate::ui::components::shared::status_bar::{StatusMessage, render_status_line, spinner};
use crate::ui::pages::dependencies::DependenciesPage;
use crate::uv::RealUvClient;

pub struct App {
    page: DependenciesPage,
    should_quit: bool,
    /// The latest error or notice; Esc dismisses it.
    status: Option<StatusMessage>,
    /// The project root, shown in the project tile.
    project: String,
    /// Runs the commands the page returns.
    runner: JobRunner<RealUvClient>,
    /// What the runner is doing, shown on the status line, and what its `uv` wrote, in the log.
    jobs: JobsState,
    /// Where the runner reports what commands produced.
    events_rx: mpsc::UnboundedReceiver<Event>,
}

impl App {
    pub fn new(project: String) -> Self {
        let (events_tx, events_rx) = mpsc::unbounded_channel();
        Self {
            page: DependenciesPage::new(),
            should_quit: false,
            status: None,
            project,
            runner: JobRunner::new(Arc::new(RealUvClient), events_tx),
            jobs: JobsState::default(),
            events_rx,
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        // Also installs a panic hook that restores the terminal; must stay after color_eyre::install.
        let mut terminal = ratatui::init();
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
                self.page.render(frame, left);
                let log = render_project_tile(frame, right, &self.project);
                render_uv_log(frame, log, &self.jobs);
                let activity = self
                    .jobs
                    .summary()
                    .map(|summary| format!("{} {summary}", spinner(clock.elapsed())));
                render_status_line(frame, status, activity.as_deref(), self.status.as_ref());
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

            if self.should_quit && self.jobs.is_idle() {
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
                    self.status = None;
                }
                for command in self.page.update(&action) {
                    self.runner.execute(command);
                }
            }
        }
    }

    /// Shows a notice, an error or a job; anything else goes to the page.
    fn receive(&mut self, event: Event) {
        match event {
            Event::Notice(msg) => self.status = Some(StatusMessage::Info(msg)),
            Event::Error(msg) => self.status = Some(StatusMessage::Error(msg)),
            Event::JobQueued { .. }
            | Event::JobStarted(_)
            | Event::JobOutput(..)
            | Event::JobFinished(_) => self.jobs.apply(&event),
            event => self.page.apply(&event),
        }
    }
}

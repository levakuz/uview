use std::time::Duration;

use color_eyre::Result;
use crossterm::event::{self, KeyEventKind};
use ratatui::DefaultTerminal;
use ratatui::layout::{Constraint, Layout};
use tokio::sync::mpsc;

use crate::message::{Action, Command, Event, Input, NavAction};
use crate::runtime;
use crate::ui::Component;
use crate::ui::components::shared::status_bar::{StatusMessage, render_status_line};
use crate::ui::pages::dependencies::DependenciesPage;

pub struct App {
    page: DependenciesPage,
    should_quit: bool,
    /// The latest error or notice; Esc dismisses it.
    status: Option<StatusMessage>,
    /// The project root, shown at the right of the status line.
    project: String,
    /// Where the runtime reports what commands produced.
    events_tx: mpsc::UnboundedSender<Event>,
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
            events_tx,
            events_rx,
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        // Also installs a panic hook that restores the terminal; must stay after color_eyre::install.
        let mut terminal = ratatui::init();
        runtime::execute(Command::Reload, &self.events_tx);

        // Restore whatever the loop returns: an I/O error must not leave the terminal raw.
        let result = self.event_loop(&mut terminal);
        ratatui::restore();
        result
    }

    fn event_loop(&mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        loop {
            terminal.draw(|frame| {
                let [main, status] = Layout::vertical([Constraint::Min(0), Constraint::Length(1)])
                    .areas(frame.area());
                self.page.render(frame, main);
                render_status_line(frame, status, self.status.as_ref(), &self.project);
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

            if self.should_quit {
                break;
            }
        }
        Ok(())
    }

    /// Hands `action` to the page and the commands it returns to the runtime. Components
    /// return commands, never actions, so nothing comes back into dispatch.
    fn dispatch(&mut self, action: Action) {
        if let Action::Quit = action {
            self.should_quit = true;
            return;
        }
        // Esc closes any popup and also dismisses the message.
        if let Action::Nav(NavAction::Back) = action {
            self.status = None;
        }
        for command in self.page.update(&action) {
            runtime::execute(command, &self.events_tx);
        }
    }

    /// Shows a notice or an error; anything else goes to the page.
    fn receive(&mut self, event: Event) {
        match event {
            Event::Notice(msg) => self.status = Some(StatusMessage::Info(msg)),
            Event::Error(msg) => self.status = Some(StatusMessage::Error(msg)),
            event => self.page.apply(&event),
        }
    }
}

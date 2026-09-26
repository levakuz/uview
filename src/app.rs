use std::time::Duration;

use color_eyre::Result;
use crossterm::event::{self, Event, KeyEventKind};
use ratatui::layout::{Constraint, Layout};
use ratatui::style::Style;
use ratatui::widgets::Paragraph;
use tokio::sync::mpsc;

use crate::controllers::dependencies;
use crate::events::Action;
use crate::ui::Component;
use crate::ui::components::shared::status_bar::StatusMessage;
use crate::ui::pages::dependencies::DependenciesPage;

pub struct App {
    page: DependenciesPage,
    should_quit: bool,
    /// The latest error or notice; Esc dismisses it.
    status: Option<StatusMessage>,
    /// Shown in the status line whenever no message is.
    project: String,
    tx: mpsc::UnboundedSender<Action>,
    rx: mpsc::UnboundedReceiver<Action>,
}

impl App {
    pub fn new(project: String) -> Self {
        let (tx, rx) = mpsc::unbounded_channel::<Action>();
        Self {
            page: DependenciesPage::new(tx.clone()),
            should_quit: false,
            status: None,
            project,
            tx,
            rx,
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        // Also installs a panic hook that restores the terminal; must stay after color_eyre::install.
        let mut terminal = ratatui::init();

        // Spawn controller tasks
        let tx_pyproject = self.tx.clone();
        tokio::spawn(async move {
            match dependencies::load_pyproject_deps().await {
                Ok(deps) => {
                    let _ = tx_pyproject.send(Action::LoadedPyprojectDeps(deps));
                }
                Err(e) => {
                    let _ = tx_pyproject.send(Action::Error(format!("pyproject.toml: {e}")));
                }
            }
        });

        let tx_env = self.tx.clone();
        tokio::spawn(async move {
            match dependencies::load_env_deps().await {
                Ok(deps) => {
                    let _ = tx_env.send(Action::LoadedEnvDeps(deps));
                }
                Err(e) => {
                    let _ = tx_env.send(Action::Error(e.to_string()));
                }
            }
        });

        loop {
            terminal.draw(|frame| {
                let [main, status] = Layout::vertical([Constraint::Min(0), Constraint::Length(1)])
                    .areas(frame.area());
                self.page.render(frame, main);
                match &self.status {
                    Some(message) => message.render(frame, status),
                    None => frame.render_widget(
                        Paragraph::new(self.project.as_str()).style(Style::new().dark_gray()),
                        status,
                    ),
                }
            })?;

            // Poll crossterm events
            if event::poll(Duration::from_millis(50))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        let action = Some(Action::ButtonPressed(key));
                        if let Some(action) = action {
                            self.dispatch(action);
                        }
                    }
                }
            }

            // Drain controller results
            while let Ok(action) = self.rx.try_recv() {
                self.dispatch(action);
            }

            if self.should_quit {
                break;
            }
        }

        ratatui::restore();
        Ok(())
    }

    fn dispatch(&mut self, action: Action) {
        match &action {
            Action::Quit => self.should_quit = true,
            Action::Error(msg) => self.status = Some(StatusMessage::Error(msg.clone())),
            Action::Notice(msg) => self.status = Some(StatusMessage::Info(msg.clone())),
            _ => {
                // Esc arrives as StopEditing: it closes any popup and dismisses the message.
                if matches!(action, Action::StopEditing) {
                    self.status = None;
                }
                if let Some(next) = self.page.handle_event(&action) {
                    self.dispatch(next);
                }
            }
        }
    }
}

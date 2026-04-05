use std::time::Duration;

use color_eyre::Result;
use crossterm::event::{self, Event, KeyEventKind};
use tokio::sync::mpsc;

use crate::controllers::dependencies;
use crate::events::Action;
use crate::ui::Component;
use crate::ui::pages::dependencies::DependenciesPage;

pub struct App {
    page: DependenciesPage,
    should_quit: bool,
    tx: mpsc::UnboundedSender<Action>,
    rx: mpsc::UnboundedReceiver<Action>,
}

impl App {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::unbounded_channel::<Action>();
        Self {
            page: DependenciesPage::new(tx.clone()),
            should_quit: false,
            tx,
            rx,
        }
    }

    pub async fn run(&mut self) -> Result<()> {
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
                    let _ = tx_env.send(Action::Error(format!("uv pip list: {e}")));
                }
            }
        });

        loop {
            terminal.draw(|frame| {
                self.page.render(frame, frame.area());
            })?;

            // Poll crossterm events
            if event::poll(Duration::from_millis(50))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        let action = match key.code {
                            _ => Some(Action::ButtonPressed(key)),
                        };
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
            Action::Error(msg) => {
                eprintln!("Error: {msg}");
            }
            _ => {
                if let Some(next) = self.page.handle_event(&action) {
                    self.dispatch(next);
                }
            }
        }
    }
}

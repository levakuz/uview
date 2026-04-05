mod app;
mod controllers;
mod events;
mod ui;

use color_eyre::Result;

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    let mut app = app::App::new();
    app.run().await
}

mod app;
mod controllers;
mod events;
mod ui;

use color_eyre::Result;

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    let cwd = std::env::current_dir()?;
    let Some(root) = controllers::project::find_project_root(&cwd) else {
        eprintln!(
            "uview: no pyproject.toml in {} or any parent directory.\n\
             Run uview inside a Python project, or create one with `uv init`.",
            cwd.display()
        );
        std::process::exit(1);
    };
    // pyproject.toml, the `.venv` handed to `uv pip`, and every `uv` subprocess resolve
    // against the working directory, so make that the project root.
    // ponytail: process-wide; the root moves into UvClient (#41) if one run ever spans projects.
    std::env::set_current_dir(&root)?;
    let mut app = app::App::new(format!("Project: {}", root.display()));
    app.run().await
}

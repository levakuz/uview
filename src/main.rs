mod app;
mod controllers;
mod events;
mod ui;

use std::path::PathBuf;

use color_eyre::Result;

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    let root = match project_root() {
        Ok(root) => root,
        Err(message) => {
            eprintln!("uview: {message}");
            std::process::exit(1);
        }
    };
    // pyproject.toml, the `.venv` handed to `uv pip`, and every `uv` subprocess resolve
    // against the working directory, so make that the project root.
    // ponytail: process-wide; the root moves into UvClient (#41) if one run ever spans projects.
    std::env::set_current_dir(&root)?;
    let mut app = app::App::new(root.display().to_string());
    app.run().await
}

/// The project uv would act on from here, or why there is none. Paths print with `{:?}`, so
/// a directory name cannot send escape sequences to the terminal.
fn project_root() -> Result<PathBuf, String> {
    let cwd =
        std::env::current_dir().map_err(|e| format!("cannot read the current directory: {e}"))?;
    // uv starts from UV_PROJECT when it is set, and so does uview; run_uv then drops it.
    let start = match std::env::var_os("UV_PROJECT") {
        Some(project) if !project.is_empty() => cwd.join(project),
        _ => cwd,
    };
    let root = controllers::project::find_project_root(&start).ok_or_else(|| {
        format!(
            "no pyproject.toml in {start:?} or any parent directory.\n\
             Run uview inside a Python project, or create one with `uv init`."
        )
    })?;
    match controllers::project::refusal(&root) {
        Some(reason) => Err(reason),
        None => Ok(root),
    }
}

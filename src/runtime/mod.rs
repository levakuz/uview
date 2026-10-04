//! Performs the commands components return and reports what came of them as events. The only
//! module that spawns tasks (ADR-0001).

pub mod jobs;

use tokio::sync::mpsc::UnboundedSender;

use crate::controllers::dependencies::{
    add_dep, load_env_deps, load_pyproject_deps, pin_dep_version, remove_dep_from_pyproject,
    sync_exact, uninstall_from_env, upgrade_env_dep,
};
use crate::message::{Command, Event};
use crate::uv::{UvClient, UvError};

/// Runs `command` against `uv` and reports how it went. The tables are reloaded after a
/// mutation by the job runner, not here.
async fn run(command: Command, uv: &impl UvClient, events: &UnboundedSender<Event>) {
    let outcome = match command {
        Command::Reload => return reload(uv, events).await,
        Command::Sync => sync_exact(uv)
            .await
            .map(|()| "Synced the environment".to_string()),
        Command::Add { requirement } => add_dep(uv, &requirement)
            .await
            .map(|()| format!("Added {requirement}")),
        Command::RemoveFromProject { name, groups } => remove_dep_from_pyproject(uv, &name, groups)
            .await
            .map(|()| format!("Removed {name} from pyproject.toml")),
        Command::Uninstall { name } => uninstall_from_env(uv, &name)
            .await
            .map(|()| format!("Uninstalled {name} from the environment")),
        Command::Upgrade { name } => upgrade_env_dep(uv, &name)
            .await
            .map(|()| format!("Updated {name}")),
        Command::Pin { name, constraint } => pin_dep_version(uv, &name, &constraint)
            .await
            .map(|pinned| format!("Pinned {pinned}")),
    };
    let event = match outcome {
        Ok(notice) => Event::Notice(notice),
        // The job runner says so.
        Err(UvError::Cancelled) => return,
        Err(e) => Event::Error(e.to_string()),
    };
    // Sent before the reload, so an error from the reload replaces it.
    let _ = events.send(event);
}

/// Reads both tables again, as at startup.
async fn reload(uv: &impl UvClient, events: &UnboundedSender<Event>) {
    let _ = events.send(match load_pyproject_deps().await {
        Ok(deps) => Event::PyprojectLoaded(deps),
        Err(e) => Event::Error(format!("pyproject.toml: {e}")),
    });
    let event = match load_env_deps(uv).await {
        Ok(deps) => Event::EnvLoaded(deps),
        Err(UvError::Cancelled) => return,
        Err(e) => Event::Error(e.to_string()),
    };
    let _ = events.send(event);
}

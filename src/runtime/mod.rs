//! Performs the commands components return and reports what came of them as events. The only
//! module that spawns tasks (ADR-0001).

pub mod jobs;

use tokio::sync::mpsc::UnboundedSender;

use crate::controllers::dependencies::{
    add_dep, load_env_deps, load_pyproject_deps, pin_dep_version, remove_dep_from_pyproject,
    sync_exact, uninstall_from_env, upgrade_env_dep,
};
use crate::controllers::project::load_project_info;
use crate::message::{Command, Event};
use crate::uv::{UvClient, UvError};

/// Runs `command` against `uv` and reports how it went. `generation` is the project's as the job
/// starts, which a reload reports its rows with. A mutation does not reload the tables: the app
/// does, when the job runner reports that the project changed.
async fn run(
    command: Command,
    uv: &impl UvClient,
    events: &UnboundedSender<Event>,
    generation: u64,
) {
    let outcome = match command {
        Command::Reload => return reload(uv, events, generation).await,
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
    let _ = events.send(event);
}

/// Reads both tables, and the project's name and version, again, as at startup. A table that
/// cannot load shows why, and the status line says so too.
async fn reload(uv: &impl UvClient, events: &UnboundedSender<Event>, generation: u64) {
    let pyproject = load_pyproject_deps().await.map_err(|e| e.to_string());
    if let Err(reason) = &pyproject {
        let _ = events.send(Event::Error(format!("pyproject.toml: {reason}")));
    }
    let _ = events.send(Event::PyprojectLoaded {
        generation,
        loaded: pyproject,
    });
    let env = load_env_deps(uv).await;
    // A cancel is the runner's to report.
    if let Err(e) = &env
        && !matches!(e, UvError::Cancelled)
    {
        let _ = events.send(Event::Error(e.to_string()));
    }
    let _ = events.send(Event::EnvLoaded {
        generation,
        loaded: env.map_err(|e| e.to_string()),
    });
    // Only the project tile says so: uv cannot read a dynamic version, and that is no mistake
    // to report after every reload.
    let info = load_project_info(uv).await.map_err(|e| match e {
        UvError::Failed { stderr, .. } => stderr,
        e => e.to_string(),
    });
    let _ = events.send(Event::ProjectLoaded(info));
}

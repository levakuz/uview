//! Performs the commands components return and reports what came of them as events. The only
//! module that spawns tasks (ADR-0001).

use tokio::sync::mpsc::UnboundedSender;

use crate::controllers::dependencies::{
    get_env_package_info, load_env_deps, load_pyproject_deps, pin_dep_version,
    remove_dep_from_pyproject, uninstall_from_env, upgrade_env_dep,
};
use crate::message::{Command, Event};
use crate::uv::{RealUvClient, UvClient};

/// Runs `command` in the background against the real `uv`.
pub fn execute(command: Command, events: &UnboundedSender<Event>) {
    let events = events.clone();
    tokio::spawn(async move { run(command, &RealUvClient, &events).await });
}

/// Runs `command` against `uv` and sends each outcome to `events`.
pub async fn run(command: Command, uv: &impl UvClient, events: &UnboundedSender<Event>) {
    let send = |event| {
        let _ = events.send(event);
    };
    match command {
        Command::Reload => reload(uv, events).await,
        Command::RemoveFromProject { name, groups } => {
            send(match remove_dep_from_pyproject(uv, &name, groups).await {
                Ok(()) => Event::Notice(format!("Removed {name} from pyproject.toml")),
                Err(e) => Event::Error(e.to_string()),
            });
            // Also after a failure: `uv remove` may already have edited pyproject.toml, and a
            // stale row would replay that membership.
            reload(uv, events).await;
        }
        Command::Uninstall { name } => {
            send(match uninstall_from_env(uv, &name).await {
                Ok(()) => Event::Notice(format!("Uninstalled {name} from the environment")),
                Err(e) => Event::Error(e.to_string()),
            });
            // Drop the row, so it cannot be "uninstalled" again as a no-op.
            reload(uv, events).await;
        }
        Command::Upgrade { name } => match upgrade_env_dep(uv, &name).await {
            Ok(()) => {
                send(Event::Notice(format!("Updated {name}")));
                // Silent when the package is not installed — e.g. an extra that a plain sync
                // leaves out — since the upgrade itself succeeded.
                match get_env_package_info(uv, &name).await {
                    Ok(Some(dep)) => send(Event::EnvUpdated(dep)),
                    Ok(None) => {}
                    Err(e) => send(Event::Error(e.to_string())),
                }
            }
            Err(e) => send(Event::Error(e.to_string())),
        },
        Command::Pin { name, constraint } => {
            // Sent first, so an error from the reload below replaces it.
            send(match pin_dep_version(uv, &name, &constraint).await {
                Ok(pinned) => Event::Notice(format!("Pinned {pinned}")),
                Err(e) => Event::Error(e.to_string()),
            });
            // Reload rather than patch rows, so the tables show what uv wrote, including after a
            // failure part-way through the memberships.
            reload(uv, events).await;
        }
    }
}

/// Reads both tables again, as at startup.
async fn reload(uv: &impl UvClient, events: &UnboundedSender<Event>) {
    let _ = events.send(match load_pyproject_deps().await {
        Ok(deps) => Event::PyprojectLoaded(deps),
        Err(e) => Event::Error(format!("pyproject.toml: {e}")),
    });
    let _ = events.send(match load_env_deps(uv).await {
        Ok(deps) => Event::EnvLoaded(deps),
        Err(e) => Event::Error(e.to_string()),
    });
}

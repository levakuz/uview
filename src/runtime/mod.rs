//! Performs the commands components return and reports what came of them as events. The only
//! module that spawns tasks (ADR-0001).

pub mod jobs;

use std::path::Path;

use tokio::sync::mpsc::UnboundedSender;

use crate::controllers::dependencies::{
    add_dep, add_from, load_env_deps, load_pyproject_deps, pin_dep_version,
    remove_dep_from_pyproject, sync_exact, uninstall_from_env, upgrade_env_dep,
};
use crate::controllers::groups::{delete_group, empty_groups};
use crate::controllers::indexes::{add_index, load_indexes, remove_index};
use crate::controllers::project::{load_project_info, preview_bumps, set_version};
use crate::controllers::pyproject::PYPROJECT;
use crate::domain::Membership;
use crate::message::{Command, Event};
use crate::uv::{UvClient, UvError};

/// Runs `command` against `uv` and reports how it went. `generation` is the project's as the job
/// starts, which a reload reports its rows with. A mutation does not reload the tables: the app
/// does, when the job runner reports that the project changed. `env` is the project's
/// environment, which `uv pip` must be told: left implicit, it follows an activated virtualenv.
async fn run(
    command: Command,
    uv: &impl UvClient,
    env: &Path,
    events: &UnboundedSender<Event>,
    generation: u64,
) {
    let outcome = match command {
        Command::Reload => return reload(uv, env, events, generation).await,
        // What uv says is the list's to show, not the status line's.
        Command::PreviewVersions => {
            let previewed = preview_bumps(uv).await;
            if !matches!(previewed, Err(UvError::Cancelled)) {
                let _ = events.send(Event::VersionsPreviewed(previewed.map_err(|e| match e {
                    UvError::Failed { stderr, .. } => stderr,
                    e => e.to_string(),
                })));
            }
            return;
        }
        Command::SetVersion(change) => set_version(uv, &change)
            .await
            .map(|version| format!("The version is now {version}")),
        Command::Sync => sync_exact(uv)
            .await
            .map(|()| "Synced the environment".to_string()),
        Command::Add {
            requirement,
            membership,
        } => add_dep(uv, &requirement, &membership)
            .await
            .map(|()| format!("Added {requirement}{}", to(&membership))),
        Command::RemoveFromProject { name, groups } => {
            let from: Vec<String> = groups.iter().map(ToString::to_string).collect();
            remove_dep_from_pyproject(uv, &name, groups)
                .await
                .map(|()| format!("Removed {name} from {}", from.join(", ")))
        }
        Command::AddFrom { source, membership } => add_from(uv, &source, &membership)
            .await
            .map(|()| format!("Added {}{}", source.label(), to(&membership))),
        Command::AddIndex { name, url, default } => {
            add_index(uv, Path::new(PYPROJECT), &name, &url, default)
                .await
                .map(|()| format!("Added index {name}"))
        }
        Command::RemoveIndex(index) => remove_index(uv, Path::new(PYPROJECT), &index)
            .await
            .map(|()| format!("Removed index {}", index_label(&index))),
        Command::DeleteGroup { name } => delete_group(uv, Path::new(PYPROJECT), &name)
            .await
            .map(|()| format!("Deleted group {name}")),
        Command::Move {
            name,
            requirement,
            from,
            to,
        } => {
            // Added first: a failed `uv add` changes nothing, while a failed remove after it
            // leaves the package in both groups, still declared.
            if let Err(e) = add_dep(uv, &requirement, &to).await {
                Err(e)
            } else {
                match remove_dep_from_pyproject(uv, &name, vec![from.clone()]).await {
                    Ok(()) => Ok(format!("Moved {name} from {from} to {to}")),
                    Err(UvError::Cancelled) => Err(UvError::Cancelled),
                    Err(e) => {
                        let _ = events.send(Event::Error(format!(
                            "{name} was added to {to} but is still in {from}: {e}"
                        )));
                        return;
                    }
                }
            }
        }
        Command::Uninstall { name } => uninstall_from_env(uv, &name, env)
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
async fn reload(uv: &impl UvClient, env: &Path, events: &UnboundedSender<Event>, generation: u64) {
    let pyproject = load_pyproject_deps().await.map_err(|e| e.to_string());
    if let Err(reason) = &pyproject {
        let _ = events.send(Event::Error(format!("pyproject.toml: {reason}")));
    }
    let (empty_groups, deletable_groups) = empty_groups().await;
    let _ = events.send(Event::PyprojectLoaded {
        generation,
        loaded: pyproject,
        empty_groups,
        deletable_groups,
        indexes: load_indexes().await,
    });
    let env = load_env_deps(uv, env).await;
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
    let _ = events.send(Event::ProjectLoaded {
        generation,
        loaded: info,
    });
}

/// Where an add goes, as its notice and label say it: nothing for the main dependencies.
fn to(membership: &Membership) -> String {
    match membership {
        Membership::Project => String::new(),
        other => format!(" to {other}"),
    }
}

/// An index as notices name it: by its name, or its URL when it has none.
fn index_label(index: &crate::domain::Index) -> &str {
    index.name.as_deref().unwrap_or(&index.url)
}

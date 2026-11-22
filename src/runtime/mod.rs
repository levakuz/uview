//! Performs the commands components return and reports what came of them as events. The only
//! module that spawns tasks (ADR-0001).

pub mod jobs;

use tokio::sync::mpsc::UnboundedSender;
use tokio_util::sync::CancellationToken;

use crate::config::{shown, write_new};
use crate::controllers::build::{build, built_notice, dist_dir};
use crate::controllers::dependencies::{
    add_dep, add_from, load_env_deps, load_pyproject_deps, pin_dep_version,
    remove_dep_from_pyproject, sync_exact, uninstall_from_env, upgrade_env_dep,
};
use crate::controllers::environment::{
    delete_env, list_pythons, load_env_info, recreate, relative_to, reset,
};
use crate::controllers::groups::{create_group, delete_group, empty_groups, load_groups};
use crate::controllers::indexes::{add_index, load_indexes, remove_index};
use crate::controllers::project::{
    ProjectPaths, check_lock, load_project_info, load_tree, lock, preview_bumps, set_version,
    uv_too_old, why_installed,
};
use crate::controllers::publish::{publish, published_notice};
use crate::controllers::versions::list_versions;
use crate::domain::Membership;
use crate::index::IndexClient;
use crate::message::{Command, Event};
use crate::uv::{UvClient, UvError};

/// What a job runs with: `uv`, the index client, and the job's cancel.
struct Clients<'a, U, I> {
    uv: &'a U,
    index: &'a I,
    cancel: &'a CancellationToken,
}

/// Runs `command` against `uv` and reports how it went. `generation` is the project's as the job
/// starts, which a reload reports its rows with. A mutation does not reload the tables: the app
/// does, when the job runner reports that the project changed. `paths` says where the
/// project's files are.
async fn run<U: UvClient, I: IndexClient>(
    command: Command,
    clients: &Clients<'_, U, I>,
    paths: &ProjectPaths,
    events: &UnboundedSender<Event>,
    generation: u64,
) {
    let uv = clients.uv;
    let outcome = match command {
        // What the index says is the list's to show, not the status line's. A cancel ends the
        // request; uv is not involved to end it.
        Command::ListVersions { name } => {
            let listed = tokio::select! {
                biased;
                () = clients.cancel.cancelled() => Err("cancelled".to_string()),
                listed = list_versions(clients.index, &paths.pyproject, &name) => listed,
            };
            let _ = events.send(Event::VersionsListed {
                name,
                loaded: listed,
            });
            return;
        }
        Command::Reload => return reload(uv, paths, events, generation).await,
        Command::ListPythons => {
            let listed = list_pythons(uv).await;
            if !matches!(listed, Err(UvError::Cancelled)) {
                let _ = events.send(Event::PythonsListed(listed.map_err(|e| said(&e))));
            }
            return;
        }
        Command::RecreateEnv { python } => recreate(uv, &python, &paths.root).await.map(|pinned| {
            let made = format!("Made the environment anew with Python {}", python.request());
            if pinned {
                format!("{made}, and pinned it in .python-version")
            } else {
                made
            }
        }),
        Command::ResetEnv => reset(uv, &paths.env)
            .await
            .map(|()| "Emptied the environment and installed the project again".into()),
        Command::DeleteEnv => match delete_env(&paths.env, &paths.root).await {
            Ok(()) => Ok("Deleted the environment; s makes it again".into()),
            Err(reason) => {
                let _ = events.send(Event::Error(reason));
                return;
            }
        },
        Command::LoadTree { outdated } => {
            return send_tree(uv, events, generation, outdated).await;
        }
        Command::WhyInstalled { name } => {
            let loaded = why_installed(uv, &name).await;
            if !matches!(loaded, Err(UvError::Cancelled)) {
                let _ = events.send(Event::WhyLoaded {
                    name,
                    loaded: loaded.map_err(|e| said(&e)),
                });
            }
            return;
        }
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
        // uv's resolution output, which says why, is in the log; its first line seldom does.
        Command::Lock { upgrade } => match lock(uv, upgrade).await {
            Ok(()) if upgrade => {
                Ok("Moved every package in uv.lock to its newest allowed version".into())
            }
            Ok(()) => Ok("Locked".into()),
            Err(UvError::Failed { .. }) => {
                let _ = events.send(Event::Error(
                    "uv could not lock the project; the uv log says why".into(),
                ));
                return;
            }
            Err(e) => Err(e),
        },
        Command::CheckUv => {
            if let Some(too_old) = uv_too_old(uv).await {
                let _ = events.send(Event::Error(too_old));
            }
            return;
        }
        // A file that is there is the user's: it is never written over.
        Command::WriteConfig { path, contents } => {
            let file = shown(&path, std::env::var("HOME").ok().as_deref());
            match write_new(&path, &contents).await {
                Ok(()) => Ok(format!(
                    "Wrote {file}: uncomment a line, then restart uview"
                )),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Ok(format!(
                    "{file} is there already: edit it, then restart uview"
                )),
                Err(e) => {
                    let _ = events.send(Event::Error(format!("could not write {file}: {e}")));
                    return;
                }
            }
        }
        // The backend's output, which says why, is in the log.
        Command::Build => match build(uv, paths).await {
            Ok(built) => {
                let notice =
                    built_notice(&built, &relative_to(&dist_dir(&paths.root), &paths.root));
                let _ = events.send(Event::Built(built));
                Ok(notice)
            }
            Err(UvError::Failed { .. }) => {
                let _ = events.send(Event::Error(
                    "uv could not build the project; the uv log says why".into(),
                ));
                return;
            }
            Err(e) => Err(e),
        },
        // Why, a missing credential or a file the index already has, is in the log.
        Command::Publish { target, files } => match publish(uv, &target, &files).await {
            Ok(()) => Ok(published_notice(&files, &target)),
            Err(UvError::Failed { .. }) => {
                let _ = events.send(Event::Error(format!(
                    "uv could not publish to {}; the uv log says why",
                    target.label()
                )));
                return;
            }
            Err(e) => Err(e),
        },
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
            add_index(uv, &paths.pyproject, &name, &url, default)
                .await
                .map(|()| format!("Added index {name}"))
        }
        Command::RemoveIndex(index) => remove_index(uv, &paths.pyproject, &index)
            .await
            .map(|()| format!("Removed index {}", index_label(&index))),
        Command::CreateGroup { name } => create_group(uv, &paths.pyproject, &name)
            .await
            .map(|()| format!("Created group {name}")),
        Command::DeleteGroup { name } => delete_group(uv, &paths.pyproject, &name)
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
        Command::Uninstall { name } => uninstall_from_env(uv, &name, &paths.env)
            .await
            .map(|()| format!("Uninstalled {name} from the environment")),
        Command::Upgrade { name } => upgrade_env_dep(uv, &name)
            .await
            .map(|()| format!("Updated {name}")),
        Command::Pin { name, constraint } => {
            pin_dep_version(uv, &paths.pyproject, &name, &constraint)
                .await
                .map(|pinned| format!("Pinned {pinned}"))
        }
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
async fn reload(
    uv: &impl UvClient,
    paths: &ProjectPaths,
    events: &UnboundedSender<Event>,
    generation: u64,
) {
    let pyproject = load_pyproject_deps(&paths.pyproject)
        .await
        .map_err(|e| e.to_string());
    if let Err(reason) = &pyproject {
        let _ = events.send(Event::Error(format!("pyproject.toml: {reason}")));
    }
    let (empty_groups, deletable_groups) = empty_groups(&paths.pyproject).await;
    let _ = events.send(Event::PyprojectLoaded {
        generation,
        loaded: pyproject,
        empty_groups,
        deletable_groups,
        indexes: load_indexes(&paths.pyproject).await,
        groups: load_groups(&paths.pyproject).await,
    });
    let env = load_env_deps(uv, &paths.env).await;
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
    if let Ok(state) = check_lock(uv).await {
        let _ = events.send(Event::LockChecked { generation, state });
    }
    send_tree(uv, events, generation, false).await;
    // Last: measuring a large environment walks thousands of files, which nothing else waits on.
    let _ = events.send(Event::EnvInfoLoaded {
        generation,
        info: load_env_info(paths).await,
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

/// Reads the dependency tree and reports it with `generation`. What uv said, when it could not,
/// is the tab's to show; a cancel is the runner's.
async fn send_tree(
    uv: &impl UvClient,
    events: &UnboundedSender<Event>,
    generation: u64,
    outdated: bool,
) {
    let loaded = load_tree(uv, outdated).await;
    if !matches!(loaded, Err(UvError::Cancelled)) {
        let _ = events.send(Event::TreeLoaded {
            generation,
            outdated,
            loaded: loaded.map_err(|e| said(&e)),
        });
    }
}

/// What uv said when it refused, without the command around it; otherwise the error.
fn said(e: &UvError) -> String {
    match e {
        UvError::Failed { stderr, .. } => stderr.clone(),
        e => e.to_string(),
    }
}

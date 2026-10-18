//! Runs commands as jobs (ADR-0003). `uv` locks the project, so of two mutations at once one
//! fails: they run one at a time, in the order accepted. Reads take no lock and start at once.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use tokio::sync::mpsc::{self, UnboundedSender};
use tokio_util::sync::CancellationToken;

use crate::controllers::project::ProjectPaths;
use crate::index::IndexClient;
use crate::message::{Command, Event, JobId};
use crate::uv::{UvClient, UvError};

/// Accepts commands as jobs and runs them.
pub struct JobRunner<U, I> {
    uv: Arc<U>,
    /// Reads package indexes, for what uv cannot list.
    index: Arc<I>,
    /// Where the project's files are.
    paths: Arc<ProjectPaths>,
    events: UnboundedSender<Event>,
    /// Mutations waiting their turn. A single task takes them in order.
    mutations: UnboundedSender<Job>,
    last_id: u64,
    /// Held by every job accepted since the last `cancel`.
    cancel: CancellationToken,
    /// How many mutations have ended. A reload reports its rows with the count as it started,
    /// so that the app can tell rows read before a mutation from rows read after it.
    generation: Arc<AtomicU64>,
}

struct Job {
    id: JobId,
    label: String,
    command: Command,
    cancel: CancellationToken,
}

impl<U: UvClient + 'static, I: IndexClient + Send + Sync + 'static> JobRunner<U, I> {
    /// Starts the task that runs mutations, so it must be called inside a tokio runtime.
    pub fn new(
        uv: Arc<U>,
        index: Arc<I>,
        paths: ProjectPaths,
        events: UnboundedSender<Event>,
    ) -> Self {
        let paths = Arc::new(paths);
        let (mutations, mut queue) = mpsc::unbounded_channel::<Job>();
        let generation = Arc::new(AtomicU64::new(0));
        tokio::spawn({
            let (uv, index, paths, events, generation) = (
                Arc::clone(&uv),
                Arc::clone(&index),
                Arc::clone(&paths),
                events.clone(),
                Arc::clone(&generation),
            );
            async move {
                while let Some(job) = queue.recv().await {
                    run_job(job, &*uv, &*index, &paths, &events, &generation).await;
                }
            }
        });
        Self {
            uv,
            index,
            paths,
            events,
            mutations,
            last_id: 0,
            cancel: CancellationToken::new(),
            generation,
        }
    }

    /// Accepts `command` as a job: a read starts at once, a mutation after those before it.
    pub fn execute(&mut self, command: Command) {
        self.last_id += 1;
        let job = Job {
            id: JobId(self.last_id),
            label: label(&command),
            command,
            cancel: self.cancel.clone(),
        };
        let _ = self.events.send(Event::JobQueued {
            id: job.id,
            label: job.label.clone(),
        });
        if mutates(&job.command) {
            let _ = self.mutations.send(job);
        } else {
            let (uv, index, paths, events) = (
                Arc::clone(&self.uv),
                Arc::clone(&self.index),
                Arc::clone(&self.paths),
                self.events.clone(),
            );
            let generation = Arc::clone(&self.generation);
            tokio::spawn(async move {
                run_job(job, &*uv, &*index, &paths, &events, &generation).await;
            });
        }
    }

    /// Cancels every job accepted so far. A running one stops its `uv` and finishes once `uv`
    /// has exited; one still waiting never starts.
    pub fn cancel(&mut self) {
        self.cancel.cancel();
        self.cancel = CancellationToken::new();
    }
}

/// Whether `command` changes the project or its environment, both of which `uv` locks.
fn mutates(command: &Command) -> bool {
    !matches!(
        command,
        Command::Reload | Command::PreviewVersions | Command::ListVersions { .. }
    )
}

/// What the progress indicator says while `command` runs.
fn label(command: &Command) -> String {
    match command {
        Command::Reload => "Loading dependencies".into(),
        Command::PreviewVersions => "Reading the next versions".into(),
        Command::ListVersions { name } => format!("Listing the versions of {name}"),
        Command::SetVersion(_) => "Setting the version".into(),
        Command::Lock { upgrade: false } => "Locking".into(),
        Command::Lock { upgrade: true } => "Upgrading every package in uv.lock".into(),
        Command::Sync => "Syncing the environment".into(),
        Command::Add {
            requirement,
            membership,
        } => format!("Adding {requirement}{}", super::to(membership)),
        Command::RemoveFromProject { name, .. } => format!("Removing {name}"),
        Command::Move { name, to, .. } => format!("Moving {name} to {to}"),
        Command::DeleteGroup { name } => format!("Deleting group {name}"),
        Command::CreateGroup { name } => format!("Creating group {name}"),
        Command::AddIndex { name, .. } => format!("Adding index {name}"),
        Command::RemoveIndex(index) => format!("Removing index {}", super::index_label(index)),
        Command::AddFrom { source, membership } => {
            format!("Adding {}{}", source.label(), super::to(membership))
        }
        Command::Uninstall { name } => format!("Uninstalling {name}"),
        Command::Upgrade { name } => format!("Updating {name}"),
        Command::Pin { name, .. } => format!("Pinning {name}"),
    }
}

/// Runs `job` and reports it from start to finish.
async fn run_job(
    job: Job,
    uv: &impl UvClient,
    index: &impl IndexClient,
    paths: &ProjectPaths,
    events: &UnboundedSender<Event>,
    generation: &AtomicU64,
) {
    let send = |event| {
        let _ = events.send(event);
    };
    // Cancelled while it waited: it never ran.
    if job.cancel.is_cancelled() {
        send(Event::JobFinished(job.id));
        return;
    }
    send(Event::JobStarted(job.id));
    let mutates = mutates(&job.command);
    let job_uv = JobUv {
        uv,
        id: job.id,
        cancel: &job.cancel,
        events,
        logged: mutates,
    };
    let as_it_starts = generation.load(Ordering::SeqCst);
    let clients = super::Clients {
        uv: &job_uv,
        index,
        cancel: &job.cancel,
    };
    super::run(job.command, &clients, paths, events, as_it_starts).await;
    if job.cancel.is_cancelled() {
        send(Event::Notice(format!("Cancelled: {}", job.label)));
    }
    // However the mutation ended: `uv` may have written pyproject.toml before it failed, and a
    // stale row would replay a removed membership or uninstall a package twice. A cancelled `uv`
    // has exited by now, after putting back what it could. Rows read before now are older.
    if mutates {
        generation.fetch_add(1, Ordering::SeqCst);
        send(Event::ProjectMutated);
    }
    send(Event::JobFinished(job.id));
}

/// The `uv` of one job: what each call of a mutation writes to stderr becomes the job's output,
/// and each call ends with `UvError::Cancelled` once the job is cancelled.
struct JobUv<'a, U> {
    uv: &'a U,
    id: JobId,
    cancel: &'a CancellationToken,
    events: &'a UnboundedSender<Event>,
    /// Whether what `uv` writes goes to the log. A read writes only errors, which its table or
    /// the project tile already shows, and a reload would repeat them after every mutation.
    logged: bool,
}

impl<U: UvClient> UvClient for JobUv<'_, U> {
    async fn run(&self, args: &[&str]) -> Result<String, UvError> {
        let on_line = |line: &str| {
            if self.logged {
                let _ = self.events.send(Event::JobOutput(self.id, line.to_owned()));
            }
        };
        self.uv.run_streaming(args, self.cancel, &on_line).await
    }
}

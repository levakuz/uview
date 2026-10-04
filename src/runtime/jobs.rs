//! Runs commands as jobs (ADR-0003). `uv` locks the project, so of two mutations at once one
//! fails: they run one at a time, in the order accepted. Reads take no lock and start at once.

use std::sync::Arc;

use tokio::sync::mpsc::{self, UnboundedSender};
use tokio_util::sync::CancellationToken;

use crate::message::{Command, Event, JobId};
use crate::uv::{UvClient, UvError};

/// Accepts commands as jobs and runs them.
pub struct JobRunner<U> {
    uv: Arc<U>,
    events: UnboundedSender<Event>,
    /// Mutations waiting their turn. A single task takes them in order.
    mutations: UnboundedSender<Job>,
    last_id: u64,
    /// Held by every job accepted since the last `cancel`.
    cancel: CancellationToken,
}

struct Job {
    id: JobId,
    label: String,
    command: Command,
    cancel: CancellationToken,
}

impl<U: UvClient + 'static> JobRunner<U> {
    /// Starts the task that runs mutations, so it must be called inside a tokio runtime.
    pub fn new(uv: Arc<U>, events: UnboundedSender<Event>) -> Self {
        let (mutations, mut queue) = mpsc::unbounded_channel::<Job>();
        tokio::spawn({
            let (uv, events) = (Arc::clone(&uv), events.clone());
            async move {
                while let Some(job) = queue.recv().await {
                    run_job(job, &*uv, &events).await;
                }
            }
        });
        Self {
            uv,
            events,
            mutations,
            last_id: 0,
            cancel: CancellationToken::new(),
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
            let (uv, events) = (Arc::clone(&self.uv), self.events.clone());
            tokio::spawn(async move { run_job(job, &*uv, &events).await });
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
    !matches!(command, Command::Reload)
}

/// What the progress indicator says while `command` runs.
fn label(command: &Command) -> String {
    match command {
        Command::Reload => "Loading dependencies".into(),
        Command::Sync => "Syncing the environment".into(),
        Command::Add { requirement } => format!("Adding {requirement}"),
        Command::RemoveFromProject { name, .. } => format!("Removing {name}"),
        Command::Uninstall { name } => format!("Uninstalling {name}"),
        Command::Upgrade { name } => format!("Updating {name}"),
        Command::Pin { name, .. } => format!("Pinning {name}"),
    }
}

/// Runs `job` and reports it from start to finish.
async fn run_job(job: Job, uv: &impl UvClient, events: &UnboundedSender<Event>) {
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
    };
    super::run(job.command, &job_uv, events).await;
    if job.cancel.is_cancelled() {
        send(Event::Notice(format!("Cancelled: {}", job.label)));
    }
    // However the mutation ended: `uv` may have written pyproject.toml before it failed, and a
    // stale row would replay a removed membership or uninstall a package twice. A cancelled `uv`
    // has exited by now, after putting back what it could. Still in the mutation's turn, so the
    // next one cannot change things under the reload.
    if mutates {
        super::reload(uv, events).await;
    }
    send(Event::JobFinished(job.id));
}

/// The `uv` of one job: what each call writes to stderr becomes the job's output, and each call
/// ends with `UvError::Cancelled` once the job is cancelled.
struct JobUv<'a, U> {
    uv: &'a U,
    id: JobId,
    cancel: &'a CancellationToken,
    events: &'a UnboundedSender<Event>,
}

impl<U: UvClient> UvClient for JobUv<'_, U> {
    async fn run(&self, args: &[&str]) -> Result<String, UvError> {
        let on_line = |line: &str| {
            let _ = self.events.send(Event::JobOutput(self.id, line.to_owned()));
        };
        self.uv.run_streaming(args, self.cancel, &on_line).await
    }
}

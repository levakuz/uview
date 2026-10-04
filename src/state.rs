//! What the app knows, held once for every view to read (ADR-0004). The app writes it from the
//! runtime's events; components only read it, and keep nothing but view state of their own.

use std::collections::VecDeque;
use std::time::Instant;

use crate::domain::{EnvDependency, PyprojectDependency};
use crate::message::{Event, JobId};

/// Everything the app knows, by domain. It carries no behaviour; a domain that needs some
/// carries it itself.
#[derive(Default)]
pub struct AppState {
    pub project: ProjectState,
    pub deps: DepsState,
    pub jobs: JobsState,
    /// The latest notice or error, until Esc dismisses it or another replaces it.
    pub message: Option<Message>,
}

/// The project uview runs in.
#[derive(Default)]
pub struct ProjectState {
    pub root: String,
}

/// The project's dependencies: as pyproject.toml declares them, and as the environment has them.
#[derive(Default)]
pub struct DepsState {
    pub pyproject: Loadable<Vec<PyprojectDependency>>,
    pub env: Loadable<Vec<EnvDependency>>,
    /// The generation of the newest load shown: how many mutations had ended as it started.
    generation: u64,
}

impl DepsState {
    /// Takes in a table's load, unless what is shown comes from a newer one. Reloads run beside
    /// mutations, so one that started before a mutation ended can land after the reload that
    /// followed it, with rows the mutation has since changed.
    pub fn apply(&mut self, event: Event) {
        match event {
            Event::PyprojectLoaded { generation, loaded } if generation >= self.generation => {
                self.generation = generation;
                self.pyproject = loaded.into();
            }
            Event::EnvLoaded { generation, loaded } if generation >= self.generation => {
                self.generation = generation;
                self.env = loaded.into();
            }
            _ => {}
        }
    }
}

/// Data loaded in the background, and how far its load has got. As a bare list, one not yet
/// loaded, one that failed and one that is empty would all look alike.
#[derive(Default)]
pub enum Loadable<T> {
    /// Nothing has asked for it yet.
    #[default]
    NotLoaded,
    Loading {
        since: Instant,
    },
    Loaded(T),
    /// Why the load failed, for the view to show.
    Failed(String),
}

impl<T> From<Result<T, String>> for Loadable<T> {
    fn from(loaded: Result<T, String>) -> Self {
        match loaded {
            Ok(value) => Self::Loaded(value),
            Err(reason) => Self::Failed(reason),
        }
    }
}

impl<T> Loadable<Vec<T>> {
    /// The rows, or none until they have loaded.
    pub fn rows(&self) -> &[T] {
        match self {
            Self::Loaded(rows) => rows,
            _ => &[],
        }
    }
}

/// What the status line says.
pub enum Message {
    Info(String),
    Error(String),
}

/// How many lines the uv log keeps.
pub const LOG_LINES: usize = 500;

/// A line of the uv log.
pub enum LogLine {
    /// The label of the job whose lines follow.
    Job(String),
    /// A line its `uv` wrote.
    Output(String),
}

/// The jobs, as the runner's events report them: those in flight, which the progress indicator
/// shows, and the log of what their `uv` wrote.
#[derive(Default)]
pub struct JobsState {
    /// In the order accepted.
    jobs: Vec<Tracked>,
    /// Oldest first, at most `LOG_LINES`.
    log: VecDeque<LogLine>,
    /// The job named by the log's last header.
    logging: Option<JobId>,
}

struct Tracked {
    id: JobId,
    label: String,
    started: bool,
}

impl JobsState {
    pub fn apply(&mut self, event: &Event) {
        match event {
            Event::JobQueued { id, label } => self.jobs.push(Tracked {
                id: *id,
                label: label.clone(),
                started: false,
            }),
            Event::JobStarted(id) => {
                for job in self.jobs.iter_mut().filter(|job| job.id == *id) {
                    job.started = true;
                }
            }
            Event::JobOutput(id, line) => {
                // A job's label heads its first line, and its lines again after another job's.
                // A job that writes nothing, such as the load at startup, leaves no trace.
                if self.logging != Some(*id) {
                    if let Some(job) = self.jobs.iter().find(|job| job.id == *id) {
                        let label = job.label.clone();
                        self.push_log(LogLine::Job(label));
                    }
                    self.logging = Some(*id);
                }
                self.push_log(LogLine::Output(line.clone()));
            }
            Event::JobFinished(id) => self.jobs.retain(|job| job.id != *id),
            _ => {}
        }
    }

    fn push_log(&mut self, line: LogLine) {
        if self.log.len() == LOG_LINES {
            self.log.pop_front();
        }
        self.log.push_back(line);
    }

    pub fn log(&self) -> &VecDeque<LogLine> {
        &self.log
    }

    pub fn is_idle(&self) -> bool {
        self.jobs.is_empty()
    }

    /// The oldest job in flight, and how many are waiting behind it, e.g.
    /// `Removing six · 1 queued`. `None` once every job has finished.
    pub fn summary(&self) -> Option<String> {
        let (first, rest) = self.jobs.split_first()?;
        Some(match rest.iter().filter(|job| !job.started).count() {
            0 => first.label.clone(),
            queued => format!("{} · {queued} queued", first.label),
        })
    }
}

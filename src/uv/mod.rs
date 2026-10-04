//! The seam to the `uv` binary (ADR-0002): argv in, stdout or a `UvError` out.

pub mod args;
pub mod parse;

use std::fmt;
use std::process::Stdio;
use std::time::Duration;

use tokio_util::sync::CancellationToken;

/// Why a `uv` invocation failed.
#[derive(Debug)]
pub enum UvError {
    /// `uv` could not be started at all.
    Spawn(std::io::Error),
    /// `uv` ran and exited unsuccessfully.
    Failed {
        command: String,
        status: std::process::ExitStatus,
        stderr: String,
    },
    /// `uv` succeeded but its output could not be read.
    Parse(String),
    /// The job it ran for was cancelled, and `uv` has stopped.
    Cancelled,
}

impl fmt::Display for UvError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UvError::Spawn(e) if e.kind() == std::io::ErrorKind::NotFound => {
                write!(f, "`uv` was not found — is it installed and on PATH?")
            }
            UvError::Spawn(e) => write!(f, "could not run `uv`: {e}"),
            UvError::Failed {
                command,
                status,
                stderr,
            } => write!(f, "`uv {command}` failed ({status}): {stderr}"),
            UvError::Parse(msg) => write!(f, "could not read `uv` output: {msg}"),
            UvError::Cancelled => write!(f, "cancelled"),
        }
    }
}

impl std::error::Error for UvError {}

/// Every `uv` call goes through this trait: `RealUvClient` in the app, a fake in the tests.
/// Operations take it as `&impl UvClient`.
pub trait UvClient: Send + Sync {
    /// Runs `uv` with `args` and returns its stdout.
    fn run(&self, args: &[&str]) -> impl Future<Output = Result<String, UvError>> + Send;

    /// `run`, ended with `UvError::Cancelled` once `cancel` fires (ADR-0006). By default the
    /// call is simply dropped.
    fn run_cancellable(
        &self,
        args: &[&str],
        cancel: &CancellationToken,
    ) -> impl Future<Output = Result<String, UvError>> + Send {
        async move {
            tokio::select! {
                biased;
                () = cancel.cancelled() => Err(UvError::Cancelled),
                result = self.run(args) => result,
            }
        }
    }
}

/// Runs the `uv` binary found on `PATH`. The only code in uview that starts a process.
#[derive(Clone, Copy)]
pub struct RealUvClient;

impl UvClient for RealUvClient {
    async fn run(&self, args: &[&str]) -> Result<String, UvError> {
        self.run_cancellable(args, &CancellationToken::new()).await
    }

    /// Stops `uv` as Ctrl+C would once `cancel` fires: with SIGINT on Unix, on which `uv add`
    /// puts pyproject.toml back. Windows has no such signal for a child, so there it is killed.
    /// Returns once `uv` has exited, so what it left behind can be read.
    async fn run_cancellable(
        &self,
        args: &[&str],
        cancel: &CancellationToken,
    ) -> Result<String, UvError> {
        if cancel.is_cancelled() {
            return Err(UvError::Cancelled);
        }
        let child = tokio::process::Command::new("uv")
            .args(args)
            // main.rs already turned UV_PROJECT into the working directory; left set, uv would
            // resolve a relative value a second time, from there.
            .env_remove("UV_PROJECT")
            // Kept off the terminal the app draws on.
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            // If this future is dropped, `uv` must not outlive it.
            .kill_on_drop(true)
            .spawn()
            .map_err(UvError::Spawn)?;
        let pid = child.id();
        let output = child.wait_with_output();
        tokio::pin!(output);
        let output = tokio::select! {
            biased;
            () = cancel.cancelled() => {
                // `uv add` puts pyproject.toml back, then exits; other commands stop at once.
                // A `uv` still running after that is killed as `output` is dropped.
                if interrupt(pid) {
                    let _ = tokio::time::timeout(Duration::from_secs(2), output).await;
                }
                return Err(UvError::Cancelled);
            }
            output = &mut output => output.map_err(UvError::Spawn)?,
        };
        if !output.status.success() {
            return Err(UvError::Failed {
                command: args.join(" "),
                status: output.status,
                stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
            });
        }
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }
}

/// Sends SIGINT to `uv`, and says whether it could.
/// ponytail: uv alone, so a build backend it started runs on until it finishes. Signal the
/// process group if that ever matters.
#[cfg(unix)]
fn interrupt(pid: Option<u32>) -> bool {
    use rustix::process::{Pid, Signal, kill_process};
    pid.and_then(|pid| i32::try_from(pid).ok())
        .and_then(Pid::from_raw)
        .is_some_and(|pid| kill_process(pid, Signal::INT).is_ok())
}

/// A child on Windows cannot be sent Ctrl+C.
#[cfg(not(unix))]
fn interrupt(_pid: Option<u32>) -> bool {
    false
}

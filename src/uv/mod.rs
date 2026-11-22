//! The seam to the `uv` binary (ADR-0002): argv in, stdout or a `UvError` out.

pub mod args;
pub mod parse;

use std::fmt;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, BufReader};
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
    /// uview's own edit of pyproject.toml, which uv has no command for, failed (ADR-0009).
    Pyproject(String),
}

impl fmt::Display for UvError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UvError::Spawn(e) if e.kind() == std::io::ErrorKind::NotFound => {
                write!(
                    f,
                    "`uv` was not found on PATH: install it, as https://docs.astral.sh/uv/ says"
                )
            }
            UvError::Spawn(e) => write!(f, "could not run `uv`: {e}"),
            UvError::Failed {
                command,
                status,
                stderr,
            } => write!(f, "`uv {command}` failed ({status}): {stderr}"),
            UvError::Parse(msg) => write!(f, "could not read `uv` output: {msg}"),
            UvError::Cancelled => write!(f, "cancelled"),
            UvError::Pyproject(msg) => write!(f, "pyproject.toml: {msg}"),
        }
    }
}

impl std::error::Error for UvError {}

/// Every `uv` call goes through this trait: `RealUvClient` in the app, a fake in the tests.
/// Operations take it as `&impl UvClient`.
pub trait UvClient: Send + Sync {
    /// Runs `uv` with `args` and returns its stdout.
    fn run(&self, args: &[&str]) -> impl Future<Output = Result<String, UvError>> + Send;

    /// `run`, handing each line `uv` writes to stderr to `on_line` as it arrives, and ended with
    /// `UvError::Cancelled` once `cancel` fires (ADR-0006). By default there are no lines, and a
    /// cancelled call is simply dropped.
    fn run_streaming(
        &self,
        args: &[&str],
        cancel: &CancellationToken,
        _on_line: &(dyn Fn(&str) + Sync),
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

/// Runs the `uv` binary found on `PATH`, in the directory it was made for. The only code in
/// uview that starts a process.
#[derive(Clone, Debug)]
pub struct RealUvClient {
    dir: PathBuf,
    /// The credentials `uv publish` reads from the environment, kept out of every line uv writes.
    secrets: Vec<String>,
}

impl RealUvClient {
    /// A client that runs `uv` in `dir`, as a shell in it would. uview's own working directory
    /// is left alone.
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        let secrets = ["UV_PUBLISH_TOKEN", "UV_PUBLISH_PASSWORD"]
            .into_iter()
            .filter_map(|key| std::env::var(key).ok())
            .filter(|value| !value.is_empty())
            .collect();
        Self {
            dir: dir.into(),
            secrets,
        }
    }
}

impl RealUvClient {
    /// Why `uv` could not start. A directory to run it in that has gone fails the same way as a
    /// `uv` that is not on PATH, and would be told as one.
    fn spawn_error(&self, e: std::io::Error) -> UvError {
        if self.dir.is_dir() {
            return UvError::Spawn(e);
        }
        let gone = format!("the project's directory {} is gone", self.dir.display());
        UvError::Spawn(std::io::Error::other(gone))
    }
}

/// `line` with each of `secrets` in it replaced by `****`. uv prints no credentials; this is
/// in case it ever does.
pub fn scrub(line: &str, secrets: &[String]) -> String {
    secrets.iter().fold(line.to_string(), |line, secret| {
        line.replace(secret.as_str(), "****")
    })
}

impl UvClient for RealUvClient {
    async fn run(&self, args: &[&str]) -> Result<String, UvError> {
        self.run_streaming(args, &CancellationToken::new(), &|_| {})
            .await
    }

    /// Hands `on_line` each line of stderr, where `uv` reports its progress line by line when
    /// stderr is not a terminal. Once `cancel` fires, stops `uv` and whatever it started, a build
    /// backend or git, as Ctrl+C would: with SIGINT on Unix, on which `uv add` puts
    /// pyproject.toml back. Windows has no such signal for a child, so there they are killed.
    /// Returns once `uv` has exited, so what it left behind can be read.
    async fn run_streaming(
        &self,
        args: &[&str],
        cancel: &CancellationToken,
        on_line: &(dyn Fn(&str) + Sync),
    ) -> Result<String, UvError> {
        if cancel.is_cancelled() {
            return Err(UvError::Cancelled);
        }
        let mut command = tokio::process::Command::new("uv");
        command
            // Plain text: FORCE_COLOR would otherwise colour even a pipe.
            .args(["--color", "never"])
            .args(args)
            .current_dir(&self.dir)
            // main.rs already turned UV_PROJECT into the directory uv runs in; left set, uv
            // would resolve a relative value a second time, from there.
            .env_remove("UV_PROJECT")
            // Kept off the terminal the app draws on.
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            // If this future is dropped, `uv` must not outlive it.
            .kill_on_drop(true);
        // Its own process group, so a cancel reaches what it started too.
        #[cfg(unix)]
        command.process_group(0);
        let mut child = command.spawn().map_err(|e| self.spawn_error(e))?;
        let pid = child.id();
        let (stdout, stderr) = (child.stdout.take(), child.stderr.take());
        let output = async {
            tokio::try_join!(
                child.wait(),
                read_all(stdout),
                read_lines(stderr, on_line, &self.secrets)
            )
        };
        tokio::pin!(output);
        let (status, stdout, stderr) = tokio::select! {
            biased;
            () = cancel.cancelled() => {
                // `uv add` puts pyproject.toml back, then exits; other commands stop at once.
                interrupt(pid).await;
                if tokio::time::timeout(Duration::from_secs(2), &mut output).await.is_err() {
                    kill(pid);
                    let _ = tokio::time::timeout(Duration::from_secs(2), output).await;
                }
                return Err(UvError::Cancelled);
            }
            output = &mut output => output.map_err(UvError::Spawn)?,
        };
        if !status.success() {
            return Err(UvError::Failed {
                command: args.join(" "),
                status,
                stderr: stderr.trim().to_string(),
            });
        }
        Ok(String::from_utf8_lossy(&stdout).into_owned())
    }
}

/// Everything `pipe` holds until it closes.
async fn read_all(pipe: Option<impl AsyncRead + Unpin>) -> std::io::Result<Vec<u8>> {
    let mut all = Vec::new();
    if let Some(mut pipe) = pipe {
        pipe.read_to_end(&mut all).await?;
    }
    Ok(all)
}

/// Everything `pipe` holds until it closes, `secrets` scrubbed, handing each line that is not
/// blank to `on_line`, trimmed, as it arrives.
async fn read_lines(
    pipe: Option<impl AsyncRead + Unpin>,
    on_line: &(dyn Fn(&str) + Sync),
    secrets: &[String],
) -> std::io::Result<String> {
    let mut all = String::new();
    let Some(pipe) = pipe else {
        return Ok(all);
    };
    let mut lines = BufReader::new(pipe).split(b'\n');
    while let Some(line) = lines.next_segment().await? {
        let line = scrub(&String::from_utf8_lossy(&line), secrets);
        // The indent stays: uv lines up its error blocks with it.
        if !line.trim().is_empty() {
            on_line(line.trim_end());
        }
        all.push_str(&line);
        all.push('\n');
    }
    Ok(all)
}

/// Sends SIGINT to `uv` and what it started, as Ctrl+C in a shell would.
#[cfg(unix)]
async fn interrupt(pid: Option<u32>) {
    signal_group(pid, rustix::process::Signal::INT);
}

/// Kills what is left of `uv`'s process group.
#[cfg(unix)]
fn kill(pid: Option<u32>) {
    signal_group(pid, rustix::process::Signal::KILL);
}

#[cfg(unix)]
fn signal_group(pid: Option<u32>, signal: rustix::process::Signal) {
    use rustix::process::{Pid, kill_process_group};
    if let Some(pid) = pid
        .and_then(|pid| i32::try_from(pid).ok())
        .and_then(Pid::from_raw)
    {
        let _ = kill_process_group(pid, signal);
    }
}

/// A child on Windows cannot be sent Ctrl+C, so `uv` and every process under it are killed.
/// A `uv add` killed after it wrote pyproject.toml leaves it so, with uv.lock behind.
#[cfg(windows)]
async fn interrupt(pid: Option<u32>) {
    let Some(pid) = pid else { return };
    let _ = tokio::process::Command::new("taskkill")
        .args(["/T", "/F", "/PID", &pid.to_string()])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .status()
        .await;
}

/// Killed already, by `interrupt`.
#[cfg(windows)]
fn kill(_pid: Option<u32>) {}

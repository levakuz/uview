//! The seam to the `uv` binary (ADR-0002): argv in, stdout or a `UvError` out.

pub mod args;
pub mod parse;

use std::fmt;

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
        }
    }
}

impl std::error::Error for UvError {}

/// Every `uv` call goes through this trait: `RealUvClient` in the app, a fake in the tests.
/// Operations take it as `&impl UvClient`.
pub trait UvClient: Send + Sync {
    /// Runs `uv` with `args` and returns its stdout.
    fn run(&self, args: &[&str]) -> impl Future<Output = Result<String, UvError>> + Send;
}

/// Runs the `uv` binary found on `PATH`. The only code in uview that starts a process.
#[derive(Clone, Copy)]
pub struct RealUvClient;

impl UvClient for RealUvClient {
    async fn run(&self, args: &[&str]) -> Result<String, UvError> {
        let output = tokio::process::Command::new("uv")
            .args(args)
            // main.rs already turned UV_PROJECT into the working directory; left set, uv would
            // resolve a relative value a second time, from there.
            .env_remove("UV_PROJECT")
            .output()
            .await
            .map_err(UvError::Spawn)?;
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

//! Publishing what the last build wrote with `uv publish`. uview handles no credentials: uv
//! finds its own, in its environment or a keyring.

use std::path::PathBuf;

use crate::domain::{Artifact, PublishTarget};
use crate::uv::{UvClient, UvError, args};

/// Where uv will take credentials from for `uv publish`, as `env` reads its environment: the
/// variable's name, never its value, or `None` when it will find none.
pub fn credentials(env: impl Fn(&str) -> Option<String>) -> Option<&'static str> {
    let set = |key| env(key).is_some_and(|value| !value.is_empty());
    if set("UV_PUBLISH_TOKEN") {
        Some("UV_PUBLISH_TOKEN")
    } else if set("UV_PUBLISH_PASSWORD") {
        // Without a username, uv sends `__token__`.
        Some("UV_PUBLISH_PASSWORD")
    } else if env("UV_KEYRING_PROVIDER").as_deref() == Some("subprocess") {
        Some("keyring")
    } else {
        None
    }
}

/// Uploads `files` to `target`.
pub async fn publish(
    uv: &impl UvClient,
    target: &PublishTarget,
    files: &[Artifact],
) -> Result<(), UvError> {
    let paths: Vec<PathBuf> = files.iter().map(|file| file.path.clone()).collect();
    let args = args::publish_args(target, &paths);
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    uv.run(&args).await?;
    Ok(())
}

/// What the status line says once `files` are on `target`.
pub fn published_notice(files: &[Artifact], target: &PublishTarget) -> String {
    let names: Vec<String> = files.iter().map(Artifact::name).collect();
    match names.len() {
        1 | 2 => format!("Published {} to {}", names.join(", "), target.label()),
        n => format!("Published {n} files to {}", target.label()),
    }
}

//! The project's environment itself: what the tile says of it, and making it anew, emptying it
//! or deleting it. uv has no command to delete one, so that alone is done here (ADR-0015).

use std::path::{Component, Path};

use crate::controllers::project::ProjectPaths;
use crate::domain::{EnvInfo, PythonOption};
use crate::uv::{UvClient, UvError, args, parse};

/// `target` as reached from `base`, both absolute: `.venv` under it, `../../.venv` above it.
pub fn relative_to(target: &Path, base: &Path) -> String {
    let target: Vec<Component> = target.components().collect();
    let base: Vec<Component> = base.components().collect();
    let shared = target.iter().zip(&base).take_while(|(t, b)| t == b).count();
    let up = std::iter::repeat_n("..".to_string(), base.len() - shared);
    let down = target[shared..]
        .iter()
        .map(|c| c.as_os_str().to_string_lossy().into_owned());
    let parts: Vec<String> = up.chain(down).collect();
    if parts.is_empty() {
        ".".into()
    } else {
        parts.join("/")
    }
}

/// The Python version `pyvenv.cfg` records, from its `version_info` (or `version`) line.
pub fn python_version(pyvenv_cfg: &str) -> Option<String> {
    pyvenv_cfg.lines().find_map(|line| {
        let (key, value) = line.split_once('=')?;
        matches!(key.trim(), "version_info" | "version").then(|| value.trim().to_string())
    })
}

/// The apparent size of what is under `dir`: the lengths of its regular files. A symlink counts
/// as itself, not as what it points to, so `bin/python` adds nothing of the interpreter.
pub fn apparent_size(dir: &Path) -> std::io::Result<u64> {
    let mut size = 0;
    let mut todo = vec![dir.to_path_buf()];
    while let Some(dir) = todo.pop() {
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let kind = entry.file_type()?;
            if kind.is_dir() {
                todo.push(entry.path());
            } else if kind.is_file() {
                size += entry.metadata()?.len();
            }
        }
    }
    Ok(size)
}

/// What the tile says of the environment, or `None` when there is none.
pub async fn load_env_info(paths: &ProjectPaths) -> Option<EnvInfo> {
    if !paths.env.is_dir() {
        return None;
    }
    let cfg = tokio::fs::read_to_string(paths.env.join("pyvenv.cfg"))
        .await
        .ok();
    let python = cfg.as_deref().and_then(python_version);
    // Only an environment is measured: `UV_PROJECT_ENVIRONMENT` set to the project root would
    // walk all of it on every reload. A large one is many files, so off the async threads.
    let size = match cfg {
        Some(_) => {
            let env = paths.env.clone();
            tokio::task::spawn_blocking(move || apparent_size(&env).ok())
                .await
                .ok()
                .flatten()
        }
        None => None,
    };
    Some(EnvInfo {
        shown_path: relative_to(&paths.env, &paths.root),
        python,
        size,
    })
}

/// Every Python uv can find or download, once each: uv lists one interpreter under every path
/// that reaches it, `python3` and `python3.13` alike, all with the one key `--python` takes.
pub async fn list_pythons(uv: &impl UvClient) -> Result<Vec<PythonOption>, UvError> {
    let mut pythons = parse::python_list(&uv.run(&args::PYTHON_LIST).await?)?;
    let mut seen = std::collections::HashSet::new();
    pythons.retain(|p| seen.insert(p.key.clone()));
    Ok(pythons)
}

/// Makes the environment anew with `python`, and installs the project in it. A `.python-version`
/// at `root` would take the next sync back to the Python it names, so it is pinned there too;
/// returns whether it was.
pub async fn recreate(
    uv: &impl UvClient,
    python: &PythonOption,
    root: &Path,
) -> Result<bool, UvError> {
    uv.run(&args::sync_python_args(&python.key)).await?;
    if !root.join(".python-version").is_file() {
        return Ok(false);
    }
    uv.run(&args::python_pin_args(&python.request())).await?;
    Ok(true)
}

/// Empties the environment at `env`, then installs the project in it again.
pub async fn reset(uv: &impl UvClient, env: &Path) -> Result<(), UvError> {
    uv.run(&args::venv_clear_args(&env.to_string_lossy()))
        .await?;
    uv.run(&args::SYNC).await?;
    Ok(())
}

/// Why `env` must not be deleted, if anything: it must be an environment, holding
/// `pyvenv.cfg`, and neither the project's root nor a directory above it, whatever
/// `UV_PROJECT_ENVIRONMENT` says.
pub fn refusal_to_delete(env: &Path, root: &Path) -> Option<String> {
    if !env.join("pyvenv.cfg").is_file() {
        return Some(format!(
            "{env:?} holds no pyvenv.cfg, so it is no environment to delete"
        ));
    }
    // As the filesystem resolves them: `/a/b/c/../..` is above `/a/b/c`, though not by its
    // spelling.
    let resolved = |path: &Path| std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    if resolved(root).starts_with(resolved(env)) {
        return Some(format!("{env:?} holds the project itself"));
    }
    None
}

/// Deletes the project's environment directory, after the checks of `refusal_to_delete`.
pub async fn delete_env(env: &Path, root: &Path) -> Result<(), String> {
    if let Some(refusal) = refusal_to_delete(env, root) {
        return Err(refusal);
    }
    tokio::fs::remove_dir_all(env)
        .await
        .map_err(|e| format!("could not delete {env:?}: {e}"))
}

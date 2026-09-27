use std::process::{Command, Stdio};
use uview::controllers::project::find_project_root;

#[test]
fn outside_a_project_exits_before_taking_the_terminal() -> std::io::Result<()> {
    let dir = std::env::temp_dir().join(format!("uview-cli-{}", std::process::id()));
    std::fs::create_dir_all(&dir)?;
    if find_project_root(&dir).is_some() {
        // A pyproject.toml above the temp directory: uview would start the TUI instead.
        eprintln!("skipped: {dir:?} is inside a project");
        return std::fs::remove_dir_all(&dir);
    }
    let output = Command::new(env!("CARGO_BIN_EXE_uview"))
        .current_dir(&dir)
        .env_remove("UV_PROJECT")
        .stdin(Stdio::null())
        .output()?;
    std::fs::remove_dir_all(&dir)?;

    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("uv init"));
    // Taking the terminal first would show here as the switch to the alternate screen.
    assert!(output.stdout.is_empty());
    Ok(())
}

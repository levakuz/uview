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

#[test]
fn version_and_help_are_answered_without_the_terminal() -> std::io::Result<()> {
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_uview"))
            .args(args)
            .stdin(Stdio::null())
            .output()
    };
    let version = run(&["--version"])?;
    assert!(version.status.success());
    // The one version, Cargo.toml's, which the Python package takes too.
    assert_eq!(
        String::from_utf8_lossy(&version.stdout).trim(),
        format!("uview {}", env!("CARGO_PKG_VERSION"))
    );
    assert_eq!(run(&["-V"])?.stdout, version.stdout);
    let help = run(&["--help"])?;
    assert!(help.status.success());
    assert!(String::from_utf8_lossy(&help.stdout).contains("Usage: uview"));
    let wrong = run(&["--nope"])?;
    assert_eq!(wrong.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&wrong.stderr).contains("unexpected argument \"--nope\""));
    Ok(())
}

#[test]
fn in_a_project_without_a_terminal_it_says_so_rather_than_crash() -> std::io::Result<()> {
    let dir = std::env::temp_dir().join(format!("uview-cli-tty-{}", std::process::id()));
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join("pyproject.toml"), "[project]\nname = \"demo\"\n")?;
    let output = Command::new(env!("CARGO_BIN_EXE_uview"))
        .current_dir(&dir)
        .env_remove("UV_PROJECT")
        .stdin(Stdio::null())
        .output()?;
    std::fs::remove_dir_all(&dir)?;
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("draws on a terminal"), "{stderr}");
    assert!(!stderr.contains("panicked"));
    Ok(())
}

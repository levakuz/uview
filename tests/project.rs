use std::fs;
use std::path::Path;

use uview::controllers::project::{
    find_project_root, init_project, project_environment, refusal, workspace_root,
};
use uview::uv::RealUvClient;
use uview::uv::args::init_args;

#[test]
fn finds_the_nearest_pyproject_toml_upwards() -> std::io::Result<()> {
    let base = std::env::temp_dir().join(format!("uview-project-root-{}", std::process::id()));
    let outer = base.join("outer");
    let project = outer.join("proj");
    let nested = project.join("src").join("pkg");
    fs::create_dir_all(&nested)?;
    fs::write(outer.join("pyproject.toml"), "")?;
    fs::write(project.join("pyproject.toml"), "")?;
    // A directory of that name is not a pyproject.toml.
    fs::create_dir(nested.join("pyproject.toml"))?;

    let from_subdirectory = find_project_root(&nested);
    let from_root = find_project_root(&project);
    let through_parent_dirs = find_project_root(&nested.join("..").join(".."));
    let from_outside = find_project_root(&base);
    fs::remove_dir_all(&base)?;

    assert_eq!(from_subdirectory, Some(project.clone()));
    assert_eq!(from_root, Some(project.clone()));
    assert_eq!(through_parent_dirs, Some(project));
    // What lies above the temp directory depends on the machine; nothing inside `base` may match.
    assert!(!from_outside.is_some_and(|root| root.starts_with(&base)));
    Ok(())
}

#[test]
fn refuses_the_projects_uv_refuses() -> std::io::Result<()> {
    let base = std::env::temp_dir().join(format!("uview-refusal-{}", std::process::id()));
    fs::create_dir_all(&base)?;
    let refused = |toml: &str| -> std::io::Result<bool> {
        fs::write(base.join("pyproject.toml"), toml)?;
        Ok(refusal(&base).is_some())
    };
    let tool_config_only = refused("[tool.ruff]\nline-length = 100\n")?;
    let project = refused("[project]\nname = \"p\"\n")?;
    let virtual_workspace = refused("[tool.uv.workspace]\nmembers = []\n")?;
    let unmanaged = refused("[project]\nname = \"p\"\n[tool.uv]\nmanaged = false\n")?;
    fs::remove_dir_all(&base)?;

    assert!(tool_config_only);
    assert!(!project);
    assert!(!virtual_workspace);
    assert!(unmanaged);
    Ok(())
}

/// A pyproject.toml in `dir` for a project named after it, with `extra` appended.
fn project(dir: &Path, extra: &str) -> std::io::Result<()> {
    fs::create_dir_all(dir)?;
    let name = dir
        .file_name()
        .map_or("p".into(), |name| name.to_string_lossy());
    fs::write(
        dir.join("pyproject.toml"),
        format!("[project]\nname = \"{name}\"\nversion = \"0.1.0\"\n{extra}"),
    )
}

#[test]
fn a_workspace_member_has_the_workspace_root_and_others_their_own() -> std::io::Result<()> {
    // Brackets, which a glob would read as a class, unless the root is escaped as uv escapes it.
    let base = std::env::temp_dir().join(format!("uview-workspace-[{}]", std::process::id()));
    let ws = base.join("ws");
    project(
        &ws,
        "[tool.uv.workspace]\nmembers = [\"packages/*\"]\nexclude = [\"packages/excluded\"]\n",
    )?;
    let member = ws.join("packages").join("a");
    let excluded = ws.join("packages").join("excluded");
    let unmatched = ws.join("other").join("b");
    // Behind a plain project: uv looks no further than the nearest pyproject.toml above.
    let behind_a_project = member.join("examples").join("demo");
    // Matched, but a workspace of its own.
    let own_workspace = ws.join("packages").join("own");
    // Two levels below `packages`: uv's `*` crosses `/`.
    let deeper = ws.join("packages").join("group").join("deep");
    for dir in [&member, &excluded, &unmatched, &behind_a_project, &deeper] {
        project(dir, "")?;
    }
    project(&own_workspace, "[tool.uv.workspace]\nmembers = []\n")?;
    // A virtual workspace root, with no [project] of its own.
    let virtual_root = base.join("virtual");
    fs::create_dir_all(&virtual_root)?;
    fs::write(
        virtual_root.join("pyproject.toml"),
        "[tool.uv.workspace]\nmembers = [\"libs/*\"]\n",
    )?;
    let library = virtual_root.join("libs").join("lib");
    project(&library, "")?;

    let roots = [
        &member,
        &excluded,
        &unmatched,
        &behind_a_project,
        &own_workspace,
        &deeper,
        &ws,
        &library,
    ]
    .map(|project| workspace_root(project));
    // Launched below a member's root, uview finds the member first, as uv does.
    let from_below = find_project_root(&member.join("src")).map(|project| workspace_root(&project));
    fs::remove_dir_all(&base)?;

    assert_eq!(
        roots,
        [
            ws.clone(),
            excluded,
            unmatched,
            behind_a_project,
            own_workspace,
            ws.clone(),
            ws.clone(),
            virtual_root,
        ]
    );
    assert_eq!(from_below, Some(ws));
    Ok(())
}

#[test]
fn the_environment_is_taken_from_the_workspace_root() -> std::io::Result<()> {
    let base = std::env::temp_dir().join(format!("uview-environment-{}", std::process::id()));
    project(&base, "[tool.uv.workspace]\nmembers = [\"a\"]\n")?;
    let member = base.join("a");
    project(&member, "")?;

    let default = project_environment(&member, None);
    let unset = project_environment(&member, Some(""));
    let relative = project_environment(&member, Some("envs/dev"));
    // Absolute on this platform: `/opt/env` is not, on Windows.
    let elsewhere = std::env::temp_dir().join("uview-env-elsewhere");
    let absolute = project_environment(&member, elsewhere.to_str());
    fs::remove_dir_all(&base)?;

    assert_eq!(default, base.join(".venv"));
    assert_eq!(unset, base.join(".venv"));
    assert_eq!(relative, base.join("envs").join("dev"));
    assert_eq!(absolute, elsewhere);
    Ok(())
}

#[test]
fn init_names_the_project_only_when_asked_to() {
    assert_eq!(
        init_args("/home/jane/acme", None),
        ["init", "/home/jane/acme"]
    );
    assert_eq!(
        init_args("/home/jane/acme", Some("acme-api")),
        ["init", "--name", "acme-api", "/home/jane/acme"]
    );
}

#[tokio::test]
async fn uv_init_makes_a_project_uview_opens() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::env::temp_dir().join(format!("uview-init-{}", std::process::id()));
    fs::create_dir_all(&dir)?;
    if find_project_root(&dir).is_some() {
        eprintln!("skipped: {dir:?} is inside a project");
        return Ok(fs::remove_dir_all(&dir)?);
    }
    let created = init_project(&RealUvClient::new(&dir), &dir, Some("acme-api")).await;
    let root = find_project_root(&dir);
    let refused = refusal(&dir);
    let pyproject = fs::read_to_string(dir.join("pyproject.toml")).unwrap_or_default();
    fs::remove_dir_all(&dir)?;

    created?;
    assert_eq!(root, Some(dir));
    assert_eq!(refused, None);
    assert!(pyproject.contains("name = \"acme-api\""), "{pyproject}");
    Ok(())
}

/// Answers `uv --version` with `said`.
struct UvSays(&'static str);

impl uview::uv::UvClient for UvSays {
    async fn run(&self, _: &[&str]) -> Result<String, uview::uv::UvError> {
        Ok(self.0.to_string())
    }
}

#[tokio::test]
async fn a_uv_older_than_uview_needs_is_told() {
    use uview::controllers::project::uv_too_old;
    assert_eq!(
        uv_too_old(&UvSays("uv 0.7.22 (Homebrew 2025-07-17)\n"))
            .await
            .as_deref(),
        Some("uv 0.7.22 is older than uview needs, 0.8.0: update it, as with `uv self update`")
    );
    assert_eq!(uv_too_old(&UvSays("uv 0.8.0\n")).await, None);
    assert_eq!(
        uv_too_old(&UvSays("uv 0.10.3 (abc 2026-10-01)\n")).await,
        None
    );
    // What cannot be read is no reason to complain.
    assert_eq!(uv_too_old(&UvSays("something else")).await, None);
}

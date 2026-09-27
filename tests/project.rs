use std::fs;
use uview::controllers::project::{find_project_root, refusal};

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

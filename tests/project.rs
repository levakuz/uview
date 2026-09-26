use std::fs;
use uview::controllers::project::find_project_root;

#[test]
fn finds_the_nearest_pyproject_toml_upwards() -> std::io::Result<()> {
    let base = std::env::temp_dir().join(format!("uview-project-root-{}", std::process::id()));
    let project = base.join("proj");
    let nested = project.join("src").join("pkg");
    fs::create_dir_all(&nested)?;
    fs::write(project.join("pyproject.toml"), "")?;

    let from_subdirectory = find_project_root(&nested);
    let from_outside = find_project_root(&base);
    fs::remove_dir_all(&base)?;

    assert_eq!(from_subdirectory, Some(project));
    assert_eq!(from_outside, None);
    Ok(())
}
